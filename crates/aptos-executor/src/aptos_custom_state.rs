use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use aptos_aggregator::bounded_math::SignedU128;
use aptos_aggregator::resolver::{TAggregatorV1View, TDelayedFieldView};
use aptos_aggregator::types::{DelayedFieldValue, DelayedFieldsSpeculativeError};
use aptos_move_binary_format::CompiledModule;
use aptos_move_binary_format::errors::{PartialVMError, PartialVMResult, VMResult};
use aptos_move_binary_format::file_format::CompiledScript;
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
use aptos_types::error::{PanicError, PanicOr};
use aptos_types::on_chain_config::ConfigStorage;
use aptos_types::state_store::StateViewId;
use aptos_types::state_store::errors::StateViewError;
use aptos_types::state_store::state_key::StateKey;
use aptos_types::state_store::state_storage_usage::StateStorageUsage;
use aptos_types::state_store::state_value::{StateValue, StateValueMetadata};
use aptos_vm::move_vm_ext::{AptosMoveResolver, AsExecutorView, AsResourceGroupView, ResourceGroupResolver};
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

// Do we need to implement this?
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
        match self.kv_state.get(state_key) {
            Some(v) => Some(v.bytes().clone()),
            None => None,
        }
    }
}

impl ResourceResolver for AptosCustomState {
    fn get_resource_bytes_with_metadata_and_layout(
        &self,
        address: &AccountAddress,
        struct_tag: &StructTag,
        metadata: &[Metadata],
        layout: Option<&MoveTypeLayout>,
    ) -> PartialVMResult<(Option<Bytes>, usize)> {
        todo!()
    }
}

impl ResourceGroupResolver for AptosCustomState {
    fn release_resource_group_cache(&self) -> Option<HashMap<StateKey, BTreeMap<StructTag, Bytes>>> {
        todo!()
    }

    fn resource_group_size(&self, group_key: &StateKey) -> PartialVMResult<ResourceGroupSize> {
        todo!()
    }

    fn resource_size_in_group(&self, group_key: &StateKey, resource_tag: &StructTag) -> PartialVMResult<usize> {
        todo!()
    }

    fn resource_exists_in_group(&self, group_key: &StateKey, resource_tag: &StructTag) -> PartialVMResult<bool> {
        todo!()
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
        maybe_layout: Option<&MoveTypeLayout>,
    ) -> Result<Option<Bytes>, PartialVMError> {
        todo!()
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
        maybe_layout: Option<&MoveTypeLayout>,
    ) -> PartialVMResult<Option<StateValue>> {
        todo!()
    }

    fn get_resource_state_value_metadata(&self, state_key: &StateKey) -> PartialVMResult<Option<StateValueMetadata>> {
        todo!()
    }

    fn get_resource_state_value_size(&self, state_key: &StateKey) -> PartialVMResult<u64> {
        todo!()
    }

    fn resource_exists(&self, state_key: &StateKey) -> PartialVMResult<bool> {
        todo!()
    }
}

impl AsResourceGroupView for AptosCustomState {
    fn as_resource_group_view(&self) -> &dyn ResourceGroupView {
        self
    }
}

impl TResourceGroupView for AptosCustomState {
    type GroupKey = StateKey;
    type ResourceTag = StructTag;
    type Layout = MoveTypeLayout;

    fn resource_group_size(&self, group_key: &StateKey) -> PartialVMResult<ResourceGroupSize> {
        todo!()
    }

    fn get_resource_from_group(
        &self,
        group_key: &StateKey,
        resource_tag: &StructTag,
        maybe_layout: Option<&MoveTypeLayout>,
    ) -> PartialVMResult<Option<Bytes>> {
        todo!()
    }

    fn resource_size_in_group(&self, group_key: &StateKey, resource_tag: &StructTag) -> PartialVMResult<usize> {
        todo!()
    }

    fn resource_exists_in_group(&self, group_key: &StateKey, resource_tag: &StructTag) -> PartialVMResult<bool> {
        todo!()
    }

    fn release_group_cache(&self) -> Option<HashMap<StateKey, BTreeMap<StructTag, Bytes>>> {
        todo!()
    }
}

impl AptosModuleStorage for AptosCustomState {
    fn unmetered_get_module_state_value_metadata(
        &self,
        address: &AccountAddress,
        module_name: &IdentStr,
    ) -> PartialVMResult<Option<StateValueMetadata>> {
        todo!()
    }
}

impl ModuleStorage for AptosCustomState {
    #[doc = " Returns true if the module exists, and false otherwise. An error is returned if there is a"]
    #[doc = " storage error."]
    #[doc = ""]
    #[doc = " Note: this API is not metered!"]
    fn unmetered_check_module_exists(&self, address: &AccountAddress, module_name: &IdentStr) -> VMResult<bool> {
        todo!()
    }

    #[doc = " Returns module bytes if module exists, or [None] otherwise. An error is returned if there"]
    #[doc = " is a storage error."]
    #[doc = ""]
    #[doc = " Note: this API is not metered!"]
    fn unmetered_get_module_bytes(&self, address: &AccountAddress, module_name: &IdentStr) -> VMResult<Option<Bytes>> {
        todo!()
    }

    #[doc = " Returns the size of a module in bytes, or [None] otherwise. An error is returned if the"]
    #[doc = " there is a storage error."]
    #[doc = ""]
    #[doc = " Note: this API is not metered! It is only used to get the size of a module so that metering"]
    #[doc = " can actually be implemented before loading a module."]
    fn unmetered_get_module_size(&self, address: &AccountAddress, module_name: &IdentStr) -> VMResult<Option<usize>> {
        todo!()
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
        todo!()
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
        todo!()
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
        address: &AccountAddress,
        module_name: &IdentStr,
    ) -> VMResult<Option<Arc<Module>>> {
        todo!()
    }

    #[doc = " Returns the verified module if it exists, or [None] otherwise. The existing module can be"]
    #[doc = " either in a cached state (it is then returned) or newly constructed. The error is returned"]
    #[doc = " if the storage fails to fetch the deserialized module and verify it. The verification is"]
    #[doc = " lazy: i.e., it is only local to the module without any linking checks."]
    #[doc = ""]
    #[doc = " Note 1: this API is not metered!"]
    #[doc = " Note 2: this API is used after lazy loading was enabled!"]
    fn unmetered_get_lazily_verified_module(&self, module_id: &ModuleId) -> VMResult<Option<Arc<Module>>> {
        todo!()
    }
}

// TODO: find out the role of script in Aptos
// in sui scripts are deprecated
// decide do we need it and how will it be used (for fuzzing)
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
    // cannot be interrupted
    fn interrupt_requested(&self) -> bool {
        false
    }
}

impl WithRuntimeEnvironment for AptosCustomState {
    fn runtime_environment(&self) -> &RuntimeEnvironment {
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
        todo!("implement")
    }
}

impl AptosCustomState {
    pub fn new_default() -> Self {
        todo!("implement")
    }

    pub fn id(&self) -> StateViewId {
        StateViewId::Miscellaneous
    }
}
