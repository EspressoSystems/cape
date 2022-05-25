// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

//! # The CAPE Wallet Server
//!
//! One of two main entrypoints to the wallet (the other being the CLI). This executable provides a
//! web server which exposes wallet functionality via an HTTP interface. It is primarily intended
//! to be run in a Docker container and used as the backend for the CAPE wallet GUI.
//!
//! ## Usage
//!
//! ### Running in Docker
//! ```
//! docker run -it -p 60000:60000  ghcr.io/espressosystems/cape/wallet:main
//! ```
//!
//! The `-p 60000:60000` option binds the port 60000 in the Docker container (where the web server
//! is hosted) to the port 60000 on the host. You can change which port on `localhost` hosts the
//! server by changing the first number, e.g. `-p 42000:60000`.
//!
//! ### Building and running locally
//! ```
//! cargo run --release --bin wallet-api -- [options]
//! ```
//!
//! You can use `--help` to see a list of the possible values for `[options]`.
//!
//! Once started, the web server will serve an HTTP API at `localhost:60000`.
//! You can override the default port by setting the `CAPE_WALLET_PORT`
//! environment variable. The endpoints are documented in `api/api.toml`.
//!
//! ## Development
//!
//! This executable file only defines the main function to process command line arguments and start
//! the web server. Most of the functionality, such as API interpretation, request parsing, and
//! route handling, is defined in the [cape_wallet] crate.

mod routes;
mod web;

use crate::web::{init_server, NodeOpt};
use rand_chacha::{rand_core::SeedableRng, ChaChaRng};
use structopt::StructOpt;

#[async_std::main]
async fn main() -> Result<(), std::io::Error> {
    tracing_subscriber::fmt()
        .pretty()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    init_server(ChaChaRng::from_entropy(), &NodeOpt::from_args())?.await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        routes::CapeAPIError,
        web::{
            DEFAULT_ETH_ADDR, DEFAULT_NATIVE_AMT_IN_FAUCET_ADDR,
            DEFAULT_NATIVE_AMT_IN_WRAPPER_ADDR, DEFAULT_WRAPPED_AMT,
        },
    };
    use ark_serialize::CanonicalDeserialize;
    use async_std::fs;
    use cap_rust_sandbox::{ledger::CapeLedger, model::EthereumAddr};
    use cape_wallet::{
        mocks::test_asset_signing_key,
        testing::{port, retry},
        ui::*,
    };
    use ethers::prelude::{Address, U256};
    use jf_cap::{
        keys::{AuditorKeyPair, AuditorPubKey, FreezerKeyPair, FreezerPubKey, UserKeyPair},
        structs::{AssetCode, AssetDefinition as JfAssetDefinition, AssetPolicy},
    };
    use net::{client, UserAddress};
    use seahorse::{
        asset_library::{Icon, VerifiedAssetLibrary},
        hd::{KeyTree, Mnemonic},
        txn_builder::{RecordInfo, TransactionReceipt},
        RecordAmount,
    };
    use serde::de::DeserializeOwned;
    use std::collections::{HashMap, HashSet};
    use std::convert::TryInto;
    use std::fmt::Debug;
    use std::io::Cursor;
    use std::iter::once;
    use std::path::{Path, PathBuf};
    use std::str::FromStr;
    use surf::Url;
    use tempdir::TempDir;
    use tracing_test::traced_test;

    fn test_icon() -> Icon {
        // Generate a simple icon in raw bytes: 4 bytes for width, 4 for height, and then
        // width*height*4 bytes for the pixels. Use 64x64 so seahorse doesn't resize the icon.
        let icon_width: u32 = 64;
        let icon_height: u32 = 64;
        let icon_data = [0; 4 * 64 * 64];
        let icon_bytes = icon_width
            .to_le_bytes()
            .iter()
            .chain(icon_height.to_le_bytes().iter())
            .chain(icon_data.iter())
            .cloned()
            .collect::<Vec<_>>();
        <Icon as CanonicalDeserialize>::deserialize(icon_bytes.as_slice()).unwrap()
    }

    fn base64(bytes: &[u8]) -> String {
        base64::encode_config(bytes, base64::URL_SAFE_NO_PAD)
    }

    fn fmt_path(path: &Path) -> String {
        let bytes = path.as_os_str().to_str().unwrap().as_bytes();
        base64(bytes)
    }

    struct TestServer {
        client: surf::Client,
        temp_dir: TempDir,
        options: NodeOpt,
    }

    impl TestServer {
        async fn new() -> Self {
            let port = port().await;

            // Run a server in the background that is unique to this test. Note that the server task
            // is leaked: tide does not provide any mechanism for graceful programmatic shutdown, so
            // the server will continue running until the process is killed, even after the test
            // ends. This is ok, since each test's server task should be idle once
            // the test is over.
            let temp_dir = TempDir::new("test_wallet_api_storage").unwrap();
            let options = NodeOpt::for_test(port as u16, temp_dir.path().to_path_buf());
            init_server(ChaChaRng::from_seed([42; 32]), &options).unwrap();
            Self::wait(port).await;

            let client: surf::Client = surf::Config::new()
                .set_base_url(Url::parse(&format!("http://localhost:{}", port)).unwrap())
                .set_timeout(None)
                .try_into()
                .unwrap();
            Self {
                client: client.with(client::parse_error_body::<CapeAPIError>),
                temp_dir,
                options,
            }
        }

        async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, surf::Error> {
            let mut res = self.client.get(path).send().await?;
            client::response_body(&mut res).await
        }

        async fn post<T: DeserializeOwned>(&self, path: &str) -> Result<T, surf::Error> {
            let mut res = self.client.post(path).send().await?;
            client::response_body(&mut res).await
        }

        async fn requires_wallet<T: Debug + DeserializeOwned>(&self, path: &str) {
            self.get::<T>(path)
                .await
                .expect_err(&format!("{} succeeded without an open wallet", path));
        }

        async fn requires_wallet_post<T: Debug + DeserializeOwned>(&self, path: &str) {
            self.post::<T>(path)
                .await
                .expect_err(&format!("{} succeeded without an open wallet", path));
        }

        fn path(&self) -> String {
            let path = [self.temp_dir.path(), Path::new("keystores/test_wallet")]
                .iter()
                .collect::<PathBuf>();
            fmt_path(&path)
        }

        fn options(&self) -> &NodeOpt {
            &self.options
        }

        async fn wait(port: u16) {
            retry(|| async move {
                // Use a one-off request, rather than going through the client, because we want to
                // skip the middleware, which can cause connect() to return an Err() even if the
                // request reaches the server successfully.
                surf::connect(format!("http://localhost:{}", port))
                    .send()
                    .await
                    .is_ok()
            })
            .await
        }
    }

    #[async_std::test]
    #[traced_test]
    async fn test_getmnemonic() {
        let server = TestServer::new().await;

        let mnemonic = server.get::<String>("getmnemonic").await.unwrap();

        // Check that the mnemonic decodes correctly.
        KeyTree::from_mnemonic(&Mnemonic::from_phrase(&mnemonic.replace('-', " ")).unwrap());

        // Check that successive calls give different mnemonics.
        assert_ne!(mnemonic, server.get::<String>("getmnemonic").await.unwrap());
    }

    #[cfg(feature = "slow-tests")]
    #[async_std::test]
    #[traced_test]
    async fn test_newwallet() {
        let server = TestServer::new().await;
        let mnemonic = server.get::<String>("getmnemonic").await.unwrap();
        let password = base64("my pass/word".as_bytes());

        // Should fail if the mnemonic is invalid.
        server
            .post::<()>(&format!(
                "newwallet/invalid-mnemonic/{}/path/{}",
                password,
                server.path()
            ))
            .await
            .expect_err("newwallet succeeded with an invalid mnemonic");

        // Should fail if the path is invalid.
        server
            .post::<()>(&format!(
                "newwallet/{}/{}/path/invalid-path",
                mnemonic, password
            ))
            .await
            .expect_err("newwallet succeeded with an invalid path");
        // Should fail if the password is invalid.
        server
            .post::<()>(&format!(
                "newwallet/{}/plaintext-password/path/{}",
                mnemonic,
                server.path()
            ))
            .await
            .expect_err("newwallet succeeded with an invalid password");
        // Should fail if the name is invalid.
        server
            .post::<()>(&format!(
                "newwallet/{}/{}/name/plaintext-name",
                mnemonic, password,
            ))
            .await
            .expect_err("newwallet succeeded with an invalid name");

        // Test successful calls, using names and passwords with spaces and slashes.
        server
            .post::<()>(&format!(
                "newwallet/{}/{}/path/{}",
                mnemonic,
                password,
                server.path()
            ))
            .await
            .unwrap();
        server
            .post::<()>(&format!(
                "newwallet/{}/{}/name/{}",
                mnemonic,
                password,
                base64("this is / a wallet name".as_bytes()),
            ))
            .await
            .unwrap();

        // Should fail if the wallet already exists.
        server
            .post::<()>(&format!(
                "newwallet/{}/{}/path/{}",
                mnemonic,
                password,
                server.path()
            ))
            .await
            .expect_err("newwallet succeeded when a wallet already existed");
    }

    #[cfg(feature = "slow-tests")]
    #[async_std::test]
    #[traced_test]
    async fn test_openwallet() {
        let server = TestServer::new().await;
        let mnemonic = server.get::<String>("getmnemonic").await.unwrap();
        let password = base64("my-password".as_bytes());

        // Should fail if no wallet exists.
        server
            .requires_wallet_post::<()>(&format!("openwallet/{}/path/{}", password, server.path()))
            .await;

        // Now create a wallet so we can open it.
        server
            .post::<()>(&format!(
                "newwallet/{}/{}/path/{}",
                mnemonic,
                password,
                server.path()
            ))
            .await
            .unwrap();
        server
            .post::<()>(&format!("openwallet/{}/path/{}", password, server.path()))
            .await
            .unwrap();

        // Should fail if the password is incorrect.
        server
            .post::<()>(&format!(
                "openwallet/invalid-password/path/{}",
                server.path()
            ))
            .await
            .expect_err("openwallet succeeded with an invalid password");

        // Should fail if the path is invalid.
        server
            .get::<()>(&format!("openwallet/{}/path/invalid-path", password))
            .await
            .expect_err("openwallet succeeded with an invalid path");
    }

    #[async_std::test]
    #[traced_test]
    async fn test_lastusedkeystore() {
        let server = TestServer::new().await;
        let mnemonic = server.get::<String>("getmnemonic").await.unwrap();
        println!("mnemonic: {}", mnemonic);
        let password = base64("my-password".as_bytes());

        // Should get None on first try if no last wallet.
        let opt = server
            .get::<Option<KeyStoreLocation>>("lastusedkeystore")
            .await
            .unwrap();
        assert!(opt.is_none());

        let url = format!("newwallet/{}/{}/path/{}", mnemonic, password, server.path());
        server.post::<()>(&url).await.unwrap();

        let mut loc = server
            .get::<Option<KeyStoreLocation>>("lastusedkeystore")
            .await
            .unwrap();
        assert_eq!(fmt_path(&loc.unwrap().path), server.path());

        // We should still get the same path after opening the wallet
        server
            .post::<()>(&format!("openwallet/{}/path/{}", password, server.path()))
            .await
            .unwrap();
        loc = server
            .get::<Option<KeyStoreLocation>>("lastusedkeystore")
            .await
            .unwrap();
        assert_eq!(fmt_path(&loc.as_ref().unwrap().path), server.path());

        // Open the wallet with the we path we retrieved
        server
            .post::<()>(&format!(
                "openwallet/{}/path/{}",
                password,
                fmt_path(&loc.as_ref().unwrap().path)
            ))
            .await
            .unwrap();

        // Test that the last path is updated when we create a new wallet w/ a new path
        server
            .post::<()>(&format!(
                "newwallet/{}/{}/name/{}",
                mnemonic,
                password,
                base64::encode_config("test_wallet_2", base64::URL_SAFE_NO_PAD),
            ))
            .await
            .unwrap();

        loc = server
            .get::<Option<KeyStoreLocation>>("lastusedkeystore")
            .await
            .unwrap();
        assert_eq!(loc.unwrap().name, Some("test_wallet_2".into()));

        // repopen the first wallet and see the path returned is also the original
        server
            .post::<()>(&format!("openwallet/{}/path/{}", password, server.path()))
            .await
            .unwrap();

        loc = server
            .get::<Option<KeyStoreLocation>>("lastusedkeystore")
            .await
            .unwrap();
        assert_eq!(fmt_path(&loc.unwrap().path), server.path());
    }

    #[cfg(feature = "slow-tests")]
    #[async_std::test]
    #[traced_test]
    async fn test_closewallet() {
        let server = TestServer::new().await;

        // Should fail if a wallet is not already open.
        server.requires_wallet_post::<()>("closewallet").await;

        // Now open a wallet and close it.
        server
            .post::<()>(&format!(
                "newwallet/{}/{}/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                base64("my-password".as_bytes()),
                server.path()
            ))
            .await
            .unwrap();
        server.post::<()>("closewallet").await.unwrap();
    }

    #[async_std::test]
    #[traced_test]
    async fn test_getinfo() {
        let server = TestServer::new().await;

        // Should fail if a wallet is not already open.
        server.requires_wallet::<WalletSummary>("getinfo").await;

        // Now open a wallet and call getinfo.
        server
            .post::<()>(&format!(
                "newwallet/{}/{}/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                base64("my-password".as_bytes()),
                server.path()
            ))
            .await
            .unwrap();
        let info = server.get::<WalletSummary>("getinfo").await.unwrap();

        assert_eq!(info.addresses, vec![]);
        assert_eq!(info.sending_keys, vec![]);
        assert_eq!(info.viewing_keys, vec![]);
        assert_eq!(info.freezing_keys, vec![]);
        assert_eq!(info.assets, vec![AssetInfo::native()]);
        // The wallet should be up-to-date with the EQS.
        assert_eq!(info.sync_time, info.real_time);
    }

    #[async_std::test]
    #[traced_test]
    async fn test_getaddress() {
        let server = TestServer::new().await;

        // Should fail if a wallet is not already open.
        server
            .requires_wallet::<Vec<UserAddress>>("getaddress")
            .await;

        // Now open a wallet and call getaddress.
        server
            .post::<()>(&format!(
                "newwallet/{}/{}/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                base64("my-password".as_bytes()),
                server.path()
            ))
            .await
            .unwrap();
        let addresses = server.get::<Vec<UserAddress>>("getaddress").await.unwrap();

        // The result is not very interesting before we add any keys, but that's for another
        // endpoint.
        assert_eq!(addresses, vec![]);
    }

    #[async_std::test]
    #[traced_test]
    async fn test_getrecords() {
        let server = TestServer::new().await;

        // Should fail if a wallet is not already open.
        server
            .requires_wallet::<Vec<UserAddress>>("getrecords")
            .await;

        // Now open a wallet populate it and call getrecords.
        server
            .post::<()>(&format!(
                "newwallet/{}/{}/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                base64("my-password".as_bytes()),
                server.path()
            ))
            .await
            .unwrap();
        server
            .get::<TransactionReceipt<CapeLedger>>("populatefortest")
            .await
            .unwrap();

        let records = server.get::<Vec<RecordInfo>>("getrecords").await.unwrap();
        let info = server.get::<WalletSummary>("getinfo").await.unwrap();

        // get the wrapped asset
        let asset = if info.assets[0].definition.code == AssetCode::native() {
            info.assets[1].definition.code
        } else {
            info.assets[0].definition.code
        };
        // populate for test should create 3 records
        assert_eq!(records.len(), 3);

        let ro1 = &records[0].ro;
        let ro2 = &records[1].ro;
        let ro3 = &records[2].ro;

        assert_eq!(ro1.amount, DEFAULT_NATIVE_AMT_IN_FAUCET_ADDR.into());
        assert_eq!(ro1.asset_def.code, AssetCode::native());
        assert_eq!(ro2.amount, DEFAULT_NATIVE_AMT_IN_WRAPPER_ADDR.into());
        assert_eq!(ro2.asset_def.code, AssetCode::native());
        assert_eq!(ro3.amount, DEFAULT_WRAPPED_AMT.into());
        assert_eq!(ro3.asset_def.code, asset);
    }

    #[async_std::test]
    #[traced_test]
    async fn test_getbalance() {
        let server = TestServer::new().await;
        let mut rng = ChaChaRng::from_seed([42u8; 32]);

        let addr = UserAddress::from(UserKeyPair::generate(&mut rng).address());
        let asset = AssetCode::native();

        // Should fail if a wallet is not already open.
        server
            .requires_wallet::<BalanceInfo>("getbalance/all")
            .await;
        server
            .requires_wallet::<BalanceInfo>(&format!("getbalance/address/{}", addr))
            .await;
        server
            .requires_wallet::<BalanceInfo>(&format!("getbalance/address/{}/asset/{}", addr, asset))
            .await;

        // Now open a wallet.
        server
            .post::<()>(&format!(
                "newwallet/{}/{}/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                base64("my-password".as_bytes()),
                server.path()
            ))
            .await
            .unwrap();

        // We can now hit the endpoints successfully, although there are currently no balances
        // because we haven't added any keys or received any records.
        assert_eq!(
            server
                .get::<BalanceInfo>("getbalance/all")
                .await
                .unwrap()
                .balances,
            Balances::All {
                by_account: HashMap::default(),
                aggregate: HashMap::default(),
            }
        );
        let assets = server.get::<WalletSummary>("getinfo").await.unwrap().assets;
        assert_eq!(
            server
                .get::<BalanceInfo>(&format!("getbalance/address/{}", addr))
                .await
                .unwrap(),
            // Even though this address has not been added to the wallet (and thus was not included
            // in the results of `getbalance/all`), if we specifically request its balance, the
            // wallet will check for records of each known asset type belonging to this address,
            // find none, and return a balance of 0 for that asset type. Since the wallet always
            // knows about the native asset type, this will actually return some data, rather than
            // an empty map or an error.
            BalanceInfo {
                balances: Balances::Account(once((AssetCode::native(), 0u64.into())).collect()),
                assets: once((AssetCode::native(), assets[0].clone())).collect(),
            }
        );
        assert_eq!(
            server
                .get::<BalanceInfo>(&format!("getbalance/address/{}/asset/{}", addr, asset))
                .await
                .unwrap()
                .balances,
            Balances::One(0u64.into()),
        );
        // If we query for a specific asset code, we should get a balance of 0 even if the wallet
        // doesn't know about this asset yet.
        assert_eq!(
            server
                .get::<BalanceInfo>(&format!(
                    "getbalance/address/{}/asset/{}",
                    addr,
                    AssetCode::random(&mut rng).0
                ))
                .await
                .unwrap()
                .balances,
            Balances::One(0u64.into()),
        );

        // Should fail with an invalid address (we'll get an invalid address by serializing an asset
        // code where the address should go.).
        server
            .get::<BalanceInfo>(&format!("getbalance/address/{}", asset))
            .await
            .expect_err("getbalance succeeded with an invalid address");
        server
            .get::<BalanceInfo>(&format!("getbalance/address/{}/asset/{}", asset, asset))
            .await
            .expect_err("getbalance succeeded with an invalid address");
        // Should fail with an invalid asset code (we'll use an address where the asset should go).
        server
            .get::<BalanceInfo>(&format!("getbalance/address/{}/asset/{}", addr, addr))
            .await
            .expect_err("getbalance succeeded with an invalid asset code");
        // Should fail with route pattern misuse (e.g. specifying `address` route component without
        // an accompanying `:address` parameter).
        server
            .get::<BalanceInfo>("getbalance/address")
            .await
            .expect_err("getbalance/address succeeded with invalid route pattern");
        server
            .get::<BalanceInfo>("getbalance/asset")
            .await
            .expect_err("getbalance/asset succeeded with invalid route pattern");
        server
            .get::<BalanceInfo>("getbalance")
            .await
            .expect_err("getbalance succeeded with invalid route pattern");
    }

    #[cfg(feature = "slow-tests")]
    #[async_std::test]
    #[traced_test]
    async fn test_aggregate_balance() {
        let server = TestServer::new().await;

        // Open a wallet and populate at least 2 addresses with at least 2 assets.
        server
            .post::<()>(&format!(
                "newwallet/{}/{}/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                base64("my-password".as_bytes()),
                server.path()
            ))
            .await
            .unwrap();
        let receipt = server
            .get::<TransactionReceipt<CapeLedger>>("populatefortest")
            .await
            .unwrap();

        // After populate for test, the faucet address has some native tokens, and the receiver
        // of the transfer has some native tokens and some wrapped tokens.
        let faucet_addr: UserAddress = receipt.submitters[0].clone().into();

        // Get the wrapped asset.
        let mut info = server.get::<WalletSummary>("getinfo").await.unwrap();
        let wrapped_asset = if info.assets[0].definition.code == AssetCode::native() {
            info.assets.remove(1)
        } else {
            info.assets.remove(0)
        };

        // Get the address with the wrapped asset.
        let mut wrapper_addr: Option<UserAddress> = None;
        for address in info.addresses {
            if Balances::One(DEFAULT_WRAPPED_AMT.into())
                == server
                    .get::<BalanceInfo>(&format!(
                        "getbalance/address/{}/asset/{}",
                        address, wrapped_asset.definition.code
                    ))
                    .await
                    .unwrap()
                    .balances
            {
                wrapper_addr = Some(address);
                break;
            }
        }
        let wrapper_addr = wrapper_addr.unwrap();

        // Transfer some of the wrapped asset to the faucet account, so that both accounts have
        // a balance of each asset type.
        server
            .post::<TransactionReceipt<CapeLedger>>(&format!(
                "send/asset/{}/recipient/{}/amount/{}/fee/0",
                wrapped_asset.definition.code,
                faucet_addr,
                DEFAULT_WRAPPED_AMT / 2
            ))
            .await
            .unwrap();
        retry(|| async {
            server
                .get::<BalanceInfo>(&format!(
                    "getbalance/address/{}/asset/{}",
                    faucet_addr, wrapped_asset.definition.code
                ))
                .await
                .unwrap()
                .balances
                == Balances::One((DEFAULT_WRAPPED_AMT / 2).into())
        })
        .await;

        // Now each asset is distributed across two accounts. Check the balance of each account
        // and the aggregate balance.
        let balance_info = server.get::<BalanceInfo>("getbalance/all").await.unwrap();
        let (by_account, aggregate) = match balance_info.balances {
            Balances::All {
                by_account,
                aggregate,
            } => (by_account, aggregate),
            balances => panic!("expected Balances::All, got {:?}", balances),
        };
        assert_eq!(
            by_account[&faucet_addr],
            vec![
                (
                    AssetCode::native(),
                    DEFAULT_NATIVE_AMT_IN_FAUCET_ADDR.into()
                ),
                (
                    wrapped_asset.definition.code,
                    (DEFAULT_WRAPPED_AMT / 2).into()
                )
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(
            by_account[&wrapper_addr],
            vec![
                (
                    AssetCode::native(),
                    DEFAULT_NATIVE_AMT_IN_WRAPPER_ADDR.into()
                ),
                (
                    wrapped_asset.definition.code,
                    (DEFAULT_WRAPPED_AMT - (DEFAULT_WRAPPED_AMT / 2)).into()
                )
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(
            aggregate,
            vec![
                (
                    AssetCode::native(),
                    (DEFAULT_NATIVE_AMT_IN_FAUCET_ADDR + DEFAULT_NATIVE_AMT_IN_WRAPPER_ADDR).into()
                ),
                (wrapped_asset.definition.code, DEFAULT_WRAPPED_AMT.into()),
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(
            balance_info.assets,
            vec![
                (wrapped_asset.definition.code, wrapped_asset),
                (AssetCode::native(), AssetInfo::native()),
            ]
            .into_iter()
            .collect()
        );
    }

    #[async_std::test]
    #[traced_test]
    async fn test_newkey() {
        let server = TestServer::new().await;

        // Should fail if a wallet is not already open.
        server
            .requires_wallet_post::<PubKey>("newkey/sending")
            .await;
        server
            .requires_wallet_post::<PubKey>("newkey/tracing")
            .await;
        server
            .requires_wallet_post::<PubKey>("newkey/freezing")
            .await;

        // Now open a wallet.
        server
            .post::<()>(&format!(
                "newwallet/{}/{}/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                base64("my-password".as_bytes()),
                server.path()
            ))
            .await
            .unwrap();

        // newkey should return a public key with the correct type and add the key to the wallet.
        let sending_key = server.post::<PubKey>("newkey/sending").await.unwrap();
        let viewing_key = server.post::<PubKey>("newkey/viewing").await.unwrap();
        let freezing_key = server.post::<PubKey>("newkey/freezing").await.unwrap();
        let info = server.get::<WalletSummary>("getinfo").await.unwrap();
        match sending_key {
            PubKey::Sending(key) => {
                assert_eq!(info.sending_keys, vec![key]);
            }
            _ => {
                panic!("Expected PubKey::Sending, found {:?}", sending_key);
            }
        }
        match viewing_key {
            PubKey::Viewing(key) => {
                assert_eq!(info.viewing_keys, vec![key]);
            }
            _ => {
                panic!("Expected PubKey::Viewing, found {:?}", viewing_key);
            }
        }
        match freezing_key {
            PubKey::Freezing(key) => {
                assert_eq!(info.freezing_keys, vec![key]);
            }
            _ => {
                panic!("Expected PubKey::Freezing, found {:?}", freezing_key);
            }
        }

        // Test named keys.
        match server
            .post::<PubKey>(&format!(
                "newkey/sending/description/{}",
                base64::encode("sending".as_bytes())
            ))
            .await
            .unwrap()
        {
            PubKey::Sending(key) => {
                let account = server
                    .get::<Account>(&format!("getaccount/{}", key))
                    .await
                    .unwrap();
                assert_eq!(account.description, "sending");
            }
            key => panic!("Expected PubKey::Sending, found {:?}", key),
        }
        match server
            .post::<PubKey>(&format!(
                "newkey/viewing/description/{}",
                base64::encode("viewing".as_bytes())
            ))
            .await
            .unwrap()
        {
            PubKey::Viewing(key) => {
                let account = server
                    .get::<Account>(&format!("getaccount/{}", key))
                    .await
                    .unwrap();
                assert_eq!(account.description, "viewing");
            }
            key => panic!("Expected PubKey::Viewing, found {:?}", key),
        }
        match server
            .post::<PubKey>(&format!(
                "newkey/freezing/description/{}",
                base64::encode("freezing".as_bytes())
            ))
            .await
            .unwrap()
        {
            PubKey::Freezing(key) => {
                let account = server
                    .get::<Account>(&format!("getaccount/{}", key))
                    .await
                    .unwrap();
                assert_eq!(account.description, "freezing");
            }
            key => panic!("Expected PubKey::Freezing, found {:?}", key),
        }

        // Should fail if the key type is invaild.
        server
            .post::<PubKey>("newkey/invalid_key_type")
            .await
            .expect_err("newkey succeeded with an invaild key type");
    }

    #[async_std::test]
    #[traced_test]
    async fn test_newasset() {
        let server = TestServer::new().await;

        // Set parameters for newasset.
        let viewing_threshold = RecordAmount::from(10u128);
        let view_amount = true;
        let view_address = false;
        let description = base64::encode_config(&[3u8; 32], base64::URL_SAFE_NO_PAD);

        // Should fail if a wallet is not already open.
        server
            .requires_wallet_post::<AssetInfo>(&format!(
                "newasset/description/{}/view_amount/{}/view_address/{}/viewing_threshold/{}",
                description, view_amount, view_address, viewing_threshold
            ))
            .await;

        // Now open a wallet.
        server
            .post::<()>(&format!(
                "newwallet/{}/{}/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                base64("my-password".as_bytes()),
                server.path()
            ))
            .await
            .unwrap();

        // Create keys.
        server.post::<PubKey>("newkey/viewing").await.unwrap();
        server.post::<PubKey>("newkey/freezing").await.unwrap();
        let info = server.get::<WalletSummary>("getinfo").await.unwrap();
        let viewing_key = &info.viewing_keys[0];
        let freezing_key = &info.freezing_keys[0];

        // newasset should return a defined asset with the correct policy if no ERC20 code is given.
        let asset = server
            .post::<AssetInfo>(&format!(
                "newasset/description/{}/freezing_key/{}/viewing_key/{}/view_amount/{}/view_address/{}/viewing_threshold/{}",
                description, freezing_key, viewing_key, view_amount, view_address, viewing_threshold
            ))
            .await
            .unwrap();
        assert_eq!(asset.wrapped_erc20, None);
        assert_eq!(&asset.definition.viewing_key.unwrap(), viewing_key);
        assert_eq!(&asset.definition.freezing_key.unwrap(), freezing_key);
        assert_eq!(asset.definition.viewing_threshold, viewing_threshold);
        let asset = server
            .post::<AssetInfo>(&format!(
            "newasset/freezing_key/{}/viewing_key/{}/view_amount/{}/view_address/{}/viewing_threshold/{}",
            freezing_key, viewing_key, view_amount, view_address, viewing_threshold
        ))
            .await
            .unwrap();
        assert_eq!(&asset.definition.viewing_key.unwrap(), viewing_key);
        assert_eq!(&asset.definition.freezing_key.unwrap(), freezing_key);
        assert_eq!(asset.definition.viewing_threshold, viewing_threshold);

        // newasset should return an asset with the default freezer public key if it's not given.
        let asset = server
            .post::<AssetInfo>(&format!(
                "newasset/description/{}/viewing_key/{}/view_amount/{}/view_address/{}/viewing_threshold/{}",
                description, viewing_key, view_amount, view_address, viewing_threshold
            ))
            .await
            .unwrap();
        assert!(asset.definition.freezing_key.is_none());

        // newasset should return an asset with the default auditor public key and no reveal threshold if an
        // auditor public key isn't given.
        let asset = server
            .post::<AssetInfo>(&format!("newasset/description/{}", description))
            .await
            .unwrap();
        assert!(asset.definition.viewing_key.is_none());
        assert_eq!(asset.definition.viewing_threshold, 0u128.into());

        // newasset should return an asset with no reveal threshold if it's not given.
        let asset = server
            .post::<AssetInfo>(&format!(
                "newasset/description/{}/freezing_key/{}/viewing_key/{}/view_amount/{}/view_address/{}",
                description, freezing_key, viewing_key, view_amount, view_address
            ))
            .await
            .unwrap();
        assert_eq!(asset.definition.viewing_threshold, 0u128.into());

        // newasset should return an asset with a given symbol.
        let asset = server
            .post::<AssetInfo>(&format!(
                "newasset/symbol/{}/description/{}/freezing_key/{}/viewing_key/{}/view_amount/{}/view_address/{}",
               base64::encode_config("my-defined-asset", base64::URL_SAFE_NO_PAD), description,
               freezing_key, viewing_key, view_amount, view_address
            ))
            .await
            .unwrap();
        assert_eq!(asset.symbol, Some("my-defined-asset".into()));
    }

    #[async_std::test]
    #[traced_test]
    async fn test_sponsor() {
        let server = TestServer::new().await;

        // Set parameters for /sponsor.
        let erc20_code = Address::from([1u8; 20]);
        let sponsor_addr = Address::from([2u8; 20]);
        let viewing_threshold = RecordAmount::from(10u128);
        let view_amount = true;
        let view_address = false;

        // Should fail if a wallet is not already open.
        server
            .requires_wallet_post::<(sol::AssetDefinition, String)>(&format!(
                "buildsponsor/erc20/{}/sponsor/{}/view_amount/{}/view_address/{}/viewing_threshold/{}",
                erc20_code, sponsor_addr, view_amount, view_address, viewing_threshold
            ))
            .await;

        // Now open a wallet.
        server
            .post::<()>(&format!(
                "newwallet/{}/{}/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                base64("my-password".as_bytes()),
                server.path()
            ))
            .await
            .unwrap();

        // Create keys.
        server.post::<PubKey>("newkey/viewing").await.unwrap();
        server.post::<PubKey>("newkey/freezing").await.unwrap();
        let info = server.get::<WalletSummary>("getinfo").await.unwrap();
        let viewing_key = &info.viewing_keys[0];
        let freezing_key = &info.freezing_keys[0];

        // Test /sponsor
        let (asset, info) = server
            .post::<(sol::AssetDefinition, String)>(&format!(
                "buildsponsor/erc20/{:#x}/sponsor/{:#x}/freezing_key/{}/viewing_key/{}/view_amount/{}/view_address/{}/viewing_threshold/{}",
                erc20_code, sponsor_addr, freezing_key, viewing_key, view_amount, view_address, viewing_threshold
            ))
            .await
            .unwrap();
        assert_eq!(
            &AuditorPubKey::from(asset.policy.auditor_pk.clone()),
            viewing_key
        );
        assert_eq!(
            &FreezerPubKey::from(asset.policy.freezer_pk.clone()),
            freezing_key
        );
        assert_eq!(asset.policy.reveal_threshold, viewing_threshold);
        // Add the asset to the library.
        server
            .client
            .post("importasset")
            .body_json(&info)
            .unwrap()
            .send()
            .await
            .unwrap();
        let info = server
            .get::<WalletSummary>("getinfo")
            .await
            .unwrap()
            .assets
            .into_iter()
            .find(|info| info.definition.code == asset.code.into())
            .unwrap();
        // `wrapped_erc20` is None because the asset isn't sponsored yet. We have only built the
        // body of the sponsor transaction.
        assert_eq!(info.wrapped_erc20, None);
        assert_eq!(sol::AssetDefinition::from(info.definition.clone()), asset);
        // After submitting the transaction, `wrapped_erc20` is populated.
        let mut submitted_info: AssetInfo = server
            .client
            .post(&format!(
                "submitsponsor/erc20/{:#x}/sponsor/{:#x}",
                erc20_code, sponsor_addr
            ))
            .body_json(&asset)
            .unwrap()
            .send()
            .await
            .unwrap()
            .body_json()
            .await
            .unwrap();
        assert_eq!(
            Address::from_str(&submitted_info.wrapped_erc20.unwrap()).unwrap(),
            erc20_code
        );
        // Other than that, the new info is the same as the old one.
        submitted_info.wrapped_erc20 = None;
        assert_eq!(submitted_info, info);

        // sponsor should return an asset with the default freezer public key if it's not given.
        let erc20_code = Address::from([2u8; 20]);
        let (asset, _) = server
                .post::<(sol::AssetDefinition, String)>(&format!(
                    "buildsponsor/erc20/{:#x}/sponsor/{:#x}/viewing_key/{}/view_amount/{}/view_address/{}/viewing_threshold/{}",
                    erc20_code, sponsor_addr, viewing_key, view_amount, view_address, viewing_threshold
                ))
                .await
                .unwrap();
        assert!(!AssetPolicy::from(asset.policy).is_freezer_pub_key_set());

        // sponsor should return an asset with the default auditor public key and no reveal
        // threshold if an auditor public key isn't given.
        let erc20_code = Address::from([3u8; 20]);
        let (asset, _) = server
            .post::<(sol::AssetDefinition, String)>(&format!(
                "buildsponsor/erc20/{:#x}/sponsor/{:#x}/freezing_key/{}",
                erc20_code, sponsor_addr, freezing_key
            ))
            .await
            .unwrap();
        assert_eq!(asset.policy.reveal_threshold, 0u128.into());
        assert!(!AssetPolicy::from(asset.policy).is_auditor_pub_key_set());

        // sponsor should return an asset with no reveal threshold if it's not given.
        let erc20_code = Address::from([4u8; 20]);
        let (asset, _) = server
                .post::<(sol::AssetDefinition, String)>(&format!(
                    "buildsponsor/erc20/{:#x}/sponsor/{:#x}/freezing_key/{}/viewing_key/{}/view_amount/{}/view_address/{}",
                    erc20_code, sponsor_addr, freezing_key, viewing_key, view_amount, view_address
                ))
                .await
                .unwrap();
        assert_eq!(asset.policy.reveal_threshold, 0u128.into());

        // sponsor should create an asset with a given symbol and description.
        let erc20_code = Address::from([5u8; 20]);
        let (asset, info) = server
                .post::<(sol::AssetDefinition, String)>(&format!(
                    "buildsponsor/symbol/{}/description/{}/erc20/{:#x}/sponsor/{:#x}/freezing_key/{}/viewing_key/{}/view_amount/{}/view_address/{}",
                    base64::encode_config("my-wrapped-asset", base64::URL_SAFE_NO_PAD), 
                    base64::encode_config("my-wrapped-asset description", base64::URL_SAFE_NO_PAD),
                    erc20_code, sponsor_addr, freezing_key, viewing_key, view_amount, view_address
                ))
                .await
                .unwrap();
        server
            .client
            .post("importasset")
            .body_json(&info)
            .unwrap()
            .send()
            .await
            .unwrap();
        let info = server
            .get::<WalletSummary>("getinfo")
            .await
            .unwrap()
            .assets
            .into_iter()
            .find(|info| info.definition.code == asset.code.into())
            .unwrap();
        assert_eq!(info.symbol, Some("my-wrapped-asset".into()));
        assert_eq!(
            info.description,
            Some("my-wrapped-asset description".into())
        );
    }

    #[async_std::test]
    #[traced_test]
    async fn test_wrap() {
        // Set parameters for sponsor and wrap.
        let erc20_code = Address::from([1u8; 20]);
        let sponsor_addr = Address::from([2u8; 20]);

        // Open a wallet.
        let server = TestServer::new().await;
        let mut rng = ChaChaRng::from_seed([42u8; 32]);
        server
            .post::<()>(&format!(
                "newwallet/{}/{}/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                base64("my-password".as_bytes()),
                server.path()
            ))
            .await
            .unwrap();

        // Sponsor an asset.
        let (asset, info) = server
            .post::<(sol::AssetDefinition, String)>(&format!(
                "buildsponsor/erc20/{:#x}/sponsor/{:#x}",
                erc20_code, sponsor_addr
            ))
            .await
            .unwrap();
        server
            .client
            .post("importasset")
            .body_json(&info)
            .unwrap()
            .send()
            .await
            .unwrap();
        server
            .client
            .post(&format!(
                "submitsponsor/erc20/{:#x}/sponsor/{:#x}",
                erc20_code, sponsor_addr
            ))
            .body_json(&asset)
            .unwrap()
            .send()
            .await
            .unwrap();
        let asset: JfAssetDefinition = asset.into();

        // Create an address to receive the wrapped asset.
        server.post::<PubKey>("newkey/sending").await.unwrap();
        let info = server.get::<WalletSummary>("getinfo").await.unwrap();
        let sending_key = &info.sending_keys[0];
        let destination: UserAddress = sending_key.address().into();

        // buildwrap should fail if the destination or asset is invalid.
        let invalid_destination = UserAddress::from(UserKeyPair::generate(&mut rng).address());
        let invalid_code = AssetCode::dummy();
        server
            .post::<sol::RecordOpening>(&format!(
                "buildwrap/destination/{}/asset/{}/amount/{}",
                invalid_destination, asset.code, 10
            ))
            .await
            .expect_err("buildwrap succeeded with an invalid user address");
        server
            .post::<sol::RecordOpening>(&format!(
                "buildwrap/destination/{}/asset/{}/amount/{}",
                destination, invalid_code, 10
            ))
            .await
            .expect_err("buildwrap succeeded with an invalid asset");

        // buildwrap should succeed with the correct information.
        let ro = server
            .post::<sol::RecordOpening>(&format!(
                "buildwrap/destination/{}/asset/{}/amount/{}",
                destination, asset.code, 10
            ))
            .await
            .unwrap();

        // submitwrap should fail if the Ethereum address or record opening is invalid.
        let invalid_ro = sol::RecordOpening::default();
        server
            .client
            .post(&format!("submitwrap/ethaddress/0xinvalid"))
            .body_json(&ro)
            .unwrap()
            .send()
            .await
            .expect_err("submitwrap succeeded with an invalid Ethereum address");
        server
            .client
            .post(&format!("submitwrap/ethaddress/{:#x}", sponsor_addr))
            .body_json(&invalid_ro)
            .unwrap()
            .send()
            .await
            .expect_err("submitwrap succeeded with an invalid record opening");

        // submitwrap should succeed with the correct information.
        server
            .client
            .post(&format!("submitwrap/ethaddress/{:#x}", sponsor_addr))
            .body_json(&ro)
            .unwrap()
            .send()
            .await
            .unwrap();
    }

    #[async_std::test]
    #[traced_test]
    async fn test_mint() {
        // Set parameters.
        let description = base64::encode_config(&[3u8; 32], base64::URL_SAFE_NO_PAD);
        let amount = 10;
        let fee = 1;
        let mut rng = ChaChaRng::from_seed([50u8; 32]);

        // Open a wallet with some initial grants and keys.
        let server = TestServer::new().await;
        server
            .post::<()>(&format!(
                "newwallet/{}/{}/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                base64("minter-password".as_bytes()),
                server.path()
            ))
            .await
            .unwrap();
        let receipt = server
            .get::<TransactionReceipt<CapeLedger>>("populatefortest")
            .await
            .unwrap();

        // Define an asset.
        let asset = server
            .post::<AssetInfo>(&format!("newasset/description/{}", description))
            .await
            .unwrap()
            .definition
            .code;

        // Get the faucet address with non-zero balance of the native asset.
        let minter: UserAddress = receipt.submitters[0].clone().into();

        // Get an address to receive the minted asset.
        let recipient: UserAddress = server
            .get::<WalletSummary>("getinfo")
            .await
            .unwrap()
            .sending_keys[0]
            .address()
            .into();

        // mint should fail if any of the asset, minter address, and recipient address is invalid.
        let invalid_asset = AssetDefinition::dummy();
        let invalid_minter = UserAddress::from(UserKeyPair::generate(&mut rng).address());
        let invalid_recipient = UserAddress::from(UserKeyPair::generate(&mut rng).address());
        server
            .post::<TransactionReceipt<CapeLedger>>(&format!(
                "mint/asset/{}/amount/{}/fee/{}/minter/{}/recipient/{}",
                invalid_asset, amount, fee, minter, recipient
            ))
            .await
            .expect_err("mint succeeded with an invalid asset");
        server
            .post::<TransactionReceipt<CapeLedger>>(&format!(
                "mint/asset/{}/amount/{}/fee/{}/minter/{}/recipient/{}",
                asset, amount, fee, invalid_minter, recipient
            ))
            .await
            .expect_err("mint succeeded with an invalid minter address");
        server
            .post::<TransactionReceipt<CapeLedger>>(&format!(
                "mint/asset/{}/amount/{}/fee/{}/minter/{}/recipient/{}",
                asset, amount, fee, minter, invalid_recipient
            ))
            .await
            .expect_err("mint succeeded with an invalid recipient address");

        // mint should succeed with the correct information.
        server
            .post::<TransactionReceipt<CapeLedger>>(&format!(
                "mint/asset/{}/amount/{}/fee/{}/minter/{}/recipient/{}",
                asset, amount, fee, minter, recipient
            ))
            .await
            .unwrap();

        // Check the balances of the minted asset and the native asset.
        retry(|| async {
            server
                .get::<BalanceInfo>(&format!("getbalance/address/{}/asset/{}", recipient, asset))
                .await
                .unwrap()
                .balances
                == Balances::One(amount.into())
        })
        .await;
        retry(|| async {
            server
                .get::<BalanceInfo>(&format!(
                    "getbalance/address/{}/asset/{}",
                    minter,
                    AssetCode::native()
                ))
                .await
                .unwrap()
                .balances
                == Balances::One((DEFAULT_NATIVE_AMT_IN_FAUCET_ADDR - fee).into())
        })
        .await;
    }

    #[async_std::test]
    #[traced_test]
    async fn test_unwrap() {
        // Set parameters.
        let eth_addr = DEFAULT_ETH_ADDR;
        let fee = 1;

        // Open a wallet with some wrapped and native assets.
        let server = TestServer::new().await;
        server
            .post::<()>(&format!(
                "newwallet/{}/{}/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                base64("my-password".as_bytes()),
                server.path()
            ))
            .await
            .unwrap();
        server
            .get::<TransactionReceipt<CapeLedger>>("populatefortest")
            .await
            .unwrap();

        // Get the wrapped asset.
        let info = server.get::<WalletSummary>("getinfo").await.unwrap();
        let asset = if info.assets[0].definition.code == AssetCode::native() {
            info.assets[1].definition.code
        } else {
            info.assets[0].definition.code
        };

        // Get the source address with the wrapped asset.
        let mut source_addr: Option<UserAddress> = None;
        for address in info.addresses {
            if Balances::One(DEFAULT_WRAPPED_AMT.into())
                == server
                    .get::<BalanceInfo>(&format!("getbalance/address/{}/asset/{}", address, asset))
                    .await
                    .unwrap()
                    .balances
            {
                source_addr = Some(address);
                break;
            }
        }
        let source = source_addr.unwrap();

        // unwrap should fail if any of the source, Ethereum address, and asset is invalid.
        let invalid_source = UserAddress::from(
            UserKeyPair::generate(&mut ChaChaRng::from_seed([50u8; 32])).address(),
        );
        let invalid_asset = AssetDefinition::dummy();
        server
            .post::<TransactionReceipt<CapeLedger>>(&format!(
                "unwrap/source/{}/ethaddress/{:#x}/asset/{}/amount/{}/fee/{}",
                invalid_source, eth_addr, asset, DEFAULT_WRAPPED_AMT, 1
            ))
            .await
            .expect_err("unwrap succeeded with an invalid source address");
        server
            .post::<TransactionReceipt<CapeLedger>>(&format!(
                "unwrap/source/{}/ethaddress/0xinvalid/asset/{}/amount/{}/fee/{}",
                source, asset, DEFAULT_WRAPPED_AMT, 1
            ))
            .await
            .expect_err("unwrap succeeded with an invalid Ethereum address");
        server
            .post::<TransactionReceipt<CapeLedger>>(&format!(
                "unwrap/source/{}/ethaddress/{:#x}/asset/{}/amount/{}/fee/{}",
                source, eth_addr, invalid_asset, DEFAULT_WRAPPED_AMT, 1
            ))
            .await
            .expect_err("unwrap succeeded with an invalid asset");

        // unwrap should succeed with the correct information.
        server
            .post::<TransactionReceipt<CapeLedger>>(&format!(
                "unwrap/source/{}/ethaddress/{:#x}/asset/{}/amount/{}/fee/{}",
                source, eth_addr, asset, DEFAULT_WRAPPED_AMT, fee
            ))
            .await
            .unwrap();

        // Check the balances of the wrapped and native assets.
        retry(|| async {
            server
                .get::<BalanceInfo>(&format!("getbalance/address/{}/asset/{}", source, asset))
                .await
                .unwrap()
                .balances
                == Balances::One(0u64.into())
        })
        .await;
        retry(|| async {
            server
                .get::<BalanceInfo>(&format!(
                    "getbalance/address/{}/asset/{}",
                    source,
                    AssetCode::native()
                ))
                .await
                .unwrap()
                .balances
                == Balances::One((DEFAULT_NATIVE_AMT_IN_WRAPPER_ADDR - fee).into())
        })
        .await;
    }

    #[async_std::test]
    #[traced_test]
    async fn test_dummy_populate() {
        let server = TestServer::new().await;
        server
            .post::<()>(&format!(
                "newwallet/{}/{}/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                base64("my-password".as_bytes()),
                server.path()
            ))
            .await
            .unwrap();
        server
            .get::<TransactionReceipt<CapeLedger>>("populatefortest")
            .await
            .unwrap();

        let info = server.get::<WalletSummary>("getinfo").await.unwrap();
        assert_eq!(info.addresses.len(), 3);
        assert_eq!(info.sending_keys.len(), 3);
        assert_eq!(info.viewing_keys.len(), 2);
        assert_eq!(info.freezing_keys.len(), 2);
        assert_eq!(info.assets.len(), 2); // native asset + wrapped asset

        // One of the addresses should have a non-zero balance of the native asset type.
        let mut found_native = false;
        for address in &info.addresses {
            if Balances::One(DEFAULT_NATIVE_AMT_IN_FAUCET_ADDR.into())
                == server
                    .get::<BalanceInfo>(&format!(
                        "getbalance/address/{}/asset/{}",
                        address,
                        AssetCode::native()
                    ))
                    .await
                    .unwrap()
                    .balances
            {
                found_native = true;
                break;
            }
        }
        assert!(found_native);

        // One of the wallet's two assets is the native asset, and the other is the wrapped asset
        // for which we have a nonzero balance, but the order depends on the hash of the wrapped
        // asset code, which is non-deterministic, so we check both.
        let wrapped_asset = if info.assets[0].definition.code == AssetCode::native() {
            info.assets[1].definition.code
        } else {
            info.assets[0].definition.code
        };
        assert_ne!(wrapped_asset, AssetCode::native());

        // One of the addresses should have the expected balance of the wrapped asset type.
        let mut found_wrapped = false;
        for address in &info.addresses {
            if Balances::One(DEFAULT_WRAPPED_AMT.into())
                == server
                    .get::<BalanceInfo>(&format!(
                        "getbalance/address/{}/asset/{}",
                        address, wrapped_asset
                    ))
                    .await
                    .unwrap()
                    .balances
            {
                found_wrapped = true;
                break;
            }
        }
        assert!(found_wrapped);
    }

    #[async_std::test]
    #[traced_test]
    async fn test_send() {
        let server = TestServer::new().await;
        let mut rng = ChaChaRng::from_seed([1; 32]);

        // Should fail if a wallet is not already open.
        server
            .requires_wallet_post::<AssetDefinition>(&format!(
                "send/sender/{}/asset/{}/recipient/{}/amount/1/fee/1",
                UserKeyPair::generate(&mut rng).address(),
                AssetCode::random(&mut rng).0,
                EthereumAddr([1; 20]),
            ))
            .await;
        server
            .requires_wallet_post::<AssetDefinition>(&format!(
                "send/asset/{}/recipient/{}/amount/1/fee/1",
                AssetCode::random(&mut rng).0,
                EthereumAddr([1; 20]),
            ))
            .await;

        // Now open a wallet.
        server
            .post::<()>(&format!(
                "newwallet/{}/{}/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                base64("my-password".as_bytes()),
                server.path()
            ))
            .await
            .unwrap();

        // Populate the wallet with some dummy data so we have a balance of an asset to send.
        let receipt = server
            .get::<TransactionReceipt<CapeLedger>>("populatefortest")
            .await
            .unwrap();
        let info = server.get::<WalletSummary>("getinfo").await.unwrap();

        // One of the wallet's addresses (the faucet address) should have a nonzero balance of the
        // native asset, and at least one should have a 0 balance. Get one of each so we can
        // transfer from an account with non-zero balance to one with 0 balance. Note that in the
        // current setup, we can't easily transfer from one wallet to another, because each instance
        // of the server uses its own ledger. So we settle for an intra-wallet transfer.
        let mut unfunded_account = None;
        for address in info.addresses {
            if Balances::One(0u64.into())
                == server
                    .get::<BalanceInfo>(&format!(
                        "getbalance/address/{}/asset/{}",
                        address,
                        AssetCode::native()
                    ))
                    .await
                    .unwrap()
                    .balances
            {
                unfunded_account = Some(address);
                break;
            }
        }
        let src_address: UserAddress = receipt.submitters[0].clone().into();
        let dst_address = unfunded_account.unwrap();

        // Make a transfer with a given sender address.
        server
            .post::<TransactionReceipt<CapeLedger>>(&format!(
                "send/sender/{}/asset/{}/recipient/{}/amount/{}/fee/{}",
                src_address,
                &AssetCode::native(),
                dst_address,
                100,
                1
            ))
            .await
            .unwrap();

        // Wait for the balance to show up.
        retry(|| async {
            server
                .get::<BalanceInfo>(&format!(
                    "getbalance/address/{}/asset/{}",
                    dst_address,
                    AssetCode::native()
                ))
                .await
                .unwrap()
                .balances
                == Balances::One(100u64.into())
        })
        .await;

        // Check that the balance was deducted from the sending account.
        retry(|| async {
            server
                .get::<BalanceInfo>(&format!(
                    "getbalance/address/{}/asset/{}",
                    src_address,
                    AssetCode::native()
                ))
                .await
                .unwrap()
                .balances
                == Balances::One((DEFAULT_NATIVE_AMT_IN_FAUCET_ADDR - 101).into())
        })
        .await;

        // Make a transfer without a sender address.
        server
            .post::<TransactionReceipt<CapeLedger>>(&format!(
                "send/asset/{}/recipient/{}/amount/{}/fee/{}",
                &AssetCode::native(),
                dst_address,
                100,
                1
            ))
            .await
            .unwrap();

        // Check that the balance was added to the receiver address.
        retry(|| async {
            server
                .get::<BalanceInfo>(&format!(
                    "getbalance/address/{}/asset/{}",
                    dst_address,
                    AssetCode::native()
                ))
                .await
                .unwrap()
                .balances
                == Balances::One(200u64.into())
        })
        .await;

        // Check transaction history.
        let (history, asset_map) = server
            .get::<(Vec<TransactionHistoryEntry>, HashMap<AssetCode, AssetInfo>)>(
                "transactionhistory",
            )
            .await
            .unwrap();
        let info = server.get::<WalletSummary>("getinfo").await.unwrap();
        let native_info = info
            .assets
            .iter()
            .find(|asset| asset.definition == AssetDefinition::native())
            .unwrap();
        assert_eq!(asset_map[&AssetCode::native()], native_info.clone());

        // At this point everything should be accepted, even the received transactions.
        for h in &history {
            assert_eq!(h.status, "accepted");
            assert!(h.hash.is_some());
        }
        // We just made 2 transfers, there may be more from populatefortest.
        assert!(history.len() >= 2);
        let history = history[history.len() - 2..].to_vec();

        assert_eq!(history[0].kind, "send");
        assert_eq!(history[0].asset, AssetCode::native());
        assert_eq!(history[0].senders, vec![src_address]);
        assert_eq!(
            history[0].receivers,
            vec![(dst_address.clone(), 100u64.into())]
        );
        assert_eq!(history[0].status, "accepted");

        assert_eq!(history[1].kind, "send");
        assert_eq!(history[1].asset, AssetCode::native());
        // We don't necessarily know the senders for the second transaction, since we allowed the
        // wallet to choose.
        assert_eq!(history[1].receivers, vec![(dst_address, 100u64.into())]);
        assert_eq!(history[1].status, "accepted");

        // Check :from and :count.
        let (from_history1, _) = server
            .get::<(Vec<TransactionHistoryEntry>, HashMap<AssetCode, AssetInfo>)>(
                "transactionhistory/from/2",
            )
            .await
            .unwrap();
        assert_eq!(history, from_history1);
        let (from_history2, _) = server
            .get::<(Vec<TransactionHistoryEntry>, HashMap<AssetCode, AssetInfo>)>(
                "transactionhistory/from/2/count/1",
            )
            .await
            .unwrap();
        assert_eq!(&history[0..1], from_history2);
        // If we ask for more entries than there are, we should just get as many as are available.
        let (from_history3, _) = server
            .get::<(Vec<TransactionHistoryEntry>, HashMap<AssetCode, AssetInfo>)>(
                "transactionhistory/from/1/count/10",
            )
            .await
            .unwrap();
        assert_eq!(&history[1..], from_history3);
    }

    #[async_std::test]
    #[traced_test]
    async fn test_getaccount() {
        let server = TestServer::new().await;
        let mut rng = ChaChaRng::from_seed([1; 32]);

        // Should fail if a wallet is not already open.
        server
            .requires_wallet::<Account>(&format!(
                "getaccount/{}",
                UserKeyPair::generate(&mut rng).address(),
            ))
            .await;
        server
            .requires_wallet::<Account>(&format!(
                "getaccount/{}",
                UserKeyPair::generate(&mut rng).pub_key(),
            ))
            .await;
        server
            .requires_wallet::<Account>(&format!(
                "getaccount/{}",
                AuditorKeyPair::generate(&mut rng).pub_key(),
            ))
            .await;
        server
            .requires_wallet::<Account>(&format!(
                "getaccount/{}",
                FreezerKeyPair::generate(&mut rng).pub_key(),
            ))
            .await;

        // Now open a wallet.
        server
            .post::<()>(&format!(
                "newwallet/{}/{}/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                base64("my-password".as_bytes()),
                server.path()
            ))
            .await
            .unwrap();
        // Populate the wallet with some dummy data so we have a balance of an asset to send.
        server
            .get::<TransactionReceipt<CapeLedger>>("populatefortest")
            .await
            .unwrap();

        // Get the wrapped asset type.
        let info = server.get::<WalletSummary>("getinfo").await.unwrap();
        let asset = if info.assets[0].definition.code == AssetCode::native() {
            info.assets[1].clone()
        } else {
            info.assets[0].clone()
        };

        // The wrapper addressed it so we can check the account interface.
        let mut addresses = info.addresses.into_iter();
        let address = loop {
            let address = addresses.next().unwrap();
            println!(
                "{:?}",
                server
                    .get::<BalanceInfo>(&format!(
                        "getbalance/address/{}/asset/{}",
                        address,
                        AssetCode::native()
                    ))
                    .await
                    .unwrap()
            );
            if Balances::One(DEFAULT_NATIVE_AMT_IN_WRAPPER_ADDR.into())
                == server
                    .get::<BalanceInfo>(&format!(
                        "getbalance/address/{}/asset/{}",
                        address,
                        AssetCode::native()
                    ))
                    .await
                    .unwrap()
                    .balances
            {
                break address;
            }
        };

        let mut account = server
            .get::<Account>(&format!("getaccount/{}", address))
            .await
            .unwrap();
        assert_eq!(account.records.len(), 2);
        assert_eq!(account.assets.len(), 2);

        // We don't know what order the records will be reported in, but we know that the native
        // transfer gets committed before the wrap, so we can figure out the UIDs and sort. The
        // faucet record should have UID 0, and the change output from the native transfer should be
        // 1. So our native record should have UID 2 and our wrapped record should be 3.
        let expected_records = vec![
            Record {
                address: address.clone(),
                asset: AssetCode::native(),
                amount: DEFAULT_NATIVE_AMT_IN_WRAPPER_ADDR.into(),
                uid: 2,
            },
            Record {
                address,
                asset: asset.definition.code,
                amount: DEFAULT_WRAPPED_AMT.into(),
                uid: 3,
            },
        ];
        account.records.sort_by_key(|rec| rec.uid);
        assert_eq!(account.records, expected_records);

        assert_eq!(account.assets[&AssetCode::native()], AssetInfo::native());
        assert_eq!(account.assets[&asset.definition.code], asset);
    }

    #[async_std::test]
    #[traced_test]
    async fn test_getaccounts() {
        let server = TestServer::new().await;

        // Should fail if a wallet is not already open.
        server
            .requires_wallet::<Vec<Account>>("getaccounts/sending")
            .await;
        server
            .requires_wallet::<Vec<Account>>("getaccounts/viewing")
            .await;
        server
            .requires_wallet::<Vec<Account>>("getaccounts/freezing")
            .await;
        server
            .requires_wallet::<Vec<Account>>("getaccounts/all")
            .await;

        // Now open a wallet.
        server
            .post::<()>(&format!(
                "newwallet/{}/{}/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                base64("my-password".as_bytes()),
                server.path()
            ))
            .await
            .unwrap();
        // Generate some accounts.
        let sending1 = server
            .post::<PubKey>(&format!("newkey/sending"))
            .await
            .unwrap();
        let sending2 = server
            .post::<PubKey>(&format!("newkey/sending"))
            .await
            .unwrap();
        let viewing1 = server
            .post::<PubKey>(&format!("newkey/viewing"))
            .await
            .unwrap();
        let viewing2 = server
            .post::<PubKey>(&format!("newkey/viewing"))
            .await
            .unwrap();
        let freezing1 = server
            .post::<PubKey>(&format!("newkey/freezing"))
            .await
            .unwrap();
        let freezing2 = server
            .post::<PubKey>(&format!("newkey/freezing"))
            .await
            .unwrap();

        // Check that `getaccounts` returns the accounts with the correct keys. The validity of the
        // rest of the account data is tested in `test_getaccount`.
        async fn check(server: &TestServer, query: &str, expected: Vec<&PubKey>) {
            let accounts = server
                .get::<Vec<Account>>(&format!("getaccounts/{}", query))
                .await
                .unwrap();
            assert_eq!(accounts.len(), expected.len());
            // Check that we got the expected accounts, modulo order.
            assert_eq!(
                accounts
                    .into_iter()
                    .map(|account| account.pub_key)
                    .collect::<HashSet<_>>(),
                expected
                    .into_iter()
                    .map(|k| match k {
                        PubKey::Sending(k) => UserAddress(k.address()).to_string(),
                        _ => k.to_string(),
                    })
                    .collect::<HashSet<_>>()
            );
        }
        check(
            &server,
            "all",
            vec![
                &sending1, &sending2, &viewing1, &viewing2, &freezing1, &freezing2,
            ],
        )
        .await;
        check(&server, "sending", vec![&sending1, &sending2]).await;
        check(&server, "viewing", vec![&viewing1, &viewing2]).await;
        check(&server, "freezing", vec![&freezing1, &freezing2]).await;
    }

    #[async_std::test]
    #[traced_test]
    async fn test_recoverkey() {
        let server = TestServer::new().await;

        // Should fail if a wallet is not already open.
        server
            .requires_wallet_post::<PubKey>(&format!("recoverkey/sending"))
            .await;
        server
            .requires_wallet_post::<PubKey>(&format!("recoverkey/sending/0"))
            .await;
        server
            .requires_wallet_post::<PubKey>(&format!("recoverkey/viewing"))
            .await;
        server
            .requires_wallet_post::<PubKey>(&format!("recoverkey/freezing"))
            .await;

        // Create a wallet and generate some keys, 2 of each type.
        let mnemonic = server.get::<String>("getmnemonic").await.unwrap();
        server
            .post::<()>(&format!(
                "newwallet/{}/{}/path/{}",
                mnemonic,
                base64("my-password".as_bytes()),
                server.path()
            ))
            .await
            .unwrap();
        let mut keys = vec![];
        for ty in &["sending", "viewing", "freezing"] {
            for _ in 0..2 {
                keys.push(
                    server
                        .post::<PubKey>(&format!("newkey/{}", ty))
                        .await
                        .unwrap(),
                );
            }
        }

        // Close the wallet, create a new wallet with the same mnemonic, and recover the keys.
        let new_dir = TempDir::new("test_recover_key_path2").unwrap();
        server.post::<()>("closewallet").await.unwrap();
        server
            .post::<()>(&format!(
                "newwallet/{}/{}/path/{}",
                mnemonic,
                base64("my-password".as_bytes()),
                fmt_path(new_dir.path())
            ))
            .await
            .unwrap();
        let mut recovered_keys = vec![];
        for ty in &["sending", "viewing", "freezing"] {
            for _ in 0..2 {
                recovered_keys.push(
                    server
                        .post::<PubKey>(&format!("recoverkey/{}", ty))
                        .await
                        .unwrap(),
                );
            }
        }
        assert_eq!(recovered_keys, keys);

        // Test named keys.
        match server
            .post::<PubKey>(&format!(
                "recoverkey/sending/description/{}",
                base64::encode("sending".as_bytes())
            ))
            .await
            .unwrap()
        {
            PubKey::Sending(key) => {
                let account = server
                    .get::<Account>(&format!("getaccount/{}", key))
                    .await
                    .unwrap();
                assert_eq!(account.description, "sending");
            }
            key => panic!("Expected PubKey::Sending, found {:?}", key),
        }
        match server
            .post::<PubKey>(&format!(
                "recoverkey/viewing/description/{}",
                base64::encode("viewing".as_bytes())
            ))
            .await
            .unwrap()
        {
            PubKey::Viewing(key) => {
                let account = server
                    .get::<Account>(&format!("getaccount/{}", key))
                    .await
                    .unwrap();
                assert_eq!(account.description, "viewing");
            }
            key => panic!("Expected PubKey::Viewing, found {:?}", key),
        }
        match server
            .post::<PubKey>(&format!(
                "recoverkey/freezing/description/{}",
                base64::encode("freezing".as_bytes())
            ))
            .await
            .unwrap()
        {
            PubKey::Freezing(key) => {
                let account = server
                    .get::<Account>(&format!("getaccount/{}", key))
                    .await
                    .unwrap();
                assert_eq!(account.description, "freezing");
            }
            key => panic!("Expected PubKey::Freezing, found {:?}", key),
        }
    }

    #[async_std::test]
    #[traced_test]
    async fn test_listkeystores() {
        let server = TestServer::new().await;
        // Keystore names may include whatever characters the user wants.
        let keystore_name = "named/key \\store";

        // There are no keystore yet.
        assert_eq!(
            Vec::<String>::new(),
            server.get::<Vec<String>>("listkeystores").await.unwrap()
        );

        // Create a named key store.
        server
            .post::<()>(&format!(
                "newwallet/{}/{}/name/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                base64("my-password".as_bytes()),
                base64(keystore_name.as_bytes()),
            ))
            .await
            .unwrap();
        assert_eq!(
            vec![String::from(keystore_name)],
            server.get::<Vec<String>>("listkeystores").await.unwrap()
        );

        // Create a wallet in a different directory, and make sure it is not listed.
        let new_dir = TempDir::new("non_keystore_dir").unwrap();
        server
            .post::<()>(&format!(
                "newwallet/{}/{}/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                base64("my-password".as_bytes()),
                fmt_path(new_dir.path())
            ))
            .await
            .unwrap();

        let from_server_vec = server.get::<Vec<String>>("listkeystores").await.unwrap();
        assert_eq!(vec![String::from(keystore_name)], from_server_vec);
    }

    #[async_std::test]
    #[traced_test]
    async fn test_resetpassword() {
        let server = TestServer::new().await;
        let password1 = base64("password1".as_bytes());
        let password2 = base64("password2".as_bytes());
        let password3 = base64("password3".as_bytes());

        // Create a wallet with `password1`.
        let mnemonic = server.get::<String>("getmnemonic").await.unwrap();
        server
            .post::<()>(&format!(
                "newwallet/{}/{}/path/{}",
                mnemonic,
                password1,
                server.path(),
            ))
            .await
            .unwrap();

        // Create some data.
        let key = match server.post::<PubKey>("newkey/sending").await.unwrap() {
            PubKey::Sending(key) => key,
            key => panic!("expected PubKey::Sending, got {:?}", key),
        };
        assert_eq!(
            vec![key.clone()],
            server
                .get::<WalletSummary>("getinfo")
                .await
                .unwrap()
                .sending_keys
        );

        // Check that the wallet does not open with the wrong password.
        server
            .post::<()>(&format!("openwallet/{}/path/{}", password2, server.path()))
            .await
            .unwrap_err();

        // Change the password and check that our data is still there.
        server
            .post::<()>(&format!(
                "resetpassword/{}/{}/path/{}",
                mnemonic,
                password2,
                server.path()
            ))
            .await
            .unwrap();
        assert_eq!(
            vec![key],
            server
                .get::<WalletSummary>("getinfo")
                .await
                .unwrap()
                .sending_keys
        );

        // Check that we can't open the wallet with the old password.
        server
            .post::<()>(&format!("openwallet/{}/path/{}", password1, server.path()))
            .await
            .unwrap_err();

        // Check that we can open the wallet with the new password.
        server
            .post::<()>(&format!("openwallet/{}/path/{}", password2, server.path()))
            .await
            .unwrap();

        // Check that we can't reset the password using the wrong mnemonic.
        server
            .post::<()>(&format!(
                "resetpassword/{}/{}/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                password3,
                server.path()
            ))
            .await
            .unwrap_err();
    }

    #[async_std::test]
    #[traced_test]
    async fn test_verified_assets() {
        let server = TestServer::new().await;
        let mut rng = ChaChaRng::from_seed([1; 32]);

        let (code, _) = AssetCode::random(&mut rng);
        let new_asset = JfAssetDefinition::new(code, AssetPolicy::default()).unwrap();
        let assets = VerifiedAssetLibrary::new(
            vec![AssetInfo::native().into(), new_asset.clone().into()],
            &test_asset_signing_key(),
        );
        let path = server.options().assets_path();
        fs::write(&path, &bincode::serialize(&assets).unwrap())
            .await
            .unwrap();

        server
            .post::<()>(&format!(
                "newwallet/{}/{}/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                base64("my-password".as_bytes()),
                server.path(),
            ))
            .await
            .unwrap();

        let info = server.get::<WalletSummary>("getinfo").await.unwrap();
        let native_info = info
            .assets
            .iter()
            .find(|asset| asset.definition == AssetDefinition::native())
            .unwrap();
        assert!(native_info.verified);
        let asset_info = info
            .assets
            .iter()
            .find(|asset| asset.definition == AssetDefinition::from(new_asset.clone()))
            .unwrap();
        assert!(asset_info.verified);
    }

    #[async_std::test]
    #[traced_test]
    async fn test_export_import_asset() {
        let server = TestServer::new().await;
        server
            .post::<()>(&format!(
                "newwallet/{}/{}/name/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                base64("my-password".as_bytes()),
                base64("wallet1".as_bytes()),
            ))
            .await
            .unwrap();

        let mut asset = server
            .post::<AssetInfo>(&format!(
                "newasset/symbol/{}/description/{}",
                base64::encode_config("symbol".as_bytes(), base64::URL_SAFE_NO_PAD),
                base64::encode_config("description".as_bytes(), base64::URL_SAFE_NO_PAD)
            ))
            .await
            .unwrap();

        // Add an icon to the asset so we can check the serialization/deserializtion.
        let mut icon_bytes = Vec::new();
        test_icon().write_png(Cursor::new(&mut icon_bytes)).unwrap();
        asset = server
            .client
            .post(&format!("updateasset/{}", asset.definition.code))
            .body_json(&UpdateAsset {
                icon: Some(base64::encode(&icon_bytes)),
                ..Default::default()
            })
            .unwrap()
            .send()
            .await
            .unwrap()
            .body_json()
            .await
            .unwrap();

        assert_eq!(asset.symbol.as_ref().unwrap(), "symbol");
        assert_eq!(asset.description.as_ref().unwrap(), "description");
        assert_eq!(asset.icon.as_ref().unwrap(), &base64::encode(&icon_bytes));

        // We know the mint info since we created the asset. Later we will check that importers
        // can't learn the mint info.
        assert!(asset.mint_info.is_some());

        // Export the asset.
        let export = server
            .get::<String>(&format!("exportasset/{}", asset.definition.code))
            .await
            .unwrap();

        // Open a different wallet and import the asset.
        server
            .post::<()>(&format!(
                "newwallet/{}/{}/name/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                base64("my-password".as_bytes()),
                base64("wallet2".as_bytes()),
            ))
            .await
            .unwrap();
        // Make sure the new wallet doesn't have the asset before we import it.
        let info = server.get::<WalletSummary>("getinfo").await.unwrap();
        assert_eq!(info.assets, vec![AssetInfo::native()]);

        // Import the asset.
        let import: AssetInfo = server
            .client
            .post("importasset")
            .body_json(&export)
            .unwrap()
            .send()
            .await
            .unwrap()
            .body_json()
            .await
            .unwrap();
        // Make sure we didn't export the mint info.
        assert!(import.mint_info.is_none());
        // Check that all the information besides the mint info is the same.
        asset.mint_info = None;
        assert_eq!(asset, import);
    }

    #[async_std::test]
    #[traced_test]
    async fn test_updateasset() {
        let server = TestServer::new().await;
        let symbol = "symbol".to_owned();
        let description = "description".to_owned();

        let icon = test_icon();

        // Write the icon as a PNG and encode it in base64.
        let mut icon_cursor = Cursor::new(vec![]);
        icon.write_png(&mut icon_cursor).unwrap();
        let icon_bytes = icon_cursor.into_inner();
        let icon = base64::encode(&icon_bytes);

        // Should fail if a wallet is not already open.
        server
            .client
            .post(&format!("updateasset/{}", AssetCode::native()))
            .body_json(&UpdateAsset::default())
            .unwrap()
            .send()
            .await
            .unwrap_err();

        // Create a wallet.
        server
            .post::<()>(&format!(
                "newwallet/{}/{}/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                base64("my-password".as_bytes()),
                server.path()
            ))
            .await
            .unwrap();

        // Update the metadata of the native asset, one field at a time.
        let info: AssetInfo = server
            .client
            .post(&format!("updateasset/{}", AssetCode::native()))
            .body_json(&UpdateAsset {
                symbol: Some(symbol.clone()),
                ..Default::default()
            })
            .unwrap()
            .send()
            .await
            .unwrap()
            .body_json()
            .await
            .unwrap();
        assert_eq!(
            info,
            server
                .get::<WalletSummary>("getinfo")
                .await
                .unwrap()
                .assets
                .into_iter()
                .find(|asset| asset.definition.code == info.definition.code)
                .unwrap()
        );
        assert_eq!(info.symbol.unwrap(), "symbol");

        let info: AssetInfo = server
            .client
            .post(&format!("updateasset/{}", AssetCode::native()))
            .body_json(&UpdateAsset {
                description: Some(description.clone()),
                ..Default::default()
            })
            .unwrap()
            .send()
            .await
            .unwrap()
            .body_json()
            .await
            .unwrap();
        assert_eq!(
            info,
            server
                .get::<WalletSummary>("getinfo")
                .await
                .unwrap()
                .assets
                .into_iter()
                .find(|asset| asset.definition.code == info.definition.code)
                .unwrap()
        );
        assert_eq!(info.description.unwrap(), "description");

        let info: AssetInfo = server
            .client
            .post(&format!("updateasset/{}", AssetCode::native()))
            .body_json(&UpdateAsset {
                icon: Some(icon.clone()),
                ..Default::default()
            })
            .unwrap()
            .send()
            .await
            .unwrap()
            .body_json()
            .await
            .unwrap();
        assert_eq!(
            info,
            server
                .get::<WalletSummary>("getinfo")
                .await
                .unwrap()
                .assets
                .into_iter()
                .find(|asset| asset.definition.code == info.definition.code)
                .unwrap()
        );
        assert_eq!(info.icon.unwrap(), icon);
    }

    #[async_std::test]
    #[traced_test]
    async fn test_getprivatekey() {
        let server = TestServer::new().await;
        let mut rng = ChaChaRng::from_seed([1; 32]);

        // Should fail if a wallet is not already open.
        server
            .requires_wallet::<PrivateKey>(&format!(
                "getprivatekey/{}",
                UserAddress::from(UserKeyPair::generate(&mut rng).address()),
            ))
            .await;
        server
            .requires_wallet::<PrivateKey>(&format!(
                "getprivatekey/{}",
                UserKeyPair::generate(&mut rng).pub_key(),
            ))
            .await;
        server
            .requires_wallet::<PrivateKey>(&format!(
                "getprivatekey/{}",
                AuditorKeyPair::generate(&mut rng).pub_key(),
            ))
            .await;
        server
            .requires_wallet::<PrivateKey>(&format!(
                "getprivatekey/{}",
                FreezerKeyPair::generate(&mut rng).pub_key(),
            ))
            .await;

        // Now open a wallet.
        server
            .post::<()>(&format!(
                "newwallet/{}/{}/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                base64("my-password".as_bytes()),
                server.path()
            ))
            .await
            .unwrap();

        //Create keys
        let sending_key = match server.post::<PubKey>("newkey/sending").await.unwrap() {
            PubKey::Sending(key) => key,
            key => panic!("Expected PubKey::Sending, found {:?}", key),
        };
        let viewing_key = match server.post::<PubKey>("newkey/viewing").await.unwrap() {
            PubKey::Viewing(key) => key,
            key => panic!("Expected PubKey::Viewing, found {:?}", key),
        };
        let freezing_key = match server.post::<PubKey>("newkey/freezing").await.unwrap() {
            PubKey::Freezing(key) => key,
            key => panic!("Expected PubKey::Freezing, found {:?}", key),
        };

        // Get the private keys
        let sending_key_addr = server
            .get::<PrivateKey>(&format!(
                "getprivatekey/{}",
                UserAddress::from(sending_key.address()),
            ))
            .await
            .unwrap();
        let sending_key_pub = server
            .get::<PrivateKey>(&format!("getprivatekey/{}", sending_key,))
            .await
            .unwrap();
        let auditor_key = server
            .get::<PrivateKey>(&format!("getprivatekey/{}", viewing_key,))
            .await
            .unwrap();
        let freezer_key = server
            .get::<PrivateKey>(&format!("getprivatekey/{}", freezing_key,))
            .await
            .unwrap();
        server
            .get::<PrivateKey>(&format!("getprivatekey/{}", "invalid_address"))
            .await
            .expect_err("getprivatekey succeeded with invalid address");

        //check that keys are correct
        match sending_key_addr {
            PrivateKey::Sending(key) => {
                assert_eq!(key.pub_key(), sending_key);
            }
            _ => {
                panic!("Expected PrivateKey::Sending, found {:?}", sending_key_addr);
            }
        }
        match sending_key_pub {
            PrivateKey::Sending(key) => {
                assert_eq!(key.pub_key(), sending_key);
            }
            _ => {
                panic!("Expected PrivateKey::Sending, found {:?}", sending_key_pub);
            }
        }
        match auditor_key {
            PrivateKey::Viewing(key) => {
                assert_eq!(key.pub_key(), viewing_key);
            }
            _ => {
                panic!("Expected PrivateKey::Viewing, found {:?}", auditor_key);
            }
        }
        match freezer_key {
            PrivateKey::Freezing(key) => {
                assert_eq!(key.pub_key(), freezing_key);
            }
            _ => {
                panic!("Expected PrivateKey::Freezing, found {:?}", freezer_key);
            }
        }
    }

    #[async_std::test]
    #[traced_test]
    async fn test_recordopening() {
        let server = TestServer::new().await;
        let mut rng = ChaChaRng::from_seed([1; 32]);

        // Create a wallet.
        server
            .post::<()>(&format!(
                "newwallet/{}/{}/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                base64("my-password".as_bytes()),
                server.path(),
            ))
            .await
            .unwrap();

        // Create an address.
        server.post::<PubKey>("newkey/sending").await.unwrap();
        let info = server.get::<WalletSummary>("getinfo").await.unwrap();
        let sending_key = &info.sending_keys[0];
        let address: UserAddress = sending_key.address().into();

        // Define an asset.
        let description = base64::encode_config(&[3u8; 32], base64::URL_SAFE_NO_PAD);
        let asset = server
            .post::<AssetInfo>(&format!("newasset/description/{}", description))
            .await
            .unwrap()
            .definition
            .code;

        // recordopening should fail with an invalid address or asset.
        let invalid_addr = UserAddress::from(UserKeyPair::generate(&mut rng).address());
        let invalid_asset = AssetCode::dummy();
        server
            .post::<sol::RecordOpening>(&format!(
                "recordopening/address/{}/asset/{}/amount/{}/freeze/true",
                invalid_addr, asset, 10
            ))
            .await
            .expect_err("recordopening succeeded with invalid address");
        server
            .post::<sol::RecordOpening>(&format!(
                "recordopening/address/{}/asset/{}/amount/{}/freeze/true",
                address, invalid_asset, 10
            ))
            .await
            .expect_err("recordopening succeeded with invalid asset");

        // recordopening should create the record opening with the correct information.
        let ro = server
            .post::<sol::RecordOpening>(&format!(
                "recordopening/address/{}/asset/{}/amount/{}/freeze/true",
                address, asset, 10
            ))
            .await
            .unwrap();
        assert_eq!(ro.amount, 10u128.into());
        assert_eq!(ro.asset_def.code, asset.into());
        assert!(ro.freeze_flag);
        let ro = server
            .post::<sol::RecordOpening>(&format!(
                "recordopening/address/{}/asset/{}/amount/{}",
                address, asset, 10
            ))
            .await
            .unwrap();
        assert_eq!(ro.amount, 10u128.into());
        assert_eq!(ro.asset_def.code, asset.into());
        assert!(!ro.freeze_flag);
    }

    #[async_std::test]
    async fn test_large_balance() {
        // Set parameters for sponsor and wrap.
        let erc20_code = Address::from([1u8; 20]);
        let sponsor_addr = Address::from([2u8; 20]);

        // Open a wallet.
        let server = TestServer::new().await;
        server
            .post::<()>(&format!(
                "newwallet/{}/{}/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                base64("my-password".as_bytes()),
                server.path()
            ))
            .await
            .unwrap();
        server
            .get::<TransactionReceipt<CapeLedger>>("populatefortest")
            .await
            .unwrap();

        // Sponsor an asset.
        let (asset, info) = server
            .post::<(sol::AssetDefinition, String)>(&format!(
                "buildsponsor/erc20/{:#x}/sponsor/{:#x}",
                erc20_code, sponsor_addr
            ))
            .await
            .unwrap();
        server
            .client
            .post("importasset")
            .body_json(&info)
            .unwrap()
            .send()
            .await
            .unwrap();
        server
            .client
            .post(&format!(
                "submitsponsor/erc20/{:#x}/sponsor/{:#x}",
                erc20_code, sponsor_addr
            ))
            .body_json(&asset)
            .unwrap()
            .send()
            .await
            .unwrap();
        let asset: JfAssetDefinition = asset.into();

        // Create an address to receive the wrapped asset.
        server.post::<PubKey>("newkey/sending").await.unwrap();
        let info = server.get::<WalletSummary>("getinfo").await.unwrap();
        let sending_key = &info.sending_keys[0];
        let destination: UserAddress = sending_key.address().into();

        // Wrap the maximum single-record amount, thrice, so that our total balance exceeds both the
        // max single record amount and the max of a u64.
        let max_record = 2u64.pow(63) - 1;
        for _ in 0..3 {
            let ro = server
                .post::<sol::RecordOpening>(&format!(
                    "buildwrap/destination/{}/asset/{}/amount/{}",
                    destination, asset.code, max_record
                ))
                .await
                .unwrap();
            server
                .client
                .post(&format!("submitwrap/ethaddress/{:#x}", sponsor_addr))
                .body_json(&ro)
                .unwrap()
                .send()
                .await
                .unwrap();
        }

        // Submit a dummy transaction to finalize the wraps.
        server
            .post::<TransactionReceipt<CapeLedger>>(&format!(
                "send/asset/{}/recipient/{}/amount/1/fee/1",
                AssetCode::native(),
                destination,
            ))
            .await
            .unwrap();

        // Wait for the wraps to be finalized.
        retry(|| async {
            server
                .get::<BalanceInfo>(&format!(
                    "getbalance/address/{}/asset/{}",
                    destination, asset.code
                ))
                .await
                .unwrap()
                .balances
                != Balances::One(0u64.into())
        })
        .await;

        // Make sure the balances are correct.
        let expected_balance = U256::from_dec_str("27670116110564327421").unwrap();
        assert_eq!(
            server
                .get::<BalanceInfo>(&format!(
                    "getbalance/address/{}/asset/{}",
                    destination, asset.code
                ))
                .await
                .unwrap()
                .balances,
            Balances::One(expected_balance)
        );
        assert_eq!(
            server
                .get::<Account>(&format!("getaccount/{}", destination))
                .await
                .unwrap()
                .balances[&asset.code],
            expected_balance
        );
    }
}
