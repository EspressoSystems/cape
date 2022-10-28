// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

#![cfg(test)]
#![deny(warnings)]

use crate::assertion::EnsureMined;
use crate::deploy::deploy_test_cape;
use crate::ethereum::GAS_LIMIT_OVERRIDE;
use crate::test_utils::keysets_for_test;
use crate::{
    cape::*,
    ledger::CapeLedger,
    model::{
        CapeContractState, CapeModelEthEffect, CapeModelEvent, CapeModelOperation, CapeModelTxn,
    },
    types::field_to_u256,
    types::{GenericInto, NullifierSol},
    universal_param::UNIVERSAL_PARAM,
};
use anyhow::Result;
use ethers::prelude::U256;
use jf_cap::keys::{UserKeyPair, UserPubKey};
use jf_cap::structs::{AssetDefinition, FreezeFlag, RecordCommitment, RecordOpening};
use jf_cap::transfer::{TransferNote, TransferNoteInput};
use jf_cap::AccMemberWitness;
use jf_cap::MerkleLeafProof;
use jf_cap::MerkleTree;
use jf_cap::TransactionNote;
use rand::SeedableRng;
use rand_chacha::ChaChaRng;
use reef::traits::Ledger as _;
use std::time::Instant;

#[tokio::test]
async fn test_2user_and_submit() -> Result<()> {
    test_2user_maybe_submit(true).await
}

// Test without submitting to make sure that the submission _should_
// succeed, and to narrow down test failures that only have to do with the
// contract interaction code.
#[tokio::test]
async fn test_2user_no_submit() -> Result<()> {
    test_2user_maybe_submit(false).await
}

async fn test_2user_maybe_submit(should_submit: bool) -> Result<()> {
    let now = Instant::now();

    println!("generating params");

    let mut prng = ChaChaRng::from_seed([0x8au8; 32]);

    let (prove_keys, verif_keys) = keysets_for_test(&UNIVERSAL_PARAM);

    println!("CRS set up: {}s", now.elapsed().as_secs_f32());
    let now = Instant::now();

    let contract = if should_submit {
        Some(deploy_test_cape().await)
    } else {
        None
    };

    println!("Contract set up: {}s", now.elapsed().as_secs_f32());
    let now = Instant::now();

    let alice_key = UserKeyPair::generate(&mut prng);
    let bob_key = UserKeyPair::generate(&mut prng);

    let coin = AssetDefinition::native();

    let alice_rec1 = RecordOpening::new(
        &mut prng,
        2u64.into(),
        coin.clone(),
        alice_key.pub_key(),
        FreezeFlag::Unfrozen,
    );

    let mut t = MerkleTree::new(CapeLedger::merkle_height()).unwrap();
    let alice_rec_comm = RecordCommitment::from(&alice_rec1);
    let alice_rec_field_elem = alice_rec_comm.to_field_element();
    t.push(alice_rec_field_elem);
    let alice_rec_path = t.get_leaf(0).expect_ok()?.1.path;
    assert_eq!(
        alice_rec_path.nodes.len(),
        CapeLedger::merkle_height() as usize
    );

    if let Some(contract) = contract.as_ref() {
        assert_eq!(contract.get_root_value().call().await?, U256::from(0u64));

        contract
            .set_initial_record_commitments(vec![field_to_u256(alice_rec_field_elem)])
            .send()
            .await?
            .await?
            .ensure_mined();

        let first_root = t.commitment().root_value;

        assert_eq!(contract.get_num_leaves().call().await?, U256::from(1u64));

        assert_eq!(
            contract.get_root_value().call().await?,
            field_to_u256(first_root.to_scalar())
        );

        assert!(
            contract
                .contains_root(field_to_u256(first_root.to_scalar()))
                .call()
                .await?
        );
    }

    println!("Tree set up: {}s", now.elapsed().as_secs_f32());
    let now = Instant::now();

    let first_root = t.commitment().root_value;

    let alice_rec_final = TransferNoteInput {
        ro: alice_rec1,
        owner_keypair: &alice_key,
        cred: None,
        acc_member_witness: AccMemberWitness {
            merkle_path: alice_rec_path.clone(),
            root: first_root,
            uid: 0,
        },
    };

    let mut wallet_merkle_tree = t.clone();
    let validator = CapeContractState::new(verif_keys, t);

    println!("Validator set up: {}s", now.elapsed().as_secs_f32());
    let now = Instant::now();

    MerkleTree::check_proof(
        validator.ledger.record_merkle_commitment.root_value,
        0,
        &MerkleLeafProof::new(alice_rec_field_elem, alice_rec_path),
    )
    .unwrap();

    println!("Merkle path checked: {}s", now.elapsed().as_secs_f32());
    let now = Instant::now();

    let ((txn1, _, _), bob_rec) = {
        let bob_rec = RecordOpening::new(
            &mut prng,
            1u64.into(), /* 1 less, for the transaction fee */
            coin,
            bob_key.pub_key(),
            FreezeFlag::Unfrozen,
        );

        let txn = TransferNote::generate_native(
            &mut prng,
            /* inputs:         */ vec![alice_rec_final],
            /* outputs:        */ &[bob_rec.clone()],
            /* fee:            */ 1u64.into(),
            /* valid_until:    */ 2,
            /* proving_key:    */ prove_keys.xfr.key_for_size(1, 2).unwrap(),
        )
        .unwrap();
        (txn, bob_rec)
    };

    println!("Transfer has {} outputs", txn1.output_commitments.len());
    println!("Transfer generated: {}s", now.elapsed().as_secs_f32());
    let now = Instant::now();

    let nullifiers = TransactionNote::Transfer(Box::new(txn1.clone())).nullifiers();

    if let Some(contract) = contract.as_ref() {
        for nf in nullifiers.iter() {
            assert!(
                !contract
                    .nullifiers(nf.clone().generic_into::<NullifierSol>().0)
                    .call()
                    .await?
            );
        }
    }

    let new_recs = txn1.output_commitments.to_vec();

    let txn1_cape = CapeModelTxn::CAP(TransactionNote::Transfer(Box::new(txn1)));

    let (new_state, effects) = validator
        .submit_operations(vec![CapeModelOperation::SubmitBlock(vec![
            txn1_cape.clone()
        ])])
        .unwrap();

    if let Some(contract) = contract.as_ref() {
        let miner = UserPubKey::default();
        let cape_block =
            CapeBlock::from_cape_transactions(vec![txn1_cape.clone()], miner.address())?;
        // Submit to the contract
        contract
            .submit_cape_block(cape_block.into())
            .gas(GAS_LIMIT_OVERRIDE)
            .send()
            .await?
            .await?
            .ensure_mined();
    }

    println!(
        "Transfer validated & applied: {}s",
        now.elapsed().as_secs_f32()
    );
    let now = Instant::now();

    assert_eq!(effects.len(), 1);
    if let CapeModelEthEffect::Emit(CapeModelEvent::BlockCommitted {
        wraps: wrapped_commitments,
        txns: filtered_txns,
    }) = effects[0].clone()
    {
        assert_eq!(wrapped_commitments, vec![]);
        assert_eq!(filtered_txns.len(), 1);
        assert_eq!(filtered_txns[0], txn1_cape);
    } else {
        panic!("Transaction emitted the wrong event!");
    }

    // Confirm that the ledger's merkle tree got updated in the way we
    // expect
    for r in new_recs {
        wallet_merkle_tree.push(r.to_field_element());
    }

    assert_eq!(
        new_state.ledger.record_merkle_commitment,
        wallet_merkle_tree.commitment()
    );

    if let Some(contract) = contract.as_ref() {
        assert_eq!(
            contract.get_root_value().call().await?,
            field_to_u256(
                new_state
                    .ledger
                    .record_merkle_commitment
                    .root_value
                    .to_scalar()
            )
        );

        for nf in nullifiers.iter() {
            assert!(
                contract
                    .nullifiers(nf.clone().generic_into::<NullifierSol>().0)
                    .call()
                    .await?
            );
        }
    }

    let ro = bob_rec;
    let merkle_path = wallet_merkle_tree.get_leaf(2).expect_ok()?.1.path;
    let merkle_root = new_state.ledger.record_merkle_commitment.root_value;
    let uid = 2;

    MerkleTree::check_proof(
        merkle_root,
        uid,
        &MerkleLeafProof::new(RecordCommitment::from(&ro).to_field_element(), merkle_path),
    )
    .unwrap();

    println!(
        "New record merkle path retrieved and checked: {}s",
        now.elapsed().as_secs_f32()
    );

    println!("Old state: {:?}", validator);
    println!("New state: {:?}", new_state);
    Ok(())
}
