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
    use ethers::prelude::*;

    use rand::SeedableRng;
    use rand_chacha::ChaChaRng;
    use std::default::Default;
    use std::path::Path;
    use zerok_lib::SetMerkleTree;

    use crate::nullifiers_merkle_tree::helpers::{
        hash_to_bytes, to_ethers_hash_bytes, to_ethers_nullifier,
    };
    use jf_txn::structs::Nullifier;
    use std::convert::TryInto;
    use zerok_lib::set_hash;

    #[tokio::test]
    async fn test_keccak_elem() {
        // TODO refactor creation of contract to avoid code duplication.
        let client = get_funded_deployer().await.unwrap();
        let contract = deploy(
            client.clone(),
            Path::new("./contracts/NullifiersMerkleTree"),
            (),
        )
        .await
        .unwrap();

        let contract = NullifiersMerkleTree::new(contract.address(), client);

        let mut prng = ChaChaRng::from_seed([0u8; 32]);

        let input = Nullifier::random_for_test(&mut prng);
        let input_ethers = to_ethers_nullifier(input);

        let hash = set_hash::elem_hash(input);
        let hash_bytes = hash_to_bytes(&hash);

        let res_u8: Vec<u8> = contract
            .elem_hash(input_ethers)
            .call()
            .await
            .unwrap()
            .into();

        assert_eq!(res_u8, hash_bytes);
    }

    #[tokio::test]
    async fn test_keccak_leaf() {
        // TODO refactor creation of contract to avoid code duplication.
        let client = get_funded_deployer().await.unwrap();
        let contract = deploy(
            client.clone(),
            Path::new("./contracts/NullifiersMerkleTree"),
            (),
        )
        .await
        .unwrap();

        let contract = NullifiersMerkleTree::new(contract.address(), client);

        let mut prng = ChaChaRng::from_seed([0u8; 32]);

        let input = Nullifier::random_for_test(&mut prng);
        let input_ethers = to_ethers_nullifier(input);

        let hash = set_hash::leaf_hash(input);
        let hash_bytes = hash_to_bytes(&hash);

        let res: Vec<u8> = contract
            .leaf_hash(input_ethers)
            .call()
            .await
            .unwrap()
            .into();

        assert_eq!(res, hash_bytes);
    }

    #[tokio::test]
    async fn test_keccak_branch() {
        let client = get_funded_deployer().await.unwrap();
        let contract = deploy(
            client.clone(),
            Path::new("./contracts/NullifiersMerkleTree"),
            (),
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

        let left_input: [u8; 32] = to_ethers_hash_bytes(left).try_into().unwrap();
        let right_input: [u8; 32] = to_ethers_hash_bytes(right).try_into().unwrap();

        let res_u8_branch_hash: Vec<u8> = contract
            .branch_hash(left_input, right_input)
            .call()
            .await
            .unwrap()
            .into();

        assert_eq!(res_u8_branch_hash, hash_bytes);
    }

    #[tokio::test]
    async fn test_terminal_node_value_empty() {
        // TODO refactor creation of contract to avoid code duplication.
        let client = get_funded_deployer().await.unwrap();
        let contract = deploy(
            client.clone(),
            Path::new("./contracts/NullifiersMerkleTree"),
            (),
        )
        .await
        .unwrap();

        let contract = NullifiersMerkleTree::new(contract.address(), client);

        let rust_value = SetMerkleTree::EmptySubtree.hash();
        let ethers_node = TerminalNode {
            is_empty_subtree: true,
            height: U256::from(0),
            elem: vec![],
        };

        let res: Vec<u8> = contract
            .terminal_node_value(ethers_node)
            .call()
            .await
            .unwrap()
            .into();

        assert_eq!(res, hash_to_bytes(&rust_value));
    }

    #[tokio::test]
    async fn test_terminal_node_value_non_empty() {
        // TODO refactor creation of contract to avoid code duplication.
        let client = get_funded_deployer().await.unwrap();
        let contract = deploy(
            client.clone(),
            Path::new("./contracts/NullifiersMerkleTree"),
            (),
        )
        .await
        .unwrap();

        let contract = NullifiersMerkleTree::new(contract.address(), client);

        let mut prng = ChaChaRng::from_seed([0u8; 32]);
        let nullifier = Nullifier::random_for_test(&mut prng);

        let mut tree = SetMerkleTree::default();
        tree.insert(nullifier);
        let ethers_node = TerminalNode {
            is_empty_subtree: false,
            height: U256::from(256),
            elem: to_ethers_nullifier(nullifier),
        };

        println!("{:?}", tree);

        let res: Vec<u8> = contract
            .terminal_node_value(ethers_node)
            .call()
            .await
            .unwrap()
            .into();

        assert_eq!(res, hash_to_bytes(&tree.hash()));
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
