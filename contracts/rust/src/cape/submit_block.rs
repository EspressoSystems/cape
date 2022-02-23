#![cfg(test)]

use crate::assertion::Matcher;
use crate::cape::CapeBlock;
use crate::deploy::{deploy_cape_test, EthMiddleware};
use crate::ethereum::EthConnection;
use crate::ledger::CapeLedger;
use crate::test_utils::PrintGas;
use crate::types::{self as sol, CAPE};
use crate::types::{GenericInto, MerkleRootSol, NullifierSol};
use anyhow::{Error, Result};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use ethers::prelude::{Bytes, Middleware, TxHash, U256};
use jf_cap::keys::UserPubKey;
use jf_cap::structs::{AssetCodeSeed, InternalAssetCode};
use jf_cap::utils::TxnsParams;
use rand::Rng;
use reef::Ledger;

use super::{BlockMemos, BlockWithMemos};

pub async fn fetch_cape_block(
    connection: &EthConnection,
    tx_hash: TxHash,
) -> Result<Option<BlockWithMemos>, Error> {
    let EthConnection {
        contract, provider, ..
    } = connection;

    // Fetch Ethereum transaction that emitted event
    let tx = if let Some(tx) = provider.get_transaction(tx_hash).await? {
        tx
    } else {
        return Ok(None); // This probably means no tx with this hash found.
    };

    // Decode the calldata (tx.input) into the function input types
    let (decoded_calldata_block, fetched_memos_bytes) =
        contract.decode::<(sol::CapeBlock, Bytes), _>("submitCapeBlockWithMemos", tx.input)?;

    let decoded_cape_block = CapeBlock::from(decoded_calldata_block);

    let decoded_memos: BlockMemos =
        CanonicalDeserialize::deserialize(&fetched_memos_bytes.to_vec()[..])?;

    Ok(Some(BlockWithMemos::new(decoded_cape_block, decoded_memos)))
}

pub async fn submit_cape_block_with_memos(
    contract: &CAPE<EthMiddleware>,
    block: BlockWithMemos,
) -> Result<()> {
    let mut memos_bytes: Vec<u8> = vec![];
    block.memos.serialize(&mut memos_bytes).unwrap();

    contract
        .submit_cape_block_with_memos(block.block.clone().into(), memos_bytes.into())
        .send()
        .await?
        .await?;

    // It would be better to return the `PendingTransaction` (before the last
    // .await?) so that the caller can monitor the transaction mining. However
    // there are some challenges with the typechecker when doing so.

    Ok(())
}

#[tokio::test]
async fn test_compute_num_commitments() {
    let contract = deploy_cape_test().await;
    let rng = &mut ark_std::test_rng();

    for _run in 0..10 {
        let mut num_comms = 0;

        let burn_notes = (0..rng.gen_range(0..2))
            .map(|_| {
                let mut note = sol::BurnNote::default();
                let n = rng.gen_range(2..10); // burn notes must have a least 2 record commitments
                note.transfer_note.output_commitments = [U256::from(0)].repeat(n);
                // subtract one because the burn record commitment is not inserted
                num_comms += n - 1;
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

        let num_comms_sol = contract
            .compute_num_commitments(cape_block)
            .call()
            .await
            .unwrap();

        assert_eq!(num_comms_sol, U256::from(num_comms));
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
        .submit_cape_block(cape_block.into())
        .send()
        .await?
        .await?
        .print_gas("Submit empty block");

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

    contract
        .add_root(root.generic_into::<MerkleRootSol>().0)
        .send()
        .await?
        .await?;

    // Submit to the contract
    contract
        .submit_cape_block(cape_block.into())
        .send()
        .await?
        .await?
        .print_gas("Submit transfer + mint + freeze");

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

    contract
        .add_root(root.generic_into::<MerkleRootSol>().0)
        .send()
        .await?
        .await?;

    contract
        .submit_cape_block(cape_block.into())
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

    contract
        .add_root(root.generic_into::<MerkleRootSol>().0)
        .send()
        .await?
        .await?;

    contract
        .submit_cape_block(cape_block.into())
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
        .submit_cape_block(block.into())
        .call()
        .await
        .should_revert_with_message("Wrong domestic asset code");

    Ok(())
}
