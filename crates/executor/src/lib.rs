use anyhow::Result;

/// Transaction executor trait for different blockchains.
///
/// Generic over blockchain-specific types:
/// * `Tx` - Transaction data
/// * `Id` - Object identifier
/// * `Obj` - Object type
/// * `R` - Execution result
/// * `T` - Tracer type (must be Send)
pub trait Executor<Tx, Id, Obj, R, T>: Send + Sync
where
    T: Send,
{
    /// Execute transaction.
    fn execute(&self, tx: Tx, override_objects: Vec<(Id, Obj)>, tracer: Option<T>) -> Result<R>;

    /// Get object by ID.
    fn get_object(&self, object_id: &Id) -> Option<Obj>;

    /// Get multiple objects by their IDs.
    fn multi_get_objects(&self, object_ids: &[Id]) -> Vec<Option<Obj>>;

    /// Get simulator implementation name.
    fn name(&self) -> &str;
}
