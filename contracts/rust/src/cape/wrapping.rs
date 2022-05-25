// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

#![cfg(test)]

use crate::assertion::Matcher;
use crate::deploy::deploy_test_cape;
use crate::model::{erc20_asset_description, Erc20Code, EthereumAddr};
use crate::types as sol;
use crate::types::{AssetCodeSol, GenericInto};
use anyhow::Result;
use ethers::prelude::Address;
use jf_cap::structs::{AssetCode, AssetCodeSeed, AssetPolicy, RecordOpening};

mod errors_when_calling_deposit_erc20 {
    use std::sync::Arc;

    use super::*;
    use crate::assertion::EnsureMined;
    use crate::deploy::{deploy_erc20_token, deploy_test_cape};
    use crate::model::{erc20_asset_description, Erc20Code, EthereumAddr};
    use crate::types as sol;
    use crate::types::TestCAPE;
    use anyhow::Result;
    use ethers::prelude::U256;
    use jf_cap::keys::UserPubKey;
    use jf_cap::structs::{AssetCode, AssetDefinition, FreezeFlag, RecordOpening};

    enum Scenario {
        AssetTypeNotRegistered,
        SkipApproval,
        PendingDepositsQueueIsFull,
        WrongERC20Address,
    }

    async fn call_deposit_erc20_with_error_helper(
        expected_error_message: &str,
        scenario: Scenario,
    ) -> Result<()> {
        let cape_contract = deploy_test_cape().await;

        // Deploy ERC20 token contract. The caller of this method receives 1000 * 10**18 tokens
        let erc20_token_contract = deploy_erc20_token().await;

        let owner_of_erc20_tokens_client = erc20_token_contract.client().clone();
        let owner_of_erc20_tokens_client_address = owner_of_erc20_tokens_client.address();
        let erc20_address = erc20_token_contract.address();

        // Approve
        let contract_address = cape_contract.address();
        let deposited_amount = 1000u64;

        let amount_u256 = U256::from(deposited_amount);

        if !(matches!(scenario, Scenario::SkipApproval)) {
            erc20_token_contract
                .approve(contract_address, amount_u256)
                .send()
                .await?
                .await?
                .ensure_mined();
        };

        // Sponsor asset type
        let rng = &mut ark_std::test_rng();
        let erc20_code = Erc20Code(EthereumAddr(erc20_address.to_fixed_bytes()));
        let sponsor = owner_of_erc20_tokens_client_address;

        let policy = AssetPolicy::rand_for_test(rng);
        let description = erc20_asset_description(
            &erc20_code,
            &EthereumAddr(sponsor.to_fixed_bytes()),
            policy.clone(),
        );
        let asset_code = AssetCode::new_foreign(&description);
        let asset_def = AssetDefinition::new(asset_code, policy).unwrap();
        let asset_def_sol = asset_def.clone().generic_into::<sol::AssetDefinition>();

        if !(matches!(scenario, Scenario::AssetTypeNotRegistered)) {
            TestCAPE::new(
                cape_contract.address(),
                Arc::new(owner_of_erc20_tokens_client.clone()),
            )
            .sponsor_cape_asset(erc20_address, asset_def_sol)
            .send()
            .await?
            .await?
            .ensure_mined();
        }

        if matches!(scenario, Scenario::PendingDepositsQueueIsFull) {
            cape_contract
                .fill_up_pending_deposits_queue()
                .send()
                .await?
                .await?
                .ensure_mined();
        };

        // Build record opening
        let ro = RecordOpening::new(
            rng,
            deposited_amount.into(),
            asset_def,
            UserPubKey::default(),
            FreezeFlag::Unfrozen,
        );

        // We call the CAPE contract from the address that owns the ERC20 tokens
        let call = cape_contract
            .deposit_erc_20(
                ro.clone().generic_into::<sol::RecordOpening>(),
                if matches!(scenario, Scenario::WrongERC20Address) {
                    Address::random()
                } else {
                    erc20_address
                },
            )
            .from(owner_of_erc20_tokens_client_address)
            .call()
            .await;

        call.should_revert_with_message(expected_error_message);

        Ok(())
    }

    #[tokio::test]
    async fn the_asset_type_is_not_registered() -> Result<()> {
        call_deposit_erc20_with_error_helper(
            "Asset definition not registered",
            Scenario::AssetTypeNotRegistered,
        )
        .await
    }

    #[tokio::test]
    async fn the_erc20_token_were_not_approved_before_calling_deposit_erc20() -> Result<()> {
        call_deposit_erc20_with_error_helper(
            "ERC20: transfer amount exceeds allowance",
            Scenario::SkipApproval,
        )
        .await
    }

    #[tokio::test]
    async fn the_erc20_tok() -> Result<()> {
        call_deposit_erc20_with_error_helper(
            "Pending deposits queue is full",
            Scenario::PendingDepositsQueueIsFull,
        )
        .await
    }

    #[tokio::test]
    async fn erc20_address_mismatch_asset_definition_fails() -> Result<()> {
        call_deposit_erc20_with_error_helper("Wrong ERC20 address", Scenario::WrongERC20Address)
            .await
    }
}

#[tokio::test]
async fn test_erc20_description() -> Result<()> {
    let rng = &mut ark_std::test_rng();
    let contract = deploy_test_cape().await;
    let sponsor = Address::random();
    let asset_address = Address::random();
    let asset_code = Erc20Code(EthereumAddr(asset_address.to_fixed_bytes()));
    let policy = AssetPolicy::rand_for_test(rng);
    let description = erc20_asset_description(
        &asset_code,
        &EthereumAddr(sponsor.to_fixed_bytes()),
        policy.clone(),
    );
    let ret = contract
        .compute_asset_description(asset_address, sponsor, policy.into())
        .call()
        .await?;
    assert_eq!(ret.to_vec(), description);
    Ok(())
}

#[tokio::test]
async fn test_check_foreign_asset_code() -> Result<()> {
    let contract = deploy_test_cape().await;

    // Fails for random record opening with random asset code.
    let rng = &mut ark_std::test_rng();
    let ro = RecordOpening::rand_for_test(rng);
    contract
        .check_foreign_asset_code(
            ro.asset_def.code.generic_into::<sol::AssetCodeSol>().0,
            Address::random(),
            Address::random(),
            ro.asset_def.policy_ref().clone().into(),
        )
        .call()
        .await
        .should_revert_with_message("Wrong foreign asset code");

    let erc20_address = Address::random();
    // This is the first account from the test mnemonic
    let sponsor = "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266".parse::<Address>()?;
    let erc20_code = Erc20Code(EthereumAddr(erc20_address.to_fixed_bytes()));

    // Fails for domestic asset code.
    let domestic_asset_code =
        AssetCode::new_domestic(AssetCodeSeed::generate(rng), erc20_address.as_bytes());
    contract
        .check_foreign_asset_code(
            domestic_asset_code.generic_into::<AssetCodeSol>().0,
            erc20_address,
            sponsor,
            AssetPolicy::rand_for_test(rng).into(),
        )
        .call()
        .await
        .should_revert_with_message("Wrong foreign asset code");

    // Fails if txn sender address does not match sponsor in asset code.
    let policy_wrong_sponsor = AssetPolicy::rand_for_test(rng);
    let description_wrong_sponsor = erc20_asset_description(
        &erc20_code,
        &EthereumAddr(Address::random().to_fixed_bytes()),
        policy_wrong_sponsor.clone(),
    );
    let asset_code_wrong_sponsor = AssetCode::new_foreign(&description_wrong_sponsor);
    contract
        .check_foreign_asset_code(
            asset_code_wrong_sponsor.generic_into::<AssetCodeSol>().0,
            sponsor,
            sponsor,
            policy_wrong_sponsor.into(),
        )
        .call()
        .await
        .should_revert_with_message("Wrong foreign asset code");

    let policy = AssetPolicy::rand_for_test(rng);
    let description = erc20_asset_description(
        &erc20_code,
        &EthereumAddr(sponsor.to_fixed_bytes()),
        policy.clone(),
    );
    let asset_code = AssetCode::new_foreign(&description);

    // Fails for random erc20 address.
    contract
        .check_foreign_asset_code(
            asset_code.generic_into::<sol::AssetCodeSol>().0,
            Address::random(),
            sponsor,
            policy.clone().into(),
        )
        .call()
        .await
        .should_revert_with_message("Wrong foreign asset code");

    // Fails for random policy.
    contract
        .check_foreign_asset_code(
            asset_code.generic_into::<sol::AssetCodeSol>().0,
            erc20_address,
            sponsor,
            AssetPolicy::rand_for_test(rng).into(),
        )
        .call()
        .await
        .should_revert_with_message("Wrong foreign asset code");

    // Passes for correctly derived asset code
    contract
        .check_foreign_asset_code(
            asset_code.generic_into::<sol::AssetCodeSol>().0,
            erc20_address,
            sponsor,
            policy.into(),
        )
        .call()
        .await
        .should_not_revert();

    Ok(())
}
