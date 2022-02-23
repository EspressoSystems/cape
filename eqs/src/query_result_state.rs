use cap_rust_sandbox::ledger::CapeLedger;
use cap_rust_sandbox::model::{CapeContractState, CAPE_MERKLE_HEIGHT};
use jf_cap::MerkleTree;
use key_set::VerifierKeySet;
use seahorse::events::LedgerEvent;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueryResultState {
    // latest state, primary source
    pub contract_state: CapeContractState,

    // accumulated list of CAPE events
    pub events: Vec<LedgerEvent<CapeLedger>>,
    // additional indexed data for queries
}

impl QueryResultState {
    pub fn new(verifier_keys: VerifierKeySet) -> QueryResultState {
        QueryResultState {
            contract_state: CapeContractState::new(
                verifier_keys,
                MerkleTree::new(CAPE_MERKLE_HEIGHT).unwrap(),
            ),
            events: Vec::new(),
        }
    }
}
