use glob::glob;
use std::{option_env, process::Command};

fn find_sol_paths() -> glob::Paths {
    glob(&format!("{}/**/*.sol", env!("CONTRACTS_DIR"))).unwrap()
}

fn main() {
    // If no abi files exist, generate them. This enables "cargo build" in a fresh repo.
    Command::new("build-abi")
        .output()
        .expect("failed to compile contracts");

    let paths = if option_env!("CAPE_DONT_WATCH_SOL_FILES").is_none() {
        find_sol_paths().into_iter().collect()
    } else {
        vec![]
    };

    // Rerun this script (and recompile crate) if any abi files change.
    for entry in paths {
        // run `cargo build -vv` to inspect output
        println!("cargo:rerun-if-changed={}", entry.unwrap().display());
    }
    println!("cargo:rerun-if-env-changed=CAPE_DONT_WATCH_SOL_FILES");
}