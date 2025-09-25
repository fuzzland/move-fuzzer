use std::borrow::Cow;

use libafl::observers::{Observer, ObserverWithHashField};
use libafl_bolts::{AsSlice, Named};
use serde::{Deserialize, Serialize};

use crate::{AptosFuzzerInput, AptosFuzzerState};

/// Simple observer that tracks transaction execution status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionStatusObserver {
    name: Cow<'static, str>,
    /// Status map:
    /// [success_count_low, success_count_high, error_count_low,
    /// error_count_high]
    pub status_map: [u8; 4],
    /// Number of successful executions
    pub success_count: u64,
    /// Number of failed executions  
    pub error_count: u64,
}

impl TransactionStatusObserver {
    pub fn new(name: &'static str) -> Self {
        Self {
            name: Cow::Borrowed(name),
            status_map: [0, 0, 0, 0],
            success_count: 0,
            error_count: 0,
        }
    }

    pub fn set_success(&mut self) {
        self.success_count += 1;
        self.update_status_map();
    }

    pub fn set_error(&mut self) {
        self.error_count += 1;
        self.update_status_map();
    }

    pub fn reset(&mut self) {
        // Don't reset counters, just update the map
        self.update_status_map();
    }

    fn update_status_map(&mut self) {
        // Encode counters into the status map for feedback
        self.status_map[0] = (self.success_count & 0xFF) as u8;
        self.status_map[1] = ((self.success_count >> 8) & 0xFF) as u8;
        self.status_map[2] = (self.error_count & 0xFF) as u8;
        self.status_map[3] = ((self.error_count >> 8) & 0xFF) as u8;
    }
}

impl Named for TransactionStatusObserver {
    fn name(&self) -> &Cow<'static, str> {
        &self.name
    }
}

impl<'a> AsSlice<'a> for TransactionStatusObserver {
    type Entry = u8;
    type SliceRef = &'a [u8];

    fn as_slice(&'a self) -> Self::SliceRef {
        &self.status_map
    }
}

impl Observer<AptosFuzzerInput, AptosFuzzerState> for TransactionStatusObserver {
    fn pre_exec(&mut self, _state: &mut AptosFuzzerState, _input: &AptosFuzzerInput) -> Result<(), libafl::Error> {
        self.reset();
        Ok(())
    }
}

impl ObserverWithHashField for TransactionStatusObserver {
    fn hash(&self) -> Option<u64> {
        None // No hashing needed for simple status
    }
}
