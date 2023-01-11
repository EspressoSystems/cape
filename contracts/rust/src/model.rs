// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use crate::types;
use ark_serialize::*;
use core::convert::TryFrom;
use core::fmt::Debug;
use ethers::abi::AbiEncode;
use jf_cap::{
    errors::TxnApiError,
    structs::{Amount, AssetDefinition, AssetPolicy, Nullifier, RecordCommitment, RecordOpening},
    transfer::TransferNote,
    txn_batch_verify, MerkleCommitment, MerkleFrontier, MerkleTree, NodeValue, TransactionNote,
};
use jf_primitives::merkle_tree::FilledMTBuilder;
use jf_utils::tagged_blob;
use key_set::VerifierKeySet;
use serde::{Deserialize, Serialize};
use snafu::Snafu;
use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::Hash;
use std::sync::Arc;

// NOTE: currently supported among list of hardcoded VK inside contract,
// can be changed later.
pub const CAPE_MERKLE_HEIGHT: u8 = 24 /*H*/;
pub const CAPE_BURN_MAGIC_BYTES: &str = "EsSCAPE burn";
pub const CAPE_NUM_ROOTS: usize = 40;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CapeModelTxn {
    CAP(TransactionNote),
    Burn {
        xfr: Box<TransferNote>,
        ro: Box<RecordOpening>,
    },
}

impl CapeModelTxn {
    pub fn nullifiers(&self) -> Vec<Nullifier> {
        match self {
            CapeModelTxn::Burn { xfr, .. } => xfr.inputs_nullifiers.clone(),

            CapeModelTxn::CAP(TransactionNote::Transfer(xfr)) => xfr.inputs_nullifiers.clone(),

            CapeModelTxn::CAP(TransactionNote::Mint(mint)) => {
                vec![mint.input_nullifier]
            }

            CapeModelTxn::CAP(TransactionNote::Freeze(freeze)) => freeze.input_nullifiers.clone(),
        }
    }

    pub fn commitments(&self) -> Vec<RecordCommitment> {
        match self {
            CapeModelTxn::Burn { xfr, .. } => {
                // All valid burn transactions have at least two outputs.
                //
                // The first output is the fee change record, the second
                // output is burned, and the rest are normal outputs which
                // get added to the Merkle tree
                let mut ret = xfr.output_commitments.clone();
                ret.remove(1); // remove the burnt record
                ret
            }
            CapeModelTxn::CAP(note) => note.output_commitments(),
        }
    }
}

#[tagged_blob("EADDR")]
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
pub struct EthereumAddr(pub [u8; 20]);

impl CanonicalSerialize for EthereumAddr {
    fn serialize<W: Write>(&self, mut writer: W) -> Result<(), SerializationError> {
        writer.write_all(&self.0).map_err(SerializationError::from)
    }

    fn serialized_size(&self) -> usize {
        self.0.len()
    }
}

impl CanonicalDeserialize for EthereumAddr {
    fn deserialize<R: Read>(mut reader: R) -> std::result::Result<Self, SerializationError> {
        let mut addr = <[u8; 20]>::default();
        reader
            .read_exact(&mut addr)
            .map_err(SerializationError::from)?;
        Ok(EthereumAddr(addr))
    }
}

impl From<ethers::prelude::Address> for EthereumAddr {
    fn from(eth_addr: ethers::prelude::Address) -> Self {
        Self(eth_addr.to_fixed_bytes())
    }
}

impl From<EthereumAddr> for ethers::prelude::Address {
    fn from(addr: EthereumAddr) -> Self {
        addr.0.into()
    }
}

impl EthereumAddr {
    pub fn as_bytes(&self) -> &[u8; 20] {
        &self.0
    }
}

// ERC20 assets are identified by the address of the smart contract
// controlling them.
#[tagged_blob("ERC20")]
#[derive(CanonicalSerialize, CanonicalDeserialize, Debug, Default, Clone, PartialEq, Eq, Hash)]
pub struct Erc20Code(pub EthereumAddr);

impl From<EthereumAddr> for Erc20Code {
    fn from(addr: EthereumAddr) -> Self {
        Self(addr)
    }
}

impl From<Erc20Code> for EthereumAddr {
    fn from(code: Erc20Code) -> Self {
        code.0
    }
}

impl From<ethers::prelude::Address> for Erc20Code {
    fn from(eth_addr: ethers::prelude::Address) -> Self {
        Self::from(EthereumAddr::from(eth_addr))
    }
}

impl From<Erc20Code> for ethers::prelude::Address {
    fn from(code: Erc20Code) -> Self {
        Self::from(EthereumAddr::from(code))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CapeModelOperation {
    SubmitBlock(Vec<CapeModelTxn>),
    RegisterErc20 {
        asset_def: Box<AssetDefinition>,
        erc20_code: Erc20Code,
        sponsor_addr: EthereumAddr,
    },
    WrapErc20 {
        erc20_code: Erc20Code,
        src_addr: EthereumAddr,
        ro: Box<RecordOpening>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CapeModelEvent {
    Erc20Deposited {
        erc20_code: Erc20Code,
        src_addr: EthereumAddr,
        ro: Box<RecordOpening>,
    },
    BlockCommitted {
        txns: Vec<CapeModelTxn>,
        wraps: Vec<RecordCommitment>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CapeModelEthEffect {
    ReceiveErc20 {
        erc20_code: Erc20Code,
        amount: Amount,
        src_addr: EthereumAddr,
    },
    CheckErc20Exists {
        erc20_code: Erc20Code,
    },
    SendErc20 {
        erc20_code: Erc20Code,
        amount: Amount,
        dst_addr: EthereumAddr,
    },
    Emit(CapeModelEvent),
}

#[derive(Debug, Snafu, Serialize, Deserialize, Clone)]
#[snafu(visibility(pub(crate)))]
pub enum CapeValidationError {
    InvalidErc20Def {
        asset_def: Box<AssetDefinition>,
        erc20_code: Erc20Code,
        sponsor: EthereumAddr,
    },
    InvalidCAPDef {
        asset_def: Box<AssetDefinition>,
    },
    UnregisteredErc20 {
        asset_def: Box<AssetDefinition>,
    },
    IncorrectErc20 {
        asset_def: Box<AssetDefinition>,
        erc20_code: Erc20Code,
        expected_erc20_code: Erc20Code,
    },
    Erc20AlreadyRegistered {
        asset_def: Box<AssetDefinition>,
    },

    NullifierAlreadyExists {
        nullifier: Nullifier,
    },

    IncorrectBurnOpening {
        expected_comm: RecordCommitment,
        ro: Box<RecordOpening>,
    },

    IncorrectBurnField {
        xfr: Box<TransferNote>,
    },

    UnsupportedBurnSize {
        num_inputs: usize,
        num_outputs: usize,
    },
    UnsupportedTransferSize {
        num_inputs: usize,
        num_outputs: usize,
    },
    UnsupportedFreezeSize {
        num_inputs: usize,
    },

    BadMerkleRoot {},
    BadMerklePath {},

    CryptoError {
        // TxnApiError cannot be serialized, and, since it depends on many foreign error types which
        // are not Serialize, it is infeasible to make it serializable. Instead, if we have to
        // serialize this variant, we will serialize Ok(err) to Err(format(err)), and when we
        // deserialize we will at least preserve the variant CryptoError and a String representation
        // of the underlying error.
        #[serde(with = "ser_display")]
        err: Result<Arc<TxnApiError>, String>,
    },

    Failed {
        msg: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapeRecordMerkleHistory(pub VecDeque<NodeValue>);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecordMerkleCommitment(pub MerkleCommitment);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecordMerkleFrontier(pub MerkleFrontier);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapeLedgerState {
    pub state_number: u64, // "block height"
    // The current record Merkle commitment
    pub record_merkle_commitment: MerkleCommitment,
    // The current frontier of the record Merkle tree
    pub record_merkle_frontier: MerkleFrontier,
    // A list of recent record Merkle root hashes for validating slightly-out- of date transactions.
    pub past_record_merkle_roots: CapeRecordMerkleHistory,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapeContractState {
    pub ledger: CapeLedgerState,
    pub verif_crs: VerifierKeySet, // hard-coded
    pub nullifiers: HashSet<Nullifier>,
    pub erc20_registrar: HashMap<AssetDefinition, (Erc20Code, EthereumAddr)>,
    pub erc20_deposited: HashMap<Erc20Code, u128>,
    pub erc20_deposits: Vec<RecordCommitment>,
}

pub fn erc20_asset_description(
    erc20_code: &Erc20Code,
    sponsor: &EthereumAddr,
    policy: AssetPolicy,
) -> Vec<u8> {
    let policy_bytes = AbiEncode::encode(types::AssetPolicy::from(policy));
    [
        "EsSCAPE ERC20".as_bytes(),
        (erc20_code.0).0.as_ref(),
        "sponsored by".as_bytes(),
        sponsor.0.as_ref(),
        "policy".as_bytes(),
        &policy_bytes,
    ]
    .concat()
}

pub fn is_erc20_asset_def_valid(
    def: &AssetDefinition,
    erc20_code: &Erc20Code,
    sponsor: &EthereumAddr,
) -> bool {
    let description = erc20_asset_description(erc20_code, sponsor, def.policy_ref().clone());
    def.code.verify_foreign(&description).is_ok()
}

#[allow(unused_variables)]
fn is_cap_asset_def_valid(def: &AssetDefinition) -> bool {
    // NOTE: we assume that this gets checked by jellyfish's MintNote
    // validation
    true
}

/// None => invalid field, should always be rejected
/// Some(None) => Valid field, not a burn
/// Some(Some(addr)) => Valid field, a burn sending to `addr`
fn extract_burn_dst(xfr: &TransferNote) -> Option<Option<EthereumAddr>> {
    let magic_bytes = CAPE_BURN_MAGIC_BYTES.as_bytes().to_vec();
    assert_eq!(magic_bytes.len(), 12);
    assert_eq!(EthereumAddr::default().0.len(), 20);

    let field_data = &xfr.aux_info.extra_proof_bound_data;

    match field_data.len() {
        0 => Some(None),
        32 => {
            if field_data[..12] != magic_bytes[..] {
                None
            } else {
                Some(Some(EthereumAddr(
                    <[u8; 20]>::try_from(&field_data[12..32]).unwrap(),
                )))
            }
        }
        _ => None,
    }
}

impl CapeContractState {
    pub fn new(verif_crs: VerifierKeySet, record_merkle_frontier: MerkleTree) -> Self {
        Self {
            ledger: CapeLedgerState {
                state_number: 0u64,
                record_merkle_commitment: record_merkle_frontier.commitment(),
                record_merkle_frontier: record_merkle_frontier.frontier(),
                past_record_merkle_roots: CapeRecordMerkleHistory(VecDeque::with_capacity(
                    CAPE_NUM_ROOTS,
                )),
            },
            verif_crs,
            nullifiers: HashSet::new(),
            erc20_registrar: HashMap::new(),
            erc20_deposited: HashMap::new(),
            erc20_deposits: Vec::new(),
        }
    }

    pub fn submit_operations(
        &self,
        ops: Vec<CapeModelOperation>,
    ) -> Result<(Self, Vec<CapeModelEthEffect>), CapeValidationError> {
        let mut new_state: CapeContractState = self.clone();
        let mut effects = vec![];

        new_state.ledger.state_number += 1;

        for o in ops {
            match o {
                CapeModelOperation::RegisterErc20 {
                    asset_def,
                    erc20_code,
                    sponsor_addr,
                } => {
                    if !is_erc20_asset_def_valid(&asset_def, &erc20_code, &sponsor_addr) {
                        return Err(CapeValidationError::InvalidErc20Def {
                            asset_def,
                            erc20_code,
                            sponsor: sponsor_addr,
                        });
                    }

                    if new_state.erc20_registrar.contains_key(&asset_def) {
                        return Err(CapeValidationError::Erc20AlreadyRegistered { asset_def });
                    }
                    new_state
                        .erc20_registrar
                        .insert(*asset_def, (erc20_code.clone(), sponsor_addr));
                    effects.push(CapeModelEthEffect::CheckErc20Exists { erc20_code });
                }
                CapeModelOperation::WrapErc20 {
                    erc20_code,
                    src_addr,
                    ro,
                } => {
                    let asset_def = ro.asset_def.clone();
                    let (expected_erc20_code, _sponsor) = new_state
                        .erc20_registrar
                        .get(&asset_def)
                        .ok_or_else(|| CapeValidationError::UnregisteredErc20 {
                            asset_def: Box::new(asset_def.clone()),
                        })?;
                    if expected_erc20_code != &erc20_code {
                        return Err(CapeValidationError::IncorrectErc20 {
                            asset_def: Box::new(asset_def),
                            erc20_code,
                            expected_erc20_code: expected_erc20_code.clone(),
                        });
                    }

                    new_state
                        .erc20_deposits
                        .push(RecordCommitment::from(ro.as_ref()));
                    *new_state
                        .erc20_deposited
                        .entry(erc20_code.clone())
                        .or_insert(0) += u128::from(ro.amount);
                    effects.push(CapeModelEthEffect::ReceiveErc20 {
                        erc20_code: erc20_code.clone(),
                        amount: ro.amount,
                        src_addr: src_addr.clone(),
                    });
                    effects.push(CapeModelEthEffect::Emit(CapeModelEvent::Erc20Deposited {
                        erc20_code,
                        src_addr,
                        ro,
                    }));
                }
                CapeModelOperation::SubmitBlock(txns) => {
                    // Step 1: filter txns for those with nullifiers that
                    // aren't already published
                    let filtered_txns = txns
                        .iter()
                        .filter(|t| {
                            t.nullifiers()
                                .into_iter()
                                .all(|x| !new_state.nullifiers.contains(&x))
                        })
                        .cloned()
                        .collect::<Vec<_>>();

                    let mut records_to_insert = vec![];

                    // past this point, if any validation error occurs the
                    // entire evm transaction rolls back, so we can mutate
                    // new_state in place.

                    // check everything except the plonk proofs, build up
                    // verif_keys

                    let mut notes = vec![];
                    let mut verif_keys = vec![];
                    let mut merkle_roots = vec![];
                    for t in filtered_txns.iter() {
                        // insert nullifiers
                        for n in t.nullifiers() {
                            if new_state.nullifiers.contains(&n) {
                                return Err(CapeValidationError::NullifierAlreadyExists {
                                    nullifier: n,
                                });
                            }
                            new_state.nullifiers.insert(n);
                        }

                        // TODO: fee-collection records
                        let (vkey, merkle_root, new_records, note) = match t {
                            CapeModelTxn::CAP(TransactionNote::Mint(mint)) => {
                                if !is_cap_asset_def_valid(&mint.mint_asset_def) {
                                    return Err(CapeValidationError::InvalidCAPDef {
                                        asset_def: Box::new(mint.mint_asset_def.clone()),
                                    });
                                }

                                (
                                    &new_state.verif_crs.mint,
                                    mint.aux_info.merkle_root,
                                    vec![mint.chg_comm, mint.mint_comm],
                                    TransactionNote::Mint(mint.clone()),
                                )
                            }

                            CapeModelTxn::Burn { xfr, ro } => {
                                let num_inputs = xfr.inputs_nullifiers.len();
                                let num_outputs = xfr.output_commitments.len();

                                // there must be at least 2 outputs for one
                                // output to be the burned record.
                                if num_outputs < 2 {
                                    return Err(CapeValidationError::UnsupportedBurnSize {
                                        num_inputs,
                                        num_outputs,
                                    });
                                }

                                let expected_comm = xfr.output_commitments[1];
                                let actual_comm = RecordCommitment::from(ro.as_ref());
                                if expected_comm != actual_comm {
                                    return Err(CapeValidationError::IncorrectBurnOpening {
                                        expected_comm,
                                        ro: ro.clone(),
                                    });
                                }

                                let asset_def = ro.asset_def.clone();

                                let (erc20_code, _sponsor) = new_state
                                    .erc20_registrar
                                    .get(&asset_def)
                                    .ok_or_else(|| CapeValidationError::UnregisteredErc20 {
                                        asset_def: Box::new(asset_def),
                                    })?;

                                let dst_addr = if let Some(Some(dst)) = extract_burn_dst(xfr) {
                                    Some(dst)
                                } else {
                                    None
                                }
                                .ok_or_else(|| {
                                    CapeValidationError::IncorrectBurnField { xfr: xfr.clone() }
                                })?;

                                effects.push(CapeModelEthEffect::SendErc20 {
                                    erc20_code: erc20_code.clone(),
                                    amount: ro.amount,
                                    dst_addr,
                                });
                                new_state
                                    .erc20_deposited
                                    .get_mut(erc20_code)
                                    .unwrap()
                                    .checked_sub(ro.amount.into())
                                    .unwrap();

                                let verif_key = new_state
                                    .verif_crs
                                    .xfr
                                    .key_for_size(num_inputs, num_outputs)
                                    .ok_or(CapeValidationError::UnsupportedBurnSize {
                                        num_inputs,
                                        num_outputs,
                                    })?;

                                // Don't include the burned record in the output commitments.
                                let mut output_commitments = xfr.output_commitments.clone();
                                output_commitments.remove(1);
                                (
                                    verif_key,
                                    xfr.aux_info.merkle_root,
                                    output_commitments,
                                    TransactionNote::Transfer(xfr.clone()),
                                )
                            }

                            CapeModelTxn::CAP(TransactionNote::Transfer(note)) => {
                                let num_inputs = note.inputs_nullifiers.len();
                                let num_outputs = note.output_commitments.len();

                                if Some(None) != extract_burn_dst(note) {
                                    return Err(CapeValidationError::IncorrectBurnField {
                                        xfr: note.clone(),
                                    });
                                }

                                let verif_key = new_state
                                    .verif_crs
                                    .xfr
                                    .key_for_size(num_inputs, num_outputs)
                                    .ok_or(CapeValidationError::UnsupportedBurnSize {
                                        num_inputs,
                                        num_outputs,
                                    })?;

                                (
                                    verif_key,
                                    note.aux_info.merkle_root,
                                    note.output_commitments.clone(),
                                    TransactionNote::Transfer(note.clone()),
                                )
                            }

                            CapeModelTxn::CAP(TransactionNote::Freeze(note)) => {
                                let num_inputs = note.input_nullifiers.len();
                                let num_outputs = note.output_commitments.len();

                                let verif_key = new_state
                                    .verif_crs
                                    .freeze
                                    .key_for_size(num_inputs, num_outputs)
                                    .ok_or(CapeValidationError::UnsupportedBurnSize {
                                        num_inputs,
                                        num_outputs,
                                    })?;

                                (
                                    verif_key,
                                    note.aux_info.merkle_root,
                                    note.output_commitments.clone(),
                                    TransactionNote::Freeze(note.clone()),
                                )
                            }
                        };

                        verif_keys.push(vkey);
                        if merkle_root != new_state.ledger.record_merkle_commitment.root_value
                            && !new_state
                                .ledger
                                .past_record_merkle_roots
                                .0
                                .contains(&merkle_root)
                        {
                            return Err(CapeValidationError::BadMerkleRoot {});
                        }
                        merkle_roots.push(merkle_root);
                        records_to_insert.extend(new_records.into_iter());
                        notes.push(note);
                    }

                    // Batch PLONK verify
                    if !filtered_txns.is_empty() {
                        assert_eq!(filtered_txns.len(), notes.len());
                        assert_eq!(filtered_txns.len(), verif_keys.len());
                        assert_eq!(filtered_txns.len(), merkle_roots.len());

                        txn_batch_verify(
                            notes.as_slice(),
                            &merkle_roots,
                            new_state.ledger.state_number,
                            &verif_keys,
                        )
                        .map_err(|err| {
                            CapeValidationError::CryptoError {
                                err: Ok(Arc::new(err)),
                            }
                        })?;
                    }

                    // Process the pending deposits
                    let wrapped_commitments = new_state.erc20_deposits.clone();
                    records_to_insert.append(&mut new_state.erc20_deposits);

                    // update the record tree
                    let (record_merkle_frontier, record_merkle_commitment) = {
                        let mut builder = FilledMTBuilder::from_frontier(
                            &new_state.ledger.record_merkle_commitment,
                            &new_state.ledger.record_merkle_frontier,
                        )
                        .ok_or(CapeValidationError::BadMerklePath {})?;

                        for rc in records_to_insert {
                            builder.push(rc.to_field_element());
                        }

                        builder.into_frontier_and_commitment()
                    };

                    if new_state.ledger.past_record_merkle_roots.0.len() >= CAPE_NUM_ROOTS {
                        new_state.ledger.past_record_merkle_roots.0.pop_back();
                    }
                    new_state
                        .ledger
                        .past_record_merkle_roots
                        .0
                        .push_front(new_state.ledger.record_merkle_commitment.root_value);
                    new_state.ledger.record_merkle_commitment = record_merkle_commitment;
                    new_state.ledger.record_merkle_frontier = record_merkle_frontier;

                    effects.push(CapeModelEthEffect::Emit(CapeModelEvent::BlockCommitted {
                        wraps: wrapped_commitments,
                        txns: filtered_txns,
                    }))
                }
            }
        }

        Ok((new_state, effects))
    }
}

mod ser_display {
    use serde::de::{Deserialize, Deserializer};
    use serde::ser::{Serialize, Serializer};
    use std::fmt::Display;

    pub fn serialize<S: Serializer, T: Display>(
        v: &Result<T, String>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        let string = match v {
            Ok(v) => format!("{}", v),
            Err(string) => string.clone(),
        };
        Serialize::serialize(&string, s)
    }

    pub fn deserialize<'de, D: Deserializer<'de>, T>(d: D) -> Result<Result<T, String>, D::Error> {
        Ok(Err(Deserialize::deserialize(d)?))
    }
}
