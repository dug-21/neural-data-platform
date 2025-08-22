/// In-memory implementation of ConfigStore
/// 
/// This implementation provides a fast, thread-safe in-memory store
/// suitable for testing, development, and single-instance deployments.
/// All data is stored in memory and will be lost on restart.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::traits::{ConfigStore, path_utils};
use crate::types::{
    ConfigValue, ConfigError, ConfigTree, ConfigNode, ConfigVersion, 
    ConfigSnapshot
};

/// In-memory configuration store implementation
/// 
/// Features:
/// - Thread-safe with RwLock for concurrent access
/// - Version history with configurable retention
/// - Inheritance resolution
/// - Atomic operations
/// - Snapshot/restore functionality
#[derive(Debug)]
pub struct InMemoryConfigStore {
    /// Current configuration data
    data: Arc<RwLock<HashMap<String, ConfigNode>>>,
    
    /// Version history storage (path -> version -> node)
    history: Arc<RwLock<HashMap<String, Vec<ConfigVersion>>>>,
    
    /// Maximum versions to retain per path
    max_versions: usize,
}

impl InMemoryConfigStore {
    /// Create a new in-memory configuration store
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
            history: Arc::new(RwLock::new(HashMap::new())),
            max_versions: 10, // As per specification
        }
    }
    
    /// Create a new store with custom version retention
    pub fn with_max_versions(max_versions: usize) -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
            history: Arc::new(RwLock::new(HashMap::new())),
            max_versions,
        }
    }
    
    /// Create a snapshot of current store state
    pub async fn snapshot(&self) -> Result<ConfigSnapshot, ConfigError> {
        let data = self.data.read()
            .map_err(|e| ConfigError::OperationFailed(format!("Lock error: {}", e)))?;
        
        Ok(data.clone())
    }
    
    /// Restore store state from a snapshot
    pub async fn restore(&self, snapshot: ConfigSnapshot) -> Result<(), ConfigError> {
        let mut data = self.data.write()
            .map_err(|e| ConfigError::OperationFailed(format!("Lock error: {}", e)))?;
        
        *data = snapshot;
        Ok(())
    }
    
    /// Store a version in history with retention management
    fn store_version(&self, path: &str, node: &ConfigNode) -> Result<(), ConfigError> {
        let mut history = self.history.write()
            .map_err(|e| ConfigError::OperationFailed(format!("Lock error: {}", e)))?;
        
        let version_entry = ConfigVersion::new(
            node.version,
            node.value.clone(),
            node.metadata.updated_by.clone(),
        );
        
        let versions = history.entry(path.to_string()).or_insert_with(Vec::new);
        versions.push(version_entry);
        
        // Maintain max_versions limit
        if versions.len() > self.max_versions {
            versions.drain(0..versions.len() - self.max_versions);
        }
        
        Ok(())
    }
    
    /// Resolve inheritance for a configuration node
    async fn resolve_inheritance(&self, node: &ConfigNode) -> Result<ConfigValue, ConfigError> {
        if !node.has_inheritance() {
            return Ok(node.value.clone());
        }
        
        let mut resolved_value = ConfigValue::new();
        
        // Apply inheritance in order
        for parent_path in node.inheritance.as_ref().unwrap() {
            // Check for inheritance cycles
            if parent_path == &node.path {
                return Err(ConfigError::InheritanceCycle(node.path.clone()));
            }
            
            let parent_config = self.get(parent_path).await?;
            resolved_value = resolved_value.merge_with(&parent_config)?;
        }
        
        // Apply the node's own value last (takes precedence)
        resolved_value = resolved_value.merge_with(&node.value)?;
        
        Ok(resolved_value)
    }
}

impl Default for InMemoryConfigStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ConfigStore for InMemoryConfigStore {
    async fn get(&self, path: &str) -> Result<ConfigValue, ConfigError> {
        // Validate path format
        if !ConfigNode::validate_path(path) {
            return Err(ConfigError::InvalidPath(path.to_string()));
        }
        
        let node = {
            let data = self.data.read()
                .map_err(|e| ConfigError::OperationFailed(format!("Lock error: {}", e)))?;
            
            data.get(path)
                .ok_or_else(|| ConfigError::NotFound(path.to_string()))?
                .clone()
        };
        
        self.resolve_inheritance(&node).await
    }
    
    async fn set(&self, path: &str, value: ConfigValue) -> Result<(), ConfigError> {
        // Validate path format
        if !ConfigNode::validate_path(path) {
            return Err(ConfigError::InvalidPath(path.to_string()));
        }
        
        let mut data = self.data.write()
            .map_err(|e| ConfigError::OperationFailed(format!("Lock error: {}", e)))?;
        
        let new_node = if let Some(existing) = data.get(path) {
            // Update existing node with new version
            existing.new_version(value, "system".to_string())
        } else {
            // Create new node
            ConfigNode::new(path.to_string(), value, "system".to_string())
        };
        
        // Store the old version in history if it exists, then update current
        if let Some(existing) = data.get(path).cloned() {
            data.insert(path.to_string(), new_node.clone());
            drop(data); // Release write lock before storing version
            self.store_version(path, &existing)?;
        } else {
            // For new entries, just insert (first version doesn't go to history until updated)
            data.insert(path.to_string(), new_node.clone());
        }
        
        Ok(())
    }
    
    async fn delete(&self, path: &str) -> Result<(), ConfigError> {
        let mut data = self.data.write()
            .map_err(|e| ConfigError::OperationFailed(format!("Lock error: {}", e)))?;
        
        let removed = data.remove(path)
            .ok_or_else(|| ConfigError::NotFound(path.to_string()))?;
        
        drop(data);
        // Store final version in history
        self.store_version(path, &removed)?;
        
        Ok(())
    }
    
    async fn get_tree(&self, prefix: &str) -> Result<ConfigTree, ConfigError> {
        let data = self.data.read()
            .map_err(|e| ConfigError::OperationFailed(format!("Lock error: {}", e)))?;
        
        let mut tree = ConfigTree::new();
        
        for (path, node) in data.iter() {
            if path_utils::is_prefix_of(prefix, path) {
                tree.insert(path.clone(), node.clone());
            }
        }
        
        Ok(tree)
    }
    
    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>, ConfigError> {
        let data = self.data.read()
            .map_err(|e| ConfigError::OperationFailed(format!("Lock error: {}", e)))?;
        
        let keys: Vec<String> = data.keys()
            .filter(|path| path_utils::is_prefix_of(prefix, path))
            .cloned()
            .collect();
        
        Ok(keys)
    }
    
    async fn get_version(&self, path: &str, version: u32) -> Result<ConfigValue, ConfigError> {
        let history = self.history.read()
            .map_err(|e| ConfigError::OperationFailed(format!("Lock error: {}", e)))?;
        
        let versions = history.get(path)
            .ok_or_else(|| ConfigError::NotFound(path.to_string()))?;
        
        let version_entry = versions.iter()
            .find(|v| v.version == version)
            .ok_or_else(|| ConfigError::VersionNotFound(version, path.to_string()))?;
        
        Ok(version_entry.value.clone())
    }
    
    async fn get_history(&self, path: &str) -> Result<Vec<ConfigVersion>, ConfigError> {
        let history = self.history.read()
            .map_err(|e| ConfigError::OperationFailed(format!("Lock error: {}", e)))?;
        
        let data = self.data.read()
            .map_err(|e| ConfigError::OperationFailed(format!("Lock error: {}", e)))?;
        
        let mut all_versions = Vec::new();
        
        // Add versions from history
        if let Some(versions) = history.get(path) {
            all_versions.extend(versions.clone());
        }
        
        // Add current version if it exists
        if let Some(current_node) = data.get(path) {
            let current_version = ConfigVersion::new(
                current_node.version,
                current_node.value.clone(),
                current_node.metadata.updated_by.clone(),
            );
            all_versions.push(current_version);
        }
        
        // Sort by version number
        all_versions.sort_by_key(|v| v.version);
        
        if all_versions.is_empty() {
            Err(ConfigError::NotFound(path.to_string()))
        } else {
            Ok(all_versions)
        }
    }
    
    async fn set_node(&self, path: &str, mut node: ConfigNode) -> Result<(), ConfigError> {
        // Validate path format
        if !ConfigNode::validate_path(path) {
            return Err(ConfigError::InvalidPath(path.to_string()));
        }
        
        // Ensure path consistency
        node.path = path.to_string();
        
        let mut data = self.data.write()
            .map_err(|e| ConfigError::OperationFailed(format!("Lock error: {}", e)))?;
        
        // Store old version in history if it exists
        if let Some(existing) = data.get(path).cloned() {
            // Update version number
            if existing.version >= node.version {
                node.version = existing.version + 1;
            }
            node.metadata.touch("system".to_string());
            
            data.insert(path.to_string(), node);
            drop(data); // Release write lock before storing version
            self.store_version(path, &existing)?;
        } else {
            data.insert(path.to_string(), node);
        }
        
        Ok(())
    }
    
    async fn get_node(&self, path: &str) -> Result<ConfigNode, ConfigError> {
        let data = self.data.read()
            .map_err(|e| ConfigError::OperationFailed(format!("Lock error: {}", e)))?;
        
        let node = data.get(path)
            .ok_or_else(|| ConfigError::NotFound(path.to_string()))?;
        
        Ok(node.clone())
    }
}

/// Thread-safe implementation marker
unsafe impl Send for InMemoryConfigStore {}
unsafe impl Sync for InMemoryConfigStore {}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_basic_operations() {
        let store = InMemoryConfigStore::new();
        
        // Test set and get
        let value = ConfigValue::String("test_value".to_string());
        store.set("/test/config", value.clone()).await.unwrap();
        
        let retrieved = store.get("/test/config").await.unwrap();
        assert_eq!(retrieved, value);
        
        // Test delete
        store.delete("/test/config").await.unwrap();
        
        let result = store.get("/test/config").await;
        assert!(result.is_err());
    }
    
    #[tokio::test]
    async fn test_versioning() {
        let store = InMemoryConfigStore::new();
        
        // Set initial version
        store.set("/test/version", ConfigValue::Integer(1)).await.unwrap();
        
        // Update to create version 2
        store.set("/test/version", ConfigValue::Integer(2)).await.unwrap();
        
        // Check current value
        let current = store.get("/test/version").await.unwrap();
        assert_eq!(current.as_integer().unwrap(), 2);
        
        // Check version history
        let history = store.get_history("/test/version").await.unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].version, 1);
        assert_eq!(history[1].version, 2);
        
        // Get specific version
        let v1 = store.get_version("/test/version", 1).await.unwrap();
        assert_eq!(v1.as_integer().unwrap(), 1);
    }
    
    #[tokio::test]
    async fn test_tree_operations() {
        let store = InMemoryConfigStore::new();
        
        // Set up hierarchical data
        store.set("/system/global", ConfigValue::Integer(1)).await.unwrap();
        store.set("/system/local", ConfigValue::Integer(2)).await.unwrap();
        store.set("/domain/trading", ConfigValue::Integer(3)).await.unwrap();
        
        // Get tree
        let tree = store.get_tree("/system").await.unwrap();
        assert_eq!(tree.len(), 2);
        assert!(tree.contains_key("/system/global"));
        assert!(tree.contains_key("/system/local"));
        assert!(!tree.contains_key("/domain/trading"));
        
        // List keys
        let keys = store.list_keys("/system").await.unwrap();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"/system/global".to_string()));
        assert!(keys.contains(&"/system/local".to_string()));
    }
}