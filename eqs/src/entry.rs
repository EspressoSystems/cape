// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use crate::api_server::init_web_server;
use crate::configuration::EQSOptions;
use crate::eth_polling::EthPolling;
use crate::query_result_state::QueryResultState;
use crate::state_persistence::StatePersistence;

use async_std::{
    sync::{Arc, RwLock},
    task::sleep,
};

pub async fn run(opt: &EQSOptions) -> std::io::Result<()> {
    let (state_persistence, query_result_state) = if opt.reset_state() {
        (
            StatePersistence::new(&opt.store_path(), "eqs").unwrap(),
            Arc::new(RwLock::new(QueryResultState::new(opt.verifier_keys()))),
        )
    } else {
        let state_persistence = StatePersistence::load(&opt.store_path(), "eqs").unwrap();
        let query_result_state =
            Arc::new(RwLock::new(state_persistence.load_latest_state().unwrap()));
        (state_persistence, query_result_state)
    };

    let _api_handle = init_web_server(opt, query_result_state.clone()).unwrap();

    // will replace with subscription in phase 3
    let mut eth_poll = EthPolling::new(opt, query_result_state, state_persistence).await;

    // TODO: mechanism to signal for exit.
    loop {
        if let Ok(_height) = eth_poll.check().await {
            // do we want an idle/backoff on unchanged?
        }
        // sleep here
        sleep(opt.query_frequency()).await;
    }
}
