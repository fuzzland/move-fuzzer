use std::borrow::Cow;
use std::collections::HashSet;

use aptos_move_core_types::account_address::AccountAddress;
use aptos_move_core_types::identifier::Identifier;
use aptos_move_core_types::language_storage::ModuleId;
use libafl::mutators::{MutationResult, Mutator};
use libafl::state::HasRand;
use libafl_bolts::rands::Rand;
use libafl_bolts::Named;

use crate::input::{AptosFuzzerInput, FuncCall};
use crate::state::AptosFuzzerState;

#[derive(Default)]
pub struct AptosFuzzerMutator {}

impl AptosFuzzerMutator {
    fn mutate_call_args(input: &mut AptosFuzzerInput, state: &mut AptosFuzzerState) -> bool {
        if input.calls.is_empty() {
            return false;
        }
        
        let call_idx = (state.rand_mut().next() as usize) % input.calls.len();
        let func_call = &mut input.calls[call_idx];
        
        if func_call.bcs_args.is_empty() {
            return false;
        }

        let mut mutated = false;
        for arg_bytes in func_call.bcs_args.iter_mut() {
            if Self::mutate_byte_vector(arg_bytes, state) {
                mutated = true;
            }
        }

        mutated
    }

    fn mutate_byte_vector(bytes: &mut Vec<u8>, state: &mut AptosFuzzerState) -> bool {
        let len = if bytes.is_empty() {
            // choose a small random length
            (1 + (state.rand_mut().next() % 16)) as usize
        } else {
            // keep current length
            bytes.len()
        };
        bytes.resize(len, 0);
        for b in bytes.iter_mut() {
            *b = (state.rand_mut().next() & 0xFF) as u8;
        }
        true
    }
    
    fn mutate_add_call_from_chain(state: &mut AptosFuzzerState, input: &mut AptosFuzzerInput) -> bool {
        let chain = match state.chain() {
            Some(c) => c.clone(),
            None => return false,
        };
        
        if chain.is_empty() {
            return false;
        }
        
        let call_idx = (state.rand_mut().next() as usize) % chain.len();
        let call = &chain.calls[call_idx];
        
        let func_call = match Self::call_to_func_call(call) {
            Ok(fc) => fc,
            Err(_) => return false,
        };
        
        let insert_pos = Self::find_insertion_position(&chain, &input.calls, call_idx);
        input.calls.insert(insert_pos, func_call);
        
        true
    }
    
    fn mutate_shuffle_calls(state: &mut AptosFuzzerState, input: &mut AptosFuzzerInput) -> bool {
        if input.calls.len() < 2 {
            return false;
        }
        
        let chain = match state.chain() {
            Some(c) => c.clone(),
            None => {
                let idx1 = (state.rand_mut().next() as usize) % input.calls.len();
                let idx2 = (state.rand_mut().next() as usize) % input.calls.len();
                if idx1 != idx2 {
                    input.calls.swap(idx1, idx2);
                    return true;
                }
                return false;
            }
        };
        
        for _ in 0..10 {
            let idx1 = (state.rand_mut().next() as usize) % input.calls.len();
            let idx2 = (state.rand_mut().next() as usize) % input.calls.len();
            
            if idx1 == idx2 {
                continue;
            }
            
            if Self::can_swap_safely(&chain, &input.calls, idx1, idx2) {
                input.calls.swap(idx1, idx2);
                return true;
            }
        }
        
        false
    }
    
    fn call_to_func_call(call: &crate::mir::Call) -> Result<FuncCall, String> {
        let addr_bytes = hex::decode(&call.module_addr)
            .map_err(|e| format!("Invalid module address: {}", e))?;
        if addr_bytes.len() != 32 {
            return Err(format!("Module address must be 32 bytes, got {}", addr_bytes.len()));
        }
        let mut addr_array = [0u8; 32];
        addr_array.copy_from_slice(&addr_bytes);
        let module_addr = AccountAddress::new(addr_array);
        
        let module_name = Identifier::new(call.module.as_str())
            .map_err(|e| format!("Invalid module name: {}", e))?;
        let function_name = Identifier::new(call.function.as_str())
            .map_err(|e| format!("Invalid function name: {}", e))?;
        
        let module_id = ModuleId::new(module_addr, module_name);
        let args = vec![vec![0u8; 8]; call.args.len()];
        
        Ok(FuncCall::new(module_id, function_name, vec![], args))
    }
    
    fn find_insertion_position(chain: &crate::mir::Chain, calls: &[FuncCall], call_idx: usize) -> usize {
        let call = &chain.calls[call_idx];
        
        let mut requires_structs = HashSet::new();
        for fact in &call.requires {
            if let crate::mir::Fact::Exists(res) = fact {
                requires_structs.insert(&res.struct_name);
            }
        }
        
        if requires_structs.is_empty() {
            return 0;
        }
        
        let mut latest_creator_pos = 0;
        
        for (pos, _entry_func) in calls.iter().enumerate() {
            for struct_name in &requires_structs {
                let creators = chain.calls_creating_struct(struct_name);
                if !creators.is_empty() {
                    latest_creator_pos = pos + 1;
                }
            }
        }
        
        latest_creator_pos.min(calls.len())
    }
    
    fn can_swap_safely(
        _chain: &crate::mir::Chain,
        _calls: &[FuncCall],
        idx1: usize,
        idx2: usize,
    ) -> bool {
        let (_earlier, later) = if idx1 < idx2 { (idx1, idx2) } else { (idx2, idx1) };
        
        if later - idx1.min(idx2) == 1 {
            return true;
        }
        
        true
    }
}

impl Mutator<AptosFuzzerInput, AptosFuzzerState> for AptosFuzzerMutator {
    fn mutate(
        &mut self,
        state: &mut AptosFuzzerState,
        input: &mut AptosFuzzerInput,
    ) -> Result<MutationResult, libafl::Error> {
        let choice = state.rand_mut().next() % 100;
        
        let mutated = if choice < 60 {
            Self::mutate_call_args(input, state)
        } else if choice < 80 {
            Self::mutate_add_call_from_chain(state, input)
        } else {
            Self::mutate_shuffle_calls(state, input)
        };

        if mutated {
            Ok(MutationResult::Mutated)
        } else {
            Ok(MutationResult::Skipped)
        }
    }

    fn post_exec(
        &mut self,
        _state: &mut AptosFuzzerState,
        _new_corpus_id: Option<libafl::corpus::CorpusId>,
    ) -> Result<(), libafl::Error> {
        Ok(())
    }
}

impl Named for AptosFuzzerMutator {
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("AptosFuzzerMutator");
        &NAME
    }
}
