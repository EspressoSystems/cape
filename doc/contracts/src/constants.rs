#![allow(dead_code)]

use ethers::prelude::*;
use jf_txn::keys::UserPubKey;

// USDC contract on Ethereum, https://etherscan.io/address/0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48
pub(crate) fn usdc_address() -> Address {
    "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
        .parse()
        .unwrap()
}

// the address to send to during burning transaction, equivalent to `address(0x0)` in ETH.
pub(crate) fn burn_pub_key() -> UserPubKey {
    UserPubKey::default()
}

pub(crate) const RECORD_MT_HEIGHT: u8 = 25;
