// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use cap_rust_sandbox::{model::CAPE_MERKLE_HEIGHT, universal_param::UNIVERSAL_PARAM};
use jf_cap::keys::{UserAddress, UserKeyPair};
use jf_cap::TransactionVerifyingKey;
use key_set::{KeySet, VerifierKeySet};

use dirs::data_local_dir;
use rand_chacha::{rand_core::SeedableRng, ChaChaRng};
use std::{env, path::PathBuf};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "CAPE Relayer",
    about = "Collects, validates, batches, and submits blocks of transactions to CAPE contract"
)]
struct RelayerOptions {
    /// Path to relayer configuration file.
    // #[structopt(long, short, default_value = "")]
    // config: String,

    // /// Flag to update config fields.
    // #[structopt(long)]
    // update_config_file: bool,

    /// Path to persistence files.
    ///
    /// Persistence files will be nested under the specified directory
    #[structopt(long, short, default_value = "")]
    store_path: String,

    /// Flag to reset persisted state.
    #[structopt(long)]
    reset_state_store: bool,
    // /// Address for EQS
    // #[structopt(long, default_value = "")]
    // eqs_address: String,

    // /// Address for CAPE submit
    // #[structopt(long, default_value = "")]
    // cape_address: String,
}

fn default_data_path() -> PathBuf {
    let mut data_dir = data_local_dir()
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from("./")));
    data_dir.push("tri");
    data_dir.push("cape_relayer");
    data_dir
}

/// Returns the path to stored persistence files.
pub(crate) fn store_path() -> PathBuf {
    let store_path = RelayerOptions::from_args().store_path;
    if store_path.is_empty() {
        let mut default_store_path = default_data_path();
        default_store_path.push("store");
        default_store_path
    } else {
        PathBuf::from(store_path)
    }
}

pub(crate) fn reset_state() -> bool {
    RelayerOptions::from_args().reset_state_store
}

pub(crate) fn verifier_keys() -> VerifierKeySet {
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
        freeze: KeySet::new(vec![TransactionVerifyingKey::Freeze(freeze_verif_key)].into_iter())
            .unwrap(),
    }
}

lazy_static! {
    static ref RELAYER_KEYPAIR: UserKeyPair = {
        // TODO: this should only be for the first time; replace with store and recover
        let mut prng = ChaChaRng::from_entropy();
        UserKeyPair::generate(&mut prng)

        // TODO: load from stored, default if not specified, unless not found or reset; output error if specified and not found

        // let mut file = File::open(path.clone()).unwrap();
        // let mut bytes = Vec::new();
        // if let Err(err) = file.read_to_end(&mut bytes).unwrap();
        // let owner_keys = bincode::deserialize::<UserKeyPair>(&bytes);
        // owner_keys.address()
    };
}

pub(crate) fn relayer_addr() -> UserAddress {
    RELAYER_KEYPAIR.address()
}
