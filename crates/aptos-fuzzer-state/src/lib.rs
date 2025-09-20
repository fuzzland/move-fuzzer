use aptos_fuzzer_input::AptosFuzzerInput;
use libafl::corpus::{CorpusId, HasCurrentCorpusId, HasTestcase, InMemoryCorpus};
use libafl::state::{
    HasCorpus, HasCurrentStageId, HasExecutions, HasImported, HasLastFoundTime, HasLastReportTime, HasRand,
    HasSolutions, Stoppable,
};
use libafl::{HasMetadata, HasNamedMetadata};
use libafl_bolts::rands::StdRand;

pub struct AptosFuzzerState {
    corpus: InMemoryCorpus<AptosFuzzerInput>,
    rand: StdRand,
}

impl AptosFuzzerState {
    pub fn new() -> Self {
        Self {
            corpus: InMemoryCorpus::new(),
            rand: StdRand::new(),
        }
    }
}

impl Default for AptosFuzzerState {
    fn default() -> Self {
        Self::new()
    }
}

impl HasCorpus<AptosFuzzerInput> for AptosFuzzerState {
    type Corpus = InMemoryCorpus<AptosFuzzerInput>;

    fn corpus(&self) -> &Self::Corpus {
        &self.corpus
    }

    fn corpus_mut(&mut self) -> &mut Self::Corpus {
        &mut self.corpus
    }
}

impl HasRand for AptosFuzzerState {
    type Rand = StdRand;

    fn rand(&self) -> &Self::Rand {
        &self.rand
    }

    fn rand_mut(&mut self) -> &mut Self::Rand {
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
        todo!()
    }

    fn request_stop(&mut self) {
        todo!()
    }

    fn discard_stop_request(&mut self) {
        todo!()
    }
}

impl HasMetadata for AptosFuzzerState {
    fn metadata_map(&self) -> &libafl_bolts::serdeany::SerdeAnyMap {
        todo!()
    }

    fn metadata_map_mut(&mut self) -> &mut libafl_bolts::serdeany::SerdeAnyMap {
        todo!()
    }
}

impl HasNamedMetadata for AptosFuzzerState {
    fn named_metadata_map(&self) -> &libafl_bolts::serdeany::NamedSerdeAnyMap {
        todo!()
    }

    fn named_metadata_map_mut(&mut self) -> &mut libafl_bolts::serdeany::NamedSerdeAnyMap {
        todo!()
    }
}

impl HasExecutions for AptosFuzzerState {
    fn executions(&self) -> &u64 {
        todo!()
    }

    fn executions_mut(&mut self) -> &mut u64 {
        todo!()
    }
}

impl HasLastFoundTime for AptosFuzzerState {
    fn last_found_time(&self) -> &std::time::Duration {
        todo!()
    }

    fn last_found_time_mut(&mut self) -> &mut std::time::Duration {
        todo!()
    }
}

impl HasSolutions<AptosFuzzerInput> for AptosFuzzerState {
    // todo check if this is correct
    type Solutions = InMemoryCorpus<AptosFuzzerInput>;

    fn solutions(&self) -> &Self::Solutions {
        todo!()
    }

    fn solutions_mut(&mut self) -> &mut Self::Solutions {
        todo!()
    }
}

impl HasTestcase<AptosFuzzerInput> for AptosFuzzerState {
    fn testcase(
        &self,
        id: CorpusId,
    ) -> Result<std::cell::Ref<'_, libafl::corpus::Testcase<AptosFuzzerInput>>, libafl::Error> {
        todo!()
    }

    fn testcase_mut(
        &self,
        id: CorpusId,
    ) -> Result<std::cell::RefMut<'_, libafl::corpus::Testcase<AptosFuzzerInput>>, libafl::Error> {
        todo!()
    }
}

impl HasImported for AptosFuzzerState {
    fn imported(&self) -> &usize {
        todo!()
    }

    fn imported_mut(&mut self) -> &mut usize {
        todo!()
    }
}

impl HasLastReportTime for AptosFuzzerState {
    fn last_report_time(&self) -> &Option<std::time::Duration> {
        todo!()
    }

    fn last_report_time_mut(&mut self) -> &mut Option<std::time::Duration> {
        todo!()
    }
}

impl HasCurrentStageId for AptosFuzzerState {
    fn set_current_stage_id(&mut self, id: libafl::stages::StageId) -> Result<(), libafl::Error> {
        todo!()
    }

    fn clear_stage_id(&mut self) -> Result<(), libafl::Error> {
        todo!()
    }

    fn current_stage_id(&self) -> Result<Option<libafl::stages::StageId>, libafl::Error> {
        todo!()
    }
}
