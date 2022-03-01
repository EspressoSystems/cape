#![deny(warnings)]

use crate::{mocks::MockCapeLedger, CapeWalletBackend, CapeWalletError};
use address_book::{address_book_port, InsertPubKey};
use async_std::sync::{Arc, Mutex, MutexGuard};
use async_trait::async_trait;
use cap_rust_sandbox::{
    deploy::EthMiddleware,
    ledger::{CapeLedger, CapeNullifierSet, CapeTransition},
    model::{Erc20Code, EthereumAddr},
    types::CAPE,
};
use ethers::{
    core::k256::ecdsa::SigningKey,
    prelude::{
        coins_bip39::English, Address, Http, LocalWallet as LocalEthWallet, MnemonicBuilder,
        Provider, SignerMiddleware, Wallet as EthWallet,
    },
    providers::Middleware,
    signers::Signer,
};
use futures::Stream;
use jf_cap::{
    keys::{UserAddress, UserKeyPair, UserPubKey},
    proof::UniversalParam,
    structs::{AssetDefinition, Nullifier, ReceiverMemo, RecordOpening},
    Signature,
};
use net::client::parse_error_body;
use rand_chacha::{rand_core::SeedableRng, ChaChaRng};
use relayer::SubmitBody;
use seahorse::{
    events::{EventIndex, EventSource, LedgerEvent},
    hd,
    loader::WalletLoader,
    persistence::AtomicWalletStorage,
    txn_builder::TransactionUID,
    WalletBackend, WalletState,
};
use serde::{de::DeserializeOwned, Serialize};
use std::convert::{TryFrom, TryInto};
use std::pin::Pin;
use surf::Url;

fn get_provider() -> Provider<Http> {
    let rpc_url = match std::env::var("RPC_URL") {
        Ok(val) => val,
        Err(_) => "http://localhost:8545".to_string(),
    };
    Provider::<Http>::try_from(rpc_url).expect("could not instantiate HTTP Provider")
}

pub struct CapeBackend<'a, Meta: Serialize + DeserializeOwned> {
    universal_param: &'a UniversalParam,
    relayer: surf::Client,
    contract: CAPE<EthMiddleware>,
    storage: Arc<Mutex<AtomicWalletStorage<'a, CapeLedger, Meta>>>,
    key_stream: hd::KeyTree,
    eth_wallet: EthWallet<SigningKey>,
    mock_eqs: Arc<Mutex<MockCapeLedger<'a>>>,
}

impl<'a, Meta: Serialize + DeserializeOwned + Send> CapeBackend<'a, Meta> {
    pub async fn new(
        universal_param: &'a UniversalParam,
        relayer_url: Url,
        contract_address: Address,
        eth_mnemonic: Option<String>,
        mock_eqs: Arc<Mutex<MockCapeLedger<'a>>>,
        loader: &mut impl WalletLoader<CapeLedger, Meta = Meta>,
    ) -> Result<CapeBackend<'a, Meta>, CapeWalletError> {
        let relayer: surf::Client = surf::Config::default()
            .set_base_url(relayer_url)
            .try_into()
            .unwrap();
        let relayer = relayer.with(parse_error_body::<relayer::Error>);

        // Create an Ethereum wallet to talk to the CAPE contract.
        let provider = get_provider();
        let chain_id = provider.get_chainid().await.unwrap().as_u64();
        // If mnemonic is set, try to use it to create a wallet, otherwise create a random wallet.
        let eth_wallet = match eth_mnemonic {
            Some(mnemonic) => MnemonicBuilder::<English>::default()
                .phrase(mnemonic.as_str())
                .build()
                .map_err(|err| CapeWalletError::Failed {
                    msg: format!("failed to open ETH wallet: {}", err),
                })?,
            None => LocalEthWallet::new(&mut ChaChaRng::from_entropy()),
        }
        .with_chain_id(chain_id);
        let client = Arc::new(SignerMiddleware::new(provider, eth_wallet.clone()));
        let contract = CAPE::new(contract_address, client);

        let storage = AtomicWalletStorage::new(loader, 1024)?;
        let key_stream = storage.key_stream();

        Ok(Self {
            universal_param,
            relayer,
            contract,
            storage: Arc::new(Mutex::new(storage)),
            key_stream,
            eth_wallet,
            mock_eqs,
        })
    }
}

#[async_trait]
impl<'a, Meta: Serialize + DeserializeOwned + Send> WalletBackend<'a, CapeLedger>
    for CapeBackend<'a, Meta>
{
    type Storage = AtomicWalletStorage<'a, CapeLedger, Meta>;
    type EventStream = Pin<Box<dyn Stream<Item = (LedgerEvent<CapeLedger>, EventSource)> + Send>>;

    async fn storage<'l>(&'l mut self) -> MutexGuard<'l, Self::Storage> {
        self.storage.lock().await
    }

    fn key_stream(&self) -> hd::KeyTree {
        self.key_stream.clone()
    }

    async fn submit(
        &mut self,
        txn: CapeTransition,
        uid: TransactionUID<CapeLedger>,
        memos: Vec<ReceiverMemo>,
        sig: Signature,
    ) -> Result<(), CapeWalletError> {
        match &txn {
            CapeTransition::Transaction(txn) => {
                self.relayer
                    .post("submit")
                    .body_json(&SubmitBody {
                        transaction: txn.clone(),
                        memos: memos.clone(),
                        signature: sig.clone(),
                    })
                    .map_err(|err| CapeWalletError::Failed {
                        msg: err.to_string(),
                    })?
                    .send()
                    .await
                    .map_err(|err| CapeWalletError::Failed {
                        msg: format!("relayer error: {}", err),
                    })?;
            }
            CapeTransition::Wrap { .. } => {
                return Err(CapeWalletError::Failed {
                    msg: String::from(
                        "invalid transaction type: wraps must be submitted using `wrap()`, not \
                        `submit()`",
                    ),
                })
            }
        }

        // The mock EQS is not connected to the real contract, so we have to update it by explicitly
        // passing it the submitted transaction. This should not fail, since the submission to the
        // contract above succeeded, and the mock EQS is supposed to be tracking the contract state.
        let mut mock_eqs = self.mock_eqs.lock().await;
        mock_eqs.network().store_call_data(uid, memos, sig);
        mock_eqs.submit(txn).unwrap();

        Ok(())
    }

    ////////////////////////////////////////////////////////////////////////////////////////////////
    // The remaining backend methods are still mocked, pending completion of the EQS
    //

    async fn create(&mut self) -> Result<WalletState<'a, CapeLedger>, CapeWalletError> {
        let state = self
            .mock_eqs
            .lock()
            .await
            .network()
            .create_wallet(self.universal_param)?;
        self.storage().await.create(&state).await?;
        Ok(state)
    }

    async fn subscribe(&self, from: EventIndex, to: Option<EventIndex>) -> Self::EventStream {
        self.mock_eqs.lock().await.network().subscribe(from, to)
    }

    async fn get_public_key(&self, address: &UserAddress) -> Result<UserPubKey, CapeWalletError> {
        let address_bytes = bincode::serialize(address).unwrap();
        let mut response = surf::post(format!(
            "http://localhost:{}/request_pubkey",
            address_book_port()
        ))
        .content_type(surf::http::mime::BYTE_STREAM)
        .body_bytes(&address_bytes)
        .await
        .map_err(|err| CapeWalletError::Failed {
            msg: format!("error requesting public key: {}", err),
        })?;
        let bytes = response.body_bytes().await.unwrap();
        let pub_key: UserPubKey = bincode::deserialize(&bytes).unwrap();
        Ok(pub_key)
    }

    async fn get_nullifier_proof(
        &self,
        nullifiers: &mut CapeNullifierSet,
        nullifier: Nullifier,
    ) -> Result<(bool, ()), CapeWalletError> {
        // Try to look up the nullifier in our local cache. If it is not there, query the contract
        // and cache it.
        match nullifiers.get(nullifier) {
            Some(ret) => Ok((ret, ())),
            None => {
                let ret = self
                    .mock_eqs
                    .lock()
                    .await
                    .network()
                    .nullifier_spent(nullifier);
                nullifiers.insert(nullifier, ret);
                Ok((ret, ()))
            }
        }
    }

    async fn get_transaction(
        &self,
        block_id: u64,
        txn_id: u64,
    ) -> Result<CapeTransition, CapeWalletError> {
        self.mock_eqs
            .lock()
            .await
            .network()
            .get_transaction(block_id, txn_id)
    }

    async fn register_user_key(&mut self, key_pair: &UserKeyPair) -> Result<(), CapeWalletError> {
        let pub_key_bytes = bincode::serialize(&key_pair.pub_key()).unwrap();
        let sig = key_pair.sign(&pub_key_bytes);
        let json_request = InsertPubKey { pub_key_bytes, sig };
        match surf::post(format!(
            "http://localhost:{}/insert_pubkey",
            address_book_port()
        ))
        .content_type(surf::http::mime::JSON)
        .body_json(&json_request)
        .unwrap()
        .await
        {
            Ok(_) => Ok(()),
            Err(err) => Err(CapeWalletError::Failed {
                msg: format!("error inserting public key: {}", err),
            }),
        }
    }
}

#[async_trait]
impl<'a, Meta: Serialize + DeserializeOwned + Send> CapeWalletBackend<'a>
    for CapeBackend<'a, Meta>
{
    async fn register_erc20_asset(
        &mut self,
        asset: &AssetDefinition,
        erc20_code: Erc20Code,
        sponsor: EthereumAddr,
    ) -> Result<(), CapeWalletError> {
        self.contract
            .sponsor_cape_asset(erc20_code.clone().into(), asset.clone().into())
            .from(Address::from(sponsor.clone()))
            .send()
            .await
            .map_err(|err| CapeWalletError::Failed {
                msg: format!("error building CAPE::sponsorCapeAsset transaction: {}", err),
            })?
            .await
            .map_err(|err| CapeWalletError::Failed {
                msg: format!(
                    "error submitting CAPE::sponsorCapeAsset transaction: {}",
                    err
                ),
            })?;

        // Update the mock EQS to correspond with the new state of the contract. This cannot fail
        // since the update to the real contract succeeded.
        self.mock_eqs
            .lock()
            .await
            .network()
            .register_erc20(asset.clone(), erc20_code, sponsor)
            .unwrap();

        Ok(())
    }

    async fn get_wrapped_erc20_code(
        &self,
        asset: &AssetDefinition,
    ) -> Result<Erc20Code, CapeWalletError> {
        self.mock_eqs
            .lock()
            .await
            .network()
            .get_wrapped_asset(asset)
    }

    async fn wrap_erc20(
        &mut self,
        erc20_code: Erc20Code,
        src_addr: EthereumAddr,
        ro: RecordOpening,
    ) -> Result<(), CapeWalletError> {
        // Wraps don't go through the relayer, they go directly to the contract.
        self.contract
            .deposit_erc_20(ro.clone().into(), erc20_code.clone().into())
            .from(Address::from(src_addr.clone()))
            .send()
            .await
            .map_err(|err| CapeWalletError::Failed {
                msg: format!("error building CAPE::depositErc20 transaction: {}", err),
            })?
            .await
            .map_err(|err| CapeWalletError::Failed {
                msg: format!("error submitting CAPE::depositErc20 transaction: {}", err),
            })?;

        // The mock EQS is not connected to the real contract, so we have to update it by explicitly
        // passing it the submitted transaction. This should not fail, since the submission to the
        // contract above succeded, and the mock EQS is supposed to be tracking the contract state.
        let mut mock_eqs = self.mock_eqs.lock().await;
        mock_eqs
            .submit(CapeTransition::Wrap {
                ro: Box::new(ro),
                erc20_code,
                src_addr,
            })
            .unwrap();

        Ok(())
    }

    fn eth_client(&self) -> Result<Arc<EthMiddleware>, CapeWalletError> {
        Ok(Arc::new(SignerMiddleware::new(
            get_provider(),
            self.eth_wallet.clone(),
        )))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::testing::{
        create_test_network, sponsor_simple_token, transfer_token, wrap_simple_token,
    };
    use crate::{mocks::MockCapeWalletLoader, CapeWallet, CapeWalletExt};
    use cap_rust_sandbox::deploy::deploy_erc20_token;
    use ethers::types::{TransactionRequest, U256};
    use jf_cap::{structs::AssetCode, testing_apis::universal_setup_for_test};
    use rand_chacha::{rand_core::SeedableRng, ChaChaRng};
    use seahorse::testing::await_transaction;
    use std::path::PathBuf;
    use std::time::Duration;
    use tempdir::TempDir;

    #[async_std::test]
    async fn test_transfer() {
        let mut rng = ChaChaRng::from_seed([1u8; 32]);
        let universal_param = universal_setup_for_test(2usize.pow(16), &mut rng).unwrap();
        let (sender_key, relayer_url, contract_address, mock_eqs) =
            create_test_network(&mut rng, &universal_param).await;

        // Create a sender wallet and add the key pair that owns the faucet record.
        let sender_dir = TempDir::new("cape_wallet_backend_test").unwrap();
        let mut sender_loader = MockCapeWalletLoader {
            path: PathBuf::from(sender_dir.path()),
            key: hd::KeyTree::random(&mut rng).unwrap().0,
        };
        let sender_backend = CapeBackend::new(
            &universal_param,
            relayer_url.clone(),
            contract_address,
            None,
            mock_eqs.clone(),
            &mut sender_loader,
        )
        .await
        .unwrap();
        let mut sender = CapeWallet::new(sender_backend).await.unwrap();
        sender
            .add_user_key(sender_key.clone(), EventIndex::default())
            .await
            .unwrap();
        // Wait for the wallet to register the balance belonging to the key, from the initial grant
        // records. Depending on exactly when the key was added relative to the background event
        // handling thread, the relevant events may be processed by either the event handling thread
        // or the key scan thread, so we will wait for both of them to complete.
        sender.sync(mock_eqs.lock().await.now()).await.unwrap();
        sender.await_key_scan(&sender_key.address()).await.unwrap();
        let total_balance = sender
            .balance(&sender_key.address(), &AssetCode::native())
            .await;
        assert!(total_balance > 0);

        // Create an empty receiver wallet, and generating a receiving key.
        let receiver_dir = TempDir::new("cape_wallet_backend_test").unwrap();
        let mut receiver_loader = MockCapeWalletLoader {
            path: PathBuf::from(receiver_dir.path()),
            key: hd::KeyTree::random(&mut rng).unwrap().0,
        };
        let receiver_backend = CapeBackend::new(
            &universal_param,
            relayer_url.clone(),
            contract_address,
            None,
            mock_eqs.clone(),
            &mut receiver_loader,
        )
        .await
        .unwrap();
        let mut receiver = CapeWallet::new(receiver_backend).await.unwrap();
        let receiver_key = receiver.generate_user_key(None).await.unwrap();

        // Transfer from sender to receiver.
        transfer_token(
            &mut sender,
            receiver_key.address(),
            2,
            AssetCode::native(),
            1,
        )
        .await
        .unwrap();
        assert_eq!(
            sender
                .balance(&sender_key.address(), &AssetCode::native())
                .await,
            total_balance - 3
        );
        assert_eq!(
            receiver
                .balance(&receiver_key.address(), &AssetCode::native())
                .await,
            2
        );

        // Transfer back, just to make sure the receiver is actually able to spend the records it
        // received.
        transfer_token(
            &mut receiver,
            sender_key.address(),
            1,
            AssetCode::native(),
            1,
        )
        .await
        .unwrap();
        assert_eq!(
            sender
                .balance(&sender_key.address(), &AssetCode::native())
                .await,
            total_balance - 2
        );
        assert_eq!(
            receiver
                .balance(&receiver_key.address(), &AssetCode::native())
                .await,
            0
        );
    }

    #[async_std::test]
    async fn test_anonymous_erc20_transfer() {
        let mut rng = ChaChaRng::from_seed([1u8; 32]);
        let universal_param = universal_setup_for_test(2usize.pow(16), &mut rng).unwrap();
        let (wrapper_key, relayer_url, contract_address, mock_eqs) =
            create_test_network(&mut rng, &universal_param).await;

        // Create a wallet to sponsor an asset and a different wallet to deposit (we should be able
        // to deposit from an account other than the sponsor).
        let sponsor_dir = TempDir::new("cape_wallet_backend_test").unwrap();
        let mut sponsor_loader = MockCapeWalletLoader {
            path: PathBuf::from(sponsor_dir.path()),
            key: hd::KeyTree::random(&mut rng).unwrap().0,
        };
        let sponsor_backend = CapeBackend::new(
            &universal_param,
            relayer_url.clone(),
            contract_address.clone(),
            None,
            mock_eqs.clone(),
            &mut sponsor_loader,
        )
        .await
        .unwrap();
        let mut sponsor = CapeWallet::new(sponsor_backend).await.unwrap();
        let sponsor_key = sponsor.generate_user_key(None).await.unwrap();
        let sponsor_eth_addr = sponsor.eth_address().await.unwrap();

        let wrapper_dir = TempDir::new("cape_wallet_backend_test").unwrap();
        let mut wrapper_loader = MockCapeWalletLoader {
            path: PathBuf::from(wrapper_dir.path()),
            key: hd::KeyTree::random(&mut rng).unwrap().0,
        };
        let wrapper_backend = CapeBackend::new(
            &universal_param,
            relayer_url.clone(),
            contract_address.clone(),
            None,
            mock_eqs.clone(),
            &mut wrapper_loader,
        )
        .await
        .unwrap();
        let mut wrapper = CapeWallet::new(wrapper_backend).await.unwrap();
        // Add the faucet key to the wrapper wallet, so that they have the native tokens they need
        // to pay the fee to transfer the wrapped tokens.
        wrapper
            .add_user_key(wrapper_key.clone(), EventIndex::default())
            .await
            .unwrap();
        // Wait for the wrapper to register the balance belonging to the key, from the initial grant
        // records. Depending on exactly when the key was added relative to the background event
        // handling thread, the relevant events may be processed by either the event handling thread
        // or the key scan thread, so we will wait for both of them to complete.
        wrapper.sync(mock_eqs.lock().await.now()).await.unwrap();
        wrapper
            .await_key_scan(&wrapper_key.address())
            .await
            .unwrap();
        let total_native_balance = wrapper
            .balance(&wrapper_key.address(), &AssetCode::native())
            .await;
        assert!(total_native_balance > 0);

        // Fund the Ethereum wallets for contract calls.
        let provider = get_provider().interval(Duration::from_millis(100u64));
        let accounts = provider.get_accounts().await.unwrap();
        assert!(!accounts.is_empty());
        for wallet in [&sponsor, &wrapper] {
            let tx = TransactionRequest::new()
                .to(Address::from(wallet.eth_address().await.unwrap()))
                .value(ethers::utils::parse_ether(U256::from(1)).unwrap())
                .from(accounts[0]);
            provider
                .send_transaction(tx, None)
                .await
                .unwrap()
                .await
                .unwrap();
        }

        let erc20_contract = deploy_erc20_token().await;

        // Sponsor a CAPE asset corresponding to an ERC20 token.
        let cape_asset = sponsor_simple_token(&mut sponsor, &erc20_contract)
            .await
            .unwrap();

        wrap_simple_token(
            &mut wrapper,
            &wrapper_key.address(),
            cape_asset.clone(),
            &erc20_contract,
            contract_address.clone(),
            100,
        )
        .await
        .unwrap();

        // To force the wrap to be processed, we need to submit a block of CAPE transactions. We'll
        // transfer some native tokens from `wrapper` to `sponsor`.
        let receipt = wrapper
            .transfer(
                &wrapper_key.address(),
                &AssetCode::native(),
                &[(sponsor_key.address(), 1)],
                1,
            )
            .await
            .unwrap();
        await_transaction(&receipt, &wrapper, &[&sponsor]).await;
        assert_eq!(
            wrapper
                .balance(&wrapper_key.address(), &AssetCode::native())
                .await,
            total_native_balance - 2
        );
        assert_eq!(
            sponsor
                .balance(&sponsor_key.address(), &AssetCode::native())
                .await,
            1
        );
        // The transfer transaction caused the wrap record to be created.
        assert_eq!(
            wrapper
                .balance(&wrapper_key.address(), &cape_asset.code)
                .await,
            100
        );

        // Make sure the wrapper can access the wrapped tokens, by transferring them to someone else
        // (we'll reuse the `sponsor` wallet, but this could be a separate role).
        let receipt = wrapper
            .transfer(
                &wrapper_key.address(),
                &cape_asset.code,
                &[(sponsor_key.address(), 100)],
                1,
            )
            .await
            .unwrap();
        await_transaction(&receipt, &wrapper, &[&sponsor]).await;
        assert_eq!(
            wrapper
                .balance(&wrapper_key.address(), &cape_asset.code)
                .await,
            0
        );
        assert_eq!(
            sponsor
                .balance(&sponsor_key.address(), &cape_asset.code)
                .await,
            100
        );

        // Finally, withdraw the wrapped tokens back into the ERC20 token type.
        let receipt = sponsor
            .burn(
                &sponsor_key.address(),
                sponsor_eth_addr.clone().into(),
                &cape_asset.code,
                100,
                1,
            )
            .await
            .unwrap();
        await_transaction(&receipt, &sponsor, &[]).await;
        assert_eq!(
            sponsor
                .balance(&sponsor_key.address(), &cape_asset.code)
                .await,
            0
        );
        assert_eq!(
            erc20_contract
                .balance_of(sponsor_eth_addr.into())
                .call()
                .await
                .unwrap(),
            100.into()
        );
    }
}
