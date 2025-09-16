use anyhow::Result;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use sui_types::base_types::SuiAddress;

use crate::error::{FuzzerError, FuzzerResult};
use crate::mutation::strategy::{GenerativeStrategy, MutationStrategy};
use crate::types::CloneableValue;

/// Strategy for generating completely random values
///
/// This strategy provides general-purpose random mutations that complement
/// the more targeted strategies like shift violation and boundary values.
pub struct RandomStrategy {
    rng: StdRng,
}

impl RandomStrategy {
    pub fn new() -> Self {
        Self {
            rng: StdRng::from_rng(&mut rand::rng()),
        }
    }

    fn generate_random_integer(&mut self, type_name: &str) -> FuzzerResult<CloneableValue> {
        match type_name {
            "u8" => Ok(CloneableValue::U8(self.rng.random())),
            "u16" => Ok(CloneableValue::U16(self.rng.random())),
            "u32" => Ok(CloneableValue::U32(self.rng.random())),
            "u64" => Ok(CloneableValue::U64(self.rng.random())),
            "u128" => Ok(CloneableValue::U128(self.rng.random())),
            "u256" => {
                let mut bytes = [0u8; 32];
                self.rng.fill(&mut bytes);
                Ok(CloneableValue::U256(bytes))
            }
            _ => Err(FuzzerError::ConversionError(format!(
                "Unsupported integer type: {}",
                type_name
            ))),
        }
    }
}

impl GenerativeStrategy for RandomStrategy {
    fn generate(&mut self, type_name: &str) -> FuzzerResult<CloneableValue> {
        match type_name {
            "u8" | "u16" | "u32" | "u64" | "u128" | "u256" => self.generate_random_integer(type_name),
            "bool" => Ok(CloneableValue::Bool(self.rng.random_bool(0.5))),
            "address" => Ok(CloneableValue::Address(SuiAddress::random_for_testing_only())),
            _ => Err(FuzzerError::ConversionError(format!(
                "Unsupported type for random generation: {}",
                type_name
            ))),
        }
    }

    fn supported_types(&self) -> &[&'static str] {
        &["u8", "u16", "u32", "u64", "u128", "u256", "bool", "address"]
    }

    fn description(&self) -> &'static str {
        "Random strategy: generates uniformly random values"
    }
}

impl MutationStrategy for RandomStrategy {
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
                    *value = CloneableValue::Address(SuiAddress::random_for_testing_only());
                }
                CloneableValue::Vector(vec) if !vec.is_empty() => {
                    // Mutate a random element in the vector
                    let index = self.rng.random_range(0..vec.len());
                    self.mutate(&mut vec[index])?;
                }
                CloneableValue::NestedStruct { mutated_fields, .. } => {
                    // Mutate a random field in the nested struct
                    if !mutated_fields.is_empty() {
                        let field_names: Vec<_> = mutated_fields.keys().cloned().collect();
                        let selected_field = &field_names[self.rng.random_range(0..field_names.len())];
                        if let Some(field_value) = mutated_fields.get_mut(selected_field) {
                            self.mutate(field_value)?;
                        }
                    }
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
            matches!(value, CloneableValue::Vector(v) if !v.is_empty()) ||
            matches!(value, CloneableValue::NestedStruct { mutated_fields, .. } if !mutated_fields.is_empty())
    }

    fn description(&self) -> &'static str {
        "Random strategy: applies uniformly random mutations"
    }
}

impl Default for RandomStrategy {
    fn default() -> Self {
        Self::new()
    }
}
