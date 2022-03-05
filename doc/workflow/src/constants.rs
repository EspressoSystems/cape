// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

#![allow(dead_code)]

use ethers::prelude::*;
use jf_cap::keys::UserPubKey;

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

// allow building transaction against any one of the most recent 10 merkle root
pub(crate) const MERKLE_ROOT_QUEUE_CAP: usize = 10;
