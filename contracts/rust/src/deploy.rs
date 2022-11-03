// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

use crate::assertion::EnsureMined;
use crate::cape::{CAPEConstructorArgs, RecordsMerkleTreeConstructorArgs};
use crate::ethereum::{deploy, get_funded_client};
use crate::model::{CAPE_MERKLE_HEIGHT, CAPE_NUM_ROOTS};
use crate::test_utils::contract_abi_path;
use crate::types::{
    AssetRegistry, MaliciousToken, RecordsMerkleTree, SimpleToken, TestBN254, TestCAPE,
    TestCapeTypes, TestEdOnBN254, TestPlonkVerifier, TestPolynomialEval, TestRescue, TestRootStore,
    TestTranscript, TestVerifyingKeys, CAPE,
};
use ethers::prelude::{k256::ecdsa::SigningKey, Http, Provider, SignerMiddleware, Wallet};
use ethers::types::Address;
use std::env;
use std::sync::Arc;

// Middleware used for locally signing transactions
pub type EthMiddleware = SignerMiddleware<Provider<Http>, Wallet<SigningKey>>;

pub async fn deploy_test_cape() -> TestCAPE<EthMiddleware> {
    let client = get_funded_client().await.unwrap();
    deploy_test_cape_with_deployer(client).await
}

pub async fn deploy_cape() -> CAPE<EthMiddleware> {
    let client = get_funded_client().await.unwrap();
    deploy_cape_with_deployer(client).await
}

pub async fn deploy_test_cape_with_deployer(
    deployer: Arc<EthMiddleware>,
) -> TestCAPE<EthMiddleware> {
    // deploy the PlonkVerifier

    // If VERIFIER_ADDRESS is set, use that address instead of deploying a new
    // contract.
    let verifier_address = match env::var("VERIFIER_ADDRESS") {
        Ok(val) => {
            println!("Using Verifier at {val}");
            val.parse::<Address>().unwrap()
        }
        Err(_) => deploy(
            deployer.clone(),
            &contract_abi_path("verifier/PlonkVerifier.sol/PlonkVerifier"),
            (),
        )
        .await
        .unwrap()
        .address(),
    };

    let records_merkle_tree = deploy(
        deployer.clone(),
        &contract_abi_path("RecordsMerkleTree.sol/RecordsMerkleTree"),
        RecordsMerkleTreeConstructorArgs::new(CAPE_MERKLE_HEIGHT).to_tuple(),
    )
    .await
    .unwrap();

    // deploy TestCAPE.sol
    let cape = deploy(
        deployer.clone(),
        &contract_abi_path("mocks/TestCAPE.sol/TestCAPE"),
        CAPEConstructorArgs::new(
            CAPE_NUM_ROOTS as u64,
            verifier_address,
            records_merkle_tree.address(),
        )
        .to_tuple(),
    )
    .await
    .unwrap();

    RecordsMerkleTree::new(records_merkle_tree.address(), deployer.clone())
        .transfer_ownership(cape.address())
        .send()
        .await
        .unwrap()
        .await
        .unwrap()
        .ensure_mined();

    TestCAPE::new(cape.address(), deployer)
}

pub async fn deploy_cape_with_deployer(deployer: Arc<EthMiddleware>) -> CAPE<EthMiddleware> {
    // deploy the PlonkVerifier
    let verifier = deploy(
        deployer.clone(),
        &contract_abi_path("verifier/PlonkVerifier.sol/PlonkVerifier"),
        (),
    )
    .await
    .unwrap();

    let records_merkle_tree = deploy(
        deployer.clone(),
        &contract_abi_path("RecordsMerkleTree.sol/RecordsMerkleTree"),
        RecordsMerkleTreeConstructorArgs::new(CAPE_MERKLE_HEIGHT).to_tuple(),
    )
    .await
    .unwrap();

    // deploy CAPE.sol
    let cape = deploy(
        deployer.clone(),
        &contract_abi_path("CAPE.sol/CAPE"),
        CAPEConstructorArgs::new(
            CAPE_NUM_ROOTS as u64,
            verifier.address(),
            records_merkle_tree.address(),
        )
        .to_tuple(),
    )
    .await
    .unwrap();

    RecordsMerkleTree::new(records_merkle_tree.address(), deployer.clone())
        .transfer_ownership(cape.address())
        .send()
        .await
        .unwrap()
        .await
        .unwrap()
        .ensure_mined();

    CAPE::new(cape.address(), deployer)
}

macro_rules! mk_deploy_fun {
    ($func:ident, $output_type:ty, $path:expr) => {
        pub async fn $func() -> $output_type {
            let client = get_funded_client().await.unwrap();
            let call = deploy(client.clone(), &contract_abi_path($path), ()).await;
            let contract = call.unwrap();
            <$output_type>::new(contract.address(), client)
        }
    };
}

mk_deploy_fun!(
    deploy_test_cape_types_contract,
    TestCapeTypes<EthMiddleware>,
    "mocks/TestCapeTypes.sol/TestCapeTypes"
);

mk_deploy_fun!(
    deploy_erc20_token,
    SimpleToken<EthMiddleware>,
    "SimpleToken.sol/SimpleToken"
);
mk_deploy_fun!(
    deploy_malicious_erc20_token,
    MaliciousToken<EthMiddleware>,
    "MaliciousToken.sol/MaliciousToken"
);
mk_deploy_fun!(
    deploy_test_plonk_verifier_contract,
    TestPlonkVerifier<EthMiddleware>,
    "mocks/TestPlonkVerifier.sol/TestPlonkVerifier"
);
mk_deploy_fun!(
    deploy_test_polynomial_eval_contract,
    TestPolynomialEval<EthMiddleware>,
    "mocks/TestPolynomialEval.sol/TestPolynomialEval"
);
mk_deploy_fun!(
    deploy_test_verifying_keys_contract,
    TestVerifyingKeys<EthMiddleware>,
    "mocks/TestVerifyingKeys.sol/TestVerifyingKeys"
);
mk_deploy_fun!(
    deploy_test_asset_registry_contract,
    AssetRegistry<EthMiddleware>,
    "AssetRegistry.sol/AssetRegistry"
);
mk_deploy_fun!(
    deploy_test_rescue,
    TestRescue<EthMiddleware>,
    "mocks/TestRescue.sol/TestRescue"
);
mk_deploy_fun!(
    deploy_test_bn254_contract,
    TestBN254<EthMiddleware>,
    "mocks/TestBN254.sol/TestBN254"
);
mk_deploy_fun!(
    deploy_test_ed_on_bn_254_contract,
    TestEdOnBN254<EthMiddleware>,
    "mocks/TestEdOnBN254.sol/TestEdOnBN254"
);

// We do not call the macro for the contracts below because they take some argument
pub async fn deploy_test_root_store_contract() -> TestRootStore<EthMiddleware> {
    let client = get_funded_client().await.unwrap();
    let contract = deploy(
        client.clone(),
        &contract_abi_path("mocks/TestRootStore.sol/TestRootStore"),
        (3u64,), /* num_roots */
    )
    .await
    .unwrap();
    TestRootStore::new(contract.address(), client)
}

pub async fn deploy_test_transcript_contract() -> TestTranscript<EthMiddleware> {
    let client = get_funded_client().await.unwrap();
    let contract = deploy(
        client.clone(),
        &contract_abi_path("mocks/TestTranscript.sol/TestTranscript"),
        (),
    )
    .await
    .unwrap();
    TestTranscript::new(contract.address(), client)
}

pub async fn deploy_records_merkle_tree_contract(height: u8) -> RecordsMerkleTree<EthMiddleware> {
    let client = get_funded_client().await.unwrap();
    let contract = deploy(
        client.clone(),
        &contract_abi_path("RecordsMerkleTree.sol/RecordsMerkleTree"),
        height,
    )
    .await
    .unwrap();
    RecordsMerkleTree::new(contract.address(), client)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::ethereum::{
        ensure_connected_to_contract, get_provider, has_code_at_block, is_connected_to_contract,
    };
    use anyhow::Result;
    use ethers::prelude::{Address, Middleware};

    #[tokio::test]
    async fn test_is_connected_to_contract() -> Result<()> {
        let provider = get_provider();
        let block_before = provider.get_block_number().await?;
        let contract = deploy_test_cape().await;

        // Checking a random address returns false.
        assert!(!is_connected_to_contract(&provider, Address::random()).await?);

        // Checking the correct address returns true.
        assert!(is_connected_to_contract(&provider, contract.address()).await?);

        // Check this doesn't panic.
        ensure_connected_to_contract(&provider, contract.address()).await?;

        // Checking the correct address before the contract was deployed returns false.
        assert!(
            !has_code_at_block(&provider, contract.address(), Some(block_before.into())).await?
        );

        Ok(())
    }

    #[tokio::test]
    #[should_panic]
    async fn test_ensure_connected_panics_if_not_connected() {
        let provider = get_provider();
        ensure_connected_to_contract(&provider, Address::random())
            .await
            .unwrap();
    }
}
