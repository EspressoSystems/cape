// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use crate::configuration::EQSOptions;
use crate::query_result_state::{EthEventIndex, QueryResultState};
use crate::state_persistence::StatePersistence;

use async_std::sync::{Arc, RwLock};
use cap_rust_sandbox::{
    cape::submit_block::fetch_cape_block,
    ethereum::EthConnection,
    ledger::{CapeTransactionKind, CapeTransition},
    model::{CapeModelTxn, Erc20Code, EthereumAddr},
    types::{CAPEEvents, RecordOpening as RecordOpeningSol},
};
use commit::Committable;
use core::mem;
use ethers::abi::AbiDecode;
use jf_cap::structs::{ReceiverMemo, RecordCommitment};
use jf_cap::{structs::RecordOpening, MerkleTree, TransactionNote};
use rand_chacha::rand_core::SeedableRng;
use rand_chacha::ChaChaRng;
use reef::traits::{Block, Transaction};
use seahorse::events::LedgerEvent;

pub(crate) struct EthPolling {
    pub query_result_state: Arc<RwLock<QueryResultState>>,
    pub state_persistence: StatePersistence,
    /// The index of the last event incorporated into `self`.
    pub last_event_index: Option<EthEventIndex>,
    /// The index of the earliest block which we might not have finished processing.
    pub next_block_to_query: u64,
    pub pending_commit_event: Vec<CapeTransition>,
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
                pending_commit_event: Vec::new(),
                last_event_index: None,
                next_block_to_query: 0,
                connection: EthConnection::for_test().await,
            };
        }

        let (connection, last_event_index) = if let Some(contract_address) = opt.cape_address() {
            let mut state_updater = query_result_state.write().await;
            let last_reported_index = state_updater.last_reported_index;

            if state_updater.contract_address.is_none() && last_reported_index.is_some() {
                panic!(
                    "Persisted state is malformed! Run again with --reset_store_state to repair"
                );
            }

            if let Some(persisted_contract_address) = state_updater.contract_address {
                if persisted_contract_address != contract_address {
                    panic!("The specified persisted state was generated for a different contract.");
                }
            } else {
                state_updater.contract_address = Some(contract_address);
            }

            (
                EthConnection::from_config_for_query(
                    &format!("{:?}", contract_address),
                    opt.rpc_url(),
                ),
                last_reported_index,
            )
        } else {
            panic!("Invocation Error! Address required unless launched for testing");
        };

        let next_block_to_query = match last_event_index {
            Some((block, _log_index)) => {
                // We start querying from the same block that we last reported from, if there is
                // one, just in case that there were more events in that block which we didn't have
                // time to process before shutting down.
                block
            }
            None => {
                // If we haven't seen any events yet, start from the beginning.
                0
            }
        };
        EthPolling {
            query_result_state,
            state_persistence,
            last_event_index,
            next_block_to_query,
            pending_commit_event: Vec::new(),
            connection,
        }
    }

    pub async fn check(&mut self) -> Result<u64, async_std::io::Error> {
        // do eth poll, unpack updates
        // select cape events starting from the next block which we haven't finished processing
        let new_event = self
            .connection
            .contract
            .events()
            .from_block(self.next_block_to_query)
            .query_with_meta()
            .await
            .unwrap();

        let mut latest_block = None;
        for (filter, meta) in new_event {
            let current_block = meta.block_number.as_u64();
            let current_log_index = meta.log_index.as_u64();
            let current_index = (current_block, current_log_index);

            // We sometimes see repeat events in spite of the `from_block(next_block_to_query)`
            // filter. This appears to be a hardhat bug. Skip this event if it does not come after
            // the last reported event.
            if let Some(last_event_index) = self.last_event_index {
                if current_index <= last_event_index {
                    continue;
                }
            }
            latest_block = Some(current_block);
            match filter {
                CAPEEvents::BlockCommittedFilter(_) => {
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

                    // TODO Instead of panicking here we need to handle cases of missing memos gracefully
                    let num_txn = model_txns.len();
                    let num_txn_memo = fetched_block_with_memos.memos.len();
                    if num_txn != num_txn_memo {
                        panic!(
                            "Different number of txns and txn memos: {} vs {}",
                            num_txn, num_txn_memo
                        );
                    }

                    // Get all the wrap transitions which are pending since the last commit.
                    let wraps = mem::take(&mut self.pending_commit_event);

                    // Add transactions followed by wraps to pending commit
                    let mut transitions = Vec::new();
                    for tx in model_txns.clone() {
                        transitions.push(CapeTransition::Transaction(tx.clone()));
                    }
                    let pending_commit =
                        transitions.iter().cloned().chain(wraps).collect::<Vec<_>>();
                    let output_record_commitments = pending_commit
                        .iter()
                        .flat_map(|txn| txn.output_commitments())
                        .collect::<Vec<_>>();

                    let mut merkle_tree = {
                        let state_lock = self.query_result_state.read().await;
                        MerkleTree::restore_from_frontier(
                            state_lock.ledger_state.record_merkle_commitment,
                            &state_lock.ledger_state.record_merkle_frontier,
                        )
                    };

                    //add commitments to merkle tree
                    let mut uids = Vec::new();
                    let mut merkle_paths = Vec::new();
                    if let Some(merkle_tree) = merkle_tree.as_mut() {
                        for (_record_id, record_commitment) in
                            output_record_commitments.iter().enumerate()
                        {
                            uids.push(merkle_tree.num_leaves());
                            merkle_tree.push(record_commitment.to_field_element());
                        }
                        merkle_paths = uids
                            .iter()
                            .map(|uid| merkle_tree.get_leaf(*uid).expect_ok().unwrap().1.path)
                            .collect::<Vec<_>>();
                    }

                    let memos_sig_valid: Vec<_> = model_txns
                        .iter()
                        .zip(fetched_block_with_memos.memos.iter())
                        .map(|(tx, (recv_memos, sig))| {
                            match tx {
                                CapeModelTxn::CAP(note) => note.clone(),
                                CapeModelTxn::Burn { xfr, .. } => {
                                    TransactionNote::from(*xfr.clone())
                                }
                            }
                            .verify_receiver_memos_signature(recv_memos, sig)
                            .is_ok()
                        })
                        .collect();

                    let mut updated_state = self.query_result_state.write().await;

                    //create/push pending commit to QueryResultState events
                    let block_id = updated_state.ledger_state.state_number;
                    updated_state.events.push(LedgerEvent::Commit {
                        block: cap_rust_sandbox::ledger::CapeBlock::new(pending_commit),
                        block_id,
                        state_comm: block_id + 1,
                    });

                    // Create LedgerEvent::Memos if memo signature is valid, skip otherwise
                    let mut memo_events = Vec::new();
                    let mut index = 0;
                    fetched_block_with_memos
                        .memos
                        .iter()
                        .enumerate()
                        .filter_map(|(txn_id, (txn_memo, _))| match memos_sig_valid[txn_id] {
                            true => Some((txn_id, txn_memo)),
                            false => None,
                        })
                        .for_each(|(txn_id, txn_memo)| {
                            let mut outputs = Vec::new();
                            for memo in txn_memo.iter() {
                                outputs.push((
                                    memo.clone(),
                                    output_record_commitments[index],
                                    uids[index],
                                    merkle_paths[index].clone(),
                                ));
                                index += 1;
                            }
                            let memo_event = LedgerEvent::Memos {
                                outputs,
                                transaction: Some((
                                    block_id,
                                    txn_id as u64,
                                    transitions[txn_id].kind(),
                                )),
                            };
                            memo_events.push(memo_event);
                        });
                    updated_state.events.append(&mut memo_events);

                    //update merkle tree
                    if let Some(merkle_tree) = merkle_tree {
                        updated_state.ledger_state.record_merkle_commitment =
                            merkle_tree.commitment();
                        updated_state.ledger_state.record_merkle_frontier = merkle_tree.frontier();
                    }

                    //update transaction_by_id and transaction_id_by_hash hashmap
                    let mut record_index = 0;
                    transitions
                        .iter()
                        .enumerate()
                        .for_each(|(txn_id, transition)| {
                            updated_state.transaction_by_id.insert(
                                (block_id, txn_id as u64),
                                cap_rust_sandbox::ledger::CommittedCapeTransition {
                                    block_id,
                                    txn_id: txn_id as u64,
                                    output_start: uids[record_index],
                                    output_size: transition.output_len() as u64,
                                    transition: transition.clone(),
                                },
                            );
                            updated_state
                                .transaction_id_by_hash
                                .insert(transition.commit(), (block_id, txn_id as u64));
                            for nullifier in transition.proven_nullifiers().iter() {
                                updated_state.nullifiers.insert(nullifier.0);
                            }

                            record_index += transition.output_len();
                        });

                    updated_state.ledger_state.state_number += 1;
                    updated_state.last_reported_index = Some(current_index);
                    self.last_event_index = Some(current_index);

                    // persist the state block updates (will be more fine grained in r3)
                    self.state_persistence.store_latest_state(&*updated_state);
                }

                CAPEEvents::Erc20TokensDepositedFilter(filter_data) => {
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

                    self.pending_commit_event.push(new_transition_wrap);
                    self.last_event_index = Some(current_index);
                }
                CAPEEvents::FaucetInitializedFilter(filter_data) => {
                    // Obtain record opening
                    let ro_sol: RecordOpeningSol = AbiDecode::decode(filter_data.ro_bytes).unwrap();
                    let ro = ro_sol.into();

                    // Compute record commmitment
                    let rc = RecordCommitment::from(&ro);

                    // Update the Merkle tree
                    let merkle_tree = {
                        let state_lock = self.query_result_state.read().await;
                        MerkleTree::restore_from_frontier(
                            state_lock.ledger_state.record_merkle_commitment,
                            &state_lock.ledger_state.record_merkle_frontier,
                        )
                    };

                    let mut merkle_tree = merkle_tree.unwrap().clone();
                    let uid = merkle_tree.num_leaves();
                    merkle_tree.push(rc.to_field_element());
                    let merkle_path = merkle_tree.get_leaf(uid).expect_ok().unwrap().1.path;

                    // Generate commit event.
                    let transition = CapeTransition::Faucet {
                        ro: Box::new(ro.clone()),
                    };
                    let commit_event = LedgerEvent::Commit {
                        block: cap_rust_sandbox::ledger::CapeBlock::new(vec![transition.clone()]),
                        block_id: 0,
                        state_comm: 1,
                    };

                    // Generate the memo
                    let mut rng = ChaChaRng::from_entropy();
                    let memo = ReceiverMemo::from_ro(&mut rng, &ro, &[]).unwrap();
                    let output = (memo.clone(), rc, uid, merkle_path.clone());
                    let memo_event = LedgerEvent::Memos {
                        outputs: vec![output],
                        transaction: Some((0, 0, CapeTransactionKind::Faucet)),
                    };

                    // Update the local data structures
                    let mut updated_state = self.query_result_state.write().await;
                    assert_eq!(updated_state.ledger_state.state_number, 0);
                    assert!(updated_state.transaction_by_id.is_empty());
                    assert!(updated_state.transaction_id_by_hash.is_empty());
                    assert!(updated_state.events.is_empty());

                    updated_state.ledger_state.record_merkle_commitment = merkle_tree.commitment();
                    updated_state.ledger_state.record_merkle_frontier = merkle_tree.frontier();
                    updated_state
                        .events
                        .append(&mut vec![commit_event, memo_event]);

                    //update transaction_by_id and transaction_id_by_hash hashmap
                    updated_state.transaction_by_id.insert(
                        (0, 0),
                        cap_rust_sandbox::ledger::CommittedCapeTransition {
                            block_id: 0,
                            txn_id: 0,
                            output_start: 0,
                            output_size: 1,
                            transition: transition.clone(),
                        },
                    );
                    updated_state
                        .transaction_id_by_hash
                        .insert(transition.commit(), (0, 0));

                    updated_state.last_reported_index = Some(current_index);
                    self.last_event_index = Some(current_index);

                    // persist the state block updates (will be more fine grained in r3)
                    self.state_persistence.store_latest_state(&updated_state);
                }
            }
        }

        // If we finished processing a block, query for the following block next time.
        if let Some(latest_block) = latest_block {
            self.next_block_to_query = latest_block + 1;
        }

        Ok(0)
    }
}
