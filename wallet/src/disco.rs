// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Configurable API loading.

use std::fs::read_to_string;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use strum::IntoEnumIterator;
use strum_macros::{AsRefStr, EnumIter, EnumString};
use tide::http::Method;

/// Loads the message catalog or panics
pub fn load_messages(path: &Path) -> toml::Value {
    let messages = read_to_string(&path).unwrap_or_else(|_| panic!("Unable to read {:?}.", &path));
    let api: toml::Value =
        toml::from_str(&messages).unwrap_or_else(|_| panic!("Unable to parse {:?}.", &path));
    if let Err(err) = check_api(api.clone()) {
        panic!("{}", err);
    }
    api
}

/// Index entries for documentation fragments
#[allow(non_camel_case_types)]
#[derive(AsRefStr, Copy, Clone, Debug, EnumIter, EnumString, strum_macros::Display)]
pub enum ApiRouteKey {
    buildsponsor,
    buildwrap,
    closewallet,
    exportasset,
    freeze,
    getaddress,
    getaccount,
    getaccounts,
    getbalance,
    getinfo,
    getmnemonic,
    importasset,
    healthcheck,
    importkey,
    listkeystores,
    mint,
    newasset,
    newkey,
    newwallet,
    openwallet,
    recordopening,
    recoverkey,
    resetpassword,
    send,
    submitsponsor,
    submitwrap,
    transaction,
    transactionhistory,
    unfreeze,
    unwrap,
    updateasset,
    view,
    getrecords,
    lastusedkeystore,
    getprivatekey,
}

/// Check consistency of `api.toml`
///
/// * Verify that every variant of [ApiRouteKey] is defined
/// * Check that every URL parameter has a valid type
pub fn check_api(api: toml::Value) -> Result<(), String> {
    for key in ApiRouteKey::iter() {
        let route = api["route"]
            .get(key.as_ref())
            .ok_or_else(|| format!("Missing API definition for [route.{}]", key))?;
        if let Some(method) = route.get("METHOD") {
            // If specified, METHOD must be an HTTP method.
            let method = method
                .as_str()
                .ok_or_else(|| format!("Malformed METHOD for [route.{}] (expected string)", key))?;
            Method::from_str(method).map_err(|_| {
                format!(
                    "METHOD {} for [route.{}] is not an HTTP method",
                    method, key
                )
            })?;
        }
        let paths = route["PATH"]
            .as_array()
            .ok_or_else(|| format!("Malformed PATH for [route.{}] (expected array)", key))?;
        for path in paths {
            let path = path.as_str().ok_or_else(|| {
                format!("Malformed pattern for [route.{}] (expected string)", key)
            })?;
            if path.ends_with('/') {
                return Err(format!(
                    "Malformed pattern for [route.{}] (trailing slash)",
                    key
                ));
            }

            for segment in path.split('/') {
                if segment.starts_with(':') {
                    let ty = route
                        .get(segment)
                        .ok_or_else(|| {
                            format!("Missing parameter type for {} in [route.{}]", segment, key)
                        })?
                        .as_str()
                        .ok_or_else(|| {
                            format!(
                                "Malformed parameter type for {} in [route.{}] (expected string)",
                                segment, key
                            )
                        })?;
                    UrlSegmentType::from_str(ty).map_err(|err| {
                        format!("Invalid type for {} in [route.{}] ({})", segment, key, err)
                    })?;
                }
            }
        }
    }
    Ok(())
}

/// Compose `api.toml` into HTML.
///
/// This function iterates over the routes, adding headers and HTML class attributes to make
/// a documentation page for the web API.
///
/// The results of this could be precomputed and cached.
pub fn compose_help(api: &toml::Value) -> String {
    let meta = &api["meta"];
    let mut help = meta["HTML_TOP"]
        .as_str()
        .expect("HTML_TOP must be a string in api.toml")
        .to_owned();
    if let Some(api_map) = api["route"].as_table() {
        api_map.values().for_each(|entry| {
            let paths = entry["PATH"].as_array().expect("Expecting TOML array.");
            let first_path = paths[0].as_str().expect("Expecting TOML string.");
            let first_segment = first_path.split_once('/').unwrap_or((first_path, "")).0;
            help += &format!(
                "<a name='{}'><h3 class='entry'>{}</h3></a>\n<h3>{}</h3>",
                first_segment,
                first_segment,
                &meta["HEADING_ROUTES"]
                    .as_str()
                    .expect("HEADING_ROUTES must be a string in api.toml")
            );
            for path in paths.iter() {
                help += &format!(
                    "<p class='path'>{}</p>\n",
                    path.as_str()
                        .expect("PATH must be an array of strings in api.toml")
                );
            }
            help += &format!(
                "<h3>{}</h3>\n<table>\n",
                &meta["HEADING_PARAMETERS"]
                    .as_str()
                    .expect("HEADING_PARAMETERS must be a string in api.toml")
            );
            let mut has_parameters = false;
            for (parameter, ptype) in entry
                .as_table()
                .expect("Route definitions must be tables in api.toml")
                .iter()
            {
                if parameter.starts_with(':') {
                    has_parameters = true;
                    help += &format!(
                        "<tr><td class='parameter'>{}</td><td class='type'>{}</td></tr>\n",
                        parameter
                            .strip_prefix(':')
                            .expect("Parameters must begin with ':' in api.toml"),
                        ptype
                            .as_str()
                            .expect("Parameter types must be strings in api.toml")
                    );
                }
            }
            if !has_parameters {
                help += "<div class='meta'>None</div>";
            }
            help += &format!(
                "</table>\n<h3>{}</h3>\n{}\n",
                &meta["HEADING_DESCRIPTION"]
                    .as_str()
                    .expect("HEADING_DESCRIPTION must be a string in api.toml"),
                markdown::to_html(
                    entry["DOC"]
                        .as_str()
                        .expect("DOC must be a string in api.toml")
                        .trim()
                )
            )
        });
    }
    help += &format!(
        "{}\n",
        &api["meta"]["HTML_BOTTOM"]
            .as_str()
            .expect("HTML_BOTTOM must be a string in api.toml")
    );
    help
}

/// Returns the default path to the API file.
pub fn default_api_path() -> PathBuf {
    const API_FILE: &str = "api/api.toml";
    let dir = project_path();
    [&dir, Path::new(API_FILE)].iter().collect()
}

/// Returns the default path to the web directory.
pub fn default_web_path() -> PathBuf {
    const ASSET_DIR: &str = "public";
    let dir = project_path();
    [&dir, Path::new(ASSET_DIR)].iter().collect()
}

/// Returns the project directory.
pub fn project_path() -> PathBuf {
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

#[derive(Clone, Copy, Debug, EnumString)]
pub enum UrlSegmentType {
    Boolean,
    Hexadecimal,
    Integer,
    TaggedBase64,
    Base64,
    Literal,
}
