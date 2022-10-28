// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

#![cfg_attr(debug_assertions, allow(dead_code))]
use crate::{
    deploy::{deploy_test_cape_with_deployer, EthMiddleware},
    test_utils::contract_abi_path,
    types::{TestCAPE, CAPE},
};
use anyhow::Result;
use async_recursion::async_recursion;
use ethers::{
    abi::{Abi, Tokenize},
    contract::Contract,
    prelude::{
        artifacts::BytecodeObject, coins_bip39::English, Address, BlockId, ContractFactory, Http,
        LocalWallet, Middleware, MnemonicBuilder, Provider, Signer, SignerMiddleware,
        TransactionRequest, U256,
    },
};

use std::{convert::TryFrom, env, fs, path::Path, sync::Arc, time::Duration};

/// Utility to interact with the CAPE contract on some Ethereum blockchain
#[derive(Clone, Debug)]
pub struct EthConnection {
    pub provider: Provider<Http>,
    pub client: Arc<EthMiddleware>,
    pub contract: CAPE<EthMiddleware>,
}

impl EthConnection {
    /// Deploy a test contract and connect to that
    pub async fn for_test() -> Self {
        let provider = get_provider();
        let client = get_funded_client().await.unwrap();
        let contract = deploy_test_cape_with_deployer(client.clone()).await;
        Self::connect(provider, client, contract.address())
    }

    /// Connect to an existing contract to query its state
    /// * `contract_address` example: "0x60417B5Ad8629778A46A2cAaA924D7498618622B"
    /// * `provider_url` example: "http://localhost:8545"
    pub fn from_config_for_query(contract_address: &str, provider_url: &str) -> Self {
        let provider =
            Provider::<Http>::try_from(provider_url).expect("could not instantiate HTTP Provider");

        let dummy_wallet = LocalWallet::new(&mut rand::thread_rng());
        Self::connect(
            provider.clone(),
            Arc::new(SignerMiddleware::new(provider, dummy_wallet)),
            contract_address
                .parse::<Address>()
                .expect("could no parse contract_address"),
        )
    }

    /// Connect to an existing contract at `contract_address`
    pub fn connect(
        provider: Provider<Http>,
        client: Arc<EthMiddleware>,
        contract_address: Address,
    ) -> Self {
        Self {
            contract: CAPE::new(contract_address, client.clone()),
            client,
            provider,
        }
    }

    /// Get a TestCAPE contract object for calling functions only available on
    /// the test contact. Do not use this if connected to a real CAPE contract.
    pub fn test_contract(&self) -> TestCAPE<EthMiddleware> {
        TestCAPE::new(self.contract.address(), self.client.clone())
    }
}

/// Obtain the Web3 provider using some default url or an environment variable.
pub fn get_provider() -> Provider<Http> {
    let rpc_url = match env::var("CAPE_WEB3_PROVIDER_URL") {
        Ok(url) => url,
        Err(_) => "http://localhost:8545".to_string(),
    };
    get_provider_from_url(&rpc_url)
}

/// Obtain the Web3 provider from an url.
pub fn get_provider_from_url(rpc_url: &str) -> Provider<Http> {
    Provider::<Http>::try_from(rpc_url).expect("could not instantiate HTTP Provider")
}

/// Obtain a client already funded with 1 ETH.
pub async fn get_funded_client() -> Result<Arc<EthMiddleware>> {
    let mut provider = get_provider();
    let chain_id = provider.get_chainid().await.unwrap().as_u64();

    // If MNEMONIC is set, try to use it to create a wallet,
    // otherwise create a random wallet.
    let deployer_wallet = match env::var("ETH_MNEMONIC") {
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

async fn load_contract(path: &Path) -> Result<(Abi, BytecodeObject)> {
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
    let bytecode: BytecodeObject = serde_json::from_value(serde_json::json!(trimmed)).unwrap();

    Ok((abi, bytecode))
}

async fn link_unlinked_libraries<M: 'static + Middleware>(
    bytecode: &mut BytecodeObject,
    client: &Arc<M>,
) -> Result<()> {
    if bytecode.contains_fully_qualified_placeholder("contracts/libraries/RescueLib.sol:RescueLib")
    {
        // Connect to linked library if env var with address is set
        // otherwise, deploy the library.
        let rescue_lib_address = match env::var("RESCUE_LIB_ADDRESS") {
            Ok(val) => {
                println!("Using RescueLib library at {val}");
                val.parse::<Address>()?
            }
            Err(_) => deploy(
                client.clone(),
                &contract_abi_path("libraries/RescueLib.sol/RescueLib"),
                (),
            )
            .await?
            .address(),
        };
        bytecode
            .link(
                "contracts/libraries/RescueLib.sol",
                "RescueLib",
                rescue_lib_address,
            )
            .resolve();
    }

    if bytecode
        .contains_fully_qualified_placeholder("contracts/libraries/VerifyingKeys.sol:VerifyingKeys")
    {
        // Connect to linked library if env var with address is set
        // otherwise, deploy the library.
        let verifying_keys_lib_address = match env::var("VERIFYING_KEYS_LIB_ADDRESS") {
            Ok(val) => {
                println!("Using VerifyingKeys library at {val}");
                val.parse::<Address>()?
            }
            Err(_) => deploy(
                client.clone(),
                &contract_abi_path("libraries/VerifyingKeys.sol/VerifyingKeys"),
                (),
            )
            .await?
            .address(),
        };
        bytecode
            .link(
                "contracts/libraries/VerifyingKeys.sol",
                "VerifyingKeys",
                verifying_keys_lib_address,
            )
            .resolve();
    }

    Ok(())
}

// TODO: why do we need 'static ?
// https://docs.rs/anyhow/1.0.44/anyhow/struct.Error.html ?
/// Deploy a contract linking it to some (possibly already deployed) libraries.
#[async_recursion]
pub async fn deploy<M: 'static + Middleware, T: Send + Tokenize>(
    client: Arc<M>,
    path: &Path,
    constructor_args: T,
) -> Result<Contract<M>> {
    let (abi, mut bytecode) = load_contract(path).await?;

    link_unlinked_libraries(&mut bytecode, &client).await?;
    let factory = ContractFactory::new(abi.clone(), bytecode.into_bytes().unwrap(), client.clone());

    let contract = factory.deploy(constructor_args)?.send().await?;
    Ok(contract)
}

/// Panic if no contract code found at the specified address.
pub async fn ensure_connected_to_contract(
    provider: &Provider<Http>,
    contract_address: Address,
) -> Result<()> {
    if !is_connected_to_contract(provider, contract_address).await? {
        panic!("No contract code found at {}", contract_address);
    }
    Ok(())
}

/// Check if the default provider can find a deployed contract at the specified address.
pub async fn is_connected_to_contract(
    provider: &Provider<Http>,
    contract_address: Address,
) -> Result<bool> {
    has_code_at_block(provider, contract_address, None).await
}

/// Check if the provider can find a deployed contract at the specified address.
/// This function is used for testing.
/// * `block` - The ethereum block number to check against, `None` implies the
/// latest block
pub(crate) async fn has_code_at_block(
    provider: &Provider<Http>,
    contract_address: Address,
    block: Option<BlockId>,
) -> Result<bool> {
    let code = provider.get_code(contract_address, block).await?;
    Ok(!code.to_vec().is_empty())
}
