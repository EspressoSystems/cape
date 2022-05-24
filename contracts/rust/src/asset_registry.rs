// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

#![cfg(test)]
use anyhow::Result;
use ethers::prelude::Address;
use jf_cap::structs::{AssetCode, AssetDefinition, AssetPolicy};

use crate::assertion::Matcher;
use crate::deploy::deploy_test_asset_registry_contract;
use crate::ethereum::get_funded_client;
use crate::model::{erc20_asset_description, Erc20Code, EthereumAddr};
use crate::types::{AssetCodeSol, AssetRegistry, GenericInto};

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
        AssetPolicy::default(),
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

    // Check the record commitments for the deposits were emitted
    let logs = contract
        .asset_sponsored_filter()
        .from_block(0u64)
        .query()
        .await?;

    assert_eq!(logs[0].erc_20_address, erc20_address);

    assert_eq!(
        logs[0].asset_definition_code,
        asset_code.generic_into::<AssetCodeSol>().0
    );

    // Lookup the address given the asset definition
    let address = contract
        .lookup(asset_def.clone().into())
        .call()
        .await
        .unwrap();
    assert_eq!(address, erc20_address);

    // Asset cannot be registered again
    contract
        .sponsor_cape_asset(erc20_address, asset_def.clone().into())
        .from(sponsor.address())
        .call()
        .await
        .should_revert_with_message("Asset already registered");

    Ok(())
}
