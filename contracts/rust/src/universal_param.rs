// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

#![deny(warnings)]
use jf_cap::testing_apis::universal_setup_for_test;
use lazy_static::lazy_static;
use rand_chacha::rand_core::SeedableRng;
use rand_chacha::ChaChaRng;

const MAX_DEGREE_SUPPORTED: usize = 2u64.pow(17) as usize;

lazy_static! {
    pub static ref UNIVERSAL_PARAM: jf_cap::proof::UniversalParam =
        universal_setup_for_test(MAX_DEGREE_SUPPORTED, &mut ChaChaRng::from_seed([0u8; 32]))
            .unwrap();
}
