#![cfg(test)]

use crate::assertion::Matcher;
use crate::deploy::deploy_cape_test;
use crate::state::{erc20_asset_description, Erc20Code, EthereumAddr};
use crate::types as sol;
use crate::types::{AssetCodeSol, GenericInto};
use anyhow::Result;
use ethers::prelude::Address;
use jf_aap::structs::{AssetCode, AssetCodeSeed, RecordOpening};

mod errors_when_calling_deposit_erc20 {
    use std::sync::Arc;

    use super::*;
    use crate::deploy::{deploy_cape_test, deploy_erc20_token};
    use crate::state::{erc20_asset_description, Erc20Code, EthereumAddr};
    use crate::types as sol;
    use crate::types::TestCAPE;
    use anyhow::Result;
    use ethers::prelude::U256;
    use jf_aap::keys::UserPubKey;
    use jf_aap::structs::{AssetCode, AssetDefinition, AssetPolicy, FreezeFlag, RecordOpening};

    enum WrongCallDepositErc20 {
        AssetTypeNotRegistered,
        SkipApproval,
    }

    async fn call_deposit_erc20_with_error_helper(
        expected_error_message: &str,
        wrong_call: WrongCallDepositErc20,
    ) -> Result<()> {
        let cape_contract = deploy_cape_test().await;

        // Deploy ERC20 token contract. The caller of this method receives 1000 * 10**18 tokens
        let erc20_token_contract = deploy_erc20_token().await;

        let owner_of_erc20_tokens_client = erc20_token_contract.client().clone();
        let owner_of_erc20_tokens_client_address = owner_of_erc20_tokens_client.address();
        let erc20_address = erc20_token_contract.address();

        // Approve
        let contract_address = cape_contract.address();
        let deposited_amount = 1000;

        let amount_u256 = U256::from(deposited_amount);

        match wrong_call {
            WrongCallDepositErc20::AssetTypeNotRegistered => {
                erc20_token_contract
                    .approve(contract_address, amount_u256)
                    .send()
                    .await?
                    .await?;
            }
            WrongCallDepositErc20::SkipApproval => {}
        };

        // Call CAPE contract function

        // Sponsor asset type
        let rng = &mut ark_std::test_rng();
        let erc20_code = Erc20Code(EthereumAddr(erc20_address.to_fixed_bytes()));
        let sponsor = owner_of_erc20_tokens_client_address;

        let description =
            erc20_asset_description(&erc20_code, &EthereumAddr(sponsor.to_fixed_bytes()));
        let asset_code = AssetCode::new_foreign(&description);
        let asset_def = AssetDefinition::new(asset_code, AssetPolicy::rand_for_test(rng)).unwrap();
        let asset_def_sol = asset_def.clone().generic_into::<sol::AssetDefinition>();

        match wrong_call {
            WrongCallDepositErc20::SkipApproval => {
                TestCAPE::new(
                    cape_contract.address(),
                    Arc::new(owner_of_erc20_tokens_client.clone()),
                )
                .sponsor_cape_asset(erc20_address, asset_def_sol)
                .send()
                .await?
                .await?;
            }
            WrongCallDepositErc20::AssetTypeNotRegistered => {}
        }

        // Build record opening
        let ro = RecordOpening::new(
            rng,
            deposited_amount,
            asset_def,
            UserPubKey::default(),
            FreezeFlag::Unfrozen,
        );

        // We call the CAPE contract from the address that owns the ERC20 tokens
        let call = cape_contract
            .deposit_erc_20(
                ro.clone().generic_into::<sol::RecordOpening>(),
                erc20_address,
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
            WrongCallDepositErc20::AssetTypeNotRegistered,
        )
        .await
    }

    #[tokio::test]
    async fn the_erc20_token_were_not_approved_before_calling_deposit_erc20() -> Result<()> {
        call_deposit_erc20_with_error_helper(
            "ERC20: transfer amount exceeds allowance",
            WrongCallDepositErc20::SkipApproval,
        )
        .await
    }
}

#[tokio::test]
async fn test_erc20_description() -> Result<()> {
    let contract = deploy_cape_test().await;
    let sponsor = Address::random();
    let asset_address = Address::random();
    let asset_code = Erc20Code(EthereumAddr(asset_address.to_fixed_bytes()));
    let description = erc20_asset_description(&asset_code, &EthereumAddr(sponsor.to_fixed_bytes()));
    let ret = contract
        .compute_asset_description(asset_address, sponsor)
        .call()
        .await?;
    assert_eq!(ret.to_vec(), description);
    Ok(())
}

#[tokio::test]
async fn test_check_foreign_asset_code() -> Result<()> {
    let contract = deploy_cape_test().await;

    // Fails for random record opening with random asset code.
    let rng = &mut ark_std::test_rng();
    let ro = RecordOpening::rand_for_test(rng);
    contract
        .check_foreign_asset_code(
            ro.asset_def.code.generic_into::<sol::AssetCodeSol>().0,
            Address::random(),
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
        )
        .from(sponsor)
        .call()
        .await
        .should_revert_with_message("Wrong foreign asset code");

    // Fails if txn sender address does not match sponsor in asset code.
    let description_wrong_sponsor = erc20_asset_description(
        &erc20_code,
        &EthereumAddr(Address::random().to_fixed_bytes()),
    );
    let asset_code_wrong_sponsor = AssetCode::new_foreign(&description_wrong_sponsor);
    contract
        .check_foreign_asset_code(
            asset_code_wrong_sponsor.generic_into::<AssetCodeSol>().0,
            sponsor,
        )
        .from(sponsor)
        .call()
        .await
        .should_revert_with_message("Wrong foreign asset code");

    let description = erc20_asset_description(&erc20_code, &EthereumAddr(sponsor.to_fixed_bytes()));
    let asset_code = AssetCode::new_foreign(&description);

    // Fails for random erc20 address.
    contract
        .check_foreign_asset_code(
            asset_code.generic_into::<sol::AssetCodeSol>().0,
            Address::random(),
        )
        .from(sponsor)
        .call()
        .await
        .should_revert_with_message("Wrong foreign asset code");

    // Passes for correctly derived asset code
    contract
        .check_foreign_asset_code(
            asset_code.generic_into::<sol::AssetCodeSol>().0,
            erc20_address,
        )
        .from(sponsor)
        .call()
        .await
        .should_not_revert();

    Ok(())
}
