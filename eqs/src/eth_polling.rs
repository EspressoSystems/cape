use crate::query_result_state::QueryResultState;
use crate::state_persistence::StatePersistence;

use async_std::sync::{Arc, RwLock};
use cap_rust_sandbox::{
    deploy::EthMiddleware,
    ethereum::EthConnection,
    types::TestCAPE,
};

pub(crate) struct EthPolling {
    pub query_result_state: Arc<RwLock<QueryResultState>>,
    pub state_persistence: StatePersistence,
    pub last_updated_block_height: u64,
    pub contract: TestCAPE<EthMiddleware>,
    pub connection: EthConnection,
}

impl EthPolling {
    pub async fn new(query_result_state: Arc<RwLock<QueryResultState>>, state_persistence: StatePersistence) -> EthPolling {
        let connection = EthConnection::for_test().await;
        EthPolling {
            query_result_state,
            state_persistence,
            last_updated_block_height: 0,
            contract: connection.test_contract(),
            connection, // replace with EthConnection::connect()
        }
    }

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
