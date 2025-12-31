# ADR-003: Silver Layer Schema Choice

## Status

Proposed

## Date

2025-12-31

## Context

The Silver Layer in NDP uses TimescaleDB for queryable, validated time-series data. With the introduction of `ndp_id` and `context`, we need to decide how to represent these in the TimescaleDB schema.

### Requirements

1. **Query Performance**: Efficient filtering on `ndp_id` and common context fields
2. **Schema Flexibility**: Support dynamic context fields without migrations
3. **Point-in-Time Accuracy**: Historical records retain context at write time
4. **Grafana Compatibility**: Easy to query from Grafana dashboards
5. **Storage Efficiency**: Minimize storage overhead on resource-constrained Pi

### Current Silver Layer

The current virtual Silver Layer uses DuckDB views over Bronze Parquet:

```sql
-- Current: Virtual views, no ETL
CREATE VIEW silver_indoor_air AS
SELECT
    timestamp,
    location_id,
    pm25,
    temperature,
    humidity
FROM read_parquet('/data/air-quality/*.parquet')
WHERE pm25 BETWEEN 0 AND 500;
```

### Options for Context Storage

1. **Flattened Columns**: Each context field becomes a column
2. **JSONB Column**: Store entire flattened context as JSONB
3. **Hybrid**: Common fields as columns + JSONB for dynamic fields

## Decision

**Use JSONB for flexible context storage, with ndp_id as a dedicated indexed column.**

### Schema Design

```sql
-- Create readings hypertable with ndp_id and JSONB context
CREATE TABLE readings (
    -- Time dimension (required for hypertable)
    time TIMESTAMPTZ NOT NULL,

    -- Stable identity (indexed, always present)
    ndp_id TEXT NOT NULL,

    -- Stream identification
    stream_id TEXT NOT NULL,

    -- Original device identifier (from payload)
    location_id TEXT,

    -- Flexible context (flattened, queryable via JSONB operators)
    context JSONB DEFAULT '{}',

    -- Metric values (stream-specific)
    -- Air quality metrics
    pm25 DOUBLE PRECISION,
    pm10 DOUBLE PRECISION,
    co2 INTEGER,
    temperature DOUBLE PRECISION,
    humidity DOUBLE PRECISION,
    tvoc INTEGER,
    nox INTEGER,

    -- Weather metrics
    pressure DOUBLE PRECISION,
    wind_speed DOUBLE PRECISION,
    wind_direction DOUBLE PRECISION,
    clouds INTEGER,
    visibility INTEGER,

    -- Air pollution metrics
    aqi INTEGER,
    co DOUBLE PRECISION,
    no DOUBLE PRECISION,
    no2 DOUBLE PRECISION,
    o3 DOUBLE PRECISION,
    so2 DOUBLE PRECISION,
    nh3 DOUBLE PRECISION
);

-- Convert to hypertable for time-series optimization
SELECT create_hypertable('readings', 'time');

-- Index for source identity queries (most common)
CREATE INDEX idx_readings_ndp_id ON readings (ndp_id, time DESC);

-- Index for stream queries
CREATE INDEX idx_readings_stream ON readings (stream_id, time DESC);

-- GIN index for JSONB context queries
CREATE INDEX idx_readings_context ON readings USING GIN (context jsonb_path_ops);

-- Compound index for common query patterns
CREATE INDEX idx_readings_ndp_stream_time
    ON readings (ndp_id, stream_id, time DESC);
```

### Context Storage Format

The flattened context is stored as JSONB:

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

### Query Patterns

```sql
-- Query by ndp_id (uses dedicated index)
SELECT time, pm25, temperature
FROM readings
WHERE ndp_id = 'airgradient-office-001'
  AND time > NOW() - INTERVAL '24 hours'
ORDER BY time DESC;

-- Query by context field (uses GIN index)
SELECT ndp_id, AVG(pm25) as avg_pm25
FROM readings
WHERE context->>'location.type' = 'indoor'
  AND time > NOW() - INTERVAL '7 days'
GROUP BY ndp_id;

-- Query by coordinates (JSONB array)
SELECT *
FROM readings
WHERE context->'location.coordinates' = '[29.958, -81.308]'::jsonb
  AND time > NOW() - INTERVAL '1 hour';

-- Query by multiple context fields
SELECT time, pm25, context->>'location.path' as room
FROM readings
WHERE context @> '{"device_type": "airgradient"}'::jsonb
  AND context->>'location.type' = 'indoor'
ORDER BY time DESC
LIMIT 100;

-- Compare sources with different contexts
SELECT
    ndp_id,
    context->>'location.path' as location,
    AVG(pm25) as avg_pm25,
    AVG(temperature) as avg_temp
FROM readings
WHERE stream_id = 'air-quality'
  AND time > NOW() - INTERVAL '24 hours'
GROUP BY ndp_id, context->>'location.path';
```

### Grafana Dashboard Queries

```sql
-- Time series by source
SELECT
    time_bucket('5 minutes', time) as time,
    ndp_id,
    AVG(pm25) as pm25
FROM readings
WHERE $__timeFilter(time)
  AND stream_id = 'air-quality'
GROUP BY time_bucket('5 minutes', time), ndp_id
ORDER BY time;

-- Latest values per source with context
SELECT DISTINCT ON (ndp_id)
    ndp_id,
    context->>'location.path' as location,
    pm25,
    temperature,
    time
FROM readings
WHERE stream_id = 'air-quality'
ORDER BY ndp_id, time DESC;
```

## Consequences

### Positive

1. **Schema Flexibility**: Add new context fields without ALTER TABLE
   - No migrations needed for new context keys
   - Users can define any context structure

2. **Query Performance**: GIN index enables efficient JSONB queries
   - `@>` containment operator is well-optimized
   - Common queries use ndp_id index (not JSONB)

3. **Point-in-Time Accuracy**: Full context snapshot per record
   - Historical records retain original context
   - No JOINs needed for context data

4. **Grafana Compatibility**: JSONB operators work in SQL queries
   - Standard PostgreSQL syntax
   - Variable interpolation works

5. **Storage Efficiency**: JSONB compression is effective
   - Repeated keys are deduplicated
   - NULL metric columns don't use space

### Negative

1. **Query Syntax Complexity**: JSONB operators less intuitive than columns
   - `context->>'field'` vs `field`
   - Must quote field names with dots

2. **Type Safety**: JSONB values are not type-checked at insert
   - Strings, numbers, arrays all accepted
   - Validation must happen at ingestion layer

3. **Index Limitations**: GIN indexes support containment, not range queries
   - Cannot efficiently query `location.coordinates[0] > 29.0`
   - Workaround: Extract to computed column if needed

4. **Debugging Complexity**: Context structure harder to inspect
   - Need to expand JSONB to see fields
   - Parquet browser tools may show as raw JSON

### Trade-offs

| Aspect | JSONB (Chosen) | Flattened Columns |
|--------|----------------|-------------------|
| Schema changes | None needed | ALTER TABLE for each field |
| Query syntax | `context->>'field'` | `field` |
| Type safety | At insert time | At schema level |
| Index efficiency | GIN (containment) | B-tree (range) |
| Storage | Compressed | One column per field |
| Grafana ease | Moderate | High |
| Flexibility | High | Low |

## Alternatives Considered

### Alternative 1: Flattened Columns for All Context Fields

```sql
CREATE TABLE readings (
    time TIMESTAMPTZ NOT NULL,
    ndp_id TEXT NOT NULL,
    location_type TEXT,
    location_path TEXT,
    location_lat DOUBLE PRECISION,
    location_lon DOUBLE PRECISION,
    device_type TEXT,
    model TEXT,
    -- etc.
);
```

**Rejected because**:
- Requires schema migration for each new context field
- Column count explosion (users can add arbitrary context)
- Breaks dynamic context requirement

### Alternative 2: Hybrid (Common Columns + JSONB Overflow)

```sql
CREATE TABLE readings (
    time TIMESTAMPTZ NOT NULL,
    ndp_id TEXT NOT NULL,
    location_type TEXT,          -- Promoted
    location_path TEXT,          -- Promoted
    context_extra JSONB,         -- Everything else
);
```

**Rejected because**:
- Unclear which fields get promoted
- Inconsistent query patterns
- Complicates ETL logic

### Alternative 3: Separate Context Table with JOINs

```sql
CREATE TABLE readings (
    time TIMESTAMPTZ NOT NULL,
    ndp_id TEXT NOT NULL,
    -- metrics only
);

CREATE TABLE source_context (
    ndp_id TEXT PRIMARY KEY,
    context JSONB
);
```

**Rejected because**:
- Violates point-in-time accuracy (context changes affect all records)
- Requires JOIN for every query
- Cannot answer "what was the context when this was recorded?"

### Alternative 4: Keep Virtual Silver Layer (DuckDB Views)

```sql
-- Continue using DuckDB views over Parquet
-- Add context columns to Parquet, query via DuckDB
```

**Considered viable** but:
- Less flexible for complex analytics
- DuckDB container dependency
- TimescaleDB needed for continuous aggregates (future)

**Decision**: Support both - Virtual DuckDB views for simple analytics, TimescaleDB for advanced features and continuous aggregates.

## Implementation Details

### ETL from Bronze to Silver

```rust
async fn etl_bronze_to_silver(
    bronze_record: BronzeRecord,
    db: &Database,
) -> Result<(), Error> {
    let insert_sql = r#"
        INSERT INTO readings (
            time, ndp_id, stream_id, location_id, context,
            pm25, pm10, co2, temperature, humidity, tvoc, nox,
            pressure, wind_speed, wind_direction, clouds, visibility,
            aqi, co, no, no2, o3, so2, nh3
        ) VALUES (
            $1, $2, $3, $4, $5,
            $6, $7, $8, $9, $10, $11, $12,
            $13, $14, $15, $16, $17,
            $18, $19, $20, $21, $22, $23, $24
        )
    "#;

    // Convert flattened context to JSONB
    let context_json = serde_json::to_value(&bronze_record.context)?;

    sqlx::query(insert_sql)
        .bind(bronze_record.timestamp)
        .bind(&bronze_record.ndp_id)
        .bind(&bronze_record.stream_id)
        .bind(&bronze_record.location_id)
        .bind(&context_json)
        // ... bind metrics
        .execute(db)
        .await?;

    Ok(())
}
```

### Index Strategy

| Query Pattern | Index Used | Notes |
|--------------|-----------|-------|
| `WHERE ndp_id = 'x'` | `idx_readings_ndp_id` | Primary access pattern |
| `WHERE stream_id = 'x'` | `idx_readings_stream` | Stream filtering |
| `WHERE context @> '{...}'` | `idx_readings_context` | GIN containment |
| `WHERE context->>'field' = 'x'` | `idx_readings_context` | GIN path |
| `WHERE ndp_id = 'x' AND stream_id = 'y'` | `idx_readings_ndp_stream_time` | Compound |

### Continuous Aggregates (Future)

```sql
-- Future: Materialized hourly averages
CREATE MATERIALIZED VIEW hourly_readings
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', time) AS bucket,
    ndp_id,
    stream_id,
    context->>'location.type' as location_type,
    AVG(pm25) as avg_pm25,
    AVG(temperature) as avg_temp,
    COUNT(*) as sample_count
FROM readings
GROUP BY bucket, ndp_id, stream_id, context->>'location.type';

-- Refresh policy
SELECT add_continuous_aggregate_policy('hourly_readings',
    start_offset => INTERVAL '1 day',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour'
);
```

## Migration Path

### Phase 1: Schema Creation

```sql
-- Create new schema with ndp_id and context
-- Existing virtual DuckDB views continue to work
CREATE TABLE readings (...);
```

### Phase 2: ETL Implementation

```rust
// Implement Bronze-to-Silver ETL
// Include ndp_id and context extraction
```

### Phase 3: Dual-Write Period

```
Bronze (Parquet) --> Virtual DuckDB Views (existing queries)
        |
        +--> TimescaleDB Silver (new queries)
```

### Phase 4: Migration Complete

```
Bronze (Parquet) --> TimescaleDB Silver (all queries)
        |
        +--> Archive (cold storage)
```

## Related Decisions

- [ADR-001: ndp_id Design](./ADR-001-ndp-id-design.md) - Why ndp_id is a dedicated column
- [ADR-002: Context Flattening Approach](./ADR-002-context-flattening.md) - How context is flattened

## References

- [TimescaleDB JSONB Best Practices](https://docs.timescale.com/use-timescale/latest/schema-management/json/)
- [PostgreSQL JSONB Indexing](https://www.postgresql.org/docs/current/datatype-json.html#JSON-INDEXING)
- [GIN Index Performance](https://www.postgresql.org/docs/current/gin-intro.html)
- [SCOPE.md](../SCOPE.md) - "Leaning toward JSONB" recommendation
