// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use crate::configuration::relayer_addr;
use crate::state_persistence::StatePersistence;
use crate::txn_queue::TxnQueue;

use cap_rust_sandbox::{
    cape::CapeBlock,
    model::{CapeContractState, CapeModelOperation},
};

use std::vec::Vec;

use async_std::sync::{Arc, RwLock};

pub struct Builder {
    queue: Arc<RwLock<TxnQueue>>,
    state: CapeContractState,
    store: StatePersistence,
}

impl Builder {
    pub fn new(
        queue: Arc<RwLock<TxnQueue>>,
        state: CapeContractState,
        store: StatePersistence,
    ) -> Builder {
        Builder {
            queue,
            state,
            store,
        }
    }

    pub async fn build_next(&mut self) -> Option<CapeBlock> {
        let queue_waiter = self.queue.read().await;
        if let Ok(txns) = queue_waiter.wait_for_block_ready() {
            let mut valid_txns = Vec::new();
            for txn in txns.into_iter() {
                if let Ok((new_state, _effects)) = self
                    .state
                    .submit_operations(vec![CapeModelOperation::SubmitBlock(vec![txn.clone()])])
                {
                    self.state = new_state;
                    valid_txns.push(txn);
                }
            }
            if valid_txns.is_empty() {
                None
            } else {
                self.store.store_latest_state(&self.state);
                CapeBlock::from_cape_transactions(valid_txns, relayer_addr()).ok()
            }
        } else {
            None
        }
    }
}
