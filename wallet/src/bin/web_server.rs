// Copyright © 2021 Translucence Research, Inc. All rights reserved.

use cape_wallet::web::{default_api_path, default_web_path, init_server, NodeOpt};
use rand_chacha::{rand_core::SeedableRng, ChaChaRng};
use std::path::PathBuf;
use structopt::StructOpt;

#[async_std::main]
async fn main() -> Result<(), std::io::Error> {
    tracing_subscriber::fmt().pretty().init();

    // Initialize the web server.
    //
    // opt_web_path is the path to the web assets directory. If the path
    // is empty, the default is constructed assuming Cargo is used to
    // build the executable in the customary location.
    //
    // own_id is the identifier of this instance of the executable. The
    // port the web server listens on is 60000, unless the
    // PORT environment variable is set.

    // Take the command line option for the web asset directory path
    // provided it is not empty. Otherwise, construct the default from
    // the executable path.
    let opt_api_path = NodeOpt::from_args().api_path;
    let opt_web_path = NodeOpt::from_args().web_path;
    let web_path = if opt_web_path.is_empty() {
        default_web_path()
    } else {
        PathBuf::from(opt_web_path)
    };
    let api_path = if opt_api_path.is_empty() {
        default_api_path()
    } else {
        PathBuf::from(opt_api_path)
    };
    println!("Web path: {:?}", web_path);

    // TODO Use something different than the default Spectrum port (60000 vs 50000).
    let port = std::env::var("PORT").unwrap_or_else(|_| String::from("60000"));
    init_server(
        ChaChaRng::from_entropy(),
        api_path,
        web_path,
        port.parse().unwrap(),
    )?
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_std::task::sleep;
    use cap_rust_sandbox::{
        ledger::CapeLedger,
        model::{Erc20Code, EthereumAddr},
    };
    use cape_wallet::{
        routes::{BalanceInfo, CapeAPIError, PubKey, WalletSummary},
        testing::port,
        web::{
            DEFAULT_ETH_ADDR, DEFAULT_NATIVE_AMT_IN_FAUCET_ADDR,
            DEFAULT_NATIVE_AMT_IN_WRAPPER_ADDR, DEFAULT_WRAPPED_AMT,
        },
    };
    use futures::Future;
    use jf_cap::{
        keys::UserKeyPair,
        structs::{AssetCode, AssetDefinition},
    };
    use net::{client, UserAddress};
    use seahorse::{
        hd::KeyTree,
        txn_builder::{AssetInfo, TransactionReceipt},
    };
    use serde::de::DeserializeOwned;
    use std::collections::hash_map::HashMap;
    use std::convert::TryInto;
    use std::fmt::Debug;
    use std::iter::once;
    use std::time::Duration;
    use surf::Url;
    use tagged_base64::TaggedBase64;
    use tempdir::TempDir;
    use tracing_test::traced_test;

    async fn retry<Fut: Future<Output = bool>>(f: impl Fn() -> Fut) {
        let mut backoff = Duration::from_millis(100);
        for _ in 0..10 {
            if f().await {
                return;
            }
            sleep(backoff).await;
            backoff *= 2;
        }
        panic!("retry loop did not complete in {:?}", backoff);
    }

    struct TestServer {
        client: surf::Client,
        temp_dir: TempDir,
    }

    impl TestServer {
        async fn new() -> Self {
            let port = port().await;

            // Run a server in the background that is unique to this test. Note that the server task
            // is leaked: tide does not provide any mechanism for graceful programmatic shutdown, so
            // the server will continue running until the process is killed, even after the test
            // ends. This is ok, since each test's server task should be idle once
            // the test is over.
            init_server(
                ChaChaRng::from_seed([42; 32]),
                default_api_path(),
                default_web_path(),
                port,
            )
            .unwrap();
            Self::wait(port).await;

            let client: surf::Client = surf::Config::new()
                .set_base_url(Url::parse(&format!("http://localhost:{}", port)).unwrap())
                .set_timeout(None)
                .try_into()
                .unwrap();
            Self {
                client: client.with(client::parse_error_body::<CapeAPIError>),
                temp_dir: TempDir::new("test_cape_wallet").unwrap(),
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

        fn path(&self) -> TaggedBase64 {
            TaggedBase64::new(
                "PATH",
                self.temp_dir
                    .path()
                    .as_os_str()
                    .to_str()
                    .unwrap()
                    .as_bytes(),
            )
            .unwrap()
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
        KeyTree::from_mnemonic(mnemonic.as_str().as_bytes()).unwrap();

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
                spend_keys: vec![],
                audit_keys: vec![],
                freeze_keys: vec![],
                assets: vec![AssetInfo::from(AssetDefinition::native())]
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
        server.requires_wallet::<PubKey>("newkey/send").await;
        server.requires_wallet::<PubKey>("newkey/trace").await;
        server.requires_wallet::<PubKey>("newkey/freeze").await;

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
        let spend_key = server.get::<PubKey>("newkey/send").await.unwrap();
        let audit_key = server.get::<PubKey>("newkey/trace").await.unwrap();
        let freeze_key = server.get::<PubKey>("newkey/freeze").await.unwrap();
        let info = server.get::<WalletSummary>("getinfo").await.unwrap();
        match spend_key {
            PubKey::Spend(key) => {
                assert_eq!(info.spend_keys, vec![key]);
            }
            _ => {
                panic!("Expected PubKey::Spend, found {:?}", spend_key);
            }
        }
        match audit_key {
            PubKey::Audit(key) => {
                assert_eq!(info.audit_keys, vec![key]);
            }
            _ => {
                panic!("Expected PubKey::Audit, found {:?}", audit_key);
            }
        }
        match freeze_key {
            PubKey::Freeze(key) => {
                assert_eq!(info.freeze_keys, vec![key]);
            }
            _ => {
                panic!("Expected PubKey::Freeze, found {:?}", freeze_key);
            }
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
        let reveal_threshold = 10;
        let trace_amount = true;
        let trace_address = false;
        let description = TaggedBase64::new("DESC", &[3u8; 32]).unwrap();

        // Should fail if a wallet is not already open.
        server
            .requires_wallet::<AssetDefinition>(&format!(
                "newasset/erc20/{}/issuer/{}/traceamount/{}/traceaddress/{}/revealthreshold/{}",
                erc20_code, sponsor_addr, trace_amount, trace_address, reveal_threshold
            ))
            .await;
        server
            .requires_wallet::<AssetDefinition>(&format!(
                "newasset/description/{}/traceamount/{}/traceaddress/{}/revealthreshold/{}",
                description, trace_amount, trace_address, reveal_threshold
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
        server.get::<PubKey>("newkey/trace").await.unwrap();
        server.get::<PubKey>("newkey/freeze").await.unwrap();
        let info = server.get::<WalletSummary>("getinfo").await.unwrap();
        let audit_key = &info.audit_keys[0];
        let freeze_key = &info.freeze_keys[0];

        // newasset should return a sponsored asset with the correct policy if an ERC20 code is given.
        let sponsored_asset = server
            .get::<AssetDefinition>(&format!(
                "newasset/erc20/{}/issuer/{}/freezekey/{}/tracekey/{}/traceamount/{}/traceaddress/{}/revealthreshold/{}",
                erc20_code, sponsor_addr, freeze_key, audit_key, trace_amount, trace_address, reveal_threshold
            ))
            .await
            .unwrap();
        assert_eq!(sponsored_asset.policy_ref().auditor_pub_key(), audit_key);
        assert_eq!(sponsored_asset.policy_ref().freezer_pub_key(), freeze_key);
        assert_eq!(
            sponsored_asset.policy_ref().reveal_threshold(),
            reveal_threshold
        );

        // newasset should return a defined asset with the correct policy if no ERC20 code is given.
        let defined_asset = server
            .get::<AssetDefinition>(&format!(
                "newasset/description/{}/freezekey/{}/tracekey/{}/traceamount/{}/traceaddress/{}/revealthreshold/{}",
                description, freeze_key, audit_key, trace_amount, trace_address, reveal_threshold
            ))
            .await
            .unwrap();
        assert_eq!(defined_asset.policy_ref().auditor_pub_key(), audit_key);
        assert_eq!(defined_asset.policy_ref().freezer_pub_key(), freeze_key);
        assert_eq!(
            defined_asset.policy_ref().reveal_threshold(),
            reveal_threshold
        );
        let defined_asset = server
            .get::<AssetDefinition>(&format!(
            "newasset/freezekey/{}/tracekey/{}/traceamount/{}/traceaddress/{}/revealthreshold/{}",
            freeze_key, audit_key, trace_amount, trace_address, reveal_threshold
        ))
            .await
            .unwrap();
        assert_eq!(defined_asset.policy_ref().auditor_pub_key(), audit_key);
        assert_eq!(defined_asset.policy_ref().freezer_pub_key(), freeze_key);
        assert_eq!(
            defined_asset.policy_ref().reveal_threshold(),
            reveal_threshold
        );

        // newasset should return an asset with the default freezer public key if it's not given.
        let sponsored_asset = server
                .get::<AssetDefinition>(&format!(
                    "newasset/erc20/{}/issuer/{}/tracekey/{}/traceamount/{}/traceaddress/{}/revealthreshold/{}",
                    erc20_code, sponsor_addr, audit_key, trace_amount, trace_address, reveal_threshold
                ))
                .await
                .unwrap();
        assert!(!sponsored_asset.policy_ref().is_freezer_pub_key_set());
        let sponsored_asset = server
            .get::<AssetDefinition>(&format!(
                "newasset/description/{}/tracekey/{}/traceamount/{}/traceaddress/{}/revealthreshold/{}",
                description, audit_key, trace_amount, trace_address, reveal_threshold
            ))
            .await
            .unwrap();
        assert!(!sponsored_asset.policy_ref().is_freezer_pub_key_set());

        // newasset should return an asset with the default auditor public key and no reveal threshold if an
        // auditor public key isn't given.
        let sponsored_asset = server
            .get::<AssetDefinition>(&format!(
                "newasset/erc20/{}/issuer/{}/freezekey/{}",
                erc20_code, sponsor_addr, freeze_key
            ))
            .await
            .unwrap();
        assert!(!sponsored_asset.policy_ref().is_auditor_pub_key_set());
        assert_eq!(sponsored_asset.policy_ref().reveal_threshold(), 0);
        let sponsored_asset = server
            .get::<AssetDefinition>(&format!("newasset/description/{}", description))
            .await
            .unwrap();
        assert!(!sponsored_asset.policy_ref().is_auditor_pub_key_set());
        assert_eq!(sponsored_asset.policy_ref().reveal_threshold(), 0);

        // newasset should return an asset with no reveal threshold if it's not given.
        let sponsored_asset = server
                .get::<AssetDefinition>(&format!(
                    "newasset/erc20/{}/issuer/{}/freezekey/{}/tracekey/{}/traceamount/{}/traceaddress/{}",
                    erc20_code, sponsor_addr, freeze_key, audit_key, trace_amount, trace_address
                ))
                .await
                .unwrap();
        assert_eq!(sponsored_asset.policy_ref().reveal_threshold(), 0);
        let defined_asset = server
            .get::<AssetDefinition>(&format!(
                "newasset/description/{}/freezekey/{}/tracekey/{}/traceamount/{}/traceaddress/{}",
                description, freeze_key, audit_key, trace_amount, trace_address
            ))
            .await
            .unwrap();
        assert_eq!(defined_asset.policy_ref().reveal_threshold(), 0);
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
            .get::<AssetDefinition>(&format!(
                "newasset/erc20/{}/issuer/{}",
                erc20_code, sponsor_addr
            ))
            .await
            .unwrap();

        // Create an address to receive the wrapped asset.
        server.get::<PubKey>("newkey/send").await.unwrap();
        let info = server.get::<WalletSummary>("getinfo").await.unwrap();
        let spend_key = &info.spend_keys[0];
        let destination: UserAddress = spend_key.address().into();

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
                destination, sponsor_addr, sponsored_asset, 10
            ))
            .await
            .unwrap();
    }

    // Issue: https://github.com/EspressoSystems/cape/issues/600.
    #[async_std::test]
    #[traced_test]
    #[ignore]
    async fn test_mint() {
        // Set parameters.
        let description = TaggedBase64::new("DESC", &[3u8; 32]).unwrap();
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
        server.get::<()>("populatefortest").await.unwrap();

        // Define an asset.
        let asset = server
            .get::<AssetDefinition>(&format!("newasset/description/{}", description))
            .await
            .unwrap()
            .code;

        // Get the address with non-zero balance of the native asset.
        let info = server.get::<WalletSummary>("getinfo").await.unwrap();
        let mut minter_addr: Option<UserAddress> = None;
        for address in info.addresses {
            if let BalanceInfo::Balance(DEFAULT_NATIVE_AMT_IN_FAUCET_ADDR) = server
                .get::<BalanceInfo>(&format!(
                    "getbalance/address/{}/asset/{}",
                    address,
                    AssetCode::native()
                ))
                .await
                .unwrap()
            {
                minter_addr = Some(address);
                break;
            }
        }
        let minter = minter_addr.unwrap();

        // Get an address to receive the minted asset.
        let recipient: UserAddress = server
            .get::<WalletSummary>("getinfo")
            .await
            .unwrap()
            .spend_keys[0]
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

    // Issue: https://github.com/EspressoSystems/cape/issues/600.
    #[async_std::test]
    #[traced_test]
    #[ignore]
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
        server.get::<()>("populatefortest").await.unwrap();

        // Get the wrapped asset.
        let info = server.get::<WalletSummary>("getinfo").await.unwrap();
        let asset = if info.assets[0].asset.code == AssetCode::native() {
            info.assets[1].asset.code
        } else {
            info.assets[0].asset.code
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

    // Issue: https://github.com/EspressoSystems/cape/issues/600.
    #[async_std::test]
    #[traced_test]
    #[ignore]
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
        server.get::<()>("populatefortest").await.unwrap();

        let info = server.get::<WalletSummary>("getinfo").await.unwrap();
        assert_eq!(info.addresses.len(), 3);
        assert_eq!(info.spend_keys.len(), 3);
        assert_eq!(info.audit_keys.len(), 2);
        assert_eq!(info.freeze_keys.len(), 2);
        assert_eq!(info.assets.len(), 2); // native asset + wrapped asset

        // One of the addresses should have a non-zero balance of the native asset type.
        let mut found = false;
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
                found = true;
                break;
            }
        }
        assert!(found);

        let address = info.addresses[0].clone();
        // One of the wallet's two assets is the native asset, and the other is the wrapped asset
        // for which we have a nonzero balance, but the order depends on the hash of the wrapped
        // asset code, which is non-deterministic, so we check both.
        let wrapped_asset = if info.assets[0].asset.code == AssetCode::native() {
            info.assets[1].asset.code
        } else {
            info.assets[0].asset.code
        };
        assert_ne!(wrapped_asset, AssetCode::native());
        assert_eq!(
            server
                .get::<BalanceInfo>(&format!(
                    "getbalance/address/{}/asset/{}",
                    address, wrapped_asset
                ))
                .await
                .unwrap(),
            BalanceInfo::Balance(DEFAULT_WRAPPED_AMT)
        );
    }

    // Issue: https://github.com/EspressoSystems/cape/issues/600.
    #[async_std::test]
    #[traced_test]
    #[ignore]
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
        server.get::<()>("populatefortest").await.unwrap();
        let info = server.get::<WalletSummary>("getinfo").await.unwrap();
        // One of the wallet's addresses (the faucet address) should have a nonzero balance of the
        // native asset, and at least one should have a 0 balance. Get one of each so we can
        // transfer from an account with non-zero balance to one with 0 balance. Note that in the
        // current setup, we can't easily transfer from one wallet to another, because each instance
        // of the server uses its own ledger. So we settle for an intra-wallet transfer.
        let mut funded_account = None;
        let mut unfunded_account = None;
        for address in info.addresses {
            if let BalanceInfo::Balance(DEFAULT_NATIVE_AMT_IN_FAUCET_ADDR) = server
                .get::<BalanceInfo>(&format!(
                    "getbalance/address/{}/asset/{}",
                    address,
                    AssetCode::native()
                ))
                .await
                .unwrap()
            {
                funded_account = Some(address);
            } else {
                unfunded_account = Some(address);
            }
        }
        let src_address = funded_account.unwrap();
        let dst_address = unfunded_account.unwrap();

        // Make a transfer.
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
        assert_eq!(
            BalanceInfo::Balance(DEFAULT_NATIVE_AMT_IN_FAUCET_ADDR - 101),
            server
                .get::<BalanceInfo>(&format!(
                    "getbalance/address/{}/asset/{}",
                    src_address,
                    AssetCode::native()
                ))
                .await
                .unwrap()
        );
    }
}
