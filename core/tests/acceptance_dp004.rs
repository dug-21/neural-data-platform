//! DP-004 Acceptance Tests: Bronze Layer Raw JSON Schema
//!
//! E2E tests for RawDataPoint storage and querying via ParquetStore.
//! These tests verify the full pipeline from RawDataPoint creation through
//! Parquet storage to query, ensuring data round-trip preserves all fields.
//!
//! ## Test Cases (from TEST_CASES.md)
//!
//! - AT-001: RawDataPoint stored and queryable (single point round-trip)
//! - AT-002: Multiple source types (multi-source filtering)
//! - AT-004: JSON structure preservation (nested fields extractable)
//!
//! ## London School TDD Approach
//!
//! These tests verify BEHAVIOR, not implementation details:
//! - Test the FULL pipeline from creation to storage to query
//! - Use TempDir for isolation
//! - Verify data round-trip preserves all fields exactly

use chrono::{TimeZone, Utc};
use platform_core::storage::ParquetStore;
use platform_core::traits::RawStore;
use platform_core::types::RawDataPoint;
use tempfile::TempDir;

// =============================================================================
// AT-001: RAWDATAPOINT STORED AND QUERYABLE
// =============================================================================

/// AT-001: Verify RawDataPoint can be written and queried back with all fields preserved.
///
/// GIVEN: ParquetStore with temp directory AND RawDataPoint with all fields populated
/// WHEN: write_raw is called
/// THEN: query_raw returns the same data AND raw_payload matches exactly (not parsed)
#[tokio::test]
async fn acceptance_raw_data_point_stored_and_queryable() {
    // GIVEN: ParquetStore with temp directory
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let store = ParquetStore::new(temp_dir.path()).expect("Failed to create ParquetStore");

    // AND: RawDataPoint with all fields populated
    let timestamp = Utc.with_ymd_and_hms(2026, 1, 15, 10, 30, 0).unwrap();
    let raw_payload = serde_json::json!({
        "pm25": 12.5,
        "pm10": 25.3,
        "temperature": 22.1,
        "humidity": 65.0,
        "co2": 450,
        "firmware": "v2.1.0",
        "status": "active",
        "nested": {
            "sensor_id": "abc123",
            "calibration": {
                "offset": 0.5,
                "date": "2026-01-01"
            }
        }
    });

    let context = serde_json::json!({
        "room": "Office 201",
        "floor": 2,
        "building": "Building A"
    });

    let original = RawDataPoint::new("air-quality-Http", raw_payload.clone())
        .with_timestamp(timestamp)
        .with_ndp_id("air-quality-office-001")
        .with_context(context.clone());

    // WHEN: write_raw is called
    store
        .write_raw(original.clone())
        .await
        .expect("Failed to write raw point");

    // THEN: query_raw returns the same data
    let start = timestamp - chrono::Duration::hours(1);
    let end = timestamp + chrono::Duration::hours(1);
    let results = store
        .query_raw(start, end, Some("air-quality-Http".to_string()))
        .await
        .expect("Failed to query raw points");

    assert_eq!(results.len(), 1, "Should return exactly 1 point");
    let retrieved = &results[0];

    // Verify all fields match exactly
    assert_eq!(
        retrieved.source_id, "air-quality-Http",
        "source_id should match"
    );
    assert_eq!(
        retrieved.ndp_id,
        Some("air-quality-office-001".to_string()),
        "ndp_id should match"
    );
    assert_eq!(retrieved.context, Some(context), "context should match");

    // AND: raw_payload matches exactly (not parsed)
    assert_eq!(
        retrieved.raw_payload, raw_payload,
        "raw_payload should match exactly"
    );

    // Verify nested structures are preserved
    assert_eq!(retrieved.raw_payload["pm25"], 12.5);
    assert_eq!(retrieved.raw_payload["nested"]["sensor_id"], "abc123");
    assert_eq!(
        retrieved.raw_payload["nested"]["calibration"]["offset"],
        0.5
    );
}

/// AT-001 Variant: Verify nullable fields (ndp_id, context) can be None
#[tokio::test]
async fn acceptance_raw_data_point_nullable_fields() {
    // GIVEN: ParquetStore
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let store = ParquetStore::new(temp_dir.path()).expect("Failed to create ParquetStore");

    // AND: RawDataPoint with only required fields (nullable fields are None)
    let timestamp = Utc.with_ymd_and_hms(2026, 1, 15, 11, 0, 0).unwrap();
    let raw_payload = serde_json::json!({"value": 42, "type": "minimal"});

    let original =
        RawDataPoint::new("minimal-source", raw_payload.clone()).with_timestamp(timestamp);
    // ndp_id and context are None

    // WHEN: write and query
    store.write_raw(original).await.expect("Failed to write");

    let results = store
        .query_raw(
            timestamp - chrono::Duration::hours(1),
            timestamp + chrono::Duration::hours(1),
            Some("minimal-source".to_string()),
        )
        .await
        .expect("Failed to query");

    // THEN: nullable fields should be None
    assert_eq!(results.len(), 1);
    assert!(results[0].ndp_id.is_none(), "ndp_id should be None");
    assert!(results[0].context.is_none(), "context should be None");
    assert_eq!(results[0].raw_payload, raw_payload);
}

// =============================================================================
// AT-002: MULTIPLE SOURCE TYPES
// =============================================================================

/// AT-002: Verify points from different sources are correctly filtered.
///
/// GIVEN: Two RawDataPoints from different sources
/// WHEN: Both are written
/// THEN: source_filter query returns correct subset
#[tokio::test]
async fn acceptance_multiple_source_types() {
    // GIVEN: ParquetStore
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let store = ParquetStore::new(temp_dir.path()).expect("Failed to create ParquetStore");

    let timestamp = Utc.with_ymd_and_hms(2026, 1, 15, 12, 0, 0).unwrap();

    // Two RawDataPoints from different sources
    let http_point = RawDataPoint::new(
        "air-quality-Http",
        serde_json::json!({
            "source": "http",
            "pm25": 15.5,
            "temperature": 23.0
        }),
    )
    .with_timestamp(timestamp)
    .with_ndp_id("http-sensor-001");

    let mqtt_point = RawDataPoint::new(
        "home-assistant-Mqtt",
        serde_json::json!({
            "source": "mqtt",
            "temperature": 21.5,
            "humidity": 55.0
        }),
    )
    .with_timestamp(timestamp)
    .with_ndp_id("mqtt-sensor-002");

    let webhook_point = RawDataPoint::new(
        "external-api-Webhook",
        serde_json::json!({
            "source": "webhook",
            "aqi": 42
        }),
    )
    .with_timestamp(timestamp)
    .with_ndp_id("webhook-source-003");

    // WHEN: Both are written
    store
        .write_raw(http_point)
        .await
        .expect("Failed to write HTTP point");
    store
        .write_raw(mqtt_point)
        .await
        .expect("Failed to write MQTT point");
    store
        .write_raw(webhook_point)
        .await
        .expect("Failed to write webhook point");

    let start = timestamp - chrono::Duration::hours(1);
    let end = timestamp + chrono::Duration::hours(1);

    // THEN: source_filter query returns correct subset

    // Filter by HTTP source
    let http_results = store
        .query_raw(start, end, Some("air-quality-Http".to_string()))
        .await
        .expect("Failed to query HTTP");
    assert_eq!(http_results.len(), 1, "Should return 1 HTTP point");
    assert_eq!(http_results[0].source_id, "air-quality-Http");
    assert_eq!(http_results[0].raw_payload["source"], "http");

    // Filter by MQTT source
    let mqtt_results = store
        .query_raw(start, end, Some("home-assistant-Mqtt".to_string()))
        .await
        .expect("Failed to query MQTT");
    assert_eq!(mqtt_results.len(), 1, "Should return 1 MQTT point");
    assert_eq!(mqtt_results[0].source_id, "home-assistant-Mqtt");
    assert_eq!(mqtt_results[0].raw_payload["source"], "mqtt");

    // Filter by Webhook source
    let webhook_results = store
        .query_raw(start, end, Some("external-api-Webhook".to_string()))
        .await
        .expect("Failed to query Webhook");
    assert_eq!(webhook_results.len(), 1, "Should return 1 Webhook point");
    assert_eq!(webhook_results[0].source_id, "external-api-Webhook");

    // Query ALL sources (no filter)
    let all_results = store
        .query_raw(start, end, None)
        .await
        .expect("Failed to query all");
    assert_eq!(all_results.len(), 3, "Should return all 3 points");
}

/// AT-002 Variant: Batch write with multiple sources partitions correctly
#[tokio::test]
async fn acceptance_batch_write_multiple_sources() {
    // GIVEN: ParquetStore
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let store = ParquetStore::new(temp_dir.path()).expect("Failed to create ParquetStore");

    let timestamp = Utc.with_ymd_and_hms(2026, 1, 15, 13, 0, 0).unwrap();

    // Multiple points from different sources in a single batch
    let points = vec![
        RawDataPoint::new("source-alpha", serde_json::json!({"index": 0}))
            .with_timestamp(timestamp),
        RawDataPoint::new("source-beta", serde_json::json!({"index": 1})).with_timestamp(timestamp),
        RawDataPoint::new("source-alpha", serde_json::json!({"index": 2}))
            .with_timestamp(timestamp),
        RawDataPoint::new("source-gamma", serde_json::json!({"index": 3}))
            .with_timestamp(timestamp),
        RawDataPoint::new("source-beta", serde_json::json!({"index": 4})).with_timestamp(timestamp),
    ];

    // WHEN: Batch write
    store
        .write_raw_batch(points)
        .await
        .expect("Failed to write batch");

    let start = timestamp - chrono::Duration::hours(1);
    let end = timestamp + chrono::Duration::hours(1);

    // THEN: Each source filter returns correct count
    let alpha = store
        .query_raw(start, end, Some("source-alpha".to_string()))
        .await
        .unwrap();
    assert_eq!(alpha.len(), 2, "source-alpha should have 2 points");

    let beta = store
        .query_raw(start, end, Some("source-beta".to_string()))
        .await
        .unwrap();
    assert_eq!(beta.len(), 2, "source-beta should have 2 points");

    let gamma = store
        .query_raw(start, end, Some("source-gamma".to_string()))
        .await
        .unwrap();
    assert_eq!(gamma.len(), 1, "source-gamma should have 1 point");

    // Total should be 5
    let all = store.query_raw(start, end, None).await.unwrap();
    assert_eq!(all.len(), 5, "Total should be 5 points");
}

// =============================================================================
// AT-004: JSON STRUCTURE PRESERVATION (DUCKDB-STYLE EXTRACTION)
// =============================================================================

/// AT-004: Verify nested JSON fields are preserved and extractable.
///
/// GIVEN: RawDataPoint with nested JSON
/// WHEN: Queried via query_raw
/// THEN: Nested fields are extractable (simulates DuckDB JSON functions)
#[tokio::test]
async fn acceptance_json_structure_preservation() {
    // GIVEN: ParquetStore
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let store = ParquetStore::new(temp_dir.path()).expect("Failed to create ParquetStore");

    let timestamp = Utc.with_ymd_and_hms(2026, 1, 15, 14, 0, 0).unwrap();

    // Complex nested JSON payload (like real sensor data)
    let complex_payload = serde_json::json!({
        "device": {
            "mac": "d83bda1cd074",
            "model": "AirGradient ONE",
            "firmware": {
                "version": "3.1.4",
                "build_date": "2026-01-01"
            }
        },
        "readings": {
            "air_quality": {
                "pm01": 5,
                "pm02": 8,
                "pm10": 12,
                "pm003_count": 500
            },
            "environment": {
                "temperature": 23.5,
                "humidity": 55.0,
                "pressure": 1013.25
            },
            "gas": {
                "tvoc_index": 100,
                "nox_index": 50,
                "co2": 450
            }
        },
        "metadata": {
            "timestamp_local": "2026-01-15T14:00:00+00:00",
            "boot_count": 42,
            "wifi_signal": -45,
            "array_data": [1, 2, 3, 4, 5]
        }
    });

    let point = RawDataPoint::new("airgradient-Http", complex_payload.clone())
        .with_timestamp(timestamp)
        .with_ndp_id("office-sensor-001")
        .with_context(serde_json::json!({"location": "Office 201"}));

    // WHEN: Write and query
    store.write_raw(point).await.expect("Failed to write");

    let results = store
        .query_raw(
            timestamp - chrono::Duration::hours(1),
            timestamp + chrono::Duration::hours(1),
            Some("airgradient-Http".to_string()),
        )
        .await
        .expect("Failed to query");

    // THEN: Nested fields are extractable
    assert_eq!(results.len(), 1);
    let payload = &results[0].raw_payload;

    // Device info
    assert_eq!(payload["device"]["mac"], "d83bda1cd074");
    assert_eq!(payload["device"]["model"], "AirGradient ONE");
    assert_eq!(payload["device"]["firmware"]["version"], "3.1.4");

    // Air quality readings
    assert_eq!(payload["readings"]["air_quality"]["pm01"], 5);
    assert_eq!(payload["readings"]["air_quality"]["pm02"], 8);
    assert_eq!(payload["readings"]["air_quality"]["pm10"], 12);

    // Environment readings
    assert_eq!(payload["readings"]["environment"]["temperature"], 23.5);
    assert_eq!(payload["readings"]["environment"]["humidity"], 55.0);

    // Gas readings
    assert_eq!(payload["readings"]["gas"]["co2"], 450);
    assert_eq!(payload["readings"]["gas"]["tvoc_index"], 100);

    // Metadata
    assert_eq!(payload["metadata"]["boot_count"], 42);
    assert_eq!(payload["metadata"]["wifi_signal"], -45);

    // Array data preserved
    assert!(payload["metadata"]["array_data"].is_array());
    assert_eq!(payload["metadata"]["array_data"][0], 1);
    assert_eq!(payload["metadata"]["array_data"][4], 5);
}

/// AT-004 Variant: Verify all JSON types are preserved (string, number, boolean, null, array, object)
#[tokio::test]
async fn acceptance_all_json_types_preserved() {
    // GIVEN: ParquetStore
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let store = ParquetStore::new(temp_dir.path()).expect("Failed to create ParquetStore");

    let timestamp = Utc.with_ymd_and_hms(2026, 1, 15, 15, 0, 0).unwrap();

    // Payload with ALL JSON types
    let all_types_payload = serde_json::json!({
        "string_value": "hello world",
        "integer_value": 42,
        "float_value": 3.14159,
        "negative_value": -273.15,
        "boolean_true": true,
        "boolean_false": false,
        "null_value": null,
        "empty_string": "",
        "zero": 0,
        "array_mixed": [1, "two", true, null, {"nested": "value"}],
        "object_empty": {},
        "object_nested": {
            "level1": {
                "level2": {
                    "level3": "deep"
                }
            }
        }
    });

    let point =
        RawDataPoint::new("types-test", all_types_payload.clone()).with_timestamp(timestamp);

    // WHEN: Write and query
    store.write_raw(point).await.expect("Failed to write");

    let results = store
        .query_raw(
            timestamp - chrono::Duration::hours(1),
            timestamp + chrono::Duration::hours(1),
            Some("types-test".to_string()),
        )
        .await
        .expect("Failed to query");

    // THEN: All types are preserved exactly
    assert_eq!(results.len(), 1);
    let payload = &results[0].raw_payload;

    // String types
    assert_eq!(payload["string_value"], "hello world");
    assert_eq!(payload["empty_string"], "");

    // Number types
    assert_eq!(payload["integer_value"], 42);
    assert_eq!(payload["float_value"], 3.14159);
    assert_eq!(payload["negative_value"], -273.15);
    assert_eq!(payload["zero"], 0);

    // Boolean types
    assert_eq!(payload["boolean_true"], true);
    assert_eq!(payload["boolean_false"], false);

    // Null type
    assert!(payload["null_value"].is_null());

    // Array with mixed types
    assert!(payload["array_mixed"].is_array());
    assert_eq!(payload["array_mixed"][0], 1);
    assert_eq!(payload["array_mixed"][1], "two");
    assert_eq!(payload["array_mixed"][2], true);
    assert!(payload["array_mixed"][3].is_null());
    assert_eq!(payload["array_mixed"][4]["nested"], "value");

    // Object types
    assert!(payload["object_empty"].is_object());
    assert_eq!(
        payload["object_nested"]["level1"]["level2"]["level3"],
        "deep"
    );
}

// =============================================================================
// TIME RANGE QUERY TESTS
// =============================================================================

/// Verify time range filtering works correctly
#[tokio::test]
async fn acceptance_time_range_filtering() {
    // GIVEN: ParquetStore
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let store = ParquetStore::new(temp_dir.path()).expect("Failed to create ParquetStore");

    // Points at different times
    let t1 = Utc.with_ymd_and_hms(2026, 1, 15, 10, 0, 0).unwrap();
    let t2 = Utc.with_ymd_and_hms(2026, 1, 15, 12, 0, 0).unwrap();
    let t3 = Utc.with_ymd_and_hms(2026, 1, 15, 14, 0, 0).unwrap();
    let t4 = Utc.with_ymd_and_hms(2026, 1, 15, 16, 0, 0).unwrap();

    let points = vec![
        RawDataPoint::new("time-test", serde_json::json!({"hour": 10})).with_timestamp(t1),
        RawDataPoint::new("time-test", serde_json::json!({"hour": 12})).with_timestamp(t2),
        RawDataPoint::new("time-test", serde_json::json!({"hour": 14})).with_timestamp(t3),
        RawDataPoint::new("time-test", serde_json::json!({"hour": 16})).with_timestamp(t4),
    ];

    // WHEN: Write batch
    store
        .write_raw_batch(points)
        .await
        .expect("Failed to write batch");

    // THEN: Time range queries return correct subsets

    // Query middle range (12:00 - 14:00)
    let middle = store
        .query_raw(t2, t3, Some("time-test".to_string()))
        .await
        .unwrap();
    assert_eq!(middle.len(), 2, "Middle range should have 2 points");

    // Query early range
    let early = store
        .query_raw(
            t1,
            t1 + chrono::Duration::hours(1),
            Some("time-test".to_string()),
        )
        .await
        .unwrap();
    assert_eq!(early.len(), 1, "Early range should have 1 point");
    assert_eq!(early[0].raw_payload["hour"], 10);

    // Query late range
    let late = store
        .query_raw(
            t4,
            t4 + chrono::Duration::hours(1),
            Some("time-test".to_string()),
        )
        .await
        .unwrap();
    assert_eq!(late.len(), 1, "Late range should have 1 point");
    assert_eq!(late[0].raw_payload["hour"], 16);

    // Query full day
    let full_day = store
        .query_raw(t1, t4, Some("time-test".to_string()))
        .await
        .unwrap();
    assert_eq!(full_day.len(), 4, "Full day should have all 4 points");
}

// =============================================================================
// PARTITION STRUCTURE TESTS
// =============================================================================

/// Verify partition directory structure is correct
#[tokio::test]
async fn acceptance_partition_structure() {
    // GIVEN: ParquetStore
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let store = ParquetStore::new(temp_dir.path()).expect("Failed to create ParquetStore");

    let timestamp = Utc.with_ymd_and_hms(2026, 6, 15, 14, 30, 0).unwrap();

    let point = RawDataPoint::new("partition-test-Http", serde_json::json!({"test": true}))
        .with_timestamp(timestamp);

    // WHEN: Write point
    store.write_raw(point).await.expect("Failed to write");

    // THEN: Correct partition directory structure exists
    // Expected: raw/{source_id}/year=YYYY/month=MM/day=DD/hour=HH/data.parquet
    let expected_dir = temp_dir
        .path()
        .join("raw")
        .join("partition-test-Http")
        .join("year=2026")
        .join("month=06")
        .join("day=15")
        .join("hour=14");

    assert!(expected_dir.exists(), "Partition directory should exist");
    assert!(
        expected_dir.join("data.parquet").exists(),
        "Parquet file should exist"
    );
}

// =============================================================================
// CONTEXT METADATA TESTS
// =============================================================================

/// Verify context metadata is preserved completely
#[tokio::test]
async fn acceptance_context_metadata_preserved() {
    // GIVEN: ParquetStore
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let store = ParquetStore::new(temp_dir.path()).expect("Failed to create ParquetStore");

    let timestamp = Utc.with_ymd_and_hms(2026, 1, 15, 16, 0, 0).unwrap();

    // Complex context metadata
    let context = serde_json::json!({
        "location": {
            "building": "Building A",
            "floor": 2,
            "room": "Office 201",
            "coordinates": {
                "lat": 37.7749,
                "lon": -122.4194
            }
        },
        "sensor": {
            "model": "AirGradient ONE",
            "serial": "AG-001-2026",
            "calibration": {
                "date": "2026-01-01",
                "technician": "John Doe"
            }
        },
        "tags": ["indoor", "office", "monitoring"]
    });

    let point = RawDataPoint::new("context-test", serde_json::json!({"value": 1}))
        .with_timestamp(timestamp)
        .with_context(context.clone());

    // WHEN: Write and query
    store.write_raw(point).await.expect("Failed to write");

    let results = store
        .query_raw(
            timestamp - chrono::Duration::hours(1),
            timestamp + chrono::Duration::hours(1),
            Some("context-test".to_string()),
        )
        .await
        .expect("Failed to query");

    // THEN: Context is preserved exactly
    assert_eq!(results.len(), 1);
    let stored_context = results[0].context.as_ref().expect("Context should exist");

    assert_eq!(stored_context["location"]["building"], "Building A");
    assert_eq!(stored_context["location"]["floor"], 2);
    assert_eq!(stored_context["location"]["coordinates"]["lat"], 37.7749);
    assert_eq!(stored_context["sensor"]["model"], "AirGradient ONE");
    assert_eq!(
        stored_context["sensor"]["calibration"]["technician"],
        "John Doe"
    );
    assert!(stored_context["tags"].is_array());
    assert_eq!(stored_context["tags"][0], "indoor");
}
