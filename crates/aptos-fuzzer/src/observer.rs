use std::borrow::Cow;

use libafl_bolts::Named;
use serde::{Deserialize, Serialize};

use crate::{AptosFuzzerInput, AptosFuzzerState};

const MAP_SIZE: usize = 1 << 16;

/// Observer that records executed Move bytecode indices (pc offsets) per run.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PcIndexObserver {
    name: Cow<'static, str>,
    pcs: Vec<u32>,
    // AFL-style edge hitcount map derived from pcs
    map: Vec<u8>,
    // previous location used for edge hashing
    prev_loc: u32,
}

impl PcIndexObserver {
    pub fn new() -> Self {
        Self {
            name: Cow::Borrowed("PcIndexObserver"),
            pcs: Vec::new(),
            map: vec![0; MAP_SIZE],
            prev_loc: 0,
        }
    }

    pub fn with_name(name: &'static str) -> Self {
        Self {
            name: Cow::Borrowed(name),
            pcs: Vec::new(),
            map: vec![0; MAP_SIZE],
            prev_loc: 0,
        }
    }

    pub fn pcs(&self) -> &Vec<u32> {
        &self.pcs
    }

    pub fn set_pcs(&mut self, pcs: Vec<u32>) {
        self.pcs = pcs;
    }

    /// Returns the internal coverage hitcount map (AFL-style buckets)
    pub fn coverage_map(&self) -> &[u8] {
        &self.map
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
        // Reset coverage map and prev_loc
        for b in &mut self.map {
            *b = 0;
        }
        self.prev_loc = 0;
        Ok(())
    }

    fn post_exec(
        &mut self,
        _state: &mut AptosFuzzerState,
        _input: &AptosFuzzerInput,
        _exit_kind: &libafl::executors::ExitKind,
    ) -> Result<(), libafl::Error> {
        // Fold pcs into AFL-style edge coverage
        for &pc in &self.pcs {
            let cur_id = pc;
            let idx = ((cur_id ^ self.prev_loc) as usize) & (MAP_SIZE - 1);
            let byte = &mut self.map[idx];
            *byte = byte.saturating_add(1);
            self.prev_loc = cur_id >> 1;
        }
        Ok(())
    }
}
