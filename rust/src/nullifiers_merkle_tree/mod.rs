pub mod helpers;

use ethers::prelude::abigen;

abigen!(
    NullifiersMerkleTree,
    "./contracts/NullifiersMerkleTree/abi.json",
    event_derives(serde::Deserialize, serde::Serialize)
);

#[derive(Debug, PartialEq)]
enum MembershipCheckResult {
    NotInSet = 0,
    InSet = 1,
    RootMismatch = 2,
    Unexpected = 3,
}

impl From<u8> for MembershipCheckResult {
    fn from(n: u8) -> Self {
        match n {
            0 => MembershipCheckResult::NotInSet,
            1 => MembershipCheckResult::InSet,
            2 => MembershipCheckResult::RootMismatch,
            _ => MembershipCheckResult::Unexpected,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ethereum, nullifiers_merkle_tree::helpers::to_ethers_hash};
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

    async fn get_contract() -> NullifiersMerkleTree<
        SignerMiddleware<Provider<Http>, Wallet<ethers::core::k256::ecdsa::SigningKey>>,
    > {
        let client = ethereum::get_funded_deployer().await.unwrap();
        let contract = ethereum::deploy(
            client.clone(),
            Path::new("./contracts/NullifiersMerkleTree"),
            (),
        )
        .await
        .unwrap();
        NullifiersMerkleTree::new(contract.address(), client)
    }

    #[tokio::test]
    async fn test_keccak_elem() {
        let contract = get_contract().await;
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
        let contract = get_contract().await;

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
        let contract = get_contract().await;

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
        let contract = get_contract().await;

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
        let contract = get_contract().await;

        let mut prng = ChaChaRng::from_seed([0u8; 32]);
        let nullifier = Nullifier::random_for_test(&mut prng);

        let ethers_node = TerminalNode {
            is_empty_subtree: false,
            height: U256::from(256),
            elem: to_ethers_nullifier(nullifier),
        };

        // Create a tree with just one nullifier: its hash is the same as the
        // the "value" of a terminal node.
        let mut tree = SetMerkleTree::default();
        tree.insert(nullifier);
        let terminal_node_value = tree.hash();

        let res: Vec<u8> = contract
            .terminal_node_value(ethers_node)
            .call()
            .await
            .unwrap()
            .into();

        assert_eq!(res, hash_to_bytes(&terminal_node_value));
    }

    #[tokio::test]
    async fn test_is_in_set_empty() {
        let contract = get_contract().await;

        let mut prng = ChaChaRng::from_seed([0u8; 32]);
        let nullifier_ethers = to_ethers_nullifier(Nullifier::random_for_test(&mut prng));

        let root = SetMerkleTree::EmptySubtree.hash();
        let ethers_node = TerminalNode {
            is_empty_subtree: true,
            height: U256::from(0),          // ignored
            elem: nullifier_ethers.clone(), // ignored
        };

        let path = vec![];

        println!("path {:?}", path);
        let res: MembershipCheckResult = contract
            .is_in_set(to_ethers_hash(root), path, ethers_node, nullifier_ethers)
            .call()
            .await
            .unwrap()
            .into();

        assert_eq!(res, MembershipCheckResult::NotInSet);
    }

    #[tokio::test]
    async fn test_is_in_set_works_with_correct_nullifier() {
        let contract = get_contract().await;

        let mut prng = ChaChaRng::from_seed([0u8; 32]);
        let nullifier = Nullifier::random_for_test(&mut prng);
        let nullifier_ethers = to_ethers_nullifier(nullifier);

        let mut tree = SetMerkleTree::default();
        // add a few more nullifiers, otherwise the path is of zero length
        tree.insert(nullifier);
        tree.insert(Nullifier::random_for_test(&mut prng));
        tree.insert(Nullifier::random_for_test(&mut prng));
        let root = tree.hash();

        let ethers_node = TerminalNode {
            is_empty_subtree: false,
            height: U256::from(254), // TODO get this from proof
            elem: nullifier_ethers.clone(),
        };
        let (contains, proof) = tree.contains(nullifier).unwrap();
        println!("proof {} {:?}", contains, proof);
        let path = proof.path.into_iter().map(to_ethers_hash).collect();

        println!("path {:?}", path);
        let res: MembershipCheckResult = contract
            .is_in_set(to_ethers_hash(root), path, ethers_node, nullifier_ethers)
            .call()
            .await
            .unwrap()
            .into();

        assert_eq!(res, MembershipCheckResult::InSet);
    }

    #[tokio::test]
    async fn test_is_in_set_fails_with_other_nullifier() {
        let contract = get_contract().await;

        let mut prng = ChaChaRng::from_seed([0u8; 32]);
        let nullifier = Nullifier::random_for_test(&mut prng);
        let nullifier_ethers = to_ethers_nullifier(nullifier);

        let mut tree = SetMerkleTree::default();
        // add a few more nullifiers, otherwise the path is of zero length
        tree.insert(nullifier);
        tree.insert(Nullifier::random_for_test(&mut prng));
        tree.insert(Nullifier::random_for_test(&mut prng));
        let root = tree.hash();

        let ethers_node = TerminalNode {
            is_empty_subtree: false,
            height: U256::from(254), // TODO get this from proof
            elem: nullifier_ethers.clone(),
        };
        let (contains, proof) = tree.contains(nullifier).unwrap();
        println!("proof {} {:?}", contains, proof);
        let path = proof.path.into_iter().map(to_ethers_hash).collect();

        println!("path {:?}", path);
        let res: MembershipCheckResult = contract
            .is_in_set(
                to_ethers_hash(root),
                path,
                ethers_node,
                to_ethers_nullifier(Nullifier::random_for_test(&mut prng)),
            )
            .call()
            .await
            .unwrap()
            .into();

        assert_eq!(res, MembershipCheckResult::RootMismatch);
    }

    // #[tokio::test]
    // async fn test_is_elem_not_in_set_non_empty() {
    //     let contract = get_contract().await;

    //     let mut prng = ChaChaRng::from_seed([0u8; 32]);
    //     let nullifier = Nullifier::random_for_test(&mut prng);
    //     let nullifier_ethers = to_ethers_nullifier(nullifier);

    //     let mut tree = SetMerkleTree::default();
    //     tree.insert(nullifier);
    //     let root = tree.hash();

    //     let ethers_node = TerminalNode {
    //         is_empty_subtree: false,
    //         height: U256::from(256),
    //         elem: nullifier_ethers.clone(),
    //     };

    //     // Create a tree falseh just one nullifier: its hash is the same as the
    //     // the "value" of a terminal node.
    //     let path = vec![];

    //     println!("path {:?}", path);

    //     let res: MembershipCheckResult = contract
    //         .is_in_set(
    //             to_ethers_hash(root),
    //             path,
    //             ethers_node,
    //             to_ethers_nullifier(Nullifier::random_for_test(&mut prng)),
    //         )
    //         .call()
    //         .await
    //         .unwrap()
    //         .into();
    //     println!("res {:?}", res);
    //     assert_eq!(res, false);
    // }

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
