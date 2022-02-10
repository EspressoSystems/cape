use async_std::sync::{Arc, RwLock};
use jf_aap::keys::{UserAddress, UserPubKey};
use jf_aap::Signature;
use std::collections::HashMap;
use tide::{prelude::*, StatusCode};

#[derive(Debug, Deserialize)]
struct InsertPubKey {
    pub_key_bytes: Vec<u8>,
    sig: Signature,
}

const DEFAULT_MAP_PORT: u16 = 50078u16;

#[derive(Clone, Default)]
struct ServerState {
    map: Arc<RwLock<HashMap<UserAddress, UserPubKey>>>,
}

/// Run a web server that provides a key/value store mapping user
/// addresses to public keys.
#[async_std::main]
async fn main() -> Result<(), std::io::Error> {
    tide::log::start();
    let mut app = tide::with_state(ServerState::default());
    app.at("/insert_pubkey").post(insert_pubkey);
    app.at("/request_pubkey").post(request_pubkey);
    let port = std::env::var("PORT").unwrap_or_else(|_| DEFAULT_MAP_PORT.to_string());
    let address = format!("0.0.0.0:{}", port);
    // TODO !corbett signal SIGUSR1 indicating we're just about to start.
    app.listen(address).await?;
    Ok(())
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

async fn request_pubkey(
    mut req: tide::Request<ServerState>,
) -> Result<tide::Response, tide::Error> {
    let address: UserAddress = net::server::request_body(&mut req).await?;
    let hash_map = req.state().map.write().await;
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
