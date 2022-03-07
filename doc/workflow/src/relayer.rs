// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Describes how relayers are instantiated
#![allow(dead_code)]
use jf_cap::{keys::UserKeyPair, MerkleTree};

use crate::constants::RECORD_MT_HEIGHT;

pub(crate) struct Relayer {
    pub(crate) mt: MerkleTree,
    pub(crate) wallet: UserKeyPair,
}

impl Relayer {
    /// Instantiate a new relayer
    pub(crate) fn new() -> Self {
        Self {
            mt: MerkleTree::new(RECORD_MT_HEIGHT).unwrap(),
            wallet: UserKeyPair::generate(&mut rand::thread_rng()),
        }
    }
}
