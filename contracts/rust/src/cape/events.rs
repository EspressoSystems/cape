// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use crate::{cape::CapeBlock, types::BlockCommittedFilter};
use ethers::{abi::AbiDecode, prelude::AbiError};

pub fn decode_cape_block_from_event(block: BlockCommittedFilter) -> Result<CapeBlock, AbiError> {
    Ok(crate::types::CapeBlock {
        miner_addr: AbiDecode::decode(block.miner_addr)?,
        note_types: AbiDecode::decode(block.note_types)?,
        transfer_notes: AbiDecode::decode(block.transfer_notes)?,
        mint_notes: AbiDecode::decode(block.mint_notes)?,
        freeze_notes: AbiDecode::decode(block.freeze_notes)?,
        burn_notes: AbiDecode::decode(block.burn_notes)?,
    }
    .into())
}

#[cfg(test)]
mod tests {
    use crate::{
        assertion::EnsureMined,
        cape::{
            events::decode_cape_block_from_event,
            submit_block::{fetch_cape_memos, submit_cape_block_with_memos},
            BlockWithMemos, CapeBlock,
        },
        ethereum::EthConnection,
        ledger::CapeLedger,
        types::{GenericInto, MerkleRootSol},
    };
    use anyhow::Result;
    use ethers::prelude::BlockNumber;
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
    use std::iter::repeat_with;

    #[tokio::test]
    async fn test_fetch_cape_memos_from_event() -> Result<()> {
        let connection = EthConnection::for_test().await;

        let mut rng = ChaChaRng::from_seed([0x42u8; 32]);
        let params = TxnsParams::generate_txns(&mut rng, 1, 1, 1, CapeLedger::merkle_height());
        let miner = UserPubKey::default();

        let root = params.txns[0].merkle_root();

        let cape_block = CapeBlock::generate(params.txns, vec![], miner.address())?;

        connection
            .test_contract()
            .add_root(root.generic_into::<MerkleRootSol>().0)
            .send()
            .await?
            .await?
            .ensure_mined();

        // Adapted from seahorse
        // https://github.com/EspressoSystems/seahorse/blob/ace20bc5f1bcf5b88ca0562799b8e80e6c52e933/src/persistence.rs#L574
        // Generate some memos with default UserKeyPair
        let key_pair = UserKeyPair::default();
        let memos_with_sigs = repeat_with(|| {
            let memos = repeat_with(|| {
                let amount = rng.next_u64();
                let ro = RecordOpening::new(
                    &mut rng,
                    amount.into(),
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

        let block_with_memos = BlockWithMemos::new(cape_block.clone(), memos_with_sigs.clone());

        submit_cape_block_with_memos(
            &connection.contract,
            block_with_memos.clone(),
            BlockNumber::Latest,
            1_000_000, // extra gas. This transaction sometimes runs out of gas, reason unclear.
        )
        .await?
        .await?
        .ensure_mined();

        // A connection with a random wallet (for queries only)
        let query_connection = EthConnection::from_config_for_query(
            &format!("{:?}", connection.contract.address()), // 0x123...cdf
            &match std::env::var("CAPE_WEB3_PROVIDER_URL") {
                Ok(url) => url,
                Err(_) => "http://localhost:8545".to_string(),
            },
        );

        let events = query_connection
            .contract
            .block_committed_filter()
            .from_block(0u64)
            .query_with_meta()
            .await?;

        let (data, meta) = events[0].clone();

        let fetched_memos = fetch_cape_memos(&query_connection, meta.transaction_hash)
            .await?
            .unwrap();
        assert_eq!(fetched_memos, memos_with_sigs);

        let event_cape_block = decode_cape_block_from_event(data)?;
        assert_eq!(cape_block, event_cape_block);

        Ok(())
    }
}
