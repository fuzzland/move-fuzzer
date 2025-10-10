use std::marker::PhantomData;

use aptos_move_core_types::vm_status::{StatusCode, VMStatus};
use aptos_types::transaction::{ExecutionStatus, TransactionPayload, TransactionStatus};
use aptos_vm::aptos_vm::ExecOutcomeKind;
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
use crate::{AptosFuzzerInput, AptosFuzzerState};

// Type aliases to simplify complex observer tuple types
type AptosObservers = (
    HitcountsMapObserver<OwnedMapObserver<u8>>,
    (AbortCodeObserver, (ShiftOverflowObserver, ())),
);

const MAP_SIZE: usize = 1 << 16;

pub struct AptosMoveExecutor<EM, Z> {
    aptos_vm: AptosVM,
    _phantom: PhantomData<(EM, Z)>,
    // Simple execution counters for debugging
    success_count: u64,
    error_count: u64,
    observers: AptosObservers,
    prev_loc: u32,
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
        }
    }

    #[inline]
    fn hash32(bytes: &[u8]) -> u32 {
        // FNV-1a 32-bit
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
                // Only transform minimal data for caller; no processing here
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
        let (result, outcome, pcs, shift_losses) =
            self.execute_transaction(input.payload().clone(), state.aptos_state(), None);
        match result {
            Ok(result) => {
                self.success_count += 1;
                // Build AFL-style edge coverage map from pcs
                let map = self.observers.0.as_slice_mut();
                for b in map.iter_mut() {
                    *b = 0;
                }
                self.prev_loc = 0;
                // Build a stable per-function base id to reduce inter-function collisions
                let base_id: u32 = match input.payload() {
                    TransactionPayload::EntryFunction(ef) => {
                        let (module, function, _ty_args, _args) = ef.clone().into_inner();
                        let mut buf = Vec::new();
                        buf.extend_from_slice(module.address().as_ref());
                        buf.extend_from_slice(module.name().as_str().as_bytes());
                        buf.extend_from_slice(function.as_str().as_bytes());
                        Self::hash32(&buf)
                    }
                    TransactionPayload::Script(script) => Self::hash32(script.code()),
                    _ => 0,
                };
                for pc in pcs {
                    let cur_id = base_id ^ pc;
                    let idx = ((cur_id ^ self.prev_loc) as usize) & (MAP_SIZE - 1);
                    let byte = &mut map[idx];
                    *byte = byte.saturating_add(1);
                    self.prev_loc = cur_id >> 1;
                }
                // Shift overflow observer
                let cause_loss = shift_losses.into_iter().any(|b| b);
                self.observers.1 .1 .0.set_cause_loss(cause_loss);
                if let TransactionStatus::Keep(ExecutionStatus::MoveAbort { location: _, code, .. }) = &result.status {
                    self.observers.1 .0.set_last(Some(*code));
                    if *code == 1337 {
                        println!("[fuzzer] abort code 1337 captured");
                    }
                } else {
                    self.observers.1 .0.set_last(None);
                }
                // state.aptos_state_mut().apply_write_set(&result.write_set);
                *state.executions_mut() += 1;
                Ok(ExitKind::Ok)
            }
            Err(vm_status) => {
                self.error_count += 1;
                // Even on error, reset coverage map to a clean state for next exec
                let map = self.observers.0.as_slice_mut();
                for b in map.iter_mut() {
                    *b = 0;
                }
                self.prev_loc = 0;
                self.observers.1 .1 .0.set_cause_loss(false);
                if let VMStatus::MoveAbort(ref _loc, code) = vm_status {
                    self.observers.1 .0.set_last(Some(code));
                    if code == 1337 {
                        println!("[fuzzer] abort code 1337 captured");
                    }
                } else {
                    self.observers.1 .0.set_last(None);
                }
                let exit_kind = match outcome {
                    ExecOutcomeKind::Ok => ExitKind::Ok,
                    ExecOutcomeKind::MoveAbort(_) => ExitKind::Ok,
                    ExecOutcomeKind::OutOfGas => ExitKind::Ok,
                    ExecOutcomeKind::OtherError => ExitKind::Ok,
                    ExecOutcomeKind::InvariantViolation => ExitKind::Crash,
                    ExecOutcomeKind::Panic => ExitKind::Crash,
                };
                *state.executions_mut() += 1;
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
