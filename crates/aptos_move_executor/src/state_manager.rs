use std::collections::HashMap;

use anyhow::Result;
use aptos_storage_interface::state_store::state::{LedgerState, State};
use aptos_storage_interface::state_store::state_view::cached_state_view::CachedStateView;
use aptos_types::state_store::state_key::StateKey;
use aptos_types::state_store::state_value::StateValue;
use aptos_types::state_store::TStateView;
use aptos_types::transaction::{Transaction, WriteSetPayload};
use aptos_types::write_set::WriteSet;
use aptos_vm::AptosVM;
use aptos_vm_environment::environment::AptosEnvironment;
use aptos_vm_genesis::test_genesis_transaction;

use crate::overlay_state_view::OverlayStateView;
pub use crate::transaction_result::TransactionResult;

pub struct StateManager {
    cached_overlay_view: std::sync::RwLock<OverlayStateView>,
    cached_env: std::sync::RwLock<AptosEnvironment>,
    cached_vm: std::sync::RwLock<AptosVM>,
}

impl StateManager {
    pub fn new() -> Result<Self> {
        // Start with an empty in-memory state and a dummy base view (no disk)
        let state = State::new_empty();
        let ledger_state = LedgerState::new(state.clone(), state);
        let base_view = CachedStateView::new_dummy(ledger_state.latest());
        let mut cached_overlay_view = OverlayStateView {
            base: base_view,
            overlay: HashMap::new(),
        };

        // Apply genesis immediately during construction
        let genesis_txn: Transaction = test_genesis_transaction();
        if let Transaction::GenesisTransaction(write_set_payload) = genesis_txn {
            match write_set_payload {
                WriteSetPayload::Direct(change_set) => {
                    let (write_set, _events) = change_set.into_inner();
                    // Apply genesis writes directly to the overlay
                    for (state_key, op) in write_set.write_op_iter() {
                        let value_opt: Option<StateValue> =
                            op.bytes().map(|bytes| StateValue::new_legacy(bytes.to_vec().into()));
                        cached_overlay_view.overlay.insert(state_key.clone(), value_opt);
                    }
                }
                WriteSetPayload::Script { .. } => {
                    panic!("Genesis transaction should be a direct write set");
                }
            }
        }

        let env = AptosEnvironment::new(&cached_overlay_view);
        let vm = AptosVM::new(&env, &cached_overlay_view);

        Ok(Self {
            cached_overlay_view: std::sync::RwLock::new(cached_overlay_view),
            cached_env: std::sync::RwLock::new(env),
            cached_vm: std::sync::RwLock::new(vm),
        })
    }

    pub fn from_hashmap(map: HashMap<StateKey, Option<Vec<u8>>>) -> Result<Self> {
        let sm = Self::new()?;
        {
            let mut view = sm.cached_overlay_view.write().unwrap();
            for (k, v) in map {
                let entry = v.as_ref().map(|b| StateValue::new_legacy(b.clone().into()));
                view.overlay.insert(k, entry);
            }
        }
        sm.rebuild_cached_runtime()?;
        Ok(sm)
    }

    pub fn from_objects(items: Vec<(Vec<u8>, Option<Vec<u8>>)>) -> Result<Self> {
        let sm = Self::new()?;
        {
            let mut view = sm.cached_overlay_view.write().unwrap();
            for (key_bytes, v) in items {
                if let Ok(k) = bcs::from_bytes::<StateKey>(&key_bytes) {
                    let entry = v.as_ref().map(|b| StateValue::new_legacy(b.clone().into()));
                    view.overlay.insert(k, entry);
                }
            }
        }
        sm.rebuild_cached_runtime()?;
        Ok(sm)
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
        {
            let mut view = self.cached_overlay_view.write().unwrap();
            view.overlay.clear();
        }
        self.rebuild_cached_runtime()
    }

    pub fn apply_write_set_to_overlay(&self, write_set: &WriteSet) -> Result<()> {
        {
            let mut view = self.cached_overlay_view.write().unwrap();
            for (state_key, op) in write_set.write_op_iter() {
                let value_opt: Option<StateValue> =
                    op.bytes().map(|bytes| StateValue::new_legacy(bytes.to_vec().into()));
                view.overlay.insert(state_key.clone(), value_opt);
            }
        }
        self.rebuild_cached_runtime()
    }

    pub fn get_state_value(&self, key: &StateKey) -> Result<Option<Vec<u8>>> {
        let view = self.cached_overlay_view.read().unwrap();
        if let Some(v) = view.overlay.get(key) {
            return Ok(v.as_ref().map(|sv| sv.bytes().to_vec()));
        }
        // Fallback to base view
        match view.base.get_state_value(key) {
            Ok(maybe) => Ok(maybe.map(|sv| sv.bytes().to_vec())),
            Err(_) => Ok(None),
        }
    }

    pub fn read_state(&self, key: &StateKey) -> Result<Option<Vec<u8>>> {
        self.get_state_value(key)
    }

    pub fn insert_state(&self, key: StateKey, value: Option<Vec<u8>>) -> Result<()> {
        {
            let mut view = self.cached_overlay_view.write().unwrap();
            let entry = value.as_ref().map(|b| StateValue::new_legacy(b.clone().into()));
            view.overlay.insert(key, entry);
        }
        self.rebuild_cached_runtime()
    }

    pub fn insert_states(&self, insert_objects: Vec<(Vec<u8>, Option<Vec<u8>>)>) -> Result<()> {
        {
            let mut view = self.cached_overlay_view.write().unwrap();
            for (key_bytes, value_opt) in insert_objects {
                if let Ok(key) = bcs::from_bytes::<StateKey>(&key_bytes) {
                    let entry = value_opt.as_ref().map(|b| StateValue::new_legacy(b.clone().into()));
                    view.overlay.insert(key, entry);
                }
            }
        }
        self.rebuild_cached_runtime()
    }
}

impl StateManager {
    pub fn rebuild_cached_runtime(&self) -> Result<()> {
        // Recreate environment and VM to reflect latest overlay/configs
        let new_env;
        let new_vm;
        {
            let view_guard = self.cached_overlay_view.read().unwrap();
            new_env = AptosEnvironment::new(&*view_guard);
            new_vm = AptosVM::new(&new_env, &*view_guard);
        }
        {
            let mut env_guard = self.cached_env.write().unwrap();
            *env_guard = new_env;
        }
        {
            let mut vm_guard = self.cached_vm.write().unwrap();
            *vm_guard = new_vm;
        }
        Ok(())
    }
}
