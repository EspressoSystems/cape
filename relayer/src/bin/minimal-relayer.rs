// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

#[warn(unused_imports)]
use cap_rust_sandbox::{
    ethereum::{ensure_connected_to_contract, get_provider_from_url},
    types::CAPE,
};
use ethers::prelude::{
    coins_bip39::English, Address, Middleware, MnemonicBuilder, Signer, SignerMiddleware,
};
use relayer::{
    init_web_server, submit_empty_block_loop, NonceCountRule, WebState, DEFAULT_RELAYER_GAS_LIMIT,
    DEFAULT_RELAYER_PORT,
};
use std::{num::NonZeroU64, sync::Arc, time::Duration};
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

    /// Port the relayer web server listens on.
    #[structopt(long, env = "CAPE_RELAYER_PORT", default_value = DEFAULT_RELAYER_PORT)]
    port: u16,

    /// Determines how transaction nonces should be calculated.
    ///
    /// * `"mined"` - only count mined transaction when creating the nonce.
    /// * `"pending"` - also include pending transactions when creating the nonce.
    ///
    /// Including "pending" transactions allows the relayer to submit the next
    /// transaction as soon as the previous one hit the nodes' mempool.
    #[structopt(
        long,
        env = "CAPE_RELAYER_NONCE_COUNT_RULE",
        default_value = "pending",
        verbatim_doc_comment
    )]
    nonce_count_rule: NonceCountRule,

    /// Amount of time between submission of empty blocks.
    ///
    /// The empty blocks process the pending deposits and prevent the pending
    /// deposits queue from filling up.
    #[structopt(
        long,
        env = "CAPE_RELAYER_EMPTY_BLOCK_INTERVAL_SECS",
        default_value = "300"
    )]
    empty_block_interval: NonZeroU64,

    /// Ethereum gas limit for the relayer's Ethereum transactions.
    ///
    /// The default of 10M is enough to cover the gas cost of submitting one note
    /// and crediting up to 10 pending deposits in the smart contract.
    #[structopt(long, env = "CAPE_RELAYER_GAS_LIMIT", default_value = DEFAULT_RELAYER_GAS_LIMIT)]
    gas_limit: NonZeroU64,
}

#[async_std::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .compact()
        .with_ansi(false)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

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

    let web_state = WebState::new(contract, opt.nonce_count_rule, opt.gas_limit.into());
    // Start serving CAPE transaction submissions.
    let periodic_block_submission = async_std::task::spawn(submit_empty_block_loop(
        web_state.clone(),
        Duration::from_secs(opt.empty_block_interval.into()),
    ));
    let web_server = init_web_server(web_state, opt.port);
    let _result = futures::future::join(periodic_block_submission, web_server).await;
    Ok(())
}
