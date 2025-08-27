//! Configuration loaders for different sources

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::{ConfigSource, Result};

/// File-based configuration source
pub struct FileSource {
    path: PathBuf,
}

impl FileSource {
    /// Create a new file source
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }
}

impl ConfigSource for FileSource {
    fn load(&self) -> Result<HashMap<String, serde_json::Value>> {
        let content = fs::read_to_string(&self.path)?;
        let extension = self.path.extension().and_then(|s| s.to_str());

        let value: serde_json::Value = match extension {
            Some("json") => serde_json::from_str(&content)?,
            Some("toml") => {
                let toml_value: toml::Value = toml::from_str(&content)?;
                serde_json::to_value(toml_value)?
            }
            Some("yaml") | Some("yml") => {
                let yaml_value: serde_yaml::Value = serde_yaml::from_str(&content)?;
                serde_json::to_value(yaml_value)?
            }
            _ => {
                // Try to parse as JSON first, then TOML, then YAML
                serde_json::from_str(&content)
                    .or_else(|_| {
                        let toml_value: toml::Value = toml::from_str(&content)?;
                        Ok(serde_json::to_value(toml_value)?)
                    })
                    .or_else(|_| {
                        let yaml_value: serde_yaml::Value = serde_yaml::from_str(&content)?;
                        Ok(serde_json::to_value(yaml_value)?)
                    })?
            }
        };

        flatten_json_object(value)
    }

    fn name(&self) -> &str {
        self.path.to_str().unwrap_or("unknown")
    }
}

/// Environment variable configuration source
pub struct EnvSource {
    prefix: String,
}

impl EnvSource {
    /// Create a new environment source with prefix
    pub fn new(prefix: &str) -> Self {
        Self {
            prefix: prefix.to_string(),
        }
    }
}

impl ConfigSource for EnvSource {
    fn load(&self) -> Result<HashMap<String, serde_json::Value>> {
        let mut config = HashMap::new();

        for (key, value) in std::env::vars() {
            if key.starts_with(&self.prefix) {
                let config_key = key
                    .strip_prefix(&self.prefix)
                    .unwrap_or(&key)
                    .trim_start_matches('_')
                    .to_lowercase()
                    .replace('_', ".");

                // Try to parse as JSON, fallback to string
                let json_value = serde_json::from_str(&value)
                    .unwrap_or_else(|_| serde_json::Value::String(value));

                config.insert(config_key, json_value);
            }
        }

        Ok(config)
    }

    fn name(&self) -> &str {
        "environment"
    }
}

/// Flatten nested JSON object into dot-notation keys
fn flatten_json_object(value: serde_json::Value) -> Result<HashMap<String, serde_json::Value>> {
    let mut result = HashMap::new();
    flatten_recursive("", value, &mut result);
    Ok(result)
}

fn flatten_recursive(
    prefix: &str,
    value: serde_json::Value,
    result: &mut HashMap<String, serde_json::Value>,
) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, val) in map {
                let new_key = if prefix.is_empty() {
                    key
                } else {
                    format!("{}.{}", prefix, key)
                };
                flatten_recursive(&new_key, val, result);
            }
        }
        other => {
            result.insert(prefix.to_string(), other);
        }
    }
}