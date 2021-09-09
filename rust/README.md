# Ethereum rust sandbox
This directory has it's own nix-shell.

Run a geth node (in a separate terminal, from anywhere)

    geth --dev --http

Run the example

    cargo run -p aap-rust-sandbox --example contract_local_signer

The example `examples/contract_local_signer.rs` is adapted from the `ethers-rs`
example
[contract_with_abi.rs](https://github.com/gakonst/ethers-rs/blob/master/examples/contract_with_abi.rs)
Here we sign the contract tx manually tx before sending it to the node.

If it workes the logs should look as follows

    [examples/contract_local_signer.rs:25] "Compiled!" = "Compiled!"
    [examples/contract_local_signer.rs:43] "Sent funding tx to deployer" = "Sent funding tx to deployer"
    Value: hi. Logs: [{"author":"0xd806fb32888de9225b73cd2406d09b1d6cfa8425","old_value":"","new_value":"initial value"},{"author":"0xd806fb32888de9225b73cd2406d09b1d6cfa8425","old_value":"initial value","new_value":"hi"}]
