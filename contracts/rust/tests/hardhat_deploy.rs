// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.
use anyhow::Result;
use cap_rust_sandbox::{
    cape::faucet::FAUCET_MANAGER_ENCRYPTION_KEY,
    ethereum::get_funded_client,
    types::{self as sol, GenericInto, CAPE},
};
use ethers::{abi::AbiDecode, prelude::Address};
use jf_cap::{keys::UserPubKey, structs::RecordOpening};
use regex::Regex;
use std::{process::Command, str::FromStr};

/// This test sometimes fails if it runs as part of the rust test suite. This is
/// likely because it deploys the contracts with the unlocked account which is
/// also used to fund the random addresses used for deploying contracts in the
/// rust test suite. Having a separate integration test file for this test frees
/// it from interference (at the expense of compiling a separate binary).
#[tokio::test]
async fn test_hardhat_deploy() -> Result<()> {
    let output = Command::new("hardhat")
        .arg("deploy")
        .arg("--reset")
        .output()
        .expect("\"hardhat deploy --reset\" failed to execute");

    if !output.status.success() {
        panic!(
            "Command \"hardhat deploy --reset\" exited with error: {}",
            String::from_utf8(output.stderr)?,
        )
    }

    let text = String::from_utf8(output.stdout).unwrap();
    // Get the address out of
    // deploying "CAPE" (tx: 0x64...211)...: deployed at 0x8A791620dd6260079BF849Dc5567aDC3F2FdC318 with 7413790 gas
    let re = Regex::new(r#""CAPE".*(0x[0-9a-fA-F]{40})"#).unwrap();
    let address = re
        .captures_iter(&text)
        .next()
        .unwrap_or_else(|| panic!("Address not found in {}", text))[1]
        .parse::<Address>()
        .unwrap_or_else(|_| panic!("Address not found in {}", text));

    let client = get_funded_client().await.unwrap();
    let contract = CAPE::new(address, client.clone());
    let event = contract
        .faucet_initialized_filter()
        .from_block(0u64)
        .query()
        .await?[0]
        .clone();
    let ro_sol: sol::RecordOpening = AbiDecode::decode(event.ro_bytes).unwrap();

    // Check that the faucet record opening in the deployed contract is the
    // same as the hardcoded one in this crate.
    assert_eq!(
        UserPubKey::from_str(FAUCET_MANAGER_ENCRYPTION_KEY).unwrap(),
        ro_sol.generic_into::<RecordOpening>().pub_key,
    );

    Ok(())
}
