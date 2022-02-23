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
