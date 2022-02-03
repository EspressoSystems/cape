#![cfg(test)]

use crate::assertion::Matcher;
use crate::cape::{CapeBlock, DOM_SEP_CAPE_BURN};
use crate::deploy::deploy_cape_test;
use crate::ledger::CapeLedger;
use crate::types as sol;
use crate::types::{GenericInto, MerkleRootSol};
use anyhow::Result;
use ethers::prelude::U256;
use jf_aap::keys::UserPubKey;
use jf_aap::utils::TxnsParams;
use jf_aap::TransactionNote;
use reef::Ledger;

// TODO add a test to check if includedNotes is computed correctly

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
async fn test_contains_burn_prefix() {
    let contract = deploy_cape_test().await;

    let dom_sep_str = std::str::from_utf8(DOM_SEP_CAPE_BURN).unwrap();
    for (input, expected) in [
        ("", false),
        ("x", false),
        ("TRICAPE bur", false),
        ("more data but but still not a burn", false),
        (dom_sep_str, true),
        (&(dom_sep_str.to_owned() + "more stuff"), true),
    ] {
        assert_eq!(
            contract
                .contains_burn_prefix(input.as_bytes().to_vec().into())
                .call()
                .await
                .unwrap(),
            expected
        )
    }
}

#[tokio::test]
async fn test_contains_burn_record() {
    let contract = deploy_cape_test().await;

    assert!(!contract
        .contains_burn_record(sol::BurnNote::default())
        .call()
        .await
        .unwrap());

    // TODO test with a valid note
    // let mut rng = ark_std::test_rng();
    // let note = TransferNote::...
    // let burned_ro = RecordOpening::rand_for_test(&mut rng);
    // let burn_note = BurnNote::generate(note, burned_ro);
    // assert!(contract.contains_burn_record(burn_note).call().await.unwrap());
}

#[tokio::test]
async fn test_check_burn_bad_prefix() {
    let contract = deploy_cape_test().await;
    let mut note = sol::BurnNote::default();
    let extra = [
        hex::decode("ffffffffffffffffffffffff").unwrap(),
        hex::decode(b"0000000000000000000000000000000000000000").unwrap(),
    ]
    .concat();
    note.transfer_note.aux_info.extra_proof_bound_data = extra.into();

    let call = contract.check_burn(note).call().await;
    call.should_revert_with_message("Bad burn tag");
}

#[tokio::test]
async fn test_check_burn_bad_record_commitment() {
    let contract = deploy_cape_test().await;
    let mut note = sol::BurnNote::default();
    let extra = [
        DOM_SEP_CAPE_BURN.to_vec(),
        hex::decode("0000000000000000000000000000000000000000").unwrap(),
    ]
    .concat();
    note.transfer_note.aux_info.extra_proof_bound_data = extra.into();

    note.transfer_note.output_commitments.push(U256::from(1));
    note.transfer_note.output_commitments.push(U256::from(2));

    let call = contract.check_burn(note).call().await;
    call.should_revert_with_message("Bad record commitment");
}

// TODO Add test for check_burn that passes

// TODO integration test to check if check_burn is hooked up correctly in
// main block validation loop.

#[tokio::test]
async fn test_check_transfer_expired_note_triggers_an_error() -> Result<()> {
    let rng = &mut ark_std::test_rng();
    let params = TxnsParams::generate_txns(rng, 1, 0, 0, CapeLedger::merkle_height());
    let miner = UserPubKey::default();

    let tx = params.txns[0].clone();
    let root = tx.merkle_root();

    let cape_block = CapeBlock::generate(params.txns, vec![], miner.address())?;
    let valid_until = match tx {
        TransactionNote::Transfer(note) => note.aux_info.valid_until,
        TransactionNote::Mint(_) => todo!(),
        TransactionNote::Freeze(_) => todo!(),
    };

    // Set the height to expire note
    let contract = deploy_cape_test().await;
    contract.set_height(valid_until + 1).send().await?.await?;

    contract
        .add_root(root.generic_into::<MerkleRootSol>().0)
        .send()
        .await?
        .await?;

    contract
        .submit_cape_block(cape_block.into())
        .call()
        .await
        .should_revert_with_message("Expired note");

    Ok(())
}

#[tokio::test]
async fn test_check_transfer_note_with_burn_prefix_rejected() {
    let contract = deploy_cape_test().await;
    let mut note = sol::TransferNote::default();
    let extra = [
        DOM_SEP_CAPE_BURN.to_vec(),
        hex::decode("0000000000000000000000000000000000000000").unwrap(),
    ]
    .concat();
    note.aux_info.extra_proof_bound_data = extra.into();

    let call = contract.check_transfer(note).call().await;
    call.should_revert_with_message("Burn prefix in transfer note");
}

#[tokio::test]
async fn test_check_transfer_without_burn_prefix_accepted() {
    let contract = deploy_cape_test().await;
    let note = sol::TransferNote::default();
    assert!(contract.check_transfer(note).call().await.is_ok());
}
