use std::path::PathBuf;

use aptos_fuzzer::{AbortCodeFeedback, AbortCodeObjective, AptosFuzzerMutator, AptosFuzzerState, AptosMoveExecutor};
use clap::Parser;
use libafl::corpus::Corpus;
use libafl::events::SimpleEventManager;
use libafl::fuzzer::Fuzzer;
use libafl::monitors::SimpleMonitor;
use libafl::schedulers::QueueScheduler;
use libafl::stages::StdMutationalStage;
use libafl::state::HasCorpus;
// use libafl::evaluators::Evaluator; // old path; not present in this version
use libafl::StdFuzzer;
use libafl_bolts::tuples::tuple_list;
use libafl::Evaluator; // bring trait for evaluate_input into scope

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

    // Use abort code feedback to track new abort codes and find bugs
    let feedback = AbortCodeFeedback::new();
    let objective = AbortCodeObjective::new();

    let mon = SimpleMonitor::new(|s| println!("{s}"));
    let mut mgr = SimpleEventManager::new(mon);
    let scheduler = QueueScheduler::new();

    let mut fuzzer = StdFuzzer::new(scheduler, feedback, objective);

    let mut executor = AptosMoveExecutor::new();
    let mut state = AptosFuzzerState::new(cli.abi_path, cli.module_path);
    let mutator = AptosFuzzerMutator::default();

    let mut stages = tuple_list!(StdMutationalStage::new(mutator));

    println!(
        "Starting fuzzing loop with {} initial inputs in corpus",
        state.corpus().count()
    );

    // Pre-execute all initial corpus inputs once (seed evaluation)
    let ids: Vec<_> = state.corpus().ids().collect();
    for id in ids {
        let input = state
            .corpus()
            .cloned_input_for_id(id)
            .expect("failed to clone input");
        let _ = libafl::Evaluator::evaluate_input(&mut fuzzer, &mut state, &mut executor, &mut mgr, &input)
            .expect("failed to evaluate initial input");
    }

    fuzzer
        .fuzz_loop(&mut stages, &mut executor, &mut state, &mut mgr)
        .expect("Fuzzing loop failed");
}
