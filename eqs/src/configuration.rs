// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use cap_rust_sandbox::{model::CAPE_MERKLE_HEIGHT, universal_param::UNIVERSAL_PARAM};
use jf_cap::TransactionVerifyingKey;
use key_set::{KeySet, VerifierKeySet};

use dirs::data_local_dir;
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
    #[structopt(long = "store_path", short = "s", default_value = "")]
    pub store_path: String,

    /// Flag to reset persisted state.
    #[structopt(long = "reset_store_state")]
    pub reset_state_store: bool,

    /// Polling frequency, in milliseconds, for commits to the contract.
    #[structopt(long = "query_frequency", default_value = "500")]
    pub query_frequency: u64,

    // Ethereum connection is specified by env variable.
    /// Web service port .
    #[structopt(long = "eqs_port", default_value = "50087")]
    pub eqs_port: u16,
}

fn default_data_path() -> PathBuf {
    let mut data_dir = data_local_dir()
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from("./")));
    data_dir.push("tri");
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

    pub(crate) fn reset_state(&self) -> bool {
        self.reset_state_store
    }

    pub(crate) fn verifier_keys(&self) -> VerifierKeySet {
        // Set up the validator.
        let univ_setup = &*UNIVERSAL_PARAM;
        let (_, xfr_verif_key_12, _) =
            jf_cap::proof::transfer::preprocess(univ_setup, 1, 2, CAPE_MERKLE_HEIGHT).unwrap();
        let (_, xfr_verif_key_23, _) =
            jf_cap::proof::transfer::preprocess(univ_setup, 2, 3, CAPE_MERKLE_HEIGHT).unwrap();
        let (_, mint_verif_key, _) =
            jf_cap::proof::mint::preprocess(univ_setup, CAPE_MERKLE_HEIGHT).unwrap();
        let (_, freeze_verif_key, _) =
            jf_cap::proof::freeze::preprocess(univ_setup, 2, CAPE_MERKLE_HEIGHT).unwrap();
        VerifierKeySet {
            mint: TransactionVerifyingKey::Mint(mint_verif_key),
            xfr: KeySet::new(
                vec![
                    TransactionVerifyingKey::Transfer(xfr_verif_key_12),
                    TransactionVerifyingKey::Transfer(xfr_verif_key_23),
                ]
                .into_iter(),
            )
            .unwrap(),
            freeze: KeySet::new(
                vec![TransactionVerifyingKey::Freeze(freeze_verif_key)].into_iter(),
            )
            .unwrap(),
        }
    }

    pub(crate) fn query_frequency(&self) -> Duration {
        Duration::from_millis(self.query_frequency)
    }

    pub(crate) fn eqs_port(&self) -> u16 {
        self.eqs_port
    }
}
// If we want EQS instances to provide authenticated identities in the future, for monitoring, reputation, etc...

// lazy_static! {
//     static ref EQS_KEYPAIR: UserKeyPair = {
//         // TODO: this should only be for the first time; replace with store and recover
//         let mut prng = ChaChaRng::from_entropy();
//         UserKeyPair::generate(&mut prng)

//         // TODO: load from stored, default if not specified, unless not found or reset; output error if specified and not found

//         // let mut file = File::open(path.clone()).unwrap();
//         // let mut bytes = Vec::new();
//         // if let Err(err) = file.read_to_end(&mut bytes).unwrap();
//         // let owner_keys = bincode::deserialize::<UserKeyPair>(&bytes);
//         // owner_keys.address()
//     };
// }

// pub(crate) fn eqs_addr() -> UserAddress {
//     EQS_KEYPAIR.address()
// }
