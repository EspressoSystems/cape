use crate::api_server::init_web_server;
use crate::configuration::{query_frequency, reset_state, store_path, verifier_keys};
use crate::eth_polling::EthPolling;
use crate::query_result_state::QueryResultState;
use crate::state_persistence::StatePersistence;

use async_std::{
    sync::{Arc, RwLock},
    task::sleep,
};

mod api_server;
mod configuration;
mod disco; // really needs to go into a shared crate
mod errors;
mod eth_polling;
mod query_result_state;
mod route_parsing;
mod routes;
mod state_persistence;

#[async_std::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt().pretty().init();
    let (state_persistence, query_result_state) = if reset_state() {
        (
            StatePersistence::new(&store_path(), "eqs").unwrap(),
            Arc::new(RwLock::new(QueryResultState::new(verifier_keys()))),
        )
    } else {
        let state_persistence = StatePersistence::load(&store_path(), "eqs").unwrap();
        let query_result_state =
            Arc::new(RwLock::new(state_persistence.load_latest_state().unwrap()));
        (state_persistence, query_result_state)
    };

    let _api_handle = init_web_server(query_result_state.clone()).unwrap();

    // will replace with subscription in phase 3
    let mut eth_poll = EthPolling {
        query_result_state,
        state_persistence,
        last_updated_block_height: 0,
    };

    // TODO: mechanism to signal for exit.
    loop {
        if let Ok(_height) = eth_poll.check().await {
            // do we want an idle/backoff on unchanged?
        }
        // sleep here
        sleep(query_frequency()).await;
    }
}
