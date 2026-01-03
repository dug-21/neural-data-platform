//! etcd Client Implementation.
//!
//! Implements the `ConfigStore` trait using the etcd-client crate.
//! Reads stream configurations from the `/streams/` prefix.

use std::collections::HashMap;

use async_trait::async_trait;
use etcd_client::{Client, GetOptions};
use tracing::{debug, error, info, warn};

use super::{
    ConfigStore, EntitySchema, FieldMapping, SchemaAttribute, StreamConfig,
};
use crate::error::{McpError, McpResult};

/// etcd configuration store implementation.
///
/// Connects to etcd and reads stream configurations from the `/streams/` prefix.
pub struct EtcdConfigStore {
    /// etcd endpoints (e.g., ["http://localhost:2379"])
    endpoints: Vec<String>,
    /// Prefix for stream configurations
    prefix: String,
}

impl EtcdConfigStore {
    /// Create a new etcd configuration store.
    ///
    /// # Arguments
    ///
    /// * `endpoints` - etcd server endpoints
    ///
    /// # Example
    ///
    /// ```ignore
    /// let store = EtcdConfigStore::new(vec!["http://localhost:2379".to_string()]);
    /// ```
    pub fn new(endpoints: Vec<String>) -> Self {
        Self {
            endpoints,
            prefix: "/streams/".to_string(),
        }
    }

    /// Create with a custom prefix.
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = prefix.into();
        self
    }

    /// Connect to etcd and return a client.
    async fn connect(&self) -> McpResult<Client> {
        Client::connect(self.endpoints.clone(), None)
            .await
            .map_err(|e| {
                error!(error = %e, "Failed to connect to etcd");
                McpError::EtcdUnavailable(format!("Connection failed: {}", e))
            })
    }

    /// Parse raw etcd values into StreamConfig.
    fn parse_stream_config(
        stream_id: &str,
        raw_values: HashMap<String, String>,
    ) -> StreamConfig {
        let mut config = StreamConfig {
            stream_id: stream_id.to_string(),
            raw_config: raw_values.clone(),
            ..Default::default()
        };

        // Parse enabled flag
        if let Some(enabled) = raw_values.get("enabled") {
            config.enabled = enabled.to_lowercase() == "true";
        }

        // Parse source type
        if let Some(source_type) = raw_values.get("source_type") {
            config.source_type = source_type.clone();
        }

        // Parse field mappings
        config.field_mappings = Self::parse_field_mappings(&raw_values);

        // Parse entity schema
        config.entity_schema = Self::parse_entity_schema(stream_id, &raw_values);

        config
    }

    /// Parse field mappings from raw config values.
    fn parse_field_mappings(raw: &HashMap<String, String>) -> Vec<FieldMapping> {
        let mut mappings = Vec::new();
        let mut index = 0;

        loop {
            let source_key = format!("parser/field_mappings/{}/source", index);
            if let Some(source) = raw.get(&source_key) {
                let target_key = format!("parser/field_mappings/{}/target", index);
                let type_key = format!("parser/field_mappings/{}/type", index);

                mappings.push(FieldMapping {
                    source: source.clone(),
                    target: raw.get(&target_key).cloned(),
                    field_type: raw.get(&type_key).cloned(),
                });
                index += 1;
            } else {
                break;
            }
        }

        mappings
    }

    /// Parse entity schema from raw config values.
    fn parse_entity_schema(stream_id: &str, raw: &HashMap<String, String>) -> EntitySchema {
        let mut schema = EntitySchema {
            name: raw
                .get("entity_schema/name")
                .cloned()
                .unwrap_or_else(|| stream_id.to_string()),
            version: raw
                .get("entity_schema/version")
                .cloned()
                .unwrap_or_else(|| "1.0".to_string()),
            ..Default::default()
        };

        // Parse attributes
        let mut index = 0;
        loop {
            let name_key = format!("entity_schema/attributes/{}/name", index);
            if let Some(name) = raw.get(&name_key) {
                let type_key = format!("entity_schema/attributes/{}/type", index);
                let unit_key = format!("entity_schema/attributes/{}/unit", index);
                let required_key = format!("entity_schema/attributes/{}/required", index);

                schema.attributes.push(SchemaAttribute {
                    name: name.clone(),
                    attr_type: raw
                        .get(&type_key)
                        .cloned()
                        .unwrap_or_else(|| "string".to_string()),
                    unit: raw.get(&unit_key).cloned(),
                    required: raw
                        .get(&required_key)
                        .map(|v| v.to_lowercase() == "true")
                        .unwrap_or(false),
                });
                index += 1;
            } else {
                break;
            }
        }

        schema
    }
}

#[async_trait]
impl ConfigStore for EtcdConfigStore {
    async fn list_streams(&self) -> McpResult<Vec<String>> {
        let mut client = self.connect().await?;

        debug!(prefix = %self.prefix, "Listing streams from etcd");

        // Get all keys under /streams/
        let options = GetOptions::new().with_prefix();
        let response = client
            .get(self.prefix.as_bytes(), Some(options))
            .await
            .map_err(|e| {
                error!(error = %e, "Failed to list streams from etcd");
                McpError::EtcdUnavailable(format!("List failed: {}", e))
            })?;

        // Extract unique stream IDs from keys
        let mut stream_ids: Vec<String> = Vec::new();
        for kv in response.kvs() {
            let key = String::from_utf8_lossy(kv.key());
            // Key format: /streams/{stream_id}/{setting}
            if let Some(rest) = key.strip_prefix(&self.prefix) {
                if let Some(stream_id) = rest.split('/').next() {
                    if !stream_id.is_empty() && !stream_ids.contains(&stream_id.to_string()) {
                        stream_ids.push(stream_id.to_string());
                    }
                }
            }
        }

        info!(count = stream_ids.len(), "Found streams in etcd");
        Ok(stream_ids)
    }

    async fn get_config(&self, stream_id: &str) -> McpResult<StreamConfig> {
        let mut client = self.connect().await?;

        let prefix = format!("{}{}/", self.prefix, stream_id);
        debug!(prefix = %prefix, "Getting stream config from etcd");

        // Get all keys for this stream
        let options = GetOptions::new().with_prefix();
        let response = client
            .get(prefix.as_bytes(), Some(options))
            .await
            .map_err(|e| {
                error!(error = %e, stream_id = %stream_id, "Failed to get stream config");
                McpError::EtcdUnavailable(format!("Get config failed: {}", e))
            })?;

        if response.kvs().is_empty() {
            warn!(stream_id = %stream_id, "Stream not found in etcd");
            return Err(McpError::StreamNotFound(stream_id.to_string()));
        }

        // Parse key-value pairs into a map
        let mut raw_values: HashMap<String, String> = HashMap::new();
        for kv in response.kvs() {
            let key = String::from_utf8_lossy(kv.key());
            let value = String::from_utf8_lossy(kv.value());

            // Strip the prefix to get the relative key
            if let Some(relative_key) = key.strip_prefix(&prefix) {
                raw_values.insert(relative_key.to_string(), value.to_string());
            }
        }

        let config = Self::parse_stream_config(stream_id, raw_values);
        debug!(stream_id = %stream_id, enabled = config.enabled, "Parsed stream config");

        Ok(config)
    }

    async fn get_enabled_streams(&self) -> McpResult<Vec<StreamConfig>> {
        let stream_ids = self.list_streams().await?;
        let mut enabled_streams = Vec::new();

        for stream_id in stream_ids {
            match self.get_config(&stream_id).await {
                Ok(config) if config.enabled => {
                    enabled_streams.push(config);
                }
                Ok(_) => {
                    debug!(stream_id = %stream_id, "Stream is disabled");
                }
                Err(e) => {
                    warn!(stream_id = %stream_id, error = %e, "Failed to get stream config");
                }
            }
        }

        info!(count = enabled_streams.len(), "Found enabled streams");
        Ok(enabled_streams)
    }

    async fn validate(&self) -> McpResult<()> {
        let mut client = self.connect().await?;

        // Try to get a key to verify connectivity
        client
            .get("/__health_check__", None)
            .await
            .map_err(|e| McpError::EtcdUnavailable(format!("Health check failed: {}", e)))?;

        info!("etcd connection validated");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_etcd_config_store_new() {
        let store = EtcdConfigStore::new(vec!["http://localhost:2379".to_string()]);
        assert_eq!(store.endpoints, vec!["http://localhost:2379"]);
        assert_eq!(store.prefix, "/streams/");
    }

    #[test]
    fn test_etcd_config_store_with_prefix() {
        let store = EtcdConfigStore::new(vec!["http://localhost:2379".to_string()])
            .with_prefix("/custom/");
        assert_eq!(store.prefix, "/custom/");
    }

    #[test]
    fn test_parse_stream_config_enabled() {
        let mut raw = HashMap::new();
        raw.insert("enabled".to_string(), "true".to_string());
        raw.insert("source_type".to_string(), "http".to_string());

        let config = EtcdConfigStore::parse_stream_config("test-stream", raw);
        assert_eq!(config.stream_id, "test-stream");
        assert!(config.enabled);
        assert_eq!(config.source_type, "http");
    }

    #[test]
    fn test_parse_stream_config_disabled() {
        let mut raw = HashMap::new();
        raw.insert("enabled".to_string(), "false".to_string());

        let config = EtcdConfigStore::parse_stream_config("test-stream", raw);
        assert!(!config.enabled);
    }

    #[test]
    fn test_parse_field_mappings() {
        let mut raw = HashMap::new();
        raw.insert("parser/field_mappings/0/source".to_string(), "pm25".to_string());
        raw.insert("parser/field_mappings/0/target".to_string(), "pm25_value".to_string());
        raw.insert("parser/field_mappings/0/type".to_string(), "number".to_string());
        raw.insert("parser/field_mappings/1/source".to_string(), "temp".to_string());

        let mappings = EtcdConfigStore::parse_field_mappings(&raw);
        assert_eq!(mappings.len(), 2);
        assert_eq!(mappings[0].source, "pm25");
        assert_eq!(mappings[0].target, Some("pm25_value".to_string()));
        assert_eq!(mappings[1].source, "temp");
        assert!(mappings[1].target.is_none());
    }

    #[test]
    fn test_parse_entity_schema() {
        let mut raw = HashMap::new();
        raw.insert("entity_schema/name".to_string(), "AirQuality".to_string());
        raw.insert("entity_schema/version".to_string(), "2.0".to_string());
        raw.insert("entity_schema/attributes/0/name".to_string(), "pm25".to_string());
        raw.insert("entity_schema/attributes/0/type".to_string(), "number".to_string());
        raw.insert("entity_schema/attributes/0/unit".to_string(), "µg/m³".to_string());
        raw.insert("entity_schema/attributes/0/required".to_string(), "true".to_string());

        let schema = EtcdConfigStore::parse_entity_schema("air-quality", &raw);
        assert_eq!(schema.name, "AirQuality");
        assert_eq!(schema.version, "2.0");
        assert_eq!(schema.attributes.len(), 1);
        assert_eq!(schema.attributes[0].name, "pm25");
        assert_eq!(schema.attributes[0].unit, Some("µg/m³".to_string()));
        assert!(schema.attributes[0].required);
    }
}
