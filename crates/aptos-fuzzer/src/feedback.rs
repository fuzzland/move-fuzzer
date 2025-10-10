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

/// Feedback that tracks abort codes encountered during execution.
/// Considers an input interesting if it produces a new abort code that hasn't
/// been seen before.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AbortCodeFeedback {
    seen_abort_codes: HashSet<u64>,
    name: Cow<'static, str>,
}

impl AbortCodeFeedback {
    pub fn new() -> Self {
        Self {
            seen_abort_codes: HashSet::new(),
            name: Cow::Borrowed("AbortCodeFeedback"),
        }
    }

    pub fn with_name(name: &'static str) -> Self {
        Self {
            seen_abort_codes: HashSet::new(),
            name: Cow::Borrowed(name),
        }
    }
}

impl Named for AbortCodeFeedback {
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}

impl StateInitializer<AptosFuzzerState> for AbortCodeFeedback {}

impl<EM, OT> Feedback<EM, AptosFuzzerInput, OT, AptosFuzzerState> for AbortCodeFeedback
where
    OT: ObserversTuple<AptosFuzzerInput, AptosFuzzerState>,
{
    #[allow(clippy::wrong_self_convention)]
    fn is_interesting(
        &mut self,
        _state: &mut AptosFuzzerState,
        _manager: &mut EM,
        _input: &AptosFuzzerInput,
        observers: &OT,
        exit_kind: &libafl::executors::ExitKind,
    ) -> Result<bool, Error> {
        // Always keep crashers
        if matches!(exit_kind, libafl::executors::ExitKind::Crash) {
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
            // If this is a new abort code we haven't seen before, it's interesting
            if !self.seen_abort_codes.contains(&abort_code) {
                self.seen_abort_codes.insert(abort_code);
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
        _state: &mut AptosFuzzerState,
        _manager: &mut EM,
        _input: &AptosFuzzerInput,
        observers: &OT,
        exit_kind: &libafl::executors::ExitKind,
    ) -> Result<bool, Error> {
        // Treat VM invariant violations / panics as objectives
        if matches!(exit_kind, libafl::executors::ExitKind::Crash) {
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
                    return Ok(true);
                }
            } else {
                // If no specific targets, any abort code is an objective
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

/// Marks inputs with shift overflow as interesting.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ShiftOverflowFeedback {
    name: Cow<'static, str>,
}

impl ShiftOverflowFeedback {
    pub fn new() -> Self {
        Self {
            name: Cow::Borrowed("ShiftOverflowFeedback"),
        }
    }
}

impl Named for ShiftOverflowFeedback {
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}

impl StateInitializer<AptosFuzzerState> for ShiftOverflowFeedback {}

impl<EM, OT> Feedback<EM, AptosFuzzerInput, OT, AptosFuzzerState> for ShiftOverflowFeedback
where
    OT: ObserversTuple<AptosFuzzerInput, AptosFuzzerState>,
{
    fn is_interesting(
        &mut self,
        _state: &mut AptosFuzzerState,
        _manager: &mut EM,
        _input: &AptosFuzzerInput,
        observers: &OT,
        _exit_kind: &libafl::executors::ExitKind,
    ) -> Result<bool, Error> {
        let mut cause_loss = false;
        // Access ShiftOverflowObserver through Handle
        let shift_handle: Handle<ShiftOverflowObserver> = Handle::new(Cow::Borrowed("ShiftOverflowObserver"));
        if let Some(obs_ref) = observers.get(&shift_handle) {
            cause_loss = obs_ref.cause_loss();
        }
        Ok(cause_loss)
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
        _state: &mut AptosFuzzerState,
        _manager: &mut EM,
        _input: &AptosFuzzerInput,
        observers: &OT,
        _exit_kind: &libafl::executors::ExitKind,
    ) -> Result<bool, Error> {
        let mut cause_loss = false;
        // Access ShiftOverflowObserver through Handle
        let shift_handle: Handle<ShiftOverflowObserver> = Handle::new(Cow::Borrowed("ShiftOverflowObserver"));
        if let Some(obs_ref) = observers.get(&shift_handle) {
            cause_loss = obs_ref.cause_loss();
        }
        Ok(cause_loss)
    }
}
