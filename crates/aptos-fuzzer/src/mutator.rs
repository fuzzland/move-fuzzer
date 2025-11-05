use std::borrow::Cow;
use std::collections::HashSet;

use aptos_dynamic_transaction_composer::CallArgument;
use aptos_move_core_types::account_address::AccountAddress;
use aptos_move_core_types::identifier::Identifier;
use aptos_move_core_types::language_storage::ModuleId;
use libafl::mutators::{MutationResult, Mutator};
use libafl::state::HasRand;
use libafl_bolts::rands::Rand;
use libafl_bolts::Named;
use aptos_move_binary_format::access::ModuleAccess;
use aptos_move_core_types::language_storage::TypeTag;
use aptos_move_core_types::value::MoveValue;
use aptos_move_vm_runtime::ModuleStorage;

use crate::input::{AptosFuzzerInput, Call};
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
        // load parameter signature tokens for this function
        let param_toks = Self::param_tokens_for_call(state, func_call);
        let mut mutated = false;
        for (idx, arg) in func_call.args.iter_mut().enumerate() {
            if let CallArgument::Raw(ref mut bytes) = arg {
                if let Some(ref toks) = param_toks {
                    if let Some(tok) = toks.get(idx) {
                        if Self::mutate_arg_bytes_for_token(bytes, tok, state) {
                            mutated = true;
                            continue;
                        }
                    }
                }
                if Self::mutate_byte_vector(bytes, state) { mutated = true; }
            }
        }

        mutated
    }

    fn mutate_arg_bytes_for_token(
        bytes: &mut Vec<u8>,
        tok: &aptos_move_binary_format::file_format::SignatureToken,
        state: &mut AptosFuzzerState,
    ) -> bool {
        use aptos_move_binary_format::file_format::SignatureToken as ST;
        match tok {
            ST::Vector(inner) => {
                // Allow free byte mutation only for vector<u8>
                if matches!(**inner, ST::U8) {
                    return Self::mutate_byte_vector(bytes, state);
                }
                // For vector<T> with T != u8, perform structured mutation with valid BCS
                let choice = state.rand_mut().next() % 3;
                let len = match choice { 0 => 0usize, 1 => 1usize, _ => 2usize };
                let mut elems = Vec::with_capacity(len);
                for _ in 0..len {
                    if let Some(tag) = Self::token_to_typetag_primitive(inner) {
                        // generate a small random element for primitive types
                        let mv = Self::random_move_value_for_tag(&tag, state);
                        if let Some(mv) = mv { elems.push(mv) } else { elems.push(MoveValue::U8(0)) }
                    } else {
                        elems.push(MoveValue::U8(0));
                    }
                }
                let mv = MoveValue::Vector(elems);
                if let Some(bcs) = mv.simple_serialize() { *bytes = bcs; return true; }
                false
            }
            // For fixed-size primitives, clamp length to expected and fill random
            ST::Bool | ST::U8 | ST::U16 | ST::U32 | ST::U64 | ST::U128 | ST::U256 | ST::Address => {
                let tag = Self::token_to_typetag_primitive(tok).unwrap();
                if let Some(expected) = Self::expected_fixed_len_bytes(&tag) {
                    let mut v = vec![0u8; expected];
                    for b in v.iter_mut() { *b = (state.rand_mut().next() & 0xFF) as u8; }
                    *bytes = v;
                    return true;
                }
                false
            }
            _ => false,
        }
    }

    fn token_to_typetag_primitive(
        tok: &aptos_move_binary_format::file_format::SignatureToken,
    ) -> Option<TypeTag> {
        use aptos_move_binary_format::file_format::SignatureToken as ST;
        match tok {
            ST::Bool => Some(TypeTag::Bool),
            ST::U8 => Some(TypeTag::U8),
            ST::U16 => Some(TypeTag::U16),
            ST::U32 => Some(TypeTag::U32),
            ST::U64 => Some(TypeTag::U64),
            ST::U128 => Some(TypeTag::U128),
            ST::U256 => Some(TypeTag::U256),
            ST::Address => Some(TypeTag::Address),
            ST::Vector(inner) => Self::token_to_typetag_primitive(inner).map(|t| TypeTag::Vector(Box::new(t))),
            _ => None,
        }
    }

    fn random_move_value_for_tag(tag: &TypeTag, state: &mut AptosFuzzerState) -> Option<MoveValue> {
        match tag {
            TypeTag::Bool => Some(MoveValue::Bool((state.rand_mut().next() & 1) != 0)),
            TypeTag::U8 => Some(MoveValue::U8((state.rand_mut().next() & 0xFF) as u8)),
            TypeTag::U16 => Some(MoveValue::U16((state.rand_mut().next() & 0xFFFF) as u16)),
            TypeTag::U32 => Some(MoveValue::U32((state.rand_mut().next() & 0xFFFF_FFFF) as u32)),
            TypeTag::U64 => Some(MoveValue::U64(state.rand_mut().next() as u64)),
            TypeTag::U128 => Some(MoveValue::U128((state.rand_mut().next() as u128) << 64)),
            TypeTag::U256 => Some(MoveValue::U256(0u128.into())),
            TypeTag::Address => Some(MoveValue::Address(AccountAddress::random())),
            _ => None,
        }
    }

    fn expected_fixed_len_bytes(tag: &TypeTag) -> Option<usize> {
        match tag {
            TypeTag::Bool => Some(1),
            TypeTag::U8 => Some(1),
            TypeTag::U16 => Some(2),
            TypeTag::U32 => Some(4),
            TypeTag::U64 => Some(8),
            TypeTag::U128 => Some(16),
            TypeTag::U256 => Some(32),
            TypeTag::Address => Some(32),
            _ => None,
        }
    }

    fn param_tokens_for_call(state: &AptosFuzzerState, call: &Call) -> Option<Vec<aptos_move_binary_format::file_format::SignatureToken>> {
        let cm = state
            .aptos_state()
            .unmetered_get_deserialized_module(call.module_id.address(), call.module_id.name())
            .ok()
            .flatten()?;
        for def in cm.function_defs() {
            let h = cm.function_handle_at(def.function);
            if cm.identifier_at(h.name) == call.function_name.as_ident_str() {
                return Some(cm.signature_at(h.parameters).0.clone());
            }
        }
        None
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

        let func_call = match Self::call_to_call(call) {
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

    fn call_to_call(call: &crate::mir::Call) -> Result<Call, String> {
        let addr_bytes = hex::decode(&call.module_addr).map_err(|e| format!("Invalid module address: {}", e))?;
        if addr_bytes.len() != 32 {
            return Err(format!("Module address must be 32 bytes, got {}", addr_bytes.len()));
        }
        let mut addr_array = [0u8; 32];
        addr_array.copy_from_slice(&addr_bytes);
        let module_addr = AccountAddress::new(addr_array);

        let module_name = Identifier::new(call.module.as_str()).map_err(|e| format!("Invalid module name: {}", e))?;
        let function_name =
            Identifier::new(call.function.as_str()).map_err(|e| format!("Invalid function name: {}", e))?;

        let module_id = ModuleId::new(module_addr, module_name);
        let args = vec![CallArgument::Raw(vec![0u8; 8]); call.args.len()];
        Ok(Call::new(module_id, function_name, vec![], args))
    }

    fn find_insertion_position(chain: &crate::mir::Chain, calls: &[Call], call_idx: usize) -> usize {
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

    fn can_swap_safely(_chain: &crate::mir::Chain, _calls: &[Call], idx1: usize, idx2: usize) -> bool {
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
