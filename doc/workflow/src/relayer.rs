#![allow(dead_code)]
use jf_cap::{keys::UserKeyPair, MerkleTree};

use crate::constants::RECORD_MT_HEIGHT;

pub(crate) struct Relayer {
    pub(crate) mt: MerkleTree,
    pub(crate) wallet: UserKeyPair,
}

impl Relayer {
    pub(crate) fn new() -> Self {
        Self {
            mt: MerkleTree::new(RECORD_MT_HEIGHT).unwrap(),
            wallet: UserKeyPair::generate(&mut rand::thread_rng()),
        }
    }
}
