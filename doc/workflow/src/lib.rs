//! This crate describes the workflow and interfaces of a CAPE contract deployed on Ethereum.

use ethers::prelude::*;
use jf_txn::keys::UserPubKey;
use jf_txn::structs::{AssetDefinition, FreezeFlag, Nullifier, RecordCommitment, RecordOpening};
use jf_txn::{MerkleCommitment, MerkleFrontier, TransactionNote};
use std::collections::{HashMap, HashSet};

mod constants;
mod erc20;
mod merkle_tree;
mod relayer;
use crate::constants::burn_pub_key;
use crate::erc20::Erc20Contract;
use crate::merkle_tree::RecordMerkleTree;

/// a block in CAPE blockchain
#[derive(Default, Clone)]
pub struct CapeBlock {
    txns: Vec<TransactionNote>,
    // fee_blind: BlindFactor,
    miner: UserPubKey,
}

impl CapeBlock {
    // Refer to `validate_block()` in:
    // https://gitlab.com/translucence/crypto/jellyfish/-/blob/main/transactions/tests/examples.rs
    pub fn verify(&self) -> bool {
        // Prmarily a Plonk proof verification!
        true
    }
}
#[derive(Default)]
pub struct CapeContract {
    // set of spent records' nullifiers, stored as mapping in contract
    nullifiers: HashSet<Nullifier>,
    // blockchain history: block content and post-block merkle tree commitment
    blocks: Vec<(CapeBlock, MerkleCommitment)>,
    // TODO: in Solidity impl, we should use `keccak256(abi.encodePacked(AssetDefinition))` as the mapping key
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

    /// registering a new CAPE asset for an ERC20.
    pub fn register_cape_asset(&mut self, erc20_addr: Address, new_asset: AssetDefinition) {
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

    /// Relayer submit the next block
    pub fn submit_cape_block(&mut self, new_block: CapeBlock, mt_frontier: MerkleFrontier) {
        // 1. verify the block, and insert its input nullifiers and output record commitments
        assert!(new_block.verify());
        let mut rc_to_be_inserted = vec![];
        for txn in new_block.txns.iter() {
            for &nf in txn.nullifiers().iter() {
                self.nullifiers.insert(nf);
            }
            rc_to_be_inserted.extend_from_slice(&txn.output_commitments());
        }

        // 2. insert all pending deposits AND output records from the txns into record merkle tree
        // and update the merkle root/commitment.
        rc_to_be_inserted.extend_from_slice(&self.pending_deposit_queue);
        let updated_mt_comm = self.batch_insert_with_frontier(mt_frontier, &rc_to_be_inserted);
        self.blocks.push((new_block, updated_mt_comm));
    }

    /// Withdraw CAPE asset back to the ERC20 that it wraps over.
    pub fn withdraw_erc20(
        &self,
        erc20_addr: Address,
        recipient: Address,
        withdraw_amount: U256,
        block_num: U256,
        txn_index: U256,
        burned_ro: RecordOpening,
    ) {
        // 1. fetch the burn txn from blockchain history
        let block = &self.blocks[block_num.as_usize()].0;
        let burn_txn = &block.txns[txn_index.as_usize()];
        assert!(matches!(burn_txn, TransactionNote::Transfer(_)));

        // 2. check if the burn txn has sent the correct amount to the correct burn address
        assert_eq!(burned_ro.amount, withdraw_amount.as_u64());
        assert_eq!(
            self.wrapped_erc20_registrar
                .get(&burned_ro.asset_def)
                .unwrap(),
            &erc20_addr
        );
        assert_eq!(burned_ro.pub_key, burn_pub_key());
        assert_eq!(
            RecordCommitment::from(&burned_ro),
            burn_txn.output_commitments()[1]
        );

        // 3. upon successful verification, credit the `recipient` ERC20
        let mut erc20_contract = Erc20Contract::at(erc20_addr);
        erc20_contract.transfer(recipient, withdraw_amount);
    }
}

#[cfg(test)]
mod test {
    use jf_txn::{
        keys::{AuditorKeyPair, FreezerKeyPair, UserKeyPair},
        structs::{AssetCode, AssetCodeSeed, AssetPolicy, FreezeFlag},
        transfer::TransferNote,
    };

    use super::*;
    use crate::relayer::Relayer;
    use constants::*;

    impl CapeContract {
        // return a mocked contract with some pre-filled states.
        fn mock() -> Self {
            Self::default()
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
        cape_contract.register_cape_asset(usdc_address(), asset_def.clone());
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
        cape_contract.submit_cape_block(new_block, relayer.mt.frontier());
    }

    #[test]
    #[ignore = "ignore panic due to unimplemented logic"]
    fn unwrap_workflow() {
        let mut rng = rand::thread_rng();
        let mut cape_contract = CapeContract::mock();
        let cape_user_keypair = UserKeyPair::generate(&mut rng);
        let eth_user_address = Address::random();
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

        // 2. relayer: build a new block containing the burn txn broadcasted by the user (off-chain)
        let mut new_block = CapeBlock::build_next();
        new_block
            .txns
            .push(TransactionNote::Transfer(Box::new(burn_txn)));
        cape_contract.submit_cape_block(new_block.clone(), relayer.mt.frontier());

        // 3. user: invoke withdraw on layer 1 (on-chain)
        // NOTE: block_num and txn_index are only available after the burn txn was
        // submitted to the CAPE contract and accepted.
        let block_num = cape_contract.blocks.len();
        let txn_index = new_block.txns.len();
        cape_contract.withdraw_erc20(
            usdc_address(),
            eth_user_address,
            U256::from(burn_amount),
            U256::from(block_num),
            U256::from(txn_index),
            burned_ro,
        );
    }
}
