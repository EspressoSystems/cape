use glob::glob;
use std::{env, process::Command};

fn find_abi_paths() -> glob::Paths {
    glob(&format!("{}/abi/**/*.json", env!("CONTRACTS_DIR"))).unwrap()
}

fn main() {
    // If no abi files exist, generate them. This enables "cargo build" in a fresh repo.
    Command::new("build-abi")
        .output()
        .expect("failed to compile contracts");

    // Rerun this script (and recompile crate) if any abi files change.
    for entry in find_abi_paths() {
        // run `cargo build -vv` to inspect output
        println!("cargo:rerun-if-changed={}", entry.unwrap().display());
    }
}
