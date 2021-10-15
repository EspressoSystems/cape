use ark_serialize::CanonicalSerialize;
use blake2;
use byteorder::{BigEndian, WriteBytesExt};
use ethers::prelude::abigen;
use jf_txn::structs::Nullifier;
use jf_utils::to_bytes;
use std::convert::TryInto;
use zerok_lib::set_hash::Hash;

abigen!(
    NullifiersMerkleTree,
    "./contracts/NullifiersMerkleTree/abi.json",
    event_derives(serde::Deserialize, serde::Serialize)
);

// nullifier is 32 byte
pub fn to_ethers(nullifier: Nullifier) -> Vec<u8> {
    zerok_lib::canonical::serialize(&nullifier).unwrap()
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
        // TODO is LittleEndian correct?
        output.write_u64::<BigEndian>(elem).unwrap();
    }
    output
}

pub fn convert_vec_u8_into_vec_u64(input: Vec<u8>) -> Vec<u64> {
    let mut output: Vec<u64> = vec![];

    for elem in input.chunks(8) {
        // TODO is LittleEndian correct?
        output.push(u64::from_be_bytes(elem.try_into().unwrap()));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ethereum::{deploy, get_funded_deployer};
    use blake2::crypto_mac::Mac;
    use rand::SeedableRng;
    use rand_chacha::ChaChaRng;
    use std::path::Path;

    use zerok_lib::set_hash;

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

    #[tokio::test]
    async fn test_blake2b_elem() {
        let client = get_funded_deployer().await.unwrap();
        let contract = deploy(
            client.clone(),
            Path::new("./contracts/NullifiersMerkleTree"),
        )
        .await
        .unwrap();

        let contract = NullifiersMerkleTree::new(contract.address(), client);

        let mut prng = ChaChaRng::from_seed([0u8; 32]);

        let input = Nullifier::random_for_test(&mut prng);

        let input_ethers = to_ethers(input);

        let hash = set_hash::elem_hash(input);

        let hash_bytes: [u8; 64] = to_bytes!(&hash)
            .expect("Unable to serialize")
            .try_into()
            .expect("Unable to convert to array");

        let res_u64: Vec<u64> = contract
            .elem_hash(input_ethers)
            .call()
            .await
            .unwrap()
            .into();

        let res_u8 = convert_vec_u64_into_vec_u8(res_u64);
        assert_eq!(res_u8, hash_bytes);
    }

    #[tokio::test]
    async fn test_blake2b_branch() {
        let client = get_funded_deployer().await.unwrap();
        let contract = deploy(
            client.clone(),
            Path::new("./contracts/NullifiersMerkleTree"),
        )
        .await
        .unwrap();

        let contract = NullifiersMerkleTree::new(contract.address(), client);

        let mut prng = ChaChaRng::from_seed([0u8; 32]);

        let left = set_hash::elem_hash(Nullifier::random_for_test(&mut prng));
        let right = set_hash::elem_hash(Nullifier::random_for_test(&mut prng));

        let hash = set_hash::branch_hash(left, right);
        println!("left {:?}", left);
        println!("hash {:?}", hash);

        let hash_bytes: [u8; 64] = to_bytes!(&hash)
            .expect("Unable to serialize")
            .try_into()
            .expect("Unable to convert to array");

        // 1. Compare packing

        // TODO check if it's really packed like this
        let mut rust_packed: Vec<u8> = Vec::new();
        rust_packed.extend("l".as_bytes().iter());
        rust_packed.extend(to_bytes!(&left).unwrap());
        rust_packed.extend("r".as_bytes().iter());
        rust_packed.extend(to_bytes!(&right).unwrap());

        let solidity_packed = contract
            .pack(to_ethers_hash(left), to_ethers_hash(right))
            .call()
            .await
            .unwrap();

        // XXX fails!
        assert_eq!(solidity_packed, rust_packed);
        println!("Packing ok!");

        // 2. Compare hashing

        // Manually hash with blake2 lib
        let mut hasher = blake2::Blake2b::with_params(&[], &[], "AAPSet Branch".as_bytes());
        hasher.update(&rust_packed);
        let manual_hash = Hash::new(hasher.finalize().into_bytes());

        assert_eq!(manual_hash, hash);
        println!("Hashing ok!");

        // 3. Full contract call

        let res_u64: Vec<u64> = contract
            .branch_hash(to_ethers_hash(left), to_ethers_hash(right))
            .call()
            .await
            .unwrap()
            .into();

        let res_u8 = convert_vec_u64_into_vec_u8(res_u64);

        // XXX fails!
        assert_eq!(res_u8, hash_bytes);
    }

    fn test_merkle_tree_set(updates: Vec<u16>, checks: Vec<Result<u16, u8>>) {
        use std::collections::HashMap;
        let mut prng = ChaChaRng::from_seed([0u8; 32]);
        let _update_vals = updates
            .iter()
            .cloned()
            .chain(checks.iter().filter_map(|x| x.ok().clone()))
            .map(|u| (u, Nullifier::random_for_test(&mut prng)))
            .collect::<HashMap<_, _>>();
        // let mut hset = HashSet::new();
        let t = zerok_lib::SetMerkleTree::default();
        let lw_t = zerok_lib::SetMerkleTree::ForgottenSubtree { value: t.hash() };
        assert_eq!(t.hash(), lw_t.hash());
    }

    #[test]
    fn test_set_merkle_tree() {
        test_merkle_tree_set(vec![20, 0], vec![Ok(20)]);
    }
}
