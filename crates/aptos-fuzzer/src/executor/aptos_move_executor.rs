use std::marker::PhantomData;

use aptos_move_core_types::vm_status::{StatusCode, VMStatus};
use aptos_types::transaction::{ExecutionStatus, TransactionPayload, TransactionStatus};
use aptos_vm::AptosVM;
use libafl::executors::{Executor, ExitKind, HasObservers};
use libafl::state::HasExecutions;
use libafl_bolts::tuples::RefIndexable;

use super::aptos_custom_state::AptosCustomState;
use super::custom_state_view::CustomStateView;
use super::types::TransactionResult;
use crate::observer::PcIndexObserver;
use crate::{AptosFuzzerInput, AptosFuzzerState};

pub struct AptosMoveExecutor<EM, Z> {
    aptos_vm: AptosVM,
    _phantom: PhantomData<(EM, Z)>,
    // Simple execution counters for debugging
    success_count: u64,
    error_count: u64,
    observers: (PcIndexObserver, ()),
}

impl<EM, Z> AptosMoveExecutor<EM, Z> {
    pub fn new() -> Self {
        let env = super::aptos_custom_state::AptosCustomState::default_env();
        Self {
            aptos_vm: AptosVM::new_fuzzer(&env),
            _phantom: PhantomData,
            success_count: 0,
            error_count: 0,
            observers: (PcIndexObserver::new(), ()),
        }
    }

    pub fn execute_transaction(
        &mut self,
        transaction: TransactionPayload,
        state: &AptosCustomState,
        sender: Option<aptos_move_core_types::account_address::AccountAddress>,
    ) -> core::result::Result<TransactionResult, VMStatus> {
        match &transaction {
            TransactionPayload::EntryFunction(_) | TransactionPayload::Script(_) => {
                let view = CustomStateView::new(state);
                // Use the state's runtime environment for code storage
                let code_storage =
                    aptos_vm_types::module_and_script_storage::AsAptosCodeStorage::as_aptos_code_storage(&view, state);

                let (result, pcs) = self.aptos_vm.execute_user_payload_no_checking_with_counter(
                    state,
                    &code_storage,
                    &transaction,
                    sender,
                );
                self.observers.0.set_pcs(pcs);
                match result {
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
                }
            }
            _ => Err(VMStatus::Error {
                status_code: StatusCode::UNKNOWN_STATUS,
                sub_status: None,
                message: Some("Unsupported payload type for this executor".to_string()),
            }),
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
        *state.last_abort_code_mut() = None;
        let result = self.execute_transaction(input.payload().clone(), state.aptos_state(), None);
        match result {
            Ok(result) => {
                self.success_count += 1;
                if let TransactionStatus::Keep(ExecutionStatus::MoveAbort { location: _, code, .. }) = &result.status {
                    *state.last_abort_code_mut() = Some(*code);
                    if *code == 1337 {
                        println!("[fuzzer] abort code 1337 captured");
                    }
                }
                state.aptos_state_mut().apply_write_set(&result.write_set);
                *state.executions_mut() += 1;
                Ok(ExitKind::Ok)
            }
            Err(vm_status) => {
                self.error_count += 1;
                if let VMStatus::MoveAbort(_loc, code) = vm_status {
                    *state.last_abort_code_mut() = Some(code);
                    if code == 1337 {
                        println!("[fuzzer] abort code 1337 captured");
                    }
                }
                *state.executions_mut() += 1;
                Ok(ExitKind::Ok)
            }
        }
    }
}

impl<EM, Z> HasObservers for AptosMoveExecutor<EM, Z> {
    type Observers = (PcIndexObserver, ());

    fn observers(&self) -> RefIndexable<&Self::Observers, Self::Observers> {
        RefIndexable::from(&self.observers)
    }

    fn observers_mut(&mut self) -> RefIndexable<&mut Self::Observers, Self::Observers> {
        RefIndexable::from(&mut self.observers)
    }
}
