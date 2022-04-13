// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use crate::cape::CapeBlock;
use crate::deploy::EthMiddleware;
use crate::ethereum::EthConnection;
use crate::types::{self as sol, CAPE};

use anyhow::{Error, Result};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use ethers::prelude::signer::SignerMiddlewareError;
use ethers::prelude::{BlockNumber, Provider, Wallet};
use ethers::prelude::{Bytes, Http, Middleware, PendingTransaction, TxHash};
use ethers_core::k256::ecdsa::SigningKey;

use super::{BlockMemos, BlockWithMemos};

/// Fetch a cape block given the (Ethereum) tx hash of the tx in which the block
/// was submitted.
pub async fn fetch_cape_block(
    connection: &EthConnection,
    tx_hash: TxHash,
) -> Result<Option<BlockWithMemos>, Error> {
    // Fetch Ethereum transaction that emitted event
    let tx = if let Some(tx) = connection.provider.get_transaction(tx_hash).await? {
        tx
    } else {
        return Ok(None); // This probably means no tx with this hash found.
    };

    // Decode the calldata (tx.input) into the function input types
    let (decoded_calldata_block, fetched_memos_bytes) =
        connection
            .contract
            .decode::<(sol::CapeBlock, Bytes), _>("submitCapeBlockWithMemos", tx.input)?;

    let decoded_cape_block = CapeBlock::from(decoded_calldata_block);
    let decoded_memos: BlockMemos =
        CanonicalDeserialize::deserialize(&fetched_memos_bytes.to_vec()[..])?;

    Ok(Some(BlockWithMemos::new(decoded_cape_block, decoded_memos)))
}

pub async fn submit_cape_block_with_memos(
    contract: &CAPE<EthMiddleware>,
    block: BlockWithMemos,
    block_number: BlockNumber,
) -> Result<PendingTransaction<'_, Http>, SignerMiddlewareError<Provider<Http>, Wallet<SigningKey>>>
{
    let mut memos_bytes: Vec<u8> = vec![];
    block.memos.serialize(&mut memos_bytes).unwrap();

    // There is some nonce subtlety going on here, in what amounts to a simple
    //  `contract.submit_cape_block_with_memos(...).send().await`.
    //
    //  We must manually call `fill_transaction` in order pass a `BlockNumber`.
    //  The value is passed to `eth_getTransactionCount` to calculate the nonce.
    //
    //  For `BlockNumber::Latest` the nonce is calculated based on the last
    //  mined block. This is also the default behaviour of `CotractCall.send`.
    //
    //  For `BlockNumber::Pending` pending transactions will be included. This
    //  allow to submit new transactions before the previous one is mined and
    //  therefore enables the relayer to submit more than a single txn per
    //  block. Note: this would still create duplicate nonces if called a second time
    //  before the previous transaction goes into the mempool of the node.
    let mut tx = contract
        .submit_cape_block_with_memos(block.block.clone().into(), memos_bytes.into())
        .tx
        .clone();
    contract
        .client()
        .fill_transaction(&mut tx, Some(block_number.into()))
        .await?;
    Ok(contract.client().send_transaction(tx, None).await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        assertion::Matcher,
        deploy::deploy_test_cape,
        ledger::CapeLedger,
        test_utils::PrintGas,
        types::{GenericInto, MerkleRootSol, NullifierSol},
    };
    use ark_ff::Fp256;
    use ethers::prelude::U256;
    use jf_cap::{
        keys::UserPubKey,
        structs::{AssetCodeSeed, InternalAssetCode, RecordCommitment},
        utils::TxnsParams,
        TransactionNote,
    };
    use num_traits::Zero;
    use rand::Rng;
    use reef::Ledger;

    #[tokio::test]
    async fn test_compute_num_commitments() {
        let contract = deploy_test_cape().await;
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

    async fn submit_block_test_helper(
        num_transfer_tx: usize,
        num_mint_tx: usize,
        num_freeze_tx: usize,
    ) -> Result<()> {
        let contract = deploy_test_cape().await;

        let rng = &mut ark_std::test_rng();

        let params = TxnsParams::generate_txns(
            rng,
            num_transfer_tx,
            num_mint_tx,
            num_freeze_tx,
            CapeLedger::merkle_height(),
        );
        let miner = UserPubKey::default();

        let nf = params.txns[0].nullifiers()[0];
        let root = params.txns[0].merkle_root();

        let cape_block = CapeBlock::generate(params.txns.clone(), vec![], miner.address())?;

        // Check that some nullifier is not yet inserted
        assert!(
            !contract
                .nullifiers(nf.generic_into::<NullifierSol>().0)
                .call()
                .await?
        );

        // Set the root
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
            .await?;

        // Check that now the nullifier has been inserted
        assert!(
            contract
                .nullifiers(nf.generic_into::<NullifierSol>().0)
                .call()
                .await?
        );

        // Now alter the transaction so that it is invalid, resubmit the block and check it is rejected
        let zero_rc = RecordCommitment::from_field_element(Fp256::zero());

        let tx_note = params.txns[0].clone();
        let altered_tx_note = match tx_note {
            TransactionNote::Transfer(tx) => {
                let mut altered_tx = tx.clone();
                altered_tx.output_commitments[0] = zero_rc;
                TransactionNote::Transfer(altered_tx.clone())
            }
            TransactionNote::Mint(tx) => {
                let mut altered_tx = tx.clone();
                altered_tx.mint_comm = zero_rc;
                TransactionNote::Mint(altered_tx.clone())
            }
            TransactionNote::Freeze(tx) => {
                let mut altered_tx = tx.clone();
                altered_tx.output_commitments[0] = zero_rc;
                TransactionNote::Freeze(altered_tx.clone())
            }
        };

        // We redeploy the contract so that we start with a clean state.
        let contract = deploy_test_cape().await;

        let cape_block = CapeBlock::generate(vec![altered_tx_note], vec![], miner.address())?;

        // Set the root
        contract
            .add_root(root.generic_into::<MerkleRootSol>().0)
            .send()
            .await?
            .await?;

        // Submit to the contract
        contract
            .submit_cape_block(cape_block.into())
            .call()
            .await
            .should_revert_with_message("Cape: batch verify failed");

        Ok(())
    }

    #[tokio::test]
    async fn test_submit_block_with_transfer_tx() -> Result<()> {
        submit_block_test_helper(1, 0, 0).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_submit_block_with_mint_tx() -> Result<()> {
        submit_block_test_helper(0, 1, 0).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_submit_block_with_freeze_tx() -> Result<()> {
        submit_block_test_helper(0, 0, 1).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_submit_block_with_three_txs_to_cape_contract() -> Result<()> {
        submit_block_test_helper(1, 1, 1).await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_submit_empty_block_to_cape_contract() -> Result<()> {
        let contract = deploy_test_cape().await;

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
    async fn test_block_height() -> Result<()> {
        let contract = deploy_test_cape().await;
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
        let contract = deploy_test_cape().await;

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
        let contract = deploy_test_cape().await;
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
}
