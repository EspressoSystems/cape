mod rescue;

use ethers::prelude::abigen;

use crate::ethereum;
use ark_ed_on_bn254::Fq as Fr254;
use ethers::prelude::*;
use jf_primitives::merkle_tree::{
    MerkleFrontier, MerkleLeaf, MerkleLeafProof, MerklePath, MerklePathNode, NodePos, NodeValue,
};
use jf_rescue::Permutation;
use jf_rescue::RescueParameter;
use std::path::Path;
abigen!(
    TestRecordsMerkleTree,
    "artifacts/contracts/TestRecordsMerkleTree.sol/TestRecordsMerkleTree/abi.json",
    event_derives(serde::Deserialize, serde::Serialize)
);

// TODO make this function public in Jellyfish?
/// Hash function used to compute an internal node value
/// * `a` - first input value (e.g.: left child value)
/// * `b` - second input value (e.g.: middle child value)
/// * `c` - third input value (e.g.: right child value)
/// * `returns` - rescue_sponge_no_padding(a,b,c)
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

pub(crate) fn compute_hash_leaf(leaf_value: Fr254, uid: u64) -> Fr254 {
    hash(
        &NodeValue::empty_node_value(),
        &NodeValue::from(uid),
        &NodeValue::from_scalar(leaf_value),
    )
    .to_scalar()
}

#[allow(dead_code)]
pub(crate) async fn get_contract_records_merkle_tree() -> TestRecordsMerkleTree<
    SignerMiddleware<Provider<Http>, Wallet<ethers::core::k256::ecdsa::SigningKey>>,
> {
    let client = ethereum::get_funded_deployer().await.unwrap();
    let contract = ethereum::deploy(
        client.clone(),
        Path::new("../artifacts/contracts/TestRecordsMerkleTree.sol/TestRecordsMerkleTree"),
        (),
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
/// * `returns` - flattened frontier. If the frontier is empty, returns an empty vector.
///
// TODO improve return the array of siblings and the array of 0,1,3 value (positions) to save space. Actually the uid should be enough.
// TODO the uid can be deduced from the frontier (path)
fn flatten_frontier(frontier: &MerkleFrontier<Fr254>, uid: u64) -> Vec<Fr254> {
    match frontier {
        MerkleFrontier::Proof(lap) => {
            let mut res: Vec<Fr254> = vec![];
            // The leaf value comes first
            // Compute the hash of the leaf and position
            let mut current_val = compute_hash_leaf(lap.leaf.0, uid);
            res.push(current_val);
            for node in lap.path.nodes.iter() {
                res.push(node.sibling1.to_scalar());
                res.push(node.sibling2.to_scalar());
                match node.pos {
                    NodePos::Left => res.push(Fr254::from(0)),
                    NodePos::Middle => res.push(Fr254::from(1)),
                    NodePos::Right => res.push(Fr254::from(2)),
                }
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
fn parse_flattened_frontier(flattened_frontier: &[Fr254]) -> MerkleFrontier<Fr254> {
    if flattened_frontier.is_empty() {
        MerkleFrontier::Empty { height: 0 }
    } else {
        let mut nodes: Vec<MerklePathNode<Fr254>> = vec![];

        let mut i = 1;
        while i < flattened_frontier.len() {
            let pos = if flattened_frontier[i + 2] == Fr254::from(0) {
                NodePos::Left
            } else if flattened_frontier[i + 2] == Fr254::from(1) {
                NodePos::Middle
            } else if flattened_frontier[i + 2] == Fr254::from(2) {
                NodePos::Right
            } else {
                NodePos::Left // Should not happen
            };

            let node = MerklePathNode::new(
                pos,
                NodeValue::from_scalar(flattened_frontier[i]),
                NodeValue::from_scalar(flattened_frontier[i + 1]),
            );
            nodes.push(node.clone());
            i = i + 3;
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
    use bincode::Error;
    use itertools::Itertools;
    use jf_primitives::merkle_tree::{MerkleTree, NodeValue};

    // TODO strangely this function does not work. The assert_eq! never raise any error...
    // async fn compare_roots(
    //     mt: &MerkleTree<Fr254>,
    //     contract: &TestRecordsMerkleTree<
    //         SignerMiddleware<Provider<Http>, Wallet<ethers::core::k256::ecdsa::SigningKey>>,
    //     >,
    // ) {
    //     let root_fr254 = mt.commitment().root_value;
    //     let root_value_u256 = contract.get_root_value().call().await.unwrap();
    //
    //     assert_eq!(
    //         convert_u256_to_bytes_le(root_value_u256).as_slice(),
    //         root_fr254.to_scalar().into_repr().to_bytes_le()
    //     );
    // }

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
        let HEIGHT: u8 = 3;
        let mut mt = MerkleTree::<Fr254>::new(HEIGHT).unwrap();

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
            Fr254::from(2),
            merkle_path_nodes[1].sibling1.to_scalar(),
            merkle_path_nodes[1].sibling2.to_scalar(),
            Fr254::from(0),
            merkle_path_nodes[2].sibling1.to_scalar(),
            merkle_path_nodes[2].sibling2.to_scalar(),
            Fr254::from(0),
        ];
        // Size of the vector containing the Merkle path and the leaf value
        let expected_len = usize::from(HEIGHT * 3 + 1);
        assert_eq!(flattened_frontier.len(), expected_len);
        assert_eq!(expected_flattened_frontier, flattened_frontier);

        // Test the reverse operation of flattening
        let HEIGHT: u8 = 3;
        let mut mt = MerkleTree::<Fr254>::new(HEIGHT).unwrap();

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
            parse_flattened_frontier(flattened_frontier.as_slice());

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

    #[tokio::test]
    async fn test_check_frontier() {
        // TODO edge case: empty tree

        let contract = get_contract_records_merkle_tree().await;

        // TODO make height part of the constructor of the contract
        let HEIGHT = 25;
        let mut mt = MerkleTree::<Fr254>::new(HEIGHT).unwrap();
        let elem1 = Fr254::from(3);
        let elem2 = Fr254::from(17);
        let elem3 = Fr254::from(22);
        let elem4 = Fr254::from(78787);

        // In order to get the frontier
        mt.push(elem1);
        mt.push(elem2);
        mt.push(elem3);
        mt.push(elem4);

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
        // TODO compute the position
        let frontier_u256 = flatten_frontier(&frontier_fr254, num_leaves - 1)
            .iter()
            .map(|v| convert_fr254_to_u256(*v))
            .collect_vec();

        // Check the frontier resolves correctly to the root.
        contract
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
        let wrong_number_of_leaves = num_leaves - 1;
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

        // TODO refactor into compare_roots(). Note that it is not so easy that as apparently
        // putting this code into an async function does not allow to catch errors...
        let root_fr254 = mt.commitment().root_value;
        let root_value_u256 = contract.get_root_value().call().await.unwrap();

        assert_eq!(
            convert_u256_to_bytes_le(root_value_u256).as_slice(),
            root_fr254.to_scalar().into_repr().to_bytes_le()
        );

        mt.push(Fr254::from(7878));
        // Insert another element into the Jellyfish Merkle tree to check that roots are differents
        let root_fr254 = mt.commitment().root_value;
        let root_value_u256 = contract.get_root_value().call().await.unwrap();

        assert!(
            convert_u256_to_bytes_le(root_value_u256).as_slice()
                != root_fr254.to_scalar().into_repr().to_bytes_le()
        );
    }

    #[tokio::test]
    async fn test_update_records_merkle_tree() {
        // Check that we can insert values in the Merkle tree
        let contract = get_contract_records_merkle_tree().await;

        // TODO make height part of the constructor of the contract
        let HEIGHT = 25;
        let mut mt = MerkleTree::<Fr254>::new(HEIGHT).unwrap();
        let elem1 = Fr254::from(3);
        let elem2 = Fr254::from(17);
        let elem3 = Fr254::from(22);
        let elem4 = Fr254::from(78787);

        // In order to get the frontier
        mt.push(elem1);
        mt.push(elem2);
        mt.push(elem3);
        mt.push(elem4);

        let frontier_fr254 = mt.frontier();
        // TODO compute the position
        let root_fr254 = mt.commitment().root_value.to_scalar();
        let num_leaves = mt.commitment().num_leaves;
        let root_u256 = convert_fr254_to_u256(root_fr254);

        let frontier_u256 = flatten_frontier(&frontier_fr254, num_leaves - 1)
            .iter()
            .map(|v| convert_fr254_to_u256(*v))
            .collect_vec();

        // TODO when inserting logic works
        contract
            .test_set_root_and_num_leaves(root_u256, num_leaves)
            .legacy()
            .send()
            .await
            .unwrap()
            .await
            .unwrap();

        let elem5 = Fr254::from(875421);
        let elem6 = Fr254::from(3331);
        mt.push(elem5);
        mt.push(elem6);

        // Do not insert any element yet into the records merkle tree of the smart contract
        contract
            .test_update_records_merkle_tree(frontier_u256.clone(), vec![])
            .legacy()
            .send()
            .await
            .unwrap()
            .await
            .unwrap();

        // TODO refactor into compare_roots(). Note that it is not so easy that as apparently
        // putting this code into an async function does not allow to catch errors...
        let root_fr254 = mt.commitment().root_value;
        let root_value_u256 = contract.get_root_value().call().await.unwrap();

        // Roots are different because no element was inserted
        assert!(
            convert_u256_to_bytes_le(root_value_u256).as_slice()
                != root_fr254.to_scalar().into_repr().to_bytes_le()
        );

        // Now we insert the elements into the smart contract

        let elem5_u256 = convert_fr254_to_u256(elem5);
        let elem6_u256 = convert_fr254_to_u256(elem6);
        let elements_u256 = vec![elem5_u256, elem6_u256];

        contract
            .test_update_records_merkle_tree(frontier_u256, elements_u256)
            .legacy()
            .send()
            .await
            .unwrap()
            .await
            .unwrap();
    }
}
