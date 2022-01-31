use crate::cape::CAPEConstructorArgs;
use crate::ethereum::{deploy, get_funded_client};
use crate::state::CAPE_MERKLE_HEIGHT;
use crate::types::{GenericInto, Greeter, SimpleToken, TestCAPE, TestQueue};
use anyhow::Result;
use ethers::prelude::{k256::ecdsa::SigningKey, Address, Http, Provider, SignerMiddleware, Wallet};
use std::path::Path;

pub async fn deploy_cape_test() -> TestCAPE<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>> {
    let client = get_funded_client().await.unwrap();
    // deploy the PlonkVerifier
    let verifier = deploy(
        client.clone(),
        Path::new("../abi/contracts/verifier/PlonkVerifier.sol/PlonkVerifier"),
        (),
    )
    .await
    .unwrap();

    // deploy TestCAPE.sol
    let contract = deploy(
        client.clone(),
        Path::new("../abi/contracts/mocks/TestCAPE.sol/TestCAPE"),
        CAPEConstructorArgs::new(CAPE_MERKLE_HEIGHT, 10, verifier.address())
            .generic_into::<(u8, u64, Address)>(),
    )
    .await
    .unwrap();
    TestCAPE::new(contract.address(), client)
}

pub async fn deploy_queue_test() -> TestQueue<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>
{
    let client = get_funded_client().await.unwrap();
    let call = deploy(
        client.clone(),
        Path::new("../abi/contracts/mocks/TestQueue.sol/TestQueue"),
        (),
    )
    .await;
    let contract = call.unwrap();
    TestQueue::new(contract.address(), client)
}

pub async fn deploy_erc20_token(
) -> SimpleToken<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>> {
    let client = get_funded_client().await.unwrap();
    let call = deploy(
        client.clone(),
        Path::new("../abi/contracts/SimpleToken.sol/SimpleToken"),
        (),
    )
    .await;
    let contract = call.unwrap();
    SimpleToken::new(contract.address(), client)
}

pub async fn deploy_greeter_contract(
) -> Result<Greeter<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>> {
    let client = get_funded_client().await.unwrap();
    let contract = deploy(
        client.clone(),
        Path::new("../abi/contracts/Greeter.sol/Greeter"),
        ("Initial Greeting".to_string(),),
    )
    .await
    .unwrap();
    Ok(Greeter::new(contract.address(), client))
}
