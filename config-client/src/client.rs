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

        let kv = resp
            .kvs()
            .first()
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

        let resp = self
            .client
            .clone()
            .get(full_prefix.clone(), Some(opts))
            .await?;

        let keys: Vec<String> = resp
            .kvs()
            .iter()
            .filter_map(|kv| String::from_utf8(kv.key().to_vec()).ok())
            .collect();

        Ok(keys)
    }

    /// Get all key-value pairs under a prefix as raw JSON values
    ///
    /// Returns a vector of (key, value) pairs where keys are relative to the prefix.
    pub async fn get_prefix_raw(
        &self,
        prefix: &str,
    ) -> Result<Vec<(String, serde_json::Value)>, ConfigError> {
        let full_prefix = self.full_key(prefix);
        let opts = GetOptions::new().with_prefix();

        let resp = self
            .client
            .clone()
            .get(full_prefix.clone(), Some(opts))
            .await?;

        let mut results = Vec::new();
        for kv in resp.kvs() {
            if let Ok(key) = String::from_utf8(kv.key().to_vec()) {
                // Strip the prefix to get relative key
                let relative_key = key
                    .strip_prefix(&full_prefix)
                    .unwrap_or(&key)
                    .trim_start_matches('/')
                    .to_string();

                if let Ok(value) = serde_json::from_slice::<serde_json::Value>(kv.value()) {
                    results.push((relative_key, value));
                }
            }
        }

        Ok(results)
    }

    /// Get all keys under a prefix and unflatten into a nested JSON object
    ///
    /// Converts flattened keys like "a/b/c" with value "x" into {"a": {"b": {"c": "x"}}}
    pub async fn get_prefix_nested(&self, prefix: &str) -> Result<serde_json::Value, ConfigError> {
        let pairs = self.get_prefix_raw(prefix).await?;

        if pairs.is_empty() {
            return Err(ConfigError::NotFound(prefix.to_string()));
        }

        let mut root = serde_json::Map::new();

        for (key, value) in pairs {
            let parts: Vec<&str> = key.split('/').collect();
            insert_nested(&mut root, &parts, value);
        }

        Ok(serde_json::Value::Object(root))
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
    pub async fn get_with_env<T: DeserializeOwned>(
        &self,
        key: &str,
        env_prefix: &str,
    ) -> Result<T, ConfigError> {
        // Convert key to env var name: /mqtt/broker_url -> MQTT_BROKER_URL
        let env_key = format!(
            "{}_{}",
            env_prefix,
            key.trim_start_matches('/').replace('/', "_").to_uppercase()
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

/// Insert a value into a nested JSON map structure
///
/// Given parts ["a", "b", "c"] and value "x", creates {"a": {"b": {"c": "x"}}}
fn insert_nested(
    map: &mut serde_json::Map<String, serde_json::Value>,
    parts: &[&str],
    value: serde_json::Value,
) {
    if parts.is_empty() {
        return;
    }

    if parts.len() == 1 {
        map.insert(parts[0].to_string(), value);
        return;
    }

    let key = parts[0].to_string();
    let entry = map
        .entry(key)
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));

    if let serde_json::Value::Object(ref mut nested) = entry {
        insert_nested(nested, &parts[1..], value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_insert_nested_single_part() {
        let mut map = serde_json::Map::new();
        insert_nested(&mut map, &["key"], json!("value"));

        assert_eq!(map.get("key"), Some(&json!("value")));
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn test_insert_nested_deep_path() {
        let mut map = serde_json::Map::new();
        insert_nested(&mut map, &["a", "b", "c"], json!("deep_value"));

        let expected = json!({
            "a": {
                "b": {
                    "c": "deep_value"
                }
            }
        });

        assert_eq!(serde_json::Value::Object(map), expected);
    }

    #[test]
    fn test_insert_nested_multiple_keys_building_tree() {
        let mut map = serde_json::Map::new();

        // Insert multiple keys that share common prefixes
        insert_nested(
            &mut map,
            &["config", "database", "host"],
            json!("localhost"),
        );
        insert_nested(&mut map, &["config", "database", "port"], json!(5432));
        insert_nested(&mut map, &["config", "cache", "enabled"], json!(true));
        insert_nested(&mut map, &["metrics", "interval"], json!(60));

        let expected = json!({
            "config": {
                "database": {
                    "host": "localhost",
                    "port": 5432
                },
                "cache": {
                    "enabled": true
                }
            },
            "metrics": {
                "interval": 60
            }
        });

        assert_eq!(serde_json::Value::Object(map), expected);
    }

    #[test]
    fn test_insert_nested_empty_parts() {
        let mut map = serde_json::Map::new();
        map.insert("existing".to_string(), json!("value"));

        // Empty parts should be a no-op
        insert_nested(&mut map, &[], json!("should_not_appear"));

        // Map should remain unchanged
        assert_eq!(map.len(), 1);
        assert_eq!(map.get("existing"), Some(&json!("value")));
        assert!(!map.contains_key("should_not_appear"));
    }

    #[test]
    fn test_insert_nested_overwrites_existing_value() {
        let mut map = serde_json::Map::new();
        insert_nested(&mut map, &["key"], json!("original"));
        insert_nested(&mut map, &["key"], json!("updated"));

        assert_eq!(map.get("key"), Some(&json!("updated")));
    }

    #[test]
    fn test_insert_nested_complex_json_value() {
        let mut map = serde_json::Map::new();
        let complex_value = json!({
            "array": [1, 2, 3],
            "nested": {"inner": "data"}
        });

        insert_nested(&mut map, &["settings", "config"], complex_value.clone());

        let expected = json!({
            "settings": {
                "config": {
                    "array": [1, 2, 3],
                    "nested": {"inner": "data"}
                }
            }
        });

        assert_eq!(serde_json::Value::Object(map), expected);
    }

    #[test]
    fn test_insert_nested_preserves_sibling_keys() {
        let mut map = serde_json::Map::new();

        // Insert first path
        insert_nested(&mut map, &["parent", "child1"], json!("value1"));

        // Insert sibling path - should not overwrite child1
        insert_nested(&mut map, &["parent", "child2"], json!("value2"));

        let parent = map.get("parent").unwrap().as_object().unwrap();
        assert_eq!(parent.get("child1"), Some(&json!("value1")));
        assert_eq!(parent.get("child2"), Some(&json!("value2")));
    }
}
