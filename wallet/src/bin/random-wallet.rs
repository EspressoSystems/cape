// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

// A wallet that generates random transactions, for testing purposes.
// This test is still a work in progress and will not work until we have
// integration with the EQS.  See Issue: https://github.com/EspressoSystems/cape/issues/548
#![deny(warnings)]

use async_std::task::sleep;
use cap_rust_sandbox::deploy::deploy_erc20_token;
use cape_wallet::backend::{CapeBackend, CapeBackendConfig};
use cape_wallet::mocks::*;
use cape_wallet::testing::create_test_network;
use cape_wallet::testing::get_burn_amount;
use cape_wallet::testing::OperationType;
use cape_wallet::testing::{
    burn_token, find_freezable_records, freeze_token, fund_eth_wallet, rpc_url_for_test, spawn_eqs,
    sponsor_simple_token, unfreeze_token, wrap_simple_token,
};
use cape_wallet::CapeWallet;
use jf_cap::keys::UserKeyPair;
use jf_cap::structs::AssetCode;
use jf_cap::structs::AssetPolicy;
use jf_cap::structs::FreezeFlag;
use jf_cap::{keys::UserPubKey, testing_apis::universal_setup_for_test};
use rand::distributions::weighted::WeightedError;
use rand::seq::SliceRandom;
use rand_chacha::{rand_core::SeedableRng, ChaChaRng};
use seahorse::txn_builder::RecordInfo;
use seahorse::{events::EventIndex, hd::KeyTree};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;
use structopt::StructOpt;
use tracing::{event, Level};

#[derive(StructOpt)]
struct Args {
    /// Path to a private key file to use for the wallet.
    ///
    /// If not given, new keys are generated randomly.
    /// Ignored if the sender flag is set
    #[structopt(short, long)]
    key_path: Option<PathBuf>,

    /// Seed for random number generation.
    #[structopt(short, long)]
    seed: Option<u64>,

    /// Path to a saved wallet, or a new directory where this wallet will be saved.
    storage: PathBuf,

    /// Path to all pub keys for sending assets to other wallets.  Stored in file until
    /// Address Book is ready
    pub_key_storage: PathBuf,

    /// If true then give the wallet the faucet key to send some native assets.
    #[structopt(long)]
    sender: bool,
}

async fn retry_delay() {
    sleep(Duration::from_secs(1)).await
}

// Use the Address Book here instead: https://github.com/EspressoSystems/cape/issues/641
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
        key: KeyTree::random(&mut rng).0,
    };

    // Everyone creates own relayer and EQS, not sure it works without EQS
    let (sender_key, relayer_url, address_book_url, contract_address, _) =
        create_test_network(&mut rng, &universal_param).await;
    // Spawn our own EQS since we have our own relayer and contract connection.
    // TODO connect to an existing EQS.
    let (eqs_url, _eqs_dir, _join_eqs) = spawn_eqs(contract_address).await;
    println!("Ledger Created");
    let backend = CapeBackend::new(
        &universal_param,
        CapeBackendConfig {
            rpc_url: rpc_url_for_test(),
            eqs_url,
            relayer_url: relayer_url.clone(),
            address_book_url,
            contract_address,
            eth_mnemonic: None,
            min_polling_delay: Duration::from_millis(500),
        },
        &mut loader,
    )
    .await
    .unwrap();
    println!("Backend Created");

    let mut wallet = CapeWallet::new(backend).await.unwrap();
    let pub_key = if args.sender {
        wallet
            .add_user_key(
                sender_key.clone(),
                "sending account".into(),
                EventIndex::default(),
            )
            .await
            .unwrap();
        wallet.await_key_scan(&sender_key.address()).await.unwrap();
        sender_key.pub_key()
    } else {
        match args.key_path {
            Some(path) => {
                let mut file = File::open(path).unwrap_or_else(|err| {
                    panic!("cannot open private key file: {}", err);
                });
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes).unwrap_or_else(|err| {
                    panic!("error reading private key file: {}", err);
                });
                let key: UserKeyPair = bincode::deserialize(&bytes).unwrap_or_else(|err| {
                    panic!("invalid private key file: {}", err);
                });
                wallet
                    .add_user_key(key.clone(), "sending account".into(), EventIndex::default())
                    .await
                    .unwrap_or_else(|err| {
                        panic!("error loading key: {}", err);
                    });
                key.pub_key()
            }
            None => wallet
                .generate_user_key("sending account".into(), None)
                .await
                .unwrap_or_else(|err| {
                    panic!("error generating random key: {}", err);
                }),
        }
    };

    fund_eth_wallet(&mut wallet).await;

    let freeze_key = wallet
        .generate_freeze_key("freezing account".into())
        .await
        .unwrap();
    let audit_key = wallet
        .generate_audit_key("viewing account".into())
        .await
        .unwrap();

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
    while wallet
        .balance_breakdown(&address, &AssetCode::native())
        .await
        == 0
    {
        event!(Level::INFO, "waiting for initial balance");
        retry_delay().await;
    }

    // Check if we already have a mintable asset (if we are loading from a saved wallet).
    let my_asset = match wallet
        .assets()
        .await
        .into_iter()
        .find(|asset| asset.mint_info.is_some())
    {
        Some(asset) => {
            event!(
                Level::INFO,
                "found saved wallet with custom asset type {}",
                asset.definition.code
            );
            asset.definition
        }
        None => {
            let policy = AssetPolicy::default()
                .set_freezer_pub_key(freeze_key.clone())
                .set_auditor_pub_key(audit_key)
                .reveal_user_address()
                .unwrap()
                .reveal_amount()
                .unwrap()
                .reveal_blinding_factor()
                .unwrap();
            let my_asset = wallet
                .define_asset("my_asset".into(), &[], policy)
                .await
                .expect("failed to define asset");
            event!(Level::INFO, "defined a new asset type: {}", my_asset.code);
            my_asset
        }
    };
    // If we don't yet have a balance of our asset type, mint some.
    if wallet.balance_breakdown(&address, &my_asset.code).await == 0 {
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

    // Deploy contract + simple token
    let erc20_contract = deploy_erc20_token().await;
    sponsor_simple_token(&mut wallet, &erc20_contract)
        .await
        .unwrap();

    loop {
        // Use the Address book for this: https://github.com/EspressoSystems/cape/issues/641
        let peers: Vec<UserPubKey> = get_pub_keys_from_file(&args.pub_key_storage).await;
        let operation: OperationType = rand::random();
        match operation {
            OperationType::Mint => {
                // Skip mint, each wallet mints their own token when created
            }
            OperationType::Transfer => {
                let recipient = match peers.choose_weighted(&mut rng, |pk| {
                    if *pk == pub_key {
                        0
                    } else {
                        1
                    }
                }) {
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
                for asset in wallet.assets().await {
                    if wallet
                        .balance_breakdown(&address, &asset.definition.code)
                        .await
                        > 0
                    {
                        asset_balances.push(asset.definition.code);
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
                    .transfer(Some(&address), asset, &[(recipient.address(), amount)], fee)
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
            OperationType::Freeze => {
                let freezable_records: Vec<RecordInfo> =
                    find_freezable_records(&wallet, FreezeFlag::Unfrozen).await;
                if freezable_records.is_empty() {
                    continue;
                }
                let record = freezable_records.choose(&mut rng).unwrap();
                let owner_address = record.ro.pub_key.address().clone();
                let asset_def = &record.ro.asset_def;

                freeze_token(
                    &mut wallet,
                    &asset_def.code,
                    record.ro.amount,
                    owner_address,
                )
                .await
                .unwrap();
            }
            OperationType::Unfreeze => {
                let freezable_records: Vec<RecordInfo> =
                    find_freezable_records(&wallet, FreezeFlag::Frozen).await;
                if freezable_records.is_empty() {
                    continue;
                }
                let record = freezable_records.choose(&mut rng).unwrap();
                let owner_address = record.ro.pub_key.address();
                let asset_def = &record.ro.asset_def;

                unfreeze_token(
                    &mut wallet,
                    &asset_def.code,
                    record.ro.amount,
                    owner_address,
                )
                .await
                .unwrap();
            }
            OperationType::Wrap => {
                let asset_def = wallet
                    .define_asset("my_asset".into(), &[], AssetPolicy::default())
                    .await
                    .expect("failed to define asset");
                wrap_simple_token(&mut wallet, &address, asset_def, &erc20_contract, 100)
                    .await
                    .unwrap();
            }
            OperationType::Burn => {
                let assets = wallet.assets().await;
                let asset = assets.choose(&mut rng).unwrap();
                let amount = get_burn_amount(&wallet, asset.definition.code).await;
                burn_token(&mut wallet, asset.definition.clone(), amount)
                    .await
                    .unwrap();
            }
        }
    }
}
