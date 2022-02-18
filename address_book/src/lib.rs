use async_std::{
    sync::{Arc, RwLock},
    task::{spawn, JoinHandle},
};
use atomic_store::{
    error::PersistenceError, load_store::BincodeLoadStore, AppendLog, AtomicStore,
    AtomicStoreLoader,
};
use jf_aap::keys::{UserAddress, UserPubKey};
use jf_aap::Signature;
use std::collections::HashMap;
use std::env;
use tide::{prelude::*, StatusCode};

pub const DEFAULT_PORT: u16 = 50078u16;

#[derive(Debug, Deserialize, Serialize)]
pub struct InsertPubKey {
    pub pub_key_bytes: Vec<u8>,
    pub sig: Signature,
}

type AddressKeyMap = HashMap<UserAddress, UserPubKey>;

#[derive(Clone, Default)]
struct ServerState {
    map: Arc<RwLock<AddressKeyMap>>,
}

pub async fn init_web_server(
    base_url: &str,
    store: Option<String>,
) -> std::io::Result<JoinHandle<std::io::Result<()>>> {
    match store {
        Some(store) => println!("Got: {}", store),
        None => println!("Got none"),
    };
    tide::log::start();
    let mut app = tide::with_state(ServerState::default());
    app.at("/insert_pubkey").post(insert_pubkey);
    app.at("/request_pubkey").post(request_pubkey);
    Ok(spawn(app.listen(base_url.to_string())))
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
    let mut hash_map = req.state().map.write().await;
    hash_map.insert(pub_key.address(), pub_key.clone());
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

fn write_store(address_key_map: BincodeLoadStore<AddressKeyMap>) {
    let mut test_path = env::current_dir()
        .map_err(|e| PersistenceError::StdIoDirOpsError { source: e })
        .expect("Bad cwd");
    test_path.push("testing_tmp");
    let mut store_loader =
        AtomicStoreLoader::create(test_path.as_path(), "append_log_test_empty_iterator")
            .expect("AtomicStoreLoader::create failed");
    // let mut persisted_thing = AppendLog::create(
    //     &mut store_loader,
    //     //<BincodeLoadStore<AddressKeyMap>>::default(),
    //     address_key_map,
    //     "append_thing",
    //     1024,
    // )
    // .expect("AppendLog::create failed");

    // TODO write the AddressKeyMap
    // let _location = persisted_thing
    //     .store_resource(&address_key_map)
    //     .expect("store_resource failed");
}

fn read_store(_address_key_map: BincodeLoadStore<AddressKeyMap>) {
    let mut test_path = env::current_dir()
        .map_err(|e| PersistenceError::StdIoDirOpsError { source: e })
        .expect("Bad cwd");
    test_path.push("testing_tmp");
    // let mut store_loader =
    //     AtomicStoreLoader::create(test_path.as_path(), "append_log_test_empty_iterator")
    //         .expect("AtomicStoreLoader::create failed");

    // let _atomic_store = AtomicStore::open(store_loader).expect("open failed");
    // let mut persisted_thing = AppendLog::create(
    //     &mut store_loader,
    //     <BincodeLoadStore<AddressKeyMap>>::default(),
    //     "append_thing",
    //     1024,
    // )
    // .expect("AppendLog::create failed");

    // TODO read the AddressKeyMap
}
