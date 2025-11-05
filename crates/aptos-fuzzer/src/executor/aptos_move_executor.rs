use std::marker::PhantomData;

use aptos_dynamic_transaction_composer::{CallArgument, TransactionComposer};
use aptos_move_core_types::language_storage::TypeTag;
use aptos_move_core_types::vm_status::{StatusCode, VMStatus};
use aptos_move_vm_runtime::ModuleStorage;
use aptos_types::transaction::{ExecutionStatus, TransactionPayload, TransactionStatus};
use aptos_vm::aptos_vm::{ExecOutcomeKind, FUZZER_SENDER};
use aptos_vm::AptosVM;
use libafl::executors::{Executor, ExitKind, HasObservers};
use libafl::observers::map::{HitcountsMapObserver, OwnedMapObserver};
use libafl::state::HasExecutions;
use libafl_bolts::tuples::RefIndexable;
use libafl_bolts::AsSliceMut;
use aptos_move_binary_format::access::ModuleAccess;

use crate::executor::aptos_custom_state::AptosCustomState;
use crate::executor::custom_state_view::CustomStateView;
use crate::executor::types::TransactionResult;
use crate::observers::{AbortCodeObserver, ShiftOverflowObserver};
use crate::state::MAP_SIZE;
use crate::{AptosFuzzerInput, AptosFuzzerState};

// Type aliases to simplify complex observer tuple types
type AptosObservers = (
    HitcountsMapObserver<OwnedMapObserver<u8>>,
    (AbortCodeObserver, (ShiftOverflowObserver, ()))
);

pub struct AptosMoveExecutor<EM, Z> {
    aptos_vm: AptosVM,
    _phantom: PhantomData<(EM, Z)>,
    success_count: u64,
    error_count: u64,
    observers: AptosObservers,
    prev_loc: u32,
    total_instructions_executed: u64,
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
            total_instructions_executed: 0,
        }
    }

    pub fn total_instructions_executed(&self) -> u64 { self.total_instructions_executed }

    #[inline]
    fn hash32(bytes: &[u8]) -> u32 {
        // FNV-1a hash
        let mut hash: u32 = 0x811C9DC5;
        for &b in bytes { hash ^= b as u32; hash = hash.wrapping_mul(0x01000193); }
        hash
    }

    pub fn pc_observer(&self) -> &HitcountsMapObserver<OwnedMapObserver<u8>> { &self.observers.0 }
    pub fn pc_observer_mut(&mut self) -> &mut HitcountsMapObserver<OwnedMapObserver<u8>> { &mut self.observers.0 }

    pub fn execute_transaction(
        &mut self,
        transaction: TransactionPayload,
        state: &AptosCustomState,
        sender: Option<aptos_move_core_types::account_address::AccountAddress>,
    ) -> (
        core::result::Result<TransactionResult, VMStatus>,
        ExecOutcomeKind,
        Vec<u64>,
        Vec<bool>,
    ) {
        match &transaction {
            TransactionPayload::EntryFunction(_) | TransactionPayload::Script(_) => {
                let view = CustomStateView::new(state);
                let code_storage = aptos_vm_types::module_and_script_storage::AsAptosCodeStorage::as_aptos_code_storage(&view, state);
                let (result, pcs, shifts, outcome) = self.aptos_vm.execute_user_payload_no_checking(state, &code_storage, &transaction, sender);
                let shift_losses: Vec<bool> = shifts.iter().map(|ev| ev.lost_high_bits).collect();
                let res = match result {
                    Ok((write_set, events)) => Ok(TransactionResult {
                        status: aptos_types::transaction::TransactionStatus::Keep(aptos_types::vm_status::KeptVMStatus::Executed.into()),
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
                Err(VMStatus::Error { status_code: StatusCode::UNKNOWN_STATUS, sub_status: None, message: Some("Unsupported payload type for this executor".to_string()) }),
                ExecOutcomeKind::OtherError,
                Vec::new(),
                Vec::new(),
            ),
        }
    }

    fn calls_to_script_payload(
        &self,
        input: &AptosFuzzerInput,
        state: &AptosCustomState,
    ) -> Option<TransactionPayload> {
        if input.calls.is_empty() { return None; }
        // Build the script using TransactionComposer
        let mut composer = TransactionComposer::single_signer();
        // Load module bytes from state into composer
        for call in &input.calls {
            if let Ok(Some(bytes)) = state.unmetered_get_module_bytes(call.module_id.address(), call.module_id.name()) {
                let _ = composer.store_module(bytes.to_vec());
            }
        }
        // Use shaped calls from input
        let shaped = input.shaped_calls(state);
        // Track previous return type tags and composer handles
        let mut prev_return_types: Vec<Vec<TypeTag>> = Vec::with_capacity(shaped.len());
        let mut prev_handles: Vec<Vec<CallArgument>> = Vec::with_capacity(shaped.len());
        for (idx_call, call) in shaped.iter().enumerate() {
            let module_str = format!("{}::{}", call.module_id.address().to_standard_string(), call.module_id.name());
            let function_str = call.function_name.to_string();
            let ty_args: Vec<String> = call.ty_args.iter().map(TypeTag::to_canonical_string).collect();
            let mut args = call.args.clone();
            // try to satisfy &T/&mut T by borrowing previous results with same TypeTag
            if let Ok(Some(cm)) = state.unmetered_get_deserialized_module(call.module_id.address(), call.module_id.name()) {
                let mut params = None;
                let mut returns = None;
                for def in cm.function_defs() {
                    let h = cm.function_handle_at(def.function);
                    if cm.identifier_at(h.name) == call.function_name.as_ident_str() {
                        params = Some(cm.signature_at(h.parameters).0.clone());
                        returns = Some(cm.signature_at(h.return_).0.clone());
                        break;
                    }
                }
                if let Some(param_toks) = params {
                    for (i, arg) in args.iter_mut().enumerate() {
                        if let Some(tok) = param_toks.get(i) {
                            use aptos_move_binary_format::file_format::SignatureToken as ST;
                            // helper: convert ST->TypeTag with current call.ty_args
                            fn tag_of(
                                module: &aptos_move_binary_format::CompiledModule,
                                t: &aptos_move_binary_format::file_format::SignatureToken,
                                ty_args: &[TypeTag],
                            ) -> Option<TypeTag> {
                                match t {
                                    ST::Bool => Some(TypeTag::Bool),
                                    ST::U8 => Some(TypeTag::U8),
                                    ST::U16 => Some(TypeTag::U16),
                                    ST::U32 => Some(TypeTag::U32),
                                    ST::U64 => Some(TypeTag::U64),
                                    ST::U128 => Some(TypeTag::U128),
                                    ST::U256 => Some(TypeTag::U256),
                                    ST::Address => Some(TypeTag::Address),
                                    ST::Signer => Some(TypeTag::Signer),
                                    ST::Vector(x) => tag_of(module, x, ty_args).map(|t| TypeTag::Vector(Box::new(t))),
                                    ST::Struct(idx) => {
                                        let st = module.struct_handle_at(*idx);
                                        let mh = module.module_handle_at(st.module);
                                        Some(TypeTag::Struct(Box::new(aptos_move_core_types::language_storage::StructTag {
                                            address: *module.address_identifier_at(mh.address),
                                            module: module.identifier_at(mh.name).to_owned(),
                                            name: module.identifier_at(st.name).to_owned(),
                                            type_args: vec![],
                                        })))
                                    }
                                    ST::StructInstantiation(idx, toks) => {
                                        let st = module.struct_handle_at(*idx);
                                        let mh = module.module_handle_at(st.module);
                                        let mut targs = Vec::with_capacity(toks.len());
                                        for t2 in toks { if let Some(tag) = tag_of(module, t2, ty_args) { targs.push(tag) } else { return None } }
                                        Some(TypeTag::Struct(Box::new(aptos_move_core_types::language_storage::StructTag {
                                            address: *module.address_identifier_at(mh.address),
                                            module: module.identifier_at(mh.name).to_owned(),
                                            name: module.identifier_at(st.name).to_owned(),
                                            type_args: targs,
                                        })))
                                    }
                                    ST::TypeParameter(i) => ty_args.get(*i as usize).cloned(),
                                    ST::Reference(_) | ST::MutableReference(_) | ST::Function(_, _, _) => None,
                                }
                            }
                            match tok {
                                ST::Reference(inner) => {
                                    if let Some(base) = tag_of(&cm, inner, &call.ty_args) {
                                        // scan previous returns for same TypeTag
                                        'outer_ref: for (j, rets) in prev_return_types.iter().enumerate().rev() {
                                            if let Some(k) = rets.iter().position(|tt| *tt == base) {
                                                // take handle and borrow
                                                if let Some(hs) = prev_handles.get(j) {
                                                    if let Some(h) = hs.get(k) {
                                                        if let Ok(borrowed) = h.borrow() {
                                                            *arg = borrowed;
                                                            break 'outer_ref;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                ST::MutableReference(inner) => {
                                    if let Some(base) = tag_of(&cm, inner, &call.ty_args) {
                                        'outer_mref: for (j, rets) in prev_return_types.iter().enumerate().rev() {
                                            if let Some(k) = rets.iter().position(|tt| *tt == base) {
                                                if let Some(hs) = prev_handles.get(j) {
                                                    if let Some(h) = hs.get(k) {
                                                        if let Ok(borrowed) = h.borrow_mut() {
                                                            *arg = borrowed;
                                                            break 'outer_mref;
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    // compute and record return types for this call index after add
                    if let Some(ret_toks) = returns { let _ = ret_toks; }
                }
            }
            // add call and capture return handles
            let ret_handles = match composer.add_batched_call(module_str, function_str, ty_args, args) { Ok(v) => v, Err(_) => return None };
            // compute return type tags for this call
            let mut ret_types: Vec<TypeTag> = Vec::new();
            if let Ok(Some(cm2)) = state.unmetered_get_deserialized_module(call.module_id.address(), call.module_id.name()) {
                for def in cm2.function_defs() {
                    let h = cm2.function_handle_at(def.function);
                    if cm2.identifier_at(h.name) == call.function_name.as_ident_str() {
                        let rts = cm2.signature_at(h.return_).0.clone();
                        for rt in rts.iter() {
                            // reuse small helper
                            use aptos_move_binary_format::file_format::SignatureToken as ST;
                            fn tag_of(
                                module: &aptos_move_binary_format::CompiledModule,
                                t: &aptos_move_binary_format::file_format::SignatureToken,
                                ty_args: &[TypeTag],
                            ) -> Option<TypeTag> {
                                match t {
                                    ST::Bool => Some(TypeTag::Bool),
                                    ST::U8 => Some(TypeTag::U8),
                                    ST::U16 => Some(TypeTag::U16),
                                    ST::U32 => Some(TypeTag::U32),
                                    ST::U64 => Some(TypeTag::U64),
                                    ST::U128 => Some(TypeTag::U128),
                                    ST::U256 => Some(TypeTag::U256),
                                    ST::Address => Some(TypeTag::Address),
                                    ST::Signer => Some(TypeTag::Signer),
                                    ST::Vector(x) => tag_of(module, x, ty_args).map(|t| TypeTag::Vector(Box::new(t))),
                                    ST::Struct(idx) => {
                                        let st = module.struct_handle_at(*idx);
                                        let mh = module.module_handle_at(st.module);
                                        Some(TypeTag::Struct(Box::new(aptos_move_core_types::language_storage::StructTag {
                                            address: *module.address_identifier_at(mh.address),
                                            module: module.identifier_at(mh.name).to_owned(),
                                            name: module.identifier_at(st.name).to_owned(),
                                            type_args: vec![],
                                        })))
                                    }
                                    ST::StructInstantiation(idx, toks) => {
                                        let st = module.struct_handle_at(*idx);
                                        let mh = module.module_handle_at(st.module);
                                        let mut targs = Vec::with_capacity(toks.len());
                                        for t2 in toks { if let Some(tag) = tag_of(module, t2, ty_args) { targs.push(tag) } else { return None } }
                                        Some(TypeTag::Struct(Box::new(aptos_move_core_types::language_storage::StructTag {
                                            address: *module.address_identifier_at(mh.address),
                                            module: module.identifier_at(mh.name).to_owned(),
                                            name: module.identifier_at(st.name).to_owned(),
                                            type_args: targs,
                                        })))
                                    }
                                    ST::TypeParameter(i) => ty_args.get(*i as usize).cloned(),
                                    ST::Reference(_) | ST::MutableReference(_) | ST::Function(_, _, _) => None,
                                }
                            }
                            if let Some(tag) = tag_of(&cm2, rt, &call.ty_args) { ret_types.push(tag); }
                        }
                        break;
                    }
                }
            }
            prev_handles.push(ret_handles);
            prev_return_types.push(ret_types);
        }
        let script_bytes = match composer.generate_batched_calls(true) { Ok(b) => b, Err(_) => return None };
        let script = match bcs::from_bytes::<aptos_types::transaction::Script>(&script_bytes) { Ok(s) => s, Err(_) => return None };
        Some(TransactionPayload::Script(script))
    }
}

impl<EM, Z> Default for AptosMoveExecutor<EM, Z> { fn default() -> Self { Self::new() } }

impl<EM, Z> Executor<EM, AptosFuzzerInput, AptosFuzzerState, Z> for AptosMoveExecutor<EM, Z> {
    fn run_target(
        &mut self,
        _fuzzer: &mut Z,
        state: &mut AptosFuzzerState,
        _mgr: &mut EM,
        input: &AptosFuzzerInput,
    ) -> Result<ExitKind, libafl::Error> {
        state.clear_current_execution_path();
        if input.calls.is_empty() { return Ok(ExitKind::Ok); }
        // PCs from VM are packed u64 per step: upper 32 bits = function hash, lower 32 bits = local pc (u16 widened)
        let mut all_pcs = Vec::new();
        let mut all_shift_losses = Vec::new();
        let mut final_outcome = ExecOutcomeKind::Ok;
        let mut final_result = None;
        let payload = match self.calls_to_script_payload(input, state.aptos_state()) { Some(p) => p, None => return Ok(ExitKind::Ok) };
        let (result, outcome, pcs, shift_losses) = self.execute_transaction(payload, state.aptos_state(), Some(FUZZER_SENDER));
            all_pcs.extend(pcs);
            all_shift_losses.extend(shift_losses);
            final_outcome = outcome;
            final_result = Some(result);
        *state.executions_mut() += 1;
        match final_result.unwrap() {
            Ok(result) => {
                self.success_count += 1;
                let map = self.observers.0.as_slice_mut();
                for byte in map.iter_mut() { *byte = 0; }
                self.prev_loc = 0;
                // Convert packed u64 to hashed u32 tokens for coverage and path recording
                self.total_instructions_executed += all_pcs.len() as u64;
                let mut tokens: Vec<u32> = Vec::with_capacity(all_pcs.len());
                {
                    let cumulative_map = state.cumulative_coverage_mut();
                    for &packed in &all_pcs {
                        let fn_hash = (packed >> 32) as u32;
                        let pc32 = (packed & 0xFFFF_FFFF) as u32;
                        // Build 8-byte buffer = fn_hash || pc32 and hash to u32
                        let mut buf = [0u8; 8];
                        buf[..4].copy_from_slice(&fn_hash.to_le_bytes());
                        buf[4..].copy_from_slice(&pc32.to_le_bytes());
                        let token = Self::hash32(&buf);
                        tokens.push(token);
                        let idx = ((token ^ self.prev_loc) as usize) & (MAP_SIZE - 1);
                        map[idx] = map[idx].saturating_add(1);
                        cumulative_map[idx] = cumulative_map[idx].max(1);
                        self.prev_loc = token >> 1;
                    }
                }
                state.set_current_execution_path(tokens);
                let cause_loss = all_shift_losses.into_iter().any(|b| b);
                self.observers.1 .1 .0.set_cause_loss(cause_loss);
                if let TransactionStatus::Keep(ExecutionStatus::MoveAbort { location: _, code, .. }) = &result.status {
                    self.observers.1 .0.set_last(Some(*code));
                } else { self.observers.1 .0.set_last(None); }
                Ok(ExitKind::Ok)
            }
            Err(vm_status) => {
                self.error_count += 1;
                let map = self.observers.0.as_slice_mut();
                for byte in map.iter_mut() { *byte = 0; }
                self.prev_loc = 0;
                self.observers.1 .1 .0.set_cause_loss(false);
                // On error, still record hashed tokens for current path (may be empty)
                let mut tokens: Vec<u32> = Vec::with_capacity(all_pcs.len());
                for &packed in &all_pcs {
                    let fn_hash = (packed >> 32) as u32;
                    let pc32 = (packed & 0xFFFF_FFFF) as u32;
                    let mut buf = [0u8; 8];
                    buf[..4].copy_from_slice(&fn_hash.to_le_bytes());
                    buf[4..].copy_from_slice(&pc32.to_le_bytes());
                    tokens.push(Self::hash32(&buf));
                }
                state.set_current_execution_path(tokens);
                if let VMStatus::MoveAbort(ref _loc, code) = vm_status { self.observers.1 .0.set_last(Some(code)); } else { self.observers.1 .0.set_last(None); }
                let exit_kind = match final_outcome {
                    ExecOutcomeKind::Ok => ExitKind::Ok,
                    ExecOutcomeKind::MoveAbort(_) => ExitKind::Ok,
                    ExecOutcomeKind::OutOfGas => ExitKind::Ok,
                    ExecOutcomeKind::OtherError => ExitKind::Ok,
                    ExecOutcomeKind::InvariantViolation => ExitKind::Crash,
                    ExecOutcomeKind::Panic => ExitKind::Crash,
                };
                Ok(exit_kind)
            }
        }
    }
}

impl<EM, Z> HasObservers for AptosMoveExecutor<EM, Z> {
    type Observers = AptosObservers;
    fn observers(&self) -> RefIndexable<&Self::Observers, Self::Observers> { RefIndexable::from(&self.observers) }
    fn observers_mut(&mut self) -> RefIndexable<&mut Self::Observers, Self::Observers> { RefIndexable::from(&mut self.observers) }
}
