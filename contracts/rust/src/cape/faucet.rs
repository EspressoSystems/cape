#![cfg(test)]

use crate::{
    assertion::Matcher,
    deploy::deploy_cape_test_with_deployer,
    ethereum::get_funded_client,
    state::CAPE_MERKLE_HEIGHT,
    types::{field_to_u256, TestCAPE},
};
use anyhow::Result;
use jf_cap::{
    keys::UserKeyPair,
    structs::{AssetDefinition, BlindFactor, FreezeFlag, RecordCommitment, RecordOpening},
    BaseField, MerkleTree,
};

#[tokio::test]
async fn test_faucet() -> Result<()> {
    let rng = &mut ark_std::test_rng();
    let deployer = get_funded_client().await?;
    let non_deployer = get_funded_client().await?;
    let contract = deploy_cape_test_with_deployer(deployer.clone()).await;
    let faucet_manager = UserKeyPair::generate(rng);

    // after Cape deployment, faucet is yet setup
    assert!(!contract.faucet_initialized().call().await?);
    assert_eq!(contract.faucet_setter().call().await?, deployer.address());

    // attempts to setup faucet by non deployer should fail
    let contract = TestCAPE::new(contract.address(), non_deployer);
    contract
        .faucet_setup_for_testnet(faucet_manager.address().into())
        .send()
        .await
        .should_revert_with_message("Only invocable by deployer");

    // setting up
    let contract = TestCAPE::new(contract.address(), deployer);
    contract
        .faucet_setup_for_testnet(faucet_manager.address().into())
        .send()
        .await?
        .await?;
    assert!(contract.faucet_initialized().call().await?);

    // try to setup again should fail
    contract
        .faucet_setup_for_testnet(faucet_manager.address().into())
        .send()
        .await
        .should_revert_with_message("Faucet already set up");

    // check the native token record is properly allocated
    let ro = RecordOpening {
        amount: u64::MAX,
        asset_def: AssetDefinition::native(),
        pub_key: faucet_manager.pub_key(),
        freeze_flag: FreezeFlag::Unfrozen,
        blind: BlindFactor::from(BaseField::from(0u32)),
    };

    let mut mt = MerkleTree::new(CAPE_MERKLE_HEIGHT).unwrap();
    mt.push(RecordCommitment::from(&ro).to_field_element());
    let root: ark_bn254::Fr = mt.commitment().root_value.to_scalar();

    assert!(contract.contains_root(field_to_u256(root)).call().await?);

    Ok(())
}
