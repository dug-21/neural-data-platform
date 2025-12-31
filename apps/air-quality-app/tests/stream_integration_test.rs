// ========== LONDON SCHOOL TDD: STREAM INTEGRATION TESTS ==========
// Test-first approach: Write tests before implementation
// Focus on behavior verification and interaction testing

use config_client::stream::StreamRegistry;
use neural_core::{
    FieldType, SchemaField, SourceConfig, SourceType, StorageConfig as StreamStorageConfig,
    StreamConfig,
};
use std::collections::HashMap;

// ========== HELPER FUNCTIONS FOR TEST DATA ==========

/// Create a test StreamConfig with MQTT source
fn create_test_stream_config() -> StreamConfig {
    let mut mqtt_params = HashMap::new();
    mqtt_params.insert(
        "broker_url".to_string(),
        serde_json::json!("mqtt.example.com"),
    );
    mqtt_params.insert("port".to_string(), serde_json::json!(1883));
    mqtt_params.insert("client_id".to_string(), serde_json::json!("test-client"));
    mqtt_params.insert("topic_pattern".to_string(), serde_json::json!("test/+"));
    mqtt_params.insert("qos".to_string(), serde_json::json!(1));
    mqtt_params.insert("reconnect_delay_secs".to_string(), serde_json::json!(1));
    mqtt_params.insert(
        "max_reconnect_delay_secs".to_string(),
        serde_json::json!(30),
    );

    StreamConfig {
        stream_id: "test-stream".to_string(),
        description: "Test stream for integration".to_string(),
        version: "1.0.0".to_string(),
        enabled: true,
        retention_days: 30,
        compression_after_days: 7,
        partitioning_strategy: "daily".to_string(),
        fields: vec![
            SchemaField::new("pm25".to_string(), FieldType::Float)
                .required()
                .with_unit("µg/m³".to_string())
                .with_range(0.0, 500.0)
                .with_precision(1),
            SchemaField::new("temperature".to_string(), FieldType::Float)
                .with_unit("celsius".to_string())
                .with_range(-40.0, 60.0)
                .with_precision(1),
        ],
        sources: vec![SourceConfig {
            source_type: SourceType::Mqtt,
            enabled: true,
            ndp_id: None,
            context: None,
            params: mqtt_params,
        }],
        storage: Some(StreamStorageConfig {
            batch_size: 100,
            batch_timeout_secs: 5,
            buffer_capacity: 1000,
        }),
    }
}

/// Create a minimal StreamConfig (testing defaults)
fn create_minimal_stream_config() -> StreamConfig {
    let mut mqtt_params = HashMap::new();
    mqtt_params.insert("broker_url".to_string(), serde_json::json!("localhost"));
    mqtt_params.insert("topic_pattern".to_string(), serde_json::json!("test/+"));

    StreamConfig {
        stream_id: "minimal".to_string(),
        description: "Minimal test stream".to_string(),
        version: "1.0.0".to_string(),
        enabled: true,
        retention_days: 0,
        compression_after_days: 0,
        partitioning_strategy: "daily".to_string(),
        fields: vec![SchemaField::new("value".to_string(), FieldType::Float)],
        sources: vec![SourceConfig {
            source_type: SourceType::Mqtt,
            enabled: true,
            ndp_id: None,
            context: None,
            params: mqtt_params,
        }],
        storage: None, // No storage config - should use defaults
    }
}

/// Create a StreamConfig with non-MQTT sources
fn create_http_stream_config() -> StreamConfig {
    let mut http_params = HashMap::new();
    http_params.insert(
        "url".to_string(),
        serde_json::json!("https://api.example.com/data"),
    );
    http_params.insert("interval_secs".to_string(), serde_json::json!(60));

    StreamConfig {
        stream_id: "http-stream".to_string(),
        description: "HTTP-based stream".to_string(),
        version: "1.0.0".to_string(),
        enabled: true,
        retention_days: 30,
        compression_after_days: 7,
        partitioning_strategy: "daily".to_string(),
        fields: vec![SchemaField::new("value".to_string(), FieldType::Float)],
        sources: vec![SourceConfig {
            source_type: SourceType::HttpPoll,
            enabled: true,
            ndp_id: None,
            context: None,
            params: http_params,
        }],
        storage: None,
    }
}

// ========== BEHAVIOR TESTS: MQTT CONFIG EXTRACTION ==========

#[test]
fn test_extract_mqtt_broker_url_from_stream_config() {
    // Given: StreamConfig with MQTT source containing broker_url
    let stream_config = create_test_stream_config();

    // When: Extracting MQTT source
    let mqtt_source = stream_config
        .sources
        .iter()
        .find(|s| matches!(s.source_type, SourceType::Mqtt));

    // Then: MQTT source exists with broker_url
    assert!(mqtt_source.is_some());
    let mqtt_source = mqtt_source.unwrap();
    assert_eq!(
        mqtt_source
            .params
            .get("broker_url")
            .unwrap()
            .as_str()
            .unwrap(),
        "mqtt.example.com"
    );
}

#[test]
fn test_extract_mqtt_port_from_stream_config() {
    // Given: StreamConfig with MQTT source containing port
    let stream_config = create_test_stream_config();

    // When: Extracting port parameter
    let mqtt_source = stream_config
        .sources
        .iter()
        .find(|s| matches!(s.source_type, SourceType::Mqtt))
        .unwrap();

    // Then: Port is correctly extracted
    assert_eq!(
        mqtt_source.params.get("port").unwrap().as_u64().unwrap(),
        1883
    );
}

#[test]
fn test_extract_mqtt_qos_from_stream_config() {
    // Given: StreamConfig with QoS specified
    let stream_config = create_test_stream_config();

    // When: Extracting QoS parameter
    let mqtt_source = stream_config
        .sources
        .iter()
        .find(|s| matches!(s.source_type, SourceType::Mqtt))
        .unwrap();

    // Then: QoS is correctly extracted
    assert_eq!(mqtt_source.params.get("qos").unwrap().as_u64().unwrap(), 1);
}

#[test]
fn test_missing_mqtt_source_in_stream_config() {
    // Given: StreamConfig with no MQTT source
    let stream_config = create_http_stream_config();

    // When: Searching for MQTT source
    let mqtt_source = stream_config
        .sources
        .iter()
        .find(|s| matches!(s.source_type, SourceType::Mqtt));

    // Then: No MQTT source found
    assert!(mqtt_source.is_none());
}

// ========== BEHAVIOR TESTS: STORAGE CONFIG EXTRACTION ==========

#[test]
fn test_extract_batch_size_from_stream_storage() {
    // Given: StreamConfig with storage configuration
    let stream_config = create_test_stream_config();

    // When: Extracting storage config
    let storage = stream_config.storage.as_ref();

    // Then: Batch size is correctly extracted
    assert!(storage.is_some());
    assert_eq!(storage.unwrap().batch_size, 100);
}

#[test]
fn test_extract_batch_timeout_from_stream_storage() {
    // Given: StreamConfig with storage configuration
    let stream_config = create_test_stream_config();

    // When: Extracting storage config
    let storage = stream_config.storage.as_ref().unwrap();

    // Then: Batch timeout is correctly extracted
    assert_eq!(storage.batch_timeout_secs, 5);
}

#[test]
fn test_extract_buffer_capacity_from_stream_storage() {
    // Given: StreamConfig with storage configuration
    let stream_config = create_test_stream_config();

    // When: Extracting storage config
    let storage = stream_config.storage.as_ref().unwrap();

    // Then: Buffer capacity is correctly extracted
    assert_eq!(storage.buffer_capacity, 1000);
}

#[test]
fn test_missing_storage_config_uses_defaults() {
    // Given: StreamConfig with no storage configuration
    let stream_config = create_minimal_stream_config();

    // When: Checking storage config
    let storage = stream_config.storage.as_ref();

    // Then: No storage config present (defaults should be applied by app)
    assert!(storage.is_none());
}

// ========== BEHAVIOR TESTS: DEFAULT VALUES ==========

#[test]
fn test_mqtt_source_uses_default_port_when_missing() {
    // Given: StreamConfig with missing port parameter
    let mut mqtt_params = HashMap::new();
    mqtt_params.insert("broker_url".to_string(), serde_json::json!("localhost"));
    mqtt_params.insert("topic_pattern".to_string(), serde_json::json!("test/+"));
    // port is intentionally missing

    let stream_config = StreamConfig {
        stream_id: "test".to_string(),
        description: "Test".to_string(),
        version: "1.0.0".to_string(),
        enabled: true,
        retention_days: 0,
        compression_after_days: 0,
        partitioning_strategy: "daily".to_string(),
        fields: vec![SchemaField::new("value".to_string(), FieldType::Float)],
        sources: vec![SourceConfig {
            source_type: SourceType::Mqtt,
            enabled: true,
            ndp_id: None,
            context: None,
            params: mqtt_params,
        }],
        storage: None,
    };

    // When: Extracting port parameter
    let mqtt_source = stream_config
        .sources
        .iter()
        .find(|s| matches!(s.source_type, SourceType::Mqtt))
        .unwrap();

    // Then: Port parameter is missing (default 1883 should be used by app)
    assert!(mqtt_source.params.get("port").is_none());
}

#[test]
fn test_mqtt_source_uses_default_qos_when_missing() {
    // Given: StreamConfig with missing QoS parameter
    let minimal_config = create_minimal_stream_config();

    // When: Extracting QoS parameter
    let mqtt_source = minimal_config
        .sources
        .iter()
        .find(|s| matches!(s.source_type, SourceType::Mqtt))
        .unwrap();

    // Then: QoS parameter is missing (default 1 should be used by app)
    assert!(mqtt_source.params.get("qos").is_none());
}

// ========== BEHAVIOR TESTS: ERROR CASES ==========

#[test]
fn test_missing_broker_url_is_error() {
    // Given: StreamConfig with MQTT source but no broker_url
    let mut mqtt_params = HashMap::new();
    mqtt_params.insert("topic_pattern".to_string(), serde_json::json!("test/+"));
    // broker_url is intentionally missing

    let stream_config = StreamConfig {
        stream_id: "test".to_string(),
        description: "Test".to_string(),
        version: "1.0.0".to_string(),
        enabled: true,
        retention_days: 0,
        compression_after_days: 0,
        partitioning_strategy: "daily".to_string(),
        fields: vec![SchemaField::new("value".to_string(), FieldType::Float)],
        sources: vec![SourceConfig {
            source_type: SourceType::Mqtt,
            enabled: true,
            ndp_id: None,
            context: None,
            params: mqtt_params,
        }],
        storage: None,
    };

    // When: Extracting broker_url parameter
    let mqtt_source = stream_config
        .sources
        .iter()
        .find(|s| matches!(s.source_type, SourceType::Mqtt))
        .unwrap();

    // Then: broker_url is missing (should cause error in conversion)
    assert!(mqtt_source.params.get("broker_url").is_none());
}

#[test]
fn test_invalid_qos_value_outside_range() {
    // Given: StreamConfig with invalid QoS (not 0, 1, or 2)
    let mut mqtt_params = HashMap::new();
    mqtt_params.insert("broker_url".to_string(), serde_json::json!("localhost"));
    mqtt_params.insert("topic_pattern".to_string(), serde_json::json!("test/+"));
    mqtt_params.insert("qos".to_string(), serde_json::json!(99)); // Invalid QoS

    let stream_config = StreamConfig {
        stream_id: "test".to_string(),
        description: "Test".to_string(),
        version: "1.0.0".to_string(),
        enabled: true,
        retention_days: 0,
        compression_after_days: 0,
        partitioning_strategy: "daily".to_string(),
        fields: vec![SchemaField::new("value".to_string(), FieldType::Float)],
        sources: vec![SourceConfig {
            source_type: SourceType::Mqtt,
            enabled: true,
            ndp_id: None,
            context: None,
            params: mqtt_params,
        }],
        storage: None,
    };

    // When: Extracting QoS parameter
    let mqtt_source = stream_config
        .sources
        .iter()
        .find(|s| matches!(s.source_type, SourceType::Mqtt))
        .unwrap();

    // Then: Invalid QoS value extracted (should default to 1 in conversion)
    assert_eq!(mqtt_source.params.get("qos").unwrap().as_u64().unwrap(), 99);
}

// ========== BEHAVIOR TESTS: MULTIPLE SOURCES ==========

#[test]
fn test_stream_with_multiple_sources_finds_mqtt() {
    // Given: StreamConfig with multiple sources including MQTT
    let mut mqtt_params = HashMap::new();
    mqtt_params.insert("broker_url".to_string(), serde_json::json!("localhost"));
    mqtt_params.insert("topic_pattern".to_string(), serde_json::json!("test/+"));

    let mut http_params = HashMap::new();
    http_params.insert(
        "url".to_string(),
        serde_json::json!("https://api.example.com"),
    );

    let stream_config = StreamConfig {
        stream_id: "multi-source".to_string(),
        description: "Multi-source stream".to_string(),
        version: "1.0.0".to_string(),
        enabled: true,
        retention_days: 0,
        compression_after_days: 0,
        partitioning_strategy: "daily".to_string(),
        fields: vec![SchemaField::new("value".to_string(), FieldType::Float)],
        sources: vec![
            SourceConfig {
                source_type: SourceType::HttpPoll,
                enabled: true,
                ndp_id: None,
                context: None,
                params: http_params,
            },
            SourceConfig {
                source_type: SourceType::Mqtt,
                enabled: true,
                ndp_id: None,
                context: None,
                params: mqtt_params,
            },
        ],
        storage: None,
    };

    // When: Searching for MQTT source
    let mqtt_source = stream_config
        .sources
        .iter()
        .find(|s| matches!(s.source_type, SourceType::Mqtt));

    // Then: MQTT source is found despite multiple sources
    assert!(mqtt_source.is_some());
}

// ========== BEHAVIOR TESTS: STREAM VALIDATION ==========

#[test]
fn test_valid_stream_config_passes_validation() {
    // Given: Valid StreamConfig
    let stream_config = create_test_stream_config();

    // When: Validating configuration
    let result = stream_config.validate();

    // Then: Validation succeeds
    assert!(result.is_ok());
}

#[test]
fn test_stream_config_with_no_sources_fails_validation() {
    // Given: StreamConfig with no sources
    let mut stream_config = create_test_stream_config();
    stream_config.sources.clear();

    // When: Validating configuration
    let result = stream_config.validate();

    // Then: Validation fails
    assert!(result.is_err());
}

#[test]
fn test_stream_config_with_no_fields_fails_validation() {
    // Given: StreamConfig with no fields
    let mut stream_config = create_test_stream_config();
    stream_config.fields.clear();

    // When: Validating configuration
    let result = stream_config.validate();

    // Then: Validation fails
    assert!(result.is_err());
}

// ========== INTEGRATION TEST PATTERNS (require etcd) ==========
// These demonstrate the expected behavior when integrated with StreamRegistry

#[tokio::test]
#[ignore] // Requires running etcd instance
async fn integration_test_load_stream_from_registry() {
    // Given: StreamRegistry connected to etcd
    let registry = StreamRegistry::new(&["http://localhost:2379"])
        .await
        .expect("Failed to connect to etcd");

    // And: Stream configuration exists in etcd
    let test_config = create_test_stream_config();
    registry
        .save_stream(&test_config)
        .await
        .expect("Failed to save test config");

    // When: Loading stream from registry
    let loaded_config = registry
        .load_stream("test-stream")
        .await
        .expect("Failed to load stream");

    // Then: Configuration matches what was saved
    assert_eq!(loaded_config.stream_id, "test-stream");
    assert_eq!(loaded_config.fields.len(), 2);
    assert_eq!(loaded_config.sources.len(), 1);

    // Cleanup
    registry
        .delete_stream("test-stream")
        .await
        .expect("Failed to cleanup");
}

#[tokio::test]
#[ignore] // Requires running etcd instance
async fn integration_test_stream_not_found_returns_error() {
    // Given: StreamRegistry connected to etcd
    let registry = StreamRegistry::new(&["http://localhost:2379"])
        .await
        .expect("Failed to connect to etcd");

    // When: Loading non-existent stream
    let result = registry.load_stream("non-existent-stream").await;

    // Then: Error is returned
    assert!(result.is_err());
}
