// A wallet that generates random transactions, for testing purposes.

use async_std::sync::{Arc, Mutex};
use async_std::task::sleep;
use cap_rust_sandbox::ledger::*;
use cape_wallet::backend::CapeBackend;
use cape_wallet::mocks::*;
use cape_wallet::testing::port;
use cape_wallet::CapeWallet;
use ethers::prelude::Address;
use jf_cap::proof::UniversalParam;
use jf_cap::structs::AssetPolicy;
use jf_cap::structs::{AssetCode, ReceiverMemo};
use key_set::VerifierKeySet;
use rand_chacha::{rand_core::SeedableRng, ChaChaRng};
use relayer::testing::start_minimal_relayer_for_test;
use seahorse::{events::EventIndex, hd::KeyTree};
use std::path::{Path, PathBuf};
use std::time::Duration;
use structopt::StructOpt;
use surf::Url;
use tracing::{event, Level};
// TODO remove copy paste from router.rs
// TODO: Add back freezer and auditor keys and test audit/freeze
use jf_cap::{
    keys::UserKeyPair, keys::UserPubKey, testing_apis::universal_setup_for_test,
    TransactionVerifyingKey,
};
use rand::distributions::weighted::WeightedError;
use rand::seq::SliceRandom;
use reef::traits::Ledger;
use seahorse::WalletBackend;
use std::fs::File;
use std::io::{Read, Write};

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

    // Path to all pub keys for sending assets to other wallets.  Stored in file until
    // Address Book is ready
    pub_key_storage: PathBuf,

    // If true create then give the wallet the faucet key to send some native assets.
    #[structopt(long)]
    sender: bool,
}

// TODO remove; Copied from backend.rs
async fn create_test_network<'a>(
    rng: &mut ChaChaRng,
    universal_param: &UniversalParam,
) -> (UserKeyPair, Url, Address, Arc<Mutex<MockCapeLedger<'a>>>) {
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

async fn retry_delay() {
    sleep(Duration::from_secs(1)).await
}

// Read then overwrite the whole file.  Plenty of race conditions possible
// but it's fine for the test if you wait between spinnin up processes.
async fn write_pub_key(key: &UserPubKey, path: &Path) {
    let mut keys: Vec<UserPubKey> = if path.exists() {
        get_pub_keys_from_file(path).await
    } else {
        vec![]
    };
    keys.push(key.clone());
    let mut file = File::create(path).unwrap_or_else(|err| {
        panic!("cannot open private key file: {}", err);
    });
    file.write_all(&bincode::serialize(&keys).unwrap()).unwrap();
}

async fn get_pub_keys_from_file(path: &Path) -> Vec<UserPubKey> {
    let mut file = File::open(path).unwrap_or_else(|err| {
        panic!("cannot open pub keys file: {}", err);
    });
    let mut bytes = Vec::new();
    let num_bytes = file.read_to_end(&mut bytes).unwrap_or_else(|err| {
        panic!("error reading pub keys file: {}", err);
    });
    if num_bytes == 0 {
        return vec![];
    }
    bincode::deserialize(&bytes).unwrap_or_else(|err| {
        panic!("invalid private key file: {}", err);
    })
}

#[async_std::main]
async fn main() {
    tracing_subscriber::fmt().pretty().init();
    println!("Starting Test Wallet");

    let args = Args::from_args();

    let mut rng = ChaChaRng::seed_from_u64(args.seed.unwrap_or(0));

    let universal_param = universal_setup_for_test(2usize.pow(16), &mut rng).unwrap();
    let mut loader = MockCapeWalletLoader {
        path: args.storage,
        key: KeyTree::random(&mut rng).unwrap().0,
    };

    // Everyone creates own relayer and EQS, not sure it works without EQS
    let (sender_key, relayer_url, contract_address, mock_eqs) =
        create_test_network(&mut rng, &universal_param).await;
    println!("Ledger Created");
    let backend = CapeBackend::new(
        &universal_param,
        relayer_url.clone(),
        contract_address,
        None,
        mock_eqs.clone(),
        &mut loader,
    )
    .await
    .unwrap();
    println!("Backend Created");

    let mut wallet = CapeWallet::new(backend).await.unwrap();
    let pub_key = if args.sender {
        println!("Sender");

        wallet
            .add_user_key(sender_key.clone(), EventIndex::default())
            .await
            .unwrap();
        wallet.await_key_scan(&sender_key.address()).await.unwrap();
        sender_key.pub_key()
    } else {
        println!("Not Sender");
        wallet.generate_user_key(None).await.unwrap()
    };
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

    println!("Wallet created");

    write_pub_key(&pub_key, &args.pub_key_storage).await;
    println!("Wrote pub key to file");

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

    // TODO actually get the peers from Address Book service.
    loop {
        let peers: Vec<UserPubKey> = get_pub_keys_from_file(&args.pub_key_storage).await;
        // Register the keys from file to memory because we don't have Address book Service yet
        for other_pk in peers.iter() {
            if *other_pk != pub_key {
                let state = &mut *wallet.lock().await;
                state
                    .backend_mut()
                    .register_user_key(other_pk)
                    .await
                    .unwrap();
            }
        }
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
