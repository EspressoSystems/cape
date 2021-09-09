use anyhow::Result;
use rand;
use ethers::{
    prelude::*,
    utils::{compile, Solc},
    signers::Signer,
};
use std::{convert::TryFrom, sync::Arc, time::Duration};

// Generate the type-safe contract bindings by providing the ABI
// definition in human readable format
abigen!(
    SimpleContract,
    "./examples/contract_abi.json",
    event_derives(serde::Deserialize, serde::Serialize)
);

#[tokio::main]
async fn main() -> Result<()> {

    let compiled = compile(Solc::new("**/contract.sol")).await?;
    let contract = compiled
        .get("SimpleStorage")
        .expect("could not find contract");
    dbg!("Compiled!");

    // start "geth --dev --http" in another terminal
    let provider = Provider::<Http>::try_from("http://localhost:8545")
        .expect("could not instantiate HTTP Provider")
        .interval(Duration::from_millis(500u64));
    let chain_id = provider.get_chainid().await.unwrap().as_u64();

    // fund deployer account
    let coinbase = provider.get_accounts().await.unwrap()[0];
    let deployer_wallet = LocalWallet::new(&mut rand::thread_rng())
        .with_chain_id(chain_id); // XXX setting chain_id seems to be required

    let tx = TransactionRequest::new()
        .to(deployer_wallet.address())
        .value(u64::pow(10, 18))
        .from(coinbase); // specify the `from` field so that the client knows which account to use
    provider.send_transaction(tx, None).await?.await?;

    dbg!("Sent funding tx to deployer");


    // Deploy contract
    let deployer_client =  Arc::new(SignerMiddleware::new(provider.clone(), deployer_wallet.clone()));
    let factory = ContractFactory::new(
        contract.abi.clone(),
        contract.bytecode.clone(),
        deployer_client.clone(),
    );

    let contract = factory
        .deploy("initial value".to_string())?
        .legacy()
        .send()
        .await?;

    // 7. get the contract's address
    let addr = contract.address();

    // 8. instantiate the contract
    let contract = SimpleContract::new(addr, deployer_client);

    // 9. call the `setValue` method
    // (first `await` returns a PendingTransaction, second one waits for it to be mined)
    let _receipt = contract
        .set_value("hi".to_owned())
        .legacy()
        .send()
        .await?
        .await?;

    // 10. get all events
    let logs = contract
        .value_changed_filter()
        .from_block(0u64)
        .query()
        .await?;

    // 11. get the new value
    let value = contract.get_value().call().await?;

    println!("Value: {}. Logs: {}", value, serde_json::to_string(&logs)?);

    Ok(())
}
