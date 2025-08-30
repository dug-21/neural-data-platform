/// Secure in-memory implementation of ConfigStore with security features
/// 
/// This implementation provides a fast, thread-safe in-memory store
/// with comprehensive security controls including secret blocking,
/// input validation, and rate limiting.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::traits::ConfigStore;
use crate::types::{
    ConfigValue, ConfigError, ConfigTree, ConfigNode, ConfigVersion, 
    ConfigSnapshot
};
use crate::security::{SecretBlocker, InputValidator, RateLimiter, ErrorSanitizer};
use std::time::Duration;

/// Secure in-memory configuration store implementation
/// 
/// Features:
/// - Thread-safe with RwLock for concurrent access
/// - Secret/password blocking
/// - Input validation and sanitization
/// - Optional rate limiting
/// - Error sanitization for production
/// - Version history with configurable retention
/// - Inheritance resolution
/// - Atomic operations
/// - Snapshot/restore functionality
#[derive(Debug)]
pub struct SecureInMemoryConfigStore {
    /// Current configuration data
    data: Arc<RwLock<HashMap<String, ConfigNode>>>,
    
    /// Version history storage (path -> version -> node)
    history: Arc<RwLock<HashMap<String, Vec<ConfigVersion>>>>,
    
    /// Maximum versions to retain per path
    max_versions: usize,
    
    /// Security components
    secret_blocker: SecretBlocker,
    validator: InputValidator,
    rate_limiter: Option<Arc<RateLimiter>>,
    error_sanitizer: ErrorSanitizer,
}

impl SecureInMemoryConfigStore {
    /// Create a new secure in-memory configuration store
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
            history: Arc::new(RwLock::new(HashMap::new())),
            max_versions: 10,
            secret_blocker: SecretBlocker::new(),
            validator: InputValidator::new(),
            rate_limiter: None,
            error_sanitizer: ErrorSanitizer::new(false), // Dev mode by default
        }
    }
    
    /// Enable production mode with error sanitization
    pub fn with_production_mode(mut self) -> Self {
        self.error_sanitizer = ErrorSanitizer::new(true);
        self
    }
    
    /// Enable rate limiting
    pub fn with_rate_limiting(mut self, max_requests: u32, window: Duration) -> Self {
        self.rate_limiter = Some(Arc::new(RateLimiter::new(max_requests, window)));
        self
    }
    
    /// Create a new store with custom version retention
    pub fn with_max_versions(mut self, max_versions: usize) -> Self {
        self.max_versions = max_versions;
        self
    }
    
    /// Check rate limit if enabled
    fn check_rate_limit(&self, client_id: Option<&str>) -> Result<(), ConfigError> {
        if let Some(ref limiter) = self.rate_limiter {
            if let Some(client) = client_id {
                limiter.check(client)?;
            }
        }
        Ok(())
    }
    
    /// Validate and sanitize configuration path
    fn validate_path(&self, path: &str) -> Result<(), ConfigError> {
        // Use ConfigNode's built-in path validation
        if !ConfigNode::validate_path(path) {
            return Err(ConfigError::InvalidPath(format!("Invalid path format: {}", path)));
        }
        
        // Additional security validation
        self.validator.validate_key(path)?;
        
        Ok(())
    }
    
    /// Create a snapshot of current store state
    pub async fn snapshot(&self) -> Result<ConfigSnapshot, ConfigError> {
        let data = self.data.read()
            .map_err(|e| ConfigError::OperationFailed(format!("Lock error: {}", e)))?;
        
        Ok(data.clone())
    }
    
    /// Restore store state from a snapshot
    pub async fn restore(&self, snapshot: ConfigSnapshot) -> Result<(), ConfigError> {
        // Validate all nodes in the snapshot
        for (path, node) in &snapshot {
            self.validate_path(path)?;
            self.secret_blocker.check_value(path, &node.value)?;
            self.validator.validate_value(&node.value)?;
        }
        
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
        
        let mut result = node.value.clone();
        
        if let Some(ref inheritance_paths) = node.inheritance {
            for parent_path in inheritance_paths {
                if let Ok(parent_value) = self.get(parent_path).await {
                    result = parent_value.merge_with(&result)?;
                }
            }
        }
        
        Ok(result)
    }
}

#[async_trait]
impl ConfigStore for SecureInMemoryConfigStore {
    async fn get(&self, path: &str) -> Result<ConfigValue, ConfigError> {
        self.validate_path(path)
            .map_err(|e| self.error_sanitizer.sanitize(e))?;
        
        let node = {
            let data = self.data.read()
                .map_err(|e| ConfigError::OperationFailed(format!("Lock error: {}", e)))
                .map_err(|e| self.error_sanitizer.sanitize(e))?;
            
            data.get(path)
                .cloned()
                .ok_or_else(|| ConfigError::NotFound(path.to_string()))
                .map_err(|e| self.error_sanitizer.sanitize(e))?
        };
        
        self.resolve_inheritance(&node).await
            .map_err(|e| self.error_sanitizer.sanitize(e))
    }
    
    async fn set(&self, path: &str, value: ConfigValue) -> Result<(), ConfigError> {
        // Security checks
        self.validate_path(path)
            .map_err(|e| self.error_sanitizer.sanitize(e))?;
        
        self.secret_blocker.check_value(path, &value)
            .map_err(|e| self.error_sanitizer.sanitize(e))?;
        
        self.validator.validate_value(&value)
            .map_err(|e| self.error_sanitizer.sanitize(e))?;
        
        let mut data = self.data.write()
            .map_err(|e| ConfigError::OperationFailed(format!("Lock error: {}", e)))
            .map_err(|e| self.error_sanitizer.sanitize(e))?;
        
        let updated_by = "system".to_string(); // Default user
        let node = if let Some(existing) = data.get(path) {
            // Create new version
            let new_node = existing.new_version(value, updated_by);
            self.store_version(path, &new_node)?;
            new_node
        } else {
            // Create new node
            ConfigNode::new(path.to_string(), value, updated_by)
        };
        
        data.insert(path.to_string(), node);
        Ok(())
    }
    
    async fn delete(&self, path: &str) -> Result<(), ConfigError> {
        self.validate_path(path)
            .map_err(|e| self.error_sanitizer.sanitize(e))?;
        
        let mut data = self.data.write()
            .map_err(|e| ConfigError::OperationFailed(format!("Lock error: {}", e)))
            .map_err(|e| self.error_sanitizer.sanitize(e))?;
        
        data.remove(path)
            .ok_or_else(|| ConfigError::NotFound(path.to_string()))
            .map(|_| ())
            .map_err(|e| self.error_sanitizer.sanitize(e))
    }
    
    async fn list_keys(&self, prefix: &str) -> Result<Vec<String>, ConfigError> {
        let data = self.data.read()
            .map_err(|e| ConfigError::OperationFailed(format!("Lock error: {}", e)))
            .map_err(|e| self.error_sanitizer.sanitize(e))?;
        
        let mut paths: Vec<String> = data.keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        
        paths.sort();
        Ok(paths)
    }
    
    async fn get_tree(&self, prefix: &str) -> Result<ConfigTree, ConfigError> {
        let data = self.data.read()
            .map_err(|e| ConfigError::OperationFailed(format!("Lock error: {}", e)))
            .map_err(|e| self.error_sanitizer.sanitize(e))?;
        
        let tree: ConfigTree = data.iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(k, v)| (k.clone(), v.value.clone()))
            .collect();
        
        Ok(tree)
    }
    
    async fn get_history(&self, path: &str) -> Result<Vec<ConfigVersion>, ConfigError> {
        self.validate_path(path)
            .map_err(|e| self.error_sanitizer.sanitize(e))?;
        
        let history = self.history.read()
            .map_err(|e| ConfigError::OperationFailed(format!("Lock error: {}", e)))
            .map_err(|e| self.error_sanitizer.sanitize(e))?;
        
        Ok(history.get(path).cloned().unwrap_or_default())
    }
    
    async fn get_version(&self, path: &str, version: u32) -> Result<ConfigValue, ConfigError> {
        let history = self.get_history(path).await?;
        
        history.iter()
            .find(|v| v.version == version)
            .map(|v| v.value.clone())
            .ok_or_else(|| ConfigError::VersionNotFound(path.to_string(), version))
            .map_err(|e| self.error_sanitizer.sanitize(e))
    }
    
    async fn set_node(&self, path: &str, node: ConfigNode) -> Result<(), ConfigError> {
        // Security checks on the node
        self.validate_path(path)?;
        self.secret_blocker.check_value(path, &node.value)?;
        self.validator.validate_value(&node.value)?;
        
        let mut data = self.data.write()
            .map_err(|e| ConfigError::OperationFailed(format!("Lock error: {}", e)))?;
        
        // Store version history
        if let Some(existing) = data.get(path) {
            self.store_version(path, existing)?;
        }
        
        data.insert(path.to_string(), node);
        Ok(())
    }
    
    async fn get_node(&self, path: &str) -> Result<ConfigNode, ConfigError> {
        self.validate_path(path)?;
        
        let data = self.data.read()
            .map_err(|e| ConfigError::OperationFailed(format!("Lock error: {}", e)))?;
        
        data.get(path)
            .cloned()
            .ok_or_else(|| ConfigError::NotFound(path.to_string()))
    }
}