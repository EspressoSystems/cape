// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use crate::deploy::deploy_test_verifying_keys_contract;
use crate::ethereum::get_funded_client;
use crate::{types as sol, types::TestVerifyingKeys};
use anyhow::Result;
use ark_std::{rand::Rng, test_rng};
use ethers::prelude::*;
use jf_cap::proof::universal_setup_for_staging;
use jf_cap::proof::{freeze, mint, transfer};
use jf_cap::structs::NoteType;

const TREE_DEPTH: u8 = 24;
const SUPPORTED_VKS: [(NoteType, u8, u8, u8); 3] = [
    (NoteType::Transfer, 2, 2, TREE_DEPTH),
    (NoteType::Mint, 1, 2, TREE_DEPTH),
    (NoteType::Freeze, 3, 3, TREE_DEPTH),
];

#[tokio::test]
async fn test_get_encoded_id() -> Result<()> {
    let contract = deploy_test_verifying_keys_contract().await;
    let rng = &mut test_rng();

    for _ in 0..5 {
        let note_type: u8 = rng.gen_range(0..=3);
        let num_input: u8 = rng.gen_range(0..=5);
        let num_output: u8 = rng.gen_range(0..=5);
        let tree_depth: u8 = rng.gen_range(20..=26);

        assert_eq!(
            contract
                .get_encoded_id(note_type, num_input, num_output, tree_depth)
                .call()
                .await?,
            (U256::from(note_type) << 24)
                + (U256::from(num_input) << 16)
                + (U256::from(num_output) << 8)
                + U256::from(tree_depth)
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_get_vk_by_id() -> Result<()> {
    let contract = deploy_test_verifying_keys_contract().await;
    let rng = &mut test_rng();

    let max_degree = 2usize.pow(17);
    let srs = universal_setup_for_staging(max_degree, rng).unwrap();

    for (note_type, num_input, num_output, tree_depth) in SUPPORTED_VKS {
        // load rust vk
        let vk = match note_type {
            NoteType::Transfer => {
                let (_, vk, _) = transfer::preprocess(
                    &srs,
                    num_input as usize,
                    num_output as usize,
                    tree_depth,
                )?;
                vk.get_verifying_key()
            }
            NoteType::Mint => {
                let (_, vk, _) = mint::preprocess(&srs, tree_depth)?;
                vk.get_verifying_key()
            }
            NoteType::Freeze => {
                let (_, vk, _) = freeze::preprocess(&srs, num_input as usize, tree_depth)?;
                vk.get_verifying_key()
            }
        };
        let vk: sol::VerifyingKey = vk.into();

        // reconnect to contract to avoid connection reset problem
        let client = get_funded_client().await?;
        let contract = TestVerifyingKeys::new(contract.address(), client);

        let note_type_sol = match note_type {
            NoteType::Transfer => 0u8,
            NoteType::Mint => 1u8,
            NoteType::Freeze => 2u8,
        };
        let vk_id = contract
            .get_encoded_id(note_type_sol, num_input, num_output, tree_depth)
            .call()
            .await?;
        assert_eq!(contract.get_vk_by_id(vk_id).call().await?, vk);
    }

    Ok(())
}
