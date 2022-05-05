# The EQS needs to run in the ./eqs directory
# The log is a bit noisy, don't show it.

eqs: cd eqs && env RUST_LOG=error ../target/release/eqs --reset-store-state
address-book: target/release/address-book
wallet-api-alice: wallet-api-alice
wallet-api-bob: wallet-api-bob
relayer: target/release/minimal-relayer
faucet: target/release/faucet 
