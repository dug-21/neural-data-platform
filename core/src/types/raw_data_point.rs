//! Bronze layer raw data point - stores exact source payloads without transformation.
//!
//! This struct implements the raw JSON storage model from ADR-001. Key principles:
//! - `raw_payload` is sacred: exactly what the source sent
//! - `context` is a snapshot: config-derived metadata frozen at ingestion time
//! - No parsing in Bronze: field extraction happens in Silver layer

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Bronze layer record - raw JSON storage
///
/// # Schema
/// | Column | Type | Description |
/// |--------|------|-------------|
/// | `timestamp` | DateTime | Ingestion timestamp |
/// | `source_id` | String | Source identifier (e.g., "air-quality-Http") |
/// | `ndp_id` | String? | Platform-assigned stable identifier |
/// | `context` | JSON? | Config-derived metadata snapshot |
/// | `raw_payload` | JSON | Exact payload from source |
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawDataPoint {
    /// Ingestion timestamp (when NDP received the message)
    pub timestamp: DateTime<Utc>,

    /// Source identifier in format "{stream_id}-{source_type}"
    /// Examples: "air-quality-Http", "outdoor-weather-Mqtt"
    pub source_id: String,

    /// Platform-assigned stable identifier (from config ndp_id field)
    /// Example: "airgradient-office-001"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ndp_id: Option<String>,

    /// Config-derived metadata snapshot at ingestion time
    /// Stored as JSON blob; queried via DuckDB/JSONB operators
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Value>,

    /// Exact payload from source, untransformed
    /// Contains all fields, types, and nested structures as received
    pub raw_payload: Value,
}

impl Default for RawDataPoint {
    fn default() -> Self {
        Self {
            timestamp: Utc::now(),
            source_id: String::new(),
            ndp_id: None,
            context: None,
            raw_payload: Value::Null,
        }
    }
}

impl RawDataPoint {
    /// Create a new RawDataPoint with required fields
    pub fn new(source_id: impl Into<String>, raw_payload: Value) -> Self {
        Self {
            timestamp: Utc::now(),
            source_id: source_id.into(),
            ndp_id: None,
            context: None,
            raw_payload,
        }
    }

    /// Set custom timestamp
    pub fn with_timestamp(mut self, timestamp: DateTime<Utc>) -> Self {
        self.timestamp = timestamp;
        self
    }

    /// Set ndp_id metadata
    pub fn with_ndp_id(mut self, ndp_id: impl Into<String>) -> Self {
        self.ndp_id = Some(ndp_id.into());
        self
    }

    /// Set context metadata
    pub fn with_context(mut self, context: Value) -> Self {
        self.context = Some(context);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    // ========== TDD CYCLE 1: RawDataPoint Construction ==========

    #[test]
    fn test_construction_all_fields() {
        let point = RawDataPoint {
            timestamp: Utc::now(),
            source_id: "test-Http".to_string(),
            ndp_id: Some("device-001".to_string()),
            context: Some(json!({"room": "office"})),
            raw_payload: json!({"pm25": 12.5, "status": "active"}),
        };

        assert_eq!(point.source_id, "test-Http");
        assert_eq!(point.ndp_id, Some("device-001".to_string()));
        assert_eq!(point.raw_payload["pm25"], 12.5);
        assert_eq!(point.raw_payload["status"], "active");
    }

    #[test]
    fn test_construction_minimal_fields() {
        let point = RawDataPoint::new("minimal-Http", json!({"value": 42}));

        assert_eq!(point.source_id, "minimal-Http");
        assert!(point.ndp_id.is_none());
        assert!(point.context.is_none());
        assert_eq!(point.raw_payload["value"], 42);
    }

    #[test]
    fn test_builder_pattern() {
        let point = RawDataPoint::new("test-Http", json!({"value": 42}))
            .with_ndp_id("test-001")
            .with_context(json!({"room": "lab"}));

        assert_eq!(point.source_id, "test-Http");
        assert_eq!(point.ndp_id, Some("test-001".to_string()));
        assert_eq!(point.context.unwrap()["room"], "lab");
    }

    #[test]
    fn test_builder_with_timestamp() {
        let custom_time = Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap();
        let point = RawDataPoint::new("test-Http", json!({"value": 1})).with_timestamp(custom_time);

        assert_eq!(point.timestamp, custom_time);
    }

    #[test]
    fn test_default_implementation() {
        let point = RawDataPoint::default();

        assert!(point.source_id.is_empty());
        assert!(point.ndp_id.is_none());
        assert!(point.context.is_none());
        assert!(point.raw_payload.is_null());
    }

    // ========== TDD CYCLE 2: RawDataPoint Serialization ==========

    #[test]
    fn test_serialization_round_trip() {
        let original = RawDataPoint::new("test-Http", json!({"value": 42}))
            .with_ndp_id("test-001")
            .with_context(json!({"key": "value"}));

        let json_str = serde_json::to_string(&original).unwrap();
        let restored: RawDataPoint = serde_json::from_str(&json_str).unwrap();

        assert_eq!(original, restored);
    }

    #[test]
    fn test_serialization_with_fixed_timestamp() {
        let original = RawDataPoint {
            timestamp: Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap(),
            source_id: "test-Http".to_string(),
            ndp_id: Some("test-001".to_string()),
            context: Some(json!({"nested": {"key": "value"}})),
            raw_payload: json!({"array": [1, 2, 3], "bool": true}),
        };

        let json_str = serde_json::to_string(&original).unwrap();
        let restored: RawDataPoint = serde_json::from_str(&json_str).unwrap();

        assert_eq!(original, restored);
    }

    #[test]
    fn test_serialization_skips_none_fields() {
        let point = RawDataPoint::new("test-Http", json!({"value": 42}));

        let json_str = serde_json::to_string(&point).unwrap();

        // When ndp_id and context are None, they should not appear in JSON
        assert!(!json_str.contains("ndp_id"));
        assert!(!json_str.contains("context"));
    }

    // ========== TDD CYCLE 3: RawDataPoint Type Preservation ==========

    #[test]
    fn test_preserves_non_numeric_types() {
        let point = RawDataPoint::new(
            "test-source",
            json!({
                "string": "hello",
                "boolean": true,
                "null": null,
                "array": [1, "two", false],
                "object": {"nested": "value"}
            }),
        );

        assert_eq!(point.raw_payload["string"], "hello");
        assert_eq!(point.raw_payload["boolean"], true);
        assert!(point.raw_payload["null"].is_null());
        assert_eq!(point.raw_payload["array"][1], "two");
        assert_eq!(point.raw_payload["object"]["nested"], "value");
    }

    #[test]
    fn test_preserves_numeric_types() {
        let point = RawDataPoint::new(
            "test-source",
            json!({
                "integer": 42,
                "float": 3.14159,
                "negative": -100,
                "zero": 0
            }),
        );

        assert_eq!(point.raw_payload["integer"], 42);
        assert_eq!(point.raw_payload["float"], 3.14159);
        assert_eq!(point.raw_payload["negative"], -100);
        assert_eq!(point.raw_payload["zero"], 0);
    }

    #[test]
    fn test_preserves_deeply_nested_structures() {
        let point = RawDataPoint::new(
            "test-source",
            json!({
                "level1": {
                    "level2": {
                        "level3": {
                            "value": "deep"
                        }
                    }
                }
            }),
        );

        assert_eq!(
            point.raw_payload["level1"]["level2"]["level3"]["value"],
            "deep"
        );
    }

    #[test]
    fn test_preserves_array_of_objects() {
        let point = RawDataPoint::new(
            "test-source",
            json!({
                "sensors": [
                    {"id": 1, "value": 23.5},
                    {"id": 2, "value": 24.1},
                    {"id": 3, "value": 22.9}
                ]
            }),
        );

        assert_eq!(point.raw_payload["sensors"].as_array().unwrap().len(), 3);
        assert_eq!(point.raw_payload["sensors"][0]["id"], 1);
        assert_eq!(point.raw_payload["sensors"][1]["value"], 24.1);
    }

    #[test]
    fn test_context_preserves_complex_metadata() {
        let complex_context = json!({
            "location": {
                "building": "A",
                "floor": 2,
                "room": "Office 201"
            },
            "calibration": {
                "date": "2026-01-01",
                "offset": 0.5,
                "verified": true
            },
            "tags": ["indoor", "primary", "calibrated"]
        });

        let point = RawDataPoint::new("test-Http", json!({})).with_context(complex_context.clone());

        assert_eq!(point.context, Some(complex_context));
    }
}
