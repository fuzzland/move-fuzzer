use std::marker::PhantomData;

use anyhow::Result;
use aptos_types::transaction::TransactionPayload;
use aptos_vm::AptosVM;
use libafl::executors::{Executor, ExitKind, HasObservers};
use libafl_bolts::tuples::RefIndexable;

use super::aptos_custom_state::AptosCustomState;
use super::types::TransactionResult;
use crate::{AptosFuzzerInput, AptosFuzzerState};
use super::custom_state_view::CustomStateView;

pub struct AptosMoveExecutor<EM, Z> {
    aptos_vm: AptosVM,
    _phantom: PhantomData<(EM, Z)>,
    // Simple execution counters for debugging
    success_count: u64,
    error_count: u64,
    observers: (),
}

impl<EM, Z> AptosMoveExecutor<EM, Z> {
    pub fn new() -> Self {
        let env = super::aptos_custom_state::AptosCustomState::default_env();
        Self {
            aptos_vm: AptosVM::new_fuzzer(&env),
            _phantom: PhantomData,
            success_count: 0,
            error_count: 0,
            observers: (),
        }
    }

    pub fn execute_transaction(
        &self,
        transaction: TransactionPayload,
        state: &AptosCustomState,
        sender: Option<aptos_move_core_types::account_address::AccountAddress>,
    ) -> Result<TransactionResult> {
        match &transaction {
            TransactionPayload::EntryFunction(_) | TransactionPayload::Script(_) => {
                let view = CustomStateView::new(state);
                // Use the state's runtime environment for code storage
                let code_storage = aptos_vm_types::module_and_script_storage::AsAptosCodeStorage::as_aptos_code_storage(
                    &view,
                    state,
                );

                let (write_set, events) = self
                    .aptos_vm
                    .execute_user_payload_no_checking(state, &code_storage, &transaction, sender)?;
                Ok(TransactionResult {
                    status: aptos_types::transaction::TransactionStatus::Keep(
                        aptos_types::vm_status::KeptVMStatus::Executed.into(),
                    ),
                    gas_used: 0,
                    write_set,
                    events,
                    fee_statement: None,
                })
            }
            _ => {
                anyhow::bail!("Unsupported payload type for this executor")
            }
        }
    }
}

impl<EM, Z> Executor<EM, AptosFuzzerInput, AptosFuzzerState, Z> for AptosMoveExecutor<EM, Z> {
    fn run_target(
        &mut self,
        fuzzer: &mut Z,
        state: &mut AptosFuzzerState,
        mgr: &mut EM,
        input: &AptosFuzzerInput,
    ) -> Result<ExitKind, libafl::Error> {
        let result = self.execute_transaction(input.payload().clone(), state.aptos_state(), None);
        match result {
            Ok(result) => {
                self.success_count += 1;
                if self.success_count % 100 == 0 {
                    println!("[aptos-fuzzer] Executed {} successful transactions", self.success_count);
                }
                state.aptos_state_mut().apply_write_set(&result.write_set);
                Ok(ExitKind::Ok)
            }
            Err(e) => {
                self.error_count += 1;
                if self.error_count % 10 == 0 {
                    println!("[aptos-fuzzer] {} execution errors so far", self.error_count);
                }
                // Log the error but don't shut down - continue fuzzing
                eprintln!("[aptos-fuzzer] execution error: {e}");
                Ok(ExitKind::Ok) // Return Ok to continue fuzzing even with errors
            }
        }
    }
}

impl<EM, Z> HasObservers for AptosMoveExecutor<EM, Z> {
    type Observers = ();

    fn observers(&self) -> RefIndexable<&Self::Observers, Self::Observers> {
        RefIndexable::from(&self.observers)
    }

    fn observers_mut(&mut self) -> RefIndexable<&mut Self::Observers, Self::Observers> {
        RefIndexable::from(&mut self.observers)
    }
}
