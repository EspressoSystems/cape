use crate::wallet::{CapeWalletBackend, CapeWalletError};
use async_std::sync::{Mutex, MutexGuard};
use async_trait::async_trait;
use cap_rust_sandbox::{
    deploy::EthMiddleware, ledger::*, model::*, universal_param::UNIVERSAL_PARAM,
};
use commit::Committable;
use futures::stream::Stream;
use itertools::izip;
use jf_cap::{
    keys::{UserAddress, UserKeyPair, UserPubKey},
    proof::{freeze::FreezeProvingKey, transfer::TransferProvingKey, UniversalParam},
    structs::{AssetDefinition, Nullifier, ReceiverMemo, RecordCommitment, RecordOpening},
    MerklePath, MerkleTree, Signature, TransactionNote,
};
use key_set::{OrderByOutputs, ProverKeySet, SizedKey, VerifierKeySet};
use lazy_static::lazy_static;
use rand_chacha::{rand_core::SeedableRng, ChaChaRng};
use reef::{
    traits::{Block as _, Ledger as _, Transaction as _},
    Block,
};
use seahorse::{
    events::{EventIndex, EventSource, LedgerEvent},
    hd::KeyTree,
    loader::WalletLoader,
    persistence::AtomicWalletStorage,
    testing,
    txn_builder::{RecordDatabase, TransactionState, TransactionUID},
    WalletBackend, WalletError, WalletState,
};
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use tempdir::TempDir;
use testing::{MockEventSource, MockLedger, MockNetwork, SystemUnderTest};

#[derive(Clone)]
struct CommittedTransaction {
    txn: CapeTransition,
    uids: Vec<u64>,
    #[allow(clippy::type_complexity)]
    memos: Option<(
        Vec<(ReceiverMemo, RecordCommitment, u64, MerklePath)>,
        Signature,
    )>,
}

// A mock implementation of a CAPE network which maintains the full state of a CAPE ledger locally.
#[derive(Clone)]
pub struct MockCapeNetwork {
    contract: CapeContractState,
    call_data: HashMap<TransactionUID<CapeLedger>, (Vec<ReceiverMemo>, Signature)>,

    // Mock EQS and peripheral services
    block_height: u64,
    records: MerkleTree,
    // When an ERC20 deposit is finalized during a block submission, the contract emits an event
    // containing only the commitment of the new record. Therefore, to correlate these events with
    // the other information needed to reconstruct a CapeTransition::Wrap, the query service needs
    // to monitor the contracts Erc20Deposited events and keep track of the deposits which are
    // pending finalization.
    pending_erc20_deposits:
        HashMap<RecordCommitment, (Erc20Code, EthereumAddr, Box<RecordOpening>)>,
    events: MockEventSource<CapeLedger>,
    txns: HashMap<(u64, u64), CommittedTransaction>,
    address_map: HashMap<UserAddress, UserPubKey>,
}

impl MockCapeNetwork {
    pub fn new(
        verif_crs: VerifierKeySet,
        records: MerkleTree,
        initial_grant_memos: Vec<(ReceiverMemo, u64)>,
    ) -> Self {
        let mut ledger = Self {
            contract: CapeContractState::new(verif_crs, records.clone()),
            call_data: Default::default(),
            block_height: 0,
            records,
            pending_erc20_deposits: Default::default(),
            events: MockEventSource::new(EventSource::QueryService),
            txns: Default::default(),
            address_map: Default::default(),
        };

        // Broadcast receiver memos for the records which are included in the tree from the start,
        // so that clients can access records they have been granted at ledger setup time in a
        // uniform way.
        let memo_outputs = initial_grant_memos
            .into_iter()
            .map(|(memo, uid)| {
                let (comm, merkle_path) = ledger
                    .records
                    .get_leaf(uid)
                    .expect_ok()
                    .map(|(_, proof)| {
                        (
                            RecordCommitment::from_field_element(proof.leaf.0),
                            proof.path,
                        )
                    })
                    .unwrap();
                (memo, comm, uid, merkle_path)
            })
            .collect();
        ledger.generate_event(LedgerEvent::Memos {
            outputs: memo_outputs,
            transaction: None,
        });

        ledger
    }

    pub fn register_erc20(
        &mut self,
        asset_def: AssetDefinition,
        erc20_code: Erc20Code,
        sponsor_addr: EthereumAddr,
    ) -> Result<(), CapeValidationError> {
        self.submit_operations(vec![CapeModelOperation::RegisterErc20 {
            asset_def: Box::new(asset_def),
            erc20_code,
            sponsor_addr,
        }])
    }

    pub fn wrap_erc20(
        &mut self,
        erc20_code: Erc20Code,
        src_addr: EthereumAddr,
        ro: RecordOpening,
    ) -> Result<(), CapeValidationError> {
        self.submit_operations(vec![CapeModelOperation::WrapErc20 {
            erc20_code,
            src_addr,
            ro: Box::new(ro),
        }])
    }

    pub fn create_wallet<'a>(
        &self,
        univ_param: &'a UniversalParam,
    ) -> Result<WalletState<'a, CapeLedger>, CapeWalletError> {
        // Construct proving keys of the same arities as the verifier keys from the validator.
        let proving_keys = Arc::new(ProverKeySet {
            mint: jf_cap::proof::mint::preprocess(univ_param, CAPE_MERKLE_HEIGHT)
                .map_err(|source| CapeWalletError::CryptoError { source })?
                .0,
            freeze: self
                .contract
                .verif_crs
                .freeze
                .iter()
                .map(|k| {
                    Ok::<FreezeProvingKey, WalletError<CapeLedger>>(
                        jf_cap::proof::freeze::preprocess(
                            univ_param,
                            k.num_inputs(),
                            CAPE_MERKLE_HEIGHT,
                        )
                        .map_err(|source| CapeWalletError::CryptoError { source })?
                        .0,
                    )
                })
                .collect::<Result<_, _>>()?,
            xfr: self
                .contract
                .verif_crs
                .xfr
                .iter()
                .map(|k| {
                    Ok::<TransferProvingKey, WalletError<CapeLedger>>(
                        jf_cap::proof::transfer::preprocess(
                            univ_param,
                            k.num_inputs(),
                            k.num_outputs(),
                            CAPE_MERKLE_HEIGHT,
                        )
                        .map_err(|source| CapeWalletError::CryptoError { source })?
                        .0,
                    )
                })
                .collect::<Result<_, _>>()?,
        });

        // `records` should be _almost_ completely sparse. However, even a fully pruned Merkle tree
        // contains the last leaf appended, but as a new wallet, we don't care about _any_ of the
        // leaves, so make a note to forget the last one once more leaves have been appended.
        let record_mt = self.records.clone();
        let merkle_leaf_to_forget = if record_mt.num_leaves() > 0 {
            Some(record_mt.num_leaves() - 1)
        } else {
            None
        };

        Ok(WalletState {
            proving_keys,
            txn_state: TransactionState {
                validator: CapeTruster::new(self.block_height, record_mt.num_leaves()),
                now: self.now(),
                nullifiers: Default::default(),
                // Completely sparse nullifier set
                record_mt,
                records: RecordDatabase::default(),
                merkle_leaf_to_forget,

                transactions: Default::default(),
            },
            key_scans: Default::default(),
            key_state: Default::default(),
            auditable_assets: Default::default(),
            audit_keys: Default::default(),
            freeze_keys: Default::default(),
            user_keys: Default::default(),
            defined_assets: Default::default(),
        })
    }

    pub fn subscribe(
        &mut self,
        from: EventIndex,
        to: Option<EventIndex>,
    ) -> Pin<Box<dyn Stream<Item = (LedgerEvent<CapeLedger>, EventSource)> + Send>> {
        self.events.subscribe(from, to)
    }

    pub fn get_public_key(&self, address: &UserAddress) -> Result<UserPubKey, CapeWalletError> {
        Ok(self
            .address_map
            .get(address)
            .ok_or(CapeWalletError::Failed {
                msg: String::from("invalid user address"),
            })?
            .clone())
    }

    pub fn nullifier_spent(&self, nullifier: Nullifier) -> bool {
        self.contract.nullifiers.contains(&nullifier)
    }

    pub fn get_transaction(
        &self,
        block_id: u64,
        txn_id: u64,
    ) -> Result<CapeTransition, CapeWalletError> {
        Ok(self
            .txns
            .get(&(block_id, txn_id))
            .ok_or(CapeWalletError::Failed {
                msg: String::from("invalid transaction ID"),
            })?
            .txn
            .clone())
    }

    pub fn register_user_key(&mut self, key_pair: &UserKeyPair) -> Result<(), CapeWalletError> {
        let pub_key = key_pair.pub_key();
        self.address_map.insert(pub_key.address(), pub_key);
        Ok(())
    }

    pub fn get_wrapped_asset(&self, asset: &AssetDefinition) -> Result<Erc20Code, CapeWalletError> {
        match self.contract.erc20_registrar.get(asset) {
            Some((erc20_code, _)) => Ok(erc20_code.clone()),
            None => Err(WalletError::<CapeLedger>::UndefinedAsset { asset: asset.code }),
        }
    }

    pub fn store_call_data(
        &mut self,
        txn: TransactionUID<CapeLedger>,
        memos: Vec<ReceiverMemo>,
        sig: Signature,
    ) {
        self.call_data.insert(txn, (memos, sig));
    }

    pub fn submit_operations(
        &mut self,
        ops: Vec<CapeModelOperation>,
    ) -> Result<(), CapeValidationError> {
        let (new_state, effects) = self.contract.submit_operations(ops)?;
        let mut events = vec![];
        for effect in effects {
            if let CapeModelEthEffect::Emit(event) = effect {
                events.push(event);
            } else {
                //TODO Simulate and validate the other ETH effects. If any effects fail, the
                // whole transaction must be considered reverted with no visible effects.
            }
        }

        // Simulate the EQS processing the events emitted by the contract, updating its state, and
        // broadcasting processed events to subscribers.
        for event in events {
            self.handle_event(event);
        }
        self.contract = new_state;

        Ok(())
    }

    fn handle_event(&mut self, event: CapeModelEvent) {
        match event {
            CapeModelEvent::BlockCommitted { txns, wraps } => {
                // Convert the transactions and wraps into CapeTransitions, and collect them all
                // into a single block, in the order they were processed by the contract
                // (transactions first, then wraps).
                let block = txns
                    .into_iter()
                    .map(CapeTransition::Transaction)
                    .chain(wraps.into_iter().map(|comm| {
                        // Look up the auxiliary information associated with this deposit which
                        // we saved when we processed the deposit event. This lookup cannot
                        // fail, because the contract only finalizes a Wrap operation after it
                        // has already processed the deposit, which involves emitting an
                        // Erc20Deposited event.
                        let (erc20_code, src_addr, ro) =
                            self.pending_erc20_deposits.remove(&comm).unwrap();
                        CapeTransition::Wrap {
                            erc20_code,
                            src_addr,
                            ro,
                        }
                    }))
                    .collect::<Vec<_>>();

                // Add transactions and outputs to query service data structures.
                for (i, txn) in block.iter().enumerate() {
                    let mut uids = Vec::new();
                    for comm in txn.output_commitments() {
                        uids.push(self.records.num_leaves());
                        self.records.push(comm.to_field_element());
                    }
                    self.txns.insert(
                        (self.block_height, i as u64),
                        CommittedTransaction {
                            txn: txn.clone(),
                            uids,
                            memos: None,
                        },
                    );
                }

                self.generate_event(LedgerEvent::Commit {
                    block: CapeBlock::new(block.clone()),
                    block_id: self.block_height,
                    state_comm: self.block_height + 1,
                });

                // The memos for this block should have already been posted in the calldata, so we
                // can now generate the corresponding Memos events.
                for (txn_id, txn) in block.into_iter().enumerate() {
                    if let Some((memos, sig)) = self.call_data.remove(&TransactionUID(txn.commit()))
                    {
                        self.post_memos(self.block_height, txn_id as u64, memos, sig)
                            .unwrap();
                    }
                }

                self.block_height += 1;
            }

            CapeModelEvent::Erc20Deposited {
                erc20_code,
                src_addr,
                ro,
            } => {
                self.pending_erc20_deposits
                    .insert(RecordCommitment::from(&*ro), (erc20_code, src_addr, ro));
            }
        }
    }
}

impl<'a> MockNetwork<'a, CapeLedger> for MockCapeNetwork {
    fn now(&self) -> EventIndex {
        self.events.now()
    }

    fn submit(&mut self, block: Block<CapeLedger>) -> Result<(), WalletError<CapeLedger>> {
        // Convert the submitted transactions to CapeOperations.
        let ops = block
            .txns()
            .into_iter()
            .map(|txn| match txn {
                CapeTransition::Transaction(txn) => CapeModelOperation::SubmitBlock(vec![txn]),
                CapeTransition::Wrap {
                    erc20_code,
                    src_addr,
                    ro,
                } => CapeModelOperation::WrapErc20 {
                    erc20_code,
                    src_addr,
                    ro,
                },
            })
            .collect();

        self.submit_operations(ops).map_err(cape_to_wallet_err)
    }

    fn post_memos(
        &mut self,
        block_id: u64,
        txn_id: u64,
        memos: Vec<ReceiverMemo>,
        sig: Signature,
    ) -> Result<(), WalletError<CapeLedger>> {
        let txn = match self.txns.get_mut(&(block_id, txn_id)) {
            Some(txn) => txn,
            None => {
                return Err(CapeWalletError::Failed {
                    msg: String::from("invalid transaction ID"),
                });
            }
        };
        if txn.memos.is_some() {
            return Err(CapeWalletError::Failed {
                msg: String::from("memos already posted"),
            });
        }

        // Validate the new memos.
        match &txn.txn {
            CapeTransition::Transaction(CapeModelTxn::CAP(note)) => {
                if note.verify_receiver_memos_signature(&memos, &sig).is_err() {
                    return Err(CapeWalletError::Failed {
                        msg: String::from("invalid memos signature"),
                    });
                }
                if memos.len() != txn.txn.output_len() {
                    return Err(CapeWalletError::Failed {
                        msg: format!("wrong number of memos (expected {})", txn.txn.output_len()),
                    });
                }
            }
            CapeTransition::Transaction(CapeModelTxn::Burn { xfr, .. }) => {
                if TransactionNote::Transfer(Box::new(*xfr.clone()))
                    .verify_receiver_memos_signature(&memos, &sig)
                    .is_err()
                {
                    return Err(CapeWalletError::Failed {
                        msg: String::from("invalid memos signature"),
                    });
                }
                if memos.len() != txn.txn.output_len() {
                    return Err(CapeWalletError::Failed {
                        msg: format!("wrong number of memos (expected {})", txn.txn.output_len()),
                    });
                }
            }
            _ => {
                return Err(CapeWalletError::Failed {
                    msg: String::from("cannot post memos for wrap transactions"),
                });
            }
        }

        // Authenticate the validity of the records corresponding to the memos.
        let merkle_tree = &self.records;
        let merkle_paths = txn
            .uids
            .iter()
            .map(|uid| merkle_tree.get_leaf(*uid).expect_ok().unwrap().1.path)
            .collect::<Vec<_>>();

        // Store and broadcast the new memos.
        let memos = izip!(
            memos,
            txn.txn.output_commitments(),
            txn.uids.iter().cloned(),
            merkle_paths
        )
        .collect::<Vec<_>>();
        txn.memos = Some((memos.clone(), sig));
        let event = LedgerEvent::Memos {
            outputs: memos,
            transaction: Some((block_id as u64, txn_id as u64)),
        };
        self.generate_event(event);

        Ok(())
    }

    fn memos_source(&self) -> EventSource {
        EventSource::QueryService
    }

    fn generate_event(&mut self, event: LedgerEvent<CapeLedger>) {
        self.events.publish(event)
    }

    fn event(
        &self,
        index: EventIndex,
        source: EventSource,
    ) -> Result<LedgerEvent<CapeLedger>, WalletError<CapeLedger>> {
        match source {
            EventSource::QueryService => self.events.get(index),
            _ => Err(WalletError::Failed {
                msg: String::from("invalid event source"),
            }),
        }
    }
}

pub type MockCapeLedger<'a> =
    MockLedger<'a, CapeLedger, MockCapeNetwork, AtomicWalletStorage<'a, CapeLedger, ()>>;

pub struct MockCapeBackend<'a, Meta: Serialize + DeserializeOwned> {
    storage: Arc<Mutex<AtomicWalletStorage<'a, CapeLedger, Meta>>>,
    pub(crate) ledger: Arc<Mutex<MockCapeLedger<'a>>>,
    key_stream: KeyTree,
}

impl<'a, Meta: Serialize + DeserializeOwned + Send> MockCapeBackend<'a, Meta> {
    pub fn new(
        ledger: Arc<Mutex<MockCapeLedger<'a>>>,
        loader: &mut impl WalletLoader<CapeLedger, Meta = Meta>,
    ) -> Result<Self, WalletError<CapeLedger>> {
        // Workaround for https://github.com/SpectrumXYZ/atomicstore/issues/2, which affects logs
        // containing more than one entry in a file. We simply set the fill size small enough that
        // there will only ever be one entry per file.
        //
        // Note that this issue only effects CAPE (not the Spectrum wallet, which uses the same
        // storage implementation) because the CAPE wallet state is much smaller than the Spectrum
        // wallet state due to the CapeLedger types not doing lightweight validation.
        let storage = AtomicWalletStorage::new(loader, 128)?;
        Ok(Self {
            key_stream: storage.key_stream(),
            storage: Arc::new(Mutex::new(storage)),
            ledger,
        })
    }

    pub fn new_for_test(
        ledger: Arc<Mutex<MockCapeLedger<'a>>>,
        storage: Arc<Mutex<AtomicWalletStorage<'a, CapeLedger, Meta>>>,
        key_stream: KeyTree,
    ) -> Result<MockCapeBackend<'a, Meta>, WalletError<CapeLedger>> {
        Ok(Self {
            key_stream,
            storage,
            ledger,
        })
    }
}

#[async_trait]
impl<'a, Meta: Serialize + DeserializeOwned + Send> WalletBackend<'a, CapeLedger>
    for MockCapeBackend<'a, Meta>
{
    type EventStream = Pin<Box<dyn Stream<Item = (LedgerEvent<CapeLedger>, EventSource)> + Send>>;
    type Storage = AtomicWalletStorage<'a, CapeLedger, Meta>;

    async fn storage<'l>(&'l mut self) -> MutexGuard<'l, Self::Storage> {
        self.storage.lock().await
    }

    async fn create(&mut self) -> Result<WalletState<'a, CapeLedger>, WalletError<CapeLedger>> {
        let univ_param = &*UNIVERSAL_PARAM;
        let state = self
            .ledger
            .lock()
            .await
            .network()
            .create_wallet(univ_param)?;
        self.storage().await.create(&state).await?;
        Ok(state)
    }

    async fn subscribe(&self, from: EventIndex, to: Option<EventIndex>) -> Self::EventStream {
        self.ledger.lock().await.network().subscribe(from, to)
    }

    async fn get_public_key(
        &self,
        address: &UserAddress,
    ) -> Result<UserPubKey, WalletError<CapeLedger>> {
        self.ledger.lock().await.network().get_public_key(address)
    }

    async fn register_user_key(
        &mut self,
        key_pair: &UserKeyPair,
    ) -> Result<(), WalletError<CapeLedger>> {
        self.ledger
            .lock()
            .await
            .network()
            .register_user_key(key_pair)
    }

    async fn get_nullifier_proof(
        &self,
        nullifiers: &mut CapeNullifierSet,
        nullifier: Nullifier,
    ) -> Result<(bool, ()), WalletError<CapeLedger>> {
        // Try to look up the nullifier in our "local" cache. If it is not there, query the contract
        // and cache it.
        match nullifiers.get(nullifier) {
            Some(ret) => Ok((ret, ())),
            None => {
                let ret = self
                    .ledger
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
    ) -> Result<CapeTransition, WalletError<CapeLedger>> {
        self.ledger
            .lock()
            .await
            .network()
            .get_transaction(block_id, txn_id)
    }

    fn key_stream(&self) -> KeyTree {
        self.key_stream.clone()
    }

    async fn submit(
        &mut self,
        txn: CapeTransition,
        uid: TransactionUID<CapeLedger>,
        memos: Vec<ReceiverMemo>,
        sig: Signature,
    ) -> Result<(), WalletError<CapeLedger>> {
        let mut ledger = self.ledger.lock().await;
        ledger.network().store_call_data(uid, memos, sig);
        ledger.submit(txn)
    }
}

#[async_trait]
impl<'a, Meta: Serialize + DeserializeOwned + Send> CapeWalletBackend<'a>
    for MockCapeBackend<'a, Meta>
{
    async fn register_erc20_asset(
        &mut self,
        asset: &AssetDefinition,
        erc20_code: Erc20Code,
        sponsor: EthereumAddr,
    ) -> Result<(), WalletError<CapeLedger>> {
        self.ledger
            .lock()
            .await
            .network()
            .register_erc20(asset.clone(), erc20_code, sponsor)
            .map_err(cape_to_wallet_err)
    }

    async fn get_wrapped_erc20_code(
        &self,
        asset: &AssetDefinition,
    ) -> Result<Erc20Code, WalletError<CapeLedger>> {
        self.ledger.lock().await.network().get_wrapped_asset(asset)
    }

    async fn wrap_erc20(
        &mut self,
        erc20_code: Erc20Code,
        src_addr: EthereumAddr,
        ro: RecordOpening,
    ) -> Result<(), WalletError<CapeLedger>> {
        self.ledger
            .lock()
            .await
            .network()
            .wrap_erc20(erc20_code, src_addr, ro)
            .map_err(cape_to_wallet_err)
    }

    fn eth_client(&self) -> Result<Arc<EthMiddleware>, CapeWalletError> {
        Err(CapeWalletError::Failed {
            msg: String::from("eth_client is not implemented for MockCapeBackend"),
        })
    }
}

fn cape_to_wallet_err(err: CapeValidationError) -> WalletError<CapeLedger> {
    //TODO Convert CapeValidationError to WalletError in a better way. Maybe WalletError should be
    // parameterized on the ledger type and there should be a ledger trait ValidationError.
    WalletError::Failed {
        msg: err.to_string(),
    }
}

pub struct MockCapeWalletLoader {
    pub path: PathBuf,
    pub key: KeyTree,
}

impl WalletLoader<CapeLedger> for MockCapeWalletLoader {
    type Meta = ();

    fn location(&self) -> PathBuf {
        self.path.clone()
    }

    fn create(&mut self) -> Result<(Self::Meta, KeyTree), WalletError<CapeLedger>> {
        Ok(((), self.key.clone()))
    }

    fn load(&mut self, _meta: &Self::Meta) -> Result<KeyTree, WalletError<CapeLedger>> {
        Ok(self.key.clone())
    }
}

pub struct CapeTest {
    rng: ChaChaRng,
    temp_dirs: Vec<TempDir>,
}

impl CapeTest {
    fn temp_dir(&mut self) -> PathBuf {
        let dir = TempDir::new("cape_wallet").unwrap();
        let path = PathBuf::from(dir.path());
        self.temp_dirs.push(dir);
        path
    }
}

impl Default for CapeTest {
    fn default() -> Self {
        Self {
            rng: ChaChaRng::from_seed([42u8; 32]),
            temp_dirs: Vec::new(),
        }
    }
}

lazy_static! {
    static ref CAPE_UNIVERSAL_PARAM: UniversalParam = universal_param::get(
        &mut ChaChaRng::from_seed([1u8; 32]),
        CapeLedger::merkle_height()
    );
}

#[async_trait]
impl<'a> SystemUnderTest<'a> for CapeTest {
    type Ledger = CapeLedger;
    type MockBackend = MockCapeBackend<'a, ()>;
    type MockNetwork = MockCapeNetwork;
    type MockStorage = AtomicWalletStorage<'a, CapeLedger, ()>;

    async fn create_network(
        &mut self,
        verif_crs: VerifierKeySet,
        _proof_crs: ProverKeySet<'a, OrderByOutputs>,
        records: MerkleTree,
        initial_grants: Vec<(RecordOpening, u64)>,
    ) -> Self::MockNetwork {
        let initial_memos = initial_grants
            .into_iter()
            .map(|(ro, uid)| (ReceiverMemo::from_ro(&mut self.rng, &ro, &[]).unwrap(), uid))
            .collect();
        MockCapeNetwork::new(verif_crs, records, initial_memos)
    }

    async fn create_storage(&mut self) -> Self::MockStorage {
        let mut loader = MockCapeWalletLoader {
            path: self.temp_dir(),
            key: KeyTree::random(&mut self.rng).unwrap().0,
        };
        AtomicWalletStorage::new(&mut loader, 128).unwrap()
    }

    async fn create_backend(
        &mut self,
        ledger: Arc<Mutex<MockLedger<'a, Self::Ledger, Self::MockNetwork, Self::MockStorage>>>,
        _initial_grants: Vec<(RecordOpening, u64)>,
        key_stream: KeyTree,
        storage: Arc<Mutex<Self::MockStorage>>,
    ) -> Self::MockBackend {
        MockCapeBackend::new_for_test(ledger, storage, key_stream).unwrap()
    }

    fn universal_param(&self) -> &'a UniversalParam {
        &*CAPE_UNIVERSAL_PARAM
    }
}

// CAPE-specific tests
#[cfg(test)]
mod cape_wallet_tests {
    use super::*;
    use crate::wallet::CapeWalletExt;
    use jf_cap::structs::{AssetCode, AssetPolicy};
    use seahorse::txn_builder::TransactionError;
    use std::time::Instant;

    #[cfg(feature = "slow-tests")]
    use testing::generic_wallet_tests;
    #[cfg(feature = "slow-tests")]
    seahorse::instantiate_generic_wallet_tests!(CapeTest);

    #[async_std::test]
    async fn test_cape_wallet() -> std::io::Result<()> {
        let mut t = CapeTest::default();

        // Initialize a ledger and wallet, and get the owner address.
        let mut now = Instant::now();
        let num_inputs = 2;
        let num_outputs = 2;
        let initial_grant = 10;
        let (ledger, mut wallets) = t
            .create_test_network(&[(num_inputs, num_outputs)], vec![initial_grant], &mut now)
            .await;
        assert_eq!(wallets.len(), 1);
        let owner = wallets[0].1.clone();
        t.sync(&ledger, &wallets).await;
        println!("CAPE wallet created: {}s", now.elapsed().as_secs_f32());

        // Check the balance after CAPE wallet initialization.
        assert_eq!(
            wallets[0].0.balance(&owner, &AssetCode::native()).await,
            initial_grant
        );

        // Create an ERC20 code, sponsor address, and asset information.
        now = Instant::now();
        let erc20_addr = EthereumAddr([1u8; 20]);
        let erc20_code = Erc20Code(erc20_addr);
        let sponsor_addr = EthereumAddr([2u8; 20]);
        let cap_asset_policy = AssetPolicy::default();

        // Sponsor the ERC20 token.
        let cap_asset = wallets[0]
            .0
            .sponsor(erc20_code, sponsor_addr.clone(), cap_asset_policy)
            .await
            .unwrap();
        println!("Sponsor completed: {}s", now.elapsed().as_secs_f32());

        // Wrapping an undefined asset should fail.
        let wrap_amount = 6;
        match wallets[0]
            .0
            .wrap(
                sponsor_addr.clone(),
                AssetDefinition::dummy(),
                owner.clone(),
                wrap_amount,
            )
            .await
        {
            Err(WalletError::UndefinedAsset { asset: _ }) => {}
            e => {
                panic!("Expected WalletError::UndefinedAsset, found {:?}", e);
            }
        };

        // Wrap the sponsored asset.
        now = Instant::now();
        wallets[0]
            .0
            .wrap(
                sponsor_addr.clone(),
                cap_asset.clone(),
                owner.clone(),
                wrap_amount,
            )
            .await
            .unwrap();
        println!("Wrap completed: {}s", now.elapsed().as_secs_f32());
        assert_eq!(wallets[0].0.balance(&owner, &cap_asset.code).await, 0);

        // Submit dummy transactions to finalize the wrap.
        now = Instant::now();
        let dummy_coin = wallets[0]
            .0
            .define_asset("Dummy asset".as_bytes(), Default::default())
            .await
            .unwrap();
        let mint_fee = 1;
        wallets[0]
            .0
            .mint(&owner, mint_fee, &dummy_coin.code, 5, owner.clone())
            .await
            .unwrap();
        t.sync(&ledger, &wallets).await;
        println!(
            "Dummy transactions submitted and wrap finalized: {}s",
            now.elapsed().as_secs_f32()
        );

        // Check the balance after the wrap.
        assert_eq!(
            wallets[0].0.balance(&owner, &AssetCode::native()).await,
            initial_grant - mint_fee
        );
        assert_eq!(
            wallets[0].0.balance(&owner, &cap_asset.code).await,
            wrap_amount
        );

        // Burning an amount more than the wrapped asset should fail.
        let mut burn_amount = wrap_amount + 1;
        let burn_fee = 1;
        match wallets[0]
            .0
            .burn(
                &owner,
                sponsor_addr.clone(),
                &cap_asset.code.clone(),
                burn_amount,
                burn_fee,
            )
            .await
        {
            Err(WalletError::TransactionError {
                source: TransactionError::InsufficientBalance { .. },
            }) => {}
            e => {
                panic!(
                    "Expected TransactionError::InsufficientBalance, found {:?}",
                    e
                );
            }
        }

        // Burning an amount not corresponding to the wrapped asset should fail.
        burn_amount = wrap_amount - 1;
        match wallets[0]
            .0
            .burn(
                &owner,
                sponsor_addr.clone(),
                &cap_asset.code.clone(),
                burn_amount,
                burn_fee,
            )
            .await
        {
            Err(WalletError::TransactionError {
                source: TransactionError::InvalidSize { .. },
            }) => {}
            e => {
                panic!("Expected TransactionError::InvalidSize, found {:?}", e);
            }
        }

        // Burn the wrapped asset.
        now = Instant::now();
        burn_amount = wrap_amount;
        wallets[0]
            .0
            .burn(
                &owner,
                sponsor_addr.clone(),
                &cap_asset.code.clone(),
                burn_amount,
                burn_fee,
            )
            .await
            .unwrap();
        t.sync(&ledger, &wallets).await;
        println!("Burn completed: {}s", now.elapsed().as_secs_f32());

        // Check the balance after the burn.
        assert_eq!(wallets[0].0.balance(&owner, &cap_asset.code).await, 0);
        assert_eq!(
            wallets[0].0.balance(&owner, &AssetCode::native()).await,
            initial_grant - mint_fee - burn_fee
        );

        Ok(())
    }
}
