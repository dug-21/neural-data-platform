//! Configuration store implementation

use std::collections::HashMap;

use crate::{ConfigError, ConfigSource, ConfigStore, Result};

/// Default implementation of ConfigStore
pub struct DefaultConfigStore {
    config: HashMap<String, serde_json::Value>,
}

impl DefaultConfigStore {
    /// Create a new configuration store
    pub fn new(
        sources: Vec<Box<dyn ConfigSource>>,
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

        Ok(Self { config })
    }

    /// Get raw value by key
    pub fn get_raw(&self, key: &str) -> Option<&serde_json::Value> {
        self.config.get(key)
    }

    /// Set raw value by key
    pub fn set_raw(&mut self, key: &str, value: serde_json::Value) {
        self.config.insert(key.to_string(), value);
    }

    /// Merge another configuration into this one
    pub fn merge(&mut self, other: HashMap<String, serde_json::Value>) {
        self.config.extend(other);
    }

    /// Get all configuration as a nested JSON object
    pub fn as_nested_object(&self) -> serde_json::Value {
        let mut result = serde_json::Map::new();

        for (key, value) in &self.config {
            set_nested_value(&mut result, key, value.clone());
        }

        serde_json::Value::Object(result)
    }
}

impl ConfigStore for DefaultConfigStore {
    fn load<T>(&self, key: &str) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        let value = self.config.get(key)
            .ok_or_else(|| ConfigError::key_not_found(key))?;

        serde_json::from_value(value.clone())
            .map_err(|e| ConfigError::from(e))
    }

    fn store<T>(&mut self, key: &str, value: &T) -> Result<()>
    where
        T: serde::Serialize,
    {
        let json_value = serde_json::to_value(value)?;
        self.config.insert(key.to_string(), json_value);
        Ok(())
    }

    fn exists(&self, key: &str) -> bool {
        self.config.contains_key(key)
    }

    fn remove(&mut self, key: &str) -> Result<()> {
        self.config.remove(key)
            .ok_or_else(|| ConfigError::key_not_found(key))?;
        Ok(())
    }

    fn keys(&self) -> Vec<String> {
        self.config.keys().cloned().collect()
    }
}

/// Set a nested value in a JSON object using dot notation
fn set_nested_value(object: &mut serde_json::Map<String, serde_json::Value>, key: &str, value: serde_json::Value) {
    let parts: Vec<&str> = key.split('.').collect();
    
    if parts.len() == 1 {
        object.insert(key.to_string(), value);
        return;
    }

    let mut current = object;
    
    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            // Last part, insert the value
            current.insert(part.to_string(), value);
            return;
        } else {
            // Intermediate part, ensure there's an object
            let entry = current.entry(part.to_string()).or_insert_with(|| {
                serde_json::Value::Object(serde_json::Map::new())
            });
            
            match entry {
                serde_json::Value::Object(ref mut map) => {
                    current = map;
                }
                _ => {
                    // Overwrite non-object with object
                    *entry = serde_json::Value::Object(serde_json::Map::new());
                    if let serde_json::Value::Object(ref mut map) = entry {
                        current = map;
                    }
                }
            }
        }
    }
}