// Copyright (c) 2022 Espresso Systems (espressosys.com)
// This file is part of the Configurable Asset Privacy for Ethereum (CAPE) library.
//
// This program is free software: you can redistribute it and/or modify it under the terms of the GNU General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
// This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
// You should have received a copy of the GNU General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.

//! An implementation of [seahorse::WalletBackend] for CAPE.
#![deny(warnings)]

use crate::{loader::CapeMetadata, CapeWalletBackend, CapeWalletError};
use address_book::InsertPubKey;
use async_std::{
    sync::{Arc, Mutex, MutexGuard},
    task::sleep,
};
use async_trait::async_trait;
use cap_rust_sandbox::{
    deploy::EthMiddleware,
    ledger::{CapeLedger, CapeNullifierSet, CapeTransition, CapeTruster},
    model::{Erc20Code, EthereumAddr},
    types::{GenericInto, CAPE, ERC20},
    universal_param::{SUPPORTED_FREEZE_SIZES, SUPPORTED_TRANSFER_SIZES},
};
use eqs::{errors::EQSNetError, routes::CapState};
use ethers::{
    core::k256::ecdsa::SigningKey,
    prelude::{
        coins_bip39::English, Address, Http, LocalWallet as LocalEthWallet, MnemonicBuilder,
        Provider, SignerMiddleware, Wallet as EthWallet,
    },
    providers::Middleware,
    signers::Signer,
};
use futures::stream::{self, Stream, StreamExt};
use jf_cap::{
    keys::{UserAddress, UserKeyPair, UserPubKey},
    proof::UniversalParam,
    structs::{AssetCode, AssetDefinition, Nullifier, RecordOpening},
    MerkleTree, VerKey,
};
use key_set::ProverKeySet;
use net::client::{parse_error_body, response_body};
use rand_chacha::{rand_core::SeedableRng, ChaChaRng};
use reef::Ledger;
use relayer::SubmitBody;
use seahorse::{
    events::{EventIndex, EventSource, LedgerEvent},
    hd,
    loader::WalletLoader,
    persistence::AtomicWalletStorage,
    txn_builder::{RecordDatabase, TransactionInfo, TransactionState},
    WalletBackend, WalletState,
};
use serde::de::DeserializeOwned;
use std::cmp::min;
use std::convert::{TryFrom, TryInto};
use std::pin::Pin;
use std::time::{Duration, Instant};
use surf::{StatusCode, Url};

pub struct CapeBackendConfig {
    pub eqs_url: Url,
    pub relayer_url: Url,
    pub address_book_url: Url,
    /// JSON-RPC endpoint
    pub web3_provider: Option<Url>,
    pub eth_mnemonic: Option<String>,
    pub min_polling_delay: Duration,
}

struct EthRpc {
    url: Url,
    contract: CAPE<EthMiddleware>,
    wallet: EthWallet<SigningKey>,
}

impl EthRpc {
    fn provider(url: &Url) -> Provider<Http> {
        Provider::<Http>::try_from(url.to_string()).expect("could not instantiate HTTP Provider")
    }

    async fn new(
        url: Url,
        contract_address: Address,
        mnemonic: Option<String>,
    ) -> Result<Self, CapeWalletError> {
        // Create an Ethereum wallet to talk to the CAPE contract.
        let provider = Self::provider(&url);
        let chain_id = provider.get_chainid().await.unwrap().as_u64();
        // If mnemonic is set, try to use it to create a wallet, otherwise create a random wallet.
        let wallet = match mnemonic {
            Some(mnemonic) => MnemonicBuilder::<English>::default()
                .phrase(mnemonic.as_str())
                .build()
                .map_err(|err| CapeWalletError::Failed {
                    msg: format!("failed to open ETH wallet: {}", err),
                })?,
            None => LocalEthWallet::new(&mut ChaChaRng::from_entropy()),
        }
        .with_chain_id(chain_id);
        let client = Arc::new(SignerMiddleware::new(provider, wallet.clone()));
        let contract = CAPE::new(contract_address, client);

        Ok(Self {
            url,
            wallet,
            contract,
        })
    }

    fn client(&self) -> Arc<EthMiddleware> {
        Arc::new(SignerMiddleware::new(
            Self::provider(&self.url),
            self.wallet.clone(),
        ))
    }
}

pub struct CapeBackend<'a> {
    universal_param: &'a UniversalParam,
    eqs: surf::Client,
    relayer: surf::Client,
    address_book: surf::Client,
    storage: Arc<Mutex<AtomicWalletStorage<'a, CapeLedger, CapeMetadata>>>,
    key_stream: hd::KeyTree,
    min_polling_delay: Duration,
    eth: Option<EthRpc>,
}

impl<'a> CapeBackend<'a> {
    pub async fn new(
        universal_param: &'a UniversalParam,
        config: CapeBackendConfig,
        loader: &mut impl WalletLoader<CapeLedger, Meta = CapeMetadata>,
    ) -> Result<CapeBackend<'a>, CapeWalletError> {
        let eqs: surf::Client = surf::Config::default()
            .set_base_url(config.eqs_url)
            .try_into()
            .expect("Failed to configure EQS client");
        let eqs = eqs.with(parse_error_body::<EQSNetError>);
        let relayer: surf::Client = surf::Config::default()
            .set_base_url(config.relayer_url)
            .try_into()
            .expect("Failed to configure Relayer client");
        let relayer = relayer.with(parse_error_body::<relayer::Error>);
        let address_book: surf::Client = surf::Config::default()
            .set_base_url(config.address_book_url)
            .try_into()
            .expect("Failed to configure Address Book client");

        let storage = AtomicWalletStorage::new(loader, 1024)?;
        let key_stream = storage.key_stream();

        let eth = match config.web3_provider {
            Some(url) => Some(
                EthRpc::new(
                    url,
                    storage.meta().contract.clone().into(),
                    config.eth_mnemonic,
                )
                .await?,
            ),
            None => None,
        };

        Ok(Self {
            universal_param,
            eqs,
            relayer,
            address_book,
            storage: Arc::new(Mutex::new(storage)),
            key_stream,
            min_polling_delay: config.min_polling_delay,
            eth,
        })
    }
}

impl<'a> CapeBackend<'a> {
    async fn get_eqs<T: DeserializeOwned>(
        &self,
        route: impl AsRef<str>,
    ) -> Result<T, CapeWalletError> {
        let mut res =
            self.eqs
                .get(route.as_ref())
                .send()
                .await
                .map_err(|err| CapeWalletError::Failed {
                    msg: format!("eqs error: {}", err),
                })?;
        response_body::<T>(&mut res)
            .await
            .map_err(|err| CapeWalletError::Failed {
                msg: format!("error deserializing eqs response: {}", err),
            })
    }

    async fn wait_for_eqs(&self) -> Result<(), CapeWalletError> {
        let mut backoff = Duration::from_millis(500);
        for _ in 0..8 {
            // We use a direct `surf::connect` instead of `self.eqs.connect` because the client
            // middleware isn't set up to handle connect requests, only API requests.
            if surf::connect(
                &self
                    .eqs
                    .config()
                    .base_url
                    .as_ref()
                    .expect("eqs config has no base url"),
            )
            .send()
            .await
            .is_ok()
            {
                return Ok(());
            }
            tracing::warn!("unable to connect to EQS; sleeping for {:?}", backoff);
            sleep(backoff).await;
            backoff *= 2;
        }

        let msg = format!("failed to connect to EQS after {:?}", backoff);
        tracing::error!("{}", msg);
        Err(CapeWalletError::Failed { msg })
    }
}

#[async_trait]
impl<'a> WalletBackend<'a, CapeLedger> for CapeBackend<'a> {
    type Storage = AtomicWalletStorage<'a, CapeLedger, CapeMetadata>;
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
        info: TransactionInfo<CapeLedger>,
    ) -> Result<(), CapeWalletError> {
        match &txn {
            CapeTransition::Transaction(txn) => self
                .relayer
                .post("submit")
                .body_json(&SubmitBody {
                    transaction: txn.clone(),
                    memos: info.memos.clone().into_iter().flatten().collect(),
                    signature: info.sig.clone(),
                })
                .map_err(|err| CapeWalletError::Failed {
                    msg: err.to_string(),
                })?
                .send()
                .await
                .map_err(|err| CapeWalletError::Failed {
                    msg: format!("relayer error: {}", err),
                })
                // Ignore the response, which contains a hash of the submitted Ethereum transaction.
                // The EQS will track this transaction for us and send us an event if/when it gets
                // mined.
                .map(|_| ()),
            CapeTransition::Wrap { .. } => Err(CapeWalletError::Failed {
                msg: String::from(
                    "invalid transaction type: wraps must be submitted using `wrap()`, not \
                        `submit()`",
                ),
            }),
            CapeTransition::Faucet { .. } => Err(CapeWalletError::Failed {
                msg: String::from("submitting a faucet transaction from a wallet is not supported"),
            }),
        }
    }

    async fn create(&mut self) -> Result<WalletState<'a, CapeLedger>, CapeWalletError> {
        // Other network queries can fail, and they will simply cause the wallet operations that use
        // them to fail, without rendering the entire wallet unusable. But we need `get_cap_state`
        // to work in order to even open the wallet, so we will first make sure we can connect to
        // the EQS, retrying several times up to a couple of minutes before reporting an error.
        self.wait_for_eqs().await?;

        let state: CapState = self.get_eqs("get_cap_state").await?;

        // `records` should be _almost_ completely sparse. However, even a fully pruned Merkle tree
        // contains the last leaf appended, but as a new wallet, we don't care about _any_ of the
        // leaves, so make a note to forget the last one once more leaves have been appended.
        let record_mt = MerkleTree::restore_from_frontier(
            state.ledger.record_merkle_commitment,
            &state.ledger.record_merkle_frontier,
        )
        .ok_or_else(|| CapeWalletError::Failed {
            msg: String::from("cannot reconstruct Merkle tree from frontier"),
        })?;
        let merkle_leaf_to_forget = if record_mt.num_leaves() > 0 {
            Some(record_mt.num_leaves() - 1)
        } else {
            None
        };

        let state = WalletState {
            proving_keys: Arc::new(gen_proving_keys(self.universal_param)),
            txn_state: TransactionState {
                validator: CapeTruster::new(state.ledger.state_number, record_mt.num_leaves()),
                now: EventIndex::from_source(EventSource::QueryService, state.num_events as usize),
                // Completely sparse nullifier set
                nullifiers: Default::default(),
                record_mt,
                records: RecordDatabase::default(),
                merkle_leaf_to_forget,
                transactions: Default::default(),
            },
            key_state: Default::default(),
            assets: Default::default(),
            viewing_accounts: Default::default(),
            freezing_accounts: Default::default(),
            sending_accounts: Default::default(),
        };

        // Store the initial state.
        self.storage().await.create(&state).await?;

        Ok(state)
    }

    async fn subscribe(&self, from: EventIndex, to: Option<EventIndex>) -> Self::EventStream {
        // To avoid overloading the EQS with spurious network traffic, we will increase the backoff
        // time as long as we are not getting any new events, up to a maximum of 1 minute.
        let max_backoff = Duration::from_secs(60);

        struct StreamState {
            from: usize,
            to: Option<usize>,
            eqs: surf::Client,
            backoff: Duration,
            min_backoff: Duration,
            max_backoff: Duration,
        }
        let state = StreamState {
            from: from.index(EventSource::QueryService),
            to: to.map(|to| to.index(EventSource::QueryService)),
            eqs: self.eqs.clone(),
            backoff: self.min_polling_delay,
            min_backoff: self.min_polling_delay,
            max_backoff,
        };

        // Create a stream from a function which polls the EQS. The polling function itself returns
        // a stream of events, since in any given request we may receive more than one event, or
        // zero. Below, we will flatten this stream and tag each event with the event source (which
        // is always QueryService) as required by the WalletBackend API.
        Box::pin(
            stream::unfold(state, |mut state| async move {
                let req = if let Some(to) = state.to {
                    if state.from >= to {
                        // Returning `None` terminates the stream.
                        return None;
                    }
                    state.eqs.get(&format!(
                        "get_events_since/{}/{}",
                        state.from,
                        to - state.from
                    ))
                } else {
                    state.eqs.get(&format!("get_events_since/{}", state.from))
                };
                let mut res = if let Ok(res) = req.send().await {
                    res
                } else {
                    // Could not connect to EQS. Continue without updating state or yielding
                    // any events, and retry with a backoff.
                    sleep(state.backoff).await;
                    state.backoff = min(state.backoff * 2, state.max_backoff);
                    return Some((stream::iter(vec![]), state));
                };
                // Get events from the response. Panic if the response body does not
                // deserialize properly, as this should never happen as long as the EQS is
                // correct/honest.
                let events = response_body::<Vec<LedgerEvent<CapeLedger>>>(&mut res)
                    .await
                    .expect("Failed to deserialize EQS response");
                if events.is_empty() {
                    // If there were no new events, increase the backoff before retrying.
                    sleep(state.backoff).await;
                    state.backoff = min(state.backoff * 2, state.max_backoff);
                } else {
                    // If we succeeded in getting new events, reset the backoff.
                    state.backoff = state.min_backoff;
                    // Still sleep for the minimum duration, since we know there will not be
                    // new events at least until the EQS polls again.
                    sleep(state.backoff).await;
                }

                // Update state and yield the events we received.
                state.from += events.len();
                Some((stream::iter(events), state))
            })
            .flatten()
            .map(|event| (event, EventSource::QueryService)),
        )
    }

    async fn get_public_key(&self, address: &UserAddress) -> Result<UserPubKey, CapeWalletError> {
        let address_bytes = bincode::serialize(address).expect("Failed to serialize user address");
        let mut response = self
            .address_book
            .post("request_pubkey")
            .content_type(surf::http::mime::BYTE_STREAM)
            .body_bytes(&address_bytes)
            .await
            .map_err(|err| CapeWalletError::Failed {
                msg: format!("error requesting public key: {}", err),
            })?;
        if response.status() == StatusCode::Ok {
            let bytes = response
                .body_bytes()
                .await
                .expect("failed deserializing response from address book");
            let pub_key: UserPubKey = bincode::deserialize(&bytes)
                .expect("failed deserializing UserPubKey from address book.");
            Ok(pub_key)
        } else {
            Err(CapeWalletError::Failed {
                msg: "Error response from address book".into(),
            })
        }
    }

    async fn get_initial_scan_state(
        &self,
        _from: EventIndex,
    ) -> Result<(MerkleTree, EventIndex), CapeWalletError> {
        // We need to provide a Merkle frontier before the event `from`, but the EQS doesn't store
        // Merkle frontiers at each event index. To be safe, we provide the original frontier, which
        // is always empty.
        Ok((
            MerkleTree::new(CapeLedger::merkle_height()).expect("failed to create merkle tree"),
            EventIndex::default(),
        ))
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
                    .get_eqs(format!("check_nullifier/{}", nullifier))
                    .await?;
                // Inserting the nullifier in the local cache is probably not a good idea here.
                // Generally a nullifier is only useful once (when we want to spend it) so adding it
                // to the cache after we've already queried it once is likely to grow the size of
                // the cache without making its contents more useful.
                Ok((ret, ()))
            }
        }
    }

    async fn register_user_key(&mut self, key_pair: &UserKeyPair) -> Result<(), CapeWalletError> {
        let pub_key_bytes =
            bincode::serialize(&key_pair.pub_key()).expect("failed to serialize pbu key");
        let sig = key_pair.sign(&pub_key_bytes);
        let json_request = InsertPubKey { pub_key_bytes, sig };
        match self
            .address_book
            .post("insert_pubkey")
            .content_type(surf::http::mime::JSON)
            .body_json(&json_request)
            .expect("failed to read response from address book")
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
impl<'a> CapeWalletBackend<'a> for CapeBackend<'a> {
    async fn register_erc20_asset(
        &mut self,
        asset: &AssetDefinition,
        erc20_code: Erc20Code,
        sponsor: EthereumAddr,
    ) -> Result<(), CapeWalletError> {
        let contract = match &self.eth {
            Some(eth) => &eth.contract,
            None => {
                return Err(CapeWalletError::Failed {
                    msg: "cannot sponsor without JSON-RPC connection".into(),
                })
            }
        };
        contract
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
            })
            // Ignore the status code.
            .map(|_| ())?;

        // Don't report success until the EQS reflects the results of the sponsor.
        self.wait_for_wrapped_erc20_code(&asset.code, None).await?;

        Ok(())
    }

    async fn get_wrapped_erc20_code(
        &self,
        asset: &AssetCode,
    ) -> Result<Option<Erc20Code>, CapeWalletError> {
        let address: Option<Address> = self
            .get_eqs(format!("get_wrapped_erc20_address/{}", asset))
            .await?;
        Ok(address.map(Erc20Code::from))
    }

    async fn wait_for_wrapped_erc20_code(
        &mut self,
        asset: &AssetCode,
        timeout: Option<Duration>,
    ) -> Result<(), CapeWalletError> {
        let mut backoff = self.min_polling_delay;
        let now = Instant::now();
        loop {
            let address: Option<Address> = self
                .get_eqs(format!("get_wrapped_erc20_address/{}", asset))
                .await?;
            if address.is_some() {
                break;
            }
            if let Some(time) = timeout {
                if now.elapsed() >= time {
                    return Err(CapeWalletError::Failed {
                        msg: format!("asset not reflected in the EQS in {:?}", time),
                    });
                }
            }
            sleep(backoff).await;
            backoff = min(backoff * 2, Duration::from_secs(60));
        }
        Ok(())
    }

    async fn wrap_erc20(
        &mut self,
        erc20_code: Erc20Code,
        src_addr: EthereumAddr,
        ro: RecordOpening,
    ) -> Result<(), CapeWalletError> {
        let eth = match &self.eth {
            Some(eth) => eth,
            None => {
                return Err(CapeWalletError::Failed {
                    msg: "cannot wrap without JSON-RPC connection".into(),
                })
            }
        };

        // Before the contract can transfer from our account, in accordance with the ERC20 protocol,
        // we have to approve the transfer.
        ERC20::new(erc20_code.clone(), eth.client())
            .approve(
                eth.contract.address(),
                ro.amount.generic_into::<u128>().into(),
            )
            .send()
            .await
            .map_err(|err| CapeWalletError::Failed {
                msg: format!("error building ERC20::approve transaction: {}", err),
            })?
            .await
            .map_err(|err| CapeWalletError::Failed {
                msg: format!("error submitting ERC20::approve transaction: {}", err),
            })?;

        // Wraps don't go through the relayer, they go directly to the contract.
        eth.contract
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
            })
            // Ignore the status code
            .map(|_| ())
    }

    fn eth_client(&self) -> Result<Arc<EthMiddleware>, CapeWalletError> {
        if let Some(eth) = &self.eth {
            Ok(eth.client())
        } else {
            Err(CapeWalletError::Failed {
                msg: "eth_client is unsupported without JSON-RPC connection".into(),
            })
        }
    }

    fn asset_verifier(&self) -> VerKey {
        // Read the verification key from the environment if it is set, otherwise default to the
        // official verification key for the CAPE Goerli deployment.
        //
        // Reading the key from the environment allows us to set a different key for testing. While
        // this does allow user to set a different verification key and display unofficial assets as
        // "verified" in their UI, it does not violate the core property of the signed official
        // asset library, which is that _in the default configuration_, the UI will not display a
        // maliciously or fraudulently crafted asset library as official. It does not protect users
        // from themselves; after all, we can't stop a user from forking the wallet code and
        // commenting out the signature check altogether.
        std::env::var("CAPE_WALLET_ASSET_LIBRARY_VERIFIER_KEY")
            .unwrap_or_else(|_| "SCHNORRVERKEY~b7yvQPPxjPlZ5gjKofFkf8T7CwAZ2xPnkkVRhE48D4jz".into())
            .parse()
            .expect("failed to parse verification key")
    }

    async fn eqs_time(&self) -> Result<EventIndex, CapeWalletError> {
        let state: CapState = self.get_eqs("get_cap_state").await?;
        Ok(EventIndex::from_source(
            EventSource::QueryService,
            state.num_events as usize,
        ))
    }

    async fn wait_for_eqs(&self) -> Result<(), CapeWalletError> {
        self.wait_for_eqs().await
    }

    async fn contract_address(&self) -> Result<Erc20Code, CapeWalletError> {
        Ok(self.storage.lock().await.meta().contract.clone())
    }

    async fn latest_contract_address(&self) -> Result<Erc20Code, CapeWalletError> {
        let address: Address = self.get_eqs("get_cape_contract_address").await?;
        Ok(address.into())
    }
}

pub fn gen_proving_keys(srs: &UniversalParam) -> ProverKeySet<key_set::OrderByOutputs> {
    use jf_cap::proof::{freeze, mint, transfer};

    ProverKeySet {
        mint: mint::preprocess(srs, CapeLedger::merkle_height())
            .expect("failed preprocess of mint circuit")
            .0,
        xfr: SUPPORTED_TRANSFER_SIZES
            .iter()
            .map(|&(inputs, outputs)| {
                transfer::preprocess(srs, inputs, outputs, CapeLedger::merkle_height())
                    .expect("failed preprocess of transfer circuit")
                    .0
            })
            .collect(),
        freeze: SUPPORTED_FREEZE_SIZES
            .iter()
            .map(|&inputs| {
                freeze::preprocess(srs, inputs, CapeLedger::merkle_height())
                    .expect("failed preprocess of freeze circuit")
                    .0
            })
            .collect(),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        loader::CapeLoader,
        testing::{
            create_test_network, retry, rpc_url_for_test, spawn_eqs, sponsor_simple_token,
            transfer_token, wrap_simple_token,
        },
        ui::AssetInfo,
    };
    use crate::{CapeWallet, CapeWalletExt};
    use cap_rust_sandbox::{deploy::deploy_erc20_token, universal_param::UNIVERSAL_PARAM};
    use ethers::types::{TransactionRequest, U256};
    use jf_cap::structs::AssetCode;
    use rand_chacha::{rand_core::SeedableRng, ChaChaRng};
    use seahorse::testing::await_transaction;
    use std::str::FromStr;
    use std::time::Duration;
    use tempdir::TempDir;

    #[async_std::test]
    async fn test_transfer() {
        let mut rng = ChaChaRng::from_seed([1u8; 32]);
        let universal_param = &*UNIVERSAL_PARAM;
        let (sender_key, relayer_url, address_book_url, contract_address, _) =
            create_test_network(&mut rng, universal_param, None).await;
        let (eqs_url, _eqs_dir, _join_eqs) = spawn_eqs(contract_address).await;

        // Create a sender wallet and add the key pair that owns the faucet record.
        let sender_dir = TempDir::new("cape_wallet_backend_test").unwrap();
        let mut sender_loader = CapeLoader::from_literal(
            Some(hd::KeyTree::random(&mut rng).1.into_phrase()),
            "password".into(),
            sender_dir.path().to_owned(),
            contract_address.into(),
        );
        let sender_backend = CapeBackend::new(
            universal_param,
            CapeBackendConfig {
                web3_provider: None,
                eqs_url: eqs_url.clone(),
                relayer_url: relayer_url.clone(),
                address_book_url: address_book_url.clone(),
                eth_mnemonic: None,
                min_polling_delay: Duration::from_millis(500),
            },
            &mut sender_loader,
        )
        .await
        .unwrap();
        let mut sender = CapeWallet::new(sender_backend).await.unwrap();
        sender
            .add_user_key(sender_key.clone(), "sender".into(), EventIndex::default())
            .await
            .unwrap();

        // Wait for the wallet to register the balance belonging to the key, from the initial grant
        // records.
        retry(|| async {
            sender
                .balance_breakdown(&sender_key.address(), &AssetCode::native())
                .await
                > 0u64.into()
        })
        .await;
        let total_balance = sender
            .balance_breakdown(&sender_key.address(), &AssetCode::native())
            .await;

        // Create an empty receiver wallet, and generating a receiving key.
        let receiver_dir = TempDir::new("cape_wallet_backend_test").unwrap();
        let mut receiver_loader = CapeLoader::from_literal(
            Some(hd::KeyTree::random(&mut rng).1.into_phrase()),
            "password".into(),
            receiver_dir.path().to_owned(),
            contract_address.into(),
        );
        let receiver_backend = CapeBackend::new(
            universal_param,
            CapeBackendConfig {
                web3_provider: None,
                eqs_url: eqs_url.clone(),
                relayer_url: relayer_url.clone(),
                address_book_url: address_book_url.clone(),
                eth_mnemonic: None,
                min_polling_delay: Duration::from_millis(500),
            },
            &mut receiver_loader,
        )
        .await
        .unwrap();
        let mut receiver = CapeWallet::new(receiver_backend).await.unwrap();
        let receiver_key = receiver
            .generate_user_key("receiver".into(), None)
            .await
            .unwrap();

        // Transfer from sender to receiver.
        let txn = transfer_token(
            &mut sender,
            receiver_key.address(),
            2,
            AssetCode::native(),
            1,
        )
        .await
        .unwrap();
        await_transaction(&txn, &sender, &[&receiver]).await;
        assert_eq!(
            sender
                .balance_breakdown(&sender_key.address(), &AssetCode::native())
                .await,
            total_balance - U256::from(3u64)
        );

        assert_eq!(
            receiver
                .balance_breakdown(&receiver_key.address(), &AssetCode::native())
                .await,
            2u64.into()
        );

        // Transfer back, just to make sure the receiver is actually able to spend the records it
        // received.
        let txn = transfer_token(
            &mut receiver,
            sender_key.address(),
            1,
            AssetCode::native(),
            1,
        )
        .await
        .unwrap();
        await_transaction(&txn, &receiver, &[&sender]).await;
        assert_eq!(
            sender
                .balance_breakdown(&sender_key.address(), &AssetCode::native())
                .await,
            total_balance - U256::from(2u64)
        );
        assert_eq!(
            receiver
                .balance_breakdown(&receiver_key.address(), &AssetCode::native())
                .await,
            0u64.into()
        );
    }

    #[async_std::test]
    async fn test_anonymous_erc20_transfer() {
        let mut rng = ChaChaRng::from_seed([1u8; 32]);
        let universal_param = &*UNIVERSAL_PARAM;
        let (wrapper_key, relayer_url, address_book_url, contract_address, _) =
            create_test_network(&mut rng, universal_param, None).await;
        let (eqs_url, _eqs_dir, _join_eqs) = spawn_eqs(contract_address).await;

        // Create a wallet to sponsor an asset and a different wallet to deposit (we should be able
        // to deposit from an account other than the sponsor).
        let sponsor_dir = TempDir::new("cape_wallet_backend_test").unwrap();
        let mut sponsor_loader = CapeLoader::from_literal(
            Some(hd::KeyTree::random(&mut rng).1.into_phrase()),
            "password".into(),
            sponsor_dir.path().to_owned(),
            contract_address.into(),
        );
        let sponsor_backend = CapeBackend::new(
            universal_param,
            CapeBackendConfig {
                web3_provider: Some(rpc_url_for_test()),
                eqs_url: eqs_url.clone(),
                relayer_url: relayer_url.clone(),
                address_book_url: address_book_url.clone(),
                eth_mnemonic: None,
                min_polling_delay: Duration::from_millis(500),
            },
            &mut sponsor_loader,
        )
        .await
        .unwrap();
        let mut sponsor = CapeWallet::new(sponsor_backend).await.unwrap();
        let sponsor_key = sponsor
            .generate_user_key("sponsor".into(), None)
            .await
            .unwrap();
        let sponsor_eth_addr = sponsor.eth_address().await.unwrap();

        let wrapper_dir = TempDir::new("cape_wallet_backend_test").unwrap();
        let mut wrapper_loader = CapeLoader::from_literal(
            Some(hd::KeyTree::random(&mut rng).1.into_phrase()),
            "password".into(),
            wrapper_dir.path().to_owned(),
            contract_address.into(),
        );
        let wrapper_backend = CapeBackend::new(
            universal_param,
            CapeBackendConfig {
                web3_provider: Some(rpc_url_for_test()),
                eqs_url: eqs_url.clone(),
                relayer_url: relayer_url.clone(),
                address_book_url: address_book_url.clone(),
                eth_mnemonic: None,
                min_polling_delay: Duration::from_millis(500),
            },
            &mut wrapper_loader,
        )
        .await
        .unwrap();
        let mut wrapper = CapeWallet::new(wrapper_backend).await.unwrap();
        // Add the faucet key to the wrapper wallet, so that they have the native tokens they need
        // to pay the fee to transfer the wrapped tokens.
        wrapper
            .add_user_key(wrapper_key.clone(), "wrapper".into(), EventIndex::default())
            .await
            .unwrap();
        // Wait for the wrapper to register the balance belonging to the key, from the initial grant
        // records.
        retry(|| async {
            wrapper
                .balance_breakdown(&wrapper_key.address(), &AssetCode::native())
                .await
                > 0u64.into()
        })
        .await;
        let total_native_balance = wrapper
            .balance_breakdown(&wrapper_key.address(), &AssetCode::native())
            .await;

        // Fund the Ethereum wallets for contract calls.
        let provider =
            EthRpc::provider(&rpc_url_for_test()).interval(Duration::from_millis(100u64));
        let accounts = provider.get_accounts().await.unwrap();
        assert!(!accounts.is_empty());
        for wallet in [&sponsor, &wrapper] {
            let tx = TransactionRequest::new()
                .to(Address::from(wallet.eth_address().await.unwrap()))
                .value(ethers::utils::parse_ether(U256::from(1u64)).unwrap())
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
        // Check that the asset is correctly reported as a wrapped asset in both wallets.
        assert_eq!(
            Address::from_str(
                &AssetInfo::from_code(&sponsor, cape_asset.code)
                    .await
                    .unwrap()
                    .wrapped_erc20
                    .unwrap()
            )
            .unwrap(),
            erc20_contract.address()
        );
        wrapper
            .import_asset(cape_asset.clone().into())
            .await
            .unwrap();
        assert_eq!(
            Address::from_str(
                &AssetInfo::from_code(&wrapper, cape_asset.code)
                    .await
                    .unwrap()
                    .wrapped_erc20
                    .unwrap()
            )
            .unwrap(),
            erc20_contract.address()
        );

        wrap_simple_token(
            &mut wrapper,
            &wrapper_key.address(),
            cape_asset.clone(),
            &erc20_contract,
            100,
        )
        .await
        .unwrap();

        // To force the wrap to be processed, we need to submit a block of CAPE transactions. We'll
        // transfer some native tokens from `wrapper` to `sponsor`.
        let receipt = wrapper
            .transfer(
                Some(&wrapper_key.address()),
                &AssetCode::native(),
                &[(sponsor_key.address(), 1u64)],
                1u64,
            )
            .await
            .unwrap();
        await_transaction(&receipt, &wrapper, &[&sponsor]).await;
        // Wraps are processed after transactions, so we may have to wait a short time after the
        // transaction is completed for the wrapped balance to show up.
        retry(|| async {
            wrapper
                .balance_breakdown(&wrapper_key.address(), &AssetCode::native())
                .await
                == total_native_balance - U256::from(2u64)
        })
        .await;
        assert_eq!(
            sponsor
                .balance_breakdown(&sponsor_key.address(), &AssetCode::native())
                .await,
            1u64.into()
        );
        // The transfer transaction caused the wrap record to be created.
        assert_eq!(
            wrapper
                .balance_breakdown(&wrapper_key.address(), &cape_asset.code)
                .await,
            100u64.into()
        );

        assert!(wrapper.is_wrapped_asset(cape_asset.code).await);
        assert_eq!(wrapper.is_wrapped_asset(AssetCode::native()).await, false);

        // Make sure the wrapper can access the wrapped tokens, by transferring them to someone else
        // (we'll reuse the `sponsor` wallet, but this could be a separate role).
        let receipt = wrapper
            .transfer(
                Some(&wrapper_key.address()),
                &cape_asset.code,
                &[(sponsor_key.address(), 100u64)],
                1u64,
            )
            .await
            .unwrap();
        await_transaction(&receipt, &wrapper, &[&sponsor]).await;
        assert_eq!(
            wrapper
                .balance_breakdown(&wrapper_key.address(), &cape_asset.code)
                .await,
            0u64.into()
        );
        assert_eq!(
            sponsor
                .balance_breakdown(&sponsor_key.address(), &cape_asset.code)
                .await,
            100u64.into()
        );

        // Finally, withdraw the wrapped tokens back into the ERC20 token type.
        let receipt = sponsor
            .burn(
                Some(&sponsor_key.address()),
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
                .balance_breakdown(&sponsor_key.address(), &cape_asset.code)
                .await,
            0u64.into()
        );
        assert_eq!(
            erc20_contract
                .balance_of(sponsor_eth_addr.into())
                .call()
                .await
                .unwrap(),
            100u64.into()
        );
    }
}
