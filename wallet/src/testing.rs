// Utilities for writing wallet tests
// #![deny(warnings)]

use crate::backend::CapeBackend;
use crate::mocks::*;
use crate::CapeWallet;
use crate::CapeWalletError;
use address_book::init_web_server;
use address_book::wait_for_server;
use async_std::sync::{Arc, Mutex};
use cap_rust_sandbox::ledger::CapeLedger;
use ethers::prelude::Address;
use jf_cap::keys::UserAddress;
use jf_cap::keys::UserKeyPair;
use jf_cap::proof::UniversalParam;
use jf_cap::structs::AssetCode;
use jf_cap::structs::ReceiverMemo;
use jf_cap::TransactionVerifyingKey;
use key_set::VerifierKeySet;
use lazy_static::lazy_static;
use rand_chacha::ChaChaRng;
use reef::Ledger;
use relayer::testing::start_minimal_relayer_for_test;
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
