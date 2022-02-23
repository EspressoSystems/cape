// Copyright © 2021 Translucence Research, Inc. All rights reserved.

use crate::{api_server::WebState, routes::check_api};
use std::fs::read_to_string;
use std::path::Path;

/// Loads the message catalog or dies trying.
pub fn load_messages(path: &Path) -> toml::Value {
    let messages = read_to_string(&path).unwrap_or_else(|_| panic!("Unable to read {:?}.", &path));
    let api: toml::Value =
        toml::from_str(&messages).unwrap_or_else(|_| panic!("Unable to parse {:?}.", &path));
    check_api(api.clone());
    api
}

/// Compose `api.toml` into HTML.
///
/// This function iterates over the routes, adding headers and HTML class attributes to make
/// a documentation page for the web API.
///
/// The results of this could be precomputed and cached.
pub async fn compose_help(req: tide::Request<WebState>) -> Result<tide::Response, tide::Error> {
    let api = &req.state().api;
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
                        parameter.strip_prefix(':').unwrap(),
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
    Ok(tide::Response::builder(200)
        .content_type(tide::http::mime::HTML)
        .body(help)
        .build())
}
