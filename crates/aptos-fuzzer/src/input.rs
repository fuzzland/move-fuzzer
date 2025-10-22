use aptos_types::transaction::TransactionPayload;
use libafl::inputs::Input;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Hash, Eq, PartialEq, Deserialize, Serialize)]
pub struct AptosFuzzerInput {
    payload: TransactionPayload,
}

impl Input for AptosFuzzerInput {}

// Currently we only support TransactionPayload::EntryFunction
// TODO: add script
impl AptosFuzzerInput {
    pub fn new(payload: TransactionPayload) -> Self {
        Self { payload }
    }

    pub fn payload(&self) -> &TransactionPayload {
        &self.payload
    }

    pub fn payload_mut(&mut self) -> &mut TransactionPayload {
        &mut self.payload
    }
}
