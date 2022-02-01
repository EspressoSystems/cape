#![cfg(test)]
use anyhow::Result;
use ethers::prelude::*;
use jf_aap::structs::{AssetCode, AssetCodeSeed, AssetDefinition, AssetPolicy, RecordOpening};

use crate::assertion::Matcher;
use crate::deploy::deploy_cape_test;
use crate::ethereum::get_funded_client;
use crate::state::{erc20_asset_description, Erc20Code, EthereumAddr};
use crate::types::{AssetCodeSol, GenericInto, TestCAPE};

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
    let erc20_address = Address::random();
    let sponsor_address = Address::random();
    let erc20_code = Erc20Code(EthereumAddr(erc20_address.to_fixed_bytes()));

    // Fails for random record opening with random asset code.
    let rng = &mut ark_std::test_rng();
    let ro = RecordOpening::rand_for_test(rng);
    contract
        .check_foreign_asset_code(
            ro.asset_def.code.generic_into::<AssetCodeSol>().0,
            Address::random(),
        )
        .from(sponsor_address)
        .call()
        .await
        .should_revert_with_message("Wrong foreign asset code");

    // Fails for domestic asset code.
    let domestic_asset_code =
        AssetCode::new_domestic(AssetCodeSeed::generate(rng), erc20_address.as_bytes());
    contract
        .check_foreign_asset_code(
            domestic_asset_code.generic_into::<AssetCodeSol>().0,
            erc20_address,
        )
        .from(sponsor_address)
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
            erc20_address,
        )
        .from(sponsor_address)
        .call()
        .await
        .should_revert_with_message("Wrong foreign asset code");

    let description =
        erc20_asset_description(&erc20_code, &EthereumAddr(sponsor_address.to_fixed_bytes()));
    let asset_code = AssetCode::new_foreign(&description);

    // Fails for random erc20 address.
    contract
        .check_foreign_asset_code(
            asset_code.generic_into::<AssetCodeSol>().0,
            Address::random(),
        )
        .from(sponsor_address)
        .call()
        .await
        .should_revert_with_message("Wrong foreign asset code");

    // Fails if not called by correct sponsor
    contract
        .check_foreign_asset_code(asset_code.generic_into::<AssetCodeSol>().0, erc20_address)
        .from(Address::random())
        .call()
        .await
        .should_revert_with_message("Wrong foreign asset code");

    // Passes for correctly derived asset code
    contract
        .check_foreign_asset_code(asset_code.generic_into::<AssetCodeSol>().0, erc20_address)
        .from(sponsor_address)
        .call()
        .await
        .should_not_revert();

    Ok(())
}

#[tokio::test]
async fn test_asset_registry() -> Result<()> {
    let contract = deploy_cape_test().await;

    let erc20_address = Address::random();

    let sponsor = get_funded_client().await?;
    // Send transactions signed by the sponsor's wallet
    let contract = TestCAPE::new(contract.address(), sponsor.clone());

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
