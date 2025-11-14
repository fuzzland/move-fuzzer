use std::cell::{Ref, RefMut};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use aptos_move_binary_format::CompiledModule;
use aptos_move_core_types::language_storage::ModuleId;
use libafl::corpus::{Corpus, CorpusId, HasCurrentCorpusId, HasTestcase, InMemoryCorpus, Testcase};
use libafl::stages::StageId;
use libafl::state::{
    HasCorpus, HasCurrentStageId, HasExecutions, HasImported, HasLastFoundTime, HasLastReportTime, HasRand,
    HasSolutions, HasStartTime, StageStack, Stoppable,
};
use libafl::{HasMetadata, HasNamedMetadata};
use libafl_bolts::rands::StdRand;
use libafl_bolts::serdeany::{NamedSerdeAnyMap, SerdeAnyMap};

use crate::executor::aptos_custom_state::{AptosCustomState, LayerSnapshot};
use crate::input::AptosFuzzerInput;
use crate::mir::Chain;

// AFL-style map size constant
pub const MAP_SIZE: usize = 1 << 16;

// Similar to libafl::state::StdState
pub struct AptosFuzzerState {
    // RNG instance
    rand: StdRand,
    /// How many times the executor ran the harness/target
    executions: u64,
    /// At what time the fuzzing started
    start_time: Duration,
    /// the number of new paths that imported from other fuzzers
    imported: usize,
    /// The corpus
    corpus: InMemoryCorpus<AptosFuzzerInput>,
    /// Solution corpus
    solutions: InMemoryCorpus<AptosFuzzerInput>,
    /// Execution path captured during the most recent run
    current_execution_path: Option<Vec<u32>>,
    current_execution_path_id: Option<u64>,
    /// Execution paths recorded for interesting inputs
    execution_paths_by_input: HashMap<AptosFuzzerInput, ExecutionPathRecord>,
    /// Execution path IDs observed so far to deduplicate interesting inputs
    seen_execution_paths: HashSet<u64>,
    /// Snapshot from the most recent execution (pending promotion)
    pending_snapshot: Option<LayerSnapshot>,
    /// Interesting per-path snapshots
    snapshots_by_path: HashMap<u64, LayerSnapshot>,
    /// Metadata stored for this state by one of the components
    metadata: SerdeAnyMap,
    /// Metadata stored with names
    named_metadata: NamedSerdeAnyMap,
    /// The last time something was added to the corpus
    last_found_time: Duration,
    /// The last time we reported progress (if available/used).
    /// This information is used by fuzzer `maybe_report_progress`.
    last_report_time: Option<Duration>,
    /// The current index of the corpus; used to record for resumable fuzzing.
    corpus_id: Option<CorpusId>,
    /// Request the fuzzer to stop at the start of the next stage
    /// or at the beginning of the next fuzzing iteration
    stop_requested: bool,
    stage_stack: StageStack,

    /// Aptos specific fields
    aptos_state: AptosCustomState,
    /// Cumulative coverage map for statistics (Observer map resets each
    /// execution)
    cumulative_coverage: Vec<u8>,
    /// Execution path IDs that triggered abort-code objectives
    pub abort_code_paths: HashSet<u64>,
    /// Execution path IDs that triggered shift overflow objectives
    pub shift_overflow_paths: HashSet<u64>,
    /// MIR Chain loaded from file
    chain: Option<Chain>,
}

#[derive(Clone)]
struct ExecutionPathRecord {
    id: u64,
    path: Vec<u32>,
}

impl AptosFuzzerState {
    /// Load Chain from MIR JSON file and populate corpus
    pub fn load_from_mir(mir_path: Option<PathBuf>, module_path: Option<PathBuf>) -> Self {
        let module_bytes = Self::load_module_from_path(module_path.clone());
        let mut state = Self {
            aptos_state: AptosCustomState::new_default(),
            rand: StdRand::new(),
            executions: 0,
            start_time: Duration::from_secs(0),
            imported: 0,
            corpus: InMemoryCorpus::new(),
            solutions: InMemoryCorpus::new(),
            current_execution_path: None,
            current_execution_path_id: None,
            execution_paths_by_input: HashMap::new(),
            seen_execution_paths: HashSet::new(),
            pending_snapshot: None,
            snapshots_by_path: HashMap::new(),
            abort_code_paths: HashSet::new(),
            shift_overflow_paths: HashSet::new(),
            metadata: SerdeAnyMap::new(),
            named_metadata: NamedSerdeAnyMap::new(),
            last_found_time: Duration::from_secs(0),
            last_report_time: None,
            corpus_id: None,
            stop_requested: false,
            stage_stack: StageStack::default(),
            cumulative_coverage: vec![0u8; MAP_SIZE],
            chain: None,
        };

        if let Some((module_id, code)) = module_bytes {
            state.aptos_state.deploy_module_bytes(module_id, code);
        }

        // Load MIR Chain and convert to inputs
        if let Some(mir_path) = mir_path {
            match Self::load_chain_from_mir(&mir_path) {
                Ok(mut chain) => {
                    let call_count = chain.len();
                    println!("Loaded MIR chain with {} calls", call_count);

                    chain.sort_by_dependencies();

                    match chain.to_calls() {
                        Ok(func_calls) => {
                            // Seed corpus with each MIR call as a single-call input
                            for (idx, func_call) in func_calls.into_iter().enumerate() {
                                let input = AptosFuzzerInput::new(func_call);
                                let _ = state.corpus.add(Testcase::new(input));
                                if (idx + 1) % 10 == 0 || idx + 1 == call_count {
                                    println!("  Added {}/{} individual calls to corpus", idx + 1, call_count);
                                }
                            }
                            println!(
                                "Successfully seeded corpus with {} single-call inputs",
                                state.corpus.count()
                            );
                        }
                        Err(e) => {
                            eprintln!("Failed to convert MIR chain to func calls: {}", e);
                        }
                    }
                    state.chain = Some(chain);
                }
                Err(e) => {
                    eprintln!("Failed to load MIR from {:?}: {}", mir_path, e);
                }
            }
        }

        state
    }

    fn load_chain_from_mir(path: &Path) -> Result<crate::mir::Chain, String> {
        let content = fs::read_to_string(path).map_err(|e| format!("Failed to read MIR file: {}", e))?;
        let chain: crate::mir::Chain =
            serde_json::from_str(&content).map_err(|e| format!("Failed to parse MIR JSON: {}", e))?;
        Ok(chain)
    }

    /// Drain corpus entries for re-insertion via fuzzer.add_input
    pub fn take_initial_inputs(&mut self) -> Vec<AptosFuzzerInput> {
        let ids: Vec<_> = self.corpus().ids().collect();
        let mut inputs = Vec::with_capacity(ids.len());
        for id in ids {
            if let Ok(input) = self.corpus().cloned_input_for_id(id) {
                inputs.push(input);
            }
        }
        // Clear existing entries
        while let Some(id) = self.corpus().ids().next() {
            let _ = self.corpus_mut().remove(id);
        }
        inputs
    }

    pub fn aptos_state(&self) -> &AptosCustomState {
        &self.aptos_state
    }

    pub fn aptos_state_mut(&mut self) -> &mut AptosCustomState {
        &mut self.aptos_state
    }

    pub fn cumulative_coverage(&self) -> &[u8] {
        &self.cumulative_coverage
    }

    pub fn cumulative_coverage_mut(&mut self) -> &mut [u8] {
        &mut self.cumulative_coverage
    }

    pub fn chain(&self) -> Option<&Chain> {
        self.chain.as_ref()
    }

    pub fn chain_mut(&mut self) -> Option<&mut Chain> {
        self.chain.as_mut()
    }

    pub fn set_chain(&mut self, chain: Chain) {
        self.chain = Some(chain);
    }

    pub fn take_chain(&mut self) -> Option<Chain> {
        self.chain.take()
    }

    pub fn take_solutions(&self) -> Vec<AptosFuzzerInput> {
        let solutions = self.solutions();
        let mut seen_ids = HashSet::new();
        let mut inputs = Vec::new();
        for id in solutions.ids() {
            if let Ok(input) = solutions.cloned_input_for_id(id) {
                if let Some(record) = self.execution_paths_by_input.get(&input) {
                    if seen_ids.insert(record.id) {
                        inputs.push(input);
                    }
                }
            }
        }
        inputs
    }

    pub fn clear_current_execution_path(&mut self) {
        self.current_execution_path = None;
        self.current_execution_path_id = None;
    }

    pub fn set_current_execution_path(&mut self, execution_path: Vec<u32>) {
        let id = Self::compute_execution_path_id(&execution_path);
        self.current_execution_path = Some(execution_path);
        self.current_execution_path_id = Some(id);
    }

    pub fn current_execution_path_id(&self) -> Option<u64> {
        self.current_execution_path_id
    }

    pub fn record_current_execution_path_for(&mut self, input: &AptosFuzzerInput) -> Option<u64> {
        match (self.current_execution_path_id, self.current_execution_path.as_ref()) {
            (Some(id), Some(path)) => {
                self.execution_paths_by_input
                    .entry(input.clone())
                    .or_insert_with(|| ExecutionPathRecord { id, path: path.clone() });
                Some(id)
            }
            _ => None,
        }
    }

    pub fn mark_execution_path_seen(&mut self, path_id: u64) -> bool {
        self.seen_execution_paths.insert(path_id)
    }

    pub fn has_seen_execution_path(&self, path_id: u64) -> bool {
        self.seen_execution_paths.contains(&path_id)
    }

    pub fn discard_pending_snapshot(&mut self) {
        self.pending_snapshot = None;
    }

    pub fn set_pending_snapshot(&mut self, snapshot: Option<LayerSnapshot>) {
        self.pending_snapshot = snapshot;
    }

    pub fn promote_pending_snapshot(&mut self, path_id: u64) {
        if let Some(snapshot) = self.pending_snapshot.take() {
            self.snapshots_by_path.insert(path_id, snapshot);
        }
    }

    pub fn snapshot_for_path(&self, path_id: &u64) -> Option<&LayerSnapshot> {
        self.snapshots_by_path.get(path_id)
    }

    pub fn get_solution_execution_path(&self, input: &AptosFuzzerInput) -> Option<Vec<u32>> {
        self.execution_paths_by_input
            .get(input)
            .map(|record| record.path.clone())
    }

    pub fn get_solution_execution_path_id(&self, input: &AptosFuzzerInput) -> Option<u64> {
        self.execution_paths_by_input.get(input).map(|record| record.id)
    }

    pub fn compute_execution_path_id(execution_path: &[u32]) -> u64 {
        const FNV_OFFSET: u64 = 0xcbf29ce484222325;
        const FNV_PRIME: u64 = 0x100000001b3;
        execution_path.iter().fold(FNV_OFFSET, |hash, value| {
            let hash = hash ^ (*value as u64);
            hash.wrapping_mul(FNV_PRIME)
        })
    }
}

// initial inputs
impl HasCorpus<AptosFuzzerInput> for AptosFuzzerState {
    type Corpus = InMemoryCorpus<AptosFuzzerInput>;

    fn corpus(&self) -> &InMemoryCorpus<AptosFuzzerInput> {
        &self.corpus
    }

    fn corpus_mut(&mut self) -> &mut InMemoryCorpus<AptosFuzzerInput> {
        &mut self.corpus
    }
}

impl HasRand for AptosFuzzerState {
    type Rand = StdRand;

    fn rand(&self) -> &StdRand {
        &self.rand
    }

    fn rand_mut(&mut self) -> &mut StdRand {
        &mut self.rand
    }
}

impl HasCurrentCorpusId for AptosFuzzerState {
    fn set_corpus_id(&mut self, id: CorpusId) -> Result<(), libafl::Error> {
        self.corpus_id = Some(id);
        Ok(())
    }

    fn clear_corpus_id(&mut self) -> Result<(), libafl::Error> {
        self.corpus_id = None;
        Ok(())
    }

    fn current_corpus_id(&self) -> Result<Option<CorpusId>, libafl::Error> {
        Ok(self.corpus_id)
    }
}

impl Stoppable for AptosFuzzerState {
    fn stop_requested(&self) -> bool {
        self.stop_requested
    }

    fn request_stop(&mut self) {
        self.stop_requested = true;
    }

    fn discard_stop_request(&mut self) {
        self.stop_requested = false;
    }
}

impl HasMetadata for AptosFuzzerState {
    fn metadata_map(&self) -> &SerdeAnyMap {
        &self.metadata
    }

    fn metadata_map_mut(&mut self) -> &mut SerdeAnyMap {
        &mut self.metadata
    }
}

impl HasNamedMetadata for AptosFuzzerState {
    fn named_metadata_map(&self) -> &NamedSerdeAnyMap {
        &self.named_metadata
    }

    fn named_metadata_map_mut(&mut self) -> &mut NamedSerdeAnyMap {
        &mut self.named_metadata
    }
}

impl HasExecutions for AptosFuzzerState {
    fn executions(&self) -> &u64 {
        &self.executions
    }

    fn executions_mut(&mut self) -> &mut u64 {
        &mut self.executions
    }
}

impl HasLastFoundTime for AptosFuzzerState {
    fn last_found_time(&self) -> &Duration {
        &self.last_found_time
    }

    fn last_found_time_mut(&mut self) -> &mut Duration {
        &mut self.last_found_time
    }
}

// inputs that can trigger a bug
impl HasSolutions<AptosFuzzerInput> for AptosFuzzerState {
    type Solutions = InMemoryCorpus<AptosFuzzerInput>;
    fn solutions(&self) -> &InMemoryCorpus<AptosFuzzerInput> {
        &self.solutions
    }

    fn solutions_mut(&mut self) -> &mut InMemoryCorpus<AptosFuzzerInput> {
        &mut self.solutions
    }
}

impl HasTestcase<AptosFuzzerInput> for AptosFuzzerState {
    fn testcase(&self, id: CorpusId) -> Result<Ref<'_, Testcase<AptosFuzzerInput>>, libafl::Error> {
        Ok(self.corpus().get(id)?.borrow())
    }

    fn testcase_mut(&self, id: CorpusId) -> Result<RefMut<'_, Testcase<AptosFuzzerInput>>, libafl::Error> {
        Ok(self.corpus().get(id)?.borrow_mut())
    }
}

impl HasImported for AptosFuzzerState {
    fn imported(&self) -> &usize {
        &self.imported
    }

    fn imported_mut(&mut self) -> &mut usize {
        &mut self.imported
    }
}

impl HasLastReportTime for AptosFuzzerState {
    fn last_report_time(&self) -> &Option<Duration> {
        &self.last_report_time
    }

    fn last_report_time_mut(&mut self) -> &mut Option<Duration> {
        &mut self.last_report_time
    }
}

impl HasCurrentStageId for AptosFuzzerState {
    fn set_current_stage_id(&mut self, id: StageId) -> Result<(), libafl::Error> {
        self.stage_stack.set_current_stage_id(id)
    }

    fn clear_stage_id(&mut self) -> Result<(), libafl::Error> {
        self.stage_stack.clear_stage_id()
    }

    fn current_stage_id(&self) -> Result<Option<StageId>, libafl::Error> {
        self.stage_stack.current_stage_id()
    }
}

impl HasStartTime for AptosFuzzerState {
    fn start_time(&self) -> &Duration {
        &self.start_time
    }

    fn start_time_mut(&mut self) -> &mut Duration {
        &mut self.start_time
    }
}

impl AptosFuzzerState {
    fn load_module_from_path(path: Option<PathBuf>) -> Option<(ModuleId, Vec<u8>)> {
        let path = path?;
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(_) => return None,
        };

        let module = match CompiledModule::deserialize(bytes.as_slice()) {
            Ok(module) => module,
            Err(_) => return None,
        };

        let module_id = module.self_id();

        Some((module_id, bytes))
    }
}
