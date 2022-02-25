// Copyright © 2021 Translucence Research, Inc. All rights reserved.

use crate::routes::{
    dispatch_url, dispatch_web_socket, CapeAPIError, RouteBinding, UrlSegmentType, UrlSegmentValue,
    Wallet,
};
use async_std::{
    sync::{Arc, Mutex},
    task::{spawn, JoinHandle},
};
use cap_rust_sandbox::model::EthereumAddr;
use jf_cap::{keys::UserKeyPair, structs::AssetCode};
use net::server;
use rand_chacha::ChaChaRng;
use std::collections::hash_map::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use structopt::StructOpt;
use tide::StatusCode;
use tide_websockets::{WebSocket, WebSocketConnection};

pub const DEFAULT_ETH_ADDR: EthereumAddr = EthereumAddr([2; 20]);
pub const DEFAULT_WRAPPED_AMT: u64 = 1000;
pub const DEFAULT_NATIVE_AMT_IN_FAUCET_ADDR: u64 = 500;
pub const DEFAULT_NATIVE_AMT_IN_WRAPPER_ADDR: u64 = 400;

#[derive(Debug, StructOpt)]
#[structopt(
    name = "Wallet Web API",
    about = "Performs wallet operations in response to web requests"
)]
pub struct NodeOpt {
    /// Path to assets including web server files.
    #[structopt(
        long = "assets",
        default_value = ""      // See fn default_web_path().
    )]
    pub web_path: String,

    /// Path to API specification and messages.
    #[structopt(
        long = "api",
        default_value = ""      // See fn default_api_path().
    )]
    pub api_path: String,
}

/// Returns the project directory.
fn project_path() -> PathBuf {
    let dir = std::env::var("WALLET").unwrap_or_else(|_| {
        println!(
            "WALLET directory is not set. Using the default paths, ./public and ./api for asset \
            and API paths, respectively. To use different paths, set the WALLET environment \
            variable, or specify :assets and :api arguments."
        );
        String::from(".")
    });
    PathBuf::from(dir)
}

/// Returns the default path to the web directory.
pub fn default_web_path() -> PathBuf {
    const ASSET_DIR: &str = "public";
    let dir = project_path();
    [&dir, Path::new(ASSET_DIR)].iter().collect()
}

/// Returns the default path to the API file.
pub fn default_api_path() -> PathBuf {
    const API_FILE: &str = "api/api.toml";
    let dir = project_path();
    [&dir, Path::new(API_FILE)].iter().collect()
}

#[derive(Clone)]
pub struct WebState {
    pub(crate) web_path: PathBuf,
    pub(crate) api: toml::Value,
    pub(crate) wallet: Arc<Mutex<Option<Wallet>>>,
    pub(crate) rng: Arc<Mutex<ChaChaRng>>,
    pub(crate) faucet_key_pair: UserKeyPair,
}

async fn form_demonstration(req: tide::Request<WebState>) -> Result<tide::Body, tide::Error> {
    let mut index_html: PathBuf = req.state().web_path.clone();
    index_html.push("index.html");
    Ok(tide::Body::from_file(index_html).await?)
}

// Get the route pattern that matches the URL of a request, and the bindings for parameters in the
// pattern. If no route matches, the error is a documentation string explaining what went wrong.
fn parse_route(
    req: &tide::Request<WebState>,
) -> Result<(String, HashMap<String, RouteBinding>), String> {
    let first_segment = &req
        .url()
        .path_segments()
        .ok_or_else(|| String::from("No path segments"))?
        .next()
        .ok_or_else(|| String::from("Empty path"))?;
    let api = &req.state().api["route"][first_segment];
    let route_patterns = api["PATH"]
        .as_array()
        .expect("Invalid PATH type. Expecting array.");
    let mut arg_doc: String = api["DOC"].as_str().expect("Missing DOC").to_string();
    let mut matching_route_count = 0u64;
    let mut matching_route = String::new();
    let mut bindings: HashMap<String, HashMap<String, RouteBinding>> = HashMap::new();
    for route_pattern in route_patterns.iter() {
        let mut found_literal_mismatch = false;
        let mut argument_parse_failed = false;
        arg_doc.push_str(&format!(
            "\n\nRoute: {}\n--------------------\n",
            &route_pattern.as_str().unwrap()
        ));
        // The `path_segments()` succeeded above, so `unwrap()` is safe.
        let mut req_segments = req.url().path_segments().unwrap();
        for pat_segment in route_pattern
            .as_str()
            .expect("PATH must be an array of strings")
            .split('/')
        {
            // Each route parameter has an associated type. The lookup
            // will only succeed if the current segment is a parameter
            // placeholder, such as :id. Otherwise, it is assumed to
            // be a literal.
            if let Some(segment_type_value) = &api.get(pat_segment) {
                let segment_type = segment_type_value
                    .as_str()
                    .expect("The path pattern must be a string.");
                let req_segment = req_segments.next().unwrap_or("");
                arg_doc.push_str(&format!(
                    "  Argument: {} as type {} and value: {} ",
                    pat_segment, segment_type, req_segment
                ));
                let ptype =
                    UrlSegmentType::from_str(segment_type).map_err(|err| err.to_string())?;
                if let Some(value) = UrlSegmentValue::parse(ptype, req_segment) {
                    let rb = RouteBinding {
                        parameter: pat_segment.to_string(),
                        ptype,
                        value,
                    };
                    bindings
                        .entry(String::from(route_pattern.as_str().unwrap()))
                        .or_default()
                        .insert(pat_segment.to_string(), rb);
                    arg_doc.push_str("(Parse succeeded)\n");
                } else {
                    arg_doc.push_str("(Parse failed)\n");
                    argument_parse_failed = true;
                    // TODO !corbett capture parse failures documentation
                    // UrlSegmentValue::ParseFailed(segment_type, req_segment)
                }
            } else {
                // No type information. Assume pat_segment is a literal.
                let req_segment = req_segments.next().unwrap_or("");
                if req_segment != pat_segment {
                    found_literal_mismatch = true;
                    arg_doc.push_str(&format!(
                        "Request segment {} does not match route segment {}.\n",
                        req_segment, pat_segment
                    ));
                }
                // TODO !corbett else capture the matching literal in bindings
                // TODO !corebtt if the edit distance is small, capture spelling suggestion
            }
        }
        if !found_literal_mismatch {
            arg_doc.push_str(&format!(
                "Literals match for {}\n",
                &route_pattern.as_str().unwrap(),
            ));
        }
        let mut length_matches = false;
        if req_segments.next().is_none() {
            arg_doc.push_str(&format!(
                "Length match for {}\n",
                &route_pattern.as_str().unwrap(),
            ));
            length_matches = true;
        }
        if argument_parse_failed {
            arg_doc.push_str(&"Argument parsing failed.\n".to_string());
        } else {
            arg_doc.push_str(&"No argument parsing errors!\n".to_string());
        }
        if !argument_parse_failed && length_matches && !found_literal_mismatch {
            let route_pattern_str = route_pattern.as_str().unwrap();
            arg_doc.push_str(&format!("Route matches request: {}\n", &route_pattern_str));
            matching_route_count += 1;
            matching_route = String::from(route_pattern_str);
        } else {
            arg_doc.push_str("Route does not match request.\n");
        }
    }
    match matching_route_count {
        0 => {
            arg_doc.push_str("\nNeed documentation");
            Err(arg_doc)
        }
        1 => {
            let route_bindings = bindings.remove(&matching_route).unwrap_or_default();
            Ok((matching_route, route_bindings))
        }
        _ => {
            arg_doc.push_str("\nAmbiguity in api.toml");
            Err(arg_doc)
        }
    }
}

/// Handle API requests defined in api.toml.
///
/// This function duplicates the logic for deciding which route was requested. This
/// is an unfortunate side-effect of defining the routes in an external file.
// todo !corbett Convert the error feedback into HTML
async fn entry_page(req: tide::Request<WebState>) -> Result<tide::Response, tide::Error> {
    match parse_route(&req) {
        Ok((pattern, bindings)) => dispatch_url(req, pattern.as_str(), &bindings).await,
        Err(arg_doc) => Ok(tide::Response::builder(200).body(arg_doc).build()),
    }
}

#[cfg(any(test, feature = "testing"))]
async fn populatefortest(req: tide::Request<WebState>) -> Result<tide::Response, tide::Error> {
    use crate::{
        routes::{require_wallet, wallet_error},
        wallet::CapeWalletExt,
    };
    use cap_rust_sandbox::model::Erc20Code;

    let wallet = &mut *req.state().wallet.lock().await;
    let wallet = require_wallet(wallet)?;

    // Generate two of each kind of key, to simulate multiple accounts.
    for _ in 0..2 {
        wallet.generate_user_key(None).await.map_err(wallet_error)?;
        wallet.generate_audit_key().await.map_err(wallet_error)?;
        wallet.generate_freeze_key().await.map_err(wallet_error)?;
    }

    // Add the faucet key, so we get a balance of the native asset.
    wallet
        .add_user_key(req.state().faucet_key_pair.clone(), Default::default())
        .await
        .unwrap();
    wallet
        .await_key_scan(&req.state().faucet_key_pair.address())
        .await
        .unwrap();

    // Add a wrapped asset, and give it some nonzero balance.
    let erc20_code = Erc20Code(EthereumAddr([1; 20]));
    let sponsor_addr = DEFAULT_ETH_ADDR;
    let asset_def = wallet
        .sponsor(erc20_code, sponsor_addr.clone(), Default::default())
        .await
        .map_err(wallet_error)?;
    let wrapped_asset_addr = wallet.pub_keys().await[0].address();
    wallet
        .wrap(
            sponsor_addr,
            asset_def,
            wrapped_asset_addr.clone(),
            DEFAULT_WRAPPED_AMT,
        )
        .await
        .map_err(wallet_error)?;

    // Transfer some native asset from the faucet address to the address with
    // the wrapped asset, so that it can be used for the unwrapping fee.
    // The transfer also finalizes the wrap.
    wallet
        .transfer(
            &req.state().faucet_key_pair.address(),
            &AssetCode::native(),
            &[(wrapped_asset_addr, DEFAULT_NATIVE_AMT_IN_WRAPPER_ADDR)],
            1000 - DEFAULT_NATIVE_AMT_IN_FAUCET_ADDR - DEFAULT_NATIVE_AMT_IN_WRAPPER_ADDR,
        )
        .await
        .map_err(wallet_error)?;

    server::response(&req, ())
}

async fn handle_web_socket(
    req: tide::Request<WebState>,
    wsc: WebSocketConnection,
) -> tide::Result<()> {
    match parse_route(&req) {
        Ok((pattern, bindings)) => dispatch_web_socket(req, wsc, pattern.as_str(), &bindings).await,
        Err(arg_doc) => Err(tide::Error::from_str(StatusCode::BadRequest, arg_doc)),
    }
}

// This route is a demonstration of a form with a WebSocket connection
// for asynchronous updates. Once we have useful forms, this can go...
fn add_form_demonstration(web_server: &mut tide::Server<WebState>) {
    web_server
        .at("/transfer/:id/:recipient/:amount")
        .with(WebSocket::new(handle_web_socket))
        .get(form_demonstration);
}

pub fn init_server(
    mut rng: ChaChaRng,
    api_path: PathBuf,
    web_path: PathBuf,
    port: u64,
) -> std::io::Result<JoinHandle<std::io::Result<()>>> {
    let api = crate::disco::load_messages(&api_path);
    let faucet_key_pair = UserKeyPair::generate(&mut rng);
    let mut web_server = tide::with_state(WebState {
        web_path: web_path.clone(),
        api: api.clone(),
        wallet: Arc::new(Mutex::new(None)),
        rng: Arc::new(Mutex::new(rng)),
        faucet_key_pair,
    });
    web_server
        .with(server::trace)
        .with(server::add_error_body::<_, CapeAPIError>);

    // Define the routes handled by the web server.
    web_server.at("/public").serve_dir(web_path)?;
    web_server.at("/").get(crate::disco::compose_help);

    add_form_demonstration(&mut web_server);

    // Add routes from a configuration file.
    if let Some(api_map) = api["route"].as_table() {
        api_map.values().for_each(|v| {
            let web_socket = v
                .get("WEB_SOCKET")
                .map(|v| v.as_bool().expect("expected boolean value for WEB_SOCKET"))
                .unwrap_or(false);
            let routes = match &v["PATH"] {
                toml::Value::String(s) => {
                    vec![s.clone()]
                }
                toml::Value::Array(a) => a
                    .iter()
                    .filter_map(|v| {
                        if let Some(s) = v.as_str() {
                            Some(String::from(s))
                        } else {
                            println!("Oops! Array element: {:?}", v);
                            None
                        }
                    })
                    .collect(),
                _ => panic!("Expecting a toml::String or toml::Array, but got: {:?}", &v),
            };
            for path in routes {
                let mut route = web_server.at(&path);
                if web_socket {
                    route.get(WebSocket::new(handle_web_socket));
                } else {
                    route.get(entry_page);
                }
            }
        });
    }

    #[cfg(any(test, feature = "testing"))]
    web_server.at("populatefortest").get(populatefortest);

    let addr = format!("0.0.0.0:{}", port);
    Ok(spawn(web_server.listen(addr)))
}
