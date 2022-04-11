// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

//! A script to export environment variables for test deployments of the CAPE
//! contract with hardhat.

use cap_rust_sandbox::helpers::compute_faucet_key_pair_from_mnemonic;
use cap_rust_sandbox::types::EdOnBN254Point;
use ethers::{abi::AbiEncode, prelude::U256};
use rand_chacha::{rand_core::SeedableRng, ChaChaRng};
use seahorse::hd::{KeyTree, Mnemonic};
use structopt::StructOpt;

pub fn u256_to_hex(n: U256) -> String {
    hex::encode(AbiEncode::encode(n))
}

#[derive(Debug, StructOpt)]
#[structopt(
    name = "CAPE Faucet utility",
    about = "Create address and encryption key from mnemonic to pass to contract for testing"
)]
pub struct Options {
    /// mnemonic for the faucet wallet (if not provided, a random mnemonic will be generated)
    #[structopt(long, env = "CAPE_FAUCET_MANAGER_MNEMONIC")]
    pub mnemonic: Option<String>,
}

#[async_std::main]
async fn main() -> Result<(), std::io::Error> {
    let opt = Options::from_args();
    let mnemonic = match opt.mnemonic {
        Some(phrase) => Mnemonic::from_phrase(phrase.replace('-', " ")).unwrap(),
        None => KeyTree::random(&mut ChaChaRng::from_entropy()).1,
    };

    // We don't actually want to create a wallet, just generate a key, so we will directly generate
    // the key stream that the faucet wallet will use.
    let pub_key = compute_faucet_key_pair_from_mnemonic(&mnemonic).pub_key();

    eprintln!("Faucet manager encryption key: {}", pub_key);
    eprintln!(
        "Faucet manager address: {}",
        net::UserAddress(pub_key.address())
    );

    let enc_key_bytes: [u8; 32] = pub_key.enc_key().into();
    let address: EdOnBN254Point = pub_key.address().into();

    println!("export CAPE_FAUCET_MANAGER_MNEMONIC=\"{}\"", mnemonic);
    println!(
        "export CAPE_FAUCET_MANAGER_ENC_KEY=0x{}",
        hex::encode(enc_key_bytes)
    );
    println!(
        "export CAPE_FAUCET_MANAGER_ADDRESS_X=0x{}",
        u256_to_hex(address.x)
    );
    println!(
        "export CAPE_FAUCET_MANAGER_ADDRESS_Y=0x{}",
        u256_to_hex(address.y)
    );
    Ok(())
}
