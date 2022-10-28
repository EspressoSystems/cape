// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use anyhow::Result;
use cap_rust_sandbox::assertion::EnsureMined;
use cap_rust_sandbox::cape::CapeBlock;
use cap_rust_sandbox::deploy::{deploy_cape, deploy_erc20_token};
use cap_rust_sandbox::ethereum::get_funded_client;
use cap_rust_sandbox::helpers::compute_faucet_key_pair_from_mnemonic;
use cap_rust_sandbox::ledger::CapeLedger;
use cap_rust_sandbox::model::{erc20_asset_description, Erc20Code, EthereumAddr};
use cap_rust_sandbox::test_utils::{
    compare_roots_records_test_cape_contract, compute_faucet_record_opening, create_faucet,
    generate_burn_tx, ContractsInfo, PrintGas,
};
use cap_rust_sandbox::types as sol;
use cap_rust_sandbox::types::{GenericInto, CAPE};
use core::str::FromStr;
use ethers::prelude::{H160, U256};
use jf_cap::keys::{CredIssuerPubKey, UserKeyPair, UserPubKey};
use jf_cap::structs::{
    AssetCode, AssetDefinition, AssetPolicy, FreezeFlag, RecordCommitment, RecordOpening,
};
use jf_cap::{MerkleTree, TransactionNote};
use reef::Ledger;
use seahorse::hd::Mnemonic;
use std::env;

#[tokio::test]
async fn smoke_tests() -> Result<()> {
    // Deploy the contracts
    let cape_contract = match env::var("CAPE_CONTRACT_ADDRESS") {
        Ok(address) => {
            println!("Using existing CAPE contract deployment at {}.", address);
            let deployer = get_funded_client().await.unwrap();
            CAPE::new(H160::from_str(&address).unwrap(), deployer)
        }
        Err(_) => {
            println!("Deploying CAPE contract.");
            deploy_cape().await
        }
    };

    // Deploy ERC20 token contract. The client deploying the erc20 token contract receives 1000 * 10**18 tokens
    println!("Deploying ERC20 contract for testing.");
    let erc20_token_contract = deploy_erc20_token().await;

    let contracts_info = ContractsInfo::new(&cape_contract, &erc20_token_contract).await;

    // Generate a new client that will receive the unwrapped assets
    let final_recipient_of_unwrapped_assets = get_funded_client().await?;

    let mut mt = MerkleTree::new(CapeLedger::merkle_height()).unwrap();

    // Create some fee asset record if needed
    let (faucet_key_pair, faucet_record_opening) = if env::var("CAPE_CONTRACT_ADDRESS").is_err() {
        println!("Creating test faucet keypair.");
        create_faucet(&cape_contract, None).await
    } else {
        println!("Deriving faucet keypair from faucet manager mnemonic.");
        let faucet_mnemonic_str = env::var("CAPE_FAUCET_MANAGER_MNEMONIC").unwrap();
        let mnemonic = Mnemonic::from_phrase(faucet_mnemonic_str.replace('-', " ")).unwrap();
        let faucet_key_pair: UserKeyPair = compute_faucet_key_pair_from_mnemonic(&mnemonic);

        let ro = compute_faucet_record_opening(faucet_key_pair.pub_key());
        (faucet_key_pair, ro)
    };
    let faucet_record_comm = RecordCommitment::from(&faucet_record_opening).to_field_element();
    mt.push(faucet_record_comm);

    // Sponsor CAPE asset
    let rng = &mut ark_std::test_rng();
    let erc20_code = Erc20Code(EthereumAddr(
        contracts_info.erc20_token_address.to_fixed_bytes(),
    ));

    // We use an asset policy that does not track the user's credential.
    let asset_policy =
        AssetPolicy::rand_for_test(rng).set_cred_issuer_pub_key(CredIssuerPubKey::default());
    let sponsor = contracts_info.owner_of_erc20_tokens_client_address;
    let description = erc20_asset_description(
        &erc20_code,
        &EthereumAddr(sponsor.to_fixed_bytes()),
        asset_policy.clone(),
    );
    let asset_code = AssetCode::new_foreign(&description);

    let asset_def = AssetDefinition::new(asset_code, asset_policy).unwrap();
    let asset_def_sol = asset_def.clone().generic_into::<sol::AssetDefinition>();

    println!("Sponsoring asset.");
    contracts_info
        .cape_contract_for_erc20_owner
        .sponsor_cape_asset(contracts_info.erc20_token_address, asset_def_sol)
        .send()
        .await?
        .await?
        .ensure_mined();

    let deposited_amount = 1000u64;

    let cape_contract_address = contracts_info.cape_contract.address();

    println!("Approving CAPE contract.");
    let amount_u256 = U256::from(deposited_amount);
    contracts_info
        .erc20_token_contract
        .approve(cape_contract_address, amount_u256)
        .send()
        .await?
        .await?
        .ensure_mined();

    let wrapped_ro = RecordOpening::new(
        rng,
        deposited_amount.into(),
        asset_def,
        faucet_key_pair.pub_key(),
        FreezeFlag::Unfrozen,
    );

    let wrapped_record_comm = RecordCommitment::from(&wrapped_ro).to_field_element();
    mt.push(wrapped_record_comm);

    println!("Depositing tokens into CAPE contract.");
    contracts_info
        .cape_contract_for_erc20_owner
        .deposit_erc_20(
            wrapped_ro.clone().generic_into::<sol::RecordOpening>(),
            contracts_info.erc20_token_address,
        )
        .send()
        .await?
        .await?
        .ensure_mined();

    // Submit empty block to trigger the inclusion of the pending deposit record commitment into the merkle tree
    let miner = UserPubKey::default();
    let empty_block = CapeBlock::generate(vec![], vec![], miner.address())?;

    println!("Submitting empty block to credit deposit.");
    cape_contract
        .submit_cape_block(empty_block.clone().into())
        .gas(5_000_000u64) // If the gas estimate is made against an outdated state it will be too low.
        .send()
        .await?
        .await?
        .ensure_mined()
        .print_gas("Credit deposit");

    println!("Verifying tokens have been deposited into CAPE contract.");
    for attempt in 0..3 {
        let balance = contracts_info
            .erc20_token_contract
            .balance_of(cape_contract_address)
            .call()
            .await?;
        if balance == U256::from(deposited_amount) {
            break;
        };
        if attempt == 2 {
            panic!("Tokens were not deposited.")
        };
        println!("Tokens were not deposited yet, retrying ...");
        std::thread::sleep(std::time::Duration::from_secs(1))
    }

    // Create burn transaction and record opening based on the content of the records merkle tree
    let unwrapped_assets_recipient_eth_address = final_recipient_of_unwrapped_assets.address();

    const POS_FEE_COMM: u64 = 0;
    const POS_WRAPPED_ASSET_COMM: u64 = 1;

    let cape_burn_tx = generate_burn_tx(
        &faucet_key_pair,
        faucet_record_opening,
        wrapped_ro,
        &mt,
        POS_FEE_COMM,
        POS_WRAPPED_ASSET_COMM,
        unwrapped_assets_recipient_eth_address,
    );

    println!("Submitting block with burn transaction.");
    let burn_transaction_note =
        TransactionNote::Transfer(Box::new(cape_burn_tx.clone().transfer_note));
    let cape_block = CapeBlock::generate(
        vec![burn_transaction_note],
        vec![cape_burn_tx.clone().burned_ro],
        miner.address(),
    )
    .unwrap();

    cape_contract
        .submit_cape_block(cape_block.clone().into())
        .gas(10_000_000) // out of gas with estimate
        .send()
        .await?
        .await?
        .ensure_mined()
        .print_gas("Burn transaction");

    println!("Verifying tokens exited CAPE contract.");
    for attempt in 0..3 {
        let balance = contracts_info
            .erc20_token_contract
            .balance_of(cape_contract_address)
            .call()
            .await?;
        if balance == U256::from(0u64) {
            break;
        };
        if attempt == 2 {
            panic!("Tokens did not exit CAPE contract")
        };
        println!("Tokens did not exit CAPE contract yet, retrying ...");
        std::thread::sleep(std::time::Duration::from_secs(1))
    }

    // Check that the records merkle tree is updated correctly, in particular that the burned record commitment is NOT inserted
    let burned_tx_fee_rc =
        cape_block.burn_notes[0].transfer_note.output_commitments[0].to_field_element();
    mt.push(burned_tx_fee_rc);

    compare_roots_records_test_cape_contract(&mt, &cape_contract, true).await;

    println!("Smoke test completed.");

    Ok(())
}
