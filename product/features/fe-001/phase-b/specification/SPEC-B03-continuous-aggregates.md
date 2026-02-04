# SPEC-B03: Per-Stream Continuous Aggregates

> **Feature ID:** v11-003
> **Priority:** Critical
> **Status:** Specification
> **Dependencies:** v11-A02 (Gold DDL Tool), v11-002 (Classification Propagation)
> **Blocks:** v11-004 (Refresh Policy), Phase C (Multi-Stream Alignment)

---

## User Story

**As a** data analyst,
**I want** pre-computed hourly and daily aggregates for the air-quality stream,
**So that** I can efficiently query historical trends and enable pattern detection without expensive runtime aggregation.

---

## Goal

Generate and deploy TimescaleDB continuous aggregates for the air-quality stream:
1. `gold.air_quality_hourly` - Hourly bucket aggregates
2. `gold.air_quality_daily` - Daily bucket aggregates (optional, config-driven)
3. All aggregate columns derived from `gold_etl.aggregates` config
4. Generated via `ndp-gold-ddl` tool, deployed via `deploy.sh`

---

## Background: Continuous Aggregates

TimescaleDB continuous aggregates are materialized views that automatically update as data arrives. They provide:

- **Performance**: Pre-computed aggregates for fast queries
- **Automation**: Background refresh on configurable schedule
- **Consistency**: Always in sync with source data (within refresh lag)

### Air-Quality Aggregation Needs

| Metric | Hourly | Daily | Rationale |
|--------|--------|-------|-----------|
| pm25 | mean, std, min, max, p95 | mean, max | Health monitoring, trend analysis |
| pm10 | mean, min, max | mean, max | Coarse particle tracking |
| co2 | mean, std, min, max | mean, max | Ventilation correlation |
| temperature_c | mean, min, max | mean, min, max | Comfort correlation |
| humidity_pct | mean, min, max | mean, min, max | Comfort correlation |
| tvoc_index | mean, max | mean, max | Air quality index |
| nox_index | mean, max | mean, max | Air quality index |

---

## Functional Requirements

### FR-B03-001: Hourly Continuous Aggregate

The system SHALL generate a continuous aggregate with:
- Bucket: `time_bucket('1 hour', observation_time)`
- Partition: `ndp_id` (per-entity aggregates)
- Columns: All metrics specified in `gold_etl.aggregates.fields`
- Sample count: `COUNT(*) AS sample_count`

### FR-B03-002: Daily Continuous Aggregate (Optional)

If `gold_etl.aggregates.granularities` includes `"1 day"`:
- Generate `gold.air_quality_daily` continuous aggregate
- Bucket: `time_bucket('1 day', observation_time)`
- Same structure as hourly

### FR-B03-003: Column Naming Convention

Generated columns SHALL follow the naming convention:
```
{field}_{metric}
```

Examples:
- `pm25_mean` - Average PM2.5
- `pm25_std` - Standard deviation of PM2.5
- `co2_max` - Maximum CO2
- `temperature_c_min` - Minimum temperature

### FR-B03-004: Percentile Computation

For p95 and p99 metrics, use `PERCENTILE_CONT`:
```sql
PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY pm25) AS pm25_p95
```

### FR-B03-005: DDL Generation

The `ndp-gold-ddl` tool SHALL generate idempotent SQL:
```bash
ndp-gold-ddl generate --stream air-quality --action sync
```

For `sync` action:
- Check if aggregate exists
- Create if not exists
- Skip if exists (idempotent)

For `recreate` action:
- DROP IF EXISTS
- CREATE new aggregate
- Re-add policies

### FR-B03-006: deploy.sh Integration

The `handle_gold_table()` function SHALL:
```bash
handle_gold_table() {
    local declaration="$1"
    local stream_id=$(echo "$declaration" | jq -r '.stream_id')
    local action=$(echo "$declaration" | jq -r '.action // "sync"')

    log "Gold Table: $stream_id (action=$action)"

    local ddl=$(ndp-gold-ddl generate --stream "$stream_id" --action "$action" 2>&1)
    if [ $? -ne 0 ]; then
        error "Gold DDL generation failed: $ddl"
        return 1
    fi

    echo "$ddl" | dcx timescaledb psql -U postgres -d ndp
}
```

---

## Non-Functional Requirements

### NFR-B03-001: Query Performance

30-day range queries on hourly aggregate SHALL complete in < 100ms on Pi 5:
```sql
SELECT bucket, pm25_mean, co2_mean
FROM gold.air_quality_hourly
WHERE bucket >= NOW() - INTERVAL '30 days';
```

### NFR-B03-002: Storage Efficiency

Continuous aggregates SHALL use < 5 MB storage per stream for 30 days of data.

### NFR-B03-003: Refresh Resource Usage

Aggregate refresh SHALL use < 100 MB peak memory and < 10% CPU sustained.

### NFR-B03-004: Idempotency

Running `deploy.sh apply` multiple times SHALL not fail or create duplicates.

---

## Acceptance Criteria

### AC-B03-001: Hourly Aggregate Created

```gherkin
Scenario: Hourly continuous aggregate is created
  Given the air-quality config has gold_etl.enabled = true
  And gold_etl.aggregates.granularities includes "1 hour"
  When deploy.sh apply is executed with air-quality Gold table
  Then gold.air_quality_hourly SHALL exist
  And it SHALL be a continuous aggregate
```

### AC-B03-002: Daily Aggregate Created

```gherkin
Scenario: Daily continuous aggregate is created
  Given the air-quality config has gold_etl.enabled = true
  And gold_etl.aggregates.granularities includes "1 day"
  When deploy.sh apply is executed with air-quality Gold table
  Then gold.air_quality_daily SHALL exist
```

### AC-B03-003: Correct Columns Generated

```gherkin
Scenario: All configured metrics are generated as columns
  Given gold_etl.aggregates.fields.pm25.metrics = ["mean", "std", "min", "max", "p95"]
  When gold.air_quality_hourly is created
  Then it SHALL have columns:
    | pm25_mean | double precision |
    | pm25_std  | double precision |
    | pm25_min  | double precision |
    | pm25_max  | double precision |
    | pm25_p95  | double precision |
```

### AC-B03-004: Sample Count Included

```gherkin
Scenario: Sample count is always included
  Given gold_etl.enabled = true
  When gold.air_quality_hourly is created
  Then it SHALL have a sample_count column
  And sample_count SHALL be COUNT(*) of source rows
```

### AC-B03-005: Query Returns Data

```gherkin
Scenario: Continuous aggregate contains aggregated data
  Given silver.air_quality_observations has data for the past 24 hours
  When I query gold.air_quality_hourly
  Then I SHALL receive rows with hourly buckets
  And pm25_mean SHALL be a valid average
```

### AC-B03-006: Idempotent Creation

```gherkin
Scenario: Running deploy twice does not fail
  Given gold.air_quality_hourly already exists
  When deploy.sh apply is executed with action = "sync"
  Then deployment SHALL succeed
  And gold.air_quality_hourly SHALL remain unchanged
```

### AC-B03-007: Recreate Drops and Creates

```gherkin
Scenario: Recreate action drops and recreates aggregate
  Given gold.air_quality_hourly exists with old schema
  When deploy.sh apply is executed with action = "recreate"
  Then old aggregate SHALL be dropped
  And new aggregate SHALL be created with updated schema
```

---

## Generated SQL

### Hourly Continuous Aggregate

Based on the air-quality config, the following SQL SHALL be generated:

```sql
-- Generated by ndp-gold-ddl for stream: air-quality
-- Granularity: 1 hour
-- Generated: 2026-02-04

-- Check if aggregate exists (sync mode)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM timescaledb_information.continuous_aggregates
        WHERE view_schema = 'gold' AND view_name = 'air_quality_hourly'
    ) THEN

        CREATE MATERIALIZED VIEW gold.air_quality_hourly
        WITH (timescaledb.continuous) AS
        SELECT
            time_bucket('1 hour', observation_time) AS bucket,
            ndp_id,

            -- PM2.5 metrics (mean, std, min, max, p95)
            AVG(pm25) AS pm25_mean,
            STDDEV(pm25) AS pm25_std,
            MIN(pm25) AS pm25_min,
            MAX(pm25) AS pm25_max,
            PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY pm25) AS pm25_p95,

            -- PM10 metrics (mean, min, max)
            AVG(pm10) AS pm10_mean,
            MIN(pm10) AS pm10_min,
            MAX(pm10) AS pm10_max,

            -- CO2 metrics (mean, std, min, max)
            AVG(co2) AS co2_mean,
            STDDEV(co2) AS co2_std,
            MIN(co2) AS co2_min,
            MAX(co2) AS co2_max,

            -- Temperature metrics (mean, min, max)
            AVG(temperature_c) AS temperature_c_mean,
            MIN(temperature_c) AS temperature_c_min,
            MAX(temperature_c) AS temperature_c_max,

            -- Humidity metrics (mean, min, max)
            AVG(humidity_pct) AS humidity_pct_mean,
            MIN(humidity_pct) AS humidity_pct_min,
            MAX(humidity_pct) AS humidity_pct_max,

            -- TVOC metrics (mean, max)
            AVG(tvoc_index) AS tvoc_index_mean,
            MAX(tvoc_index) AS tvoc_index_max,

            -- NOx metrics (mean, max)
            AVG(nox_index) AS nox_index_mean,
            MAX(nox_index) AS nox_index_max,

            -- Sample count (always included)
            COUNT(*) AS sample_count

        FROM silver.air_quality_observations
        GROUP BY bucket, ndp_id;

        RAISE NOTICE 'Created continuous aggregate gold.air_quality_hourly';
    ELSE
        RAISE NOTICE 'gold.air_quality_hourly already exists, skipping (sync mode)';
    END IF;
END $$;
```

### Recreate Mode SQL

```sql
-- Generated by ndp-gold-ddl for stream: air-quality
-- Action: recreate (schema change detected)

-- Drop existing aggregate and policies
DROP MATERIALIZED VIEW IF EXISTS gold.air_quality_hourly CASCADE;

-- Create new aggregate
CREATE MATERIALIZED VIEW gold.air_quality_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', observation_time) AS bucket,
    ndp_id,
    -- ... same SELECT as above ...
FROM silver.air_quality_observations
GROUP BY bucket, ndp_id;
```

---

## Air-Quality Config Extension

### gold_etl Section

```yaml
# config/base/streams/air-quality/config.yaml
# Add after silver_etl section

gold_etl:
  enabled: true
  description: "Hourly and daily aggregates for air quality metrics"

  aggregates:
    granularities: ["1 hour", "1 day"]
    default_metrics: ["mean", "count"]

    fields:
      pm25:
        metrics: ["mean", "std", "min", "max", "p95"]
      pm10:
        metrics: ["mean", "min", "max"]
      co2:
        metrics: ["mean", "std", "min", "max"]
      temperature_c:
        metrics: ["mean", "min", "max"]
      humidity_pct:
        metrics: ["mean", "min", "max"]
      tvoc_index:
        metrics: ["mean", "max"]
      nox_index:
        metrics: ["mean", "max"]
```

---

## Integration Test Requirements

### Test: Aggregate Creation

```bash
# Deploy air-quality Gold layer
deploy.sh apply .deploy/test/phase-b-air-quality.manifest.json

# Verify continuous aggregates exist
dcx timescaledb psql -U postgres -d ndp -c "
SELECT view_name, view_definition IS NOT NULL AS has_definition
FROM timescaledb_information.continuous_aggregates
WHERE view_schema = 'gold'
  AND view_name LIKE 'air_quality%'
ORDER BY view_name;
"
# Expected:
# view_name             | has_definition
# ----------------------+----------------
# air_quality_daily     | t
# air_quality_hourly    | t
```

### Test: Column Verification

```bash
# Verify expected columns exist
dcx timescaledb psql -U postgres -d ndp -c "
SELECT column_name, data_type
FROM information_schema.columns
WHERE table_schema = 'gold'
  AND table_name = 'air_quality_hourly'
ORDER BY ordinal_position;
"
# Expected: bucket, ndp_id, pm25_mean, pm25_std, pm25_min, pm25_max, pm25_p95, ...
```

### Test: Data Query

```bash
# Query aggregated data (after initial refresh)
dcx timescaledb psql -U postgres -d ndp -c "
SELECT bucket, ndp_id, pm25_mean, co2_mean, sample_count
FROM gold.air_quality_hourly
ORDER BY bucket DESC
LIMIT 5;
"
```

### Test: Performance

```bash
# 30-day query performance
dcx timescaledb psql -U postgres -d ndp -c "
EXPLAIN (ANALYZE, TIMING)
SELECT bucket, pm25_mean, co2_mean
FROM gold.air_quality_hourly
WHERE bucket >= NOW() - INTERVAL '30 days';
"
# Expected: Execution Time < 100ms
```

### Test: Idempotency

```bash
# Run deploy twice
deploy.sh apply .deploy/test/phase-b-air-quality.manifest.json
deploy.sh apply .deploy/test/phase-b-air-quality.manifest.json

# Should not error, should log "already exists, skipping"
```

---

## London TDD Interfaces

### Trait: ContinuousAggregateGenerator

```rust
/// Generates DDL for continuous aggregates
trait ContinuousAggregateGenerator {
    /// Generate CREATE MATERIALIZED VIEW statement
    fn generate_create(
        &self,
        config: &GoldEtlConfig,
        stream_id: &str,
        silver_table: &str,
        granularity: &str,
    ) -> Result<String, GeneratorError>;

    /// Generate idempotent check wrapper
    fn wrap_idempotent(&self, sql: &str, view_name: &str) -> String;

    /// Generate DROP IF EXISTS statement
    fn generate_drop(&self, view_name: &str) -> String;

    /// Get view name for a stream and granularity
    fn get_view_name(&self, stream_id: &str, granularity: &str) -> String;
}
```

### Trait: AggregateExpressionBuilder

```rust
/// Builds SQL expressions for aggregate metrics
trait AggregateExpressionBuilder {
    /// Build expression for a metric
    fn build_expression(&self, field: &str, metric: &str) -> Result<String, ExpressionError>;

    /// Get the column alias for a field + metric
    fn get_alias(&self, field: &str, metric: &str) -> String;
}

impl AggregateExpressionBuilder for DefaultAggregateExpressionBuilder {
    fn build_expression(&self, field: &str, metric: &str) -> Result<String, ExpressionError> {
        let expr = match metric {
            "mean" => format!("AVG({field})"),
            "std" => format!("STDDEV({field})"),
            "min" => format!("MIN({field})"),
            "max" => format!("MAX({field})"),
            "count" => format!("COUNT({field})"),
            "first" => format!("FIRST({field}, observation_time)"),
            "last" => format!("LAST({field}, observation_time)"),
            "p95" => format!("PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY {field})"),
            "p99" => format!("PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY {field})"),
            _ => return Err(ExpressionError::UnknownMetric(metric.into())),
        };
        Ok(expr)
    }

    fn get_alias(&self, field: &str, metric: &str) -> String {
        format!("{field}_{metric}")
    }
}
```

### Mock: ContinuousAggregateGenerator

```rust
mock! {
    pub ContinuousAggregateGenerator {}

    impl ContinuousAggregateGenerator for ContinuousAggregateGenerator {
        fn generate_create(
            &self,
            config: &GoldEtlConfig,
            stream_id: &str,
            silver_table: &str,
            granularity: &str,
        ) -> Result<String, GeneratorError>;

        fn wrap_idempotent(&self, sql: &str, view_name: &str) -> String;
        fn generate_drop(&self, view_name: &str) -> String;
        fn get_view_name(&self, stream_id: &str, granularity: &str) -> String;
    }
}
```

---

## Error Handling

### Error Codes

| Code | Name | Description | Recovery |
|------|------|-------------|----------|
| 410 | GOLD_GENERATION_FAILED | DDL generation failed | Check config syntax |
| 411 | UNKNOWN_SILVER_TABLE | Silver table not found | Deploy Silver first |
| 412 | INVALID_FIELD_REFERENCE | gold_etl references unknown field | Check field_mappings |
| 413 | AGGREGATE_EXISTS | Aggregate already exists (recreate required) | Use action: recreate |

---

## Data Dictionary Population

After aggregate creation, populate metadata:

```sql
-- Populate gold_tables
INSERT INTO data_dictionary.gold_tables
    (table_name, object_type, source_silver_table, bucket_interval, description)
VALUES
    ('gold.air_quality_hourly', 'continuous_aggregate',
     'silver.air_quality_observations', INTERVAL '1 hour',
     'Hourly aggregates for air quality metrics')
ON CONFLICT (table_name) DO UPDATE SET
    updated_at = NOW();

-- Populate gold_columns (for each column)
INSERT INTO data_dictionary.gold_columns
    (table_name, column_name, data_type, feature_type, source_expression, description)
VALUES
    ('gold.air_quality_hourly', 'pm25_mean', 'double precision', 'aggregate',
     'AVG(pm25)', 'Average PM2.5 in bucket'),
    ('gold.air_quality_hourly', 'pm25_std', 'double precision', 'aggregate',
     'STDDEV(pm25)', 'Standard deviation of PM2.5 in bucket'),
    -- ... more columns ...
ON CONFLICT (table_name, column_name) DO UPDATE SET
    updated_at = NOW();
```

---

## References

- [SCOPE.md](../../SCOPE.md) - v11-003 Per-Stream Continuous Aggregates
- [DECISIONS.md](../../architecture/DECISIONS.md) - ADR-FE001-001: Gold DDL in Rust
- [SPEC-A02](../phase-a/specification/SPEC-A02-gold-ddl-tool.md) - Gold DDL Tool
- [TimescaleDB Continuous Aggregates](https://docs.timescale.com/use-timescale/latest/continuous-aggregates/)

---

*Specification created: 2026-02-04*
