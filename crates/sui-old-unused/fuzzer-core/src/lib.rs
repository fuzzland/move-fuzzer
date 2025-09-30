pub mod cache;
pub mod config;
pub mod fuzzer;
pub mod reporter;
pub mod types;

use std::fmt::Debug;
use std::hash::Hash;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
pub use types::*;

/// Core trait for blockchain-specific value types
pub trait ChainValue: Clone + Debug + Send + Sync + Serialize + for<'de> Deserialize<'de> {
    /// Check if this value is an integer type (for mutation strategies)
    fn is_integer(&self) -> bool;

    /// Check if this value is a vector containing only integers (for mutation
    /// optimization)
    fn is_integer_vector(&self) -> bool;

    /// Check if this value contains integers (recursive check for struct
    /// mutation strategies)
    fn contains_integers(&self) -> bool;

    /// Check if this value represents a mutable object (for caching)
    fn is_mutable_object(&self) -> bool;

    /// Get the object ID if this value represents an object reference
    fn get_object_id(&self) -> Option<Vec<u8>>;

    /// Get the type name for debugging/logging
    fn type_name(&self) -> &'static str;
}

/// Core trait for mutation strategies
pub trait ChainMutationStrategy<V: ChainValue>: Send + Sync {
    /// Apply mutation to the given value
    fn mutate(&mut self, value: &mut V) -> Result<()>;
}

/// Core abstraction trait for blockchain adapters
#[async_trait]
pub trait ChainAdapter: Sized {
    /// Blockchain-specific value type (e.g., CloneableValue for Sui)
    type Value: ChainValue;

    /// Blockchain-specific address type
    type Address: Clone + Debug + Send + Sync;

    /// Blockchain-specific object identifier type
    type ObjectId: Clone + Debug + Send + Sync + Hash + Eq;

    /// Blockchain-specific object type
    type Object: Clone + Debug + Send + Sync;

    /// Blockchain-specific execution result type
    type ExecutionResult: Clone + Debug + Send + Sync;

    /// Blockchain-specific mutation strategy type
    type Mutator: ChainMutationStrategy<Self::Value>;

    // === Initialization Interface ===

    /// Create a chain-specific mutation strategy
    fn create_mutator(&self) -> Self::Mutator;

    /// Resolve function information from the given configuration
    async fn resolve_function(&self, config: &FuzzerConfig) -> Result<FunctionInfo>;

    /// Initialize function parameters from the given arguments
    async fn initialize_parameters(
        &self,
        function: &FunctionInfo,
        args: &[String],
    ) -> Result<Vec<Parameter<Self::Value>>>;

    // === Execution Interface ===

    /// Execute a function with the given parameters
    async fn execute(
        &self,
        sender: &Self::Address,
        function: &FunctionInfo,
        params: &[Parameter<Self::Value>],
    ) -> Result<Self::ExecutionResult>;

    // === Object Management Interface ===

    /// Compute the digest of an object
    fn compute_object_digest(&self, object: &Self::Object) -> Vec<u8>;

    /// Update a value with a cached object
    fn update_value_with_cached_object(&self, value: &mut Self::Value, object: &Self::Object) -> Result<()>;

    /// Convert bytes to ObjectId
    fn bytes_to_object_id(&self, bytes: &[u8]) -> Result<Self::ObjectId>;

    /// Convert ObjectId to bytes
    fn object_id_to_bytes(&self, id: &Self::ObjectId) -> Vec<u8>;

    // === Result Analysis Interface ===

    /// Check if the execution result contains shift violations
    fn has_shift_violations(&self, result: &Self::ExecutionResult) -> bool;

    /// Extract violation information from the execution result
    fn extract_violations(&self, result: &Self::ExecutionResult) -> Vec<ViolationInfo>;

    /// Extract object changes from the execution result for cache updates
    fn extract_object_changes(&self, result: &Self::ExecutionResult)
        -> Vec<ObjectChange<Self::ObjectId, Self::Object>>;

    /// Get the sender address from the configuration
    fn get_sender_from_config(&self, config: &FuzzerConfig) -> Self::Address;
}
