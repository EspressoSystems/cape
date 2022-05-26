// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

//! # The CAPE Faucet
//!

extern crate cape_wallet;

use async_std::{
    sync::{Arc, Mutex},
    task::{spawn, JoinHandle},
};
use cap_rust_sandbox::universal_param::UNIVERSAL_PARAM;
use cape_wallet::{
    backend::{CapeBackend, CapeBackendConfig},
    wallet::{CapeWallet, CapeWalletError},
};
use futures::{channel::mpsc, SinkExt, StreamExt};
use itertools::Itertools;
use jf_cap::{
    keys::{UserKeyPair, UserPubKey},
    structs::{AssetCode, FreezeFlag},
};
use rand::distributions::{Alphanumeric, DistString};
use reef::traits::Validator;
use seahorse::{
    events::EventIndex,
    loader::{Loader, LoaderMetadata},
    txn_builder::{RecordInfo, TransactionStatus},
    RecordAmount,
};
use serde::{Deserialize, Serialize};
use snafu::Snafu;
use std::path::PathBuf;
use std::time::Duration;
use structopt::StructOpt;
use surf::Url;
use tide::{
    http::headers::HeaderValue,
    security::{CorsMiddleware, Origin},
    StatusCode,
};

#[derive(Debug, StructOpt)]
#[structopt(
    name = "CAPE Faucet Server",
    about = "Grants a native asset seed to a provided UserPubKey"
)]
pub struct FaucetOptions {
    /// mnemonic for the faucet wallet
    #[structopt(long, env = "CAPE_FAUCET_WALLET_MNEMONIC")]
    pub mnemonic: String,

    /// path to the faucet wallet
    #[structopt(long = "wallet-path", env = "CAPE_FAUCET_WALLET_PATH")]
    pub faucet_wallet_path: PathBuf,

    /// password on the faucet account keyfile
    #[structopt(
        long = "wallet-password",
        env = "CAPE_FAUCET_WALLET_PASSWORD",
        default_value = ""
    )]
    pub faucet_password: String,

    /// binding port for the faucet service
    #[structopt(long, env = "CAPE_FAUCET_PORT", default_value = "50079")]
    pub faucet_port: String,

    /// size of transfer for faucet grant
    #[structopt(long, env = "CAPE_FAUCET_GRANT_SIZE", default_value = "1000")]
    pub grant_size: RecordAmount,

    /// number of grants to give out per request
    #[structopt(long, env = "CAPE_FAUCET_NUM_GRANTS", default_value = "5")]
    pub num_grants: usize,

    /// fee for faucet grant
    #[structopt(long, env = "CAPE_FAUCET_FEE_SIZE", default_value = "0")]
    pub fee_size: RecordAmount,

    /// number of records to maintain simultaneously.
    ///
    /// This allows N CAPE transfers to take place simultaneously. A reasonable value is the number
    /// of simultaneous faucet requests you want to allow times CAPE_FAUCET_NUM_GRANTS. There is a
    /// tradeoff in startup cost for having more simultaneous records: when the faucet initializes,
    /// it must execute transfers to itself to break up its records into more, smaller ones. This
    /// can take a long time, and it also forces the relayer to pay a lot of gas.
    #[structopt(
        long,
        name = "N",
        env = "CAPE_FAUCET_NUM_RECORDS",
        default_value = "25"
    )]
    pub num_records: usize,

    /// URL for the Ethereum Query Service.
    #[structopt(long, env = "CAPE_EQS_URL", default_value = "http://localhost:50087")]
    pub eqs_url: Url,

    /// URL for the CAPE relayer.
    #[structopt(
        long,
        env = "CAPE_RELAYER_URL",
        default_value = "http://localhost:50077"
    )]
    pub relayer_url: Url,

    /// URL for the Ethereum Query Service.
    #[structopt(
        long,
        env = "CAPE_ADDRESS_BOOK_URL",
        default_value = "http://localhost:50078"
    )]
    pub address_book_url: Url,

    /// Minimum amount of time to wait between polling requests to EQS.
    #[structopt(long, env = "CAPE_WALLET_MIN_POLLING_DELAY", default_value = "500")]
    pub min_polling_delay_ms: u64,
}

#[derive(Clone)]
struct FaucetState {
    wallet: Arc<Mutex<CapeWallet<'static, CapeBackend<'static, LoaderMetadata>>>>,
    grant_size: RecordAmount,
    num_grants: usize,
    fee_size: RecordAmount,
    num_records: usize,
    // Channel to signal when the distribution of records owned by the faucet changes. This will
    // wake the record breaker thread (which waits on the receiver) so it can create more records by
    // breaking up larger ones to maintain the target of `num_records`.
    //
    // We use a bounded channel so that a crashed or deadlocked record breaker thread that is not
    // pulling messages out of the queue does not result in an unbounded memory leak.
    signal_breaker_thread: mpsc::Sender<()>,
}

impl FaucetState {
    pub fn new(
        wallet: CapeWallet<'static, CapeBackend<'static, LoaderMetadata>>,
        signal_breaker_thread: mpsc::Sender<()>,
        opt: &FaucetOptions,
    ) -> Self {
        Self {
            wallet: Arc::new(Mutex::new(wallet)),
            grant_size: opt.grant_size,
            num_grants: opt.num_grants,
            fee_size: opt.fee_size,
            num_records: opt.num_records,
            signal_breaker_thread,
        }
    }
}

#[derive(Debug, Snafu, Serialize, Deserialize)]
#[snafu(module(error))]
pub enum FaucetError {
    #[snafu(display("error in faucet transfer: {}", msg))]
    Transfer { msg: String },

    #[snafu(display("internal server error: {}", msg))]
    Internal { msg: String },
}

impl net::Error for FaucetError {
    fn catch_all(msg: String) -> Self {
        Self::Internal { msg }
    }
    fn status(&self) -> StatusCode {
        match self {
            Self::Transfer { .. } => StatusCode::BadRequest,
            Self::Internal { .. } => StatusCode::InternalServerError,
        }
    }
}

pub fn faucet_server_error<E: Into<FaucetError>>(err: E) -> tide::Error {
    net::server_error(err)
}

pub fn faucet_error(source: CapeWalletError) -> tide::Error {
    faucet_server_error(FaucetError::Transfer {
        msg: source.to_string(),
    })
}

/// Return a JSON expression with status 200 indicating the server
/// is up and running. The JSON expression is simply,
///    {"status": "available"}
/// When the server is running but unable to process requests
/// normally, a response with status 503 and payload {"status":
/// "unavailable"} should be added.
async fn healthcheck(_req: tide::Request<FaucetState>) -> Result<tide::Response, tide::Error> {
    Ok(tide::Response::builder(200)
        .content_type(tide::http::mime::JSON)
        .body(tide::prelude::json!({"status": "available"}))
        .build())
}

async fn request_fee_assets(
    mut req: tide::Request<FaucetState>,
) -> Result<tide::Response, tide::Error> {
    let pub_key: UserPubKey = net::server::request_body(&mut req).await?;
    let mut wallet = req.state().wallet.lock().await;
    let faucet_addr = wallet.pub_keys().await[0].address();

    let mut txns = Vec::new();
    let mut errs = Vec::new();
    for _ in 0..req.state().num_grants {
        tracing::info!(
            "transferring {} tokens from {} to {}",
            req.state().grant_size,
            net::UserAddress(faucet_addr.clone()),
            net::UserAddress(pub_key.address())
        );
        let balance = wallet.balance(&AssetCode::native()).await;
        let records = spendable_records(&wallet, req.state().grant_size)
            .await
            .count();
        tracing::info!(
            "Wallet balance before transfer: {} across {} records",
            balance,
            records
        );
        match wallet
            .transfer(
                Some(&faucet_addr),
                &AssetCode::native(),
                &[(pub_key.address(), req.state().grant_size)],
                req.state().fee_size,
            )
            .await
        {
            Ok(receipt) => {
                txns.push(receipt);
            }
            Err(err) => {
                tracing::warn!("Failed to transfer: {}", err);
                errs.push(err);
            }
        }
    }

    // Signal the record breaking thread that we have spent some records, so that it can create more
    // by breaking up larger records. Drop our handle to the wallet (which we no longer need) so
    // that the thread can access it.
    drop(wallet);
    if req
        .state()
        .signal_breaker_thread
        .clone()
        .try_send(())
        .is_err()
    {
        tracing::error!("Error signalling the breaker thread. Perhaps it has crashed?");
    }

    if !txns.is_empty() {
        // If we successfully transferred any assets, return success.
        net::server::response(&req, txns)
    } else {
        // Otherwise, explain why the transactions failed.
        let mut msgs = errs.into_iter().map(|err| format!("  - {}", err));
        Err(faucet_server_error(FaucetError::Transfer {
            msg: format!(
                "All transfers failed for the following reasons:\n{}",
                msgs.join("\n")
            ),
        }))
    }
}

async fn spendable_records(
    wallet: &CapeWallet<'static, CapeBackend<'static, LoaderMetadata>>,
    grant_size: RecordAmount,
) -> impl Iterator<Item = RecordInfo> {
    let now = wallet.lock().await.state().txn_state.validator.now();
    wallet.records().await.filter(move |record| {
        record.ro.asset_def.code == AssetCode::native()
            && record.amount() >= grant_size
            && record.ro.freeze_flag == FreezeFlag::Unfrozen
            && !record.on_hold(now)
    })
}

/// Break large records into smaller records.
///
/// When signalled on `wakeup`, this thread will break large records into small records of size
/// `state.grant_size`, until there are at least `state.num_records` distinct records in the wallet.
async fn break_up_records(state: FaucetState, mut wakeup: mpsc::Receiver<()>) {
    loop {
        // Wait until we have few enough records that we need to break them up, and we have a big
        // enough record to break up.
        //
        // This is a simulation of a condvar loop, since async condvar is unstable, hence the manual
        // drop and reacquisition of the wallet mutex guard.
        loop {
            let wallet = state.wallet.lock().await;
            let records = spendable_records(&*wallet, state.grant_size)
                .await
                .collect::<Vec<_>>();
            if records.len() >= state.num_records {
                // We have enough records for now, wait for a signal that the number of records has
                // changed.
                tracing::info!(
                    "got {}/{} records, waiting for a change",
                    records.len(),
                    state.num_records
                );
            } else if !records
                .into_iter()
                .any(|record| record.amount() > state.grant_size * 2u64)
            {
                // There are no big records to break up, so there's nothing for us to do. Exit
                // the inner loop and wait for a notification that the record distribution has
                // changed.
                tracing::warn!("not enough records, but no large records to break up");
                break;
            } else {
                break;
            }

            drop(wallet);
            wakeup.next().await;
        }

        // Start breaking up records until we have enough again.
        loop {
            // Acquire the wallet lock inside the loop, so we release it after each transfer.
            // Holding the lock for too long can unneccessarily slow down faucet requests.
            let mut wallet = state.wallet.lock().await;
            let address = wallet.pub_keys().await[0].address();
            let records = spendable_records(&*wallet, state.grant_size)
                .await
                .collect::<Vec<_>>();
            if records.len() >= state.num_records {
                // We have enough records again.
                break;
            }

            // Find a record which can be broken down into two smaller `grant_size` records.
            let record = match records
                .into_iter()
                .find(|record| record.amount() > state.grant_size * 2u64)
            {
                Some(record) => record,
                None => {
                    // There are no big records to break up, so there's nothing for us to do. Exit
                    // the inner loop and wait for a notification that the record distribution has
                    // changed.
                    break;
                }
            };

            tracing::info!(
                "breaking up a record of size {} into records of size {} and {}",
                record.amount(),
                state.grant_size,
                record.amount() - state.grant_size
            );

            // There is not yet an interface for transferring a specific record, so we just have to
            // specify the appropriate amounts and trust that Seahorse will use the largest record
            // available (it should). Just to be extra safe, we specify the larger of the two
            // amounts -- record.amount() - state.grant_size -- as the output amount, which should
            // create a change record of size `state.grant_size`. This makes it impossible for
            // Seahorse to choose a record of exactly `state.grant_size` with no change, which would
            // prevent this loop from making progress.
            let receipt = match wallet
                .transfer(
                    None,
                    &AssetCode::native(),
                    &[(address.clone(), record.amount() - state.grant_size)],
                    0u64,
                )
                .await
            {
                Ok(receipt) => receipt,
                Err(err) => {
                    // If our transfers start failing, we will assume there is something wrong and
                    // try not to put extra stress on the system. Break out of the inner loop and
                    // wait for a notification that something has changed.
                    tracing::error!("record breakup transfer failed: {}", err);
                    break;
                }
            };

            // Wait for the transaction to complete so we get the change record before continuing.
            match wallet.await_transaction(&receipt).await {
                Ok(TransactionStatus::Retired) => continue,
                _ => {
                    tracing::error!("record breakup transfer did not complete successfully");
                    break;
                }
            }
        }
    }
}

/// Wait until the record breaker thread has created as many records as it can.
///
/// Repeatedly signals the thread until either `state.num_records` have been created, or there are
/// no more big records to create. Returns the total number of records now in the wallet.
async fn wait_for_records(state: &FaucetState) -> usize {
    loop {
        let wallet = state.wallet.lock().await;

        // Break if we already have enough records, or if there are no big records to break up.
        let num_records = spendable_records(&*wallet, state.grant_size).await.count();
        if num_records >= state.num_records
            || !spendable_records(&*wallet, state.grant_size)
                .await
                .any(|record| record.amount() > state.grant_size * 2u64)
        {
            return num_records;
        }

        tracing::info!(
            "have {} records, waiting for {} more",
            num_records,
            state.num_records - num_records
        );

        drop(wallet);
        if state.signal_breaker_thread.clone().send(()).await.is_err() {
            tracing::error!("Error signalling the breaker thread. Perhaps it has crashed?");
            return num_records;
        }
    }
}

/// `faucet_key_pair` - If provided, will be added to the faucet wallet.
pub async fn init_web_server(
    opt: &FaucetOptions,
    faucet_key_pair: Option<UserKeyPair>,
) -> std::io::Result<JoinHandle<std::io::Result<()>>> {
    let mut password = opt.faucet_password.clone();
    if password.is_empty() {
        password = Alphanumeric.sample_string(&mut rand::thread_rng(), 16);
    }
    let mut loader = Loader::recovery(
        opt.mnemonic.clone().replace('-', " "),
        password,
        opt.faucet_wallet_path.clone(),
    );
    let backend = CapeBackend::new(
        &*UNIVERSAL_PARAM,
        CapeBackendConfig {
            // We're not going to do any direct-to-contract operations that would require a
            // connection to the CAPE contract or an ETH wallet. Everything we do will go through
            // the relayer.
            cape_contract: None,
            eth_mnemonic: None,
            eqs_url: opt.eqs_url.clone(),
            relayer_url: opt.relayer_url.clone(),
            address_book_url: opt.address_book_url.clone(),
            min_polling_delay: Duration::from_millis(opt.min_polling_delay_ms),
        },
        &mut loader,
    )
    .await
    .unwrap();
    let mut wallet = CapeWallet::new(backend).await.unwrap();

    // If a faucet key pair is provided, add it to the wallet. Otherwise, if we're initializing
    // for the first time, we need to generate a key. The faucet should be set up so that the
    // first HD sending key is the faucet key.
    let new_key = if let Some(key) = faucet_key_pair {
        wallet
            .add_user_key(key.clone(), "faucet".into(), EventIndex::default())
            .await
            .unwrap();
        Some(key.pub_key())
    } else if wallet.pub_keys().await.is_empty() {
        // We pass `EventIndex::default()` to start a scan of the ledger from the beginning, in
        // order to discover the faucet record.
        Some(
            wallet
                .generate_user_key("faucet".into(), Some(EventIndex::default()))
                .await
                .unwrap(),
        )
    } else {
        None
    };
    if let Some(key) = new_key {
        // Wait until we have scanned the ledger for records belonging to this key.
        wallet.await_key_scan(&key.address()).await.unwrap();
    }

    let bal = wallet.balance(&AssetCode::native()).await;
    tracing::info!("Wallet balance before init: {}", bal);
    // We use the total number of records to maintain as a conservative upper bound on how backed up
    // the message channel can get.
    let signal_breaker_thread = mpsc::channel(opt.num_records);
    let state = FaucetState::new(wallet, signal_breaker_thread.0, opt);

    // Spawn a thread to break records into smaller records to maintain `opt.num_records` at a time.
    spawn(break_up_records(state.clone(), signal_breaker_thread.1));

    // Wait for the thread to create at least `opt.num_records` if possible, before starting to
    // handle requests.
    wait_for_records(&state).await;

    let mut app = tide::with_state(state);
    app.with(
        CorsMiddleware::new()
            .allow_methods("GET, POST".parse::<HeaderValue>().unwrap())
            .allow_headers("*".parse::<HeaderValue>().unwrap())
            .allow_origin(Origin::from("*")),
    );
    app.at("/healthcheck").get(healthcheck);
    app.at("/request_fee_assets").post(request_fee_assets);
    let address = format!("0.0.0.0:{}", opt.faucet_port);
    Ok(spawn(app.listen(address)))
}

#[async_std::main]
async fn main() -> Result<(), std::io::Error> {
    tracing_subscriber::fmt()
        .compact()
        .with_ansi(false)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Initialize the faucet web server.
    init_web_server(&FaucetOptions::from_args(), None)
        .await?
        .await?;

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use cap_rust_sandbox::{ledger::CapeLedger, universal_param::UNIVERSAL_PARAM};
    use cape_wallet::testing::{create_test_network, retry, rpc_url_for_test, spawn_eqs};
    use ethers::prelude::U256;
    use jf_cap::structs::AssetDefinition;
    use rand_chacha::{rand_core::SeedableRng, ChaChaRng};
    use seahorse::{hd::KeyTree, txn_builder::TransactionReceipt};
    use std::path::PathBuf;
    use tempdir::TempDir;
    use tracing_test::traced_test;

    #[async_std::test]
    #[traced_test]
    async fn test_faucet_transfer() {
        let mut rng = ChaChaRng::from_seed([1u8; 32]);
        let universal_param = &*UNIVERSAL_PARAM;

        // Create test network with a faucet key pair.
        let (key_stream, mnemonic) = KeyTree::random(&mut rng);
        let faucet_key_pair = key_stream
            .derive_sub_tree("wallet".as_bytes())
            .derive_sub_tree("user".as_bytes())
            .derive_user_key_pair(&0u64.to_le_bytes());
        let (_, relayer_url, address_book_url, contract_address, _) =
            create_test_network(&mut rng, universal_param, Some(faucet_key_pair.clone())).await;
        let (eqs_url, _eqs_dir, _join_eqs) = spawn_eqs(contract_address).await;

        // Initiate a faucet server with the mnemonic associated with the faucet key pair.
        let faucet_dir = TempDir::new("cape_wallet_faucet").unwrap();
        let faucet_port = "50079".to_string();
        let grant_size = RecordAmount::from(1000u64);
        let num_grants = 5;
        let opt = FaucetOptions {
            mnemonic: mnemonic.to_string(),
            faucet_wallet_path: PathBuf::from(faucet_dir.path()),
            faucet_password: "".to_string(),
            faucet_port: faucet_port.clone(),
            grant_size,
            num_grants,
            num_records: num_grants,
            fee_size: 0u64.into(),
            eqs_url: eqs_url.clone(),
            relayer_url: relayer_url.clone(),
            address_book_url: address_book_url.clone(),
            min_polling_delay_ms: 500,
        };
        init_web_server(&opt, Some(faucet_key_pair)).await.unwrap();
        println!("Faucet server initiated.");

        // Create a receiver wallet.
        let receiver_dir = TempDir::new("cape_wallet_receiver").unwrap();
        let mut receiver_loader = Loader::from_literal(
            Some(KeyTree::random(&mut rng).1.to_string().replace('-', " ")),
            Alphanumeric.sample_string(&mut rand::thread_rng(), 16),
            PathBuf::from(receiver_dir.path()),
        );
        let receiver_backend = CapeBackend::new(
            universal_param,
            CapeBackendConfig {
                cape_contract: Some((rpc_url_for_test(), contract_address)),
                eqs_url,
                relayer_url,
                address_book_url,
                eth_mnemonic: None,
                min_polling_delay: Duration::from_millis(500),
            },
            &mut receiver_loader,
        )
        .await
        .unwrap();
        let mut receiver = CapeWallet::new(receiver_backend).await.unwrap();
        let receiver_key = receiver
            .generate_user_key("receiver".into(), None)
            .await
            .unwrap();
        let receiver_key_bytes = bincode::serialize(&receiver_key).unwrap();
        println!("Receiver wallet created.");

        // Request native asset for the receiver.
        let mut response = surf::post(format!(
            "http://localhost:{}/request_fee_assets",
            faucet_port
        ))
        .content_type(surf::http::mime::BYTE_STREAM)
        .body_bytes(&receiver_key_bytes)
        .await
        .unwrap();
        println!("Asset transferred.");

        let receipts: Vec<TransactionReceipt<CapeLedger>> = response.body_json().await.unwrap();
        assert_eq!(receipts.len(), 5);

        // Check the balance.
        retry(|| async {
            receiver.balance(&AssetCode::native()).await == U256::from(grant_size) * num_grants
        })
        .await;

        // We should have received `num_grants` records of `grant_size` each.
        let records = receiver.records().await.collect::<Vec<_>>();
        assert_eq!(records.len(), 5);
        for record in records {
            assert_eq!(record.ro.asset_def, AssetDefinition::native());
            assert_eq!(record.ro.pub_key, receiver_key);
            assert_eq!(record.amount(), grant_size);
        }
    }
}
