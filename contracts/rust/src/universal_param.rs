// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use jf_cap::proof::universal_setup_for_staging;
use jf_cap::TransactionVerifyingKey;
use key_set::{KeySet, VerifierKeySet};
use lazy_static::lazy_static;
use rand_chacha::rand_core::SeedableRng;
use rand_chacha::ChaChaRng;

use crate::model::CAPE_MERKLE_HEIGHT;

const MAX_DEGREE_SUPPORTED: usize = 2u64.pow(17) as usize;

lazy_static! {
    pub static ref UNIVERSAL_PARAM: jf_cap::proof::UniversalParam =
        universal_setup_for_staging(MAX_DEGREE_SUPPORTED, &mut ChaChaRng::from_seed([0u8; 32]))
            .unwrap();
}

pub const SUPPORTED_TRANSFER_SIZES: &[(usize, usize)] = &[(1, 2), (2, 2), (2, 3), (3, 3)];
pub const SUPPORTED_FREEZE_SIZES: &[usize] = &[2, 3];

/// Compute the verifier keys for different types and sizes of CAP transactions.
pub fn verifier_keys() -> VerifierKeySet {
    use TransactionVerifyingKey::*;
    let univ_setup = &UNIVERSAL_PARAM;
    let xfr_verif_keys = SUPPORTED_TRANSFER_SIZES.iter().map(|&(inputs, outputs)| {
        Transfer(
            jf_cap::proof::transfer::preprocess(univ_setup, inputs, outputs, CAPE_MERKLE_HEIGHT)
                .unwrap()
                .1,
        )
    });
    let mint_verif_key = Mint(
        jf_cap::proof::mint::preprocess(univ_setup, CAPE_MERKLE_HEIGHT)
            .unwrap()
            .1,
    );
    let freeze_verif_keys = SUPPORTED_FREEZE_SIZES.iter().map(|&size| {
        Freeze(
            jf_cap::proof::freeze::preprocess(univ_setup, size, CAPE_MERKLE_HEIGHT)
                .unwrap()
                .1,
        )
    });
    VerifierKeySet {
        mint: mint_verif_key,
        xfr: KeySet::new(xfr_verif_keys).unwrap(),
        freeze: KeySet::new(freeze_verif_keys).unwrap(),
    }
}
