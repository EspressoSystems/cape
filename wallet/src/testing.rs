// Utilities for writing wallet tests
// #![deny(warnings)]

use crate::backend::CapeBackend;
use crate::mocks::*;
use crate::wallet::CapeWalletExt;
use crate::CapeWallet;
use crate::CapeWalletError;
use address_book::init_web_server;
use address_book::wait_for_server;
use async_std::sync::{Arc, Mutex};
use cap_rust_sandbox::deploy::EthMiddleware;
use cap_rust_sandbox::ledger::CapeLedger;
use cap_rust_sandbox::types::SimpleToken;
use ethers::prelude::Address;
use jf_cap::keys::UserAddress;
use jf_cap::keys::UserKeyPair;
use jf_cap::proof::UniversalParam;
use jf_cap::structs::AssetCode;
use jf_cap::structs::AssetDefinition;
use jf_cap::structs::AssetPolicy;
use jf_cap::structs::ReceiverMemo;
use jf_cap::TransactionVerifyingKey;
use key_set::VerifierKeySet;
use lazy_static::lazy_static;
use rand::{
    distributions::{Distribution, Standard},
    Rng,
};
use rand_chacha::ChaChaRng;
use reef::Ledger;
use relayer::testing::start_minimal_relayer_for_test;
use seahorse::testing::await_transaction;
use seahorse::txn_builder::TransactionStatus;
use surf::Url;
use tide::log::LevelFilter;

lazy_static! {
    static ref PORT: Arc<Mutex<u64>> = {
        let port_offset = std::env::var("PORT").unwrap_or_else(|_| String::from("60000"));
        Arc::new(Mutex::new(port_offset.parse().unwrap()))
    };
}

pub async fn port() -> u64 {
    let mut counter = PORT.lock().await;
    let port = *counter;
    *counter += 1;
    port
}

#[allow(clippy::needless_lifetimes)]
pub async fn create_test_network<'a>(
    rng: &mut ChaChaRng,
    universal_param: &'a UniversalParam,
) -> (UserKeyPair, Url, Address, Arc<Mutex<MockCapeLedger<'a>>>) {
    init_web_server(LevelFilter::Error)
        .await
        .expect("Failed to run server.");
    wait_for_server().await;

    // Set up a network that includes a minimal relayer, connected to a real Ethereum
    // blockchain, as well as a mock EQS which will track the blockchain in parallel, since we
    // don't yet have a real EQS.
    let relayer_port = port().await;
    let (contract, sender_key, sender_rec, records) =
        start_minimal_relayer_for_test(relayer_port).await;
    let relayer_url = Url::parse(&format!("http://localhost:{}", relayer_port)).unwrap();
    let sender_memo = ReceiverMemo::from_ro(rng, &sender_rec, &[]).unwrap();

    let verif_crs = VerifierKeySet {
        xfr: vec![
            // For regular transfers, including non-native transfers
            TransactionVerifyingKey::Transfer(
                jf_cap::proof::transfer::preprocess(
                    universal_param,
                    2,
                    3,
                    CapeLedger::merkle_height(),
                )
                .unwrap()
                .1,
            ),
            // For burns (which currently require exactly 2 inputs and outputs, but this is an
            // artificial restriction which should be lifted)
            TransactionVerifyingKey::Transfer(
                jf_cap::proof::transfer::preprocess(
                    universal_param,
                    2,
                    2,
                    CapeLedger::merkle_height(),
                )
                .unwrap()
                .1,
            ),
        ]
        .into_iter()
        .collect(),
        freeze: vec![TransactionVerifyingKey::Freeze(
            jf_cap::proof::freeze::preprocess(universal_param, 2, CapeLedger::merkle_height())
                .unwrap()
                .1,
        )]
        .into_iter()
        .collect(),
        mint: TransactionVerifyingKey::Mint(
            jf_cap::proof::mint::preprocess(universal_param, CapeLedger::merkle_height())
                .unwrap()
                .1,
        ),
    };
    let mut mock_eqs = MockCapeLedger::new(MockCapeNetwork::new(
        verif_crs,
        records,
        vec![(sender_memo, 0)],
    ));
    mock_eqs.set_block_size(1).unwrap();
    // The minimal test relayer does not block transactions, so the mock EQS shouldn't
    // either.
    let mock_eqs = Arc::new(Mutex::new(mock_eqs));

    (sender_key, relayer_url, contract.address(), mock_eqs)
}

#[derive(Debug)]
pub enum OperationType {
    Transfer,
    Freeze,
    Unfreeze,
    Wrap,
    Burn,
}

impl Distribution<OperationType> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> OperationType {
        match rng.gen_range(0..=4) {
            0 => OperationType::Transfer,
            1 => OperationType::Freeze,
            2 => OperationType::Unfreeze,
            3 => OperationType::Wrap,
            _ => OperationType::Burn,
        }
    }
}

pub async fn freeze_token<'a>(
    freezer: &mut CapeWallet<'a, CapeBackend<'a, ()>>,
    asset: &AssetCode,
    amount: u64,
    owner_address: UserAddress,
) -> Result<TransactionStatus, CapeWalletError> {
    let freeze_address = freezer.pub_keys().await[0].address();
    let txn = freezer
        .freeze(&freeze_address, 1, asset, amount, owner_address)
        .await
        .unwrap();
    freezer.await_transaction(&txn).await
}

pub async fn unfreeze_token<'a>(
    freezer: &mut CapeWallet<'a, CapeBackend<'a, ()>>,
    asset: &AssetCode,
    amount: u64,
    owner_address: UserAddress,
) -> Result<TransactionStatus, CapeWalletError> {
    let unfreeze_address = freezer.pub_keys().await[0].address();
    let txn = freezer
        .unfreeze(&unfreeze_address, 1, asset, amount, owner_address)
        .await
        .unwrap();
    freezer.await_transaction(&txn).await
}

pub async fn wrap_simple_token<'a>(
    wrapper: &mut CapeWallet<'a, CapeBackend<'a, ()>>,
    wrapper_addr: &UserAddress,
    cape_asset: AssetDefinition,
    erc20_contract: &SimpleToken<EthMiddleware>,
    contract_address: Address,
    amount: u64,
) -> Result<(), CapeWalletError> {
    let wrapper_eth_addr = wrapper.eth_address().await.unwrap();

    let total_native_balance = wrapper.balance(wrapper_addr, &AssetCode::native()).await;
    assert!(total_native_balance > 0);
    // Prepare to wrap: approve the transfer from the wrapper's ETH wallet to the CAPE contract.
    SimpleToken::new(
        erc20_contract.address(),
        wrapper.eth_client().await.unwrap(),
    )
    .approve(contract_address, amount.into())
    .send()
    .await
    .unwrap()
    .await
    .unwrap();

    // Prepare to wrap: deposit some ERC20 tokens into the wrapper's ETH wallet.
    erc20_contract
        .transfer(wrapper_eth_addr.clone().into(), amount.into())
        .send()
        .await
        .unwrap()
        .await
        .unwrap();
    assert_eq!(
        erc20_contract
            .balance_of(wrapper_eth_addr.clone().into())
            .call()
            .await
            .unwrap(),
        amount.into()
    );

    // Deposit some ERC20 into the CAPE contract.
    wrapper
        .wrap(
            wrapper_eth_addr.clone(),
            cape_asset.clone(),
            wrapper_addr.clone(),
            amount,
        )
        .await
        .unwrap();
    assert_eq!(
        erc20_contract
            .balance_of(wrapper_eth_addr.clone().into())
            .call()
            .await
            .unwrap(),
        0.into()
    );
    Ok(())
}

pub async fn sponsor_simple_token<'a>(
    sponsor: &mut CapeWallet<'a, CapeBackend<'a, ()>>,
    erc20_contract: &SimpleToken<EthMiddleware>,
) -> Result<AssetDefinition, CapeWalletError> {
    let sponsor_eth_addr = sponsor.eth_address().await.unwrap();
    sponsor
        .sponsor(
            erc20_contract.address().into(),
            sponsor_eth_addr.clone(),
            AssetPolicy::default(),
        )
        .await
}

pub async fn burn_token<'a>(
    burner: &mut CapeWallet<'a, CapeBackend<'a, ()>>,
    cape_asset: AssetDefinition,
    amount: u64,
) -> Result<(), CapeWalletError> {
    let burner_key = burner.pub_keys().await[0].clone();
    let receipt = burner
        .burn(
            &burner_key.address(),
            burner.eth_address().await.unwrap().clone(),
            &cape_asset.code,
            amount,
            1,
        )
        .await
        .unwrap();
    await_transaction(&receipt, burner, &[]).await;
    Ok(())
}

pub async fn transfer_token<'a>(
    sender: &mut CapeWallet<'a, CapeBackend<'a, ()>>,
    receiver_address: UserAddress,
    amount: u64,
    asset_code: AssetCode,
    fee: u64,
) -> Result<TransactionStatus, CapeWalletError> {
    let sender_address = sender.pub_keys().await[0].address();
    let txn = sender
        .transfer(
            &sender_address,
            &asset_code,
            &[(receiver_address, amount)],
            fee,
        )
        .await
        .unwrap();
    sender.await_transaction(&txn).await
}
