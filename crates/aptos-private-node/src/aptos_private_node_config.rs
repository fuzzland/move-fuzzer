use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AptosPrivateNodeConfig {
    pub data_dir: String,
}

impl Default for AptosPrivateNodeConfig {
    fn default() -> Self {
        Self {
            data_dir: "./private-node-data".to_string(),
        }
    }
}

impl AptosPrivateNodeConfig {
    pub fn from_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let contents = fs::read_to_string(path)?;
        let config: AptosPrivateNodeConfig = serde_yaml::from_str(&contents)?;
        Ok(config)
    }
}
