use aptos_move_core_types::identifier::Identifier;
use aptos_move_core_types::language_storage::{ModuleId, TypeTag};
use aptos_types::transaction::EntryFunction;
use libafl::inputs::Input;
use serde::{Deserialize, Serialize};

/// Represents a single function call with all necessary parameters.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub struct FuncCall {
    pub module_id: ModuleId,
    pub function_name: Identifier,
    pub ty_args: Vec<TypeTag>,
    pub bcs_args: Vec<Vec<u8>>,
}

impl FuncCall {
    pub fn new(
        module_id: ModuleId,
        function_name: Identifier,
        ty_args: Vec<TypeTag>,
        bcs_args: Vec<Vec<u8>>,
    ) -> Self {
        Self {
            module_id,
            function_name,
            ty_args,
            bcs_args,
        }
    }
    
    /// Convert to EntryFunction for execution
    pub fn to_entry_function(&self) -> EntryFunction {
        EntryFunction::new(
            self.module_id.clone(),
            self.function_name.clone(),
            self.ty_args.clone(),
            self.bcs_args.clone(),
        )
    }
}


#[derive(Debug, Clone, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub struct AptosFuzzerInput {
    pub calls: Vec<FuncCall>,
}

impl Input for AptosFuzzerInput {}

impl AptosFuzzerInput {
    pub fn new(call: FuncCall) -> Self {
        Self { calls: vec![call] }
    }
    
    pub fn from_calls(calls: Vec<FuncCall>) -> Self {
        Self { calls }
    }
    
    pub fn from_entry_function(entry_func: EntryFunction) -> Self {
        let (module_id, function_name, ty_args, bcs_args) = entry_func.into_inner();
        Self {
            calls: vec![FuncCall::new(module_id, function_name, ty_args, bcs_args)],
        }
    }
    
    pub fn push(&mut self, call: FuncCall) {
        self.calls.push(call);
    }
    
    pub fn len(&self) -> usize {
        self.calls.len()
    }
    
    pub fn is_empty(&self) -> bool {
        self.calls.is_empty()
    }
}
