use anyhow::Result;

use crate::error::FuzzerResult;
use crate::types::CloneableValue;

/// Core trait for mutation strategies
///
/// Each strategy implements a specific algorithm for mutating CloneableValue
/// instances. Strategies are pure functions with no state dependencies, making
/// them easy to test and compose.
pub trait MutationStrategy: Send + Sync {
    /// Apply this strategy to mutate the given value
    fn mutate(&mut self, value: &mut CloneableValue) -> Result<()>;

    /// Check if this strategy can be applied to the given value
    fn can_apply(&self, value: &CloneableValue) -> bool;

    /// Get a description of this strategy (for debugging/logging)
    fn description(&self) -> &'static str;
}

/// Strategy for generating specific types of values rather than mutating
/// existing ones
pub trait GenerativeStrategy: Send + Sync {
    /// Generate a new value of the specified type
    fn generate(&mut self, type_name: &str) -> FuzzerResult<CloneableValue>;

    /// Get available type names this strategy can generate
    fn supported_types(&self) -> &[&'static str];

    /// Get a description of this strategy
    fn description(&self) -> &'static str;
}
