use std::borrow::Cow;

use libafl_bolts::Named;
use serde::{Deserialize, Serialize};

use crate::{AptosFuzzerInput, AptosFuzzerState};

/// Observer that records executed Move bytecode indices (pc offsets) per run.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PcIndexObserver {
    name: Cow<'static, str>,
    pcs: Vec<u32>,
}

impl PcIndexObserver {
    pub fn new() -> Self {
        Self {
            name: Cow::Borrowed("PcIndexObserver"),
            pcs: Vec::new(),
        }
    }

    pub fn with_name(name: &'static str) -> Self {
        Self {
            name: Cow::Borrowed(name),
            pcs: Vec::new(),
        }
    }

    pub fn pcs(&self) -> &Vec<u32> {
        &self.pcs
    }

    pub fn set_pcs(&mut self, pcs: Vec<u32>) {
        self.pcs = pcs;
    }
}

impl Named for PcIndexObserver {
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}

impl libafl::observers::Observer<AptosFuzzerInput, AptosFuzzerState> for PcIndexObserver {
    fn pre_exec(&mut self, _state: &mut AptosFuzzerState, _input: &AptosFuzzerInput) -> Result<(), libafl::Error> {
        // Clear previous pcs before each run
        self.pcs.clear();
        Ok(())
    }

    fn post_exec(
        &mut self,
        _state: &mut AptosFuzzerState,
        _input: &AptosFuzzerInput,
        _exit_kind: &libafl::executors::ExitKind,
    ) -> Result<(), libafl::Error> {
        Ok(())
    }
}
