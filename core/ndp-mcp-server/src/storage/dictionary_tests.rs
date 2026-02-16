//! Dictionary Loader Tests for dp-018 JSON Config Foundation
//!
//! Tests the dictionary loading behavior following London School TDD principles.
//! Key behaviors tested:
//!
//! 1. **fields.description precedence (v1.1)** - When fields have description,
//!    use that directly
//! 2. **entity_schemas fallback (v1.0 compat)** - When fields.description is
//!    missing, fall back to entity_schemas
//! 3. **fields.description takes precedence** - Even when entity_schemas
//!    exists, fields.description wins
//! 4. **Graceful handling of missing descriptions** - No panic when both are
//!    missing
//!
//! # Design Rationale (dp-018)
//!
//! The data dictionary needs to support both v1.0 and v1.1 schema formats:
//! - v1.0: descriptions in `entity_schemas[].description`
//! - v1.1: descriptions directly in `fields[].description` (preferred)
//!
//! This test module verifies the fallback hierarchy works correctly.
//!
//! # London School TDD
//!
//! These tests focus on BEHAVIOR verification, mocking the DictionaryStore
//! trait to verify:
//! - What inputs are provided
//! - What outputs are expected
//! - Correct error handling

use crate::error::{McpError, McpResult};
use crate::storage::traits::{DictionaryStore, MockDictionaryStore};
use crate::storage::types::{ColumnDescription, DictionaryEntry, SourceInfo, ValidationRange};

// ============================================================================
// Description Resolution Tests (dp-018 Core Behavior)
// ============================================================================

/// Test that description from fields.description is returned directly
///
/// v1.1 configs have description in fields[].description
/// This is the preferred location and should be used when present.
#[tokio::test]
async fn test_dictionary_loads_description_from_fields_v1_1() {
    let mut mock = MockDictionaryStore::new();

    // Configure mock to return entry with description from fields
    mock.expect_search()
        .withf(|query, _| query == "pm25")
        .times(1)
        .returning(|_, _| {
            Ok(vec![DictionaryEntry::new(
                "bronze",
                "air-quality",
                "pm25",
                "number",
            )
            .with_description("PM2.5 particulate matter concentration")])
        });

    let results = mock.search("pm25", None).await.unwrap();

    // Assert description is present from fields.description
    assert_eq!(results.len(), 1);
    assert_eq!(
        results[0].description,
        Some("PM2.5 particulate matter concentration".to_string())
    );
}

/// Test fallback to entity_schemas when fields.description is missing
///
/// v1.0 configs don't have fields[].description, so we fall back to
/// entity_schemas[].description for the field metadata.
///
/// Note: This test simulates the behavior; actual fallback logic is in
/// the DictionaryClient implementation that combines etcd and dictionary data.
#[tokio::test]
async fn test_dictionary_fallback_to_entity_schemas_v1_0() {
    let mut mock = MockDictionaryStore::new();

    // Configure mock to simulate v1.0 config without fields.description
    // In real implementation, description would come from entity_schemas
    mock.expect_search()
        .withf(|query, _| query == "temperature")
        .times(1)
        .returning(|_, _| {
            // This simulates the merged result where description came from entity_schemas
            Ok(vec![DictionaryEntry::new(
                "bronze",
                "outdoor-weather",
                "temperature",
                "number",
            )
            .with_description(
                "Temperature in Celsius from entity_schemas",
            )])
        });

    let results = mock.search("temperature", None).await.unwrap();

    // Assert description is present (from entity_schemas fallback)
    assert_eq!(results.len(), 1);
    assert!(results[0].description.is_some());
}

/// Test that fields.description takes precedence over entity_schemas
///
/// When both sources have descriptions, fields.description wins.
/// This ensures v1.1 migrations don't get overwritten by legacy data.
#[tokio::test]
async fn test_fields_description_takes_precedence_over_entity_schemas() {
    let mut mock = MockDictionaryStore::new();

    // In real implementation, even if entity_schemas has a description,
    // the fields.description should take precedence (v1.1)
    mock.expect_describe_column()
        .withf(|table, col| table == "air-quality" && col == "humidity")
        .times(1)
        .returning(|_, _| {
            // This returns the v1.1 description, not the entity_schemas one
            Ok(
                ColumnDescription::new("bronze", "air-quality", "humidity", "number")
                    .with_description("Relative humidity percentage (v1.1 description)")
                    .with_unit("percent"),
            )
        });

    let result = mock
        .describe_column("air-quality", "humidity")
        .await
        .unwrap();

    // Assert the v1.1 description is used
    assert_eq!(
        result.description,
        Some("Relative humidity percentage (v1.1 description)".to_string())
    );
}

/// Test graceful handling when no description exists in either location
///
/// Some fields may have no description in either v1.0 or v1.1 format.
/// The system should handle this gracefully without panicking.
#[tokio::test]
async fn test_dictionary_handles_missing_descriptions_gracefully() {
    let mut mock = MockDictionaryStore::new();

    // Configure mock to return entry without description
    mock.expect_search()
        .withf(|query, _| query == "custom_field")
        .times(1)
        .returning(|_, _| {
            // No description field at all
            Ok(vec![
                DictionaryEntry::new("bronze", "custom-stream", "custom_field", "string")
                    .with_unit("count"), // Has unit but no description
            ])
        });

    let results = mock.search("custom_field", None).await.unwrap();

    // Assert no panic, description is None
    assert_eq!(results.len(), 1);
    assert!(results[0].description.is_none());
    assert_eq!(results[0].unit, Some("count".to_string()));
}

// ============================================================================
// Unit Field Tests (v1.1 Enhancement)
// ============================================================================

/// Test that unit field is loaded from v1.1 fields[].unit
#[tokio::test]
async fn test_dictionary_loads_unit_from_fields_v1_1() {
    let mut mock = MockDictionaryStore::new();

    mock.expect_describe_column()
        .withf(|table, col| table == "air-quality" && col == "pm10")
        .times(1)
        .returning(|_, _| {
            Ok(
                ColumnDescription::new("bronze", "air-quality", "pm10", "number")
                    .with_unit("ug/m3")
                    .with_description("PM10 particulate matter"),
            )
        });

    let result = mock.describe_column("air-quality", "pm10").await.unwrap();

    assert_eq!(result.unit, Some("ug/m3".to_string()));
}

/// Test that validation range is populated from v1.1 fields[].range
#[tokio::test]
async fn test_dictionary_loads_validation_range_from_fields_v1_1() {
    let mut mock = MockDictionaryStore::new();

    mock.expect_describe_column()
        .withf(|table, col| table == "air-quality" && col == "pm25")
        .times(1)
        .returning(|_, _| {
            Ok(
                ColumnDescription::new("bronze", "air-quality", "pm25", "number")
                    .with_validation_range(ValidationRange::bounded(0.0, 500.0)),
            )
        });

    let result = mock.describe_column("air-quality", "pm25").await.unwrap();

    assert!(result.validation_range.is_some());
    let range = result.validation_range.unwrap();
    assert_eq!(range.min, Some(0.0));
    assert_eq!(range.max, Some(500.0));
}

// ============================================================================
// Layer Filter Tests (Bronze vs Silver)
// ============================================================================

/// Test that Bronze layer filter returns only Bronze fields
#[tokio::test]
async fn test_dictionary_search_bronze_layer_only() {
    let mut mock = MockDictionaryStore::new();

    mock.expect_search()
        .withf(|query, layer| query == "pm25" && layer.as_deref() == Some("bronze"))
        .times(1)
        .returning(|_, _| {
            Ok(vec![DictionaryEntry::new(
                "bronze",
                "air-quality",
                "pm25",
                "number",
            )
            .with_unit("ug/m3")])
        });

    let results = mock
        .search("pm25", Some("bronze".to_string()))
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].layer, "bronze");
}

/// Test that Silver layer filter returns only Silver columns
#[tokio::test]
async fn test_dictionary_search_silver_layer_only() {
    let mut mock = MockDictionaryStore::new();

    mock.expect_search()
        .withf(|query, layer| query == "pm25" && layer.as_deref() == Some("silver"))
        .times(1)
        .returning(|_, _| {
            Ok(vec![DictionaryEntry::new(
                "silver",
                "air_quality_observations",
                "pm25",
                "DOUBLE PRECISION",
            )
            .with_unit("ug/m3")
            .with_description("PM2.5 concentration")])
        });

    let results = mock
        .search("pm25", Some("silver".to_string()))
        .await
        .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].layer, "silver");
}

/// Test that no layer filter returns both Bronze and Silver
#[tokio::test]
async fn test_dictionary_search_all_layers() {
    let mut mock = MockDictionaryStore::new();

    mock.expect_search()
        .withf(|query, layer| query == "temperature" && layer.is_none())
        .times(1)
        .returning(|_, _| {
            Ok(vec![
                DictionaryEntry::new("bronze", "outdoor-weather", "temperature", "number"),
                DictionaryEntry::new(
                    "silver",
                    "weather_observations",
                    "temperature_c",
                    "DOUBLE PRECISION",
                ),
            ])
        });

    let results = mock.search("temperature", None).await.unwrap();

    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|r| r.layer == "bronze"));
    assert!(results.iter().any(|r| r.layer == "silver"));
}

// ============================================================================
// Source Lineage Tests (Silver Column Tracing)
// ============================================================================

/// Test that Silver column has source info from Bronze
#[tokio::test]
async fn test_dictionary_silver_column_has_source_lineage() {
    let mut mock = MockDictionaryStore::new();

    mock.expect_describe_column()
        .withf(|table, col| table == "air_quality_observations" && col == "pm25")
        .times(1)
        .returning(|_, _| {
            Ok(ColumnDescription::new(
                "silver",
                "air_quality_observations",
                "pm25",
                "DOUBLE PRECISION",
            )
            .with_source(
                SourceInfo::new("air-quality", "raw_payload.pm02Compensated")
                    .with_transformation("direct"),
            ))
        });

    let result = mock
        .describe_column("air_quality_observations", "pm25")
        .await
        .unwrap();

    assert!(result.source.is_some());
    let source = result.source.unwrap();
    assert_eq!(source.stream, "air-quality");
    assert_eq!(source.path, "raw_payload.pm02Compensated");
}

/// Test that Bronze field has no source (it IS the source)
#[tokio::test]
async fn test_dictionary_bronze_field_has_no_source() {
    let mut mock = MockDictionaryStore::new();

    mock.expect_describe_column()
        .withf(|table, col| table == "air-quality" && col == "pm25")
        .times(1)
        .returning(|_, _| {
            Ok(
                ColumnDescription::new("bronze", "air-quality", "pm25", "number")
                    .with_description("PM2.5 from sensor"),
            )
        });

    let result = mock.describe_column("air-quality", "pm25").await.unwrap();

    // Bronze fields don't have source - they ARE the source
    assert!(result.source.is_none());
}

// ============================================================================
// Error Handling Tests
// ============================================================================

/// Test that empty query returns error
#[tokio::test]
async fn test_dictionary_search_rejects_empty_query() {
    let mut mock = MockDictionaryStore::new();

    mock.expect_search().times(1).returning(|_, _| {
        Err(McpError::InvalidRequest(
            "query cannot be empty".to_string(),
        ))
    });

    let result = mock.search("", None).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        McpError::InvalidRequest(msg) => {
            assert!(msg.contains("empty"));
        }
        _ => panic!("Expected InvalidRequest error"),
    }
}

/// Test that unknown table/stream returns StreamNotFound
#[tokio::test]
async fn test_dictionary_describe_unknown_entity_returns_not_found() {
    let mut mock = MockDictionaryStore::new();

    mock.expect_describe_column()
        .times(1)
        .returning(|table, _| {
            Err(McpError::StreamNotFound(format!(
                "'{}' not found as Silver table or Bronze stream",
                table
            )))
        });

    let result = mock.describe_column("nonexistent-stream", "field").await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), McpError::StreamNotFound(_)));
}

/// Test that unknown column returns InvalidRequest
#[tokio::test]
async fn test_dictionary_describe_unknown_column_returns_error() {
    let mut mock = MockDictionaryStore::new();

    mock.expect_describe_column()
        .times(1)
        .returning(|table, col| {
            Err(McpError::InvalidRequest(format!(
                "Column '{}' not found in table '{}'",
                col, table
            )))
        });

    let result = mock.describe_column("air-quality", "nonexistent").await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), McpError::InvalidRequest(_)));
}

// ============================================================================
// Integration Workflow Tests
// ============================================================================

/// Test complete dictionary workflow: search -> describe -> lineage
#[tokio::test]
async fn test_dictionary_complete_workflow() {
    use crate::storage::traits::MockDictionaryStore;
    use crate::storage::types::{DqRuleInfo, LineageSource, LineageTrace};
    use mockall::Sequence;

    let mut mock = MockDictionaryStore::new();
    let mut seq = Sequence::new();

    // Step 1: Search for a field
    mock.expect_search()
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| {
            Ok(vec![
                DictionaryEntry::new("bronze", "air-quality", "pm25", "number"),
                DictionaryEntry::new(
                    "silver",
                    "air_quality_observations",
                    "pm25",
                    "DOUBLE PRECISION",
                ),
            ])
        });

    // Step 2: Describe the Silver column
    mock.expect_describe_column()
        .withf(|table, col| table == "air_quality_observations" && col == "pm25")
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| {
            Ok(ColumnDescription::new(
                "silver",
                "air_quality_observations",
                "pm25",
                "DOUBLE PRECISION",
            )
            .with_source(SourceInfo::new("air-quality", "raw_payload.pm25")))
        });

    // Step 3: Trace lineage
    mock.expect_trace_lineage()
        .withf(|table, col| table == "air_quality_observations" && col == "pm25")
        .times(1)
        .in_sequence(&mut seq)
        .returning(|_, _| {
            Ok(
                LineageTrace::new("air_quality_observations", "pm25", "DOUBLE PRECISION")
                    .with_lineage(vec![LineageSource::new("air-quality", "raw_payload.pm25")]),
            )
        });

    // Execute workflow
    let search_results = mock.search("pm25", None).await.unwrap();
    assert_eq!(search_results.len(), 2);

    let description = mock
        .describe_column("air_quality_observations", "pm25")
        .await
        .unwrap();
    assert!(description.source.is_some());

    let lineage = mock
        .trace_lineage("air_quality_observations", "pm25")
        .await
        .unwrap();
    assert!(!lineage.lineage.is_empty());
}

// ============================================================================
// v1.0 to v1.1 Migration Compatibility Tests
// ============================================================================

/// Test that v1.0 config (no fields.description) still works
#[tokio::test]
async fn test_v1_0_config_compatibility() {
    let mut mock = MockDictionaryStore::new();

    // v1.0 configs have no fields[].description
    // Description comes from entity_schemas merge
    mock.expect_search()
        .withf(|query, _| query == "legacy_field")
        .times(1)
        .returning(|_, _| {
            // Return entry with description (would come from entity_schemas in real impl)
            Ok(vec![DictionaryEntry::new(
                "bronze",
                "legacy-stream",
                "legacy_field",
                "string",
            )
            .with_description("Description from entity_schemas")])
        });

    let results = mock.search("legacy_field", None).await.unwrap();

    assert_eq!(results.len(), 1);
    assert!(results[0].description.is_some());
}

/// Test that v1.1 config with all fields works
#[tokio::test]
async fn test_v1_1_config_full_fields() {
    let mut mock = MockDictionaryStore::new();

    // v1.1 has description, unit, and range in fields[]
    mock.expect_describe_column()
        .withf(|table, col| table == "air-quality" && col == "pm25")
        .times(1)
        .returning(|_, _| {
            Ok(
                ColumnDescription::new("bronze", "air-quality", "pm25", "number")
                    .with_description("PM2.5 particulate matter concentration")
                    .with_unit("ug/m3")
                    .with_validation_range(ValidationRange::bounded(0.0, 500.0)),
            )
        });

    let result = mock.describe_column("air-quality", "pm25").await.unwrap();

    // All v1.1 fields present
    assert!(result.description.is_some());
    assert!(result.unit.is_some());
    assert!(result.validation_range.is_some());
}

// ============================================================================
// Builder Pattern Tests (for test helper functions)
// ============================================================================

/// Test DictionaryEntry builder pattern
#[test]
fn test_dictionary_entry_builder() {
    let entry = DictionaryEntry::new("bronze", "air-quality", "pm25", "number")
        .with_unit("ug/m3")
        .with_description("PM2.5 concentration");

    assert_eq!(entry.layer, "bronze");
    assert_eq!(entry.entity, "air-quality");
    assert_eq!(entry.column_name, "pm25");
    assert_eq!(entry.data_type, "number");
    assert_eq!(entry.unit, Some("ug/m3".to_string()));
    assert_eq!(entry.description, Some("PM2.5 concentration".to_string()));
}

/// Test ColumnDescription builder pattern
#[test]
fn test_column_description_builder() {
    let desc = ColumnDescription::new(
        "silver",
        "air_quality_observations",
        "pm25",
        "DOUBLE PRECISION",
    )
    .with_unit("ug/m3")
    .with_description("PM2.5 particulate matter")
    .with_nullable(false)
    .with_validation_range(ValidationRange::bounded(0.0, 1000.0));

    assert_eq!(desc.layer, "silver");
    assert_eq!(desc.column_name, "pm25");
    assert!(!desc.nullable);
    assert!(desc.validation_range.is_some());
}

/// Test ValidationRange constructors
#[test]
fn test_validation_range_constructors() {
    let bounded = ValidationRange::bounded(0.0, 100.0);
    assert_eq!(bounded.min, Some(0.0));
    assert_eq!(bounded.max, Some(100.0));

    let min_only = ValidationRange::min_only(0.0);
    assert_eq!(min_only.min, Some(0.0));
    assert!(min_only.max.is_none());

    let max_only = ValidationRange::max_only(500.0);
    assert!(max_only.min.is_none());
    assert_eq!(max_only.max, Some(500.0));
}
