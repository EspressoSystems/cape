use ethers::prelude::{Bytes, U256};
use jf_aap::transfer::{AuxInfo, TransferNote};
use jf_aap::TransactionNote;
use jf_aap::{freeze::FreezeNote, structs::AuditMemo, VerKey};
use jf_aap::{keys::UserPubKey, mint::MintNote};

use crate::helpers::{convert_fr254_to_u256, convert_nullifier_to_u256};
use crate::types as sol;
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

impl From<AuxInfo> for sol::TransferAuxInfo {
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
    use ethers::prelude::k256::ecdsa::SigningKey;
    use ethers::prelude::{Address, Http, Provider, SignerMiddleware, Wallet};
    use itertools::Itertools;

    use crate::assertion::Matcher;
    use crate::cap_jf::create_anon_xfr_2in_3out;
    use crate::ethereum::{deploy, get_funded_deployer};
    use crate::helpers::convert_nullifier_to_u256;
    use crate::types::{CapeBlock, TestCAPE, TestCapeTypes, CAPE};
    use anyhow::Result;
    use ethers::core::k256::ecdsa::SigningKey;
    use ethers::prelude::*;
    use std::env;
    use std::path::Path;

    #[allow(dead_code)] // TODO: remove this
    async fn deploy_cape_contract(
    ) -> Result<TestCAPE<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>> {
        let client = get_funded_deployer().await.unwrap();
        let contract = deploy(
            client.clone(),
            Path::new("../artifacts/contracts/TestCAPE.sol/TestCAPE"),
            (),
        )
        .await
        .unwrap();
        Ok(TestCAPE::new(contract.address(), client))
    }

    mod type_conversion {
        use super::*;
        use crate::types::GenericInto;
        use jf_aap::structs::Nullifier;

        async fn deploy_type_contract(
        ) -> Result<TestCapeTypes<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>> {
            let client = get_funded_deployer().await.unwrap();
            let contract = deploy(
                client.clone(),
                Path::new("../artifacts/contracts/mocks/TestCapeTypes.sol/TestCapeTypes"),
                (),
            )
            .await
            .unwrap();
            Ok(TestCapeTypes::new(contract.address(), client))
        }

        #[tokio::test]
        async fn test_nullifier() -> Result<()> {
            let rng = &mut ark_std::test_rng();
            let contract = deploy_type_contract().await?;
            for _ in 0..5 {
                let nf = Nullifier::random_for_test(rng);
                let res: Nullifier = contract
                    .check_nullifier(nf.generic_into::<sol::NullifierSol>().0)
                    .call()
                    .await?
                    .generic_into::<sol::NullifierSol>()
                    .generic_into::<Nullifier>();
                assert_eq!(nf, res);
            }
            Ok(())
        }
    }

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

    async fn deploy_cape_test() -> TestCAPE<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>> {
        let client = get_funded_deployer().await.unwrap();
        let contract = deploy(
            client.clone(),
            Path::new("../artifacts/contracts/mocks/TestCAPE.sol/TestCAPE"),
            (),
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
    // main block validaton loop.
}
