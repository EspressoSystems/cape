// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use cape_wallet::disco::{compose_help, default_api_path, default_web_path, load_messages};
use std::fs;
use std::path::PathBuf;
use structopt::StructOpt;

/// Export documentation for the CAPE wallet API.
#[derive(StructOpt)]
struct Options {
    /// Path to API specification and messages.
    #[structopt(long = "api")]
    api_path: Option<PathBuf>,

    /// Path to assets including web server files.
    #[structopt(long = "assets")]
    pub web_path: Option<PathBuf>,

    /// Directory to create with API documentation.
    #[structopt(name = "OUT")]
    dir: PathBuf,
}

fn main() -> std::io::Result<()> {
    let options = Options::from_args();
    let api = load_messages(&options.api_path.unwrap_or_else(default_api_path));
    let help = compose_help(&api);

    fs::create_dir_all(&options.dir.join("public/css"))?;
    fs::write(options.dir.join("index.html"), help.as_bytes())?;
    fs::copy(
        options
            .web_path
            .unwrap_or_else(default_web_path)
            .join("css/style.css"),
        options.dir.join("public/css/style.css"),
    )?;

    Ok(())
}
