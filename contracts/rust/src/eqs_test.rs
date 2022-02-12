#[cfg(test)]
mod tests {
    use crate::cape::*;
    use crate::deploy::deploy_cape_test;
    use crate::ledger::CapeLedger;
    use reef::Ledger;

    use crate::types::GenericInto;

    use crate::types::MerkleRootSol;
    use jf_cap::keys::UserPubKey;
    use jf_cap::utils::TxnsParams;

    use async_std::sync::Mutex;

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
        //create Mutex for testing
        let contract = Mutex::new(deploy_cape_test().await);

        let rng = &mut ark_std::test_rng();
        let params = TxnsParams::generate_txns(rng, 1, 0, 0, CapeLedger::merkle_height());

        let root = params.txns[0].merkle_root();
        //add root
        let contract_lock = contract.lock().await;
        (*contract_lock)
            .add_root(root.generic_into::<MerkleRootSol>().0)
            .send()
            .await?
            .await?;
        drop(contract_lock);

        //event listener
        let event_listener = async {
            let mut number_events = 0;
            //TODO: better loop-stopping mechanism
            while number_events < 5 {
                let contract_lock = contract.lock().await;
                let new_entry = contract_lock
                //TODO: select over events once Erc20Deposited event is merged
                    .block_committed_filter()
                    .from_block(0u64)
                    .query()
                    .await
                    .unwrap();
                if new_entry.len() > number_events {
                    dbg!(&new_entry[(number_events)..]);
                    number_events = new_entry.len();
                }
            }
        };

        //block submitter
        let block_submitter = async {
            let params = vec![];
            let miner = UserPubKey::default();
            let mut blocks_submitted = 0;
            while blocks_submitted < 5 {
                blocks_submitted += 1;
                let cape_block =
                    CapeBlock::generate(params.clone(), vec![], miner.address()).unwrap();
                contract
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
        let ((), ()) = futures::join!(event_listener, block_submitter);
        Ok(())
    }
}
