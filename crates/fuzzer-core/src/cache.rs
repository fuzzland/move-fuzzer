use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;

use lru::LruCache;
use rand::Rng;
use tracing::{debug, info};

use crate::{ChainAdapter, ObjectChange};

/// Generic object cache for storing historical versions of objects
/// Uses LRU eviction policy with adapter-provided digest-based deduplication
pub struct ObjectCache<A: ChainAdapter> {
    /// Per-object LRU caches, keyed by digest for deduplication
    caches: HashMap<A::ObjectId, LruCache<Vec<u8>, A::Object>>,
    /// Maximum versions to cache per object
    max_versions_per_object: usize,
    /// Reference to the chain adapter for computing digests
    adapter: Arc<A>,
}

impl<A: ChainAdapter> ObjectCache<A> {
    pub fn new(adapter: Arc<A>) -> Self {
        Self {
            caches: HashMap::new(),
            max_versions_per_object: 10_000,
            adapter,
        }
    }

    pub fn with_capacity(adapter: Arc<A>, max_versions_per_object: usize) -> Self {
        Self {
            caches: HashMap::new(),
            max_versions_per_object,
            adapter,
        }
    }

    pub fn process_changes(&mut self, changes: &[ObjectChange<A::ObjectId, A::Object>]) {
        let mut cached_count = 0;

        for change in changes {
            let digest = self.adapter.compute_object_digest(&change.object);
            self.add_object_with_digest(change.id.clone(), change.object.clone(), digest);
            cached_count += 1;
            debug!("Cached modified object: {:?}", change.id);
        }

        if cached_count > 0 {
            info!("Cached {} modified objects", cached_count);
        }
    }

    fn add_object_with_digest(&mut self, id: A::ObjectId, object: A::Object, digest: Vec<u8>) {
        let cache = self
            .caches
            .entry(id)
            .or_insert_with(|| LruCache::new(NonZeroUsize::new(self.max_versions_per_object).unwrap()));

        // LRU automatically handles:
        // - Capacity limits (evicts least recently used)
        // - Deduplication (same digest overwrites)
        cache.put(digest, object);
    }

    pub fn get_random_version(&self, id: &A::ObjectId) -> Option<A::Object> {
        self.caches.get(id).and_then(|cache| {
            let items: Vec<_> = cache.iter().map(|(_, obj)| obj.clone()).collect();

            if items.is_empty() {
                None
            } else {
                let mut rng = rand::rng();
                let index = rng.random_range(0..items.len());
                Some(items[index].clone())
            }
        })
    }

    pub fn has_cached_versions(&self, id: &A::ObjectId) -> bool {
        self.caches.get(id).map_or(false, |cache| !cache.is_empty())
    }

    pub fn cached_version_count(&self, id: &A::ObjectId) -> usize {
        self.caches.get(id).map_or(0, |cache| cache.len())
    }

    pub fn total_cached_objects(&self) -> usize {
        self.caches.values().map(|cache| cache.len()).sum()
    }

    pub fn cached_object_ids(&self) -> Vec<A::ObjectId> {
        self.caches.keys().cloned().collect()
    }

    #[cfg(test)]
    pub fn clear(&mut self) {
        self.caches.clear();
    }
}
