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

    /// Mutate Script arguments using state's random source and ensuring
    /// increasing values
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

    /// Mutate a byte vector using state's random source and ensuring increasing
    /// values
    fn mutate_byte_vector(bytes: &mut Vec<u8>, state: &mut AptosFuzzerState) -> bool {
        if bytes.is_empty() {
            // Generate monotonic u64 using mutator-local logic (no nested types)
            let value = Self::next_u64_for_mutation(state);
            *bytes = bcs::to_bytes(&value).unwrap_or_else(|_| vec![0u8; 8]);
            return true;
        }

        // Try to decode as different types and replace with increasing values
        // First try u64 (most common in our contract)
        if bytes.len() == 8 {
            let new_value = Self::next_u64_for_mutation(state);
            if let Ok(new_bytes) = bcs::to_bytes(&new_value) {
                *bytes = new_bytes;
                return true;
            }
        }

        // Try u32
        if bytes.len() == 4 {
            let new_value = Self::next_u32_for_mutation(state);
            if let Ok(new_bytes) = bcs::to_bytes(&new_value) {
                *bytes = new_bytes;
                return true;
            }
        }

        // Try u8
        if bytes.len() == 1 {
            let new_value = Self::next_u8_for_mutation(state);
            if let Ok(new_bytes) = bcs::to_bytes(&new_value) {
                *bytes = new_bytes;
                return true;
            }
        }

        // For other sizes, generate new u64 value
        let new_value = Self::next_u64_for_mutation(state);
        if let Ok(new_bytes) = bcs::to_bytes(&new_value) {
            *bytes = new_bytes;
            return true;
        }

        false
    }

    /// Mutate a TransactionArgument using state's random source and ensuring
    /// increasing values
    fn mutate_transaction_argument(arg: &mut TransactionArgument, state: &mut AptosFuzzerState) -> bool {
        match arg {
            TransactionArgument::U8(val) => {
                *val = Self::next_u8_for_mutation(state);
                true
            }
            TransactionArgument::U16(val) => {
                *val = Self::next_u16_for_mutation(state);
                true
            }
            TransactionArgument::U32(val) => {
                *val = Self::next_u32_for_mutation(state);
                true
            }
            TransactionArgument::U64(val) => {
                *val = Self::next_u64_for_mutation(state);
                true
            }
            TransactionArgument::U128(val) => {
                *val = Self::next_u128_for_mutation(state);
                true
            }
            TransactionArgument::U256(val) => {
                let high_part = Self::next_u128_for_mutation(state);
                let low_part = Self::next_u128_for_mutation(state);
                let mut bytes = [0u8; 32];
                bytes[0..16].copy_from_slice(&low_part.to_le_bytes());
                bytes[16..32].copy_from_slice(&high_part.to_le_bytes());
                *val = aptos_move_core_types::u256::U256::from_le_bytes(&bytes);
                true
            }
            TransactionArgument::Bool(val) => {
                *val = state.rand_mut().next().is_multiple_of(2);
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
                let new_value = Self::next_u64_for_mutation(state);
                *vec = bcs::to_bytes(&new_value).unwrap_or_else(|_| vec![0u8; 8]);
                true
            }
            TransactionArgument::Serialized(bytes) => {
                let new_value = Self::next_u64_for_mutation(state);
                *bytes = bcs::to_bytes(&new_value).unwrap_or_else(|_| vec![0u8; 8]);
                true
            }
        }
    }

    // Mutator-local monotonic generators (do not rely on state fields)
    #[inline]
    fn next_u8_for_mutation(state: &mut AptosFuzzerState) -> u8 {
        // small step, monotonic modulo wrap guarded by saturating to MAX
        let step = 1 + (state.rand_mut().next() % 4) as u8;
        step.saturating_add(1) // ensure > 0
    }

    #[inline]
    fn next_u16_for_mutation(state: &mut AptosFuzzerState) -> u16 {
        let base = (state.rand_mut().next() % 1024) as u16;
        base.saturating_add(1)
    }

    #[inline]
    fn next_u32_for_mutation(state: &mut AptosFuzzerState) -> u32 {
        let base = (state.rand_mut().next() % (1u64 << 20)) as u32;
        base.saturating_add(1)
    }

    #[inline]
    fn next_u64_for_mutation(state: &mut AptosFuzzerState) -> u64 {
        // Start near 2^32 and grow to guarantee (value >> 32) > 0 soon
        let base: u64 = (1u64 << 32) + (state.rand_mut().next() % (1u64 << 20));
        base
    }

    #[inline]
    fn next_u128_for_mutation(state: &mut AptosFuzzerState) -> u128 {
        let hi = Self::next_u64_for_mutation(state) as u128;
        let lo = Self::next_u64_for_mutation(state) as u128;
        (hi << 64) | lo
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
