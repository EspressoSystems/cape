use crate::api_server::WebState;
use crate::route_parsing::*;
use crate::QueryResultState;

use cap_rust_sandbox::model::CapeLedgerState;
use jf_cap::structs::Nullifier;
use net::server::response;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use strum::IntoEnumIterator;
use strum_macros::{AsRefStr, EnumIter, EnumString};

/// Index entries for documentation fragments
#[allow(non_camel_case_types)]
#[derive(AsRefStr, Copy, Clone, Debug, EnumIter, EnumString)]
pub enum ApiRouteKey {
    get_cap_state,
}

/// Verifiy that every variant of enum ApiRouteKey is defined in api.toml
// TODO !corbett Check all the other things that might fail after startup.
pub fn check_api(api: toml::Value) -> bool {
    let mut missing_definition = false;
    for key in ApiRouteKey::iter() {
        let key_str = key.as_ref();
        if api["route"].get(key_str).is_none() {
            println!("Missing API definition for [route.{}]", key_str);
            missing_definition = true;
        }
    }
    if missing_definition {
        panic!("api.toml is inconsistent with enum ApiRouteKey");
    }
    !missing_definition
}

#[allow(dead_code)]
pub fn dummy_url_eval(
    route_pattern: &str,
    bindings: &HashMap<String, RouteBinding>,
) -> Result<tide::Response, tide::Error> {
    let route_str = route_pattern.to_string();
    let title = route_pattern.split_once('/').unwrap_or((&route_str, "")).0;
    Ok(tide::Response::builder(200)
        .body(tide::Body::from_string(format!(
            "<!DOCTYPE html>
<html lang='en'>
  <head>
    <meta charset='utf-8'>
    <title>{}</title>
    <link rel='stylesheet' href='style.css'>
    <script src='script.js'></script>
  </head>
  <body>
    <h1>{}</h1>
    <p>{:?}</p>
  </body>
</html>",
            title, route_str, bindings
        )))
        .content_type(tide::http::mime::HTML)
        .build())
}

////////////////////////////////////////////////////////////////////////////////
// Endpoints
//
// Each endpoint function handles one API endpoint, returning an instance of
// Serialize (or an error). The main entrypoint, dispatch_url, is in charge of
// serializing the endpoint responses according to the requested content type
// and building a Response object.
//

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapState {
    pub ledger: CapeLedgerState,
    pub nullifiers: HashSet<Nullifier>,
    pub num_events: u64,
}

pub async fn get_cap_state(query_result_state: &QueryResultState) -> Result<CapState, tide::Error> {
    Ok(CapState {
        ledger: query_result_state.contract_state.ledger.clone(),
        nullifiers: query_result_state.contract_state.nullifiers.clone(),
        num_events: query_result_state.events.len() as u64,
    })
}

pub async fn dispatch_url(
    req: tide::Request<WebState>,
    route_pattern: &str,
    _bindings: &HashMap<String, RouteBinding>,
) -> Result<tide::Response, tide::Error> {
    let first_segment = route_pattern
        .split_once('/')
        .unwrap_or((route_pattern, ""))
        .0;
    let key = ApiRouteKey::from_str(first_segment).expect("Unknown route");
    let query_state_guard = req.state().query_result_state.read().await;
    let query_state = &*query_state_guard;
    match key {
        ApiRouteKey::get_cap_state => response(&req, get_cap_state(query_state).await?),
    }
}
