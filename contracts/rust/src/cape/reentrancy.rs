// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

#![cfg(test)]
use crate::assertion::Matcher;
use crate::deploy::{deploy_cape_test, deploy_malicious_erc20_token};
use crate::model::{erc20_asset_description, Erc20Code, EthereumAddr};
use crate::types::{self as sol};
use crate::types::{GenericInto, TestCAPE};
use anyhow::Result;
use ethers::prelude::U256;
use jf_cap::keys::UserPubKey;
use jf_cap::structs::{AssetCode, AssetDefinition, AssetPolicy, FreezeFlag, RecordOpening};
use std::sync::Arc;

#[tokio::test]
async fn test_reentrancy_guard() -> Result<()> {
    let cape_contract = deploy_cape_test().await;
    let cape_contract_address = cape_contract.address();
    let malicious_erc20_contract = deploy_malicious_erc20_token().await;
    let malicious_erc20_address = malicious_erc20_contract.address();

    // Register asset with malicious erc20
    let rng = &mut ark_std::test_rng();
    let erc20_code = Erc20Code(EthereumAddr(malicious_erc20_address.to_fixed_bytes()));

    let owner_of_erc20_tokens_client = malicious_erc20_contract.client().clone();
    let owner_of_erc20_tokens_client_address = owner_of_erc20_tokens_client.address();

    let cape_contract_erc20_owner = TestCAPE::new(
        cape_contract.address(),
        Arc::new(owner_of_erc20_tokens_client.clone()),
    );

    let sponsor = owner_of_erc20_tokens_client_address;

    let description = erc20_asset_description(&erc20_code, &EthereumAddr(sponsor.to_fixed_bytes()));
    let asset_code = AssetCode::new_foreign(&description);
    let asset_def = AssetDefinition::new(asset_code, AssetPolicy::rand_for_test(rng)).unwrap();
    let asset_def_sol = asset_def.clone().generic_into::<sol::AssetDefinition>();

    cape_contract_erc20_owner
        .sponsor_cape_asset(malicious_erc20_address, asset_def_sol)
        .send()
        .await?
        .await?;

    // Call CAPE.deposit and check error
    let deposited_amount = 1000;
    let amount_u256 = U256::from(deposited_amount);
    malicious_erc20_contract
        .approve(cape_contract_address, amount_u256)
        .send()
        .await?
        .await?;

    // Build record opening
    let ro = RecordOpening::new(
        rng,
        deposited_amount,
        asset_def,
        UserPubKey::default(),
        FreezeFlag::Unfrozen,
    );

    // By default no error is triggered
    let call = cape_contract
        .deposit_erc_20(
            ro.clone().generic_into::<sol::RecordOpening>(),
            malicious_erc20_address,
        )
        .from(owner_of_erc20_tokens_client_address)
        .call()
        .await;
    assert!(call.is_ok());

    // Set the CAPE contract address as the target of the malicious contract
    malicious_erc20_contract
        .set_target_contract_address(cape_contract_address)
        .send()
        .await?
        .await?;

    // Decide to run CAPE.depositErc20 when call MaliciousContract.transferFrom
    malicious_erc20_contract.run_deposit().send().await?.await?;

    let call = cape_contract
        .deposit_erc_20(
            ro.clone().generic_into::<sol::RecordOpening>(),
            malicious_erc20_address,
        )
        .from(owner_of_erc20_tokens_client_address)
        .call()
        .await;

    call.should_revert_with_message("ReentrancyGuard: reentrant call");

    // Decide to run CAPE.submitBlock when call MaliciousContract.transferFrom
    malicious_erc20_contract
        .run_submit_block()
        .send()
        .await?
        .await?;

    let call = cape_contract
        .deposit_erc_20(
            ro.clone().generic_into::<sol::RecordOpening>(),
            malicious_erc20_address,
        )
        .from(owner_of_erc20_tokens_client_address)
        .call()
        .await;

    call.should_revert_with_message("ReentrancyGuard: reentrant call");

    Ok(())
}
