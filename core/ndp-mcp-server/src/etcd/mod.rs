//! etcd Configuration Store Module.
//!
//! Provides the `ConfigStore` trait and implementations for accessing
//! stream configuration stored in etcd. Follows the NDP Domain Adapter pattern.
//!
//! # Architecture
//!
//! - **Port**: `ConfigStore` trait defines the interface
//! - **Adapter**: `StreamRegistryAdapter` - Wraps config-client's StreamRegistry
//!
//! # etcd Schema
//!
//! Stream configurations are stored at `/streams/{stream_id}/*`:
//! ```text
//! /streams/air-quality/enabled = "true"
//! /streams/air-quality/source_type = "http"
//! /streams/air-quality/parser/field_mappings/0/source = "pm25"
//! ```

mod registry_adapter;

pub use registry_adapter::StreamRegistryAdapter;

use crate::error::McpResult;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[cfg(test)]
use mockall::automock;

/// Stream configuration from etcd.
///
/// Contains the parsed configuration for a single stream.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StreamConfig {
    /// Stream identifier
    pub stream_id: String,

    /// Whether the stream is enabled
    #[serde(default)]
    pub enabled: bool,

    /// Source type (http, mqtt, file, etc.)
    #[serde(default)]
    pub source_type: String,

    /// Parser field mappings
    #[serde(default)]
    pub field_mappings: Vec<FieldMapping>,

    /// Entity schema attributes (target schema)
    #[serde(default)]
    pub entity_schema: EntitySchema,

    /// Raw configuration values from etcd
    #[serde(skip)]
    pub raw_config: HashMap<String, String>,
}

/// Field mapping from source to Bronze/Silver.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FieldMapping {
    /// Source field path (e.g., "main.temp", "pm25")
    pub source: String,

    /// Target field name (e.g., "temperature", "pm25")
    #[serde(default)]
    pub target: Option<String>,

    /// Field type hint
    #[serde(default)]
    pub field_type: Option<String>,
}

/// Entity schema defining the target structure.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EntitySchema {
    /// Schema name
    #[serde(default)]
    pub name: String,

    /// Schema version
    #[serde(default)]
    pub version: String,

    /// Attribute definitions
    #[serde(default)]
    pub attributes: Vec<SchemaAttribute>,
}

/// Schema attribute definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaAttribute {
    /// Attribute name
    pub name: String,

    /// Attribute type (string, number, boolean, etc.)
    #[serde(rename = "type")]
    pub attr_type: String,

    /// Unit of measurement (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,

    /// Whether the attribute is required
    #[serde(default)]
    pub required: bool,
}

/// Configuration store abstraction (Port).
///
/// Defines the interface for accessing stream configuration.
/// Implementations handle different backends (etcd, files, etc.).
///
/// # Methods
///
/// - `list_streams()`: Get all configured stream IDs
/// - `get_config()`: Get full configuration for a stream
/// - `get_enabled_streams()`: Get only enabled streams
/// - `validate()`: Check that the store is accessible
#[cfg_attr(test, automock)]
#[async_trait]
pub trait ConfigStore: Send + Sync {
    /// List all configured stream IDs.
    ///
    /// Scans etcd for stream configurations and returns their IDs.
    ///
    /// # Returns
    ///
    /// Vector of stream IDs (e.g., ["air-quality", "outdoor-weather"])
    ///
    /// # Errors
    ///
    /// Returns `McpError::EtcdUnavailable` if etcd cannot be reached.
    async fn list_streams(&self) -> McpResult<Vec<String>>;

    /// Get configuration for a specific stream.
    ///
    /// Reads all configuration values for the stream from etcd
    /// and parses them into a `StreamConfig` struct.
    ///
    /// # Arguments
    ///
    /// * `stream_id` - The stream identifier
    ///
    /// # Returns
    ///
    /// Full stream configuration including field mappings and entity schema.
    ///
    /// # Errors
    ///
    /// - `McpError::StreamNotFound` if the stream is not configured
    /// - `McpError::EtcdUnavailable` if etcd cannot be reached
    async fn get_config(&self, stream_id: &str) -> McpResult<StreamConfig>;

    /// Get only enabled streams.
    ///
    /// Returns configurations for streams where `enabled = true`.
    ///
    /// # Returns
    ///
    /// Vector of enabled stream configurations.
    async fn get_enabled_streams(&self) -> McpResult<Vec<StreamConfig>>;

    /// Validate that the configuration store is accessible.
    ///
    /// Performs a health check on etcd.
    ///
    /// # Errors
    ///
    /// Returns `McpError::EtcdUnavailable` if etcd is not accessible.
    async fn validate(&self) -> McpResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::McpError;

    // ========== TYPE TESTS ==========

    #[test]
    fn test_stream_config_default() {
        let config = StreamConfig::default();
        assert!(config.stream_id.is_empty());
        assert!(!config.enabled);
        assert!(config.field_mappings.is_empty());
    }

    #[test]
    fn test_field_mapping_serialization() {
        let mapping = FieldMapping {
            source: "main.temp".to_string(),
            target: Some("temperature".to_string()),
            field_type: Some("number".to_string()),
        };

        let json = serde_json::to_string(&mapping).unwrap();
        assert!(json.contains("main.temp"));
        assert!(json.contains("temperature"));
    }

    #[test]
    fn test_schema_attribute() {
        let attr = SchemaAttribute {
            name: "pm25".to_string(),
            attr_type: "number".to_string(),
            unit: Some("µg/m³".to_string()),
            required: true,
        };

        let json = serde_json::to_string(&attr).unwrap();
        assert!(json.contains("pm25"));
        assert!(json.contains("number"));
    }

    // ========== LONDON SCHOOL TDD: BEHAVIOR VERIFICATION TESTS ==========

    #[tokio::test]
    async fn test_list_stream_ids_returns_all_streams() {
        let mut mock = MockConfigStore::new();

        mock.expect_list_streams().times(1).returning(|| {
            Ok(vec![
                "air-quality".to_string(),
                "outdoor-weather".to_string(),
                "nws-forecast".to_string(),
            ])
        });

        let result = mock.list_streams().await;
        assert!(result.is_ok());
        let streams = result.unwrap();
        assert_eq!(streams.len(), 3);
        assert!(streams.contains(&"air-quality".to_string()));
        assert!(streams.contains(&"outdoor-weather".to_string()));
    }

    #[tokio::test]
    async fn test_list_stream_ids_returns_empty_when_none_configured() {
        let mut mock = MockConfigStore::new();

        mock.expect_list_streams().times(1).returning(|| Ok(vec![]));

        let result = mock.list_streams().await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_list_stream_ids_propagates_etcd_error() {
        let mut mock = MockConfigStore::new();

        mock.expect_list_streams()
            .times(1)
            .returning(|| Err(McpError::EtcdUnavailable("Connection refused".to_string())));

        let result = mock.list_streams().await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::EtcdUnavailable(_)));
    }

    #[tokio::test]
    async fn test_get_stream_config_returns_full_config() {
        let mut mock = MockConfigStore::new();

        mock.expect_get_config()
            .with(mockall::predicate::eq("air-quality"))
            .times(1)
            .returning(|stream_id| {
                Ok(StreamConfig {
                    stream_id: stream_id.to_string(),
                    enabled: true,
                    source_type: "mqtt".to_string(),
                    field_mappings: vec![FieldMapping {
                        source: "pm25".to_string(),
                        target: Some("pm25".to_string()),
                        field_type: Some("number".to_string()),
                    }],
                    entity_schema: EntitySchema {
                        name: "AirGradient".to_string(),
                        version: "1.0".to_string(),
                        attributes: vec![SchemaAttribute {
                            name: "pm25".to_string(),
                            attr_type: "number".to_string(),
                            unit: Some("µg/m³".to_string()),
                            required: true,
                        }],
                    },
                    raw_config: HashMap::new(),
                })
            });

        let result = mock.get_config("air-quality").await;
        assert!(result.is_ok());
        let config = result.unwrap();
        assert_eq!(config.stream_id, "air-quality");
        assert!(config.enabled);
        assert_eq!(config.source_type, "mqtt");
        assert_eq!(config.field_mappings.len(), 1);
        assert_eq!(config.entity_schema.attributes.len(), 1);
    }

    #[tokio::test]
    async fn test_get_stream_config_returns_not_found_for_unknown() {
        let mut mock = MockConfigStore::new();

        mock.expect_get_config()
            .with(mockall::predicate::eq("unknown-stream"))
            .times(1)
            .returning(|stream_id| Err(McpError::StreamNotFound(stream_id.to_string())));

        let result = mock.get_config("unknown-stream").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, McpError::StreamNotFound(_)));
        assert!(err.to_string().contains("unknown-stream"));
    }

    #[tokio::test]
    async fn test_get_enabled_streams_returns_only_enabled() {
        let mut mock = MockConfigStore::new();

        mock.expect_get_enabled_streams().times(1).returning(|| {
            Ok(vec![
                StreamConfig {
                    stream_id: "air-quality".to_string(),
                    enabled: true,
                    ..Default::default()
                },
                StreamConfig {
                    stream_id: "outdoor-weather".to_string(),
                    enabled: true,
                    ..Default::default()
                },
            ])
        });

        let result = mock.get_enabled_streams().await;
        assert!(result.is_ok());
        let streams = result.unwrap();
        assert_eq!(streams.len(), 2);
        assert!(streams.iter().all(|s| s.enabled));
    }

    #[tokio::test]
    async fn test_health_check_success() {
        let mut mock = MockConfigStore::new();

        mock.expect_validate().times(1).returning(|| Ok(()));

        let result = mock.validate().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_health_check_etcd_unavailable() {
        let mut mock = MockConfigStore::new();

        mock.expect_validate()
            .times(1)
            .returning(|| Err(McpError::EtcdUnavailable("Connection timeout".to_string())));

        let result = mock.validate().await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), McpError::EtcdUnavailable(_)));
    }

    // ========== WORKFLOW TESTS ==========

    #[tokio::test]
    async fn test_workflow_validate_then_list_then_get_config() {
        let mut mock = MockConfigStore::new();
        let mut seq = mockall::Sequence::new();

        // Step 1: Validate connection
        mock.expect_validate()
            .times(1)
            .in_sequence(&mut seq)
            .returning(|| Ok(()));

        // Step 2: List streams
        mock.expect_list_streams()
            .times(1)
            .in_sequence(&mut seq)
            .returning(|| Ok(vec!["air-quality".to_string()]));

        // Step 3: Get config for discovered stream
        mock.expect_get_config()
            .with(mockall::predicate::eq("air-quality"))
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| {
                Ok(StreamConfig {
                    stream_id: "air-quality".to_string(),
                    enabled: true,
                    ..Default::default()
                })
            });

        // Execute workflow
        mock.validate().await.unwrap();
        let streams = mock.list_streams().await.unwrap();
        assert_eq!(streams.len(), 1);

        let config = mock.get_config("air-quality").await.unwrap();
        assert_eq!(config.stream_id, "air-quality");
    }

    // ========== TYPE BUILDER TESTS ==========

    #[test]
    fn test_stream_config_with_field_mappings() {
        let config = StreamConfig {
            stream_id: "test".to_string(),
            enabled: true,
            source_type: "http".to_string(),
            field_mappings: vec![
                FieldMapping {
                    source: "main.temp".to_string(),
                    target: Some("temperature".to_string()),
                    field_type: Some("number".to_string()),
                },
                FieldMapping {
                    source: "main.humidity".to_string(),
                    target: Some("humidity".to_string()),
                    field_type: Some("number".to_string()),
                },
            ],
            entity_schema: EntitySchema::default(),
            raw_config: HashMap::new(),
        };

        assert_eq!(config.field_mappings.len(), 2);
        assert_eq!(config.field_mappings[0].source, "main.temp");
        assert_eq!(
            config.field_mappings[1].target,
            Some("humidity".to_string())
        );
    }

    #[test]
    fn test_entity_schema_with_attributes() {
        let schema = EntitySchema {
            name: "Weather".to_string(),
            version: "2.0".to_string(),
            attributes: vec![
                SchemaAttribute {
                    name: "temperature".to_string(),
                    attr_type: "number".to_string(),
                    unit: Some("celsius".to_string()),
                    required: true,
                },
                SchemaAttribute {
                    name: "humidity".to_string(),
                    attr_type: "number".to_string(),
                    unit: Some("percent".to_string()),
                    required: false,
                },
            ],
        };

        assert_eq!(schema.name, "Weather");
        assert_eq!(schema.attributes.len(), 2);
        assert!(schema.attributes[0].required);
        assert!(!schema.attributes[1].required);
    }

    #[test]
    fn test_stream_config_serialization() {
        let config = StreamConfig {
            stream_id: "air-quality".to_string(),
            enabled: true,
            source_type: "mqtt".to_string(),
            field_mappings: vec![FieldMapping {
                source: "pm25".to_string(),
                target: None,
                field_type: None,
            }],
            entity_schema: EntitySchema {
                name: "AirQuality".to_string(),
                version: "1.0".to_string(),
                attributes: vec![],
            },
            raw_config: HashMap::new(),
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("air-quality"));
        assert!(json.contains("mqtt"));
        assert!(json.contains("pm25"));
    }
}
