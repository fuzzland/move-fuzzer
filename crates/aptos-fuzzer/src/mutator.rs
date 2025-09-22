use std::borrow::Cow;

use crate::input::AptosFuzzerInput;
use crate::state::AptosFuzzerState;
use libafl::mutators::Mutator;
use libafl_bolts::Named;

#[derive(Default)]
pub struct AptosFuzzerMutator {}

impl Mutator<AptosFuzzerInput, AptosFuzzerState> for AptosFuzzerMutator {
    fn mutate(
        &mut self,
        state: &mut AptosFuzzerState,
        input: &mut AptosFuzzerInput,
    ) -> Result<libafl::mutators::MutationResult, libafl::Error> {
        todo!()
    }

    fn post_exec(
        &mut self,
        _state: &mut AptosFuzzerState,
        _new_corpus_id: Option<libafl::corpus::CorpusId>,
    ) -> Result<(), libafl::Error> {
        todo!()
    }
}

impl Named for AptosFuzzerMutator {
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("AptosFuzzerMutator");
        &NAME
    }
}
