use std::sync::Arc;

use aptos_aggregator::resolver::{TAggregatorV1View, TDelayedFieldView};
use aptos_move_binary_format::CompiledModule;
use aptos_move_binary_format::errors::{PartialVMError, VMResult};
use aptos_move_binary_format::file_format::CompiledScript;
use aptos_move_core_types::account_address::AccountAddress;
use aptos_move_core_types::identifier::IdentStr;
use aptos_move_core_types::language_storage::{ModuleId, StructTag};
use aptos_move_core_types::metadata::Metadata;
use aptos_move_core_types::value::MoveTypeLayout;
use aptos_move_table_extension::TableResolver;
use aptos_move_vm_runtime::{Module, ModuleStorage, RuntimeEnvironment, Script, WithRuntimeEnvironment};
use aptos_move_vm_types::code::{Code, ScriptCache};
use aptos_move_vm_types::delayed_values::delayed_field_id::DelayedFieldID;
use aptos_move_vm_types::resolver::ResourceResolver;
use aptos_types::on_chain_config::ConfigStorage;
use aptos_types::state_store::StateViewId;
use aptos_types::state_store::state_key::StateKey;
use aptos_vm::move_vm_ext::{AptosMoveResolver, AsExecutorView, AsResourceGroupView, ResourceGroupResolver};
use aptos_vm_types::module_and_script_storage::module_storage::AptosModuleStorage;
use aptos_vm_types::resolver::{BlockSynchronizationKillSwitch, StateStorageView};
use bytes::Bytes;

pub struct AptosCustomState {}

impl AptosMoveResolver for AptosCustomState {}

// derived
// impl AggregatorV1Resolver for AptosCustomState {}
impl TAggregatorV1View for AptosCustomState {
    type Identifier = StateKey;

    fn get_aggregator_v1_state_value(
        &self,
        id: &Self::Identifier,
    ) -> aptos_move_binary_format::errors::PartialVMResult<Option<aptos_types::state_store::state_value::StateValue>>
    {
        todo!()
    }
}

impl ConfigStorage for AptosCustomState {
    fn fetch_config_bytes(&self, state_key: &StateKey) -> Option<Bytes> {
        todo!()
    }
}

// derived
// impl DelayedFieldResolver for AptosCustomState {}
impl TDelayedFieldView for AptosCustomState {
    type Identifier = DelayedFieldID;
    type ResourceKey = StateKey;
    type ResourceGroupTag = StructTag;

    fn get_delayed_field_value(
        &self,
        id: &Self::Identifier,
    ) -> Result<
        aptos_aggregator::types::DelayedFieldValue,
        aptos_types::error::PanicOr<aptos_aggregator::types::DelayedFieldsSpeculativeError>,
    > {
        todo!()
    }

    fn delayed_field_try_add_delta_outcome(
        &self,
        id: &Self::Identifier,
        base_delta: &aptos_aggregator::bounded_math::SignedU128,
        delta: &aptos_aggregator::bounded_math::SignedU128,
        max_value: u128,
    ) -> Result<bool, aptos_types::error::PanicOr<aptos_aggregator::types::DelayedFieldsSpeculativeError>> {
        todo!()
    }

    fn generate_delayed_field_id(&self, width: u32) -> Self::Identifier {
        todo!()
    }

    fn validate_delayed_field_id(&self, id: &Self::Identifier) -> Result<(), aptos_types::error::PanicError> {
        todo!()
    }

    fn get_reads_needing_exchange(
        &self,
        delayed_write_set_ids: &std::collections::HashSet<Self::Identifier>,
        skip: &std::collections::HashSet<Self::ResourceKey>,
    ) -> Result<
        std::collections::BTreeMap<
            Self::ResourceKey,
            (
                aptos_types::state_store::state_value::StateValueMetadata,
                u64,
                std::sync::Arc<MoveTypeLayout>,
            ),
        >,
        aptos_types::error::PanicError,
    > {
        todo!()
    }

    fn get_group_reads_needing_exchange(
        &self,
        delayed_write_set_ids: &std::collections::HashSet<Self::Identifier>,
        skip: &std::collections::HashSet<Self::ResourceKey>,
    ) -> aptos_move_binary_format::errors::PartialVMResult<
        std::collections::BTreeMap<Self::ResourceKey, (aptos_types::state_store::state_value::StateValueMetadata, u64)>,
    > {
        todo!()
    }
}

impl ResourceResolver for AptosCustomState {
    fn get_resource_bytes_with_metadata_and_layout(
        &self,
        address: &aptos_types::PeerId,
        struct_tag: &aptos_move_core_types::language_storage::StructTag,
        metadata: &[aptos_move_core_types::metadata::Metadata],
        layout: Option<&MoveTypeLayout>,
    ) -> aptos_move_binary_format::errors::PartialVMResult<(Option<Bytes>, usize)> {
        todo!()
    }
}

impl ResourceGroupResolver for AptosCustomState {
    fn release_resource_group_cache(
        &self,
    ) -> Option<
        std::collections::HashMap<
            aptos_types::state_store::state_key::StateKey,
            std::collections::BTreeMap<aptos_move_core_types::language_storage::StructTag, Bytes>,
        >,
    > {
        todo!()
    }

    fn resource_group_size(
        &self,
        group_key: &aptos_types::state_store::state_key::StateKey,
    ) -> aptos_move_binary_format::errors::PartialVMResult<aptos_vm_types::resolver::ResourceGroupSize> {
        todo!()
    }

    fn resource_size_in_group(
        &self,
        group_key: &aptos_types::state_store::state_key::StateKey,
        resource_tag: &aptos_move_core_types::language_storage::StructTag,
    ) -> aptos_move_binary_format::errors::PartialVMResult<usize> {
        todo!()
    }

    fn resource_exists_in_group(
        &self,
        group_key: &aptos_types::state_store::state_key::StateKey,
        resource_tag: &aptos_move_core_types::language_storage::StructTag,
    ) -> aptos_move_binary_format::errors::PartialVMResult<bool> {
        todo!()
    }
}

impl StateStorageView for AptosCustomState {
    type Key = StateKey;

    fn id(&self) -> StateViewId {
        todo!()
    }

    fn read_state_value(&self, state_key: &Self::Key) -> Result<(), aptos_types::state_store::errors::StateViewError> {
        todo!()
    }

    fn get_usage(
        &self,
    ) -> Result<
        aptos_types::state_store::state_storage_usage::StateStorageUsage,
        aptos_types::state_store::errors::StateViewError,
    > {
        todo!()
    }
}

impl TableResolver for AptosCustomState {
    fn resolve_table_entry_bytes_with_layout(
        &self,
        handle: &aptos_move_table_extension::TableHandle,
        key: &[u8],
        maybe_layout: Option<&MoveTypeLayout>,
    ) -> Result<Option<Bytes>, PartialVMError> {
        todo!()
    }
}

impl AsExecutorView for AptosCustomState {
    fn as_executor_view(&self) -> &dyn aptos_vm_types::resolver::ExecutorView {
        todo!()
    }
}

impl AsResourceGroupView for AptosCustomState {
    fn as_resource_group_view(&self) -> &dyn aptos_vm_types::resolver::ResourceGroupView {
        todo!()
    }
}

impl AptosCustomState {
    pub fn new() -> Self {
        Self {}
    }

    pub fn id(&self) -> StateViewId {
        StateViewId::Miscellaneous
    }
}

impl AptosModuleStorage for AptosCustomState {
    fn unmetered_get_module_state_value_metadata(
        &self,
        address: &aptos_types::PeerId,
        module_name: &aptos_move_core_types::identifier::IdentStr,
    ) -> aptos_move_binary_format::errors::PartialVMResult<
        Option<aptos_types::state_store::state_value::StateValueMetadata>,
    > {
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

impl ScriptCache for AptosCustomState {
    type Key = [u8; 32];

    type Deserialized = CompiledScript;

    type Verified = Script;

    #[doc = " If the entry associated with the key is vacant, inserts the script and returns its copy."]
    #[doc = " Otherwise, there is no insertion and the copy of existing entry is returned."]
    fn insert_deserialized_script(
        &self,
        key: Self::Key,
        deserialized_script: Self::Deserialized,
    ) -> Arc<Self::Deserialized> {
        todo!()
    }

    #[doc = " If the entry associated with the key is vacant, inserts the script and returns its copy."]
    #[doc = " If the entry associated with the key is occupied, but the entry is not verified, inserts"]
    #[doc = " the script returning the copy. Otherwise, there is no insertion and the copy of existing"]
    #[doc = " (verified) entry is returned."]
    fn insert_verified_script(&self, key: Self::Key, verified_script: Self::Verified) -> Arc<Self::Verified> {
        todo!()
    }

    #[doc = " Returns the script if it has been cached before, or [None] otherwise."]
    fn get_script(&self, key: &Self::Key) -> Option<Code<Self::Deserialized, Self::Verified>> {
        todo!()
    }

    #[doc = " Returns the number of scripts stored in cache."]
    fn num_scripts(&self) -> usize {
        todo!()
    }
}

impl BlockSynchronizationKillSwitch for AptosCustomState {
    fn interrupt_requested(&self) -> bool {
        todo!()
    }
}

impl WithRuntimeEnvironment for AptosCustomState {
    fn runtime_environment(&self) -> &RuntimeEnvironment {
        todo!()
    }
}
