#[cfg(test)]
mod tests {
    use crate::{
        cape::CapeBlock,
        deploy::deploy_cape_test,
        ethereum::get_provider,
        ledger::CapeLedger,
        types::{self as sol, GenericInto, MerkleRootSol},
    };
    use anyhow::Result;
    use ethers::prelude::Middleware;
    use jf_aap::{keys::UserPubKey, utils::TxnsParams};
    use reef::Ledger;

    #[tokio::test]
    async fn test_fetch_cape_block_from_event() -> Result<()> {
        let contract = deploy_cape_test().await;
        let rng = &mut ark_std::test_rng();
        let params = TxnsParams::generate_txns(rng, 1, 0, 0, CapeLedger::merkle_height());
        let miner = UserPubKey::default();

        let root = params.txns[0].merkle_root();

        let cape_block = CapeBlock::generate(params.txns, vec![], miner.address())?;

        contract
            .add_root(root.generic_into::<MerkleRootSol>().0)
            .send()
            .await?
            .await?;

        contract
            .submit_cape_block(cape_block.clone().into())
            .send()
            .await?
            .await?;

        // Fetch event with metadata
        let events = contract
            .block_committed_filter()
            .from_block(0u64)
            .query_with_meta()
            .await?;

        let (_, meta) = events[0].clone();

        let provider = get_provider();

        // Fetch Ethereum transaction that emitted event
        let tx = provider
            .get_transaction(meta.transaction_hash)
            .await?
            .unwrap();

        let decoded_calldata_block = contract
            .decode::<sol::CapeBlock, _>("submitCapeBlock", tx.input)
            .unwrap();

        let decoded_cape_block = CapeBlock::from(decoded_calldata_block);

        assert_eq!(decoded_cape_block, cape_block);

        Ok(())
    }
}
