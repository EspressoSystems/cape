//! This crate describes the workflow and interfaces of a CAPE contract deployed on Ethereum.

use ethers::prelude::*;
use jf_txn::keys::UserPubKey;
use jf_txn::structs::{AssetDefinition, FreezeFlag, Nullifier, RecordCommitment, RecordOpening};
use jf_txn::{MerkleCommitment, MerkleFrontier, NodeValue, TransactionNote};
use std::collections::{HashMap, HashSet, LinkedList};

mod constants;
mod erc20;
mod merkle_tree;
mod relayer;
use crate::constants::{burn_pub_key, MERKLE_ROOT_QUEUE_CAP};
use crate::erc20::Erc20Contract;
use crate::merkle_tree::RecordMerkleTree;

/// a block in CAPE blockchain
#[derive(Default, Clone)]
pub struct CapeBlock {
    // NOTE: separated out list of burn transaction
    pub(crate) burn_txns: Vec<TransactionNote>,
    // rest of the transactions (except burn txn)
    pub(crate) txns: Vec<TransactionNote>,
    // fee_blind: BlindFactor,
    pub(crate) miner: UserPubKey,
    // targeting block height
    pub(crate) block_height: u64,
}

impl CapeBlock {
    // Refer to `validate_block()` in:
    // https://gitlab.com/translucence/crypto/jellyfish/-/blob/main/transactions/tests/examples.rs
    pub fn validate(&self) -> bool {
        // Primarily a Plonk proof verification!
        true
    }
}

pub struct CapeContract {
    // set of spent records' nullifiers, stored as mapping in contract
    nullifiers: HashSet<Nullifier>,
    // latest block height
    height: u64,
    // TODO: should we further hash MerkleCommitment and only store a bytes32 in contract?
    // latest record merkle tree commitment (including merkle root, tree height and num of leaves)
    merkle_commitment: MerkleCommitment,
    // The merkle frontier is stored locally
    mt_frontier: MerkleFrontier,
    // last X merkle root, allowing transaction building against recent merkle roots (instead of just
    // the latest merkle root) as a buffer.
    // where X is the capacity the Queue and can be specified during constructor. (rust doesn't have queue
    // so we use LinkedList to simulate)
    // NOTE: in Solidity, we can instantiate with a fixed array and an indexer to build a FIFO queue.
    recent_merkle_roots: LinkedList<NodeValue>,
    // NOTE: in Solidity impl, we should use `keccak256(abi.encode(AssetDefinition))` as the mapping key
    wrapped_erc20_registrar: HashMap<AssetDefinition, Address>,
    pending_deposit_queue: Vec<RecordCommitment>,
}

impl CapeContract {
    pub(crate) fn address(&self) -> Address {
        // NOTE: in Solidity, use expression: `address(this)`
        Address::from_low_u64_le(666u64)
    }

    /// getter for registrar
    pub fn is_cape_asset_registered(&self, asset_def: &AssetDefinition) -> bool {
        self.wrapped_erc20_registrar.contains_key(asset_def)
    }

    /// Create a new asset type for an ERC20 and register it to the contract.
    pub fn sponsor_cape_asset(&mut self, erc20_addr: Address, new_asset: AssetDefinition) {
        assert!(
            !self.is_cape_asset_registered(&new_asset),
            "this CAPE asset is already registered"
        );
        // check correct ERC20 address.
        let _ = Erc20Contract::at(erc20_addr);

        self.wrapped_erc20_registrar.insert(new_asset, erc20_addr);
    }

    // NOTE: in Solidity, we can
    // - avoid passing in `ro.freeze_flag` (to save a little gas?)
    // - remove `depositor` from input parameters, and directly replaced with `msg.sender`
    pub fn deposit_erc20(&mut self, ro: RecordOpening, erc20_addr: Address, depositor: Address) {
        let mut erc20_contract = Erc20Contract::at(erc20_addr);

        // 1. verify matching registered CAPE asset and the erc20 address
        assert_eq!(
            self.wrapped_erc20_registrar.get(&ro.asset_def).unwrap(),
            &erc20_addr,
            "Mismatched `erc20_addr` for the CAPE record."
        );

        // 1.1 (optional) more sanity check on user provided CAPE asset record,
        // may help prevent users from crediting into some unspendable record or waste more gas.
        assert!(ro.amount > 0);
        assert_ne!(ro.pub_key, UserPubKey::default()); // this would be EC point equality check
        assert_eq!(ro.freeze_flag, FreezeFlag::Unfrozen); // just a boolean flag

        // 2. attempt to `transferFrom` before mutating contract state to mitigate reentrancy attack
        erc20_contract.transfer_from(depositor, self.address(), U256::from(ro.amount));

        // 3. compute record commitment
        // this requires implementing `RecordOpening::derive_record_commitment()` function in Solidity
        // which uses Rescue-based Commitment over the record opening.
        let rc = RecordCommitment::from(&ro);

        // 4. append the commitment to pending queue to be inserted into the MT by relayer.
        self.pending_deposit_queue.push(rc);
    }

    /// Relayer submits the next block, and withdraw for users who had burn transactions included
    /// in `new_block` with the help of record openings of the "burned records" (output of the burn
    /// transaction) submitted by user.
    pub fn submit_cape_block(&mut self, new_block: CapeBlock, burned_ros: Vec<RecordOpening>) {
        // 1. verify the block, and insert its input nullifiers and output record commitments
        assert!(new_block.validate());
        assert!(
            new_block
                .txns
                .iter()
                .chain(new_block.burn_txns.iter())
                .all(|txn| self.recent_merkle_roots.contains(&txn.merkle_root())),
            "should produce txn validity proof against recent merkle root",
        );
        assert_eq!(new_block.block_height, self.height + 1); // targeting the next block

        let mut rc_to_be_inserted = vec![];
        for txn in new_block.txns.iter() {
            for &nf in txn.nullifiers().iter() {
                self.nullifiers.insert(nf);
            }
            rc_to_be_inserted.extend_from_slice(&txn.output_commitments());
        }

        // 2. process all burn transaction and withdraw for user immediately
        assert_eq!(new_block.burn_txns.len(), burned_ros.len());
        for (burn_txn, burned_ro) in new_block.burn_txns.iter().zip(burned_ros.iter()) {
            // burn transaction is basically a "Transfer-to-dedicated-burn-pk" transaction
            if let TransactionNote::Transfer(note) = burn_txn {
                // 2.1. validate the burned record opening against the burn transaction.
                let withdraw_amount = burned_ro.amount;
                // get recipient address from txn's proof bounded data field
                let recipient = {
                    let mut proof_bounded_address = [0u8; 20];
                    proof_bounded_address
                        .copy_from_slice(&note.aux_info.extra_proof_bound_data[..20]);
                    Address::from(proof_bounded_address)
                };

                // to validate the burn transaction, we need to check if the second output
                // of the transfer (while the first being fee change) is sent to dedicated
                // "burn address/pubkey". Since the contract only have record commitments,
                // we require the user to provide the record opening and check against it.
                assert_eq!(
                    RecordCommitment::from(burned_ro),
                    burn_txn.output_commitments()[1]
                );
                assert_eq!(burned_ro.pub_key, burn_pub_key());
                let erc20_addr = self
                    .wrapped_erc20_registrar
                    .get(&burned_ro.asset_def)
                    .unwrap();

                // 2.2. upon successful verification, execute the withdraw for user
                let mut erc20_contract = Erc20Contract::at(*erc20_addr);
                erc20_contract.transfer(recipient, U256::from(withdraw_amount));

                // 2.3. like other txn, insert input nullifiers and output record commitments
                for &nf in burn_txn.nullifiers().iter() {
                    self.nullifiers.insert(nf);
                }
                rc_to_be_inserted.extend_from_slice(&burn_txn.output_commitments());
            } else {
                panic!("burn txn should be of TransferNote type");
            }
        }

        // 3. insert all pending deposits AND output records from the txns into record merkle tree
        // and update the merkle root/commitment.
        rc_to_be_inserted.extend_from_slice(&self.pending_deposit_queue);
        let (updated_mt_comm, updated_mt_frontier) =
            self.batch_insert_with_frontier(self.mt_frontier.clone(), &rc_to_be_inserted);

        // 4. update the blockchain state digest
        self.height += 1;
        self.merkle_commitment = updated_mt_comm;
        if self.recent_merkle_roots.len() == MERKLE_ROOT_QUEUE_CAP {
            self.recent_merkle_roots.pop_front(); // remove the oldest root
        }
        self.recent_merkle_roots
            .push_back(updated_mt_comm.root_value); // add the new root

        // Store the new frontier
        self.mt_frontier = updated_mt_frontier;
    }
}

#[cfg(test)]
mod test {
    use jf_txn::{
        keys::{AuditorKeyPair, FreezerKeyPair, UserKeyPair},
        structs::{AssetCode, AssetCodeSeed, AssetPolicy, FreezeFlag},
        transfer::TransferNote,
        NodeValue,
    };

    use super::*;
    use crate::relayer::Relayer;
    use constants::*;

    impl CapeContract {
        // return a mocked contract with some pre-filled states.
        fn mock() -> Self {
            Self {
                nullifiers: HashSet::default(),
                height: 0,
                merkle_commitment: MerkleCommitment {
                    root_value: NodeValue::empty_node_value(),
                    height: 20,
                    num_leaves: 0,
                },
                mt_frontier: MerkleFrontier,
                recent_merkle_roots: LinkedList::default(),
                wrapped_erc20_registrar: HashMap::default(),
                pending_deposit_queue: vec![],
            }
        }
    }

    impl CapeBlock {
        // build the next block
        fn build_next() -> Self {
            Self::default()
        }
    }

    fn usdc_cape_asset_def() -> AssetDefinition {
        let mut rng = rand::thread_rng();
        let asset_code_seed = AssetCodeSeed::generate(&mut rng);
        let asset_code = AssetCode::new(asset_code_seed, b"Official wrapped USDC in CAPE system.");
        let usdc_freezer = FreezerKeyPair::generate(&mut rng);
        let usdc_auditor = AuditorKeyPair::generate(&mut rng);
        // USDC have freezer, auditor, but temporarily leave credential issuer as empty
        // and only reveal record opening (for freezing) but not any id attributes.
        let asset_policy = AssetPolicy::default()
            .set_freezer_pub_key(usdc_freezer.pub_key())
            .set_auditor_pub_key(usdc_auditor.pub_key())
            .reveal_record_opening()
            .unwrap();
        AssetDefinition::new(asset_code, asset_policy).unwrap()
    }

    fn generate_burn_transaction(_ro: &RecordOpening) -> TransferNote {
        // internally call `transfer.rs::generate_non_native()`
        // for simplicity, we skip preparing for all input params.
        unimplemented!();
    }

    #[test]
    #[ignore = "ignore panic due to unimplemented logic"]
    fn asset_registration_workflow() {
        let mut cape_contract = CapeContract::mock();
        // 1. sponser: design  the CAPE asset type (off-chain).
        let asset_def = usdc_cape_asset_def();

        // 2. sponser: register the asset (on-L1-chain).
        cape_contract.sponsor_cape_asset(usdc_address(), asset_def.clone());
    }

    #[test]
    #[ignore = "ignore panic due to unimplemented logic"]
    fn wrap_workflow() {
        let mut rng = rand::thread_rng();
        let mut cape_contract = CapeContract::mock();
        let mut usdc_contract = Erc20Contract::at(usdc_address());

        let cape_user_keypair = UserKeyPair::generate(&mut rng);
        let eth_user_address = Address::random();
        let relayer = Relayer::new();

        // 1. user: fetch the CAPE asset definition from UI or sponsor.
        let asset_def = usdc_cape_asset_def();

        // 2. user: invoke ERC20's approve (on-L1-chain)
        //
        // NOTE: it's probably a good idea to add u64 overflow check at UI level so that
        // users won't deposit more than u64::MAX, since our CAPE code base are dealing with amount
        // in `u64` whereas Solidity is dealing with `U256`.
        let deposit_amount = U256::from(1000);
        usdc_contract.approve(cape_contract.address(), deposit_amount);

        // 3. user: prepare the CAPE Record Opening, then invoke wrapper's deposit erc20
        let ro = RecordOpening::new(
            &mut rng,
            deposit_amount.as_u64(),
            asset_def,
            cape_user_keypair.pub_key(),
            FreezeFlag::Unfrozen,
        );
        cape_contract.deposit_erc20(ro, usdc_address(), eth_user_address);

        // 4. relayer: build next block and user's deposit will be credited (inserted into
        // record merkle tree) when CAPE contract process the next valid block.
        let new_block = CapeBlock::build_next();
        cape_contract.submit_cape_block(new_block, relayer.mt.frontier(), vec![]);
    }

    #[test]
    #[ignore = "ignore panic due to unimplemented logic"]
    fn unwrap_workflow() {
        let mut rng = rand::thread_rng();
        let mut cape_contract = CapeContract::mock();
        let cape_user_keypair = UserKeyPair::generate(&mut rng);
        let relayer = Relayer::new();

        // 1. user: build and send a burn transaction to relayer (off-chain)
        let asset_def = usdc_cape_asset_def();
        let burn_amount = 1000;
        let burned_ro = RecordOpening::new(
            &mut rng,
            burn_amount,
            asset_def,
            cape_user_keypair.pub_key(),
            FreezeFlag::Unfrozen,
        );
        let burn_txn = generate_burn_transaction(&burned_ro);

        // 2. user: send over the burn_txn and burned_ro to relayer.

        // 3. relayer: build a new block containing the burn txn broadcasted by the user (off-chain)
        let mut new_block = CapeBlock::build_next();
        new_block
            .burn_txns
            .push(TransactionNote::Transfer(Box::new(burn_txn)));
        let mut burned_ros = vec![];
        burned_ros.push(burned_ro);

        cape_contract.submit_cape_block(new_block.clone(), relayer.mt.frontier(), burned_ros);

        // done! when CAPE contract process a new block,
        // it will automatically withdraw for user that has submitted the burn transaction.
    }
}
