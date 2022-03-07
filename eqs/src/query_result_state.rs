// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

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
