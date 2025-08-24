//! Inference Cache Implementation

use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

pub struct InferenceCache {
    cache: RwLock<HashMap<String, CachedItem>>,
    ttl: Duration,
}

struct CachedItem {
    data: String,
    cached_at: Instant,
}

impl InferenceCache {
    pub fn new(ttl_seconds: u64) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            ttl: Duration::from_secs(ttl_seconds),
        }
    }

    pub async fn get(&self, key: &str) -> Option<String> {
        let cache = self.cache.read().await;
        if let Some(item) = cache.get(key) {
            if item.cached_at.elapsed() < self.ttl {
                return Some(item.data.clone());
            }
        }
        None
    }

    pub async fn set(&self, key: String, value: String) {
        let mut cache = self.cache.write().await;
        cache.insert(key, CachedItem {
            data: value,
            cached_at: Instant::now(),
        });
    }

    pub async fn clear(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }
}