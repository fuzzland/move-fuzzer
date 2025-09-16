use std::convert::TryInto;
use std::str::FromStr;
use std::time::Duration;

use fuzzer_core::ChainValue;
use serde::{Deserialize, Serialize};
use sui_json_rpc_types::{SuiMoveNormalizedType, SuiObjectData, SuiObjectDataOptions};
use sui_move_core_types::u256::U256;
use sui_sdk::SuiClient;
use sui_simulator::SimulateResult;
use sui_tracer::shift_violation_tracer::ShiftViolation;
use sui_types::base_types::{ObjectID, SequenceNumber, SuiAddress};
use sui_types::object::{Object, Owner};
use sui_types::type_input::TypeInput;

use crate::error::{FuzzerError, FuzzerResult};

/// Represents a target function to be fuzzed
#[derive(Debug, Clone)]
pub struct TargetFunction {
    pub package_id: ObjectID,
    pub module_name: String,
    pub function_name: String,
    pub type_arguments: Vec<sui_types::type_input::TypeInput>,
}

/// Represents a function parameter with its type and value
#[derive(Debug, Clone)]
pub struct FunctionParameter {
    pub index: usize,
    pub name: String,
    pub param_type: SuiMoveNormalizedType,
    pub value: CloneableValue,
}

impl FunctionParameter {
    pub fn is_integer(&self) -> bool {
        self.value.is_integer()
    }

    pub fn is_integer_vector(&self) -> bool {
        self.value.is_integer_vector()
    }

    pub fn contains_integers(&self) -> bool {
        self.value.contains_integers()
    }

    pub fn is_mutable_object(&self) -> bool {
        self.value.is_mutable_object()
    }
}

/// Object ownership types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ObjectOwnershipType {
    Owned,
    ImmutableShared,
    MutableShared { initial_shared_version: SequenceNumber },
}

/// Cloneable value type
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CloneableValue {
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    U128(u128),
    U256([u8; 32]),
    Bool(bool),
    Address(SuiAddress),
    Vector(Vec<CloneableValue>),
    UID {
        id: ObjectID,
    },
    StructObject {
        object_id: ObjectID,
        ownership_type: ObjectOwnershipType,
        initial_object: Option<Object>,
        cached_object: Option<Object>,
    },
}

impl CloneableValue {
    pub fn type_name(&self) -> &str {
        match self {
            CloneableValue::U8(_) => "u8",
            CloneableValue::U16(_) => "u16",
            CloneableValue::U32(_) => "u32",
            CloneableValue::U64(_) => "u64",
            CloneableValue::U128(_) => "u128",
            CloneableValue::U256(_) => "u256",
            CloneableValue::Bool(_) => "bool",
            CloneableValue::Address(_) => "address",
            CloneableValue::Vector(_) => "vector",
            CloneableValue::UID { .. } => "uid",
            CloneableValue::StructObject { .. } => "struct_object",
        }
    }
}

// Implement ChainValue trait for CloneableValue
impl fuzzer_core::ChainValue for CloneableValue {
    fn is_integer(&self) -> bool {
        matches!(
            self,
            CloneableValue::U8(_) |
                CloneableValue::U16(_) |
                CloneableValue::U32(_) |
                CloneableValue::U64(_) |
                CloneableValue::U128(_) |
                CloneableValue::U256(_)
        )
    }

    fn is_integer_vector(&self) -> bool {
        match self {
            CloneableValue::Vector(vec) => vec.iter().all(CloneableValue::is_integer),
            _ => false,
        }
    }

    fn contains_integers(&self) -> bool {
        match self {
            CloneableValue::Vector(vec) => vec.iter().any(|v| v.is_integer()),
            _ => self.is_integer(),
        }
    }

    fn is_mutable_object(&self) -> bool {
        if let CloneableValue::StructObject { ownership_type, .. } = self {
            matches!(ownership_type, ObjectOwnershipType::MutableShared { .. })
        } else {
            false
        }
    }

    fn get_object_id(&self) -> Option<Vec<u8>> {
        match self {
            CloneableValue::UID { id } => Some(id.to_vec()),
            CloneableValue::StructObject { object_id, .. } => Some(object_id.to_vec()),
            _ => None,
        }
    }

    fn type_name(&self) -> &'static str {
        match self {
            CloneableValue::U8(_) => "u8",
            CloneableValue::U16(_) => "u16",
            CloneableValue::U32(_) => "u32",
            CloneableValue::U64(_) => "u64",
            CloneableValue::U128(_) => "u128",
            CloneableValue::U256(_) => "u256",
            CloneableValue::Bool(_) => "bool",
            CloneableValue::Address(_) => "address",
            CloneableValue::Vector(_) => "vector",
            CloneableValue::UID { .. } => "uid",
            CloneableValue::StructObject { .. } => "struct_object",
        }
    }
}

/// Execution result with tracer-detected shift violations
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Standard simulation result from sui-simulator
    pub simulate_result: SimulateResult,
    /// Shift violations detected by local tracer
    pub shift_violations: Vec<ShiftViolation>,
    /// Execution duration
    pub execution_time: Duration,
}

impl CloneableValue {
    pub fn parse_u256(s: &str) -> FuzzerResult<CloneableValue> {
        let value = if s.starts_with("0x") {
            U256::from_str_radix(&s[2..], 16)
                .map_err(|e| FuzzerError::ConversionError(format!("Invalid U256 hex: {}", e)))?
        } else {
            U256::from_str(s).map_err(|e| FuzzerError::ConversionError(format!("Invalid U256 decimal: {}", e)))?
        };

        let bytes = value.to_be_bytes();
        Ok(CloneableValue::U256(bytes))
    }

    pub fn parse_vector(inner_type: &SuiMoveNormalizedType, s: &str) -> FuzzerResult<CloneableValue> {
        // Handle JSON array format like "[1,2,3]"
        let s = s.trim();
        if !s.starts_with('[') || !s.ends_with(']') {
            return Err(FuzzerError::ConversionError(format!("Invalid vector format: {}", s)));
        }

        let inner_str = &s[1..s.len() - 1];
        if inner_str.is_empty() {
            return Ok(CloneableValue::Vector(vec![]));
        }

        let mut values = Vec::new();
        for item in inner_str.split(',') {
            let item = item.trim();
            let value = match inner_type {
                SuiMoveNormalizedType::U8 => CloneableValue::U8(item.parse().unwrap_or_default()),
                SuiMoveNormalizedType::U16 => CloneableValue::U16(item.parse().unwrap_or_default()),
                SuiMoveNormalizedType::U32 => CloneableValue::U32(item.parse().unwrap_or_default()),
                SuiMoveNormalizedType::U64 => CloneableValue::U64(item.parse().unwrap_or_default()),
                SuiMoveNormalizedType::U128 => CloneableValue::U128(item.parse().unwrap_or_default()),
                SuiMoveNormalizedType::U256 => CloneableValue::parse_u256(item)?,
                SuiMoveNormalizedType::Bool => CloneableValue::Bool(item.parse().unwrap_or_default()),
                SuiMoveNormalizedType::Address => {
                    CloneableValue::Address(SuiAddress::from_str(item).unwrap_or_default())
                }
                _ => {
                    return Err(FuzzerError::ConversionError(format!(
                        "Unsupported vector inner type: {:?}",
                        inner_type
                    )));
                }
            };
            values.push(value);
        }

        Ok(CloneableValue::Vector(values))
    }

    /// Create CloneableValue from object ID
    pub async fn from_object_id(
        object_id: &str,
        rpc_client: &SuiClient,
        param_type: &SuiMoveNormalizedType,
    ) -> FuzzerResult<CloneableValue> {
        // 1. Parse object_id string
        let obj_id = ObjectID::from_hex_literal(object_id)
            .map_err(|e| FuzzerError::ConversionError(format!("Invalid object ID: {}", e)))?;

        // 2. Fetch SuiObjectData from RPC
        let opts = SuiObjectDataOptions::full_content().with_bcs();
        let object_response = rpc_client
            .read_api()
            .get_object_with_options(obj_id, opts)
            .await
            .map_err(|e| FuzzerError::NetworkError(format!("Failed to fetch object: {}", e)))?;

        let object_data = object_response
            .data
            .ok_or_else(|| FuzzerError::ConversionError("Object not found".to_string()))?;

        // 3. Create Sui Object from object data
        let sui_object = sui_object_data_to_object(&object_data)?;

        // 4. Determine ownership type
        let ownership_type = get_object_ownership_type(&object_data, param_type);

        Ok(CloneableValue::StructObject {
            object_id: obj_id,
            ownership_type,
            initial_object: Some(sui_object),
            cached_object: None,
        })
    }

    /// Get the actual Object from StructObject, prioritizing cached over
    /// initial
    pub fn get_struct_object(&self) -> FuzzerResult<&Object> {
        match self {
            CloneableValue::StructObject {
                initial_object,
                cached_object,
                ..
            } => cached_object
                .as_ref()
                .or(initial_object.as_ref())
                .ok_or_else(|| FuzzerError::ConversionError("No object available for StructObject".to_string())),
            _ => Err(FuzzerError::ConversionError("Not a StructObject".to_string())),
        }
    }

    /// Get the actual Object from StructObject (owned), prioritizing cached
    /// over initial
    pub fn get_struct_object_owned(&self) -> FuzzerResult<Object> {
        self.get_struct_object().map(|obj| obj.clone())
    }

    /// Check if this StructObject has a cached version
    pub fn has_cached_object(&self) -> bool {
        matches!(
            self,
            CloneableValue::StructObject {
                cached_object: Some(_),
                ..
            }
        )
    }
}

/// Helper functions from original sui-fuzzer
pub fn unwrap_reference_type(param_type: &SuiMoveNormalizedType) -> &SuiMoveNormalizedType {
    match param_type {
        SuiMoveNormalizedType::Reference(inner_type) => unwrap_reference_type(inner_type),
        SuiMoveNormalizedType::MutableReference(inner_type) => unwrap_reference_type(inner_type),
        _ => param_type,
    }
}

/// Convert TypeInput to SuiMoveNormalizedType
pub fn type_input_to_normalized_type(type_input: &TypeInput) -> FuzzerResult<SuiMoveNormalizedType> {
    match type_input {
        TypeInput::U8 => Ok(SuiMoveNormalizedType::U8),
        TypeInput::U16 => Ok(SuiMoveNormalizedType::U16),
        TypeInput::U32 => Ok(SuiMoveNormalizedType::U32),
        TypeInput::U64 => Ok(SuiMoveNormalizedType::U64),
        TypeInput::U128 => Ok(SuiMoveNormalizedType::U128),
        TypeInput::U256 => Ok(SuiMoveNormalizedType::U256),
        TypeInput::Bool => Ok(SuiMoveNormalizedType::Bool),
        TypeInput::Address => Ok(SuiMoveNormalizedType::Address),
        TypeInput::Vector(inner) => {
            let inner_type = type_input_to_normalized_type(inner)?;
            Ok(SuiMoveNormalizedType::Vector(Box::new(inner_type)))
        }
        _ => Err(FuzzerError::ConversionError(format!(
            "Cannot convert TypeInput {:?} to SuiMoveNormalizedType",
            type_input
        ))),
    }
}

/// Resolve TypeParameter to concrete SuiMoveNormalizedType
pub fn resolve_type_parameter(index: usize, type_arguments: &[TypeInput]) -> FuzzerResult<SuiMoveNormalizedType> {
    type_arguments
        .get(index)
        .ok_or_else(|| {
            FuzzerError::ConversionError(format!(
                "TypeParameter index {} out of range (type_arguments has {} elements)",
                index,
                type_arguments.len()
            ))
        })
        .and_then(type_input_to_normalized_type)
}

/// Determine object ownership type for transaction building
pub fn get_object_ownership_type(
    object_data: &SuiObjectData,
    param_type: &SuiMoveNormalizedType,
) -> ObjectOwnershipType {
    match &object_data.owner {
        Some(Owner::AddressOwner(_)) => ObjectOwnershipType::Owned,
        Some(Owner::ObjectOwner(_)) => ObjectOwnershipType::Owned,
        Some(Owner::Shared { initial_shared_version }) => {
            // Distinguish mutable vs immutable based on parameter reference type
            match param_type {
                SuiMoveNormalizedType::MutableReference(_) => ObjectOwnershipType::MutableShared {
                    initial_shared_version: *initial_shared_version,
                },
                SuiMoveNormalizedType::Reference(_) => ObjectOwnershipType::ImmutableShared,
                _ => ObjectOwnershipType::MutableShared {
                    initial_shared_version: *initial_shared_version,
                }, // Default to mutable for non-reference types
            }
        }
        Some(Owner::Immutable) => ObjectOwnershipType::ImmutableShared,
        Some(Owner::ConsensusAddressOwner { .. }) => ObjectOwnershipType::Owned,
        None => ObjectOwnershipType::Owned, // Default fallback
    }
}

/// Convert SuiObjectData to Object using built-in TryInto implementation
pub fn sui_object_data_to_object(object_data: &SuiObjectData) -> FuzzerResult<Object> {
    object_data
        .clone()
        .try_into()
        .map_err(|e| FuzzerError::ConversionError(format!("Failed to convert SuiObjectData to Object: {}", e)))
}

#[cfg(test)]
mod tests {
    use fuzzer_core::ChainValue;
    use sui_types::base_types::ObjectID;

    use super::*;

    #[test]
    fn test_chain_value_methods() {
        // Test integer values
        let u32_value = CloneableValue::U32(42);
        assert!(u32_value.is_integer());
        assert!(!u32_value.is_integer_vector());
        assert!(u32_value.contains_integers());

        // Test integer vector
        let int_vector = CloneableValue::Vector(vec![
            CloneableValue::U32(1),
            CloneableValue::U32(2),
            CloneableValue::U32(3),
        ]);
        assert!(!int_vector.is_integer());
        assert!(int_vector.is_integer_vector());
        assert!(int_vector.contains_integers());

        // Test mixed vector (should not be integer vector)
        let mixed_vector = CloneableValue::Vector(vec![CloneableValue::U32(1), CloneableValue::Bool(true)]);
        assert!(!mixed_vector.is_integer());
        assert!(!mixed_vector.is_integer_vector());
        assert!(mixed_vector.contains_integers());

        // Test non-integer value
        let bool_value = CloneableValue::Bool(true);
        assert!(!bool_value.is_integer());
        assert!(!bool_value.is_integer_vector());
        assert!(!bool_value.contains_integers());

        // Test UID
        let uid_value = CloneableValue::UID { id: ObjectID::random() };
        assert!(!uid_value.is_integer());
        assert!(!uid_value.is_integer_vector());
        assert!(!uid_value.contains_integers());
        assert!(uid_value.get_object_id().is_some());
    }
}
