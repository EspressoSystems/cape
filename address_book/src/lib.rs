// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

#[warn(unused_imports)]
use async_std::{
    sync::{Arc, RwLock},
    task::{sleep, spawn, JoinHandle},
};
use jf_cap::keys::{UserAddress, UserPubKey};
use jf_cap::Signature;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::time::Duration;
use tide::{log::LevelFilter, prelude::*, StatusCode};

pub const DEFAULT_PORT: u16 = 50078u16;
const ADDRESS_BOOK_STARTUP_RETRIES: usize = 8;

pub static mut LOG_LEVEL: LevelFilter = LevelFilter::Info;

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

#[derive(Clone, Default)]
struct ServerState {
    map: Arc<RwLock<HashMap<UserAddress, UserPubKey>>>,
}

pub fn address_book_port() -> String {
    std::env::var("PORT").unwrap_or_else(|_| DEFAULT_PORT.to_string())
}

pub async fn init_web_server(
    log_level: LevelFilter,
) -> std::io::Result<JoinHandle<std::io::Result<()>>> {
    // Accessing `LOG_LEVEL` is considered unsafe since it is a static mutable
    // variable, but we need this to ensure that only one logger is running.
    unsafe {
        LOG_LEVEL = log_level;
    }
    Lazy::force(&LOGGING);
    let mut app = tide::with_state(ServerState::default());
    app.at("/insert_pubkey").post(insert_pubkey);
    app.at("/request_pubkey").post(request_pubkey);
    let address = format!("0.0.0.0:{}", address_book_port());
    Ok(spawn(app.listen(address)))
}

pub async fn wait_for_server() {
    // Wait for the server to come up and start serving.
    let mut backoff = Duration::from_millis(100);
    for _ in 0..ADDRESS_BOOK_STARTUP_RETRIES {
        if surf::connect(format!("http://localhost:{}", address_book_port()))
            .send()
            .await
            .is_ok()
        {
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
