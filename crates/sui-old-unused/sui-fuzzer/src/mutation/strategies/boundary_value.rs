use anyhow::Result;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use sui_types::base_types::SuiAddress;

use crate::error::{FuzzerError, FuzzerResult};
use crate::mutation::strategy::{GenerativeStrategy, MutationStrategy};
use crate::types::CloneableValue;

/// Strategy for generating boundary values and edge cases
///
/// This strategy focuses on values at the boundaries of their types:
/// - Minimum values (0 for unsigned integers)
/// - Maximum values (TYPE_MAX)
/// - Values just above minimum (1)
/// - Values just below maximum (TYPE_MAX - 1)
pub struct BoundaryValueStrategy {
    rng: StdRng,
}

impl BoundaryValueStrategy {
    pub fn new() -> Self {
        Self {
            rng: StdRng::from_rng(&mut rand::rng()),
        }
    }

    fn generate_integer_boundary(&mut self, type_name: &str) -> FuzzerResult<CloneableValue> {
        let boundary_index = self.rng.random_range(0..4);

        match type_name {
            "u8" => {
                let values = [0u8, 1, u8::MAX - 1, u8::MAX];
                Ok(CloneableValue::U8(values[boundary_index]))
            }
            "u16" => {
                let values = [0u16, 1, u16::MAX - 1, u16::MAX];
                Ok(CloneableValue::U16(values[boundary_index]))
            }
            "u32" => {
                let values = [0u32, 1, u32::MAX - 1, u32::MAX];
                Ok(CloneableValue::U32(values[boundary_index]))
            }
            "u64" => {
                let values = [0u64, 1, u64::MAX - 1, u64::MAX];
                Ok(CloneableValue::U64(values[boundary_index]))
            }
            "u128" => {
                let values = [0u128, 1, u128::MAX - 1, u128::MAX];
                Ok(CloneableValue::U128(values[boundary_index]))
            }
            "u256" => {
                let boundary_values = [
                    [0u8; 32], // Zero
                    {
                        let mut v = [0u8; 32];
                        v[31] = 1;
                        v
                    }, // One
                    {
                        let mut v = [0xFFu8; 32];
                        v[31] = 0xFE;
                        v
                    }, // max - 1
                    [0xFFu8; 32], // Max value
                ];
                Ok(CloneableValue::U256(boundary_values[boundary_index]))
            }
            _ => Err(FuzzerError::ConversionError(format!(
                "Unsupported integer type: {}",
                type_name
            ))),
        }
    }
}

impl GenerativeStrategy for BoundaryValueStrategy {
    fn generate(&mut self, type_name: &str) -> FuzzerResult<CloneableValue> {
        match type_name {
            "u8" | "u16" | "u32" | "u64" | "u128" | "u256" => self.generate_integer_boundary(type_name),
            "bool" => {
                // For booleans, we always have only two boundary values
                Ok(CloneableValue::Bool(self.rng.random_bool(0.5)))
            }
            "address" => {
                // For addresses, generate zero address or random
                if self.rng.random_bool(0.5) {
                    Ok(CloneableValue::Address(SuiAddress::ZERO))
                } else {
                    Ok(CloneableValue::Address(SuiAddress::random_for_testing_only()))
                }
            }
            _ => Err(FuzzerError::ConversionError(format!(
                "Unsupported type for boundary values: {}",
                type_name
            ))),
        }
    }

    fn supported_types(&self) -> &[&'static str] {
        &["u8", "u16", "u32", "u64", "u128", "u256", "bool", "address"]
    }

    fn description(&self) -> &'static str {
        "Boundary value strategy: generates edge case values at type boundaries"
    }
}

impl MutationStrategy for BoundaryValueStrategy {
    fn mutate(&mut self, value: &mut CloneableValue) -> Result<()> {
        use fuzzer_core::ChainValue;

        if value.is_integer() {
            let type_name = value.type_name();
            *value = self.generate(type_name)?;
        } else {
            match value {
                CloneableValue::Bool(_) => {
                    *value = CloneableValue::Bool(self.rng.random_bool(0.5));
                }
                CloneableValue::Address(_) => {
                    *value = if self.rng.random_bool(0.5) {
                        CloneableValue::Address(SuiAddress::ZERO)
                    } else {
                        CloneableValue::Address(SuiAddress::random_for_testing_only())
                    };
                }
                CloneableValue::Vector(vec) if !vec.is_empty() => {
                    // Mutate a random element in the vector
                    let index = self.rng.random_range(0..vec.len());
                    self.mutate(&mut vec[index])?;
                }
                _ => {} // No mutation for unsupported types
            }
        }

        Ok(())
    }

    fn can_apply(&self, value: &CloneableValue) -> bool {
        use fuzzer_core::ChainValue;

        value.is_integer() ||
            matches!(value, CloneableValue::Bool(_) | CloneableValue::Address(_)) ||
            matches!(value, CloneableValue::Vector(v) if !v.is_empty())
    }

    fn description(&self) -> &'static str {
        "Boundary value strategy: mutates to edge case values at type boundaries"
    }
}

impl Default for BoundaryValueStrategy {
    fn default() -> Self {
        Self::new()
    }
}
