use async_std::path::{Path, PathBuf};
use async_std::{
    sync::{Arc, RwLock},
    task::{sleep, spawn, JoinHandle},
};
use atomic_store::{
    error::PersistenceError, load_store::BincodeLoadStore, AtomicStore, AtomicStoreLoader,
};
use jf_cap::keys::{UserAddress, UserPubKey};
use jf_cap::Signature;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::HashMap;
use std::env;
use std::time::Duration;
use structopt::StructOpt;
use tide::{log::LevelFilter, prelude::*, StatusCode};

const ADDRESS_BOOK_STARTUP_RETRIES: usize = 8;

pub static mut LOG_LEVEL: LevelFilter = LevelFilter::Info;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "Address Book",
    about = "Server that provides a key/value store mapping user addresses to public keys"
)]
pub struct ServerOpt {
    /// Whether to load from persisted state. Defaults to true.
    ///
    #[structopt(
        long = "load_from_store",
        short = "l",
        parse(try_from_str),
        default_value = "true"
    )]
    pub load_from_store: bool,

    /// Path to persistence files.
    ///
    /// Persistence files will be nested under the specified directory
    #[structopt(
        long = "store_path",
        short = "s",
        default_value = ""      // See fn default_store_path().
    )]
    pub store_path: String,

    /// Base URL. Defaults to http://0.0.0.0:50078.
    #[structopt(long = "url", default_value = "http://0.0.0.0:50078")]
    pub base_url: String,
}

pub fn address_book_port() -> Option<String> {
    std::env::var("PORT").ok()
}

/// Returns the project directory.
pub fn project_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Returns the default directory to store persistence files.
pub fn default_store_path() -> PathBuf {
    const STORE_DIR: &str = "src/store/address_book";
    let dir = project_path();
    [&dir, Path::new(STORE_DIR)].iter().collect()
}

/// Runs one and only one logger.
///
/// Accessing `LOG_LEVEL` is considered unsafe since it is a static mutable
/// variable, but we need this to ensure that only one logger is running.
static LOGGING: Lazy<()> = Lazy::new(|| unsafe {
    tide::log::with_level(LOG_LEVEL);
});

#[derive(Debug, Deserialize, Serialize)]
pub struct InsertPubKey {
    pub pub_key_bytes: Vec<u8>,
    pub sig: Signature,
}

type AddressKeyMap = HashMap<UserAddress, UserPubKey>;

#[derive(Clone, Default)]
pub struct ServerState {
    pub map: Arc<RwLock<AddressKeyMap>>,
}

/// If PORT is set in the environment, derive a new base_url from
/// the given one with the port from the environment.
pub fn override_port_from_env(base_url: &str) -> String {
    let override_url;
    let url = if address_book_port().is_none() {
        base_url
    } else {
        let override_port = address_book_port().unwrap();
        let re = Regex::new(":[0-9]+").unwrap();
        if re.is_match(base_url) {
            // Replace the port from the base URL.
            //    http://localhost:0123 -> http://localhost:0456
            let colon_port = format!(":{}", override_port);
            override_url = re.replace(base_url, colon_port);
        } else {
            // Add the port before the first single slash.
            // This slash does not have a colon or slash preceeding it.
            let re2 = Regex::new("(?P<z>[^:/])/").unwrap();
            if re2.is_match(base_url) {
                let port_slash = format!("$z:{}/", override_port);
                override_url = re2.replace(base_url, port_slash);
            } else {
                // The base URL does not specify the port and does not
                // have a slash after the protocol. Simply append
                // a colon and the port.
                override_url = format!("{}:{}", base_url, override_port).into()
            }
        }
        override_url.as_ref()
    };
    url.to_string()
}

pub async fn init_web_server(
    log_level: LevelFilter,
    base_url: &str,
    _store_path: Option<&PathBuf>,
) -> std::io::Result<JoinHandle<std::io::Result<()>>> {
    // Accessing `LOG_LEVEL` is considered unsafe since it is a static mutable
    // variable, but we need this to ensure that only one logger is running.
    // This is the only place in the code that modifies this and this function
    // is only called once at startup.
    unsafe {
        LOG_LEVEL = log_level;
    }
    Lazy::force(&LOGGING);
    // TODO !corbett Initialize the ServerState from AtomicStore.
    let mut app = tide::with_state(ServerState::default());
    app.at("/insert_pubkey").post(insert_pubkey);
    app.at("/request_pubkey").post(request_pubkey);
    let url = override_port_from_env(base_url);
    Ok(spawn(app.listen(url)))
}

pub async fn wait_for_server(base_url: &str) {
    // Wait for the server to come up and start serving.
    let mut backoff = Duration::from_millis(100);
    for _ in 0..ADDRESS_BOOK_STARTUP_RETRIES {
        if surf::connect(base_url.to_string()).send().await.is_ok() {
            return;
        }
        sleep(backoff).await;
        backoff *= 2;
    }
    panic!("Address Book did not start in {:?} milliseconds", backoff);
}

/// Lookup a user public key from a signed public key address. Fail with
/// tide::StatusCode::BadRequest if key deserialization or the signature check
/// fail.
fn verify_sig_and_get_pub_key(insert_request: InsertPubKey) -> Result<UserPubKey, tide::Error> {
    let pub_key: UserPubKey = bincode::deserialize(&insert_request.pub_key_bytes)
        .map_err(|e| tide::Error::new(tide::StatusCode::BadRequest, e))?;
    pub_key
        .verify_sig(&insert_request.pub_key_bytes, &insert_request.sig)
        .map_err(|e| tide::Error::new(tide::StatusCode::BadRequest, e))?;
    Ok(pub_key)
}

/// Insert or update the public key at the given address.
async fn insert_pubkey(mut req: tide::Request<ServerState>) -> Result<tide::Response, tide::Error> {
    let insert_request: InsertPubKey = net::server::request_body(&mut req).await?;
    let pub_key = verify_sig_and_get_pub_key(insert_request)?;
    {
        let mut hash_map = req.state().map.write().await;
        hash_map.insert(pub_key.address(), pub_key.clone());
    }
    Ok(tide::Response::new(StatusCode::Ok))
}

/// Fetch the public key for the given address. If not found, return
/// StatusCode::NotFound.
async fn request_pubkey(
    mut req: tide::Request<ServerState>,
) -> Result<tide::Response, tide::Error> {
    let address: UserAddress = net::server::request_body(&mut req).await?;
    let hash_map = req.state().map.read().await;
    match hash_map.get(&address) {
        Some(value) => {
            let bytes = bincode::serialize(value).unwrap();
            let response = tide::Response::builder(StatusCode::Ok)
                .body(bytes)
                .content_type(tide::http::mime::BYTE_STREAM)
                .build();
            Ok(response)
        }
        _ => Ok(tide::Response::new(StatusCode::NotFound)),
    }
}

pub fn write_store(_address_key_map: AddressKeyMap) {
    let mut test_path = env::current_dir()
        .map_err(|e| PersistenceError::StdIoDirOpsError { source: e })
        .expect("Bad cwd");
    test_path.push("testing_tmp");
    let mut _store_loader =
        AtomicStoreLoader::create(test_path.as_path(), "append_log_test_empty_iterator")
            .expect("AtomicStoreLoader::create failed");
}

pub fn read_store(_address_key_map: BincodeLoadStore<AddressKeyMap>) {
    let mut test_path = env::current_dir()
        .map_err(|e| PersistenceError::StdIoDirOpsError { source: e })
        .expect("Bad cwd");
    test_path.push("testing_tmp");
    let store_loader =
        AtomicStoreLoader::create(test_path.as_path(), "append_log_test_empty_iterator")
            .expect("AtomicStoreLoader::create failed");

    let _atomic_store = AtomicStore::open(store_loader).expect("open failed");
}
