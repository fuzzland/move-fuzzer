use anyhow::Result;

/// Transaction executor trait for different blockchains.
///
/// * `Transaction` - Transaction data
/// * `ObjectID` - Object identifier
/// * `Object` - Object type
/// * `ExecutionResult` - Execution result
/// * `Tracer` - Tracer type (must be Send)
pub trait Executor {
    type Transaction;
    type ObjectID;
    type Object;
    type ExecutionResult;
    type Tracer;

    /// Execute transaction.
    fn execute(
        &self,
        tx: Self::Transaction,
        override_objects: Vec<(Self::ObjectID, Self::Object)>,
        tracer: Option<Self::Tracer>,
    ) -> Result<Self::ExecutionResult>;

    /// Get object by ID.
    fn get_object(&self, object_id: &Self::ObjectID) -> Option<Self::Object>;

    /// Get multiple objects by their IDs.
    fn multi_get_objects(&self, object_ids: &[Self::ObjectID]) -> Vec<Option<Self::Object>>;

    /// Get simulator implementation name.
    fn name(&self) -> &str;
}
