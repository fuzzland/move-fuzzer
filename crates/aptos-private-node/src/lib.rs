// Copyright © Aptos Foundation
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;
use anyhow::Result;
use async_trait::async_trait;
use aptos_types::transaction::SignedTransaction;
use aptos_types::transaction::{RawTransaction};
use aptos_types::transaction::authenticator::TransactionAuthenticator;

pub mod config;
pub mod executor;
pub mod snapshot;
pub mod state_manager;

pub use config::AptosPrivateNodeConfig;
pub use executor::{TestExecutor, TransactionValidator};
pub use snapshot::{SnapshotManager, SnapshotMetadata};
pub use state_manager::{StateManager, StateSummary, TransactionResult};
pub use simulator::Simulator;

pub struct AptosPrivateNode {
    state_manager: Arc<StateManager>,
    executor: Arc<TestExecutor>,
}

impl AptosPrivateNode {
    /// Create a new private node instance (overlay mode; no disk commits).
    pub fn new(data_dir: &str) -> Result<Self> {
        let state_manager = Arc::new(StateManager::new(data_dir)?);
        let executor = Arc::new(TestExecutor::new(state_manager.clone()));
        Ok(Self {
            state_manager,
            executor,
        })
    }

    // removed new_with_persistence (disk persistence is no longer supported)

    pub async fn initialize_from_genesis(&self) -> Result<()> {
        self.state_manager.ensure_genesis()?;
        Ok(())
    }

    // RPC fork is not supported in this crate. See README for rationale.

    pub async fn initialize_from_snapshot(&self, snapshot_path: &str) -> Result<()> {
        self.state_manager.load_readonly_from_dir(snapshot_path)?;
        Ok(())
    }

    pub fn get_state_summary(&self) -> Result<StateSummary> {
        self.state_manager.get_state_summary()
    }



    pub async fn execute_transaction(&self, transaction: SignedTransaction) -> Result<TransactionResult> {
        self.executor.execute_transaction(transaction).await
    }

    pub async fn execute_raw_transaction(&self, raw_txn: RawTransaction, authenticator: TransactionAuthenticator) -> Result<TransactionResult> {
        self.executor.execute_raw_transaction(raw_txn, authenticator).await
    }

    pub async fn execute_bcs_signed_transaction(&self, txn_bytes: &[u8]) -> Result<TransactionResult> {
        self.executor.execute_bcs_signed_transaction(txn_bytes).await
    }

    pub fn validate_transaction(&self, transaction: &SignedTransaction) -> Result<()> {
        let validator = TransactionValidator::new(self.state_manager.clone());
        validator.validate_transaction(transaction)
    }

    pub fn state_manager(&self) -> Arc<StateManager> {
        self.state_manager.clone()
    }

    pub fn executor(&self) -> Arc<TestExecutor> {
        self.executor.clone()
    }
}

pub struct AptosPrivateNodeBuilder {
    data_dir: String,
}

impl AptosPrivateNodeBuilder {
    pub fn new() -> Self {
        Self {
            data_dir: "./private-node-data".to_string(),
        }
    }

    pub fn with_data_dir(mut self, data_dir: &str) -> Self {
        self.data_dir = data_dir.to_string();
        self
    }

    pub fn build(self) -> Result<AptosPrivateNode> {
        AptosPrivateNode::new(&self.data_dir)
    }
}

impl Default for AptosPrivateNodeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Simulator<
    aptos_types::transaction::SignedTransaction,
    aptos_types::state_store::state_key::StateKey,
    Option<Vec<u8>>,
    TransactionResult,
    (),
> for AptosPrivateNode {
    async fn simulate(
        &self,
        tx: aptos_types::transaction::SignedTransaction,
        override_objects: Vec<(
            aptos_types::state_store::state_key::StateKey,
            Option<Vec<u8>>,
        )>,
        _tracer: Option<()>,
    ) -> Result<TransactionResult> {
        // Apply overrides into the in-memory overlay (latest write wins)
        for (key, value_opt) in override_objects {
            self.state_manager.insert_state(key, value_opt);
        }

        // Execute against overlay-backed StateView so overrides take effect
        self.executor.execute_transaction_with_overlay(tx).await
    }

    async fn get_object(
        &self,
        object_id: &aptos_types::state_store::state_key::StateKey,
    ) -> Option<Option<Vec<u8>>> {
        match self.state_manager.read_state(object_id) {
            Ok(maybe_bytes) => maybe_bytes.map(Some),
            Err(_) => None,
        }
    }

    async fn multi_get_objects(
        &self,
        object_ids: &[aptos_types::state_store::state_key::StateKey],
    ) -> Vec<Option<Option<Vec<u8>>>> {
        let mut results = Vec::with_capacity(object_ids.len());
        for key in object_ids {
            let item = match self.state_manager.read_state(key) {
                Ok(maybe_bytes) => maybe_bytes.map(Some),
                Err(_) => None,
            };
            results.push(item);
        }
        results
    }

    fn name(&self) -> &str {
        "AptosPrivateNodeSimulator"
    }
}
