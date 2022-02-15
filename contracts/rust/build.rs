use glob::glob;
use std::process::Command;

fn main() {
    for entry in glob("../abi/**/*.json").unwrap() {
        println!("cargo:rerun-if-changed={}", entry.unwrap().display());
    }

    Command::new("build-abi")
        .output()
        .expect("failed to compile contracts");
}
