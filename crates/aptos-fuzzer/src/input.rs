use aptos_dynamic_transaction_composer::CallArgument;
use aptos_move_core_types::identifier::Identifier;
use aptos_move_core_types::language_storage::{ModuleId, TypeTag};
use libafl::inputs::Input;
use serde::{Deserialize, Serialize};

/// Represents a single function call with batched-call compatible arguments.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub struct Call {
    pub module_id: ModuleId,
    pub function_name: Identifier,
    pub ty_args: Vec<TypeTag>,
    pub args: Vec<CallArgument>,
}

impl Call {
    pub fn new(module_id: ModuleId, function_name: Identifier, ty_args: Vec<TypeTag>, args: Vec<CallArgument>) -> Self {
        Self {
            module_id,
            function_name,
            ty_args,
            args,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub struct AptosFuzzerInput {
    pub calls: Vec<Call>,
}

impl Input for AptosFuzzerInput {}

impl AptosFuzzerInput {
    pub fn new(call: Call) -> Self {
        Self { calls: vec![call] }
    }

    pub fn from_calls(calls: Vec<Call>) -> Self {
        Self { calls }
    }

    pub fn push(&mut self, call: Call) {
        self.calls.push(call);
    }

    pub fn len(&self) -> usize {
        self.calls.len()
    }

    pub fn is_empty(&self) -> bool {
        self.calls.is_empty()
    }
}
