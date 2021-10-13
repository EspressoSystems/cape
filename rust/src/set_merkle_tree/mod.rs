use ethers::prelude::{abigen, U256};
use hex;
use jf_txn::structs::Nullifier;
use jf_utils::to_bytes;
use std::convert::TryInto;

abigen!(
    SetMerkleTree,
    "./contracts/SetMerkleTree/abi.json",
    event_derives(serde::Deserialize, serde::Serialize)
);

// nullifier is 32 byte
pub fn to_ethers(nullifier: Nullifier) -> [u8; 32] {
    let b = to_bytes!(&nullifier).expect("Failed to serialize ark type");
    b.try_into().expect("Failed to convert to byte array")
}

// hash is 64 byte

#[cfg(test)]
mod tests {
    use super::*;

    use ark_serialize::CanonicalSerialize;
    use hex_literal::hex;
    use rand::SeedableRng;
    use rand_chacha::ChaChaRng;
    use zerok_lib;

    use crate::ethereum::{deploy, get_funded_deployer};
    use ark_std::UniformRand;
    use ethers;
    use ethers::prelude::{Bytes, Middleware};
    use std::{ops::Neg, path::Path};

    #[tokio::test]
    async fn test_hash() {
        let client = get_funded_deployer().await.unwrap();
        let contract = deploy(client.clone(), Path::new("./contracts/SetMerkleTree"))
            .await
            .unwrap();
        let contract = SetMerkleTree::new(contract.address(), client);

        let mut prng = ChaChaRng::from_seed([0u8; 32]);

        let input = Nullifier::random_for_test(&mut prng);
        let input_ethers = to_ethers(input);

        println!("input {:?}", input);
        println!("input_ethers {:?}", input_ethers);

        let hash = zerok_lib::set_hash::elem_hash(input);
        println!("hash {:?}", hash);

        let hash_bytes: [u8; 64] = to_bytes!(&hash)
            .expect("Unable to serialize")
            .try_into()
            .expect("Unable to convert to array");

        // let res = contract.elem_hash(input_ethers).call().await.unwrap();
        // assert_eq!(res, hash_bytes);
    }

    // use zerok_lib::SetMerkleTree; // XXX name conflict with contract

    fn test_merkle_tree_set(updates: Vec<u16>, checks: Vec<Result<u16, u8>>) {
        use std::collections::{HashMap, HashSet};
        let mut prng = ChaChaRng::from_seed([0u8; 32]);
        let update_vals = updates
            .iter()
            .cloned()
            .chain(checks.iter().filter_map(|x| x.ok().clone()))
            .map(|u| (u, Nullifier::random_for_test(&mut prng)))
            .collect::<HashMap<_, _>>();
        // let mut hset = HashSet::new();
        let mut t = zerok_lib::SetMerkleTree::default();
        let mut lw_t = zerok_lib::SetMerkleTree::ForgottenSubtree { value: t.hash() };
        assert_eq!(t.hash(), lw_t.hash());
    }

    #[test]
    fn test_set_merkle_tree() {
        test_merkle_tree_set(vec![20, 0], vec![Ok(20)]);
    }
}
