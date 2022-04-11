// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

#![deny(warnings)]
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

pub fn verifier_keys() -> VerifierKeySet {
    let univ_setup = &*UNIVERSAL_PARAM;
    let (_, xfr_verif_key_12, _) =
        jf_cap::proof::transfer::preprocess(univ_setup, 1, 2, CAPE_MERKLE_HEIGHT).unwrap();
    let (_, xfr_verif_key_22, _) =
        jf_cap::proof::transfer::preprocess(univ_setup, 2, 2, CAPE_MERKLE_HEIGHT).unwrap();
    let (_, xfr_verif_key_23, _) =
        jf_cap::proof::transfer::preprocess(univ_setup, 2, 3, CAPE_MERKLE_HEIGHT).unwrap();
    let (_, mint_verif_key, _) =
        jf_cap::proof::mint::preprocess(univ_setup, CAPE_MERKLE_HEIGHT).unwrap();
    let (_, freeze_verif_key_2, _) =
        jf_cap::proof::freeze::preprocess(univ_setup, 2, CAPE_MERKLE_HEIGHT).unwrap();
    let (_, freeze_verif_key_3, _) =
        jf_cap::proof::freeze::preprocess(univ_setup, 3, CAPE_MERKLE_HEIGHT).unwrap();
    VerifierKeySet {
        mint: TransactionVerifyingKey::Mint(mint_verif_key),
        xfr: KeySet::new(
            vec![
                TransactionVerifyingKey::Transfer(xfr_verif_key_12),
                TransactionVerifyingKey::Transfer(xfr_verif_key_22),
                TransactionVerifyingKey::Transfer(xfr_verif_key_23),
            ]
            .into_iter(),
        )
        .unwrap(),
        freeze: KeySet::new(
            vec![
                TransactionVerifyingKey::Freeze(freeze_verif_key_2),
                TransactionVerifyingKey::Freeze(freeze_verif_key_3),
            ]
            .into_iter(),
        )
        .unwrap(),
    }
}
