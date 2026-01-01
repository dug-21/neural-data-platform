# Pseudocode: RawDataPoint Struct

## Overview

The `RawDataPoint` struct represents a single raw data record in the Bronze layer. It stores the original source payload without transformation, enabling full replay and reprocessing capabilities.

## Related ADR

- [ADR-001: Bronze Layer Raw JSON Schema](../architecture/ADR-001-bronze-raw-json-schema.md)

---

## Struct Definition

```pseudocode
STRUCT RawDataPoint:
    timestamp: DateTime<Utc>      // When NDP received the message
    source_id: String             // Source identifier from config (e.g., "air-quality-Http")
    ndp_id: Option<String>        // Stable platform-owned identifier (from AIR-009)
    context: Option<JSON>         // Config-derived metadata snapshot at ingestion time
    raw_payload: JSON             // Exact payload from source, untransformed
```

---

## Constructor

```pseudocode
FUNCTION RawDataPoint::new(
    source_id: String,
    raw_payload: JSON
) -> RawDataPoint:

    RETURN RawDataPoint {
        timestamp: Utc::now(),
        source_id: source_id,
        ndp_id: None,
        context: None,
        raw_payload: raw_payload,
    }
END FUNCTION
```

### Constructor With Context (AIR-009 Compatible)

```pseudocode
FUNCTION RawDataPoint::with_context(
    source_id: String,
    raw_payload: JSON,
    ndp_id: Option<String>,
    context: Option<JSON>
) -> RawDataPoint:

    RETURN RawDataPoint {
        timestamp: Utc::now(),
        source_id: source_id,
        ndp_id: ndp_id,
        context: context,
        raw_payload: raw_payload,
    }
END FUNCTION
```

---

## Serialization to Parquet Row

```pseudocode
FUNCTION RawDataPoint::to_parquet_row(self) -> ParquetRow:
    // Convert struct fields to Parquet-compatible types

    row = ParquetRow::new()

    // Timestamp as microseconds since epoch (INT64)
    row.set("timestamp", self.timestamp.timestamp_micros())

    // Source ID as UTF8 string
    row.set("source_id", self.source_id)

    // NDP ID as nullable UTF8 string
    IF self.ndp_id IS Some(value):
        row.set("ndp_id", value)
    ELSE:
        row.set_null("ndp_id")
    END IF

    // Context as nullable UTF8 string (JSON serialized)
    IF self.context IS Some(json_value):
        row.set("context", json_value.to_string())
    ELSE:
        row.set_null("context")
    END IF

    // Raw payload as UTF8 string (JSON serialized)
    row.set("raw_payload", self.raw_payload.to_string())

    RETURN row
END FUNCTION
```

---

## From Source Payload Conversion

### HTTP Source Response

```pseudocode
FUNCTION RawDataPoint::from_http_response(
    response_body: String,
    source_id: String,
    parse_context: ParseContext
) -> Result<RawDataPoint>:

    // Parse the response body as JSON
    raw_payload = TRY json::parse(response_body)
    IF raw_payload IS Error:
        RETURN Error("Failed to parse HTTP response as JSON: {error}")
    END IF

    // Create RawDataPoint with context injection
    RETURN Ok(RawDataPoint {
        timestamp: Utc::now(),
        source_id: source_id,
        ndp_id: parse_context.ndp_id,
        context: parse_context.context,
        raw_payload: raw_payload,
    })
END FUNCTION
```

### MQTT Message (Legacy - for future reference)

```pseudocode
FUNCTION RawDataPoint::from_mqtt_message(
    topic: String,
    payload: Bytes,
    source_id: String,
    parse_context: ParseContext
) -> Result<RawDataPoint>:

    // Decode payload as UTF-8
    payload_str = TRY String::from_utf8(payload)
    IF payload_str IS Error:
        RETURN Error("Failed to decode MQTT payload as UTF-8")
    END IF

    // Parse as JSON
    raw_payload = TRY json::parse(payload_str)
    IF raw_payload IS Error:
        RETURN Error("Failed to parse MQTT payload as JSON: {error}")
    END IF

    // Create RawDataPoint with topic metadata in context
    enriched_context = parse_context.context.clone() OR json::empty_object()
    enriched_context["mqtt_topic"] = topic

    RETURN Ok(RawDataPoint {
        timestamp: Utc::now(),
        source_id: source_id,
        ndp_id: parse_context.ndp_id,
        context: Some(enriched_context),
        raw_payload: raw_payload,
    })
END FUNCTION
```

---

## Validation

```pseudocode
FUNCTION RawDataPoint::validate(self) -> Result<()>:
    // Timestamp validation
    max_future_offset = Duration::minutes(5)
    IF self.timestamp > Utc::now() + max_future_offset:
        RETURN Error("Timestamp is too far in the future")
    END IF

    // Source ID validation
    IF self.source_id.is_empty():
        RETURN Error("source_id cannot be empty")
    END IF

    IF NOT is_valid_source_id(self.source_id):
        RETURN Error("Invalid source_id format")
    END IF

    // Raw payload validation
    IF self.raw_payload.is_null():
        RETURN Error("raw_payload cannot be null")
    END IF

    RETURN Ok(())
END FUNCTION

FUNCTION is_valid_source_id(source_id: String) -> Bool:
    // Source ID format: {stream-id}-{SourceType}
    // Examples: "air-quality-Http", "outdoor-weather-Http"

    // Must contain at least one hyphen
    IF NOT source_id.contains("-"):
        RETURN false
    END IF

    // Must not contain whitespace
    IF source_id.contains_whitespace():
        RETURN false
    END IF

    RETURN true
END FUNCTION
```

---

## Serialization Traits

```pseudocode
// Implement Serialize for RawDataPoint
IMPLEMENT Serialize FOR RawDataPoint:
    FUNCTION serialize(self, serializer) -> Result:
        struct_serializer = serializer.serialize_struct("RawDataPoint", 5)

        struct_serializer.serialize_field("timestamp", self.timestamp)
        struct_serializer.serialize_field("source_id", self.source_id)

        // Skip None fields for compact JSON
        IF self.ndp_id IS Some(value):
            struct_serializer.serialize_field("ndp_id", value)
        END IF

        IF self.context IS Some(value):
            struct_serializer.serialize_field("context", value)
        END IF

        struct_serializer.serialize_field("raw_payload", self.raw_payload)

        RETURN struct_serializer.end()
    END FUNCTION
END IMPLEMENT

// Implement Deserialize for RawDataPoint
IMPLEMENT Deserialize FOR RawDataPoint:
    FUNCTION deserialize(deserializer) -> Result<RawDataPoint>:
        // Use serde's derive macro in Rust
        // Handle missing optional fields with defaults

        map = deserializer.deserialize_struct("RawDataPoint", FIELDS)

        RETURN RawDataPoint {
            timestamp: map.get("timestamp") OR RETURN Error("missing timestamp"),
            source_id: map.get("source_id") OR RETURN Error("missing source_id"),
            ndp_id: map.get_optional("ndp_id"),
            context: map.get_optional("context"),
            raw_payload: map.get("raw_payload") OR RETURN Error("missing raw_payload"),
        }
    END FUNCTION
END IMPLEMENT
```

---

## Rust Implementation Signature

```rust
/// Bronze layer record - raw JSON storage
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RawDataPoint {
    /// Ingestion timestamp (when NDP received the message)
    pub timestamp: DateTime<Utc>,
    /// Source identifier from config (e.g., "air-quality-Http")
    pub source_id: String,
    /// Stable platform-owned identifier (from ADR-001 air-009)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ndp_id: Option<String>,
    /// Config-derived metadata snapshot at ingestion time
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,
    /// Exact payload from source, untransformed
    pub raw_payload: serde_json::Value,
}

impl RawDataPoint {
    pub fn new(source_id: impl Into<String>, raw_payload: serde_json::Value) -> Self {
        Self {
            timestamp: Utc::now(),
            source_id: source_id.into(),
            ndp_id: None,
            context: None,
            raw_payload,
        }
    }

    pub fn with_context(
        source_id: impl Into<String>,
        raw_payload: serde_json::Value,
        ndp_id: Option<String>,
        context: Option<serde_json::Value>,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            source_id: source_id.into(),
            ndp_id,
            context,
            raw_payload,
        }
    }

    pub fn validate(&self) -> crate::error::CoreResult<()> {
        // Validation logic here
        Ok(())
    }
}
```

---

## File Location

**Target**: `core/src/traits.rs` (add alongside `TimeSeriesPoint`)

## Related Files

| File | Change |
|------|--------|
| `core/src/traits.rs` | Add `RawDataPoint` struct |
| `core/src/storage/parquet.rs` | Add write support for `RawDataPoint` |
| `core/src/sources/*.rs` | Return `RawDataPoint` instead of `Vec<TimeSeriesPoint>` |
