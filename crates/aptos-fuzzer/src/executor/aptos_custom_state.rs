use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use aptos_aggregator::bounded_math::SignedU128;
use aptos_aggregator::resolver::{TAggregatorV1View, TDelayedFieldView};
use aptos_aggregator::types::{DelayedFieldValue, DelayedFieldsSpeculativeError};
use aptos_cached_packages::head_release_bundle;
use aptos_gas_schedule::{MiscGasParameters, NativeGasParameters};
use aptos_move_binary_format::access::ModuleAccess;
use aptos_move_binary_format::control_flow_graph::{ControlFlowGraph, VMControlFlowGraph};
use aptos_move_binary_format::errors::{PartialVMError, PartialVMResult, VMResult};
use aptos_move_binary_format::file_format::CompiledScript;
use aptos_move_binary_format::CompiledModule;
use aptos_move_core_types::account_address::AccountAddress;
use aptos_move_core_types::identifier::IdentStr;
use aptos_move_core_types::language_storage::{ModuleId, StructTag};
use aptos_move_core_types::metadata::Metadata;
use aptos_move_core_types::value::MoveTypeLayout;
use aptos_move_table_extension::{TableHandle, TableResolver};
use aptos_move_vm_runtime::{Module, ModuleStorage, RuntimeEnvironment, Script, WithRuntimeEnvironment};
use aptos_move_vm_types::code::{Code, ScriptCache};
use aptos_move_vm_types::delayed_values::delayed_field_id::DelayedFieldID;
use aptos_move_vm_types::resolver::ResourceResolver;
use aptos_native_interface::SafeNativeBuilder;
use aptos_types::chain_id::ChainId;
use aptos_types::error::{PanicError, PanicOr};
use aptos_types::on_chain_config::{ConfigStorage, Features, TimedFeaturesBuilder};
use aptos_types::state_store::errors::StateViewError;
use aptos_types::state_store::state_key::inner::StateKeyInner;
use aptos_types::state_store::state_key::StateKey;
use aptos_types::state_store::state_storage_usage::StateStorageUsage;
use aptos_types::state_store::state_value::{StateValue, StateValueMetadata};
use aptos_types::state_store::StateViewId;
use aptos_types::write_set::{TransactionWrite, WriteSet};
use aptos_vm::move_vm_ext::{AptosMoveResolver, AsExecutorView, AsResourceGroupView, ResourceGroupResolver};
use aptos_vm_environment::natives::aptos_natives_with_builder;
use aptos_vm_environment::prod_configs::{aptos_default_ty_builder, aptos_prod_vm_config};
use aptos_vm_types::module_and_script_storage::module_storage::AptosModuleStorage;
use aptos_vm_types::resolver::{
    BlockSynchronizationKillSwitch, ExecutorView, ResourceGroupSize, ResourceGroupView, StateStorageView,
    TResourceGroupView, TResourceView,
};
use bytes::Bytes;
use dashmap::DashMap;

#[derive(Clone)]
pub struct AptosCustomState {
    kv_state: HashMap<StateKey, StateValue>,
    tables: HashMap<(TableHandle, Vec<u8>), Bytes>,
    modules: HashMap<ModuleId, Bytes>,
    scripts_deser: DashMap<[u8; 32], Arc<CompiledScript>>,
    scripts_verified: DashMap<[u8; 32], Arc<Script>>,
    runtime_environment: RuntimeEnvironment,
}

macro_rules! unknown_status {
    () => {
        PartialVMError::new(aptos_types::vm_status::StatusCode::UNKNOWN_STATUS)
    };
}

impl AptosMoveResolver for AptosCustomState {}

impl TAggregatorV1View for AptosCustomState {
    type Identifier = StateKey;

    fn get_aggregator_v1_state_value(&self, id: &StateKey) -> PartialVMResult<Option<StateValue>> {
        match self.kv_state.get(id) {
            Some(v) => Ok(Some(v.clone())),
            None => Err(unknown_status!()),
        }
    }
}

// Delayed fields unused in this executor; fail fast to surface accidental
// usage.
impl TDelayedFieldView for AptosCustomState {
    type Identifier = DelayedFieldID;
    type ResourceKey = StateKey;
    type ResourceGroupTag = StructTag;

    fn get_delayed_field_value(
        &self,
        _id: &DelayedFieldID,
    ) -> Result<DelayedFieldValue, PanicOr<DelayedFieldsSpeculativeError>> {
        Err(PanicOr::CodeInvariantError("unreachable".to_string()))
    }

    fn delayed_field_try_add_delta_outcome(
        &self,
        _id: &DelayedFieldID,
        _base_delta: &SignedU128,
        _delta: &SignedU128,
        _max_value: u128,
    ) -> Result<bool, PanicOr<DelayedFieldsSpeculativeError>> {
        Err(PanicOr::CodeInvariantError("unreachable".to_string()))
    }

    fn generate_delayed_field_id(&self, _width: u32) -> DelayedFieldID {
        DelayedFieldID::new_with_width(0x1337, 0x1338)
    }

    fn validate_delayed_field_id(&self, _id: &DelayedFieldID) -> Result<(), PanicError> {
        Err(PanicError::CodeInvariantError("unreachable".to_string()))
    }

    fn get_reads_needing_exchange(
        &self,
        _delayed_write_set_ids: &HashSet<DelayedFieldID>,
        _skip: &HashSet<StateKey>,
    ) -> Result<BTreeMap<StateKey, (StateValueMetadata, u64, Arc<MoveTypeLayout>)>, PanicError> {
        Err(PanicError::CodeInvariantError("unreachable".to_string()))
    }

    fn get_group_reads_needing_exchange(
        &self,
        _delayed_write_set_ids: &HashSet<DelayedFieldID>,
        _skip: &HashSet<StateKey>,
    ) -> PartialVMResult<BTreeMap<StateKey, (StateValueMetadata, u64)>> {
        Err(unknown_status!())
    }
}

impl ConfigStorage for AptosCustomState {
    fn fetch_config_bytes(&self, state_key: &StateKey) -> Option<Bytes> {
        self.kv_state.get(state_key).map(|v| v.bytes().clone())
    }
}

impl ResourceResolver for AptosCustomState {
    fn get_resource_bytes_with_metadata_and_layout(
        &self,
        address: &AccountAddress,
        struct_tag: &StructTag,
        _metadata: &[Metadata],
        _layout: Option<&MoveTypeLayout>,
    ) -> PartialVMResult<(Option<Bytes>, usize)> {
        let state_key = StateKey::resource(address, struct_tag).map_err(|_| unknown_status!())?;

        match self.kv_state.get(&state_key) {
            Some(state_value) => {
                let bytes = state_value.bytes();
                let size = bytes.len();
                Ok((Some(bytes.clone()), size))
            }
            None => Ok((None, 0)),
        }
    }
}

// Simple resolver; delegates to group view; no internal caching.
impl ResourceGroupResolver for AptosCustomState {
    fn release_resource_group_cache(&self) -> Option<HashMap<StateKey, BTreeMap<StructTag, Bytes>>> {
        // Return empty: no internal cache
        Some(HashMap::new())
    }

    fn resource_group_size(&self, group_key: &StateKey) -> PartialVMResult<ResourceGroupSize> {
        <Self as TResourceGroupView>::resource_group_size(self, group_key)
    }

    fn resource_size_in_group(&self, group_key: &StateKey, resource_tag: &StructTag) -> PartialVMResult<usize> {
        <Self as TResourceGroupView>::resource_size_in_group(self, group_key, resource_tag)
    }

    fn resource_exists_in_group(&self, group_key: &StateKey, resource_tag: &StructTag) -> PartialVMResult<bool> {
        <Self as TResourceGroupView>::resource_exists_in_group(self, group_key, resource_tag)
    }
}

impl StateStorageView for AptosCustomState {
    type Key = StateKey;

    fn id(&self) -> StateViewId {
        self.id()
    }

    fn read_state_value(&self, state_key: &StateKey) -> Result<(), StateViewError> {
        match self.kv_state.get(state_key) {
            Some(_) => Ok(()),
            None => Err(StateViewError::NotFound(format!("Key not found: {:?}", state_key))),
        }
    }

    fn get_usage(&self) -> Result<StateStorageUsage, StateViewError> {
        Ok(StateStorageUsage::Untracked)
    }
}

impl TableResolver for AptosCustomState {
    fn resolve_table_entry_bytes_with_layout(
        &self,
        handle: &TableHandle,
        key: &[u8],
        _maybe_layout: Option<&MoveTypeLayout>,
    ) -> Result<Option<Bytes>, PartialVMError> {
        let table_key = (*handle, key.to_vec());
        match self.tables.get(&table_key) {
            Some(bytes) => Ok(Some(bytes.clone())),
            None => Ok(None),
        }
    }
}

impl AsExecutorView for AptosCustomState {
    fn as_executor_view(&self) -> &dyn ExecutorView {
        self
    }
}

impl TResourceView for AptosCustomState {
    type Key = StateKey;
    type Layout = MoveTypeLayout;

    fn get_resource_state_value(
        &self,
        state_key: &StateKey,
        _maybe_layout: Option<&MoveTypeLayout>,
    ) -> PartialVMResult<Option<StateValue>> {
        match self.kv_state.get(state_key) {
            Some(state_value) => Ok(Some(state_value.clone())),
            None => Ok(None),
        }
    }

    fn get_resource_state_value_metadata(&self, state_key: &StateKey) -> PartialVMResult<Option<StateValueMetadata>> {
        match self.kv_state.get(state_key) {
            Some(state_value) => Ok(Some(state_value.metadata().clone())),
            None => Ok(None),
        }
    }

    fn get_resource_state_value_size(&self, state_key: &StateKey) -> PartialVMResult<u64> {
        match self.kv_state.get(state_key) {
            Some(state_value) => Ok(state_value.bytes().len() as u64),
            None => Ok(0),
        }
    }

    fn resource_exists(&self, state_key: &StateKey) -> PartialVMResult<bool> {
        Ok(self.kv_state.contains_key(state_key))
    }
}

impl AsResourceGroupView for AptosCustomState {
    fn as_resource_group_view(&self) -> &dyn ResourceGroupView {
        self
    }
}

// Group value is a BCS-encoded BTreeMap<StructTag, Bytes> under the group key.
impl TResourceGroupView for AptosCustomState {
    type GroupKey = StateKey;
    type ResourceTag = StructTag;
    type Layout = MoveTypeLayout;

    fn resource_group_size(&self, group_key: &StateKey) -> PartialVMResult<ResourceGroupSize> {
        match self.kv_state.get(group_key) {
            Some(state_value) => Ok(ResourceGroupSize::Concrete(state_value.bytes().len() as u64)),
            None => Ok(ResourceGroupSize::Concrete(0)),
        }
    }

    fn get_resource_from_group(
        &self,
        group_key: &StateKey,
        resource_tag: &StructTag,
        _maybe_layout: Option<&MoveTypeLayout>,
    ) -> PartialVMResult<Option<Bytes>> {
        let maybe_bytes = self.kv_state.get(group_key).map(|sv| sv.bytes().clone());
        if let Some(blob) = maybe_bytes {
            let map: BTreeMap<StructTag, Bytes> = bcs::from_bytes(&blob).map_err(|_| unknown_status!())?;
            Ok(map.get(resource_tag).cloned())
        } else {
            Ok(None)
        }
    }

    fn resource_size_in_group(&self, group_key: &StateKey, resource_tag: &StructTag) -> PartialVMResult<usize> {
        let maybe_bytes = self.kv_state.get(group_key).map(|sv| sv.bytes().clone());
        if let Some(blob) = maybe_bytes {
            let map: BTreeMap<StructTag, Bytes> = bcs::from_bytes(&blob).map_err(|_| unknown_status!())?;
            Ok(map.get(resource_tag).map_or(0, |v| v.len()))
        } else {
            Ok(0)
        }
    }

    fn resource_exists_in_group(&self, group_key: &StateKey, resource_tag: &StructTag) -> PartialVMResult<bool> {
        let maybe_bytes = self.kv_state.get(group_key).map(|sv| sv.bytes().clone());
        if let Some(blob) = maybe_bytes {
            let map: BTreeMap<StructTag, Bytes> = bcs::from_bytes(&blob).map_err(|_| unknown_status!())?;
            Ok(map.contains_key(resource_tag))
        } else {
            Ok(false)
        }
    }

    fn release_group_cache(&self) -> Option<HashMap<StateKey, BTreeMap<StructTag, Bytes>>> {
        // Return empty: no caching
        Some(HashMap::new())
    }
}

impl AptosModuleStorage for AptosCustomState {
    fn unmetered_get_module_state_value_metadata(
        &self,
        address: &AccountAddress,
        module_name: &IdentStr,
    ) -> PartialVMResult<Option<StateValueMetadata>> {
        let state_key = StateKey::module(address, module_name);

        match self.kv_state.get(&state_key) {
            Some(state_value) => Ok(Some(state_value.metadata().clone())),
            None => Ok(None),
        }
    }
}

impl ModuleStorage for AptosCustomState {
    #[doc = " Returns true if the module exists, and false otherwise. An error is returned if there is a"]
    #[doc = " storage error."]
    #[doc = ""]
    #[doc = " Note: this API is not metered!"]
    fn unmetered_check_module_exists(&self, address: &AccountAddress, module_name: &IdentStr) -> VMResult<bool> {
        let module_id = ModuleId::new(*address, module_name.to_owned());
        Ok(self.modules.contains_key(&module_id))
    }

    #[doc = " Returns module bytes if module exists, or [None] otherwise. An error is returned if there"]
    #[doc = " is a storage error."]
    #[doc = ""]
    #[doc = " Note: this API is not metered!"]
    fn unmetered_get_module_bytes(&self, address: &AccountAddress, module_name: &IdentStr) -> VMResult<Option<Bytes>> {
        let module_id = ModuleId::new(*address, module_name.to_owned());
        Ok(self.modules.get(&module_id).cloned())
    }

    #[doc = " Returns the size of a module in bytes, or [None] otherwise. An error is returned if the"]
    #[doc = " there is a storage error."]
    #[doc = ""]
    #[doc = " Note: this API is not metered! It is only used to get the size of a module so that metering"]
    #[doc = " can actually be implemented before loading a module."]
    fn unmetered_get_module_size(&self, address: &AccountAddress, module_name: &IdentStr) -> VMResult<Option<usize>> {
        let module_id = ModuleId::new(*address, module_name.to_owned());
        Ok(self.modules.get(&module_id).map(|bytes| bytes.len()))
    }

    #[doc = " Returns the metadata in the module, or [None] otherwise. An error is returned if there is"]
    #[doc = " a storage error or the module fails deserialization."]
    #[doc = ""]
    #[doc = " Note: this API is not metered!"]
    fn unmetered_get_module_metadata(
        &self,
        address: &AccountAddress,
        module_name: &IdentStr,
    ) -> VMResult<Option<Vec<Metadata>>> {
        let module_id = ModuleId::new(*address, module_name.to_owned());
        match self.modules.get(&module_id) {
            Some(bytes) => match CompiledModule::deserialize(bytes) {
                Ok(module) => Ok(Some(module.metadata)),
                Err(_) => Ok(None),
            },
            None => Ok(None),
        }
    }

    #[doc = " Returns the deserialized module, or [None] otherwise. An error is returned if:"]
    #[doc = "   1. the deserialization fails, or"]
    #[doc = "   2. there is an error from the underlying storage."]
    #[doc = ""]
    #[doc = " Note: this API is not metered!"]
    fn unmetered_get_deserialized_module(
        &self,
        address: &AccountAddress,
        module_name: &IdentStr,
    ) -> VMResult<Option<Arc<CompiledModule>>> {
        let module_id = ModuleId::new(*address, module_name.to_owned());
        match self.modules.get(&module_id) {
            Some(bytes) => match CompiledModule::deserialize(bytes) {
                Ok(module) => Ok(Some(Arc::new(module))),
                Err(_) => Ok(None),
            },
            None => Ok(None),
        }
    }

    #[doc = " Returns the verified module if it exists, or [None] otherwise. The existing module can be"]
    #[doc = " either in a cached state (it is then returned) or newly constructed. The error is returned"]
    #[doc = " if the storage fails to fetch the deserialized module and verify it. The verification is"]
    #[doc = " eager: i.e., it addition to local module verification there are also linking checks and"]
    #[doc = " verification of transitive dependencies."]
    #[doc = ""]
    #[doc = " Note 1: this API is not metered!"]
    #[doc = " Note 2: this API is used before lazy loading was enabled!"]
    fn unmetered_get_eagerly_verified_module(
        &self,
        _address: &AccountAddress,
        _module_name: &IdentStr,
    ) -> VMResult<Option<Arc<Module>>> {
        // No caching/verification here; upstream handles verification.
        Ok(None)
    }

    #[doc = " Returns the verified module if it exists, or [None] otherwise. The existing module can be"]
    #[doc = " either in a cached state (it is then returned) or newly constructed. The error is returned"]
    #[doc = " if the storage fails to fetch the deserialized module and verify it. The verification is"]
    #[doc = " lazy: i.e., it is only local to the module without any linking checks."]
    #[doc = ""]
    #[doc = " Note 1: this API is not metered!"]
    #[doc = " Note 2: this API is used after lazy loading was enabled!"]
    fn unmetered_get_lazily_verified_module(&self, _module_id: &ModuleId) -> VMResult<Option<Arc<Module>>> {
        // No lazy verification; return None.
        Ok(None)
    }
}

// TODO: Clarify script role in Aptos; decide fuzzing relevance.
impl ScriptCache for AptosCustomState {
    type Key = [u8; 32];
    type Deserialized = CompiledScript;
    type Verified = Script;

    fn insert_deserialized_script(&self, key: [u8; 32], deserialized_script: CompiledScript) -> Arc<CompiledScript> {
        let deserialized_script = Arc::new(deserialized_script);
        self.scripts_deser.insert(key, deserialized_script.clone());
        deserialized_script
    }

    fn insert_verified_script(&self, key: [u8; 32], verified_script: Script) -> Arc<Script> {
        let verified_script = Arc::new(verified_script);
        self.scripts_verified.insert(key, verified_script.clone());
        verified_script
    }

    fn get_script(&self, key: &[u8; 32]) -> Option<Code<CompiledScript, Script>> {
        if let Some(script) = self.scripts_deser.get(key) {
            return Some(Code::from_deserialized(script.as_ref().clone()));
        }

        if let Some(script) = self.scripts_verified.get(key) {
            return Some(Code::from_arced_verified(script.clone()));
        }

        None
    }

    fn num_scripts(&self) -> usize {
        let keys_deser = self
            .scripts_deser
            .iter()
            .map(|item| *item.key())
            .collect::<HashSet<_>>();
        let keys_verified = self
            .scripts_verified
            .iter()
            .map(|item| *item.key())
            .collect::<HashSet<_>>();
        let keys = keys_deser.union(&keys_verified).collect::<HashSet<_>>();
        keys.len()
    }
}

impl BlockSynchronizationKillSwitch for AptosCustomState {
    // Cannot be interrupted
    fn interrupt_requested(&self) -> bool {
        false
    }
}

impl WithRuntimeEnvironment for AptosCustomState {
    fn runtime_environment(&self) -> &RuntimeEnvironment {
        &self.runtime_environment
    }
}

impl AptosCustomState {
    pub fn runtime_environment(&self) -> &RuntimeEnvironment {
        &self.runtime_environment
    }
}

impl Default for AptosCustomState {
    fn default() -> Self {
        Self::new_default()
    }
}

impl std::fmt::Debug for AptosCustomState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AptosCustomState")
            .field("kv_state_len", &self.kv_state.len())
            .field("tables_len", &self.tables.len())
            .field("modules_len", &self.modules.len())
            .field("scripts_deser_len", &self.scripts_deser.len())
            .field("scripts_verified_len", &self.scripts_verified.len())
            .finish()
    }
}

impl AptosCustomState {
    pub fn new_default() -> Self {
        // This mirrors aptos-core's AptosEnvironment defaults when on-chain configs are
        // missing.
        let chain_id = ChainId::test();
        let features = Features::default();
        let timed_features = TimedFeaturesBuilder::new(chain_id, 0).build();
        let gas_feature_version = 0u64;
        let mut builder = SafeNativeBuilder::new(
            gas_feature_version,
            NativeGasParameters::zeros(),
            MiscGasParameters::zeros(),
            timed_features.clone(),
            features.clone(),
            None,
        );
        let natives = aptos_natives_with_builder(&mut builder, false);
        let vm_config = aptos_prod_vm_config(
            gas_feature_version,
            &features,
            &timed_features,
            aptos_default_ty_builder(),
        );
        let runtime_environment = RuntimeEnvironment::new_with_config(natives, vm_config);

        // Seed essential on-chain config state with sane defaults works.
        let mut kv_state: HashMap<StateKey, StateValue> = HashMap::new();

        // 0x1::chain_id::ChainId
        if let Ok(state_key) = aptos_types::state_store::state_key::StateKey::on_chain_config::<ChainId>() {
            let bytes = bcs::to_bytes(&chain_id).expect("serialize ChainId");
            kv_state.insert(state_key, StateValue::new_legacy(bytes.into()));
        }

        // 0x1::aptos_features::Features
        if let Ok(state_key) = aptos_types::state_store::state_key::StateKey::on_chain_config::<Features>() {
            let bytes = bcs::to_bytes(&features).expect("serialize Features");
            kv_state.insert(state_key, StateValue::new_legacy(bytes.into()));
        }
        let mut this = Self {
            kv_state,
            tables: HashMap::new(),
            modules: HashMap::new(),
            scripts_deser: DashMap::new(),
            scripts_verified: DashMap::new(),
            runtime_environment,
        };

        // Load and deploy Aptos framework bundle (includes move-stdlib, aptos-stdlib,
        // aptos-framework, etc.)
        let bundle = head_release_bundle();
        for (module_bytes, module) in bundle.code_and_compiled_modules() {
            let module_id = module.self_id();
            this.deploy_module_bytes(module_id.clone(), module_bytes.to_vec());
        }

        this
    }

    pub fn default_env() -> aptos_vm_environment::environment::AptosEnvironment {
        let tmp = Self::new_default();
        let view = crate::executor::custom_state_view::CustomStateView::new(&tmp);
        aptos_vm_environment::environment::AptosEnvironment::new(&view)
    }

    pub fn id(&self) -> StateViewId {
        StateViewId::Miscellaneous
    }

    pub fn get_state_value(&self, state_key: &StateKey) -> Option<StateValue> {
        self.kv_state.get(state_key).cloned()
    }

    // Apply WriteSet to in-memory state; mirror modules from code access paths.
    pub fn apply_write_set(&mut self, write_set: &WriteSet) {
        for (state_key, write_op) in write_set.write_op_iter() {
            match state_key.inner() {
                StateKeyInner::TableItem { handle, key } => {
                    let table_handle = TableHandle(handle.0);
                    match write_op.bytes() {
                        Some(bytes) => {
                            self.tables.insert((table_handle, key.clone()), bytes.clone());
                        }
                        None => {
                            self.tables.remove(&(table_handle, key.clone()));
                        }
                    }
                }
                StateKeyInner::AccessPath(access_path) => {
                    // Always update kv_state
                    match write_op.as_state_value() {
                        Some(state_value) => {
                            self.kv_state.insert(state_key.clone(), state_value);
                        }
                        None => {
                            self.kv_state.remove(state_key);
                        }
                    }

                    // If module code, also maintain modules cache
                    if access_path.is_code() {
                        if let Some(module_id) = access_path.try_get_module_id() {
                            match write_op.bytes() {
                                Some(bytes) => {
                                    self.modules.insert(module_id, bytes.clone());
                                }
                                None => {
                                    self.modules.remove(&module_id);
                                }
                            }
                        }
                    }
                }
                StateKeyInner::Raw(_) => match write_op.as_state_value() {
                    Some(state_value) => {
                        self.kv_state.insert(state_key.clone(), state_value);
                    }
                    None => {
                        self.kv_state.remove(state_key);
                    }
                },
            }
        }
    }

    pub fn deploy_module_bytes(&mut self, module_id: ModuleId, code: Vec<u8>) {
        let bytes = Bytes::from(code);
        let state_key = StateKey::module(module_id.address(), module_id.name());
        self.modules.insert(module_id.clone(), bytes.clone());
        self.kv_state.insert(state_key, StateValue::new_legacy(bytes));
    }

    // Calculate total bytecode instructions across all modules
    pub fn total_bytecode_instructions(&self) -> usize {
        let mut total = 0;
        for bytes in self.modules.values() {
            if let Ok(module) = CompiledModule::deserialize(bytes) {
                for func_def in module.function_defs() {
                    if let Some(code_unit) = &func_def.code {
                        total += code_unit.code.len();
                    }
                }
            }
        }
        total
    }

    // Calculate total possible edges (control flow transitions) across all modules
    pub fn total_possible_edges(&self) -> usize {
        let mut total = 0;
        for bytes in self.modules.values() {
            if let Ok(module) = CompiledModule::deserialize(bytes) {
                for func_def in module.function_defs() {
                    if let Some(code_unit) = &func_def.code {
                        let cfg = VMControlFlowGraph::new(&code_unit.code);
                        for block_id in cfg.blocks() {
                            total += cfg.successors(block_id).len();
                        }
                    }
                }
            }
        }
        total
    }
}
