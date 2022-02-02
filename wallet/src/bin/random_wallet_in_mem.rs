// A wallet that generates random transactions, for testing purposes.
#![deny(warnings)]

use async_std::sync::{Arc, Mutex};
use async_std::task::sleep;
use cap_rust_sandbox::ledger::*;
use cape_wallet::backend::CapeBackend;
use cape_wallet::mocks::*;
use cape_wallet::testing::port;
use cape_wallet::CapeWallet;
use ethers::prelude::Address;
use jf_aap::proof::UniversalParam;
use jf_aap::structs::AssetPolicy;
use jf_aap::structs::{AssetCode, ReceiverMemo};
use key_set::VerifierKeySet;
use rand_chacha::{rand_core::SeedableRng, ChaChaRng};
use relayer::testing::start_minimal_relayer_for_test;
use seahorse::{events::EventIndex, hd::KeyTree};
use std::path::PathBuf;
use std::time::Duration;
use structopt::StructOpt;
use surf::Url;
use tracing::{event, Level};
// TODO remove copy paste from router.rs
// TODO: Add back freezer and auditor keys and test audit/freeze
use jf_aap::keys::UserPubKey;
use jf_aap::{keys::UserKeyPair, testing_apis::universal_setup_for_test, TransactionVerifyingKey};
use rand::seq::SliceRandom;
use reef::traits::Ledger;
use std::path::Path;
// use seahorse::WalletBackend;

#[derive(StructOpt)]
struct Args {
    /// Path to a private key file to use for the wallet.
    ///
    /// If not given, new keys are generated randomly.
    // #[structopt(short, long)]
    // key_path: Option<PathBuf>,

    /// Seed for random number generation.
    #[structopt(short, long)]
    seed: Option<u64>,

    /// Path to a saved wallet, or a new directory where this wallet will be saved.
    storage: PathBuf,

    /// Spin up this many wallets to talk to eachother
    num_wallets: u64,
    // TODO: How many transactions to do in Paralell
    // #[structopt(short, long)]
    // batch_size: Option<u64>,
}

struct NetworkInfo<'a> {
    sender_key: UserKeyPair,
    relayer_url: Url,
    contract_address: Address,
    mock_eqs: Arc<Mutex<MockCapeLedger<'a>>>,
}

// TODO remove; Copied from backend.rs
async fn create_test_network<'a>(
    rng: &mut ChaChaRng,
    universal_param: &UniversalParam,
) -> NetworkInfo<'a> {
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
                jf_aap::proof::transfer::preprocess(
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
                jf_aap::proof::transfer::preprocess(
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
            jf_aap::proof::freeze::preprocess(universal_param, 2, CapeLedger::merkle_height())
                .unwrap()
                .1,
        )]
        .into_iter()
        .collect(),
        mint: TransactionVerifyingKey::Mint(
            jf_aap::proof::mint::preprocess(universal_param, CapeLedger::merkle_height())
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

    NetworkInfo {
        sender_key,
        relayer_url,
        contract_address: contract.address(),
        mock_eqs,
    }
}

async fn retry_delay() {
    sleep(Duration::from_secs(1)).await
}

#[allow(clippy::needless_lifetimes)]
async fn create_backend_and_sender_wallet<'a>(
    rng: &mut ChaChaRng,
    universal_param: &'a UniversalParam,
    storage: &Path,
) -> (NetworkInfo<'a>, CapeWallet<'a, CapeBackend<'a, ()>>) {
    let mut loader = MockCapeWalletLoader {
        path: storage.to_path_buf(),
        key: KeyTree::random(rng).unwrap().0,
    };

    let network = create_test_network(rng, universal_param).await;

    let backend = CapeBackend::new(
        universal_param,
        network.relayer_url.clone(),
        network.contract_address,
        None,
        network.mock_eqs.clone(),
        &mut loader,
    )
    .await
    .unwrap();

    let mut wallet = CapeWallet::new(backend).await.unwrap();

    wallet
        .add_user_key(network.sender_key.clone(), EventIndex::default())
        .await
        .unwrap();

    wallet
        .await_key_scan(&network.sender_key.address())
        .await
        .unwrap();
    let pub_key = network.sender_key.pub_key();

    let address = pub_key.address();
    event!(
        Level::INFO,
        "initialized sender wallet\n  address: {}\n  pub key: {}",
        address,
        pub_key,
    );

    // Wait for initial balance.
    while wallet.balance(&address, &AssetCode::native()).await == 0 {
        event!(Level::INFO, "waiting for initial balance");
        retry_delay().await;
    }
    (network, wallet)
}

async fn create_wallet<'a>(
    rng: &mut ChaChaRng,
    universal_param: &'a UniversalParam,
    network: &NetworkInfo<'a>,
    storage: &Path,
) -> (UserPubKey, CapeWallet<'a, CapeBackend<'a, ()>>) {
    let mut loader = MockCapeWalletLoader {
        path: storage.to_path_buf(),
        key: KeyTree::random(rng).unwrap().0,
    };

    let backend = CapeBackend::new(
        universal_param,
        network.relayer_url.clone(),
        network.contract_address,
        None,
        network.mock_eqs.clone(),
        &mut loader,
    )
    .await
    .unwrap();

    let mut wallet = CapeWallet::new(backend).await.unwrap();

    (wallet.generate_user_key(None).await.unwrap(), wallet)
}

#[async_std::main]
async fn main() {
    tracing_subscriber::fmt().pretty().init();

    let args = Args::from_args();
    let mut rng = ChaChaRng::seed_from_u64(args.seed.unwrap_or(0));
    let universal_param = universal_setup_for_test(2usize.pow(16), &mut rng).unwrap();
    let (network, mut wallet) =
        create_backend_and_sender_wallet(&mut rng, &universal_param, &args.storage).await;

    let my_asset = wallet
        .define_asset(&[], AssetPolicy::default())
        .await
        .expect("failed to define asset");
    event!(Level::INFO, "defined a new asset type: {}", my_asset.code);
    let address = wallet.pub_keys().await[0].address();

    // Mint some custom asset
    if wallet.balance(&address, &my_asset.code).await == 0 {
        event!(Level::INFO, "minting my asset type {}", my_asset.code);
        loop {
            let txn = wallet
                .mint(&address, 1, &my_asset.code, 1u64 << 32, address.clone())
                .await
                .expect("failed to generate mint transaction");
            let status = wallet
                .await_transaction(&txn)
                .await
                .expect("error waiting for mint to complete");
            if status.succeeded() {
                break;
            }
            // The mint transaction is allowed to fail due to contention from other clients.
            event!(Level::WARN, "mint transaction failed, retrying...");
            retry_delay().await;
        }
        event!(Level::INFO, "minted custom asset");
    }

    let mut wallets = vec![];
    let mut public_keys = vec![];

    for _i in 0..(args.num_wallets) {
        // TODO send native asset from sender to all wallets.
        let (k, w) = create_wallet(&mut rng, &universal_param, &network, &args.storage).await;

        public_keys.push(k);
        wallets.push(w);
    }

    loop {
        let sender = wallets.choose_mut(&mut rng).unwrap();

        let recipient_pk = public_keys.choose(&mut rng).unwrap();
        // Can't choose weighted and check this because async lambda not allowed.
        // There is probably a betterw way
        if sender.pub_keys().await[0] == *recipient_pk {
            continue;
        }

        // Get a list of assets for which we have a non-zero balance.
        let mut asset_balances = vec![];
        for code in sender.assets().await.keys() {
            if sender.balance(&address, code).await > 0 {
                asset_balances.push(*code);
            }
        }
        // Randomly choose an asset type for the transfer.
        let asset = asset_balances.choose(&mut rng).unwrap();
        let amount = 1;
        let fee = 1;

        event!(
            Level::INFO,
            "transferring {} units of {} to user {}",
            amount,
            if *asset == AssetCode::native() {
                String::from("the native asset")
            } else if *asset == my_asset.code {
                String::from("my asset")
            } else {
                asset.to_string()
            },
            recipient_pk,
        );
        let txn = match sender
            .transfer(&address, asset, &[(recipient_pk.address(), amount)], fee)
            .await
        {
            Ok(txn) => txn,
            Err(err) => {
                event!(Level::ERROR, "Error generating transfer: {}", err);
                continue;
            }
        };
        match sender.await_transaction(&txn).await {
            Ok(status) => {
                if !status.succeeded() {
                    // Transfers are allowed to fail. It can happen, for instance, if we get starved
                    // out until our transfer becomes too old for the validators. Thus we make this
                    // a warning, not an error.
                    event!(Level::WARN, "transfer failed!");
                }
            }
            Err(err) => {
                event!(Level::ERROR, "error while waiting for transaction: {}", err);
            }
        }
    }
}
