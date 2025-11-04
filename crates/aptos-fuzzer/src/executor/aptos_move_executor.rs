use std::marker::PhantomData;

use aptos_dynamic_transaction_composer::TransactionComposer;
use aptos_move_core_types::language_storage::TypeTag;
use aptos_move_core_types::vm_status::{StatusCode, VMStatus};
use aptos_move_vm_runtime::ModuleStorage;
use aptos_types::transaction::{ExecutionStatus, TransactionPayload, TransactionStatus};
use aptos_vm::aptos_vm::{ExecOutcomeKind, FUZZER_SENDER};
use aptos_vm::AptosVM;
use libafl::executors::{Executor, ExitKind, HasObservers};
use libafl::observers::map::{HitcountsMapObserver, OwnedMapObserver};
use libafl::state::HasExecutions;
use libafl_bolts::tuples::RefIndexable;
use libafl_bolts::AsSliceMut;

use crate::executor::aptos_custom_state::AptosCustomState;
use crate::executor::custom_state_view::CustomStateView;
use crate::executor::types::TransactionResult;
use crate::observers::{AbortCodeObserver, ShiftOverflowObserver};
use crate::state::MAP_SIZE;
use crate::{AptosFuzzerInput, AptosFuzzerState};

// Type aliases to simplify complex observer tuple types
type AptosObservers = (
    HitcountsMapObserver<OwnedMapObserver<u8>>,
    (AbortCodeObserver, (ShiftOverflowObserver, ())),
);

pub struct AptosMoveExecutor<EM, Z> {
    aptos_vm: AptosVM,
    _phantom: PhantomData<(EM, Z)>,
    success_count: u64,
    error_count: u64,
    observers: AptosObservers,
    prev_loc: u32,
    total_instructions_executed: u64,
}

impl<EM, Z> AptosMoveExecutor<EM, Z> {
    pub fn new() -> Self {
        let env = super::aptos_custom_state::AptosCustomState::default_env();
        let edges = OwnedMapObserver::new("edges", vec![0u8; MAP_SIZE]);
        let edges = HitcountsMapObserver::new(edges);
        let abort_obs = AbortCodeObserver::new();
        let shift_obs = ShiftOverflowObserver::new();
        Self {
            aptos_vm: AptosVM::new_fuzzer(&env),
            _phantom: PhantomData,
            success_count: 0,
            error_count: 0,
            observers: (edges, (abort_obs, (shift_obs, ()))),
            prev_loc: 0,
            total_instructions_executed: 0,
        }
    }

    pub fn total_instructions_executed(&self) -> u64 {
        self.total_instructions_executed
    }

    #[inline]
    fn hash32(bytes: &[u8]) -> u32 {
        // FNV-1a hash
        let mut hash: u32 = 0x811C9DC5;
        for &b in bytes {
            hash ^= b as u32;
            hash = hash.wrapping_mul(0x01000193);
        }
        hash
    }

    pub fn pc_observer(&self) -> &HitcountsMapObserver<OwnedMapObserver<u8>> {
        &self.observers.0
    }
    pub fn pc_observer_mut(&mut self) -> &mut HitcountsMapObserver<OwnedMapObserver<u8>> {
        &mut self.observers.0
    }

    pub fn execute_transaction(
        &mut self,
        transaction: TransactionPayload,
        state: &AptosCustomState,
        sender: Option<aptos_move_core_types::account_address::AccountAddress>,
    ) -> (
        core::result::Result<TransactionResult, VMStatus>,
        ExecOutcomeKind,
        Vec<u32>,
        Vec<bool>,
    ) {
        match &transaction {
            TransactionPayload::EntryFunction(_) | TransactionPayload::Script(_) => {
                let view = CustomStateView::new(state);
                let code_storage =
                    aptos_vm_types::module_and_script_storage::AsAptosCodeStorage::as_aptos_code_storage(&view, state);

                let (result, pcs, shifts, outcome) =
                    self.aptos_vm
                        .execute_user_payload_no_checking(state, &code_storage, &transaction, sender);
                let shift_losses: Vec<bool> = shifts.iter().map(|ev| ev.lost_high_bits).collect();

                let res = match result {
                    Ok((write_set, events)) => Ok(TransactionResult {
                        status: aptos_types::transaction::TransactionStatus::Keep(
                            aptos_types::vm_status::KeptVMStatus::Executed.into(),
                        ),
                        gas_used: 0,
                        write_set,
                        events,
                        fee_statement: None,
                    }),
                    Err(e) => Err(e),
                };
                (res, outcome, pcs, shift_losses)
            }
            _ => (
                Err(VMStatus::Error {
                    status_code: StatusCode::UNKNOWN_STATUS,
                    sub_status: None,
                    message: Some("Unsupported payload type for this executor".to_string()),
                }),
                ExecOutcomeKind::OtherError,
                Vec::new(),
                Vec::new(),
            ),
        }
    }

    fn calls_to_script_payload(
        &self,
        input: &AptosFuzzerInput,
        state: &AptosCustomState,
    ) -> Option<TransactionPayload> {
        if input.calls.is_empty() {
            return None;
        }

        // Build the script using TransactionComposer
        let mut composer = TransactionComposer::single_signer();

        // Load module bytes from state into composer
        for call in &input.calls {
            if let Ok(Some(bytes)) = state.unmetered_get_module_bytes(call.module_id.address(), call.module_id.name()) {
                let _ = composer.store_module(bytes.to_vec());
            }
        }

        // Add each batched call with ty args and arguments
        for call in &input.calls {
            let module_str = call.module_id.to_string();
            let function_str = call.function_name.to_string();
            let ty_args: Vec<String> = call.ty_args.iter().map(TypeTag::to_canonical_string).collect();
            let args = call.args.clone();
            if composer
                .add_batched_call(module_str, function_str, ty_args, args)
                .is_err()
            {
                return None;
            }
        }

        let script_bytes = match composer.generate_batched_calls(true) {
            Ok(b) => b,
            Err(_) => return None,
        };
        let script = match bcs::from_bytes::<aptos_types::transaction::Script>(&script_bytes) {
            Ok(s) => s,
            Err(_) => return None,
        };
        Some(TransactionPayload::Script(script))
    }
}

impl<EM, Z> Default for AptosMoveExecutor<EM, Z> {
    fn default() -> Self {
        Self::new()
    }
}

impl<EM, Z> Executor<EM, AptosFuzzerInput, AptosFuzzerState, Z> for AptosMoveExecutor<EM, Z> {
    fn run_target(
        &mut self,
        _fuzzer: &mut Z,
        state: &mut AptosFuzzerState,
        _mgr: &mut EM,
        input: &AptosFuzzerInput,
    ) -> Result<ExitKind, libafl::Error> {
        state.clear_current_execution_path();

        if input.calls.is_empty() {
            return Ok(ExitKind::Ok);
        }

        let mut all_pcs = Vec::new();
        let mut all_shift_losses = Vec::new();
        let mut final_outcome = ExecOutcomeKind::Ok;
        let mut final_result = None;

        // Build a single Script payload from all calls and execute once
        let payload = match self.calls_to_script_payload(input, state.aptos_state()) {
            Some(p) => p,
            None => return Ok(ExitKind::Ok),
        };

        let (result, outcome, pcs, shift_losses) =
            self.execute_transaction(payload, state.aptos_state(), Some(FUZZER_SENDER));
        all_pcs.extend(pcs);
        all_shift_losses.extend(shift_losses);
        final_outcome = outcome;
        final_result = Some(result);

        // Update execution counter
        *state.executions_mut() += 1;

        match final_result.unwrap() {
            Ok(result) => {
                self.success_count += 1;
                let map = self.observers.0.as_slice_mut();
                for byte in map.iter_mut() {
                    *byte = 0;
                }
                self.prev_loc = 0;

                self.total_instructions_executed += all_pcs.len() as u64;

                {
                    let cumulative_map = state.cumulative_coverage_mut();
                    for &pc in &all_pcs {
                        // PCs are now global in the VM trace; use directly
                        let cur_id = pc;
                        let idx = ((cur_id ^ self.prev_loc) as usize) & (MAP_SIZE - 1);
                        map[idx] = map[idx].saturating_add(1);
                        cumulative_map[idx] = cumulative_map[idx].max(1);
                        self.prev_loc = cur_id >> 1;
                    }
                }

                state.set_current_execution_path(all_pcs);

                let cause_loss = all_shift_losses.into_iter().any(|b| b);
                self.observers.1 .1 .0.set_cause_loss(cause_loss);
                if let TransactionStatus::Keep(ExecutionStatus::MoveAbort { location: _, code, .. }) = &result.status {
                    self.observers.1 .0.set_last(Some(*code));
                } else {
                    self.observers.1 .0.set_last(None);
                }

                Ok(ExitKind::Ok)
            }
            Err(vm_status) => {
                self.error_count += 1;
                let map = self.observers.0.as_slice_mut();
                for byte in map.iter_mut() {
                    *byte = 0;
                }
                self.prev_loc = 0;
                self.observers.1 .1 .0.set_cause_loss(false);
                state.set_current_execution_path(all_pcs);
                if let VMStatus::MoveAbort(ref _loc, code) = vm_status {
                    self.observers.1 .0.set_last(Some(code));
                } else {
                    self.observers.1 .0.set_last(None);
                }
                let exit_kind = match final_outcome {
                    ExecOutcomeKind::Ok => ExitKind::Ok,
                    ExecOutcomeKind::MoveAbort(_) => ExitKind::Ok,
                    ExecOutcomeKind::OutOfGas => ExitKind::Ok,
                    ExecOutcomeKind::OtherError => ExitKind::Ok,
                    ExecOutcomeKind::InvariantViolation => ExitKind::Crash,
                    ExecOutcomeKind::Panic => ExitKind::Crash,
                };
                Ok(exit_kind)
            }
        }
    }
}

impl<EM, Z> HasObservers for AptosMoveExecutor<EM, Z> {
    type Observers = AptosObservers;

    fn observers(&self) -> RefIndexable<&Self::Observers, Self::Observers> {
        RefIndexable::from(&self.observers)
    }

    fn observers_mut(&mut self) -> RefIndexable<&mut Self::Observers, Self::Observers> {
        RefIndexable::from(&mut self.observers)
    }
}
