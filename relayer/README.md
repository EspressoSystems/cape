# Relayer

To spin up a geth node with deployed contracts for testing run the
[run-geth-and-deploy](../bin/run-geth-and-deploy) script in a separate terminal.

    run-geth-and-deploy

The CAPE contract address shown in the terminal and an Ethereum wallet mnemonic
need to be passed to relayer executable, for example:

    cargo run --release --bin minimal-relayer -- 0xCf7Ed3AccA5a467e9e704C703E8D87F634fB0Fc9 "$TEST_MNEMONIC"
