use crate::api_server::init_web_server;
use crate::block_builder::Builder;
use crate::configuration::{reset_state, store_path};
use crate::state_persistence::StatePersistence;
use crate::txn_queue::TxnQueue;
use crate::validation_state::ValidationState;

use async_std::sync::{Arc, RwLock};

mod api_server;
mod block_builder;
mod configuration;
mod shared_types;
mod state_persistence;
mod txn_queue;
mod validation_state;

#[async_std::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt().pretty().init();
    let queue = Arc::new(RwLock::new(TxnQueue::new()));
    let _api_handle = init_web_server(queue.clone()).unwrap();

    let (state_persistence, validation_state) = if reset_state() {
        (
            StatePersistence::new(&store_path(), "relayer").unwrap(),
            ValidationState::new(),
        )
    } else {
        let state_persistence = StatePersistence::load(&store_path(), "relayer").unwrap();
        let validation_state = state_persistence.load_latest_state().unwrap();
        (state_persistence, validation_state)
    };

    let mut block_builder = Builder::new(queue, validation_state, state_persistence);

    // TODO: mechanism to signal for exit.
    loop {
        if let Some(_next_block) = block_builder.build_next().await {
            // TODO: serialize and submit block
        }
    }

    // api_handle.await.unwrap_or_else(|err| {
    //     panic!("web server exited with an error: {}", err);
    // });
    // Ok(())
}
