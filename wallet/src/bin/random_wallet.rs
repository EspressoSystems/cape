// A wallet that generates random transactions, for testing purposes.
//
// Spin up a random wallet and point it at a query service like so:
//  random_wallet storage/random_wallet_N localhost:50000
//
// The wallet will discover its peers (all of the other wallets connected to the same query service)
// and begin making random transactions as follows:
//  * define a new custom asset type and mint 2^32 tokens for ourselves
//  * repeatedly:
//      - randomly select an asset type for which we have a nonzero balance
//      - transfer a fraction of that asset type to a randomly selected peer
//
// There can be multiple groups of wallets connected to different query servers. Wallets will only
// interact with other wallets in the same group.
//
// Note that the ledger must be initialized with a balance of native assets for each random wallet
// by passing the public key of each wallet that should receive funds to each validator with
// `--wallet`. This requires having the public key before starting `random_wallet`. You can generate
// a key pair using `zerok_client -g KEY_FILE`, and then pass the public key to the validators with
// `-w KEY_FILE.pub` and pass the key pair to `random_wallet` with `-k KEY_FILE`.

use async_std::sync::{Arc, Mutex};
use async_std::task::sleep;
use cape_wallet::mocks::*;
// use cape_wallet::wallet::{CapeWalletBackend, CapeWalletError};
// use jf_aap::keys::UserPubKey;
use cap_rust_sandbox::{ledger::*, universal_param::UNIVERSAL_PARAM};
use jf_aap::structs::{AssetCode, AssetPolicy};
use key_set::{KeySet, VerifierKeySet};
use rand::distributions::weighted::WeightedError;
use rand::seq::SliceRandom;
use rand_chacha::{rand_core::SeedableRng, ChaChaRng};
use seahorse::{events::EventIndex, hd::KeyTree, testing::MockLedger};
// use snafu::ResultExt;
// use std::convert::TryInto;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::time::Duration;
use structopt::StructOpt;
use tracing::{event, Level};
// TODO remove copy pasta from router.rs
// TODO: Add back freezer and auditor keys and test audit/freeze
use jf_aap::{keys::UserPubKey, MerkleTree, TransactionVerifyingKey};
use reef::traits::Ledger;

// use cape_wallet::wallet::*;

// use zerok_lib::{
//     api::client,
//     api::SpectrumError,
//     ledger::SpectrumLedger,
//     universal_params::UNIVERSAL_PARAM,
//     wallet::network::{NetworkBackend, Url},
// };

type Wallet = seahorse::Wallet<'static, MockCapeBackend<'static, ()>, CapeLedger>;

#[derive(StructOpt)]
struct Args {
    /// Path to a private key file to use for the wallet.
    ///
    /// If not given, new keys are generated randomly.
    #[structopt(short, long)]
    key_path: Option<PathBuf>,

    /// Seed for random number generation.
    #[structopt(short, long)]
    seed: Option<u64>,

    /// Path to a saved wallet, or a new directory where this wallet will be saved.
    storage: PathBuf,
}

async fn retry_delay() {
    sleep(Duration::from_secs(1)).await
}

#[async_std::main]
async fn main() {
    tracing_subscriber::fmt().pretty().init();

    let args = Args::from_args();

    let mut rng = ChaChaRng::seed_from_u64(args.seed.unwrap_or(0));

    let mut loader = MockCapeWalletLoader {
        path: args.storage,
        key: KeyTree::random(&mut rng).unwrap().0,
    };

    let verif_crs = VerifierKeySet {
        mint: TransactionVerifyingKey::Mint(
            jf_aap::proof::mint::preprocess(&*UNIVERSAL_PARAM, CapeLedger::merkle_height())
                .unwrap()
                .1,
        ),
        xfr: KeySet::new(
            vec![TransactionVerifyingKey::Transfer(
                jf_aap::proof::transfer::preprocess(
                    &*UNIVERSAL_PARAM,
                    3,
                    3,
                    CapeLedger::merkle_height(),
                )
                .unwrap()
                .1,
            )]
            .into_iter(),
        )
        .unwrap(),
        freeze: KeySet::new(
            vec![TransactionVerifyingKey::Freeze(
                jf_aap::proof::freeze::preprocess(
                    &*UNIVERSAL_PARAM,
                    2,
                    CapeLedger::merkle_height(),
                )
                .unwrap()
                .1,
            )]
            .into_iter(),
        )
        .unwrap(),
    };

    let ledger = Arc::new(Mutex::new(MockLedger::new(MockCapeNetwork::new(
        verif_crs,
        MerkleTree::new(CapeLedger::merkle_height()).unwrap(),
        vec![],
    ))));

    let backend =
        MockCapeBackend::new(ledger.clone(), &mut loader).expect("failed to connect to backend");
    let mut wallet = Wallet::new(backend).await.expect("error loading wallet");
    match args.key_path {
        Some(path) => {
            let mut file = File::open(path).unwrap_or_else(|err| {
                panic!("cannot open private key file: {}", err);
            });
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes).unwrap_or_else(|err| {
                panic!("error reading private key file: {}", err);
            });
            wallet
                .add_user_key(
                    bincode::deserialize(&bytes).unwrap_or_else(|err| {
                        panic!("invalid private key file: {}", err);
                    }),
                    EventIndex::default(),
                )
                .await
                .unwrap_or_else(|err| {
                    panic!("error loading key: {}", err);
                });
        }
        None => {
            wallet.generate_user_key(None).await.unwrap_or_else(|err| {
                panic!("error generating random key: {}", err);
            });
        }
    }
    let pub_key = wallet.pub_keys().await.remove(0);
    let address = pub_key.address();
    event!(
        Level::INFO,
        "initialized wallet\n  address: {}\n  pub key: {}",
        address,
        pub_key,
    );

    // Wait for initial balance.
    while wallet.balance(&address, &AssetCode::native()).await == 0 {
        event!(Level::INFO, "waiting for initial balance");
        retry_delay().await;
    }

    // Check if we already have a mintable asset (if we are loading from a saved wallet).
    let my_asset = match wallet
        .assets()
        .await
        .into_values()
        .find(|info| info.mint_info.is_some())
    {
        Some(info) => {
            event!(
                Level::INFO,
                "found saved wallet with custom asset type {}",
                info.asset.code
            );
            info.asset
        }
        None => {
            let my_asset = wallet
                .define_asset(&[], AssetPolicy::default())
                .await
                .expect("failed to define asset");
            event!(Level::INFO, "defined a new asset type: {}", my_asset.code);
            my_asset
        }
    };
    // If we don't yet have a balance of our asset type, mint some.
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

    // TODO actually get the peers.

    // let client: surf::Client = surf::Config::new()
    //     .set_base_url(args.server)
    //     .try_into()
    //     .expect("failed to start HTTP client");
    // TODO: Fix the error parseing/uncomment this line
    // let client = client.with(client::parse_error_body::<>);
    loop {
        let peers: Vec<UserPubKey> = vec![];
        // Get a list of all users in our group (this will include our own public key).
        // let peers: Vec<UserPubKey> = match client.get("getusers").recv_json().await {
        //     Ok(peers) => peers,
        //     Err(err) => {
        //         event!(
        //             Level::ERROR,
        //             "error getting users from server: {}\nretrying...",
        //             err
        //         );
        //         retry_delay().await;
        //         continue;
        //     }
        // };
        // Filter out our own public key and randomly choose one of the other ones to transfer to.
        let recipient =
            match peers.choose_weighted(&mut rng, |pk| if *pk == pub_key { 0 } else { 1 }) {
                Ok(recipient) => recipient,
                Err(WeightedError::NoItem | WeightedError::AllWeightsZero) => {
                    event!(Level::WARN, "no peers yet, retrying...");
                    retry_delay().await;
                    continue;
                }
                Err(err) => {
                    panic!("error in weighted choice of peer: {}", err);
                }
            };

        // Get a list of assets for which we have a non-zero balance.
        let mut asset_balances = vec![];
        for code in wallet.assets().await.keys() {
            if wallet.balance(&address, code).await > 0 {
                asset_balances.push(*code);
            }
        }
        // Randomly choose an asset type for the transfer.
        let asset = asset_balances.choose(&mut rng).unwrap();

        // All transfers are the same, small size. This should prevent fragmentation errors and
        // allow us to make as many transactions as possible with the assets we have.
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
            recipient,
        );
        let txn = match wallet
            .transfer(&address, asset, &[(recipient.address(), amount)], fee)
            .await
        {
            Ok(txn) => txn,
            Err(err) => {
                event!(Level::ERROR, "Error generating transfer: {}", err);
                continue;
            }
        };
        match wallet.await_transaction(&txn).await {
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
