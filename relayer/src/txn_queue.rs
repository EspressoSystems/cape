// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

// when it comes to Condvar usage, clippy gets this so very, very wrong...
#![allow(clippy::mutex_atomic)]

use cap_rust_sandbox::model::CapeModelTxn;

use std::sync::{Condvar, Mutex};

pub struct TxnQueue {
    txns: Vec<CapeModelTxn>,
    block_ready: Mutex<bool>,
    block_notify: Condvar,
}

impl TxnQueue {
    pub fn new() -> Self {
        TxnQueue {
            txns: Vec::new(),
            block_ready: Mutex::new(false),
            block_notify: Condvar::new(),
        }
    }

    pub fn push(&mut self, txn: CapeModelTxn) {
        self.txns.push(txn);
        if self.check_for_block_limit() {
            let mut ready = self.block_ready.lock().unwrap();
            *ready = true;
            self.block_notify.notify_one();
        }
    }

    fn check_for_block_limit(&self) -> bool {
        // TODO: calculate maximum block size for contract, only return true when one more txn would exceed...
        true
    }

    pub fn wait_for_block_ready(&self) -> Result<Vec<CapeModelTxn>, bool> {
        let mut ready = self.block_ready.lock().unwrap();
        while !*ready {
            ready = self.block_notify.wait(ready).unwrap();
        }
        Err(true)
    }
}
