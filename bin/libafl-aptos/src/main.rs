use aptos_fuzzer_executor::aptos_move_executor::AptosMoveExecutor;
use aptos_fuzzer_mutator::AptosFuzzerMutator;
use aptos_fuzzer_state::AptosFuzzerState;
use libafl::events::SimpleEventManager;
use libafl::feedbacks::MaxMapFeedback;
use libafl::fuzzer::Fuzzer;
use libafl::monitors::SimpleMonitor;
use libafl::observers::StdMapObserver;
use libafl::schedulers::QueueScheduler;
use libafl::stages::StdMutationalStage;
use libafl::{feedback_and_fast, StdFuzzer};
use libafl_bolts::tuples::tuple_list;

static mut MAP_COVERAGE: [u8; 16] = [0; 16];
#[allow(static_mut_refs)] // only a problem in nightly
static mut MAP_COVERAGE_PTR: *mut u8 = unsafe { MAP_COVERAGE.as_mut_ptr() };

fn main() {
    #[allow(static_mut_refs)] // only a problem in nightly
    let observer = unsafe { StdMapObserver::from_mut_ptr("dummy_map", MAP_COVERAGE_PTR, MAP_COVERAGE.len()) };
    // TODO: besides coverage e.g. object touched
    let feedback = MaxMapFeedback::new(&observer);

    let objective = feedback_and_fast!(
        // TODO: add crash feedback
        MaxMapFeedback::with_name("on_coverage", &observer)
    );
    let mon = SimpleMonitor::new(|s| println!("{s}"));
    let mut mgr = SimpleEventManager::new(mon);
    let scheduler = QueueScheduler::new();

    let mut fuzzer = StdFuzzer::new(scheduler, feedback, objective);

    let mut executor = AptosMoveExecutor::new();
    let mut state = AptosFuzzerState::new();
    let mutator = AptosFuzzerMutator::default();

    let mut stages = tuple_list!(StdMutationalStage::new(mutator));

    fuzzer
        .fuzz_loop(&mut stages, &mut executor, &mut state, &mut mgr)
        .expect("Fuzzing loop failed");
}
