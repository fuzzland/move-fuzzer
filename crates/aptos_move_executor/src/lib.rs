use anyhow::Result;
use async_trait::async_trait;

pub mod overlay_state_view;
pub mod state_manager;
pub mod transaction_result;
use aptos_types::state_store::state_key::StateKey;
use aptos_types::state_store::TStateView;
use aptos_types::transaction::SignedTransaction;
use aptos_vm::data_cache::AsMoveResolver;
use aptos_vm_logging::log_schema::AdapterLogSchema;
use aptos_vm_types::module_and_script_storage::AsAptosCodeStorage;
pub use simulator::Simulator;
pub use state_manager::StateManager;
pub use transaction_result::TransactionResult;

pub struct AptosMoveExecutor {
    state_manager: StateManager,
}

impl AptosMoveExecutor {
    /// Create a new executor from an existing StateManager
    pub fn new(state_manager: StateManager) -> Self {
        Self { state_manager }
    }

    /// Execute a single transaction against the cached overlay-backed
    /// StateView.
    pub async fn execute_transaction_with_overlay(&self, transaction: SignedTransaction) -> Result<TransactionResult> {
        // Genesis is already applied during StateManager::new()

        // Borrow cached components
        let (status, gas_used, write_set, events, fee_statement, cache_misses) = {
            let view_guard = self.state_manager.overlay_view();
            let env_guard = self.state_manager.environment();
            let vm_guard = self.state_manager.vm();

            let overlay_view = &*view_guard;
            let env = &*env_guard;
            let vm = &*vm_guard;

            let resolver = overlay_view.as_move_resolver();
            let code_storage = overlay_view.as_aptos_code_storage(env);
            let log_context = AdapterLogSchema::new(overlay_view.id(), 0);
            let aux = aptos_types::transaction::AuxiliaryInfo::new(
                aptos_types::transaction::PersistedAuxiliaryInfo::None,
                None,
            );
            let (_vm_status, vm_output) =
                vm.execute_user_transaction(&resolver, &code_storage, &transaction, &log_context, &aux);

            let txn_output = vm_output
                .try_materialize_into_transaction_output(&resolver)
                .expect("Materializing aggregator deltas should not fail");

            let status = txn_output.status().clone();
            let gas_used = txn_output.gas_used();
            let write_set = txn_output.write_set().clone();
            let events = txn_output.events().to_vec();
            let fee_statement = txn_output.try_extract_fee_statement().ok().flatten();
            let cache_misses: u64 = 0;
            (status, gas_used, write_set, events, fee_statement, cache_misses)
        };

        // Apply overlay updates and rebuild cached runtime
        self.state_manager.apply_write_set_to_overlay(&write_set)?;

        Ok(TransactionResult {
            status,
            gas_used,
            write_set,
            events,
            fee_statement,
            cache_misses,
        })
    }
}

#[async_trait]
impl Simulator<Vec<u8>, Vec<u8>, Option<Vec<u8>>, TransactionResult, ()> for AptosMoveExecutor {
    async fn simulate(
        &self,
        tx_bytes: Vec<u8>,
        override_objects: Vec<(Vec<u8>, Option<Vec<u8>>)>,
        _tracer: Option<()>,
    ) -> Result<TransactionResult> {
        if !override_objects.is_empty() {
            self.state_manager.insert_states(override_objects)?;
        }
        let tx: SignedTransaction = bcs::from_bytes(&tx_bytes)?;
        let result: TransactionResult = self.execute_transaction_with_overlay(tx).await?;
        Ok(result)
    }

    async fn get_object(&self, object_id_bytes: &Vec<u8>) -> Option<Option<Vec<u8>>> {
        let key: StateKey = bcs::from_bytes(object_id_bytes).ok()?;
        match self.state_manager.read_state(&key) {
            Ok(Some(bytes)) => Some(Some(bytes)),
            Ok(None) => None,
            Err(_) => None,
        }
    }

    async fn multi_get_objects(&self, object_ids: &[Vec<u8>]) -> Vec<Option<Option<Vec<u8>>>> {
        let mut results = Vec::with_capacity(object_ids.len());
        for key_bytes in object_ids {
            let item = if let Ok(key) = bcs::from_bytes::<StateKey>(key_bytes) {
                match self.state_manager.read_state(&key) {
                    Ok(Some(bytes)) => Some(Some(bytes)),
                    Ok(None) => None,
                    Err(_) => None,
                }
            } else {
                None
            };
            results.push(item);
        }
        results
    }

    fn name(&self) -> &str {
        "AptosMoveExecutor"
    }
}
