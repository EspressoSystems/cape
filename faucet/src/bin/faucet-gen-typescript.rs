// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

//! A script to export typescript code with the faucet manager address and
//! encryption key. This code is necessary to deploy the contract with hardhat.
use cap_rust_sandbox::{cape::faucet::FAUCET_MANAGER_ENCRYPTION_KEY, types::EdOnBN254Point};
use faucet::faucet_wallet_test_setup::u256_to_hex;
use jf_cap::keys::UserPubKey;
use std::str::FromStr;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(name = "Faucet setup utility")]
struct Options {
    #[structopt(long, default_value = FAUCET_MANAGER_ENCRYPTION_KEY)]
    pub_key: String,
}

fn main() {
    let opt = Options::from_args();

    // output the typescript code for deployment script
    let pub_key = UserPubKey::from_str(&opt.pub_key).unwrap_or_default();
    let enc_key_bytes: [u8; 32] = pub_key.enc_key().into();
    let address: EdOnBN254Point = pub_key.address().into();

    println!(
        r#"
// Derived from {}
let faucetManagerEncKey = "0x{}";
let faucetManagerAddress = {{
  x: BigNumber.from("0x{}"),
  y: BigNumber.from("0x{}"),
}};
"#,
        pub_key,
        hex::encode(enc_key_bytes),
        u256_to_hex(address.x),
        u256_to_hex(address.y),
    );
}
