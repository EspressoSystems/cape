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
use atomic_store::PersistenceError;

use async_std::{
    sync::{Arc, RwLock},
    task::sleep,
};
use cap_rust_sandbox::{
    ethereum::{ensure_connected_to_contract, get_provider_from_url},
    universal_param::verifier_keys,
};

pub async fn run(opt: &EQSOptions) -> std::io::Result<()> {
    if !opt.temp_test_run {
        let provider = get_provider_from_url(opt.rpc_url());
        ensure_connected_to_contract(&provider, opt.cape_address().unwrap())
            .await
            .unwrap();
    }

    let (state_persistence, query_result_state) = if opt.reset_state() {
        (
            StatePersistence::new(&opt.store_path(), "eth_query").unwrap(),
            Arc::new(RwLock::new(QueryResultState::new(verifier_keys()))),
        )
    } else {
        let state_persistence = StatePersistence::load(&opt.store_path(), "eth_query").unwrap();
        let query_result_state = Arc::new(RwLock::new(
            state_persistence.load_latest_state().unwrap_or_else(|err| {
                if let PersistenceError::FailedToFindExpectedResource { key: _ } = err {
                    QueryResultState::new(verifier_keys())
                } else {
                    panic!("{:?}", err);
                }
            }),
        ));
        (state_persistence, query_result_state)
    };

    let _api_handle = init_web_server(opt, query_result_state.clone()).unwrap();

    // will replace with subscription in phase 3
    let mut eth_poll = EthPolling::new(opt, query_result_state, state_persistence).await;

    loop {
        if let Ok(_height) = eth_poll.check().await {}
        // sleep here
        sleep(opt.query_frequency()).await;
    }
}
