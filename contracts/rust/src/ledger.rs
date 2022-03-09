// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use crate::model::*;
use arbitrary::{Arbitrary, Unstructured};
use arbitrary_wrappers::*;
use ark_serialize::*;
use commit::{Commitment, Committable, RawCommitmentBuilder};
use espresso_macros::ser_test;
use jf_cap::{
    keys::{AuditorKeyPair, AuditorPubKey},
    structs::{AssetCode, AssetDefinition, Nullifier, RecordCommitment, RecordOpening},
    TransactionNote,
};
use jf_utils::tagged_blob;
use reef::{cap, traits::*, AuditError, AuditMemoOpening};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Display;
use std::iter::repeat;

// A representation of an unauthenticated sparse set of nullifiers (it is "authenticated" by
// querying the ultimate source of truth, the CAPE smart contract). The HashMap maps any nullifier
// to one of 3 states:
//  * Some(true): definitely in the set
//  * Some(false): definitely not in the set
//  * None: outside the sparse domain of this set, query a full node for a definitive answer
#[ser_test(arbitrary, ark(false))]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CapeNullifierSet(HashMap<Nullifier, bool>);

impl CapeNullifierSet {
    pub fn get(&self, n: Nullifier) -> Option<bool> {
        self.0.get(&n).cloned()
    }

    pub fn insert(&mut self, n: Nullifier, value: bool) {
        self.0.insert(n, value);
    }
}

impl NullifierSet for CapeNullifierSet {
    type Proof = ();

    fn multi_insert(&mut self, nullifiers: &[(Nullifier, Self::Proof)]) -> Result<(), Self::Proof> {
        for (n, _) in nullifiers {
            self.0.insert(*n, true);
        }
        Ok(())
    }
}

impl<'a> Arbitrary<'a> for CapeNullifierSet {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let m: HashMap<ArbitraryNullifier, bool> = u.arbitrary()?;
        Ok(Self(m.into_iter().map(|(k, v)| (k.into(), v)).collect()))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, strum_macros::Display)]
pub enum CapeTransactionKind {
    CAP(cap::TransactionKind),
    Burn,
    Wrap,
}

impl TransactionKind for CapeTransactionKind {
    fn send() -> Self {
        Self::CAP(cap::TransactionKind::send())
    }

    fn receive() -> Self {
        Self::CAP(cap::TransactionKind::receive())
    }

    fn mint() -> Self {
        Self::CAP(cap::TransactionKind::mint())
    }

    fn freeze() -> Self {
        Self::CAP(cap::TransactionKind::freeze())
    }

    fn unfreeze() -> Self {
        Self::CAP(cap::TransactionKind::unfreeze())
    }

    fn unknown() -> Self {
        Self::CAP(cap::TransactionKind::unknown())
    }
}

// CapeTransition models all of the objects which can transition a CAPE ledger. This includes
// transactions, submitted from users to the validator via the relayer, as well as ERC20 wrap
// operations, which are submitted directly to the contract but whose outputs end up being included
// in the next committed block as well a call to the faucet function of the CAPE contract.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CapeTransition {
    Transaction(CapeModelTxn),
    Wrap {
        erc20_code: Erc20Code,
        src_addr: EthereumAddr,
        ro: Box<RecordOpening>,
    },
}

impl Committable for CapeTransition {
    fn commit(&self) -> Commitment<Self> {
        RawCommitmentBuilder::new("CapeTransition")
            .var_size_bytes(&bincode::serialize(self).unwrap())
            .finalize()
    }
}

impl Transaction for CapeTransition {
    type NullifierSet = CapeNullifierSet;
    type Hash = Commitment<Self>;
    type Kind = CapeTransactionKind;

    fn cap(note: TransactionNote, _proofs: Vec<()>) -> Self {
        Self::Transaction(CapeModelTxn::CAP(note))
    }

    fn open_audit_memo(
        &self,
        assets: &HashMap<AssetCode, AssetDefinition>,
        keys: &HashMap<AuditorPubKey, AuditorKeyPair>,
    ) -> Result<AuditMemoOpening, AuditError> {
        match self {
            Self::Transaction(CapeModelTxn::CAP(note)) => note.open_audit_memo(assets, keys),
            Self::Transaction(CapeModelTxn::Burn { xfr, .. }) => {
                cap::open_xfr_audit_memo(assets, keys, xfr)
            }
            _ => Err(AuditError::NoAuditMemos),
        }
    }

    fn proven_nullifiers(&self) -> Vec<(Nullifier, ())> {
        let nullifiers = match self {
            Self::Transaction(txn) => txn.nullifiers(),
            Self::Wrap { .. } => Vec::new(),
        };
        nullifiers.into_iter().zip(repeat(())).collect()
    }

    fn output_commitments(&self) -> Vec<RecordCommitment> {
        match self {
            Self::Transaction(txn) => txn.commitments(),
            Self::Wrap { ro, .. } => vec![RecordCommitment::from(&**ro)],
        }
    }

    fn output_openings(&self) -> Option<Vec<RecordOpening>> {
        match self {
            Self::Wrap { ro, .. } => Some(vec![(**ro).clone()]),
            _ => None,
        }
    }

    fn hash(&self) -> Self::Hash {
        self.commit()
    }

    fn kind(&self) -> CapeTransactionKind {
        match self {
            Self::Transaction(CapeModelTxn::CAP(txn)) => match txn {
                TransactionNote::Transfer(..) => CapeTransactionKind::send(),
                TransactionNote::Mint(..) => CapeTransactionKind::mint(),
                TransactionNote::Freeze(..) => CapeTransactionKind::freeze(),
            },
            Self::Transaction(CapeModelTxn::Burn { .. }) => CapeTransactionKind::Burn,
            Self::Wrap { .. } => CapeTransactionKind::Wrap,
        }
    }

    fn set_proofs(&mut self, _proofs: Vec<()>) {}
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommittedCapeTransition {
    pub block_id: u64,
    pub txn_id: u64,
    pub output_start: u64,
    pub output_size: u64,
    pub transition: CapeTransition,
}

#[tagged_blob("CMTMNT_CAPE_TRNSTN")]
#[derive(CanonicalSerialize, CanonicalDeserialize, Debug, Clone)]
pub struct CommitmentToCapeTransition(pub Commitment<CapeTransition>);

impl ValidationError for CapeValidationError {
    fn new(msg: impl Display) -> Self {
        Self::Failed {
            msg: msg.to_string(),
        }
    }

    fn is_bad_nullifier_proof(&self) -> bool {
        // CAPE doesn't have nullifier proofs, so validation never fails due to a bad one.
        false
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapeBlock(Vec<CapeTransition>);

impl Committable for CapeBlock {
    fn commit(&self) -> Commitment<Self> {
        RawCommitmentBuilder::new("CapeBlock")
            .array_field(
                "txns",
                &self.0.iter().map(|x| x.commit()).collect::<Vec<_>>(),
            )
            .finalize()
    }
}

impl Block for CapeBlock {
    type Transaction = CapeTransition;
    type Error = CapeValidationError;

    fn new(txns: Vec<CapeTransition>) -> Self {
        Self(txns)
    }

    fn txns(&self) -> Vec<CapeTransition> {
        self.0.clone()
    }

    fn add_transaction(&mut self, txn: CapeTransition) -> Result<(), CapeValidationError> {
        self.0.push(txn);
        Ok(())
    }
}

// In CAPE, we don't do local lightweight validation to check the results of queries. We trust the
// results of Ethereum query services, and our local validator stores just enough information to
// satisfy the Validator interface required by the wallet. Thus, the CAPE integration for the
// Validator interface is actually more Truster than Validator.
#[ser_test(arbitrary, ark(false))]
#[derive(Arbitrary, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapeTruster {
    // The current timestamp. The only requirement is that this is a monotonically increasing value,
    // but in this implementation it tracks the number of blocks committed.
    now: u64,
    // Number of records, for generating new UIDs.
    num_records: u64,
}

impl CapeTruster {
    pub fn new(now: u64, num_records: u64) -> Self {
        Self { now, num_records }
    }
}

impl Validator for CapeTruster {
    type StateCommitment = u64;
    type Block = CapeBlock;

    fn now(&self) -> u64 {
        self.now
    }

    fn commit(&self) -> Self::StateCommitment {
        // Our commitment is just the block height of the ledger. Since we are trusting a query
        // service anyways, this can be used to determine a unique ledger state by querying for the
        // state of the ledger at this block index.
        self.now
    }

    fn validate_and_apply(&mut self, block: Self::Block) -> Result<Vec<u64>, CapeValidationError> {
        // We don't actually do validation here, since in this implementation we trust the query
        // service to provide only valid blocks. Instead, just compute the UIDs of the new records
        // assuming the block successfully validates.
        let mut uids = vec![];
        let mut uid = self.num_records;
        for txn in block.0 {
            for _ in 0..txn.output_len() {
                uids.push(uid);
                uid += 1;
            }
        }
        self.num_records = uid;
        self.now += 1;

        Ok(uids)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CapeLedger;

impl Ledger for CapeLedger {
    type Validator = CapeTruster;

    fn name() -> String {
        String::from("CAPE")
    }

    fn record_root_history() -> usize {
        CapeContractState::RECORD_ROOT_HISTORY_SIZE
    }

    fn merkle_height() -> u8 {
        CAPE_MERKLE_HEIGHT
    }
}
