use crate::{ConfigError, WatchHandle};
use etcd_client::{Client, GetOptions};
use serde::{de::DeserializeOwned, Serialize};
use tracing::{debug, info};

/// Configuration client backed by etcd
pub struct ConfigClient {
    client: Client,
    prefix: String,
}

impl ConfigClient {
    /// Create a new config client connected to etcd
    pub async fn new(endpoints: &[&str]) -> Result<Self, ConfigError> {
        Self::with_prefix(endpoints, "").await
    }

    /// Create a new config client with a key prefix
    pub async fn with_prefix(endpoints: &[&str], prefix: &str) -> Result<Self, ConfigError> {
        info!("Connecting to etcd at {:?}", endpoints);
        let client = Client::connect(endpoints, None).await?;
        Ok(Self {
            client,
            prefix: prefix.to_string(),
        })
    }

    /// Get a typed configuration value
    pub async fn get<T: DeserializeOwned>(&self, key: &str) -> Result<T, ConfigError> {
        let full_key = self.full_key(key);
        debug!("Getting config: {}", full_key);

        let resp = self.client.clone().get(full_key.clone(), None).await?;

        let kv = resp.kvs().first()
            .ok_or_else(|| ConfigError::NotFound(full_key.clone()))?;

        let value: T = serde_json::from_slice(kv.value())?;
        Ok(value)
    }

    /// Get a raw JSON value
    pub async fn get_raw(&self, key: &str) -> Result<serde_json::Value, ConfigError> {
        self.get(key).await
    }

    /// Set a configuration value
    pub async fn set<T: Serialize>(&self, key: &str, value: &T) -> Result<(), ConfigError> {
        let full_key = self.full_key(key);
        let json = serde_json::to_vec(value)?;

        debug!("Setting config: {}", full_key);
        self.client.clone().put(full_key, json, None).await?;
        Ok(())
    }

    /// Set a raw JSON value
    pub async fn set_raw(&self, key: &str, value: &serde_json::Value) -> Result<(), ConfigError> {
        self.set(key, value).await
    }

    /// Delete a configuration key
    pub async fn delete(&self, key: &str) -> Result<(), ConfigError> {
        let full_key = self.full_key(key);
        debug!("Deleting config: {}", full_key);
        self.client.clone().delete(full_key, None).await?;
        Ok(())
    }

    /// List all keys under a prefix
    pub async fn list(&self, prefix: &str) -> Result<Vec<String>, ConfigError> {
        let full_prefix = self.full_key(prefix);
        let opts = GetOptions::new().with_prefix();

        let resp = self.client.clone().get(full_prefix.clone(), Some(opts)).await?;

        let keys: Vec<String> = resp.kvs()
            .iter()
            .filter_map(|kv| String::from_utf8(kv.key().to_vec()).ok())
            .collect();

        Ok(keys)
    }

    /// Watch for changes on a key prefix
    pub async fn watch<F>(&self, prefix: &str, callback: F) -> Result<WatchHandle, ConfigError>
    where
        F: Fn(String, Option<serde_json::Value>) + Send + Sync + 'static,
    {
        let full_prefix = self.full_key(prefix);
        WatchHandle::new(self.client.clone(), &full_prefix, callback).await
    }

    /// Get with environment variable override
    /// Checks ENV_PREFIX_KEY before etcd
    pub async fn get_with_env<T: DeserializeOwned>(&self, key: &str, env_prefix: &str) -> Result<T, ConfigError> {
        // Convert key to env var name: /mqtt/broker_url -> MQTT_BROKER_URL
        let env_key = format!("{}_{}",
            env_prefix,
            key.trim_start_matches('/')
                .replace('/', "_")
                .to_uppercase()
        );

        if let Ok(env_val) = std::env::var(&env_key) {
            debug!("Using env override for {}: {}", key, env_key);
            return serde_json::from_str(&format!("\"{}\"", env_val))
                .or_else(|_| serde_json::from_str(&env_val))
                .map_err(|e| ConfigError::EnvError(format!("Failed to parse {}: {}", env_key, e)));
        }

        self.get(key).await
    }

    fn full_key(&self, key: &str) -> String {
        if self.prefix.is_empty() {
            key.to_string()
        } else {
            format!("{}{}", self.prefix, key)
        }
    }
}
