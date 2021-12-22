use crate::shared_types::{Block, Transaction};
use crate::state_persistence::StatePersistence;
use crate::txn_queue::TxnQueue;
use crate::validation_state::ValidationState;

use std::vec::Vec;

use async_std::sync::{Arc, RwLock};

pub struct Builder {
    queue: Arc<RwLock<TxnQueue>>,
    state: ValidationState,
    store: StatePersistence,
}

impl Builder {
    pub fn new(
        queue: Arc<RwLock<TxnQueue>>,
        state: ValidationState,
        store: StatePersistence,
    ) -> Builder {
        Builder {
            queue,
            state,
            store,
        }
    }

    pub async fn build_next(&mut self) -> Option<Block> {
        let queue_waiter = self.queue.read().await;
        if let Ok(txns) = queue_waiter.wait_for_block_ready() {
            let valid_txns: Vec<Transaction> = txns
                .into_iter()
                .filter(|txn| self.state.check(txn))
                .collect();
            if valid_txns.is_empty() {
                None
            } else {
                self.store.store_latest_state(&self.state);
                Some(Block::build_from(valid_txns))
            }
        } else {
            None
        }
    }
}
