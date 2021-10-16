use blake2::crypto_mac::Mac;
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

// hash is 64 byte
pub fn to_ethers_hash(hash: Hash) -> [u64; 8] {
    let bytes = zerok_lib::canonical::serialize(&hash).unwrap();

    let uints = convert_vec_u8_into_vec_u64(bytes);
    uints.try_into().unwrap()

    // same result as above
    // let arr: [u8; 64] = bytes.try_into().unwrap();
    // let mut ret: [u64; 8] = [0, 0, 0, 0, 0, 0, 0, 0];

    // for (i, group) in arr.chunks(8).enumerate() {
    //     let arr: [u8; 8] = group.try_into().unwrap();
    //     ret[i] = u64::from_be_bytes(arr);
    // }
    // ret
}

pub fn convert_vec_u64_into_vec_u8(input: Vec<u64>) -> Vec<u8> {
    let mut output: Vec<u8> = vec![];

    for elem in input {
        output.write_u64::<BigEndian>(elem).unwrap();
    }
    output
}

pub fn convert_vec_u8_into_vec_u64(input: Vec<u8>) -> Vec<u64> {
    let mut output: Vec<u64> = vec![];

    for elem in input.chunks(8) {
        output.push(u64::from_be_bytes(elem.try_into().unwrap()));
    }
    output
}

pub fn hash_to_bytes(hash: &Hash) -> Vec<u8> {
    let res: Vec<u8> = to_bytes!(hash)
        .expect("Unable to serialize")
        .try_into()
        .expect("Unable to convert to array");

    assert_eq!(res.len(), 64);

    res
}

pub fn blake2b_elem(input: &[u8]) -> Vec<u8> {
    let mut hasher = blake2::Blake2b::with_params(&[], &[], "AAPSet Elem".as_bytes());
    hasher.update(&input);
    let hash = Hash::new(hasher.finalize().into_bytes());
    hash_to_bytes(&hash)
}

#[cfg(test)]
mod test {
    use crate::nullifiers_merkle_tree::helpers::{
        convert_vec_u64_into_vec_u8, convert_vec_u8_into_vec_u64,
    };

    fn to_u8_and_back(x: Vec<u64>) -> Vec<u64> {
        let as_bytes = convert_vec_u64_into_vec_u8(x);
        convert_vec_u8_into_vec_u64(as_bytes)
    }

    #[test]
    fn test_u8_u64_convert() {
        assert_eq!(to_u8_and_back(vec![0]), vec![0]);
        assert_eq!(to_u8_and_back(vec![1]), vec![1]);
        assert_eq!(to_u8_and_back(vec![2, 3]), vec![2, 3]);
        assert_eq!(to_u8_and_back(vec![u64::MAX]), vec![u64::MAX]);
    }
}
