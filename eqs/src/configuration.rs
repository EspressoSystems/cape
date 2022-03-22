// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use dirs::data_local_dir;
use ethers::prelude::Address;
use std::{env, path::PathBuf, time::Duration};
use structopt::StructOpt;

// TODO: migrate to clap; clap 3.0 incorporates most of StructOpt
#[derive(Debug, StructOpt)]
#[structopt(
    name = "CAPE Ethereum Query Server",
    about = "Monitors for changes on the CAPE contract, provides query service for contract state"
)]
pub struct EQSOptions {
    /// Path to assets including web server files.
    #[structopt(long = "assets", default_value = "")]
    pub web_path: String,

    /// Path to API specification and messages.
    #[structopt(long = "api", default_value = "")]
    pub api_path: String,

    /// Path to persistence files.
    ///
    /// Persistence files will be nested under the specified directory
    #[structopt(long, short, default_value = "", env = "CAPE_EQS_STORE_PATH")]
    pub store_path: String,

    /// URL for Ethers HTTP Provider
    #[structopt(
        long,
        env = "CAPE_WEB3_PROVIDER_URL",
        default_value = "http://localhost:8545"
    )]
    pub rpc_url: String,

    /// Address for CAPE contract
    #[structopt(long, env = "CAPE_CONTRACT_ADDRESS")]
    pub cape_address: Option<Address>,

    /// Invoke as a test-only instance; will create and use test contract
    /// Will also use a temp persistence path and not restore history
    #[structopt(long)]
    pub temp_test_run: bool,

    /// Flag to reset persisted state.
    #[structopt(long)]
    pub reset_store_state: bool,

    /// Polling frequency, in milliseconds, for commits to the contract.
    #[structopt(long, default_value = "500")]
    pub query_frequency: u64,

    /// Web service port .
    #[structopt(long, default_value = "50087", env = "CAPE_EQS_PORT")]
    pub eqs_port: u16,
}

fn default_data_path() -> PathBuf {
    let mut data_dir = data_local_dir()
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from("./")));
    data_dir.push("espresso");
    data_dir.push("cape_eqs");
    data_dir
}

impl EQSOptions {
    pub fn web_path(&self) -> PathBuf {
        let web_path = &self.web_path;
        if web_path.is_empty() {
            let mut cur_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("./"));
            cur_dir.push("local");
            cur_dir
        } else {
            PathBuf::from(web_path)
        }
    }

    pub(crate) fn api_path(&self) -> PathBuf {
        let api_path = &self.api_path;
        if api_path.is_empty() {
            let mut cur_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("./"));
            cur_dir.push("api");
            cur_dir.push("api.toml");
            cur_dir
        } else {
            PathBuf::from(api_path)
        }
    }

    /// Returns the path to stored persistence files.
    pub(crate) fn store_path(&self) -> PathBuf {
        let store_path = &self.store_path;
        if store_path.is_empty() {
            let mut default_store_path = default_data_path();
            default_store_path.push("store");
            default_store_path
        } else {
            PathBuf::from(store_path)
        }
    }

    pub(crate) fn rpc_url(&self) -> &str {
        &self.rpc_url
    }

    pub(crate) fn cape_address(&self) -> Option<Address> {
        self.cape_address
    }

    pub(crate) fn temp_test_run(&self) -> bool {
        self.temp_test_run
    }

    pub(crate) fn reset_state(&self) -> bool {
        self.reset_store_state
    }

    pub(crate) fn query_frequency(&self) -> Duration {
        Duration::from_millis(self.query_frequency)
    }

    pub(crate) fn eqs_port(&self) -> u16 {
        self.eqs_port
    }
}
