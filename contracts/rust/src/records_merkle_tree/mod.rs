// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

mod rescue;
use ark_ed_on_bn254::Fq as Fr254;
use jf_primitives::merkle_tree::{
    MerkleFrontier, MerkleLeaf, MerkleLeafProof, MerklePath, MerklePathNode, NodePos, NodeValue,
};
use jf_rescue::Permutation;
use jf_rescue::RescueParameter;
use std::convert::TryFrom;

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
#[allow(dead_code)]
pub(crate) fn flatten_frontier(frontier: &MerkleFrontier<Fr254>, uid: u64) -> Vec<Fr254> {
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
            i += 2;
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
    use crate::deploy::deploy_test_records_merkle_tree_contract;
    use crate::helpers::convert_fr254_to_u256;
    use crate::test_utils::compare_roots_records_merkle_tree_contract;
    use ark_ed_on_bn254::Fq as Fr254;
    use ark_std::UniformRand;
    use ethers::prelude::U256;
    use jf_primitives::merkle_tree::{MerkleTree, NodeValue};

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

    fn insert_elements_into_jellyfish_mt(mt: &mut MerkleTree<Fr254>, n_elems: u32) -> Vec<U256> {
        let mut rng = ark_std::test_rng();
        let mut elems_u256 = vec![];
        for _ in 0..n_elems {
            let elem = Fr254::rand(&mut rng);
            let elem_u256 = convert_fr254_to_u256(elem);
            elems_u256.push(elem_u256);
            mt.push(elem.clone());
        }
        return elems_u256;
    }

    async fn check_update_records_merkle_tree(
        height: u8,
        n_leaves_before: u32,
        n_leaves_after: u32,
    ) {
        // Check that we can insert values in the Merkle tree

        let contract = deploy_test_records_merkle_tree_contract(height).await;
        let mut mt = MerkleTree::<Fr254>::new(height).unwrap();

        // At beginning (no leaf inserted) both roots are the same.
        compare_roots_records_merkle_tree_contract(&mt, &contract, true).await;

        // We insert the first set of leaves
        let elems_u256 = insert_elements_into_jellyfish_mt(&mut mt, n_leaves_before);
        contract
            .test_update_records_merkle_tree(elems_u256)
            .legacy()
            .send()
            .await
            .unwrap()
            .await
            .unwrap();

        compare_roots_records_merkle_tree_contract(&mt, &contract, true).await;

        // We insert the second set of leaves
        let elems_u256 = insert_elements_into_jellyfish_mt(&mut mt, n_leaves_after);
        contract
            .test_update_records_merkle_tree(elems_u256)
            .legacy()
            .send()
            .await
            .unwrap()
            .await
            .unwrap();

        compare_roots_records_merkle_tree_contract(&mt, &contract, true).await;
    }

    #[tokio::test]
    async fn test_update_records_merkle_tree() {
        // We can insert elements in an empty tree
        check_update_records_merkle_tree(3, 0, 4).await;

        // We can fill up a tree of height 3 with 27 leaves
        check_update_records_merkle_tree(3, 1, 26).await;

        // We can insert elements after the frontier has been internally updated by the CAPE contract
        // w.r.t. different leaves positions
        check_update_records_merkle_tree(3, 9, 1).await;
        check_update_records_merkle_tree(3, 10, 17).await;
        check_update_records_merkle_tree(3, 25, 2).await;

        // It still works with different heights
        check_update_records_merkle_tree(4, 6, 30).await;
        check_update_records_merkle_tree(6, 5, 8).await;
    }
}
