use std::collections::HashMap;

use aptos_storage_interface::state_store::state_view::cached_state_view::CachedStateView;
use aptos_types::state_store::state_key::StateKey;
use aptos_types::state_store::state_storage_usage::StateStorageUsage;
use aptos_types::state_store::state_value::StateValue;
use aptos_types::state_store::{StateViewId, TStateView};

pub struct OverlayStateView {
    pub(crate) base: CachedStateView,
    pub(crate) overlay: HashMap<StateKey, Option<StateValue>>,
}

impl TStateView for OverlayStateView {
    type Key = StateKey;

    fn id(&self) -> StateViewId {
        StateViewId::Miscellaneous
    }

    fn get_usage(&self) -> aptos_types::state_store::StateViewResult<StateStorageUsage> {
        self.base.get_usage()
    }

    fn next_version(&self) -> aptos_types::transaction::Version {
        self.base.next_version()
    }

    fn get_state_value(&self, state_key: &Self::Key) -> aptos_types::state_store::StateViewResult<Option<StateValue>> {
        if let Some(v) = self.overlay.get(state_key) {
            return Ok(v.clone());
        }
        self.base.get_state_value(state_key)
    }
}
