use glob::glob;
use std::{env, process::Command};

fn main() {
    for entry in glob(&format!("{}/abi/**/*.json", env!("CONTRACTS_DIR"))).unwrap() {
        // run `cargo build -vv` to inspect output
        println!("cargo:rerun-if-changed={}", entry.unwrap().display());
    }

    Command::new("build-abi")
        .output()
        .expect("failed to compile contracts");
}
