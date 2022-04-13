// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

pub const FAUCET_MANAGER_ENCRYPTION_KEY: &str = "USERPUBKEY~muN7VKxj1GbJ4D6rU6gANdvwD05oPKy_XmhkBxSByq0gAAAAAAAAAIRN-Rik8czFiToI8Ft5fsIf9HAEtWHDsOHh-ZBJZl1KxQ";

#[cfg(test)]
mod test {
    use super::FAUCET_MANAGER_ENCRYPTION_KEY;
    use crate::{
        assertion::Matcher,
        deploy::deploy_test_cape_with_deployer,
        ethereum::get_funded_client,
        model::CAPE_MERKLE_HEIGHT,
        types::{self as sol, field_to_u256, GenericInto, TestCAPE, CAPE},
    };
    use anyhow::Result;
    use ethers::{abi::AbiDecode, prelude::Address};
    use jf_cap::{
        keys::{UserKeyPair, UserPubKey},
        structs::{AssetDefinition, BlindFactor, FreezeFlag, RecordCommitment, RecordOpening},
        BaseField, MerkleTree,
    };
    use regex::Regex;
    use std::{process::Command, str::FromStr};

    #[tokio::test]
    async fn test_faucet() -> Result<()> {
        let rng = &mut ark_std::test_rng();
        let deployer = get_funded_client().await?;
        let non_deployer = get_funded_client().await?;
        let contract = deploy_test_cape_with_deployer(deployer.clone()).await;
        let faucet_manager = UserKeyPair::generate(rng);

        // after Cape deployment, faucet is not yet setup
        assert!(!contract.faucet_initialized().call().await?);
        assert_eq!(contract.deployer().call().await?, deployer.address());

        // attempts to setup faucet by non deployer should fail
        let contract = TestCAPE::new(contract.address(), non_deployer);
        contract
            .faucet_setup_for_testnet(
                faucet_manager.address().into(),
                faucet_manager.pub_key().enc_key().into(),
            )
            .send()
            .await
            .should_revert_with_message("Only invocable by deployer");

        // setting up
        let contract = TestCAPE::new(contract.address(), deployer);
        contract
            .faucet_setup_for_testnet(
                faucet_manager.address().into(),
                faucet_manager.pub_key().enc_key().into(),
            )
            .send()
            .await?
            .await?;
        assert!(contract.faucet_initialized().call().await?);

        // try to setup again should fail
        contract
            .faucet_setup_for_testnet(
                faucet_manager.address().into(),
                faucet_manager.pub_key().enc_key().into(),
            )
            .send()
            .await
            .should_revert_with_message("Faucet already set up");

        // check the native token record is properly allocated
        let ro = RecordOpening {
            amount: u64::MAX / 2,
            asset_def: AssetDefinition::native(),
            pub_key: faucet_manager.pub_key(),
            freeze_flag: FreezeFlag::Unfrozen,
            blind: BlindFactor::from(BaseField::from(0u32)),
        };

        // Check FaucetInitialized event with matching RecordOpening was emitted
        let events = contract
            .faucet_initialized_filter()
            .from_block(0u64)
            .query()
            .await?;
        let event_ro: sol::RecordOpening = AbiDecode::decode(events[0].ro_bytes.clone()).unwrap();
        assert_eq!(event_ro.generic_into::<RecordOpening>(), ro);

        let mut mt = MerkleTree::new(CAPE_MERKLE_HEIGHT).unwrap();
        mt.push(RecordCommitment::from(&ro).to_field_element());
        let root: ark_bn254::Fr = mt.commitment().root_value.to_scalar();

        assert!(contract.contains_root(field_to_u256(root)).call().await?);

        Ok(())
    }

    // This test sometimes fails for currently unclear reasons.
    #[ignore]
    #[tokio::test]
    async fn test_hardhat_deploy() -> Result<()> {
        let output = Command::new("hardhat")
            .arg("deploy")
            .arg("--reset")
            .output()
            .expect("failed to deploy");
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
}
