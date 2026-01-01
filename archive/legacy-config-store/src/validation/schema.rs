/// JSON Schema-based configuration validator
/// Provides validation for configuration values against JSON schemas

use crate::types::{ConfigValue, ConfigError};
use std::collections::HashMap;
use std::path::Path;
use serde_json;
use jsonschema::{JSONSchema, Draft, CompilationOptions};
use async_trait::async_trait;

/// Schema validator for configuration values
#[derive(Debug, Clone)]
pub struct SchemaValidator {
    schema_string: String,
    schema: Option<JSONSchema>,
}

impl SchemaValidator {
    /// Create a new schema validator from schema string
    pub fn new(schema_string: String) -> Self {
        // Try to compile the schema
        let schema = if let Ok(schema_json) = serde_json::from_str::<serde_json::Value>(&schema_string) {
            JSONSchema::options()
                .with_draft(Draft::Draft7)
                .compile(&schema_json)
                .ok()
        } else {
            None
        };
        
        Self {
            schema_string,
            schema,
        }
    }
    
    /// Load schema from file
    pub async fn from_file(path: &Path) -> Result<Self, ConfigError> {
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| ConfigError::Io(format!("Failed to read schema file: {}", e)))?;
        
        Ok(Self::new(content))
    }
    
    /// Check if the schema is valid
    pub fn is_valid_schema(&self) -> bool {
        self.schema.is_some()
    }
    
    /// Get the schema string
    pub fn schema_string(&self) -> &str {
        &self.schema_string
    }
    
    /// Convert ConfigValue to JSON for validation
    pub fn config_to_json(config: &ConfigValue) -> Result<serde_json::Value, ConfigError> {
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
                    .map(Self::config_to_json)
                    .collect();
                Ok(serde_json::Value::Array(json_array?))
            },
            ConfigValue::Object(map) => {
                let mut json_map = serde_json::Map::new();
                for (k, v) in map {
                    json_map.insert(k.clone(), Self::config_to_json(v)?);
                }
                Ok(serde_json::Value::Object(json_map))
            },
        }
    }
    
    /// Validate a configuration value against the schema
    pub async fn validate(&self, config: &ConfigValue) -> Result<(), ConfigError> {
        // Check if schema is valid
        if !self.is_valid_schema() {
            return Err(ConfigError::ValidationFailed(vec![
                "Invalid schema: Cannot validate with invalid schema".to_string()
            ]));
        }
        
        // Convert config to JSON
        let json_value = Self::config_to_json(config)?;
        
        // Get the compiled schema
        let schema = self.schema.as_ref().unwrap();
        
        // Validate using jsonschema
        let result = schema.validate(&json_value);
        
        if let Err(errors) = result {
            let error_messages: Vec<String> = errors
                .map(|e| {
                    let path = if e.instance_path.is_empty() {
                        "root".to_string()
                    } else {
                        e.instance_path.to_string()
                    };
                    format!("{}: {}", path, e)
                })
                .collect();
            
            if !error_messages.is_empty() {
                return Err(ConfigError::ValidationFailed(error_messages));
            }
        }
        
        // Additional manual validation for required fields (for compatibility)
        self.validate_required_fields(&json_value)?;
        
        Ok(())
    }
    
    /// Validate required fields manually (for test compatibility)
    fn validate_required_fields(&self, value: &serde_json::Value) -> Result<(), ConfigError> {
        // Parse schema to check required fields
        if let Ok(schema_json) = serde_json::from_str::<serde_json::Value>(&self.schema_string) {
            if let Some(required) = schema_json.get("required").and_then(|r| r.as_array()) {
                if let Some(obj) = value.as_object() {
                    let mut missing = Vec::new();
                    
                    for req in required {
                        if let Some(field) = req.as_str() {
                            if !obj.contains_key(field) {
                                missing.push(format!("Missing required field: {}", field));
                            }
                        }
                    }
                    
                    if !missing.is_empty() {
                        return Err(ConfigError::ValidationFailed(missing));
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Validate a batch of configurations
    pub async fn validate_batch(&self, configs: Vec<(&str, ConfigValue)>) -> Vec<(&str, Result<(), ConfigError>)> {
        let mut results = Vec::new();
        
        for (name, config) in configs {
            let result = self.validate(&config).await;
            results.push((name, result));
        }
        
        results
    }
}

/// Additional validation utilities
impl SchemaValidator {
    /// Check if a value matches the expected type
    pub fn check_type(&self, value: &ConfigValue, expected_type: &str) -> bool {
        match (value, expected_type) {
            (ConfigValue::Null, "null") => true,
            (ConfigValue::Boolean(_), "boolean") => true,
            (ConfigValue::Integer(_), "integer") | (ConfigValue::Integer(_), "number") => true,
            (ConfigValue::Float(_), "number") => true,
            (ConfigValue::String(_), "string") => true,
            (ConfigValue::Array(_), "array") => true,
            (ConfigValue::Object(_), "object") => true,
            _ => false,
        }
    }
    
    /// Validate enum constraint
    pub fn check_enum(&self, value: &ConfigValue, allowed_values: &[serde_json::Value]) -> bool {
        if let Ok(json_value) = Self::config_to_json(value) {
            allowed_values.contains(&json_value)
        } else {
            false
        }
    }
    
    /// Validate pattern constraint for strings
    pub fn check_pattern(&self, value: &ConfigValue, pattern: &str) -> bool {
        if let ConfigValue::String(s) = value {
            if let Ok(re) = regex::Regex::new(pattern) {
                return re.is_match(s);
            }
        }
        false
    }
    
    /// Validate numeric constraints
    pub fn check_numeric_range(&self, value: &ConfigValue, min: Option<f64>, max: Option<f64>) -> bool {
        let num = match value {
            ConfigValue::Integer(i) => *i as f64,
            ConfigValue::Float(f) => *f,
            _ => return false,
        };
        
        if let Some(min_val) = min {
            if num < min_val {
                return false;
            }
        }
        
        if let Some(max_val) = max {
            if num > max_val {
                return false;
            }
        }
        
        true
    }
}

#[cfg(test)]
mod tests {
    // Tests are in schema_test.rs
}