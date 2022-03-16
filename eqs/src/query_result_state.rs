// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use cap_rust_sandbox::ledger::{CapeLedger, CapeTransition, CommittedCapeTransition};
use cap_rust_sandbox::model::{CapeLedgerState, CapeRecordMerkleHistory, CAPE_MERKLE_HEIGHT};
use commit::Commitment;
use ethers::prelude::Address;
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
    pub last_fetched_block: u64,
    pub last_fetched_log_index: u64,
    pub contract_address: Option<Address>,

    // accumulated list of CAPE events
    pub events: Vec<LedgerEvent<CapeLedger>>,

    // additional indexed data for queries
    pub transaction_by_id: HashMap<(u64, u64), CommittedCapeTransition>,
    pub transaction_id_by_hash: HashMap<Commitment<CapeTransition>, (u64, u64)>,
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
            nullifiers: HashSet::new(),
            verifier_keys,
            last_fetched_block: 0u64,
            last_fetched_log_index: 0u64,
            contract_address: None,

            events: Vec::new(),

            transaction_by_id: HashMap::new(),
            transaction_id_by_hash: HashMap::new(),
        }
    }
}
