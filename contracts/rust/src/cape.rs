use ethers::prelude::U256;
use jf_aap::transfer::{AuxInfo, TransferNote};
use jf_aap::TransactionNote;
use jf_aap::{freeze::FreezeNote, structs::AuditMemo, VerKey};
use jf_aap::{keys::UserPubKey, mint::MintNote};

use crate::helpers::{convert_fr254_to_u256, convert_nullifier_to_u256};
use crate::types as sol; // TODO figure out what to do about type collisions
use itertools::Itertools;

const DUMMY_UINT: U256 = U256::zero();
pub const CAPE_BURN_MAGIC_BYTES: &str = "TRICAPE burn";

#[derive(Debug, Clone, PartialEq)]
pub enum TransferType {
    Transfer,
    Burn,
}

impl From<TransferNote> for sol::TransferNote {
    fn from(note: TransferNote) -> Self {
        Self {
            inputs_nullifiers: note
                .inputs_nullifiers
                .clone()
                .iter()
                .map(convert_nullifier_to_u256)
                .collect_vec(),

            output_commitments: note
                .output_commitments
                .clone()
                .iter()
                .map(|c| convert_fr254_to_u256(c.to_field_element()))
                .collect_vec(),

            // TODO
            proof: sol::PlonkProof { dummy: DUMMY_UINT },

            audit_memo: note.audit_memo.into(),
            aux_info: note.aux_info.into(),
        }
    }
}

impl From<AuditMemo> for sol::AuditMemo {
    fn from(_item: AuditMemo) -> Self {
        // TODO
        Self::default()
    }
}

impl From<VerKey> for sol::EdOnBn254Point {
    fn from(_item: VerKey) -> Self {
        // TODO
        Self::default()
    }
}

impl From<MintNote> for sol::MintNote {
    fn from(_note: MintNote) -> Self {
        unimplemented!() // TODO
    }
}

impl From<FreezeNote> for sol::FreezeNote {
    fn from(_note: FreezeNote) -> Self {
        unimplemented!() // TODO
    }
}

impl From<UserPubKey> for sol::UserPubKey {
    fn from(_key: UserPubKey) -> Self {
        Self::default() // TODO
    }
}

impl From<AuxInfo> for sol::AuxInfo {
    fn from(item: AuxInfo) -> Self {
        Self {
            merkle_root: convert_fr254_to_u256(item.merkle_root.to_scalar()),
            fee: item.fee,
            valid_until: item.valid_until,
            txn_memo_ver_key: item.txn_memo_ver_key.into(),
            extra_proof_bound_data: item.extra_proof_bound_data.into(),
        }
    }
}

fn transfer_type(xfr: &TransferNote) -> TransferType {
    let magic_bytes = CAPE_BURN_MAGIC_BYTES.as_bytes().to_vec();
    let field_data = &xfr.aux_info.extra_proof_bound_data;

    match field_data.len() {
        32 => {
            if field_data[..12] == magic_bytes[..] {
                TransferType::Burn
            } else {
                TransferType::Transfer
            }
        }
        _ => TransferType::Transfer,
    }
}

#[allow(dead_code)]
fn get_note_type(tx: TransactionNote) -> u8 {
    match tx {
        TransactionNote::Transfer(note) => match transfer_type(&note) {
            TransferType::Transfer => 0u8,
            TransferType::Burn => 3u8,
        },
        TransactionNote::Mint(_) => 1u8,
        TransactionNote::Freeze(_) => 2u8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethers::prelude::k256::ecdsa::SigningKey;
    use ethers::prelude::{Address, Http, Provider, SignerMiddleware, Wallet};
    use itertools::Itertools;

    use crate::assertion::Matcher;
    use crate::cap_jf::create_anon_xfr_2in_3out;
    use crate::ethereum::{deploy, get_funded_deployer};
    use crate::helpers::{convert_nullifier_to_u256, convert_u256_to_bytes_le};
    use crate::records_merkle_tree::flatten_frontier;
    use crate::types::{BurnNote, CapeBlock, RecordOpening, TestCAPE};
    use ark_ed_on_bn254::Fq as Fr254;
    use ark_ff::BigInteger;
    use ark_ff::PrimeField;
    use jf_primitives::merkle_tree::MerkleTree;
    use std::env;
    use std::path::Path;

    const TREE_HEIGHT: u8 = 20;
    const N_ROOTS: u8 = 3;

    async fn compare_roots(
        mt: &MerkleTree<Fr254>,
        contract: &TestCAPE<
            SignerMiddleware<Provider<Http>, Wallet<ethers::core::k256::ecdsa::SigningKey>>,
        >,
        should_be_equal: bool,
    ) {
        let root_fr254 = mt.commitment().root_value;
        let root_value_u256 = contract.get_root_value().call().await.unwrap();

        assert_eq!(
            should_be_equal,
            (convert_u256_to_bytes_le(root_value_u256).as_slice()
                == root_fr254.to_scalar().into_repr().to_bytes_le())
        );
    }

    async fn submit_cape_block_to_contract(
        contract: &TestCAPE<
            SignerMiddleware<Provider<Http>, Wallet<ethers::core::k256::ecdsa::SigningKey>>,
        >,
        notes: Vec<TransactionNote>,
    ) {
        let note_types = notes
            .iter()
            .map(|note| get_note_type(note.clone()))
            .collect_vec();

        let miner = UserPubKey::default();

        let mut transfer_notes = vec![];
        let mut mint_notes = vec![];
        let mut freeze_notes = vec![];
        let mut burn_notes = vec![];

        for note in notes.clone() {
            match note.clone() {
                TransactionNote::Transfer(n) => match transfer_type(&n) {
                    TransferType::Transfer => transfer_notes.push((*n).clone().into()),
                    TransferType::Burn => {
                        let note = (*n).clone().into();
                        let ro = RecordOpening { field: true }; // TODO real implementation
                        let burn_note = BurnNote {
                            transfer_note: note,
                            record_opening: ro,
                        };
                        burn_notes.push(burn_note);
                    }
                },
                TransactionNote::Freeze(n) => freeze_notes.push((*n).clone().into()),
                TransactionNote::Mint(n) => mint_notes.push((*n).clone().into()),
            }
        }

        // Create dummy records openings array
        let records_openings = vec![];

        // Convert the AAP transactions into some solidity friendly representation
        let block = CapeBlock {
            miner: miner.into(),
            block_height: 123u64,
            transfer_notes,
            note_types,
            mint_notes,
            freeze_notes,
            burn_notes,
        };

        // Submit to the contract
        let _receipt = contract
            .submit_cape_block(block, records_openings)
            .legacy()
            .send()
            .await
            .unwrap()
            .await
            .unwrap()
            .expect("Failed to get tx receipt");
    }

    /// Performs some verifications when sending a block to the CAPE contract
    /// * `notes` - list of notes conforming the block to be submitted
    /// * `idx_invalid_txs` - list of indexes corresponding to invalid transactions.
    /// We simulate the invalidity of those transactions by inserting one of their nullifiers into the contract state before sending the block.
    async fn check_block(notes: Vec<TransactionNote>, idx_invalid_txs: Vec<usize>) {
        let client = get_funded_deployer().await.unwrap();

        let contract_address: Address = match env::var("CAPE_ADDRESS") {
            Ok(val) => val.parse::<Address>().unwrap(),
            Err(_) => deploy(
                client.clone(),
                Path::new("../artifacts/contracts/mocks/TestCAPE.sol/TestCAPE"),
                (TREE_HEIGHT, N_ROOTS),
            )
            .await
            .unwrap()
            .address(),
        };

        let contract = TestCAPE::new(contract_address, client);

        // Simulate invalid transactions by inserting one of their nullifiers into the contract state.
        let mut already_published_nullifiers = vec![];
        for i in &idx_invalid_txs {
            let null = &notes.clone()[*i].nullifiers()[0];
            already_published_nullifiers.push(null.clone());
        }

        let u256_nullifiers = already_published_nullifiers
            .iter()
            .map(|n| convert_nullifier_to_u256(&n))
            .collect_vec();
        for null in u256_nullifiers {
            let _receipt = contract
                .insert_nullifier(null)
                .legacy()
                .send()
                .await
                .unwrap()
                .await
                .unwrap()
                .expect("Failed to get tx receipt");
        }

        // Check that the first nullifier of the first transaction is not yet inserted
        let nullifier = match notes[0].clone() {
            TransactionNote::Transfer(n) => {
                convert_nullifier_to_u256(&(*n).clone().inputs_nullifiers[0])
            }
            _ => panic!("The first note should be a transfer note"),
        };

        let is_nullifier_inserted = contract.nullifiers(nullifier).call().await.unwrap();
        assert!(!is_nullifier_inserted);

        submit_cape_block_to_contract(&contract, notes.clone()).await;

        // Check that now the first nullifier of the first transaction has been inserted
        let is_nullifier_inserted = contract.nullifiers(nullifier).call().await.unwrap();
        assert!(is_nullifier_inserted);

        // Check that the records Merkle tree has been updated correctly
        let mut record_commitments_to_insert = vec![];
        for (i, note) in notes.iter().enumerate() {
            if !idx_invalid_txs.contains(&i)
            // We skip invalid transactions
            {
                record_commitments_to_insert.extend(note.clone().output_commitments());
            }
        }

        let mut mt = MerkleTree::<Fr254>::new(TREE_HEIGHT).unwrap();
        for r in record_commitments_to_insert {
            mt.push(r.to_field_element());
        }
        compare_roots(&mt, &contract, true).await;

        // Check the frontier has been stored correctly
        let flattened_frontier_from_contract =
            contract.get_flattened_frontier().call().await.unwrap();
        let pos = mt.num_leaves() - 1;
        let flattened_frontier = flatten_frontier(&mt.frontier(), pos)
            .iter()
            .map(|v| convert_fr254_to_u256((*v).clone()))
            .collect_vec();
        assert_eq!(flattened_frontier_from_contract, flattened_frontier);
    }

    #[tokio::test]
    async fn test_submit_block_to_cape_contract() {
        let mut prng = ark_std::test_rng();

        // Two transfer transactions
        let notes = create_anon_xfr_2in_3out(&mut prng, 2);
        let transaction_notes = notes
            .iter()
            .map(|n| TransactionNote::from(n.clone()))
            .collect_vec();
        check_block(transaction_notes.clone(), vec![]).await;

        // Two transactions with one invalid (the second transaction has an already published nullifier)
        // We reuse the notes created above
        check_block(transaction_notes, vec![1]).await;

        // One transaction of each type (Transfer,Mint,Freeze,Burn)

        // TODO more test cases to capture different combinations of transactions
        // (different types, valid and invalid transactions, different number of inputs/outputs,....)
        // Create new ticket ^^^
    }

    #[tokio::test]
    async fn test_storage_of_n_roots() {
        let contract = deploy_cape_test().await;

        let mut prng = ark_std::test_rng();

        const N_BLOCKS: u32 = 4;

        // We create blocks of a single transaction each
        let notes = create_anon_xfr_2in_3out(&mut prng, N_BLOCKS);

        // Check the roots are initially all set to 0
        let (roots, index) = contract.get_roots().call().await.unwrap();
        assert_eq!(roots, vec![U256::from(0); 3]);
        assert_eq!(index, U256::from(1));

        let mut root_values_after_each_update = vec![];

        // Submit four blocks.
        for i in 0..(N_BLOCKS as usize) {
            submit_cape_block_to_contract(&contract, vec![TransactionNote::from(notes[i].clone())])
                .await;
            let root_value_u256 = contract.get_root_value().call().await.unwrap();
            root_values_after_each_update.push(root_value_u256);
        }

        // Check that the last three roots are stored correctly.
        let (roots, index) = contract.get_roots().call().await.unwrap();
        let expected_roots = [
            root_values_after_each_update[2],
            root_values_after_each_update[3],
            root_values_after_each_update[1],
        ];
        assert_eq!(index, U256::from(2));
        assert_eq!(roots, expected_roots);
    }

    #[test]
    fn test_note_types() {
        // TODO
        // let rng = ark_std::test_rng();
        // assert_eq!(get_note_type(TransferNote::rand_for_test(&rng)), 0u8);
        // assert_eq!(get_note_type(FreezeNote::rand_for_test(&rng)), 1u8);
        // assert_eq!(get_note_type(MintNote::rand_for_test(&rng)), 2u8);
        // assert_eq!(get_note_type(create_test_burn_note(&rng)), 3u8);
    }

    async fn deploy_cape_test() -> TestCAPE<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>> {
        let client = get_funded_deployer().await.unwrap();
        let contract = deploy(
            client.clone(),
            Path::new("../artifacts/contracts/mocks/TestCAPE.sol/TestCAPE"),
            (TREE_HEIGHT, N_ROOTS),
        )
        .await
        .unwrap();
        TestCAPE::new(contract.address(), client)
    }

    #[tokio::test]
    async fn test_has_burn_prefix() {
        let contract = deploy_cape_test().await;

        for (input, expected) in [
            ("", false),
            ("x", false),
            ("TRICAPE bur", false),
            ("more data but but still not a burn", false),
            (CAPE_BURN_MAGIC_BYTES, true),
            (&(CAPE_BURN_MAGIC_BYTES.to_owned() + "more stuff"), true),
        ] {
            assert_eq!(
                contract
                    .has_burn_prefix(input.as_bytes().to_vec().into())
                    .call()
                    .await
                    .unwrap(),
                expected
            )
        }
    }

    #[tokio::test]
    async fn test_has_burn_destination() {
        let contract = deploy_cape_test().await;

        for (input, expected) in [
            (
                // ok, zero address from byte 12 to 32
                "ffffffffffffffffffffffff0000000000000000000000000000000000000000",
                true,
            ),
            (
                // ok, with more data
                "ffffffffffffffffffffffff0000000000000000000000000000000000000000ff",
                true,
            ),
            (
                // wrong address
                "ffffffffffffffffffffffff0000000000000000000000000000000000000001",
                false,
            ),
            (
                // address too short
                "ffffffffffffffffffffffff00000000000000000000000000000000000000",
                false,
            ),
        ] {
            assert_eq!(
                contract
                    .has_burn_destination(hex::decode(input).unwrap().into())
                    .call()
                    .await
                    .unwrap(),
                expected
            )
        }
    }

    #[tokio::test]
    async fn test_check_burn() {
        let contract = deploy_cape_test().await;

        let wrong_address =
            CAPE_BURN_MAGIC_BYTES.to_owned() + "000000000000000000000000000000000000000f";
        println!("wrong address {}", wrong_address);
        assert!(contract
            .check_burn(wrong_address.as_bytes().to_vec().into())
            .call()
            .await
            .should_revert_with_message("destination wrong"));

        let wrong_tag = "ffffffffffffffffffffffff0000000000000000000000000000000000000000";
        assert!(contract
            .check_burn(hex::decode(wrong_tag).unwrap().into())
            .call()
            .await
            .should_revert_with_message("not tagged"));
    }

    // TODO integration test to check if check_burn is hooked up correctly in
    // main block validation loop.
}
