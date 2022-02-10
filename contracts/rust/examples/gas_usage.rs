use anyhow::Result;
use cap_rust_sandbox::{
    cape::CapeBlock,
    deploy::deploy_cape_test,
    ledger::CapeLedger,
    test_utils::PrintGas,
    types::{GenericInto, MerkleRootSol},
};
use jf_aap::{keys::UserPubKey, utils::TxnsParams};
use reef::Ledger;

#[tokio::main]
async fn main() -> Result<()> {
    let rng = &mut ark_std::test_rng();

    for (n_xfr, n_mint, n_freeze) in [
        (0, 0, 0),
        (1, 0, 0),
        (2, 0, 0),
        (0, 1, 0),
        (0, 2, 0),
        (0, 0, 1),
        (0, 0, 2),
    ] {
        let contract = deploy_cape_test().await;

        // Slow to run this each time
        let params =
            TxnsParams::generate_txns(rng, n_xfr, n_mint, n_freeze, CapeLedger::merkle_height());
        let miner = UserPubKey::default();

        if !params.txns.is_empty() {
            let root = params.txns[0].merkle_root();
            contract
                .add_root(root.generic_into::<MerkleRootSol>().0)
                .send()
                .await?
                .await?;
        }

        let cape_block = CapeBlock::generate(params.txns, vec![], miner.address())?;

        contract
            .submit_cape_block(cape_block.into())
            .send()
            .await?
            .await?
            .print_gas(&format!(
                "xfr {} mint {} freeze {}",
                n_xfr, n_mint, n_freeze
            ));
    }

    Ok(())
}
