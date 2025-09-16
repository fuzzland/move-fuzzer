use std::sync::Arc;

use dashmap::DashMap;
use sui_json_rpc_types::SuiObjectDataOptions;
use sui_sdk::SuiClient;
use sui_types::base_types::{ObjectID, ObjectRef, SequenceNumber};
use sui_types::committee::EpochId;
use sui_types::error::{SuiError, SuiResult};
use sui_types::object::Object;
use sui_types::storage::{BackingPackageStore, ChildObjectResolver, ObjectStore, PackageObject, ParentSync};

/// RPC-based backing store that lazily fetches objects from a Sui node
pub struct RpcBackingStore {
    /// Sui RPC client
    pub sui_client: Arc<SuiClient>,
    /// Override objects (highest priority)
    pub overrides: Arc<DashMap<ObjectID, Object>>,
    /// Object cache (lazy loading from RPC)
    pub object_cache: Arc<DashMap<ObjectID, Object>>,
    /// Package cache
    pub package_cache: Arc<DashMap<ObjectID, PackageObject>>,
}

impl RpcBackingStore {
    pub fn new(sui_client: Arc<SuiClient>) -> Self {
        Self {
            sui_client,
            overrides: Arc::new(DashMap::new()),
            object_cache: Arc::new(DashMap::new()),
            package_cache: Arc::new(DashMap::new()),
        }
    }

    /// Add override objects
    pub fn add_overrides(&self, objects: Vec<(ObjectID, Object)>) {
        for (id, obj) in objects {
            self.overrides.insert(id, obj);
        }
    }

    /// Helper function to fetch object from RPC
    fn fetch_object_from_rpc(&self, object_id: &ObjectID) -> Option<Object> {
        // Use block_in_place to bridge async RPC call to sync context
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.sui_client
                    .read_api()
                    .get_object_with_options(*object_id, SuiObjectDataOptions::bcs_lossless())
                    .await
                    .ok()?
                    .data?
                    .try_into()
                    .ok()
            })
        })
    }
}

impl ObjectStore for RpcBackingStore {
    fn get_object(&self, object_id: &ObjectID) -> Option<Object> {
        // Priority 1: Check overrides
        if let Some(entry) = self.overrides.get(object_id) {
            return Some(entry.clone());
        }

        // Priority 2: Check cache
        if let Some(entry) = self.object_cache.get(object_id) {
            return Some(entry.clone());
        }

        // Priority 3: Fetch from RPC
        let obj = self.fetch_object_from_rpc(object_id)?;

        // Cache and return
        self.object_cache.insert(*object_id, obj.clone());
        Some(obj)
    }

    fn get_object_by_key(&self, object_id: &ObjectID, version: SequenceNumber) -> Option<Object> {
        // Priority 1: Check overrides
        if let Some(entry) = self.overrides.get(object_id) {
            if entry.version() == version {
                return Some(entry.clone());
            }
        }

        // Priority 2: Check cache
        if let Some(entry) = self.object_cache.get(object_id) {
            if entry.version() == version {
                return Some(entry.clone());
            }
        }

        // Fetch from RPC
        let obj = self.fetch_object_from_rpc(object_id)?;

        // Check version matches
        if obj.version() != version {
            return None;
        }

        // Cache and return
        self.object_cache.insert(*object_id, obj.clone());
        Some(obj)
    }
}

impl BackingPackageStore for RpcBackingStore {
    fn get_package_object(&self, package_id: &ObjectID) -> SuiResult<Option<PackageObject>> {
        // First check package cache
        if let Some(entry) = self.package_cache.get(package_id) {
            return Ok(Some(entry.clone()));
        }

        // Try to get object
        let obj = self.get_object(package_id);

        match obj {
            Some(obj) => {
                if !obj.is_package() {
                    return Err(SuiError::BadObjectType {
                        error: format!("Expected package, got: {:?}", obj.type_()),
                    });
                }

                let pkg = PackageObject::new(obj);

                // Cache and return
                self.package_cache.insert(*package_id, pkg.clone());
                Ok(Some(pkg))
            }
            None => Ok(None),
        }
    }
}

impl ChildObjectResolver for RpcBackingStore {
    fn read_child_object(
        &self,
        parent: &ObjectID,
        child: &ObjectID,
        child_version_upper_bound: SequenceNumber,
    ) -> SuiResult<Option<Object>> {
        // Get object and verify ownership and version
        let obj = self.get_object(child);

        if let Some(obj) = obj {
            // Check if object is a child of parent
            match obj.owner() {
                sui_types::object::Owner::ObjectOwner(owner_addr) => {
                    let owner_id = ObjectID::from(*owner_addr);
                    if owner_id != *parent {
                        return Ok(None);
                    }
                }
                _ => return Ok(None),
            }

            // Check version constraint
            if obj.version() > child_version_upper_bound {
                return Ok(None);
            }

            Ok(Some(obj))
        } else {
            Ok(None)
        }
    }

    fn get_object_received_at_version(
        &self,
        owner: &ObjectID,
        receiving_object_id: &ObjectID,
        receive_object_at_version: SequenceNumber,
        _epoch_id: EpochId,
    ) -> SuiResult<Option<Object>> {
        // Get object and verify ownership and version
        let obj = self.get_object(receiving_object_id);

        if let Some(obj) = obj {
            // Check if object is owned by owner
            match obj.owner() {
                sui_types::object::Owner::AddressOwner(addr) => {
                    if ObjectID::from(*addr) != *owner {
                        return Ok(None);
                    }
                }
                _ => return Ok(None),
            }

            // Check version matches
            if obj.version() != receive_object_at_version {
                return Ok(None);
            }

            Ok(Some(obj))
        } else {
            Ok(None)
        }
    }
}

impl ParentSync for RpcBackingStore {
    fn get_latest_parent_entry_ref_deprecated(&self, object_id: ObjectID) -> Option<ObjectRef> {
        // Get the latest version of the object
        self.get_object(&object_id).map(|obj| obj.compute_object_reference())
    }
}
