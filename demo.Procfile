# The EQS needs to run in the ./eqs directory
# The log is a bit noisy, don't show it.
eqs: cd eqs && env RUST_LOG=error ../target/release/eqs --reset-store-state >> $LOG_FILENAME 2>&1
address-book: target/release/address-book >> $LOG_FILENAME 2>&1
wallet-api-alice: wallet-api-alice >> $LOG_FILENAME 2>&1
wallet-api-bob: wallet-api-bob >> $LOG_FILENAME 2>&1
relayer: target/release/minimal-relayer >> $LOG_FILENAME 2>&1
faucet: target/release/faucet >> $LOG_FILENAME 2>&1
