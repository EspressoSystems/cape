// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use ethers_contract_abigen::{Abigen, MultiAbigen};
use glob::glob;
use std::{option_env, process::Command};

fn find_paths(dir: &str, ext: &str) -> glob::Paths {
    glob(&format!("{}/**/*{}", dir, ext)).unwrap()
}

fn main() {
    // Contract compilation with ethers-rs is broken. Likely because of our
    // non-standard directory layout. Compile with hardhat instead.
    Command::new("hardhat")
        .arg("compile")
        .output()
        .expect("failed to compile contracts");

    let paths = if option_env!("CAPE_DONT_WATCH_SOL_FILES").is_none() {
        find_paths(env!("CONTRACTS_DIR"), ".sol")
            .into_iter()
            .collect()
    } else {
        vec![]
    };

    // Rerun this script (and recompile crate) if any abi files change.
    for entry in paths {
        // run `cargo build -vv` to inspect output
        println!("cargo:rerun-if-changed={}", entry.unwrap().display());
    }
    println!("cargo:rerun-if-env-changed=CAPE_DONT_WATCH_SOL_FILES");

    // Hardhat's debug files trip up MultiAbigen
    // otherwise we could use MultiAbigen::from_json_files instead
    let artifacts: Vec<_> = find_paths(
        &format!("{}/artifacts/contracts", env!("CONTRACTS_DIR")),
        ".json",
    )
    .chain(find_paths(
        &format!("{}/artifacts/@openzeppelin", env!("CONTRACTS_DIR")),
        "ERC20.json",
    ))
    .map(|path| path.unwrap())
    .filter(|path| !path.to_str().unwrap().ends_with(".dbg.json"))
    .collect();

    let abigens: Vec<_> = artifacts
        .iter()
        .map(|path| Abigen::from_file(path).unwrap())
        .collect();

    let gen = MultiAbigen::from_abigens(abigens);

    let bindings = gen.build().unwrap();
    bindings.write_to_module("src/bindings", true).unwrap();
}
