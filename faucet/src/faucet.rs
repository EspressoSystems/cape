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
use ethers::prelude::Address;
use jf_cap::{
    keys::{UserKeyPair, UserPubKey},
    structs::AssetCode,
};
use rand::distributions::{Alphanumeric, DistString};
use seahorse::{
    events::EventIndex,
    loader::{Loader, LoaderMetadata},
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
    #[structopt(long, env = "CAPE_FAUCET_GRANT_SIZE", default_value = "5000")]
    pub grant_size: u64,

    /// fee for faucet grant
    #[structopt(long, env = "CAPE_FAUCET_FEE_SIZE", default_value = "100")]
    pub fee_size: u64,

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

    /// Address of the CAPE smart contract.
    #[structopt(long, env = "CAPE_CONTRACT_ADDRESS")]
    pub contract_address: Address,

    /// URL for Ethers HTTP Provider
    #[structopt(
        long,
        env = "CAPE_WEB3_PROVIDER_URL",
        default_value = "http://localhost:8545"
    )]
    pub rpc_url: Url,

    /// Minimum amount of time to wait between polling requests to EQS.
    #[structopt(long, env = "CAPE_WALLET_MIN_POLLING_DELAY", default_value = "500")]
    pub min_polling_delay_ms: u64,
}

#[derive(Clone)]
struct FaucetState {
    wallet: Arc<Mutex<CapeWallet<'static, CapeBackend<'static, LoaderMetadata>>>>,
    grant_size: u64,
    fee_size: u64,
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
    tracing::info!(
        "transferring {} tokens from {} to {}",
        req.state().grant_size,
        net::UserAddress(faucet_addr.clone()),
        net::UserAddress(pub_key.address())
    );
    let bal = wallet.balance(&AssetCode::native()).await;
    tracing::info!("Wallet balance before transfer: {}", bal);
    wallet
        .transfer(
            Some(&faucet_addr),
            &AssetCode::native(),
            &[(pub_key.address(), req.state().grant_size)],
            req.state().fee_size,
        )
        .await
        .map_err(|err| {
            tracing::error!("Failed to transfer {}", err);
            faucet_error(err)
        })?;
    net::server::response(&req, ())
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
            rpc_url: opt.rpc_url.clone(),
            eqs_url: opt.eqs_url.clone(),
            relayer_url: opt.relayer_url.clone(),
            address_book_url: opt.address_book_url.clone(),
            contract_address: opt.contract_address,
            // We don't use an Ethereum wallet. The faucet only has to do transfers. It should not
            // have to do any operations that go directly to the contract and thus require an ETH
            // fee.
            eth_mnemonic: None,
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
    if let Some(key) = faucet_key_pair {
        wallet
            .add_user_key(key, "faucet".into(), EventIndex::default())
            .await
            .unwrap();
    } else if wallet.pub_keys().await.is_empty() {
        // We pass `EventIndex::default()` to start a scan of the ledger from the beginning, in
        // order to discove the faucet record.
        wallet
            .generate_user_key("faucet".into(), Some(EventIndex::default()))
            .await
            .unwrap();
    }

    let bal = wallet.balance(&AssetCode::native()).await;
    tracing::info!("Wallet balance before init: {}", bal);
    let state = FaucetState {
        wallet: Arc::new(Mutex::new(wallet)),
        grant_size: opt.grant_size,
        fee_size: opt.fee_size,
    };
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
    use cap_rust_sandbox::universal_param::UNIVERSAL_PARAM;
    use cape_wallet::testing::{create_test_network, retry, rpc_url_for_test, spawn_eqs};
    use rand_chacha::{rand_core::SeedableRng, ChaChaRng};
    use seahorse::hd::KeyTree;
    use std::path::PathBuf;
    use tempdir::TempDir;

    #[async_std::test]
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
        let grant_size = 5000;
        let opt = FaucetOptions {
            mnemonic: mnemonic.to_string(),
            faucet_wallet_path: PathBuf::from(faucet_dir.path()),
            faucet_password: "".to_string(),
            faucet_port: faucet_port.clone(),
            grant_size,
            fee_size: 100,
            eqs_url: eqs_url.clone(),
            relayer_url: relayer_url.clone(),
            address_book_url: address_book_url.clone(),
            contract_address,
            rpc_url: rpc_url_for_test(),
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
                rpc_url: rpc_url_for_test(),
                eqs_url,
                relayer_url,
                address_book_url,
                contract_address,
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
        surf::post(format!(
            "http://localhost:{}/request_fee_assets",
            faucet_port
        ))
        .content_type(surf::http::mime::BYTE_STREAM)
        .body_bytes(&receiver_key_bytes)
        .await
        .unwrap();
        println!("Asset transferred.");

        // Check the balance.
        retry(|| async { receiver.balance(&AssetCode::native()).await == grant_size }).await;
    }
}
