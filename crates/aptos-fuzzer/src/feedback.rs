use std::borrow::Cow;
use std::collections::HashSet;

use libafl::feedbacks::{Feedback, StateInitializer};
use libafl::observers::ObserversTuple;
use libafl::Error;
use libafl_bolts::Named;
use serde::{Deserialize, Serialize};

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
    /// Creates a new AbortCodeFeedback
    pub fn new() -> Self {
        Self {
            seen_abort_codes: HashSet::new(),
            name: Cow::Borrowed("AbortCodeFeedback"),
        }
    }

    /// Creates a new AbortCodeFeedback with a custom name
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
        state: &mut AptosFuzzerState,
        _manager: &mut EM,
        _input: &AptosFuzzerInput,
        _observers: &OT,
        exit_kind: &libafl::executors::ExitKind,
    ) -> Result<bool, Error> {
        // Always keep crashers
        if matches!(exit_kind, libafl::executors::ExitKind::Crash) {
            return Ok(true);
        }
        // Check if the last execution produced an abort code
        if let Some(abort_code) = state.last_abort_code() {
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
    /// Creates a new AbortCodeObjective that treats any abort code as an
    /// objective
    pub fn new() -> Self {
        Self {
            target_abort_codes: HashSet::new(),
            name: Cow::Borrowed("AbortCodeObjective"),
        }
    }

    /// Creates a new AbortCodeObjective that only treats specific abort codes
    /// as objectives
    pub fn with_target_codes(codes: &[u64]) -> Self {
        Self {
            target_abort_codes: codes.iter().cloned().collect(),
            name: Cow::Borrowed("AbortCodeObjective"),
        }
    }

    /// Creates a new AbortCodeObjective with a custom name
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
        _input: &AptosFuzzerInput,
        _observers: &OT,
        exit_kind: &libafl::executors::ExitKind,
    ) -> Result<bool, Error> {
        // Treat VM invariant violations / panics as objectives
        if matches!(exit_kind, libafl::executors::ExitKind::Crash) {
            return Ok(true);
        }
        // Check if the last execution produced an abort code
        if let Some(abort_code) = state.last_abort_code() {
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
        Ok(())
    }
}
