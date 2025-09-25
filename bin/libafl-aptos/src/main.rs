use aptos_fuzzer::{AptosFuzzerMutator, AptosFuzzerState, AptosMoveExecutor};
use libafl::corpus::Corpus;
use libafl::events::SimpleEventManager;
use libafl::feedbacks::ConstFeedback;
use libafl::fuzzer::Fuzzer;
use libafl::monitors::SimpleMonitor;
use libafl::schedulers::QueueScheduler;
use libafl::stages::StdMutationalStage;
use libafl::state::HasCorpus;
use libafl::StdFuzzer;
use libafl_bolts::tuples::tuple_list;

fn main() {
    println!("Starting Aptos Move Fuzzer...");
    
    // Use simple constant feedback for now - focus on mutation testing
    // rather than coverage-guided fuzzing
    let feedback = ConstFeedback::new(true);
    let objective = ConstFeedback::new(false);
    
    let mon = SimpleMonitor::new(|s| println!("{s}"));
    let mut mgr = SimpleEventManager::new(mon);
    let scheduler = QueueScheduler::new();

    let mut fuzzer = StdFuzzer::new(scheduler, feedback, objective);

    let mut executor = AptosMoveExecutor::new();
    let mut state = AptosFuzzerState::new();
    let mutator = AptosFuzzerMutator::default();

    let mut stages = tuple_list!(StdMutationalStage::new(mutator));

    println!("Starting fuzzing loop with {} initial inputs in corpus", state.corpus().count());

    fuzzer
        .fuzz_loop(&mut stages, &mut executor, &mut state, &mut mgr)
        .expect("Fuzzing loop failed");
}