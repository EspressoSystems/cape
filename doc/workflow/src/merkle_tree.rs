// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Describes the interface of the Records Merkle tree
#![allow(unused_variables)]
use jf_cap::{structs::RecordCommitment, MerkleCommitment, MerkleFrontier};

use crate::CapeContract;

pub(crate) trait RecordMerkleTree {
    /// verify the claimed current frontier
    fn verify_frontier(&self, frontier: &MerkleFrontier) -> bool;

    /// Given the current frontier, and a list of new RecordCommitments to be inserted,
    /// this function verifies the frontier against the current merkle tree commitment (part of `self`);
    /// then batch insert all the `rc` and finally update the Merkle root and
    /// return an updated MerkleCommitment and the new MerkleFrontier.
    fn batch_insert_with_frontier(
        &mut self,
        current_frontier: MerkleFrontier,
        rcs: &[RecordCommitment],
    ) -> (MerkleCommitment, MerkleFrontier);
}

impl RecordMerkleTree for CapeContract {
    fn verify_frontier(&self, frontier: &MerkleFrontier) -> bool {
        unimplemented!();
    }
    fn batch_insert_with_frontier(
        &mut self,
        current_frontier: MerkleFrontier,
        rcs: &[RecordCommitment],
    ) -> (MerkleCommitment, MerkleFrontier) {
        assert!(self.verify_frontier(&current_frontier));
        unimplemented!();
    }
}
