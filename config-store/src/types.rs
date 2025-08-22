/// Core types for the configuration store system
/// 
/// This module defines the fundamental data structures used throughout
/// the configuration management system, including values, errors, and metadata.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;
use thiserror::Error;

/// Represents a configuration value with support for various data types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConfigValue {
    /// Null value
    Null,
    /// Boolean value
    Boolean(bool),
    /// Integer value
    Integer(i64),
    /// Floating point value
    Float(f64),
    /// String value
    String(String),
    /// Array of configuration values
    Array(Vec<ConfigValue>),
    /// Object/map of configuration values
    Object(HashMap<String, ConfigValue>),
}

impl ConfigValue {
    /// Create a new null value
    pub fn new() -> Self {
        ConfigValue::Null
    }
    
    /// Check if this value is a string
    pub fn is_string(&self) -> bool {
        matches!(self, ConfigValue::String(_))
    }
    
    /// Check if this value is an integer
    pub fn is_integer(&self) -> bool {
        matches!(self, ConfigValue::Integer(_))
    }
    
    /// Check if this value is a boolean
    pub fn is_boolean(&self) -> bool {
        matches!(self, ConfigValue::Boolean(_))
    }
    
    /// Check if this value is an array
    pub fn is_array(&self) -> bool {
        matches!(self, ConfigValue::Array(_))
    }
    
    /// Check if this value is an object
    pub fn is_object(&self) -> bool {
        matches!(self, ConfigValue::Object(_))
    }
    
    /// Get string value if this is a string
    pub fn as_string(&self) -> Option<&str> {
        if let ConfigValue::String(s) = self {
            Some(s)
        } else {
            None
        }
    }
    
    /// Get integer value if this is an integer
    pub fn as_integer(&self) -> Option<i64> {
        if let ConfigValue::Integer(i) = self {
            Some(*i)
        } else {
            None
        }
    }
    
    /// Get boolean value if this is a boolean
    pub fn as_boolean(&self) -> Option<bool> {
        if let ConfigValue::Boolean(b) = self {
            Some(*b)
        } else {
            None
        }
    }
    
    /// Get array value if this is an array
    pub fn as_array(&self) -> Option<&Vec<ConfigValue>> {
        if let ConfigValue::Array(arr) = self {
            Some(arr)
        } else {
            None
        }
    }
    
    /// Get object value if this is an object
    pub fn as_object(&self) -> Option<&HashMap<String, ConfigValue>> {
        if let ConfigValue::Object(obj) = self {
            Some(obj)
        } else {
            None
        }
    }
    
    /// Get mutable object value if this is an object
    pub fn as_object_mut(&mut self) -> Option<&mut HashMap<String, ConfigValue>> {
        if let ConfigValue::Object(obj) = self {
            Some(obj)
        } else {
            None
        }
    }
    
    /// Merge this value with another value (other takes precedence)
    pub fn merge_with(&self, other: &ConfigValue) -> Result<ConfigValue, ConfigError> {
        match (self, other) {
            // If both are objects, merge recursively
            (ConfigValue::Object(base), ConfigValue::Object(override_vals)) => {
                let mut result = base.clone();
                
                for (key, value) in override_vals {
                    if let Some(existing) = base.get(key) {
                        // Recursively merge if both are objects
                        if existing.is_object() && value.is_object() {
                            result.insert(key.clone(), existing.merge_with(value)?);
                        } else {
                            // Override takes precedence
                            result.insert(key.clone(), value.clone());
                        }
                    } else {
                        // New key from override
                        result.insert(key.clone(), value.clone());
                    }
                }
                
                Ok(ConfigValue::Object(result))
            }
            // For non-object types, other takes precedence
            _ => Ok(other.clone()),
        }
    }
}

impl Default for ConfigValue {
    fn default() -> Self {
        ConfigValue::Null
    }
}

/// Configuration-specific error types
#[derive(Error, Debug, Clone, PartialEq)]
pub enum ConfigError {
    /// Configuration key not found
    #[error("Configuration not found: {0}")]
    NotFound(String),
    
    /// Validation failed
    #[error("Validation failed: {0}")]
    ValidationFailed(String),
    
    /// Serialization/deserialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),
    
    /// Invalid path format
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    
    /// Connection error (for remote stores)
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    
    /// Operation failed
    #[error("Operation failed: {0}")]
    OperationFailed(String),
    
    /// Version not found
    #[error("Version {0} not found for path {1}")]
    VersionNotFound(u32, String),
    
    /// Inheritance cycle detected
    #[error("Inheritance cycle detected: {0}")]
    InheritanceCycle(String),
    
    /// I/O error
    #[error("I/O error: {0}")]
    Io(String),
    
    /// Parse error
    #[error("Parse error: {0}")]
    Parse(String),
    
    /// Type mismatch error
    #[error("Type mismatch: expected {expected}, got {actual}")]
    TypeMismatch { expected: String, actual: String },
    
    /// Rate limit exceeded
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    /// Security violation
    #[error("Security violation: {0}")]
    SecurityViolation(String),
}

impl From<std::io::Error> for ConfigError {
    fn from(err: std::io::Error) -> Self {
        ConfigError::Io(err.to_string())
    }
}

/// Metadata associated with a configuration node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigMetadata {
    /// Human-readable description
    pub description: Option<String>,
    
    /// Owner/creator of this configuration
    pub owner: Option<String>,
    
    /// Whether this configuration contains sensitive data
    pub sensitive: bool,
    
    /// Whether this configuration can be modified at runtime
    pub runtime_modifiable: bool,
    
    /// When this configuration was created
    pub created_at: SystemTime,
    
    /// When this configuration was last updated
    pub updated_at: SystemTime,
    
    /// Who last updated this configuration
    pub updated_by: String,
}

impl ConfigMetadata {
    /// Create new metadata with current timestamp
    pub fn new(updated_by: String) -> Self {
        let now = SystemTime::now();
        Self {
            description: None,
            owner: None,
            sensitive: false,
            runtime_modifiable: true,
            created_at: now,
            updated_at: now,
            updated_by,
        }
    }
    
    /// Update the modification timestamp and user
    pub fn touch(&mut self, updated_by: String) {
        self.updated_at = SystemTime::now();
        self.updated_by = updated_by;
    }
}

/// Represents a single configuration node with full metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigNode {
    /// Hierarchical path for this configuration
    pub path: String,
    
    /// The configuration value
    pub value: ConfigValue,
    
    /// Version number (starts at 1)
    pub version: u32,
    
    /// Metadata about this configuration
    pub metadata: ConfigMetadata,
    
    /// Paths from which this configuration inherits
    pub inheritance: Option<Vec<String>>,
    
    /// Optional JSON schema for validation
    pub schema: Option<String>,
}

impl ConfigNode {
    /// Create a new configuration node
    pub fn new(path: String, value: ConfigValue, updated_by: String) -> Self {
        if !Self::validate_path(&path) {
            panic!("Invalid path format: {}", path);
        }
        
        Self {
            path,
            value,
            version: 1,
            metadata: ConfigMetadata::new(updated_by),
            inheritance: None,
            schema: None,
        }
    }
    
    /// Validate configuration path format
    pub fn validate_path(path: &str) -> bool {
        // Must start with /
        if !path.starts_with('/') {
            return false;
        }
        
        // Cannot be just root
        if path == "/" {
            return false;
        }
        
        // Cannot have double slashes
        if path.contains("//") {
            return false;
        }
        
        // Check depth (max 6 levels: /a/b/c/d/e/f)
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if parts.len() > 6 {
            return false;
        }
        
        // Each part should be valid (no empty parts, no special chars)
        for part in parts {
            if part.is_empty() || part.contains(' ') {
                return false;
            }
        }
        
        true
    }
    
    /// Create a new version of this node with updated value
    pub fn new_version(&self, value: ConfigValue, updated_by: String) -> Self {
        let mut node = self.clone();
        node.value = value;
        node.version += 1;
        node.metadata.touch(updated_by);
        node
    }
    
    /// Check if this node has inheritance relationships
    pub fn has_inheritance(&self) -> bool {
        self.inheritance.is_some() && !self.inheritance.as_ref().unwrap().is_empty()
    }
}

/// Version information for configuration history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigVersion {
    /// Version number
    pub version: u32,
    
    /// Configuration value at this version
    pub value: ConfigValue,
    
    /// When this version was created
    pub created_at: SystemTime,
    
    /// Who created this version
    pub created_by: String,
}

impl ConfigVersion {
    /// Create a new version entry
    pub fn new(version: u32, value: ConfigValue, created_by: String) -> Self {
        Self {
            version,
            value,
            created_at: SystemTime::now(),
            created_by,
        }
    }
}

/// A tree of configuration nodes organized hierarchically
pub type ConfigTree = HashMap<String, ConfigNode>;

/// Snapshot of configuration store state
pub type ConfigSnapshot = HashMap<String, ConfigNode>;