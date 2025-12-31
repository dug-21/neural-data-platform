# ADR-002 Amendment 001: Hybrid Context Storage

## Status

Superseded by [ADR-002-AMENDMENT-002](./ADR-002-AMENDMENT-002-simple-blob.md)

> **Note**: This hybrid approach was superseded in favor of simple blob storage. The complexity of promoted fields and dual query patterns was deemed unnecessary. See ADR-002-AMENDMENT-002 for the current design.

## Date

2025-12-31

## Context

ADR-002 specified full flattening of the context structure at ingestion. During architectural review, we identified limitations:

1. **Reconstruction impossibility** - Once flattened, the original nested structure cannot be reliably rebuilt
2. **Depth ambiguity** - Cannot distinguish `{"a.b": "x"}` from `{"a": {"b": "x"}}`
3. **Array-of-objects handling** - Deeply nested or complex structures serialize awkwardly
4. **Future flexibility** - Unknown future context shapes may not flatten cleanly

## Decision

**Adopt a hybrid approach: promote specific fields to columns while preserving the original context blob.**

### Promoted Fields (Flattened)

Only these well-known fields are promoted to dedicated columns in Bronze:

| Field Path | Column Name | Type | Rationale |
|------------|-------------|------|-----------|
| `location.type` | `ctx_location_type` | STRING | Common filter criterion |
| `location.path` | `ctx_location_path` | STRING | Hierarchical queries |
| `location.coordinates` | `ctx_location_coordinates` | ARRAY<FLOAT64> | Geospatial queries |

### Preserved Blob

The complete, unmodified context is stored as:

| Layer | Column | Type |
|-------|--------|------|
| Bronze (Parquet) | `context_raw` | STRING (JSON) |
| Silver (TimescaleDB) | `context` | JSONB |

### Updated Schema

**Bronze Layer (Parquet):**
```
├── timestamp: TIMESTAMP
├── ndp_id: STRING
├── ctx_location_type: STRING (nullable)
├── ctx_location_path: STRING (nullable)
├── ctx_location_coordinates: LIST<DOUBLE> (nullable)
├── context_raw: STRING (JSON blob)
└── <measurement fields>
```

**Silver Layer (TimescaleDB):**
```sql
CREATE TABLE sensor_readings (
    time TIMESTAMPTZ NOT NULL,
    ndp_id TEXT NOT NULL,
    ctx_location_type TEXT,
    ctx_location_path TEXT,
    ctx_location_coordinates POINT,  -- PostGIS point
    context JSONB,                    -- Full structure preserved
    -- measurement columns
);

-- Indexes for common queries
CREATE INDEX idx_readings_ndp_id ON sensor_readings(ndp_id);
CREATE INDEX idx_readings_location_type ON sensor_readings(ctx_location_type);
CREATE INDEX idx_readings_context ON sensor_readings USING GIN(context);
```

## Algorithm Update

Replace the recursive flattening with selective promotion:

```rust
use serde_json::{Map, Value};

/// Promoted context fields with their extraction paths
const PROMOTED_FIELDS: &[(&str, &[&str])] = &[
    ("ctx_location_type", &["location", "type"]),
    ("ctx_location_path", &["location", "path"]),
    ("ctx_location_coordinates", &["location", "coordinates"]),
];

/// Result of context processing
pub struct ProcessedContext {
    /// Promoted fields extracted for columnar storage
    pub promoted: HashMap<String, Value>,
    /// Original context preserved as JSON string
    pub raw: String,
}

/// Process context: extract promoted fields, preserve original
pub fn process_context(context: &Value) -> ProcessedContext {
    let mut promoted = HashMap::new();

    // Extract each promoted field by path
    for (column_name, path) in PROMOTED_FIELDS {
        if let Some(value) = extract_path(context, path) {
            promoted.insert(column_name.to_string(), value.clone());
        }
    }

    // Preserve original as JSON string
    let raw = serde_json::to_string(context).unwrap_or_default();

    ProcessedContext { promoted, raw }
}

/// Extract value at a given path
fn extract_path<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = value;
    for &key in path {
        current = current.get(key)?;
    }
    Some(current)
}
```

## Example Transformation

**Input (from config):**
```yaml
context:
  location:
    coordinates: [29.958, -81.308]
    type: indoor
    path: home/upstairs/office
  device_type: airgradient
  model: ONE-V9
  calibration:
    sensor_a:
      offset: 0.5
      last_date: 2024-01-15
  tags: [primary, calibrated]
```

**Output (Bronze Parquet row):**
```
ndp_id:                    "airgradient-office-001"
ctx_location_type:         "indoor"
ctx_location_path:         "home/upstairs/office"
ctx_location_coordinates:  [29.958, -81.308]
context_raw:               '{"location":{"coordinates":[29.958,-81.308],"type":"indoor","path":"home/upstairs/office"},"device_type":"airgradient","model":"ONE-V9","calibration":{"sensor_a":{"offset":0.5,"last_date":"2024-01-15"}},"tags":["primary","calibrated"]}'
```

Note: `device_type`, `model`, `calibration`, and `tags` are NOT promoted columns—they live in `context_raw` and are queryable via JSON functions.

## Querying

**Query promoted field (fast, indexed):**
```sql
SELECT * FROM sensor_readings
WHERE ctx_location_type = 'indoor';
```

**Query arbitrary context field (via JSONB):**
```sql
SELECT * FROM sensor_readings
WHERE context->>'device_type' = 'airgradient';

SELECT * FROM sensor_readings
WHERE context->'calibration'->'sensor_a'->>'offset' = '0.5';
```

**Full context reconstruction:**
```sql
SELECT ndp_id, context FROM sensor_readings;
-- Returns complete original structure
```

## Adding New Promoted Fields

To promote additional fields in the future:

1. Add entry to `PROMOTED_FIELDS` constant
2. Add column to Parquet schema
3. Run backfill migration to extract from `context_raw`/`context`
4. Create index if query-heavy

This is a schema change but does NOT require re-ingesting data—the original context is preserved.

## Consequences

### Positive

1. **Full reconstruction** - Original context always available
2. **Query flexibility** - Promoted fields for speed, JSONB for anything else
3. **Future-proof** - Unknown structures stored safely
4. **Simpler ingestion** - No recursive flattening logic
5. **Clear promotion list** - Explicit, documented fields

### Negative

1. **Storage overhead** - Promoted fields duplicated in blob (~5-10% overhead)
2. **Two query patterns** - Column queries vs JSONB queries
3. **Promotion decisions** - Must explicitly choose what to promote

### Trade-off Analysis

| Aspect | Full Flatten (ADR-002) | Hybrid (This Amendment) |
|--------|------------------------|-------------------------|
| Storage | Columns only | Columns + blob |
| Query (promoted) | Fast | Fast (same) |
| Query (arbitrary) | Column if exists | JSONB extraction |
| Reconstruction | Impossible | Always possible |
| Complex nesting | Awkward | Preserved |
| Schema evolution | Add columns | Promote or use JSONB |

## Migration

For in-flight development (no production data):
- Update `ProcessedContext` struct
- Modify Parquet writer to include `context_raw`
- Update Bronze schema definition
- No data migration needed

## Related Decisions

- [ADR-002: Context Flattening Approach](./ADR-002-context-flattening.md) - Original decision (superseded by this amendment)
- [ADR-003: Silver Layer Schema Choice](./ADR-003-silver-layer-schema.md) - JSONB decision (aligned with this amendment)

## References

- Discussion: Architectural review session 2025-12-31
- Pattern: "Promote and preserve" common in data lake architectures
