use crate::query_result_state::QueryResultState;
use atomic_store::{
    load_store::BincodeLoadStore, AppendLog, AtomicStore, AtomicStoreLoader, PersistenceError,
};

use std::path::{Path, PathBuf};

// hook up with atomic_store

pub struct StatePersistence {
    atomic_store: AtomicStore,
    state_snapshot: AppendLog<BincodeLoadStore<QueryResultState>>,
}

impl StatePersistence {
    pub fn new(store_path: &Path, key_tag: &str) -> Result<StatePersistence, PersistenceError> {
        let mut store_path = PathBuf::from(store_path);
        store_path.push("eqs");
        let mut loader = AtomicStoreLoader::create(&store_path, key_tag)?;
        let snapshot_tag = format!("{}_state", key_tag);
        let state_snapshot =
            AppendLog::create(&mut loader, Default::default(), &snapshot_tag, 1024)?;
        let atomic_store = AtomicStore::open(loader)?;
        Ok(StatePersistence {
            atomic_store,
            state_snapshot,
        })
    }

    pub fn load(store_path: &Path, key_tag: &str) -> Result<StatePersistence, PersistenceError> {
        let mut store_path = PathBuf::from(store_path);
        store_path.push("eqs");
        let mut loader = AtomicStoreLoader::load(&store_path, key_tag)?;
        let snapshot_tag = format!("{}_state", key_tag);
        let state_snapshot = AppendLog::load(&mut loader, Default::default(), &snapshot_tag, 1024)?;
        let atomic_store = AtomicStore::open(loader)?;
        Ok(StatePersistence {
            atomic_store,
            state_snapshot,
        })
    }

    pub fn store_latest_state(&mut self, state: &QueryResultState) {
        self.state_snapshot.store_resource(state).unwrap();
        self.state_snapshot.commit_version().unwrap();
        self.atomic_store.commit_version().unwrap();
    }

    pub fn load_latest_state(&self) -> Result<QueryResultState, PersistenceError> {
        self.state_snapshot.load_latest()
    }
}
