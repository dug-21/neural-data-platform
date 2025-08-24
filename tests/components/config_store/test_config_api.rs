//! Configuration Management API Tests
//!
//! Comprehensive independent tests for Config Store configuration management,
//! covering all core API operations with full isolation.

use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex};
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde_json::json;

/// Mock Config Store implementation for isolated testing
#[derive(Debug, Clone)]
pub struct MockConfigStore {
    storage: std::sync::Arc<Mutex<HashMap<String, StoredConfig>>>,
    watchers: std::sync::Arc<Mutex<Vec<ConfigWatcher>>>,
    schemas: std::sync::Arc<Mutex<HashMap<String, ConfigSchema>>>,
    audit_trail: std::sync::Arc<Mutex<Vec<AuditEntry>>>,
}

#[derive(Debug, Clone)]
struct StoredConfig {
    namespace_path: String,
    key: String,
    value: ConfigValue,
    metadata: ConfigMetadata,
    version: String,
}

#[derive(Debug, Clone)]
struct ConfigWatcher {
    namespace_path: String,
    keys: Vec<String>,
    sender: mpsc::UnboundedSender<ConfigChangeEvent>,
}

#[derive(Debug, Clone)]
pub struct ConfigValue {
    pub value_type: ValueType,
    pub data: ConfigData,
}

#[derive(Debug, Clone)]
pub enum ConfigData {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Json(serde_json::Value),
    Binary(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ValueType {
    String,
    Int,
    Float,
    Bool,
    Json,
    Binary,
}

#[derive(Debug, Clone)]
pub struct ConfigMetadata {
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub created_by: String,
    pub modified_by: String,
    pub version: String,
    pub tags: Vec<String>,
    pub annotations: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct ConfigChangeEvent {
    pub namespace_path: String,
    pub key: String,
    pub change_type: ChangeType,
    pub old_value: Option<ConfigValue>,
    pub new_value: Option<ConfigValue>,
    pub timestamp: DateTime<Utc>,
    pub change_reason: String,
    pub changed_by: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ChangeType {
    Created,
    Updated,
    Deleted,
}

#[derive(Debug, Clone)]
struct ConfigSchema {
    namespace_path: String,
    schema_version: String,
    json_schema: String,
}

#[derive(Debug, Clone)]
struct AuditEntry {
    timestamp: DateTime<Utc>,
    namespace_path: String,
    key: String,
    change_type: ChangeType,
    old_value: Option<ConfigValue>,
    new_value: Option<ConfigValue>,
    changed_by: String,
    change_reason: String,
    version: String,
    session_id: String,
}

impl MockConfigStore {
    pub fn new() -> Self {
        Self {
            storage: std::sync::Arc::new(Mutex::new(HashMap::new())),
            watchers: std::sync::Arc::new(Mutex::new(Vec::new())),
            schemas: std::sync::Arc::new(Mutex::new(HashMap::new())),
            audit_trail: std::sync::Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Get configuration value with optional versioning
    pub async fn get_config(
        &self,
        namespace_path: &str,
        key: &str,
        version: Option<&str>,
        context: Option<HashMap<String, String>>,
        include_metadata: bool,
    ) -> Result<Option<GetConfigResponse>> {
        let storage = self.storage.lock().await;
        let storage_key = format!("{}#{}", namespace_path, key);

        if let Some(stored) = storage.get(&storage_key) {
            // Version filtering (simplified)
            if let Some(req_version) = version {
                if stored.version != req_version {
                    return Ok(None);
                }
            }

            // Context filtering (simplified - in real impl would be more complex)
            if let Some(_ctx) = context {
                // In a real implementation, this would filter based on environment context
            }

            Ok(Some(GetConfigResponse {
                success: true,
                namespace_path: stored.namespace_path.clone(),
                key: stored.key.clone(),
                version: stored.version.clone(),
                value: Some(stored.value.clone()),
                metadata: if include_metadata { Some(stored.metadata.clone()) } else { None },
                error_message: None,
            }))
        } else {
            Ok(None)
        }
    }

    /// Get multiple configuration values in bulk
    pub async fn get_bulk_config(
        &self,
        namespace_path: &str,
        keys: Vec<String>,
        version: Option<&str>,
        context: Option<HashMap<String, String>>,
        include_metadata: bool,
    ) -> Result<GetBulkConfigResponse> {
        let mut values = HashMap::new();
        let mut metadata = HashMap::new();
        let mut missing_keys = Vec::new();

        for key in &keys {
            match self.get_config(namespace_path, key, version, context.clone(), include_metadata).await? {
                Some(response) => {
                    if let Some(value) = response.value {
                        values.insert(key.clone(), value);
                    }
                    if let Some(meta) = response.metadata {
                        metadata.insert(key.clone(), meta);
                    }
                }
                None => missing_keys.push(key.clone()),
            }
        }

        Ok(GetBulkConfigResponse {
            success: true,
            namespace_path: namespace_path.to_string(),
            version: version.unwrap_or("latest").to_string(),
            values,
            metadata: if include_metadata { Some(metadata) } else { None },
            missing_keys,
            error_message: None,
        })
    }

    /// Set configuration value with validation and versioning
    pub async fn set_config(
        &self,
        namespace_path: &str,
        key: &str,
        value: ConfigValue,
        change_reason: &str,
        validate_only: bool,
        expected_version: Option<&str>,
        changed_by: &str,
    ) -> Result<SetConfigResponse> {
        // Optimistic concurrency control
        let storage_key = format!("{}#{}", namespace_path, key);
        let mut storage = self.storage.lock().await;

        if let Some(existing) = storage.get(&storage_key) {
            if let Some(exp_version) = expected_version {
                if existing.version != exp_version {
                    return Ok(SetConfigResponse {
                        success: false,
                        namespace_path: namespace_path.to_string(),
                        key: key.to_string(),
                        new_version: None,
                        validation_errors: Vec::new(),
                        error_message: Some("Version conflict".to_string()),
                    });
                }
            }
        }

        // Schema validation (simplified)
        let validation_errors = self.validate_against_schema(namespace_path, key, &value).await?;
        if !validation_errors.is_empty() {
            return Ok(SetConfigResponse {
                success: false,
                namespace_path: namespace_path.to_string(),
                key: key.to_string(),
                new_version: None,
                validation_errors,
                error_message: Some("Schema validation failed".to_string()),
            });
        }

        if validate_only {
            return Ok(SetConfigResponse {
                success: true,
                namespace_path: namespace_path.to_string(),
                key: key.to_string(),
                new_version: None,
                validation_errors: Vec::new(),
                error_message: None,
            });
        }

        // Generate new version
        let new_version = format!("v{}", Utc::now().timestamp());
        let now = Utc::now();

        let old_value = storage.get(&storage_key).map(|s| s.value.clone());

        let metadata = ConfigMetadata {
            created_at: old_value.as_ref().map(|_| {
                storage.get(&storage_key).unwrap().metadata.created_at
            }).unwrap_or(now),
            modified_at: now,
            created_by: old_value.as_ref().map(|_| {
                storage.get(&storage_key).unwrap().metadata.created_by.clone()
            }).unwrap_or_else(|| changed_by.to_string()),
            modified_by: changed_by.to_string(),
            version: new_version.clone(),
            tags: Vec::new(),
            annotations: HashMap::new(),
        };

        let stored_config = StoredConfig {
            namespace_path: namespace_path.to_string(),
            key: key.to_string(),
            value: value.clone(),
            metadata,
            version: new_version.clone(),
        };

        storage.insert(storage_key, stored_config);

        // Record audit trail
        let change_type = if old_value.is_some() { ChangeType::Updated } else { ChangeType::Created };
        self.add_audit_entry(
            namespace_path,
            key,
            change_type.clone(),
            old_value.clone(),
            Some(value.clone()),
            changed_by,
            change_reason,
            &new_version,
        ).await;

        // Notify watchers
        self.notify_watchers(namespace_path, key, change_type, old_value, Some(value), change_reason, changed_by, &new_version).await;

        Ok(SetConfigResponse {
            success: true,
            namespace_path: namespace_path.to_string(),
            key: key.to_string(),
            new_version: Some(new_version),
            validation_errors: Vec::new(),
            error_message: None,
        })
    }

    /// Watch for configuration changes
    pub async fn watch_config(
        &self,
        namespace_path: &str,
        keys: Vec<String>,
        include_initial_values: bool,
    ) -> Result<mpsc::UnboundedReceiver<ConfigChangeEvent>> {
        let (tx, rx) = mpsc::unbounded_channel();

        // Send initial values if requested
        if include_initial_values {
            for key in &keys {
                if let Ok(Some(response)) = self.get_config(namespace_path, key, None, None, false).await {
                    if let Some(value) = response.value {
                        let event = ConfigChangeEvent {
                            namespace_path: namespace_path.to_string(),
                            key: key.clone(),
                            change_type: ChangeType::Created,
                            old_value: None,
                            new_value: Some(value),
                            timestamp: Utc::now(),
                            change_reason: "initial_value".to_string(),
                            changed_by: "system".to_string(),
                            version: response.version,
                        };
                        let _ = tx.send(event);
                    }
                }
            }
        }

        // Register watcher
        let watcher = ConfigWatcher {
            namespace_path: namespace_path.to_string(),
            keys,
            sender: tx,
        };

        self.watchers.lock().await.push(watcher);

        Ok(rx)
    }

    /// Health check
    pub async fn health_check(&self) -> Result<HealthStatus> {
        let storage_count = self.storage.lock().await.len();
        let watcher_count = self.watchers.lock().await.len();

        Ok(HealthStatus {
            healthy: true,
            status: "SERVING".to_string(),
            details: HashMap::from([
                ("storage_entries".to_string(), storage_count.to_string()),
                ("active_watchers".to_string(), watcher_count.to_string()),
            ]),
            timestamp: Utc::now(),
            version: "1.0.0".to_string(),
        })
    }

    // Helper methods

    async fn validate_against_schema(&self, namespace_path: &str, key: &str, value: &ConfigValue) -> Result<Vec<String>> {
        let schemas = self.schemas.lock().await;
        
        // Simple validation - in real implementation would use JSON Schema validation
        if let Some(_schema) = schemas.get(namespace_path) {
            // Perform actual schema validation here
            // For now, just check basic type consistency
            match &value.data {
                ConfigData::String(s) if s.is_empty() => {
                    return Ok(vec!["String value cannot be empty".to_string()]);
                }
                ConfigData::Int(i) if *i < 0 && key.contains("positive") => {
                    return Ok(vec!["Value must be positive".to_string()]);
                }
                _ => {}
            }
        }

        Ok(Vec::new())
    }

    async fn add_audit_entry(
        &self,
        namespace_path: &str,
        key: &str,
        change_type: ChangeType,
        old_value: Option<ConfigValue>,
        new_value: Option<ConfigValue>,
        changed_by: &str,
        change_reason: &str,
        version: &str,
    ) {
        let entry = AuditEntry {
            timestamp: Utc::now(),
            namespace_path: namespace_path.to_string(),
            key: key.to_string(),
            change_type,
            old_value,
            new_value,
            changed_by: changed_by.to_string(),
            change_reason: change_reason.to_string(),
            version: version.to_string(),
            session_id: "test-session".to_string(),
        };

        self.audit_trail.lock().await.push(entry);
    }

    async fn notify_watchers(
        &self,
        namespace_path: &str,
        key: &str,
        change_type: ChangeType,
        old_value: Option<ConfigValue>,
        new_value: Option<ConfigValue>,
        change_reason: &str,
        changed_by: &str,
        version: &str,
    ) {
        let watchers = self.watchers.lock().await;

        for watcher in watchers.iter() {
            if watcher.namespace_path == namespace_path && 
               (watcher.keys.is_empty() || watcher.keys.contains(&key.to_string())) {
                let event = ConfigChangeEvent {
                    namespace_path: namespace_path.to_string(),
                    key: key.to_string(),
                    change_type: change_type.clone(),
                    old_value: old_value.clone(),
                    new_value: new_value.clone(),
                    timestamp: Utc::now(),
                    change_reason: change_reason.to_string(),
                    changed_by: changed_by.to_string(),
                    version: version.to_string(),
                };

                let _ = watcher.sender.send(event);
            }
        }
    }

    pub async fn add_schema(&self, namespace_path: &str, schema_version: &str, json_schema: &str) {
        let schema = ConfigSchema {
            namespace_path: namespace_path.to_string(),
            schema_version: schema_version.to_string(),
            json_schema: json_schema.to_string(),
        };

        self.schemas.lock().await.insert(namespace_path.to_string(), schema);
    }

    pub async fn get_audit_trail(&self) -> Vec<AuditEntry> {
        self.audit_trail.lock().await.clone()
    }
}

#[derive(Debug, Clone)]
pub struct GetConfigResponse {
    pub success: bool,
    pub namespace_path: String,
    pub key: String,
    pub version: String,
    pub value: Option<ConfigValue>,
    pub metadata: Option<ConfigMetadata>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GetBulkConfigResponse {
    pub success: bool,
    pub namespace_path: String,
    pub version: String,
    pub values: HashMap<String, ConfigValue>,
    pub metadata: Option<HashMap<String, ConfigMetadata>>,
    pub missing_keys: Vec<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SetConfigResponse {
    pub success: bool,
    pub namespace_path: String,
    pub key: String,
    pub new_version: Option<String>,
    pub validation_errors: Vec<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub healthy: bool,
    pub status: String,
    pub details: HashMap<String, String>,
    pub timestamp: DateTime<Utc>,
    pub version: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::timeout;

    #[tokio::test]
    async fn test_get_config_basic() {
        let store = MockConfigStore::new();
        
        // Set a configuration value
        let value = ConfigValue {
            value_type: ValueType::String,
            data: ConfigData::String("test_value".to_string()),
        };

        let set_response = store.set_config(
            "/neural-trading/data-ingestion",
            "batch_size",
            value,
            "Initial setup",
            false,
            None,
            "test_user",
        ).await.unwrap();

        assert!(set_response.success);
        assert!(set_response.new_version.is_some());

        // Get the configuration value
        let get_response = store.get_config(
            "/neural-trading/data-ingestion",
            "batch_size",
            None,
            None,
            true,
        ).await.unwrap().unwrap();

        assert!(get_response.success);
        assert_eq!(get_response.namespace_path, "/neural-trading/data-ingestion");
        assert_eq!(get_response.key, "batch_size");
        assert!(get_response.value.is_some());
        assert!(get_response.metadata.is_some());

        if let Some(ConfigValue { data: ConfigData::String(val), .. }) = get_response.value {
            assert_eq!(val, "test_value");
        } else {
            panic!("Expected string value");
        }
    }

    #[tokio::test]
    async fn test_get_config_nonexistent() {
        let store = MockConfigStore::new();
        
        let result = store.get_config(
            "/nonexistent/namespace",
            "missing_key",
            None,
            None,
            false,
        ).await.unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_bulk_config_operations() {
        let store = MockConfigStore::new();
        let namespace = "/neural-trading/ml-ops";
        
        // Set multiple configurations
        let configs = vec![
            ("model_timeout", ConfigValue {
                value_type: ValueType::Int,
                data: ConfigData::Int(30),
            }),
            ("accuracy_threshold", ConfigValue {
                value_type: ValueType::Float,
                data: ConfigData::Float(0.85),
            }),
            ("enable_monitoring", ConfigValue {
                value_type: ValueType::Bool,
                data: ConfigData::Bool(true),
            }),
        ];

        for (key, value) in configs {
            let response = store.set_config(
                namespace,
                key,
                value,
                "Bulk setup",
                false,
                None,
                "admin",
            ).await.unwrap();
            assert!(response.success);
        }

        // Test bulk retrieval
        let keys = vec![
            "model_timeout".to_string(),
            "accuracy_threshold".to_string(),
            "enable_monitoring".to_string(),
            "nonexistent_key".to_string(),
        ];

        let bulk_response = store.get_bulk_config(
            namespace,
            keys,
            None,
            None,
            true,
        ).await.unwrap();

        assert!(bulk_response.success);
        assert_eq!(bulk_response.values.len(), 3);
        assert_eq!(bulk_response.missing_keys, vec!["nonexistent_key"]);
        assert!(bulk_response.metadata.is_some());

        // Verify specific values
        assert!(matches!(
            bulk_response.values.get("model_timeout").unwrap().data,
            ConfigData::Int(30)
        ));
        assert!(matches!(
            bulk_response.values.get("accuracy_threshold").unwrap().data,
            ConfigData::Float(f) if (f - 0.85).abs() < f64::EPSILON
        ));
        assert!(matches!(
            bulk_response.values.get("enable_monitoring").unwrap().data,
            ConfigData::Bool(true)
        ));
    }

    #[tokio::test]
    async fn test_version_control() {
        let store = MockConfigStore::new();
        let namespace = "/config/versioning";
        let key = "test_param";

        // Set initial value
        let initial_value = ConfigValue {
            value_type: ValueType::String,
            data: ConfigData::String("v1".to_string()),
        };

        let v1_response = store.set_config(
            namespace,
            key,
            initial_value,
            "Initial version",
            false,
            None,
            "user1",
        ).await.unwrap();

        assert!(v1_response.success);
        let v1_version = v1_response.new_version.unwrap();

        // Update with correct version
        let updated_value = ConfigValue {
            value_type: ValueType::String,
            data: ConfigData::String("v2".to_string()),
        };

        let v2_response = store.set_config(
            namespace,
            key,
            updated_value,
            "Version update",
            false,
            Some(&v1_version),
            "user2",
        ).await.unwrap();

        assert!(v2_response.success);

        // Try to update with old version (should fail)
        let conflicted_value = ConfigValue {
            value_type: ValueType::String,
            data: ConfigData::String("conflict".to_string()),
        };

        let conflict_response = store.set_config(
            namespace,
            key,
            conflicted_value,
            "Conflicted update",
            false,
            Some(&v1_version), // Using old version
            "user3",
        ).await.unwrap();

        assert!(!conflict_response.success);
        assert!(conflict_response.error_message.is_some());
    }

    #[tokio::test]
    async fn test_validation_only() {
        let store = MockConfigStore::new();
        
        // Add a schema for validation
        store.add_schema(
            "/config/validation",
            "v1.0",
            r#"{"type": "object", "properties": {"test": {"type": "string"}}}"#,
        ).await;

        let value = ConfigValue {
            value_type: ValueType::String,
            data: ConfigData::String("valid_value".to_string()),
        };

        // Test validation only (dry run)
        let validation_response = store.set_config(
            "/config/validation",
            "test",
            value,
            "Validation test",
            true, // validate_only = true
            None,
            "validator",
        ).await.unwrap();

        assert!(validation_response.success);
        assert!(validation_response.new_version.is_none());

        // Verify nothing was actually stored
        let get_response = store.get_config(
            "/config/validation",
            "test",
            None,
            None,
            false,
        ).await.unwrap();

        assert!(get_response.is_none());
    }

    #[tokio::test]
    async fn test_config_watching() {
        let store = MockConfigStore::new();
        let namespace = "/config/watch-test";
        
        // Start watching
        let mut watcher = store.watch_config(
            namespace,
            vec!["watched_key".to_string()],
            false,
        ).await.unwrap();

        // Set a configuration (should trigger watch)
        let value = ConfigValue {
            value_type: ValueType::String,
            data: ConfigData::String("watched_value".to_string()),
        };

        store.set_config(
            namespace,
            "watched_key",
            value,
            "Watch trigger",
            false,
            None,
            "watcher_test",
        ).await.unwrap();

        // Should receive change event
        let event = timeout(Duration::from_millis(100), watcher.recv())
            .await
            .expect("Should receive event within timeout")
            .expect("Should receive valid event");

        assert_eq!(event.namespace_path, namespace);
        assert_eq!(event.key, "watched_key");
        assert_eq!(event.change_type, ChangeType::Created);
        assert!(event.new_value.is_some());
    }

    #[tokio::test]
    async fn test_multiple_value_types() {
        let store = MockConfigStore::new();
        let namespace = "/config/types";

        let test_cases = vec![
            ("string_val", ConfigValue {
                value_type: ValueType::String,
                data: ConfigData::String("hello world".to_string()),
            }),
            ("int_val", ConfigValue {
                value_type: ValueType::Int,
                data: ConfigData::Int(42),
            }),
            ("float_val", ConfigValue {
                value_type: ValueType::Float,
                data: ConfigData::Float(3.14159),
            }),
            ("bool_val", ConfigValue {
                value_type: ValueType::Bool,
                data: ConfigData::Bool(true),
            }),
            ("json_val", ConfigValue {
                value_type: ValueType::Json,
                data: ConfigData::Json(json!({"nested": {"value": 123}})),
            }),
            ("binary_val", ConfigValue {
                value_type: ValueType::Binary,
                data: ConfigData::Binary(vec![0xFF, 0xFE, 0xFD]),
            }),
        ];

        // Set all values
        for (key, value) in &test_cases {
            let response = store.set_config(
                namespace,
                key,
                value.clone(),
                "Type test",
                false,
                None,
                "type_tester",
            ).await.unwrap();
            assert!(response.success, "Failed to set {}", key);
        }

        // Retrieve and verify all values
        for (key, expected_value) in &test_cases {
            let response = store.get_config(
                namespace,
                key,
                None,
                None,
                false,
            ).await.unwrap().unwrap();

            assert!(response.success);
            let actual_value = response.value.unwrap();
            assert_eq!(actual_value.value_type, expected_value.value_type);
            
            match (&actual_value.data, &expected_value.data) {
                (ConfigData::String(a), ConfigData::String(b)) => assert_eq!(a, b),
                (ConfigData::Int(a), ConfigData::Int(b)) => assert_eq!(a, b),
                (ConfigData::Float(a), ConfigData::Float(b)) => assert!((a - b).abs() < f64::EPSILON),
                (ConfigData::Bool(a), ConfigData::Bool(b)) => assert_eq!(a, b),
                (ConfigData::Json(a), ConfigData::Json(b)) => assert_eq!(a, b),
                (ConfigData::Binary(a), ConfigData::Binary(b)) => assert_eq!(a, b),
                _ => panic!("Type mismatch for key {}", key),
            }
        }
    }

    #[tokio::test]
    async fn test_health_check() {
        let store = MockConfigStore::new();
        
        // Add some data to make health check more meaningful
        for i in 0..5 {
            let value = ConfigValue {
                value_type: ValueType::Int,
                data: ConfigData::Int(i),
            };
            store.set_config(
                "/health/test",
                &format!("key_{}", i),
                value,
                "Health test setup",
                false,
                None,
                "health_tester",
            ).await.unwrap();
        }

        let health = store.health_check().await.unwrap();
        
        assert!(health.healthy);
        assert_eq!(health.status, "SERVING");
        assert!(health.details.contains_key("storage_entries"));
        assert_eq!(health.details.get("storage_entries").unwrap(), "5");
    }

    #[tokio::test]
    async fn test_performance_requirements() {
        let store = MockConfigStore::new();
        let namespace = "/performance/test";

        // Test read performance (<1ms requirement)
        let value = ConfigValue {
            value_type: ValueType::String,
            data: ConfigData::String("performance_test".to_string()),
        };

        store.set_config(namespace, "perf_key", value, "Perf test", false, None, "perf_tester").await.unwrap();

        let start = Instant::now();
        let _result = store.get_config(namespace, "perf_key", None, None, false).await.unwrap();
        let read_duration = start.elapsed();

        assert!(read_duration < Duration::from_millis(1), 
               "Read took {}µs, should be <1ms", read_duration.as_micros());

        // Test write performance (<5ms requirement)
        let write_value = ConfigValue {
            value_type: ValueType::String,
            data: ConfigData::String("write_performance_test".to_string()),
        };

        let start = Instant::now();
        let _result = store.set_config(namespace, "write_perf_key", write_value, "Write perf test", false, None, "perf_tester").await.unwrap();
        let write_duration = start.elapsed();

        assert!(write_duration < Duration::from_millis(5), 
               "Write took {}ms, should be <5ms", write_duration.as_millis());
    }

    #[tokio::test]
    async fn test_audit_trail() {
        let store = MockConfigStore::new();
        let namespace = "/audit/test";
        let key = "audit_key";

        // Perform several operations
        let operations = vec![
            ("create", ConfigValue {
                value_type: ValueType::String,
                data: ConfigData::String("initial".to_string()),
            }),
            ("update1", ConfigValue {
                value_type: ValueType::String,
                data: ConfigData::String("updated1".to_string()),
            }),
            ("update2", ConfigValue {
                value_type: ValueType::String,
                data: ConfigData::String("updated2".to_string()),
            }),
        ];

        for (reason, value) in operations {
            store.set_config(namespace, key, value, reason, false, None, "auditor").await.unwrap();
        }

        // Check audit trail
        let audit_entries = store.get_audit_trail().await;
        let relevant_entries: Vec<_> = audit_entries.iter()
            .filter(|entry| entry.namespace_path == namespace && entry.key == key)
            .collect();

        assert_eq!(relevant_entries.len(), 3);
        assert_eq!(relevant_entries[0].change_type, ChangeType::Created);
        assert_eq!(relevant_entries[1].change_type, ChangeType::Updated);
        assert_eq!(relevant_entries[2].change_type, ChangeType::Updated);
    }

    #[tokio::test]
    async fn test_concurrent_access() {
        let store = std::sync::Arc::new(MockConfigStore::new());
        let namespace = "/concurrent/test";
        
        let handles: Vec<_> = (0..10).map(|i| {
            let store_clone = store.clone();
            let namespace = namespace.to_string();
            tokio::spawn(async move {
                let value = ConfigValue {
                    value_type: ValueType::Int,
                    data: ConfigData::Int(i),
                };
                store_clone.set_config(
                    &namespace,
                    &format!("key_{}", i),
                    value,
                    "Concurrent test",
                    false,
                    None,
                    &format!("user_{}", i),
                ).await.unwrap();

                // Also read
                store_clone.get_config(&namespace, &format!("key_{}", i), None, None, false).await.unwrap()
            })
        }).collect();

        // Wait for all operations to complete
        for handle in handles {
            let result = handle.await.unwrap();
            assert!(result.is_some());
        }

        // Verify all data was stored correctly
        let keys: Vec<String> = (0..10).map(|i| format!("key_{}", i)).collect();
        let bulk_result = store.get_bulk_config(namespace, keys, None, None, false).await.unwrap();
        assert_eq!(bulk_result.values.len(), 10);
        assert!(bulk_result.missing_keys.is_empty());
    }
}