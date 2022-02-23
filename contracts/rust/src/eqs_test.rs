#[cfg(test)]
mod tests {
    use crate::{
        cape::CapeBlock,
        deploy::{deploy_cape_test, deploy_erc20_token},
        ethereum::get_provider,
        ledger::CapeLedger,
        state::{erc20_asset_description, Erc20Code, EthereumAddr},
        test_utils::ContractsInfo,
        types::{self as sol, GenericInto, MerkleRootSol, TestCAPEEvents},
    };
    use async_std::sync::Mutex;
    use ethers::prelude::{Middleware, U256};
    use futures::FutureExt;
    use itertools::Itertools;
    use jf_cap::{
        keys::UserPubKey,
        structs::{
            AssetCode, AssetDefinition, AssetPolicy, FreezeFlag, RecordCommitment, RecordOpening,
        },
        utils::TxnsParams,
    };
    use reef::Ledger;

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub(crate) struct CAPEConstructorArgs {
        height: u8,
        n_roots: u64,
    }

    #[allow(dead_code)]
    impl CAPEConstructorArgs {
        pub(crate) fn new(height: u8, n_roots: u64) -> Self {
            Self { height, n_roots }
        }
    }

    impl From<CAPEConstructorArgs> for (u8, u64) {
        fn from(args: CAPEConstructorArgs) -> (u8, u64) {
            (args.height, args.n_roots)
        }
    }

    #[tokio::test]
    async fn eqs_test() -> anyhow::Result<()> {
        //create cape block with transaction for erc20dep
        let rng = &mut ark_std::test_rng();
        let num_transfer_txn = 1;
        let num_mint_txn = 0;
        let num_freeze_txn = 0;
        let params_erc20 = TxnsParams::generate_txns(
            rng,
            num_transfer_txn,
            num_mint_txn,
            num_freeze_txn,
            CapeLedger::merkle_height(),
        );
        let miner = UserPubKey::default();

        let cape_block_erc20 = CapeBlock::generate(params_erc20.txns, vec![], miner.address())?;

        let params_empty = TxnsParams::generate_txns(rng, 1, 0, 0, CapeLedger::merkle_height());

        //create Mutex for testing on depolyed CAPE contract
        let cape_contract_lock = Mutex::new(deploy_cape_test().await);

        // Deploy ERC20 token contract. The client deploying the erc20 token contract receives 1000 * 10**18 tokens
        let erc20_token_contract = deploy_erc20_token().await;

        let cape_contract = cape_contract_lock.lock().await;
        let contracts_info = ContractsInfo::new(&cape_contract, &erc20_token_contract).await;

        let root = params_empty.txns[0].merkle_root();

        //add root
        (cape_contract)
            .add_root(root.generic_into::<MerkleRootSol>().0)
            .send()
            .await?
            .await?;
        drop(cape_contract);

        let block_committed_event_listener = async {
            let mut number_events = 0;
            while number_events < 5 {
                let cape_contract = cape_contract_lock.lock().await;
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
                    let tx = provider
                        .get_transaction(meta.transaction_hash)
                        .await
                        .unwrap();

                    let decoded_calldata_block = cape_contract_lock
                        .lock()
                        .await
                        .decode::<sol::CapeBlock, _>("submitCapeBlock", tx.unwrap().input)
                        .unwrap();

                    let decoded_cape_block = CapeBlock::from(decoded_calldata_block);
                    let wraps = filter
                        .deposit_commitments
                        .iter()
                        .map(|&rc| {
                            rc.generic_into::<sol::RecordCommitmentSol>()
                                .generic_into::<RecordCommitment>()
                        })
                        .collect_vec();
                    let input_rc = decoded_cape_block.get_list_of_input_record_commitments();

                    number_events += 1;
                }
            }
        };

        let erc_20_tokens_deposited_event_listener = async {
            let mut number_events = 0;
            while number_events < 1 {
                let cape_contract = cape_contract_lock.lock().await;

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
            let mut last_block = 0;
            while last_block < 9 {
                let cape_contract = cape_contract_lock.lock().await;

                let new_event = cape_contract
                    .events()
                    .from_block(last_block as u64)
                    .query_with_meta()
                    .await
                    .unwrap();

                drop(cape_contract);

                while new_event.len() > last_block {
                    let (filter, meta) = new_event[last_block].clone();

                    match filter {
                        //TODO: make this useful
                        TestCAPEEvents::BlockCommittedFilter(filter_inside) => {
                            let provider = get_provider();

                            // Fetch Ethereum transaction that emitted event
                            let txs = provider
                                .get_transaction(meta.transaction_hash)
                                .await
                                .unwrap();

                            let decoded_calldata_block = cape_contract_lock
                                .lock()
                                .await
                                .decode::<sol::CapeBlock, _>("submitCapeBlock", txs.unwrap().input)
                                .unwrap();

                            let decoded_cape_block = CapeBlock::from(decoded_calldata_block);

                            let wraps = filter_inside
                                .deposit_commitments
                                .iter()
                                .map(|&rc| {
                                    rc.generic_into::<sol::RecordCommitmentSol>()
                                        .generic_into::<RecordCommitment>()
                                })
                                .collect_vec();
                            println!("here");
                        }
                        TestCAPEEvents::Erc20TokensDepositedFilter(_) => {}
                    }
                    last_block += 1;
                }
            }
        };

        let empty_block_submitter = async {
            let params = vec![];
            let mut blocks_submitted = 0;
            while blocks_submitted < 7 {
                blocks_submitted += 1;
                let cape_block =
                    CapeBlock::generate(params.clone(), vec![], miner.address()).unwrap();

                let cape_contract = cape_contract_lock.lock().await;
                cape_contract
                    .submit_cape_block(cape_block.into())
                    .send()
                    .await
                    .unwrap()
                    .await
                    .unwrap();
            }
        };

        let erc_20_tokens_deposited_submitter = async {
            //Test if events appear in order- submit empty block
            let params = vec![];
            let cape_block = CapeBlock::generate(params.clone(), vec![], miner.address()).unwrap();
            let cape_contract_1 = cape_contract_lock.lock().await;
            cape_contract_1
                .submit_cape_block(cape_block.into())
                .send()
                .await
                .unwrap()
                .await
                .unwrap();
            drop(cape_contract_1);

            //Approve
            let cape_contract = cape_contract_lock.lock().await;
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
                rng,
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
        };
        let ((), (), ()) = futures::join!(
            //block_committed_event_listener,
            //erc_20_tokens_deposited_event_listener,
            events_listener,
            erc_20_tokens_deposited_submitter,
            empty_block_submitter
        );
        Ok(())
    }
}
