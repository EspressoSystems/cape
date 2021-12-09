use ark_bn254::{G1Affine, G2Affine};
use ark_ff::{to_bytes, PrimeField, Zero};
use ethers::prelude::*;

abigen!(
    TestBN254,
    "../artifacts/contracts/TestBN254.sol/TestBN254/abi.json",
    event_derives(serde::Deserialize, serde::Serialize);

    TestRecordsMerkleTree,
    "../artifacts/contracts/TestRecordsMerkleTree.sol/TestRecordsMerkleTree/abi.json",
    event_derives(serde::Deserialize, serde::Serialize);

    TestTranscript,
    "../artifacts/contracts/TestTranscript.sol/TestTranscript/abi.json",
    event_derives(serde::Deserialize, serde::Serialize);

    CAPE,
    "../artifacts/contracts/CAPE.sol/CAPE/abi.json",
    event_derives(serde::Deserialize, serde::Serialize);

    // TestCAPE,
    // "../artifacts/contracts/TestCAPE.sol/TestCAPE/abi.json",
    // event_derives(serde::Deserialize, serde::Serialize);

    Greeter,
    "../artifacts/contracts/Greeter.sol/Greeter/abi.json",
    event_derives(serde::Deserialize, serde::Serialize);
);

impl From<G1Affine> for G1Point {
    fn from(p: G1Affine) -> Self {
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

impl From<G2Affine> for G2Point {
    fn from(p: G2Affine) -> Self {
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

// TODO: remove this once https://github.com/gakonst/ethers-rs/issues/661 is resolved
impl From<(U256, U256)> for G1Point {
    fn from(tuple: (U256, U256)) -> Self {
        Self {
            x: tuple.0,
            y: tuple.1,
        }
    }
}

// TODO: remove this once https://github.com/gakonst/ethers-rs/issues/661 is resolved
impl From<(U256, U256, U256, U256)> for G2Point {
    fn from(tuple: (U256, U256, U256, U256)) -> Self {
        Self {
            x_0: tuple.0,
            x_1: tuple.1,
            y_0: tuple.2,
            y_1: tuple.3,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use ark_bn254::Fq;
    use ark_ec::AffineCurve;
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
    }
}
