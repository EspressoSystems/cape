use std::process::Command;

fn main() {
    // Generate the solidity abi files. This enables "cargo build" in a fresh repo.
    Command::new("build-abi")
        .output()
        .expect("failed to compile contracts");
}
