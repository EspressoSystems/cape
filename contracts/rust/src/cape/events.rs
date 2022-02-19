#[cfg(test)]
mod tests {
    use std::iter::repeat_with;

    use crate::{
        cape::CapeBlock,
        deploy::deploy_cape_test,
        ethereum::get_provider,
        ledger::CapeLedger,
        types::{self as sol, GenericInto, MerkleRootSol},
    };
    use anyhow::Result;
    use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
    use ethers::prelude::{Bytes, Middleware};
    use itertools::Itertools;
    use jf_cap::{
        keys::{UserKeyPair, UserPubKey},
        sign_receiver_memos,
        structs::{AssetDefinition, FreezeFlag, ReceiverMemo, RecordOpening},
        utils::TxnsParams,
        KeyPair, Signature,
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

        let mut memos_bytes: Vec<u8> = vec![];
        memos_with_sigs.serialize(&mut memos_bytes).unwrap();

        // Submit the block with memos
        contract
            .submit_cape_block_with_memos(cape_block.clone().into(), memos_bytes.into())
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

        // Decode the calldata (tx.input) into the function input types
        let (decoded_calldata_block, fetched_memos_bytes) = contract
            .decode::<(sol::CapeBlock, Bytes), _>("submitCapeBlockWithMemos", tx.input)
            .unwrap();

        let decoded_cape_block = CapeBlock::from(decoded_calldata_block);
        assert_eq!(decoded_cape_block, cape_block);

        let decoded_memos: Vec<(Vec<ReceiverMemo>, Signature)> =
            CanonicalDeserialize::deserialize(&fetched_memos_bytes.to_vec()[..]).unwrap();
        assert_eq!(decoded_memos, memos_with_sigs);

        Ok(())
    }
}
