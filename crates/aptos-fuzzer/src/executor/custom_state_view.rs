use aptos_types::state_store::state_key::StateKey;
use aptos_types::state_store::state_storage_usage::StateStorageUsage;
use aptos_types::state_store::{StateViewResult, TStateView};

use super::aptos_custom_state::AptosCustomState;

/// Minimal StateView wrapper over `AptosCustomState` so we can adapt it to
/// AptosCodeStorage and reuse Move VM's loader and caches.
pub struct CustomStateView<'a> {
    pub(crate) state: &'a AptosCustomState,
}

impl<'a> CustomStateView<'a> {
    pub fn new(state: &'a AptosCustomState) -> Self {
        Self { state }
    }
}

impl<'a> TStateView for CustomStateView<'a> {
    type Key = StateKey;

    fn get_usage(&self) -> StateViewResult<StateStorageUsage> {
        Ok(StateStorageUsage::Untracked)
    }

    fn get_state_value(
        &self,
        state_key: &StateKey,
    ) -> StateViewResult<Option<aptos_types::state_store::state_value::StateValue>> {
        Ok(self.state.get_state_value(state_key))
    }
}


