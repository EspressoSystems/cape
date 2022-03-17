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
use cap_rust_sandbox::{ledger::CapeLedger, universal_param::UNIVERSAL_PARAM};
use cape_wallet::{
    mocks::{MockCapeBackend, MockCapeNetwork},
    routes::{wallet_error, Wallet},
    wallet::CapeWalletError,
};
use jf_cap::{
    keys::{UserKeyPair, UserPubKey},
    structs::{
        AssetCode, AssetDefinition as JfAssetDefinition, FreezeFlag, ReceiverMemo,
        RecordCommitment, RecordOpening,
    },
    MerkleTree, TransactionVerifyingKey,
};
use key_set::{KeySet, VerifierKeySet};
use rand::distributions::{Alphanumeric, DistString};
use rand_chacha::{rand_core::SeedableRng, ChaChaRng};
use seahorse::loader::Loader;
use seahorse::reef::Ledger;
use seahorse::testing::MockLedger;
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
    #[structopt(long, env = "CAPE_FAUCET_WALLET_MNEMONIC", default_value = "")]
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

pub async fn load_faucet_wallet(
    rng: &mut ChaChaRng,
    faucet_pub_key: UserPubKey,
    mut loader: Loader,
) -> Result<Wallet, tide::Error> {
    let verif_crs = VerifierKeySet {
        mint: TransactionVerifyingKey::Mint(
            jf_cap::proof::mint::preprocess(&*UNIVERSAL_PARAM, CapeLedger::merkle_height())?.1,
        ),
        xfr: KeySet::new(
            vec![
                TransactionVerifyingKey::Transfer(
                    jf_cap::proof::transfer::preprocess(
                        &*UNIVERSAL_PARAM,
                        2,
                        2,
                        CapeLedger::merkle_height(),
                    )?
                    .1,
                ),
                TransactionVerifyingKey::Transfer(
                    jf_cap::proof::transfer::preprocess(
                        &*UNIVERSAL_PARAM,
                        3,
                        3,
                        CapeLedger::merkle_height(),
                    )?
                    .1,
                ),
            ]
            .into_iter(),
        )
        .unwrap(),
        freeze: KeySet::new(
            vec![TransactionVerifyingKey::Freeze(
                jf_cap::proof::freeze::preprocess(
                    &*UNIVERSAL_PARAM,
                    2,
                    CapeLedger::merkle_height(),
                )?
                .1,
            )]
            .into_iter(),
        )
        .unwrap(),
    };

    // Set up a faucet record.
    let mut records = MerkleTree::new(CapeLedger::merkle_height()).unwrap();
    let faucet_ro = RecordOpening::new(
        rng,
        1000,
        JfAssetDefinition::native(),
        faucet_pub_key,
        FreezeFlag::Unfrozen,
    );
    records.push(RecordCommitment::from(&faucet_ro).to_field_element());
    let faucet_memo = ReceiverMemo::from_ro(rng, &faucet_ro, &[]).unwrap();

    let mut ledger = MockLedger::new(MockCapeNetwork::new(
        verif_crs,
        records,
        vec![(faucet_memo, 0)],
    ));
    ledger.set_block_size(1).unwrap();

    let backend = MockCapeBackend::new(Arc::new(Mutex::new(ledger)), &mut loader)?;
    let wallet = Wallet::new(backend).await.map_err(wallet_error)?;
    Ok(wallet)
}

pub async fn init_web_server(
    opt: &FaucetOptions,
) -> std::io::Result<JoinHandle<std::io::Result<()>>> {
    let mut rng = ChaChaRng::from_entropy();
    let faucet_key_pair = UserKeyPair::generate(&mut rng);
    let mut password = opt.faucet_password.clone();
    if password.is_empty() {
        password = Alphanumeric.sample_string(&mut rand::thread_rng(), 16);
    }
    let loader = Loader::from_literal(
        Some(opt.mnemonic.clone().replace('-', " ")),
        password,
        opt.faucet_wallet_path.clone(),
    );
    let state = FaucetState {
        wallet: Arc::new(Mutex::new(
            load_faucet_wallet(&mut rng, faucet_key_pair.pub_key(), loader)
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
    init_web_server(&FaucetOptions::from_args()).await?.await?;

    Ok(())
}
