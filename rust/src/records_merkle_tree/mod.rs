mod rescue;

use crate::bindings::TestRecordsMerkleTree;
use crate::ethereum;
use ark_ed_on_bn254::Fq as Fr254;
use ethers::prelude::*;
use jf_primitives::merkle_tree::{
    MerkleFrontier, MerkleLeaf, MerkleLeafProof, MerklePath, MerklePathNode, NodePos, NodeValue,
};
use jf_rescue::Permutation;
use jf_rescue::RescueParameter;
use std::convert::TryFrom;
use std::path::Path;

// TODO make this function public in Jellyfish?
/// Hash function used to compute an internal node value
/// * `a` - first input value (e.g.: left child value)
/// * `b` - second input value (e.g.: middle child value)
/// * `c` - third input value (e.g.: right child value)
/// * `returns` - rescue_sponge_no_padding(a,b,c)
#[allow(dead_code)]
pub(crate) fn hash<F: RescueParameter>(
    a: &NodeValue<F>,
    b: &NodeValue<F>,
    c: &NodeValue<F>,
) -> NodeValue<F> {
    let perm = Permutation::default();
    let digest = perm
        .sponge_no_padding(&[a.to_scalar(), b.to_scalar(), c.to_scalar()], 1)
        .unwrap()[0];
    NodeValue::from_scalar(digest)
}

#[allow(dead_code)]
pub(crate) fn compute_hash_leaf(leaf_value: Fr254, uid: u64) -> Fr254 {
    hash(
        &NodeValue::empty_node_value(),
        &NodeValue::from(uid),
        &NodeValue::from_scalar(leaf_value),
    )
    .to_scalar()
}

#[allow(dead_code)]
pub(crate) async fn get_contract_records_merkle_tree(
    height: u8,
) -> TestRecordsMerkleTree<
    SignerMiddleware<Provider<Http>, Wallet<ethers::core::k256::ecdsa::SigningKey>>,
> {
    let client = ethereum::get_funded_deployer().await.unwrap();
    let contract = ethereum::deploy(
        client.clone(),
        Path::new("../artifacts/contracts/TestRecordsMerkleTree.sol/TestRecordsMerkleTree"),
        height,
    )
    .await
    .unwrap();
    TestRecordsMerkleTree::new(contract.address(), client)
}
/// Takes a frontier from a Merkle tree and returns
/// [leaf,s_{0,first},s_{0,second},pos_0,
/// s_{1,first},s_{1,second},pos_1,
/// ...,
/// s_{n,first},s_{n,second},pos_n]
/// where (s_{i,first},s_{i,second},pos_i) is the ith Merkle path node,
/// and `leaf` is the final node of the path.
/// Note that we ignore the leaf.
/// * `frontier` - frontier to be flattened
/// * `uid` - uid of the leaf, needed to compute the commitment
/// * `returns` - flattened frontier. If the frontier is empty, returns an empty vector.
///
// TODO the uid can be deduced from the frontier (path)
#[allow(dead_code)]
fn flatten_frontier(frontier: &MerkleFrontier<Fr254>, uid: u64) -> Vec<Fr254> {
    match frontier {
        MerkleFrontier::Proof(lap) => {
            let mut res: Vec<Fr254> = vec![];
            // The leaf value comes first
            // Compute the hash of the leaf and position
            let current_val = compute_hash_leaf(lap.leaf.0, uid);
            res.push(current_val);
            for node in lap.path.nodes.iter() {
                res.push(node.sibling1.to_scalar());
                res.push(node.sibling2.to_scalar());
            }
            res
        }
        _ => vec![],
    }
}

/// Parse the flattened frontier in order to create a "real" frontier.
/// This function is here for testing and documenting purpose.
/// The smart contract somehow follows some similar logic in order to create the tree structure from the flattened frontier.
/// * `flattened_frontier` - flat representation of the frontier
/// * `returns` - structured representation of the frontier
#[allow(dead_code)]
fn parse_flattened_frontier(flattened_frontier: &[Fr254], uid: u64) -> MerkleFrontier<Fr254> {
    if flattened_frontier.is_empty() {
        MerkleFrontier::Empty { height: 0 }
    } else {
        let mut nodes: Vec<MerklePathNode<Fr254>> = vec![];

        // Obtain the position from the uid
        let mut absolute_position = uid;
        let mut local_position = u8::try_from(absolute_position % 3).unwrap();

        let mut i = 1;
        while i < flattened_frontier.len() {
            let node = MerklePathNode::new(
                NodePos::from(local_position),
                NodeValue::from_scalar(flattened_frontier[i]),
                NodeValue::from_scalar(flattened_frontier[i + 1]),
            );

            if i < flattened_frontier.len() - 1 {
                absolute_position /= 3;
                local_position = u8::try_from(absolute_position % 3).unwrap();
            } else {
                local_position = u8::try_from(absolute_position / 3).unwrap()
            }

            nodes.push(node.clone());
            i = i + 2;
        }
        MerkleFrontier::Proof(MerkleLeafProof {
            leaf: MerkleLeaf(flattened_frontier[0]),
            path: MerklePath { nodes },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helpers::{convert_fr254_to_u256, convert_u256_to_bytes_le};
    use ark_ed_on_bn254::Fq as Fr254;
    use ark_ff::BigInteger;
    use ark_ff::PrimeField;
    use ark_std::UniformRand;

    use itertools::Itertools;
    use jf_primitives::merkle_tree::{MerkleTree, NodeValue};

    async fn compare_roots(
        mt: &MerkleTree<Fr254>,
        contract: &TestRecordsMerkleTree<
            SignerMiddleware<Provider<Http>, Wallet<ethers::core::k256::ecdsa::SigningKey>>,
        >,
        should_be_equal: bool,
    ) {
        let root_fr254 = mt.commitment().root_value;
        let root_value_u256 = contract.get_root_value().call().await.unwrap();

        assert_eq!(
            should_be_equal,
            (convert_u256_to_bytes_le(root_value_u256).as_slice()
                == root_fr254.to_scalar().into_repr().to_bytes_le())
        );
    }

    #[test]
    fn test_jellyfish_records_merkle_tree() {
        const HEIGHT: u8 = 5;
        let mt = MerkleTree::<Fr254>::new(HEIGHT).unwrap();
        assert_eq!(mt.height(), HEIGHT);
        assert_eq!(mt.commitment().root_value, NodeValue::empty_node_value());
        assert_eq!(mt.num_leaves(), 0);
    }

    #[test]
    fn test_flatten_frontier() {
        let height: u8 = 3;
        let mut mt = MerkleTree::<Fr254>::new(height).unwrap();

        let frontier = mt.frontier();
        let flattened_frontier = flatten_frontier(&frontier, 0);

        // When the frontier is empty the flattened frontier is empty as well
        assert_eq!(flattened_frontier, vec![]);

        let elem1 = Fr254::from(5);
        let elem2 = Fr254::from(6);
        let elem3 = Fr254::from(7);
        mt.push(elem1);
        mt.push(elem2);
        mt.push(elem3);
        let frontier = mt.frontier();
        let uid = 2;
        let flattened_frontier = flatten_frontier(&frontier, uid);

        let (merkle_path_nodes, leaf) = match frontier.clone() {
            MerkleFrontier::Proof(lap) => (lap.path.nodes, lap.leaf.0),
            _ => (vec![], Fr254::from(0)),
        };

        let expected_flattened_frontier: Vec<Fr254> = vec![
            compute_hash_leaf(leaf, uid),
            merkle_path_nodes[0].sibling1.to_scalar(),
            merkle_path_nodes[0].sibling2.to_scalar(),
            merkle_path_nodes[1].sibling1.to_scalar(),
            merkle_path_nodes[1].sibling2.to_scalar(),
            merkle_path_nodes[2].sibling1.to_scalar(),
            merkle_path_nodes[2].sibling2.to_scalar(),
        ];
        // Size of the vector containing the Merkle path and the leaf value
        let expected_len = usize::from(height * 2 + 1);
        assert_eq!(flattened_frontier.len(), expected_len);
        assert_eq!(expected_flattened_frontier, flattened_frontier);

        // Test the reverse operation of flattening
        let height: u8 = 3;
        let mut mt = MerkleTree::<Fr254>::new(height).unwrap();

        let frontier = mt.frontier();
        let flattened_frontier = flatten_frontier(&frontier, 0);

        // When the frontier is empty the flattened frontier is empty as well
        assert_eq!(flattened_frontier, vec![]);

        let elem1 = Fr254::from(5);
        let elem2 = Fr254::from(6);
        mt.push(elem1);
        mt.push(elem2);
        let frontier = mt.frontier();
        let uid = 1;

        // Check the parsing of flattened frontier
        // Only the paths obtained from the flattened frontier and the original frontier are the same
        // as in the case of the flatten frontier we have the hash of the leaf
        // ie. v = H(0,l,uid) instead of the value of the leaf `l`.
        let flattened_frontier = flatten_frontier(&frontier, uid);
        let frontier_from_flattened_frontier =
            parse_flattened_frontier(flattened_frontier.as_slice(), uid);

        let merkle_path_from_flattened = match frontier_from_flattened_frontier {
            MerkleFrontier::Proof(lap) => lap.path.nodes,
            _ => vec![],
        };

        let merkle_path_from_frontier = match frontier {
            MerkleFrontier::Proof(lap) => lap.path.nodes,
            _ => vec![],
        };

        assert_eq!(merkle_path_from_flattened, merkle_path_from_frontier);
    }

    async fn check_check_frontier(n_leaves_before: u8, height: u8) {
        let contract = get_contract_records_merkle_tree(height).await;

        let mut mt = MerkleTree::<Fr254>::new(height).unwrap();

        // Insert several elements
        let mut rng = ark_std::test_rng();

        for _ in 0..n_leaves_before {
            let elem = Fr254::rand(&mut rng);
            mt.push(elem.clone());
        }

        let root_fr254 = mt.commitment().root_value.to_scalar();
        let num_leaves = mt.commitment().num_leaves;
        let root_u256 = convert_fr254_to_u256(root_fr254);

        contract
            .test_set_root_and_num_leaves(root_u256, num_leaves)
            .legacy()
            .send()
            .await
            .unwrap()
            .await
            .unwrap();

        let frontier_fr254 = mt.frontier();

        let frontier_u256 = flatten_frontier(&frontier_fr254, num_leaves - 1)
            .iter()
            .map(|v| convert_fr254_to_u256(*v))
            .collect_vec();

        // Check the frontier resolves correctly to the root.
        let _res = contract
            .clone()
            .test_update_records_merkle_tree(frontier_u256.clone(), vec![])
            .legacy()
            .send()
            .await
            .unwrap()
            .await;

        // Negative paths

        // Wrong frontier
        let mut wrong_frontier_u256 = frontier_u256.clone();
        wrong_frontier_u256[0] = U256::from(1777);
        let c = contract
            .test_update_records_merkle_tree(wrong_frontier_u256.clone(), vec![])
            .legacy();
        let receipt = c.send().await;
        assert!(receipt.is_err()); // TODO add a test like this in ethereum_test?

        // Wrong number of leaves
        let wrong_number_of_leaves = 33;
        contract
            .test_set_root_and_num_leaves(root_u256, wrong_number_of_leaves)
            .legacy()
            .send()
            .await
            .unwrap()
            .await
            .unwrap();

        let c = contract
            .test_update_records_merkle_tree(frontier_u256.clone(), vec![])
            .legacy();
        let receipt = c.send().await;
        assert!(receipt.is_err()); // TODO add a test like this in ethereum_test?

        // Restore the right number of leaves
        contract
            .test_set_root_and_num_leaves(root_u256, num_leaves)
            .legacy()
            .send()
            .await
            .unwrap()
            .await
            .unwrap();

        compare_roots(&mt, &contract, true).await;

        // Insert another element into the Jellyfish Merkle tree to check that roots are differents
        mt.push(Fr254::from(7878));
        compare_roots(&mt, &contract, false).await;
    }

    #[tokio::test]
    async fn test_check_frontier() {
        // TODO edge case: empty tree
        check_check_frontier(1, 3).await;
        check_check_frontier(2, 5).await;
        check_check_frontier(3, 7).await;
    }

    async fn check_update_records_merkle_tree(
        height: u8,
        n_leaves_before: u32,
        n_leaves_after: u32,
    ) {
        // Check that we can insert values in the Merkle tree

        let contract = get_contract_records_merkle_tree(height).await;
        let mut mt = MerkleTree::<Fr254>::new(height).unwrap();

        // Insert several elements
        let mut rng = ark_std::test_rng();

        for _ in 0..n_leaves_before {
            let elem = Fr254::rand(&mut rng);
            mt.push(elem.clone());
        }

        let frontier_fr254 = mt.frontier();

        let root_fr254 = mt.commitment().root_value.to_scalar();
        let num_leaves = mt.commitment().num_leaves;
        let root_u256 = convert_fr254_to_u256(root_fr254);

        let frontier_u256 = flatten_frontier(&frontier_fr254, num_leaves - 1)
            .iter()
            .map(|v| convert_fr254_to_u256(*v))
            .collect_vec();

        println!("frontier_u256: {:?}", frontier_u256);

        println!("root_u256: {}", root_u256);
        println!("num_leaves: {}", num_leaves);

        contract
            .test_set_root_and_num_leaves(root_u256, num_leaves)
            .legacy()
            .send()
            .await
            .unwrap()
            .await
            .unwrap();

        // Do not insert any element yet into the records merkle tree of the smart contract
        contract
            .test_update_records_merkle_tree(frontier_u256.clone(), vec![])
            .legacy()
            .send()
            .await
            .unwrap()
            .await
            .unwrap();

        // Roots are the same
        compare_roots(&mt, &contract, true).await;

        // After insertion into the Jellyfish Merkle tree roots are different
        let mut elems_u256 = vec![];
        for _ in 0..n_leaves_after {
            let elem = Fr254::rand(&mut rng);
            let elem_u256 = convert_fr254_to_u256(elem);
            elems_u256.push(elem_u256);
            mt.push(elem.clone());
        }

        if n_leaves_after > 0 {
            compare_roots(&mt, &contract, false).await;
        } else {
            compare_roots(&mt, &contract, true).await;
        }

        // Now we insert the elements into the smart contract
        contract
            .test_update_records_merkle_tree(frontier_u256, elems_u256)
            .legacy()
            .send()
            .await
            .unwrap()
            .await
            .unwrap();

        // Roots are the same
        compare_roots(&mt, &contract, true).await;
    }

    #[tokio::test]
    async fn test_update_records_merkle_tree() {
        for n_leaves_before in 1..9 {
            // TODO greater values yield the EVM "Stack too deep exception"
            let n_leaves_after_values = [0, 1, 2, 3, 4];
            for n_leaves_after in n_leaves_after_values {
                println!("n_leaves_before: {}", n_leaves_before);
                println!("n_leaves_after: {}", n_leaves_after);
                check_update_records_merkle_tree(3, n_leaves_before, n_leaves_after).await;
            }
        }

        for height in [4, 10, 15] {
            check_update_records_merkle_tree(height, 1, 3).await;
        }
    }
}
