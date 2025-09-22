use std::marker::PhantomData;

use anyhow::Result;
use aptos_types::transaction::{SignedTransaction, TransactionPayload};
use aptos_vm::AptosVM;
use aptos_vm_logging::log_schema::AdapterLogSchema;
use libafl::executors::{Executor, ExitKind, HasObservers};
use libafl_bolts::tuples::RefIndexable;

use super::aptos_custom_state::AptosCustomState;
use super::types::TransactionResult;
use crate::{AptosFuzzerInput, AptosFuzzerState};

pub struct AptosMoveExecutor<EM, Z> {
    vm: AptosVM,

    _phantom: PhantomData<(EM, Z)>,
}

impl<EM, Z> AptosMoveExecutor<EM, Z> {
    pub fn new() -> Self {
        todo!()
    }

    fn to_signed_transaction(input: TransactionPayload) -> SignedTransaction {
        todo!()
    }

    pub fn execute_transaction(
        &self,
        transaction: TransactionPayload,
        state: &AptosCustomState,
    ) -> Result<TransactionResult> {
        let (vm_status, vm_output) = self.vm.execute_user_transaction(
            state,
            state,
            &Self::to_signed_transaction(transaction),
            &AdapterLogSchema::new(state.id(), 0),
            &aptos_types::transaction::AuxiliaryInfo::new(aptos_types::transaction::PersistedAuxiliaryInfo::None, None),
        );

        let txn_output = vm_output
            .try_materialize_into_transaction_output(state)
            .expect("Materializing aggregator deltas should not fail");

        Ok(TransactionResult {
            status: txn_output.status().clone(),
            gas_used: txn_output.gas_used(),
            write_set: txn_output.write_set().clone(),
            events: txn_output.events().to_vec(),
            fee_statement: txn_output.try_extract_fee_statement().ok().flatten(),
        })
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
        todo!()
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
