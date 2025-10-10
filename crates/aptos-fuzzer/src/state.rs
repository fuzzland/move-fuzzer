use std::cell::{Ref, RefMut};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use aptos_move_binary_format::CompiledModule;
use aptos_move_core_types::account_address::AccountAddress;
use aptos_move_core_types::identifier::Identifier;
use aptos_move_core_types::language_storage::{ModuleId, TypeTag};
use aptos_move_core_types::u256::U256;
use aptos_types::transaction::{EntryABI, EntryFunction as AptosEntryFunction, EntryFunctionABI, TransactionPayload};
use libafl::corpus::{Corpus, CorpusId, HasCurrentCorpusId, HasTestcase, InMemoryCorpus, Testcase};
use libafl::stages::StageId;
use libafl::state::{
    HasCorpus, HasCurrentStageId, HasExecutions, HasImported, HasLastFoundTime, HasLastReportTime, HasRand,
    HasSolutions, HasStartTime, StageStack, Stoppable,
};
use libafl::{HasMetadata, HasNamedMetadata};
use libafl_bolts::rands::StdRand;
use libafl_bolts::serdeany::{NamedSerdeAnyMap, SerdeAnyMap};

use crate::executor::aptos_custom_state::AptosCustomState;
use crate::input::AptosFuzzerInput;

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
}

impl AptosFuzzerState {
    pub fn new(abi_path: Option<PathBuf>, module_path: Option<PathBuf>) -> Self {
        let entry_abis = Self::load_abis_from_path(abi_path);
        let module_bytes = Self::load_module_from_path(module_path);
        let mut state = Self {
            // TODO: replace me with actual aptos state
            aptos_state: AptosCustomState::new_default(),
            rand: StdRand::new(),
            executions: 0,
            start_time: Duration::from_secs(0),
            imported: 0,
            corpus: InMemoryCorpus::new(),
            solutions: InMemoryCorpus::new(),
            metadata: SerdeAnyMap::new(),
            named_metadata: NamedSerdeAnyMap::new(),
            last_found_time: Duration::from_secs(0),
            last_report_time: None,
            corpus_id: None,
            stop_requested: false,
            stage_stack: StageStack::default(),
        };

        if let Some((module_id, code)) = module_bytes {
            state.aptos_state.deploy_module_bytes(module_id, code);
        }

        for payload in Self::padding_abis(entry_abis) {
            let input = AptosFuzzerInput::new(payload);
            let _ = state.corpus.add(Testcase::new(input));
        }

        state
    }

    /// Drain current corpus entries into a vector of inputs and clear the
    /// corpus. Useful to re-insert seeds via fuzzer.add_input so
    /// events/feedback are fired.
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

    // no per-execution flags kept in state anymore
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
    fn load_abis_from_path(path: Option<PathBuf>) -> Vec<EntryFunctionABI> {
        let Some(path) = path else {
            return Vec::new();
        };

        let mut paths = Vec::new();
        let mut abis = Vec::new();
        Self::collect_abis(path.as_path(), &mut paths, &mut abis);
        abis
    }

    fn collect_abis(path: &Path, paths: &mut Vec<PathBuf>, abis: &mut Vec<EntryFunctionABI>) {
        if path.is_dir() {
            let read_dir = match fs::read_dir(path) {
                Ok(rd) => rd,
                Err(_) => return,
            };
            for entry in read_dir {
                match entry {
                    Ok(dir_entry) => Self::collect_abis(&dir_entry.path(), paths, abis),
                    Err(err) => eprintln!("[aptos-fuzzer] failed to read entry in {}: {err}", path.display()),
                }
            }
            return;
        }

        if path.extension().map(|ext| ext != "abi").unwrap_or(true) {
            return;
        }

        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(err) => {
                eprintln!("[aptos-fuzzer] failed to read ABI file {}: {err}", path.display());
                return;
            }
        };
        // Try to decode as EntryABI first (new format from aptos move compile)
        match bcs::from_bytes::<EntryABI>(&bytes) {
            Ok(entry_abi) => {
                if let EntryABI::EntryFunction(abi) = entry_abi {
                    paths.push(path.to_path_buf());
                    abis.push(abi);
                }
            }
            Err(_) => {
                // Fallback: try to decode as EntryFunctionABI directly (legacy format)
                if let Ok(abi) = bcs::from_bytes::<EntryFunctionABI>(&bytes) {
                    paths.push(path.to_path_buf());
                    abis.push(abi);
                }
            }
        }
    }

    fn padding_abis(abis: Vec<EntryFunctionABI>) -> Vec<TransactionPayload> {
        let mut payloads = Vec::new();

        for abi in abis {
            if !abi.ty_args().is_empty() {
                continue;
            }

            let identifier = match Identifier::new(abi.name()) {
                Ok(id) => id,
                Err(_) => continue,
            };

            let mut arg_bytes = Vec::new();
            let mut unsupported = false;

            for arg in abi.args() {
                match Self::default_arg_bytes(arg.type_tag()) {
                    Some(bytes) => arg_bytes.push(bytes),
                    None => {
                        unsupported = true;
                        eprintln!(
                            "[aptos-fuzzer] skipping {}::{}: unsupported argument type {:?}",
                            abi.module_name(),
                            abi.name(),
                            arg.type_tag()
                        );
                        break;
                    }
                }
            }

            if unsupported {
                continue;
            }

            while arg_bytes.len() < abi.args().len() {
                // Fallback to empty vector<u8> if some type was not covered
                if let Ok(bytes) = bcs::to_bytes::<Vec<u8>>(&Vec::new()) {
                    arg_bytes.push(bytes);
                } else {
                    break;
                }
            }

            let entry = AptosEntryFunction::new(abi.module_name().clone(), identifier, Vec::new(), arg_bytes);
            payloads.push(TransactionPayload::EntryFunction(entry));
        }

        payloads
    }

    fn default_arg_bytes(type_tag: &TypeTag) -> Option<Vec<u8>> {
        match type_tag {
            TypeTag::Bool => bcs::to_bytes(&false).ok(),
            TypeTag::U8 => bcs::to_bytes(&0u8).ok(),
            TypeTag::U16 => bcs::to_bytes(&0u16).ok(),
            TypeTag::U32 => bcs::to_bytes(&0u32).ok(),
            TypeTag::U64 => bcs::to_bytes(&0u64).ok(),
            TypeTag::U128 => bcs::to_bytes(&0u128).ok(),
            TypeTag::U256 => bcs::to_bytes(&U256::from(0u8)).ok(),
            TypeTag::Address => bcs::to_bytes(&AccountAddress::ZERO).ok(),
            TypeTag::Vector(inner) => match &**inner {
                TypeTag::Bool => bcs::to_bytes::<Vec<bool>>(&Vec::new()).ok(),
                TypeTag::U8 => bcs::to_bytes::<Vec<u8>>(&Vec::new()).ok(),
                TypeTag::U16 => bcs::to_bytes::<Vec<u16>>(&Vec::new()).ok(),
                TypeTag::U32 => bcs::to_bytes::<Vec<u32>>(&Vec::new()).ok(),
                TypeTag::U64 => bcs::to_bytes::<Vec<u64>>(&Vec::new()).ok(),
                TypeTag::U128 => bcs::to_bytes::<Vec<u128>>(&Vec::new()).ok(),
                TypeTag::U256 => bcs::to_bytes::<Vec<U256>>(&Vec::new()).ok(),
                TypeTag::Address => bcs::to_bytes::<Vec<AccountAddress>>(&Vec::new()).ok(),
                _ => None,
            },
            _ => None,
        }
    }

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
