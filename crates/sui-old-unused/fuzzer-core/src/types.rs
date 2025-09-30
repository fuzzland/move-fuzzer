use serde::{Deserialize, Serialize};

use crate::ChainValue;

/// Generic function parameter using blockchain-specific value types
#[derive(Debug, Clone, Serialize)]
#[serde(bound = "")]
pub struct Parameter<V: ChainValue> {
    pub index: usize,
    pub name: String,
    pub type_name: String,
    pub value: V,
}

impl<V: ChainValue> Parameter<V> {
    pub fn type_name(&self) -> &str {
        self.value.type_name()
    }

    pub fn is_integer(&self) -> bool {
        self.value.is_integer()
    }

    pub fn is_mutable_object(&self) -> bool {
        self.value.is_mutable_object()
    }
}

/// Generic function info
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionInfo {
    pub package_id: String,
    pub module_name: String,
    pub function_name: String,
    pub type_arguments: Vec<String>,
}

/// Violation information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViolationInfo {
    pub location: String,
    pub operation: String,
    pub left_operand: u64,
    pub right_operand: u64,
}

/// Object change information for cache updates
#[derive(Debug, Clone)]
pub struct ObjectChange<Id, Obj> {
    pub id: Id,
    pub object: Obj,
}

/// Fuzzer configuration
#[derive(Debug, Clone)]
pub struct FuzzerConfig {
    pub rpc_url: String,
    pub package_id: String,
    pub module_name: String,
    pub function_name: String,
    pub type_arguments: Vec<String>,
    pub args: Vec<String>,
    pub iterations: u64,
    pub timeout_seconds: u64,
    pub sender: Option<String>,
}

/// Fuzzing result status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FuzzingStatus {
    InProgress,
    ViolationFound,
    NoViolationFound,
    Error(String),
}

/// Final fuzzing result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuzzingResult {
    pub status: FuzzingStatus,
    pub violations: Vec<ViolationInfo>,
    pub iterations_completed: u64,
    pub total_iterations: u64,
}

impl FuzzingResult {
    pub fn violation_found(violations: Vec<ViolationInfo>, iterations: u64) -> Self {
        Self {
            status: FuzzingStatus::ViolationFound,
            violations,
            iterations_completed: iterations,
            total_iterations: iterations,
        }
    }

    pub fn no_violation_found() -> Self {
        Self {
            status: FuzzingStatus::NoViolationFound,
            violations: vec![],
            iterations_completed: 0,
            total_iterations: 0,
        }
    }

    pub fn error(msg: String) -> Self {
        Self {
            status: FuzzingStatus::Error(msg),
            violations: vec![],
            iterations_completed: 0,
            total_iterations: 0,
        }
    }
}
