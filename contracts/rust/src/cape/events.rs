use crate::{cape::CapeBlock, types as sol};
use anyhow::{Error, Result};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use ethers::prelude::{Bytes, Http, Middleware, Provider, TxHash};
use jf_cap::{structs::ReceiverMemo, Signature};

use crate::{deploy::EthMiddleware, types::TestCAPE};

// XXX Should also allow CAPE, not just TestCAPE
#[derive(Clone, Debug)]
pub struct EthConnection {
    pub contract: TestCAPE<EthMiddleware>,
    pub provider: Provider<Http>,
}

impl EthConnection {
    #[allow(dead_code)]
    pub fn new(contract: TestCAPE<EthMiddleware>, provider: Provider<Http>) -> Self {
        Self { contract, provider }
    }
}

type BlockMemos = Vec<(Vec<ReceiverMemo>, Signature)>;
#[derive(Clone, Debug)]
pub struct BlockWithMemos {
    pub block: CapeBlock,
    pub memos: BlockMemos,
}

impl BlockWithMemos {
    pub fn new(block: CapeBlock, memos: BlockMemos) -> Self {
        Self { block, memos }
    }
}

#[allow(dead_code)]
pub async fn fetch_cape_block(
    connection: &EthConnection,
    tx_hash: TxHash,
) -> Result<Option<BlockWithMemos>, Error> {
    let EthConnection { contract, provider } = connection;

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

#[allow(dead_code)]
pub async fn submit_cape_block_with_memos(
    contract: &TestCAPE<EthMiddleware>,
    block: BlockWithMemos,
) -> Result<()> {
    let mut memos_bytes: Vec<u8> = vec![];
    block.memos.serialize(&mut memos_bytes).unwrap();

    // Submit the block with memos
    contract
        .submit_cape_block_with_memos(block.block.clone().into(), memos_bytes.into())
        .send()
        .await?
        .await?;

    // XXX better to return the `PendingTransaction` (before last .await?) but
    // struggling with type checker if doing so.

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::iter::repeat_with;

    use crate::{
        deploy::deploy_cape_test,
        ethereum::get_provider,
        ledger::CapeLedger,
        types::{GenericInto, MerkleRootSol},
    };

    use itertools::Itertools;
    use jf_cap::KeyPair;
    use jf_cap::{
        keys::{UserKeyPair, UserPubKey},
        sign_receiver_memos,
        structs::{AssetDefinition, FreezeFlag, ReceiverMemo, RecordOpening},
        utils::TxnsParams,
    };
    use rand::{RngCore, SeedableRng};
    use rand_chacha::ChaChaRng;
    use reef::Ledger;

    #[tokio::test]
    async fn test_fetch_cape_block_from_event() -> Result<()> {
        let contract = deploy_cape_test().await;
        let mut rng = ChaChaRng::from_seed([0x42u8; 32]);
        let params = TxnsParams::generate_txns(&mut rng, 1, 0, 0, CapeLedger::merkle_height());
        let miner = UserPubKey::default();

        let root = params.txns[0].merkle_root();

        let cape_block = CapeBlock::generate(params.txns, vec![], miner.address())?;

        contract
            .add_root(root.generic_into::<MerkleRootSol>().0)
            .send()
            .await?
            .await?;

        // XXX Adapted from seahorse
        // https://github.com/SpectrumXYZ/seahorse/blob/ace20bc5f1bcf5b88ca0562799b8e80e6c52e933/src/persistence.rs#L574
        // Generate some memos with default UserKeyPair
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

        submit_cape_block_with_memos(
            &contract,
            BlockWithMemos::new(cape_block.clone(), memos_with_sigs.clone()),
        )
        .await?;

        let events = contract
            .block_committed_filter()
            .from_block(0u64)
            .query_with_meta()
            .await?;

        let (_, meta) = events[0].clone();

        let provider = get_provider();
        let connection = EthConnection::new(contract, provider);

        let BlockWithMemos { block, memos } = fetch_cape_block(&connection, meta.transaction_hash)
            .await?
            .unwrap();

        assert_eq!(block, cape_block);
        assert_eq!(memos, memos_with_sigs);

        Ok(())
    }
}
