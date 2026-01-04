//! Trait definitions for MCP tool dependencies (dp-005)
//!
//! These traits follow the Domain Adapter pattern (ADR-002) to abstract
//! storage and configuration access, enabling London School TDD with mocks.
//!
//! # Traits
//!
//! - `BronzeStorage`: Parquet file access for Bronze layer
//! - `ConfigStore`: etcd configuration access
//!
//! # Mock Usage
//!
//! ```ignore
//! use mockall::predicate::*;
//! use neural_core::mcp::tools::traits::MockBronzeStorage;
//!
//! let mut mock = MockBronzeStorage::new();
//! mock.expect_list()
//!     .returning(|| Ok(vec![StreamStorageInfo { ... }]));
//! ```

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use thiserror::Error;

#[cfg(test)]
use mockall::automock;

// =============================================================================
// Error Types
// =============================================================================

/// Errors from Bronze storage operations
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Stream not found: {0}")]
    StreamNotFound(String),

    #[error("No data available for stream: {0}")]
    NoDataAvailable(String),

    #[error("Storage I/O error: {0}")]
    IoError(String),

    #[error("Parquet parse error: {0}")]
    ParseError(String),

    #[error("Storage unavailable: {0}")]
    Unavailable(String),
}

/// Errors from configuration store operations
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Stream not found: {0}")]
    StreamNotFound(String),

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Config parse error: {0}")]
    ParseError(String),

    #[error("Configuration unavailable: {0}")]
    Unavailable(String),
}

// =============================================================================
// Data Types
// =============================================================================

/// Storage metadata for a Bronze stream
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamStorageInfo {
    /// Stream identifier (e.g., "air-quality")
    pub stream_id: String,

    /// Most recent partition path (e.g., "year=2026/month=01/day=03")
    pub latest_partition: Option<String>,

    /// Size of data.parquet file in bytes
    pub file_size_bytes: Option<u64>,

    /// File modification timestamp
    pub file_modified: Option<DateTime<Utc>>,

    /// Estimated row count (if available from metadata)
    pub row_count: Option<u64>,
}

/// Configuration metadata for a stream
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamConfigInfo {
    /// Stream identifier
    pub stream_id: String,

    /// Human-readable description
    pub description: String,

    /// Whether stream is enabled
    pub enabled: bool,

    /// Semantic version
    pub version: String,

    /// Source types (mqtt, http_poll, etc.)
    pub sources: Vec<String>,
}

/// Field mapping from source to target
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldMapping {
    /// JSON path in raw_payload (e.g., "main.temp")
    pub source_path: String,

    /// Target field name (e.g., "temperature")
    pub target_field: String,

    /// Unit of measurement
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}

/// Entity schema attribute definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityAttribute {
    /// Attribute name
    pub name: String,

    /// Data type (float, int, string, boolean)
    #[serde(rename = "type")]
    pub data_type: String,

    /// Unit of measurement
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,

    /// Whether field can be null
    #[serde(default)]
    pub nullable: bool,

    /// Field description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Entity schema definition from config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitySchema {
    /// Schema name
    pub schema_name: String,

    /// Schema attributes
    pub attributes: Vec<EntityAttribute>,
}

/// Parser configuration from stream config
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParserInfo {
    /// Parser type (json_path, flat_json, etc.)
    pub parser_type: String,

    /// Field mappings
    pub field_mappings: Vec<FieldMapping>,
}

/// Full stream configuration including entity schemas
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullStreamConfig {
    /// Basic stream info
    pub info: StreamConfigInfo,

    /// Parser configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parser: Option<ParserInfo>,

    /// Entity schemas for Silver layer
    pub entity_schemas: Vec<EntitySchema>,
}

/// Raw payload structure analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayloadStructure {
    /// Top-level keys
    pub keys: Vec<String>,

    /// Nested object keys (parent -> child keys)
    pub nested: HashMap<String, Vec<String>>,
}

/// Bronze row data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BronzeRow {
    /// Ingestion timestamp (microseconds)
    pub timestamp: i64,

    /// Source identifier
    pub source_id: String,

    /// Stable platform identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ndp_id: Option<String>,

    /// Context metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Value>,

    /// Raw source payload
    pub raw_payload: Value,
}

// =============================================================================
// Bronze Storage Trait
// =============================================================================

/// Bronze layer storage abstraction (Port)
///
/// Provides read-only access to Bronze Parquet files following
/// the Domain Adapter pattern (ADR-002).
///
/// # Implementations
/// - `LocalParquetStorage`: Local filesystem (Pi deployment)
/// - `S3ParquetStorage`: S3 object storage (future cloud)
#[cfg_attr(test, automock)]
#[async_trait]
pub trait BronzeStorage: Send + Sync {
    /// List all streams that have data in Bronze storage
    ///
    /// Returns stream IDs with storage metadata (latest partition, file size, etc.)
    async fn list(&self) -> Result<Vec<StreamStorageInfo>, StorageError>;

    /// Get the Parquet schema for a specific stream
    ///
    /// Returns Arrow schema column names and types
    async fn schema(&self, stream_id: &str) -> Result<Vec<String>, StorageError>;

    /// Analyze raw_payload structure from sample data
    ///
    /// Returns keys and nested structure from JSON payloads
    async fn analyze_payload(&self, stream_id: &str) -> Result<PayloadStructure, StorageError>;

    /// Sample N rows from the most recent partition
    ///
    /// Returns rows ordered by timestamp descending (most recent first)
    async fn sample(&self, stream_id: &str, n: usize) -> Result<Vec<BronzeRow>, StorageError>;

    /// Validate that Bronze storage is accessible
    ///
    /// Used for health checks and startup validation
    async fn validate(&self) -> Result<(), StorageError>;

    /// Get the file path that was analyzed (for response metadata)
    async fn get_latest_file_path(&self, stream_id: &str) -> Result<Option<String>, StorageError>;
}

// =============================================================================
// Config Store Trait
// =============================================================================

/// Configuration store abstraction (Port)
///
/// Provides read-only access to stream configuration from etcd.
///
/// # Implementations
/// - `EtcdConfigStore`: Real etcd v3 client
/// - `MockConfigStore`: Test mock
#[cfg_attr(test, automock)]
#[async_trait]
pub trait ConfigStore: Send + Sync {
    /// List all configured stream IDs
    async fn list_stream_ids(&self) -> Result<Vec<String>, ConfigError>;

    /// Get basic stream configuration
    async fn get_stream_config(&self, stream_id: &str) -> Result<StreamConfigInfo, ConfigError>;

    /// Get full stream configuration including entity schemas
    async fn get_full_config(&self, stream_id: &str) -> Result<FullStreamConfig, ConfigError>;

    /// Check if etcd is healthy and reachable
    async fn health_check(&self) -> Result<(), ConfigError>;
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_storage_info_serialization() {
        let info = StreamStorageInfo {
            stream_id: "air-quality".to_string(),
            latest_partition: Some("year=2026/month=01/day=03".to_string()),
            file_size_bytes: Some(7310),
            file_modified: Some(Utc::now()),
            row_count: Some(100),
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("air-quality"));
        assert!(json.contains("7310"));
    }

    #[test]
    fn test_stream_config_info_serialization() {
        let info = StreamConfigInfo {
            stream_id: "air-quality".to_string(),
            description: "AirGradient sensor readings".to_string(),
            enabled: true,
            version: "1.0.0".to_string(),
            sources: vec!["mqtt".to_string()],
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("AirGradient"));
        assert!(json.contains("mqtt"));
    }

    #[test]
    fn test_field_mapping_serialization() {
        let mapping = FieldMapping {
            source_path: "main.temp".to_string(),
            target_field: "temperature".to_string(),
            unit: Some("celsius".to_string()),
        };

        let json = serde_json::to_string(&mapping).unwrap();
        assert!(json.contains("main.temp"));
        assert!(json.contains("celsius"));
    }

    #[test]
    fn test_storage_error_display() {
        let err = StorageError::StreamNotFound("invalid".to_string());
        assert_eq!(err.to_string(), "Stream not found: invalid");

        let err = StorageError::NoDataAvailable("empty-stream".to_string());
        assert_eq!(err.to_string(), "No data available for stream: empty-stream");
    }

    #[test]
    fn test_config_error_display() {
        let err = ConfigError::ConnectionFailed("connection refused".to_string());
        assert_eq!(err.to_string(), "Connection failed: connection refused");
    }

    #[test]
    fn test_payload_structure_with_nested() {
        let mut nested = HashMap::new();
        nested.insert("main".to_string(), vec!["temp".to_string(), "humidity".to_string()]);

        let structure = PayloadStructure {
            keys: vec!["main".to_string(), "wind".to_string()],
            nested,
        };

        assert_eq!(structure.keys.len(), 2);
        assert_eq!(structure.nested.get("main").unwrap().len(), 2);
    }

    #[test]
    fn test_bronze_row_serialization() {
        let row = BronzeRow {
            timestamp: 1767452639760716,
            source_id: "air-quality-Mqtt".to_string(),
            ndp_id: Some("sensor-001".to_string()),
            context: Some(serde_json::json!({"room": "office"})),
            raw_payload: serde_json::json!({"pm25": 12.5}),
        };

        let json = serde_json::to_string(&row).unwrap();
        assert!(json.contains("air-quality-Mqtt"));
        assert!(json.contains("pm25"));
    }
}
