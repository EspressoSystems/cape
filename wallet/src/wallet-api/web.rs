// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

//! # Wallet server library
//!
//! This module provides functions and types needed to run the wallet web server. It includes
//! configuration options, request parsing, and the main web server entrypoint. The implementation
//! of the actual routes is defined in [crate::routes].

use crate::routes::{
    dispatch_url, CapeAPIError, RouteBinding, UrlSegmentType, UrlSegmentValue, Wallet,
};
use async_std::{
    sync::{Arc, Mutex},
    task::{spawn, JoinHandle},
};
use cap_rust_sandbox::model::EthereumAddr;
use ethers::prelude::{Address, H160};
use jf_cap::{keys::UserKeyPair, structs::AssetCode};
use net::server;
use rand_chacha::ChaChaRng;
use std::collections::hash_map::HashMap;
use std::fs::create_dir_all;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;
use structopt::StructOpt;
use tagged_base64::TaggedBase64;
use tide::{
    http::{headers::HeaderValue, Method, Url},
    security::{CorsMiddleware, Origin},
};

pub const DEFAULT_ETH_ADDR: Address = H160([2; 20]);
pub const DEFAULT_WRAPPED_AMT: u64 = 1000;
pub const DEFAULT_NATIVE_AMT_IN_FAUCET_ADDR: u64 = 500;
pub const DEFAULT_NATIVE_AMT_IN_WRAPPER_ADDR: u64 = 400;

/// Server configuration with command line parsing support.
#[derive(Clone, Debug, StructOpt)]
#[structopt(
    name = "Wallet Web API",
    about = "Performs wallet operations in response to web requests"
)]
pub struct NodeOpt {
    /// Path to assets including web server files.
    #[structopt(long = "assets")]
    pub web_path: Option<PathBuf>,

    /// Path to API specification and messages.
    #[structopt(long = "api")]
    pub api_path: Option<PathBuf>,

    /// Path to store keystores and location of most recent wallet
    #[structopt(
        long,
        env = "CAPE_WALLET_STORAGE", // Fallback to env_var or $HOME
    )]
    pub storage: Option<PathBuf>,

    /// Port to host the server on.
    #[structopt(short, long, env = "CAPE_WALLET_PORT", default_value = "60000")]
    pub port: u16,

    /// URL for the Ethereum Query Service.
    #[structopt(long, env = "CAPE_EQS_URL", default_value = "http://localhost:50087")]
    pub eqs_url: Url,

    /// URL for the CAPE relayer.
    #[structopt(
        long,
        env = "CAPE_RELAYER_URL",
        default_value = "http://localhost:50077"
    )]
    pub relayer_url: Url,

    /// URL for the Ethereum Query Service.
    #[structopt(
        long,
        env = "CAPE_ADDRESS_BOOK_URL",
        default_value = "http://localhost:50078"
    )]
    pub address_book_url: Url,

    /// Address of the CAPE smart contract.
    #[structopt(long, env = "CAPE_CONTRACT_ADDRESS", requires = "rpc_url")]
    pub contract_address: Option<Address>,

    /// URL for Ethers HTTP Provider
    #[structopt(long, env = "CAPE_WEB3_PROVIDER_URL", requires = "contract_address")]
    pub rpc_url: Option<Url>,

    /// Mnemonic for a local Ethereum wallet for direct contract calls.
    #[structopt(long, env = "ETH_MNEMONIC")]
    pub eth_mnemonic: Option<String>,

    /// Minimum amount of time to wait between polling requests to EQS.
    #[structopt(long, env = "CAPE_WALLET_MIN_POLLING_DELAY", default_value = "500")]
    pub min_polling_delay_ms: u64,
}

impl Default for NodeOpt {
    fn default() -> Self {
        Self {
            web_path: None,
            api_path: None,
            storage: None,
            port: 60000,
            eqs_url: "http://localhost:50087"
                .parse()
                .expect("Default eqs url couldn't be parsed"),
            relayer_url: "http://localhost:50077"
                .parse()
                .expect("Default relayer url couldn't be parsed"),
            address_book_url: "http://localhost:50078"
                .parse()
                .expect("Default address book url couldn't be parsed"),
            contract_address: None,
            rpc_url: None,
            eth_mnemonic: None,
            min_polling_delay_ms: 500,
        }
    }
}

// Some of these functions are only used in non-test configs. Rather than annotate each such
// function with #[cfg(not(test))], it's easier just to allow them to be unused.
#[allow(dead_code)]
impl NodeOpt {
    #[cfg(any(test, feature = "testing"))]
    pub fn for_test(port: u16, tmp_storage: PathBuf) -> Self {
        Self {
            port,
            storage: Some(tmp_storage),
            ..Default::default()
        }
    }

    pub fn web_path(&self) -> PathBuf {
        self.web_path
            .as_ref()
            .map(|path| path.to_path_buf())
            .unwrap_or_else(default_web_path)
    }

    pub fn api_path(&self) -> PathBuf {
        self.api_path
            .as_ref()
            .map(|path| path.to_path_buf())
            .unwrap_or_else(default_api_path)
    }

    pub fn storage(&self) -> PathBuf {
        self.storage
            .as_ref()
            .map(|path| path.to_path_buf())
            .unwrap_or_else(default_storage_path)
    }

    pub fn last_used_path(&self) -> PathBuf {
        [&self.storage(), Path::new("last_wallet_path")]
            .iter()
            .collect()
    }

    pub fn keystores_dir(&self) -> PathBuf {
        [&self.storage(), Path::new("keystores")].iter().collect()
    }

    pub fn keystore_path(&self, name: &str) -> PathBuf {
        // base64-encode the name to remove characters with special meaning in the file system, like
        // slashes.
        let enc =
            TaggedBase64::new("KEYSTORE", name.as_bytes()).expect("failed encoding Keystore path");
        [self.keystores_dir().as_path(), Path::new(&enc.to_string())]
            .iter()
            .collect()
    }

    pub fn assets_path(&self) -> PathBuf {
        [&self.storage(), Path::new("verified_assets")]
            .iter()
            .collect()
    }

    pub fn cape_contract(&self) -> Option<(Url, Address)> {
        match (self.rpc_url.clone(), self.contract_address) {
            (Some(url), Some(address)) => Some((url, address)),
            _ => None,
        }
    }

    pub fn eqs_url(&self) -> Url {
        self.eqs_url.clone()
    }

    pub fn relayer_url(&self) -> Url {
        self.relayer_url.clone()
    }

    pub fn address_book_url(&self) -> Url {
        self.address_book_url.clone()
    }

    pub fn eth_mnemonic(&self) -> Option<String> {
        self.eth_mnemonic.clone()
    }

    pub fn min_polling_delay(&self) -> Duration {
        Duration::from_millis(self.min_polling_delay_ms)
    }
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
fn default_web_path() -> PathBuf {
    const ASSET_DIR: &str = "public";
    let dir = project_path();
    [&dir, Path::new(ASSET_DIR)].iter().collect()
}

/// Returns the default path to the API file.
fn default_api_path() -> PathBuf {
    const API_FILE: &str = "api/api.toml";
    let dir = project_path();
    [&dir, Path::new(API_FILE)].iter().collect()
}

/// Returns the default path to store generated files.
fn default_storage_path() -> PathBuf {
    let home = std::env::var("HOME")
        .expect("HOME directory is not set. Please set the server's HOME directory.");
    [&home, ".espresso/cape/wallet"].iter().collect()
}

/// State maintained by the server, used to handle requests.
#[derive(Clone)]
pub struct WebState {
    pub(crate) api: toml::Value,
    pub(crate) wallet: Arc<Mutex<Option<Wallet>>>,
    pub(crate) rng: Arc<Mutex<ChaChaRng>>,
    pub(crate) faucet_key_pair: UserKeyPair,
    pub(crate) options: NodeOpt,
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
            &route_pattern
                .as_str()
                .expect("PATH must be an array of strings")
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
                        .entry(String::from(
                            route_pattern
                                .as_str()
                                .expect("PATH must be an array of strings"),
                        ))
                        .or_default()
                        .insert(pat_segment.to_string(), rb);
                    arg_doc.push_str("(Parse succeeded)\n");
                } else {
                    arg_doc.push_str("(Parse failed)\n");
                    argument_parse_failed = true;
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
            }
        }
        if !found_literal_mismatch {
            arg_doc.push_str(&format!(
                "Literals match for {}\n",
                &route_pattern
                    .as_str()
                    .expect("PATH must be an array of strings"),
            ));
        }
        let mut length_matches = false;
        if req_segments.next().is_none() {
            arg_doc.push_str(&format!(
                "Length match for {}\n",
                &route_pattern
                    .as_str()
                    .expect("PATH must be an array of strings"),
            ));
            length_matches = true;
        }
        if argument_parse_failed {
            arg_doc.push_str("Argument parsing failed.\n");
        } else {
            arg_doc.push_str("No argument parsing errors!\n");
        }
        if !argument_parse_failed && length_matches && !found_literal_mismatch {
            let route_pattern_str = route_pattern
                .as_str()
                .expect("PATH must be an array of strings");
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
async fn entry_page(req: tide::Request<WebState>) -> Result<tide::Response, tide::Error> {
    match parse_route(&req) {
        Ok((pattern, bindings)) => dispatch_url(req, pattern.as_str(), &bindings).await,
        Err(arg_doc) => Ok(tide::Response::builder(200).body(arg_doc).build()),
    }
}

/// Testing route handler which populates a wallet with dummy data.
///
/// This route will modify the wallet by generating 2 of each kind of key (viewing, freezing, and
/// sending), adding the faucet key to the wallet so that the wallet owns a large amount of CAPE fee
/// tokens, transfer some of the fee tokens to another one of its addresses, and sponsor and wrap an
/// ERC-20 asset for that same address.
#[cfg(any(test, feature = "testing"))]
async fn populatefortest(req: tide::Request<WebState>) -> Result<tide::Response, tide::Error> {
    use crate::routes::{require_wallet, wallet_error};
    use cap_rust_sandbox::model::Erc20Code;
    use cape_wallet::{testing::retry, wallet::CapeWalletExt};
    use rand::{RngCore, SeedableRng};
    use seahorse::testing::await_transaction;

    let wallet = &mut *req.state().wallet.lock().await;
    let wallet = require_wallet(wallet)?;

    // Generate two of each kind of key, to simulate multiple accounts.
    for i in 0..2 {
        wallet
            .generate_user_key(format!("test sending account {}", i), None)
            .await
            .map_err(wallet_error)?;
        wallet
            .generate_audit_key(format!("test viewing account {}", i))
            .await
            .map_err(wallet_error)?;
        wallet
            .generate_freeze_key(format!("test freezing account {}", i))
            .await
            .map_err(wallet_error)?;
    }

    // Add the faucet key, so we get a balance of the native asset.
    // Check before adding it to avoid the race condition.
    let faucet_key_pair = req.state().faucet_key_pair.clone();
    if !wallet.pub_keys().await.contains(&faucet_key_pair.pub_key()) {
        wallet
            .add_user_key(
                faucet_key_pair.clone(),
                "faucet account".into(),
                Default::default(),
            )
            .await
            .unwrap();
    }
    let faucet_addr = faucet_key_pair.address();
    wallet.await_key_scan(&faucet_addr).await.unwrap();

    // Add a wrapped asset, and give it some nonzero balance.
    // Sample the Ethereum address from entropy to avoid ERC 20 code collision.
    let mut rng = ChaChaRng::from_entropy();
    let mut random_addr = [0u8; 20];
    rng.fill_bytes(&mut random_addr);
    let erc20_code = Erc20Code(EthereumAddr(random_addr));
    let sponsor_addr: EthereumAddr = DEFAULT_ETH_ADDR.into();
    let asset_def = wallet
        .sponsor(
            "dummy_wrapped_asset".into(),
            erc20_code.clone(),
            sponsor_addr.clone(),
            Default::default(),
        )
        .await
        .map_err(wallet_error)?;

    // Ensure this address is different from the faucet address.
    let mut wrapped_asset_addr = wallet.pub_keys().await[0].address();
    if wrapped_asset_addr == req.state().faucet_key_pair.address() {
        wrapped_asset_addr = wallet.pub_keys().await[1].address();
    }
    wallet
        .wrap(
            sponsor_addr,
            asset_def.clone(),
            wrapped_asset_addr.clone(),
            DEFAULT_WRAPPED_AMT,
        )
        .await
        .map_err(wallet_error)?;

    // Transfer some native asset from the faucet address to the address with
    // the wrapped asset, so that it can be used for the unwrapping fee.
    // The transfer also finalizes the wrap.
    let receipt = wallet
        .transfer(
            Some(&faucet_addr),
            &AssetCode::native(),
            &[(
                wrapped_asset_addr.clone(),
                DEFAULT_NATIVE_AMT_IN_WRAPPER_ADDR,
            )],
            1000 - DEFAULT_NATIVE_AMT_IN_FAUCET_ADDR - DEFAULT_NATIVE_AMT_IN_WRAPPER_ADDR,
        )
        .await
        .map_err(wallet_error)?;

    // Wait for transactions to complete.
    await_transaction(&receipt, wallet, &[]).await;
    retry(|| async {
        wallet
            .balance_breakdown(&wrapped_asset_addr, &AssetCode::native())
            .await
            != 0u64.into()
    })
    .await;
    retry(|| async {
        wallet
            .balance_breakdown(&wrapped_asset_addr, &asset_def.code)
            .await
            != 0u64.into()
    })
    .await;

    server::response(&req, receipt)
}

/// Start the CAPE wallet server.
///
/// The server runs on `localhost` at the specified port. A new task is spawned to run the server,
/// and a handle to the task is returned. Waiting on the handle will join the task; dropping the
/// handle will detach the task.
///
/// Note that there is currently no way to stop the server task once started, other than killing the
/// entire process. This is a limitation of the Tide server framework.
pub fn init_server(
    mut rng: ChaChaRng,
    options: &NodeOpt,
) -> std::io::Result<JoinHandle<std::io::Result<()>>> {
    // Make sure relevant sub-directories of `storage` exist.
    create_dir_all(options.keystores_dir())?;

    let api = crate::disco::load_messages(&options.api_path());
    let faucet_key_pair = UserKeyPair::generate(&mut rng);
    let mut web_server = tide::with_state(WebState {
        api: api.clone(),
        wallet: Arc::new(Mutex::new(None)),
        rng: Arc::new(Mutex::new(rng)),
        faucet_key_pair,
        options: options.clone(),
    });
    web_server
        .with(
            CorsMiddleware::new()
                .allow_methods("GET, POST".parse::<HeaderValue>().unwrap())
                .allow_headers("*".parse::<HeaderValue>().unwrap())
                .allow_origin(Origin::from("*"))
                .allow_credentials(true),
        )
        .with(server::trace)
        .with(server::add_error_body::<_, CapeAPIError>);

    // Define the routes handled by the web server.
    web_server.at("/public").serve_dir(options.web_path())?;
    web_server.at("/").get(crate::disco::compose_help);

    // Add routes from a configuration file.
    if let Some(api_map) = api["route"].as_table() {
        api_map.values().for_each(|v| {
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
            let method = match v.get("METHOD") {
                Some(m) => m
                    .as_str()
                    .expect("PATH must be string")
                    .parse()
                    .expect("METHOD must be GET or POST"),
                None => Method::Get,
            };
            for path in routes {
                web_server.at(&path).method(method, entry_page);
            }
        });
    }

    #[cfg(any(test, feature = "testing"))]
    web_server.at("populatefortest").get(populatefortest);

    let addr = format!("0.0.0.0:{}", options.port);
    Ok(spawn(web_server.listen(addr)))
}
