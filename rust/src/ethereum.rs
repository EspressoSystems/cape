#![cfg_attr(debug_assertions, allow(dead_code))]
use anyhow::Result;
use ethers::{
    core::k256::ecdsa::SigningKey,
    prelude::*,
    utils::{compile, CompiledContract, Solc},
};
use rand;
use std::{convert::TryFrom, sync::Arc, time::Duration};

pub async fn get_funded_deployer(
) -> Result<Arc<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>> {
    let provider = Provider::<Http>::try_from("http://localhost:8545")
        .expect("could not instantiate HTTP Provider")
        .interval(Duration::from_millis(100u64));

    let chain_id = provider.get_chainid().await.unwrap().as_u64();

    // fund deployer account
    let coinbase = provider.get_accounts().await.unwrap()[0];
    let deployer_wallet = LocalWallet::new(&mut rand::thread_rng()).with_chain_id(chain_id); // XXX setting chain_id seems to be required

    let tx = TransactionRequest::new()
        .to(deployer_wallet.address())
        .value(u64::pow(10, 18))
        .from(coinbase);

    provider.send_transaction(tx, None).await?.await?;

    println!("Sent funding tx to deployer");

    Ok(Arc::new(SignerMiddleware::new(
        provider,
        deployer_wallet.clone(),
    )))
}

async fn compile_contract(path: &String, name: &String) -> Result<CompiledContract> {
    let compiled = compile(Solc::new(path).allowed_paths(vec!["../contracts".into()])).await?;
    Ok(compiled.get(name).expect("could not find contract").clone())
}

// TODO: why do we need 'static ?
pub async fn deploy<C: 'static + Middleware>(
    client: Arc<C>,
    path: &String,
    name: &String,
) -> Result<Contract<C>> {
    let contract = compile_contract(&path, &name).await?;
    let factory = ContractFactory::new(
        contract.abi.clone(),
        contract.bytecode.clone(),
        client.clone(),
    );
    let contract = factory.deploy(())?.send().await?;
    Ok(contract)
}
