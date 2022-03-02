#[cfg(test)]
mod tests {
    use crate::{
        cape::{
            submit_block::{fetch_cape_block, submit_cape_block_with_memos},
            BlockWithMemos, CapeBlock, NoteType,
        },
        deploy::deploy_erc20_token,
        ethereum::{get_provider, EthConnection},
        ledger::{CapeLedger, CapeTransition},
        model::{erc20_asset_description, CapeModelTxn, Erc20Code, EthereumAddr},
        test_utils::ContractsInfo,
        types::{
            self as sol, GenericInto, MerkleRootSol, RecordOpening as RecordOpeningSol,
            TestCAPEEvents,
        },
    };
    use async_std::sync::Mutex;
    use ethers::{
        abi::AbiDecode,
        prelude::{Middleware, U256},
    };
    use itertools::Itertools;
    use jf_cap::{
        keys::{UserKeyPair, UserPubKey},
        sign_receiver_memos,
        structs::{
            AssetCode, AssetDefinition, AssetPolicy, FreezeFlag, ReceiverMemo, RecordCommitment,
            RecordOpening,
        },
        utils::TxnsParams,
        KeyPair, TransactionNote,
    };
    use rand::{RngCore, SeedableRng};
    use rand_chacha::ChaChaRng;
    use reef::Ledger;
    use std::iter::repeat_with;

    fn cape_block_to_transactions(block: CapeBlock) -> Option<Vec<CapeModelTxn>> {
        let note_types = block.note_types.into_iter();
        let mut transfer_notes = block.transfer_notes.into_iter();
        let mut mint_notes = block.mint_notes.into_iter();
        let mut freeze_notes = block.freeze_notes.into_iter();
        let mut burn_notes = block.burn_notes.into_iter();
        let mut ret = vec![];
        for nt in note_types {
            match nt {
                NoteType::Transfer => {
                    ret.push(CapeModelTxn::CAP(TransactionNote::Transfer(Box::new(
                        transfer_notes.next()?,
                    ))));
                }
                NoteType::Mint => {
                    ret.push(CapeModelTxn::CAP(TransactionNote::Mint(Box::new(
                        mint_notes.next()?,
                    ))));
                }
                NoteType::Freeze => {
                    ret.push(CapeModelTxn::CAP(TransactionNote::Freeze(Box::new(
                        freeze_notes.next()?,
                    ))));
                }
                NoteType::Burn => {
                    let burn_note = burn_notes.next()?;
                    ret.push(CapeModelTxn::Burn {
                        xfr: Box::new(burn_note.transfer_note),
                        ro: Box::new(burn_note.burned_ro),
                    });
                }
            }
        }
        if transfer_notes.next().is_some()
            || mint_notes.next().is_some()
            || freeze_notes.next().is_some()
            || burn_notes.next().is_some()
        {
            None
        } else {
            Some(ret)
        }
    }

    #[tokio::test]
    async fn eqs_test() -> anyhow::Result<()> {
        //create cape block with transaction for erc20dep
        let mut rng = ChaChaRng::from_seed([0x42u8; 32]);
        let num_transfer_txn = 1;
        let num_mint_txn = 0;
        let num_freeze_txn = 0;
        let params_erc20 = TxnsParams::generate_txns(
            &mut rng,
            num_transfer_txn,
            num_mint_txn,
            num_freeze_txn,
            CapeLedger::merkle_height(),
        );
        let miner = UserPubKey::default();

        let cape_block_erc20 =
            CapeBlock::generate(params_erc20.txns.clone(), vec![], miner.address())?;

        let params_empty =
            TxnsParams::generate_txns(&mut rng, 1, 0, 0, CapeLedger::merkle_height());

        //create Mutex for testing on depolyed CAPE contract
        let eth_connection = Mutex::new(EthConnection::for_test().await);

        // Deploy ERC20 token contract. The client deploying the erc20 token contract receives 1000 * 10**18 tokens
        let erc20_token_contract = deploy_erc20_token().await;

        let cape_contract = eth_connection.lock().await.test_contract();
        let contracts_info = ContractsInfo::new(&cape_contract, &erc20_token_contract).await;

        let root = params_empty.txns[0].merkle_root();

        let key_pair = UserKeyPair::default();
        let memos_with_sigs = repeat_with(|| {
            let memos = repeat_with(|| {
                let amount = rng.next_u64();
                let ro = RecordOpening::new(
                    &mut rng,
                    amount,
                    AssetDefinition::native(),
                    key_pair.pub_key(),
                    FreezeFlag::Unfrozen,
                );
                ReceiverMemo::from_ro(&mut rng, &ro, &[]).unwrap()
            })
            .take(3)
            .collect::<Vec<_>>();
            let sig = sign_receiver_memos(&KeyPair::generate(&mut rng), &memos).unwrap();
            (memos, sig)
        })
        .take(3)
        .collect_vec();

        //add root
        cape_contract
            .add_root(root.generic_into::<MerkleRootSol>().0)
            .send()
            .await?
            .await?;

        drop(cape_contract);

        let _block_committed_event_listener = async {
            let mut number_events = 0;
            while number_events < 5 {
                let cape_contract = eth_connection.lock().await.test_contract();
                let new_entry = cape_contract
                    .block_committed_filter()
                    .from_block(0u64)
                    .query_with_meta()
                    .await
                    .unwrap();

                drop(cape_contract);

                while new_entry.len() > number_events {
                    //get block from transaction hash
                    let (filter, meta) = new_entry[number_events].clone();

                    let provider = get_provider();

                    // Fetch Ethereum transaction that emitted event
                    let _tx = provider
                        .get_transaction(meta.transaction_hash)
                        .await
                        .unwrap();

                    let _wraps = filter
                        .deposit_commitments
                        .iter()
                        .map(|&rc| {
                            rc.generic_into::<sol::RecordCommitmentSol>()
                                .generic_into::<RecordCommitment>()
                        })
                        .collect_vec();

                    number_events += 1;
                }
            }
        };

        let _erc_20_tokens_deposited_event_listener = async {
            let mut number_events = 0;
            while number_events < 1 {
                let cape_contract = eth_connection.lock().await.test_contract();

                let new_erc20_deposit = cape_contract
                    .erc_20_tokens_deposited_filter()
                    .from_block(0u64)
                    .query()
                    .await
                    .unwrap();

                while new_erc20_deposit.len() > number_events {
                    dbg!(new_erc20_deposit.clone());
                    number_events += 1;
                }
            }
        };

        let events_listener = async {
            let mut last_event = 0u64;
            while last_event < 100 {
                let cape_contract = eth_connection.lock().await.test_contract();

                let new_event = cape_contract
                    .events()
                    .from_block(last_event + 1)
                    .query_with_meta()
                    .await
                    .unwrap();

                drop(cape_contract);

                for event in new_event {
                    dbg!(event.clone());
                    let (filter, meta) = event.clone();

                    match filter {
                        TestCAPEEvents::BlockCommittedFilter(filter_data) => {
                            let provider = get_provider();

                            // Fetch Ethereum transaction that emitted event
                            let _txs = provider
                                .get_transaction(meta.transaction_hash)
                                .await
                                .unwrap()
                                .unwrap();

                            let connection = eth_connection.lock().await;

                            let fetched_block_with_memos =
                                fetch_cape_block(&connection, meta.transaction_hash)
                                    .await
                                    .unwrap()
                                    .unwrap();

                            let _transitions =
                                cape_block_to_transactions(fetched_block_with_memos.block.clone());

                            let input_record_commitment = fetched_block_with_memos
                                .block
                                .get_list_of_input_record_commitments();
                            for (_record_id, record_commitment) in
                                input_record_commitment.iter().enumerate()
                            {
                                dbg!(record_commitment);
                            }

                            let _wraps = filter_data
                                .deposit_commitments
                                .iter()
                                .map(|&rc| {
                                    rc.generic_into::<sol::RecordCommitmentSol>()
                                        .generic_into::<RecordCommitment>()
                                })
                                .collect_vec();
                            println!("blockcomm");
                        }
                        TestCAPEEvents::Erc20TokensDepositedFilter(filter_data) => {
                            let ro_bytes = filter_data.ro_bytes.clone();
                            let ro_sol: RecordOpeningSol = AbiDecode::decode(ro_bytes).unwrap();
                            let expected_ro = RecordOpening::from(ro_sol);

                            let erc20_code = Erc20Code(EthereumAddr(
                                filter_data.erc_20_token_address.to_fixed_bytes(),
                            ));

                            let _new_transition_wrap = CapeTransition::Wrap {
                                ro: Box::new(expected_ro),
                                erc20_code,
                                src_addr: EthereumAddr(filter_data.from.to_fixed_bytes()),
                            };
                            println!("erc20");
                        }
                    }
                    last_event = meta.block_number.as_u64();
                }
            }
        };

        let _memos_block_submitter = async {
            let params = vec![];
            let mut blocks_submitted = 0;
            while blocks_submitted < 2 {
                blocks_submitted += 1;
                let cape_block =
                    CapeBlock::generate(params.clone(), vec![], miner.address()).unwrap();
                let block_with_memos =
                    BlockWithMemos::new(cape_block.clone(), memos_with_sigs.clone());

                let cape_contract = eth_connection.lock().await;
                submit_cape_block_with_memos(&cape_contract.contract, block_with_memos.clone())
                    .await
                    .unwrap();
            }
        };

        let erc_20_tokens_deposited_submitter = async {
            //Approve
            let cape_contract = eth_connection.lock().await.test_contract();
            let deposited_amount = 1000u64;
            let amount_u256 = U256::from(deposited_amount);
            let contract_address = cape_contract.address();
            erc20_token_contract
                .approve(contract_address, amount_u256)
                .send()
                .await
                .unwrap()
                .await
                .unwrap();

            // Sponsor asset type
            let rng_sponsor = &mut ark_std::test_rng();
            let erc20_code = Erc20Code(EthereumAddr(
                contracts_info.erc20_token_address.to_fixed_bytes(),
            ));

            let sponsor = contracts_info.owner_of_erc20_tokens_client_address;

            let description =
                erc20_asset_description(&erc20_code, &EthereumAddr(sponsor.to_fixed_bytes()));
            let asset_code = AssetCode::new_foreign(&description);
            let asset_def =
                AssetDefinition::new(asset_code, AssetPolicy::rand_for_test(rng_sponsor)).unwrap();
            let asset_def_sol = asset_def.clone().generic_into::<sol::AssetDefinition>();

            contracts_info
                .cape_contract_for_erc20_owner
                .sponsor_cape_asset(contracts_info.erc20_token_address, asset_def_sol)
                .send()
                .await
                .unwrap()
                .await
                .unwrap();

            let ro = RecordOpening::new(
                &mut rng,
                deposited_amount,
                asset_def,
                UserPubKey::default(),
                FreezeFlag::Unfrozen,
            );
            // We call the CAPE contract from the address that owns the ERC20 tokens
            contracts_info
                .cape_contract_for_erc20_owner
                .deposit_erc_20(
                    ro.clone().generic_into::<sol::RecordOpening>(),
                    contracts_info.erc20_token_address,
                )
                .send()
                .await
                .unwrap()
                .await
                .unwrap();

            // Submit to the contract
            cape_contract
                .submit_cape_block(cape_block_erc20.into())
                .gas(U256::from(10_000_000))
                .send()
                .await
                .unwrap()
                .await
                .unwrap();

            println!("erc done and submitted");
        };
        let ((), ()) = futures::join!(
            //block_committed_event_listener,
            //erc_20_tokens_deposited_event_listener,
            events_listener,
            erc_20_tokens_deposited_submitter,
            //_memos_block_submitter
        );
        Ok(())
    }
}
