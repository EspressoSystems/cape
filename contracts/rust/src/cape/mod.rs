// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

#![deny(warnings)]
mod events;
mod faucet;
mod note_types;
mod reentrancy;
pub mod submit_block;
mod wrapping;

use crate::model::CapeModelTxn;
use crate::types as sol;
use anyhow::{anyhow, bail, Result};
use ark_serialize::*;
use ethers::prelude::Address;
use itertools::Itertools;
use jf_cap::freeze::FreezeNote;
use jf_cap::keys::UserAddress;
use jf_cap::mint::MintNote;
use jf_cap::structs::{ReceiverMemo, RecordCommitment, RecordOpening};
use jf_cap::transfer::TransferNote;
use jf_cap::{Signature, TransactionNote};
use num_traits::{FromPrimitive, ToPrimitive};
use std::str::from_utf8;

pub const DOM_SEP_CAPE_BURN: &[u8] = b"EsSCAPE burn";
type BlockMemos = Vec<(Vec<ReceiverMemo>, Signature)>;

/// Burning transaction structure for a single asset (with fee)
#[derive(Debug, PartialEq, Eq, Hash, Clone, CanonicalSerialize, CanonicalDeserialize)]
pub struct BurnNote {
    /// Burn is effectively a transfer, this is the txn note.
    pub transfer_note: TransferNote,
    /// Record opening of the burned output (2nd in the transfer).
    pub burned_ro: RecordOpening,
}

impl BurnNote {
    /// Construct a `BurnNote` using the underlying transfer note and the burned
    /// record opening (namely of the second output)
    pub fn generate(note: TransferNote, burned_ro: RecordOpening) -> Result<Self> {
        if note.output_commitments.len() < 2
            || note.output_commitments[1] != RecordCommitment::from(&burned_ro)
            || note.aux_info.extra_proof_bound_data.len() != 32
            || !Self::is_burn_note(&note)
        {
            bail!("Malformed Burned Note parameters");
        }
        Ok(Self {
            transfer_note: note,
            burned_ro,
        })
    }

    /// Retrieve the Ethereum recipient address
    pub fn withdraw_recipient(&self) -> Result<Address> {
        from_utf8(&self.transfer_note.aux_info.extra_proof_bound_data[DOM_SEP_CAPE_BURN.len()..])?
            .parse::<Address>()
            .map_err(|_| anyhow!("Invalid Ethereum address!"))
    }

    /// utility function to check if a `TransferNote` is a `BurnNote`
    pub fn is_burn_note(note: &TransferNote) -> bool {
        note.aux_info
            .extra_proof_bound_data
            .starts_with(DOM_SEP_CAPE_BURN)
    }
}

impl From<BurnNote> for sol::BurnNote {
    fn from(note: BurnNote) -> Self {
        Self {
            transfer_note: note.transfer_note.into(),
            record_opening: note.burned_ro.into(),
        }
    }
}

impl From<sol::BurnNote> for BurnNote {
    fn from(note_sol: sol::BurnNote) -> Self {
        Self {
            transfer_note: note_sol.transfer_note.into(),
            burned_ro: note_sol.record_opening.into(),
        }
    }
}

/// A cape block containing a batch of transaction notes.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct CapeBlock {
    /// miner (a.k.a fee collector)
    pub miner_addr: UserAddress,
    /// the ordering of txn within the block
    pub note_types: Vec<NoteType>,
    /// sorted transfer notes
    pub transfer_notes: Vec<TransferNote>,
    /// sorted mint notes
    pub mint_notes: Vec<MintNote>,
    /// sorted freeze notes
    pub freeze_notes: Vec<FreezeNote>,
    /// sorted burn notes
    pub burn_notes: Vec<BurnNote>,
}

impl CapeBlock {
    /// Generate a CapeBlock
    pub fn generate(
        notes: Vec<TransactionNote>,
        burned_ros: Vec<RecordOpening>,
        miner: UserAddress,
    ) -> Result<Self> {
        let mut transfer_notes = vec![];
        let mut mint_notes = vec![];
        let mut freeze_notes = vec![];
        let mut burn_notes = vec![];
        let mut note_types = vec![];
        for note in notes {
            match note {
                TransactionNote::Transfer(n) => {
                    if BurnNote::is_burn_note(&n) {
                        burn_notes.push(*n);
                        note_types.push(NoteType::Burn);
                    } else {
                        transfer_notes.push(*n);
                        note_types.push(NoteType::Transfer);
                    }
                }
                TransactionNote::Mint(n) => {
                    mint_notes.push(*n);
                    note_types.push(NoteType::Mint);
                }
                TransactionNote::Freeze(n) => {
                    freeze_notes.push(*n);
                    note_types.push(NoteType::Freeze);
                }
            }
        }

        if burn_notes.len() != burned_ros.len() {
            bail!("Mismatched number of burned openings");
        }
        let burn_notes: Vec<BurnNote> = burn_notes
            .iter()
            .zip(burned_ros.iter())
            .map(|(note, ro)| BurnNote::generate(note.clone(), ro.clone()).unwrap())
            .collect();

        Ok(Self {
            miner_addr: miner,
            note_types,
            transfer_notes,
            mint_notes,
            freeze_notes,
            burn_notes,
        })
    }

    /// Collect the record commitments from the transaction outputs
    pub fn commitments(self) -> Vec<RecordCommitment> {
        let (txns, _) = self.into_cape_transactions().unwrap();
        txns.iter().flat_map(|tx| tx.commitments()).collect_vec()
    }

    pub fn from_cape_transactions(
        transactions: Vec<CapeModelTxn>,
        miner: UserAddress,
    ) -> Result<Self> {
        let mut burned_ros = vec![];
        let mut notes = vec![];

        for tx in transactions {
            match tx {
                CapeModelTxn::CAP(note) => notes.push(note),
                CapeModelTxn::Burn { xfr, ro } => {
                    notes.push(TransactionNote::from(*xfr));
                    burned_ros.push(*ro);
                }
            }
        }
        Self::generate(notes, burned_ros, miner)
    }
    pub fn into_cape_transactions(self) -> Result<(Vec<CapeModelTxn>, UserAddress)> {
        let mut transfer_notes = self.transfer_notes.into_iter().rev();
        let mut mint_notes = self.mint_notes.into_iter().rev();
        let mut freeze_notes = self.freeze_notes.into_iter().rev();
        let mut burn_notes = self.burn_notes.into_iter().rev();
        let txns: Option<Vec<CapeModelTxn>> = self
            .note_types
            .into_iter()
            .map(|note_type| match note_type {
                NoteType::Transfer => Some(CapeModelTxn::CAP(transfer_notes.next()?.into())),
                NoteType::Mint => Some(CapeModelTxn::CAP(mint_notes.next()?.into())),
                NoteType::Freeze => Some(CapeModelTxn::CAP(freeze_notes.next()?.into())),
                NoteType::Burn => {
                    let burn_note = burn_notes.next()?;
                    Some(CapeModelTxn::Burn {
                        xfr: Box::new(burn_note.transfer_note),
                        ro: Box::new(burn_note.burned_ro),
                    })
                }
            })
            .collect();
        Ok((
            txns.ok_or_else(|| anyhow!("Malformed CapeBlock"))?,
            self.miner_addr,
        ))
    }
}

impl From<CapeBlock> for sol::CapeBlock {
    fn from(blk: CapeBlock) -> Self {
        Self {
            miner_addr: blk.miner_addr.into(),
            note_types: blk
                .note_types
                .iter()
                .map(|t| t.to_u8().unwrap_or(0))
                .collect(),
            transfer_notes: blk
                .transfer_notes
                .iter()
                .map(|n| n.clone().into())
                .collect(),
            mint_notes: blk.mint_notes.iter().map(|n| n.clone().into()).collect(),
            freeze_notes: blk.freeze_notes.iter().map(|n| n.clone().into()).collect(),
            burn_notes: blk.burn_notes.iter().map(|n| n.clone().into()).collect(),
        }
    }
}

impl From<sol::CapeBlock> for CapeBlock {
    fn from(blk_sol: sol::CapeBlock) -> Self {
        Self {
            miner_addr: blk_sol.miner_addr.into(),
            note_types: blk_sol
                .note_types
                .iter()
                .map(|t| NoteType::from_u8(*t).unwrap_or(NoteType::Transfer))
                .collect(),
            transfer_notes: blk_sol
                .transfer_notes
                .iter()
                .map(|n| n.clone().into())
                .collect(),
            mint_notes: blk_sol
                .mint_notes
                .iter()
                .map(|n| n.clone().into())
                .collect(),
            freeze_notes: blk_sol
                .freeze_notes
                .iter()
                .map(|n| n.clone().into())
                .collect(),
            burn_notes: blk_sol
                .burn_notes
                .iter()
                .map(|n| n.clone().into())
                .collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BlockWithMemos {
    pub block: CapeBlock,
    pub memos: BlockMemos,
}

impl BlockWithMemos {
    pub fn new(block: CapeBlock, memos: BlockMemos) -> Self {
        Self { block, memos }
    }
}

/// Note type available in CAPE.
#[derive(FromPrimitive, ToPrimitive, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NoteType {
    Transfer,
    Mint,
    Freeze,
    Burn,
}

impl From<TransactionNote> for NoteType {
    fn from(note: TransactionNote) -> Self {
        match note {
            TransactionNote::Transfer(n) => {
                if BurnNote::is_burn_note(&n) {
                    Self::Burn
                } else {
                    Self::Transfer
                }
            }
            TransactionNote::Mint(_) => Self::Mint,
            TransactionNote::Freeze(_) => Self::Freeze,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CAPEConstructorArgs {
    height: u8,
    n_roots: u64,
    verifier_addr: Address,
}

#[allow(dead_code)]
impl CAPEConstructorArgs {
    pub fn new(height: u8, n_roots: u64, verifier_addr: Address) -> Self {
        Self {
            height,
            n_roots,
            verifier_addr,
        }
    }
}

impl From<CAPEConstructorArgs> for (u8, u64, Address) {
    fn from(args: CAPEConstructorArgs) -> (u8, u64, Address) {
        (args.height, args.n_roots, args.verifier_addr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assertion::Matcher;
    use crate::deploy::deploy_cape_test;
    use crate::ethereum::get_funded_client;
    use crate::ledger::CapeLedger;
    use crate::types::{GenericInto, MerkleRootSol, RecordCommitmentSol, TestCapeTypes};
    use anyhow::Result;
    use ethers::prelude::U256;
    use itertools::Itertools;
    use jf_cap::keys::UserKeyPair;
    use jf_cap::structs::RecordOpening;
    use jf_cap::utils::TxnsParams;
    use reef::Ledger;

    #[tokio::test]
    async fn test_batch_verify_validity_proof() -> Result<()> {
        let rng = &mut ark_std::test_rng();
        // Create a block with 3 transfer, 1 mint, 2 freeze
        let params = TxnsParams::generate_txns(rng, 3, 1, 2, CapeLedger::merkle_height());
        let miner = UserKeyPair::generate(rng);

        // simulate initial contract state to contain those record to be consumed
        let contract = deploy_cape_test().await;
        for root in params.txns.iter().map(|txn| txn.merkle_root()).unique() {
            contract
                .add_root(root.generic_into::<MerkleRootSol>().0)
                .send()
                .await?
                .await?;
        }

        // submit the block during which validity proofs would be verified in batch
        let cape_block = CapeBlock::generate(params.txns, vec![], miner.address())?;
        contract
            .submit_cape_block(cape_block.into())
            .send()
            .await?
            .await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_derive_record_commitment() {
        let contract = deploy_cape_test().await;
        let mut rng = ark_std::test_rng();

        for _run in 0..10 {
            let ro = RecordOpening::rand_for_test(&mut rng);
            let rc = RecordCommitment::from(&ro);

            let rc_u256 = contract
                .derive_record_commitment(ro.into())
                .call()
                .await
                .unwrap();

            assert_eq!(
                rc_u256
                    .generic_into::<RecordCommitmentSol>()
                    .generic_into::<RecordCommitment>(),
                rc
            );
        }
    }

    #[tokio::test]
    async fn test_derive_record_commitment_checks_reveal_map() {
        let contract = deploy_cape_test().await;
        let mut ro = sol::RecordOpening::default();
        ro.asset_def.policy.reveal_map = U256::from(2).pow(12.into());

        contract
            .derive_record_commitment(ro)
            .call()
            .await
            .should_revert_with_message("Reveal map exceeds 12 bits")
    }

    mod type_conversion {
        use super::*;
        use crate::deploy::deploy_test_cape_types_contract;
        use crate::types::{AssetCodeSol, GenericInto, InternalAssetCodeSol};
        use ark_bn254::{Bn254, Fr};
        use ark_std::UniformRand;
        use jf_cap::structs::{AssetCodeSeed, InternalAssetCode};
        use jf_cap::{
            freeze::FreezeNote,
            mint::MintNote,
            structs::{
                AssetCode, AssetDefinition, AssetPolicy, AuditMemo, Nullifier, RecordCommitment,
                RecordOpening,
            },
            utils::TxnsParams,
            BaseField, NodeValue,
        };
        use jf_plonk::proof_system::structs::Proof;
        use jf_primitives::elgamal;

        #[tokio::test]
        async fn test_nullifier() -> Result<()> {
            let rng = &mut ark_std::test_rng();
            let contract = deploy_test_cape_types_contract().await;
            for _ in 0..5 {
                let nf = Nullifier::random_for_test(rng);
                let res = contract
                    .check_nullifier(nf.generic_into::<sol::NullifierSol>().0)
                    .call()
                    .await?
                    .generic_into::<sol::NullifierSol>()
                    .generic_into::<Nullifier>();
                assert_eq!(nf, res);
            }
            Ok(())
        }

        #[tokio::test]
        async fn test_record_commitment() -> Result<()> {
            let rng = &mut ark_std::test_rng();
            let contract = deploy_test_cape_types_contract().await;
            for _ in 0..5 {
                let rc = RecordCommitment::from_field_element(BaseField::rand(rng));
                let res = contract
                    .check_record_commitment(rc.generic_into::<sol::RecordCommitmentSol>().0)
                    .call()
                    .await?
                    .generic_into::<sol::RecordCommitmentSol>()
                    .generic_into::<RecordCommitment>();
                assert_eq!(rc, res);
            }
            Ok(())
        }

        #[tokio::test]
        async fn test_merkle_root() -> Result<()> {
            let rng = &mut ark_std::test_rng();
            let contract = deploy_test_cape_types_contract().await;
            for _ in 0..5 {
                let root = NodeValue::rand(rng);
                let res = contract
                    .check_merkle_root(root.generic_into::<sol::MerkleRootSol>().0)
                    .call()
                    .await?
                    .generic_into::<sol::MerkleRootSol>()
                    .generic_into::<NodeValue>();
                assert_eq!(root, res);
            }
            Ok(())
        }

        #[tokio::test]
        async fn test_check_domestic_asset_code() -> Result<()> {
            let contract = deploy_cape_test().await;

            // Create a matching pair of codes
            let rng = &mut ark_std::test_rng();
            let description = b"cap_usdx";
            let seed = AssetCodeSeed::generate(rng);
            let internal_asset_code = InternalAssetCode::new(seed, description);
            let asset_code = AssetCode::new_domestic(seed, description);

            // Passes for matching asset codes
            contract
                .check_domestic_asset_code(
                    asset_code.generic_into::<AssetCodeSol>().0,
                    internal_asset_code.generic_into::<InternalAssetCodeSol>().0,
                )
                .call()
                .await
                .should_not_revert();

            // Fails with non-matching description
            contract
                .check_domestic_asset_code(
                    AssetCode::new_domestic(seed, b"other description")
                        .generic_into::<AssetCodeSol>()
                        .0,
                    internal_asset_code.generic_into::<InternalAssetCodeSol>().0,
                )
                .call()
                .await
                .should_revert_with_message("Wrong domestic asset code");

            // Fails for foreign asset code
            contract
                .check_domestic_asset_code(
                    AssetCode::new_foreign(description)
                        .generic_into::<AssetCodeSol>()
                        .0,
                    internal_asset_code.generic_into::<InternalAssetCodeSol>().0,
                )
                .call()
                .await
                .should_revert_with_message("Wrong domestic asset code");

            // Fails if internal asset code doesn't match (different seed)
            contract
                .check_domestic_asset_code(
                    asset_code.generic_into::<AssetCodeSol>().0,
                    InternalAssetCode::new(AssetCodeSeed::generate(rng), description)
                        .generic_into::<InternalAssetCodeSol>()
                        .0,
                )
                .call()
                .await
                .should_revert_with_message("Wrong domestic asset code");

            Ok(())
        }

        #[tokio::test]
        async fn test_asset_code() -> Result<()> {
            let rng = &mut ark_std::test_rng();
            let contract = deploy_test_cape_types_contract().await;
            for _ in 0..5 {
                let (ac, _) = AssetCode::random(rng);
                let res = contract
                    .check_merkle_root(ac.generic_into::<sol::AssetCodeSol>().0)
                    .call()
                    .await?
                    .generic_into::<sol::AssetCodeSol>()
                    .generic_into::<AssetCode>();
                assert_eq!(ac, res);
            }
            Ok(())
        }

        #[tokio::test]
        async fn test_asset_policy_and_definition() -> Result<()> {
            let rng = &mut ark_std::test_rng();
            let contract = deploy_test_cape_types_contract().await;

            // The native asset definition has a dedicated constructor.
            assert_eq!(
                AssetDefinition::native(),
                contract
                    .check_asset_definition(
                        AssetDefinition::native().generic_into::<sol::AssetDefinition>()
                    )
                    .call()
                    .await?
                    .generic_into::<AssetDefinition>(),
            );

            for _ in 0..5 {
                // NOTE: `sol::AssetPolicy` is from abigen! on contract,
                // it collides with `jf_cap::structs::AssetPolicy`
                let policy = AssetPolicy::rand_for_test(rng);
                assert_eq!(
                    policy.clone(),
                    contract
                        .check_asset_policy(policy.generic_into::<sol::AssetPolicy>())
                        .call()
                        .await?
                        .generic_into::<AssetPolicy>()
                );

                let asset_def = AssetDefinition::rand_for_test(rng);
                assert_eq!(
                    asset_def.clone(),
                    contract
                        .check_asset_definition(asset_def.generic_into::<sol::AssetDefinition>())
                        .call()
                        .await?
                        .generic_into::<AssetDefinition>()
                );
            }
            Ok(())
        }

        #[tokio::test]
        async fn test_record_opening() -> Result<()> {
            let rng = &mut ark_std::test_rng();
            let contract = deploy_test_cape_types_contract().await;
            for _ in 0..5 {
                // NOTE: `sol::RecordOpening` is from abigen! on contract,
                // it collides with `jf_cap::structs::RecordOpening`
                let ro = RecordOpening::rand_for_test(rng);
                let res = contract
                    .check_record_opening(ro.clone().generic_into::<sol::RecordOpening>())
                    .call()
                    .await?
                    .generic_into::<RecordOpening>();
                assert_eq!(ro.amount, res.amount);
                assert_eq!(ro.asset_def, res.asset_def);
                assert_eq!(ro.pub_key.address(), res.pub_key.address());
                assert_eq!(ro.pub_key.enc_key(), res.pub_key.enc_key());
                assert_eq!(ro.freeze_flag, res.freeze_flag);
                assert_eq!(ro.blind, res.blind);
            }
            Ok(())
        }

        #[tokio::test]
        async fn test_audit_memo() -> Result<()> {
            let rng = &mut ark_std::test_rng();
            let contract = deploy_test_cape_types_contract().await;
            for _ in 0..5 {
                let keypair = elgamal::KeyPair::generate(rng);
                let message = Fr::rand(rng);
                let ct = keypair.enc_key().encrypt(rng, &[message]);

                let audit_memo = AuditMemo::new(ct);
                assert_eq!(
                    audit_memo.clone(),
                    contract
                        .check_audit_memo(audit_memo.generic_into::<sol::AuditMemo>())
                        .call()
                        .await?
                        .generic_into::<AuditMemo>()
                );
            }
            Ok(())
        }

        #[tokio::test]
        async fn test_plonk_proof_and_txn_notes() -> Result<()> {
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

            let contract = deploy_test_cape_types_contract().await;
            for txn in params.txns {
                // reconnect with peer
                let client = get_funded_client().await?;
                let contract = TestCapeTypes::new(contract.address(), client);

                let proof = txn.validity_proof();
                assert_eq!(
                    proof.clone(),
                    contract
                        .check_plonk_proof(proof.into())
                        .call()
                        .await?
                        .generic_into::<Proof<Bn254>>()
                );

                match txn {
                    TransactionNote::Transfer(note) => {
                        assert_eq!(
                            *note.clone(),
                            contract
                                .check_transfer_note((*note).generic_into::<sol::TransferNote>())
                                .call()
                                .await?
                                .generic_into::<TransferNote>()
                        )
                    }
                    TransactionNote::Mint(note) => {
                        assert_eq!(
                            *note.clone(),
                            contract
                                .check_mint_note((*note).generic_into::<sol::MintNote>())
                                .call()
                                .await?
                                .generic_into::<MintNote>()
                        )
                    }
                    TransactionNote::Freeze(note) => {
                        assert_eq!(
                            *note.clone(),
                            contract
                                .check_freeze_note((*note).generic_into::<sol::FreezeNote>())
                                .call()
                                .await?
                                .generic_into::<FreezeNote>()
                        )
                    }
                }
            }

            Ok(())
        }

        #[tokio::test]
        async fn test_note_type() -> Result<()> {
            let contract = deploy_test_cape_types_contract().await;
            let invalid = 10;
            assert_eq!(
                contract
                    .check_note_type(NoteType::Transfer.to_u8().unwrap_or_else(|| invalid))
                    .call()
                    .await?,
                0u8
            );
            assert_eq!(
                contract
                    .check_note_type(NoteType::Mint.to_u8().unwrap_or_else(|| invalid))
                    .call()
                    .await?,
                1u8
            );
            assert_eq!(
                contract
                    .check_note_type(NoteType::Freeze.to_u8().unwrap_or_else(|| invalid))
                    .call()
                    .await?,
                2u8
            );

            assert_eq!(
                contract
                    .check_note_type(NoteType::Burn.to_u8().unwrap_or_else(|| invalid))
                    .call()
                    .await?,
                3u8
            );

            Ok(())
        }
    }
}
