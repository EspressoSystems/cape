// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

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
pub async fn fetch_cape_memos(
    connection: &EthConnection,
    tx_hash: TxHash,
) -> Result<Option<BlockMemos>, Error> {
    // Fetch Ethereum transaction that emitted event
    let tx = if let Some(tx) = connection.provider.get_transaction(tx_hash).await? {
        tx
    } else {
        return Ok(None); // This probably means no tx with this hash found.
    };

    // Decode the calldata (tx.input) into the function input types
    let (_, fetched_memos_bytes) = connection
        .contract
        .decode::<(sol::CapeBlock, Bytes), _>("submitCapeBlockWithMemos", tx.input)?;

    let decoded_memos: BlockMemos =
        CanonicalDeserialize::deserialize(&fetched_memos_bytes.to_vec()[..])?;

    Ok(Some(decoded_memos))
}

pub async fn submit_cape_block_with_memos(
    contract: &CAPE<EthMiddleware>,
    block: BlockWithMemos,
    block_number: BlockNumber,
    extra_gas: u64,
) -> Result<PendingTransaction<'_, Http>, SignerMiddlewareError<Provider<Http>, Wallet<SigningKey>>>
{
    let mut memos_bytes: Vec<u8> = vec![];
    block.memos.serialize(&mut memos_bytes).unwrap();

    // There is some nonce subtlety going on here, in what amounts to a simple
    //  `contract.submit_cape_block_with_memos(...).send().await`.
    //
    //  We must manually call `fill_transaction` in order pass a `BlockNumber`.
    //  `BlockNumber` can be an integer block number or "latest", "earliest" or
    //  "pending". The value is passed to `eth_getTransactionCount` and the
    //  return value of that call is used as the nonce of the Ethereum
    //  transaction.
    //
    //  See https://eth.wiki/json-rpc/API#eth_gettransactioncount for details
    //  about the Ethereum RPC endpoint.
    //
    //  With `BlockNumber::Latest` the nonce is calculated based on the last
    //  mined Ethereum block. This is also the default behaviour of
    //  `ContractCall.send`.
    //
    //  With `BlockNumber::Pending` pending Ethereum transactions will be
    //  included in the transaction count. This enables submitting new Ethereum
    //  transactions before the previous one is mined and therefore enables the
    //  relayer to submit more than a single Ethererum transaction per Ethereum
    //  block.
    //
    //  Note: using `BlockNumber::Pending` will still create duplicate nonces if
    //  called a second time before the previous transaction goes into the
    //  mempool of the node.
    let mut tx = contract
        .submit_cape_block_with_memos(block.block.clone().into(), memos_bytes.into())
        .tx
        .clone();

    contract
        .client()
        .fill_transaction(&mut tx, Some(block_number.into()))
        .await?;

    // The estimated gas cost can be too low. For example, if a deposit is made
    // in an earlier transaction in the same block the estimate would not include
    // the cost for crediting the deposit.
    //
    // Note that the CAPE contract calls out to ERC20 contracts which means the
    // gas usage of processing a burn note is potentially unbounded. Using
    // tokens whose transfer function far exceeds normal gas consumption is
    // currently not supported.
    //
    // TODO: mathis: it's a bit wasteful to download the entire block for this
    // but I don't know of another way to obtain the current block gas limit.
    let block = contract
        .client()
        .get_block(BlockNumber::Latest)
        .await?
        .unwrap();
    tx.set_gas(std::cmp::min(
        tx.gas().unwrap() + extra_gas,
        block.gas_limit,
    ));

    contract.client().send_transaction(tx, None).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        assertion::{EnsureMined, Matcher},
        cape::CapeBlock,
        deploy::deploy_test_cape,
        ethereum::GAS_LIMIT_OVERRIDE,
        ledger::CapeLedger,
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

        println!("adding root");
        // Set the root
        contract
            .add_root(root.generic_into::<MerkleRootSol>().0)
            .send()
            .await?
            .await?
            .ensure_mined();

        println!("submit");
        // Submit to the contract
        contract
            .submit_cape_block(cape_block.into())
            .gas(GAS_LIMIT_OVERRIDE) // runs out of gas with estimate
            .send()
            .await?
            .await?
            .ensure_mined();

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

        contract
            .submit_cape_block(cape_block.into())
            .call()
            .await
            .should_revert_with_message("Block must be non-empty");

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
            .await?
            .ensure_mined();

        contract
            .submit_cape_block(cape_block.into())
            .gas(GAS_LIMIT_OVERRIDE)
            .send()
            .await?
            .await?
            .ensure_mined();

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
            .await?
            .ensure_mined();

        contract
            .submit_cape_block(cape_block.into())
            .gas(GAS_LIMIT_OVERRIDE)
            .send()
            .await?
            .await?
            .ensure_mined();

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
