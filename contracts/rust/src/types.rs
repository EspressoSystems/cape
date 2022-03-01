use ark_bn254::Fr;
use ark_bn254::{Bn254, Fq};
use ark_ff::{to_bytes, PrimeField, Zero};
use ark_poly::EvaluationDomain;
use ark_poly::Radix2EvaluationDomain;
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use ethers::prelude::*;
use jf_cap::constants::REVEAL_MAP_LEN;
use jf_cap::{
    keys::{AuditorPubKey, CredIssuerPubKey, FreezerPubKey, UserPubKey},
    structs::{
        AssetCode, BlindFactor, FreezeFlag, InternalAssetCode, Nullifier, RecordCommitment,
        RevealMap,
    },
    BaseField, CurveParam, NodeValue, VerKey,
};
use jf_plonk::proof_system::structs::Proof;
use jf_primitives::{
    aead,
    elgamal::{self, EncKey},
};
use std::convert::TryInto;

pub use crate::bindings::{
    AssetDefinition, AssetPolicy, AssetRegistry, AuditMemo, BurnNote, CapeBlock, Challenges,
    EdOnBN254Point, EvalData, EvalDomain, FreezeAuxInfo, FreezeNote, G1Point, G2Point, Greeter,
    MintAuxInfo, MintNote, PcsInfo, PlonkProof, RecordOpening, SimpleToken, TestBN254, TestCAPE,
    TestCAPEEvents, TestCapeTypes, TestEdOnBN254, TestPlonkVerifier, TestPolynomialEval,
    TestRecordsMerkleTree, TestRescue, TestRootStore, TestTranscript, TestVerifyingKeys,
    TranscriptData, TransferAuxInfo, TransferNote, VerifyingKey, CAPE,
};

// The number of input wires of TurboPlonk.
const GATE_WIDTH: usize = 4;

impl From<ark_bn254::G1Affine> for G1Point {
    fn from(p: ark_bn254::G1Affine) -> Self {
        if p.is_zero() {
            // Solidity precompile have a different affine repr for Point of Infinity
            Self {
                x: U256::from(0),
                y: U256::from(0),
            }
        } else {
            Self {
                x: U256::from_little_endian(&to_bytes!(p.x).unwrap()[..]),
                y: U256::from_little_endian(&to_bytes!(p.y).unwrap()[..]),
            }
        }
    }
}

impl From<(ark_bn254::Fq, ark_bn254::Fq)> for G1Point {
    fn from(p: (ark_bn254::Fq, ark_bn254::Fq)) -> Self {
        let zero = ark_bn254::G1Affine::zero();
        if p.0 == zero.x && p.1 == zero.y {
            // Solidity repr of infinity/zero
            Self {
                x: U256::from(0),
                y: U256::from(0),
            }
        } else {
            Self {
                x: U256::from_little_endian(&to_bytes!(p.0).unwrap()[..]),
                y: U256::from_little_endian(&to_bytes!(p.1).unwrap()[..]),
            }
        }
    }
}

impl From<G1Point> for ark_bn254::G1Affine {
    fn from(p_sol: G1Point) -> Self {
        if p_sol.x.is_zero() && p_sol.y.is_zero() {
            Self::zero()
        } else {
            Self::new(u256_to_field(p_sol.x), u256_to_field(p_sol.y), false)
        }
    }
}

impl From<ark_bn254::G2Affine> for G2Point {
    fn from(p: ark_bn254::G2Affine) -> Self {
        // NOTE: in contract, x = x0 * z + x1, whereas in arkwork x = c0 + c1 * X.
        Self {
            x_0: U256::from_little_endian(&to_bytes!(p.x.c1).unwrap()[..]),
            x_1: U256::from_little_endian(&to_bytes!(p.x.c0).unwrap()[..]),
            y_0: U256::from_little_endian(&to_bytes!(p.y.c1).unwrap()[..]),
            y_1: U256::from_little_endian(&to_bytes!(p.y.c0).unwrap()[..]),
        }
    }
}

/// convert a field element (at most BigInteger256).
pub fn field_to_u256<F: PrimeField>(f: F) -> U256 {
    if F::size_in_bits() > 256 {
        panic!("Don't support field size larger than 256 bits.");
    }
    U256::from_little_endian(&to_bytes!(&f).unwrap())
}

/// convert a U256 to a field element.
pub fn u256_to_field<F: PrimeField>(v: U256) -> F {
    let mut bytes = vec![0u8; 32];
    v.to_little_endian(&mut bytes);
    F::from_le_bytes_mod_order(&bytes)
}

impl From<ark_ed_on_bn254::EdwardsAffine> for EdOnBN254Point {
    fn from(p: ark_ed_on_bn254::EdwardsAffine) -> Self {
        // Even though solidity precompile for BN254 has a different Point of Infinity
        // affine representation, we stick with arkwork's (0,1) for EdOnBN254
        Self {
            x: U256::from_little_endian(&to_bytes!(p.x).unwrap()[..]),
            y: U256::from_little_endian(&to_bytes!(p.y).unwrap()[..]),
        }
    }
}

impl From<Radix2EvaluationDomain<Fr>> for EvalDomain {
    fn from(domain: Radix2EvaluationDomain<Fr>) -> Self {
        Self {
            log_size: domain.log_size_of_group.into(),
            size: domain.size.into(),
            size_inv: field_to_u256(domain.size_inv),
            group_gen: field_to_u256(domain.group_gen),
            group_gen_inv: field_to_u256(domain.group_gen_inv),
        }
    }
}

impl From<EvalDomain> for Radix2EvaluationDomain<Fr> {
    fn from(domain: EvalDomain) -> Self {
        let res = Radix2EvaluationDomain::<Fr>::new(domain.size.try_into().unwrap()).unwrap();
        assert!(res.group_gen == u256_to_field::<Fr>(domain.group_gen));
        assert!(res.group_gen_inv == u256_to_field::<Fr>(domain.group_gen_inv));
        assert!(res.size_inv == u256_to_field::<Fr>(domain.size_inv));
        res
    }
}

impl From<Challenges> for jf_plonk::testing_apis::Challenges<Fr> {
    fn from(chal_sol: Challenges) -> Self {
        Self {
            tau: Fr::zero(), // not used
            alpha: u256_to_field(chal_sol.alpha),
            beta: u256_to_field(chal_sol.beta),
            gamma: u256_to_field(chal_sol.gamma),
            zeta: u256_to_field(chal_sol.zeta),
            v: u256_to_field(chal_sol.v),
            u: u256_to_field(chal_sol.u),
        }
    }
}

impl From<jf_plonk::testing_apis::Challenges<Fr>> for Challenges {
    fn from(chal: jf_plonk::testing_apis::Challenges<Fr>) -> Self {
        let alpha2 = chal.alpha * chal.alpha;
        let alpha3 = chal.alpha * alpha2;

        Self {
            alpha: field_to_u256(chal.alpha),
            alpha_2: field_to_u256(alpha2),
            alpha_3: field_to_u256(alpha3),
            beta: field_to_u256(chal.beta),
            gamma: field_to_u256(chal.gamma),
            zeta: field_to_u256(chal.zeta),
            v: field_to_u256(chal.v),
            u: field_to_u256(chal.u),
        }
    }
}

/// a helper trait to help with fully-qualified generic into syntax:
/// `x.generic_into::<DestType>();`
/// This is particularly helpful in a chained `generic_into()` statements.
pub trait GenericInto {
    fn generic_into<T>(self) -> T
    where
        Self: Into<T>,
    {
        self.into()
    }
}

// blanket implementation
impl<T: ?Sized> GenericInto for T {}

macro_rules! jf_conversion_for_u256_new_type {
    ($new_type:ident, $jf_type:ident) => {
        impl From<$jf_type> for $new_type {
            fn from(v: $jf_type) -> Self {
                let mut bytes = vec![];
                v.serialize(&mut bytes).unwrap();
                Self(U256::from_little_endian(&bytes))
            }
        }

        impl From<U256> for $new_type {
            fn from(v: U256) -> Self {
                Self(v)
            }
        }

        impl From<$new_type> for $jf_type {
            fn from(v_sol: $new_type) -> Self {
                let mut bytes = vec![0u8; 32];
                v_sol.0.to_little_endian(&mut bytes);
                let v: $jf_type = CanonicalDeserialize::deserialize(&bytes[..])
                    .expect("Failed to deserialize U256.");
                v
            }
        }
    };
}
/// Intermediate type used during conversion between Solidity's nullifier value to that in Jellyfish.
pub struct NullifierSol(pub U256);
jf_conversion_for_u256_new_type!(NullifierSol, Nullifier);

pub struct RecordCommitmentSol(pub U256);
jf_conversion_for_u256_new_type!(RecordCommitmentSol, RecordCommitment);

pub struct MerkleRootSol(pub U256);
jf_conversion_for_u256_new_type!(MerkleRootSol, NodeValue);

pub struct AssetCodeSol(pub U256);
jf_conversion_for_u256_new_type!(AssetCodeSol, AssetCode);

pub struct InternalAssetCodeSol(pub U256);
jf_conversion_for_u256_new_type!(InternalAssetCodeSol, InternalAssetCode);

pub struct BlindFactorSol(pub U256);
jf_conversion_for_u256_new_type!(BlindFactorSol, BlindFactor);

macro_rules! jf_conversion_for_ed_on_bn254_new_type {
    ($jf_type:ident) => {
        impl From<EdOnBN254Point> for $jf_type {
            fn from(p: EdOnBN254Point) -> Self {
                let x: ark_bn254::Fr = u256_to_field(p.x);
                let y: ark_bn254::Fr = u256_to_field(p.y);
                let mut bytes = vec![];
                (x, y)
                    .serialize(&mut bytes)
                    .expect("Failed to serialize EdOnBN254Point into bytes.");
                assert_eq!(bytes.len(), 64); // 32 bytes for each coordinate
                let pk: $jf_type = CanonicalDeserialize::deserialize_uncompressed(&bytes[..])
                    .expect("Fail to deserialize EdOnBN254Point bytes into Jellyfish types.");
                pk
            }
        }

        impl From<$jf_type> for EdOnBN254Point {
            fn from(pk: $jf_type) -> Self {
                let mut bytes = vec![];
                CanonicalSerialize::serialize_uncompressed(&pk, &mut bytes).unwrap();
                let x = U256::from_little_endian(&bytes[..32]);
                let y = U256::from_little_endian(&bytes[32..]);
                Self { x, y }
            }
        }
    };
}

jf_conversion_for_ed_on_bn254_new_type!(AuditorPubKey);
jf_conversion_for_ed_on_bn254_new_type!(CredIssuerPubKey);
jf_conversion_for_ed_on_bn254_new_type!(FreezerPubKey);
jf_conversion_for_ed_on_bn254_new_type!(VerKey);
type EncKeyCAP = EncKey<CurveParam>;
jf_conversion_for_ed_on_bn254_new_type!(EncKeyCAP);

impl From<jf_cap::structs::AssetPolicy> for AssetPolicy {
    fn from(policy: jf_cap::structs::AssetPolicy) -> Self {
        Self {
            auditor_pk: policy.auditor_pub_key().clone().into(),
            cred_pk: policy.cred_issuer_pub_key().clone().into(),
            freezer_pk: policy.freezer_pub_key().clone().into(),
            reveal_map: field_to_u256(BaseField::from(policy.reveal_map())),
            reveal_threshold: policy.reveal_threshold(),
        }
    }
}

impl From<AssetPolicy> for jf_cap::structs::AssetPolicy {
    fn from(policy_sol: AssetPolicy) -> Self {
        // Internal representation has two fields for pk (pk.x, pk.y), thus + 1 in length
        const REVEAL_MAP_INTERNAL_LEN: usize = REVEAL_MAP_LEN + 1;
        jf_cap::structs::AssetPolicy::default()
            .set_auditor_pub_key(policy_sol.auditor_pk.into())
            .set_cred_issuer_pub_key(policy_sol.cred_pk.into())
            .set_freezer_pub_key(policy_sol.freezer_pk.into())
            .set_reveal_threshold(policy_sol.reveal_threshold)
            .set_reveal_map_for_test({
                let map_sol = policy_sol.reveal_map;
                if map_sol >= U256::from(2u32.pow(REVEAL_MAP_INTERNAL_LEN as u32)) {
                    panic!("Reveal map has more than 12 bits")
                }
                let bits: [bool; REVEAL_MAP_INTERNAL_LEN] = (0..REVEAL_MAP_INTERNAL_LEN)
                    .map(|i| map_sol.bit(REVEAL_MAP_INTERNAL_LEN - 1 - i))
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap();

                RevealMap::new(bits)
            })
    }
}

impl From<jf_cap::structs::AssetDefinition> for AssetDefinition {
    fn from(def: jf_cap::structs::AssetDefinition) -> Self {
        Self {
            code: def.code.generic_into::<AssetCodeSol>().0,
            policy: def.policy_ref().clone().into(),
        }
    }
}

impl From<AssetDefinition> for jf_cap::structs::AssetDefinition {
    fn from(def_sol: AssetDefinition) -> Self {
        Self::new(
            def_sol
                .code
                .generic_into::<AssetCodeSol>()
                .generic_into::<AssetCode>(),
            def_sol.policy.into(),
        )
        .unwrap()
    }
}

impl From<jf_cap::structs::RecordOpening> for RecordOpening {
    fn from(ro: jf_cap::structs::RecordOpening) -> Self {
        Self {
            amount: ro.amount,
            asset_def: ro.asset_def.into(),
            user_addr: ro.pub_key.address().into(),
            freeze_flag: ro.freeze_flag == FreezeFlag::Frozen,
            blind: ro.blind.generic_into::<BlindFactorSol>().0,
        }
    }
}

impl From<RecordOpening> for jf_cap::structs::RecordOpening {
    fn from(ro_sol: RecordOpening) -> Self {
        let pub_key = UserPubKey::new(ro_sol.user_addr.into(), aead::EncKey::default());

        Self {
            amount: ro_sol.amount,
            asset_def: ro_sol.asset_def.into(),
            pub_key,
            freeze_flag: if ro_sol.freeze_flag {
                FreezeFlag::Frozen
            } else {
                FreezeFlag::Unfrozen
            },
            blind: ro_sol
                .blind
                .generic_into::<BlindFactorSol>()
                .generic_into::<BlindFactor>(),
        }
    }
}

impl From<jf_cap::structs::AuditMemo> for AuditMemo {
    fn from(memo: jf_cap::structs::AuditMemo) -> Self {
        let scalars = memo.internal().clone().to_scalars();
        let ephemeral_key = EdOnBN254Point {
            x: field_to_u256(scalars[0]),
            y: field_to_u256(scalars[1]),
        };
        let data: Vec<U256> = scalars.iter().skip(2).map(|&f| field_to_u256(f)).collect();
        Self {
            ephemeral_key,
            data,
        }
    }
}

impl From<AuditMemo> for jf_cap::structs::AuditMemo {
    fn from(memo_sol: AuditMemo) -> Self {
        let mut scalars = vec![
            u256_to_field(memo_sol.ephemeral_key.x),
            u256_to_field(memo_sol.ephemeral_key.y),
        ];
        for v in memo_sol.data {
            scalars.push(u256_to_field(v));
        }
        Self::new(elgamal::Ciphertext::from_scalars(&scalars).unwrap())
    }
}

impl From<jf_cap::transfer::AuxInfo> for TransferAuxInfo {
    fn from(aux: jf_cap::transfer::AuxInfo) -> Self {
        Self {
            merkle_root: aux.merkle_root.generic_into::<MerkleRootSol>().0,
            fee: aux.fee,
            valid_until: aux.valid_until,
            txn_memo_ver_key: aux.txn_memo_ver_key.into(),
            extra_proof_bound_data: aux.extra_proof_bound_data.into(),
        }
    }
}

impl From<TransferAuxInfo> for jf_cap::transfer::AuxInfo {
    fn from(aux_sol: TransferAuxInfo) -> Self {
        Self {
            merkle_root: aux_sol
                .merkle_root
                .generic_into::<MerkleRootSol>()
                .generic_into::<NodeValue>(),
            fee: aux_sol.fee,
            valid_until: aux_sol.valid_until,
            txn_memo_ver_key: aux_sol.txn_memo_ver_key.into(),
            extra_proof_bound_data: aux_sol.extra_proof_bound_data.to_vec(),
        }
    }
}

impl From<jf_cap::mint::MintAuxInfo> for MintAuxInfo {
    fn from(aux: jf_cap::mint::MintAuxInfo) -> Self {
        Self {
            merkle_root: aux.merkle_root.generic_into::<MerkleRootSol>().0,
            fee: aux.fee,
            txn_memo_ver_key: aux.txn_memo_ver_key.into(),
        }
    }
}

impl From<MintAuxInfo> for jf_cap::mint::MintAuxInfo {
    fn from(aux_sol: MintAuxInfo) -> Self {
        Self {
            merkle_root: aux_sol
                .merkle_root
                .generic_into::<MerkleRootSol>()
                .generic_into::<NodeValue>(),
            fee: aux_sol.fee,
            txn_memo_ver_key: aux_sol.txn_memo_ver_key.into(),
        }
    }
}

impl From<jf_cap::freeze::FreezeAuxInfo> for FreezeAuxInfo {
    fn from(aux: jf_cap::freeze::FreezeAuxInfo) -> Self {
        Self {
            merkle_root: aux.merkle_root.generic_into::<MerkleRootSol>().0,
            fee: aux.fee,
            txn_memo_ver_key: aux.txn_memo_ver_key.into(),
        }
    }
}

impl From<FreezeAuxInfo> for jf_cap::freeze::FreezeAuxInfo {
    fn from(aux_sol: FreezeAuxInfo) -> Self {
        Self {
            merkle_root: aux_sol
                .merkle_root
                .generic_into::<MerkleRootSol>()
                .generic_into::<NodeValue>(),
            fee: aux_sol.fee,
            txn_memo_ver_key: aux_sol.txn_memo_ver_key.into(),
        }
    }
}

impl From<Proof<Bn254>> for PlonkProof {
    fn from(proof: Proof<Bn254>) -> Self {
        // both wires_poly_comms and split_quot_poly_comms are (GATE_WIDTH +1)
        // Commitments, each point takes two base fields elements;
        // 3 individual commitment points;
        // (GATE_WIDTH + 1) * 2 scalar fields in poly_evals are  converted to base
        // fields.
        // NOTE: we reorder the points in proof a bit, please refer to
        // https://github.com/SpectrumXYZ/jellyfish/blob/2a40d01c938cdcc716071af5a0dc9b3242181c2c/plonk/src/proof_system/structs.rs#L91
        const TURBO_PLONK_LEN: usize = (GATE_WIDTH + 1) * 2 * 2 + 2 * 3 + (GATE_WIDTH + 1) * 2;
        const NUM_G1_POINT: usize = 13;
        const NUM_EVAL: usize = 10;
        assert_eq!(TURBO_PLONK_LEN, NUM_G1_POINT * 2 + NUM_EVAL);

        let fields: Vec<ark_bn254::Fq> = proof.into();
        if fields.len() != TURBO_PLONK_LEN {
            panic!("Malformed TurboPlonk proof");
        }
        let points: Vec<G1Point> = fields[..2 * NUM_G1_POINT]
            .chunks_exact(2)
            .map(|chunk| {
                if chunk.len() == 2 {
                    G1Point {
                        x: field_to_u256(chunk[0]),
                        y: field_to_u256(chunk[1]),
                    }
                } else {
                    unreachable!();
                }
            })
            .collect();
        let evals: Vec<U256> = fields[2 * NUM_G1_POINT..]
            .iter()
            .map(|f| field_to_u256(*f))
            .collect();

        Self {
            wire_0: points[0].clone(),
            wire_1: points[1].clone(),
            wire_2: points[2].clone(),
            wire_3: points[3].clone(),
            wire_4: points[4].clone(),
            prod_perm: points[10].clone(),
            split_0: points[5].clone(),
            split_1: points[6].clone(),
            split_2: points[7].clone(),
            split_3: points[8].clone(),
            split_4: points[9].clone(),
            zeta: points[11].clone(),
            zeta_omega: points[12].clone(),
            wire_eval_0: evals[0],
            wire_eval_1: evals[1],
            wire_eval_2: evals[2],
            wire_eval_3: evals[3],
            wire_eval_4: evals[4],
            sigma_eval_0: evals[5],
            sigma_eval_1: evals[6],
            sigma_eval_2: evals[7],
            sigma_eval_3: evals[8],
            prod_perm_zeta_omega_eval: evals[9],
        }
    }
}

impl From<PlonkProof> for Proof<Bn254> {
    fn from(pf_sol: PlonkProof) -> Self {
        fn g1_point_to_fields(p: G1Point) -> Vec<ark_bn254::Fq> {
            let p: ark_bn254::G1Affine = p.into();
            vec![p.x, p.y]
        }

        let wires_evals = vec![
            u256_to_field(pf_sol.wire_eval_0),
            u256_to_field(pf_sol.wire_eval_1),
            u256_to_field(pf_sol.wire_eval_2),
            u256_to_field(pf_sol.wire_eval_3),
            u256_to_field(pf_sol.wire_eval_4),
        ];
        let wire_sigma_evals = vec![
            u256_to_field(pf_sol.sigma_eval_0),
            u256_to_field(pf_sol.sigma_eval_1),
            u256_to_field(pf_sol.sigma_eval_2),
            u256_to_field(pf_sol.sigma_eval_3),
        ];
        let perm_next_eval = u256_to_field(pf_sol.prod_perm_zeta_omega_eval);

        let fields: Vec<ark_bn254::Fq> = [
            g1_point_to_fields(pf_sol.wire_0),
            g1_point_to_fields(pf_sol.wire_1),
            g1_point_to_fields(pf_sol.wire_2),
            g1_point_to_fields(pf_sol.wire_3),
            g1_point_to_fields(pf_sol.wire_4),
            g1_point_to_fields(pf_sol.split_0),
            g1_point_to_fields(pf_sol.split_1),
            g1_point_to_fields(pf_sol.split_2),
            g1_point_to_fields(pf_sol.split_3),
            g1_point_to_fields(pf_sol.split_4),
            g1_point_to_fields(pf_sol.prod_perm),
            g1_point_to_fields(pf_sol.zeta),
            g1_point_to_fields(pf_sol.zeta_omega),
            wires_evals,
            wire_sigma_evals,
            vec![perm_next_eval],
        ]
        .concat();

        fields
            .try_into()
            .expect("Failed to convert base fields to Proof.")
    }
}

impl From<jf_cap::transfer::TransferNote> for TransferNote {
    fn from(note: jf_cap::transfer::TransferNote) -> Self {
        let input_nullifiers: Vec<U256> = note
            .inputs_nullifiers
            .iter()
            .map(|&nf| nf.generic_into::<NullifierSol>().0)
            .collect();
        let output_commitments: Vec<U256> = note
            .output_commitments
            .iter()
            .map(|&cm| cm.generic_into::<RecordCommitmentSol>().0)
            .collect();
        Self {
            input_nullifiers,
            output_commitments,
            proof: note.proof.into(),
            audit_memo: note.audit_memo.into(),
            aux_info: note.aux_info.into(),
        }
    }
}

impl From<TransferNote> for jf_cap::transfer::TransferNote {
    fn from(note_sol: TransferNote) -> Self {
        let inputs_nullifiers = note_sol
            .input_nullifiers
            .iter()
            .map(|&nf| nf.generic_into::<NullifierSol>().into())
            .collect();
        let output_commitments = note_sol
            .output_commitments
            .iter()
            .map(|&cm| cm.generic_into::<RecordCommitmentSol>().into())
            .collect();
        Self {
            inputs_nullifiers,
            output_commitments,
            proof: note_sol.proof.into(),
            audit_memo: note_sol.audit_memo.into(),
            aux_info: note_sol.aux_info.into(),
        }
    }
}

impl From<jf_cap::mint::MintNote> for MintNote {
    fn from(note: jf_cap::mint::MintNote) -> Self {
        Self {
            input_nullifier: note.input_nullifier.generic_into::<NullifierSol>().0,
            chg_comm: note.chg_comm.generic_into::<RecordCommitmentSol>().0,
            mint_comm: note.mint_comm.generic_into::<RecordCommitmentSol>().0,
            mint_amount: note.mint_amount,
            mint_asset_def: note.mint_asset_def.into(),
            mint_internal_asset_code: note
                .mint_internal_asset_code
                .generic_into::<InternalAssetCodeSol>()
                .0,
            proof: note.proof.into(),
            audit_memo: note.audit_memo.into(),
            aux_info: note.aux_info.into(),
        }
    }
}

impl From<MintNote> for jf_cap::mint::MintNote {
    fn from(note_sol: MintNote) -> Self {
        Self {
            input_nullifier: note_sol
                .input_nullifier
                .generic_into::<NullifierSol>()
                .into(),
            chg_comm: note_sol
                .chg_comm
                .generic_into::<RecordCommitmentSol>()
                .into(),
            mint_comm: note_sol
                .mint_comm
                .generic_into::<RecordCommitmentSol>()
                .into(),
            mint_amount: note_sol.mint_amount,
            mint_asset_def: note_sol.mint_asset_def.into(),
            mint_internal_asset_code: note_sol
                .mint_internal_asset_code
                .generic_into::<InternalAssetCodeSol>()
                .into(),
            proof: note_sol.proof.into(),
            audit_memo: note_sol.audit_memo.into(),
            aux_info: note_sol.aux_info.into(),
        }
    }
}

impl From<jf_cap::freeze::FreezeNote> for FreezeNote {
    fn from(note: jf_cap::freeze::FreezeNote) -> Self {
        let input_nullifiers: Vec<U256> = note
            .input_nullifiers
            .iter()
            .map(|&nf| nf.generic_into::<NullifierSol>().0)
            .collect();
        let output_commitments: Vec<U256> = note
            .output_commitments
            .iter()
            .map(|&cm| cm.generic_into::<RecordCommitmentSol>().0)
            .collect();
        Self {
            input_nullifiers,
            output_commitments,
            proof: note.proof.into(),
            aux_info: note.aux_info.into(),
        }
    }
}

impl From<FreezeNote> for jf_cap::freeze::FreezeNote {
    fn from(note_sol: FreezeNote) -> Self {
        let input_nullifiers = note_sol
            .input_nullifiers
            .iter()
            .map(|&nf| nf.generic_into::<NullifierSol>().into())
            .collect();
        let output_commitments = note_sol
            .output_commitments
            .iter()
            .map(|&cm| cm.generic_into::<RecordCommitmentSol>().into())
            .collect();
        Self {
            input_nullifiers,
            output_commitments,
            proof: note_sol.proof.into(),
            aux_info: note_sol.aux_info.into(),
        }
    }
}

impl From<jf_cap::VerifyingKey> for VerifyingKey {
    fn from(vk: jf_cap::VerifyingKey) -> Self {
        // scalars are organized as
        // - domain size, 1 element
        // - number of inputs, 1 element
        // - sigmas, 10 elements
        // - selectors, 26 elements
        // - k, 5 elements
        // - g, h, bete_h, 10 elements
        let scalars: Vec<Fq> = vk.into();
        assert_eq!(scalars.len(), 53, "cannot parse vk from rust to solidity");

        let mut scalars = scalars.iter();
        let domain_size = *scalars.next().unwrap();
        let num_inputs = *scalars.next().unwrap();

        let mut sigmas: Vec<G1Point> = vec![];
        for _ in 0..5 {
            let p = (*scalars.next().unwrap(), *scalars.next().unwrap());
            sigmas.push(p.into());
        }

        let mut selectors: Vec<G1Point> = vec![];
        for _ in 0..13 {
            let p = (*scalars.next().unwrap(), *scalars.next().unwrap());
            selectors.push(p.into());
        }

        Self {
            domain_size: field_to_u256(domain_size),
            num_inputs: field_to_u256(num_inputs),
            sigma_0: sigmas[0].clone(),
            sigma_1: sigmas[1].clone(),
            sigma_2: sigmas[2].clone(),
            sigma_3: sigmas[3].clone(),
            sigma_4: sigmas[4].clone(),
            // The order of selectors: q_lc, q_mul, q_hash, q_o, q_c, q_ecc
            q_1: selectors[0].clone(),
            q_2: selectors[1].clone(),
            q_3: selectors[2].clone(),
            q_4: selectors[3].clone(),
            q_m12: selectors[4].clone(),
            q_m34: selectors[5].clone(),
            q_h1: selectors[6].clone(),
            q_h2: selectors[7].clone(),
            q_h3: selectors[8].clone(),
            q_h4: selectors[9].clone(),
            q_o: selectors[10].clone(),
            q_c: selectors[11].clone(),
            q_ecc: selectors[12].clone(),
        }
    }
}

impl From<jf_plonk::testing_apis::PcsInfo<Bn254>> for PcsInfo {
    fn from(info: jf_plonk::testing_apis::PcsInfo<Bn254>) -> Self {
        let mut comm_scalars = vec![];
        let mut comm_bases = vec![];
        for (&base, &scalar) in info.comm_scalars_and_bases.base_scalar_map.iter() {
            comm_scalars.push(field_to_u256(scalar));
            comm_bases.push(base.into());
        }
        Self {
            u: field_to_u256(info.u),
            eval_point: field_to_u256(info.eval_point),
            next_eval_point: field_to_u256(info.next_eval_point),
            eval: field_to_u256(info.eval),
            comm_scalars,
            comm_bases,
            opening_proof: info.opening_proof.0.into(),
            shifted_opening_proof: info.shifted_opening_proof.0.into(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use ark_bn254::{Fq, G1Affine, G2Affine};
    use ark_ec::AffineCurve;
    use ark_ed_on_bn254::EdwardsAffine;
    use ark_ff::field_new;
    use ark_std::UniformRand;

    #[test]
    fn field_types_conversion() {
        let rng = &mut ark_std::test_rng();
        let f1 = ark_bn254::Fr::rand(rng);
        let f2 = ark_bn254::Fq::rand(rng);
        // trivial test, prevent accidental change to the function
        assert_eq!(
            field_to_u256(f1),
            U256::from_little_endian(&to_bytes!(f1).unwrap())
        );
        assert_eq!(
            field_to_u256(f2),
            U256::from_little_endian(&to_bytes!(f2).unwrap())
        );

        assert_eq!(f1, u256_to_field(field_to_u256(f1)));
        assert_eq!(f2, u256_to_field(field_to_u256(f2)));
    }

    #[test]
    fn group_types_conversion() {
        // special case: point of infinity (zero)
        let p1 = G1Affine::default();
        let p1_sol: G1Point = p1.into();
        assert_eq!(p1_sol.x, U256::from(0));
        assert_eq!(p1_sol.y, U256::from(0));
        assert_eq!(p1, p1_sol.generic_into::<G1Affine>());

        // a point (not on the curve, which doesn't matter since we only check conversion)
        let p2 = G1Affine::new(field_new!(Fq, "12345"), field_new!(Fq, "2"), false);
        let p2_sol: G1Point = p2.into();
        assert_eq!(p2_sol.x, U256::from(12345));
        assert_eq!(p2_sol.y, U256::from(2));
        assert_eq!(p2, p2_sol.generic_into::<G1Affine>());

        // check G2 point conversion
        let p3 = G2Affine::prime_subgroup_generator();
        let p3_sol: G2Point = p3.into();
        assert_eq!(
            p3_sol.x_0,
            U256::from_str_radix(
                "0x198e9393920d483a7260bfb731fb5d25f1aa493335a9e71297e485b7aef312c2",
                16
            )
            .unwrap()
        );
        assert_eq!(
            p3_sol.x_1,
            U256::from_str_radix(
                "0x1800deef121f1e76426a00665e5c4479674322d4f75edadd46debd5cd992f6ed",
                16
            )
            .unwrap()
        );
        assert_eq!(
            p3_sol.y_0,
            U256::from_str_radix(
                "0x090689d0585ff075ec9e99ad690c3395bc4b313370b38ef355acdadcd122975b",
                16
            )
            .unwrap()
        );
        assert_eq!(
            p3_sol.y_1,
            U256::from_str_radix(
                "0x12c85ea5db8c6deb4aab71808dcb408fe3d1e7690c43d37b4ce6cc0166fa7daa",
                16
            )
            .unwrap()
        );

        // check ed_on_bn254 point conversion
        let p4 = EdwardsAffine::prime_subgroup_generator();
        let p4_sol: EdOnBN254Point = p4.into();
        assert_eq!(
            p4_sol.x,
            U256::from_str_radix(
                "0x2B8CFD91B905CAE31D41E7DEDF4A927EE3BC429AAD7E344D59D2810D82876C32",
                16
            )
            .unwrap()
        );
        assert_eq!(
            p4_sol.y,
            U256::from_str_radix(
                "0x2AAA6C24A758209E90ACED1F10277B762A7C1115DBC0E16AC276FC2C671A861F",
                16
            )
            .unwrap()
        );
    }
}
