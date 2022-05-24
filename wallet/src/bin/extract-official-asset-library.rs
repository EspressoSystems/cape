// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use cape_wallet::ui::WalletSummary;
use jf_cap::KeyPair;
use rand_chacha::{rand_core::SeedableRng, ChaChaRng};
use seahorse::asset_library::VerifiedAssetLibrary;
use std::collections::HashMap;
use std::fs;
use std::io::{self, stdout, ErrorKind, Write};
use std::path::PathBuf;
use structopt::StructOpt;

/// Extract an official asset library from a CAPE wallet.
///
/// To extract an official asset library for the Goerli deployment, start the Goerli client (e.g.
/// by running `docker compose up` in a `cape-ui` checkout), set up or log in to your wallet, and
/// create the assets you want to include in the library, including metadata like descriptions and
/// icons. Then run
///
///     extract-official-asset-library -k $SIGNING_KEY_PAIR SYM1 SYM2 ...
///
/// where `SYM1`, `SYM2`, etc. denote the symbols of the assets to include in the library (in case
/// the wallet used to create the official assets also has other assets in its library).
///
/// To extract the library for any other deployment (including possibly a local one) the process is
/// very much the same, only you will change how you launch the wallet GUI according to the
/// deployment, and you may have to use the `--port` argument to `extract-official-asset-library`
/// accordingly.
#[derive(StructOpt)]
struct Options {
    /// The port where the wallet API is being served.
    #[structopt(short, long, default_value = "60000", name = "PORT")]
    port: u16,

    /// The signing key pair to use to authenticate the generated asset library.
    ///
    /// If not provided, the library will be signed with a random key pair, and the generated key
    /// pair will be printed out on stderr.
    #[structopt(short, long, name = "KEY")]
    key_pair: Option<KeyPair>,

    /// The path of the library file to generate.
    ///
    /// If not provided, the library will be written to stdout.
    #[structopt(short = "o", long = "output", name = "FILE")]
    file: Option<PathBuf>,

    /// The symbols of assets to include in the library.
    #[structopt(name = "SYM")]
    symbols: Vec<String>,
}

#[async_std::main]
async fn main() -> io::Result<()> {
    let options = Options::from_args();
    let mut res = surf::get(&format!("http://localhost:{}/getinfo", options.port))
        .send()
        .await
        .map_err(|err| io::Error::new(ErrorKind::Other, err.to_string()))?;
    let info: WalletSummary = res
        .body_json()
        .await
        .map_err(|err| io::Error::new(ErrorKind::Other, err.to_string()))?;

    let assets_by_symbol = info
        .assets
        .into_iter()
        .filter_map(|asset| {
            if let Some(sym) = &asset.symbol {
                Some((sym.clone(), asset.into()))
            } else {
                None
            }
        })
        .collect::<HashMap<_, _>>();
    let assets = options
        .symbols
        .into_iter()
        .map(|sym| {
            assets_by_symbol
                .get(&sym)
                .cloned()
                .ok_or_else(|| io::Error::new(ErrorKind::Other, format!("no such asset {}", sym)))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let key_pair = options.key_pair.unwrap_or_else(|| {
        let key_pair = KeyPair::generate(&mut ChaChaRng::from_entropy());
        eprintln!(
            "Signing asset library with private key: {}, public key: {}",
            key_pair,
            key_pair.ver_key()
        );
        key_pair
    });
    let library = VerifiedAssetLibrary::new(assets, &key_pair);

    let bytes = bincode::serialize(&library)
        .map_err(|err| io::Error::new(ErrorKind::Other, err.to_string()))?;
    if let Some(file) = options.file {
        fs::write(&file, &bytes)?;
    } else {
        stdout().write_all(&bytes)?;
    }

    Ok(())
}
