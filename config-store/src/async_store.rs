//! Async configuration store implementation

#[cfg(feature = "async")]
use std::collections::HashMap;
#[cfg(feature = "async")]
use tokio::sync::RwLock;

#[cfg(feature = "async")]
use crate::{ConfigError, ConfigSource, Result};

/// Async configuration store
#[cfg(feature = "async")]
pub struct AsyncConfigStore {
    config: RwLock<HashMap<String, serde_json::Value>>,
}

#[cfg(feature = "async")]
impl AsyncConfigStore {
    /// Create a new async configuration store
    pub async fn new(
        sources: Vec<Box<dyn ConfigSource + Send + Sync>>,
        env_prefix: Option<String>,
        defaults: HashMap<String, serde_json::Value>,
    ) -> Result<Self> {
        let mut config = defaults;

        // Load from all sources in order (later sources override earlier ones)
        for source in sources {
            let source_config = source.load().map_err(|e| {
                ConfigError::source(format!("Failed to load from {}: {}", source.name(), e))
            })?;
            config.extend(source_config);
        }

        // Load environment variables if prefix is provided
        if let Some(prefix) = env_prefix {
            let env_source = crate::loader::EnvSource::new(&prefix);
            let env_config = env_source.load()?;
            config.extend(env_config);
        }

        Ok(Self {
            config: RwLock::new(config),
        })
    }

    /// Load configuration value by key
    pub async fn load<T>(&self, key: &str) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let config = self.config.read().await;
        let value = config.get(key)
            .ok_or_else(|| ConfigError::key_not_found(key))?;

        serde_json::from_value(value.clone())
            .map_err(|e| ConfigError::from(e))
    }

    /// Store configuration value by key
    pub async fn store<T>(&self, key: &str, value: &T) -> Result<()>
    where
        T: serde::Serialize,
    {
        let json_value = serde_json::to_value(value)?;
        let mut config = self.config.write().await;
        config.insert(key.to_string(), json_value);
        Ok(())
    }

    /// Check if a configuration key exists
    pub async fn exists(&self, key: &str) -> bool {
        let config = self.config.read().await;
        config.contains_key(key)
    }

    /// Remove a configuration key
    pub async fn remove(&self, key: &str) -> Result<()> {
        let mut config = self.config.write().await;
        config.remove(key)
            .ok_or_else(|| ConfigError::key_not_found(key))?;
        Ok(())
    }

    /// List all configuration keys
    pub async fn keys(&self) -> Vec<String> {
        let config = self.config.read().await;
        config.keys().cloned().collect()
    }
}

#[cfg(not(feature = "async"))]
mod no_async {
    //! Placeholder when async feature is disabled
}

#[cfg(all(test, feature = "async"))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_async_store_basic() {
        let store = AsyncConfigStore::new(vec![], None, HashMap::new()).await.unwrap();
        
        store.store("test.key", &"test_value").await.unwrap();
        let value: String = store.load("test.key").await.unwrap();
        
        assert_eq!(value, "test_value");
    }
}