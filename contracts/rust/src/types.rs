use ark_ff::{to_bytes, PrimeField, Zero};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use ethers::prelude::*;
use jf_aap::{
    keys::{AuditorPubKey, CredIssuerPubKey, FreezerPubKey, UserPubKey},
    structs::{AssetCode, BlindFactor, FreezeFlag, Nullifier, RecordCommitment, RevealMap},
    CurveParam, NodeValue, VerKey,
};
use jf_primitives::elgamal::{self, EncKey};

abigen!(
    TestBN254,
    "../artifacts/contracts/mocks/TestBN254.sol/TestBN254/abi.json",
    event_derives(serde::Deserialize, serde::Serialize);

    TestRecordsMerkleTree,
    "../artifacts/contracts/mocks/TestRecordsMerkleTree.sol/TestRecordsMerkleTree/abi.json",
    event_derives(serde::Deserialize, serde::Serialize);

    TestTranscript,
    "../artifacts/contracts/mocks/TestTranscript.sol/TestTranscript/abi.json",
    event_derives(serde::Deserialize, serde::Serialize);

    CAPE,
    "../artifacts/contracts/CAPE.sol/CAPE/abi.json",
    event_derives(serde::Deserialize, serde::Serialize);

    TestCapeTypes,
    "../artifacts/contracts/mocks/TestCapeTypes.sol/TestCapeTypes/abi.json",
    event_derives(serde::Deserialize, serde::Serialize);

    TestCAPE,
    "../artifacts/contracts/mocks/TestCAPE.sol/TestCAPE/abi.json",
    event_derives(serde::Deserialize, serde::Serialize);

    Greeter,
    "../artifacts/contracts/Greeter.sol/Greeter/abi.json",
    event_derives(serde::Deserialize, serde::Serialize);
);

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

impl From<ark_ed_on_bn254::EdwardsAffine> for EdOnBn254Point {
    fn from(p: ark_ed_on_bn254::EdwardsAffine) -> Self {
        Self {
            x: U256::from_little_endian(&to_bytes!(p.x).unwrap()[..]),
            y: U256::from_little_endian(&to_bytes!(p.y).unwrap()[..]),
        }
    }
}

/// a helper trait to help with fully-qualified generic into synatx:
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
                    .expect("Failed to deserialze U256.");
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

pub struct BlindFactorSol(pub U256);
jf_conversion_for_u256_new_type!(BlindFactorSol, BlindFactor);

macro_rules! jf_conversion_for_ed_on_bn254_new_type {
    ($jf_type:ident) => {
        impl From<EdOnBn254Point> for $jf_type {
            fn from(p: EdOnBn254Point) -> Self {
                let x: ark_bn254::Fr = u256_to_field(p.x);
                let y: ark_bn254::Fr = u256_to_field(p.y);
                let mut bytes = vec![];
                (x, y)
                    .serialize(&mut bytes)
                    .expect("Failed to serialize EdOnBn254Point into bytes.");
                assert_eq!(bytes.len(), 64); // 32 bytes for each coordinate
                let pk: $jf_type = CanonicalDeserialize::deserialize_uncompressed(&bytes[..])
                    .expect("Fail to deserialize EdOnBn254Point bytes into Jellyfish types.");
                pk
            }
        }

        impl From<$jf_type> for EdOnBn254Point {
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
type EncKeyAAP = EncKey<CurveParam>;
jf_conversion_for_ed_on_bn254_new_type!(EncKeyAAP);

impl From<jf_aap::structs::AssetPolicy> for AssetPolicy {
    fn from(policy: jf_aap::structs::AssetPolicy) -> Self {
        Self {
            auditor_pk: policy.auditor_pub_key().clone().into(),
            cred_pk: policy.cred_issuer_pub_key().clone().into(),
            freezer_pk: policy.freezer_pub_key().clone().into(),
            reveal_map: policy.reveal_map().internal(),
            reveal_threshold: policy.reveal_threshold(),
        }
    }
}

impl From<AssetPolicy> for jf_aap::structs::AssetPolicy {
    fn from(policy_sol: AssetPolicy) -> Self {
        jf_aap::structs::AssetPolicy::default()
            .set_auditor_pub_key(policy_sol.auditor_pk.into())
            .set_cred_issuer_pub_key(policy_sol.cred_pk.into())
            .set_freezer_pub_key(policy_sol.freezer_pk.into())
            .set_reveal_threshold(policy_sol.reveal_threshold)
            .set_reveal_map(RevealMap::new(policy_sol.reveal_map))
    }
}

impl From<jf_aap::structs::AssetDefinition> for AssetDefinition {
    fn from(def: jf_aap::structs::AssetDefinition) -> Self {
        Self {
            code: def.code.generic_into::<AssetCodeSol>().0,
            policy: def.policy_ref().clone().into(),
        }
    }
}

impl From<AssetDefinition> for jf_aap::structs::AssetDefinition {
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

impl From<jf_aap::structs::RecordOpening> for RecordOpening {
    fn from(ro: jf_aap::structs::RecordOpening) -> Self {
        Self {
            amount: ro.amount,
            asset_def: ro.asset_def.into(),
            user_addr: ro.pub_key.address().into(),
            freeze_flag: ro.freeze_flag == FreezeFlag::Frozen,
            blind: ro.blind.generic_into::<BlindFactorSol>().0,
        }
    }
}

impl From<RecordOpening> for jf_aap::structs::RecordOpening {
    fn from(ro_sol: RecordOpening) -> Self {
        let mut pub_key = UserPubKey::default();
        pub_key.set_address(ro_sol.user_addr.into());
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

impl From<jf_aap::structs::AuditMemo> for AuditMemo {
    fn from(memo: jf_aap::structs::AuditMemo) -> Self {
        let scalars = memo.internal().clone().to_scalars();
        let ephemeral_key = EdOnBn254Point {
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

impl From<AuditMemo> for jf_aap::structs::AuditMemo {
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

        // a point (not on the curve, which doesn't matter since we only check conversion)
        let p2 = G1Affine::new(field_new!(Fq, "12345"), field_new!(Fq, "2"), false);
        let p2_sol: G1Point = p2.into();
        assert_eq!(p2_sol.x, U256::from(12345));
        assert_eq!(p2_sol.y, U256::from(2));

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
        let p4_sol: EdOnBn254Point = p4.into();
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
