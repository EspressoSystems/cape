use ethers::prelude::{Bytes, U256};
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
            extra_proof_bound_data: Bytes::from(b""),
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
    use ethers::prelude::Address;
    use itertools::Itertools;

    use crate::cap_jf::create_anon_xfr_2in_3out;
    use crate::ethereum::{deploy, get_funded_deployer};
    use crate::helpers::convert_nullifier_to_u256;
    use crate::types::{CapeBlock, TestCAPE, CAPE};
    use std::env;
    use std::path::Path;

    #[tokio::test]
    async fn test_submit_block_to_cape_contract() {
        let client = get_funded_deployer().await.unwrap();

        let contract_address: Address = match env::var("CAPE_ADDRESS") {
            Ok(val) => val.parse::<Address>().unwrap(),
            Err(_) => deploy(
                client.clone(),
                Path::new("../artifacts/contracts/CAPE.sol/CAPE"),
                (),
            )
            .await
            .unwrap()
            .address(),
        };

        let contract = CAPE::new(contract_address, client);

        // Create two transactions
        let mut prng = ark_std::test_rng();
        let notes = create_anon_xfr_2in_3out(&mut prng, 2);

        let note_types = notes
            .iter()
            .map(|note| get_note_type(TransactionNote::from(note.clone())))
            .collect_vec();

        let miner = UserPubKey::default();

        // Convert the AAP transactions into some solidity friendly representation
        let block = CapeBlock {
            miner: miner.into(),
            block_height: 123u64,
            transfer_notes: notes.iter().map(|note| note.clone().into()).collect_vec(),
            note_types,
            mint_notes: vec![],
            freeze_notes: vec![],
            burn_notes: vec![],
        };

        // Create dummy records openings arrary
        let records_openings = vec![];

        // Check that some nullifier is not yet inserted
        let nullifier = convert_nullifier_to_u256(&notes[0].inputs_nullifiers[0]);
        let is_nullifier_inserted = contract.nullifiers(nullifier).call().await.unwrap();
        assert!(!is_nullifier_inserted);

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

        // Check that now the nullifier has been inserted
        let is_nullifier_inserted = contract.nullifiers(nullifier).call().await.unwrap();

        assert!(is_nullifier_inserted);
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

    #[tokio::test]
    async fn test_is_burn_tx_burn_prefix_check() {
        let client = get_funded_deployer().await.unwrap();
        let contract_address = deploy(
            client.clone(),
            Path::new("../artifacts/contracts/mocks/TestCAPE.sol/TestCAPE"),
            (),
        )
        .await
        .unwrap()
        .address();
        let contract = TestCAPE::new(contract_address, client);

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
                    .is_burn(input.as_bytes().to_vec().into())
                    .call()
                    .await
                    .unwrap(),
                expected
            )
        }
    }
}
