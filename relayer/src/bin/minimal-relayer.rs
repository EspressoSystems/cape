// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

#[warn(unused_imports)]
use cap_rust_sandbox::types::CAPE;
use coins_bip39::English;
use ethers::prelude::*;
use relayer::{init_web_server, DEFAULT_RELAYER_PORT};
use std::sync::Arc;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "Minimal CAPE Relayer")]
struct MinimalRelayerOptions {
    /// URL for Ethers provider
    #[structopt(short = "u", long, default_value = "http://localhost:8545")]
    rpc_url: String,

    /// Address for CAPE submit
    cape_address: Address,

    /// Mnemonic phrase for ETH wallet, for paying submission gas fees.
    mnemonic: String,
}

#[async_std::main]
async fn main() -> std::io::Result<()> {
    let opt = MinimalRelayerOptions::from_args();

    // Set up a client to submit ETH transactions.
    let wallet = MnemonicBuilder::<English>::default()
        .phrase(opt.mnemonic.as_str())
        .build()
        .expect("could not open relayer wallet");
    let provider = Provider::<Http>::try_from(opt.rpc_url.clone())
        .expect("could not instantiate HTTP Provider");
    let client = Arc::new(SignerMiddleware::new(provider, wallet));

    // Connect to CAPE smart contract.
    let contract = CAPE::new(opt.cape_address, client);

    // Start serving CAPE transaction submissions.
    let port = std::env::var("PORT").unwrap_or_else(|_| DEFAULT_RELAYER_PORT.to_string());
    init_web_server(contract, port).await
}
