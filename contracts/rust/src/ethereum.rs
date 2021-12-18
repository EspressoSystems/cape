#![cfg_attr(debug_assertions, allow(dead_code))]
use anyhow::Result;
use ethers::{
    abi::{Abi, Tokenize},
    core::k256::ecdsa::SigningKey,
    prelude::{
        coins_bip39::English, Bytes, Contract, ContractFactory, Http, LocalWallet, Middleware,
        MnemonicBuilder, Provider, Signer, SignerMiddleware, TransactionRequest, Wallet, U256,
    },
};
use std::{convert::TryFrom, env, fs, path::Path, sync::Arc, time::Duration};

pub async fn get_funded_deployer(
) -> Result<Arc<SignerMiddleware<Provider<Http>, Wallet<SigningKey>>>> {
    let rpc_url = match env::var("RPC_URL") {
        Ok(val) => val,
        Err(_) => "http://localhost:8545".to_string(),
    };

    let mut provider =
        Provider::<Http>::try_from(rpc_url.clone()).expect("could not instantiate HTTP Provider");

    let chain_id = provider.get_chainid().await.unwrap().as_u64();

    // If MNEMONIC is set, try to use it to create a wallet,
    // otherwise create a random wallet.
    let deployer_wallet = match env::var("MNEMONIC") {
        Ok(val) => MnemonicBuilder::<English>::default()
            .phrase(val.as_str())
            .build()?,
        Err(_) => LocalWallet::new(&mut rand::thread_rng()),
    }
    .with_chain_id(chain_id);

    // Fund the deployer if we have unlocked accounts
    let accounts = provider.get_accounts().await.unwrap();
    if !accounts.is_empty() {
        let tx = TransactionRequest::new()
            .to(deployer_wallet.address())
            .value(ethers::utils::parse_ether(U256::from(1))?)
            .from(accounts[0]);

        // Set a lower polling interval to avoid very slow tests
        provider = provider.interval(Duration::from_millis(100u64));
        provider.send_transaction(tx, None).await?.await?;
        println!("Sent funding tx to deployer");
    }

    Ok(Arc::new(SignerMiddleware::new(provider, deployer_wallet)))
}

async fn load_contract(path: &Path) -> Result<(Abi, Bytes)> {
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

    Ok((abi, bytecode))
}

// TODO: why do we need 'static ?
// https://docs.rs/anyhow/1.0.44/anyhow/struct.Error.html ?
pub async fn deploy<C: 'static + Middleware, T: Tokenize>(
    client: Arc<C>,
    path: &Path,
    constructor_args: T,
) -> Result<Contract<C>> {
    let (abi, bytecode) = load_contract(path).await?;
    let factory = ContractFactory::new(abi.clone(), bytecode.clone(), client.clone());
    let contract = factory
        .deploy(constructor_args)?
        .legacy() // XXX This is required!
        .send()
        .await?;
    Ok(contract)
}
