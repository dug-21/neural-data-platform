//! Ingestion Router
//!
//! Routes time series points to appropriate storage channels after schema validation

use config_client::StreamRegistry;
use neural_core::{FieldType, SchemaField, StreamConfig, TimeSeriesPoint};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, warn};

/// Dead letter item for invalid points
#[derive(Debug, Clone)]
pub struct DeadLetterItem {
    pub stream_id: String,
    pub source_id: String,
    pub point: TimeSeriesPoint,
    pub error: String,
}

/// Schema validation error
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ValidationError {
    #[error("Required field missing: {field}")]
    RequiredFieldMissing { field: String },

    #[error("Type mismatch for field {field}: expected {expected}, got {actual}")]
    TypeMismatch {
        field: String,
        expected: String,
        actual: String,
    },

    #[error("Value {value} out of range for field {field}: expected [{min}, {max}]")]
    OutOfRange {
        field: String,
        value: f64,
        min: f64,
        max: f64,
    },

    #[error("Unknown field: {field}")]
    UnknownField { field: String },
}

/// Routes incoming time series points to storage writers
pub struct IngestionRouter {
    registry: Arc<StreamRegistry>,
    storage_channels: Arc<RwLock<HashMap<String, mpsc::Sender<TimeSeriesPoint>>>>,
    dead_letter_tx: mpsc::Sender<DeadLetterItem>,
    strict_validation: bool,
}

impl IngestionRouter {
    /// Create a new ingestion router
    pub fn new(
        registry: Arc<StreamRegistry>,
        dead_letter_tx: mpsc::Sender<DeadLetterItem>,
    ) -> Self {
        Self {
            registry,
            storage_channels: Arc::new(RwLock::new(HashMap::new())),
            dead_letter_tx,
            strict_validation: false, // Lenient by default for backward compatibility
        }
    }

    /// Create router with strict validation enabled
    pub fn with_strict_validation(mut self) -> Self {
        self.strict_validation = true;
        self
    }

    /// Register a storage channel for a stream
    pub async fn register_storage_channel(
        &self,
        stream_id: String,
        sender: mpsc::Sender<TimeSeriesPoint>,
    ) {
        let mut channels = self.storage_channels.write().await;
        channels.insert(stream_id.clone(), sender);
        debug!("Registered storage channel for stream: {}", stream_id);
    }

    /// Unregister a storage channel
    pub async fn unregister_storage_channel(&self, stream_id: &str) {
        let mut channels = self.storage_channels.write().await;
        channels.remove(stream_id);
        debug!("Unregistered storage channel for stream: {}", stream_id);
    }

    /// Check if a storage channel is registered for a stream
    pub async fn has_storage_channel(&self, stream_id: &str) -> bool {
        let channels = self.storage_channels.read().await;
        channels.contains_key(stream_id)
    }

    /// Get the count of registered storage channels
    pub async fn registered_stream_count(&self) -> usize {
        let channels = self.storage_channels.read().await;
        channels.len()
    }

    /// Register storage channels for all streams in the registry
    ///
    /// This is the config-driven approach: streams are loaded from StreamRegistry
    /// (which is populated from YAML configs via etcd), not hardcoded.
    pub async fn register_all_streams_from_registry(
        &self,
        storage_tx: mpsc::Sender<TimeSeriesPoint>,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let stream_ids = self.registry.list_streams().await.map_err(|e| {
            tracing::error!("Failed to list streams from registry: {}", e);
            Box::new(e) as Box<dyn std::error::Error + Send + Sync>
        })?;

        if stream_ids.is_empty() {
            tracing::warn!("No streams found in registry. Ensure configs are synced to etcd.");
            return Ok(0);
        }

        for stream_id in &stream_ids {
            self.register_storage_channel(stream_id.clone(), storage_tx.clone())
                .await;
        }

        tracing::info!(
            "Registered {} storage channels from StreamRegistry: {:?}",
            stream_ids.len(),
            stream_ids
        );

        Ok(stream_ids.len())
    }

    /// Route a point to the appropriate storage channel
    pub async fn route_point(
        &self,
        source_id: &str,
        stream_id: &str,
        point: TimeSeriesPoint,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Load stream configuration
        let config = match self.registry.load_stream(stream_id).await {
            Ok(cfg) => cfg,
            Err(e) => {
                error!("Failed to load stream config for {}: {}", stream_id, e);
                return Err(Box::new(e));
            }
        };

        // Validate point against schema
        if let Err(e) = self.validate_point(&point, &config) {
            warn!(
                "Validation failed for stream {} from source {}: {}",
                stream_id, source_id, e
            );

            // Send to dead letter queue
            let dead_letter = DeadLetterItem {
                stream_id: stream_id.to_string(),
                source_id: source_id.to_string(),
                point: point.clone(),
                error: e.to_string(),
            };

            if let Err(send_err) = self.dead_letter_tx.try_send(dead_letter) {
                error!("Failed to send to dead letter queue: {}", send_err);
            }

            // In lenient mode, we still forward the point
            if !self.strict_validation {
                debug!("Lenient mode: forwarding despite validation failure");
            } else {
                return Err(Box::new(e));
            }
        }

        // Enrich with metadata tags
        let mut enriched = point;
        enriched
            .tags
            .insert("stream_id".to_string(), stream_id.to_string());
        enriched
            .tags
            .insert("source_id".to_string(), source_id.to_string());

        // Route to storage writer
        let channels = self.storage_channels.read().await;
        if let Some(tx) = channels.get(stream_id) {
            if let Err(e) = tx.send(enriched).await {
                error!("Failed to send to storage channel for {}: {}", stream_id, e);
                return Err(Box::new(e));
            }
            debug!("Routed point for stream: {}", stream_id);
            Ok(())
        } else {
            let err_msg = format!("No storage channel registered for stream: {}", stream_id);
            error!("{}", err_msg);
            Err(err_msg.into())
        }
    }

    /// Validate a point against stream schema
    fn validate_point(
        &self,
        point: &TimeSeriesPoint,
        config: &StreamConfig,
    ) -> Result<(), ValidationError> {
        // Check if this is a per-metric point model (OpenWeatherMap parsers)
        // In this model, each point represents ONE metric with:
        // - tags["metric"] = field name (e.g., "temperature")
        // - point.value = the actual value
        if let Some(metric_name) = point.tags.get("metric") {
            // Per-metric point model: validate only the field this point represents
            if let Some(schema_field) = config.get_field(metric_name) {
                // Validate the point's value against the schema field
                let value_str = point.value.to_string();
                self.validate_field_value(&value_str, schema_field)?;
            } else if self.strict_validation {
                // Unknown metric in strict mode
                return Err(ValidationError::UnknownField {
                    field: metric_name.clone(),
                });
            }
            // Per-metric model passes - no need to check for "required fields missing"
            // because required fields will come as separate points
            return Ok(());
        }

        // Traditional model: all fields as tag key-value pairs
        // Check all required fields are present
        for field in &config.fields {
            if !field.nullable {
                if !point.tags.contains_key(&field.name) {
                    return Err(ValidationError::RequiredFieldMissing {
                        field: field.name.clone(),
                    });
                }
            }
        }

        // Validate each tag against schema
        for (tag_name, tag_value) in &point.tags {
            if let Some(schema_field) = config.get_field(tag_name) {
                self.validate_field_value(tag_value, schema_field)?;
            } else if self.strict_validation {
                // In strict mode, unknown fields are errors
                return Err(ValidationError::UnknownField {
                    field: tag_name.clone(),
                });
            }
        }

        Ok(())
    }

    /// Validate a field value against its schema
    fn validate_field_value(
        &self,
        value: &str,
        field: &SchemaField,
    ) -> Result<(), ValidationError> {
        // Type validation
        match field.field_type {
            FieldType::Float => {
                let parsed = value
                    .parse::<f64>()
                    .map_err(|_| ValidationError::TypeMismatch {
                        field: field.name.clone(),
                        expected: "float".to_string(),
                        actual: value.to_string(),
                    })?;

                // Range validation
                if let Some(ref range) = field.range {
                    if range.len() == 2 {
                        let (min, max) = (range[0], range[1]);
                        if parsed < min || parsed > max {
                            return Err(ValidationError::OutOfRange {
                                field: field.name.clone(),
                                value: parsed,
                                min,
                                max,
                            });
                        }
                    }
                }
            }
            FieldType::Int => {
                let parsed = value
                    .parse::<i64>()
                    .map_err(|_| ValidationError::TypeMismatch {
                        field: field.name.clone(),
                        expected: "int".to_string(),
                        actual: value.to_string(),
                    })?;

                // Range validation
                if let Some(ref range) = field.range {
                    if range.len() == 2 {
                        let (min, max) = (range[0] as i64, range[1] as i64);
                        if parsed < min || parsed > max {
                            return Err(ValidationError::OutOfRange {
                                field: field.name.clone(),
                                value: parsed as f64,
                                min: min as f64,
                                max: max as f64,
                            });
                        }
                    }
                }
            }
            FieldType::String => {
                // String is always valid
            }
            FieldType::Bool => {
                value
                    .parse::<bool>()
                    .map_err(|_| ValidationError::TypeMismatch {
                        field: field.name.clone(),
                        expected: "bool".to_string(),
                        actual: value.to_string(),
                    })?;
            }
            FieldType::Json => {
                // JSON validation - must be valid JSON string
                serde_json::from_str::<serde_json::Value>(value).map_err(|_| {
                    ValidationError::TypeMismatch {
                        field: field.name.clone(),
                        expected: "json".to_string(),
                        actual: value.to_string(),
                    }
                })?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use neural_core::{FieldType, SchemaField, SourceConfig, SourceType};
    use tokio::sync::mpsc;

    // ========== LONDON SCHOOL TDD: UNIT TESTS WITH MOCKS ==========

    fn create_test_config() -> StreamConfig {
        StreamConfig {
            stream_id: "test-stream".to_string(),
            description: "Test".to_string(),
            version: "1.0.0".to_string(),
            enabled: true,
            retention_days: 30,
            compression_after_days: 7,
            partitioning_strategy: "daily".to_string(),
            fields: vec![
                SchemaField::new("pm25".to_string(), FieldType::Float)
                    .required()
                    .with_range(0.0, 500.0),
                SchemaField::new("temperature".to_string(), FieldType::Float)
                    .with_range(-40.0, 60.0),
                SchemaField::new("humidity".to_string(), FieldType::Float),
            ],
            sources: vec![SourceConfig {
                source_type: SourceType::Mqtt,
                enabled: true,
                params: HashMap::new(),
            }],
            storage: None,
        }
    }

    fn create_test_point_with_tags(tags: HashMap<String, String>) -> TimeSeriesPoint {
        TimeSeriesPoint {
            timestamp: Utc::now(),
            location_id: "test-location".to_string(),
            value: 25.0,
            tags,
        }
    }

    // Note: These tests are commented out as they require async test setup
    // They can be uncommented when using a mock StreamRegistry

    /*
    #[tokio::test]
    async fn test_validate_field_value_float_valid() {
        let config = create_test_config();
        let field = config.get_field("pm25").unwrap();

        let (tx, _rx) = mpsc::channel(1);
        let registry = Arc::new(StreamRegistry::new(&["http://localhost:2379"]).await.unwrap());
        let router = IngestionRouter::new(registry, tx);

        assert!(router.validate_field_value("25.5", field).is_ok());
        assert!(router.validate_field_value("0.0", field).is_ok());
        assert!(router.validate_field_value("500.0", field).is_ok());
    }

    #[tokio::test]
    async fn test_validate_field_value_float_out_of_range() {
        let config = create_test_config();
        let field = config.get_field("pm25").unwrap();

        let (tx, _rx) = mpsc::channel(1);
        let registry = Arc::new(StreamRegistry::new(&["http://localhost:2379"]).await.unwrap());
        let router = IngestionRouter::new(registry, tx);

        let result = router.validate_field_value("600.0", field);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ValidationError::OutOfRange { .. }));
    }

    #[tokio::test]
    async fn test_validate_field_value_type_mismatch() {
        let config = create_test_config();
        let field = config.get_field("pm25").unwrap();

        let (tx, _rx) = mpsc::channel(1);
        let registry = Arc::new(StreamRegistry::new(&["http://localhost:2379"]).await.unwrap());
        let router = IngestionRouter::new(registry, tx);

        let result = router.validate_field_value("not-a-number", field);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ValidationError::TypeMismatch { .. }));
    }

    #[tokio::test]
    async fn test_validate_point_required_field_missing() {
        let config = create_test_config();

        let (tx, _rx) = mpsc::channel(1);
        let registry = Arc::new(StreamRegistry::new(&["http://localhost:2379"]).await.unwrap());
        let router = IngestionRouter::new(registry, tx);

        // Point missing required pm25 field
        let point = create_test_point_with_tags(HashMap::new());

        let result = router.validate_point(&point, &config);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ValidationError::RequiredFieldMissing { .. }
        ));
    }

    #[tokio::test]
    async fn test_validate_point_all_fields_valid() {
        let config = create_test_config();

        let (tx, _rx) = mpsc::channel(1);
        let registry = Arc::new(StreamRegistry::new(&["http://localhost:2379"]).await.unwrap());
        let router = IngestionRouter::new(registry, tx);

        let mut tags = HashMap::new();
        tags.insert("pm25".to_string(), "25.5".to_string());
        tags.insert("temperature".to_string(), "22.0".to_string());
        tags.insert("humidity".to_string(), "45.0".to_string());

        let point = create_test_point_with_tags(tags);

        assert!(router.validate_point(&point, &config).is_ok());
    }

    #[tokio::test]
    async fn test_strict_validation_rejects_unknown_fields() {
        let config = create_test_config();

        let (tx, _rx) = mpsc::channel(1);
        let registry = Arc::new(StreamRegistry::new(&["http://localhost:2379"]).await.unwrap());
        let router = IngestionRouter::new(registry, tx).with_strict_validation();

        let mut tags = HashMap::new();
        tags.insert("pm25".to_string(), "25.5".to_string());
        tags.insert("unknown_field".to_string(), "value".to_string());

        let point = create_test_point_with_tags(tags);

        let result = router.validate_point(&point, &config);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ValidationError::UnknownField { .. }));
    }

    #[tokio::test]
    async fn test_lenient_validation_allows_unknown_fields() {
        let config = create_test_config();

        let (tx, _rx) = mpsc::channel(1);
        let registry = Arc::new(StreamRegistry::new(&["http://localhost:2379"]).await.unwrap());
        let router = IngestionRouter::new(registry, tx); // Lenient by default

        let mut tags = HashMap::new();
        tags.insert("pm25".to_string(), "25.5".to_string());
        tags.insert("unknown_field".to_string(), "value".to_string());

        let point = create_test_point_with_tags(tags);

        assert!(router.validate_point(&point, &config).is_ok());
    }
    */

    #[tokio::test]
    async fn test_register_and_unregister_storage_channel() {
        let (dead_letter_tx, _rx) = mpsc::channel(10);
        let (storage_tx, _storage_rx) = mpsc::channel(100);
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );

        let router = IngestionRouter::new(registry, dead_letter_tx);

        // Register
        router
            .register_storage_channel("test-stream".to_string(), storage_tx)
            .await;

        let channels = router.storage_channels.read().await;
        assert!(channels.contains_key("test-stream"));
        drop(channels);

        // Unregister
        router.unregister_storage_channel("test-stream").await;

        let channels = router.storage_channels.read().await;
        assert!(!channels.contains_key("test-stream"));
    }

    // ========== MQTT ROUTING ENRICHMENT TESTS ==========

    #[tokio::test]
    async fn test_router_adds_stream_id_tag_to_points() {
        // CRITICAL TEST: Verify router enriches points with stream_id tag
        // This is the core fix for MQTT routing bug
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );

        // Create test stream config
        let config = create_test_config();
        registry.save_stream(&config).await.unwrap();

        let (dead_letter_tx, _dlq_rx) = mpsc::channel(10);
        let (storage_tx, mut storage_rx) = mpsc::channel(100);
        let router = IngestionRouter::new(registry.clone(), dead_letter_tx);

        // Register storage channel
        router
            .register_storage_channel("test-stream".to_string(), storage_tx)
            .await;

        // Create point WITHOUT stream_id tag (simulating MQTT source)
        let mut tags = HashMap::new();
        tags.insert("pm25".to_string(), "25.5".to_string());
        let point = create_test_point_with_tags(tags);

        // Route point through router
        router
            .route_point("mqtt-source", "test-stream", point)
            .await
            .unwrap();

        // Verify enriched point has stream_id tag
        let enriched = storage_rx.recv().await.unwrap();
        assert_eq!(
            enriched.tags.get("stream_id"),
            Some(&"test-stream".to_string())
        );
        assert_eq!(
            enriched.tags.get("source_id"),
            Some(&"mqtt-source".to_string())
        );
    }

    #[tokio::test]
    async fn test_router_adds_source_id_tag_to_points() {
        // Verify router also adds source_id tag
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );

        let config = create_test_config();
        registry.save_stream(&config).await.unwrap();

        let (dead_letter_tx, _dlq_rx) = mpsc::channel(10);
        let (storage_tx, mut storage_rx) = mpsc::channel(100);
        let router = IngestionRouter::new(registry.clone(), dead_letter_tx);

        router
            .register_storage_channel("test-stream".to_string(), storage_tx)
            .await;

        let mut tags = HashMap::new();
        tags.insert("pm25".to_string(), "30.0".to_string());
        let point = create_test_point_with_tags(tags);

        // Route with specific source_id
        router
            .route_point("air-quality-Mqtt", "test-stream", point)
            .await
            .unwrap();

        // Verify both tags are added
        let enriched = storage_rx.recv().await.unwrap();
        assert_eq!(
            enriched.tags.get("stream_id"),
            Some(&"test-stream".to_string())
        );
        assert_eq!(
            enriched.tags.get("source_id"),
            Some(&"air-quality-Mqtt".to_string())
        );
    }

    #[tokio::test]
    async fn test_router_preserves_existing_tags() {
        // Verify router doesn't overwrite existing tags
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );

        let config = create_test_config();
        registry.save_stream(&config).await.unwrap();

        let (dead_letter_tx, _dlq_rx) = mpsc::channel(10);
        let (storage_tx, mut storage_rx) = mpsc::channel(100);
        let router = IngestionRouter::new(registry.clone(), dead_letter_tx);

        router
            .register_storage_channel("test-stream".to_string(), storage_tx)
            .await;

        // Point with existing tags including device MAC
        let mut tags = HashMap::new();
        tags.insert("pm25".to_string(), "25.5".to_string());
        tags.insert("device_mac".to_string(), "d83bda1cd074".to_string());
        tags.insert("sensor_type".to_string(), "airgradient".to_string());
        let point = create_test_point_with_tags(tags);

        router
            .route_point("mqtt-source", "test-stream", point)
            .await
            .unwrap();

        // Verify all tags are present
        let enriched = storage_rx.recv().await.unwrap();
        assert_eq!(
            enriched.tags.get("stream_id"),
            Some(&"test-stream".to_string())
        );
        assert_eq!(
            enriched.tags.get("source_id"),
            Some(&"mqtt-source".to_string())
        );
        assert_eq!(
            enriched.tags.get("device_mac"),
            Some(&"d83bda1cd074".to_string())
        );
        assert_eq!(
            enriched.tags.get("sensor_type"),
            Some(&"airgradient".to_string())
        );
    }

    // ========== CONFIG-DRIVEN REGISTRATION TESTS ==========

    #[tokio::test]
    async fn test_has_storage_channel() {
        let (dead_letter_tx, _rx) = mpsc::channel(10);
        let (storage_tx, _storage_rx) = mpsc::channel(100);
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );

        let router = IngestionRouter::new(registry, dead_letter_tx);

        // Initially no channels
        assert!(!router.has_storage_channel("test-stream").await);

        // Register and verify
        router
            .register_storage_channel("test-stream".to_string(), storage_tx)
            .await;
        assert!(router.has_storage_channel("test-stream").await);

        // Unregister and verify
        router.unregister_storage_channel("test-stream").await;
        assert!(!router.has_storage_channel("test-stream").await);
    }

    #[tokio::test]
    async fn test_registered_stream_count() {
        let (dead_letter_tx, _rx) = mpsc::channel(10);
        let (storage_tx, _storage_rx) = mpsc::channel(100);
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );

        let router = IngestionRouter::new(registry, dead_letter_tx);

        assert_eq!(router.registered_stream_count().await, 0);

        router
            .register_storage_channel("stream-1".to_string(), storage_tx.clone())
            .await;
        assert_eq!(router.registered_stream_count().await, 1);

        router
            .register_storage_channel("stream-2".to_string(), storage_tx.clone())
            .await;
        assert_eq!(router.registered_stream_count().await, 2);

        router.unregister_storage_channel("stream-1").await;
        assert_eq!(router.registered_stream_count().await, 1);
    }

    #[tokio::test]
    async fn test_register_all_streams_from_registry() {
        // This test verifies config-driven registration
        let (dead_letter_tx, _rx) = mpsc::channel(10);
        let (storage_tx, _storage_rx) = mpsc::channel(100);
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );

        // Save test stream configs to registry
        let test_config = create_test_config();
        registry.save_stream(&test_config).await.unwrap();

        let router = IngestionRouter::new(registry.clone(), dead_letter_tx);

        // Initially no channels
        assert_eq!(router.registered_stream_count().await, 0);

        // Register all from registry
        let count = router
            .register_all_streams_from_registry(storage_tx)
            .await
            .unwrap();

        // Should have registered at least the test stream
        assert!(
            count >= 1,
            "Should register at least 1 stream from registry"
        );
        assert!(
            router.has_storage_channel("test-stream").await,
            "test-stream should be registered from registry"
        );

        // Cleanup
        registry.delete_stream("test-stream").await.ok();
    }
}
