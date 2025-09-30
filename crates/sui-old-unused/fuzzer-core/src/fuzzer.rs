use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use tokio::time::timeout;
use tracing::{debug, info, warn};

use crate::cache::ObjectCache;
use crate::{ChainAdapter, ChainMutationStrategy, ChainValue, FunctionInfo, FuzzerConfig, FuzzingResult, Parameter};

/// Core fuzzer that orchestrates the fuzzing process using blockchain-specific
/// adapters
pub struct CoreFuzzer<A: ChainAdapter> {
    adapter: Arc<A>,
    config: FuzzerConfig,
    function: FunctionInfo,
    parameters: Vec<Parameter<A::Value>>,
    mutator: A::Mutator,
    cache: ObjectCache<A>,
}

impl<A: ChainAdapter> CoreFuzzer<A> {
    pub async fn new(adapter: A, config: FuzzerConfig) -> anyhow::Result<Self> {
        info!("Initializing CoreFuzzer with config: {:?}", config);

        let adapter = Arc::new(adapter);

        // Initialize components using the adapter
        let function = adapter.resolve_function(&config).await?;
        let parameters = adapter.initialize_parameters(&function, &config.args).await?;
        let mutator = adapter.create_mutator();
        let cache = ObjectCache::new(adapter.clone());

        info!(
            "CoreFuzzer initialized for {}::{}::{} with {} parameters",
            function.package_id,
            function.module_name,
            function.function_name,
            parameters.len()
        );

        Ok(Self {
            adapter,
            config,
            function,
            parameters,
            mutator,
            cache,
        })
    }

    pub async fn run(&mut self) -> anyhow::Result<FuzzingResult> {
        let start_time = Instant::now();
        let max_iterations = self.config.iterations;
        let timeout_duration = std::time::Duration::from_secs(self.config.timeout_seconds);

        info!(
            "Starting fuzzing: {} iterations, timeout: {}s",
            max_iterations, self.config.timeout_seconds
        );

        // Shared counter for tracking iterations across timeout scenarios
        let iteration_counter = Arc::new(AtomicU64::new(0));
        let counter_clone = iteration_counter.clone();

        let sender = self.adapter.get_sender_from_config(&self.config);

        let result = timeout(
            timeout_duration,
            self.fuzzing_loop(sender, max_iterations, counter_clone),
        )
        .await;

        let total_execution_time = start_time.elapsed();

        match result {
            Ok(loop_result) => match loop_result {
                Ok(fuzzing_result) => {
                    info!("Fuzzing completed in {:.2}s", total_execution_time.as_secs_f64());
                    Ok(fuzzing_result)
                }
                Err(error) => {
                    warn!("Fuzzing failed: {}", error);
                    Ok(FuzzingResult::error(error.to_string()))
                }
            },
            Err(_) => {
                warn!("Fuzzing timed out after {:.2}s", total_execution_time.as_secs_f64());
                Ok(FuzzingResult::error("Timeout".to_string()))
            }
        }
    }

    async fn fuzzing_loop(
        &mut self,
        sender: A::Address,
        max_iterations: u64,
        iteration_counter: Arc<AtomicU64>,
    ) -> anyhow::Result<FuzzingResult> {
        let start_time = Instant::now();

        for iteration in 1..=max_iterations {
            iteration_counter.store(iteration, Ordering::Relaxed);
            debug!("Starting iteration {}/{}", iteration, max_iterations);

            if iteration % 10_000 == 0 {
                info!("Progress: {}/{} iterations", iteration, max_iterations);
            }

            // Step 1: Execute the function with current parameters
            let execution_result = self.adapter.execute(&sender, &self.function, &self.parameters).await?;

            let object_changes = self.adapter.extract_object_changes(&execution_result);
            if !object_changes.is_empty() {
                debug!("Processing {} object changes to update cache", object_changes.len());
                self.cache.process_changes(&object_changes);
            }

            // Step 2: Check for shift violations
            if self.adapter.has_shift_violations(&execution_result) {
                info!(
                    "🎯 Shift violation detected on iteration {}/{}!",
                    iteration, max_iterations
                );

                let violations = self.adapter.extract_violations(&execution_result);
                return Ok(FuzzingResult::violation_found(violations, iteration));
            }

            debug!("Iteration {} completed - no violations found", iteration);

            // Step 3: Mutate parameters for next iteration
            if iteration < max_iterations {
                self.update_cached_objects()?;
                self.mutate_parameters()?;
            }
        }

        // All iterations completed without finding violations
        let total_time = start_time.elapsed();
        info!(
            "Completed all {} iterations in {:.2}s - no violations found",
            max_iterations,
            total_time.as_secs_f64()
        );

        Ok(FuzzingResult::no_violation_found())
    }

    /// Update cached objects from the object cache for mutable shared objects
    fn update_cached_objects(&mut self) -> anyhow::Result<()> {
        let mut updated_count = 0;

        for param in &mut self.parameters {
            if param.value.is_mutable_object() {
                if let Some(obj_id_bytes) = param.value.get_object_id() {
                    if let Ok(object_id) = self.adapter.bytes_to_object_id(&obj_id_bytes) {
                        if let Some(cached_obj) = self.cache.get_random_version(&object_id) {
                            self.adapter
                                .update_value_with_cached_object(&mut param.value, &cached_obj)?;
                            updated_count += 1;
                            debug!("Updated parameter {} with cached object for ObjectId", param.index);
                        }
                    }
                }
            }
        }

        if updated_count > 0 {
            debug!("Updated {} parameters with cached objects", updated_count);
        }

        Ok(())
    }

    fn mutate_parameters(&mut self) -> anyhow::Result<()> {
        debug!("Mutating {} parameters", self.parameters.len());

        for param in &mut self.parameters {
            self.mutator.mutate(&mut param.value)?;
            debug!(
                "Mutated parameter {}: {} = {:?}",
                param.index,
                param.type_name(),
                param.value
            );
        }

        Ok(())
    }

    pub fn adapter(&self) -> &A {
        &self.adapter
    }

    pub fn function(&self) -> &FunctionInfo {
        &self.function
    }

    pub fn parameters(&self) -> &[Parameter<A::Value>] {
        &self.parameters
    }

    pub fn cache_stats(&self) -> (usize, Vec<A::ObjectId>) {
        (self.cache.total_cached_objects(), self.cache.cached_object_ids())
    }
}
