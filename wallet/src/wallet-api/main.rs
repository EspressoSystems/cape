// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.

// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

//! # The CAPE Wallet Server
//!
//! One of two main entrypoints to the wallet (the other being the CLI) this executable provides a
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

mod disco;
mod routes;
mod web;

use crate::web::{init_server, NodeOpt};
use rand_chacha::{rand_core::SeedableRng, ChaChaRng};
use structopt::StructOpt;

#[async_std::main]
async fn main() -> Result<(), std::io::Error> {
    tracing_subscriber::fmt().pretty().init();
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
    use cap_rust_sandbox::{
        ledger::CapeLedger,
        model::{Erc20Code, EthereumAddr},
    };
    use cape_wallet::{
        mocks::test_asset_signing_key,
        testing::{port, retry},
        ui::*,
    };
    use jf_cap::{
        keys::{AuditorKeyPair, FreezerKeyPair, UserKeyPair},
        structs::{AssetCode, AssetDefinition as JfAssetDefinition, AssetPolicy},
    };
    use net::{client, UserAddress};
    use seahorse::{
        asset_library::{Icon, VerifiedAssetLibrary},
        hd::{KeyTree, Mnemonic},
        txn_builder::{RecordInfo, TransactionReceipt},
    };
    use serde::de::DeserializeOwned;
    use std::collections::hash_map::HashMap;
    use std::collections::HashSet;
    use std::convert::TryInto;
    use std::fmt::Debug;
    use std::io::Cursor;
    use std::iter::once;
    use std::iter::FromIterator;
    use std::path::{Path, PathBuf};
    use surf::Url;
    use tempdir::TempDir;
    use tracing_test::traced_test;

    fn fmt_path(path: &Path) -> String {
        let bytes = path.as_os_str().to_str().unwrap().as_bytes();
        base64::encode_config(bytes, base64::URL_SAFE_NO_PAD)
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

        async fn requires_wallet<T: Debug + DeserializeOwned>(&self, path: &str) {
            self.get::<T>(path)
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

        async fn wait(port: u64) {
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
        let password = "my-password";

        // Should fail if the mnemonic is invalid.
        server
            .get::<()>(&format!(
                "newwallet/invalid-mnemonic/{}/path/{}",
                password,
                server.path()
            ))
            .await
            .expect_err("newwallet succeeded with an invalid mnemonic");

        // Should fail if the path is invalid.
        server
            .get::<()>(&format!(
                "newwallet/{}/{}/path/invalid-path",
                mnemonic, password
            ))
            .await
            .expect_err("newwallet succeeded with an invalid path");

        server
            .get::<()>(&format!(
                "newwallet/{}/{}/path/{}",
                mnemonic,
                password,
                server.path()
            ))
            .await
            .unwrap();

        // Should fail if the wallet already exists.
        server
            .get::<()>(&format!(
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
        println!("mnemonic: {}", mnemonic);
        let password = "my-password";

        // Should fail if no wallet exists.
        server
            .requires_wallet::<()>(&format!("openwallet/{}/path/{}", password, server.path()))
            .await;

        // Now create a wallet so we can open it.
        server
            .get::<()>(&format!(
                "newwallet/{}/{}/path/{}",
                mnemonic,
                password,
                server.path()
            ))
            .await
            .unwrap();
        server
            .get::<()>(&format!("openwallet/{}/path/{}", password, server.path()))
            .await
            .unwrap();

        // Should fail if the password is incorrect.
        server
            .get::<()>(&format!(
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
        let password = "my-password";

        // Should get None on first try if no last wallet.
        let opt = server
            .get::<Option<PathBuf>>("lastusedkeystore")
            .await
            .unwrap();
        assert!(opt.is_none());

        let url = format!("newwallet/{}/{}/path/{}", mnemonic, password, server.path());
        server.get::<()>(&url).await.unwrap();

        let mut path = server
            .get::<Option<PathBuf>>("lastusedkeystore")
            .await
            .unwrap();
        assert_eq!(fmt_path(path.as_ref().unwrap()), server.path());

        // We should still get the same path after opening the wallet
        server
            .get::<()>(&format!("openwallet/{}/path/{}", password, server.path()))
            .await
            .unwrap();
        path = server
            .get::<Option<PathBuf>>("lastusedkeystore")
            .await
            .unwrap();
        assert_eq!(fmt_path(path.as_ref().unwrap()), server.path());

        // Open the wallet with the we path we retrieved
        server
            .get::<()>(&format!(
                "openwallet/{}/path/{}",
                password,
                fmt_path(path.as_ref().unwrap())
            ))
            .await
            .unwrap();

        // Test that the last path is updated when we create a new wallet w/ a new path
        let second_path = fmt_path(TempDir::new("test_cape_wallet_2").unwrap().path());

        server
            .get::<()>(&format!(
                "newwallet/{}/{}/path/{}",
                mnemonic, password, second_path
            ))
            .await
            .unwrap();

        path = server
            .get::<Option<PathBuf>>("lastusedkeystore")
            .await
            .unwrap();
        assert_eq!(fmt_path(path.as_ref().unwrap()), second_path);

        // repopen the first wallet and see the path returned is also the original
        server
            .get::<()>(&format!("openwallet/{}/path/{}", password, server.path()))
            .await
            .unwrap();

        path = server
            .get::<Option<PathBuf>>("lastusedkeystore")
            .await
            .unwrap();
        assert_eq!(fmt_path(path.as_ref().unwrap()), server.path());
    }

    #[cfg(feature = "slow-tests")]
    #[async_std::test]
    #[traced_test]
    async fn test_closewallet() {
        let server = TestServer::new().await;

        // Should fail if a wallet is not already open.
        server.requires_wallet::<()>("closewallet").await;

        // Now open a wallet and close it.
        server
            .get::<()>(&format!(
                "newwallet/{}/my-password/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                server.path()
            ))
            .await
            .unwrap();
        server.get::<()>("closewallet").await.unwrap();
    }

    #[async_std::test]
    #[traced_test]
    async fn test_getinfo() {
        let server = TestServer::new().await;

        // Should fail if a wallet is not already open.
        server.requires_wallet::<WalletSummary>("getinfo").await;

        // Now open a wallet and call getinfo.
        server
            .get::<()>(&format!(
                "newwallet/{}/my-password/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                server.path()
            ))
            .await
            .unwrap();
        let info = server.get::<WalletSummary>("getinfo").await.unwrap();

        assert_eq!(
            info,
            WalletSummary {
                addresses: vec![],
                sending_keys: vec![],
                viewing_keys: vec![],
                freezing_keys: vec![],
                assets: vec![AssetInfo::native()]
            }
        )
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
            .get::<()>(&format!(
                "newwallet/{}/my-password/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
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
            .get::<()>(&format!(
                "newwallet/{}/my-password/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
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

        assert_eq!(ro1.amount, DEFAULT_NATIVE_AMT_IN_FAUCET_ADDR);
        assert_eq!(ro1.asset_def.code, AssetCode::native());
        assert_eq!(ro2.amount, DEFAULT_NATIVE_AMT_IN_WRAPPER_ADDR);
        assert_eq!(ro2.asset_def.code, AssetCode::native());
        assert_eq!(ro3.amount, DEFAULT_WRAPPED_AMT);
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
            .get::<()>(&format!(
                "newwallet/{}/my-password/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                server.path()
            ))
            .await
            .unwrap();

        // We can now hit the endpoints successfully, although there are currently no balances
        // because we haven't added any keys or received any records.
        assert_eq!(
            server.get::<BalanceInfo>("getbalance/all").await.unwrap(),
            BalanceInfo::AllBalances(HashMap::default())
        );
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
            BalanceInfo::AccountBalances(once((AssetCode::native(), 0)).collect())
        );
        assert_eq!(
            server
                .get::<BalanceInfo>(&format!("getbalance/address/{}/asset/{}", addr, asset))
                .await
                .unwrap(),
            BalanceInfo::Balance(0),
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
                .unwrap(),
            BalanceInfo::Balance(0),
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

    #[async_std::test]
    #[traced_test]
    async fn test_newkey() {
        let server = TestServer::new().await;

        // Should fail if a wallet is not already open.
        server.requires_wallet::<PubKey>("newkey/sending").await;
        server.requires_wallet::<PubKey>("newkey/tracing").await;
        server.requires_wallet::<PubKey>("newkey/freezing").await;

        // Now open a wallet.
        server
            .get::<()>(&format!(
                "newwallet/{}/my-password/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                server.path()
            ))
            .await
            .unwrap();

        // newkey should return a public key with the correct type and add the key to the wallet.
        let sending_key = server.get::<PubKey>("newkey/sending").await.unwrap();
        let viewing_key = server.get::<PubKey>("newkey/viewing").await.unwrap();
        let freezing_key = server.get::<PubKey>("newkey/freezing").await.unwrap();
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
            .get::<PubKey>(&format!(
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
            .get::<PubKey>(&format!(
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
            .get::<PubKey>(&format!(
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
            .get::<PubKey>("newkey/invalid_key_type")
            .await
            .expect_err("newkey succeeded with an invaild key type");
    }

    #[async_std::test]
    #[traced_test]
    async fn test_newasset() {
        let server = TestServer::new().await;

        // Set parameters for newasset.
        let erc20_code = Erc20Code(EthereumAddr([1u8; 20]));
        let sponsor_addr = EthereumAddr([2u8; 20]);
        let viewing_threshold = 10;
        let view_amount = true;
        let view_address = false;
        let description = base64::encode_config(&[3u8; 32], base64::URL_SAFE_NO_PAD);

        // Should fail if a wallet is not already open.
        server
            .requires_wallet::<AssetInfo>(&format!(
                "newasset/erc20/{}/sponsor/{}/view_amount/{}/view_address/{}/viewing_threshold/{}",
                erc20_code, sponsor_addr, view_amount, view_address, viewing_threshold
            ))
            .await;
        server
            .requires_wallet::<AssetInfo>(&format!(
                "newasset/description/{}/view_amount/{}/view_address/{}/viewing_threshold/{}",
                description, view_amount, view_address, viewing_threshold
            ))
            .await;

        // Now open a wallet.
        server
            .get::<()>(&format!(
                "newwallet/{}/my-password/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                server.path()
            ))
            .await
            .unwrap();

        // Create keys.
        server.get::<PubKey>("newkey/viewing").await.unwrap();
        server.get::<PubKey>("newkey/freezing").await.unwrap();
        let info = server.get::<WalletSummary>("getinfo").await.unwrap();
        let viewing_key = &info.viewing_keys[0];
        let freezing_key = &info.freezing_keys[0];

        // newasset should return a sponsored asset with the correct policy if an ERC20 code is given.
        let sponsored_asset = server
            .get::<AssetInfo>(&format!(
                "newasset/erc20/{}/sponsor/{}/freezing_key/{}/viewing_key/{}/view_amount/{}/view_address/{}/viewing_threshold/{}",
                erc20_code, sponsor_addr, freezing_key, viewing_key, view_amount, view_address, viewing_threshold
            ))
            .await
            .unwrap();
        assert_eq!(sponsored_asset.wrapped_erc20, Some(erc20_code.clone()));
        assert_eq!(
            &sponsored_asset.definition.viewing_key.unwrap(),
            viewing_key
        );
        assert_eq!(
            &sponsored_asset.definition.freezing_key.unwrap(),
            freezing_key
        );
        assert_eq!(
            sponsored_asset.definition.viewing_threshold,
            viewing_threshold
        );

        // newasset should return a defined asset with the correct policy if no ERC20 code is given.
        let defined_asset = server
            .get::<AssetInfo>(&format!(
                "newasset/description/{}/freezing_key/{}/viewing_key/{}/view_amount/{}/view_address/{}/viewing_threshold/{}",
                description, freezing_key, viewing_key, view_amount, view_address, viewing_threshold
            ))
            .await
            .unwrap();
        assert_eq!(defined_asset.wrapped_erc20, None);
        assert_eq!(&defined_asset.definition.viewing_key.unwrap(), viewing_key);
        assert_eq!(
            &defined_asset.definition.freezing_key.unwrap(),
            freezing_key
        );
        assert_eq!(
            defined_asset.definition.viewing_threshold,
            viewing_threshold
        );
        let defined_asset = server
            .get::<AssetInfo>(&format!(
            "newasset/freezing_key/{}/viewing_key/{}/view_amount/{}/view_address/{}/viewing_threshold/{}",
            freezing_key, viewing_key, view_amount, view_address, viewing_threshold
        ))
            .await
            .unwrap();
        assert_eq!(&defined_asset.definition.viewing_key.unwrap(), viewing_key);
        assert_eq!(
            &defined_asset.definition.freezing_key.unwrap(),
            freezing_key
        );
        assert_eq!(
            defined_asset.definition.viewing_threshold,
            viewing_threshold
        );

        // newasset should return an asset with the default freezer public key if it's not given.
        let erc20_code = Erc20Code(EthereumAddr([2; 20]));
        let sponsored_asset = server
                .get::<AssetInfo>(&format!(
                    "newasset/erc20/{}/sponsor/{}/viewing_key/{}/view_amount/{}/view_address/{}/viewing_threshold/{}",
                    erc20_code, sponsor_addr, viewing_key, view_amount, view_address, viewing_threshold
                ))
                .await
                .unwrap();
        assert!(sponsored_asset.definition.freezing_key.is_none());
        let sponsored_asset = server
            .get::<AssetInfo>(&format!(
                "newasset/description/{}/viewing_key/{}/view_amount/{}/view_address/{}/viewing_threshold/{}",
                description, viewing_key, view_amount, view_address, viewing_threshold
            ))
            .await
            .unwrap();
        assert!(sponsored_asset.definition.freezing_key.is_none());

        // newasset should return an asset with the default auditor public key and no reveal threshold if an
        // auditor public key isn't given.
        let erc20_code = Erc20Code(EthereumAddr([3; 20]));
        let sponsored_asset = server
            .get::<AssetInfo>(&format!(
                "newasset/erc20/{}/sponsor/{}/freezing_key/{}",
                erc20_code, sponsor_addr, freezing_key
            ))
            .await
            .unwrap();
        assert!(sponsored_asset.definition.viewing_key.is_none());
        assert_eq!(sponsored_asset.definition.viewing_threshold, 0);
        let sponsored_asset = server
            .get::<AssetInfo>(&format!("newasset/description/{}", description))
            .await
            .unwrap();
        assert!(sponsored_asset.definition.viewing_key.is_none());
        assert_eq!(sponsored_asset.definition.viewing_threshold, 0);

        // newasset should return an asset with no reveal threshold if it's not given.
        let erc20_code = Erc20Code(EthereumAddr([4; 20]));
        let sponsored_asset = server
                .get::<AssetInfo>(&format!(
                    "newasset/erc20/{}/sponsor/{}/freezing_key/{}/viewing_key/{}/view_amount/{}/view_address/{}",
                    erc20_code, sponsor_addr, freezing_key, viewing_key, view_amount, view_address
                ))
                .await
                .unwrap();
        assert_eq!(sponsored_asset.definition.viewing_threshold, 0);
        let defined_asset = server
            .get::<AssetInfo>(&format!(
                "newasset/description/{}/freezing_key/{}/viewing_key/{}/view_amount/{}/view_address/{}",
                description, freezing_key, viewing_key, view_amount, view_address
            ))
            .await
            .unwrap();
        assert_eq!(defined_asset.definition.viewing_threshold, 0);

        // newasset should return an asset with a given symbol.
        let erc20_code = Erc20Code(EthereumAddr([5; 20]));
        let sponsored_asset = server
                .get::<AssetInfo>(&format!(
                    "newasset/symbol/{}/erc20/{}/sponsor/{}/freezing_key/{}/viewing_key/{}/view_amount/{}/view_address/{}",
                    base64::encode_config("my-wrapped-asset", base64::URL_SAFE_NO_PAD), erc20_code,
                    sponsor_addr, freezing_key, viewing_key, view_amount, view_address
                ))
                .await
                .unwrap();
        assert_eq!(sponsored_asset.symbol, Some("my-wrapped-asset".into()));
        let defined_asset = server
            .get::<AssetInfo>(&format!(
                "newasset/symbol/{}/description/{}/freezing_key/{}/viewing_key/{}/view_amount/{}/view_address/{}",
               base64::encode_config("my-defined-asset", base64::URL_SAFE_NO_PAD), description,
               freezing_key, viewing_key, view_amount, view_address
            ))
            .await
            .unwrap();
        assert_eq!(defined_asset.symbol, Some("my-defined-asset".into()));
    }

    #[async_std::test]
    #[traced_test]
    async fn test_wrap() {
        // Set parameters for sponsor and wrap.
        let erc20_code = Erc20Code(EthereumAddr([1u8; 20]));
        let sponsor_addr = EthereumAddr([2u8; 20]);

        // Open a wallet.
        let server = TestServer::new().await;
        let mut rng = ChaChaRng::from_seed([42u8; 32]);
        server
            .get::<()>(&format!(
                "newwallet/{}/my-password/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                server.path()
            ))
            .await
            .unwrap();

        // Sponsor an asset.
        let sponsored_asset = server
            .get::<AssetInfo>(&format!(
                "newasset/erc20/{}/sponsor/{}",
                erc20_code, sponsor_addr
            ))
            .await
            .unwrap();

        // Create an address to receive the wrapped asset.
        server.get::<PubKey>("newkey/sending").await.unwrap();
        let info = server.get::<WalletSummary>("getinfo").await.unwrap();
        let sending_key = &info.sending_keys[0];
        let destination: UserAddress = sending_key.address().into();

        // wrap should fail if any of the destination, Ethereum address, and asset is invalid.
        let invalid_destination = UserAddress::from(UserKeyPair::generate(&mut rng).address());
        let invalid_eth_addr = Erc20Code(EthereumAddr([0u8; 20]));
        let invalid_asset = AssetDefinition::dummy();
        server
            .get::<()>(&format!(
                "wrap/destination/{}/ethaddress/{}/asset/{}/amount/{}",
                invalid_destination, sponsor_addr, sponsored_asset, 10
            ))
            .await
            .expect_err("wrap succeeded with an invalid user address");
        server
            .get::<()>(&format!(
                "wrap/destination/{}/ethaddress/{}/asset/{}/amount/{}",
                destination, invalid_eth_addr, sponsored_asset, 10
            ))
            .await
            .expect_err("wrap succeeded with an invalid Ethereum address");
        server
            .get::<()>(&format!(
                "wrap/destination/{}/ethaddress/{}/asset/{}/amount/{}",
                destination, sponsor_addr, invalid_asset, 10
            ))
            .await
            .expect_err("wrap succeeded with an invalid asset");

        // wrap should succeed with the correct information.
        server
            .get::<()>(&format!(
                "wrap/destination/{}/ethaddress/{}/asset/{}/amount/{}",
                destination, sponsor_addr, sponsored_asset.definition.code, 10
            ))
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
            .get::<()>(&format!(
                "newwallet/{}/minter-password/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
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
            .get::<AssetInfo>(&format!("newasset/description/{}", description))
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
            .get::<TransactionReceipt<CapeLedger>>(&format!(
                "mint/asset/{}/amount/{}/fee/{}/minter/{}/recipient/{}",
                invalid_asset, amount, fee, minter, recipient
            ))
            .await
            .expect_err("mint succeeded with an invalid asset");
        server
            .get::<TransactionReceipt<CapeLedger>>(&format!(
                "mint/asset/{}/amount/{}/fee/{}/minter/{}/recipient/{}",
                asset, amount, fee, invalid_minter, recipient
            ))
            .await
            .expect_err("mint succeeded with an invalid minter address");
        server
            .get::<TransactionReceipt<CapeLedger>>(&format!(
                "mint/asset/{}/amount/{}/fee/{}/minter/{}/recipient/{}",
                asset, amount, fee, minter, invalid_recipient
            ))
            .await
            .expect_err("mint succeeded with an invalid recipient address");

        // mint should succeed with the correct information.
        server
            .get::<TransactionReceipt<CapeLedger>>(&format!(
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
                == BalanceInfo::Balance(amount)
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
                == BalanceInfo::Balance(DEFAULT_NATIVE_AMT_IN_FAUCET_ADDR - fee)
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
            .get::<()>(&format!(
                "newwallet/{}/minter-password/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
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
            if let BalanceInfo::Balance(DEFAULT_WRAPPED_AMT) = server
                .get::<BalanceInfo>(&format!("getbalance/address/{}/asset/{}", address, asset))
                .await
                .unwrap()
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
        let invalid_eth_addr = Erc20Code(EthereumAddr([0u8; 20]));
        let invalid_asset = AssetDefinition::dummy();
        server
            .get::<TransactionReceipt<CapeLedger>>(&format!(
                "unwrap/source/{}/ethaddress/{}/asset/{}/amount/{}/fee/{}",
                invalid_source, eth_addr, asset, DEFAULT_WRAPPED_AMT, 1
            ))
            .await
            .expect_err("unwrap succeeded with an invalid source address");
        server
            .get::<TransactionReceipt<CapeLedger>>(&format!(
                "unwrap/source/{}/ethaddress/{}/asset/{}/amount/{}/fee/{}",
                source, invalid_eth_addr, asset, DEFAULT_WRAPPED_AMT, 1
            ))
            .await
            .expect_err("unwrap succeeded with an invalid Ethereum address");
        server
            .get::<TransactionReceipt<CapeLedger>>(&format!(
                "unwrap/source/{}/ethaddress/{}/asset/{}/amount/{}/fee/{}",
                source, eth_addr, invalid_asset, DEFAULT_WRAPPED_AMT, 1
            ))
            .await
            .expect_err("unwrap succeeded with an invalid asset");

        // unwrap should succeed with the correct information.
        server
            .get::<TransactionReceipt<CapeLedger>>(&format!(
                "unwrap/source/{}/ethaddress/{}/asset/{}/amount/{}/fee/{}",
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
                == BalanceInfo::Balance(0)
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
                == BalanceInfo::Balance(DEFAULT_NATIVE_AMT_IN_WRAPPER_ADDR - fee)
        })
        .await;
    }

    #[async_std::test]
    #[traced_test]
    async fn test_dummy_populate() {
        let server = TestServer::new().await;
        server
            .get::<()>(&format!(
                "newwallet/{}/my-password/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
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
            if let BalanceInfo::Balance(DEFAULT_NATIVE_AMT_IN_FAUCET_ADDR) = server
                .get::<BalanceInfo>(&format!(
                    "getbalance/address/{}/asset/{}",
                    address,
                    AssetCode::native()
                ))
                .await
                .unwrap()
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
            if let BalanceInfo::Balance(DEFAULT_WRAPPED_AMT) = server
                .get::<BalanceInfo>(&format!(
                    "getbalance/address/{}/asset/{}",
                    address, wrapped_asset
                ))
                .await
                .unwrap()
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
            .requires_wallet::<AssetDefinition>(&format!(
                "send/sender/{}/asset/{}/recipient/{}/amount/1/fee/1",
                UserKeyPair::generate(&mut rng).address(),
                AssetCode::random(&mut rng).0,
                EthereumAddr([1; 20]),
            ))
            .await;
        server
            .requires_wallet::<AssetDefinition>(&format!(
                "send/asset/{}/recipient/{}/amount/1/fee/1",
                AssetCode::random(&mut rng).0,
                EthereumAddr([1; 20]),
            ))
            .await;

        // Now open a wallet.
        server
            .get::<()>(&format!(
                "newwallet/{}/my-password/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
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
            if let BalanceInfo::Balance(0) = server
                .get::<BalanceInfo>(&format!(
                    "getbalance/address/{}/asset/{}",
                    address,
                    AssetCode::native()
                ))
                .await
                .unwrap()
            {
                unfunded_account = Some(address);
                break;
            }
        }
        let src_address: UserAddress = receipt.submitters[0].clone().into();
        let dst_address = unfunded_account.unwrap();

        // Make a transfer with a given sender address.
        server
            .get::<TransactionReceipt<CapeLedger>>(&format!(
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
                == BalanceInfo::Balance(100)
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
                == BalanceInfo::Balance(DEFAULT_NATIVE_AMT_IN_FAUCET_ADDR - 101)
        })
        .await;

        // Make a transfer without a sender address.
        server
            .get::<TransactionReceipt<CapeLedger>>(&format!(
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
                == BalanceInfo::Balance(200)
        })
        .await;

        // Check transaction history.
        let history = server
            .get::<Vec<TransactionHistoryEntry>>("transactionhistory")
            .await
            .unwrap();
        // We just made 2 transfers, there may be more from populatefortest.
        assert!(history.len() >= 2);
        let history = history[history.len() - 2..].to_vec();

        assert_eq!(history[0].kind, "send");
        assert_eq!(history[0].asset, AssetCode::native());
        assert_eq!(history[0].senders, vec![src_address]);
        assert_eq!(history[0].receivers, vec![(dst_address.clone(), 100)]);
        assert_eq!(history[0].status, "accepted");

        assert_eq!(history[1].kind, "send");
        assert_eq!(history[1].asset, AssetCode::native());
        // We don't necessarily know the senders for the second transaction, since we allowed the
        // wallet to choose.
        assert_eq!(history[1].receivers, vec![(dst_address, 100)]);
        assert_eq!(history[1].status, "accepted");

        // Check :from and :count.
        assert_eq!(
            history,
            server
                .get::<Vec<TransactionHistoryEntry>>("transactionhistory/from/2")
                .await
                .unwrap()
        );
        assert_eq!(
            &history[0..1],
            server
                .get::<Vec<TransactionHistoryEntry>>("transactionhistory/from/2/count/1")
                .await
                .unwrap()
        );
        // If we ask for more entries than there are, we should just get as many as are available.
        assert_eq!(
            &history[1..],
            server
                .get::<Vec<TransactionHistoryEntry>>("transactionhistory/from/1/count/10")
                .await
                .unwrap()
        );
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
            .get::<()>(&format!(
                "newwallet/{}/my-password/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
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

        // The wrapper addressœd it so we can check the account interface.
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
            if let BalanceInfo::Balance(DEFAULT_NATIVE_AMT_IN_WRAPPER_ADDR) = server
                .get::<BalanceInfo>(&format!(
                    "getbalance/address/{}/asset/{}",
                    address,
                    AssetCode::native()
                ))
                .await
                .unwrap()
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
                amount: DEFAULT_NATIVE_AMT_IN_WRAPPER_ADDR,
                uid: 2,
            },
            Record {
                address,
                asset: asset.definition.code,
                amount: DEFAULT_WRAPPED_AMT,
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
    async fn test_recoverkey() {
        let server = TestServer::new().await;

        // Should fail if a wallet is not already open.
        server
            .requires_wallet::<PubKey>(&format!("recoverkey/sending"))
            .await;
        server
            .requires_wallet::<PubKey>(&format!("recoverkey/sending/0"))
            .await;
        server
            .requires_wallet::<PubKey>(&format!("recoverkey/viewing"))
            .await;
        server
            .requires_wallet::<PubKey>(&format!("recoverkey/freezing"))
            .await;

        // Create a wallet and generate some keys, 2 of each type.
        let mnemonic = server.get::<String>("getmnemonic").await.unwrap();
        server
            .get::<()>(&format!(
                "newwallet/{}/my-password/path/{}",
                mnemonic,
                server.path()
            ))
            .await
            .unwrap();
        let mut keys = vec![];
        for ty in &["sending", "viewing", "freezing"] {
            for _ in 0..2 {
                keys.push(
                    server
                        .get::<PubKey>(&format!("newkey/{}", ty))
                        .await
                        .unwrap(),
                );
            }
        }

        // Close the wallet, create a new wallet with the same mnemonic, and recover the keys.
        let new_dir = TempDir::new("test_recover_key_path2").unwrap();
        server.get::<()>("closewallet").await.unwrap();
        server
            .get::<()>(&format!(
                "newwallet/{}/my-password/path/{}",
                mnemonic,
                fmt_path(new_dir.path())
            ))
            .await
            .unwrap();
        let mut recovered_keys = vec![];
        for ty in &["sending", "viewing", "freezing"] {
            for _ in 0..2 {
                recovered_keys.push(
                    server
                        .get::<PubKey>(&format!("recoverkey/{}", ty))
                        .await
                        .unwrap(),
                );
            }
        }
        assert_eq!(recovered_keys, keys);

        // Test named keys.
        match server
            .get::<PubKey>(&format!(
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
            .get::<PubKey>(&format!(
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
            .get::<PubKey>(&format!(
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

        // There are not keystores yet.
        assert_eq!(
            Vec::<String>::new(),
            server.get::<Vec<String>>("listkeystores").await.unwrap()
        );

        // Create a named key store.
        server
            .get::<()>(&format!(
                "newwallet/{}/my-password/name/named_keystore",
                server.get::<String>("getmnemonic").await.unwrap()
            ))
            .await
            .unwrap();
        assert_eq!(
            vec![String::from("named_keystore")],
            server.get::<Vec<String>>("listkeystores").await.unwrap()
        );

        // Create a key store by path, in the directory containing named keystores.
        server
            .get::<()>(&format!(
                "newwallet/{}/my-password/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                server.path()
            ))
            .await
            .unwrap();
        let from_server_vec = server.get::<Vec<String>>("listkeystores").await.unwrap();
        let expected: HashSet<String> =
            vec![String::from("named_keystore"), String::from("test_wallet")]
                .into_iter()
                .collect();

        assert_eq!(
            expected,
            HashSet::from_iter(from_server_vec.iter().cloned())
        );

        // Create a wallet in a different directory, and make sure it is not listed.
        let new_dir = TempDir::new("non_keystoer_dir").unwrap();
        server
            .get::<()>(&format!(
                "newwallet/{}/my-password/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                fmt_path(new_dir.path())
            ))
            .await
            .unwrap();

        let from_server_vec = server.get::<Vec<String>>("listkeystores").await.unwrap();
        assert_eq!(
            expected,
            HashSet::from_iter(from_server_vec.iter().cloned())
        );
    }

    #[async_std::test]
    #[traced_test]
    async fn test_resetpassword() {
        let server = TestServer::new().await;

        // Create a wallet with `password1`.
        let mnemonic = server.get::<String>("getmnemonic").await.unwrap();
        server
            .get::<()>(&format!(
                "newwallet/{}/password1/path/{}",
                mnemonic,
                server.path(),
            ))
            .await
            .unwrap();

        // Create some data.
        let key = match server.get::<PubKey>("newkey/sending").await.unwrap() {
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
            .get::<()>(&format!("openwallet/password2/path/{}", server.path()))
            .await
            .unwrap_err();

        // Change the password and check that our data is still there.
        server
            .get::<()>(&format!(
                "resetpassword/{}/password2/path/{}",
                mnemonic,
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
            .get::<()>(&format!("openwallet/password1/path/{}", server.path()))
            .await
            .unwrap_err();

        // Check that we can open the wallet with the new password.
        server
            .get::<()>(&format!("openwallet/password2/path/{}", server.path()))
            .await
            .unwrap();

        // Check that we can't reset the password using the wrong mnemonic.
        server
            .get::<()>(&format!(
                "resetpassword/{}/password3/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
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
            .get::<()>(&format!(
                "newwallet/{}/password1/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
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
            .get::<()>(&format!(
                "newwallet/{}/password1/name/wallet1",
                server.get::<String>("getmnemonic").await.unwrap(),
            ))
            .await
            .unwrap();

        let mut asset = server
            .get::<AssetInfo>(&format!(
                "newasset/description/{}",
                base64::encode_config("description".as_bytes(), base64::URL_SAFE_NO_PAD)
            ))
            .await
            .unwrap();
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
            .get::<()>(&format!(
                "newwallet/{}/password2/name/wallet2",
                server.get::<String>("getmnemonic").await.unwrap(),
            ))
            .await
            .unwrap();
        // Make sure the new wallet doesn't have the asset before we import it.
        let info = server.get::<WalletSummary>("getinfo").await.unwrap();
        assert_eq!(info.assets, vec![AssetInfo::native()]);

        // Import the asset.
        let import = server
            .get::<AssetInfo>(&format!("importasset/{}", export))
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
        let symbol = base64::encode_config("symbol".as_bytes(), base64::URL_SAFE_NO_PAD);
        let description = base64::encode_config("description".as_bytes(), base64::URL_SAFE_NO_PAD);

        // Generate a simple icon in raw bytes: 4 bytes for width, 4 for height, and then
        // width*height*3 zerox bytes for the pixels. Use 64x64 so seahorse doesn't resize the icon.
        let icon_width: u32 = 64;
        let icon_height: u32 = 64;
        let icon_data = [0; 3 * 64 * 64];
        let icon_bytes = icon_width
            .to_le_bytes()
            .iter()
            .chain(icon_height.to_le_bytes().iter())
            .chain(icon_data.iter())
            .cloned()
            .collect::<Vec<_>>();
        let icon = <Icon as CanonicalDeserialize>::deserialize(icon_bytes.as_slice()).unwrap();

        // Now write the icon as a PNG and encode it in base64.
        let mut icon_cursor = Cursor::new(vec![]);
        icon.write_png(&mut icon_cursor).unwrap();
        let icon_bytes = icon_cursor.into_inner();
        // We use URL_SAFE_NO_PAD for URL parameters
        let icon = base64::encode_config(&icon_bytes, base64::URL_SAFE_NO_PAD);
        // We use standard base 64 for responses, since that's what HTML image converts expect.
        let icon_response = base64::encode(&icon_bytes);

        // Should fail if a wallet is not already open.
        server
            .requires_wallet::<AssetInfo>(&format!(
                "updateasset/{}/symbol/{}",
                AssetCode::native(),
                symbol
            ))
            .await;
        server
            .requires_wallet::<AssetInfo>(&format!(
                "updateasset/{}/description/{}",
                AssetCode::native(),
                description
            ))
            .await;
        server
            .requires_wallet::<AssetInfo>(&format!(
                "updateasset/{}/icon/{}",
                AssetCode::native(),
                icon
            ))
            .await;

        // Create a wallet.
        server
            .get::<()>(&format!(
                "newwallet/{}/my-password/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                server.path()
            ))
            .await
            .unwrap();

        // Update the metadata of the native asset, one field at a time.
        let info = server
            .get::<AssetInfo>(&format!(
                "updateasset/{}/symbol/{}",
                AssetCode::native(),
                symbol
            ))
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

        let info = server
            .get::<AssetInfo>(&format!(
                "updateasset/{}/description/{}",
                AssetCode::native(),
                description
            ))
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

        let info = server
            .get::<AssetInfo>(&format!(
                "updateasset/{}/icon/{}",
                AssetCode::native(),
                icon
            ))
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
        assert_eq!(info.icon.unwrap(), icon_response);

        // Test route parsing for updating multiple fields at a time, although updating with the
        // same data will have no affect.
        server
            .get::<AssetInfo>(&format!(
                "updateasset/{}/symbol/{}/description/{}",
                AssetCode::native(),
                symbol,
                description
            ))
            .await
            .unwrap();
        server
            .get::<AssetInfo>(&format!(
                "updateasset/{}/symbol/{}/icon/{}",
                AssetCode::native(),
                symbol,
                icon
            ))
            .await
            .unwrap();
        server
            .get::<AssetInfo>(&format!(
                "updateasset/{}/description/{}/icon/{}",
                AssetCode::native(),
                description,
                icon
            ))
            .await
            .unwrap();
        server
            .get::<AssetInfo>(&format!(
                "updateasset/{}/symbol/{}/description/{}/icon/{}",
                AssetCode::native(),
                symbol,
                description,
                icon
            ))
            .await
            .unwrap();
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
            .get::<()>(&format!(
                "newwallet/{}/my-password/path/{}",
                server.get::<String>("getmnemonic").await.unwrap(),
                server.path()
            ))
            .await
            .unwrap();

        //Create keys
        let sending_key = match server.get::<PubKey>("newkey/sending").await.unwrap() {
            PubKey::Sending(key) => key,
            key => panic!("Expected PubKey::Sending, found {:?}", key),
        };
        let viewing_key = match server.get::<PubKey>("newkey/viewing").await.unwrap() {
            PubKey::Viewing(key) => key,
            key => panic!("Expected PubKey::Viewing, found {:?}", key),
        };
        let freezing_key = match server.get::<PubKey>("newkey/freezing").await.unwrap() {
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
}
