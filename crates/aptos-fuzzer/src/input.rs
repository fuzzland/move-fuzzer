use aptos_types::transaction::EntryFunction;
use libafl::inputs::Input;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub struct AptosFuzzerInput {
    pub calls: Vec<EntryFunction>,
}

impl Input for AptosFuzzerInput {}

impl AptosFuzzerInput {
    pub fn new(call: EntryFunction) -> Self {
        Self { calls: vec![call] }
    }
    
    pub fn from_calls(calls: Vec<EntryFunction>) -> Self {
        Self { calls }
    }
    
    pub fn push(&mut self, call: EntryFunction) {
        self.calls.push(call);
    }
    
    pub fn len(&self) -> usize {
        self.calls.len()
    }
    
    pub fn is_empty(&self) -> bool {
        self.calls.is_empty()
    }
}
