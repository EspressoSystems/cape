use crate::{mocks::MockCapeLedger, CapeWalletError};
use async_std::sync::{Arc, Mutex, MutexGuard};
use async_trait::async_trait;
use cap_rust_sandbox::{
    ledger::{CapeLedger, CapeNullifierSet, CapeTransition},
    types::CAPE,
    universal_param::UNIVERSAL_PARAM,
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
use jf_aap::{
    keys::{UserAddress, UserPubKey},
    structs::{Nullifier, ReceiverMemo},
    Signature,
};
use net::client::parse_error_body;
use rand_chacha::{rand_core::SeedableRng, ChaChaRng};
use seahorse::{
    events::{EventIndex, EventSource, LedgerEvent},
    hd,
    loader::WalletLoader,
    persistence::AtomicWalletStorage,
    WalletBackend, WalletState,
};
use serde::{de::DeserializeOwned, Serialize};
use std::convert::{TryFrom, TryInto};
use std::pin::Pin;
use surf::Url;

type EthMiddleware = SignerMiddleware<Provider<Http>, EthWallet<SigningKey>>;
type EthClient = Arc<EthMiddleware>;

async fn connect_to_eth(
    wallet_seed: Result<String, &mut ChaChaRng>,
) -> Result<EthClient, CapeWalletError> {
    let rpc_url = match std::env::var("RPC_URL") {
        Ok(url) => url,
        Err(_) => "http://localhost:8545".to_string(),
    };

    let provider =
        Provider::<Http>::try_from(rpc_url.clone()).expect("could not instantiate HTTP Provider");
    let chain_id = provider.get_chainid().await.unwrap().as_u64();

    // If mnemonic is set, try to use it to create a wallet, otherwise create a random wallet.
    let wallet = match wallet_seed {
        Ok(mnemonic) => MnemonicBuilder::<English>::default()
            .phrase(mnemonic.as_str())
            .build()
            .map_err(|err| CapeWalletError::Failed {
                msg: format!("failed to open ETH wallet: {}", err),
            })?,
        Err(rng) => LocalEthWallet::new(rng),
    }
    .with_chain_id(chain_id);

    Ok(Arc::new(SignerMiddleware::new(provider, wallet)))
}

pub struct CapeBackend<'a, Meta: Serialize + DeserializeOwned> {
    relayer: surf::Client,
    contract: CAPE<EthMiddleware>,
    storage: Arc<Mutex<AtomicWalletStorage<'a, CapeLedger, Meta>>>,
    key_stream: hd::KeyTree,
    mock_eqs: Arc<Mutex<MockCapeLedger<'a>>>,
}

impl<'a, Meta: Serialize + DeserializeOwned + Send> CapeBackend<'a, Meta> {
    pub async fn new(
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
        let client = connect_to_eth(eth_mnemonic.ok_or(&mut ChaChaRng::from_entropy())).await?;
        let contract = CAPE::new(contract_address, client);
        let storage = AtomicWalletStorage::new(loader, 1024)?;
        let key_stream = storage.key_stream();
        Ok(Self {
            relayer,
            contract,
            storage: Arc::new(Mutex::new(storage)),
            key_stream,
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

    async fn submit(&mut self, txn: CapeTransition) -> Result<(), CapeWalletError> {
        match &txn {
            CapeTransition::Transaction(txn) => {
                self.relayer
                    .post("submit")
                    .body_json(txn)
                    .map_err(|err| CapeWalletError::Failed {
                        msg: err.to_string(),
                    })?
                    .send()
                    .await
                    .map_err(|err| CapeWalletError::Failed {
                        msg: format!("relayer error: {}", err),
                    })?;
            }
            CapeTransition::Wrap { ro, erc20_code, .. } => {
                // Wraps don't go through the relayer, they go directly to the contract.
                // TODO wraps shouldn't go through here at all, they should be submitted by the
                // frontend using Metamask.
                self.contract
                    .deposit_erc_20((**ro).clone().into(), erc20_code.clone().into())
                    .send()
                    .await
                    .map_err(|err| CapeWalletError::Failed {
                        msg: format!("error building CAPE::depositErc20 transaction: {}", err),
                    })?
                    .await
                    .map_err(|err| CapeWalletError::Failed {
                        msg: format!("error submitting CAPE::depositErc20 transaction: {}", err),
                    })?;
            }
        }

        // The mock EQS is not connected to the real contract, so we have to update it by explicitly
        // passing it the submitted transaction.
        self.mock_eqs.lock().await.submit(txn)
    }

    ////////////////////////////////////////////////////////////////////////////////////////////////
    // The remaining backend methods are still mocked, pending completion of the EQS
    //

    async fn create(&mut self) -> Result<WalletState<'a, CapeLedger>, CapeWalletError> {
        let univ_param = &*UNIVERSAL_PARAM;
        let state = self
            .mock_eqs
            .lock()
            .await
            .network()
            .create_wallet(univ_param)?;
        self.storage().await.create(&state).await?;
        Ok(state)
    }

    async fn subscribe(&self, from: EventIndex, to: Option<EventIndex>) -> Self::EventStream {
        self.mock_eqs.lock().await.network().subscribe(from, to)
    }

    async fn get_public_key(&self, address: &UserAddress) -> Result<UserPubKey, CapeWalletError> {
        self.mock_eqs.lock().await.network().get_public_key(address)
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

    async fn register_user_key(&mut self, pub_key: &UserPubKey) -> Result<(), CapeWalletError> {
        self.mock_eqs
            .lock()
            .await
            .network()
            .register_user_key(pub_key)
    }

    async fn post_memos(
        &mut self,
        block_id: u64,
        txn_id: u64,
        memos: Vec<ReceiverMemo>,
        sig: Signature,
    ) -> Result<(), CapeWalletError> {
        self.mock_eqs
            .lock()
            .await
            .post_memos(block_id, txn_id, memos, sig)
    }
}
