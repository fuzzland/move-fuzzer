use std::io::{self, Write};
use std::time::Duration;

use crate::types::{FunctionInfo, FuzzingResult, FuzzingStatus, Parameter};
use crate::ChainValue;

/// Console reporter for fuzzing results
#[derive(Debug, Clone)]
pub struct ConsoleReporter {
    show_progress: bool,
    last_progress_iteration: u64,
}

impl Default for ConsoleReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl ConsoleReporter {
    pub fn new() -> Self {
        Self {
            show_progress: true,
            last_progress_iteration: 0,
        }
    }

    pub fn with_progress(show_progress: bool) -> Self {
        Self {
            show_progress,
            last_progress_iteration: 0,
        }
    }

    pub fn print_progress(&mut self, current_iteration: u64, max_iterations: u64) -> anyhow::Result<()> {
        if !self.show_progress {
            return Ok(());
        }

        // Only print progress every 10,000 iterations to avoid spam
        if current_iteration % 10_000 == 0 && current_iteration != self.last_progress_iteration {
            let percentage = (current_iteration as f64 / max_iterations as f64) * 100.0;
            print!(
                "\rProgress: {}/{} iterations ({:.1}%)",
                current_iteration, max_iterations, percentage
            );
            io::stdout().flush()?;
            self.last_progress_iteration = current_iteration;
        }

        Ok(())
    }

    pub fn print_fuzzing_result(&self, result: &FuzzingResult) -> anyhow::Result<()> {
        // Clear progress line if it was shown
        if self.show_progress {
            print!("\r{}\r", " ".repeat(80));
        }

        println!("\n{}", "=".repeat(80));
        println!("FUZZING RESULT");
        println!("{}", "=".repeat(80));

        match &result.status {
            FuzzingStatus::ViolationFound => {
                println!("🎯 STATUS: VIOLATION DETECTED!");
                println!("🚨 Found {} shift violation(s)", result.violations.len());

                for (i, violation) in result.violations.iter().enumerate() {
                    println!("\nViolation #{}: ", i + 1);
                    println!("  Location: {}", violation.location);
                    println!("  Operation: {}", violation.operation);
                    println!("  Left operand: {}", violation.left_operand);
                    println!("  Right operand: {}", violation.right_operand);
                }
            }
            FuzzingStatus::NoViolationFound => {
                println!("✅ STATUS: NO VIOLATIONS FOUND");
                println!(
                    "Completed all {} iterations without detecting violations",
                    result.total_iterations
                );
            }
            FuzzingStatus::InProgress => {
                println!("⏳ STATUS: IN PROGRESS");
            }
            FuzzingStatus::Error(error) => {
                println!("❌ STATUS: ERROR");
                println!("Error: {}", error);
            }
        }

        println!(
            "\nIterations completed: {}/{}",
            result.iterations_completed, result.total_iterations
        );

        println!("\n{}", "=".repeat(80));
        Ok(())
    }

    pub fn print_function_info<V: ChainValue>(
        &self,
        function: &FunctionInfo,
        parameters: &[Parameter<V>],
    ) -> anyhow::Result<()> {
        println!("\n{}", "=".repeat(80));
        println!("FUZZING TARGET");
        println!("{}", "=".repeat(80));

        println!("Package: {}", function.package_id);
        println!("Module: {}", function.module_name);
        println!("Function: {}", function.function_name);

        if !function.type_arguments.is_empty() {
            println!("Type Arguments: {:?}", function.type_arguments);
        }

        println!("\nParameters ({}):", parameters.len());
        for (i, param) in parameters.iter().enumerate() {
            println!("  {}: {} = {:?}", i, param.type_name, param.value);
        }

        println!("{}", "=".repeat(80));
        Ok(())
    }

    pub fn print_fuzzing_start(&self, iterations: u64, timeout: Duration) -> anyhow::Result<()> {
        println!("\n🚀 Starting fuzzing...");
        println!("  Max iterations: {}", iterations);
        println!("  Timeout: {}s", timeout.as_secs());
        println!("  Target: Shift violations in integer operations");
        println!();
        Ok(())
    }

    pub fn print_message(&self, message: &str) -> anyhow::Result<()> {
        println!("{}", message);
        Ok(())
    }

    pub fn print_error(&self, error: &str) -> anyhow::Result<()> {
        eprintln!("❌ Error: {}", error);
        Ok(())
    }

    pub fn print_warning(&self, warning: &str) -> anyhow::Result<()> {
        println!("⚠️  Warning: {}", warning);
        Ok(())
    }

    pub fn print_success(&self, message: &str) -> anyhow::Result<()> {
        println!("✅ {}", message);
        Ok(())
    }
}
