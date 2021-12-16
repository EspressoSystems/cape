use ark_ff::{to_bytes, PrimeField, Zero};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use ethers::prelude::*;
use jf_aap::{
    structs::{AssetCode, Nullifier, RecordCommitment},
    NodeValue,
};

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
