// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use cape_wallet::mocks::{MockCapeWalletLoader, ReplayBackend};
use seahorse::{
    events::EventIndex,
    hd::{KeyTree, Mnemonic},
    Wallet,
};
use std::fs;
use std::path::PathBuf;
use structopt::StructOpt;
use tempdir::TempDir;

#[derive(StructOpt)]
struct Options {
    #[structopt(long, env = "CAPE_REPLAY_MNEMONIC")]
    mnemonic: Mnemonic,
    events: PathBuf,
}

#[async_std::main]
async fn main() {
    let opt = Options::from_args();
    let storage = TempDir::new("cape-wallet-replay").unwrap();
    let mut loader = MockCapeWalletLoader {
        key: KeyTree::from_mnemonic(&opt.mnemonic),
        path: storage.path().to_path_buf(),
    };
    let events_bytes = fs::read(opt.events).unwrap();
    let events = serde_json::from_slice(&events_bytes).unwrap();
    let backend = ReplayBackend::new(events, &mut loader);
    let mut wallet = Wallet::new(backend).await.unwrap();

    let key = wallet
        .generate_user_key("key".into(), Some(EventIndex::default()))
        .await
        .unwrap();
    wallet.await_key_scan(&key.address()).await.unwrap();
}
