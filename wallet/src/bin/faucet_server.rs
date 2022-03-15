// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

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
use cape_wallet::{
    routes::{init_wallet, Wallet},
    wallet::CapeWalletError,
    web::default_storage_path,
};
use jf_cap::{
    keys::{UserKeyPair, UserPubKey},
    structs::AssetCode,
};
use rand_chacha::{rand_core::SeedableRng, ChaChaRng};
use seahorse::loader::Loader;
use serde::{Deserialize, Serialize};
use snafu::Snafu;
use std::path::PathBuf;
use structopt::StructOpt;
use tide::StatusCode;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "CAPE Faucet Server",
    about = "Grants a native asset seed to a provided UserPubKey"
)]
pub struct FaucetOptions {
    /// mnemonic for the faucet wallet
    #[structopt(long = "mnemonic", env = "MNEMONIC", default_value = "")]
    pub mnemonic: String,

    /// path to the faucet wallet
    #[structopt(long = "faucet_wallet_path", env = "FAUCET_WALLET_PATH")]
    pub faucet_wallet_path: PathBuf,

    /// password on the faucet account keyfile
    #[structopt(long = "faucet_password", env = "FAUCET_PASSWORD", default_value = "")]
    pub faucet_password: String,

    /// binding port for the faucet service
    #[structopt(long = "faucet_port", env = "FAUCET_PORT", default_value = "50079")]
    pub faucet_port: String,

    /// size of transfer for faucet grant
    #[structopt(long = "grant_size", env = "FAUCET_GRANT", default_value = "5000")]
    pub grant_size: u64,

    /// fee for faucet grant
    #[structopt(long = "fee_size", env = "FAUCET_FEE", default_value = "100")]
    pub fee_size: u64,
}

#[derive(Clone)]
struct FaucetState {
    wallet: Arc<Mutex<Wallet>>,
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

async fn request_fee_assets(
    mut req: tide::Request<FaucetState>,
) -> Result<tide::Response, tide::Error> {
    let bytes = req.body_bytes().await?;
    let pub_key: UserPubKey = bincode::deserialize(&bytes)?;
    let mut wallet = req.state().wallet.lock().await;
    let faucet_addr = wallet.pub_keys().await[0].address();
    wallet
        .transfer(
            Some(&faucet_addr),
            &AssetCode::native(),
            &[(pub_key.address(), req.state().grant_size)],
            req.state().fee_size,
        )
        .await
        .map_err(faucet_error)?;
    net::server::response(&req, ())
}

pub async fn init_web_server(
    opt: &FaucetOptions,
) -> std::io::Result<JoinHandle<std::io::Result<()>>> {
    let mut rng = ChaChaRng::from_entropy();
    let faucet_key_pair = UserKeyPair::generate(&mut rng);
    let loader = Loader::from_literal(
        Some(opt.mnemonic.clone().replace('-', " ")),
        opt.faucet_password.clone(),
        opt.faucet_wallet_path.clone(),
    );
    let state = FaucetState {
        wallet: Arc::new(Mutex::new(
            init_wallet(
                &mut rng,
                faucet_key_pair.pub_key(),
                loader,
                true,
                &default_storage_path(),
            )
            .await
            .unwrap(),
        )),
        grant_size: opt.grant_size,
        fee_size: opt.fee_size,
    };
    let mut app = tide::with_state(state);
    app.at("/request_fee_assets").post(request_fee_assets);
    let address = format!("0.0.0.0:{}", opt.faucet_port);
    Ok(spawn(app.listen(address)))
}

#[async_std::main]
async fn main() -> Result<(), std::io::Error> {
    tracing_subscriber::fmt().pretty().init();

    // Initialize the faucet web server.
    //
    init_web_server(&FaucetOptions::from_args()).await?;

    Ok(())
}
