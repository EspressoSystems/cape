// A wallet that generates random transactions, for testing purposes.
#![deny(warnings)]

use async_std::sync::{Arc, Mutex};
use async_std::task::sleep;
use cape_wallet::backend::create_test_network;
use cape_wallet::backend::CapeBackend;
use cape_wallet::mocks::*;
use cape_wallet::CapeWallet;
use ethers::prelude::Address;
use jf_cap::keys::UserAddress;
use jf_cap::keys::UserPubKey;
use jf_cap::proof::UniversalParam;
use jf_cap::structs::AssetCode;
use jf_cap::structs::AssetPolicy;
use jf_cap::{keys::UserKeyPair, testing_apis::universal_setup_for_test};
use rand::seq::SliceRandom;
use rand_chacha::{rand_core::SeedableRng, ChaChaRng};
use seahorse::{events::EventIndex, hd::KeyTree};
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use structopt::StructOpt;
use surf::Url;
use tracing::{event, Level};
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
    // TODO: How many transactions to do in Parallel
    // #[structopt(short, long)]
    // batch_size: Option<u64>,
}

struct NetworkInfo<'a> {
    sender_key: UserKeyPair,
    relayer_url: Url,
    contract_address: Address,
    mock_eqs: Arc<Mutex<MockCapeLedger<'a>>>,
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

    let nework_tuple = create_test_network(rng, universal_param).await;
    let network = NetworkInfo {
        sender_key: nework_tuple.0,
        relayer_url: nework_tuple.1,
        contract_address: nework_tuple.2,
        mock_eqs: nework_tuple.3,
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

fn update_balances(
    send_addr: &UserAddress,
    receiver_addr: &UserAddress,
    amount: u64,
    asset: &AssetCode,
    balances: &mut HashMap<UserAddress, HashMap<AssetCode, u64>>,
) {
    assert!(
        balances.contains_key(send_addr),
        "Test never recorded the sender having any assets"
    );

    if !balances.contains_key(receiver_addr) {
        balances.insert(receiver_addr.clone(), HashMap::new());
    }

    let sender_assets = balances.get_mut(send_addr).unwrap();
    // Udate with asset code
    let send_balance = *sender_assets.get(asset).unwrap_or(&0);
    assert!(
        send_balance > amount,
        "Address {} only has {} balance but is trying to send {}.",
        send_addr,
        send_balance,
        amount
    );
    sender_assets.insert(*asset, send_balance - amount);

    let rec_assets = balances.get_mut(receiver_addr).unwrap();
    let receive_balance = *rec_assets.get(asset).unwrap_or(&0);
    rec_assets.insert(*asset, receive_balance + amount);
}

#[async_std::main]
async fn main() {
    tracing_subscriber::fmt().pretty().init();

    let mut balances = HashMap::new();

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
    balances.insert(address.clone(), HashMap::new());
    balances.get_mut(&address).unwrap().insert(
        my_asset.code,
        wallet.balance(&address, &my_asset.code).await,
    );

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
        let sender_address = wallet.pub_keys().await[0].address();

        let recipient_pk = public_keys.choose(&mut rng).unwrap();
        // Can't choose weighted and check this because async lambda not allowed.
        // There is probably a betterw way
        if sender.pub_keys().await[0] == *recipient_pk {
            continue;
        }

        // Get a list of assets for which we have a non-zero balance.
        let mut asset_balances = vec![];
        for code in sender.assets().await.keys() {
            if sender.balance(&sender_address, code).await > 0 {
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
            .transfer(
                &sender_address,
                asset,
                &[(recipient_pk.address(), amount)],
                fee,
            )
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
                update_balances(
                    &sender_address,
                    &recipient_pk.address(),
                    amount,
                    asset,
                    &mut balances,
                )
            }
            Err(err) => {
                event!(Level::ERROR, "error while waiting for transaction: {}", err);
            }
        }
    }
}
