use cap_rust_sandbox::ledger::{CapeLedger, CommittedCapeTransition};
use cap_rust_sandbox::model::{CapeLedgerState, CapeRecordMerkleHistory, CAPE_MERKLE_HEIGHT};
use jf_cap::structs::Nullifier;
use jf_cap::MerkleTree;
use key_set::VerifierKeySet;
use seahorse::events::LedgerEvent;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueryResultState {
    // latest state, primary source
    pub ledger_state: CapeLedgerState,
    pub nullifiers: HashSet<Nullifier>,
    pub verifier_keys: VerifierKeySet,

    // accumulated list of CAPE events
    pub events: Vec<LedgerEvent<CapeLedger>>,

    // additional indexed data for queries
    pub transaction_by_id: HashMap<(u64, u64), CommittedCapeTransition>,
    pub transaction_by_hash: HashMap<(u64, u64), CommittedCapeTransition>,
}

impl QueryResultState {
    pub const RECORD_ROOT_HISTORY_SIZE: usize = 10;

    pub fn new(verifier_keys: VerifierKeySet) -> QueryResultState {
        let record_merkle_frontier = MerkleTree::new(CAPE_MERKLE_HEIGHT).unwrap();
        QueryResultState {
            ledger_state: CapeLedgerState {
                state_number: 0u64,
                record_merkle_commitment: record_merkle_frontier.commitment(),
                record_merkle_frontier: record_merkle_frontier.frontier(),
                past_record_merkle_roots: CapeRecordMerkleHistory(VecDeque::with_capacity(
                    Self::RECORD_ROOT_HISTORY_SIZE,
                )),
            },
            verifier_keys,
            nullifiers: HashSet::new(),

            events: Vec::new(),

            transaction_by_id: HashMap::new(),
            transaction_by_hash: HashMap::new(),
        }
    }
}
