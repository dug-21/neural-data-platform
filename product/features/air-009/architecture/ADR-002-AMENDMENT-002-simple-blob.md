# ADR-002 Amendment 002: Simple Context Blob Storage

## Status

Accepted (supersedes ADR-002 and ADR-002-AMENDMENT-001)

## Date

2025-12-31

## Context

After reviewing both the original flattening approach (ADR-002) and the hybrid approach (ADR-002-AMENDMENT-001), we determined that simplicity should win. The complexity of promoted fields, column management, and dual query patterns outweighs the marginal query performance benefits.

### Problems with Previous Approaches

**Full Flattening (ADR-002):**
- Cannot reconstruct original structure
- Depth ambiguity with dot notation
- Schema explosion with many columns

**Hybrid (ADR-002-AMENDMENT-001):**
- Dual storage (promoted columns + blob) adds complexity
- Two query patterns to maintain
- Arbitrary decisions about what to promote
- Still requires column management

## Decision

**Store context as a single JSON blob at all layers. No flattening. No promoted fields.**

### Schema

**Bronze Layer (Parquet):**
```
├── timestamp: TIMESTAMP
├── ndp_id: STRING
├── context: STRING (JSON blob)
└── <measurement fields>
```

**Silver Layer (TimescaleDB):**
```sql
CREATE TABLE sensor_readings (
    time TIMESTAMPTZ NOT NULL,
    ndp_id TEXT NOT NULL,
    context JSONB,
    -- measurement columns
);

-- Indexes
CREATE INDEX idx_readings_ndp_id ON sensor_readings(ndp_id);
CREATE INDEX idx_readings_context ON sensor_readings USING GIN(context);
```

### Data Flow

```
YAML Config                    Bronze (Parquet)              Silver (TimescaleDB)
─────────────                  ────────────────              ────────────────────
context:                       ndp_id: "ag-001"              ndp_id: "ag-001"
  location:           ──►      context: '{"location":   ──►  context: {"location":
    type: indoor               {"type":"indoor",...}}'         {"type":"indoor",...}}
    coordinates: [..]
    path: home/office
  device_type: ag
```

### Algorithm

```rust
/// Process context for storage - simple JSON serialization
pub fn process_context(context: &serde_json::Value) -> String {
    serde_json::to_string(context).unwrap_or_default()
}

/// Record structure with context blob
pub struct EnrichedRecord {
    pub timestamp: DateTime<Utc>,
    pub ndp_id: String,
    pub context: String,  // JSON blob
    pub fields: HashMap<String, f64>,
}
```

### Querying

All context queries use JSONB operators in Silver:

```sql
-- Query by location type
SELECT * FROM sensor_readings
WHERE context->>'device_type' = 'airgradient';

-- Query nested location
SELECT * FROM sensor_readings
WHERE context->'location'->>'type' = 'indoor';

-- Query coordinates (array access)
SELECT * FROM sensor_readings
WHERE (context->'location'->'coordinates'->>0)::float > 29.0;

-- Query by tag
SELECT * FROM sensor_readings
WHERE context->'tags' ? 'calibrated';

-- Full text search on context
SELECT * FROM sensor_readings
WHERE context::text ILIKE '%office%';
```

### Bronze Layer Queries (Parquet)

For Bronze queries, use JSON functions in your query engine:

```sql
-- DuckDB example
SELECT * FROM 'bronze/*.parquet'
WHERE json_extract_string(context, '$.location.type') = 'indoor';

-- Spark example
SELECT * FROM bronze_table
WHERE get_json_object(context, '$.device_type') = 'airgradient';
```

## Consequences

### Positive

1. **Maximum simplicity** - One column, one format, one query pattern
2. **Full flexibility** - Any nested structure supported without schema changes
3. **No information loss** - Original structure always preserved
4. **Easier maintenance** - No promoted field management
5. **Clear mental model** - Context is a blob, period
6. **JSONB performance** - GIN indexes make queries fast enough for most use cases

### Negative

1. **No columnar optimization** - Context fields not individually optimized in Parquet
2. **Query syntax** - JSONB operators slightly more verbose than column access
3. **Index limitations** - GIN index covers all paths but less efficient than B-tree on specific columns

### Performance Considerations

| Query Type | Performance | Mitigation |
|------------|-------------|------------|
| `ndp_id = 'x'` | Fast (B-tree index) | Primary query pattern |
| `context->>'field' = 'x'` | Good (GIN index) | Sufficient for expected query volume |
| Complex nested queries | Moderate | Create expression indexes if needed |

**Expression index for hot paths (optional):**
```sql
-- Only if query patterns show need
CREATE INDEX idx_location_type ON sensor_readings ((context->'location'->>'type'));
```

## Rejected Alternatives

### Why Not Promoted Fields?

The hybrid approach with promoted fields was rejected because:
1. Arbitrary decisions about what to promote
2. Dual query patterns (column vs JSONB) create cognitive overhead
3. Storage duplication (promoted fields exist in both column and blob)
4. Schema changes required when promotion decisions change
5. Marginal performance benefit doesn't justify complexity

### Why Not Full Flattening?

The original flattening approach was rejected because:
1. Cannot reconstruct original nested structure
2. Ambiguity between `a.b` (flat key) and `a: {b: x}` (nested)
3. Awkward handling of arrays and deep nesting
4. Column explosion in Parquet

## Implementation Notes

### SourceConfig Update

```rust
pub struct SourceConfig {
    pub source_type: SourceType,
    pub enabled: bool,
    pub ndp_id: Option<String>,
    pub context: Option<serde_json::Value>,  // Preserved as-is
    pub params: HashMap<String, serde_json::Value>,
}
```

### Ingestion Handler

```rust
impl MqttHandler {
    fn enrich_record(&self, mut record: TimeSeriesPoint) -> TimeSeriesPoint {
        // Add ndp_id as a field/tag
        if let Some(ref ndp_id) = self.config.ndp_id {
            record.tags.insert("ndp_id".to_string(), ndp_id.clone());
        }

        // Serialize context as JSON blob
        if let Some(ref context) = self.config.context {
            let context_json = serde_json::to_string(context).unwrap_or_default();
            record.tags.insert("context".to_string(), context_json);
        }

        record
    }
}
```

### ETL (Bronze → Silver)

```rust
// Simple parse - no transformation needed
fn transform_context(bronze_context: &str) -> serde_json::Value {
    serde_json::from_str(bronze_context).unwrap_or(serde_json::Value::Null)
}
```

## Migration

No migration needed - this simplifies the approach before implementation begins.

## Related Decisions

- [ADR-001: ndp_id Design](./ADR-001-ndp-id-design.md) - ndp_id remains a separate column (not inside blob)
- [ADR-002: Context Flattening](./ADR-002-context-flattening.md) - Original decision (superseded)
- [ADR-002-AMENDMENT-001: Hybrid Context](./ADR-002-AMENDMENT-001-hybrid-context.md) - Hybrid approach (superseded)
- [ADR-003: Silver Layer Schema](./ADR-003-silver-layer-schema.md) - JSONB choice (aligned)
