use std::sync::Arc;

use async_trait::async_trait;
use sui_json_rpc_types::{BalanceChange, SuiTransactionBlockEffects, SuiTransactionBlockEvents};
use sui_move_trace_format::interface::Tracer;
use sui_sdk::SuiClient;
use sui_types::base_types::ObjectID;
use sui_types::committee::EpochId;
use sui_types::messages_checkpoint::CheckpointTimestamp;
use sui_types::object::Object;
use sui_types::sui_system_state::sui_system_state_summary::SuiSystemStateSummary;
use sui_types::transaction::{ObjectReadResult, TransactionData};
use thiserror::Error;

pub mod db_simulator;
pub mod rpc_backing_store;
pub mod rpc_simulator;

// Re-exports for convenience
pub use db_simulator::DBSimulator;
pub use rpc_simulator::RpcSimulator;

// Only required for db simulator (deprecated)
#[derive(Debug, Clone, Copy, Default)]
pub struct EpochInfo {
    pub epoch_id: EpochId,
    pub epoch_start_timestamp: CheckpointTimestamp,
    pub epoch_duration_ms: u64,
    pub gas_price: u64,
}

impl From<SuiSystemStateSummary> for EpochInfo {
    fn from(summary: SuiSystemStateSummary) -> Self {
        Self {
            epoch_id: summary.epoch,
            epoch_start_timestamp: summary.epoch_start_timestamp_ms,
            epoch_duration_ms: summary.epoch_duration_ms,
            gas_price: summary.reference_gas_price,
        }
    }
}

impl EpochInfo {
    pub fn is_stale(&self) -> bool {
        (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64) <
            self.epoch_start_timestamp + self.epoch_duration_ms
    }

    pub async fn get_latest_epoch(sui: Arc<SuiClient>) -> eyre::Result<Self> {
        let sys_state = sui.governance_api().get_latest_sui_system_state().await?;
        Ok(sys_state.into())
    }
}

/// Simulation result containing transaction effects and related information
#[derive(Debug, Clone)]
pub struct SimulateResult {
    pub effects: SuiTransactionBlockEffects,
    pub events: SuiTransactionBlockEvents,
    pub object_changes: Vec<ObjectReadResult>,
    pub balance_changes: Vec<BalanceChange>,
}

/// Errors that can occur during simulation
#[derive(Error, Debug)]
pub enum SimulatorError {
    #[error("Execution error: {0}")]
    ExecutionError(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Protocol error: {0}")]
    ProtocolError(String),

    #[error("Object not found: {0}")]
    ObjectNotFound(ObjectID),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Unknown error: {0}")]
    UnknownError(String),
}

/// Main trait for transaction simulation.
#[async_trait]
pub trait Simulator: Send + Sync {
    /// Simulate execution of a transaction
    ///
    /// # Arguments
    ///
    /// * `tx` - The transaction data to simulate
    /// * `override_objects` - Objects to override in the simulation environment
    /// * `tracer` - Optional tracer for inspecting execution
    ///
    /// # Returns
    ///
    /// Returns a `SimulateResult` containing the execution effects and related
    /// information.
    ///
    /// # Errors
    ///
    /// Returns `SimulatorError` if the simulation fails.
    async fn simulate(
        &self,
        tx: TransactionData,
        override_objects: Vec<(ObjectID, Object)>,
        tracer: Option<Box<dyn Tracer + Send>>,
    ) -> Result<SimulateResult, SimulatorError>;

    /// Get an object by its ID
    ///
    /// # Arguments
    ///
    /// * `object_id` - The ID of the object to retrieve
    ///
    /// # Returns
    ///
    /// Returns the object if found, or `None` if not found.
    async fn get_object(&self, object_id: &ObjectID) -> Option<Object>;

    /// Get multiple objects by their IDs
    ///
    /// # Arguments
    ///
    /// * `object_ids` - The IDs of the objects to retrieve
    ///
    /// # Returns
    ///
    /// Returns a vector of optional objects in the same order as the input IDs.
    async fn multi_get_objects(&self, object_ids: &[ObjectID]) -> Vec<Option<Object>>;

    /// Get the name of this simulator implementation
    fn name(&self) -> &str;
}
