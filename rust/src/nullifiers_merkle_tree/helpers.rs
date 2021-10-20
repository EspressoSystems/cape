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
pub fn to_ethers_hash(hash: Hash) -> [u8; 32] {
    let bytes = zerok_lib::canonical::serialize(&hash).unwrap();
    bytes.try_into().unwrap()
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
