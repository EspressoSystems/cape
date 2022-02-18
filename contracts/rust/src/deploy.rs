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

pub async fn deploy_cape_test() -> TestCAPE<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>> {
    let client = get_funded_client().await.unwrap();
    // deploy the PlonkVerifier
    let verifier = deploy(
        client.clone(),
        &contract_abi_path("verifier/PlonkVerifier.sol/PlonkVerifier"),
        (),
    )
    .await
    .unwrap();

    // deploy TestCAPE.sol
    let contract = deploy(
        client.clone(),
        &contract_abi_path("mocks/TestCAPE.sol/TestCAPE"),
        CAPEConstructorArgs::new(CAPE_MERKLE_HEIGHT, 1000, verifier.address())
            .generic_into::<(u8, u64, Address)>(),
    )
    .await
    .unwrap();
    TestCAPE::new(contract.address(), client)
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
    TestCapeTypes<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>,
    "mocks/TestCapeTypes.sol/TestCapeTypes"
);

mk_deploy_fun!(
    deploy_erc20_token,
    SimpleToken<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>,
    "SimpleToken.sol/SimpleToken"
);
mk_deploy_fun!(
    deploy_test_plonk_verifier_contract,
    TestPlonkVerifier<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>,
    "mocks/TestPlonkVerifier.sol/TestPlonkVerifier"
);
mk_deploy_fun!(
    deploy_test_polynomial_eval_contract,
    TestPolynomialEval<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>,
    "mocks/TestPolynomialEval.sol/TestPolynomialEval"
);
mk_deploy_fun!(
    deploy_test_verifying_keys_contract,
    TestVerifyingKeys<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>,
    "mocks/TestVerifyingKeys.sol/TestVerifyingKeys"
);
mk_deploy_fun!(
    deploy_test_asset_registry_contract,
    AssetRegistry<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>,
    "AssetRegistry.sol/AssetRegistry"
);
mk_deploy_fun!(
    deploy_test_rescue,
    TestRescue<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>,
    "mocks/TestRescue.sol/TestRescue"
);
mk_deploy_fun!(
    deploy_test_bn254_contract,
    TestBN254<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>,
    "mocks/TestBN254.sol/TestBN254"
);
mk_deploy_fun!(
    deploy_test_ed_on_bn_254_contract,
    TestEdOnBN254<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>,
    "mocks/TestEdOnBN254.sol/TestEdOnBN254"
);

// We do not call the macro for the contracts below because they take some argument
pub async fn deploy_greeter_contract(
) -> Result<Greeter<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>> {
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

pub async fn deploy_test_root_store_contract(
) -> TestRootStore<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>> {
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

pub async fn deploy_test_transcript_contract(
) -> TestTranscript<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>> {
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
) -> TestRecordsMerkleTree<
    SignerMiddleware<Provider<Http>, Wallet<ethers::core::k256::ecdsa::SigningKey>>,
> {
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
