//! MockConfigLoader for dp-018 London TDD
//!
//! This module provides a mock implementation of the ConfigLoader trait
//! for unit testing without etcd dependency. Follows London School TDD
//! principles: test behavior, not implementation.
//!
//! # Design Rationale (dp-018)
//!
//! The MockConfigLoader enables testing of components that depend on
//! configuration loading without requiring etcd infrastructure. This is
//! critical for the dp-018 goal of establishing patterns for testing
//! WITHOUT infrastructure.
//!
//! # Usage
//!
//! ```ignore
//! use neural_core::config::{MockConfigLoader, ConfigLoader};
//!
//! let loader = MockConfigLoader::new()
//!     .with_stream(create_test_config("air-quality"))
//!     .with_silver_config("air-quality", create_test_silver_config());
//!
//! let config = loader.load_stream_config("air-quality").await?;
//! ```
//!
//! # Error Simulation
//!
//! The mock can be configured to simulate errors:
//!
//! ```ignore
//! let loader = MockConfigLoader::new()
//!     .with_error(ConfigLoaderError::ConnectionError("etcd unreachable".into()));
//!
//! let result = loader.load_stream_config("any").await;
//! assert!(result.is_err());
//! ```

use crate::config::silver_etl::SilverEtlConfig;
use crate::types::StreamConfig;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::RwLock;
use thiserror::Error;

// ============================================================================
// ConfigLoaderError
// ============================================================================

/// Configuration loading errors
///
/// These errors represent the failure modes for configuration loading.
/// The error types are designed to be:
/// - Specific enough to enable proper error handling
/// - General enough to work with multiple backends (etcd, mock, file)
#[derive(Debug, Error, Clone)]
pub enum ConfigLoaderError {
    /// Stream not found in configuration store
    #[error("Stream not found: {0}")]
    StreamNotFound(String),

    /// Connection error to configuration store
    #[error("Connection error: {0}")]
    ConnectionError(String),

    /// Error parsing configuration data
    #[error("Parse error: {0}")]
    ParseError(String),

    /// Configuration validation error
    #[error("Validation error: {0}")]
    ValidationError(String),
}

// ============================================================================
// ConfigLoader Trait
// ============================================================================

/// Unified trait for configuration loading
///
/// This trait defines the interface for loading stream and Silver ETL
/// configurations. Implementations can be:
/// - `EtcdConfigLoader` - Production implementation using etcd
/// - `MockConfigLoader` - Test implementation with predefined responses
///
/// # Design Rationale (ADR-002)
///
/// Following the Domain Adapter pattern:
/// - This trait is the **port** (interface)
/// - `EtcdConfigLoader` is the **adapter** for etcd
/// - `MockConfigLoader` is the **adapter** for testing
///
/// # Methods
///
/// - `load_stream_config`: Load Bronze layer stream configuration
/// - `load_silver_etl_config`: Load Silver ETL transformation config
/// - `list_streams`: Enumerate all configured streams
/// - `stream_exists`: Check if a stream configuration exists
/// - `source_name`: Get the configuration source name (for logging)
#[async_trait]
pub trait ConfigLoader: Send + Sync {
    /// Load stream configuration by stream_id
    ///
    /// # Arguments
    ///
    /// * `stream_id` - The stream identifier (e.g., "air-quality")
    ///
    /// # Returns
    ///
    /// The `StreamConfig` for the specified stream.
    ///
    /// # Errors
    ///
    /// - `ConfigLoaderError::StreamNotFound` if stream doesn't exist
    /// - `ConfigLoaderError::ConnectionError` if backend is unreachable
    /// - `ConfigLoaderError::ParseError` if config is malformed
    async fn load_stream_config(&self, stream_id: &str) -> Result<StreamConfig, ConfigLoaderError>;

    /// Load Silver ETL configuration for a stream
    ///
    /// # Arguments
    ///
    /// * `stream_id` - The stream identifier
    ///
    /// # Returns
    ///
    /// The `SilverEtlConfig` for Bronze-to-Silver transformation.
    ///
    /// # Errors
    ///
    /// - `ConfigLoaderError::StreamNotFound` if stream doesn't exist
    /// - `ConfigLoaderError::ConnectionError` if backend is unreachable
    async fn load_silver_etl_config(
        &self,
        stream_id: &str,
    ) -> Result<SilverEtlConfig, ConfigLoaderError>;

    /// List all stream IDs
    ///
    /// # Returns
    ///
    /// A vector of all configured stream identifiers.
    ///
    /// # Errors
    ///
    /// - `ConfigLoaderError::ConnectionError` if backend is unreachable
    async fn list_streams(&self) -> Result<Vec<String>, ConfigLoaderError>;

    /// Check if stream exists
    ///
    /// # Arguments
    ///
    /// * `stream_id` - The stream identifier to check
    ///
    /// # Returns
    ///
    /// `true` if the stream configuration exists, `false` otherwise.
    ///
    /// # Errors
    ///
    /// - `ConfigLoaderError::ConnectionError` if backend is unreachable
    async fn stream_exists(&self, stream_id: &str) -> Result<bool, ConfigLoaderError>;

    /// Get config source name (for logging)
    ///
    /// Returns a human-readable name for the configuration source.
    /// Used in log messages to identify which backend is being used.
    ///
    /// # Examples
    ///
    /// - "etcd" for production etcd backend
    /// - "mock" for test mock backend
    /// - "file" for file-based configuration
    fn source_name(&self) -> &'static str;
}

// ============================================================================
// MockConfigLoader Implementation
// ============================================================================

/// Mock implementation for unit testing without etcd
///
/// This mock provides a builder pattern for configuring expected responses
/// and simulated errors. It stores configurations in-memory using RwLock
/// for thread-safe access.
///
/// # Builder Pattern
///
/// The mock uses a builder pattern for configuration:
///
/// ```ignore
/// let loader = MockConfigLoader::new()
///     .with_stream(config1)
///     .with_stream(config2)
///     .with_silver_config("air-quality", silver_config)
///     .with_error(ConfigLoaderError::ConnectionError("test".into()));
/// ```
///
/// # Thread Safety
///
/// The mock uses `RwLock` for interior mutability, allowing it to be
/// shared across async tasks safely.
pub struct MockConfigLoader {
    /// Stream configurations indexed by stream_id
    streams: RwLock<HashMap<String, StreamConfig>>,

    /// Silver ETL configurations indexed by stream_id
    silver_configs: RwLock<HashMap<String, SilverEtlConfig>>,

    /// Optional error to return for all operations
    should_fail: RwLock<Option<ConfigLoaderError>>,
}

impl Default for MockConfigLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl MockConfigLoader {
    /// Create a new empty MockConfigLoader
    ///
    /// # Example
    ///
    /// ```ignore
    /// let loader = MockConfigLoader::new();
    /// ```
    pub fn new() -> Self {
        Self {
            streams: RwLock::new(HashMap::new()),
            silver_configs: RwLock::new(HashMap::new()),
            should_fail: RwLock::new(None),
        }
    }

    /// Add a stream config for testing (builder pattern)
    ///
    /// The stream_id is extracted from the config's `stream_id` field.
    ///
    /// # Arguments
    ///
    /// * `config` - The StreamConfig to add
    ///
    /// # Returns
    ///
    /// Self for method chaining.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let loader = MockConfigLoader::new()
    ///     .with_stream(StreamConfig { stream_id: "test".into(), ... });
    /// ```
    pub fn with_stream(self, config: StreamConfig) -> Self {
        self.streams
            .write()
            .unwrap()
            .insert(config.stream_id.clone(), config);
        self
    }

    /// Add a Silver ETL config for testing (builder pattern)
    ///
    /// # Arguments
    ///
    /// * `stream_id` - The stream identifier
    /// * `config` - The SilverEtlConfig to add
    ///
    /// # Returns
    ///
    /// Self for method chaining.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let loader = MockConfigLoader::new()
    ///     .with_silver_config("air-quality", SilverEtlConfig { ... });
    /// ```
    pub fn with_silver_config(self, stream_id: &str, config: SilverEtlConfig) -> Self {
        self.silver_configs
            .write()
            .unwrap()
            .insert(stream_id.to_string(), config);
        self
    }

    /// Configure mock to fail with specific error (builder pattern)
    ///
    /// When this is set, all operations will return this error.
    ///
    /// # Arguments
    ///
    /// * `error` - The error to return for all operations
    ///
    /// # Returns
    ///
    /// Self for method chaining.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let loader = MockConfigLoader::new()
    ///     .with_error(ConfigLoaderError::ConnectionError("etcd down".into()));
    /// ```
    pub fn with_error(self, error: ConfigLoaderError) -> Self {
        *self.should_fail.write().unwrap() = Some(error);
        self
    }

    /// Clear the configured error
    ///
    /// Removes any previously configured error, allowing operations
    /// to succeed again.
    pub fn clear_error(&self) {
        *self.should_fail.write().unwrap() = None;
    }

    /// Add multiple stream configs at once
    ///
    /// # Arguments
    ///
    /// * `configs` - A vector of StreamConfig to add
    ///
    /// # Returns
    ///
    /// Self for method chaining.
    pub fn with_streams(self, configs: Vec<StreamConfig>) -> Self {
        let mut streams = self.streams.write().unwrap();
        for config in configs {
            streams.insert(config.stream_id.clone(), config);
        }
        drop(streams);
        self
    }
}

#[async_trait]
impl ConfigLoader for MockConfigLoader {
    async fn load_stream_config(&self, stream_id: &str) -> Result<StreamConfig, ConfigLoaderError> {
        // Check if we should fail
        if let Some(ref err) = *self.should_fail.read().unwrap() {
            return Err(err.clone());
        }

        // Look up the stream config
        self.streams
            .read()
            .unwrap()
            .get(stream_id)
            .cloned()
            .ok_or_else(|| ConfigLoaderError::StreamNotFound(stream_id.to_string()))
    }

    async fn load_silver_etl_config(
        &self,
        stream_id: &str,
    ) -> Result<SilverEtlConfig, ConfigLoaderError> {
        // Check if we should fail
        if let Some(ref err) = *self.should_fail.read().unwrap() {
            return Err(err.clone());
        }

        // Look up the silver config
        self.silver_configs
            .read()
            .unwrap()
            .get(stream_id)
            .cloned()
            .ok_or_else(|| ConfigLoaderError::StreamNotFound(stream_id.to_string()))
    }

    async fn list_streams(&self) -> Result<Vec<String>, ConfigLoaderError> {
        // Check if we should fail
        if let Some(ref err) = *self.should_fail.read().unwrap() {
            return Err(err.clone());
        }

        Ok(self.streams.read().unwrap().keys().cloned().collect())
    }

    async fn stream_exists(&self, stream_id: &str) -> Result<bool, ConfigLoaderError> {
        // Check if we should fail
        if let Some(ref err) = *self.should_fail.read().unwrap() {
            return Err(err.clone());
        }

        Ok(self.streams.read().unwrap().contains_key(stream_id))
    }

    fn source_name(&self) -> &'static str {
        "mock"
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::silver_etl::{SilverEtlConfig, TimestampMapping, TimestampTransform};
    use crate::types::{FieldType, SchemaField, SourceConfig, SourceType, StreamConfig};
    use std::collections::HashMap;

    // ============================================================
    // Helper Functions
    // ============================================================

    fn create_test_stream_config(stream_id: &str) -> StreamConfig {
        StreamConfig {
            stream_id: stream_id.to_string(),
            stream_type: None, // FE-001: Optional for backward compatibility
            description: format!("{} test stream", stream_id),
            version: "1.0.0".to_string(),
            enabled: true,
            retention_days: 365,
            compression_after_days: 7,
            partitioning_strategy: "daily".to_string(),
            fields: vec![
                SchemaField::new("pm25".to_string(), FieldType::Float)
                    .required()
                    .with_unit("ug/m3".to_string())
                    .with_range(0.0, 500.0),
                SchemaField::new("temperature".to_string(), FieldType::Float)
                    .with_unit("celsius".to_string()),
            ],
            sources: vec![SourceConfig {
                source_type: SourceType::Mqtt,
                enabled: true,
                ndp_id: Some(format!("{}-sensor-001", stream_id)),
                context: None,
                params: HashMap::new(),
            }],
            storage: None,
            silver_etl: None,
            entity_schemas: None,
        }
    }

    fn create_test_silver_config() -> SilverEtlConfig {
        SilverEtlConfig {
            enabled: true,
            target_table: "silver.air_quality_observations".to_string(),
            target_schema: None,
            timestamp: TimestampMapping {
                source_field: "timestamp".to_string(),
                target_field: "observation_time".to_string(),
                transform: TimestampTransform::MicrosecondsToTimestamp,
            },
            valid_timestamp: None,
            pre_transform: None,
            identity_fields: vec![],
            field_mappings: vec![],
            dq_rules: vec![],
            dq_output: Default::default(),
            deduplication: Default::default(),
            incremental: Default::default(),
        }
    }

    // ============================================================
    // Test: MockConfigLoader new creates empty loader
    // ============================================================

    #[test]
    fn test_mock_config_loader_new_creates_empty() {
        let loader = MockConfigLoader::new();

        assert!(loader.streams.read().unwrap().is_empty());
        assert!(loader.silver_configs.read().unwrap().is_empty());
        assert!(loader.should_fail.read().unwrap().is_none());
    }

    // ============================================================
    // Test: with_stream adds stream config
    // ============================================================

    #[test]
    fn test_with_stream_adds_config() {
        let config = create_test_stream_config("test-stream");

        let loader = MockConfigLoader::new().with_stream(config);

        assert!(loader.streams.read().unwrap().contains_key("test-stream"));
    }

    // ============================================================
    // Test: with_silver_config adds silver config
    // ============================================================

    #[test]
    fn test_with_silver_config_adds_config() {
        let silver_config = create_test_silver_config();

        let loader = MockConfigLoader::new().with_silver_config("air-quality", silver_config);

        assert!(loader
            .silver_configs
            .read()
            .unwrap()
            .contains_key("air-quality"));
    }

    // ============================================================
    // Test: with_error configures failure
    // ============================================================

    #[test]
    fn test_with_error_configures_failure() {
        let loader = MockConfigLoader::new()
            .with_error(ConfigLoaderError::ConnectionError("test error".into()));

        assert!(loader.should_fail.read().unwrap().is_some());
    }

    // ============================================================
    // Test: load_stream_config success
    // ============================================================

    #[tokio::test]
    async fn test_load_stream_config_success() {
        let config = create_test_stream_config("air-quality");
        let loader = MockConfigLoader::new().with_stream(config.clone());

        let result = loader.load_stream_config("air-quality").await;

        assert!(result.is_ok());
        let loaded = result.unwrap();
        assert_eq!(loaded.stream_id, "air-quality");
        assert_eq!(loaded.fields.len(), 2);
    }

    // ============================================================
    // Test: load_stream_config not found
    // ============================================================

    #[tokio::test]
    async fn test_load_stream_config_not_found() {
        let loader = MockConfigLoader::new();

        let result = loader.load_stream_config("nonexistent").await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ConfigLoaderError::StreamNotFound(_)
        ));
    }

    // ============================================================
    // Test: load_stream_config connection error
    // ============================================================

    #[tokio::test]
    async fn test_load_stream_config_connection_error() {
        let loader = MockConfigLoader::new().with_error(ConfigLoaderError::ConnectionError(
            "etcd unreachable".into(),
        ));

        let result = loader.load_stream_config("any").await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ConfigLoaderError::ConnectionError(_)
        ));
    }

    // ============================================================
    // Test: load_silver_etl_config success
    // ============================================================

    #[tokio::test]
    async fn test_load_silver_etl_config_success() {
        let silver_config = create_test_silver_config();

        let loader = MockConfigLoader::new().with_silver_config("air-quality", silver_config);

        let result = loader.load_silver_etl_config("air-quality").await;

        assert!(result.is_ok());
        let loaded = result.unwrap();
        assert!(loaded.enabled);
        assert_eq!(loaded.target_table, "silver.air_quality_observations");
    }

    // ============================================================
    // Test: load_silver_etl_config not found
    // ============================================================

    #[tokio::test]
    async fn test_load_silver_etl_config_not_found() {
        let loader = MockConfigLoader::new();

        let result = loader.load_silver_etl_config("nonexistent").await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ConfigLoaderError::StreamNotFound(_)
        ));
    }

    // ============================================================
    // Test: list_streams returns all stream ids
    // ============================================================

    #[tokio::test]
    async fn test_list_streams_returns_all_ids() {
        let loader = MockConfigLoader::new()
            .with_stream(create_test_stream_config("stream-a"))
            .with_stream(create_test_stream_config("stream-b"))
            .with_stream(create_test_stream_config("stream-c"));

        let result = loader.list_streams().await;

        assert!(result.is_ok());
        let streams = result.unwrap();
        assert_eq!(streams.len(), 3);
        assert!(streams.contains(&"stream-a".to_string()));
        assert!(streams.contains(&"stream-b".to_string()));
        assert!(streams.contains(&"stream-c".to_string()));
    }

    // ============================================================
    // Test: list_streams empty when no streams
    // ============================================================

    #[tokio::test]
    async fn test_list_streams_empty() {
        let loader = MockConfigLoader::new();

        let result = loader.list_streams().await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    // ============================================================
    // Test: stream_exists true for existing stream
    // ============================================================

    #[tokio::test]
    async fn test_stream_exists_true() {
        let loader =
            MockConfigLoader::new().with_stream(create_test_stream_config("existing-stream"));

        let result = loader.stream_exists("existing-stream").await;

        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    // ============================================================
    // Test: stream_exists false for non-existing stream
    // ============================================================

    #[tokio::test]
    async fn test_stream_exists_false() {
        let loader = MockConfigLoader::new();

        let result = loader.stream_exists("nonexistent").await;

        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    // ============================================================
    // Test: source_name returns "mock"
    // ============================================================

    #[test]
    fn test_source_name_returns_mock() {
        let loader = MockConfigLoader::new();
        assert_eq!(loader.source_name(), "mock");
    }

    // ============================================================
    // Test: clear_error removes error
    // ============================================================

    #[tokio::test]
    async fn test_clear_error_removes_error() {
        let loader = MockConfigLoader::new()
            .with_stream(create_test_stream_config("test"))
            .with_error(ConfigLoaderError::ConnectionError("test".into()));

        // Initially should fail
        assert!(loader.load_stream_config("test").await.is_err());

        // Clear error
        loader.clear_error();

        // Now should succeed
        assert!(loader.load_stream_config("test").await.is_ok());
    }

    // ============================================================
    // Test: with_streams adds multiple configs
    // ============================================================

    #[test]
    fn test_with_streams_adds_multiple() {
        let configs = vec![
            create_test_stream_config("stream-1"),
            create_test_stream_config("stream-2"),
            create_test_stream_config("stream-3"),
        ];

        let loader = MockConfigLoader::new().with_streams(configs);

        let streams = loader.streams.read().unwrap();
        assert_eq!(streams.len(), 3);
        assert!(streams.contains_key("stream-1"));
        assert!(streams.contains_key("stream-2"));
        assert!(streams.contains_key("stream-3"));
    }

    // ============================================================
    // Test: Builder pattern chaining
    // ============================================================

    #[tokio::test]
    async fn test_builder_pattern_chaining() {
        let loader = MockConfigLoader::new()
            .with_stream(create_test_stream_config("air-quality"))
            .with_stream(create_test_stream_config("outdoor-weather"))
            .with_silver_config("air-quality", create_test_silver_config());

        // Verify stream configs
        assert!(loader.load_stream_config("air-quality").await.is_ok());
        assert!(loader.load_stream_config("outdoor-weather").await.is_ok());

        // Verify silver config
        assert!(loader.load_silver_etl_config("air-quality").await.is_ok());
        assert!(loader
            .load_silver_etl_config("outdoor-weather")
            .await
            .is_err());

        // Verify source name
        assert_eq!(loader.source_name(), "mock");
    }

    // ============================================================
    // Test: Error propagation to all methods
    // ============================================================

    #[tokio::test]
    async fn test_error_propagation_to_all_methods() {
        let loader = MockConfigLoader::new()
            .with_stream(create_test_stream_config("test"))
            .with_silver_config("test", create_test_silver_config())
            .with_error(ConfigLoaderError::ConnectionError(
                "simulated failure".into(),
            ));

        // All methods should return the error
        assert!(matches!(
            loader.load_stream_config("test").await,
            Err(ConfigLoaderError::ConnectionError(_))
        ));
        assert!(matches!(
            loader.load_silver_etl_config("test").await,
            Err(ConfigLoaderError::ConnectionError(_))
        ));
        assert!(matches!(
            loader.list_streams().await,
            Err(ConfigLoaderError::ConnectionError(_))
        ));
        assert!(matches!(
            loader.stream_exists("test").await,
            Err(ConfigLoaderError::ConnectionError(_))
        ));
    }

    // ============================================================
    // Test: Default trait implementation
    // ============================================================

    #[test]
    fn test_default_creates_empty_loader() {
        let loader: MockConfigLoader = Default::default();

        assert!(loader.streams.read().unwrap().is_empty());
        assert!(loader.silver_configs.read().unwrap().is_empty());
    }
}
