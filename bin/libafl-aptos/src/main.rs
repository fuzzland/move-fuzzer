mod utils;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use aptos_fuzzer::{
    AbortCodeObjective, AptosFuzzerMutator, AptosFuzzerState, AptosMoveExecutor,
    ShiftOverflowObjective,
};
use clap::Parser;
use libafl::corpus::Corpus;
use libafl::events::SimpleEventManager;
use libafl::feedbacks::{EagerOrFeedback, MaxMapFeedback, StateInitializer};
use libafl::fuzzer::Fuzzer;
use libafl::monitors::NopMonitor;
use libafl::schedulers::QueueScheduler;
use libafl::stages::StdMutationalStage;
use libafl::state::{HasCorpus, HasExecutions, HasSolutions};
use libafl::{Evaluator, StdFuzzer};
use libafl_bolts::tuples::tuple_list;
use utils::print_fuzzer_stats;

#[derive(Debug, Parser)]
#[command(author, version, about = "LibAFL-based fuzzer for Aptos Move modules")]
struct Cli {
    /// Path to an ABI file or directory to seed initial inputs
    #[arg(long = "abi-path", value_name = "ABI_PATH")]
    abi_path: Option<PathBuf>,

    /// Path to a compiled Move module to publish before fuzzing
    #[arg(long = "module-path", value_name = "MODULE_PATH")]
    module_path: Option<PathBuf>,

    /// Timeout in seconds (0 = no timeout, run indefinitely)
    #[arg(long = "timeout", short = 't', default_value = "0")]
    timeout_seconds: u64,
}

fn main() {
    let cli = Cli::parse();
    println!("Starting Aptos Move Fuzzer...");

    if cli.timeout_seconds > 0 {
        println!("Timeout: {} seconds", cli.timeout_seconds);
    } else {
        println!("Timeout: None (will run indefinitely, use Ctrl+C to stop)");
    }

    // Setup executor and feedback
    let mut executor = AptosMoveExecutor::new();
    let mut feedback = MaxMapFeedback::new(executor.pc_observer());
    let objective = EagerOrFeedback::new(ShiftOverflowObjective::new(), AbortCodeObjective::new());

    let mon = NopMonitor::new();
    let mut mgr = SimpleEventManager::new(mon);
    let scheduler = QueueScheduler::new();

    let abi = cli
        .abi_path
        .clone()
        .unwrap_or_else(|| panic!("--abi-path is required (no fallback)."));
    let module = cli
        .module_path
        .clone()
        .unwrap_or_else(|| panic!("--module-path is required (no fallback)."));
    let mut state = AptosFuzzerState::new(Some(abi), Some(module));
    let _ = feedback.init_state(&mut state);
    let mut fuzzer = StdFuzzer::new(scheduler, feedback, objective);

    let mutator = AptosFuzzerMutator::default();
    let mut stages = tuple_list!(StdMutationalStage::new(mutator));

    println!(
        "Starting fuzzing loop with {} initial inputs in corpus",
        state.corpus().count()
    );

    // Prefer adding initial seeds via fuzzer.add_input to fire events and reflect
    // in monitor
    let initial_inputs = state.take_initial_inputs();
    for input in initial_inputs {
        let _ = fuzzer
            .add_input(&mut state, &mut executor, &mut mgr, input)
            .expect("failed to add initial input");
    }

    // Setup graceful shutdown
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
        println!("\n[*] Received interrupt signal, shutting down gracefully...");
    })
    .expect("Error setting Ctrl-C handler");

    // Setup timeout thread
    if cli.timeout_seconds > 0 {
        let r = running.clone();
        let timeout_secs = cli.timeout_seconds;
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(timeout_secs));
            r.store(false, Ordering::SeqCst);
        });
    }

    // Main fuzzing loop
    let start_time = Instant::now();
    let mut last_print_time = Instant::now();
    let print_interval = Duration::from_millis(500);

    while running.load(Ordering::SeqCst) {
        match fuzzer.fuzz_one(&mut stages, &mut executor, &mut state, &mut mgr) {
            Ok(_) => {
                if last_print_time.elapsed() >= print_interval {
                    // Read cumulative coverage from state
                    let coverage_map = state.cumulative_coverage();
                    let total_instructions_executed = executor.total_instructions_executed();
                    let total_possible_edges = state.aptos_state().total_possible_edges();
                    print_fuzzer_stats(
                        start_time,
                        *state.executions(),
                        state.corpus().count(),
                        state.solutions().count(),
                        coverage_map,
                        total_instructions_executed,
                        total_possible_edges,
                    );
                    last_print_time = Instant::now();
                }
            }
            Err(e) => {
                eprintln!("Error during fuzzing: {:?}", e);
                break;
            }
        }
    }

    // Print final statistics
    println!("\n[+] Fuzzing completed");
    println!("\nFinal Statistics:");
    let coverage_map = state.cumulative_coverage();
    let total_instructions_executed = executor.total_instructions_executed();
    let total_possible_edges = state.aptos_state().total_possible_edges();
    print_fuzzer_stats(
        start_time,
        *state.executions(),
        state.corpus().count(),
        state.solutions().count(),
        coverage_map,
        total_instructions_executed,
        total_possible_edges,
    );
    let solutions = state.take_solutions();
    if !solutions.is_empty() {
        println!("Discovered solutions:");
        for input in solutions {
            println!("  {:?}", input);
            if let Some(execution_path) = state.get_solution_execution_path(&input) {
                println!("    Execution path: {:?}", execution_path);
                if let Some(path_id) = state.get_solution_execution_path_id(&input) {
                    if state.abort_code_paths.contains(&path_id) {
                        println!("    Found InvariantViolation!");
                    }
                    if state.shift_overflow_paths.contains(&path_id) {
                        println!("    Found ShiftOverflow!");
                    }
                }
            }
        }
    }
}
