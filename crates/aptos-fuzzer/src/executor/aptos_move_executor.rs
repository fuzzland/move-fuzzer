use std::marker::PhantomData;

use anyhow::Result;
use aptos_types::transaction::TransactionPayload;
use aptos_vm::AptosVM;
use libafl::executors::{Executor, ExitKind, HasObservers};
use libafl_bolts::tuples::RefIndexable;

use super::aptos_custom_state::AptosCustomState;
use super::custom_state_view::CustomStateView;
use super::types::TransactionResult;
use crate::{AptosFuzzerInput, AptosFuzzerState};

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
                let code_storage =
                    aptos_vm_types::module_and_script_storage::AsAptosCodeStorage::as_aptos_code_storage(&view, state);

                let (write_set, events) =
                    self.aptos_vm
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
        _fuzzer: &mut Z,
        state: &mut AptosFuzzerState,
        _mgr: &mut EM,
        input: &AptosFuzzerInput,
    ) -> Result<ExitKind, libafl::Error> {
        let result = self.execute_transaction(input.payload().clone(), state.aptos_state(), None);
        match result {
            Ok(result) => {
                self.success_count += 1;
                state.aptos_state_mut().apply_write_set(&result.write_set);
                Ok(ExitKind::Ok)
            }
            Err(_) => {
                self.error_count += 1;
                Ok(ExitKind::Ok)
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
