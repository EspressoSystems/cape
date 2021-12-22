use dirs::data_local_dir;
use std::{env, path::PathBuf};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "CAPE Relayer",
    about = "Collects, validates, batches, and submits blocks of transactions to CAPE contract"
)]
struct RelayerOptions {
    /// Path to relayer configuration file.
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
    // /// Address for EQS
    // #[structopt(long = "eqs_address", default_value = "")]
    // eqs_address: String,

    // /// Address for CAPE submit
    // #[structopt(long = "cape_address", default_value = "")]
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
