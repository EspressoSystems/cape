use byteorder::{BigEndian, WriteBytesExt};
use jf_txn::structs::Nullifier;
use jf_utils::to_bytes;
use std::convert::TryInto;
use zerok_lib::set_hash::Hash;

// nullifier is 32 byte
pub fn to_ethers_nullifier(nullifier: Nullifier) -> Vec<u8> {
    zerok_lib::canonical::serialize(&nullifier).unwrap()
}

pub fn to_ethers_hash_bytes(hash: Hash) -> Vec<u8> {
    zerok_lib::canonical::serialize(&hash).unwrap()
}

// hash is 32 byte
pub fn to_ethers_hash(hash: Hash) -> [u32; 4] {
    let bytes = zerok_lib::canonical::serialize(&hash).unwrap();

    let uints = convert_vec_u8_into_vec_u32(bytes);
    uints.try_into().unwrap()
}

pub fn convert_vec_u32_into_vec_u8(input: Vec<u32>) -> Vec<u8> {
    let mut output: Vec<u8> = vec![];

    for elem in input {
        output.write_u32::<BigEndian>(elem).unwrap();
    }
    output
}

pub fn convert_vec_u8_into_vec_u32(input: Vec<u8>) -> Vec<u32> {
    let mut output: Vec<u32> = vec![];

    for elem in input.chunks(8) {
        output.push(u32::from_be_bytes(elem.try_into().unwrap()));
    }
    output
}

pub fn hash_to_bytes(hash: &Hash) -> Vec<u8> {
    let res: Vec<u8> = to_bytes!(hash)
        .expect("Unable to serialize")
        .try_into()
        .expect("Unable to convert to array");

    assert_eq!(res.len(), 32);

    res
}

#[cfg(test)]
mod test {
    use crate::nullifiers_merkle_tree::helpers::{
        convert_vec_u32_into_vec_u8, convert_vec_u8_into_vec_u32,
    };

    fn to_u8_and_back(x: Vec<u32>) -> Vec<u32> {
        let as_bytes = convert_vec_u32_into_vec_u8(x);
        convert_vec_u8_into_vec_u32(as_bytes)
    }

    #[test]
    fn test_u8_u32_convert() {
        assert_eq!(to_u8_and_back(vec![0]), vec![0]);
        assert_eq!(to_u8_and_back(vec![1]), vec![1]);
        assert_eq!(to_u8_and_back(vec![2, 3]), vec![2, 3]);
        assert_eq!(to_u8_and_back(vec![u32::MAX]), vec![u32::MAX]);
    }
}
