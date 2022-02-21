use cap_rust_sandbox::state::{CapeContractState, CAPE_MERKLE_HEIGHT};
use jf_cap::MerkleTree;
use key_set::VerifierKeySet;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueryResultState {
    // latest state, primary source
    pub contract_state: CapeContractState,
    // additional indexed data for queries
}

impl QueryResultState {
    pub fn new(verifier_keys: VerifierKeySet) -> QueryResultState {
        QueryResultState {
            contract_state: CapeContractState::new(
                verifier_keys,
                MerkleTree::new(CAPE_MERKLE_HEIGHT).unwrap(),
            ),
        }
    }
}
