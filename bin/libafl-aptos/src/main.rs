use std::path::PathBuf;
use std::env;

use aptos_fuzzer::{AbortCodeFeedback, AbortCodeObjective, AptosFuzzerMutator, AptosFuzzerState, AptosMoveExecutor};
use libafl::feedbacks::{EagerOrFeedback, MaxMapFeedback};
use libafl::observers::map::HitcountsMapObserver;
use libafl::Evaluator;
use clap::Parser;
use libafl::corpus::Corpus;
use libafl::events::SimpleEventManager;
use libafl::fuzzer::Fuzzer;
use libafl::monitors::SimpleMonitor;
use libafl::schedulers::QueueScheduler;
use libafl::stages::StdMutationalStage;
use libafl::state::HasCorpus;
use libafl::StdFuzzer;
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
    let feedback = EagerOrFeedback::new(cov_feedback, AbortCodeFeedback::new());
    let objective = AbortCodeObjective::new();

    let mon = SimpleMonitor::new(|s| println!("{s}"));
    let mut mgr = SimpleEventManager::new(mon);
    let scheduler = QueueScheduler::new();

    let mut fuzzer = StdFuzzer::new(scheduler, feedback, objective);

    let mut state = AptosFuzzerState::new(cli.abi_path.clone(), cli.module_path.clone());

    // Fallback: if no seeds loaded, try the demo ABI directory in contracts/
    if state.corpus().count() == 0 {
        let mut default_abi = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        default_abi.push("contracts/aptos-demo/build/aptos-demo/abis/aptos_demo");
        if default_abi.is_dir() {
            // Also try to locate the compiled module to publish
            let mut default_mod = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            default_mod.push("contracts/aptos-demo/build/aptos-demo/bytecode_modules/shl_demo.mv");
            println!(
                "No seeds provided. Falling back to demo ABI directory: {}",
                default_abi.display()
            );
            let module_path = if default_mod.is_file() {
                Some(default_mod)
            } else {
                cli.module_path.clone()
            };
            state = AptosFuzzerState::new(Some(default_abi), module_path);
        } else {
            eprintln!(
                "Warning: No initial inputs found. Provide --abi-path to seed the corpus."
            );
        }
    }
    let mutator = AptosFuzzerMutator::default();

    let mut stages = tuple_list!(StdMutationalStage::new(mutator));

    println!(
        "Starting fuzzing loop with {} initial inputs in corpus",
        state.corpus().count()
    );

    // Prefer adding initial seeds via fuzzer.add_input to fire events and reflect in monitor
    if state.corpus().count() > 0 {
        let ids: Vec<_> = state.corpus().ids().collect();
        let mut initial_inputs = Vec::new();
        for id in ids {
            if let Ok(input) = state.corpus().cloned_input_for_id(id) {
                initial_inputs.push(input);
            }
        }
        // Clear current corpus and re-add via fuzzer.add_input
        while let Some(id) = state.corpus().ids().next() {
            let _ = state.corpus_mut().remove(id);
        }
        for input in initial_inputs {
            let _ = fuzzer
                .add_input(&mut state, &mut executor, &mut mgr, input)
                .expect("failed to add initial input");
        }
    }

    fuzzer
        .fuzz_loop(&mut stages, &mut executor, &mut state, &mut mgr)
        .expect("Fuzzing loop failed");
}
