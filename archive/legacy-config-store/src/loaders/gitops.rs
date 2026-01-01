/// GitOps Configuration Loader
/// Loads configurations from Git repository structure with base/overlay pattern

use crate::types::{ConfigValue, ConfigError, ConfigTree};
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use serde_yaml;
use serde_json;
use walkdir::WalkDir;
use async_trait::async_trait;

/// GitOps loader for loading configurations from filesystem
#[derive(Debug, Clone)]
pub struct GitOpsLoader {
    base_path: PathBuf,
    environment: String,
}

impl GitOpsLoader {
    /// Create a new GitOps loader
    pub fn new(base_path: PathBuf, environment: String) -> Self {
        Self {
            base_path,
            environment,
        }
    }
    
    /// Get the environment
    pub fn environment(&self) -> &str {
        &self.environment
    }
    
    /// Get the base path
    pub fn base_path(&self) -> &Path {
        &self.base_path
    }
    
    /// Load a YAML file and convert to ConfigValue
    pub async fn load_yaml_file(&self, path: &Path) -> Result<ConfigValue, ConfigError> {
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| ConfigError::Io(format!("Failed to read file {}: {}", path.display(), e)))?;
        
        let yaml_value: serde_yaml::Value = serde_yaml::from_str(&content)
            .map_err(|e| ConfigError::Parse(format!("Failed to parse YAML: {}", e)))?;
        
        self.yaml_to_config_value(yaml_value)
    }
    
    /// Convert YAML value to ConfigValue
    fn yaml_to_config_value(&self, yaml: serde_yaml::Value) -> Result<ConfigValue, ConfigError> {
        match yaml {
            serde_yaml::Value::Null => Ok(ConfigValue::Null),
            serde_yaml::Value::Bool(b) => Ok(ConfigValue::Boolean(b)),
            serde_yaml::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(ConfigValue::Integer(i))
                } else if let Some(f) = n.as_f64() {
                    Ok(ConfigValue::Float(f))
                } else {
                    Err(ConfigError::Parse("Invalid number format".to_string()))
                }
            },
            serde_yaml::Value::String(s) => Ok(ConfigValue::String(s)),
            serde_yaml::Value::Sequence(seq) => {
                let array: Result<Vec<ConfigValue>, ConfigError> = seq
                    .into_iter()
                    .map(|v| self.yaml_to_config_value(v))
                    .collect();
                Ok(ConfigValue::Array(array?))
            },
            serde_yaml::Value::Mapping(map) => {
                let mut object = HashMap::new();
                for (k, v) in map {
                    if let serde_yaml::Value::String(key) = k {
                        object.insert(key, self.yaml_to_config_value(v)?);
                    } else {
                        return Err(ConfigError::Parse("Non-string key in YAML mapping".to_string()));
                    }
                }
                Ok(ConfigValue::Object(object))
            },
        }
    }
    
    /// Load all base configurations
    pub async fn load_base_configs(&self) -> Result<HashMap<String, ConfigValue>, ConfigError> {
        let base_dir = self.base_path.join("base");
        self.load_configs_from_dir(&base_dir).await
    }
    
    /// Load overlay configurations for the environment
    pub async fn load_overlay_configs(&self) -> Result<HashMap<String, ConfigValue>, ConfigError> {
        let overlay_dir = self.base_path.join("overlays").join(&self.environment);
        if !overlay_dir.exists() {
            // No overlays for this environment
            return Ok(HashMap::new());
        }
        self.load_configs_from_dir(&overlay_dir).await
    }
    
    /// Load configurations from a directory
    async fn load_configs_from_dir(&self, dir: &Path) -> Result<HashMap<String, ConfigValue>, ConfigError> {
        let mut configs = HashMap::new();
        
        if !dir.exists() {
            return Ok(configs);
        }
        
        // Find all service directories
        let entries = std::fs::read_dir(dir)
            .map_err(|e| ConfigError::Io(format!("Failed to read directory {}: {}", dir.display(), e)))?;
        
        for entry in entries {
            let entry = entry.map_err(|e| ConfigError::Io(e.to_string()))?;
            let path = entry.path();
            
            if path.is_dir() {
                let service_name = path.file_name()
                    .and_then(|n| n.to_str())
                    .ok_or_else(|| ConfigError::Parse("Invalid service directory name".to_string()))?;
                
                let config_file = path.join("config.yaml");
                if config_file.exists() {
                    let config = self.load_yaml_file(&config_file).await?;
                    configs.insert(service_name.to_string(), config);
                }
            }
        }
        
        Ok(configs)
    }
    
    /// Merge two ConfigValues (overlay takes precedence)
    pub fn merge_configs(&self, base: &ConfigValue, overlay: &ConfigValue) -> Result<ConfigValue, ConfigError> {
        match (base, overlay) {
            (ConfigValue::Object(base_map), ConfigValue::Object(overlay_map)) => {
                let mut merged = base_map.clone();
                
                for (key, overlay_value) in overlay_map {
                    if let Some(base_value) = base_map.get(key) {
                        // Recursively merge if both are objects
                        merged.insert(key.clone(), self.merge_configs(base_value, overlay_value)?);
                    } else {
                        // Add new key from overlay
                        merged.insert(key.clone(), overlay_value.clone());
                    }
                }
                
                Ok(ConfigValue::Object(merged))
            },
            // For non-objects, overlay completely replaces base
            (_, overlay) => Ok(overlay.clone()),
        }
    }
    
    /// Load all configurations with base/overlay merging
    pub async fn load_all_configs(&self) -> Result<HashMap<String, ConfigValue>, ConfigError> {
        let base_configs = self.load_base_configs().await?;
        let overlay_configs = self.load_overlay_configs().await?;
        
        let mut merged_configs = HashMap::new();
        
        // Start with base configs
        for (service, base_config) in base_configs {
            if let Some(overlay_config) = overlay_configs.get(&service) {
                // Merge with overlay
                let merged = self.merge_configs(&base_config, overlay_config)?;
                merged_configs.insert(service, merged);
            } else {
                // No overlay, use base as-is
                merged_configs.insert(service, base_config);
            }
        }
        
        // Add any overlay-only configs
        for (service, overlay_config) in overlay_configs {
            if !merged_configs.contains_key(&service) {
                merged_configs.insert(service, overlay_config);
            }
        }
        
        Ok(merged_configs)
    }
    
    /// Validate configuration against schema
    pub async fn validate_config(&self, service: &str, config: &ConfigValue) -> Result<(), ConfigError> {
        let schema_path = self.base_path.join("schemas").join(format!("{}.json", service));
        
        if !schema_path.exists() {
            // No schema to validate against
            return Ok(());
        }
        
        // Load schema
        let schema_content = tokio::fs::read_to_string(&schema_path)
            .await
            .map_err(|e| ConfigError::Io(format!("Failed to read schema: {}", e)))?;
        
        let schema: serde_json::Value = serde_json::from_str(&schema_content)
            .map_err(|e| ConfigError::Parse(format!("Failed to parse schema: {}", e)))?;
        
        // Convert ConfigValue to JSON for validation
        let config_json = self.config_to_json(config)?;
        
        // Basic validation (would use jsonschema crate in production)
        self.validate_against_schema(&config_json, &schema)
    }
    
    /// Convert ConfigValue to JSON Value
    fn config_to_json(&self, config: &ConfigValue) -> Result<serde_json::Value, ConfigError> {
        match config {
            ConfigValue::Null => Ok(serde_json::Value::Null),
            ConfigValue::Boolean(b) => Ok(serde_json::Value::Bool(*b)),
            ConfigValue::Integer(i) => Ok(serde_json::Value::Number((*i).into())),
            ConfigValue::Float(f) => {
                serde_json::Number::from_f64(*f)
                    .map(serde_json::Value::Number)
                    .ok_or_else(|| ConfigError::Parse("Invalid float value".to_string()))
            },
            ConfigValue::String(s) => Ok(serde_json::Value::String(s.clone())),
            ConfigValue::Array(arr) => {
                let json_array: Result<Vec<serde_json::Value>, ConfigError> = arr
                    .iter()
                    .map(|v| self.config_to_json(v))
                    .collect();
                Ok(serde_json::Value::Array(json_array?))
            },
            ConfigValue::Object(map) => {
                let mut json_map = serde_json::Map::new();
                for (k, v) in map {
                    json_map.insert(k.clone(), self.config_to_json(v)?);
                }
                Ok(serde_json::Value::Object(json_map))
            },
        }
    }
    
    /// Basic schema validation
    fn validate_against_schema(&self, value: &serde_json::Value, schema: &serde_json::Value) -> Result<(), ConfigError> {
        // Check required fields
        if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
            if let serde_json::Value::Object(obj) = value {
                for req in required {
                    if let Some(field) = req.as_str() {
                        if !obj.contains_key(field) {
                            return Err(ConfigError::ValidationFailed(
                                vec![format!("Missing required field: {}", field)]
                            ));
                        }
                    }
                }
            }
        }
        
        // This is a simplified validation - in production would use jsonschema crate
        Ok(())
    }
    
    /// Build a configuration tree from loaded configs
    pub async fn build_config_tree(&self) -> Result<ConfigTree, ConfigError> {
        let configs = self.load_all_configs().await?;
        let mut tree = std::collections::BTreeMap::new();
        
        for (service, config) in configs {
            // Add service root
            let service_path = format!("/{}", service);
            tree.insert(service_path.clone(), config.clone());
            
            // Flatten the configuration into the tree
            self.flatten_config(&service_path, &config, &mut tree);
        }
        
        Ok(tree)
    }
    
    /// Flatten a configuration into a tree structure
    fn flatten_config(&self, prefix: &str, value: &ConfigValue, tree: &mut ConfigTree) {
        match value {
            ConfigValue::Object(map) => {
                for (key, val) in map {
                    let path = format!("{}/{}", prefix, key);
                    tree.insert(path.clone(), val.clone());
                    self.flatten_config(&path, val, tree);
                }
            },
            ConfigValue::Array(arr) => {
                for (i, val) in arr.iter().enumerate() {
                    let path = format!("{}[{}]", prefix, i);
                    tree.insert(path.clone(), val.clone());
                    self.flatten_config(&path, val, tree);
                }
            },
            _ => {
                // Leaf values are already inserted
            }
        }
    }
}

#[cfg(test)]
mod tests {
    // Tests are in gitops_test.rs
}