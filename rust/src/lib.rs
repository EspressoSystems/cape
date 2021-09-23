pub mod aap_jf;
mod contract_read_aaptx;
mod ethereum;

use ark_serialize::{CanonicalDeserialize, CanonicalSerialize};
use ark_std;
use ethers::prelude::U256;
use jf_utils::to_bytes;

// 52435875175126190479447740508185965837690552500527637822603658699938581184513
const MODULUS_ETHERS: U256 = U256([
    0xffffffff00000001,
    0x53bda402fffe5bfe,
    0x3339d80809a1d805,
    0x73eda753299d7d48,
]);

/// # Examples
/// ```
/// use jf_txn::BlsScalar;
/// use aap_rust_sandbox::to_ethers;
///
/// let n = BlsScalar::from(1);
/// let ethers_uint = to_ethers(n);
/// # use ethers::prelude::U256;
/// # assert_eq!(ethers_uint, U256::from(1));
/// ```
pub fn to_ethers<T: CanonicalSerialize>(number: T) -> U256 {
    let b = to_bytes!(&number).expect("Failed to serialize ark type");
    U256::from_little_endian(&b)
}

pub fn to_bls<T: CanonicalDeserialize>(number: U256) -> T {
    if number >= MODULUS_ETHERS {
        panic!("Value {} is too large", number)
    }
    let mut bytes: Vec<u8> = vec![0; 32];
    number.to_little_endian(&mut bytes);
    T::deserialize(&bytes[..]).expect("Failed to deserialize as ark type")
}

#[cfg(test)]
mod tests {

    use crate::{to_bls, to_ethers, MODULUS_ETHERS};
    use ark_bls12_381;
    use ark_ff::{BigInteger256, FpParameters};
    use ethers::prelude::U256;
    use jf_txn::BlsScalar;
    use proptest::prelude::*;

    fn check_serde(n: BlsScalar) {
        assert_eq!(to_bls::<BlsScalar>(to_ethers(n)), n);
    }

    #[test]
    fn to_bls_works() {
        assert_eq!(to_bls::<BlsScalar>(U256::from(1)), BlsScalar::from(1));
    }

    #[test]
    fn to_ethers_works() {
        assert_eq!(to_ethers(BlsScalar::from(1)), U256::from(1));
    }

    #[test]
    fn to_ethers_and_back_works_with_one() {
        check_serde(BlsScalar::from(1));
    }

    #[test]
    fn to_ethers_and_back_works_with_largest_element() {
        check_serde(BlsScalar::from(0) - BlsScalar::from(1));
    }

    #[test]
    #[should_panic(expected = "too large")]
    fn to_bls_fails_with_modulus() {
        to_bls(MODULUS_ETHERS)
    }

    #[test]
    fn test_ethers_modulus_value_matches_bls12_381() {
        assert_eq!(
            to_ethers(ark_bls12_381::FrParameters::MODULUS),
            MODULUS_ETHERS
        );
    }

    proptest! {
        #[test]
        fn prop_test_to_ethers_and_back(n in prop::array::uniform4(0u64..)
            .prop_map(|limbs| BigInteger256::new(limbs))
            .prop_filter("Must not exceed Modulus",
                         |v| v < &ark_bls12_381::FrParameters::MODULUS)
                 .prop_map(|v| BlsScalar::new(v)))
        {
            check_serde(n);
        }
    }
}
