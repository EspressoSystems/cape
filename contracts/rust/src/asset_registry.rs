#![cfg(test)]
use std::path::Path;

use anyhow::Result;
use ethers::prelude::Address;

use crate::assertion::Matcher;
use crate::ethereum::{deploy, get_funded_deployer};
use crate::types::{self as sol, AssetRegistry};

#[tokio::test]
async fn test_asset_registry() -> Result<()> {
    let client = get_funded_deployer().await?;
    let contract = deploy(
        client.clone(),
        Path::new("../abi/contracts/AssetRegistry.sol/AssetRegistry"),
        (),
    )
    .await?;
    let contract = AssetRegistry::new(contract.address(), client);

    let address = Address::random();
    let asset_def = sol::AssetDefinition::default();

    // Fails for default/zero address
    contract
        .sponsor_cape_asset(Address::zero(), asset_def.clone())
        .call()
        .await
        .should_revert_with_message("Bad asset address");

    // Unknown asset is not registered
    let registered = contract
        .is_cape_asset_registered(asset_def.clone())
        .call()
        .await
        .unwrap();
    assert!(!registered);

    // Register the asset
    contract
        .sponsor_cape_asset(address, asset_def.clone())
        .send()
        .await?
        .await?;

    // Asset is now registered
    let registered = contract
        .is_cape_asset_registered(asset_def.clone())
        .call()
        .await
        .unwrap();

    assert!(registered);

    // Asset cannot be registered again
    contract
        .sponsor_cape_asset(address, asset_def.clone())
        .call()
        .await
        .should_revert_with_message("Asset already registered");

    Ok(())
}
