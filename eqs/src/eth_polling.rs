use crate::query_result_state::QueryResultState;
use crate::state_persistence::StatePersistence;

use async_std::sync::{Arc, RwLock};
use cap_rust_sandbox::{
    cape::submit_block::fetch_cape_block,
    deploy::EthMiddleware,
    ethereum::EthConnection,
    ledger::CapeTransition,
    model::{Erc20Code, EthereumAddr},
    types::{RecordOpening as RecordOpeningSol, TestCAPE, TestCAPEEvents},
};
use ethers::abi::AbiDecode;
use jf_cap::{structs::RecordOpening, MerkleTree};
use reef::traits::Block;
use seahorse::events::LedgerEvent;
pub(crate) struct EthPolling {
    pub query_result_state: Arc<RwLock<QueryResultState>>,
    pub state_persistence: StatePersistence,
    pub last_updated_block_height: u64,
    pub contract: TestCAPE<EthMiddleware>,
    pub connection: EthConnection,
}

impl EthPolling {
    pub async fn new(
        query_result_state: Arc<RwLock<QueryResultState>>,
        state_persistence: StatePersistence,
    ) -> EthPolling {
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
            //select cape events
            let new_event = self
                .contract
                .events()
                .from_block(new_updated_block_height)
                .query_with_meta()
                .await
                .unwrap();

            for event in new_event {
                let (filter, meta) = event.clone();

                match filter {
                    TestCAPEEvents::BlockCommittedFilter(_) => {
                        let fetched_block_with_memos =
                            fetch_cape_block(&self.connection, meta.transaction_hash)
                                .await
                                .unwrap()
                                .unwrap();

                        let model_txns = fetched_block_with_memos
                            .block
                            .clone()
                            .into_cape_transactions()
                            .unwrap()
                            .0;

                        //add transactions to QueryResultState pending commit
                        for tx in model_txns.clone() {
                            self.query_result_state
                                .write()
                                .await
                                .pending_commit_event
                                .push(CapeTransition::Transaction(tx.clone()));
                        }

                        let transitions = model_txns
                            .clone()
                            .clone()
                            .into_iter()
                            .map(CapeTransition::Transaction)
                            .collect::<Vec<_>>();

                        //push to state's pending commit event
                        for trn in transitions {
                            self.query_result_state
                                .write()
                                .await
                                .pending_commit_event
                                .push(trn);
                        }

                        let pending_commit = self
                            .query_result_state
                            .read()
                            .await
                            .pending_commit_event
                            .clone();

                        //create/push pending commit to QueryResultState events
                        self.query_result_state
                            .write()
                            .await
                            .events
                            .push(LedgerEvent::Commit {
                                block: cap_rust_sandbox::ledger::CapeBlock::new(pending_commit),
                                block_id: meta.block_number.as_u64(),
                                state_comm: meta.block_number.as_u64() + 1,
                            });

                        let input_record_commitment = fetched_block_with_memos
                            .block
                            .get_list_of_input_record_commitments();

                        let merkle_tree = MerkleTree::restore_from_frontier(
                            self.query_result_state
                                .read()
                                .await
                                .ledger_state
                                .record_merkle_commitment,
                            &self
                                .query_result_state
                                .read()
                                .await
                                .ledger_state
                                .record_merkle_frontier,
                        );

                        //add commitments to merkle tree
                        let mut uids = Vec::new();
                        let mut merkle_paths = Vec::new();
                        if let Some(mut merkle_tree) = merkle_tree {
                            for (_record_id, record_commitment) in
                                input_record_commitment.iter().enumerate()
                            {
                                uids.push(merkle_tree.num_leaves());
                                merkle_tree.push(record_commitment.to_field_element());
                            }
                            self.query_result_state
                                .write()
                                .await
                                .ledger_state
                                .record_merkle_commitment = merkle_tree.commitment();
                            self.query_result_state
                                .write()
                                .await
                                .ledger_state
                                .record_merkle_frontier = merkle_tree.frontier();

                            merkle_paths = uids
                                .iter()
                                .map(|uid| merkle_tree.get_leaf(*uid).expect_ok().unwrap().1.path)
                                .collect::<Vec<_>>();
                        }

                        //create LedgerEvent::Memo
                        let mut memo_events = Vec::new();
                        fetched_block_with_memos.memos.iter().enumerate().for_each(
                            |(txn_id, (txn_memo, _))| {
                                let mut outputs = Vec::new();
                                for (i, memo) in txn_memo.iter().enumerate() {
                                    outputs.push((
                                        memo.clone(),
                                        input_record_commitment[i],
                                        uids[i],
                                        merkle_paths[i].clone(),
                                    ));
                                }
                                let memo_event = LedgerEvent::Memos {
                                    outputs,
                                    transaction: Some((meta.block_number.as_u64(), txn_id as u64)),
                                };
                                memo_events.push(memo_event);
                            },
                        );

                        for event in memo_events {
                            self.query_result_state.write().await.events.push(event);
                        }
                    }

                    TestCAPEEvents::Erc20TokensDepositedFilter(filter_data) => {
                        let ro_bytes = filter_data.ro_bytes.clone();
                        let ro_sol: RecordOpeningSol = AbiDecode::decode(ro_bytes).unwrap();
                        let expected_ro = RecordOpening::from(ro_sol);

                        let erc20_code = Erc20Code(EthereumAddr(
                            filter_data.erc_20_token_address.to_fixed_bytes(),
                        ));

                        let new_transition_wrap = CapeTransition::Wrap {
                            ro: Box::new(expected_ro),
                            erc20_code,
                            src_addr: EthereumAddr(filter_data.from.to_fixed_bytes()),
                        };
                        self.query_result_state
                            .write()
                            .await
                            .pending_commit_event
                            .push(new_transition_wrap);
                    }
                }
            }
        }
        Ok(0)
    }
}
