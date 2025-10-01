use std::borrow::Cow;

use aptos_types::transaction::{EntryFunction, Script, TransactionArgument, TransactionPayload};
use libafl::mutators::{MutationResult, Mutator};
use libafl::state::HasRand;
use libafl_bolts::rands::Rand;
use libafl_bolts::Named;

use crate::input::AptosFuzzerInput;
use crate::state::AptosFuzzerState;

#[derive(Default)]
pub struct AptosFuzzerMutator {}

impl AptosFuzzerMutator {
    fn mutate_entry_function_args(entry_func: &mut EntryFunction, state: &mut AptosFuzzerState) -> bool {
        let args = entry_func.args();
        if args.is_empty() {
            return false;
        }

        // Create new mutated arguments
        let mut new_args = Vec::new();
        let mut mutated = false;

        for arg_bytes in args.iter() {
            let mut mutated_arg = arg_bytes.clone();
            if Self::mutate_byte_vector(&mut mutated_arg, state) {
                mutated = true;
            }
            new_args.push(mutated_arg);
        }

        if mutated {
            // Reconstruct EntryFunction with mutated args
            let (module, function, ty_args, _) = entry_func.clone().into_inner();
            *entry_func = EntryFunction::new(module, function, ty_args, new_args);
        }

        mutated
    }

    /// Mutate Script arguments using state's random source (pure random)
    fn mutate_script_args(script: &mut Script, state: &mut AptosFuzzerState) -> bool {
        let args = script.args();
        if args.is_empty() {
            return false;
        }

        // Create new mutated arguments
        let mut new_args = Vec::new();
        let mut mutated = false;

        for arg in args.iter() {
            let mut mutated_arg = arg.clone();
            if Self::mutate_transaction_argument(&mut mutated_arg, state) {
                mutated = true;
            }
            new_args.push(mutated_arg);
        }

        if mutated {
            // Reconstruct Script with mutated args
            let (code, ty_args, _) = script.clone().into_inner();
            *script = Script::new(code, ty_args, new_args);
        }

        mutated
    }

    /// Mutate a byte vector using state's random source (pure random bytes)
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

    /// Mutate a TransactionArgument using state's random source (pure random)
    fn mutate_transaction_argument(arg: &mut TransactionArgument, state: &mut AptosFuzzerState) -> bool {
        match arg {
            TransactionArgument::U8(val) => {
                *val = (state.rand_mut().next() & 0xFF) as u8;
                true
            }
            TransactionArgument::U16(val) => {
                *val = (state.rand_mut().next() % 65536) as u16;
                true
            }
            TransactionArgument::U32(val) => {
                *val = (state.rand_mut().next() & 0xFFFF_FFFF) as u32;
                true
            }
            TransactionArgument::U64(val) => {
                *val = state.rand_mut().next();
                true
            }
            TransactionArgument::U128(val) => {
                let hi = state.rand_mut().next() as u128;
                let lo = state.rand_mut().next() as u128;
                *val = (hi << 64) | lo;
                true
            }
            TransactionArgument::U256(val) => {
                let high_part = {
                    let hi = state.rand_mut().next() as u128;
                    let lo = state.rand_mut().next() as u128;
                    (hi << 64) | lo
                };
                let low_part = {
                    let hi = state.rand_mut().next() as u128;
                    let lo = state.rand_mut().next() as u128;
                    (hi << 64) | lo
                };
                let mut bytes = [0u8; 32];
                bytes[0..16].copy_from_slice(&low_part.to_le_bytes());
                bytes[16..32].copy_from_slice(&high_part.to_le_bytes());
                *val = aptos_move_core_types::u256::U256::from_le_bytes(&bytes);
                true
            }
            TransactionArgument::Bool(val) => {
                *val = (state.rand_mut().next() & 1) == 0;
                true
            }
            TransactionArgument::Address(_addr) => {
                let mut addr_bytes = [0u8; 32];
                for byte in addr_bytes.iter_mut() {
                    *byte = (state.rand_mut().next() % 256) as u8;
                }
                *_addr = aptos_move_core_types::account_address::AccountAddress::try_from(addr_bytes.to_vec())
                    .unwrap_or(*_addr);
                true
            }
            TransactionArgument::U8Vector(vec) => {
                let len = (state.rand_mut().next() % 64) as usize;
                vec.clear();
                for _ in 0..len {
                    vec.push((state.rand_mut().next() & 0xFF) as u8);
                }
                true
            }
            TransactionArgument::Serialized(bytes) => {
                let len = (state.rand_mut().next() % 64) as usize;
                bytes.clear();
                bytes.resize(len, 0);
                for b in bytes.iter_mut() {
                    *b = (state.rand_mut().next() & 0xFF) as u8;
                }
                true
            }
        }
    }
}

impl Mutator<AptosFuzzerInput, AptosFuzzerState> for AptosFuzzerMutator {
    fn mutate(
        &mut self,
        state: &mut AptosFuzzerState,
        input: &mut AptosFuzzerInput,
    ) -> Result<MutationResult, libafl::Error> {
        let payload = input.payload_mut();
        let mutated = match payload {
            TransactionPayload::EntryFunction(entry_func) => Self::mutate_entry_function_args(entry_func, state),
            TransactionPayload::Script(script) => Self::mutate_script_args(script, state),
            _ => false, // Other payload types not supported for current mutator
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
