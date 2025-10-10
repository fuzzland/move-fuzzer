use std::path::PathBuf;

use aptos_fuzzer::{
    AbortCodeFeedback, AbortCodeObjective, AptosFuzzerMutator, AptosFuzzerState, AptosMoveExecutor,
    ShiftOverflowObjective,
};
use clap::Parser;
use libafl::corpus::Corpus;
use libafl::events::SimpleEventManager;
use libafl::feedbacks::{EagerOrFeedback, MaxMapFeedback, StateInitializer};
use libafl::fuzzer::Fuzzer;
use libafl::monitors::SimpleMonitor;
use libafl::schedulers::QueueScheduler;
use libafl::stages::StdMutationalStage;
use libafl::state::HasCorpus;
use libafl::{Evaluator, StdFuzzer};
use libafl_bolts::tuples::tuple_list;

#[derive(Debug, Parser)]
#[command(author, version, about = "LibAFL-based fuzzer for Aptos Move modules")]
struct Cli {
    /// Path to an ABI file or directory to seed initial inputs
    #[arg(long = "abi-path", value_name = "ABI_PATH")]
    abi_path: Option<PathBuf>,

    /// Path to a compiled Move module to publish before fuzzing
    #[arg(long = "module-path", value_name = "MODULE_PATH")]
    module_path: Option<PathBuf>,
}

fn main() {
    let cli = Cli::parse();
    println!("Starting Aptos Move Fuzzer...");

    // Build coverage feedback on top of executor's pc observer
    let mut executor = AptosMoveExecutor::new();
    let cov_feedback = MaxMapFeedback::new(executor.pc_observer());
    let mut feedback = EagerOrFeedback::new(cov_feedback, AbortCodeFeedback::new());
    let objective = EagerOrFeedback::new(ShiftOverflowObjective::new(), AbortCodeObjective::new());

    let mon = SimpleMonitor::new(|s| println!("{s}"));
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

    // Prefer adding initial seeds via fuzzer.add_input to fire events and reflect in monitor
    let initial_inputs = state.take_initial_inputs();
    for input in initial_inputs {
        let _ = fuzzer
            .add_input(&mut state, &mut executor, &mut mgr, input)
            .expect("failed to add initial input");
    }

    fuzzer
        .fuzz_loop(&mut stages, &mut executor, &mut state, &mut mgr)
        .expect("Fuzzing loop failed");
}
