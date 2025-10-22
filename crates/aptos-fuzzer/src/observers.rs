use std::borrow::Cow;

use libafl::observers::Observer;
use libafl_bolts::Named;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AbortCodeObserver {
    name: Cow<'static, str>,
    last: Option<u64>,
}

impl AbortCodeObserver {
    pub fn new() -> Self {
        Self {
            name: Cow::Borrowed("AbortCodeObserver"),
            last: None,
        }
    }

    pub fn last(&self) -> Option<u64> {
        self.last
    }

    pub fn set_last(&mut self, v: Option<u64>) {
        self.last = v;
    }
}

impl Named for AbortCodeObserver {
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}

impl<I, S> Observer<I, S> for AbortCodeObserver {
    fn pre_exec(&mut self, _state: &mut S, _input: &I) -> Result<(), libafl::Error> {
        self.last = None;
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ShiftOverflowObserver {
    name: Cow<'static, str>,
    cause_loss: bool,
}

impl ShiftOverflowObserver {
    pub fn new() -> Self {
        Self {
            name: Cow::Borrowed("ShiftOverflowObserver"),
            cause_loss: false,
        }
    }

    pub fn cause_loss(&self) -> bool {
        self.cause_loss
    }

    pub fn set_cause_loss(&mut self, v: bool) {
        self.cause_loss = v;
    }
}

impl Named for ShiftOverflowObserver {
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}

impl<I, S> Observer<I, S> for ShiftOverflowObserver {
    fn pre_exec(&mut self, _state: &mut S, _input: &I) -> Result<(), libafl::Error> {
        self.cause_loss = false;
        Ok(())
    }
}
