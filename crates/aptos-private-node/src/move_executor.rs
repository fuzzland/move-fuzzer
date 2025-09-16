use std::sync::Arc;
use anyhow::Result;
use aptos_types::transaction::SignedTransaction;
use aptos_vm::AptosVM;
use aptos_vm_environment::environment::AptosEnvironment;
use crate::state_manager::{StateManager, TransactionResult};
// Logging schema
use aptos_vm_logging::log_schema::AdapterLogSchema;
// Resolver/code storage helpers
use aptos_vm::data_cache::AsMoveResolver;
use aptos_vm_types::module_and_script_storage::AsAptosCodeStorage;
use aptos_types::state_store::TStateView;
// Overlay-first reads for external consumers; DB remains base.

pub struct MoveExecutor {
    state_manager: Arc<StateManager>,
}

impl MoveExecutor {
    /// Create a new test executor
    pub fn new(state_manager: Arc<StateManager>) -> Self {
        Self { state_manager }
    }

    /// Execute a single transaction against an overlay-backed StateView.
    pub async fn execute_transaction_with_overlay(&self, transaction: SignedTransaction) -> Result<TransactionResult> {
        self.state_manager.ensure_genesis()?;

        let overlay_view = self.state_manager.make_overlay_state_view()?;
        let env = AptosEnvironment::new(&overlay_view);
        let vm = AptosVM::new(&env, &overlay_view);
        let log_context = AdapterLogSchema::new(overlay_view.id(), 0);

        let resolver = overlay_view.as_move_resolver();
        let code_storage = overlay_view.as_aptos_code_storage(&env);
        let aux = aptos_types::transaction::AuxiliaryInfo::new(
            aptos_types::transaction::PersistedAuxiliaryInfo::None,
            None,
        );
        let (_vm_status, vm_output) = vm.execute_user_transaction(
            &resolver,
            &code_storage,
            &transaction,
            &log_context,
            &aux,
        );

        let txn_output = vm_output
            .try_materialize_into_transaction_output(&resolver)
            .expect("Materializing aggregator deltas should not fail");

        let status = txn_output.status().clone();
        let gas_used = txn_output.gas_used();
        let write_set = txn_output.write_set().clone();
        let events = txn_output.events().to_vec();
        let fee_statement = txn_output.try_extract_fee_statement().ok().flatten();
        let cache_misses: u64 = 0;

        self.state_manager.apply_write_set_to_overlay(&write_set);

        Ok(TransactionResult { status, gas_used, write_set, events, fee_statement, cache_misses })
    }

}
