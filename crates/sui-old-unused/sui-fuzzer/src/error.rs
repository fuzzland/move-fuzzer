// Error types for sui-fuzzer

use thiserror::Error;

/// Result type for fuzzer operations
pub type FuzzerResult<T> = Result<T, FuzzerError>;

/// Error types that can occur during fuzzing
#[derive(Error, Debug)]
pub enum FuzzerError {
    #[error("Initialization failed: {0}")]
    InitializationFailed(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Conversion error: {0}")]
    ConversionError(String),

    #[error("Mutation failed: {0}")]
    MutationFailed(String),

    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    #[error("Type error: {0}")]
    TypeError(String),

    #[error("Other error: {0}")]
    Other(String),
}

impl From<anyhow::Error> for FuzzerError {
    fn from(err: anyhow::Error) -> Self {
        FuzzerError::Other(err.to_string())
    }
}
