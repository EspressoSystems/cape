use crate::cape::CAPEConstructorArgs;
use crate::ethereum::{deploy, get_funded_client};
use crate::state::CAPE_MERKLE_HEIGHT;
use crate::test_utils::contract_abi_path;
use crate::types::{
    AssetRegistry, GenericInto, Greeter, SimpleToken, TestBN254, TestCAPE, TestCapeTypes,
    TestEdOnBN254, TestPlonkVerifier, TestPolynomialEval, TestRecordsMerkleTree, TestRescue,
    TestRootStore, TestTranscript, TestVerifyingKeys,
};
use anyhow::Result;
use ethers::prelude::{k256::ecdsa::SigningKey, Address, Http, Provider, SignerMiddleware, Wallet};
use std::sync::Arc;

// Middleware used for locally signing transactions
pub type EthMiddleware = SignerMiddleware<Provider<Http>, Wallet<SigningKey>>;

pub async fn deploy_cape_test() -> TestCAPE<EthMiddleware> {
    let client = get_funded_client().await.unwrap();
    deploy_cape_test_with_deployer(client).await
}

pub async fn deploy_cape_test_with_deployer(
    deployer: Arc<EthMiddleware>,
) -> TestCAPE<EthMiddleware> {
    // deploy the PlonkVerifier
    let verifier = deploy(
        deployer.clone(),
        &contract_abi_path("verifier/PlonkVerifier.sol/PlonkVerifier"),
        (),
    )
    .await
    .unwrap();

    // deploy TestCAPE.sol
    let contract = deploy(
        deployer.clone(),
        &contract_abi_path("mocks/TestCAPE.sol/TestCAPE"),
        CAPEConstructorArgs::new(CAPE_MERKLE_HEIGHT, 1000, verifier.address())
            .generic_into::<(u8, u64, Address)>(),
    )
    .await
    .unwrap();
    TestCAPE::new(contract.address(), deployer)
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
pub async fn deploy_greeter_contract() -> Result<Greeter<EthMiddleware>> {
    let client = get_funded_client().await.unwrap();
    let contract = deploy(
        client.clone(),
        &contract_abi_path("Greeter.sol/Greeter"),
        ("Initial Greeting".to_string(),),
    )
    .await
    .unwrap();
    Ok(Greeter::new(contract.address(), client))
}

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

pub async fn deploy_test_records_merkle_tree_contract(
    height: u8,
) -> TestRecordsMerkleTree<EthMiddleware> {
    let client = get_funded_client().await.unwrap();
    let contract = deploy(
        client.clone(),
        &contract_abi_path("mocks/TestRecordsMerkleTree.sol/TestRecordsMerkleTree"),
        height,
    )
    .await
    .unwrap();
    TestRecordsMerkleTree::new(contract.address(), client)
}
