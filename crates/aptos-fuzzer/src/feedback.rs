use std::borrow::Cow;
use std::collections::HashSet;

use libafl::feedbacks::{Feedback, StateInitializer};
use libafl::observers::ObserversTuple;
use libafl::Error;
use libafl_bolts::tuples::{Handle, MatchNameRef};
use libafl_bolts::Named;
use serde::{Deserialize, Serialize};

use crate::observers::{AbortCodeObserver, ShiftOverflowObserver};
use crate::{AptosFuzzerInput, AptosFuzzerState};

/// Objective feedback that considers abort codes as objectives
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AbortCodeObjective {
    target_abort_codes: HashSet<u64>,
    name: Cow<'static, str>,
}

impl AbortCodeObjective {
    pub fn new() -> Self {
        Self {
            target_abort_codes: HashSet::new(),
            name: Cow::Borrowed("AbortCodeObjective"),
        }
    }

    pub fn with_target_codes(codes: &[u64]) -> Self {
        Self {
            target_abort_codes: codes.iter().cloned().collect(),
            name: Cow::Borrowed("AbortCodeObjective"),
        }
    }

    pub fn with_name(name: &'static str) -> Self {
        Self {
            target_abort_codes: HashSet::new(),
            name: Cow::Borrowed(name),
        }
    }
}

impl Named for AbortCodeObjective {
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}

impl StateInitializer<AptosFuzzerState> for AbortCodeObjective {}

impl<EM, OT> Feedback<EM, AptosFuzzerInput, OT, AptosFuzzerState> for AbortCodeObjective
where
    OT: ObserversTuple<AptosFuzzerInput, AptosFuzzerState>,
{
    #[allow(clippy::wrong_self_convention)]
    fn is_interesting(
        &mut self,
        state: &mut AptosFuzzerState,
        _manager: &mut EM,
        input: &AptosFuzzerInput,
        observers: &OT,
        exit_kind: &libafl::executors::ExitKind,
    ) -> Result<bool, Error> {
        // Treat VM invariant violations / panics as objectives
        if matches!(exit_kind, libafl::executors::ExitKind::Crash) {
            if let Some(path_id) = state.current_execution_path_id() {
                if !state.mark_execution_path_seen(path_id) {
                    return Ok(false);
                }
                state.record_current_execution_path_for(input);
            }
            return Ok(true);
        }
        // Check if the last execution produced an abort code
        let mut code_opt: Option<u64> = None;
        // Access AbortCodeObserver through Handle
        let abort_handle: Handle<AbortCodeObserver> = Handle::new(Cow::Borrowed("AbortCodeObserver"));
        if let Some(obs_ref) = observers.get(&abort_handle) {
            code_opt = obs_ref.last();
        }
        if let Some(abort_code) = code_opt {
            // If we have specific target codes, only those are objectives
            if !self.target_abort_codes.is_empty() {
                if self.target_abort_codes.contains(&abort_code) {
                    if let Some(path_id) = state.current_execution_path_id() {
                        if !state.mark_execution_path_seen(path_id) {
                            return Ok(false);
                        }
                        state.abort_code_paths.insert(path_id);
                        state.record_current_execution_path_for(input);
                    }
                    return Ok(true);
                }
            } else {
                // If no specific targets, any abort code is an objective
                if let Some(path_id) = state.current_execution_path_id() {
                    if !state.mark_execution_path_seen(path_id) {
                        return Ok(false);
                    }
                    state.abort_code_paths.insert(path_id);
                    state.record_current_execution_path_for(input);
                }
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn append_metadata(
        &mut self,
        _state: &mut AptosFuzzerState,
        _manager: &mut EM,
        _observers: &OT,
        _testcase: &mut libafl::corpus::Testcase<AptosFuzzerInput>,
    ) -> Result<(), Error> {
        // We could add metadata about the abort code to the testcase here
        Ok(())
    }
}

/// Treats shift overflow as a bug.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ShiftOverflowObjective {
    name: Cow<'static, str>,
}

impl ShiftOverflowObjective {
    pub fn new() -> Self {
        Self {
            name: Cow::Borrowed("ShiftOverflowObjective"),
        }
    }
}

impl Named for ShiftOverflowObjective {
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}

impl StateInitializer<AptosFuzzerState> for ShiftOverflowObjective {}

impl<EM, OT> Feedback<EM, AptosFuzzerInput, OT, AptosFuzzerState> for ShiftOverflowObjective
where
    OT: ObserversTuple<AptosFuzzerInput, AptosFuzzerState>,
{
    fn is_interesting(
        &mut self,
        state: &mut AptosFuzzerState,
        _manager: &mut EM,
        input: &AptosFuzzerInput,
        observers: &OT,
        _exit_kind: &libafl::executors::ExitKind,
    ) -> Result<bool, Error> {
        let mut cause_loss = false;
        // Access ShiftOverflowObserver through Handle
        let shift_handle: Handle<ShiftOverflowObserver> = Handle::new(Cow::Borrowed("ShiftOverflowObserver"));
        if let Some(obs_ref) = observers.get(&shift_handle) {
            cause_loss = obs_ref.cause_loss();
        }

        if cause_loss {
            if let Some(path_id) = state.current_execution_path_id() {
                if !state.mark_execution_path_seen(path_id) {
                    return Ok(false);
                }
                state.shift_overflow_paths.insert(path_id);
                state.record_current_execution_path_for(input);
            }
            return Ok(true);
        }
        Ok(false)
    }
}
