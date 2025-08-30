//! Configuration Management Template
//! 
//! This template provides a hierarchical configuration management system
//! that enforces module isolation and follows the architecture's configuration
//! principles: declarative, versioned, validated, and hot-reloadable.
//! 
//! Key Features:
//! - Hierarchical configuration structure (/global, /domains, /modules)
//! - Schema validation with JSON Schema
//! - Hot-reload capabilities without restarts
//! - Environment-specific overrides
//! - Configuration versioning and migration
//! - Audit logging for configuration changes
//! - Module-specific namespacing

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{RwLock, watch};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use anyhow::{Result, anyhow};

use crate::templates::service_contracts::{ServiceDomain, ContractVersion};

/// Configuration hierarchy levels
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConfigLevel {
    Global,
    Domain(ServiceDomain),
    Module(String),
}

impl ConfigLevel {
    /// Get the file path for this configuration level
    pub fn path(&self, base_path: &Path) -> PathBuf {
        match self {
            ConfigLevel::Global => base_path.join("global"),
            ConfigLevel::Domain(domain) => base_path.join("domains").join(format!("{:?}", domain).to_lowercase()),
            ConfigLevel::Module(module) => base_path.join("modules").join(module),
        }
    }

    /// Get configuration precedence (higher number = higher precedence)
    pub fn precedence(&self) -> u8 {
        match self {
            ConfigLevel::Global => 1,
            ConfigLevel::Domain(_) => 2,
            ConfigLevel::Module(_) => 3,
        }
    }
}

/// Configuration entry with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigEntry {
    pub key: String,
    pub value: Value,
    pub level: ConfigLevel,
    pub version: ContractVersion,
    pub schema: Option<String>, // JSON Schema for validation
    pub description: String,
    pub required: bool,
    pub environment_override: bool,
    pub hot_reloadable: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: String,
}

impl ConfigEntry {
    pub fn new(
        key: String,
        value: Value,
        level: ConfigLevel,
        description: String,
    ) -> Self {
        Self {
            key,
            value,
            level,
            version: ContractVersion::new(1, 0, 0),
            schema: None,
            description,
            required: false,
            environment_override: true,
            hot_reloadable: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            created_by: "system".to_string(),
        }
    }

    pub fn with_schema(mut self, schema: String) -> Self {
        self.schema = Some(schema);
        self
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    pub fn not_hot_reloadable(mut self) -> Self {
        self.hot_reloadable = false;
        self
    }

    pub fn no_env_override(mut self) -> Self {
        self.environment_override = false;
        self
    }

    /// Validate the value against the schema if present
    pub fn validate(&self) -> Result<()> {
        if let Some(schema_str) = &self.schema {
            // Parse JSON Schema
            let schema: Value = serde_json::from_str(schema_str)?;
            
            // In a real implementation, use a JSON Schema validation library
            // For this template, we'll do basic validation
            self.basic_validate(&schema)?;
        }
        Ok(())
    }

    fn basic_validate(&self, schema: &Value) -> Result<()> {
        if let Some(schema_type) = schema.get("type").and_then(|t| t.as_str()) {
            match schema_type {
                "string" => {
                    if !self.value.is_string() {
                        return Err(anyhow!("Value must be a string"));
                    }
                }
                "number" => {
                    if !self.value.is_number() {
                        return Err(anyhow!("Value must be a number"));
                    }
                }
                "boolean" => {
                    if !self.value.is_boolean() {
                        return Err(anyhow!("Value must be a boolean"));
                    }
                }
                "object" => {
                    if !self.value.is_object() {
                        return Err(anyhow!("Value must be an object"));
                    }
                }
                "array" => {
                    if !self.value.is_array() {
                        return Err(anyhow!("Value must be an array"));
                    }
                }
                _ => {} // Unknown type, skip validation
            }
        }

        // Check required fields for objects
        if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
            if let Some(obj) = self.value.as_object() {
                for req_field in required {
                    if let Some(field_name) = req_field.as_str() {
                        if !obj.contains_key(field_name) {
                            return Err(anyhow!("Required field '{}' is missing", field_name));
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

/// Configuration change event for audit logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigChangeEvent {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub level: ConfigLevel,
    pub key: String,
    pub old_value: Option<Value>,
    pub new_value: Value,
    pub changed_by: String,
    pub reason: String,
    pub environment: String,
}

/// Configuration manager trait
#[async_trait]
pub trait ConfigurationManager: Send + Sync {
    /// Get a configuration value by key
    async fn get<T>(&self, key: &str) -> Result<Option<T>>
    where
        T: for<'de> Deserialize<'de>;

    /// Get a configuration value with a default
    async fn get_or_default<T>(&self, key: &str, default: T) -> Result<T>
    where
        T: for<'de> Deserialize<'de> + Serialize;

    /// Set a configuration value
    async fn set<T>(&self, level: ConfigLevel, key: &str, value: &T, changed_by: &str, reason: &str) -> Result<()>
    where
        T: Serialize + Send + Sync;

    /// Delete a configuration key
    async fn delete(&self, level: ConfigLevel, key: &str, changed_by: &str, reason: &str) -> Result<()>;

    /// Get all configuration entries for a level
    async fn get_level_config(&self, level: ConfigLevel) -> Result<HashMap<String, Value>>;

    /// Get configuration schema for a key
    async fn get_schema(&self, key: &str) -> Result<Option<String>>;

    /// Subscribe to configuration changes
    async fn subscribe(&self, key_pattern: &str) -> Result<watch::Receiver<ConfigChangeEvent>>;

    /// Validate all configurations
    async fn validate_all(&self) -> Result<Vec<String>>; // Returns validation errors

    /// Reload configuration from files
    async fn reload(&self) -> Result<()>;

    /// Export configuration to a specific format
    async fn export(&self, format: ConfigExportFormat) -> Result<String>;
}

/// Configuration export formats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigExportFormat {
    Json,
    Yaml,
    Toml,
    Env,
}

/// Hierarchical configuration manager implementation
pub struct HierarchicalConfigManager {
    base_path: PathBuf,
    environment: String,
    config_entries: Arc<RwLock<HashMap<String, ConfigEntry>>>,
    change_notifier: Arc<RwLock<HashMap<String, watch::Sender<ConfigChangeEvent>>>>,
    audit_log: Arc<RwLock<Vec<ConfigChangeEvent>>>,
}

impl HierarchicalConfigManager {
    /// Create a new configuration manager
    pub async fn new(base_path: PathBuf, environment: String) -> Result<Self> {
        let manager = Self {
            base_path,
            environment,
            config_entries: Arc::new(RwLock::new(HashMap::new())),
            change_notifier: Arc::new(RwLock::new(HashMap::new())),
            audit_log: Arc::new(RwLock::new(Vec::new())),
        };

        // Load initial configuration
        manager.load_all_configs().await?;

        Ok(manager)
    }

    /// Load all configuration files from the hierarchy
    async fn load_all_configs(&self) -> Result<()> {
        // Load global configurations
        self.load_level_config(ConfigLevel::Global).await?;

        // Load domain configurations
        for domain in [
            ServiceDomain::DataIngestion,
            ServiceDomain::CoreDataPlatform,
            ServiceDomain::TradingDecision,
            ServiceDomain::TradingExecution,
            ServiceDomain::SystemOpsDecision,
            ServiceDomain::SystemOpsExecution,
            ServiceDomain::Observability,
            ServiceDomain::Configuration,
        ] {
            self.load_level_config(ConfigLevel::Domain(domain)).await.ok(); // OK if not exists
        }

        // Load module configurations
        let modules_path = self.base_path.join("modules");
        if modules_path.exists() {
            for entry in fs::read_dir(&modules_path)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    if let Some(module_name) = entry.file_name().to_str() {
                        self.load_level_config(ConfigLevel::Module(module_name.to_string()))
                            .await.ok(); // OK if not exists
                    }
                }
            }
        }

        Ok(())
    }

    /// Load configuration for a specific level
    async fn load_level_config(&self, level: ConfigLevel) -> Result<()> {
        let level_path = level.path(&self.base_path);
        
        if !level_path.exists() {
            return Ok(()); // Level doesn't exist, which is OK
        }

        // Load YAML files in the level directory
        for entry in fs::read_dir(&level_path)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("yaml") ||
               path.extension().and_then(|s| s.to_str()) == Some("yml") {
                self.load_config_file(&path, &level).await?;
            }
        }

        Ok(())
    }

    /// Load a specific configuration file
    async fn load_config_file(&self, path: &Path, level: &ConfigLevel) -> Result<()> {
        let content = fs::read_to_string(path)?;
        let config_data: Value = serde_yaml::from_str(&content)?;

        if let Some(obj) = config_data.as_object() {
            for (key, value) in obj {
                let entry = ConfigEntry {
                    key: key.clone(),
                    value: value.clone(),
                    level: level.clone(),
                    version: ContractVersion::new(1, 0, 0),
                    schema: None,
                    description: format!("Loaded from {}", path.display()),
                    required: false,
                    environment_override: true,
                    hot_reloadable: true,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                    created_by: "file-loader".to_string(),
                };

                // Validate the entry
                entry.validate()?;

                // Store the entry
                let mut entries = self.config_entries.write().await;
                entries.insert(self.make_key(level, key), entry);
            }
        }

        Ok(())
    }

    /// Resolve configuration value considering hierarchy and environment overrides
    async fn resolve_value(&self, key: &str) -> Option<Value> {
        let entries = self.config_entries.read().await;
        let mut candidates: Vec<&ConfigEntry> = Vec::new();

        // Collect all entries for this key across levels
        for (entry_key, entry) in entries.iter() {
            if entry_key.ends_with(&format!(":{}", key)) {
                candidates.push(entry);
            }
        }

        // Sort by precedence (module > domain > global)
        candidates.sort_by(|a, b| b.level.precedence().cmp(&a.level.precedence()));

        // Check environment overrides first
        if let Some(env_value) = self.get_env_override(key) {
            if let Some(top_entry) = candidates.first() {
                if top_entry.environment_override {
                    return Some(env_value);
                }
            }
        }

        // Return the highest precedence value
        candidates.first().map(|entry| entry.value.clone())
    }

    /// Get environment variable override
    fn get_env_override(&self, key: &str) -> Option<Value> {
        let env_key = format!("NT_{}", key.to_uppercase().replace('.', "_"));
        if let Ok(env_value) = std::env::var(&env_key) {
            // Try to parse as JSON first, then as string
            if let Ok(json_value) = serde_json::from_str::<Value>(&env_value) {
                Some(json_value)
            } else {
                Some(Value::String(env_value))
            }
        } else {
            None
        }
    }

    /// Make a composite key for storage
    fn make_key(&self, level: &ConfigLevel, key: &str) -> String {
        match level {
            ConfigLevel::Global => format!("global:{}", key),
            ConfigLevel::Domain(domain) => format!("domain:{:?}:{}", domain, key),
            ConfigLevel::Module(module) => format!("module:{}:{}", module, key),
        }
    }

    /// Parse composite key
    fn parse_key(&self, composite_key: &str) -> Option<(ConfigLevel, String)> {
        let parts: Vec<&str> = composite_key.splitn(3, ':').collect();
        match parts.as_slice() {
            ["global", key] => Some((ConfigLevel::Global, key.to_string())),
            ["domain", domain_str, key] => {
                if let Ok(domain) = serde_json::from_str::<ServiceDomain>(&format!("\"{}\"", domain_str)) {
                    Some((ConfigLevel::Domain(domain), key.to_string()))
                } else {
                    None
                }
            }
            ["module", module, key] => Some((ConfigLevel::Module(module.to_string()), key.to_string())),
            _ => None,
        }
    }

    /// Record configuration change
    async fn record_change(
        &self,
        level: ConfigLevel,
        key: String,
        old_value: Option<Value>,
        new_value: Value,
        changed_by: String,
        reason: String,
    ) {
        let change_event = ConfigChangeEvent {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            level,
            key: key.clone(),
            old_value,
            new_value,
            changed_by,
            reason,
            environment: self.environment.clone(),
        };

        // Log to audit trail
        let mut audit_log = self.audit_log.write().await;
        audit_log.push(change_event.clone());

        // Notify subscribers
        let notifiers = self.change_notifier.read().await;
        for (pattern, sender) in notifiers.iter() {
            if self.key_matches_pattern(&key, pattern) {
                let _ = sender.send(change_event.clone());
            }
        }
    }

    /// Check if a key matches a pattern
    fn key_matches_pattern(&self, key: &str, pattern: &str) -> bool {
        if pattern == "*" {
            return true;
        }
        
        if pattern.ends_with("*") {
            key.starts_with(&pattern[..pattern.len()-1])
        } else {
            key == pattern
        }
    }
}

#[async_trait]
impl ConfigurationManager for HierarchicalConfigManager {
    async fn get<T>(&self, key: &str) -> Result<Option<T>>
    where
        T: for<'de> Deserialize<'de>,
    {
        if let Some(value) = self.resolve_value(key).await {
            let result: T = serde_json::from_value(value)?;
            Ok(Some(result))
        } else {
            Ok(None)
        }
    }

    async fn get_or_default<T>(&self, key: &str, default: T) -> Result<T>
    where
        T: for<'de> Deserialize<'de> + Serialize,
    {
        if let Some(value) = self.get(key).await? {
            Ok(value)
        } else {
            Ok(default)
        }
    }

    async fn set<T>(&self, level: ConfigLevel, key: &str, value: &T, changed_by: &str, reason: &str) -> Result<()>
    where
        T: Serialize + Send + Sync,
    {
        let new_value = serde_json::to_value(value)?;
        let composite_key = self.make_key(&level, key);

        // Get old value for audit
        let old_value = {
            let entries = self.config_entries.read().await;
            entries.get(&composite_key).map(|entry| entry.value.clone())
        };

        // Create new entry
        let entry = ConfigEntry {
            key: key.to_string(),
            value: new_value.clone(),
            level: level.clone(),
            version: ContractVersion::new(1, 0, 0),
            schema: None,
            description: format!("Set by {}", changed_by),
            required: false,
            environment_override: true,
            hot_reloadable: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            created_by: changed_by.to_string(),
        };

        // Validate the entry
        entry.validate()?;

        // Store the entry
        {
            let mut entries = self.config_entries.write().await;
            entries.insert(composite_key, entry);
        }

        // Record the change
        self.record_change(
            level,
            key.to_string(),
            old_value,
            new_value,
            changed_by.to_string(),
            reason.to_string(),
        ).await;

        Ok(())
    }

    async fn delete(&self, level: ConfigLevel, key: &str, changed_by: &str, reason: &str) -> Result<()> {
        let composite_key = self.make_key(&level, key);

        let old_value = {
            let mut entries = self.config_entries.write().await;
            entries.remove(&composite_key).map(|entry| entry.value)
        };

        if let Some(old_val) = old_value {
            self.record_change(
                level,
                key.to_string(),
                Some(old_val),
                Value::Null,
                changed_by.to_string(),
                reason.to_string(),
            ).await;
        }

        Ok(())
    }

    async fn get_level_config(&self, level: ConfigLevel) -> Result<HashMap<String, Value>> {
        let entries = self.config_entries.read().await;
        let mut result = HashMap::new();

        for (composite_key, entry) in entries.iter() {
            if entry.level == level {
                result.insert(entry.key.clone(), entry.value.clone());
            }
        }

        Ok(result)
    }

    async fn get_schema(&self, key: &str) -> Result<Option<String>> {
        if let Some(value) = self.resolve_value(key).await {
            let entries = self.config_entries.read().await;
            for (composite_key, entry) in entries.iter() {
                if composite_key.ends_with(&format!(":{}", key)) && entry.value == value {
                    return Ok(entry.schema.clone());
                }
            }
        }
        Ok(None)
    }

    async fn subscribe(&self, key_pattern: &str) -> Result<watch::Receiver<ConfigChangeEvent>> {
        let (tx, rx) = watch::channel(ConfigChangeEvent {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            level: ConfigLevel::Global,
            key: "initial".to_string(),
            old_value: None,
            new_value: Value::Null,
            changed_by: "system".to_string(),
            reason: "initial".to_string(),
            environment: self.environment.clone(),
        });

        let mut notifiers = self.change_notifier.write().await;
        notifiers.insert(key_pattern.to_string(), tx);

        Ok(rx)
    }

    async fn validate_all(&self) -> Result<Vec<String>> {
        let entries = self.config_entries.read().await;
        let mut errors = Vec::new();

        for (composite_key, entry) in entries.iter() {
            if let Err(e) = entry.validate() {
                errors.push(format!("Validation error for {}: {}", composite_key, e));
            }
        }

        Ok(errors)
    }

    async fn reload(&self) -> Result<()> {
        // Clear current entries
        {
            let mut entries = self.config_entries.write().await;
            entries.clear();
        }

        // Reload from files
        self.load_all_configs().await?;

        Ok(())
    }

    async fn export(&self, format: ConfigExportFormat) -> Result<String> {
        let entries = self.config_entries.read().await;
        let mut export_data = HashMap::new();

        // Group by level
        for (_composite_key, entry) in entries.iter() {
            let level_key = match &entry.level {
                ConfigLevel::Global => "global".to_string(),
                ConfigLevel::Domain(domain) => format!("domains.{:?}", domain).to_lowercase(),
                ConfigLevel::Module(module) => format!("modules.{}", module),
            };

            let level_map = export_data
                .entry(level_key)
                .or_insert_with(|| serde_json::Map::new());

            level_map.insert(entry.key.clone(), entry.value.clone());
        }

        match format {
            ConfigExportFormat::Json => {
                Ok(serde_json::to_string_pretty(&export_data)?)
            }
            ConfigExportFormat::Yaml => {
                Ok(serde_yaml::to_string(&export_data)?)
            }
            ConfigExportFormat::Toml => {
                // Convert to TOML-compatible format
                let toml_value = toml::Value::try_from(&export_data)?;
                Ok(toml::to_string_pretty(&toml_value)?)
            }
            ConfigExportFormat::Env => {
                let mut env_vars = Vec::new();
                for (_level_key, level_data) in export_data {
                    if let Value::Object(obj) = level_data {
                        for (key, value) in obj {
                            let env_key = format!("NT_{}", key.to_uppercase().replace('.', "_"));
                            let env_value = match value {
                                Value::String(s) => s,
                                _ => serde_json::to_string(&value)?,
                            };
                            env_vars.push(format!("{}={}", env_key, env_value));
                        }
                    }
                }
                Ok(env_vars.join("\n"))
            }
        }
    }
}

/// Predefined configuration templates for common service types

/// Create default global configuration
pub fn create_global_config() -> HashMap<String, ConfigEntry> {
    let mut config = HashMap::new();

    config.insert(
        "global:platform.name".to_string(),
        ConfigEntry::new(
            "platform.name".to_string(),
            Value::String("Neural Time Series Platform".to_string()),
            ConfigLevel::Global,
            "Platform name identifier".to_string(),
        ).required(),
    );

    config.insert(
        "global:platform.version".to_string(),
        ConfigEntry::new(
            "platform.version".to_string(),
            Value::String("1.0.0".to_string()),
            ConfigLevel::Global,
            "Platform version".to_string(),
        ).required(),
    );

    config.insert(
        "global:redis.url".to_string(),
        ConfigEntry::new(
            "redis.url".to_string(),
            Value::String("redis://localhost:6379".to_string()),
            ConfigLevel::Global,
            "Redis connection URL".to_string(),
        ).with_schema(r#"{"type": "string", "pattern": "^redis://.*"}"#.to_string()),
    );

    config.insert(
        "global:observability.metrics_enabled".to_string(),
        ConfigEntry::new(
            "observability.metrics_enabled".to_string(),
            Value::Bool(true),
            ConfigLevel::Global,
            "Enable metrics collection".to_string(),
        ).with_schema(r#"{"type": "boolean"}"#.to_string()),
    );

    config
}

/// Create trading domain configuration
pub fn create_trading_domain_config() -> HashMap<String, ConfigEntry> {
    let mut config = HashMap::new();

    config.insert(
        "domain:TradingDecision:strategies".to_string(),
        ConfigEntry::new(
            "strategies".to_string(),
            Value::Array(vec![
                Value::String("momentum".to_string()),
                Value::String("mean_reversion".to_string()),
                Value::String("neural_prediction".to_string()),
            ]),
            ConfigLevel::Domain(ServiceDomain::TradingDecision),
            "Available trading strategies".to_string(),
        ).with_schema(r#"{"type": "array", "items": {"type": "string"}}"#.to_string()),
    );

    config.insert(
        "domain:TradingDecision:voting.consensus_threshold".to_string(),
        ConfigEntry::new(
            "voting.consensus_threshold".to_string(),
            Value::Number(serde_json::Number::from_f64(0.6).unwrap()),
            ConfigLevel::Domain(ServiceDomain::TradingDecision),
            "Minimum confidence for consensus".to_string(),
        ).with_schema(r#"{"type": "number", "minimum": 0.0, "maximum": 1.0}"#.to_string()),
    );

    config.insert(
        "domain:TradingExecution:risk.max_position_size".to_string(),
        ConfigEntry::new(
            "risk.max_position_size".to_string(),
            Value::Number(serde_json::Number::from(10000)),
            ConfigLevel::Domain(ServiceDomain::TradingExecution),
            "Maximum position size in dollars".to_string(),
        ).with_schema(r#"{"type": "number", "minimum": 0}"#.to_string()),
    );

    config
}

/// Create module-specific configuration
pub fn create_module_config(module_name: &str, domain: ServiceDomain) -> HashMap<String, ConfigEntry> {
    let mut config = HashMap::new();

    config.insert(
        format!("module:{}:worker_threads", module_name),
        ConfigEntry::new(
            "worker_threads".to_string(),
            Value::Number(serde_json::Number::from(4)),
            ConfigLevel::Module(module_name.to_string()),
            "Number of worker threads".to_string(),
        ).with_schema(r#"{"type": "number", "minimum": 1, "maximum": 32}"#.to_string()),
    );

    config.insert(
        format!("module:{}:memory_limit_mb", module_name),
        ConfigEntry::new(
            "memory_limit_mb".to_string(),
            Value::Number(serde_json::Number::from(512)),
            ConfigLevel::Module(module_name.to_string()),
            "Memory limit in megabytes".to_string(),
        ).with_schema(r#"{"type": "number", "minimum": 64}"#.to_string()),
    );

    // Domain-specific settings
    match domain {
        ServiceDomain::DataIngestion => {
            config.insert(
                format!("module:{}:batch_size", module_name),
                ConfigEntry::new(
                    "batch_size".to_string(),
                    Value::Number(serde_json::Number::from(1000)),
                    ConfigLevel::Module(module_name.to_string()),
                    "Batch size for data processing".to_string(),
                ).with_schema(r#"{"type": "number", "minimum": 1}"#.to_string()),
            );
        }
        ServiceDomain::TradingDecision => {
            config.insert(
                format!("module:{}:decision_timeout_ms", module_name),
                ConfigEntry::new(
                    "decision_timeout_ms".to_string(),
                    Value::Number(serde_json::Number::from(5000)),
                    ConfigLevel::Module(module_name.to_string()),
                    "Decision timeout in milliseconds".to_string(),
                ).with_schema(r#"{"type": "number", "minimum": 100}"#.to_string()),
            );
        }
        _ => {}
    }

    config
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_config_level_precedence() {
        assert!(ConfigLevel::Module("test".to_string()).precedence() > ConfigLevel::Global.precedence());
        assert!(ConfigLevel::Domain(ServiceDomain::TradingDecision).precedence() > ConfigLevel::Global.precedence());
        assert!(ConfigLevel::Module("test".to_string()).precedence() > 
                ConfigLevel::Domain(ServiceDomain::TradingDecision).precedence());
    }

    #[test]
    fn test_config_entry_validation() {
        let entry = ConfigEntry::new(
            "test_string".to_string(),
            Value::String("test".to_string()),
            ConfigLevel::Global,
            "Test string value".to_string(),
        ).with_schema(r#"{"type": "string"}"#.to_string());

        assert!(entry.validate().is_ok());

        let invalid_entry = ConfigEntry::new(
            "test_number".to_string(),
            Value::String("not_a_number".to_string()),
            ConfigLevel::Global,
            "Test number value".to_string(),
        ).with_schema(r#"{"type": "number"}"#.to_string());

        assert!(invalid_entry.validate().is_err());
    }

    #[tokio::test]
    async fn test_hierarchical_config_manager() {
        let temp_dir = TempDir::new().unwrap();
        let base_path = temp_dir.path().to_path_buf();

        // Create directory structure
        std::fs::create_dir_all(base_path.join("global")).unwrap();
        std::fs::create_dir_all(base_path.join("domains/trading")).unwrap();
        std::fs::create_dir_all(base_path.join("modules/test-module")).unwrap();

        // Create test config files
        std::fs::write(
            base_path.join("global/platform.yaml"),
            "name: \"Test Platform\"\nversion: \"1.0.0\"\n",
        ).unwrap();

        std::fs::write(
            base_path.join("domains/trading/strategies.yaml"),
            "enabled: [\"momentum\", \"mean_reversion\"]\n",
        ).unwrap();

        std::fs::write(
            base_path.join("modules/test-module/config.yaml"),
            "worker_threads: 8\nmemory_limit_mb: 1024\n",
        ).unwrap();

        let manager = HierarchicalConfigManager::new(base_path, "test".to_string())
            .await
            .unwrap();

        // Test value resolution with hierarchy
        let platform_name: Option<String> = manager.get("name").await.unwrap();
        assert_eq!(platform_name, Some("Test Platform".to_string()));

        let worker_threads: Option<i32> = manager.get("worker_threads").await.unwrap();
        assert_eq!(worker_threads, Some(8)); // Module level overrides

        // Test setting values
        manager.set(
            ConfigLevel::Global,
            "new_setting",
            &"test_value",
            "test_user",
            "testing",
        ).await.unwrap();

        let new_setting: Option<String> = manager.get("new_setting").await.unwrap();
        assert_eq!(new_setting, Some("test_value".to_string()));
    }

    #[tokio::test]
    async fn test_config_subscription() {
        let temp_dir = TempDir::new().unwrap();
        let manager = HierarchicalConfigManager::new(
            temp_dir.path().to_path_buf(),
            "test".to_string(),
        ).await.unwrap();

        let mut receiver = manager.subscribe("test.*").await.unwrap();

        // Set a value that matches the pattern
        manager.set(
            ConfigLevel::Global,
            "test.setting",
            &"value",
            "test_user",
            "testing",
        ).await.unwrap();

        // Should receive a change notification
        if let Ok(change) = receiver.changed().await {
            let change_event = receiver.borrow().clone();
            assert_eq!(change_event.key, "test.setting");
        }
    }

    #[test]
    fn test_template_configs() {
        let global_config = create_global_config();
        assert!(global_config.contains_key("global:platform.name"));

        let trading_config = create_trading_domain_config();
        assert!(trading_config.contains_key("domain:TradingDecision:strategies"));

        let module_config = create_module_config("test-module", ServiceDomain::DataIngestion);
        assert!(module_config.contains_key("module:test-module:worker_threads"));
        assert!(module_config.contains_key("module:test-module:batch_size"));
    }

    #[tokio::test]
    async fn test_environment_override() {
        std::env::set_var("NT_TEST_SETTING", "env_value");

        let temp_dir = TempDir::new().unwrap();
        let manager = HierarchicalConfigManager::new(
            temp_dir.path().to_path_buf(),
            "test".to_string(),
        ).await.unwrap();

        manager.set(
            ConfigLevel::Global,
            "test_setting",
            &"config_value",
            "test_user",
            "testing",
        ).await.unwrap();

        let value: Option<String> = manager.get("test_setting").await.unwrap();
        assert_eq!(value, Some("env_value".to_string())); // Environment override

        std::env::remove_var("NT_TEST_SETTING");
    }
}