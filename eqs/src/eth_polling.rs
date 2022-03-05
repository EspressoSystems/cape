// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use crate::query_result_state::QueryResultState;
use crate::state_persistence::StatePersistence;

use async_std::sync::{Arc, RwLock};

pub(crate) struct EthPolling {
    pub query_result_state: Arc<RwLock<QueryResultState>>,
    pub state_persistence: StatePersistence,
    pub last_updated_block_height: u64,
    // ethereum connection
}

impl EthPolling {
    pub async fn check(&mut self) -> Result<u64, async_std::io::Error> {
        // do eth poll, unpack updates
        let new_updated_block_height = 0; // replace with updated height
        if new_updated_block_height > self.last_updated_block_height {
            let updated_state = self.query_result_state.write().await;
            // update the state block
            // persist the state block updates (will be more fine grained in r3)
            self.state_persistence.store_latest_state(&*updated_state);
        }
        Ok(0)
    }
}
