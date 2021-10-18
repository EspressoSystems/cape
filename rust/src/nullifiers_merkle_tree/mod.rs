pub mod helpers;

use ethers::prelude::abigen;

abigen!(
    NullifiersMerkleTree,
    "./contracts/NullifiersMerkleTree/abi.json",
    event_derives(serde::Deserialize, serde::Serialize)
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ethereum::{deploy, get_funded_deployer};
    use blake2::crypto_mac::Mac;
    use ethers::prelude::*;
    use jf_utils::to_bytes;
    use rand::SeedableRng;
    use rand_chacha::ChaChaRng;
    use std::default::Default;
    use std::path::Path;
    use zerok_lib::{set_hash::Hash, SetMerkleTree};

    use crate::nullifiers_merkle_tree::helpers::{
        blake2b_elem, convert_vec_u64_into_vec_u8, hash_to_bytes, to_ethers_hash,
        to_ethers_hash_bytes, to_ethers_nullifier,
    };
    use jf_txn::structs::Nullifier;
    use zerok_lib::set_hash;

    #[tokio::test]
    async fn test_blake2b_elem() {
        // TODO refactor creation of contract to avoid code duplication.
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
        let input_ethers = to_ethers_nullifier(input);

        let hash = set_hash::elem_hash(input);
        let hash_bytes = hash_to_bytes(&hash);

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
    async fn test_blake2b_leaf() {
        // TODO refactor creation of contract to avoid code duplication.
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
        let input_ethers = to_ethers_nullifier(input);

        let hash = set_hash::leaf_hash(input);
        let hash_bytes = hash_to_bytes(&hash);

        let res_u64: Vec<u64> = contract
            .leaf_hash(input_ethers)
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

        let left = set_hash::leaf_hash(Nullifier::random_for_test(&mut prng));
        let right = set_hash::leaf_hash(Nullifier::random_for_test(&mut prng));

        let hash = set_hash::branch_hash(left, right);
        println!("l={:?}", "l".as_bytes());
        println!("r={:?}", "r".as_bytes());

        println!("left {:?}", left);
        println!("hash {:?}", hash);

        let hash_bytes = hash_to_bytes(&hash);

        // 1. Compare packing

        // TODO check if it's really packed like this
        let mut rust_packed: Vec<u8> = Vec::new();
        // rust_packed.extend("l".as_bytes().iter()); // TODO: re-enable
        rust_packed.extend(to_bytes!(&left).unwrap());
        // rust_packed.extend("r".as_bytes().iter()); // TODO: re-enable
        rust_packed.extend(to_bytes!(&right).unwrap());

        let solidity_packed = contract
            .pack(to_ethers_hash(left), to_ethers_hash(right))
            .call()
            .await
            .unwrap();

        assert_eq!(solidity_packed, rust_packed);
        println!("Packing ok!");

        // 2. Compare hashing

        // Manually hash with blake2 lib
        let mut hasher = blake2::Blake2b::with_params(&[], &[], "AAPSet Branch".as_bytes());
        hasher.update(&rust_packed);
        let manual_hash = Hash::new(hasher.finalize().into_bytes());

        assert_eq!(manual_hash, hash);
        println!("Hashing ok!");

        // 3. Full contract call using branch_hash

        let res_u64_branch_hash: Vec<u64> = contract
            .branch_hash(to_ethers_hash(left), to_ethers_hash(right))
            .call()
            .await
            .unwrap()
            .into();

        let res_u8_branch_hash = convert_vec_u64_into_vec_u8(res_u64_branch_hash);

        // 4. Full contract call using branch_hash_with_updates

        let res_u64_branch_hash_with_updates: Vec<u64> = contract
            .branch_hash_with_updates(to_ethers_hash_bytes(left), to_ethers_hash_bytes(right))
            .call()
            .await
            .unwrap()
            .into();

        let res_u8_branch_hash_with_updates =
            convert_vec_u64_into_vec_u8(res_u64_branch_hash_with_updates);

        // 5. Compare the results

        assert_eq!(res_u8_branch_hash, res_u8_branch_hash_with_updates);
        assert_eq!(res_u8_branch_hash, hash_bytes);
    }

    async fn check_hash_equality(size: usize, are_equal: bool) {
        let client = get_funded_deployer().await.unwrap();
        let contract = deploy(
            client.clone(),
            Path::new("./contracts/NullifiersMerkleTree"),
        )
        .await
        .unwrap();

        let contract = NullifiersMerkleTree::new(contract.address(), client);

        let input: Vec<u8> = vec![3; size];
        let rust_hash = blake2b_elem(&input);

        let res_u64: Vec<u64> = contract.elem_hash(input).call().await.unwrap().into();

        let solidity_hash = convert_vec_u64_into_vec_u8(res_u64);

        assert_eq!(rust_hash == solidity_hash, are_equal);
    }

    #[tokio::test]
    async fn test_showing_different_behaviour_between_blake2b_rust_and_blake2b_solidity() {
        // This test shows that the implementations of blake2b in rust and solidity are not equivalent:
        // They match when the size of the input is less or equal to 128 bytes and differ otherwise.

        check_hash_equality(50, true).await;
        check_hash_equality(60, true).await;
        check_hash_equality(128, true).await;
        check_hash_equality(129, false).await;
        check_hash_equality(255, false).await;
    }

    #[tokio::test]
    async fn test_terminal_node_value_empty() {
        // TODO refactor creation of contract to avoid code duplication.
        let client = get_funded_deployer().await.unwrap();
        let contract = deploy(
            client.clone(),
            Path::new("./contracts/NullifiersMerkleTree"),
        )
        .await
        .unwrap();

        let contract = NullifiersMerkleTree::new(contract.address(), client);

        // let rust_value = SetMerkleTerminalNode::EmptySubtree.value(); // .value() is private
        let rust_value = SetMerkleTree::EmptySubtree.hash();
        let ethers_node = TerminalNode {
            is_empty_subtree: true,
            height: U256::from(0),
            elem: vec![],
        };

        let res: Vec<u64> = contract
            .terminal_node_value(ethers_node)
            .call()
            .await
            .unwrap()
            .into();

        assert_eq!(convert_vec_u64_into_vec_u8(res), hash_to_bytes(&rust_value));
    }

    #[tokio::test]
    async fn test_terminal_node_value_non_empty() {
        // TODO refactor creation of contract to avoid code duplication.
        let client = get_funded_deployer().await.unwrap();
        let contract = deploy(
            client.clone(),
            Path::new("./contracts/NullifiersMerkleTree"),
        )
        .await
        .unwrap();

        let contract = NullifiersMerkleTree::new(contract.address(), client);

        let mut prng = ChaChaRng::from_seed([0u8; 32]);
        let nullifier = Nullifier::random_for_test(&mut prng);

        // let rust_value = SetMerkleTerminalNode::EmptySubtree.value(); // .value() is private
        let mut tree = SetMerkleTree::default();
        tree.insert(nullifier);
        let ethers_node = TerminalNode {
            is_empty_subtree: false,
            height: U256::from(512),
            elem: to_ethers_nullifier(nullifier),
        };

        println!("{:?}", tree);

        // TODO Fails because it consumes too much gas.
        // let res: Vec<u64> = contract
        //     .terminal_node_value(ethers_node)
        //     .call()
        //     .await
        //     .unwrap()
        //     .into();

        // assert_eq!(
        //     convert_vec_u64_into_vec_u8(res),
        //     hash_to_bytes(&tree.hash())
        // );
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
