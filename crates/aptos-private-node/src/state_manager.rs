use std::sync::Arc;
use std::collections::{VecDeque, HashMap};
use std::path::PathBuf;
use anyhow::Result;
use aptos_crypto::HashValue;
use aptos_types::{
    account_address::AccountAddress,
    ledger_info::LedgerInfoWithSignatures,
    transaction::{SignedTransaction, TransactionStatus},
    write_set::WriteSet,
    contract_event::ContractEvent,
};
use aptos_types::fee_statement::FeeStatement;
use aptos_storage_interface::{DbReader, DbReaderWriter};
use aptos_db::AptosDB;
use aptos_config::config::StorageDirPaths;
use aptos_config::config::PrunerConfig;
use aptos_executor::db_bootstrapper::{generate_waypoint, maybe_bootstrap};
use aptos_types::transaction::Transaction;
use aptos_vm::aptos_vm::AptosVMBlockExecutor;
use aptos_vm_genesis::test_genesis_transaction;
use aptos_types::state_store::{state_key::StateKey, StateViewId};
use aptos_storage_interface::state_store::{state_view::cached_state_view::CachedStateView, state::{LedgerState, State}};
use aptos_types::state_store::{state_value::StateValue, state_storage_usage::StateStorageUsage, TStateView};

#[derive(Debug, Clone)]
pub struct StateSummary {
    pub version: u64,
    pub block_height: u64,
    pub root_hash: HashValue,
    pub timestamp_usecs: u64,
}

pub struct OverlayStateView {
    base: CachedStateView,
    overlay: HashMap<StateKey, Option<StateValue>>,
}

impl TStateView for OverlayStateView {
    type Key = StateKey;

    fn id(&self) -> StateViewId { StateViewId::Miscellaneous }

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

#[derive(Debug, Clone)]
pub struct TransactionResult {
    pub status: TransactionStatus,
    pub gas_used: u64,
    pub write_set: WriteSet,
    pub events: Vec<ContractEvent>,
    pub fee_statement: Option<FeeStatement>,
    pub cache_misses: u64,
}

pub struct StateManager {
    db: std::sync::RwLock<DbReaderWriter>,
    latest_ledger_info: std::sync::RwLock<Option<LedgerInfoWithSignatures>>,
    overlay: std::sync::RwLock<VecDeque<(StateKey, Option<Vec<u8>>)>>,
    data_dir: PathBuf,
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
            None
        )?);

        let latest_ledger_info = db.reader.get_latest_ledger_info().ok();

        Ok(Self {
            db: std::sync::RwLock::new(db),
            latest_ledger_info: std::sync::RwLock::new(latest_ledger_info),
            overlay: std::sync::RwLock::new(VecDeque::new()),
            data_dir: root_dir,
        })
    }

    pub fn get_state_summary(&self) -> Result<StateSummary> {
        let ledger_info = self.db.read().unwrap().reader.get_latest_ledger_info()?;
        let info = ledger_info.ledger_info();
        Ok(StateSummary {
            version: info.commit_info().version(),
            block_height: info.commit_info().round(),
            root_hash: info.commit_info().executed_state_id(),
            timestamp_usecs: info.commit_info().timestamp_usecs(),
        })
    }

    #[deprecated(note = "StateManager::execute_transaction returns a placeholder result; use TestExecutor::execute_transaction instead")]
    pub fn execute_transaction(&self, transaction: SignedTransaction) -> Result<TransactionResult> {
        let current_ledger_info = self.db.read().unwrap().reader.get_latest_ledger_info()
            .unwrap_or_else(|_| {
                use aptos_types::ledger_info::{LedgerInfo, LedgerInfoWithSignatures};
                use aptos_types::block_info::BlockInfo;
                use aptos_types::aggregate_signature::AggregateSignature;
                let genesis_block_info = BlockInfo::new(0, 0, HashValue::zero(), HashValue::zero(), 0, 0, None);
                let genesis_li = LedgerInfo::new(genesis_block_info, HashValue::zero());
                let dummy_signature = AggregateSignature::empty();
                LedgerInfoWithSignatures::new(genesis_li, dummy_signature)
            });
        let _new_version = current_ledger_info.ledger_info().version() + 1;
        let result = TransactionResult {
            status: aptos_types::transaction::TransactionStatus::Keep(
                aptos_types::transaction::ExecutionStatus::Success
            ),
            gas_used: 1000,
            write_set: aptos_types::write_set::WriteSet::default(),
            events: vec![],
            fee_statement: None,
            cache_misses: 0,
        };
        Ok(result)
    }

    pub fn execute_transactions(&self, transactions: Vec<SignedTransaction>) -> Result<Vec<TransactionResult>> {
        let mut results = Vec::new();
        for txn in transactions {
            let result = self.execute_transaction(txn)?;
            results.push(result);
        }
        Ok(results)
    }

    pub fn create_snapshot(&self, snapshot_path: &str) -> Result<()> {
        std::fs::write(snapshot_path, "test-snapshot")?;
        Ok(())
    }

    pub fn db_reader(&self) -> Arc<dyn DbReader> {
        self.db.read().unwrap().reader.clone()
    }

    pub fn db(&self) -> DbReaderWriter {
        self.db.read().unwrap().clone()
    }

    pub fn data_dir(&self) -> PathBuf {
        self.data_dir.clone()
    }

    pub fn clear_overlay(&self) {
        let mut overlay = self.overlay.write().unwrap();
        overlay.clear();
    }

    pub fn reload_db_readwrite(&self) -> Result<()> {
        let db_path = self.data_dir.join("db");
        let storage_dir_paths = StorageDirPaths::from_path(&db_path);
        let pruner_config = PrunerConfig::default();
        let rocksdb_config = aptos_config::config::RocksdbConfigs::default();
        let new_db = DbReaderWriter::new(AptosDB::open(
            storage_dir_paths,
            false,
            pruner_config,
            rocksdb_config,
            false,
            0,
            0,
            None,
        )?);
        {
            let mut guard = self.db.write().unwrap();
            *guard = new_db;
        }
        let latest_ledger_info = self.db_reader().get_latest_ledger_info().ok();
        *self.latest_ledger_info.write().unwrap() = latest_ledger_info;
        Ok(())
    }

    pub fn make_overlay_state_view(&self) -> Result<OverlayStateView> {
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
        Ok(OverlayStateView { base: base_view, overlay: map })
    }

    pub fn apply_write_set_to_overlay(&self, write_set: &WriteSet) {
        let mut overlay = self.overlay.write().unwrap();
        for (state_key, op) in write_set.write_op_iter() {
            let value_opt: Option<Vec<u8>> = op
                .bytes()
                .map(|bytes| bytes.to_vec());
            overlay.push_back((state_key.clone(), value_opt));
            if overlay.len() > 100_000 {
                overlay.pop_front();
            }
        }
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

    pub fn overlay_insert(&self, key: StateKey, value: Option<Vec<u8>>) {
        let mut overlay = self.overlay.write().unwrap();
        overlay.push_back((key, value));
        if overlay.len() > 100_000 {
            overlay.pop_front();
        }
    }

    pub fn insert_state(&self, key: StateKey, value: Option<Vec<u8>>) {
        self.overlay_insert(key, value);
    }

    pub fn ensure_genesis(&self) -> Result<()> {
        if self.db.read().unwrap().reader.get_latest_ledger_info_option()?.is_some() {
            return Ok(());
        }
        let genesis_txn: Transaction = test_genesis_transaction();
        let dbrw = self.db.read().unwrap().clone();
        let waypoint = generate_waypoint::<AptosVMBlockExecutor>(&dbrw, &genesis_txn)?;
        maybe_bootstrap::<AptosVMBlockExecutor>(&dbrw, &genesis_txn, waypoint)?;
        Ok(())
    }

    pub fn load_readonly_from_dir(&self, state_dir: &str) -> Result<()> {
        let storage_dir_paths = StorageDirPaths::from_path(state_dir);
        let pruner_config = PrunerConfig::default();
        let rocksdb_config = aptos_config::config::RocksdbConfigs::default();
        let new_db = DbReaderWriter::new(AptosDB::open(
            storage_dir_paths,
            true,
            pruner_config,
            rocksdb_config,
            false,
            0,
            0,
            None,
        )?);
        {
            let mut guard = self.db.write().unwrap();
            *guard = new_db;
        }
        let latest_ledger_info = self.db_reader().get_latest_ledger_info().ok();
        *self.latest_ledger_info.write().unwrap() = latest_ledger_info;
        Ok(())
    }
}

