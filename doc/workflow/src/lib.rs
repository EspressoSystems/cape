// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

//! This crate describes the workflow and interfaces of a CAPE contract deployed on Ethereum.

use cap_rust_sandbox::model::{is_erc20_asset_def_valid, Erc20Code, EthereumAddr};
use cap_rust_sandbox::types::GenericInto;
use ethers::prelude::*;
use itertools::Itertools;
use jf_cap::keys::UserPubKey;
use jf_cap::structs::{AssetDefinition, FreezeFlag, Nullifier, RecordCommitment, RecordOpening};
use jf_cap::TransactionNote::Transfer;
use jf_cap::{txn_batch_verify, MerkleCommitment, MerkleFrontier, NodeValue, TransactionNote};
use std::collections::{HashMap, HashSet, LinkedList};

mod constants;
mod erc20;
mod merkle_tree;
mod relayer;
use crate::constants::MERKLE_ROOT_QUEUE_CAP;
use crate::erc20::Erc20Contract;
use crate::merkle_tree::RecordMerkleTree;

#[derive(Debug, Clone)]
pub struct NullifierRepeatedError;

/// A block in CAPE blockchain
#[derive(Default, Clone)]
pub struct CapeBlock {
    /// NOTE: separated out list of burn transaction
    pub(crate) burn_txns: Vec<TransactionNote>,
    /// rest of the transactions (except burn txn)
    pub(crate) txns: Vec<TransactionNote>,
    /// public key of the participant who will collect the fee
    pub(crate) miner: UserPubKey,
}

const CAPE_BURN_PREFIX_BYTES: &str = "EsSCAPE burn";
const CAPE_BURN_PREFIX_BYTES_LEN: usize = 12;

// Check that the transaction note corresponds to a transfer and that the prefix of the auxiliary
// information corresponds to some burn transaction.
fn is_burn_txn(txn: &TransactionNote) -> bool {
    match txn {
        TransactionNote::Transfer(tx) => {
            tx.aux_info.extra_proof_bound_data[0..CAPE_BURN_PREFIX_BYTES_LEN]
                == *CAPE_BURN_PREFIX_BYTES.as_bytes()
        }
        TransactionNote::Mint(_) => false,
        TransactionNote::Freeze(_) => false,
    }
}

impl CapeBlock {
    /// Refer to `validate_block()` in:
    /// <https://github.com/EspressoSystems/cap/blob/main/tests/examples.rs>
    /// Take a new block and remove the transactions that are not valid and update
    /// the list of record openings corresponding to burn transactions accordingly.
    /// Note that the validation of a block in the solidity implementation of the CAPE contract is slightly different:
    /// If a single transaction is invalid, then the whole block is rejected.
    pub fn validate(
        &self,
        recent_merkle_roots: &LinkedList<NodeValue>,
        burned_ros: Vec<RecordOpening>,
        contract_nullifiers: &mut HashSet<Nullifier>,
        height: u64,
    ) -> (CapeBlock, Vec<RecordOpening>) {
        // In order to avoid race conditions between block submitters (relayers or wallets), the CAPE contract
        // discards invalid transactions but keeps the valid ones (instead of rejecting the full block).
        // See https://github.com/EspressoSystems/cape/issues/157

        let mut filtered_block = CapeBlock {
            burn_txns: vec![],
            txns: vec![],
            miner: self.miner.clone(),
        };
        let mut filtered_burn_ros = vec![];

        // Ensure the proofs are checked against the latest root and are valid
        // Standard transactions
        for txn in &self.txns {
            let merkle_root = txn.merkle_root();
            if recent_merkle_roots.contains(&merkle_root)
                && CapeBlock::check_nullifiers_are_fresh(txn, contract_nullifiers)
                && !CapeBlock::is_expired(txn, height)
                && !is_burn_txn(txn)
            {
                filtered_block.txns.push(txn.clone());
            }
        }
        // Burn transactions
        for (i, txn) in self.burn_txns.iter().enumerate() {
            let merkle_root = txn.merkle_root();
            if recent_merkle_roots.contains(&merkle_root)
                && CapeBlock::check_nullifiers_are_fresh(txn, contract_nullifiers)
                && is_burn_txn(txn)
            {
                filtered_block.burn_txns.push(txn.clone());
                filtered_burn_ros.push(burned_ros[i].clone());
            }
        }

        // Validate plonk proofs in batch
        // We assume it is the responsibility of the relayer to ensure all the plonk proofs are valid
        // If not the submitting relayer will simply loose the gas needed for processing the transaction
        let mut all_txns = filtered_block.txns.clone();
        all_txns.extend(filtered_block.burn_txns.clone());
        // If the verification fails return an empty list of transactions and burned record openings

        let recent_merkle_roots_vec = recent_merkle_roots.iter().copied().collect_vec();

        if txn_batch_verify(
            all_txns.as_slice(),
            recent_merkle_roots_vec.as_slice(),
            height,
            &[],
        )
        .is_ok()
        {
            (
                CapeBlock {
                    burn_txns: vec![],
                    txns: vec![],
                    miner: self.miner.clone(),
                },
                vec![],
            )
        } else {
            (filtered_block, filtered_burn_ros)
        }
    }

    fn is_expired(txn: &TransactionNote, height: u64) -> bool {
        match txn {
            Transfer(tx) => tx.aux_info.valid_until < height,
            _ => true,
        }
    }

    /// Checks that all the nullifiers of a transaction have not been published in a previous block
    fn check_nullifiers_are_fresh(
        txn: &TransactionNote,
        contract_nullifiers: &HashSet<Nullifier>,
    ) -> bool {
        for n in txn.nullifiers().iter() {
            if contract_nullifiers.contains(n) {
                return false;
            }
        }
        true
    }
}

/// State and methods of a CAPE contract
pub struct CapeContract {
    /// set of spent records' nullifiers, stored as mapping in contract
    nullifiers: HashSet<Nullifier>,
    /// latest block height
    height: u64,
    /// latest record merkle tree commitment (including merkle root, tree height and num of leaves)
    merkle_commitment: MerkleCommitment,
    /// The merkle frontier is stored locally
    mt_frontier: MerkleFrontier,
    /// last X merkle root, allowing transaction building against recent merkle roots (instead of just
    /// the latest merkle root) as a buffer.
    /// where X is the capacity the Queue and can be specified during constructor. (rust doesn't have queue
    /// so we use LinkedList to simulate)
    /// NOTE: in Solidity, we can instantiate with a fixed array and an indexer to build a FIFO queue.
    recent_merkle_roots: LinkedList<NodeValue>,
    /// NOTE: in Solidity impl, we should use `keccak256(abi.encode(AssetDefinition))` as the mapping key
    wrapped_erc20_registrar: HashMap<AssetDefinition, Address>,
    /// List of record commitments corresponding to ERC20 deposits that will be added when the next block is processed
    pending_deposit_queue: Vec<RecordCommitment>,
}

impl CapeContract {
    /// Inserts a nullifier in the nullifiers hash set.
    /// If the nullifier has already been inserted previously return an error.
    /// In practice (solidity code), the ethereum transaction will be reverted and the smart contract state will be restored.
    /// This approach, compared to checking no duplicates appear in the list of nullifiers for a block, aims at saving ethereum gas in the concrete solidity implementation.
    fn insert_nullifier_or_revert(
        &mut self,
        nullifier: &Nullifier,
    ) -> Result<(), NullifierRepeatedError> {
        if self.nullifiers.contains(nullifier) {
            Err(NullifierRepeatedError)
        } else {
            self.nullifiers.insert(*nullifier);
            Ok(())
        }
    }

    /// Return the address of the contract.
    pub(crate) fn address(&self) -> Address {
        // NOTE: in Solidity, use expression: `address(this)`
        Address::from_low_u64_le(666u64)
    }

    /// Check if an asset is already registered.
    /// Assets need to be registered and bound to some ERC-20 before allowing users to wrap/unwrap.
    pub fn is_cape_asset_registered(&self, asset_def: &AssetDefinition) -> bool {
        self.wrapped_erc20_registrar.contains_key(asset_def)
    }

    /// Create a new asset type for an ERC20 and register it to the contract.
    pub fn sponsor_cape_asset(
        &mut self,
        erc20_addr: Address,
        sponsor: Address,
        new_asset: AssetDefinition,
    ) {
        assert!(
            !self.is_cape_asset_registered(&new_asset),
            "this CAPE asset is already registered"
        );
        // check correct ERC20 address.
        let _ = Erc20Contract::at(erc20_addr);

        // Check for valid foreign asset definition to ensure asset cannot be minted.
        assert!(is_erc20_asset_def_valid(
            &new_asset,
            &Erc20Code(EthereumAddr(erc20_addr.to_fixed_bytes())),
            &EthereumAddr(sponsor.to_fixed_bytes())
        ));

        self.wrapped_erc20_registrar.insert(new_asset, erc20_addr);
    }

    /// Deposit some ERC20 tokens so that these are wrapped into asset records
    /// NOTE: in Solidity, we can
    /// - avoid passing in `ro.freeze_flag` (e.g: to save a bit of gas)
    /// - remove `depositor` from input parameters, and directly replaced with `msg.sender`
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
        assert!(ro.amount > 0u64.into());
        assert_ne!(ro.pub_key, UserPubKey::default()); // this would be EC point equality check
        assert_eq!(ro.freeze_flag, FreezeFlag::Unfrozen); // just a boolean flag

        // 2. attempt to `transferFrom` before mutating contract state to mitigate reentrancy attack
        erc20_contract.transfer_from(
            depositor,
            self.address(),
            ro.amount.generic_into::<u128>().into(),
        );

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
    pub fn submit_cape_block(
        &mut self,
        new_block: CapeBlock,
        burned_ros: Vec<RecordOpening>,
    ) -> Result<(), NullifierRepeatedError> {
        // 1. verify the block, and insert its input nullifiers and output record commitments
        let (new_block, new_burned_ros) = new_block.validate(
            &self.recent_merkle_roots,
            burned_ros,
            &mut self.nullifiers,
            self.height,
        );

        // We allow empty blocks. See discussion https://github.com/EspressoSystems/cape/issues/156
        // Summary:
        // * Empty blocks allow to flush the queue of asset records to be inserted into the merkle tree after some call to wrap/faucet
        // * As the block height is our proxy for time it looks desirable to be able to create new blocks even though no new transactions are produced

        let mut rc_to_be_inserted = vec![];
        for txn in new_block.txns.iter() {
            for &nf in txn.nullifiers().iter() {
                self.insert_nullifier_or_revert(&nf)
                    .expect("Nullifiers should not be repeated inside a block.");
            }
            rc_to_be_inserted.extend_from_slice(&txn.output_commitments());
        }

        // 2. process all burn transaction and withdraw for user immediately
        assert_eq!(new_block.burn_txns.len(), new_burned_ros.len());
        for (burn_txn, burned_ro) in new_block.burn_txns.iter().zip(new_burned_ros.iter()) {
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

                // Since the transaction only contains record commitments,
                // we require the user to provide the record opening and check against it.
                assert_eq!(
                    RecordCommitment::from(burned_ro),
                    burn_txn.output_commitments()[1]
                );

                let erc20_addr = self
                    .wrapped_erc20_registrar
                    .get(&burned_ro.asset_def)
                    .unwrap();

                // 2.2. upon successful verification, execute the withdraw for user
                let mut erc20_contract = Erc20Contract::at(*erc20_addr);
                erc20_contract.transfer(recipient, withdraw_amount.generic_into::<u128>().into());

                // 2.3. like other txn, insert input nullifiers and output record commitments
                for &nf in burn_txn.nullifiers().iter() {
                    self.nullifiers.insert(nf);
                }

                // We insert all the output commitments except the second one that corresponds to the burned output.
                // That way we ensure that this burned output cannot be spent
                const POS_BURNED_RC: usize = 1;
                for (i, rc) in burn_txn.output_commitments().iter().enumerate() {
                    if i != POS_BURNED_RC {
                        rc_to_be_inserted.push(*rc);
                    }
                }
            } else {
                panic!("burn txn should be of TransferNote type");
            }
        }

        // 3. insert all pending deposits AND output records from the txns into record merkle tree
        // and update the merkle root/commitment.
        rc_to_be_inserted.extend_from_slice(&self.pending_deposit_queue);
        let (updated_mt_comm, updated_mt_frontier) =
            self.batch_insert_with_frontier(self.mt_frontier.clone(), &rc_to_be_inserted);

        // Empty the list record commitments corresponding to pending deposits
        self.pending_deposit_queue = vec![];

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

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use jf_cap::{
        keys::{AuditorKeyPair, FreezerKeyPair, UserKeyPair},
        structs::{AssetCode, AssetPolicy, FreezeFlag},
        transfer::TransferNote,
        NodeValue,
    };

    use super::*;
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
                mt_frontier: MerkleFrontier::Empty { height: 0 },
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
        let asset_code = AssetCode::new_foreign(b"Official wrapped USDC in CAPE system.");
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
        // 1. sponsor: design  the CAPE asset type (off-chain).
        let asset_def = usdc_cape_asset_def();
        let sponsor = Address::random();

        // 2. sponsor: register the asset (on-L1-chain).
        cape_contract.sponsor_cape_asset(usdc_address(), sponsor, asset_def);
    }

    #[test]
    #[ignore = "ignore panic due to unimplemented logic"]
    fn wrap_workflow() {
        let mut rng = rand::thread_rng();
        let mut cape_contract = CapeContract::mock();
        let mut usdc_contract = Erc20Contract::at(usdc_address());

        let cape_user_keypair = UserKeyPair::generate(&mut rng);
        let eth_user_address = Address::random();

        // 1. user: fetch the CAPE asset definition from UI or sponsor.
        let asset_def = usdc_cape_asset_def();

        // 2. user: invoke ERC20's approve (on-L1-chain)
        //
        let deposit_amount = U256::from(1000);
        usdc_contract.approve(cape_contract.address(), deposit_amount);

        // 3. user: prepare the CAPE Record Opening, then invoke wrapper's deposit erc20
        let ro = RecordOpening::new(
            &mut rng,
            deposit_amount.as_u128().into(),
            asset_def,
            cape_user_keypair.pub_key(),
            FreezeFlag::Unfrozen,
        );
        cape_contract.deposit_erc20(ro, usdc_address(), eth_user_address);

        // 4. relayer: build next block and user's deposit will be credited (inserted into
        // record merkle tree) when CAPE contract process the next valid block.
        let new_block = CapeBlock::build_next();
        let _res = cape_contract.submit_cape_block(new_block, vec![]);
        // Handle result of call to `submit_cape_block`.
    }

    #[test]
    #[ignore = "ignore panic due to unimplemented logic"]
    fn unwrap_workflow() {
        let mut rng = rand::thread_rng();
        let mut cape_contract = CapeContract::mock();
        let cape_user_keypair = UserKeyPair::generate(&mut rng);

        // 1. user: build and send a burn transaction to relayer (off-chain)
        let asset_def = usdc_cape_asset_def();
        let burn_amount = 1000u64;
        let burned_ro = RecordOpening::new(
            &mut rng,
            burn_amount.into(),
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

        let _res = cape_contract.submit_cape_block(new_block.clone(), burned_ros);
        // Handle result of call to `submit_cape_block`.

        // done! when CAPE contract process a new block,
        // it will automatically withdraw for user that has submitted the burn transaction.
    }
}
