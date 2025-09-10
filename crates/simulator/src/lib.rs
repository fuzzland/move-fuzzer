use anyhow::Result;
use async_trait::async_trait;

/// Transaction simulation trait for different blockchains.
///
/// Generic over blockchain-specific types:
/// * `Tx` - Transaction data
/// * `Id` - Object identifier
/// * `Obj` - Object type
/// * `R` - Simulation result
/// * `T` - Tracer type (must be Send)
#[async_trait]
pub trait Simulator<Tx, Id, Obj, R, T>: Send + Sync
where
    T: Send,
{
    /// Simulate transaction execution.
    async fn simulate(&self, tx: Tx, override_objects: Vec<(Id, Obj)>, tracer: Option<T>) -> Result<R>;

    /// Get object by ID.
    async fn get_object(&self, object_id: &Id) -> Option<Obj>;

    /// Get multiple objects by their IDs.
    async fn multi_get_objects(&self, object_ids: &[Id]) -> Vec<Option<Obj>>;

    /// Get simulator implementation name.
    fn name(&self) -> &str;
}
