use std::sync::Arc;
use anyhow::Result;
use async_trait::async_trait;
use aptos_types::transaction::SignedTransaction;
use aptos_types::state_store::state_key::StateKey;

pub mod aptos_private_node_config;
pub mod move_executor;
pub mod state_manager;
pub mod transaction_result;

pub use aptos_private_node_config::AptosPrivateNodeConfig;
pub use move_executor::MoveExecutor;
pub use state_manager::StateManager;
pub use transaction_result::TransactionResult;
pub use simulator::Simulator;

pub type BasicTransactionResult = (
    bool,                          // success
    u64,                           // gas_used
    Vec<(Vec<u8>, Option<Vec<u8>>)>, // write_set
    Vec<Vec<u8>>,                  // events
    Option<Vec<u8>>,               // fee_statement_bcs
    u64,                           // cache_misses
);

pub struct AptosPrivateNode {
    state_manager: Arc<StateManager>,
    executor: Arc<MoveExecutor>,
}

impl AptosPrivateNode {
    /// Create a new private node instance (overlay mode; no disk commits).
    pub fn new(data_dir: &str) -> Result<Self> {
        let state_manager = Arc::new(StateManager::new(data_dir)?);
        let executor = Arc::new(MoveExecutor::new(state_manager.clone()));
        Ok(Self {
            state_manager,
            executor,
        })
    }

    pub async fn initialize_from_snapshot(&self, snapshot_path: &str) -> Result<()> {
        self.state_manager.load_readonly_from_dir(snapshot_path)?;
        Ok(())
    }

    

    pub fn state_manager(&self) -> Arc<StateManager> {
        self.state_manager.clone()
    }

    pub fn executor(&self) -> Arc<MoveExecutor> {
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
impl Simulator<Vec<u8>, Vec<u8>, Option<Vec<u8>>, BasicTransactionResult, ()> for AptosPrivateNode {
    async fn simulate(
        &self,
        tx_bytes: Vec<u8>,
        override_objects: Vec<(Vec<u8>, Option<Vec<u8>>)>,
        _tracer: Option<()>,
    ) -> Result<BasicTransactionResult> {
        for (key_bytes, value_opt) in override_objects {
            if let Ok(key) = bcs::from_bytes::<StateKey>(&key_bytes) {
                self.state_manager.insert_state(key, value_opt);
            }
        }
        let tx: SignedTransaction = bcs::from_bytes(&tx_bytes)?;
        let result: TransactionResult = self.executor.execute_transaction_with_overlay(tx).await?;

        let success = matches!(
            result.status,
            aptos_types::transaction::TransactionStatus::Keep(
                aptos_types::transaction::ExecutionStatus::Success
            )
        );

        let mut write_set_pairs: Vec<(Vec<u8>, Option<Vec<u8>>)> = Vec::new();
        for (state_key, op) in result.write_set.write_op_iter() {
            let key_bytes = bcs::to_bytes(state_key)?;
            let val_opt = op.bytes().map(|b| b.to_vec());
            write_set_pairs.push((key_bytes, val_opt));
        }

        let mut events_bytes: Vec<Vec<u8>> = Vec::new();
        for ev in result.events.iter() {
            events_bytes.push(bcs::to_bytes(ev)?);
        }

        let fee_statement_bcs = match &result.fee_statement {
            Some(fs) => Some(bcs::to_bytes(fs)?),
            None => None,
        };

        Ok((
            success,
            result.gas_used,
            write_set_pairs,
            events_bytes,
            fee_statement_bcs,
            result.cache_misses,
        ))
    }

    async fn get_object(&self, object_id_bytes: &Vec<u8>) -> Option<Option<Vec<u8>>> {
        let key: StateKey = bcs::from_bytes(object_id_bytes).ok()?;
        match self.state_manager.read_state(&key) {
            Ok(Some(bytes)) => Some(Some(bytes)),
            Ok(None) => None,
            Err(_) => None,
        }
    }

    async fn multi_get_objects(&self, object_ids_bytes: &[Vec<u8>]) -> Vec<Option<Option<Vec<u8>>>> {
        let mut results = Vec::with_capacity(object_ids_bytes.len());
        for key_bytes in object_ids_bytes {
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

    fn name(&self) -> &str { "AptosPrivateNodeSimulator" }
}
