# ADR-001: Pre-Transform Parser Integration Design

**Feature**: dp-007 (Pre-Transform Parser Integration)
**Status**: Proposed
**Date**: 2026-01-12
**Author**: NDP Architect
**Supersedes**: None
**Related**: ADR-006-001 (ETL Engine Selection), AIR-007 (Column-Oriented Parser)

---

## Context

The NWS gridpoints forecast data presents a unique challenge for Silver layer ETL. Unlike flat observation data from sensors (air quality, outdoor weather), the NWS API returns a **columnar array structure** where each metric contains arrays of `{validTime, value}` pairs.

### The Problem

```json
{
  "properties": {
    "temperature": {
      "values": [
        {"validTime": "2025-12-24T00:00:00+00:00/PT1H", "value": 15.5},
        {"validTime": "2025-12-24T01:00:00+00:00/PT1H", "value": 14.8}
      ]
    },
    "windSpeed": {
      "values": [
        {"validTime": "2025-12-24T00:00:00+00:00/PT1H", "value": 36.0}
      ]
    }
  }
}
```

The current silver-etl uses DuckDB SQL with `json_extract_path` for field extraction. This works for flat JSON but **cannot easily UNNEST nested arrays** across multiple columns with different cardinalities.

### Existing Asset

The `ColumnOrientedParser` in `neural-core` already solves this problem for Bronze ingestion:
- Explodes columnar arrays into individual `TimeSeriesPoint` objects
- Handles ISO 8601 duration timestamps (`"2025-12-24T00:00:00+00:00/PT1H"`)
- Supports unit conversions
- Config-driven via `column_config` in stream YAML

However, Bronze stores the **raw JSON payload**, not the parsed points. Silver ETL needs access to the parsed, flattened data.

### Technical Constraints

1. **DuckDB SQL limitations**: Complex UNNEST across different-length arrays is error-prone
2. **Reuse existing code**: ColumnOrientedParser is battle-tested (AIR-007)
3. **Config-driven**: Solution must be YAML-configurable
4. **Performance**: Must not significantly impact ETL latency on Pi 5

---

## Decision

**Integrate `ColumnOrientedParser` into silver-etl as a configurable pre-transform stage.**

Streams can opt-in to pre-transformation by adding a `pre_transform` section to their `silver_etl` config:

```yaml
silver_etl:
  enabled: true
  target_table: silver.nws_forecasts

  # NEW: Pre-transform stage
  pre_transform:
    enabled: true
    parser_type: column_oriented
    # References the parser config from sources section
    parser_config_ref: sources[0].parser
```

The ETL pipeline becomes:

```
Bronze Parquet (raw JSON)
       |
       v
[Pre-Transform Stage]  <-- ColumnOrientedParser called here
       |
       v
Flattened temp table (one row per metric per validTime)
       |
       v
DuckDB SQL (standard field extraction)
       |
       v
Silver TimescaleDB
```

### Data Flow Detail

1. **Read Bronze Parquet**: Load `raw_payload` JSON column
2. **Pre-Transform**: For each row, call `ColumnOrientedParser::parse()`
3. **Flatten to Table**: Convert `Vec<TimeSeriesPoint>` to DuckDB table
4. **Apply Mappings**: Standard DuckDB SQL extracts fields from flattened data
5. **Write to Silver**: INSERT into TimescaleDB with DQ flags

### Flattened Schema

Pre-transform produces a temp table with these columns:

| Column | Type | Description |
|--------|------|-------------|
| `issue_time` | TIMESTAMPTZ | From Bronze `timestamp` (when forecast was made) |
| `valid_time` | TIMESTAMPTZ | From `forecast_valid_time` tag (when forecast applies) |
| `ndp_id` | VARCHAR | From Bronze `ndp_id` |
| `metric_name` | VARCHAR | From `metric` tag (e.g., "temperature") |
| `value` | DOUBLE | The numeric value |
| `location_id` | VARCHAR | From parser output |

This enables simple SQL like:

```sql
SELECT
    issue_time,
    valid_time,
    ndp_id,
    MAX(CASE WHEN metric_name = 'temperature' THEN value END) AS temperature_c,
    MAX(CASE WHEN metric_name = 'wind_speed' THEN value END) AS wind_speed_kmh
FROM pre_transformed
GROUP BY issue_time, valid_time, ndp_id
```

---

## Alternatives Considered

### Alternative 1: DuckDB UNNEST

**Description**: Implement array explosion purely in DuckDB SQL using `UNNEST`, `CROSS JOIN LATERAL`, and JSON functions.

```sql
SELECT
    timestamp AS issue_time,
    unnest(json_extract_path(raw_payload, '$.properties.temperature.values[*].validTime')) AS valid_time,
    unnest(json_extract_path(raw_payload, '$.properties.temperature.values[*].value')) AS temperature_c
FROM read_parquet('...')
```

| Pros | Cons |
|------|------|
| No Rust code changes | Complex SQL for 40+ columns |
| Pure SQL solution | Cross-join issues with different array lengths |
| | Hard to maintain timestamp parsing |
| | No reuse of existing parser logic |
| | Difficult to test |

**Rejected because**: The complexity of handling ISO 8601 duration parsing, unit conversions, and 40+ columns in SQL would be unmaintainable. Different metrics have different array lengths, making CROSS JOIN semantics problematic.

### Alternative 2: Separate Pre-Transform Binary

**Description**: Create a standalone Rust binary that reads Bronze Parquet, applies ColumnOrientedParser, and writes intermediate Parquet files.

```
air-quality-app -> Bronze Parquet
silver-pretransform -> Intermediate Parquet  <-- NEW BINARY
silver-etl -> Silver TimescaleDB
```

| Pros | Cons |
|------|------|
| Clean separation of concerns | Extra deployment complexity |
| Intermediate files for debugging | Duplicate file I/O |
| | Additional cron job |
| | Disk space for intermediate files |

**Rejected because**: Adds operational complexity to Pi deployment without sufficient benefit. The intermediate Parquet files would consume disk space and I/O.

### Alternative 3: Reuse ColumnOrientedParser in silver-etl (CHOSEN)

**Description**: Import `neural-core::parsers::column_oriented` into `silver-etl` and call it as a pre-transform step before DuckDB processing.

| Pros | Cons |
|------|------|
| Reuses battle-tested parser code | silver-etl depends on neural-core parsers |
| Consistent with Bronze ingestion | Slight increase in binary size |
| Config-driven via existing YAML | Pre-transform adds processing step |
| Maintains single binary deployment | |

**Selected because**: Maximum code reuse, consistent behavior with Bronze ingestion, config-driven, and maintains the single-binary deployment model from ADR-006-001.

---

## Consequences

### Positive

1. **Code reuse**: Leverages 1000+ lines of tested parser code (AIR-007)
2. **Consistency**: Same parser logic for Bronze ingestion and Silver ETL
3. **Config-driven**: `pre_transform` section in YAML, no code changes per stream
4. **Maintainability**: Parser improvements benefit both layers
5. **Proper timestamps**: `issue_time` (when made) and `valid_time` (when applies) handled correctly

### Negative

1. **New dependency**: silver-etl adds `neural-core` as a dependency
2. **Increased memory**: Pre-transform creates in-memory temp table (~20-50MB for forecast data)
3. **Two parsing passes**: Raw JSON parsed once by pre-transform, values extracted by DuckDB

### Neutral

1. **Opt-in per stream**: Only streams needing pre-transform enable it
2. **No change to existing streams**: air-quality, outdoor-weather continue as before

---

## Implementation Notes

### New Types in silver-etl

```rust
// apps/silver-etl/src/pre_transform.rs

use neural_core::parsers::{ColumnOrientedParser, ParserConfig};

/// Pre-transform configuration
#[derive(Debug, Clone, Deserialize)]
pub struct PreTransformConfig {
    /// Whether pre-transform is enabled
    pub enabled: bool,
    /// Parser type to use
    pub parser_type: String,  // "column_oriented"
    /// Reference to parser config (e.g., "sources[0].parser")
    pub parser_config_ref: String,
}

/// Apply pre-transform to raw payloads
pub fn apply_pre_transform(
    conn: &Connection,
    raw_payloads: &[String],
    timestamps: &[i64],
    ndp_ids: &[String],
    parser: &ColumnOrientedParser,
) -> Result<(), EtlError> {
    // Create temp table
    conn.execute_batch(r#"
        CREATE TEMP TABLE pre_transformed (
            issue_time TIMESTAMPTZ,
            valid_time TIMESTAMPTZ,
            ndp_id VARCHAR,
            metric_name VARCHAR,
            value DOUBLE,
            location_id VARCHAR
        );
    "#)?;

    // Parse each payload and insert
    for (i, payload) in raw_payloads.iter().enumerate() {
        let json: Value = serde_json::from_str(payload)?;
        let issue_time = DateTime::from_timestamp_micros(timestamps[i]);

        let points = parser.parse(&json, issue_time)?;

        for point in points {
            let valid_time = point.tags.get("forecast_valid_time")
                .and_then(|s| s.parse::<i64>().ok())
                .map(|ts| DateTime::from_timestamp(ts, 0));

            conn.execute(
                "INSERT INTO pre_transformed VALUES (?, ?, ?, ?, ?, ?)",
                params![
                    issue_time,
                    valid_time,
                    ndp_ids[i],
                    point.tags.get("metric"),
                    point.value,
                    point.location_id
                ]
            )?;
        }
    }

    Ok(())
}
```

### Config Schema Update

Add to `SilverEtlConfig` in `core/src/config/silver_etl.rs`:

```rust
/// Pre-transform configuration (optional)
#[serde(default)]
pub pre_transform: Option<PreTransformConfig>,
```

### ETL Pipeline Update

Modify `EtlRunner::run_etl()` in `apps/silver-etl/src/etl.rs`:

```rust
pub fn run_etl(&self, config: &SilverEtlConfig, ...) -> Result<EtlStats, EtlError> {
    // ... existing code ...

    // NEW: Apply pre-transform if enabled
    if let Some(pre_transform) = &config.pre_transform {
        if pre_transform.enabled {
            let parser = self.load_parser(&pre_transform.parser_config_ref)?;
            apply_pre_transform(&self.conn, &raw_payloads, &timestamps, &ndp_ids, &parser)?;

            // Update SQL generation to use pre_transformed table
            sql_gen.set_source_table("pre_transformed");
        }
    }

    // ... rest of existing code ...
}
```

### Memory Impact

| Component | Before | After |
|-----------|--------|-------|
| DuckDB base | 100MB | 100MB |
| Parquet buffers | 50MB | 50MB |
| Pre-transform temp | 0MB | 20-50MB |
| **Total Peak** | 200MB | 220-250MB |

Still within the 300MB budget from ADR-006-001.

---

## Component Diagram

```
                    +---------------------+
                    |   Stream Config     |
                    |   (YAML/etcd)       |
                    +----------+----------+
                               |
                               v
+--------------------------------------------------+
|                   silver-etl                      |
|                                                   |
|  +---------------+     +---------------------+    |
|  | Config Loader |---->| Pre-Transform Stage |    |
|  +---------------+     |                     |    |
|                        | +-----------------+ |    |
|                        | |ColumnOriented   | |    |
|                        | |Parser (imported)| |    |
|                        | +-----------------+ |    |
|                        +----------+----------+    |
|                                   |               |
|                                   v               |
|                        +---------------------+    |
|                        | DuckDB SQL Engine   |    |
|                        |                     |    |
|                        | - Field mappings    |    |
|                        | - DQ rules          |    |
|                        | - Pivoting          |    |
|                        +----------+----------+    |
|                                   |               |
+-----------------------------------|---------------+
                                    |
                                    v
                         +---------------------+
                         |   TimescaleDB       |
                         |   (Silver Layer)    |
                         +---------------------+


Integration Points:
==================

 neural-core                        silver-etl
+------------------+              +------------------+
| parsers/         |              | pre_transform.rs |
|   column_       ------uses----->|                  |
|   oriented.rs   |              | etl.rs           |
|                 |              +------------------+
| config/         |
|   mod.rs        ------uses------> SilverEtlConfig
+------------------+                 (pre_transform field)


Data Flow (nws-gridpoints-forecast):
====================================

Bronze Parquet
+----------------------------------------+
| timestamp | ndp_id | raw_payload       |
|-----------|--------|-------------------|
| 170656...| nws-01 | {"properties":... |
+----------------------------------------+
                |
                | Pre-Transform Stage
                v
Temp Table: pre_transformed
+----------------------------------------------------------+
| issue_time | valid_time | ndp_id | metric_name | value   |
|------------|------------|--------|-------------|---------|
| 2026-01-12 | 2026-01-12 | nws-01 | temperature | 15.5    |
| 2026-01-12 | 2026-01-12 | nws-01 | wind_speed  | 36.0    |
| 2026-01-12 | 2026-01-13 | nws-01 | temperature | 14.8    |
+----------------------------------------------------------+
                |
                | DuckDB SQL (PIVOT)
                v
Silver TimescaleDB: silver.nws_forecasts
+------------------------------------------------------------+
| issue_time | valid_time | ndp_id | temperature_c | wind_... |
|------------|------------|--------|---------------|----------|
| 2026-01-12 | 2026-01-12 | nws-01 | 15.5          | 36.0     |
| 2026-01-12 | 2026-01-13 | nws-01 | 14.8          | NULL     |
+------------------------------------------------------------+
```

---

## Test Strategy

### Unit Tests

1. **Pre-transform creates correct temp table**: Verify schema and row count
2. **Parser config loading**: Test parser_config_ref resolution
3. **Timestamp extraction**: Verify issue_time and valid_time correct
4. **Empty array handling**: Gracefully handle metrics with no values

### Integration Tests

1. **End-to-end with mock NWS data**: Bronze -> Pre-transform -> Silver
2. **Mixed streams**: Run pre-transform stream alongside flat streams
3. **Memory usage**: Verify stays within 300MB budget

---

## Migration Path

1. **Phase 1**: Implement pre-transform module (this ADR)
2. **Phase 2**: Update nws-gridpoints-forecast config to enable pre_transform
3. **Phase 3**: Verify data in Silver with correct issue_time/valid_time
4. **Phase 4**: Enable in production

No breaking changes to existing streams.

---

## References

1. SCOPE: `product/features/dp-007/SCOPE.md`
2. ColumnOrientedParser: `core/src/parsers/column_oriented.rs`
3. ETL Engine ADR: `product/features/dp-006/architecture/ADR-006-001-etl-engine-selection.md`
4. NWS Stream Config: `config/base/streams/nws-gridpoints-forecast/config.yaml`
5. Silver ETL Config: `core/src/config/silver_etl.rs`

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-12 | NDP Architect | Initial proposal |
