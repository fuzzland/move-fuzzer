use std::borrow::Cow;

use aptos_types::transaction::{EntryFunction, Script, TransactionArgument, TransactionPayload};
use libafl::mutators::{MutationResult, Mutator};
use libafl_bolts::Named;

use crate::input::AptosFuzzerInput;
use crate::state::AptosFuzzerState;

#[derive(Default)]
pub struct AptosFuzzerMutator {}

impl AptosFuzzerMutator {
    fn mutate_entry_function_args(entry_func: &mut EntryFunction) -> bool {
        let args = entry_func.args();
        if args.is_empty() {
            return false;
        }

        // Create new mutated arguments
        let mut new_args = Vec::new();
        let mut mutated = false;

        for arg_bytes in args.iter() {
            let mut mutated_arg = arg_bytes.clone();
            if Self::mutate_byte_vector(&mut mutated_arg) {
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

    /// Mutate Script arguments by flipping bits in TransactionArguments
    fn mutate_script_args(script: &mut Script) -> bool {
        let args = script.args();
        if args.is_empty() {
            return false;
        }

        // Create new mutated arguments
        let mut new_args = Vec::new();
        let mut mutated = false;

        for arg in args.iter() {
            let mut mutated_arg = arg.clone();
            if Self::mutate_transaction_argument(&mut mutated_arg) {
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

    /// Mutate a byte vector by flipping all bits
    fn mutate_byte_vector(bytes: &mut Vec<u8>) -> bool {
        if bytes.is_empty() {
            *bytes = vec![0u8; 8];
        }

        for byte in bytes.iter_mut() {
            *byte = !*byte;
        }
        true
    }

    /// Mutate a TransactionArgument by flipping bits in its data
    fn mutate_transaction_argument(arg: &mut TransactionArgument) -> bool {
        match arg {
            TransactionArgument::U8(val) => {
                *val = !*val;
                true
            }
            TransactionArgument::U16(val) => {
                *val = !*val;
                true
            }
            TransactionArgument::U32(val) => {
                *val = !*val;
                true
            }
            TransactionArgument::U64(val) => {
                *val = !*val;
                true
            }
            TransactionArgument::U128(val) => {
                *val = !*val;
                true
            }
            TransactionArgument::U256(val) => {
                // Flip all bits in U256
                let bytes = val.to_le_bytes();
                let flipped_bytes: [u8; 32] = bytes.map(|b| !b);
                *val = aptos_move_core_types::u256::U256::from_le_bytes(&flipped_bytes);
                true
            }
            TransactionArgument::Bool(val) => {
                *val = !*val;
                true
            }
            TransactionArgument::Address(addr) => {
                let mut addr_bytes = addr.to_vec();
                for byte in addr_bytes.iter_mut() {
                    *byte = !*byte;
                }
                *addr = aptos_move_core_types::account_address::AccountAddress::try_from(addr_bytes).unwrap_or(*addr);
                true
            }
            TransactionArgument::U8Vector(vec) => {
                if vec.is_empty() {
                    *vec = vec![0u8; 8];
                }
                // Flip all bits in the vector
                for byte in vec.iter_mut() {
                    *byte = !*byte;
                }
                true
            }
            TransactionArgument::Serialized(bytes) => {
                if bytes.is_empty() {
                    *bytes = vec![0u8; 8];
                }
                // Flip all bits
                for byte in bytes.iter_mut() {
                    *byte = !*byte;
                }
                true
            }
        }
    }
}

impl Mutator<AptosFuzzerInput, AptosFuzzerState> for AptosFuzzerMutator {
    fn mutate(
        &mut self,
        _state: &mut AptosFuzzerState,
        input: &mut AptosFuzzerInput,
    ) -> Result<MutationResult, libafl::Error> {
        let payload = input.payload_mut();
        let mutated = match payload {
            TransactionPayload::EntryFunction(entry_func) => Self::mutate_entry_function_args(entry_func),
            TransactionPayload::Script(script) => Self::mutate_script_args(script),
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
        // No post-execution cleanup needed for current mutator
        Ok(())
    }
}

impl Named for AptosFuzzerMutator {
    fn name(&self) -> &Cow<'static, str> {
        static NAME: Cow<'static, str> = Cow::Borrowed("AptosFuzzerMutator");
        &NAME
    }
}
