//! StreamRegistry Adapter
//!
//! Adapts the config-client's StreamRegistry to implement the MCP ConfigStore trait.
//! This follows the Domain Adapter pattern, bridging the neural_core::StreamConfig
//! types to the MCP-specific StreamConfig types.
//!
//! # Usage
//!
//! ```ignore
//! use config_client::StreamRegistry;
//! use ndp_mcp_server::etcd::StreamRegistryAdapter;
//!
//! let registry = StreamRegistry::new(&["http://localhost:2379"]).await?;
//! let adapter = StreamRegistryAdapter::new(registry);
//!
//! // Use adapter anywhere ConfigStore trait is expected
//! let streams = adapter.list_streams().await?;
//! ```

use async_trait::async_trait;
use config_client::StreamRegistry;
use tracing::{debug, info, warn};

use super::{ConfigStore, EntitySchema, FieldMapping, SchemaAttribute, StreamConfig};
use crate::error::{McpError, McpResult};

/// Adapter that wraps config-client's StreamRegistry to implement ConfigStore.
///
/// This allows the MCP server to use the shared config-client crate while
/// maintaining compatibility with the MCP-specific StreamConfig types.
pub struct StreamRegistryAdapter {
    /// The underlying StreamRegistry from config-client
    registry: StreamRegistry,
}

impl StreamRegistryAdapter {
    /// Create a new adapter wrapping a StreamRegistry.
    pub fn new(registry: StreamRegistry) -> Self {
        Self { registry }
    }

    /// Convert neural_core::StreamConfig to MCP StreamConfig.
    ///
    /// Maps fields between the two config types:
    /// - stream_id, enabled -> direct copy
    /// - sources[0].source_type -> source_type string
    /// - fields -> field_mappings (name -> source/target)
    /// - entity_schemas -> entity_schema (if present in config)
    fn convert_config(core_config: &neural_core::StreamConfig) -> StreamConfig {
        let mut mcp_config = StreamConfig {
            stream_id: core_config.stream_id.clone(),
            enabled: core_config.enabled,
            source_type: core_config
                .sources
                .first()
                .map(|s| format!("{:?}", s.source_type).to_lowercase())
                .unwrap_or_default(),
            field_mappings: Vec::new(),
            entity_schema: EntitySchema::default(),
            raw_config: std::collections::HashMap::new(),
        };

        // Convert fields to field mappings
        mcp_config.field_mappings = core_config
            .fields
            .iter()
            .map(|field| FieldMapping {
                source: field.name.clone(),
                target: Some(field.name.clone()),
                field_type: Some(format!("{:?}", field.field_type).to_lowercase()),
            })
            .collect();

        // Build entity schema from description and version
        mcp_config.entity_schema = EntitySchema {
            name: core_config.description.clone(),
            version: core_config.version.clone(),
            attributes: core_config
                .fields
                .iter()
                .map(|field| SchemaAttribute {
                    name: field.name.clone(),
                    attr_type: format!("{:?}", field.field_type).to_lowercase(),
                    unit: field.unit.clone(),
                    required: !field.nullable,
                })
                .collect(),
        };

        mcp_config
    }
}

#[async_trait]
impl ConfigStore for StreamRegistryAdapter {
    async fn list_streams(&self) -> McpResult<Vec<String>> {
        debug!("StreamRegistryAdapter: listing streams");

        self.registry.list_streams().await.map_err(|e| {
            warn!(error = %e, "Failed to list streams via StreamRegistry");
            McpError::EtcdUnavailable(format!("StreamRegistry list_streams failed: {}", e))
        })
    }

    async fn get_config(&self, stream_id: &str) -> McpResult<StreamConfig> {
        debug!(stream_id = %stream_id, "StreamRegistryAdapter: getting config");

        let core_config = self
            .registry
            .load_stream(stream_id)
            .await
            .map_err(|e| match e {
                config_client::ConfigError::NotFound(_) => {
                    McpError::StreamNotFound(stream_id.to_string())
                }
                _ => {
                    warn!(error = %e, stream_id = %stream_id, "Failed to load stream config");
                    McpError::EtcdUnavailable(format!("StreamRegistry load_stream failed: {}", e))
                }
            })?;

        let mcp_config = Self::convert_config(&core_config);
        debug!(
            stream_id = %stream_id,
            enabled = mcp_config.enabled,
            source_type = %mcp_config.source_type,
            "Converted neural_core::StreamConfig to MCP StreamConfig"
        );

        Ok(mcp_config)
    }

    async fn get_enabled_streams(&self) -> McpResult<Vec<StreamConfig>> {
        debug!("StreamRegistryAdapter: getting enabled streams");

        let all_configs = self.registry.load_all_streams().await.map_err(|e| {
            warn!(error = %e, "Failed to load all streams via StreamRegistry");
            McpError::EtcdUnavailable(format!("StreamRegistry load_all_streams failed: {}", e))
        })?;

        let enabled_configs: Vec<StreamConfig> = all_configs
            .values()
            .filter(|c| c.enabled)
            .map(Self::convert_config)
            .collect();

        info!(
            count = enabled_configs.len(),
            "Found enabled streams via StreamRegistry"
        );
        Ok(enabled_configs)
    }

    async fn validate(&self) -> McpResult<()> {
        debug!("StreamRegistryAdapter: validating connection");

        // Validate by attempting to list streams
        // This exercises the full etcd connection path
        self.registry.list_streams().await.map(|_| ()).map_err(|e| {
            warn!(error = %e, "StreamRegistry validation failed");
            McpError::EtcdUnavailable(format!("StreamRegistry validation failed: {}", e))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neural_core::{FieldType, SchemaField, SourceConfig, SourceType};

    /// Helper to create a test neural_core::StreamConfig
    fn create_test_core_config(stream_id: &str) -> neural_core::StreamConfig {
        neural_core::StreamConfig {
            stream_id: stream_id.to_string(),
            description: "Test Stream".to_string(),
            version: "1.0.0".to_string(),
            enabled: true,
            retention_days: 30,
            compression_after_days: 7,
            partitioning_strategy: "daily".to_string(),
            fields: vec![
                SchemaField::new("pm25".to_string(), FieldType::Float)
                    .required()
                    .with_unit("ug/m3".to_string()),
                SchemaField::new("temperature".to_string(), FieldType::Float)
                    .with_unit("celsius".to_string()),
            ],
            sources: vec![SourceConfig {
                source_type: SourceType::Mqtt,
                enabled: true,
                ndp_id: None,
                context: None,
                params: std::collections::HashMap::new(),
            }],
            storage: None,
        }
    }

    #[test]
    fn test_convert_config_basic_fields() {
        let core_config = create_test_core_config("test-stream");
        let mcp_config = StreamRegistryAdapter::convert_config(&core_config);

        assert_eq!(mcp_config.stream_id, "test-stream");
        assert!(mcp_config.enabled);
        assert_eq!(mcp_config.source_type, "mqtt");
    }

    #[test]
    fn test_convert_config_field_mappings() {
        let core_config = create_test_core_config("test-stream");
        let mcp_config = StreamRegistryAdapter::convert_config(&core_config);

        assert_eq!(mcp_config.field_mappings.len(), 2);
        assert_eq!(mcp_config.field_mappings[0].source, "pm25");
        assert_eq!(
            mcp_config.field_mappings[0].target,
            Some("pm25".to_string())
        );
        assert_eq!(
            mcp_config.field_mappings[0].field_type,
            Some("float".to_string())
        );
    }

    #[test]
    fn test_convert_config_entity_schema() {
        let core_config = create_test_core_config("test-stream");
        let mcp_config = StreamRegistryAdapter::convert_config(&core_config);

        assert_eq!(mcp_config.entity_schema.name, "Test Stream");
        assert_eq!(mcp_config.entity_schema.version, "1.0.0");
        assert_eq!(mcp_config.entity_schema.attributes.len(), 2);

        let pm25_attr = &mcp_config.entity_schema.attributes[0];
        assert_eq!(pm25_attr.name, "pm25");
        assert_eq!(pm25_attr.attr_type, "float");
        assert_eq!(pm25_attr.unit, Some("ug/m3".to_string()));
        assert!(pm25_attr.required); // field was marked as required (not nullable)
    }

    #[test]
    fn test_convert_config_disabled_stream() {
        let mut core_config = create_test_core_config("test-stream");
        core_config.enabled = false;

        let mcp_config = StreamRegistryAdapter::convert_config(&core_config);
        assert!(!mcp_config.enabled);
    }

    #[test]
    fn test_convert_config_http_poll_source() {
        let mut core_config = create_test_core_config("test-stream");
        core_config.sources[0].source_type = SourceType::HttpPoll;

        let mcp_config = StreamRegistryAdapter::convert_config(&core_config);
        assert_eq!(mcp_config.source_type, "httppoll");
    }
}
