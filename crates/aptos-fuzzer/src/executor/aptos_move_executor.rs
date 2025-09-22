use std::marker::PhantomData;
use std::sync::OnceLock;

use anyhow::Result;
use aptos_crypto::ed25519::{ED25519_PUBLIC_KEY_LENGTH, ED25519_SIGNATURE_LENGTH, Ed25519PublicKey, Ed25519Signature};
use aptos_move_core_types::account_address::AccountAddress;
use aptos_move_core_types::identifier::IdentStr;
use aptos_move_core_types::language_storage::TypeTag;
use aptos_move_vm_runtime::move_vm::SerializedReturnValues;
use aptos_move_vm_types::gas::UnmeteredGasMeter;
use aptos_types::account_address;
use aptos_types::chain_id::ChainId;
use aptos_types::state_store::state_key::StateKey;
use aptos_types::state_store::state_value::StateValue;
use aptos_types::transaction::authenticator::{
    AccountAuthenticator, AnyPublicKey, AnySignature, SingleKeyAuthenticator, TransactionAuthenticator,
};
use aptos_types::transaction::{RawTransaction, SignedTransaction, TransactionPayload};
use aptos_vm::AptosVM;
use aptos_vm::move_vm_ext::SessionId;
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

    fn to_signed_transaction(input: TransactionPayload) -> SignedTransaction {
        static TXN_AUTH: OnceLock<TransactionAuthenticator> = OnceLock::new();
        let txn_auth = TXN_AUTH
            .get_or_init(|| {
                let zero_pk = Ed25519PublicKey::try_from(&[0u8; ED25519_PUBLIC_KEY_LENGTH][..])
                    .expect("valid zero ed25519 pubkey bytes");
                let zero_sig = Ed25519Signature::try_from(&[0u8; ED25519_SIGNATURE_LENGTH][..])
                    .expect("valid zero ed25519 signature bytes");
                let single =
                    SingleKeyAuthenticator::new(AnyPublicKey::ed25519(zero_pk), AnySignature::ed25519(zero_sig));
                let account_auth = AccountAuthenticator::single_key(single);
                TransactionAuthenticator::single_sender(account_auth)
            })
            .clone();

        // Minimal RawTransaction: only payload varies per call.
        let raw_txn = RawTransaction::new(
            AccountAddress::ZERO,
            0,
            input,
            1_000_000,
            1,
            u32::MAX as u64,
            ChainId::test(),
        );

        SignedTransaction::new_signed_transaction(raw_txn, txn_auth)
    }

    pub fn execute_transaction(
        &self,
        transaction: TransactionPayload,
        state: &AptosCustomState,
    ) -> Result<TransactionResult> {
        let mut session = self.aptos_vm.new_session(state, SessionId::void(), None);

        let entry = match &transaction {
            TransactionPayload::EntryFunction(f) => f,
            _ => {
                anyhow::bail!("Only EntryFunction payload is supported in session mode")
            }
        };

        let module_id = entry.module().clone();
        let func_name: &IdentStr = entry.function();
        let ty_args: Vec<TypeTag> = entry.ty_args().to_vec();
        let args: Vec<&[u8]> = entry.args().iter().map(|v| v.as_slice()).collect();

        let mut gas = UnmeteredGasMeter;
        let storage = aptos_move_vm_runtime::module_traversal::TraversalStorage::new();
        let mut traversal = aptos_move_vm_runtime::module_traversal::TraversalContext::new(&storage);

        let _ret: SerializedReturnValues = session
            .execute_function_bypass_visibility(
                &module_id,
                func_name,
                ty_args,
                args,
                &mut gas,
                &mut traversal,
                state,
            )
            .map_err(|e| anyhow::anyhow!("session execute failed: {e:?}"))?;

        let change_set = session
            .finish(&ChangeSetConfigs::unlimited_at_gas_feature_version(0), state)
            .map_err(|e| anyhow::anyhow!("session finish failed: {e:?}"))?;
        let storage_change_set = change_set
            .try_combine_into_storage_change_set(ModuleWriteSet::empty())
            .map_err(|e| anyhow::anyhow!("convert change set failed: {e:?}"))?;
        let write_set = storage_change_set.write_set().clone();
        let events = storage_change_set.events().to_vec();

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
