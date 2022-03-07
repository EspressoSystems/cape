// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use address_book::init_web_server;
use tide::log::LevelFilter;

/// Run a web server that provides a key/value store mapping user
/// addresses to public keys.
#[async_std::main]
async fn main() -> Result<(), std::io::Error> {
    init_web_server(LevelFilter::Info)
        .await
        .unwrap_or_else(|err| {
            panic!("Web server exited with an error: {}", err);
        });
    Ok(())
}
