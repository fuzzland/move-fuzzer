use std::cell::{Ref, RefMut};
use std::time::Duration;

use libafl::corpus::{CorpusId, HasCurrentCorpusId, HasTestcase, InMemoryCorpus, Testcase};
use libafl::stages::StageId;
use libafl::state::{
    HasCorpus, HasCurrentStageId, HasExecutions, HasImported, HasLastFoundTime, HasLastReportTime, HasRand,
    HasSolutions, Stoppable,
};
use libafl::{HasMetadata, HasNamedMetadata};
use libafl_bolts::rands::StdRand;
use libafl_bolts::serdeany::{NamedSerdeAnyMap, SerdeAnyMap};

use crate::executor::aptos_custom_state::AptosCustomState;
use crate::input::AptosFuzzerInput;

pub struct AptosFuzzerState {
    corpus: InMemoryCorpus<AptosFuzzerInput>,
    solutions: InMemoryCorpus<AptosFuzzerInput>,

    rand: StdRand,
    aptos_state: AptosCustomState,
    stop_requested: bool,
    metadata_map: SerdeAnyMap,
    named_metadata_map: NamedSerdeAnyMap,
    last_found_time: Duration,
    last_report_time: Option<Duration>,
    executions: u64,
    imported: usize,
}

impl AptosFuzzerState {
    pub fn new() -> Self {
        Self {
            corpus: InMemoryCorpus::new(),
            solutions: InMemoryCorpus::new(),
            rand: StdRand::new(),
            aptos_state: AptosCustomState::new_default(),
            stop_requested: false,
            metadata_map: SerdeAnyMap::new(),
            named_metadata_map: NamedSerdeAnyMap::new(),
            last_found_time: Duration::from_secs(0),
            last_report_time: None,
            executions: 0,
        }
    }

    pub fn aptos_state(&self) -> &AptosCustomState {
        &self.aptos_state
    }

    pub fn aptos_state_mut(&mut self) -> &mut AptosCustomState {
        &mut self.aptos_state
    }
}

impl Default for AptosFuzzerState {
    fn default() -> Self {
        Self::new()
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
        todo!()
    }

    fn clear_corpus_id(&mut self) -> Result<(), libafl::Error> {
        todo!()
    }

    fn current_corpus_id(&self) -> Result<Option<CorpusId>, libafl::Error> {
        todo!()
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
        &self.metadata_map
    }

    fn metadata_map_mut(&mut self) -> &mut SerdeAnyMap {
        &mut self.metadata_map
    }
}

impl HasNamedMetadata for AptosFuzzerState {
    fn named_metadata_map(&self) -> &NamedSerdeAnyMap {
        &self.named_metadata_map
    }

    fn named_metadata_map_mut(&mut self) -> &mut NamedSerdeAnyMap {
        &mut self.named_metadata_map
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
        todo!()
    }

    fn testcase_mut(&self, id: CorpusId) -> Result<RefMut<'_, Testcase<AptosFuzzerInput>>, libafl::Error> {
        todo!()
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
        todo!()
    }

    fn clear_stage_id(&mut self) -> Result<(), libafl::Error> {
        todo!()
    }

    fn current_stage_id(&self) -> Result<Option<StageId>, libafl::Error> {
        todo!()
    }
}
