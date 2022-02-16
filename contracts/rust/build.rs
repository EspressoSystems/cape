use glob::glob;
use std::{env, process::Command};

fn main() {
    // Run the build command first so that the ABI files are for the glob expansion below.
    Command::new("build-abi")
        .output()
        .expect("failed to compile contracts");

    for entry in glob(&format!("{}/abi/**/*.json", env!("CONTRACTS_DIR"))).unwrap() {
        // run `cargo build -vv` to inspect output
        println!("cargo:rerun-if-changed={}", entry.unwrap().display());
    }
}
