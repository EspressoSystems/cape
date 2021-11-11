#![allow(unused_variables)]
use jf_txn::{structs::RecordCommitment, MerkleCommitment, MerkleFrontier};

use crate::CapeContract;

// TODO: @Philippe, does this align with your plan/work on MT in contract?
pub(crate) trait RecordMerkleTree {
    // verify the claimed current frontier
    fn verify_frontier(&self, frontier: &MerkleFrontier) -> bool;

    // Given the current frontier, and a list of new RecordCommitments to be inserted,
    // this function verifies the frontier against the current merkle tree commitment (part of `self`);
    // then batch insert all the `rc` and finally update the Merkle root and return an updated MerkleCommitment.
    fn batch_insert_with_frontier(
        &mut self,
        current_frontier: MerkleFrontier,
        rcs: &[RecordCommitment],
    ) -> MerkleCommitment;
}

impl RecordMerkleTree for CapeContract {
    fn verify_frontier(&self, frontier: &MerkleFrontier) -> bool {
        unimplemented!();
    }
    fn batch_insert_with_frontier(
        &mut self,
        current_frontier: MerkleFrontier,
        rcs: &[RecordCommitment],
    ) -> MerkleCommitment {
        assert!(self.verify_frontier(&current_frontier));
        unimplemented!();
    }
}
