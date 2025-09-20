use std::marker::PhantomData;

use anyhow::Result;
use aptos_types::state_store::state_key::StateKey;
use aptos_types::state_store::state_value::StateValue;
use aptos_types::transaction::{SignedTransaction, TransactionPayload};
use aptos_types::write_set::WriteSet;
use aptos_vm::AptosVM;
use aptos_vm_logging::log_schema::AdapterLogSchema;
use executor::Executor;
use libafl::executors::HasObservers;
use libafl_bolts::tuples::RefIndexable;

use crate::aptos_custom_state::AptosCustomState;
use crate::types::TransactionResult;

pub struct AptosMoveExecutor<EM, I, S, Z> {
    vm: AptosVM,
    state: AptosCustomState,

    _phantom: PhantomData<(EM, I, S, Z)>,
}

impl<EM, I, S, Z> AptosMoveExecutor<EM, I, S, Z> {
    pub fn new() -> Self {
        todo!()
    }

    fn to_signed_transaction(input: TransactionPayload) -> SignedTransaction {
        todo!()
    }

    pub fn execute_transaction(&self, transaction: TransactionPayload) -> Result<TransactionResult> {
        let (vm_status, vm_output) = self.vm.execute_user_transaction(
            &self.state,
            &self.state,
            &Self::to_signed_transaction(transaction),
            &AdapterLogSchema::new(self.state.id(), 0),
            &aptos_types::transaction::AuxiliaryInfo::new(aptos_types::transaction::PersistedAuxiliaryInfo::None, None),
        );

        let txn_output = vm_output
            .try_materialize_into_transaction_output(&self.state)
            .expect("Materializing aggregator deltas should not fail");

        Ok(TransactionResult {
            status: txn_output.status().clone(),
            gas_used: txn_output.gas_used(),
            write_set: txn_output.write_set().clone(),
            events: txn_output.events().to_vec(),
            fee_statement: txn_output.try_extract_fee_statement().ok().flatten(),
        })
    }

    pub fn execute_transaction_with_overlay(
        &self,
        transaction: TransactionPayload,
        override_objects: Vec<(StateKey, StateValue)>,
    ) -> Result<TransactionResult> {
        todo!()
    }

    pub fn get_object(&self, object_id: &StateKey) -> Option<StateValue> {
        todo!()
    }

    pub fn multi_get_objects(&self, object_ids: &[StateKey]) -> Vec<Option<StateValue>> {
        todo!()
    }

    pub fn commit(&self, write_set: WriteSet) -> Result<()> {
        todo!()
    }
}

impl<EM, I, S, Z> Executor for AptosMoveExecutor<EM, I, S, Z> {
    // Unlike in Sui we use multiple calls
    // We mimic multiple calls by using TransactionPayload::EntryFunction
    // TODO: modify aptos to ignore "entry" check
    type Transaction = TransactionPayload;
    type ObjectID = StateKey;
    // TODO: decide if we should use another wrapper to return
    // enum(StateValue, Script, Module) or just use StateVcaalue
    type Object = StateValue;
    type ExecutionResult = TransactionResult;
    type Tracer = ();

    fn execute(
        &self,
        tx: TransactionPayload,
        override_objects: Vec<(StateKey, StateValue)>,
        _tracer: Option<()>,
    ) -> Result<TransactionResult> {
        self.execute_transaction_with_overlay(tx, override_objects)
    }

    fn get_object(&self, object_id_bytes: &StateKey) -> Option<StateValue> {
        self.get_object(object_id_bytes)
    }

    fn multi_get_objects(&self, object_ids: &[StateKey]) -> Vec<Option<StateValue>> {
        self.multi_get_objects(object_ids)
    }

    fn name(&self) -> &str {
        "AptosMoveExecutor"
    }
}

impl<EM, I, S, Z> libafl::executors::Executor<EM, I, S, Z> for AptosMoveExecutor<EM, I, S, Z> {
    fn run_target(
        &mut self,
        fuzzer: &mut Z,
        state: &mut S,
        mgr: &mut EM,
        input: &I,
    ) -> std::result::Result<libafl::executors::ExitKind, libafl::Error> {
        todo!()
    }
}

impl<EM, I, S, Z> HasObservers for AptosMoveExecutor<EM, I, S, Z> {
    type Observers = ();

    fn observers(&self) -> RefIndexable<&Self::Observers, Self::Observers> {
        todo!()
    }

    fn observers_mut(&mut self) -> RefIndexable<&mut Self::Observers, Self::Observers> {
        todo!()
    }
}
