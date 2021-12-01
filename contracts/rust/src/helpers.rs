use crate::U256;
use ark_ed_on_bn254::Fq as Fr254;
use ark_ff::{BigInteger, PrimeField};

pub fn convert_u256_to_bytes_le(num: U256) -> Vec<u8> {
    let mut u8_arr = [0u8; 32];
    num.to_little_endian(&mut u8_arr);
    u8_arr.to_vec()
}

pub fn convert_fr254_to_u256(f: Fr254) -> U256 {
    U256::from(f.into_repr().to_bytes_be().as_slice())
}
