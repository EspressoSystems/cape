// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

// A wallet that generates random transactions, for testing purposes.
// This test is for testing a bunch of wallets in the same process doing random transactions.
// It allows us to mock parts of the backend like the EQS, until it is ready for use.
//
// This test is still a work in progrogress.  See: https://github.com/EspressoSystems/cape/issues/649
// for everything left before it works properly.
#![deny(warnings)]

use async_std::sync::{Arc, Mutex};
use cap_rust_sandbox::deploy::deploy_erc20_token;
use cape_wallet::backend::CapeBackend;
use cape_wallet::mocks::*;
use cape_wallet::testing::get_burn_ammount;
use cape_wallet::testing::{
    burn_token, create_test_network, find_freezable_records, freeze_token, fund_eth_wallet,
    mint_token, retry_delay, sponsor_simple_token, transfer_token, unfreeze_token,
    wrap_simple_token, OperationType,
};
use cape_wallet::CapeWallet;
use cape_wallet::CapeWalletExt;
use ethers::prelude::Address;
use futures::stream::{iter, StreamExt};
use jf_cap::keys::UserAddress;
use jf_cap::keys::UserPubKey;
use jf_cap::proof::UniversalParam;
use jf_cap::structs::AssetCode;
use jf_cap::structs::FreezeFlag;
use jf_cap::{keys::UserKeyPair, testing_apis::universal_setup_for_test};
use rand::seq::SliceRandom;
use rand_chacha::{rand_core::SeedableRng, ChaChaRng};
use seahorse::txn_builder::RecordInfo;
use seahorse::{events::EventIndex, hd::KeyTree};
use std::collections::HashMap;
use std::path::Path;
// use std::path::PathBuf;
use structopt::StructOpt;
use surf::Url;
use tempdir::TempDir;
use tracing::{event, Level};
// use seahorse::WalletBackend;

#[derive(StructOpt)]
struct Args {
    /// Seed for random number generation.
    #[structopt(short, long)]
    seed: Option<u64>,

    /// Path to a saved wallet, or a new directory where this wallet will be saved.
    // storage: PathBuf,

    /// Spin up this many wallets to talk to each other
    num_wallets: u64,
}

struct NetworkInfo<'a> {
    sender_key: UserKeyPair,
    relayer_url: Url,
    contract_address: Address,
    mock_eqs: Arc<Mutex<MockCapeLedger<'a>>>,
}

#[allow(clippy::needless_lifetimes)]
async fn create_backend_and_sender_wallet<'a>(
    rng: &mut ChaChaRng,
    universal_param: &'a UniversalParam,
    storage: &Path,
) -> (NetworkInfo<'a>, CapeWallet<'a, CapeBackend<'a, ()>>) {
    let nework_tuple = create_test_network(rng, universal_param).await;
    let network = NetworkInfo {
        sender_key: nework_tuple.0,
        relayer_url: nework_tuple.1,
        contract_address: nework_tuple.2,
        mock_eqs: nework_tuple.3,
    };

    let mut loader = MockCapeWalletLoader {
        path: storage.to_path_buf(),
        key: KeyTree::random(rng).0,
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
        .add_user_key(
            network.sender_key.clone(),
            "sending account".into(),
            EventIndex::default(),
        )
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
    wallet
        .sync(network.mock_eqs.lock().await.now())
        .await
        .unwrap();

    // Wait for initial balance.
    while wallet
        .balance_breakdown(&address, &AssetCode::native())
        .await
        == 0
    {
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
        key: KeyTree::random(rng).0,
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

    (
        wallet
            .generate_user_key("sending account".into(), None)
            .await
            .unwrap(),
        wallet,
    )
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
    let mut tmp_dirs: Vec<TempDir> = vec![];
    let mut balances = HashMap::new();
    let args = Args::from_args();
    let mut rng = ChaChaRng::seed_from_u64(args.seed.unwrap_or(0));
    let universal_param = universal_setup_for_test(2usize.pow(16), &mut rng).unwrap();
    let tmp_dir = TempDir::new("random_in_mem_test_sender").unwrap();
    tmp_dirs.push(tmp_dir);
    let (network, mut wallet) = create_backend_and_sender_wallet(
        &mut rng,
        &universal_param,
        tmp_dirs.last().unwrap().path(),
    )
    .await;
    event!(Level::INFO, "Sender wallet has some initial balance");
    fund_eth_wallet(&mut wallet).await;
    event!(Level::INFO, "Funded Sender wallet with eth");

    // sponsor some token
    let erc20_contract = deploy_erc20_token().await;
    let sponsored_asset = sponsor_simple_token(&mut wallet, &erc20_contract)
        .await
        .unwrap();
    let address = wallet.pub_keys().await[0].address();
    balances.insert(address.clone(), HashMap::new());

    let mut wallets = vec![];
    let mut public_keys = vec![];

    for _i in 0..(args.num_wallets) {
        let tmp_dir = TempDir::new("random_in_mem_test").unwrap();
        tmp_dirs.push(tmp_dir);
        let (k, mut w) = create_wallet(
            &mut rng,
            &universal_param,
            &network,
            tmp_dirs.last().unwrap().path(),
        )
        .await;
        w.generate_freeze_key("freezing account".into())
            .await
            .unwrap();
        w.generate_audit_key("viewing account".into())
            .await
            .unwrap();
        event!(
            Level::INFO,
            "initialized new wallet\n  address: {}\n  pub key: {}",
            k.address(),
            k,
        );
        fund_eth_wallet(&mut w).await;
        event!(Level::INFO, "Funded new wallet with eth");
        // Fund the wallet with some native asset for paying fees
        transfer_token(&mut wallet, k.address(), 200, AssetCode::native(), 1)
            .await
            .unwrap();

        balances.insert(k.address().clone(), HashMap::new());
        balances
            .get_mut(&k.address())
            .unwrap()
            .insert(AssetCode::native(), 200);

        event!(Level::INFO, "Sent native token to new wallet");
        public_keys.push(k);
        wallets.push(w);
    }

    loop {
        let operation: OperationType = rand::random();

        match operation {
            OperationType::Mint => {
                event!(Level::INFO, "Minting");
                let minter = wallets.choose_mut(&mut rng).unwrap();
                let address = minter.pub_keys().await[0].address();
                let asset = mint_token(minter).await.unwrap();
                event!(Level::INFO, "minted custom asset.  Code: {}", asset.code);
                balances.get_mut(&address).unwrap().insert(
                    asset.code,
                    minter.balance_breakdown(&address, &asset.code).await,
                );
            }
            OperationType::Transfer => {
                event!(Level::INFO, "Transfering");
                let sender = wallets.choose_mut(&mut rng).unwrap();
                let sender_address = sender.pub_keys().await[0].address();

                let recipient_pk = public_keys.choose(&mut rng).unwrap();
                // Can't choose weighted and check this because async lambda not allowed.
                // There is probably a betterw way
                if sender.pub_keys().await[0] == *recipient_pk {
                    continue;
                }
                // Get a list of assets for which we have a non-zero balance.
                let mut asset_balances = vec![];
                for asset in sender.assets().await {
                    if sender
                        .balance_breakdown(&sender_address, &asset.definition.code)
                        .await
                        > 0
                    {
                        asset_balances.push(asset.definition.code);
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
                    } else {
                        asset.to_string()
                    },
                    recipient_pk,
                );
                match transfer_token(sender, recipient_pk.address(), amount, *asset, fee).await {
                    Ok(txn) => match sender.await_transaction(&txn).await {
                        Ok(status) => {
                            if !status.succeeded() {
                                // Transfers are allowed to fail. It can happen, for instance, if we
                                // get starved out until our transfer becomes too old for the
                                // validators. Thus we make this a warning, not an error.
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
                    },
                    Err(err) => {
                        event!(Level::ERROR, "error while building transaction: {}", err);
                    }
                }
            }
            OperationType::Freeze => {
                event!(Level::INFO, "Freezing");
                let freezer = wallets.choose_mut(&mut rng).unwrap();

                let freezable_records: Vec<RecordInfo> =
                    find_freezable_records(freezer, FreezeFlag::Unfrozen).await;
                if freezable_records.is_empty() {
                    event!(Level::INFO, "No freezable records");
                    continue;
                }
                let record = freezable_records.choose(&mut rng).unwrap();
                let owner_address = record.ro.pub_key.address().clone();
                let asset_def = &record.ro.asset_def;

                freeze_token(freezer, &asset_def.code, record.ro.amount, owner_address)
                    .await
                    .unwrap();
            }
            OperationType::Unfreeze => {
                event!(Level::INFO, "Unfreezing");
                let freezer = wallets.choose_mut(&mut rng).unwrap();

                let freezable_records: Vec<RecordInfo> =
                    find_freezable_records(freezer, FreezeFlag::Frozen).await;
                if freezable_records.is_empty() {
                    event!(Level::INFO, "No frozen records");
                    continue;
                }
                let record = freezable_records.choose(&mut rng).unwrap();
                let owner_address = record.ro.pub_key.address();
                let asset_def = &record.ro.asset_def;
                event!(
                    Level::INFO,
                    "Unfreezing Asset: {}, Amount: {}, Owner: {}",
                    asset_def.code,
                    record.ro.amount,
                    owner_address
                );
                unfreeze_token(freezer, &asset_def.code, record.ro.amount, owner_address)
                    .await
                    .unwrap();
            }
            OperationType::Wrap => {
                event!(Level::INFO, "Wrapping");
                let wrapper = wallets.choose_mut(&mut rng).unwrap();
                let wrapper_key = wrapper.pub_keys().await[0].clone();
                wrap_simple_token(
                    wrapper,
                    &wrapper_key.address(),
                    sponsored_asset.clone(),
                    &erc20_contract,
                    100,
                )
                .await
                .unwrap();
            }
            OperationType::Burn => {
                event!(Level::INFO, "Burning");
                let burner = wallets.choose_mut(&mut rng).unwrap();
                let asset = iter(burner.assets().await)
                    .filter(|asset| burner.is_wrapped_asset(asset.definition.code))
                    .next()
                    .await;
                if let Some(asset) = asset {
                    event!(Level::INFO, "Can burn something");
                    let amount = get_burn_ammount(burner, asset.definition.code).await;
                    event!(
                        Level::INFO,
                        "Buring {} asset: {}",
                        amount,
                        asset.definition.code
                    );
                    if amount > 0 {
                        burn_token(burner, asset.definition.clone(), amount)
                            .await
                            .unwrap();
                    }
                } else {
                    event!(Level::INFO, "no burnable assets, skipping burn operation");
                }
            }
        }
    }
}
