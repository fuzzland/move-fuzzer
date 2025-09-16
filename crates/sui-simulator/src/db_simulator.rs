use std::sync::Arc;

use async_trait::async_trait;
use prometheus::Registry;
use sui_execution::executor::Executor;
use sui_execution::latest::{
    all_natives, execute_transaction_to_effects, execution_mode, new_move_vm, TypeLayoutResolver,
};
use sui_json_rpc::{get_balance_changes_from_effect, ObjectProvider};
use sui_json_rpc_types::{SuiTransactionBlockEffects, SuiTransactionBlockEvents};
use sui_move_trace_format::format::MoveTraceBuilder;
use sui_move_trace_format::interface::Tracer;
use sui_move_vm_runtime::move_vm::MoveVM;
use sui_sdk::{SuiClient, SuiClientBuilder};
use sui_types::base_types::{ObjectID, SequenceNumber, SuiAddress};
use sui_types::committee::EpochId;
use sui_types::digests::TransactionDigest;
use sui_types::effects::TransactionEffects;
use sui_types::error::ExecutionError;
use sui_types::execution::{ExecutionTiming, TypeLayoutStore};
use sui_types::execution_params::ExecutionOrEarlyError;
use sui_types::gas::SuiGasStatus;
use sui_types::inner_temporary_store::InnerTemporaryStore;
use sui_types::layout_resolver::LayoutResolver;
use sui_types::metrics::LimitsMetrics;
use sui_types::object::{Object, Owner};
use sui_types::storage::{BackingPackageStore, BackingStore, ObjectStore};
use sui_types::supported_protocol_versions::{Chain, ProtocolConfig, ProtocolVersion};
use sui_types::transaction::{
    CheckedInputObjects, GasData, InputObjectKind, ObjectReadResult, ObjectReadResultKind, TransactionData,
    TransactionDataAPI, TransactionKind,
};

use crate::rpc_backing_store::RpcBackingStore;
use crate::{EpochInfo, SimulateResult, Simulator, SimulatorError};

/// Custom Executor implementation that uses our empty MoveVM
struct CustomExecutor {
    move_vm: Arc<MoveVM>,
}

impl Executor for CustomExecutor {
    fn execute_transaction_to_effects(
        &self,
        store: &dyn BackingStore,
        protocol_config: &ProtocolConfig,
        metrics: Arc<LimitsMetrics>,
        enable_expensive_checks: bool,
        execution_params: ExecutionOrEarlyError,
        epoch_id: &EpochId,
        epoch_timestamp_ms: u64,
        input_objects: CheckedInputObjects,
        gas: GasData,
        gas_status: SuiGasStatus,
        transaction_kind: TransactionKind,
        transaction_signer: SuiAddress,
        transaction_digest: TransactionDigest,
        trace_builder_opt: &mut Option<MoveTraceBuilder>,
    ) -> (
        InnerTemporaryStore,
        SuiGasStatus,
        TransactionEffects,
        Vec<ExecutionTiming>,
        Result<(), ExecutionError>,
    ) {
        execute_transaction_to_effects::<execution_mode::Normal>(
            store,
            input_objects,
            gas,
            gas_status,
            transaction_kind,
            transaction_signer,
            transaction_digest,
            &self.move_vm,
            epoch_id,
            epoch_timestamp_ms,
            protocol_config,
            metrics,
            enable_expensive_checks,
            execution_params,
            trace_builder_opt,
        )
    }

    fn dev_inspect_transaction(
        &self,
        _store: &dyn sui_types::storage::BackingStore,
        _protocol_config: &ProtocolConfig,
        _metrics: Arc<LimitsMetrics>,
        _enable_expensive_checks: bool,
        _execution_params: ExecutionOrEarlyError,
        _epoch_id: &EpochId,
        _epoch_timestamp_ms: u64,
        _input_objects: CheckedInputObjects,
        _gas: GasData,
        _gas_status: SuiGasStatus,
        _transaction_kind: TransactionKind,
        _transaction_signer: SuiAddress,
        _transaction_digest: TransactionDigest,
        _skip_all_checks: bool,
    ) -> (
        InnerTemporaryStore,
        SuiGasStatus,
        TransactionEffects,
        Result<Vec<sui_types::execution::ExecutionResult>, ExecutionError>,
    ) {
        unimplemented!("dev_inspect_transaction not needed for simulation")
    }

    fn update_genesis_state(
        &self,
        _store: &dyn sui_types::storage::BackingStore,
        _protocol_config: &ProtocolConfig,
        _metrics: Arc<LimitsMetrics>,
        _epoch_id: EpochId,
        _epoch_timestamp_ms: u64,
        _transaction_digest: &TransactionDigest,
        _input_objects: CheckedInputObjects,
        _pt: sui_types::transaction::ProgrammableTransaction,
    ) -> Result<InnerTemporaryStore, ExecutionError> {
        unimplemented!("update_genesis_state not needed for simulation")
    }

    fn type_layout_resolver<'r, 'vm: 'r, 'store: 'r>(
        &'vm self,
        store: Box<dyn TypeLayoutStore + 'store>,
    ) -> Box<dyn LayoutResolver + 'r> {
        Box::new(TypeLayoutResolver::new(&*self.move_vm, store))
    }
}

/// New DBSimulator implementation with lazy RPC loading
pub struct DBSimulator {
    /// Protocol configuration
    protocol_config: ProtocolConfig,
    /// Sui RPC client
    sui_client: Arc<SuiClient>,
    /// RPC backing store
    rpc_store: Arc<RpcBackingStore>,
    /// Executor
    executor: Arc<dyn Executor + Send + Sync>,
    /// Metrics
    metrics: Arc<LimitsMetrics>,
}

impl DBSimulator {
    /// Create a new DBSimulator
    pub async fn new(rpc_url: &str) -> Result<Self, SimulatorError> {
        Self::new_with_protocol_version(rpc_url, None).await
    }

    /// Create a new DBSimulator with specific protocol version
    pub async fn new_with_protocol_version(
        rpc_url: &str,
        protocol_version: Option<ProtocolVersion>,
    ) -> Result<Self, SimulatorError> {
        // Create SuiClient
        let sui_client = Arc::new(
            SuiClientBuilder::default()
                .build(rpc_url)
                .await
                .map_err(|e| SimulatorError::ConfigError(format!("Failed to create Sui client: {:?}", e)))?,
        );

        // Get protocol configuration
        let version = protocol_version.unwrap_or(ProtocolVersion::MAX);
        let protocol_config = ProtocolConfig::get_for_version(version, Chain::Mainnet);

        // Create MoveVM
        let natives = all_natives(
            true, // silent
            &protocol_config,
        );
        let move_vm = Arc::new(
            new_move_vm(natives, &protocol_config, None)
                .map_err(|e| SimulatorError::ConfigError(format!("Failed to create MoveVM: {:?}", e)))?,
        );

        // Create CustomExecutor with our MoveVM
        let executor: Arc<dyn Executor + Send + Sync> = Arc::new(CustomExecutor { move_vm });

        // Create metrics
        let registry = Registry::new();
        let metrics = Arc::new(LimitsMetrics::new(&registry));

        // Create RPC backing store
        let rpc_store = Arc::new(RpcBackingStore::new(sui_client.clone()));

        Ok(Self {
            protocol_config,
            sui_client,
            rpc_store,
            executor,
            metrics,
        })
    }

    /// Get latest epoch info from RPC
    async fn get_latest_epoch(&self) -> Result<EpochInfo, SimulatorError> {
        EpochInfo::get_latest_epoch(self.sui_client.clone())
            .await
            .map_err(|e| SimulatorError::ExecutionError(format!("Failed to get epoch info: {:?}", e)))
    }

    /// Create input objects for a transaction
    fn create_input_objects(
        &self,
        input_objects: &[InputObjectKind],
        _epoch_id: EpochId,
    ) -> Result<CheckedInputObjects, SimulatorError> {
        let mut res: Vec<ObjectReadResult> = Vec::with_capacity(input_objects.len());

        for kind in input_objects {
            match kind {
                InputObjectKind::MovePackage(id) => {
                    let obj = self
                        .rpc_store
                        .get_package_object(id)
                        .map_err(|e| SimulatorError::StorageError(e.to_string()))?
                        .ok_or(SimulatorError::ObjectNotFound(*id))?;
                    res.push(ObjectReadResult {
                        input_object_kind: *kind,
                        object: ObjectReadResultKind::Object(obj.into()),
                    });
                }
                InputObjectKind::SharedMoveObject { id, .. } => {
                    match self.rpc_store.get_object(id) {
                        Some(obj) => res.push(ObjectReadResult::new(*kind, obj.into())),
                        None => {
                            // NOTE: In a full node environment, we would check for consensus stream end
                            // via get_last_consensus_stream_end_info and potentially return
                            // ObjectConsensusStreamEnded. However, this information is not available
                            // through RPC as it requires access to internal node state (Markers).
                            //
                            // In RPC-only environment, we can only determine if the object exists or not.
                            // This is a known limitation when using RPC-based simulation.
                            return Err(SimulatorError::ObjectNotFound(*id));
                        }
                    }
                }
                InputObjectKind::ImmOrOwnedMoveObject((id, version, ..)) => {
                    let obj = self
                        .rpc_store
                        .get_object_by_key(id, *version)
                        .ok_or(SimulatorError::ObjectNotFound(*id))?;
                    res.push(ObjectReadResult {
                        input_object_kind: *kind,
                        object: ObjectReadResultKind::Object(obj),
                    });
                }
            }
        }

        Ok(CheckedInputObjects::new_for_replay(res.into()))
    }

    /// Execute transaction
    fn execute_transaction(
        &self,
        epoch_info: &EpochInfo,
        input_objects: CheckedInputObjects,
        gas_data: sui_types::transaction::GasData,
        gas_status: SuiGasStatus,
        transaction_kind: sui_types::transaction::TransactionKind,
        sender: sui_types::base_types::SuiAddress,
        tx_digest: sui_types::digests::TransactionDigest,
        tracer: Option<Box<dyn Tracer + Send>>,
    ) -> Result<(InnerTemporaryStore, TransactionEffects), SimulatorError> {
        let mut trace_builder = tracer.map(|boxed_tracer| MoveTraceBuilder::new_with_tracer(boxed_tracer));

        // Execute transaction
        let (temporary_store, _gas_status, effects, _timings, execution_result) =
            self.executor.execute_transaction_to_effects(
                self.rpc_store.as_ref(),
                &self.protocol_config,
                self.metrics.clone(),
                false,  // enable_expensive_checks
                Ok(()), // ExecutionOrEarlyError is Result<(), ExecutionErrorKind>
                &epoch_info.epoch_id,
                epoch_info.epoch_start_timestamp,
                input_objects,
                gas_data,
                gas_status,
                transaction_kind,
                sender,
                tx_digest,
                &mut trace_builder,
            );

        // Check execution result
        if let Err(execution_error) = execution_result {
            tracing::warn!("Transaction execution failed: {:?}", execution_error);
        }

        Ok((temporary_store, effects))
    }
}

#[async_trait]
impl Simulator for DBSimulator {
    async fn simulate(
        &self,
        tx_data: TransactionData,
        override_objects: Vec<(ObjectID, Object)>,
        tracer: Option<Box<dyn Tracer + Send>>,
    ) -> Result<SimulateResult, SimulatorError> {
        let tx_digest = tx_data.digest();

        // Get epoch info
        let epoch_info = self.get_latest_epoch().await?;

        // Add override objects to the store
        self.rpc_store.add_overrides(override_objects);

        // Get input objects
        let raw_input_objects = tx_data
            .input_objects()
            .map_err(|e| SimulatorError::InvalidInput(e.to_string()))?;
        let input_objects = self.create_input_objects(&raw_input_objects, epoch_info.epoch_id)?;

        // Save input object kinds for balance change calculation
        let input_objs: Vec<InputObjectKind> = input_objects.inner().object_kinds().cloned().collect();

        // Create gas status
        let gas_status = if tx_data.kind().is_system_tx() {
            SuiGasStatus::new_unmetered()
        } else {
            SuiGasStatus::new(
                tx_data.gas_budget(),
                tx_data.gas_price(),
                epoch_info.gas_price,
                &self.protocol_config,
            )
            .map_err(|e| SimulatorError::ExecutionError(e.to_string()))?
        };

        // Get transaction details before moving tx_data
        let sender = tx_data.sender();
        let gas_data = tx_data.gas_data().clone();
        let transaction_kind = tx_data.into_kind();

        // Execute transaction
        let (temporary_store, effects) = self.execute_transaction(
            &epoch_info,
            input_objects,
            gas_data,
            gas_status,
            transaction_kind,
            sender,
            tx_digest,
            tracer,
        )?;

        // Get object changes
        let object_changes = get_mutated_objects(&effects, &temporary_store);

        // Get balance changes
        let object_provider = ExecutedDB {
            temp_store: &temporary_store,
        };
        let balance_changes = get_balance_changes_from_effect(&object_provider, &effects, input_objs, None)
            .await
            .map_err(|e| SimulatorError::ExecutionError(format!("Failed to get balance changes: {:?}", e)))?;

        // Convert effects
        let effects = SuiTransactionBlockEffects::try_from(effects)
            .map_err(|e| SimulatorError::ExecutionError(format!("Failed to convert effects: {:?}", e)))?;

        // Convert events
        let mut layout_resolver = self.executor.type_layout_resolver(Box::new(self.rpc_store.as_ref()));
        let events = SuiTransactionBlockEvents::try_from(
            temporary_store.events.clone(),
            tx_digest,
            None,
            layout_resolver.as_mut(),
        )
        .map_err(|e| SimulatorError::ExecutionError(format!("Failed to convert events: {:?}", e)))?;

        Ok(SimulateResult {
            effects,
            events,
            object_changes,
            balance_changes,
        })
    }

    async fn get_object(&self, object_id: &ObjectID) -> Option<Object> {
        self.rpc_store.get_object(object_id)
    }

    async fn multi_get_objects(&self, object_ids: &[ObjectID]) -> Vec<Option<Object>> {
        object_ids.iter().map(|id| self.rpc_store.get_object(id)).collect()
    }

    fn name(&self) -> &str {
        "DBSimulator"
    }
}

/// Helper function to get mutated objects from effects
fn get_mutated_objects(effects: &TransactionEffects, store: &InnerTemporaryStore) -> Vec<ObjectReadResult> {
    let mut object_changes = vec![];
    for (obj_ref, owner) in effects.mutated_excluding_gas() {
        if let Some(obj) = store.written.get(&obj_ref.0) {
            let object = ObjectReadResultKind::Object(obj.clone());

            let kind = match owner {
                Owner::Shared { initial_shared_version } => InputObjectKind::SharedMoveObject {
                    id: obj_ref.0,
                    initial_shared_version,
                    mutable: true,
                },
                _ => InputObjectKind::ImmOrOwnedMoveObject(obj_ref),
            };

            object_changes.push(ObjectReadResult::new(kind, object));
        }
    }

    object_changes
}

/// Helper struct for providing objects after execution
struct ExecutedDB<'a> {
    temp_store: &'a InnerTemporaryStore,
}

#[async_trait]
impl<'a> ObjectProvider for ExecutedDB<'a> {
    type Error = SimulatorError;

    async fn get_object(&self, id: &ObjectID, version: &SequenceNumber) -> Result<Object, Self::Error> {
        if let Some(obj) = self.temp_store.input_objects.get(id) {
            if obj.version() == *version {
                return Ok(obj.clone());
            }
        }

        if let Some(obj) = self.temp_store.written.get(id) {
            if obj.version() == *version {
                return Ok(obj.clone());
            }
        }

        Err(SimulatorError::ObjectNotFound(*id))
    }

    async fn find_object_lt_or_eq_version(
        &self,
        id: &ObjectID,
        version: &SequenceNumber,
    ) -> Result<Option<Object>, Self::Error> {
        if let Some(obj) = self.temp_store.input_objects.get(id) {
            if obj.version() <= *version {
                return Ok(Some(obj.clone()));
            }
        }

        if let Some(obj) = self.temp_store.written.get(id) {
            if obj.version() <= *version {
                return Ok(Some(obj.clone()));
            }
        }

        Ok(None)
    }
}
