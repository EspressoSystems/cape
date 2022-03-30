// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

#![doc = include_str!("../../README.md")]
#[warn(unused_imports)]
use cap_rust_sandbox::{
    ethereum::{ensure_connected_to_contract, get_provider_from_url},
    types::CAPE,
};
use ethers::prelude::{
    coins_bip39::English, Address, Middleware, MnemonicBuilder, Signer, SignerMiddleware,
};
use relayer::{init_web_server, DEFAULT_RELAYER_PORT};
use std::sync::Arc;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "Minimal CAPE Relayer")]
struct MinimalRelayerOptions {
    /// URL for Ethers provider
    #[structopt(
        long,
        env = "CAPE_WEB3_PROVIDER_URL",
        default_value = "http://localhost:8545"
    )]
    rpc_url: String,

    /// Address for CAPE submit
    #[structopt(env = "CAPE_CONTRACT_ADDRESS")]
    cape_address: Address,

    /// Mnemonic phrase for ETH wallet, for paying submission gas fees.
    #[structopt(env = "CAPE_RELAYER_WALLET_MNEMONIC")]
    mnemonic: String,
}

#[async_std::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .compact()
        .with_ansi(false)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env());

    let opt = MinimalRelayerOptions::from_args();

    // Set up a client to submit ETH transactions.
    let provider = get_provider_from_url(&opt.rpc_url);

    ensure_connected_to_contract(&provider, opt.cape_address)
        .await
        .unwrap();

    let wallet = MnemonicBuilder::<English>::default()
        .phrase(opt.mnemonic.as_str())
        .build()
        .expect("could not open relayer wallet")
        .with_chain_id(provider.get_chainid().await.unwrap().as_u64());
    let client = Arc::new(SignerMiddleware::new(provider, wallet));

    // Connect to CAPE smart contract.
    let contract = CAPE::new(opt.cape_address, client);

    // Start serving CAPE transaction submissions.
    let port =
        std::env::var("CAPE_RELAYER_PORT").unwrap_or_else(|_| DEFAULT_RELAYER_PORT.to_string());
    init_web_server(contract, port).await
}
