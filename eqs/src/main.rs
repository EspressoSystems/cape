use crate::api_server::init_web_server;
use crate::configuration::{query_frequency, reset_state, store_path, verifier_keys};
use crate::eth_polling::EthPolling;
use crate::query_result_state::QueryResultState;
use crate::state_persistence::StatePersistence;
use cap_rust_sandbox::{
    cape::submit_block::fetch_cape_block,
    ledger::CapeTransition,
    model::{Erc20Code, EthereumAddr},
    types::{RecordOpening as RecordOpeningSol, TestCAPEEvents},
};
//use cap_rust_sandbox::ethereum::get_provider;
use ethers::abi::AbiDecode;
use jf_cap::{structs::RecordOpening, MerkleTree};
use reef::traits::Block;
use seahorse::events::LedgerEvent;

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
    let mut eth_poll = EthPolling::new(query_result_state, state_persistence).await;

    // TODO: mechanism to signal for exit.
    loop {
        if let Ok(_height) = eth_poll.check().await {
            // do we want an idle/backoff on unchanged?
            //select cape events
            let new_event = eth_poll
                .contract
                .events()
                .from_block(eth_poll.last_updated_block_height + 1)
                .query_with_meta()
                .await
                .unwrap();

            for event in new_event {
                let (filter, meta) = event.clone();

                match filter {
                    TestCAPEEvents::BlockCommittedFilter(_) => {
                        let fetched_block_with_memos =
                            fetch_cape_block(&eth_poll.connection, meta.transaction_hash)
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
                            eth_poll
                                .query_result_state
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
                            eth_poll
                                .query_result_state
                                .write()
                                .await
                                .pending_commit_event
                                .push(trn);
                        }

                        let pending_commit = eth_poll
                            .query_result_state
                            .read()
                            .await
                            .pending_commit_event
                            .clone();

                        //create/push pending commit to QueryResultState events
                        eth_poll.query_result_state.write().await.events.push(
                            LedgerEvent::Commit {
                                block: cap_rust_sandbox::ledger::CapeBlock::new(pending_commit),
                                block_id: meta.block_number.as_u64(),
                                state_comm: meta.block_number.as_u64() + 1,
                            },
                        );

                        let input_record_commitment = fetched_block_with_memos
                            .block
                            .get_list_of_input_record_commitments();

                        let merkle_tree = MerkleTree::restore_from_frontier(
                            eth_poll
                                .query_result_state
                                .read()
                                .await
                                .ledger_state
                                .record_merkle_commitment,
                            &eth_poll
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
                            eth_poll
                                .query_result_state
                                .write()
                                .await
                                .ledger_state
                                .record_merkle_commitment = merkle_tree.commitment();
                            eth_poll
                                .query_result_state
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
                            eth_poll.query_result_state.write().await.events.push(event);
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
                        eth_poll
                            .query_result_state
                            .write()
                            .await
                            .pending_commit_event
                            .push(new_transition_wrap);
                    }
                }
                //update here?
                eth_poll.last_updated_block_height = meta.block_number.as_u64();
            }
        }
        // sleep here
        sleep(query_frequency()).await;
    }
}
