# Relayer

The Relayer is the component of the system that collects transactions from end
users and submit them to the CAPE contract.

The current implementation is a simplified version where the Relayer only
forwards a single transaction at a time. Moreover the Relayer currently does not
validate the transaction on its own. If the transaction is invalid it will be
rejected by the CAPE contract.

To spin up a geth node with deployed contracts for testing run the
[run-geth-and-deploy](../bin/run-geth-and-deploy) script in a separate terminal.

```console
run-geth-and-deploy
```

The CAPE contract address shown in the terminal and an Ethereum wallet mnemonic
need to be passed to relayer executable, for example:

```console
cargo run --release --bin minimal-relayer -- 0xCf7Ed3AccA5a467e9e704C703E8D87F634fB0Fc9 "$TEST_MNEMONIC"
```
