//! dp-018: JSON Schema Validation Tests
//!
//! London School TDD tests for JSON schema validation.
//! These tests verify the v1.1 JSON schema behavior for stream configurations,
//! including backward compatibility with v1.0 format.
//!
//! # Test Categories
//!
//! 1. v1.0 Backward Compatibility - Schema accepts entity_schemas-only configs
//! 2. v1.1 Enriched Fields - Schema accepts fields with description/unit
//! 3. Hybrid Configs - Schema accepts both entity_schemas and enriched fields
//! 4. Validation - Schema rejects invalid stream_id format
//! 5. Minimum Fields - Schema requires at least one field
//!
//! # References
//!
//! - TEST-STRATEGY.md: Phase 0 JSON Migration Tests
//! - PSEUDOCODE.md: Schema validation requirements

use serde_json::{json, Value};

// ============================================================================
// Helper Functions
// ============================================================================

/// Load the stream-config schema from file
///
/// Note: This function will need the actual schema file to exist.
/// For now, tests use inline schema validation via serde.
fn create_v1_0_config() -> Value {
    json!({
        "stream_id": "air-quality",
        "description": "Air quality sensor data",
        "version": "1.0.0",
        "enabled": true,
        "fields": [
            {
                "name": "pm25",
                "type": "float",
                "nullable": false
            },
            {
                "name": "temperature",
                "type": "float",
                "nullable": true
            }
        ],
        "sources": [
            {
                "type": "mqtt",
                "enabled": true
            }
        ]
    })
}

fn create_v1_1_config_enriched_fields() -> Value {
    json!({
        "stream_id": "air-quality",
        "description": "Air quality sensor data",
        "version": "1.1.0",
        "enabled": true,
        "fields": [
            {
                "name": "pm25",
                "type": "float",
                "nullable": false,
                "description": "Particulate matter 2.5um concentration",
                "unit": "ug/m3",
                "range": [0.0, 500.0],
                "display_precision": 1
            },
            {
                "name": "temperature",
                "type": "float",
                "nullable": true,
                "description": "Ambient temperature",
                "unit": "celsius"
            }
        ],
        "sources": [
            {
                "type": "mqtt",
                "enabled": true
            }
        ]
    })
}

fn create_config_with_both() -> Value {
    json!({
        "stream_id": "air-quality",
        "description": "Air quality sensor data",
        "version": "1.1.0",
        "enabled": true,
        "fields": [
            {
                "name": "pm25",
                "type": "float",
                "nullable": false,
                "description": "Particulate matter 2.5um",
                "unit": "ug/m3"
            }
        ],
        "entity_schemas": [
            {
                "schema_name": "airgradient",
                "device_class": "sensor",
                "attributes": [
                    {
                        "name": "pm25",
                        "description": "PM2.5 from entity_schemas (should be overridden)"
                    }
                ]
            }
        ],
        "sources": [
            {
                "type": "mqtt",
                "enabled": true
            }
        ]
    })
}

// ============================================================================
// Test: Schema accepts v1.0 structure (backward compat)
// ============================================================================

#[test]
fn test_schema_accepts_v1_0_config_with_basic_fields() {
    // Arrange - v1.0 config without enriched fields
    let config_json = create_v1_0_config();

    // Act - Parse as StreamConfig
    let result: Result<platform_core::StreamConfig, _> = serde_json::from_value(config_json);

    // Assert - Should parse successfully
    assert!(result.is_ok(), "v1.0 config should parse successfully");
    let config = result.unwrap();

    // Verify backward compatibility
    assert_eq!(config.stream_id, "air-quality");
    assert_eq!(config.fields.len(), 2);

    // v1.0 fields should not have description
    assert!(config.fields[0].description.is_none());
    assert!(config.fields[0].unit.is_none());
}

// ============================================================================
// Test: Schema accepts v1.1 structure (enriched fields)
// ============================================================================

#[test]
fn test_schema_accepts_v1_1_config_with_enriched_fields() {
    // Arrange - v1.1 config with enriched fields
    let config_json = create_v1_1_config_enriched_fields();

    // Act - Parse as StreamConfig
    let result: Result<platform_core::StreamConfig, _> = serde_json::from_value(config_json);

    // Assert - Should parse successfully with enriched fields
    assert!(result.is_ok(), "v1.1 config should parse successfully");
    let config = result.unwrap();

    // Verify enriched fields are populated
    assert_eq!(config.stream_id, "air-quality");
    assert_eq!(config.fields.len(), 2);

    // Check first field has description and unit
    let pm25_field = &config.fields[0];
    assert_eq!(pm25_field.name, "pm25");
    assert_eq!(
        pm25_field.description,
        Some("Particulate matter 2.5um concentration".to_string())
    );
    assert_eq!(pm25_field.unit, Some("ug/m3".to_string()));
    assert_eq!(pm25_field.range, Some(vec![0.0, 500.0]));
    assert_eq!(pm25_field.display_precision, Some(1));
}

// ============================================================================
// Test: Schema accepts config with both enriched fields and entity_schemas
// ============================================================================

#[test]
fn test_schema_accepts_config_with_both_fields_and_entity_schemas() {
    // Arrange - Config with both enriched fields and entity_schemas
    let config_json = create_config_with_both();

    // Act - Parse as StreamConfig
    let result: Result<platform_core::StreamConfig, _> = serde_json::from_value(config_json);

    // Assert - Should parse successfully
    // Note: entity_schemas is not part of StreamConfig - it's handled separately
    assert!(result.is_ok(), "Config with both should parse successfully");
    let config = result.unwrap();

    // Fields should have enriched data
    assert_eq!(config.fields[0].description, Some("Particulate matter 2.5um".to_string()));
}

// ============================================================================
// Test: Schema rejects invalid stream_id format
// ============================================================================

#[test]
fn test_schema_rejects_invalid_stream_id_uppercase() {
    // Arrange - Invalid stream_id with uppercase
    let config_json = json!({
        "stream_id": "Air-Quality",  // Uppercase not allowed
        "description": "Test stream",
        "version": "1.0.0",
        "enabled": true,
        "fields": [
            {"name": "temp", "type": "float", "nullable": true}
        ],
        "sources": [
            {"type": "mqtt", "enabled": true}
        ]
    });

    // Act - Parse and validate
    let result: Result<platform_core::StreamConfig, _> = serde_json::from_value(config_json);
    assert!(result.is_ok(), "Parsing should succeed, validation catches error");

    let config = result.unwrap();
    let validation = config.validate();

    // Assert - Validation should fail
    assert!(validation.is_err());
    assert!(matches!(
        validation.unwrap_err(),
        platform_core::StreamConfigError::InvalidStreamId(_)
    ));
}

#[test]
fn test_schema_rejects_invalid_stream_id_underscore() {
    // Arrange - Invalid stream_id with underscore
    let config_json = json!({
        "stream_id": "air_quality",  // Underscore not allowed (kebab-case required)
        "description": "Test stream",
        "version": "1.0.0",
        "enabled": true,
        "fields": [
            {"name": "temp", "type": "float", "nullable": true}
        ],
        "sources": [
            {"type": "mqtt", "enabled": true}
        ]
    });

    // Act
    let result: Result<platform_core::StreamConfig, _> = serde_json::from_value(config_json);
    let config = result.unwrap();
    let validation = config.validate();

    // Assert
    assert!(validation.is_err());
    assert!(matches!(
        validation.unwrap_err(),
        platform_core::StreamConfigError::InvalidStreamId(_)
    ));
}

#[test]
fn test_schema_rejects_stream_id_too_short() {
    // Arrange - stream_id too short (< 3 chars)
    let config_json = json!({
        "stream_id": "ab",  // Too short
        "description": "Test stream",
        "version": "1.0.0",
        "enabled": true,
        "fields": [
            {"name": "temp", "type": "float", "nullable": true}
        ],
        "sources": [
            {"type": "mqtt", "enabled": true}
        ]
    });

    // Act
    let result: Result<platform_core::StreamConfig, _> = serde_json::from_value(config_json);
    let config = result.unwrap();
    let validation = config.validate();

    // Assert
    assert!(validation.is_err());
    assert!(matches!(
        validation.unwrap_err(),
        platform_core::StreamConfigError::InvalidStreamId(_)
    ));
}

// ============================================================================
// Test: Schema requires minimum one field
// ============================================================================

#[test]
fn test_schema_requires_at_least_one_field() {
    // Arrange - Config with empty fields array
    let config_json = json!({
        "stream_id": "test-stream",
        "description": "Test stream",
        "version": "1.0.0",
        "enabled": true,
        "fields": [],  // Empty - should fail validation
        "sources": [
            {"type": "mqtt", "enabled": true}
        ]
    });

    // Act
    let result: Result<platform_core::StreamConfig, _> = serde_json::from_value(config_json);
    let config = result.unwrap();
    let validation = config.validate();

    // Assert - Validation should fail with NoFields error
    assert!(validation.is_err());
    assert_eq!(validation.unwrap_err(), platform_core::StreamConfigError::NoFields);
}

#[test]
fn test_schema_requires_at_least_one_source() {
    // Arrange - Config with empty sources array
    let config_json = json!({
        "stream_id": "test-stream",
        "description": "Test stream",
        "version": "1.0.0",
        "enabled": true,
        "fields": [
            {"name": "temp", "type": "float", "nullable": true}
        ],
        "sources": []  // Empty - should fail validation
    });

    // Act
    let result: Result<platform_core::StreamConfig, _> = serde_json::from_value(config_json);
    let config = result.unwrap();
    let validation = config.validate();

    // Assert
    assert!(validation.is_err());
    assert_eq!(validation.unwrap_err(), platform_core::StreamConfigError::NoSources);
}

// ============================================================================
// Test: JSON = YAML parsing equivalence
// ============================================================================

#[test]
fn test_json_yaml_parsing_equivalent() {
    // Arrange - YAML format
    let yaml = r#"
stream_id: air-quality
description: Air quality sensor data
version: "1.0.0"
enabled: true
fields:
  - name: pm25
    type: float
    nullable: false
sources:
  - type: mqtt
    enabled: true
"#;

    // Arrange - Equivalent JSON format
    let json = json!({
        "stream_id": "air-quality",
        "description": "Air quality sensor data",
        "version": "1.0.0",
        "enabled": true,
        "fields": [{"name": "pm25", "type": "float", "nullable": false}],
        "sources": [{"type": "mqtt", "enabled": true}]
    });

    // Act - Parse both
    let yaml_config: platform_core::StreamConfig = serde_yaml::from_str(yaml).unwrap();
    let json_config: platform_core::StreamConfig = serde_json::from_value(json).unwrap();

    // Assert - Both should produce identical configs
    assert_eq!(yaml_config.stream_id, json_config.stream_id);
    assert_eq!(yaml_config.description, json_config.description);
    assert_eq!(yaml_config.version, json_config.version);
    assert_eq!(yaml_config.enabled, json_config.enabled);
    assert_eq!(yaml_config.fields.len(), json_config.fields.len());
    assert_eq!(yaml_config.sources.len(), json_config.sources.len());

    // Compare field by field
    for (yaml_field, json_field) in yaml_config.fields.iter().zip(json_config.fields.iter()) {
        assert_eq!(yaml_field.name, json_field.name);
        assert_eq!(yaml_field.field_type, json_field.field_type);
        assert_eq!(yaml_field.nullable, json_field.nullable);
    }
}

// ============================================================================
// Test: StreamConfig with silver_etl optional field
// ============================================================================

#[test]
fn test_stream_config_without_silver_etl_is_backward_compatible() {
    // Arrange - v1.0 style config without silver_etl
    let json = json!({
        "stream_id": "test-stream",
        "description": "Test",
        "version": "1.0.0",
        "enabled": true,
        "fields": [{"name": "value", "type": "float", "nullable": true}],
        "sources": [{"type": "mqtt", "enabled": true}]
    });

    // Act
    let result: Result<platform_core::StreamConfig, _> = serde_json::from_value(json);

    // Assert - Should parse without silver_etl
    assert!(result.is_ok());
    let config = result.unwrap();
    assert_eq!(config.stream_id, "test-stream");
    // Note: silver_etl is not part of StreamConfig directly,
    // it's loaded separately via ConfigLoader
}

// ============================================================================
// Test: Field validation with invalid types
// ============================================================================

#[test]
fn test_field_validation_string_cannot_have_range() {
    // Arrange
    let json = json!({
        "stream_id": "test-stream",
        "description": "Test",
        "version": "1.0.0",
        "enabled": true,
        "fields": [{
            "name": "status",
            "type": "string",
            "nullable": true,
            "range": [0.0, 100.0]  // Invalid: string cannot have range
        }],
        "sources": [{"type": "mqtt", "enabled": true}]
    });

    // Act
    let config: platform_core::StreamConfig = serde_json::from_value(json).unwrap();
    let validation = config.validate();

    // Assert
    assert!(validation.is_err());
    assert!(matches!(
        validation.unwrap_err(),
        platform_core::StreamConfigError::InvalidFieldType { .. }
    ));
}

#[test]
fn test_field_validation_int_cannot_have_precision() {
    // Arrange
    let json = json!({
        "stream_id": "test-stream",
        "description": "Test",
        "version": "1.0.0",
        "enabled": true,
        "fields": [{
            "name": "count",
            "type": "int",
            "nullable": true,
            "display_precision": 2  // Invalid: int cannot have precision
        }],
        "sources": [{"type": "mqtt", "enabled": true}]
    });

    // Act
    let config: platform_core::StreamConfig = serde_json::from_value(json).unwrap();
    let validation = config.validate();

    // Assert
    assert!(validation.is_err());
    assert!(matches!(
        validation.unwrap_err(),
        platform_core::StreamConfigError::InvalidFieldType { .. }
    ));
}

// ============================================================================
// Test: Serialization preserves optional fields correctly
// ============================================================================

#[test]
fn test_serialization_skips_none_optional_fields() {
    // Arrange - Create config with minimal fields
    let config = platform_core::StreamConfig {
        stream_id: "test-stream".to_string(),
        description: "Test".to_string(),
        version: "1.0.0".to_string(),
        enabled: true,
        retention_days: 0,
        compression_after_days: 0,
        partitioning_strategy: "daily".to_string(),
        fields: vec![platform_core::SchemaField::new("value".to_string(), platform_core::FieldType::Float)],
        sources: vec![platform_core::SourceConfig {
            source_type: platform_core::SourceType::Mqtt,
            enabled: true,
            ndp_id: None,
            context: None,
            params: std::collections::HashMap::new(),
        }],
        storage: None,
        silver_etl: None,
        entity_schemas: None,
    };

    // Act
    let json_str = serde_json::to_string(&config).unwrap();

    // Assert - None fields should not appear in output
    assert!(!json_str.contains("\"storage\":"));
    assert!(!json_str.contains("\"ndp_id\":"));
    assert!(!json_str.contains("\"context\":"));
}

#[test]
fn test_serialization_includes_present_optional_fields() {
    // Arrange - Create field with all optional fields populated
    let field = platform_core::SchemaField::new("pm25".to_string(), platform_core::FieldType::Float)
        .with_description("PM2.5 concentration".to_string())
        .with_unit("ug/m3".to_string())
        .with_range(0.0, 500.0)
        .with_precision(1);

    // Act
    let json_str = serde_json::to_string(&field).unwrap();

    // Assert - All optional fields should be present
    assert!(json_str.contains("\"description\":"));
    assert!(json_str.contains("\"unit\":"));
    assert!(json_str.contains("\"range\":"));
    assert!(json_str.contains("\"display_precision\":"));
}

// ============================================================================
// Test: Round-trip serialization
// ============================================================================

#[test]
fn test_json_roundtrip_preserves_all_data() {
    // Arrange
    let original = platform_core::StreamConfig {
        stream_id: "roundtrip-test".to_string(),
        description: "Roundtrip test config".to_string(),
        version: "1.1.0".to_string(),
        enabled: true,
        retention_days: 365,
        compression_after_days: 7,
        partitioning_strategy: "daily".to_string(),
        fields: vec![
            platform_core::SchemaField::new("pm25".to_string(), platform_core::FieldType::Float)
                .required()
                .with_unit("ug/m3".to_string())
                .with_description("PM2.5".to_string())
                .with_range(0.0, 500.0),
        ],
        sources: vec![platform_core::SourceConfig {
            source_type: platform_core::SourceType::Mqtt,
            enabled: true,
            ndp_id: Some("sensor-001".to_string()),
            context: Some(serde_json::json!({"room": "office"})),
            params: std::collections::HashMap::new(),
        }],
        storage: None,
        silver_etl: None,
        entity_schemas: None,
    };

    // Act - Serialize to JSON
    let json_str = serde_json::to_string(&original).unwrap();

    // Act - Deserialize back
    let restored: platform_core::StreamConfig = serde_json::from_str(&json_str).unwrap();

    // Assert - All fields should match
    assert_eq!(original.stream_id, restored.stream_id);
    assert_eq!(original.description, restored.description);
    assert_eq!(original.version, restored.version);
    assert_eq!(original.enabled, restored.enabled);
    assert_eq!(original.retention_days, restored.retention_days);
    assert_eq!(original.fields.len(), restored.fields.len());

    let orig_field = &original.fields[0];
    let rest_field = &restored.fields[0];
    assert_eq!(orig_field.name, rest_field.name);
    assert_eq!(orig_field.description, rest_field.description);
    assert_eq!(orig_field.unit, rest_field.unit);
    assert_eq!(orig_field.range, rest_field.range);
    assert_eq!(orig_field.nullable, rest_field.nullable);

    let orig_source = &original.sources[0];
    let rest_source = &restored.sources[0];
    assert_eq!(orig_source.ndp_id, rest_source.ndp_id);
    assert_eq!(orig_source.context, rest_source.context);
}
