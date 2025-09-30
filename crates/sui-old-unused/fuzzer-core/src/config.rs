use std::time::Duration;

use anyhow::bail;

use crate::types::FuzzerConfig;

/// Configuration utilities for the fuzzer core
impl FuzzerConfig {
    pub fn new(rpc_url: String, package_id: String, module_name: String, function_name: String) -> Self {
        Self {
            rpc_url,
            package_id,
            module_name,
            function_name,
            type_arguments: vec![],
            args: vec![],
            iterations: 1_000_000,
            timeout_seconds: 300,
            sender: None,
        }
    }

    pub fn with_type_arguments(mut self, type_args: Vec<String>) -> Self {
        self.type_arguments = type_args;
        self
    }

    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    pub fn with_iterations(mut self, iterations: u64) -> Self {
        self.iterations = iterations;
        self
    }

    pub fn with_timeout_seconds(mut self, timeout_seconds: u64) -> Self {
        self.timeout_seconds = timeout_seconds;
        self
    }

    pub fn with_sender(mut self, sender: String) -> Self {
        self.sender = Some(sender);
        self
    }

    pub fn timeout_duration(&self) -> Duration {
        Duration::from_secs(self.timeout_seconds)
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        if self.rpc_url.is_empty() {
            bail!("RPC URL cannot be empty");
        }

        if self.package_id.is_empty() {
            bail!("Package ID cannot be empty");
        }

        if self.module_name.is_empty() {
            bail!("Module name cannot be empty");
        }

        if self.function_name.is_empty() {
            bail!("Function name cannot be empty");
        }

        if self.iterations == 0 {
            bail!("Iterations must be greater than 0");
        }

        if self.timeout_seconds == 0 {
            bail!("Timeout must be greater than 0");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder() {
        let config = FuzzerConfig::new(
            "http://localhost:9000".to_string(),
            "0x123".to_string(),
            "test_module".to_string(),
            "test_function".to_string(),
        )
        .with_iterations(5000)
        .with_timeout_seconds(60)
        .with_sender("0xabc".to_string());

        assert_eq!(config.iterations, 5000);
        assert_eq!(config.timeout_seconds, 60);
        assert_eq!(config.sender, Some("0xabc".to_string()));
    }

    #[test]
    fn test_config_validation() {
        let valid_config = FuzzerConfig::new(
            "http://localhost:9000".to_string(),
            "0x123".to_string(),
            "test_module".to_string(),
            "test_function".to_string(),
        );

        assert!(valid_config.validate().is_ok());

        let invalid_config = FuzzerConfig::new(
            "".to_string(),
            "0x123".to_string(),
            "test_module".to_string(),
            "test_function".to_string(),
        );

        assert!(invalid_config.validate().is_err());
    }
}
