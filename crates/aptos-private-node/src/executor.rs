// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use std::sync::{Arc, Mutex as StdMutex};
use anyhow::Result;
use aptos_crypto::HashValue;
use aptos_types::{
    transaction::{SignedTransaction, AuxiliaryInfo, RawTransaction},
    block_executor::{
        partitioner::ExecutableTransactions,
        config::BlockExecutorConfigFromOnchain,
    },
    state_store::StateViewId,
};
use aptos_vm::AptosVM;
use aptos_vm::aptos_vm::AptosVMBlockExecutor;
use aptos_executor::block_executor::BlockExecutor;
use aptos_executor_types::BlockExecutorTrait;
use aptos_types::block_executor::partitioner::ExecutableBlock;
use aptos_types::transaction::signature_verified_transaction::{SignatureVerifiedTransaction, into_signature_verified_block};
use aptos_storage_interface::state_store::{
    state_view::cached_state_view::CachedStateView,
    state::{LedgerState, State},
};
use aptos_vm_environment::environment::AptosEnvironment;
use crate::state_manager::{StateManager, TransactionResult};
use aptos_types::transaction::authenticator::TransactionAuthenticator;
// Logging schema
use aptos_vm_logging::log_schema::AdapterLogSchema;
// Resolver/code storage helpers
use aptos_vm::data_cache::AsMoveResolver;
use aptos_vm_types::module_and_script_storage::AsAptosCodeStorage;
use aptos_types::state_store::TStateView;
// Overlay-first reads for external consumers; DB remains base.

pub struct TestExecutor {
    state_manager: Arc<StateManager>,
    vm: AptosVM,
    config: BlockExecutorConfigFromOnchain,
    persistent_block_executor: StdMutex<Option<BlockExecutor<AptosVMBlockExecutor>>>,
    last_block_id: StdMutex<Option<HashValue>>,
}

impl TestExecutor {
    /// Create a new test executor
    pub fn new(state_manager: Arc<StateManager>) -> Self {
        // Create a simple dummy state view for VM initialization
        let dummy_state = State::new_empty();
        let dummy_state_view = CachedStateView::new_dummy(&dummy_state);
        let env = AptosEnvironment::new(&dummy_state_view);

        Self {
            state_manager,
            vm: AptosVM::new(&env, &dummy_state_view),
            config: BlockExecutorConfigFromOnchain::new_no_block_limit(),
            persistent_block_executor: StdMutex::new(None),
            last_block_id: StdMutex::new(None),
        }
    }

    /// Execute a single transaction with real VM
    pub async fn execute_transaction(&self, transaction: SignedTransaction) -> Result<TransactionResult> {
        // Ensure we have genesis
        self.state_manager.ensure_genesis()?;

        // Always use a single in-memory executor (no disk persistence path)
        let dbrw = self.state_manager.db();
        let mut guard = self.persistent_block_executor.lock().unwrap();
        if guard.is_none() {
            *guard = Some(BlockExecutor::new(dbrw.clone()));
        }
        let executor_ref: &BlockExecutor<AptosVMBlockExecutor> = guard.as_ref().as_ref().unwrap();

        // Determine parent block id: use last in-memory block when not persisting, else committed
        let parent_block_id = self
            .last_block_id
            .lock()
            .unwrap()
            .unwrap_or_else(|| executor_ref.committed_block_id());

        // Build a minimal block with a single user transaction
        let block_id = HashValue::random();
        let txns: Vec<SignatureVerifiedTransaction> = into_signature_verified_block(vec![aptos_types::transaction::Transaction::UserTransaction(transaction.clone())]);
        let auxiliary_infos = vec![AuxiliaryInfo::new(aptos_types::transaction::PersistedAuxiliaryInfo::None, None)];
        let block: ExecutableBlock = (block_id, txns.clone(), auxiliary_infos).into();

        // Execute and update state using BlockExecutor backed by DB view
        executor_ref.execute_and_update_state(block, parent_block_id, BlockExecutorConfigFromOnchain::new_no_block_limit())?;
        let compute_result = executor_ref.ledger_update(block_id, parent_block_id)?;
        let outputs = &compute_result.execution_output.to_commit.transaction_outputs;

        // Extract outputs to build TransactionResult
        let events = outputs
            .get(0)
            .map(|o| o.events().to_vec())
            .unwrap_or_default();
        let status = outputs
            .get(0)
            .map(|o| o.status().clone())
            .unwrap_or(aptos_types::transaction::TransactionStatus::Discard(aptos_types::vm_status::StatusCode::UNKNOWN_VALIDATION_STATUS));
        let gas_used = outputs.get(0).map(|o| o.gas_used()).unwrap_or(0);
        let write_set = outputs.get(0).map(|o| o.write_set().clone()).unwrap_or_default();

        // Extract fee statement if present from committed output
        let fee_statement = outputs
            .get(0)
            .and_then(|o| o.try_extract_fee_statement().ok().flatten());
        // No simple cache miss metric available from Aptos executor here; default to 0
        let cache_misses: u64 = 0;

        // If disk persistence is disabled, do NOT commit. Keep block in executor's in-memory block tree
        // and apply writes to the overlay for external reads.
        // Apply to overlay for external reads
        self.state_manager.apply_write_set_to_overlay(&write_set);
        *self.last_block_id.lock().unwrap() = Some(block_id);

        Ok(TransactionResult { status, gas_used: gas_used as u64, write_set, events, fee_statement, cache_misses })
    }

    /// Execute a single transaction using an overlay-backed StateView for fuzzing scenarios.
    /// This bypasses the block executor and runs the VM directly against an overlay+DB view.
    pub async fn execute_transaction_with_overlay(&self, transaction: SignedTransaction) -> Result<TransactionResult> {
        // Ensure genesis is initialized
        self.state_manager.ensure_genesis()?;

        // Build overlay-backed view
        let overlay_view = self.state_manager.make_overlay_state_view()?;
        let env = AptosEnvironment::new(&overlay_view);
        let mut vm = AptosVM::new(&env, &overlay_view);
        let log_context = AdapterLogSchema::new(overlay_view.id(), 0);

        // Execute directly via VM
        let resolver = overlay_view.as_move_resolver();
        let code_storage = overlay_view.as_aptos_code_storage(&env);
        let aux = aptos_types::transaction::AuxiliaryInfo::new(
            aptos_types::transaction::PersistedAuxiliaryInfo::None,
            None,
        );
        let (vm_status, vm_output) = vm.execute_user_transaction(
            &resolver,
            &code_storage,
            &transaction,
            &log_context,
            &aux,
        );

        // Materialize TransactionOutput
        let txn_output = vm_output
            .try_materialize_into_transaction_output(&resolver)
            .expect("Materializing aggregator deltas should not fail");

        let status = txn_output.status().clone();
        let gas_used = txn_output.gas_used();
        let write_set = txn_output.write_set().clone();
        let events = txn_output.events().to_vec();
        let fee_statement = txn_output.try_extract_fee_statement().ok().flatten();
        let cache_misses: u64 = 0;

        // Project writes to overlay for subsequent external reads
        self.state_manager.apply_write_set_to_overlay(&write_set);

        Ok(TransactionResult { status, gas_used: gas_used as u64, write_set, events, fee_statement, cache_misses })
    }

    /// Execute a transaction built from a raw transaction plus authenticator (more primitive than SignedTransaction)
    pub async fn execute_raw_transaction(
        &self,
        raw_txn: RawTransaction,
        authenticator: TransactionAuthenticator,
    ) -> Result<TransactionResult> {
        let signed = SignedTransaction::new_signed_transaction(raw_txn, authenticator);
        self.execute_transaction(signed).await
    }

    /// Execute a BCS-encoded SignedTransaction (convenience for external callers)
    pub async fn execute_bcs_signed_transaction(&self, txn_bytes: &[u8]) -> Result<TransactionResult> {
        let signed: SignedTransaction = bcs::from_bytes(txn_bytes)?;
        self.execute_transaction(signed).await
    }

    /// Execute a batch of transactions
    pub async fn execute_transactions(&self, transactions: Vec<SignedTransaction>) -> Result<Vec<TransactionResult>> {

        let mut results = Vec::new();
        for transaction in transactions {
            let result = self.execute_transaction(transaction).await?;
            results.push(result);
        }

        Ok(results)
    }

    /// Execute a block of transactions using block executor
    pub async fn execute_block(&self, transactions: ExecutableTransactions) -> Result<HashValue> {

        match transactions {
            ExecutableTransactions::Unsharded(txns) => {
                let converted_txns: Vec<SignedTransaction> = txns.into_iter().map(|txn| {
                    match txn.into_inner() {
                        aptos_types::transaction::Transaction::UserTransaction(signed_txn) => signed_txn,
                        _ => panic!("Expected user transaction"),
                    }
                }).collect();
                self.execute_unsharded_block(converted_txns).await
            }
            ExecutableTransactions::Sharded(partitioned_txns) => {
                // For now, extract all transactions from partitioned
                let analyzed_txns = aptos_types::block_executor::partitioner::PartitionedTransactions::flatten(partitioned_txns);
                let converted_txns: Vec<SignedTransaction> = analyzed_txns.into_iter().map(|txn| {
                    match txn.expect_p_txn().0.into_inner() {
                        aptos_types::transaction::Transaction::UserTransaction(signed_txn) => signed_txn,
                        _ => panic!("Expected user transaction"),
                    }
                }).collect();
                self.execute_unsharded_block(converted_txns).await
            }
        }
    }

    /// Execute unsharded block
    async fn execute_unsharded_block(&self, transactions: Vec<SignedTransaction>) -> Result<HashValue> {
        // Create state view
        let latest_ledger_info = self.state_manager.db_reader().get_latest_ledger_info_option()?.unwrap();
        let state = State::new_at_version(Some(latest_ledger_info.ledger_info().version()), aptos_types::state_store::state_storage_usage::StateStorageUsage::zero());
        let ledger_state = LedgerState::new(state.clone(), state);

        let _state_view = CachedStateView::new(
            StateViewId::BlockExecution { block_id: HashValue::random() },
            Arc::clone(&self.state_manager.db_reader()),
            ledger_state.latest().clone(),
        )?;

        // Create transaction provider
        let auxiliary_infos = vec![AuxiliaryInfo::new(
            aptos_types::transaction::PersistedAuxiliaryInfo::None,
            None,
        ); transactions.len()];

        // For now, execute transactions individually
        // TODO: Implement proper block execution when block executor API is stable
        let _auxiliary_infos = auxiliary_infos; // Suppress unused warning
        // transactions已经是SignedTransaction类型的Vec，直接使用
        let signed_txns = transactions;
        let results = self.execute_transactions(signed_txns).await?;

        if results.iter().any(|r| r.status != aptos_types::transaction::TransactionStatus::Keep(
            aptos_types::transaction::ExecutionStatus::Success
        )) {
            Err(anyhow::anyhow!("Block execution failed: some transactions failed"))
        } else {
            Ok(HashValue::random())
        }
    }

    /// Get execution configuration
    pub fn get_config(&self) -> &BlockExecutorConfigFromOnchain {
        &self.config
    }

    /// Update execution configuration
    pub fn update_config(&mut self, config: BlockExecutorConfigFromOnchain) {
        self.config = config;
    }

    /// Reset executor state
    pub fn reset(&self) -> Result<()> {
        // In a full implementation, this would reset internal caches
        Ok(())
    }
}

/// Transaction validator
pub struct TransactionValidator {
    state_manager: Arc<StateManager>,
}

impl TransactionValidator {
    /// Create a new transaction validator
    pub fn new(state_manager: Arc<StateManager>) -> Self {
        Self { state_manager }
    }

    /// Validate a transaction with comprehensive checks
    pub fn validate_transaction(&self, transaction: &SignedTransaction) -> Result<()> {

        // 1. Signature verification
        transaction.verify_signature()
            .map_err(|e| anyhow::anyhow!("Signature verification failed: {:?}", e))?;

        // 2. Basic transaction structure validation
        match transaction.payload() {
            aptos_types::transaction::TransactionPayload::Script(_) |
            aptos_types::transaction::TransactionPayload::ModuleBundle(_) |
            aptos_types::transaction::TransactionPayload::EntryFunction(_) |
            aptos_types::transaction::TransactionPayload::Multisig(_) |
            aptos_types::transaction::TransactionPayload::Payload(_) => {
                // Valid payload
            }
        }

        // 3. Gas limits validation
        if transaction.max_gas_amount() == 0 {
            return Err(anyhow::anyhow!("Max gas amount cannot be zero"));
        }

        Ok(())
    }

    /// Validate a batch of transactions
    pub fn validate_transactions(&self, transactions: &[SignedTransaction]) -> Result<()> {
        for txn in transactions {
            self.validate_transaction(txn)?;
        }
        Ok(())
    }
}
