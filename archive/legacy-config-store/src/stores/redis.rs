use crate::stores::in_memory::InMemoryConfigStore;
/// Redis-backed configuration store with in-memory caching
/// Provides distributed configuration management with local cache fallback
use crate::traits::ConfigStore;
use crate::types::{ConfigError, ConfigTree, ConfigValue};
use async_trait::async_trait;
use redis::aio::MultiplexedConnection;
use redis::{AsyncCommands, Client, RedisResult};
use serde_json;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

/// Redis configuration store with caching
#[derive(Clone)]
pub struct RedisConfigStore {
    environment: String,
    redis_url: String,
    client: Client,
    connection: Arc<Mutex<Option<MultiplexedConnection>>>,
    cache: Arc<Mutex<InMemoryConfigStore>>,
    ttl: Duration,
    fallback_mode: bool,
}

impl RedisConfigStore {
    /// Create a new Redis config store
    pub async fn new(redis_url: String, environment: String) -> Result<Self, ConfigError> {
        let client = Client::open(redis_url.as_str()).map_err(|e| {
            ConfigError::OperationFailed(format!("Failed to create Redis client: {}", e))
        })?;

        // Try to establish connection
        let connection = match client.get_multiplexed_tokio_connection().await {
            Ok(conn) => Some(conn),
            Err(e) => {
                // Log warning but continue with fallback mode
                eprintln!(
                    "Warning: Redis connection failed, running in fallback mode: {}",
                    e
                );
                None
            }
        };

        let fallback_mode = connection.is_none();

        Ok(Self {
            environment,
            redis_url,
            client,
            connection: Arc::new(Mutex::new(connection)),
            cache: Arc::new(Mutex::new(InMemoryConfigStore::new())),
            ttl: Duration::from_secs(3600), // Default 1 hour TTL
            fallback_mode,
        })
    }

    /// Create with custom TTL
    pub async fn with_ttl(
        redis_url: String,
        environment: String,
        ttl: Duration,
    ) -> Result<Self, ConfigError> {
        let mut store = Self::new(redis_url, environment).await?;
        store.ttl = ttl;
        Ok(store)
    }

    /// Get the environment
    pub fn environment(&self) -> &str {
        &self.environment
    }

    /// Get the TTL
    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    /// Set value in cache
    pub async fn cache_set(&self, path: &str, value: ConfigValue) {
        let cache = self.cache.lock().await;
        let _ = cache.set(path, value).await;
    }

    /// Build Redis key with environment prefix
    fn build_key(&self, path: &str) -> String {
        format!("config:{}:{}", self.environment, path)
    }

    /// Serialize ConfigValue for Redis storage
    fn serialize_value(&self, value: &ConfigValue) -> Result<String, ConfigError> {
        serde_json::to_string(value)
            .map_err(|e| ConfigError::SerializationError(format!("Failed to serialize: {}", e)))
    }

    /// Deserialize ConfigValue from Redis
    fn deserialize_value(&self, data: &str) -> Result<ConfigValue, ConfigError> {
        serde_json::from_str(data)
            .map_err(|e| ConfigError::Parse(format!("Failed to deserialize: {}", e)))
    }

    /// Set value if not exists (atomic operation)
    pub async fn set_if_not_exists(
        &self,
        path: &str,
        value: ConfigValue,
    ) -> Result<bool, ConfigError> {
        // Always update cache
        self.cache_set(path, value.clone()).await;

        if self.fallback_mode {
            return Ok(true);
        }

        let mut conn_guard = self.connection.lock().await;
        if let Some(ref mut conn) = *conn_guard {
            let key = self.build_key(path);
            let data = self.serialize_value(&value)?;

            let result: RedisResult<bool> = conn.set_nx(&key, data).await;
            match result {
                Ok(set) => Ok(set),
                Err(e) => {
                    eprintln!("Redis SET NX failed: {}", e);
                    Ok(true) // Fallback to success with cache
                }
            }
        } else {
            Ok(true) // No connection
        }
    }

    /// Bulk set operations
    pub async fn bulk_set(&self, configs: HashMap<String, ConfigValue>) -> Result<(), ConfigError> {
        // Update cache for all values
        for (path, value) in &configs {
            self.cache_set(path, value.clone()).await;
        }

        if self.fallback_mode {
            return Ok(());
        }

        let mut conn_guard = self.connection.lock().await;
        if let Some(ref mut conn) = *conn_guard {
            // Use pipeline for atomic bulk operations
            let mut pipe = redis::pipe();

            for (path, value) in configs {
                let key = self.build_key(&path);
                let data = self.serialize_value(&value)?;
                pipe.set_ex(&key, data, self.ttl.as_secs() as u64);
            }

            let _: RedisResult<()> = pipe.query_async(conn).await;
        }

        Ok(())
    }

    /// Bulk get operations
    pub async fn bulk_get(
        &self,
        paths: &[String],
    ) -> Result<HashMap<String, ConfigValue>, ConfigError> {
        let mut results = HashMap::new();

        // First try cache
        for path in paths {
            let cache = self.cache.lock().await;
            if let Ok(value) = cache.get(path).await {
                results.insert(path.clone(), value);
            }
        }

        // If all found in cache or in fallback mode, return
        if results.len() == paths.len() || self.fallback_mode {
            return Ok(results);
        }

        // Try Redis for missing values
        let mut conn_guard = self.connection.lock().await;
        if let Some(ref mut conn) = *conn_guard {
            let missing: Vec<String> = paths
                .iter()
                .filter(|p| !results.contains_key(*p))
                .map(|p| self.build_key(p))
                .collect();

            if !missing.is_empty() {
                let values: RedisResult<Vec<Option<String>>> = conn.get(missing.clone()).await;

                if let Ok(values) = values {
                    for (i, value) in values.into_iter().enumerate() {
                        if let Some(data) = value {
                            if let Ok(config_value) = self.deserialize_value(&data) {
                                let path = &paths[i];
                                results.insert(path.clone(), config_value);
                            }
                        }
                    }
                }
            }
        }

        Ok(results)
    }
}

#[async_trait]
impl ConfigStore for RedisConfigStore {
    async fn get(&self, path: &str) -> Result<ConfigValue, ConfigError> {
        // Try cache first
        {
            let cache = self.cache.lock().await;
            if let Ok(value) = cache.get(path).await {
                return Ok(value);
            }
        }

        // If in fallback mode, cache miss means not found
        if self.fallback_mode {
            return Err(ConfigError::NotFound(format!(
                "Configuration not found: {}",
                path
            )));
        }

        // Try Redis
        let mut conn_guard = self.connection.lock().await;
        if let Some(ref mut conn) = *conn_guard {
            let key = self.build_key(path);

            let result: RedisResult<Option<String>> = conn.get(&key).await;
            match result {
                Ok(Some(data)) => {
                    let value = self.deserialize_value(&data)?;
                    // Update cache with retrieved value
                    {
                        let cache = self.cache.lock().await;
                        let _ = cache.set(path, value.clone()).await;
                    }
                    Ok(value)
                }
                Ok(None) => Err(ConfigError::NotFound(format!(
                    "Configuration not found: {}",
                    path
                ))),
                Err(e) => {
                    eprintln!("Redis GET failed: {}", e);
                    Err(ConfigError::NotFound(format!(
                        "Configuration not found: {}",
                        path
                    )))
                }
            }
        } else {
            Err(ConfigError::NotFound(format!(
                "Configuration not found: {}",
                path
            )))
        }
    }

    async fn set(&self, path: &str, value: ConfigValue) -> Result<(), ConfigError> {
        // Always update cache
        {
            let cache = self.cache.lock().await;
            cache.set(path, value.clone()).await?;
        }

        // If in fallback mode, cache update is sufficient
        if self.fallback_mode {
            return Ok(());
        }

        // Update Redis with TTL
        let mut conn_guard = self.connection.lock().await;
        if let Some(ref mut conn) = *conn_guard {
            let key = self.build_key(path);
            let data = self.serialize_value(&value)?;

            let result: RedisResult<()> = conn.set_ex(&key, data, self.ttl.as_secs() as u64).await;
            match result {
                Ok(_) => Ok(()),
                Err(e) => {
                    eprintln!("Redis SET failed, but cache updated: {}", e);
                    Ok(()) // Cache update succeeded, so return success
                }
            }
        } else {
            Ok(())
        }
    }

    async fn delete(&self, path: &str) -> Result<(), ConfigError> {
        // Always delete from cache
        {
            let cache = self.cache.lock().await;
            cache.delete(path).await?;
        }

        // If in fallback mode, cache deletion is sufficient
        if self.fallback_mode {
            return Ok(());
        }

        // Delete from Redis
        let mut conn_guard = self.connection.lock().await;
        if let Some(ref mut conn) = *conn_guard {
            let key = self.build_key(path);
            let _: RedisResult<()> = conn.del(&key).await;
        }

        Ok(())
    }

    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>, ConfigError> {
        // Get keys from cache
        let cache_keys = {
            let cache = self.cache.lock().await;
            cache.list_keys(prefix).await?
        };

        // If in fallback mode, return cache keys only
        if self.fallback_mode {
            return Ok(cache_keys);
        }

        // Get keys from Redis
        let mut all_keys = cache_keys;

        let mut conn_guard = self.connection.lock().await;
        if let Some(ref mut conn) = *conn_guard {
            let pattern = format!("config:{}:{}*", self.environment, prefix);

            let result: RedisResult<Vec<String>> =
                redis::cmd("KEYS").arg(&pattern).query_async(conn).await;

            if let Ok(redis_keys) = result {
                let prefix_len = format!("config:{}:", self.environment).len();
                for key in redis_keys {
                    if key.len() > prefix_len {
                        let path = key[prefix_len..].to_string();
                        if !all_keys.contains(&path) {
                            all_keys.push(path);
                        }
                    }
                }
            }
        }

        all_keys.sort();
        Ok(all_keys)
    }

    async fn get_tree(&self, prefix: &str) -> Result<ConfigTree, ConfigError> {
        let keys = self.list_keys(prefix).await?;
        let mut tree = BTreeMap::new();

        for key in keys {
            if let Ok(value) = self.get(&key).await {
                tree.insert(key, value);
            }
        }

        Ok(tree)
    }

    async fn get_version(&self, _path: &str, _version: u32) -> Result<ConfigValue, ConfigError> {
        // Version support not implemented for Redis store
        // Would require additional data structure to store version history
        Err(ConfigError::Custom(
            "Version history not supported in Redis store".to_string(),
        ))
    }

    async fn get_history(
        &self,
        _path: &str,
    ) -> Result<Vec<crate::types::ConfigVersion>, ConfigError> {
        // History not implemented for Redis store
        // Would require additional data structure to store history
        Err(ConfigError::Custom(
            "History not supported in Redis store".to_string(),
        ))
    }

    async fn set_node(
        &self,
        path: &str,
        node: crate::types::ConfigNode,
    ) -> Result<(), ConfigError> {
        // For Redis, we just store the value part
        // Metadata could be stored in separate keys if needed
        self.set(path, node.value).await
    }

    async fn get_node(&self, path: &str) -> Result<crate::types::ConfigNode, ConfigError> {
        // Retrieve the value and construct a basic node
        let value = self.get(path).await?;
        Ok(crate::types::ConfigNode {
            path: path.to_string(),
            value,
            version: 1, // Default version since we don't track versions
            metadata: None,
            inheritance: None,
            schema: None,
        })
    }
}

#[cfg(test)]
mod tests {
    // Tests are in redis_test.rs
}
