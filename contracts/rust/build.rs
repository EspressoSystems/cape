// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use ethers_contract_abigen::{Abigen, MultiAbigen};
use glob::glob;
use itertools::Itertools;
use std::{option_env, path::PathBuf, process::Command};

fn find_paths(dir: &str, ext: &str) -> Vec<PathBuf> {
    glob(&format!("{}/**/*{}", dir, ext))
        .unwrap()
        .map(|entry| entry.unwrap())
        .collect()
}

fn main() {
    // Contract compilation with ethers-rs is broken. Likely because of our
    // non-standard directory layout. Compile with hardhat instead.
    Command::new("hardhat")
        .arg("compile")
        .output()
        .expect("failed to compile contracts");

    // Watch all solidity files, artifact directories and their parent
    // directories. Artifacts directories also end with ".sol".
    let paths = if option_env!("CAPE_DONT_WATCH_SOL_FILES").is_none() {
        find_paths(env!("CONTRACTS_DIR"), ".sol")
    } else {
        vec![]
    }
    .into_iter()
    .flat_map(|path| [path.clone(), path.parent().unwrap().to_path_buf()])
    .unique()
    .sorted();

    // Rerun this script (and recompile crate) if any abi files change.
    for path in paths {
        // run `cargo build -vv` to inspect output
        println!("cargo:rerun-if-changed={}", path.display());
    }
    println!("cargo:rerun-if-env-changed=CAPE_DONT_WATCH_SOL_FILES");

    // Hardhat's debug files trip up MultiAbigen
    // otherwise we could use MultiAbigen::from_json_files instead
    let artifacts: Vec<_> = find_paths(
        &format!("{}/artifacts/contracts", env!("CONTRACTS_DIR")),
        ".json",
    )
    .into_iter()
    .chain(find_paths(
        &format!("{}/artifacts/@openzeppelin", env!("CONTRACTS_DIR")),
        "ERC20.json",
    ))
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
