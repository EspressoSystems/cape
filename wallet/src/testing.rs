// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Utilities for writing wallet tests
// #![deny(warnings)]

use crate::backend::CapeBackend;
use crate::mocks::*;
use crate::wallet::CapeWalletExt;
use crate::CapeWallet;
use crate::CapeWalletError;
use address_book::init_web_server;
use address_book::wait_for_server;
use address_book::TransientFileStore;
use async_std::sync::{Arc, Mutex};
use async_std::task::sleep;
use cap_rust_sandbox::deploy::EthMiddleware;
use cap_rust_sandbox::ethereum::get_provider;
use cap_rust_sandbox::ledger::CapeLedger;
use cap_rust_sandbox::types::SimpleToken;
use ethers::prelude::Address;
use ethers::providers::Middleware;
use ethers::types::TransactionRequest;
use ethers::types::U256;
use jf_cap::keys::FreezerPubKey;
use jf_cap::keys::UserAddress;
use jf_cap::keys::UserKeyPair;
use jf_cap::keys::UserPubKey;
use jf_cap::proof::UniversalParam;
use jf_cap::structs::AssetCode;
use jf_cap::structs::AssetDefinition;
use jf_cap::structs::AssetPolicy;
use jf_cap::structs::FreezeFlag;
use jf_cap::structs::ReceiverMemo;
use jf_cap::TransactionVerifyingKey;
use key_set::VerifierKeySet;
use lazy_static::lazy_static;
use rand::{
    distributions::{Distribution, Standard},
    Rng,
};
use rand_chacha::ChaChaRng;
use reef::Ledger;
use relayer::testing::start_minimal_relayer_for_test;
use seahorse::testing::await_transaction;
use seahorse::txn_builder::RecordInfo;
use seahorse::txn_builder::{TransactionReceipt, TransactionStatus};
use std::collections::HashSet;
use std::time::Duration;
use surf::Url;
use tide::log::LevelFilter;
use tracing::{event, Level};

lazy_static! {
    static ref PORT: Arc<Mutex<u64>> = {
        let port_offset = std::env::var("PORT").unwrap_or_else(|_| String::from("60000"));
        Arc::new(Mutex::new(port_offset.parse().unwrap()))
    };
}

pub async fn port() -> u64 {
    let mut counter = PORT.lock().await;
    let port = *counter;
    *counter += 1;
    port
}

pub async fn retry_delay() {
    sleep(Duration::from_secs(1)).await
}

#[allow(clippy::needless_lifetimes)]
pub async fn create_test_network<'a>(
    rng: &mut ChaChaRng,
    universal_param: &'a UniversalParam,
) -> (UserKeyPair, Url, Address, Arc<Mutex<MockCapeLedger<'a>>>) {
    init_web_server(LevelFilter::Info, TransientFileStore::default())
        .await
        .expect("Failed to run server.");
    wait_for_server().await;

    // Set up a network that includes a minimal relayer, connected to a real Ethereum
    // blockchain, as well as a mock EQS which will track the blockchain in parallel, since we
    // don't yet have a real EQS.
    let relayer_port = port().await;
    let (contract, sender_key, sender_rec, records) =
        start_minimal_relayer_for_test(relayer_port).await;
    let relayer_url = Url::parse(&format!("http://localhost:{}", relayer_port)).unwrap();
    let sender_memo = ReceiverMemo::from_ro(rng, &sender_rec, &[]).unwrap();

    let verif_crs = VerifierKeySet {
        xfr: vec![
            // For regular transfers, including non-native transfers
            TransactionVerifyingKey::Transfer(
                jf_cap::proof::transfer::preprocess(
                    universal_param,
                    2,
                    3,
                    CapeLedger::merkle_height(),
                )
                .unwrap()
                .1,
            ),
            // For burns (which currently require exactly 2 inputs and outputs, but this is an
            // artificial restriction which should be lifted)
            TransactionVerifyingKey::Transfer(
                jf_cap::proof::transfer::preprocess(
                    universal_param,
                    2,
                    2,
                    CapeLedger::merkle_height(),
                )
                .unwrap()
                .1,
            ),
        ]
        .into_iter()
        .collect(),
        freeze: vec![TransactionVerifyingKey::Freeze(
            jf_cap::proof::freeze::preprocess(universal_param, 2, CapeLedger::merkle_height())
                .unwrap()
                .1,
        )]
        .into_iter()
        .collect(),
        mint: TransactionVerifyingKey::Mint(
            jf_cap::proof::mint::preprocess(universal_param, CapeLedger::merkle_height())
                .unwrap()
                .1,
        ),
    };
    let mut mock_eqs = MockCapeLedger::new(MockCapeNetwork::new(
        verif_crs,
        records,
        vec![(sender_memo, 0)],
    ));
    mock_eqs.set_block_size(1).unwrap();
    // The minimal test relayer does not block transactions, so the mock EQS shouldn't
    // either.
    let mock_eqs = Arc::new(Mutex::new(mock_eqs));

    (sender_key, relayer_url, contract.address(), mock_eqs)
}

pub async fn fund_eth_wallet<'a>(wallet: &mut CapeWallet<'a, CapeBackend<'a, ()>>) {
    // Fund the Ethereum wallets for contract calls.
    let provider = get_provider().interval(Duration::from_millis(100u64));
    let accounts = provider.get_accounts().await.unwrap();
    assert!(!accounts.is_empty());

    let tx = TransactionRequest::new()
        .to(Address::from(wallet.eth_address().await.unwrap()))
        .value(ethers::utils::parse_ether(U256::from(1000)).unwrap())
        .from(accounts[0]);
    provider
        .send_transaction(tx, None)
        .await
        .unwrap()
        .await
        .unwrap();
}

pub async fn get_burn_amount<'a>(
    wallet: &CapeWallet<'a, CapeBackend<'a, ()>>,
    asset: AssetCode,
) -> u64 {
    // get records for this this asset type
    let records = wallet.records().await;
    let filtered = records
        .filter(|rec| rec.ro.asset_def.code == asset)
        .collect::<Vec<_>>();
    if filtered.is_empty() {
        event!(Level::INFO, "No records to burn");
        0
    } else {
        filtered[0].ro.amount
    }
}

#[derive(Debug)]
pub enum OperationType {
    Transfer,
    Freeze,
    Unfreeze,
    Wrap,
    Burn,
    Mint,
}

impl Distribution<OperationType> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> OperationType {
        match rng.gen_range(0..=5) {
            0 => OperationType::Transfer,
            1 => OperationType::Freeze,
            2 => OperationType::Unfreeze,
            3 => OperationType::Wrap,
            4 => OperationType::Burn,
            _ => OperationType::Mint,
        }
    }
}

/// Mint a new token with the given wallet and add it's freezer and audit keys to
/// to the Asset Policy
pub async fn mint_token<'a>(
    wallet: &mut CapeWallet<'a, CapeBackend<'a, ()>>,
) -> Result<AssetDefinition, CapeWalletError> {
    let freeze_key = &wallet.freezer_pub_keys().await[0];
    let audit_key = &wallet.auditor_pub_keys().await[0];
    let policy = AssetPolicy::default()
        .set_freezer_pub_key(freeze_key.clone())
        .set_auditor_pub_key(audit_key.clone())
        .reveal_user_address()
        .unwrap()
        .reveal_amount()
        .unwrap()
        .reveal_blinding_factor()
        .unwrap();
    let my_asset = wallet.define_asset(&[], policy).await?;
    event!(Level::INFO, "defined a new asset type: {}", my_asset.code);
    let address = wallet.pub_keys().await[0].address();

    // Mint some custom asset
    event!(Level::INFO, "minting my asset type {}", my_asset.code);
    loop {
        let txn = wallet
            .mint(&address, 1, &my_asset.code, 1u64 << 32, address.clone())
            .await
            .expect("failed to generate mint transaction");
        let status = wallet
            .await_transaction(&txn)
            .await
            .expect("error waiting for mint to complete");
        if status.succeeded() {
            break;
        }
        // The mint transaction is allowed to fail due to contention from other clients.
        event!(Level::WARN, "mint transaction failed, retrying...");
        retry_delay().await;
    }
    Ok(my_asset)
}

/// Return records the freezer has access to freeze or unfreeze but does not own.
/// Will only return records with freeze_flag the same as the frozen arg.
pub async fn find_freezable_records<'a>(
    freezer: &CapeWallet<'a, CapeBackend<'a, ()>>,
    frozen: FreezeFlag,
) -> Vec<RecordInfo> {
    let pks: HashSet<UserPubKey> = freezer.pub_keys().await.into_iter().collect();
    let freeze_keys: HashSet<FreezerPubKey> =
        freezer.freezer_pub_keys().await.into_iter().collect();
    let records = freezer.records().await;
    records
        .filter(|r| {
            let ro = &r.ro;
            // Ignore records we own
            if pks.contains(&ro.pub_key) {
                return false;
            }
            // Check we can freeeze
            if !(freeze_keys.contains(ro.asset_def.policy_ref().freezer_pub_key())) {
                return false;
            }
            ro.freeze_flag == frozen
        })
        .collect()
}

pub async fn freeze_token<'a>(
    freezer: &mut CapeWallet<'a, CapeBackend<'a, ()>>,
    asset: &AssetCode,
    amount: u64,
    owner_address: UserAddress,
) -> Result<TransactionStatus, CapeWalletError> {
    let freeze_address = freezer.pub_keys().await[0].address();
    let txn = freezer
        .freeze(&freeze_address, 1, asset, amount, owner_address)
        .await?;
    freezer.await_transaction(&txn).await
}

pub async fn unfreeze_token<'a>(
    freezer: &mut CapeWallet<'a, CapeBackend<'a, ()>>,
    asset: &AssetCode,
    amount: u64,
    owner_address: UserAddress,
) -> Result<TransactionStatus, CapeWalletError> {
    let unfreeze_address = freezer.pub_keys().await[0].address();
    let txn = freezer
        .unfreeze(&unfreeze_address, 1, asset, amount, owner_address)
        .await
        .unwrap();
    freezer.await_transaction(&txn).await
}

pub async fn wrap_simple_token<'a>(
    wrapper: &mut CapeWallet<'a, CapeBackend<'a, ()>>,
    wrapper_addr: &UserAddress,
    cape_asset: AssetDefinition,
    erc20_contract: &SimpleToken<EthMiddleware>,
    amount: u64,
) -> Result<(), CapeWalletError> {
    let wrapper_eth_addr = wrapper.eth_address().await.unwrap();

    let total_native_balance = wrapper
        .balance_breakdown(wrapper_addr, &AssetCode::native())
        .await;
    assert!(total_native_balance > 0);

    // Prepare to wrap: deposit some ERC20 tokens into the wrapper's ETH wallet.
    erc20_contract
        .transfer(wrapper_eth_addr.clone().into(), amount.into())
        .send()
        .await
        .unwrap()
        .await
        .unwrap();

    // Deposit some ERC20 into the CAPE contract.
    wrapper
        .wrap(
            wrapper_eth_addr.clone(),
            cape_asset.clone(),
            wrapper_addr.clone(),
            amount,
        )
        .await
        .unwrap();
    Ok(())
}

pub async fn sponsor_simple_token<'a>(
    sponsor: &mut CapeWallet<'a, CapeBackend<'a, ()>>,
    erc20_contract: &SimpleToken<EthMiddleware>,
) -> Result<AssetDefinition, CapeWalletError> {
    let sponsor_eth_addr = sponsor.eth_address().await.unwrap();
    sponsor
        .sponsor(
            erc20_contract.address().into(),
            sponsor_eth_addr.clone(),
            AssetPolicy::default(),
        )
        .await
}

pub async fn burn_token<'a>(
    burner: &mut CapeWallet<'a, CapeBackend<'a, ()>>,
    cape_asset: AssetDefinition,
    amount: u64,
) -> Result<(), CapeWalletError> {
    let burner_key = burner.pub_keys().await[0].clone();
    let receipt = burner
        .burn(
            &burner_key.address(),
            burner.eth_address().await.unwrap().clone(),
            &cape_asset.code,
            amount,
            1,
        )
        .await
        .unwrap();
    await_transaction(&receipt, burner, &[]).await;
    Ok(())
}

pub async fn transfer_token<'a>(
    sender: &mut CapeWallet<'a, CapeBackend<'a, ()>>,
    receiver_address: UserAddress,
    amount: u64,
    asset_code: AssetCode,
    fee: u64,
) -> Result<TransactionReceipt<CapeLedger>, CapeWalletError> {
    event!(
        Level::INFO,
        "Sending {} to: {} from {}.  Asset: code {}",
        amount,
        sender.pub_keys().await[0].address(),
        receiver_address,
        asset_code
    );
    sender
        .transfer(None, &asset_code, &[(receiver_address, amount)], fee)
        .await
}
