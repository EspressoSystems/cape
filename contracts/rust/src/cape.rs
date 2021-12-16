use ethers::prelude::{Bytes, EthAbiType, U256};
use itertools::Itertools;
use jf_aap::freeze::FreezeAuxInfo;
use jf_aap::mint::MintAuxInfo;
use jf_aap::structs::{AssetDefinition, Nullifier, RecordCommitment};
use jf_aap::transfer::{AuxInfo, TransferNote};
use jf_aap::TransactionNote;
use jf_aap::{freeze::FreezeNote, structs::AuditMemo, VerKey};
use jf_aap::{keys::UserPubKey, mint::MintNote};

use crate::helpers::{convert_fr254_to_u256, convert_nullifier_to_u256};
use crate::types as sol; // TODO figure out what to do about type collisions

#[derive(Debug, Clone, PartialEq, EthAbiType)]
pub enum NoteType {
    Transfer,
    Mint,
    Freeze,
    Burn,
}

// TODO Move these conversion functions to types later

fn nullifiers_to_u256(items: &[Nullifier]) -> Vec<U256> {
    items.iter().map(convert_nullifier_to_u256).collect_vec()
}

fn rc_to_u256(c: &RecordCommitment) -> U256 {
    convert_fr254_to_u256(c.to_field_element())
}

fn commitments_to_sol(items: &[RecordCommitment]) -> Vec<U256> {
    items.iter().map(rc_to_u256).collect_vec()
}

// fn proof_to_sol<E>(proof: Proof<E>) -> sol::PlonkProof
// where
//     E: ark_ec::PairingEngine, // should this be PairingEngine from jf types ?
// {
//     // TODO
//     sol::PlonkProof::default()
// }

impl From<TransferNote> for sol::TransferNote {
    fn from(note: TransferNote) -> Self {
        Self {
            inputs_nullifiers: nullifiers_to_u256(&note.inputs_nullifiers),
            output_commitments: commitments_to_sol(&note.output_commitments),
            proof: sol::PlonkProof::default(), // TODO
            audit_memo: note.audit_memo.into(),
            aux_info: note.aux_info.into(),
        }
    }
}

impl From<MintNote> for sol::MintNote {
    fn from(note: MintNote) -> Self {
        Self {
            input_nullifier: convert_nullifier_to_u256(&note.input_nullifier),
            chg_comm: rc_to_u256(&note.chg_comm),
            mint_comm: rc_to_u256(&note.mint_comm),
            mint_amount: note.mint_amount,
            mint_assed_def: note.mint_asset_def.into(),
            proof: sol::PlonkProof::default(), // TODO
            audit_memo: note.audit_memo.into(),
            aux_info: note.aux_info.into(),
        }
    }
}

impl From<FreezeNote> for sol::FreezeNote {
    fn from(_note: FreezeNote) -> Self {
        unimplemented!() // TODO
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

impl From<MintAuxInfo> for sol::MintAuxInfo {
    fn from(_item: MintAuxInfo) -> Self {
        // TODO
        Self::default()
    }
}

impl From<FreezeAuxInfo> for sol::FreezeAuxInfo {
    fn from(_item: FreezeAuxInfo) -> Self {
        // TODO
        Self::default()
    }
}

impl From<AssetDefinition> for sol::AssetDefinition {
    fn from(_item: AssetDefinition) -> Self {
        // TODO
        Self::default()
    }
}

#[allow(dead_code)]
fn get_note_types(notes: Vec<TransactionNote>) -> Vec<u8> {
    // TODO does ethers have better support for encoding an enum?
    notes
        .iter()
        .map(|tx| match tx {
            TransactionNote::Transfer(_) => 0u8,
            // TODO Handle burn case => 3u8
            TransactionNote::Mint(_) => 1u8,
            TransactionNote::Freeze(_) => 2u8,
        })
        .collect_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethers::prelude::Address;
    use itertools::Itertools;

    use crate::cap_jf::create_anon_xfr_2in_3out;
    use crate::ethereum::{deploy, get_funded_deployer};
    use crate::helpers::convert_nullifier_to_u256;
    use crate::types::{CapeBlock, CAPE};
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

        let note_types = get_note_types(
            notes
                .iter()
                .map(|note| TransactionNote::from(note.clone()))
                .collect_vec(),
        );

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
}
