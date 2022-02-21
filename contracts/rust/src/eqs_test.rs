#[cfg(test)]
mod tests {
    use crate::{
        assertion::EnsureMined,
        cape::CapeBlock,
        deploy::{deploy_cape_test, deploy_erc20_token},
        ethereum::get_provider,
        ledger::CapeLedger,
        state::{erc20_asset_description, CapeEvent, Erc20Code, EthereumAddr},
        test_utils::ContractsInfo,
        types::{self as sol, GenericInto, MerkleRootSol},
    };
    use async_std::sync::Mutex;
    use ethers::prelude::{Middleware, U256};
    use futures::{pin_mut, select, FutureExt};
    use itertools::Itertools;
    use jf_cap::{
        keys::UserPubKey,
        structs::{
            AssetCode, AssetDefinition, AssetPolicy, FreezeFlag, RecordCommitment, RecordOpening,
        },
        utils::TxnsParams,
    };
    use reef::{traits::Transaction, Ledger};

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
        //let root = params.txns[0].merkle_root();

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

        //event listener

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
                dbg!(new_entry.clone());

                drop(cape_contract);

                while new_entry.len() > number_events {
                    dbg!(new_entry[0].clone());
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
                    dbg!(input_rc);
                    //eqs.handle_event(CapeEvent::BlockCommitted{wraps: wraps, txns: txns});

                    number_events += 1;
                }
            }
        };

        /*let erc20_deposited_event_listener = async {
            let mut number_events = 0;
            while number_events < 5 {
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

        };*/

        //block submitter
        let empty_block_submitter = async {
            let params = vec![];
            let miner = UserPubKey::default();
            let mut blocks_submitted = 0;
            while blocks_submitted < 5 {
                blocks_submitted += 1;
                let cape_block =
                    CapeBlock::generate(params.clone(), vec![], miner.address()).unwrap();
                cape_contract_lock
                    .lock()
                    .await
                    .submit_cape_block(cape_block.into())
                    .send()
                    .await
                    .unwrap()
                    .await
                    .unwrap();
            }
        };

        /*let erc_20_deposited_submitter = async {

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

            // Call CAPE contract function

            // Sponsor asset type
            let rng_sponsor  = &mut ark_std::test_rng();
            let erc20_code = Erc20Code(EthereumAddr(
                contracts_info.erc20_token_address.to_fixed_bytes(),
            ));

            let sponsor = contracts_info.owner_of_erc20_tokens_client_address;

            let description = erc20_asset_description(&erc20_code, &EthereumAddr(sponsor.to_fixed_bytes()));
            let asset_code = AssetCode::new_foreign(&description);
            let asset_def = AssetDefinition::new(asset_code, AssetPolicy::rand_for_test(rng_sponsor)).unwrap();
            let asset_def_sol = asset_def.clone().generic_into::<sol::AssetDefinition>();

            let ro = RecordOpening::new(
                rng,
                deposited_amount,
                asset_def,
                UserPubKey::default(),
                FreezeFlag::Unfrozen,
            );
            // We call the CAPE contract from the address that owns the ERC20 tokens
            println!("Depositing tokens");
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


            println!("Submitting block");
            // Submit to the contract
            cape_contract_lock
                .lock()
                .await
                .submit_cape_block(cape_block_erc20.into())
                .gas(U256::from(10_000_000))
                .send()
                .await
                .unwrap()
                .await
                .unwrap()
                .ensure_mined();


            //Ok(())
        };*/
        //let ((), (), ()) = futures::join!(block_committed_event_listener, erc20_deposited_event_listener, empty_block_submitter);
        let ((), ()) = futures::join!(block_committed_event_listener, empty_block_submitter);
        Ok(())
    }
}
