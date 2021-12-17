use jf_aap::transfer::TransferNote;
use jf_aap::TransactionNote;

pub const CAPE_BURN_MAGIC_BYTES: &str = "TRICAPE burn";

#[derive(Debug, Clone, PartialEq)]
pub enum TransferType {
    Transfer,
    Burn,
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
    use crate::assertion::Matcher;
    use crate::cap_jf::create_anon_xfr_2in_3out;
    use crate::ethereum::{deploy, get_funded_deployer};
    use crate::helpers::convert_nullifier_to_u256;
    use crate::types as sol;
    use crate::types::{CapeBlock, TestCAPE, TestCapeTypes, CAPE};
    use anyhow::Result;
    use ethers::core::k256::ecdsa::SigningKey;
    use ethers::prelude::*;
    use itertools::Itertools;
    use jf_aap::keys::UserPubKey;
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
        use ark_bn254::{Bn254, Fr};
        use ark_std::UniformRand;
        use jf_aap::{
            freeze::FreezeNote,
            mint::MintNote,
            structs::{
                AssetCode, AssetDefinition, AssetPolicy, AuditMemo, Nullifier, RecordCommitment,
                RecordOpening,
            },
            utils::TxnsParams,
            BaseField, NodeValue,
        };
        use jf_plonk::proof_system::structs::Proof;
        use jf_primitives::elgamal;

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
                let res = contract
                    .check_nullifier(nf.generic_into::<sol::NullifierSol>().0)
                    .call()
                    .await?
                    .generic_into::<sol::NullifierSol>()
                    .generic_into::<Nullifier>();
                assert_eq!(nf, res);
            }
            Ok(())
        }

        #[tokio::test]
        async fn test_record_commitment() -> Result<()> {
            let rng = &mut ark_std::test_rng();
            let contract = deploy_type_contract().await?;
            for _ in 0..5 {
                let rc = RecordCommitment::from_field_element(BaseField::rand(rng));
                let res = contract
                    .check_record_commitment(rc.generic_into::<sol::RecordCommitmentSol>().0)
                    .call()
                    .await?
                    .generic_into::<sol::RecordCommitmentSol>()
                    .generic_into::<RecordCommitment>();
                assert_eq!(rc, res);
            }
            Ok(())
        }

        #[tokio::test]
        async fn test_merkle_root() -> Result<()> {
            let rng = &mut ark_std::test_rng();
            let contract = deploy_type_contract().await?;
            for _ in 0..5 {
                let root = NodeValue::rand(rng);
                let res = contract
                    .check_merkle_root(root.generic_into::<sol::MerkleRootSol>().0)
                    .call()
                    .await?
                    .generic_into::<sol::MerkleRootSol>()
                    .generic_into::<NodeValue>();
                assert_eq!(root, res);
            }
            Ok(())
        }

        #[tokio::test]
        async fn test_asset_code() -> Result<()> {
            let rng = &mut ark_std::test_rng();
            let contract = deploy_type_contract().await?;
            for _ in 0..5 {
                let (ac, _) = AssetCode::random(rng);
                let res = contract
                    .check_merkle_root(ac.generic_into::<sol::AssetCodeSol>().0)
                    .call()
                    .await?
                    .generic_into::<sol::AssetCodeSol>()
                    .generic_into::<AssetCode>();
                assert_eq!(ac, res);
            }
            Ok(())
        }

        #[tokio::test]
        async fn test_asset_policy_and_definition() -> Result<()> {
            let rng = &mut ark_std::test_rng();
            let contract = deploy_type_contract().await?;
            for _ in 0..5 {
                // NOTE: `sol::AssetPolicy` is from abigen! on contract,
                // it collides with `jf_aap::structs::AssetPolicy`
                let policy = AssetPolicy::rand_for_test(rng);
                assert_eq!(
                    policy.clone(),
                    contract
                        .check_asset_policy(policy.generic_into::<sol::AssetPolicy>())
                        .call()
                        .await?
                        .generic_into::<AssetPolicy>()
                );

                let asset_def = AssetDefinition::rand_for_test(rng);
                assert_eq!(
                    asset_def.clone(),
                    contract
                        .check_asset_definition(asset_def.generic_into::<sol::AssetDefinition>())
                        .call()
                        .await?
                        .generic_into::<AssetDefinition>()
                );
            }
            Ok(())
        }

        #[tokio::test]
        async fn test_record_opening() -> Result<()> {
            let rng = &mut ark_std::test_rng();
            let contract = deploy_type_contract().await?;
            for _ in 0..5 {
                // NOTE: `sol::RecordOpening` is from abigen! on contract,
                // it collides with `jf_aap::structs::RecordOpening`
                let ro = RecordOpening::rand_for_test(rng);
                let res = contract
                    .check_record_opening(ro.clone().generic_into::<sol::RecordOpening>())
                    .call()
                    .await?
                    .generic_into::<RecordOpening>();
                assert_eq!(ro.amount, res.amount);
                assert_eq!(ro.asset_def, res.asset_def);
                assert_eq!(ro.pub_key.address(), res.pub_key.address()); // not recovering pub_key.enc_key
                assert_eq!(ro.freeze_flag, res.freeze_flag);
                assert_eq!(ro.blind, res.blind);
            }
            Ok(())
        }

        #[tokio::test]
        async fn test_audit_memo() -> Result<()> {
            let rng = &mut ark_std::test_rng();
            let contract = deploy_type_contract().await?;
            for _ in 0..5 {
                let keypair = elgamal::KeyPair::generate(rng);
                let message = Fr::rand(rng);
                let ct = keypair.enc_key().encrypt(rng, &[message]);

                let audit_memo = AuditMemo::new(ct);
                assert_eq!(
                    audit_memo.clone(),
                    contract
                        .check_audit_memo(audit_memo.generic_into::<sol::AuditMemo>())
                        .call()
                        .await?
                        .generic_into::<AuditMemo>()
                );
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
            miner_addr: miner.address().into(),
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
        async fn test_plonk_proof_and_txn_notes() -> Result<()> {
            let rng = &mut ark_std::test_rng();
            let contract = deploy_type_contract().await?;
            let num_transfer_txn = 1;
            let num_mint_txn = 1;
            let num_freeze_txn = 1;
            let params =
                TxnsParams::generate_txns(rng, num_transfer_txn, num_mint_txn, num_freeze_txn);

            for txn in params.txns {
                let proof = txn.validity_proof();
                assert_eq!(
                    proof.clone(),
                    contract
                        .check_plonk_proof(proof.into())
                        .call()
                        .await?
                        .generic_into::<Proof<Bn254>>()
                );

                match txn {
                    TransactionNote::Transfer(note) => {
                        assert_eq!(
                            *note.clone(),
                            contract
                                .check_transfer_note((*note).generic_into::<sol::TransferNote>())
                                .call()
                                .await?
                                .generic_into::<TransferNote>()
                        )
                    }
                    TransactionNote::Mint(note) => {
                        assert_eq!(
                            *note.clone(),
                            contract
                                .check_mint_note((*note).generic_into::<sol::MintNote>())
                                .call()
                                .await?
                                .generic_into::<MintNote>()
                        )
                    }
                    TransactionNote::Freeze(note) => {
                        assert_eq!(
                            *note.clone(),
                            contract
                                .check_freeze_note((*note).generic_into::<sol::FreezeNote>())
                                .call()
                                .await?
                                .generic_into::<FreezeNote>()
                        )
                    }
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
            Ok(())
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
