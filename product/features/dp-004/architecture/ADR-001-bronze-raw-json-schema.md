# ADR-001: Bronze Layer Raw JSON Schema

## Status

Accepted

## Date

2026-01-01

## Context

The Neural Data Platform currently uses a "tall" schema for Bronze layer (Parquet) storage:

```
timestamp | location_id | metric | value | tags | ndp_id | context
12:00:00  | sensor-01   | pm02   | 12.5  | ...  | ...    | ...
12:00:00  | sensor-01   | rco2   | 450   | ...  | ...    | ...
12:00:00  | sensor-01   | atmp   | 22.1  | ...  | ...    | ...
```

This schema has several problems:

### Problem 1: Data Loss Through Type Coercion

Parsers transform incoming data at ingestion time:
- Numeric extraction: `"85"` → `85.0`
- Boolean mapping: `"open"` → (dropped or error)
- Field selection: Non-numeric fields ignored

**Impact**: Original source data is lost. If source changes from `"open"` to `"Open"` or `"opened"`, we cannot recover or remediate.

### Problem 2: Parser Coupling to Source Format

Each parser must understand the exact format of each source. When sources change versions:
- Field names change (e.g., `temp` → `temperature`)
- Value formats change (e.g., `"on"` → `"ON"` → `true`)
- New fields added, old fields deprecated

**Impact**: Parser code must change for every source format change. Data collected before the fix is inconsistent with data after.

### Problem 3: No Replay Capability

With transformed data in Bronze, we cannot:
- Reprocess data with improved parsing logic
- Fix parsing bugs retroactively
- Extract fields we previously ignored

**Impact**: Bronze layer fails the "immutable source of truth" principle expected of a data lake.

### Problem 4: Numeric-Only Limitation

Current `TimeSeriesPoint.value` is `f64`. Real-world IoT data includes:
- Status strings: `"online"`, `"error"`, `"calibrating"`
- Boolean states: door open/closed, motion detected
- Firmware versions, model names, error messages

**Impact**: Non-numeric data is either dropped or requires complex workarounds.

## Decision

**Adopt a raw JSON storage model for Bronze layer with typed metadata columns.**

### New Bronze Schema

```
timestamp    | source_id  | ndp_id              | context              | raw_payload
DateTime     | String     | String (nullable)   | JSON (nullable)      | JSON
```

| Column | Type | Description |
|--------|------|-------------|
| `timestamp` | `DateTime<Utc>` | Ingestion timestamp (when NDP received the message) |
| `source_id` | `String` | Source identifier from config (e.g., "air-quality-Mqtt") |
| `ndp_id` | `String?` | Stable platform-owned identifier (from ADR-001 air-009) |
| `context` | `JSON?` | Config-derived metadata snapshot at ingestion time |
| `raw_payload` | `JSON` | Exact payload from source, untransformed |

### Example Data

```
timestamp           | source_id        | ndp_id            | context                      | raw_payload
2026-01-01 12:00:00 | air-quality-Mqtt | airgradient-001   | {"room":"office","floor":2}  | {"pm02":12.5,"rco2":450,"serialno":"abc123"}
2026-01-01 12:00:01 | window-sensor    | window-office-001 | {"room":"office"}            | {"state":"open","battery":85}
2026-01-01 12:00:02 | owm-weather      | owm-home          | {"provider":"openweathermap"}| {"main":{"temp":295.15},"wind":{"speed":3.5}}
```

### Key Principles

1. **raw_payload is sacred**: Exactly what the source sent, byte-for-byte (as JSON)
2. **context is a snapshot**: Config-derived metadata frozen at ingestion time
3. **No parsing in Bronze**: Field extraction happens in Silver layer
4. **Wide format**: One row per message, not one row per metric

### Layer Responsibilities

| Layer | Format | Purpose |
|-------|--------|---------|
| **Bronze (Parquet)** | Wide, raw JSON | Archive, replay, audit, debugging |
| **Silver (TimescaleDB)** | Tall, typed columns | Analytics, dashboards, queries |
| **Gold (Features)** | Aggregated | ML features, predictions |

### Silver ETL Transformation

Silver layer explodes raw JSON into tall format with type handling:

```sql
-- Example: Extract metrics from air quality sensor
INSERT INTO silver.readings (timestamp, ndp_id, location_id, metric, value)
SELECT
    timestamp,
    ndp_id,
    raw_payload->>'$.serialno' as location_id,
    metric_name,
    CAST(raw_payload->>json_path AS FLOAT) as value
FROM bronze.readings,
LATERAL (VALUES
    ('pm02', '$.pm02'),
    ('rco2', '$.rco2'),
    ('atmp', '$.atmp')
) AS metrics(metric_name, json_path)
WHERE source_id = 'air-quality-Mqtt'
  AND raw_payload->>json_path IS NOT NULL;

-- Example: Handle text values (window sensor)
INSERT INTO silver.events (timestamp, ndp_id, event_type, event_value)
SELECT
    timestamp,
    ndp_id,
    'window_state',
    raw_payload->>'$.state'  -- Keep as text in Silver
FROM bronze.readings
WHERE source_id = 'window-sensor';
```

### Querying Bronze Directly

DuckDB (Grafana plugin) can query raw JSON:

```sql
-- Extract specific fields
SELECT
    timestamp,
    raw_payload->>'$.pm02' as pm25,
    raw_payload->>'$.rco2' as co2,
    context->>'$.room' as room
FROM read_parquet('/data/bronze/**/*.parquet')
WHERE timestamp > NOW() - INTERVAL '1 hour';

-- Debug: see raw payloads
SELECT timestamp, source_id, raw_payload
FROM read_parquet('/data/bronze/**/*.parquet')
WHERE source_id = 'window-sensor'
LIMIT 10;
```

## Consequences

### Positive

1. **Zero Data Loss**: Original payload preserved exactly as received
2. **Source Resilience**: Format changes don't break ingestion, only ETL
3. **Replay Capability**: Can reprocess all historical data with improved logic
4. **Type Flexibility**: Text, boolean, nested objects all preserved
5. **Simpler Ingestion**: Parsers become trivial (just extract timestamp + metadata)
6. **Audit Trail**: Can always verify what source actually sent
7. **Schema Evolution**: Add new Silver extractions without touching Bronze

### Negative

1. **Larger Bronze Storage**: JSON less compact than extracted columns (mitigated by Parquet compression)
2. **Slower Bronze Queries**: JSON parsing at query time (acceptable for debug/audit use case)
3. **ETL Complexity**: Silver transformation logic more complex
4. **Two-Phase Availability**: Data visible in Silver after ETL, not immediately

### Storage Impact

| Schema | Bytes per Reading | 1M Readings |
|--------|-------------------|-------------|
| Current (tall, 5 metrics) | ~150 bytes × 5 = 750 | ~750 MB |
| Proposed (wide, raw JSON) | ~400 bytes × 1 = 400 | ~400 MB |

Raw JSON is actually **more compact** because we store one row per message instead of one row per metric. Parquet's columnar compression further reduces JSON column size.

## Alternatives Considered

### Alternative 1: Typed Columns (Current + Extension)

```
timestamp | value_int | value_float | value_text | value_bool | value_type
```

**Rejected because**:
- Still requires parsing at ingestion
- Loses nested structure
- Complex schema with many nullable columns
- Doesn't solve the replay problem

### Alternative 2: All Text Values

```
timestamp | source_id | metric | value_text
12:00:00  | sensor-01 | pm02   | "12.5"
12:00:00  | sensor-01 | state  | "open"
```

**Rejected because**:
- Still requires field extraction at ingestion
- Loses JSON structure for nested sources
- Can't replay with different field mappings

### Alternative 3: Envelope Wrapper

```
timestamp | envelope_json
12:00:00  | {"ndp_id":"...","context":{...},"payload":{...}}
```

**Rejected because**:
- Mixes platform metadata with source data
- Harder to query metadata without parsing payload
- Less clear separation of concerns

## Implementation Impact

### New Rust Types

```rust
/// Bronze layer record - raw JSON storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawDataPoint {
    pub timestamp: DateTime<Utc>,
    pub source_id: String,
    pub ndp_id: Option<String>,
    pub context: Option<Value>,
    pub raw_payload: Value,
}
```

### Files Modified

| File | Change |
|------|--------|
| `core/src/traits.rs` | Add `RawDataPoint` struct |
| `core/src/storage/parquet.rs` | New schema: 5 columns instead of 7 |
| `core/src/sources/*.rs` | Return `RawDataPoint` instead of `Vec<TimeSeriesPoint>` |
| `core/src/parsers/*.rs` | Simplify to extract metadata only |
| `apps/air-quality-app/src/pipeline/*.rs` | Update to handle `RawDataPoint` |

### Migration Strategy

1. **Phase 1**: Add `RawDataPoint` type alongside existing `TimeSeriesPoint`
2. **Phase 2**: Update storage to write both formats (dual-write)
3. **Phase 3**: Update sources to emit `RawDataPoint`
4. **Phase 4**: Deprecate `TimeSeriesPoint` in Bronze path
5. **Phase 5**: Build Silver ETL pipeline

### Backward Compatibility

- Existing Parquet files remain readable (old schema)
- New files use new schema
- Query layer detects schema version and adapts

## Related Decisions

- [ADR-001 (air-009): ndp_id Design](../../air-009/architecture/ADR-001-ndp-id-design.md) - Stable identifier
- [ADR-002 (air-009): Context Flattening](../../air-009/architecture/ADR-002-context-flattening.md) - Context as metadata
- [ADR-001 (dp-002): TimescaleDB Schema](../../dp-002/architecture/ADR-001-TIMESCALEDB-SCHEMA.md) - Silver layer design

## References

- [Databricks Lakehouse Architecture](https://docs.databricks.com/lakehouse-architecture/index.html) - Bronze/Silver/Gold pattern
- [DuckDB JSON Functions](https://duckdb.org/docs/extensions/json.html) - Querying JSON in Parquet
- [Schema-on-Read vs Schema-on-Write](https://www.dataversity.net/schema-on-read-vs-schema-on-write/) - Architectural tradeoffs
