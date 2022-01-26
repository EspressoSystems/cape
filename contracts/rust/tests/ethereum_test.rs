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
async fn test_basic_contract_deployment() -> Result<()> {
    let contract = deploy_contract().await?;
    assert_eq!(contract.greet().call().await?, "Initial Greeting");

    Ok(())
}

#[tokio::test]
async fn test_basic_contract_transaction() -> Result<()> {
    let contract = deploy_contract().await?;
    let greeting = String::from("Hi!");

    let _receipt = contract
        .method::<_, String>("setGreeting", greeting.clone())?
        .send()
        .await?
        .confirmations(0)
        .await?;
    assert_eq!(contract.greet().call().await?, greeting);

    Ok(())
}
