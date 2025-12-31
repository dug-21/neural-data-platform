# ADR-002: Context Flattening Approach

## Status

Superseded by [ADR-002-AMENDMENT-002](./ADR-002-AMENDMENT-002-simple-blob.md)

> **Note**: This ADR and its first amendment (hybrid approach) have been superseded. The final decision is to store context as a simple JSON blob with no flattening and no promoted fields. See ADR-002-AMENDMENT-002 for the current design.

## Date

2025-12-31

## Context

NDP supports a nested `context` structure in YAML configuration to describe source attributes:

```yaml
context:
  location:
    coordinates: [29.958, -81.308]
    type: indoor
    path: home/upstairs/office
  device_type: airgradient
  model: ONE-V9
  tags: [primary, calibrated]
```

This context must be stored with every record for point-in-time accuracy. We need to decide how to handle the nested structure in storage (Parquet, TimescaleDB).

### Requirements

1. **Dynamic Keys**: Context fields are not hardcoded; users can add any key
2. **Coordinates Special Case**: GPS coordinates must be preserved as a tuple for geospatial queries
3. **Query Efficiency**: Common fields should be efficiently queryable
4. **Storage Compatibility**: Must work with both Parquet and TimescaleDB
5. **Backward Compatibility**: Records without context should continue to work

### Options Under Consideration

1. **Flatten with Dot Notation**: `location.type` becomes a column/key
2. **Preserve Nested JSON**: Store entire context as JSON blob
3. **Hybrid**: Flatten most fields, preserve specific structures

## Decision

**Flatten context at ingestion time using dot-notation, with special preservation of coordinates and arrays.**

### Flattening Rules

| Nested Path | Flattened Key | Behavior |
|-------------|--------------|----------|
| `location.type` | `location.type` | Flatten to string |
| `location.path` | `location.path` | Flatten to string |
| `location.coordinates` | `location.coordinates` | Preserve as `[f64, f64]` tuple |
| `device_type` | `device_type` | Passthrough (already flat) |
| `tags` | `tags` | Preserve as `Vec<String>` |
| `nested.deep.value` | `nested.deep.value` | Flatten any depth |

### Algorithm

```rust
pub fn flatten_context(
    context: &serde_json::Value,
    prefix: &str,
) -> HashMap<String, serde_json::Value> {
    let mut result = HashMap::new();

    if let Some(obj) = context.as_object() {
        for (key, value) in obj {
            let full_key = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{}.{}", prefix, key)
            };

            match (key.as_str(), value) {
                // SPECIAL CASE: Preserve coordinates as tuple
                ("coordinates", _) => {
                    result.insert(full_key, value.clone());
                }

                // SPECIAL CASE: Preserve arrays of primitives
                (_, serde_json::Value::Array(arr))
                    if !arr.is_empty() && !arr[0].is_object() => {
                    result.insert(full_key, value.clone());
                }

                // RECURSIVE: Flatten nested objects
                (_, serde_json::Value::Object(_)) => {
                    let nested = flatten_context(value, &full_key);
                    result.extend(nested);
                }

                // PASSTHROUGH: Scalar values
                _ => {
                    result.insert(full_key, value.clone());
                }
            }
        }
    }

    result
}
```

### Integration Point

Flattening occurs in the ingestion layer, BEFORE writing to Bronze:

```
Raw MQTT/HTTP Data
        |
        v
+------------------+
| Source Handler   |
| (MqttHandler,    |
|  HttpPoller)     |
+------------------+
        |
        v (attach context from config)
+------------------+
| ContextFlattener | <-- NEW MODULE
+------------------+
        |
        v (record with flat context)
+------------------+
| Channel -> Writer|
+------------------+
        |
        v
+------------------+
| ParquetStore     |
| (Bronze Layer)   |
+------------------+
```

### Example Transformation

**Input (from config):**
```yaml
context:
  location:
    coordinates: [29.958, -81.308]
    type: indoor
    path: home/upstairs/office
  device_type: airgradient
  model: ONE-V9
  tags: [primary, calibrated]
```

**Output (flattened):**
```json
{
  "location.coordinates": [29.958, -81.308],
  "location.type": "indoor",
  "location.path": "home/upstairs/office",
  "device_type": "airgradient",
  "model": "ONE-V9",
  "tags": ["primary", "calibrated"]
}
```

**In Parquet/TimescaleDB:**
```
| ndp_id                  | location.coordinates | location.type | location.path          | device_type  | ... |
|-------------------------|---------------------|---------------|------------------------|--------------|-----|
| airgradient-office-001  | [29.958, -81.308]   | indoor        | home/upstairs/office   | airgradient  | ... |
```

## Consequences

### Positive

1. **Efficient Queries**: Can query individual context fields directly:
   ```sql
   WHERE "location.type" = 'indoor'
   ```

2. **Coordinates Preserved**: Geospatial queries work:
   ```sql
   WHERE "location.coordinates" @> ARRAY[29.958, -81.308]
   ```

3. **Schema Flexibility**: New context fields automatically appear as new columns/keys

4. **Consistent Storage**: Same flattened structure in Bronze (Parquet) and Silver (TimescaleDB)

5. **No Parsing Overhead**: No JSON parsing needed at query time for individual fields

### Negative

1. **Column Explosion**: Many context fields = many columns in Parquet

2. **Dot in Column Names**: Some tools struggle with dots in column names
   - Mitigation: Quote column names in SQL: `"location.type"`
   - Mitigation: Parquet supports arbitrary column names

3. **Reconstruction Overhead**: Reconstructing nested structure requires parsing keys
   - Mitigation: Store original JSON in Silver layer (see ADR-003)

4. **No Deep Nesting Validation**: Arbitrarily deep nesting could create very long keys
   - Mitigation: Limit nesting depth to 3 levels

### Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Column name collisions | Prefix all context columns with `ctx.` or use JSONB in Silver |
| Very long key names | Validate max key length (64 chars) |
| Array of objects | Serialize to JSON string (not common case) |
| Empty context | Pass empty HashMap, no special handling needed |

## Alternatives Considered

### Alternative 1: Store as Nested JSON Blob

```json
// Store entire context as single JSON column
{
  "context": {
    "location": { "type": "indoor", "coordinates": [29.958, -81.308] }
  }
}
```

**Rejected because**:
- Requires JSON parsing for every query
- Cannot index individual fields efficiently
- Different query syntax for Parquet vs TimescaleDB

### Alternative 2: Fully Flatten Everything (Including Coordinates)

```json
{
  "location.coordinates.0": 29.958,
  "location.coordinates.1": -81.308
}
```

**Rejected because**:
- Breaks geospatial queries (need tuple/array)
- Awkward reconstruction
- Inconsistent with how coordinates are typically used

### Alternative 3: Predefined Schema Columns

```sql
-- Fixed columns for known context fields
CREATE TABLE readings (
    location_type TEXT,
    location_path TEXT,
    location_lat DOUBLE PRECISION,
    location_lon DOUBLE PRECISION,
    device_type TEXT
);
```

**Rejected because**:
- Not dynamic (can't add new context fields without schema change)
- Violates requirement for user-defined context
- Requires code changes for each new field

### Alternative 4: Flatten at Read Time

```sql
-- Flatten during query, not storage
SELECT context->>'location.type' as location_type
FROM readings;
```

**Rejected because**:
- Query performance penalty on every read
- Inconsistent representations between Bronze and Silver
- More complex query patterns

## Implementation Details

### New Module: `core/src/ingestion/context_flattener.rs`

```rust
//! Context flattening for ingestion pipeline.
//!
//! Converts nested YAML/JSON context structures into flat key-value maps
//! suitable for columnar storage (Parquet) and relational databases (TimescaleDB).

use serde_json::Value;
use std::collections::HashMap;

/// Maximum allowed nesting depth for context structures
const MAX_DEPTH: usize = 5;

/// Maximum allowed key length after flattening
const MAX_KEY_LENGTH: usize = 128;

/// Flatten a nested context structure into dot-notation keys.
///
/// # Special Cases
/// - `coordinates` keys are preserved as arrays (for geospatial)
/// - Simple arrays (non-object elements) are preserved
/// - Nested objects are recursively flattened
///
/// # Example
/// ```
/// let context = json!({
///     "location": {
///         "type": "indoor",
///         "coordinates": [29.958, -81.308]
///     }
/// });
/// let flat = flatten_context(&context);
/// assert_eq!(flat.get("location.type"), Some(&json!("indoor")));
/// assert_eq!(flat.get("location.coordinates"), Some(&json!([29.958, -81.308])));
/// ```
pub fn flatten_context(context: &Value) -> HashMap<String, Value> {
    flatten_recursive(context, "", 0)
}

fn flatten_recursive(
    value: &Value,
    prefix: &str,
    depth: usize,
) -> HashMap<String, Value> {
    let mut result = HashMap::new();

    if depth > MAX_DEPTH {
        // Exceed max depth: serialize as JSON string
        result.insert(prefix.to_string(), value.clone());
        return result;
    }

    if let Some(obj) = value.as_object() {
        for (key, val) in obj {
            let full_key = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{}.{}", prefix, key)
            };

            if full_key.len() > MAX_KEY_LENGTH {
                // Key too long: truncate and warn
                tracing::warn!("Context key exceeds max length: {}", full_key);
                continue;
            }

            if key == "coordinates" {
                // Preserve coordinates as tuple
                result.insert(full_key, val.clone());
            } else if let Some(arr) = val.as_array() {
                if arr.is_empty() || !arr[0].is_object() {
                    // Preserve simple arrays
                    result.insert(full_key, val.clone());
                } else {
                    // Array of objects: serialize to JSON
                    result.insert(full_key, val.clone());
                }
            } else if val.is_object() {
                // Recurse into nested objects
                let nested = flatten_recursive(val, &full_key, depth + 1);
                result.extend(nested);
            } else {
                // Scalar value: passthrough
                result.insert(full_key, val.clone());
            }
        }
    }

    result
}
```

### Integration with MqttHandler

```rust
impl MqttHandler {
    fn parse_message(&self, payload: &[u8]) -> Result<TimeSeriesPoint, Error> {
        let raw_point = self.parser.parse(payload)?;

        // Attach ndp_id and flattened context
        let mut point = raw_point;

        if let Some(ref ndp_id) = self.config.ndp_id {
            point.tags.insert("ndp_id".to_string(), ndp_id.clone());
        }

        if let Some(ref context) = self.config.context {
            let flat_context = flatten_context(context);
            for (key, value) in flat_context {
                // Convert JSON value to string for tags
                let str_value = match value {
                    Value::String(s) => s,
                    Value::Array(arr) => serde_json::to_string(&arr).unwrap_or_default(),
                    other => other.to_string(),
                };
                point.tags.insert(key, str_value);
            }
        }

        Ok(point)
    }
}
```

## Related Decisions

- [ADR-001: ndp_id Design](./ADR-001-ndp-id-design.md) - ndp_id placement
- [ADR-003: Silver Layer Schema Choice](./ADR-003-silver-layer-schema.md) - JSONB vs columns

## References

- [Parquet Column Naming](https://parquet.apache.org/docs/file-format/metadata/) - Column name rules
- [TimescaleDB JSONB Performance](https://docs.timescale.com/use-timescale/latest/query-data/advanced-analytic-queries/) - JSONB indexing
- [Flat vs Nested JSON Debate](https://stackoverflow.com/questions/16597123/flatten-or-not-to-flatten-a-json-for-mongodb) - Trade-off discussion
