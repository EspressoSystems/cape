# Ethereum rust sandbox
This directory has it's own nix-shell.

## Development
Run a geth node (in a separate terminal, from anywhere)

    geth --dev --http

Compile the contracts to extract the abi for the ethers abigen (workflow to be improved!)

    build-abi

Note: structs will only be included in the ABI if there is a (public, I guess)
function that uses them.

Watch directory and run tests on changes

    cargo watch -x test

## Examples
Generate a `jf_txn::transfer::TransferNote` and save it to a file `my_note.bin`:

    cargo run -p aap-rust-sandbox --example create_note

Load the file

    cargo run -p aap-rust-sandbox --example read_note
