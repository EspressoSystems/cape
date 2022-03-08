// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use address_book::init_web_server;
use address_book::signal::handle_signals;
use signal_hook::consts::SIGINT;
use signal_hook::consts::SIGTERM;
use signal_hook_async_std::Signals;
use tide::log::LevelFilter;

/// Run a web server that provides a key/value store mapping user
/// addresses to public keys.
#[async_std::main]
async fn main() -> Result<(), std::io::Error> {
    let signals = Signals::new(&[SIGINT, SIGTERM]).unwrap();
    let handle = signals.handle();
    let signals_task = async_std::task::spawn(handle_signals(signals));

    init_web_server(LevelFilter::Info)
        .await
        .unwrap_or_else(|err| {
            panic!("Web server exited with an error: {}", err);
        })
        .await?;

    handle.close();
    signals_task.await;

    Ok(())
}
