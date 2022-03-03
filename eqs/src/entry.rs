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
    let mut eth_poll = EthPolling::new(query_result_state, state_persistence).await;

    // TODO: mechanism to signal for exit.
    loop {
        if let Ok(_height) = eth_poll.check().await {
            // do we want an idle/backoff on unchanged?
        }
        // sleep here
        sleep(opt.query_frequency()).await;
    }
}
