// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
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
use strum_macros::{AsRefStr, EnumString};

use crate::configuration::Confirmations;

/// The index of a single event in the Ethereum event stream.
///
/// Events are indexed using `(block_number, log_index)`, where `block_number` is the index of the
/// block containing the event, and `log_index` is the index of the event within the block. For any
/// event indices `i1: EthEventIndex` and `i2: EthEventIndex`, the event with index `i1` comes
/// before the event with index `i2` chronologically if and only if `i1 < i2`.
pub type EthEventIndex = (u64, u64);

#[derive(AsRefStr, Clone, Debug, Deserialize, EnumString, Serialize)]
pub enum SystemStatus {
    Initializing,
    Available,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueryResultState {
    // latest state, primary source
    pub ledger_state: CapeLedgerState,
    pub nullifiers: HashSet<Nullifier>,
    pub verifier_keys: VerifierKeySet,
    pub last_reported_index: Option<EthEventIndex>,
    pub contract_address: Option<Address>,

    // Configuration
    pub num_confirmations: Option<Confirmations>,

    // accumulated list of CAPE events
    pub events: Vec<LedgerEvent<CapeLedger>>,

    // additional indexed data for queries
    pub transaction_by_id: HashMap<(u64, u64), CommittedCapeTransition>,
    pub transaction_id_by_hash: HashMap<Commitment<CapeTransition>, (u64, u64)>,

    pub system_status: SystemStatus,
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
            last_reported_index: None,
            contract_address: None,

            num_confirmations: None,

            events: Vec::new(),

            transaction_by_id: HashMap::new(),
            transaction_id_by_hash: HashMap::new(),

            system_status: SystemStatus::Initializing,
        }
    }
}
