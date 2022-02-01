#![cfg(test)]

use crate::assertion::Matcher;
use crate::cape::CapeBlock;
use crate::deploy::deploy_cape_test;
use crate::ledger::CapeLedger;
use crate::types as sol;
use crate::types::{GenericInto, MerkleRootSol, NullifierSol};
use anyhow::Result;
use ethers::prelude::U256;
use jf_aap::keys::UserPubKey;
use jf_aap::structs::{AssetCodeSeed, InternalAssetCode};
use jf_aap::utils::TxnsParams;
use rand::Rng;
use reef::Ledger;

#[tokio::test]
async fn test_compute_max_commitments() {
    let contract = deploy_cape_test().await;
    let rng = &mut ark_std::test_rng();

    for _run in 0..10 {
        let mut num_comms = 0;

        let burn_notes = (0..rng.gen_range(0..2))
            .map(|_| {
                let mut note = sol::BurnNote::default();
                let n = rng.gen_range(0..10);
                note.transfer_note.output_commitments = [U256::from(0)].repeat(n);
                num_comms += n;
                note
            })
            .collect();

        let transfer_notes = (0..rng.gen_range(0..2))
            .map(|_| {
                let mut note = sol::TransferNote::default();
                let n = rng.gen_range(0..10);
                note.output_commitments = [U256::from(0)].repeat(n);
                num_comms += n;
                note
            })
            .collect();

        let freeze_notes = (0..rng.gen_range(0..2))
            .map(|_| {
                let mut note = sol::FreezeNote::default();
                let n = rng.gen_range(0..10);
                note.output_commitments = [U256::from(0)].repeat(n);
                num_comms += n;
                note
            })
            .collect();

        let mint_notes = (0..rng.gen_range(0..2))
            .map(|_| {
                num_comms += 2; // change and mint
                sol::MintNote::default()
            })
            .collect();

        let cape_block = sol::CapeBlock {
            transfer_notes,
            mint_notes,
            freeze_notes,
            burn_notes,
            note_types: vec![],
            miner_addr: UserPubKey::default().address().into(),
        };

        let max_comms_sol = contract
            .compute_max_commitments(cape_block)
            .call()
            .await
            .unwrap();

        assert_eq!(max_comms_sol, U256::from(num_comms));
    }
}

#[tokio::test]
async fn test_submit_empty_block_to_cape_contract() -> Result<()> {
    let contract = deploy_cape_test().await;

    // Create an empty block transactions
    let rng = &mut ark_std::test_rng();
    let params = TxnsParams::generate_txns(rng, 0, 0, 0, CapeLedger::merkle_height());
    let miner = UserPubKey::default();

    let cape_block = CapeBlock::generate(params.txns, vec![], miner.address())?;

    // Submitting an empty block does not yield a reject from the contract
    contract
        .submit_cape_block(cape_block.into(), vec![])
        .send()
        .await?
        .await?;

    // The height is incremented anyways.
    assert_eq!(contract.block_height().call().await?, 1u64);

    Ok(())
}

#[tokio::test]
async fn test_submit_block_to_cape_contract() -> Result<()> {
    let contract = deploy_cape_test().await;

    // Create three transactions
    let rng = &mut ark_std::test_rng();
    let num_transfer_txn = 1;
    let num_mint_txn = 1;
    let num_freeze_txn = 1;
    let params = TxnsParams::generate_txns(
        rng,
        num_transfer_txn,
        num_mint_txn,
        num_freeze_txn,
        CapeLedger::merkle_height(),
    );
    let miner = UserPubKey::default();

    let nf = params.txns[0].nullifiers()[0];
    let root = params.txns[0].merkle_root();

    // temporarily no burn txn yet.
    let cape_block = CapeBlock::generate(params.txns, vec![], miner.address())?;

    // Check that some nullifier is not yet inserted
    assert!(
        !contract
            .nullifiers(nf.generic_into::<NullifierSol>().0)
            .call()
            .await?
    );

    // TODO should not require to manually submit the root here
    contract
        .add_root(root.generic_into::<MerkleRootSol>().0)
        .send()
        .await?
        .await?;

    // Submit to the contract
    contract
        .submit_cape_block(cape_block.into(), vec![])
        .send()
        .await?
        .await?;

    // Check that now the nullifier has been inserted
    assert!(
        contract
            .nullifiers(nf.generic_into::<NullifierSol>().0)
            .call()
            .await?
    );
    Ok(())
}

#[tokio::test]
async fn test_block_height() -> Result<()> {
    let contract = deploy_cape_test().await;
    assert_eq!(contract.block_height().call().await?, 0u64);

    let rng = &mut ark_std::test_rng();
    let params = TxnsParams::generate_txns(rng, 1, 0, 0, CapeLedger::merkle_height());
    let miner = UserPubKey::default();

    let root = params.txns[0].merkle_root();
    let cape_block = CapeBlock::generate(params.txns, vec![], miner.address())?;

    // TODO should not require to manually submit the root here
    contract
        .add_root(root.generic_into::<MerkleRootSol>().0)
        .send()
        .await?
        .await?;

    contract
        .submit_cape_block(cape_block.into(), vec![])
        .send()
        .await?
        .await?;

    assert_eq!(contract.block_height().call().await?, 1u64);
    Ok(())
}

#[tokio::test]
async fn test_event_block_committed() -> Result<()> {
    let contract = deploy_cape_test().await;

    let rng = &mut ark_std::test_rng();
    let params = TxnsParams::generate_txns(rng, 1, 0, 0, CapeLedger::merkle_height());
    let miner = UserPubKey::default();

    let root = params.txns[0].merkle_root();
    let cape_block = CapeBlock::generate(params.txns, vec![], miner.address())?;

    // TODO should not require to manually submit the root here
    contract
        .add_root(root.generic_into::<MerkleRootSol>().0)
        .send()
        .await?
        .await?;

    contract
        .submit_cape_block(cape_block.into(), vec![])
        .send()
        .await?
        .await?;

    let logs = contract
        .block_committed_filter()
        .from_block(0u64)
        .query()
        .await?;
    assert_eq!(logs[0].height, 1);

    Ok(())
}

#[tokio::test]
async fn test_check_domestic_asset_code_in_submit_cape_block() -> Result<()> {
    let contract = deploy_cape_test().await;
    let rng = &mut ark_std::test_rng();
    let params = TxnsParams::generate_txns(rng, 0, 1, 0, CapeLedger::merkle_height());

    contract
        .add_root(params.merkle_root.generic_into::<MerkleRootSol>().0)
        .send()
        .await?
        .await?;

    let mut block = CapeBlock::generate(params.txns, vec![], UserPubKey::default().address())?;

    // Set a wrong internal asset code on the mint note
    block.mint_notes[0].mint_internal_asset_code =
        InternalAssetCode::new(AssetCodeSeed::generate(rng), b"description");

    contract
        .submit_cape_block(block.into(), vec![])
        .call()
        .await
        .should_revert_with_message("Wrong domestic asset code");

    Ok(())
}
