#![cfg_attr(debug_assertions, allow(dead_code))]
use anyhow::Result;
use ethers::{core::k256::ecdsa::SigningKey, prelude::*, utils::CompiledContract};
use rand;
use std::{convert::TryFrom, fs, path::Path, sync::Arc, time::Duration};

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

async fn load_contract(path: &Path) -> Result<CompiledContract> {
    let abi_path = path.join("abi.json");
    let bin_path = path.join("bin.txt");

    let abi = ethers::abi::Contract::load(match fs::File::open(&abi_path) {
        Ok(v) => v,
        Err(_) => panic!("Unable to open path {:?}", abi_path),
    })?;

    let bytecode_str = match fs::read_to_string(&bin_path) {
        Ok(v) => v,
        Err(_) => panic!("Unable to read from path {:?}", bin_path),
    };
    let trimmed = bytecode_str.trim().trim_start_matches("0x");
    let bytecode = match hex::decode(&trimmed) {
        Ok(v) => v,
        Err(_) => {
            panic!("Cannot parse hex {:?}", trimmed)
        }
    }
    .into();

    Ok(CompiledContract {
        abi,
        bytecode,
        runtime_bytecode: Default::default(),
    })
}

// TODO: why do we need 'static ?
// https://docs.rs/anyhow/1.0.44/anyhow/struct.Error.html ?
pub async fn deploy<C: 'static + Middleware>(client: Arc<C>, path: &Path) -> Result<Contract<C>> {
    let contract = load_contract(&path).await?;
    let factory = ContractFactory::new(
        contract.abi.clone(),
        contract.bytecode.clone(),
        client.clone(),
    );
    let contract = factory.deploy(())?.send().await?;
    Ok(contract)
}
