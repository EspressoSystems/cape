#[cfg(test)]
mod tests {
    use jf_txn::structs::Nullifier;
    use rand::SeedableRng;
    use rand_chacha::ChaChaRng;
    use zerok_lib::SetMerkleTree;

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
        let mut t = SetMerkleTree::default();
        let mut lw_t = SetMerkleTree::ForgottenSubtree { value: t.hash() };
        assert_eq!(t.hash(), lw_t.hash());
    }

    #[test]
    fn test_set_merkle_tree() {
        test_merkle_tree_set(vec![20, 0], vec![Ok(20)]);
    }
}
