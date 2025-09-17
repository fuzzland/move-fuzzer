use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use aptos_config::config::{PrunerConfig, StorageDirPaths};
use aptos_db::AptosDB;
use aptos_executor::db_bootstrapper::{generate_waypoint, maybe_bootstrap};
use aptos_storage_interface::state_store::state::{LedgerState, State};
use aptos_storage_interface::state_store::state_view::cached_state_view::CachedStateView;
use aptos_storage_interface::{DbReader, DbReaderWriter};
use aptos_types::state_store::state_key::StateKey;
use aptos_types::state_store::state_storage_usage::StateStorageUsage;
use aptos_types::state_store::state_value::StateValue;
use aptos_types::state_store::{StateViewId, TStateView};
use aptos_types::transaction::Transaction;
use aptos_types::write_set::WriteSet;
use aptos_vm::aptos_vm::AptosVMBlockExecutor;
use aptos_vm::AptosVM;
use aptos_vm_environment::environment::AptosEnvironment;
use aptos_vm_genesis::test_genesis_transaction;

pub use crate::transaction_result::TransactionResult;

pub struct OverlayStateView {
    base: CachedStateView,
    overlay: HashMap<StateKey, Option<StateValue>>,
}

impl TStateView for OverlayStateView {
    type Key = StateKey;

    fn id(&self) -> StateViewId {
        StateViewId::Miscellaneous
    }

    fn get_usage(&self) -> aptos_types::state_store::StateViewResult<StateStorageUsage> {
        self.base.get_usage()
    }

    fn next_version(&self) -> aptos_types::transaction::Version {
        self.base.next_version()
    }

    fn get_state_value(&self, state_key: &Self::Key) -> aptos_types::state_store::StateViewResult<Option<StateValue>> {
        if let Some(v) = self.overlay.get(state_key) {
            return Ok(v.clone());
        }
        self.base.get_state_value(state_key)
    }
}

pub struct StateManager {
    db: std::sync::RwLock<DbReaderWriter>,
    overlay: std::sync::RwLock<VecDeque<(StateKey, Option<Vec<u8>>)>>,
    cached_overlay_view: std::sync::RwLock<OverlayStateView>,
    cached_env: std::sync::RwLock<AptosEnvironment>,
    cached_vm: std::sync::RwLock<AptosVM>,
}

impl StateManager {
    pub fn new(data_dir: &str) -> Result<Self> {
        let root_dir = PathBuf::from(data_dir);
        let db_path = root_dir.join("db");
        std::fs::create_dir_all(&db_path)?;

        let storage_dir_paths = StorageDirPaths::from_path(&db_path);
        let pruner_config = PrunerConfig::default();
        let rocksdb_config = aptos_config::config::RocksdbConfigs::default();
        let db = DbReaderWriter::new(AptosDB::open(
            storage_dir_paths,
            false,
            pruner_config,
            rocksdb_config,
            false,
            0,
            0,
            None,
        )?);

        let latest_ledger_info = db.reader.get_latest_ledger_info().ok();

        // Build initial cached overlay view using current DB reader and empty overlay
        let state = if let Some(li) = latest_ledger_info.as_ref() {
            State::new_at_version(Some(li.ledger_info().version()), StateStorageUsage::zero())
        } else {
            State::new_empty()
        };
        let ledger_state = LedgerState::new(state.clone(), state);
        let base_view = CachedStateView::new(
            StateViewId::Miscellaneous,
            db.reader.clone(),
            ledger_state.latest().clone(),
        )?;
        let cached_overlay_view = OverlayStateView {
            base: base_view,
            overlay: HashMap::new(),
        };

        let env = AptosEnvironment::new(&cached_overlay_view);
        let vm = AptosVM::new(&env, &cached_overlay_view);

        Ok(Self {
            db: std::sync::RwLock::new(db),
            overlay: std::sync::RwLock::new(VecDeque::new()),
            cached_overlay_view: std::sync::RwLock::new(cached_overlay_view),
            cached_env: std::sync::RwLock::new(env),
            cached_vm: std::sync::RwLock::new(vm),
        })
    }

    pub fn from_hashmap(data_dir: &str, map: HashMap<StateKey, Option<Vec<u8>>>) -> Result<Self> {
        let sm = Self::new(data_dir)?;
        {
            let mut overlay = sm.overlay.write().unwrap();
            for (k, v) in map {
                overlay.push_back((k, v));
                if overlay.len() > 100_000 {
                    overlay.pop_front();
                }
            }
        }
        sm.ensure_genesis()?;
        sm.rebuild_cached_runtime()?;
        Ok(sm)
    }

    pub fn from_objects(data_dir: &str, items: Vec<(Vec<u8>, Option<Vec<u8>>)>) -> Result<Self> {
        let sm = Self::new(data_dir)?;
        {
            let mut overlay = sm.overlay.write().unwrap();
            for (key_bytes, v) in items {
                if let Ok(k) = bcs::from_bytes::<StateKey>(&key_bytes) {
                    overlay.push_back((k, v));
                    if overlay.len() > 100_000 {
                        overlay.pop_front();
                    }
                }
            }
        }
        sm.ensure_genesis()?;
        sm.rebuild_cached_runtime()?;
        Ok(sm)
    }

    pub fn db_reader(&self) -> Arc<dyn DbReader> {
        self.db.read().unwrap().reader.clone()
    }

    pub fn overlay_view(&self) -> std::sync::RwLockReadGuard<'_, OverlayStateView> {
        self.cached_overlay_view.read().unwrap()
    }

    pub fn environment(&self) -> std::sync::RwLockReadGuard<'_, AptosEnvironment> {
        self.cached_env.read().unwrap()
    }

    pub fn vm(&self) -> std::sync::RwLockReadGuard<'_, AptosVM> {
        self.cached_vm.read().unwrap()
    }

    pub fn clear_overlay(&self) -> Result<()> {
        let mut overlay = self.overlay.write().unwrap();
        overlay.clear();
        drop(overlay);
        self.rebuild_cached_runtime()
    }

    fn build_overlay_state_view_from_current(&self) -> Result<OverlayStateView> {
        let latest_li_opt = self.db_reader().get_latest_ledger_info_option()?;
        let state = if let Some(li) = latest_li_opt {
            State::new_at_version(Some(li.ledger_info().version()), StateStorageUsage::zero())
        } else {
            State::new_empty()
        };
        let ledger_state = LedgerState::new(state.clone(), state);
        let base_view = CachedStateView::new(
            StateViewId::Miscellaneous,
            self.db_reader(),
            ledger_state.latest().clone(),
        )?;
        let mut map: HashMap<StateKey, Option<StateValue>> = HashMap::new();
        {
            let overlay = self.overlay.read().unwrap();
            for (k, v_opt) in overlay.iter() {
                let entry = v_opt.as_ref().map(|b| StateValue::new_legacy(b.clone().into()));
                map.insert(k.clone(), entry);
            }
        }
        Ok(OverlayStateView {
            base: base_view,
            overlay: map,
        })
    }

    fn rebuild_cached_runtime(&self) -> Result<()> {
        let new_view = self.build_overlay_state_view_from_current()?;
        let new_env = AptosEnvironment::new(&new_view);
        let new_vm = AptosVM::new(&new_env, &new_view);

        {
            let mut guard = self.cached_overlay_view.write().unwrap();
            *guard = new_view;
        }
        {
            let mut guard = self.cached_env.write().unwrap();
            *guard = new_env;
        }
        {
            let mut guard = self.cached_vm.write().unwrap();
            *guard = new_vm;
        }
        Ok(())
    }

    pub fn apply_write_set_to_overlay(&self, write_set: &WriteSet) -> Result<()> {
        let mut overlay = self.overlay.write().unwrap();
        for (state_key, op) in write_set.write_op_iter() {
            let value_opt: Option<Vec<u8>> = op.bytes().map(|bytes| bytes.to_vec());
            overlay.push_back((state_key.clone(), value_opt));
            if overlay.len() > 100_000 {
                overlay.pop_front();
            }
        }
        drop(overlay);
        self.rebuild_cached_runtime()
    }

    pub fn get_state_value(&self, key: &StateKey) -> Result<Option<Vec<u8>>> {
        {
            let overlay = self.overlay.read().unwrap();
            for (k, v_opt) in overlay.iter().rev() {
                if k == key {
                    return Ok(v_opt.clone());
                }
            }
        }
        if let Ok(Some(li)) = self.db_reader().get_latest_ledger_info_option() {
            let version = li.ledger_info().version();
            if let Ok(maybe) = self.db_reader().get_state_value_by_version(key, version) {
                return Ok(maybe.map(|sv| sv.bytes().to_vec()));
            }
        }
        Ok(None)
    }

    pub fn read_state(&self, key: &StateKey) -> Result<Option<Vec<u8>>> {
        self.get_state_value(key)
    }

    pub fn insert_state(&self, key: StateKey, value: Option<Vec<u8>>) -> Result<()> {
        {
            let mut overlay = self.overlay.write().unwrap();
            overlay.push_back((key, value));
            if overlay.len() > 100_000 {
                overlay.pop_front();
            }
        }
        self.rebuild_cached_runtime()
    }

    pub fn insert_states(&self, insert_objects: Vec<(Vec<u8>, Option<Vec<u8>>)>) -> Result<()> {
        {
            let mut overlay = self.overlay.write().unwrap();
            for (key_bytes, value_opt) in insert_objects {
                if let Ok(key) = bcs::from_bytes::<StateKey>(&key_bytes) {
                    overlay.push_back((key, value_opt));
                    if overlay.len() > 100_000 {
                        overlay.pop_front();
                    }
                }
            }
        }
        self.rebuild_cached_runtime()
    }

    pub fn ensure_genesis(&self) -> Result<()> {
        if self
            .db
            .read()
            .unwrap()
            .reader
            .get_latest_ledger_info_option()?
            .is_some()
        {
            return Ok(());
        }
        let genesis_txn: Transaction = test_genesis_transaction();
        let dbrw = self.db.read().unwrap().clone();
        let waypoint = generate_waypoint::<AptosVMBlockExecutor>(&dbrw, &genesis_txn)?;
        maybe_bootstrap::<AptosVMBlockExecutor>(&dbrw, &genesis_txn, waypoint)?;
        self.rebuild_cached_runtime()
    }
}
