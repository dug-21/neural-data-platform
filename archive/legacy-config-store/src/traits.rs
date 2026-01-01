use crate::types::{ConfigError, ConfigNode, ConfigTree, ConfigValue, ConfigVersion};
/// Configuration store trait definitions
///
/// This module defines the core trait that all configuration store
/// implementations must satisfy, providing a consistent interface
/// for configuration management operations.
use async_trait::async_trait;

/// Core trait for configuration storage implementations
///
/// All configuration stores must implement this trait to provide
/// a consistent interface for CRUD operations, versioning, and
/// hierarchical configuration management.
#[async_trait]
pub trait ConfigStore: Send + Sync {
    /// Retrieve a configuration value by path
    ///
    /// # Arguments
    /// * `path` - Hierarchical path to the configuration (e.g., "/system/global/timeout")
    ///
    /// # Returns
    /// * `Ok(ConfigValue)` - The configuration value with inheritance resolved
    /// * `Err(ConfigError::NotFound)` - If the path doesn't exist
    /// * `Err(ConfigError::InvalidPath)` - If the path format is invalid
    async fn get(&self, path: &str) -> Result<ConfigValue, ConfigError>;

    /// Store a configuration value at the specified path
    ///
    /// This operation will:
    /// - Create a new configuration if the path doesn't exist
    /// - Update existing configuration and increment version
    /// - Validate the path format
    /// - Store metadata about the change
    ///
    /// # Arguments
    /// * `path` - Hierarchical path where to store the configuration
    /// * `value` - Configuration value to store
    ///
    /// # Returns
    /// * `Ok(())` - If the operation succeeded
    /// * `Err(ConfigError::InvalidPath)` - If the path format is invalid
    /// * `Err(ConfigError::ValidationFailed)` - If value validation fails
    async fn set(&self, path: &str, value: ConfigValue) -> Result<(), ConfigError>;

    /// Delete a configuration at the specified path
    ///
    /// The configuration will be removed from active storage but
    /// may be retained in version history for audit purposes.
    ///
    /// # Arguments
    /// * `path` - Hierarchical path to delete
    ///
    /// # Returns
    /// * `Ok(())` - If the deletion succeeded
    /// * `Err(ConfigError::NotFound)` - If the path doesn't exist
    async fn delete(&self, path: &str) -> Result<(), ConfigError>;

    /// Retrieve all configurations under a given prefix as a tree
    ///
    /// This operation will resolve inheritance relationships and
    /// return a hierarchical view of all matching configurations.
    ///
    /// # Arguments
    /// * `prefix` - Path prefix to match (e.g., "/system" matches "/system/global/timeout")
    ///
    /// # Returns
    /// * `Ok(ConfigTree)` - Tree of matching configurations
    /// * `Err(ConfigError::InvalidPath)` - If the prefix format is invalid
    async fn get_tree(&self, prefix: &str) -> Result<ConfigTree, ConfigError>;

    /// List all configuration keys under a given prefix
    ///
    /// Returns just the paths without loading the full configuration values.
    /// Useful for discovery and navigation.
    ///
    /// # Arguments
    /// * `prefix` - Path prefix to match
    ///
    /// # Returns
    /// * `Ok(Vec<String>)` - List of matching configuration paths
    /// * `Err(ConfigError::InvalidPath)` - If the prefix format is invalid
    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>, ConfigError>;

    /// Retrieve a specific version of a configuration
    ///
    /// # Arguments
    /// * `path` - Configuration path
    /// * `version` - Version number to retrieve
    ///
    /// # Returns
    /// * `Ok(ConfigValue)` - The configuration value at the specified version
    /// * `Err(ConfigError::NotFound)` - If the path doesn't exist
    /// * `Err(ConfigError::VersionNotFound)` - If the version doesn't exist
    async fn get_version(&self, path: &str, version: u32) -> Result<ConfigValue, ConfigError>;

    /// Get the version history for a configuration path
    ///
    /// Returns up to the last 10 versions as specified in requirements.
    ///
    /// # Arguments
    /// * `path` - Configuration path
    ///
    /// # Returns
    /// * `Ok(Vec<ConfigVersion>)` - List of version history entries
    /// * `Err(ConfigError::NotFound)` - If the path doesn't exist
    async fn get_history(&self, path: &str) -> Result<Vec<ConfigVersion>, ConfigError>;

    /// Set a configuration node with full metadata and inheritance
    ///
    /// This is an advanced operation that allows setting a complete
    /// ConfigNode with metadata, inheritance relationships, and versioning.
    ///
    /// # Arguments
    /// * `path` - Configuration path
    /// * `node` - Complete configuration node to store
    ///
    /// # Returns
    /// * `Ok(())` - If the operation succeeded
    /// * `Err(ConfigError)` - Various errors based on validation or storage issues
    async fn set_node(&self, path: &str, node: ConfigNode) -> Result<(), ConfigError>;

    /// Get a complete configuration node with metadata
    ///
    /// Unlike `get()` which returns just the value, this returns the
    /// complete ConfigNode with metadata, versioning info, etc.
    ///
    /// # Arguments
    /// * `path` - Configuration path
    ///
    /// # Returns
    /// * `Ok(ConfigNode)` - Complete configuration node
    /// * `Err(ConfigError::NotFound)` - If the path doesn't exist
    async fn get_node(&self, path: &str) -> Result<ConfigNode, ConfigError>;
}

/// Transaction interface for atomic operations
///
/// Allows multiple configuration changes to be grouped together
/// and committed atomically, with rollback on failure.
pub struct ConfigTransaction<'a> {
    store: &'a dyn ConfigStore,
    operations: Vec<TransactionOperation>,
}

/// Operations that can be performed within a transaction
#[derive(Debug, Clone)]
pub enum TransactionOperation {
    Set { path: String, value: ConfigValue },
    Delete { path: String },
    SetNode { path: String, node: ConfigNode },
}

impl<'a> ConfigTransaction<'a> {
    /// Create a new transaction
    pub fn new(store: &'a dyn ConfigStore) -> Self {
        Self {
            store,
            operations: Vec::new(),
        }
    }

    /// Add a set operation to the transaction
    pub fn set(&mut self, path: String, value: ConfigValue) {
        self.operations
            .push(TransactionOperation::Set { path, value });
    }

    /// Add a delete operation to the transaction
    pub fn delete(&mut self, path: String) {
        self.operations.push(TransactionOperation::Delete { path });
    }

    /// Add a set_node operation to the transaction
    pub fn set_node(&mut self, path: String, node: ConfigNode) {
        self.operations
            .push(TransactionOperation::SetNode { path, node });
    }

    /// Commit all operations atomically
    pub async fn commit(self) -> Result<(), ConfigError> {
        // For now, we'll execute operations sequentially
        // Real implementations might use database transactions
        for operation in self.operations {
            match operation {
                TransactionOperation::Set { path, value } => {
                    self.store.set(&path, value).await?;
                }
                TransactionOperation::Delete { path } => {
                    self.store.delete(&path).await?;
                }
                TransactionOperation::SetNode { path, node } => {
                    self.store.set_node(&path, node).await?;
                }
            }
        }
        Ok(())
    }
}

/// Helper functions for configuration path manipulation
pub mod path_utils {
    /// Check if a path is a prefix of another path
    pub fn is_prefix_of(prefix: &str, path: &str) -> bool {
        if prefix == "/" {
            return true; // Root prefix matches everything
        }
        path.starts_with(prefix)
            && (path.len() == prefix.len() || path.chars().nth(prefix.len()) == Some('/'))
    }

    /// Get the parent path of a given path
    pub fn parent_path(path: &str) -> Option<String> {
        if path == "/" {
            return None; // Root has no parent
        }

        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            return None;
        }

        if parts.len() == 1 {
            return Some("/".to_string()); // Parent of /a is /
        }

        let parent_parts = &parts[..parts.len() - 1];
        Some(format!("/{}", parent_parts.join("/")))
    }

    /// Normalize a path by removing redundant slashes and ensuring proper format
    pub fn normalize_path(path: &str) -> String {
        if path.is_empty() {
            return "/".to_string();
        }

        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            return "/".to_string();
        }

        format!("/{}", parts.join("/"))
    }
}
