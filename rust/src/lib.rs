pub mod aap_jf;
mod contract_group_operations;
mod contract_read_aaptx;
mod ethereum;

// TODO check which imports are really needed
use ark_bn254::fq2::Fq2;
#[allow(unused_imports)]
use ark_bn254::{
    Fq, FqParameters, Fr, FrParameters, G1Affine, G1Projective, G2Affine, G2Projective,
};
#[allow(unused_imports)]
use ark_ff::{Field, FpParameters, One, PrimeField, Zero};
use ark_serialize::CanonicalDeserialize;
use ethers::prelude::U256;
use jf_utils::to_bytes;
use std::{fmt::Debug, ops::Deref};

// const MODULUS_FR: U256 = U256(FrParameters::MODULUS.0);
// const MODULUS_FQ: U256 = U256(FqParameters::MODULUS.0);

#[derive(Debug, Clone, Copy)]
pub struct G1Ark(G1Affine);

#[derive(Debug, Clone, Copy)]
pub struct G2Ark(G2Affine);

impl Deref for G1Ark {
    type Target = G1Affine;

    fn deref(&self) -> &G1Affine {
        &self.0
    }
}

impl Deref for G2Ark {
    type Target = G2Affine;

    fn deref(&self) -> &G2Affine {
        &self.0
    }
}

// TODO this struct is not very useful. If we don't mind depending
// on G1Point from the abigen we can get rid of it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct G1Ethers {
    x: U256,
    y: U256,
}

// TODO merge with G1Ethers?
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct G2Ethers {
    x: [U256; 2],
    y: [U256; 2],
}

impl G2Ethers {
    fn is_zero(&self) -> bool {
        self.x == [U256::zero(), U256::zero()] && self.y == [U256::zero(), U256::zero()]
    }
}

impl From<G1Ark> for G1Ethers {
    fn from(point: G1Ark) -> Self {
        if (*point).is_zero() {
            return G1Ethers {
                x: U256::zero(),
                y: U256::zero(),
            };
        }

        let x = to_bytes!(&point.x).expect("Failed to serialize ark type");
        let y = to_bytes!(&point.y).expect("Failed to serialize ark type");

        G1Ethers {
            x: U256::from_little_endian(&x[..]),
            y: U256::from_little_endian(&y[..]),
        }
    }
}

// TODO merge with G1Ethers?
impl From<G2Ark> for G2Ethers {
    fn from(point: G2Ark) -> Self {
        if (*point).is_zero() {
            return G2Ethers {
                x: [U256::zero(), U256::zero()],
                y: [U256::zero(), U256::zero()],
            };
        }

        // NOTE:
        // Ark: represented as c0 + c1 * X, for c0, c1 in `P::BaseField`.
        // Contract: Encoding of field elements is: X[0] * z + X[1]
        let x1 = to_bytes!(&point.0.x.c1).expect("Failed to serialize ark type");
        let x2 = to_bytes!(&point.0.x.c0).expect("Failed to serialize ark type");
        let y1 = to_bytes!(&point.0.y.c1).expect("Failed to serialize ark type");
        let y2 = to_bytes!(&point.0.y.c0).expect("Failed to serialize ark type");

        G2Ethers {
            x: [
                U256::from_little_endian(&x1[..]),
                U256::from_little_endian(&x2[..]),
            ],
            y: [
                U256::from_little_endian(&y1[..]),
                U256::from_little_endian(&y2[..]),
            ],
        }
    }
}

impl From<G1Ethers> for G1Ark {
    fn from(point: G1Ethers) -> Self {
        // TODO check if point is valid?
        let infinity = point.x.is_zero() && point.y.is_zero();
        if infinity {
            return Self(G1Affine::zero());
        }
        Self(G1Affine::new(
            to_ark_from_number(point.x),
            to_ark_from_number(point.y),
            false,
        ))
    }
}

impl From<G2Ethers> for G2Ark {
    fn from(point: G2Ethers) -> Self {
        // TODO check if point is valid?
        let infinity = point.is_zero();
        if infinity {
            return Self(G2Affine::zero());
        }
        Self(G2Affine::new(
            to_ark_from_pair(point.x[0], point.x[1]),
            to_ark_from_pair(point.y[0], point.y[1]),
            false,
        ))
    }
}

pub fn to_ethers<T: Field>(number: T) -> U256 {
    let b = to_bytes!(&number).expect("Failed to serialize ark type");
    U256::from_little_endian(&b)
}

pub fn to_ark_from_number<T: Field>(number: U256) -> T {
    // let max = U256(T::MODULUS.0) // XXX how can we check if number is a valid T
    // if number > max {
    //     panic!("Value {} is too large", number)
    // }
    let mut bytes: Vec<u8> = vec![0; 32];
    number.to_little_endian(&mut bytes);
    T::deserialize(&bytes[..]).expect("Failed to deserialize as ark type")
}

pub fn to_ark_from_pair(number1: U256, number2: U256) -> Fq2 {
    let mut bytes1: Vec<u8> = vec![0; 32];
    number1.to_little_endian(&mut bytes1);

    let mut bytes2: Vec<u8> = vec![0; 32];
    number2.to_little_endian(&mut bytes2);

    // TODO sw
    let c1 = Fq::deserialize(&bytes1[..]).expect("Error");
    let c0 = Fq::deserialize(&bytes2[..]).expect("Error");

    Fq2::new(c0, c1)
}

#[cfg(test)]
mod tests {
    use super::*;

    use ark_bn254::Fr;
    use ark_ff::{BigInteger256, FpParameters};
    use ark_std::UniformRand;
    use proptest::prelude::*;

    #[test]
    fn g1_ark_ethers_serde_works_for_zero() {
        let zero_ark = G1Ark(G1Affine::zero());
        let zero_ethers = G1Ethers::from(zero_ark);
        let zero_ark_2: G1Ark = zero_ethers.into();
        let zero_ethers_2 = G1Ethers::from(zero_ark_2);
        assert_eq!(*zero_ark, *zero_ark_2);
        assert_eq!(zero_ethers, zero_ethers_2);
    }

    #[test]
    fn g1_ark_ethers_serde_works_for_random_group_element() {
        let mut rng = ark_std::test_rng();
        let p_ark = G1Ark(G1Affine::from(G1Projective::rand(&mut rng)));
        let p_ethers = G1Ethers::from(p_ark);
        let p_ark_2 = G1Ark::from(p_ethers);
        let p_ethers_2 = G1Ethers::from(p_ark_2);
        assert_eq!(*p_ark, *p_ark_2);
        assert_eq!(p_ethers, p_ethers_2);
    }

    #[test]
    fn g2_ark_ethers_serde_works_for_random_group_element() {
        let mut rng = ark_std::test_rng();
        let p_ark = G2Ark(G2Affine::from(G2Projective::rand(&mut rng)));
        let p_ethers = G2Ethers::from(p_ark);
        let p_ark_2 = G2Ark::from(p_ethers);
        let p_ethers_2 = G2Ethers::from(p_ark_2);
        assert_eq!(*p_ark, *p_ark_2);
        assert_eq!(p_ethers, p_ethers_2);
    }

    fn check_serde<T: Field>(n: T) {
        assert_eq!(to_ark_from_number::<T>(to_ethers(n)), n);
    }

    #[test]
    fn to_ark_works() {
        assert_eq!(to_ark_from_number::<Fr>(U256::from(1)), Fr::from(1));
    }

    #[test]
    fn to_ethers_works() {
        assert_eq!(to_ethers(Fr::from(1)), U256::from(1));
    }

    #[test]
    fn to_ethers_and_back_works_with_one() {
        check_serde(Fr::from(1));
    }

    #[test]
    fn to_ethers_and_back_works_with_largest_element() {
        check_serde(Fr::from(0) - Fr::from(1));
    }

    // TODO these 2 tests fail with
    //      'Failed to deserialize as ark type: IoError(Custom { kind: Other, error: "FromBytes::read failed" })'

    // #[test]
    // #[should_panic(expected = "too large")]
    // fn to_ark_fr_fails_with_modulus() {
    //     to_ark::<Fr>(MODULUS_FR);
    // }

    // #[test]
    // #[should_panic(expected = "too large")]
    // fn to_ark_fq_fails_with_modulus() {
    //     to_ark::<Fq>(MODULUS_FQ);
    // }

    // TODO these 2 tests fail with:
    //      panicked at 'called `Option::unwrap()` on a `None` value'
    //
    // #[test]
    // fn test_ethers_modulus_fr_value_matches_ark() {
    //     assert_eq!(to_ethers::<Fr>(FrParameters::MODULUS.into()), MODULUS_FR);
    // }

    // #[test]
    // fn test_ethers_modulus_fq_value_matches_ark() {
    //     assert_eq!(to_ethers::<Fq>(FqParameters::MODULUS.into()), MODULUS_FQ);
    // }

    proptest! {
        #[test]
        fn prop_test_to_ethers_and_back_fq(n in prop::array::uniform4(0u64..)
            .prop_map(|limbs| BigInteger256::new(limbs))
            .prop_filter("Must not exceed Modulus",
                         |v| v < &FqParameters::MODULUS)
                 .prop_map(|v| Fq::new(v)))
        {
            check_serde(n);
        }

        #[test]
        fn prop_test_to_ethers_and_back_fr(n in prop::array::uniform4(0u64..)
            .prop_map(|limbs| BigInteger256::new(limbs))
            .prop_filter("Must not exceed Modulus",
                         |v| v < &FrParameters::MODULUS)
                 .prop_map(|v| Fr::new(v)))
        {
            check_serde(n);
        }
    }
}
