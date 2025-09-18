use anyhow::Result;
use aptos_types::transaction::{RawTransaction, SignedTransaction};
use aptos_vm::AptosVM;
use aptos_vm_logging::log_schema::AdapterLogSchema;
use executor::Executor;

use crate::aptos_custom_state::AptosCustomState;
use crate::types::TransactionResult;

type Tx = RawTransaction;
type Id = Vec<u8>;
type Obj = Option<Vec<u8>>;
type R = TransactionResult;
type T = ();

pub struct AptosMoveExecutor {
    vm: AptosVM,
    state: AptosCustomState,
}

impl AptosMoveExecutor {
    fn to_signed_transaction(transaction: Tx) -> SignedTransaction {
        todo!()
    }

    fn execute_transaction(&self, transaction: Tx) -> Result<R> {
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

    fn execute_transaction_with_overlay(
        &self,
        transaction: Tx,
        override_objects: Vec<(Id, Obj)>,
    ) -> Result<TransactionResult> {
        todo!()
    }

    fn get_object(&self, object_id: &Id) -> Option<Obj> {
        todo!()
    }

    fn multi_get_objects(&self, object_ids: &[Id]) -> Vec<Option<Obj>> {
        todo!()
    }
}

impl Executor<Tx, Id, Obj, R, T> for AptosMoveExecutor {
    fn execute(
        &self,
        tx: Tx,
        override_objects: Vec<(Vec<u8>, Option<Vec<u8>>)>,
        _tracer: Option<()>,
    ) -> Result<TransactionResult> {
        self.execute_transaction_with_overlay(tx, override_objects)
    }

    fn get_object(&self, object_id_bytes: &Id) -> Option<Obj> {
        self.get_object(object_id_bytes)
    }

    fn multi_get_objects(&self, object_ids: &[Id]) -> Vec<Option<Obj>> {
        self.multi_get_objects(object_ids)
    }

    fn name(&self) -> &str {
        "AptosMoveExecutor"
    }
}
