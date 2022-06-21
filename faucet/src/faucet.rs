// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

//! # The CAPE Faucet
//!

extern crate cape_wallet;

use async_channel as mpmc;
use async_std::{
    sync::{Arc, Mutex, RwLock},
    task::{sleep, spawn, JoinHandle},
};
use atomic_store::{
    load_store::BincodeLoadStore, AppendLog, AtomicStore, AtomicStoreLoader, PersistenceError,
};
use cap_rust_sandbox::universal_param::UNIVERSAL_PARAM;
use cape_wallet::{
    backend::{CapeBackend, CapeBackendConfig},
    loader::CapeLoader,
    wallet::{CapeWallet, CapeWalletError},
};
use futures::{channel::mpsc, SinkExt, StreamExt};
use jf_cap::{
    keys::{UserKeyPair, UserPubKey},
    structs::{AssetCode, FreezeFlag},
};
use net::server::response;
use rand::distributions::{Alphanumeric, DistString};
use reef::traits::Validator;
use seahorse::{
    events::EventIndex,
    txn_builder::{RecordInfo, TransactionStatus},
    RecordAmount,
};
use serde::{Deserialize, Serialize};
use snafu::Snafu;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
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

    /// Maximum number of outstanding requests to allow in the queue.
    ///
    /// If not provided, the queue can grow arbitrarily large.
    #[structopt(long, env = "CAPE_FAUCET_MAX_QUEUE_LENGTH")]
    pub max_queue_len: Option<usize>,

    /// Number of worker threads.
    ///
    /// It is a good idea to configure the faucet so that this is the same as
    /// `num_records / num_grants`.
    #[structopt(long, env = "CAPE_FAUCET_NUM_WORKERS", default_value = "5")]
    pub num_workers: usize,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FaucetStatus {
    Initializing,
    Available,
}

#[derive(Clone)]
struct FaucetState {
    wallet: Arc<Mutex<CapeWallet<'static, CapeBackend<'static>>>>,
    status: Arc<RwLock<FaucetStatus>>,
    queue: FaucetQueue,
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
    pub async fn new(
        wallet: CapeWallet<'static, CapeBackend<'static>>,
        signal_breaker_thread: mpsc::Sender<()>,
        opt: &FaucetOptions,
    ) -> Result<Self, FaucetError> {
        Ok(Self {
            wallet: Arc::new(Mutex::new(wallet)),
            status: Arc::new(RwLock::new(FaucetStatus::Initializing)),
            queue: FaucetQueue::load(&opt.faucet_wallet_path, opt.max_queue_len).await?,
            grant_size: opt.grant_size,
            num_grants: opt.num_grants,
            fee_size: opt.fee_size,
            num_records: opt.num_records,
            signal_breaker_thread,
        })
    }
}

#[derive(Debug, Snafu, Serialize, Deserialize)]
#[snafu(module(error))]
pub enum FaucetError {
    #[snafu(display("error in faucet transfer: {}", msg))]
    Transfer { msg: String },

    #[snafu(display("internal server error: {}", msg))]
    Internal { msg: String },

    #[snafu(display("the queue is full with {} requests, try again later", max_len))]
    QueueFull { max_len: usize },

    #[snafu(display(
        "there is a pending request with key {}, you can only request once at a time",
        key
    ))]
    AlreadyInQueue { key: UserPubKey },

    #[snafu(display("error with persistent storage: {}", msg))]
    Persistence { msg: String },

    #[snafu(display("faucet service temporarily unavailable"))]
    Unavailable,
}

impl net::Error for FaucetError {
    fn catch_all(msg: String) -> Self {
        Self::Internal { msg }
    }
    fn status(&self) -> StatusCode {
        match self {
            Self::Transfer { .. } => StatusCode::BadRequest,
            Self::Internal { .. } => StatusCode::InternalServerError,
            Self::AlreadyInQueue { .. } => StatusCode::BadRequest,
            Self::QueueFull { .. } => StatusCode::InternalServerError,
            Self::Persistence { .. } => StatusCode::InternalServerError,
            Self::Unavailable => StatusCode::ServiceUnavailable,
        }
    }
}

impl From<PersistenceError> for FaucetError {
    fn from(source: PersistenceError) -> Self {
        Self::Persistence {
            msg: source.to_string(),
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

/// A shared, asynchronous queue of requests.
///
/// The queue consists of an in-memory message channel, with send and receive ends. This channel is
/// shared across instances of a given queue (created with [Clone]). A thread can add a request to
/// the queue by calling [FaucetQueue::push], which sends a message on the channel containing the
/// address requesting assets. A worker task can then dequeue the request by calling
/// [FaucetQueue::pop]. The queue supports multiple simultaneous receivers and is internally
/// thread-safe.
///
/// The queue also contains an index, which is an unordered set of all addresses currently in the
/// queue, as well as an optional maximum length. Pushing will fail if the address being pushed is
/// already in the queue, or if pushing the address would exceed the maximum length of the queue.
/// The index is not synchronized with respect to the message channel, but addresses are always
/// added to the index _before_ being pushed into the channel and removed from the index _after_
/// being removed. This means that an address is considered "in the queue" as long as it is in the
/// index, and it may be possible, for a brief time, for an address to appear in the index but not
/// be returned by [FaucetQueue::pop]. The index is managed internally and not exposed, and the
/// consistency of the queue accounts for this restriction.
///
/// The queue also supports persistence. If a request is successfully pushed onto the queue
/// ([FaucetQueue::push] returns `Ok(())`) and has not been popped before the process is shut down
/// or killed, the request will be saved in persistent storage and loaded back into the queue next
/// time the queue is loaded from the same file location. The format of the persistent storage is an
/// ordered set, represented as an `AppendLog` of `(UserPubKey, bool)` pairs. To recover the set,
/// simply replay the log, inserting whenever a key has the value `true` and removing that key when
/// it has the value `false`. In this way, we persist both the set of requests in the queue and the
/// order in which they were added.
///
/// In order to avoid dropping requests, we do not remove elements from the index immediately after
/// they are dequeued. If we did this, the server could crash after a request was dequeued but
/// before it was processed, and we would lose that request, since the index is the part of the
/// queue which is actually persisted. Instead, we keep the request in the index until its
/// processing is complete. If the request was handled successfully, we finally remove it from the
/// index. If the request failed but we want to retry it later, we keep it in the index and add it
/// back to the message channel, so a worker will pick it up again.
#[derive(Clone)]
struct FaucetQueue {
    sender: mpmc::Sender<UserPubKey>,
    receiver: mpmc::Receiver<UserPubKey>,
    index: Arc<Mutex<FaucetQueueIndex>>,
    max_len: Option<usize>,
}

// A persistent ordered set.
struct FaucetQueueIndex {
    index: HashSet<UserPubKey>,
    store: AtomicStore,
    queue: AppendLog<BincodeLoadStore<(UserPubKey, bool)>>,
}

impl FaucetQueueIndex {
    fn len(&self) -> usize {
        self.index.len()
    }

    /// Add an element to the persistent set.
    ///
    /// Returns `true` if the element was inserted or `false` if it was already in the set.
    fn insert(&mut self, key: UserPubKey) -> Result<bool, FaucetError> {
        if self.index.contains(&key) {
            // If the key is already in the set, we don't have to persist anything.
            return Ok(false);
        }

        // Add the key to our persistent log.
        self.queue.store_resource(&(key.clone(), true))?;
        self.queue.commit_version().unwrap();
        self.store.commit_version().unwrap();
        // If successful, add it to our in-memory set.
        self.index.insert(key);
        Ok(true)
    }

    /// Remove an element from the persistent set.
    fn remove(&mut self, key: &UserPubKey) -> Result<(), FaucetError> {
        // Make a persistent note to remove the key.
        self.queue.store_resource(&(key.clone(), false))?;
        self.queue.commit_version().unwrap();
        self.store.commit_version().unwrap();
        // Update our in-memory set.
        self.index.remove(key);
        Ok(())
    }
}

impl FaucetQueue {
    async fn load(store: &Path, max_len: Option<usize>) -> Result<Self, FaucetError> {
        // Load from storage.
        let mut loader = AtomicStoreLoader::load(store, "queue")?;
        let persistent_queue = AppendLog::load(&mut loader, Default::default(), "requests", 1024)?;
        let store = AtomicStore::open(loader)?;

        // Traverse the persisted queue entries backwards. This ensures that we encounter the most
        // recent value for each key first. If the most recent value for a given key is `true`, it
        // gets added to the index and message channel. Otherwise, we just store `false` in `index`
        // so that if we see this key again, we know we are not seeing the most recent value.
        let mut index = HashMap::new();
        // We are encountering requests in reverse order, so if we need to add them to the queue, we
        // will add them to this [Vec] and then reverse it at the end before adding them to the
        // message channel.
        let mut queue = Vec::new();
        let entries: Vec<(UserPubKey, bool)> = persistent_queue.iter().collect::<Result<_, _>>()?;
        for (key, insert) in entries.into_iter().rev() {
            if index.contains_key(&key) {
                // This is an older value for `key`.
                continue;
            }
            if insert {
                // This is the most recent value for `key`, and it is an insert, which means `key`
                // is in the queue. Go ahead and add it to the index and the message channel.
                index.insert(key.clone(), true);
                queue.push(key);
            } else {
                // This is the most recent value for `key`, and it is a delete, which means `key` is
                // not in the queue. Remember this information in `index`.
                index.insert(key, false);
            }
        }

        let (sender, receiver) = mpmc::unbounded();
        for key in queue.into_iter().rev() {
            // `send` only fails if the receiving end of the channel has been dropped, but we have
            // the receiving end right now, so this `unwrap` will never fail.
            sender.send(key).await.unwrap();
        }

        Ok(Self {
            index: Arc::new(Mutex::new(FaucetQueueIndex {
                index: index.into_keys().collect(),
                queue: persistent_queue,
                store,
            })),
            sender,
            receiver,
            max_len,
        })
    }

    async fn push(&self, key: UserPubKey) -> Result<(), FaucetError> {
        {
            // Try to insert this key into the index.
            let mut index = self.index.lock().await;
            if let Some(max_len) = self.max_len {
                if index.len() >= max_len {
                    return Err(FaucetError::QueueFull { max_len });
                }
            }
            if !index.insert(key.clone())? {
                return Err(FaucetError::AlreadyInQueue { key });
            }
        }
        // If we successfully added the key to the index, we can send it to a receiver.
        if self.sender.send(key).await.is_err() {
            tracing::warn!("failed to add request to the queue: channel is closed");
        }
        Ok(())
    }

    async fn pop(&mut self) -> Option<UserPubKey> {
        let key = self.receiver.next().await?;
        Some(key)
    }

    async fn finalize(&mut self, request: UserPubKey, success: bool) {
        let mut index = self.index.lock().await;
        if success {
            if let Err(err) = index.remove(&request) {
                tracing::error!("error removing request {} from index: {}.", request, err);
            }
        } else if let Err(err) = self.sender.send(request).await {
            tracing::error!(
                "error re-adding failed request; request will be dropped. {}",
                err
            );
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct HealthCheck {
    pub status: FaucetStatus,
}

/// Return a JSON expression with status 200 indicating the server
/// is up and running. The JSON expression is simply,
///    `{"status": Status}`
/// where `Status` is one of
/// * "initializing"
/// * "available"
/// When the server is running but unable to process requests
/// normally, a response with status 503 and payload {"status":
/// "unavailable"} should be added.
async fn healthcheck(req: tide::Request<FaucetState>) -> Result<tide::Response, tide::Error> {
    response(
        &req,
        &HealthCheck {
            status: *req.state().status.read().await,
        },
    )
}

async fn check_service_available(state: &FaucetState) -> Result<(), tide::Error> {
    if *state.status.read().await == FaucetStatus::Available {
        Ok(())
    } else {
        Err(faucet_server_error(FaucetError::Unavailable))
    }
}

async fn request_fee_assets(
    mut req: tide::Request<FaucetState>,
) -> Result<tide::Response, tide::Error> {
    check_service_available(req.state()).await?;
    let pub_key: UserPubKey = net::server::request_body(&mut req).await?;
    response(&req, &req.state().queue.push(pub_key).await?)
}

async fn worker(id: usize, mut state: FaucetState) {
    'wait_for_requests: while let Some(pub_key) = state.queue.pop().await {
        let mut wallet = state.wallet.lock().await;
        let faucet_addr = wallet.pub_keys().await[0].address();

        for _ in 0..state.num_grants {
            tracing::info!(
                "worker {}: transferring {} tokens from {} to {}",
                id,
                state.grant_size,
                net::UserAddress(faucet_addr.clone()),
                net::UserAddress(pub_key.address())
            );
            let balance = wallet.balance(&AssetCode::native()).await;
            let records = spendable_records(&wallet, state.grant_size).await.count();
            tracing::info!(
                "worker {}: wallet balance before transfer: {} across {} records",
                id,
                balance,
                records
            );
            if let Err(err) = wallet
                .transfer(
                    Some(&faucet_addr),
                    &AssetCode::native(),
                    &[(pub_key.address(), state.grant_size)],
                    state.fee_size,
                )
                .await
            {
                tracing::error!("worker {}: failed to transfer: {}", id, err);
                // If we failed, finalize the request as failed in the queue so it can be retried
                // later.
                state.queue.finalize(pub_key, false).await;
                continue 'wait_for_requests;
            }
        }
        drop(wallet);

        // Delete this request from the queue, as we have satisfied it.
        state.queue.finalize(pub_key, true).await;

        // Signal the record breaking thread that we have spent some records, so that it can create
        // more by breaking up larger records. Drop our handle to the wallet (which we no longer
        // need) so that the thread can access it.
        if state.signal_breaker_thread.clone().try_send(()).is_err() {
            tracing::error!(
                "worker {}: error signalling the breaker thread. Perhaps it has crashed?",
                id
            );
        }
    }

    tracing::warn!("worker {}: exiting, request queue closed", id);
}

async fn spendable_records(
    wallet: &CapeWallet<'static, CapeBackend<'static>>,
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
            } else {
                // We don't have enough records and we do have a big record to break up. Break out
                // of the wait loop and enter the next loop to break up our records.
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

async fn wait_for_eqs(opt: &FaucetOptions) -> Result<(), CapeWalletError> {
    let mut backoff = Duration::from_millis(500);
    for _ in 0..8 {
        if surf::connect(&opt.eqs_url).send().await.is_ok() {
            return Ok(());
        }
        tracing::warn!("unable to connect to EQS; sleeping for {:?}", backoff);
        sleep(backoff).await;
        backoff *= 2;
    }

    let msg = format!("failed to connect to EQS after {:?}", backoff);
    tracing::error!("{}", msg);
    Err(CapeWalletError::Failed { msg })
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
    wait_for_eqs(opt).await.unwrap();
    let mut loader = CapeLoader::recovery(
        opt.mnemonic.clone().replace('-', " "),
        password,
        opt.faucet_wallet_path.clone(),
        CapeLoader::latest_contract(opt.eqs_url.clone())
            .await
            .unwrap(),
    );
    let backend = CapeBackend::new(
        &*UNIVERSAL_PARAM,
        CapeBackendConfig {
            // We're not going to do any direct-to-contract operations that would require a
            // connection to the CAPE contract or an ETH wallet. Everything we do will go through
            // the relayer.
            web3_provider: None,
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

    // Start the app before we wait for the key scan to complete. If we have to restart the faucet
    // service from scratch (for example, if the wallet storage format changes and we need to
    // recreate our files from a mnemonic) the key scan could take a very long time. We want the
    // healthcheck endpoint to be available and returning "initializing" during that time, so the
    // load balancer doesn't kill the service before it has a chance to start up. Other endpoints
    // will fail while the app is initializing. Once initialization is complete, the healthcheck
    // state will change to "available" and the other endpoints will start to work.
    //
    // The app state includes a bounded channel used to signal the record breaking thread when we
    // need it to break large records into smaller ones. We use the total number of records to
    // maintain as a conservative upper bound on how backed up the message channel can get.
    let signal_breaker_thread = mpsc::channel(opt.num_records);
    let state = FaucetState::new(wallet, signal_breaker_thread.0, opt)
        .await
        .unwrap();
    let mut app = tide::with_state(state.clone());
    app.at("/healthcheck").get(healthcheck);
    app.with(
        CorsMiddleware::new()
            .allow_methods("GET, POST".parse::<HeaderValue>().unwrap())
            .allow_headers("*".parse::<HeaderValue>().unwrap())
            .allow_origin(Origin::from("*")),
    );
    app.at("/request_fee_assets").post(request_fee_assets);
    let address = format!("0.0.0.0:{}", opt.faucet_port);
    let handle = spawn(app.listen(address));

    if let Some(key) = new_key {
        // Wait until we have scanned the ledger for records belonging to this key.
        state
            .wallet
            .lock()
            .await
            .await_key_scan(&key.address())
            .await
            .unwrap();
    }

    let bal = state
        .wallet
        .lock()
        .await
        .balance(&AssetCode::native())
        .await;
    tracing::info!("Wallet balance before init: {}", bal);

    // Spawn a thread to break records into smaller records to maintain `opt.num_records` at a time.
    spawn(break_up_records(state.clone(), signal_breaker_thread.1));

    // Wait for the thread to create at least `opt.num_records` if possible, before starting to
    // handle requests.
    wait_for_records(&state).await;

    // Spawn the worker threads that will handle faucet requests.
    for id in 0..opt.num_workers {
        spawn(worker(id, state.clone()));
    }

    *state.status.write().await = FaucetStatus::Available;

    Ok(handle)
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
    use cap_rust_sandbox::universal_param::UNIVERSAL_PARAM;
    use cape_wallet::testing::{create_test_network, retry, rpc_url_for_test, spawn_eqs};
    use ethers::prelude::U256;
    use futures::future::join_all;
    use jf_cap::structs::AssetDefinition;
    use rand_chacha::{rand_core::SeedableRng, ChaChaRng};
    use seahorse::hd::KeyTree;
    use std::path::PathBuf;
    use tempdir::TempDir;
    use tracing_test::traced_test;

    async fn parallel_request(num_requests: usize) {
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
            num_records: num_grants * num_requests,
            fee_size: 0u64.into(),
            eqs_url: eqs_url.clone(),
            relayer_url: relayer_url.clone(),
            address_book_url: address_book_url.clone(),
            min_polling_delay_ms: 500,
            max_queue_len: Some(num_requests),
            num_workers: num_requests,
        };
        init_web_server(&opt, Some(faucet_key_pair)).await.unwrap();
        println!("Faucet server initiated.");

        // Check the status is "available".
        let mut res = surf::get(format!("http://localhost:{}/healthcheck", faucet_port))
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::Ok);
        assert_eq!(
            HealthCheck {
                status: FaucetStatus::Available
            },
            res.body_json().await.unwrap(),
        );

        // Create receiver wallets.
        let mut wallets = Vec::new();
        let mut keys = Vec::new();
        let mut temp_dirs = Vec::new();
        for i in 0..num_requests {
            let receiver_dir = TempDir::new("cape_wallet_receiver").unwrap();
            let mut receiver_loader = CapeLoader::from_literal(
                Some(KeyTree::random(&mut rng).1.to_string().replace('-', " ")),
                Alphanumeric.sample_string(&mut rand::thread_rng(), 16),
                PathBuf::from(receiver_dir.path()),
                contract_address.into(),
            );
            let receiver_backend = CapeBackend::new(
                universal_param,
                CapeBackendConfig {
                    web3_provider: Some(rpc_url_for_test()),
                    eqs_url: eqs_url.clone(),
                    relayer_url: relayer_url.clone(),
                    address_book_url: address_book_url.clone(),
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
            println!("Receiver wallet {} created.", i);

            temp_dirs.push(receiver_dir);
            wallets.push(receiver);
            keys.push(receiver_key);
        }

        join_all(
            wallets
                .into_iter()
                .zip(keys)
                .map(|(wallet, key)| {
                    let url = format!("http://localhost:{}/request_fee_assets", faucet_port);
                    async move {
                        // Request native asset for the receiver.
                        let response = surf::post(url)
                            .content_type(surf::http::mime::BYTE_STREAM)
                            .body_bytes(&bincode::serialize(&key).unwrap())
                            .await
                            .unwrap();
                        assert_eq!(response.status(), StatusCode::Ok);
                        println!("Asset transferred.");

                        // Check the balance.
                        retry(|| async {
                            wallet.balance(&AssetCode::native()).await
                                == U256::from(grant_size) * num_grants
                        })
                        .await;

                        // We should have received `num_grants` records of `grant_size` each.
                        let records = wallet.records().await.collect::<Vec<_>>();
                        assert_eq!(records.len(), 5);
                        for record in records {
                            assert_eq!(record.ro.asset_def, AssetDefinition::native());
                            assert_eq!(record.ro.pub_key, key);
                            assert_eq!(record.amount(), grant_size);
                        }
                    }
                })
                .collect::<Vec<_>>(),
        )
        .await;
    }

    #[async_std::test]
    #[traced_test]
    async fn test_faucet_transfer() {
        parallel_request(1).await;
    }

    #[cfg(feature = "slow-tests")]
    #[async_std::test]
    #[traced_test]
    async fn test_faucet_simultaneous_transfer() {
        parallel_request(5).await;
    }
}
