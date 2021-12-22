use jf_aap::{structs::Nullifier, TransactionNote};
use serde::{Deserialize, Serialize};

// not quite compatible with zerok_lib::ledger::traits::Transaction, but we may need to hack it.

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Transaction {
    note: TransactionNote,
    nullifiers: Vec<Nullifier>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Block {}

impl Block {
    pub fn build_from(_txns: Vec<Transaction>) -> Block {
        Block {}
    }
}
