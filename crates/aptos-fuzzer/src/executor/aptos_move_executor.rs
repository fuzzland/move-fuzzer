use std::marker::PhantomData;

use anyhow::Result;
use aptos_move_core_types::account_address::AccountAddress;
use aptos_move_core_types::identifier::IdentStr;
use aptos_move_core_types::language_storage::TypeTag;
use aptos_move_core_types::value::{self, MoveValue};
use aptos_move_vm_runtime::move_vm::SerializedReturnValues;
use aptos_move_vm_runtime::{LegacyLoaderConfig, ScriptLoader};
use aptos_move_vm_types::gas::UnmeteredGasMeter;
use aptos_types::chain_id::ChainId;
use aptos_types::transaction::authenticator::TransactionAuthenticator;
use aptos_types::transaction::{RawTransaction, SignedTransaction, TransactionPayload};
use aptos_vm::move_vm_ext::SessionId;
use aptos_vm::AptosVM;
use aptos_vm_types::module_write_set::ModuleWriteSet;
use aptos_vm_types::storage::change_set_configs::ChangeSetConfigs;
use libafl::executors::{Executor, ExitKind, HasObservers};
use libafl_bolts::tuples::RefIndexable;

use super::aptos_custom_state::AptosCustomState;
use super::types::TransactionResult;
use crate::{AptosFuzzerInput, AptosFuzzerState};

pub struct AptosMoveExecutor<EM, Z> {
    aptos_vm: AptosVM,
    _phantom: PhantomData<(EM, Z)>,
}

impl<EM, Z> AptosMoveExecutor<EM, Z> {
    pub fn new() -> Self {
        let env = todo!("initialize aptos environment");
        Self {
            aptos_vm: AptosVM::new_fuzzer(&env),
            _phantom: PhantomData,
        }
    }

    pub fn execute_transaction(
        &self,
        transaction: TransactionPayload,
        state: &AptosCustomState,
    ) -> Result<TransactionResult> {
        match &transaction {
            TransactionPayload::EntryFunction(_) | TransactionPayload::Script(_) => {
                let (write_set, events) = self
                    .aptos_vm
                    .execute_user_payload_no_checking(state, state, &transaction)?;
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
        let result = self.execute_transaction(input.transaction, state.aptos_state);
        match result {
            Ok(result) => {
                Ok(ExitKind::Success)
            }
            Err(e) => {
                Err(libafl::Error::TargetError(e))
            }
        }
    }
}

impl<EM, Z> HasObservers for AptosMoveExecutor<EM, Z> {
    type Observers = ();

    fn observers(&self) -> RefIndexable<&Self::Observers, Self::Observers> {
        todo!()
    }

    fn observers_mut(&mut self) -> RefIndexable<&mut Self::Observers, Self::Observers> {
        todo!()
    }
}
