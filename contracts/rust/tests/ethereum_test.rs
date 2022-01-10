use anyhow::Result;
use cap_rust_sandbox::{ethereum::*, types::Greeter};
use ethers::{core::k256::ecdsa::SigningKey, prelude::*};
use std::path::Path;

async fn deploy_contract() -> Result<Greeter<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>>
{
    let client = get_funded_deployer().await.unwrap();
    let contract = deploy(
        client.clone(),
        Path::new("../abi/contracts/Greeter.sol/Greeter"),
        ("Initial Greeting".to_string(),),
    )
    .await
    .unwrap();
    Ok(Greeter::new(contract.address(), client))
}

#[tokio::test]
async fn test_basic_contract_deployment() {
    let contract = deploy_contract().await.unwrap();
    let res: String = contract.greet().call().await.unwrap().into();
    assert_eq!(res, "Initial Greeting")
}

#[tokio::test]
async fn test_basic_contract_transaction() {
    let contract = deploy_contract().await.unwrap();
    let _receipt = contract
        .set_greeting("Hi!".to_string())
        .legacy()
        .send()
        .await
        .unwrap()
        .await
        .unwrap()
        .expect("Failed to get TX receipt");

    let res: String = contract.greet().call().await.unwrap();
    assert_eq!(res, "Hi!");
}
