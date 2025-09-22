use std::marker::PhantomData;

use anyhow::Result;
use aptos_types::state_store::state_key::StateKey;
use aptos_types::state_store::state_value::StateValue;
use aptos_types::transaction::{SignedTransaction, TransactionPayload};
use aptos_types::chain_id::ChainId;
use aptos_types::transaction::RawTransaction;
use aptos_crypto::ed25519::{Ed25519PrivateKey, Ed25519PublicKey, ED25519_PRIVATE_KEY_LENGTH};
use aptos_crypto::traits::{SigningKey, PrivateKey};
use aptos_types::account_address;
use aptos_vm::AptosVM;
use aptos_vm_logging::log_schema::AdapterLogSchema;
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

    fn to_signed_transaction(input: TransactionPayload) -> SignedTransaction {
        // Deterministic test key: use a trivial numeric seed to form a private key.
        let mut sk_bytes = [0u8; ED25519_PRIVATE_KEY_LENGTH];
        sk_bytes[ED25519_PRIVATE_KEY_LENGTH - 1] = 1;
        let privkey: Ed25519PrivateKey =
            Ed25519PrivateKey::try_from(sk_bytes.as_slice()).expect("valid ed25519 private key bytes");
        let pubkey: Ed25519PublicKey = privkey.public_key();

        let sender = account_address::from_public_key(&pubkey);
        let sequence_number = 0u64;
        let max_gas_amount = 1_000_000u64;
        let gas_unit_price = 1u64;
        let expiration_timestamp_secs = u32::MAX as u64;
        let chain_id = ChainId::test();

        let raw_txn = RawTransaction::new(
            sender,
            sequence_number,
            input,
            max_gas_amount,
            gas_unit_price,
            expiration_timestamp_secs,
            chain_id,
        );

        let signature = privkey.sign(&raw_txn).expect("signing must succeed");
        SignedTransaction::new(raw_txn, pubkey, signature)
    }

    pub fn execute_transaction(
        &self,
        transaction: TransactionPayload,
        state: &AptosCustomState,
    ) -> Result<TransactionResult> {
        let (vm_status, vm_output) = self.aptos_vm.execute_user_transaction(
            state,
            state,
            &Self::to_signed_transaction(transaction),
            &AdapterLogSchema::new(state.id(), 0),
            &aptos_types::transaction::AuxiliaryInfo::new(aptos_types::transaction::PersistedAuxiliaryInfo::None, None),
        );

        let txn_output = vm_output
            .try_materialize_into_transaction_output(state)
            .map_err(|e| anyhow::anyhow!("materialize failed: {e:?}; vm_status: {vm_status:?}"))?;

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
