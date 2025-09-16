use anyhow::Result;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::error::{FuzzerError, FuzzerResult};
use crate::mutation::strategy::{GenerativeStrategy, MutationStrategy};
use crate::types::CloneableValue;

/// Strategy for generating power-of-two values and their variations
///
/// This strategy generates values commonly used in bitwise operations:
/// - Powers of 2: 1, 2, 4, 8, 16, 32, 64, 128, ...
/// - Powers of 2 minus 1: 0, 1, 3, 7, 15, 31, 63, 127, ... (masks)
/// - Powers of 2 plus 1: 2, 3, 5, 9, 17, 33, 65, 129, ... (edge cases)
///
/// These values are particularly effective at finding edge cases in
/// arithmetic operations, bit manipulation, and array indexing.
pub struct PowerOfTwoStrategy {
    rng: StdRng,
}

impl PowerOfTwoStrategy {
    pub fn new() -> Self {
        Self {
            rng: StdRng::from_rng(&mut rand::rng()),
        }
    }

    fn generate_power_of_two_integer(&mut self, type_name: &str) -> FuzzerResult<CloneableValue> {
        match type_name {
            "u8" => {
                let powers = [1u8, 2, 4, 8, 16, 32, 64, 128];
                let index = self.rng.random_range(0..powers.len());
                let base_value = powers[index];

                let variation = match self.rng.random_range(0..3) {
                    0 => base_value,                   // Exact power of 2
                    1 => base_value.saturating_sub(1), // Power of 2 minus 1 (mask)
                    2 => base_value.saturating_add(1), // Power of 2 plus 1
                    _ => unreachable!(),
                };

                Ok(CloneableValue::U8(variation))
            }
            "u16" => {
                let power_exp = self.rng.random_range(0..16);
                let base_value = 1u16 << power_exp;

                let variation = match self.rng.random_range(0..3) {
                    0 => base_value,
                    1 => base_value.saturating_sub(1),
                    2 => base_value.saturating_add(1),
                    _ => unreachable!(),
                };

                Ok(CloneableValue::U16(variation))
            }
            "u32" => {
                let power_exp = self.rng.random_range(0..32);
                let base_value = 1u32 << power_exp;

                let variation = match self.rng.random_range(0..3) {
                    0 => base_value,
                    1 => base_value.saturating_sub(1),
                    2 => base_value.saturating_add(1),
                    _ => unreachable!(),
                };

                Ok(CloneableValue::U32(variation))
            }
            "u64" => {
                let power_exp = self.rng.random_range(0..64);
                let base_value = 1u64 << power_exp;

                let variation = match self.rng.random_range(0..3) {
                    0 => base_value,
                    1 => base_value.saturating_sub(1),
                    2 => base_value.saturating_add(1),
                    _ => unreachable!(),
                };

                Ok(CloneableValue::U64(variation))
            }
            "u128" => {
                let power_exp = self.rng.random_range(0..128);
                let base_value = 1u128 << power_exp;

                let variation = match self.rng.random_range(0..3) {
                    0 => base_value,
                    1 => base_value.saturating_sub(1),
                    2 => base_value.saturating_add(1),
                    _ => unreachable!(),
                };

                Ok(CloneableValue::U128(variation))
            }
            "u256" => {
                let power_exp = self.rng.random_range(0..256);
                let mut bytes = [0u8; 32];

                // Set the appropriate bit for 2^power_exp
                let byte_index = 31 - (power_exp / 8);
                let bit_index = power_exp % 8;
                bytes[byte_index] = 1u8 << bit_index;

                // Apply variation
                match self.rng.random_range(0..3) {
                    0 => {} // Keep exact power of 2
                    1 => {
                        // Subtract 1 (creates mask pattern)
                        if bytes[31] > 0 {
                            bytes[31] -= 1;
                        } else {
                            // Handle multi-byte subtraction
                            let mut carry = true;
                            for i in (0..32).rev() {
                                if !carry {
                                    break;
                                }
                                if bytes[i] > 0 {
                                    bytes[i] -= 1;
                                    carry = false;
                                    // Fill remaining bytes with 0xFF
                                    for j in (i + 1)..32 {
                                        bytes[j] = 0xFF;
                                    }
                                }
                            }
                        }
                    }
                    2 => {
                        // Add 1
                        let mut carry = true;
                        for i in (0..32).rev() {
                            if !carry {
                                break;
                            }
                            if bytes[i] < 0xFF {
                                bytes[i] += 1;
                                carry = false;
                            } else {
                                bytes[i] = 0;
                            }
                        }
                    }
                    _ => unreachable!(),
                }

                Ok(CloneableValue::U256(bytes))
            }
            _ => Err(FuzzerError::ConversionError(format!(
                "Unsupported integer type: {}",
                type_name
            ))),
        }
    }

    /// Generate special values that are commonly used in algorithms
    pub fn generate_common_algorithmic_values(&mut self, type_name: &str) -> FuzzerResult<CloneableValue> {
        // Common algorithmic constants that often appear in edge cases
        match type_name {
            "u8" => {
                let common = [0, 1, 2, 3, 4, 7, 8, 15, 16, 31, 32, 63, 64, 127, 128, 255];
                let value = common[self.rng.random_range(0..common.len())];
                Ok(CloneableValue::U8(value))
            }
            "u16" => {
                let common = [
                    0, 1, 255, 256, 511, 512, 1023, 1024, 2047, 2048, 4095, 4096, 8191, 8192, 16383, 16384, 32767,
                    32768, 65535,
                ];
                let value = common[self.rng.random_range(0..common.len())];
                Ok(CloneableValue::U16(value))
            }
            "u32" => {
                // Focus on smaller powers for u32 to keep it manageable
                let power_exp = self.rng.random_range(0..20); // Up to 2^19
                let base_value = 1u32 << power_exp;
                let variation = match self.rng.random_range(0..3) {
                    0 => base_value,
                    1 => base_value.saturating_sub(1),
                    2 => base_value.saturating_add(1),
                    _ => unreachable!(),
                };
                Ok(CloneableValue::U32(variation))
            }
            "u64" => {
                // Focus on smaller powers for u64
                let power_exp = self.rng.random_range(0..32); // Up to 2^31
                let base_value = 1u64 << power_exp;
                let variation = match self.rng.random_range(0..3) {
                    0 => base_value,
                    1 => base_value.saturating_sub(1),
                    2 => base_value.saturating_add(1),
                    _ => unreachable!(),
                };
                Ok(CloneableValue::U64(variation))
            }
            _ => self.generate_power_of_two_integer(type_name),
        }
    }
}

impl GenerativeStrategy for PowerOfTwoStrategy {
    fn generate(&mut self, type_name: &str) -> FuzzerResult<CloneableValue> {
        if self.rng.random_bool(0.7) {
            self.generate_power_of_two_integer(type_name)
        } else {
            self.generate_common_algorithmic_values(type_name)
        }
    }

    fn supported_types(&self) -> &[&'static str] {
        &["u8", "u16", "u32", "u64", "u128", "u256"]
    }

    fn description(&self) -> &'static str {
        "Power-of-two strategy: generates 2^n, 2^n-1, 2^n+1 values and algorithmic constants"
    }
}

impl MutationStrategy for PowerOfTwoStrategy {
    fn mutate(&mut self, value: &mut CloneableValue) -> Result<()> {
        use fuzzer_core::ChainValue;

        if value.is_integer() {
            let type_name = value.type_name();
            *value = self.generate(type_name)?;
        } else if let CloneableValue::Vector(vec) = value {
            if !vec.is_empty() {
                // Mutate a random element in the vector if it's an integer
                let index = self.rng.random_range(0..vec.len());
                if vec[index].is_integer() {
                    let type_name = vec[index].type_name();
                    vec[index] = self.generate(type_name)?;
                }
            }
        }

        Ok(())
    }

    fn can_apply(&self, value: &CloneableValue) -> bool {
        use fuzzer_core::ChainValue;

        if value.is_integer() {
            return true;
        }

        if let CloneableValue::Vector(v) = value {
            !v.is_empty() && v.iter().any(|v| v.is_integer())
        } else {
            false
        }
    }

    fn description(&self) -> &'static str {
        "Power-of-two strategy: mutates to 2^n, 2^n-1, 2^n+1 values"
    }
}

impl Default for PowerOfTwoStrategy {
    fn default() -> Self {
        Self::new()
    }
}
