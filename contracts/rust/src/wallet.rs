use crate::ledger::*;
use async_std::sync::{Mutex, MutexGuard};
use async_trait::async_trait;
use futures::stream::Stream;
use jf_aap::{
    keys::{UserAddress, UserPubKey},
    structs::{Nullifier, ReceiverMemo},
    Signature,
};
use serde::{de::DeserializeOwned, Serialize};
use std::pin::Pin;
use wallet::{persistence::AtomicWalletStorage, WalletError, WalletState};
use zerok_lib::{node::LedgerEvent, wallet};

pub struct CAPEWalletBackend<'a, Metadata: Serialize + DeserializeOwned> {
    storage: Mutex<AtomicWalletStorage<'a, CAPELedger, Metadata>>,
}

#[async_trait]
impl<'a, Metadata: Send + Serialize + DeserializeOwned> wallet::WalletBackend<'a, CAPELedger>
    for CAPEWalletBackend<'a, Metadata>
{
    type EventStream = Pin<Box<dyn Stream<Item = LedgerEvent<CAPELedger>> + Send>>;
    type Storage = AtomicWalletStorage<'a, CAPELedger, Metadata>;

    async fn storage<'l>(&'l mut self) -> MutexGuard<'l, Self::Storage> {
        self.storage.lock().await
    }

    async fn create(&mut self) -> Result<WalletState<'a, CAPELedger>, WalletError> {
        unimplemented!()
    }

    async fn subscribe(&self, _starting_at: u64) -> Self::EventStream {
        // Return an event stream containing events for committed blocks, rejected blocks, and
        // published memos. This will involve both the query service and the memo bulletin board.
        unimplemented!()
    }

    async fn get_public_key(&self, _address: &UserAddress) -> Result<UserPubKey, WalletError> {
        // Get the encryption public key associated with this address from the address map service.
        unimplemented!()
    }

    async fn get_nullifier_proof(
        &self,
        _nullifiers: &mut CAPENullifierSet,
        _nullifier: Nullifier,
    ) -> Result<(bool, ()), WalletError> {
        unimplemented!()
    }

    async fn submit(&mut self, _txn: CAPETransaction) -> Result<(), WalletError> {
        unimplemented!()
    }

    async fn post_memos(
        &mut self,
        _block_id: u64,
        _txn_id: u64,
        _memos: Vec<ReceiverMemo>,
        _sig: Signature,
    ) -> Result<(), WalletError> {
        unimplemented!()
    }
}

pub type Wallet<'a, Metadata> = wallet::Wallet<'a, CAPEWalletBackend<'a, Metadata>, CAPELedger>;
