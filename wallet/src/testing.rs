// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Utilities for writing wallet tests

use crate::backend::CapeBackend;
use crate::mocks::*;
use crate::wallet::CapeWalletExt;
use crate::CapeWallet;
use crate::CapeWalletError;
use address_book::address_book_port;
use address_book::init_web_server;
use address_book::wait_for_server;
use address_book::TransientFileStore;
use async_std::sync::{Arc, Mutex};
use async_std::task::{sleep, spawn, JoinHandle};
use cap_rust_sandbox::deploy::EthMiddleware;
use cap_rust_sandbox::ethereum::get_provider;
use cap_rust_sandbox::ledger::CapeLedger;
use cap_rust_sandbox::test_utils::keysets_for_test;
use cap_rust_sandbox::types::SimpleToken;
use eqs::configuration::Confirmations;
use eqs::{configuration::EQSOptions, run_eqs};
use ethers::prelude::Address;
use ethers::providers::Middleware;
use ethers::types::TransactionRequest;
use ethers::types::U256;
use futures::Future;
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
use lazy_static::lazy_static;
use rand::{
    distributions::{Distribution, Standard},
    Rng,
};
use rand_chacha::ChaChaRng;
use relayer::testing::start_minimal_relayer_for_test;
use seahorse::txn_builder::RecordInfo;
use seahorse::txn_builder::TransactionReceipt;
use seahorse::RecordAmount;
use std::collections::HashSet;
use std::time::Duration;
use surf::Url;
use tempdir::TempDir;
use tracing::{event, Level};

lazy_static! {
    static ref PORT: Arc<Mutex<u16>> = {
        let port_offset = std::env::var("PORT").unwrap_or_else(|_| String::from("60000"));
        Arc::new(Mutex::new(port_offset.parse().unwrap()))
    };
}

pub async fn port() -> u16 {
    let mut counter = PORT.lock().await;
    let port = *counter;
    *counter += 1;
    port
}

pub async fn retry_delay() {
    sleep(Duration::from_secs(1)).await
}

pub async fn retry<Fut: Future<Output = bool>>(f: impl Fn() -> Fut) {
    let mut backoff = Duration::from_millis(100);
    for _ in 0..12 {
        if f().await {
            return;
        }
        sleep(backoff).await;
        backoff *= 2;
    }
    panic!("retry loop did not complete in {:?}", backoff);
}

/// `faucet_key_pair` - If not provided, a random faucet key pair will be generated.
#[allow(clippy::needless_lifetimes)]
pub async fn create_test_network<'a>(
    rng: &mut ChaChaRng,
    universal_param: &'a UniversalParam,
    faucet_key_pair: Option<UserKeyPair>,
) -> (
    UserKeyPair,
    Url,
    Url,
    Address,
    Arc<Mutex<MockCapeLedger<'a>>>,
) {
    init_web_server(TransientFileStore::default())
        .await
        .expect("Failed to run server.");
    wait_for_server().await;
    let address_book_url =
        Url::parse(&format!("http://localhost:{}", address_book_port())).unwrap();

    // Set up a network that includes a minimal relayer, connected to a real Ethereum
    // blockchain, as well as a mock EQS which will track the blockchain in parallel.
    let relayer_port = port().await;
    let (contract, sender_key, sender_rec, records) =
        start_minimal_relayer_for_test(relayer_port, faucet_key_pair).await;
    let relayer_url = Url::parse(&format!("http://localhost:{}", relayer_port)).unwrap();
    let sender_memo = ReceiverMemo::from_ro(rng, &sender_rec, &[]).unwrap();

    let (_, verif_crs) = keysets_for_test(universal_param);

    let mut mock_eqs = MockCapeLedger::new(
        MockCapeNetwork::new(verif_crs, records.clone(), vec![(sender_memo, 0)]),
        records,
    );
    mock_eqs.set_block_size(1).unwrap();
    // The minimal test relayer does not block transactions, so the mock EQS shouldn't
    // either.
    let mock_eqs = Arc::new(Mutex::new(mock_eqs));

    (
        sender_key,
        relayer_url,
        address_book_url,
        contract.address(),
        mock_eqs,
    )
}

pub async fn fund_eth_wallet<'a>(wallet: &mut CapeWallet<'a, CapeBackend<'a>>) {
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
    wallet: &CapeWallet<'a, CapeBackend<'a>>,
    asset: AssetCode,
) -> RecordAmount {
    // get records for this this asset type
    let records = wallet.records().await;
    let filtered = records
        .filter(|rec| rec.ro.asset_def.code == asset)
        .collect::<Vec<_>>();
    if filtered.is_empty() {
        event!(Level::INFO, "No records to burn");
        0u64.into()
    } else {
        filtered[0].amount()
    }
}

pub fn rpc_url_for_test() -> Url {
    match std::env::var("CAPE_WEB3_PROVIDER_URL") {
        Ok(val) => val.parse().unwrap(),
        Err(_) => "http://localhost:8545".parse().unwrap(),
    }
}

pub async fn spawn_eqs(cape_address: Address) -> (Url, TempDir, JoinHandle<std::io::Result<()>>) {
    let dir = TempDir::new("wallet_testing_eqs").unwrap();
    let eqs_port = port().await;
    let opt = EQSOptions {
        web_path: String::new(),
        api_path: [
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")),
            std::path::Path::new("../eqs/api/api.toml"),
        ]
        .iter()
        .collect::<std::path::PathBuf>()
        .as_os_str()
        .to_str()
        .unwrap()
        .to_string(),
        store_path: dir.path().as_os_str().to_str().unwrap().to_owned(),
        reset_store_state: true,
        query_interval: 500,
        ethers_block_max: 5000,
        eqs_port,
        cape_address: Some(cape_address),
        rpc_url: rpc_url_for_test().to_string(),
        temp_test_run: false,
        num_confirmations: Confirmations::default(),
    };
    let join = spawn(async move { run_eqs(&opt).await });
    let url = Url::parse(&format!("http://localhost:{}", eqs_port)).unwrap();
    retry(|| async { surf::connect(url.clone()).send().await.is_ok() }).await;

    (url, dir, join)
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
    wallet: &mut CapeWallet<'a, CapeBackend<'a>>,
) -> Result<(AssetDefinition, Option<TransactionReceipt<CapeLedger>>), CapeWalletError> {
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
    let my_asset = wallet
        .define_asset("test_domestic_asset".into(), &[], policy)
        .await?;
    event!(Level::INFO, "defined a new asset type: {}", my_asset.code);
    let address = wallet.pub_keys().await[0].address();

    // Mint some custom asset
    event!(Level::INFO, "minting my asset type {}", my_asset.code);
    let txn = wallet
        .mint(
            Some(&address),
            1,
            &my_asset.code,
            1u128 << 32,
            address.clone(),
        )
        .await
        .ok();
    Ok((my_asset, txn))
}

/// Return records the freezer has access to freeze or unfreeze but does not own.
/// Will only return records with freeze_flag the same as the frozen arg.
pub async fn find_freezable_records<'a>(
    freezer: &CapeWallet<'a, CapeBackend<'a>>,
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
    freezer: &mut CapeWallet<'a, CapeBackend<'a>>,
    asset: &AssetCode,
    amount: impl Into<U256>,
    owner_address: UserAddress,
) -> Result<TransactionReceipt<CapeLedger>, CapeWalletError> {
    let freeze_address = freezer.pub_keys().await[0].address();
    freezer
        .freeze(Some(&freeze_address), 1, asset, amount, owner_address)
        .await
}

pub async fn unfreeze_token<'a>(
    freezer: &mut CapeWallet<'a, CapeBackend<'a>>,
    asset: &AssetCode,
    amount: impl Into<U256>,
    owner_address: UserAddress,
) -> Result<TransactionReceipt<CapeLedger>, CapeWalletError> {
    let unfreeze_address = freezer.pub_keys().await[0].address();
    freezer
        .unfreeze(Some(&unfreeze_address), 1, asset, amount, owner_address)
        .await
}

pub async fn wrap_simple_token<'a>(
    wrapper: &mut CapeWallet<'a, CapeBackend<'a>>,
    wrapper_addr: &UserAddress,
    cape_asset: AssetDefinition,
    erc20_contract: &SimpleToken<EthMiddleware>,
    amount: impl Into<RecordAmount>,
) -> Result<(), CapeWalletError> {
    let amount = amount.into();
    let wrapper_eth_addr = wrapper.eth_address().await.unwrap();
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
    sponsor: &mut CapeWallet<'a, CapeBackend<'a>>,
    erc20_contract: &SimpleToken<EthMiddleware>,
) -> Result<AssetDefinition, CapeWalletError> {
    let sponsor_eth_addr = sponsor.eth_address().await.unwrap();
    sponsor
        .sponsor(
            "test_wrapped_asset".into(),
            erc20_contract.address().into(),
            sponsor_eth_addr.clone(),
            AssetPolicy::default(),
        )
        .await
}

pub async fn burn_token<'a>(
    burner: &mut CapeWallet<'a, CapeBackend<'a>>,
    cape_asset: AssetDefinition,
    amount: impl Into<RecordAmount> + Send + 'static,
) -> Result<TransactionReceipt<CapeLedger>, CapeWalletError> {
    let burner_key = burner.pub_keys().await[0].clone();
    burner
        .burn(
            Some(&burner_key.address()),
            burner.eth_address().await.unwrap().clone(),
            &cape_asset.code,
            amount,
            1,
        )
        .await
}

pub async fn transfer_token<'a>(
    sender: &mut CapeWallet<'a, CapeBackend<'a>>,
    receiver_address: UserAddress,
    amount: impl Into<RecordAmount>,
    asset_code: AssetCode,
    fee: impl Into<RecordAmount>,
) -> Result<TransactionReceipt<CapeLedger>, CapeWalletError> {
    let amount = amount.into();
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
