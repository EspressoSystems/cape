use crate::shared_types::Transaction;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidationState {}

impl ValidationState {
    pub fn new() -> ValidationState {
        ValidationState {}
    }

    pub fn check(&mut self, _txn: &Transaction) -> bool {
        true
    }
}
