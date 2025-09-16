use anyhow::Result;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use super::strategies::{BoundaryValueStrategy, PowerOfTwoStrategy, RandomStrategy};
use super::strategy::{GenerativeStrategy, MutationStrategy};
use crate::types::CloneableValue;

/// Main orchestrator for Sui mutation strategies
///
/// This orchestrator combines three independent strategies with fixed weights
/// optimized for shift violation detection:
/// - 40% Power-of-two strategy (2^n, 2^n±1 patterns - high shift violation
///   rate)
/// - 40% Boundary value strategy (0, 1, MAX-1, MAX - edge cases)
/// - 20% Random strategy (general coverage)
///
/// This design uses generic strategies that can be reused for other fuzz
/// targets.
pub struct SuiMutationOrchestrator {
    power_of_two_strategy: PowerOfTwoStrategy,
    boundary_strategy: BoundaryValueStrategy,
    random_strategy: RandomStrategy,
    rng: StdRng,
}

impl SuiMutationOrchestrator {
    /// Create new orchestrator with fixed strategy weights (40/40/20)
    pub fn new() -> Self {
        Self {
            power_of_two_strategy: PowerOfTwoStrategy::new(),
            boundary_strategy: BoundaryValueStrategy::new(),
            random_strategy: RandomStrategy::new(),
            rng: StdRng::from_rng(&mut rand::rng()),
        }
    }

    /// Apply mutation using weighted strategy selection (40/40/20)
    pub fn mutate(&mut self, value: &mut CloneableValue) -> Result<()> {
        use fuzzer_core::ChainValue;

        // Weighted strategy selection: 40% power-of-two, 40% boundary, 20% random
        let strategy_choice = self.rng.random_range(0..100);

        let result = match strategy_choice {
            0..=39 => {
                // 40% - Power-of-two strategy (2^n, 2^n±1 patterns)
                if value.is_integer() {
                    // For integers, use generative approach
                    let type_name = value.type_name();
                    match self.power_of_two_strategy.generate(type_name) {
                        Ok(new_value) => {
                            *value = new_value;
                            Ok(())
                        }
                        Err(e) => Err(e.into()),
                    }
                } else if self.power_of_two_strategy.can_apply(value) {
                    // For complex types, use mutative approach
                    self.power_of_two_strategy.mutate(value)
                } else {
                    // Fallback to random strategy
                    self.random_strategy.mutate(value)
                }
            }
            40..=79 => {
                // 40% - Boundary value strategy (0, 1, MAX-1, MAX)
                if value.is_integer() {
                    // For integers, use generative approach
                    let type_name = value.type_name();
                    match self.boundary_strategy.generate(type_name) {
                        Ok(new_value) => {
                            *value = new_value;
                            Ok(())
                        }
                        Err(e) => Err(e.into()),
                    }
                } else if self.boundary_strategy.can_apply(value) {
                    // For complex types, use mutative approach
                    self.boundary_strategy.mutate(value)
                } else {
                    // Fallback to random strategy
                    self.random_strategy.mutate(value)
                }
            }
            80..=99 => {
                // 20% - Random strategy (general coverage)
                self.random_strategy.mutate(value)
            }
            _ => unreachable!(),
        };

        // Handle any mutation errors by falling back to random strategy
        if result.is_err() && self.random_strategy.can_apply(value) {
            return self.random_strategy.mutate(value);
        }

        result
    }

    /// Get statistics about the strategy distribution (for debugging)
    pub fn get_strategy_distribution(&self) -> &'static str {
        "SuiMutationOrchestrator: 40% power-of-two, 40% boundary, 20% random"
    }

    /// Check if any strategy can be applied to the given value
    pub fn can_apply(&self, value: &CloneableValue) -> bool {
        self.power_of_two_strategy.can_apply(value) ||
            self.boundary_strategy.can_apply(value) ||
            self.random_strategy.can_apply(value)
    }
}

// Implement the fuzzer-core ChainMutationStrategy trait
impl fuzzer_core::ChainMutationStrategy<CloneableValue> for SuiMutationOrchestrator {
    fn mutate(&mut self, value: &mut CloneableValue) -> Result<()> {
        self.mutate(value)
    }
}

impl Default for SuiMutationOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}
