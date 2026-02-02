use crate::{ConfigClient, ConfigError};
use neural_core::StreamConfig;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// StreamRegistry provides stream configuration management
/// It wraps ConfigClient to load and watch stream configurations from etcd
pub struct StreamRegistry {
    client: ConfigClient,
    cache: Arc<RwLock<std::collections::HashMap<String, StreamConfig>>>,
}

impl StreamRegistry {
    /// Create a new StreamRegistry connected to etcd
    pub async fn new(endpoints: &[&str]) -> Result<Self, ConfigError> {
        info!(
            "Initializing StreamRegistry with etcd endpoints: {:?}",
            endpoints
        );
        let client = ConfigClient::with_prefix(endpoints, "/streams").await?;

        Ok(Self {
            client,
            cache: Arc::new(RwLock::new(std::collections::HashMap::new())),
        })
    }

    /// Load a specific stream configuration
    pub async fn load_stream(&self, stream_id: &str) -> Result<StreamConfig, ConfigError> {
        debug!("Loading stream configuration: {}", stream_id);

        // Try cache first
        {
            let cache = self.cache.read().await;
            if let Some(config) = cache.get(stream_id) {
                debug!("Stream {} found in cache", stream_id);
                return Ok(config.clone());
            }
        }

        // Load from etcd
        let key = format!("/{}/config", stream_id);
        let config: StreamConfig = self.client.get(&key).await?;

        // Validate before caching
        config
            .validate()
            .map_err(|e| ConfigError::EnvError(format!("Invalid stream config: {}", e)))?;

        // Update cache
        {
            let mut cache = self.cache.write().await;
            cache.insert(stream_id.to_string(), config.clone());
        }

        // dp-018 Task 1.6: Log config source for observability
        info!(
            "config loaded from etcd: /streams/{}/config",
            stream_id
        );
        Ok(config)
    }

    /// List all stream IDs
    pub async fn list_streams(&self) -> Result<Vec<String>, ConfigError> {
        debug!("Listing all streams");

        let keys = self.client.list("/").await?;

        // Extract stream IDs from keys like "/streams/air-quality/config"
        let stream_ids: Vec<String> = keys
            .iter()
            .filter_map(|key| {
                let parts: Vec<&str> = key.trim_start_matches("/streams/").split('/').collect();
                if parts.len() >= 2 && parts[1] == "config" {
                    Some(parts[0].to_string())
                } else {
                    None
                }
            })
            .collect::<std::collections::HashSet<_>>() // Deduplicate
            .into_iter()
            .collect();

        info!("Found {} streams", stream_ids.len());
        Ok(stream_ids)
    }

    /// Load all stream configurations
    pub async fn load_all_streams(
        &self,
    ) -> Result<std::collections::HashMap<String, StreamConfig>, ConfigError> {
        debug!("Loading all stream configurations");

        let stream_ids = self.list_streams().await?;
        let mut configs = std::collections::HashMap::new();

        for stream_id in stream_ids {
            match self.load_stream(&stream_id).await {
                Ok(config) => {
                    configs.insert(stream_id.clone(), config);
                }
                Err(e) => {
                    tracing::warn!("Failed to load stream {}: {}", stream_id, e);
                    // Continue loading other streams
                }
            }
        }

        info!("Loaded {} stream configurations", configs.len());
        Ok(configs)
    }

    /// Check if a stream exists
    pub async fn stream_exists(&self, stream_id: &str) -> Result<bool, ConfigError> {
        let key = format!("/{}/config", stream_id);
        match self.client.get::<serde_json::Value>(&key).await {
            Ok(_) => Ok(true),
            Err(ConfigError::NotFound(_)) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Save a stream configuration
    pub async fn save_stream(&self, config: &StreamConfig) -> Result<(), ConfigError> {
        debug!("Saving stream configuration: {}", config.stream_id);

        // Validate before saving
        config
            .validate()
            .map_err(|e| ConfigError::EnvError(format!("Invalid stream config: {}", e)))?;

        let key = format!("/{}/config", config.stream_id);
        self.client.set(&key, config).await?;

        // Update cache
        {
            let mut cache = self.cache.write().await;
            cache.insert(config.stream_id.clone(), config.clone());
        }

        info!("Saved stream configuration: {}", config.stream_id);
        Ok(())
    }

    /// Delete a stream configuration
    pub async fn delete_stream(&self, stream_id: &str) -> Result<(), ConfigError> {
        debug!("Deleting stream configuration: {}", stream_id);

        let key = format!("/{}/config", stream_id);
        self.client.delete(&key).await?;

        // Remove from cache
        {
            let mut cache = self.cache.write().await;
            cache.remove(stream_id);
        }

        info!("Deleted stream configuration: {}", stream_id);
        Ok(())
    }

    /// Clear the cache
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
        debug!("Cache cleared");
    }

    /// Get cached stream count
    pub async fn cache_size(&self) -> usize {
        let cache = self.cache.read().await;
        cache.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neural_core::{FieldType, SchemaField, SourceConfig, SourceType, StreamConfigError};
    use std::collections::HashMap;

    // ========== LONDON SCHOOL TDD: UNIT TESTS ==========
    // Note: These tests require a running etcd instance for integration testing
    // For true unit tests, we would mock ConfigClient

    fn create_test_config(stream_id: &str) -> StreamConfig {
        StreamConfig {
            stream_id: stream_id.to_string(),
            description: "Test stream".to_string(),
            version: "1.0.0".to_string(),
            enabled: true,
            retention_days: 30,
            compression_after_days: 7,
            partitioning_strategy: "daily".to_string(),
            fields: vec![SchemaField::new("value".to_string(), FieldType::Float)
                .required()
                .with_unit("test".to_string())],
            sources: vec![SourceConfig {
                source_type: SourceType::Mqtt,
                enabled: true,
                ndp_id: None,
                context: None,
                params: HashMap::new(),
            }],
            storage: None,
        }
    }

    // These tests demonstrate the expected behavior
    // In a real environment with etcd, these would be integration tests

    #[test]
    fn test_stream_config_validation_before_save() {
        let mut invalid_config = create_test_config("test");
        invalid_config.stream_id = "Invalid_ID".to_string(); // Invalid format

        let result = invalid_config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_stream_config_validation_no_fields() {
        let mut config = create_test_config("test");
        config.fields.clear();

        let result = config.validate();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), StreamConfigError::NoFields));
    }

    #[test]
    fn test_stream_config_validation_no_sources() {
        let mut config = create_test_config("test");
        config.sources.clear();

        let result = config.validate();
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), StreamConfigError::NoSources));
    }

    #[test]
    fn test_stream_config_validation_valid() {
        let config = create_test_config("test-stream");
        assert!(config.validate().is_ok());
    }

    // Integration test pattern (requires etcd)
    #[tokio::test]
    #[ignore] // Ignore by default, run with --ignored flag when etcd is available
    async fn test_registry_save_and_load() {
        let registry = StreamRegistry::new(&["http://localhost:2379"])
            .await
            .expect("Failed to connect to etcd");

        let config = create_test_config("test-stream");

        // Save
        registry.save_stream(&config).await.expect("Failed to save");

        // Load
        let loaded = registry
            .load_stream("test-stream")
            .await
            .expect("Failed to load");

        assert_eq!(loaded.stream_id, config.stream_id);
        assert_eq!(loaded.description, config.description);

        // Cleanup
        registry
            .delete_stream("test-stream")
            .await
            .expect("Failed to delete");
    }

    #[tokio::test]
    #[ignore]
    async fn test_registry_list_streams() {
        let registry = StreamRegistry::new(&["http://localhost:2379"])
            .await
            .expect("Failed to connect to etcd");

        // Save test streams
        let config1 = create_test_config("test-stream-1");
        let config2 = create_test_config("test-stream-2");

        registry
            .save_stream(&config1)
            .await
            .expect("Failed to save 1");
        registry
            .save_stream(&config2)
            .await
            .expect("Failed to save 2");

        // List
        let streams = registry.list_streams().await.expect("Failed to list");

        assert!(streams.contains(&"test-stream-1".to_string()));
        assert!(streams.contains(&"test-stream-2".to_string()));

        // Cleanup
        registry
            .delete_stream("test-stream-1")
            .await
            .expect("Failed to delete 1");
        registry
            .delete_stream("test-stream-2")
            .await
            .expect("Failed to delete 2");
    }

    #[tokio::test]
    #[ignore]
    async fn test_registry_stream_exists() {
        let registry = StreamRegistry::new(&["http://localhost:2379"])
            .await
            .expect("Failed to connect to etcd");

        let config = create_test_config("test-exists");

        // Should not exist
        assert!(!registry.stream_exists("test-exists").await.unwrap());

        // Save
        registry.save_stream(&config).await.expect("Failed to save");

        // Should exist
        assert!(registry.stream_exists("test-exists").await.unwrap());

        // Cleanup
        registry
            .delete_stream("test-exists")
            .await
            .expect("Failed to delete");

        // Should not exist again
        assert!(!registry.stream_exists("test-exists").await.unwrap());
    }

    #[tokio::test]
    #[ignore]
    async fn test_registry_cache() {
        let registry = StreamRegistry::new(&["http://localhost:2379"])
            .await
            .expect("Failed to connect to etcd");

        let config = create_test_config("test-cache");

        // Save
        registry.save_stream(&config).await.expect("Failed to save");

        // Load (should cache)
        registry
            .load_stream("test-cache")
            .await
            .expect("Failed to load");

        // Check cache size
        assert_eq!(registry.cache_size().await, 1);

        // Clear cache
        registry.clear_cache().await;
        assert_eq!(registry.cache_size().await, 0);

        // Cleanup
        registry
            .delete_stream("test-cache")
            .await
            .expect("Failed to delete");
    }

    #[tokio::test]
    #[ignore]
    async fn test_registry_load_all_streams() {
        let registry = StreamRegistry::new(&["http://localhost:2379"])
            .await
            .expect("Failed to connect to etcd");

        // Save multiple streams
        let config1 = create_test_config("test-all-1");
        let config2 = create_test_config("test-all-2");
        let config3 = create_test_config("test-all-3");

        registry
            .save_stream(&config1)
            .await
            .expect("Failed to save 1");
        registry
            .save_stream(&config2)
            .await
            .expect("Failed to save 2");
        registry
            .save_stream(&config3)
            .await
            .expect("Failed to save 3");

        // Load all
        let all_configs = registry
            .load_all_streams()
            .await
            .expect("Failed to load all");

        assert!(all_configs.contains_key("test-all-1"));
        assert!(all_configs.contains_key("test-all-2"));
        assert!(all_configs.contains_key("test-all-3"));
        assert!(all_configs.len() >= 3);

        // Cleanup
        registry
            .delete_stream("test-all-1")
            .await
            .expect("Failed to delete 1");
        registry
            .delete_stream("test-all-2")
            .await
            .expect("Failed to delete 2");
        registry
            .delete_stream("test-all-3")
            .await
            .expect("Failed to delete 3");
    }
}
