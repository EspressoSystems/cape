use ark_serialize::*;
use generic_array::GenericArray;
use jf_aap::{structs::Nullifier, TransactionNote};
use jf_utils::tagged_blob;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};
use zerok_lib::{
    commit::{Commitment, Committable, RawCommitmentBuilder},
    ledger::traits::*,
    state::ValidationError,
};

// In CAPE, we don't store a sparse local copy of the nullifiers set; instead we use the on-ledger
// nullifier set whenever we need to look up a nullifier. This type is just a stub.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CAPENullifierSet;

impl NullifierSet for CAPENullifierSet {
    type Proof = ();

    fn multi_insert(
        &mut self,
        _nullifiers: &[(Nullifier, Self::Proof)],
    ) -> Result<(), Self::Proof> {
        Ok(())
    }
}

#[tagged_blob("TX")]
#[derive(Clone, Debug, CanonicalSerialize, CanonicalDeserialize)]
pub struct CAPETransaction(TransactionNote);

impl Committable for CAPETransaction {
    fn commit(&self) -> Commitment<Self> {
        RawCommitmentBuilder::new("CAPETransaction")
            .field("note", self.0.commit())
            .finalize()
    }
}

#[tagged_blob("TXHASH")]
#[derive(Clone, Debug, Hash, PartialEq, Eq, CanonicalSerialize, CanonicalDeserialize)]
pub struct CAPETransactionHash(Commitment<CAPETransaction>);

// TODO make this match actual CAPE logic
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, strum_macros::Display)]
pub enum CAPETransactionKind {
    Mint,
    Freeze,
    Unfreeze,
    Send,
    Receive,
    Unknown,
}

impl zerok_lib::ledger::traits::TransactionKind for CAPETransactionKind {
    fn send() -> Self {
        Self::Send
    }

    fn receive() -> Self {
        Self::Receive
    }

    fn mint() -> Self {
        Self::Mint
    }

    fn freeze() -> Self {
        Self::Freeze
    }

    fn unfreeze() -> Self {
        Self::Unfreeze
    }

    fn unknown() -> Self {
        Self::Unknown
    }
}

#[allow(unused_variables)] // FIXME: remove this
impl Transaction for CAPETransaction {
    type NullifierSet = CAPENullifierSet;
    type Hash = CAPETransactionHash;
    type Kind = CAPETransactionKind;

    fn aap(note: TransactionNote, _proofs: Vec<()>) -> Self {
        Self(note)
    }

    fn hash(&self) -> Self::Hash {
        CAPETransactionHash(self.commit())
    }

    fn kind(&self) -> Self::Kind {
        // TODO
        Self::Kind::Unknown
    }

    fn open_audit_memo(
        &self,
        auditable_assets: &std::collections::HashMap<
            jf_aap::structs::AssetCode,
            jf_aap::structs::AssetDefinition,
        >,
        auditor_keys: &std::collections::HashMap<
            jf_aap::keys::AuditorPubKey,
            jf_aap::keys::AuditorKeyPair,
        >,
    ) -> Result<zerok_lib::ledger::AuditMemoOpening, zerok_lib::ledger::AuditError> {
        unimplemented!();
    }

    fn proven_nullifiers(&self) -> Vec<(Nullifier, <Self::NullifierSet as NullifierSet>::Proof)> {
        unimplemented!();
    }
    fn output_commitments(&self) -> Vec<jf_aap::structs::RecordCommitment> {
        unimplemented!();
    }
    fn set_proofs(&mut self, proofs: Vec<<Self::NullifierSet as NullifierSet>::Proof>) {
        unimplemented!();
    }
}

#[tagged_blob("BK")]
#[derive(Clone, Debug, CanonicalSerialize, CanonicalDeserialize)]
pub struct CAPEBlock(Vec<CAPETransaction>);

impl Committable for CAPEBlock {
    fn commit(&self) -> Commitment<Self> {
        RawCommitmentBuilder::new("CAPEBlock")
            .array_field(
                "transactions",
                &self.0.iter().map(|t| t.commit()).collect::<Vec<_>>(),
            )
            .finalize()
    }
}

impl Block for CAPEBlock {
    type Transaction = CAPETransaction;

    fn new(txns: Vec<CAPETransaction>) -> Self {
        Self(txns)
    }

    fn txns(&self) -> Vec<CAPETransaction> {
        self.0.clone()
    }
}

// In CAPE, we don't do local lightweight validation to check the results of queries. We trust the
// results of Ethereum query services, and our local validator stores just enough information to
// satisfy the Validator interface required by the wallet.
//
// Note that this might change if we end up implementing a lightweight CAPE validator in Rust as
// part of the relayer service. In that case, we may be able to reuse that lightweight validator
// here in order to avoid trusting a query service.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CAPEValidator {
    // The current timestamp. The only requirement is that this is a monotonically increasing value,
    // but in this implementation it tracks the number of blocks committed.
    now: u64,
    // Number of records, for generating new UIDs.
    num_records: u64,
    // Current state commitment. This is a commitment to every block which has been committed, as
    // well as to the initial (now, num_records) state for good measure.
    commitment: GenericArray<u8, <Keccak256 as Digest>::OutputSize>,
}

impl CAPEValidator {
    #[allow(dead_code)]
    fn new(now: u64, num_records: u64) -> Self {
        Self {
            now,
            num_records,
            commitment: Keccak256::new()
                .chain("initial".as_bytes())
                .chain(now.to_le_bytes())
                .chain(num_records.to_le_bytes())
                .finalize(),
        }
    }
}

impl Validator for CAPEValidator {
    type StateCommitment = GenericArray<u8, <Keccak256 as Digest>::OutputSize>;
    type Block = CAPEBlock;

    fn now(&self) -> u64 {
        self.now
    }

    fn commit(&self) -> Self::StateCommitment {
        self.commitment
    }

    fn validate_and_apply(&mut self, block: Self::Block) -> Result<Vec<u64>, ValidationError> {
        // We don't actually do validation here, since in this implementation we trust the query
        // service to provide only valid blocks. Instead, just compute a new commitment (by chaining
        // the new block onto the current commitment hash, with a domain separator tag).
        self.commitment = Keccak256::new()
            .chain("block".as_bytes())
            .chain(&self.commitment)
            .chain(&block.commit())
            .finalize();
        self.now += 1;

        // Compute the unique IDs of the output records of this block. The IDs for each block are
        // a consecutive range of integers starting at the previous number of records.
        let mut uids = vec![];
        let mut uid = self.num_records;
        for txn in block.0 {
            for _ in 0..txn.0.output_len() {
                uids.push(uid);
                uid += 1;
            }
        }
        self.num_records = uid;

        Ok(uids)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CAPELedger;

impl Ledger for CAPELedger {
    type Validator = CAPEValidator;
}
