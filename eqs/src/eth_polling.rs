use crate::configuration::EQSOptions;
use crate::query_result_state::QueryResultState;
use crate::state_persistence::StatePersistence;

use async_std::sync::{Arc, RwLock};
use cap_rust_sandbox::ethereum::EthConnection;
use ethers::prelude::{
    coins_bip39::English, Http, LocalWallet, MnemonicBuilder, Provider, SignerMiddleware,
};

pub(crate) struct EthPolling {
    pub query_result_state: Arc<RwLock<QueryResultState>>,
    pub state_persistence: StatePersistence,
    pub last_updated_block_height: u64,
    pub connection: EthConnection,
}

impl EthPolling {
    pub async fn new(
        opt: &EQSOptions,
        query_result_state: Arc<RwLock<QueryResultState>>,
        state_persistence: StatePersistence,
    ) -> EthPolling {
        if opt.temp_test_run() {
            return EthPolling {
                query_result_state,
                state_persistence,
                last_updated_block_height: 0u64,
                connection: EthConnection::for_test().await,
            };
        }

        let (connection, last_updated_block_height) = if let Some(contract_address) =
            opt.cape_address()
        {
            let mut state_updater = query_result_state.write().await;
            let last_updated_block_height = state_updater.last_updated_block_height;

            if state_updater.contract_address.is_none()
                && state_updater.last_updated_block_height > 0
            {
                panic!(
                    "Persisted state is malformed! Run again with --reset_store_state to repair"
                );
            }

            if state_updater.contract_address.is_none() {
                state_updater.contract_address = Some(contract_address);
            }

            if state_updater.contract_address.unwrap() != contract_address {
                panic!("The specified persisted state was generated for a different contract. Please specify a path for persistence that is either empty, or one built against the specified CAPE address.");
            }

            let provider = Provider::<Http>::try_from(opt.rpc_url())
                .expect("could not instantiate Ethereum HTTP Provider");
            let wallet = if opt.mnemonic().is_empty() {
                LocalWallet::new(&mut rand::thread_rng())
            } else {
                MnemonicBuilder::<English>::default()
                    .phrase(opt.mnemonic())
                    .build()
                    .expect("could not open wallet for EQS")
            };
            let client = Arc::new(SignerMiddleware::new(provider.clone(), wallet));

            (
                EthConnection::connect(provider, client, contract_address),
                last_updated_block_height,
            )
        } else {
            panic!("Invocation Error! Address required unless launched for testing");
        };

        EthPolling {
            query_result_state,
            state_persistence,
            last_updated_block_height,
            connection,
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
