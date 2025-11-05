use aptos_dynamic_transaction_composer::CallArgument;
use aptos_move_core_types::identifier::Identifier;
use aptos_move_core_types::language_storage::{ModuleId, TypeTag};
use libafl::inputs::Input;
use serde::{Deserialize, Serialize};

// new imports for shaping
use aptos_move_binary_format::access::ModuleAccess;
use aptos_move_vm_runtime::ModuleStorage;
use aptos_vm::aptos_vm::FUZZER_SENDER;
use aptos_move_core_types::value::{MoveStruct, MoveValue};
use aptos_move_core_types::identifier::IdentStr;

/// Represents a single function call with batched-call compatible arguments.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub struct Call {
    pub module_id: ModuleId,
    pub function_name: Identifier,
    pub ty_args: Vec<TypeTag>,
    pub args: Vec<CallArgument>,
}

impl Call {
    pub fn new(module_id: ModuleId, function_name: Identifier, ty_args: Vec<TypeTag>, args: Vec<CallArgument>) -> Self {
        Self {
            module_id,
            function_name,
            ty_args,
            args,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub struct AptosFuzzerInput {
    pub calls: Vec<Call>,
}

impl Input for AptosFuzzerInput {}

impl AptosFuzzerInput {
    pub fn new(call: Call) -> Self {
        Self { calls: vec![call] }
    }

    pub fn from_calls(calls: Vec<Call>) -> Self {
        Self { calls }
    }

    pub fn push(&mut self, call: Call) {
        self.calls.push(call);
    }

    pub fn len(&self) -> usize { self.calls.len() }
    pub fn is_empty(&self) -> bool { self.calls.is_empty() }

    /// Return a copy of `calls` with arguments shaped to minimal deserializable BCS
    /// using module metadata from the provided storage.
    pub fn shaped_calls<S: ModuleStorage>(&self, storage: &S) -> Vec<Call> {
        let mut out = Vec::with_capacity(self.calls.len());
        // Note: &T/&mut T borrowing from previous results is handled during composing in executor
        for call in &self.calls {
            let mut shaped_call = call.clone();
            if let Ok(Some(cm)) = storage.unmetered_get_deserialized_module(call.module_id.address(), call.module_id.name()) {
                // locate function handle
                let mut params: Option<Vec<aptos_move_binary_format::file_format::SignatureToken>> = None;
                let mut returns: Option<Vec<aptos_move_binary_format::file_format::SignatureToken>> = None;
                for def in cm.function_defs() {
                    let h = cm.function_handle_at(def.function);
                    if cm.identifier_at(h.name) == call.function_name.as_ident_str() {
                        params = Some(cm.signature_at(h.parameters).0.clone());
                        returns = Some(cm.signature_at(h.return_).0.clone());
                        break;
                    }
                }
                if let Some(param_toks) = params {
                    let mut new_args = Vec::with_capacity(shaped_call.args.len());
                    for (idx, arg) in shaped_call.args.iter().cloned().enumerate() {
                        let tok_opt = param_toks.get(idx);
                        let arg = match (tok_opt, arg) {
                            // &signer / &mut signer
                            (Some(t), _) if is_signer_reference(t) => CallArgument::Signer(0),
                            // value Signer
                            (Some(aptos_move_binary_format::file_format::SignatureToken::Signer), _) => CallArgument::Signer(0),
                            // general &T / &mut T: leave Raw shaping; borrowing is composed later
                            (Some(aptos_move_binary_format::file_format::SignatureToken::Reference(inner)), prev_or_raw) => {
                                if let Some(base_tag) = type_tag_from_signature(&cm, inner, &call.ty_args) {
                                    match prev_or_raw {
                                        CallArgument::Raw(bytes) => {
                                            if !bytes.is_empty() { CallArgument::Raw(bytes) } else if let Some(minb) = minimal_bcs_for_typetag(&base_tag) { CallArgument::Raw(minb) } else { CallArgument::Raw(vec![]) }
                                        },
                                        other => other,
                                    }
                                } else { prev_or_raw }
                            },
                            (Some(aptos_move_binary_format::file_format::SignatureToken::MutableReference(inner)), prev_or_raw) => {
                                if let Some(base_tag) = type_tag_from_signature(&cm, inner, &call.ty_args) {
                                    match prev_or_raw {
                                        CallArgument::Raw(bytes) => {
                                            if !bytes.is_empty() { CallArgument::Raw(bytes) } else if let Some(minb) = minimal_bcs_for_typetag(&base_tag) { CallArgument::Raw(minb) } else { CallArgument::Raw(vec![]) }
                                        },
                                        other => other,
                                    }
                                } else { prev_or_raw }
                            },
                            // other value types: if Raw empty, fill minimal BCS
                            (Some(t), CallArgument::Raw(bytes)) => {
                                // For value types, if provided Raw length mismatches expected fixed-size,
                                // replace with minimal correct BCS; otherwise keep.
                                if let Some(tag) = resolve_token_to_typetag_with_storage(storage, &cm, t, &call.ty_args) {
                                    if let Some(expected) = expected_fixed_len_bytes(&tag) {
                                        if bytes.len() != expected {
                                            CallArgument::Raw(minimal_bcs_for_typetag(&tag).unwrap_or_else(|| vec![]))
                                        } else {
                                            if !bytes.is_empty() { CallArgument::Raw(bytes) } else if let Some(minb) = minimal_bcs_for_token_with_storage(storage, &cm, t, &call.ty_args) { CallArgument::Raw(minb) } else { CallArgument::Raw(vec![]) }
                                        }
                                    } else {
                                        if !bytes.is_empty() { CallArgument::Raw(bytes) } else if let Some(minb) = minimal_bcs_for_token_with_storage(storage, &cm, t, &call.ty_args) { CallArgument::Raw(minb) } else { CallArgument::Raw(vec![]) }
                                    }
                                } else {
                                    if !bytes.is_empty() { CallArgument::Raw(bytes) } else if let Some(minb) = minimal_bcs_for_token_with_storage(storage, &cm, t, &call.ty_args) { CallArgument::Raw(minb) } else { CallArgument::Raw(vec![]) }
                                }
                            }
                            // keep as is
                            (_, other) => other,
                        };
                        new_args.push(arg);
                    }
                    shaped_call.args = new_args;
                }
                // push shaped call, record its index
                out.push(shaped_call);
                let cur_idx = (out.len() - 1) as u16;
                continue;
            }
            // no module info, just push
            out.push(shaped_call);
        }
        out
    }
}

// --- helpers ---
fn is_signer_reference(tok: &aptos_move_binary_format::file_format::SignatureToken) -> bool {
    use aptos_move_binary_format::file_format::SignatureToken as ST;
    match tok {
        ST::Reference(inner) | ST::MutableReference(inner) => matches!(**inner, ST::Signer),
        _ => false,
    }
}

fn type_tag_from_signature(
    module: &aptos_move_binary_format::CompiledModule,
    tok: &aptos_move_binary_format::file_format::SignatureToken,
    ty_args: &[TypeTag],
) -> Option<TypeTag> {
    use aptos_move_binary_format::file_format::SignatureToken as ST;
    match tok {
        ST::Bool => Some(TypeTag::Bool),
        ST::U8 => Some(TypeTag::U8),
        ST::U16 => Some(TypeTag::U16),
        ST::U32 => Some(TypeTag::U32),
        ST::U64 => Some(TypeTag::U64),
        ST::U128 => Some(TypeTag::U128),
        ST::U256 => Some(TypeTag::U256),
        ST::Address => Some(TypeTag::Address),
        ST::Signer => Some(TypeTag::Signer),
        ST::Vector(inner) => type_tag_from_signature(module, inner, ty_args).map(|t| TypeTag::Vector(Box::new(t))),
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
            for t in toks { if let Some(tag) = type_tag_from_signature(module, t, ty_args) { targs.push(tag) } else { return None } }
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

fn minimal_bcs_for_typetag(ty: &TypeTag) -> Option<Vec<u8>> {
    use aptos_move_core_types::value::MoveValue;
    match ty {
        TypeTag::Bool => MoveValue::Bool(false).simple_serialize(),
        TypeTag::U8 => MoveValue::U8(0).simple_serialize(),
        TypeTag::U16 => MoveValue::U16(0).simple_serialize(),
        TypeTag::U32 => MoveValue::U32(0).simple_serialize(),
        TypeTag::U64 => MoveValue::U64(0).simple_serialize(),
        TypeTag::U128 => MoveValue::U128(0).simple_serialize(),
        TypeTag::U256 => MoveValue::U256(0u128.into()).simple_serialize(),
        TypeTag::Address => MoveValue::Address(FUZZER_SENDER).simple_serialize(),
        TypeTag::Signer => MoveValue::Signer(FUZZER_SENDER).simple_serialize(),
        TypeTag::Vector(inner) => {
            if matches!(**inner, TypeTag::U8) {
                MoveValue::vector_u8(vec![]).simple_serialize()
            } else {
                MoveValue::Vector(vec![]).simple_serialize()
            }
        }
        TypeTag::Struct(s) => {
            let module = s.module.as_ident_str();
            let name = s.name.as_ident_str();
            if module.as_str() == "string" && name.as_str() == "String" {
                return MoveValue::vector_u8(vec![]).simple_serialize();
            }
            if module.as_str() == "option" && name.as_str() == "Option" {
                return MoveValue::Vector(vec![]).simple_serialize();
            }
            None
        }
        TypeTag::Function(_) => None,
    }
}

fn expected_fixed_len_bytes(tag: &TypeTag) -> Option<usize> {
    match tag {
        TypeTag::Bool => Some(1),
        TypeTag::U8 => Some(1),
        TypeTag::U16 => Some(2),
        TypeTag::U32 => Some(4),
        TypeTag::U64 => Some(8),
        TypeTag::U128 => Some(16),
        TypeTag::U256 => Some(32),
        // Aptos addresses are 32 bytes
        TypeTag::Address => Some(32),
        // variable size below
        TypeTag::Signer => None,
        TypeTag::Vector(_) => None,
        TypeTag::Struct(_) => None,
        TypeTag::Function(_) => None,
    }
}

fn minimal_bcs_for_token_with_storage<S: ModuleStorage>(
    storage: &S,
    module_ctx: &aptos_move_binary_format::CompiledModule,
    tok: &aptos_move_binary_format::file_format::SignatureToken,
    func_ty_args: &[TypeTag],
) -> Option<Vec<u8>> {
    minimal_move_value_for_token_with_storage(storage, module_ctx, tok, func_ty_args)
        .and_then(|mv| mv.simple_serialize())
}

fn minimal_move_value_for_token_with_storage<S: ModuleStorage>(
    storage: &S,
    module_ctx: &aptos_move_binary_format::CompiledModule,
    tok: &aptos_move_binary_format::file_format::SignatureToken,
    func_ty_args: &[TypeTag],
) -> Option<MoveValue> {
    use aptos_move_binary_format::file_format::SignatureToken as ST;
    match tok {
        ST::Bool => Some(MoveValue::Bool(false)),
        ST::U8 => Some(MoveValue::U8(0)),
        ST::U16 => Some(MoveValue::U16(0)),
        ST::U32 => Some(MoveValue::U32(0)),
        ST::U64 => Some(MoveValue::U64(0)),
        ST::U128 => Some(MoveValue::U128(0)),
        ST::U256 => Some(MoveValue::U256(0u128.into())),
        ST::Address => Some(MoveValue::Address(FUZZER_SENDER)),
        ST::Signer => Some(MoveValue::Signer(FUZZER_SENDER)),
        ST::Vector(inner) => {
            if matches!(**inner, ST::U8) { Some(MoveValue::vector_u8(vec![])) } else { Some(MoveValue::Vector(vec![])) }
        }
        ST::Struct(idx) => {
            let sh = module_ctx.struct_handle_at(*idx);
            let mh = module_ctx.module_handle_at(sh.module);
            let addr = *module_ctx.address_identifier_at(mh.address);
            let module_ident = module_ctx.identifier_at(mh.name).to_owned();
            let struct_ident = module_ctx.identifier_at(sh.name).to_owned();
            minimal_move_value_for_struct(storage, &addr, &module_ident, &struct_ident, &[])
        }
        ST::StructInstantiation(idx, toks) => {
            let sh = module_ctx.struct_handle_at(*idx);
            let mh = module_ctx.module_handle_at(sh.module);
            let addr = *module_ctx.address_identifier_at(mh.address);
            let module_ident = module_ctx.identifier_at(mh.name).to_owned();
            let struct_ident = module_ctx.identifier_at(sh.name).to_owned();
            let mut type_args: Vec<TypeTag> = Vec::with_capacity(toks.len());
            for t in toks {
                if let Some(tag) = resolve_token_to_typetag_with_storage(storage, module_ctx, t, func_ty_args) { type_args.push(tag) } else { return None }
            }
            minimal_move_value_for_struct(storage, &addr, &module_ident, &struct_ident, &type_args)
        }
        ST::TypeParameter(i) => {
            func_ty_args.get(*i as usize).cloned().and_then(|t| minimal_move_value_for_typetag_full(storage, &t))
        }
        ST::Reference(_) | ST::MutableReference(_) | ST::Function(_, _, _) => None,
    }
}

fn resolve_token_to_typetag_with_storage<S: ModuleStorage>(
    storage: &S,
    module_ctx: &aptos_move_binary_format::CompiledModule,
    tok: &aptos_move_binary_format::file_format::SignatureToken,
    func_ty_args: &[TypeTag],
) -> Option<TypeTag> {
    use aptos_move_binary_format::file_format::SignatureToken as ST;
    match tok {
        ST::Bool => Some(TypeTag::Bool),
        ST::U8 => Some(TypeTag::U8),
        ST::U16 => Some(TypeTag::U16),
        ST::U32 => Some(TypeTag::U32),
        ST::U64 => Some(TypeTag::U64),
        ST::U128 => Some(TypeTag::U128),
        ST::U256 => Some(TypeTag::U256),
        ST::Address => Some(TypeTag::Address),
        ST::Signer => Some(TypeTag::Signer),
        ST::Vector(inner) => resolve_token_to_typetag_with_storage(storage, module_ctx, inner, func_ty_args).map(|t| TypeTag::Vector(Box::new(t))),
        ST::Struct(idx) => {
            let sh = module_ctx.struct_handle_at(*idx);
            let mh = module_ctx.module_handle_at(sh.module);
            Some(TypeTag::Struct(Box::new(aptos_move_core_types::language_storage::StructTag {
                address: *module_ctx.address_identifier_at(mh.address),
                module: module_ctx.identifier_at(mh.name).to_owned(),
                name: module_ctx.identifier_at(sh.name).to_owned(),
                type_args: vec![],
            })))
        }
        ST::StructInstantiation(idx, toks) => {
            let sh = module_ctx.struct_handle_at(*idx);
            let mh = module_ctx.module_handle_at(sh.module);
            let mut targs = Vec::with_capacity(toks.len());
            for t in toks { if let Some(tag) = resolve_token_to_typetag_with_storage(storage, module_ctx, t, func_ty_args) { targs.push(tag) } else { return None } }
            Some(TypeTag::Struct(Box::new(aptos_move_core_types::language_storage::StructTag {
                address: *module_ctx.address_identifier_at(mh.address),
                module: module_ctx.identifier_at(mh.name).to_owned(),
                name: module_ctx.identifier_at(sh.name).to_owned(),
                type_args: targs,
            })))
        }
        ST::TypeParameter(i) => func_ty_args.get(*i as usize).cloned(),
        ST::Reference(_) | ST::MutableReference(_) | ST::Function(_, _, _) => None,
    }
}

fn minimal_move_value_for_typetag_full<S: ModuleStorage>(storage: &S, ty: &TypeTag) -> Option<MoveValue> {
    match ty {
        TypeTag::Bool => Some(MoveValue::Bool(false)),
        TypeTag::U8 => Some(MoveValue::U8(0)),
        TypeTag::U16 => Some(MoveValue::U16(0)),
        TypeTag::U32 => Some(MoveValue::U32(0)),
        TypeTag::U64 => Some(MoveValue::U64(0)),
        TypeTag::U128 => Some(MoveValue::U128(0)),
        TypeTag::U256 => Some(MoveValue::U256(0u128.into())),
        TypeTag::Address => Some(MoveValue::Address(FUZZER_SENDER)),
        TypeTag::Signer => Some(MoveValue::Signer(FUZZER_SENDER)),
        TypeTag::Vector(inner) => {
            if matches!(**inner, TypeTag::U8) { Some(MoveValue::vector_u8(vec![])) } else { Some(MoveValue::Vector(vec![])) }
        }
        TypeTag::Struct(st) => minimal_move_value_for_struct_tag(storage, st),
        TypeTag::Function(_) => None,
    }
}

fn minimal_move_value_for_struct_tag<S: ModuleStorage>(storage: &S, st: &aptos_move_core_types::language_storage::StructTag) -> Option<MoveValue> {
    let module_id = ModuleId::new(st.address, st.module.clone());
    let cm = storage.unmetered_get_deserialized_module(module_id.address(), module_id.name()).ok().flatten()?;
    minimal_move_value_for_struct(storage, module_id.address(), module_id.name(), &st.name, &st.type_args)
}

fn minimal_move_value_for_struct<S: ModuleStorage>(
    storage: &S,
    module_addr: &aptos_move_core_types::account_address::AccountAddress,
    module_name: &IdentStr,
    struct_name: &aptos_move_core_types::identifier::Identifier,
    type_args: &[TypeTag],
) -> Option<MoveValue> {
    let cm = storage.unmetered_get_deserialized_module(module_addr, module_name).ok().flatten()?;
    // find struct definition by name
    for def in cm.struct_defs() {
        let sh = cm.struct_handle_at(def.struct_handle);
        if cm.identifier_at(sh.name) == struct_name.as_ident_str() {
            use aptos_move_binary_format::file_format::StructFieldInformation as SFI;
            match &def.field_information {
                SFI::Native => return None,
                SFI::Declared(fields) => {
                    let mut vals: Vec<MoveValue> = Vec::with_capacity(fields.len());
                    for f in fields {
                        let ftok = &f.signature.0;
                        if let Some(tag) = struct_field_token_to_typetag(&cm, ftok, type_args) {
                            if let Some(mv) = minimal_move_value_for_typetag_full(storage, &tag) { vals.push(mv); } else { return None }
                        } else { return None }
                    }
                    return Some(MoveValue::Struct(MoveStruct::Runtime(vals)));
                }
                SFI::DeclaredVariants(_variants) => {
                    // TODO: enum support - not yet implemented
                    return None;
                }
            }
        }
    }
    None
}

fn struct_field_token_to_typetag(
    module: &aptos_move_binary_format::CompiledModule,
    tok: &aptos_move_binary_format::file_format::SignatureToken,
    struct_type_args: &[TypeTag],
) -> Option<TypeTag> {
    use aptos_move_binary_format::file_format::SignatureToken as ST;
    match tok {
        ST::Bool => Some(TypeTag::Bool),
        ST::U8 => Some(TypeTag::U8),
        ST::U16 => Some(TypeTag::U16),
        ST::U32 => Some(TypeTag::U32),
        ST::U64 => Some(TypeTag::U64),
        ST::U128 => Some(TypeTag::U128),
        ST::U256 => Some(TypeTag::U256),
        ST::Address => Some(TypeTag::Address),
        ST::Signer => Some(TypeTag::Signer),
        ST::Vector(inner) => struct_field_token_to_typetag(module, inner, struct_type_args).map(|t| TypeTag::Vector(Box::new(t))),
        ST::Struct(idx) => {
            let sh = module.struct_handle_at(*idx);
            let mh = module.module_handle_at(sh.module);
            Some(TypeTag::Struct(Box::new(aptos_move_core_types::language_storage::StructTag {
                address: *module.address_identifier_at(mh.address),
                module: module.identifier_at(mh.name).to_owned(),
                name: module.identifier_at(sh.name).to_owned(),
                type_args: vec![],
            })))
        }
        ST::StructInstantiation(idx, toks) => {
            let sh = module.struct_handle_at(*idx);
            let mh = module.module_handle_at(sh.module);
            let mut targs = Vec::with_capacity(toks.len());
            for t in toks { if let Some(tag) = struct_field_token_to_typetag(module, t, struct_type_args) { targs.push(tag) } else { return None } }
            Some(TypeTag::Struct(Box::new(aptos_move_core_types::language_storage::StructTag {
                address: *module.address_identifier_at(mh.address),
                module: module.identifier_at(mh.name).to_owned(),
                name: module.identifier_at(sh.name).to_owned(),
                type_args: targs,
            })))
        }
        ST::TypeParameter(i) => struct_type_args.get(*i as usize).cloned(),
        ST::Reference(_) | ST::MutableReference(_) | ST::Function(_, _, _) => None,
    }
}
