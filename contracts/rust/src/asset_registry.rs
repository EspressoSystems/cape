#![cfg(test)]
use anyhow::Result;
use ethers::prelude::Address;
use jf_cap::structs::{AssetCode, AssetDefinition, AssetPolicy};

use crate::assertion::Matcher;
use crate::deploy::deploy_test_asset_registry_contract;
use crate::ethereum::get_funded_client;
use crate::model::{erc20_asset_description, Erc20Code, EthereumAddr};
use crate::types::AssetRegistry;

#[tokio::test]
async fn test_native_domestic_asset() -> Result<()> {
    let contract = deploy_test_asset_registry_contract().await;
    assert_eq!(
        contract.native_domestic_asset().call().await?,
        AssetDefinition::native().into()
    );
    Ok(())
}

#[tokio::test]
async fn test_asset_registry() -> Result<()> {
    let erc20_address = Address::random();
    let sponsor = get_funded_client().await?;
    // Send transactions signed by the sponsor's wallet
    let contract = deploy_test_asset_registry_contract().await;

    let erc20_code = Erc20Code(EthereumAddr(erc20_address.to_fixed_bytes()));

    let description = erc20_asset_description(
        &erc20_code,
        &EthereumAddr(sponsor.address().to_fixed_bytes()),
    );
    let asset_code = AssetCode::new_foreign(&description);
    let asset_def = AssetDefinition::new(asset_code, AssetPolicy::default())?;

    // Fails for default/zero address
    contract
        .sponsor_cape_asset(Address::zero(), asset_def.clone().into())
        .from(sponsor.address())
        .call()
        .await
        .should_revert_with_message("Bad asset address");

    // Unknown asset is not registered
    let registered = contract
        .is_cape_asset_registered(asset_def.clone().into())
        .from(sponsor.address())
        .call()
        .await?;
    assert!(!registered);

    // Cannot register as wrong sponsor
    contract
        .sponsor_cape_asset(erc20_address, asset_def.clone().into())
        .from(Address::random())
        .call()
        .await
        .should_revert_with_message("Wrong foreign asset code");

    // Register the asset
    let contract = AssetRegistry::new(contract.address(), sponsor.clone());
    contract
        .sponsor_cape_asset(erc20_address, asset_def.clone().into())
        .send()
        .await?
        .await?;

    // Asset is now registered
    let registered = contract
        .is_cape_asset_registered(asset_def.clone().into())
        .call()
        .await
        .unwrap();

    assert!(registered);

    // Asset cannot be registered again
    contract
        .sponsor_cape_asset(erc20_address, asset_def.clone().into())
        .from(sponsor.address())
        .call()
        .await
        .should_revert_with_message("Asset already registered");

    Ok(())
}
