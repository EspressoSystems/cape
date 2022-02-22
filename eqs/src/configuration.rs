use cap_rust_sandbox::{model::CAPE_MERKLE_HEIGHT, universal_param::UNIVERSAL_PARAM};
use jf_cap::TransactionVerifyingKey;
use key_set::{KeySet, VerifierKeySet};

use dirs::data_local_dir;
use std::{env, path::PathBuf, time::Duration};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "CAPE Ethereum Query Server",
    about = "Monitors for changes on the CAPE constract, provides query service for contract state"
)]
struct EQSOptions {
    /// Path to eqs configuration file.
    // #[structopt(long = "config", short = "c", default_value = "")]
    // config: String,

    // /// Flag to update config fields.
    // #[structopt(long = "update_config_file")]
    // update_config_file: bool,

    /// Path to persistence files.
    ///
    /// Persistence files will be nested under the specified directory
    #[structopt(long = "store_path", short = "s", default_value = "")]
    store_path: String,

    /// Flag to reset persisted state.
    #[structopt(long = "reset_store_state")]
    reset_state_store: bool,
}

fn default_data_path() -> PathBuf {
    let mut data_dir = data_local_dir()
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from("./")));
    data_dir.push("tri");
    data_dir.push("cape_eqs");
    data_dir
}

/// Returns the path to stored persistence files.
pub(crate) fn store_path() -> PathBuf {
    let store_path = EQSOptions::from_args().store_path;
    if store_path.is_empty() {
        let mut default_store_path = default_data_path();
        default_store_path.push("store");
        default_store_path
    } else {
        PathBuf::from(store_path)
    }
}

pub(crate) fn reset_state() -> bool {
    EQSOptions::from_args().reset_state_store
}

// presumably, it's worth storing and verifying a hash of these, rather than downloading the keys from the chain.
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

pub(crate) fn query_frequency() -> Duration {
    // should be a command line or config option
    Duration::from_millis(500)
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
