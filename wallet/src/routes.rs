// Copyright © 2021 Translucence Research, Inc. All rights reserved.

use crate::routes::mime::Mime;
use crate::WebState;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt::Debug;
use std::str::FromStr;
use strum::IntoEnumIterator;
use strum_macros::{AsRefStr, EnumIter, EnumString};
use tagged_base64::TaggedBase64;
use tide::http::{content::Accept, mime};
use tide::StatusCode;
use tide::{Body, Request, Response};
use tide_websockets::WebSocketConnection;

#[derive(Debug, EnumString)]
pub enum UrlSegmentType {
    Boolean,
    Hexadecimal,
    Integer,
    TaggedBase64,
    Literal,
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum UrlSegmentValue {
    Boolean(bool),
    Hexadecimal(u128),
    Integer(u128),
    Identifier(TaggedBase64),
    Unparsed(String),
    ParseFailed(UrlSegmentType, String),
    Literal(String),
}

use UrlSegmentValue::*;

#[allow(dead_code)]
impl UrlSegmentValue {
    pub fn parse(value: &str, ptype: &str) -> Option<Self> {
        Some(match ptype {
            "Boolean" => Boolean(value.parse::<bool>().ok()?),
            "Hexadecimal" => Hexadecimal(u128::from_str_radix(value, 16).ok()?),
            "Integer" => Integer(value.parse::<u128>().ok()?),
            "TaggedBase64" => Identifier(TaggedBase64::parse(value).ok()?),
            _ => panic!("Type specified in api.toml isn't supported: {}", ptype),
        })
    }

    pub fn as_boolean(&self) -> Result<bool, tide::Error> {
        if let Boolean(b) = self {
            Ok(*b)
        } else {
            Err(tide::Error::from_str(
                StatusCode::BadRequest,
                format!("expected boolean, got {:?}", self),
            ))
        }
    }

    pub fn as_index(&self) -> Result<usize, tide::Error> {
        if let Integer(ix) = self {
            Ok(*ix as usize)
        } else {
            Err(tide::Error::from_str(
                StatusCode::BadRequest,
                format!("expected index, got {:?}", self),
            ))
        }
    }

    pub fn as_identifier(&self) -> Result<TaggedBase64, tide::Error> {
        if let Identifier(i) = self {
            Ok(i.clone())
        } else {
            Err(tide::Error::from_str(
                StatusCode::BadRequest,
                format!("expected tagged base 64, got {:?}", self),
            ))
        }
    }
}

#[derive(Debug)]
pub struct RouteBinding {
    /// Placeholder from the route pattern, e.g. :id
    pub parameter: String,

    /// Type for parsing
    pub ptype: UrlSegmentType,

    /// Value
    pub value: UrlSegmentValue,
}

/// Index entries for documentation fragments
#[allow(non_camel_case_types)]
#[derive(AsRefStr, Copy, Clone, Debug, EnumIter, EnumString)]
pub enum ApiRouteKey {
    closewallet,
    deposit,
    freeze,
    getaddress,
    getbalance,
    getinfo,
    importkey,
    mint,
    newasset,
    newkey,
    newwallet,
    openwallet,
    send,
    trace,
    transaction,
    unfreeze,
    unwrap,
    wrap,
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
        panic!("api.toml is inconsistent with enum ApiRoutKey");
    }
    !missing_definition
}

// TODO !corbett Copied from zerok/zerok_lib/src/api.rs. Factor out a crate.

pub fn best_response_type(
    accept: &mut Option<Accept>,
    available: &[Mime],
) -> Result<Mime, tide::Error> {
    match accept {
        Some(accept) => {
            // The Accept type has a `negotiate` method, but it doesn't properly handle
            // wildcards. It handles * but not */* and basetype/*, because for content type
            // proposals like */* and basetype/*, it looks for a literal match in `available`,
            // it does not perform pattern matching. So, we implement negotiation ourselves.
            //
            // First sort by the weight parameter, which the Accept type does do correctly.
            accept.sort();
            // Go through each proposed content type, in the order specified by the client, and
            // match them against our available types, respecting wildcards.
            for proposed in accept.iter() {
                if proposed.basetype() == "*" {
                    // The only acceptable Accept value with a basetype of * is */*, therefore
                    // this will match any available type.
                    return Ok(available[0].clone());
                } else if proposed.subtype() == "*" {
                    // If the subtype is * but the basetype is not, look for a proposed type
                    // with a matching basetype and any subtype.
                    for mime in available {
                        if mime.basetype() == proposed.basetype() {
                            return Ok(mime.clone());
                        }
                    }
                } else if available.contains(proposed) {
                    // If neither part of the proposal is a wildcard, look for a literal match.
                    return Ok((**proposed).clone());
                }
            }

            if accept.wildcard() {
                // If no proposals are available but a wildcard flag * was given, return any
                // available content type.
                Ok(available[0].clone())
            } else {
                Err(tide::Error::from_str(
                    StatusCode::NotAcceptable,
                    "No suitable Content-Type found",
                ))
            }
        }
        None => {
            // If no content type is explicitly requested, default to the first available type.
            Ok(available[0].clone())
        }
    }
}

fn respond_with<T: Serialize>(
    accept: &mut Option<Accept>,
    body: T,
) -> Result<Response, tide::Error> {
    let ty = best_response_type(accept, &[mime::JSON, mime::BYTE_STREAM])?;
    if ty == mime::BYTE_STREAM {
        let bytes = bincode::serialize(&body)?;
        Ok(Response::builder(tide::StatusCode::Ok)
            .body(bytes)
            .content_type(mime::BYTE_STREAM)
            .build())
    } else if ty == mime::JSON {
        Ok(Response::builder(tide::StatusCode::Ok)
            .body(Body::from_json(&body)?)
            .content_type(mime::JSON)
            .build())
    } else {
        unreachable!()
    }
}

/// Serialize the body of a response.
///
/// The Accept header of the request is used to determine the serialization format.
///
/// This function combined with the [add_error_body] middleware defines the server-side protocol
/// for encoding zerok types in HTTP responses.
pub fn response<T: Serialize, S>(req: &Request<S>, body: T) -> Result<Response, tide::Error> {
    respond_with(&mut Accept::from_headers(req)?, body)
}

// End of functions copied from api.rs

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

async fn closewallet(_bindings: &HashMap<String, RouteBinding>) -> Result<(), tide::Error> {
    Ok(())
}

pub async fn dispatch_url(
    req: tide::Request<WebState>,
    route_pattern: &str,
    bindings: &HashMap<String, RouteBinding>,
) -> Result<tide::Response, tide::Error> {
    let first_segment = route_pattern
        .split_once('/')
        .unwrap_or((route_pattern, ""))
        .0;
    let key = ApiRouteKey::from_str(first_segment).expect("Unknown route");
    let query_service_guard = req.state().node.read().await;
    let _query_service = &*query_service_guard;
    match key {
        ApiRouteKey::closewallet => response(&req, closewallet(bindings).await?),
        ApiRouteKey::deposit => dummy_url_eval(route_pattern, bindings),
        ApiRouteKey::freeze => dummy_url_eval(route_pattern, bindings),
        ApiRouteKey::getaddress => dummy_url_eval(route_pattern, bindings),
        ApiRouteKey::getbalance => dummy_url_eval(route_pattern, bindings),
        ApiRouteKey::getinfo => dummy_url_eval(route_pattern, bindings),
        ApiRouteKey::importkey => dummy_url_eval(route_pattern, bindings),
        ApiRouteKey::mint => dummy_url_eval(route_pattern, bindings),
        ApiRouteKey::newasset => dummy_url_eval(route_pattern, bindings),
        ApiRouteKey::newkey => dummy_url_eval(route_pattern, bindings),
        ApiRouteKey::newwallet => dummy_url_eval(route_pattern, bindings),
        ApiRouteKey::openwallet => dummy_url_eval(route_pattern, bindings),
        ApiRouteKey::send => dummy_url_eval(route_pattern, bindings),
        ApiRouteKey::trace => dummy_url_eval(route_pattern, bindings),
        ApiRouteKey::transaction => dummy_url_eval(route_pattern, bindings),
        ApiRouteKey::unfreeze => dummy_url_eval(route_pattern, bindings),
        ApiRouteKey::unwrap => dummy_url_eval(route_pattern, bindings),
        ApiRouteKey::wrap => dummy_url_eval(route_pattern, bindings),
    }
}

pub async fn dispatch_web_socket(
    _req: tide::Request<WebState>,
    _conn: WebSocketConnection,
    route_pattern: &str,
    _bindings: &HashMap<String, RouteBinding>,
) -> Result<(), tide::Error> {
    let first_segment = route_pattern
        .split_once('/')
        .unwrap_or((route_pattern, ""))
        .0;
    let key = ApiRouteKey::from_str(first_segment).expect("Unknown route");
    match key {
        // ApiRouteKey::subscribe => subscribe(req, conn, bindings).await,
        _ => Err(tide::Error::from_str(
            StatusCode::InternalServerError,
            "server called dispatch_web_socket with an unsupported route",
        )),
    }
}
