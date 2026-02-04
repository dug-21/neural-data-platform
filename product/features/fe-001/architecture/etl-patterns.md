# ETL and SQL Generation Patterns Analysis

**Feature**: fe-001 (Gold Layer Foundation)
**Author**: NDP Architect
**Date**: 2026-02-03
**Purpose**: Document existing ETL patterns for Gold layer reuse

---

## ⚠️ CRITICAL CORRECTION (2026-02-03)

**This analysis contains OUTDATED information about DuckDB.**

### Correct Architecture:

| Component | Correct Information |
|-----------|---------------------|
| **Database** | TimescaleDB is the ONLY database. DuckDB is DEPRECATED and removed. |
| **Bronze Layer** | Parquet files on disk (unchanged) |
| **Silver ETL** | EVENT-DRIVEN subscriber to event bus, NOT batch ETL binary |
| **Silver Layer** | TimescaleDB tables with normalized timestamps, light DQ |
| **Gold Layer** | Will read directly from Silver in TimescaleDB |

### The Real Pattern:

```
Ingestion ──────┬──> Bronze (Parquet on disk)
                │
                └──> Event Bus ──> silver_etl subscriber ──> Silver (TimescaleDB)
                                                                    │
                                         Gold Layer reads from ─────┘
```

**Silver ETL subscribes to the event bus. Loading to Silver happens simultaneously with Bronze writes.**

---

## Executive Summary

This document analyzes the current Bronze-to-Silver ETL architecture and SQL generation patterns in the Neural Data Platform. The goal is to inform the design of Gold layer ETL by understanding what patterns can be reused, extended, or need reimagining.

**Key Findings** (CORRECTED):
1. SQL generation uses **string building pattern** (not query builders)
2. ETL is **config-driven** via `SilverEtlConfig` YAML/JSON structures
3. ~~DuckDB serves as ETL engine~~ **DEPRECATED** - TimescaleDB is the only database
4. **Continuous aggregates are planned for Gold layer** - native TimescaleDB feature
5. **Idempotency is built-in** via IF NOT EXISTS, ON CONFLICT patterns
6. **Silver ETL is event-driven** - subscribes to event bus, not batch job

---

## 1. Current ETL Architecture (Bronze to Silver)

### 1.1 Data Flow

```
Bronze (Parquet)          ETL Engine              Silver (TimescaleDB)
┌─────────────────┐      ┌─────────────┐         ┌──────────────────┐
│ Partitioned     │      │  DuckDB     │         │  Hypertables     │
│ Parquet files   │─────>│  In-Memory  │────────>│  (1-day chunks)  │
│ (append-only)   │      │  + Postgres │         │                  │
│                 │      │  Extension  │         │  + DQ flags      │
└─────────────────┘      └─────────────┘         └──────────────────┘
        │                       │                        │
        │                       │                        │
   config/base/            sql_gen.rs              Continuous
   streams/*.yaml         schema_gen.rs           Aggregates
                                                  (planned for Gold)
```

### 1.2 Key Components

| Component | File | Purpose |
|-----------|------|---------|
| `EtlRunner` | `apps/silver-etl/src/etl.rs` | Orchestrates ETL execution |
| `SqlGenerator` | `apps/silver-etl/src/sql_gen.rs` | Generates DuckDB SQL from config |
| `SchemaGenerator` | `apps/silver-etl/src/schema_gen.rs` | Generates DDL for TimescaleDB |
| `SilverEtlConfig` | `core/src/config/silver_etl.rs` | Configuration types and parsing |
| `ddl-generator.sh` | `deploy/pi/ddl-generator.sh` | Bash DDL generator (alternative) |

### 1.3 ETL Execution Flow

```rust
// From etl.rs - EtlRunner::run()
1. Connect DuckDB in-memory database
2. Load postgres_scanner extension
3. Attach TimescaleDB as 'silver' (POSTGRES_ATTACH)
4. For each stream config:
   a. Query watermark from Silver (last processed timestamp)
   b. Read Bronze Parquet files (filtered by watermark)
   c. Apply pre-transforms (array explosion for NWS data)
   d. Generate ETL SQL via SqlGenerator
   e. Execute INSERT ... ON CONFLICT (upsert)
   f. Update watermark
```

---

## 2. SQL Generation Patterns

### 2.1 String Building Pattern

The codebase uses **manual string building** rather than a query builder library. This provides maximum flexibility for complex SQL generation but requires careful escaping.

**Example from `sql_gen.rs`**:

```rust
// SqlGenerator::generate_select_expr()
fn generate_select_expr(&self, mapping: &SilverFieldMapping) -> String {
    let source_expr = self.generate_source_path_expr(&mapping.source_path);

    // Apply transform if present
    let transformed = if let Some(transform) = &mapping.transform {
        self.apply_transform(&source_expr, transform)
    } else {
        source_expr
    };

    // Add type cast
    format!("CAST({} AS {}) AS {}",
        transformed,
        self.type_to_sql(&mapping.field_type),
        mapping.target_column)
}
```

### 2.2 Generated SQL Examples

**SELECT with transforms**:
```sql
SELECT
    CAST(to_timestamp(timestamp / 1000000.0) AS TIMESTAMPTZ) AS observation_time,
    CAST(raw_payload->>'pm02Compensated' AS DOUBLE PRECISION) AS pm25,
    CAST(raw_payload->>'rco2' AS SMALLINT) AS co2,
    -- DQ flags computed inline
    CASE
        WHEN pm25 < 0.0 OR pm25 > 1000.0 THEN 'pm25_out_of_range'
        ELSE NULL
    END AS dq_flags
FROM read_parquet('bronze/air-quality/2026-02-03/*.parquet')
WHERE timestamp > :watermark
```

**INSERT with upsert**:
```sql
INSERT INTO silver.air_quality_observations (
    observation_time, ndp_id, pm25, pm10, co2, temperature_c, humidity_pct, dq_flags
)
SELECT ... FROM bronze_data
ON CONFLICT (observation_time, ndp_id) DO UPDATE SET
    pm25 = EXCLUDED.pm25,
    pm10 = EXCLUDED.pm10,
    ...
```

### 2.3 Transform Types Supported

| Transform | SQL Pattern | Use Case |
|-----------|-------------|----------|
| `JsonExtract` | `raw_payload->>'field'` | Extract from JSON payload |
| `UnitConversion` | `value * factor + offset` | Temperature C->F, etc. |
| `Expression` | Custom SQL expression | Complex calculations |
| `Lookup` | `CASE WHEN ... THEN ...` | Enum mapping |
| `Timestamp` | `to_timestamp(val / 1000000)` | Microseconds to timestamp |
| `Computed` | `COALESCE(a, b)` | Derived fields |

### 2.4 PIVOT Pattern for Array Data

For data sources like NWS forecasts that return arrays, a pre-transform explodes arrays into rows:

```rust
// From sql_gen.rs - generate_pivot_sql()
pub fn generate_pivot_sql(&self, config: &PreTransformConfig) -> String {
    match &config.transform_type {
        PreTransformType::ArrayExplosion {
            source_array,
            index_field,
            value_field
        } => {
            format!(
                "SELECT *,
                    unnest({}).index AS {},
                    unnest({}).value AS {}
                FROM source_table",
                source_array, index_field,
                source_array, value_field
            )
        }
    }
}
```

---

## 3. Config-Driven SQL Generation

### 3.1 Configuration Structure

The `silver_etl` section in stream configs drives all SQL generation:

```yaml
# From config/base/streams/air-quality/config.yaml
silver_etl:
  enabled: true
  target_table: silver.air_quality_observations

  timestamp:
    source_field: timestamp
    target_field: observation_time
    transform: microseconds_to_timestamp

  identity_fields:
    - source: ndp_id
      target: ndp_id

  field_mappings:
    - source_path: raw_payload.pm02Compensated
      target_column: pm25
      type: double_precision
      nullable: false
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 1000.0
          action: flag

  deduplication:
    enabled: true
    key_columns: [observation_time, ndp_id]
    strategy: upsert

  incremental:
    enabled: true
    watermark_column: observation_time
    lag_interval: 5 minutes
```

### 3.2 Configuration Types (Rust)

```rust
// From core/src/config/silver_etl.rs

pub struct SilverEtlConfig {
    pub enabled: bool,
    pub target_table: String,
    pub description: Option<String>,
    pub grain: Option<String>,
    pub timestamp: TimestampMapping,
    pub identity_fields: Vec<IdentityField>,
    pub field_mappings: Vec<SilverFieldMapping>,
    pub dq_rules: Vec<DqRule>,
    pub dq_output: Option<DqOutputConfig>,
    pub deduplication: Option<DeduplicationConfig>,
    pub incremental: Option<IncrementalConfig>,
    pub pre_transform: Option<PreTransformConfig>,
}

pub struct SilverFieldMapping {
    pub source_path: String,
    pub target_column: String,
    pub field_type: FieldType,
    pub unit: Option<String>,
    pub description: Option<String>,
    pub nullable: bool,
    pub transform: Option<TransformConfig>,
    pub dq_rules: Vec<DqRule>,
}
```

### 3.3 Data Quality Rules (11 Types)

| Rule Type | Purpose | SQL Pattern |
|-----------|---------|-------------|
| `range_check` | Value bounds | `BETWEEN min AND max` |
| `null_check` | Required fields | `IS NOT NULL` |
| `enum_check` | Valid values | `IN ('a', 'b', 'c')` |
| `pattern_check` | Regex validation | `~ 'pattern'` |
| `freshness_check` | Timestamp recency | `> NOW() - INTERVAL` |
| `monotonic_check` | Increasing values | LAG window function |
| `rate_of_change` | Delta limits | Window with LAG |
| `cross_field_check` | Field relationships | Custom expression |
| `conditional_check` | Dependent validation | `CASE WHEN THEN` |
| `completeness_check` | Batch-level null % | Aggregate query |
| `cardinality_check` | Unique counts | `COUNT(DISTINCT)` |

---

## 4. DDL Generation (Schema)

### 4.1 Rust SchemaGenerator

```rust
// From schema_gen.rs

pub fn generate_full_migration(&self, config: &SilverEtlConfig) -> String {
    let mut sql = String::new();

    // 1. Create schema
    sql.push_str(&self.generate_create_schema());

    // 2. Create table
    sql.push_str(&self.generate_create_table(config));

    // 3. Create hypertable
    sql.push_str(&self.generate_create_hypertable(config));

    // 4. Create indexes
    sql.push_str(&self.generate_indexes(config));

    // 5. Compression policy (optional)
    sql.push_str(&self.generate_compression_policy(config));

    // 6. Retention policy (optional)
    sql.push_str(&self.generate_retention_policy(config));

    sql
}
```

**Generated DDL Example**:
```sql
-- Schema
CREATE SCHEMA IF NOT EXISTS silver;

-- Table
CREATE TABLE IF NOT EXISTS silver.air_quality_observations (
    observation_time TIMESTAMPTZ NOT NULL,
    ndp_id TEXT NOT NULL,
    pm25 DOUBLE PRECISION NOT NULL,
    pm10 DOUBLE PRECISION,
    co2 SMALLINT,
    temperature_c DOUBLE PRECISION,
    humidity_pct DOUBLE PRECISION,
    dq_flags TEXT[],
    ingestion_time TIMESTAMPTZ DEFAULT NOW()
);

-- Hypertable (idempotent)
SELECT create_hypertable(
    'silver.air_quality_observations',
    'observation_time',
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE
);

-- Unique index for upsert
CREATE UNIQUE INDEX IF NOT EXISTS idx_aq_obs_pk
ON silver.air_quality_observations (observation_time, ndp_id);

-- Compression policy
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM timescaledb_information.jobs
        WHERE proc_name = 'policy_compression'
        AND hypertable_name = 'air_quality_observations'
    ) THEN
        ALTER TABLE silver.air_quality_observations
        SET (timescaledb.compress);

        SELECT add_compression_policy(
            'silver.air_quality_observations',
            INTERVAL '7 days'
        );
    END IF;
END $$;
```

### 4.2 Bash DDL Generator

An alternative DDL generator exists in `deploy/pi/ddl-generator.sh`:

```bash
# Type mapping
map_type() {
    case "$1" in
        float|double) echo "DOUBLE PRECISION" ;;
        int|integer) echo "INTEGER" ;;
        smallint) echo "SMALLINT" ;;
        text|string) echo "TEXT" ;;
        timestamp) echo "TIMESTAMPTZ" ;;
        *) echo "TEXT" ;;
    esac
}

# Generate CREATE TABLE
generate_table() {
    local stream_id="$1"
    local config_file="$2"

    echo "CREATE TABLE IF NOT EXISTS silver.${stream_id}_observations ("

    # Parse field_mappings from YAML
    yq '.silver_etl.field_mappings[]' "$config_file" | while read -r mapping; do
        # ... generate column definitions
    done

    echo ");"
}
```

**Recommendation**: Consolidate on Rust-based generation for consistency and type safety.

---

## 5. Idempotency Patterns

### 5.1 Schema Idempotency

| Pattern | SQL | Purpose |
|---------|-----|---------|
| Schema exists | `CREATE SCHEMA IF NOT EXISTS` | Safe re-run |
| Table exists | `CREATE TABLE IF NOT EXISTS` | Safe re-run |
| Index exists | `CREATE INDEX IF NOT EXISTS` | Safe re-run |
| Hypertable exists | `if_not_exists => TRUE` | TimescaleDB specific |
| Policy exists | `DO $$ IF NOT EXISTS ... $$` | Policies can't use IF NOT EXISTS |

### 5.2 Data Idempotency

```sql
-- Upsert pattern (ON CONFLICT)
INSERT INTO silver.table (...)
SELECT ... FROM bronze
ON CONFLICT (observation_time, ndp_id) DO UPDATE SET
    field1 = EXCLUDED.field1,
    field2 = EXCLUDED.field2;

-- Watermark-based incremental (avoid reprocessing)
SELECT * FROM bronze_parquet
WHERE timestamp > (
    SELECT COALESCE(MAX(observation_time), '1970-01-01'::timestamptz)
    FROM silver.table
)
```

### 5.3 Deduplication Strategies

```rust
// From core/src/config/silver_etl.rs
pub enum DeduplicationStrategy {
    Upsert,    // ON CONFLICT DO UPDATE
    Skip,      // ON CONFLICT DO NOTHING
    Replace,   // DELETE + INSERT (for complex updates)
}
```

---

## 6. Error Handling Patterns

### 6.1 ETL Error Types

```rust
// From etl.rs
pub enum EtlError {
    ConfigError(String),
    DuckDbError(duckdb::Error),
    PostgresError(tokio_postgres::Error),
    ParquetReadError(String),
    TransformError { field: String, message: String },
    DqValidationError { rule: String, count: usize },
}
```

### 6.2 Error Propagation

```rust
// Pattern: map_err for context
let conn = duckdb::Connection::open_in_memory()
    .map_err(|e| EtlError::DuckDbError(e))?;

// Pattern: batch-level error handling
match execute_batch(&conn, &sql) {
    Ok(rows) => {
        tracing::info!(stream_id, rows, "ETL batch completed");
    }
    Err(e) => {
        tracing::error!(stream_id, error = %e, "ETL batch failed");
        // Record to DQ transparency table
        record_etl_failure(&e, &batch_metadata)?;
    }
}
```

### 6.3 DQ Failure Handling

```yaml
# Action types for DQ failures
dq_rules:
  - rule: range_check
    action: flag      # Continue, add to dq_flags array
  - rule: null_check
    action: reject    # Exclude row from insert
  - rule: range_check
    action: clamp     # Adjust value to bounds
    clamp_to_bounds: true
```

---

## 7. Recommendations for Gold Layer ETL

### 7.1 Reuse Existing Patterns

| Pattern | Reuse? | Notes |
|---------|--------|-------|
| Config-driven generation | Yes | Extend SilverEtlConfig to GoldEtlConfig |
| String building SQL | Yes | Proven approach, add continuous aggregate syntax |
| Deduplication strategies | Yes | Same patterns apply |
| DQ rules framework | Yes | Add Gold-specific rules (completeness after aggregation) |
| Idempotent DDL | Yes | Same IF NOT EXISTS patterns |

### 7.2 New Patterns for Gold

**Continuous Aggregates**:

```yaml
# Proposed gold_etl config section
gold_etl:
  enabled: true
  target_table: gold.air_quality_hourly
  aggregate_type: continuous_aggregate

  source_table: silver.air_quality_observations
  refresh_policy:
    schedule: "1 hour"
    start_offset: "3 hours"
    end_offset: "1 hour"

  group_by:
    - time_bucket('1 hour', observation_time) AS bucket
    - ndp_id

  aggregations:
    - source: pm25
      function: avg
      target: pm25_avg
    - source: pm25
      function: percentile_cont(0.95)
      target: pm25_p95
    - source: co2
      function: max
      target: co2_max
```

**Generated SQL**:

```sql
-- Create continuous aggregate (idempotent)
CREATE MATERIALIZED VIEW IF NOT EXISTS gold.air_quality_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', observation_time) AS bucket,
    ndp_id,
    AVG(pm25) AS pm25_avg,
    percentile_cont(0.95) WITHIN GROUP (ORDER BY pm25) AS pm25_p95,
    MAX(co2) AS co2_max,
    COUNT(*) AS observation_count
FROM silver.air_quality_observations
GROUP BY 1, 2
WITH NO DATA;

-- Refresh policy (idempotent via DO block)
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM timescaledb_information.jobs
        WHERE hypertable_name = 'air_quality_hourly'
        AND proc_name = 'policy_refresh_continuous_aggregate'
    ) THEN
        SELECT add_continuous_aggregate_policy(
            'gold.air_quality_hourly',
            start_offset => INTERVAL '3 hours',
            end_offset => INTERVAL '1 hour',
            schedule_interval => INTERVAL '1 hour'
        );
    END IF;
END $$;
```

### 7.3 Schema Evolution Strategy

```rust
// Add to schema_gen.rs
pub fn generate_schema_evolution(&self,
    current: &GoldEtlConfig,
    previous: &GoldEtlConfig
) -> Vec<String> {
    let mut migrations = Vec::new();

    // Detect new columns
    for agg in &current.aggregations {
        if !previous.has_aggregation(&agg.target) {
            migrations.push(format!(
                "ALTER MATERIALIZED VIEW gold.{} ADD COLUMN {} {};",
                current.target_table,
                agg.target,
                self.agg_to_type(agg)
            ));
        }
    }

    // Note: Continuous aggregates don't support DROP COLUMN
    // Mark as deprecated via comments
    for agg in &previous.aggregations {
        if !current.has_aggregation(&agg.target) {
            migrations.push(format!(
                "COMMENT ON COLUMN gold.{}.{} IS 'DEPRECATED: scheduled for removal';",
                current.target_table,
                agg.target
            ));
        }
    }

    migrations
}
```

### 7.4 Proposed GoldEtlConfig Type

```rust
pub struct GoldEtlConfig {
    pub enabled: bool,
    pub target_table: String,
    pub aggregate_type: GoldAggregateType,  // ContinuousAggregate | MaterializedView | Table
    pub source: GoldSourceConfig,
    pub grain: GrainConfig,
    pub group_by: Vec<GroupByExpression>,
    pub aggregations: Vec<AggregationConfig>,
    pub refresh_policy: Option<RefreshPolicy>,
    pub retention_policy: Option<RetentionPolicy>,
    pub indexes: Vec<IndexConfig>,
    pub dq_rules: Vec<GoldDqRule>,  // Post-aggregation validation
}

pub enum GoldAggregateType {
    ContinuousAggregate,  // TimescaleDB native (recommended)
    MaterializedView,      // Standard Postgres (for complex queries)
    Table,                 // For ML feature tables with custom refresh
}

pub struct AggregationConfig {
    pub source_column: String,
    pub function: AggregateFunction,
    pub target_column: String,
    pub filter: Option<String>,  // WHERE within aggregate
}

pub enum AggregateFunction {
    Avg,
    Sum,
    Min,
    Max,
    Count,
    CountDistinct,
    Percentile { p: f64 },
    StdDev,
    Variance,
    First,
    Last,
    ArrayAgg,
    Custom { expression: String },
}
```

---

## 8. Implementation Recommendations

### 8.1 Phase 1: Extend Config Types

1. Create `GoldEtlConfig` in `core/src/config/gold_etl.rs`
2. Add `gold_etl` section parsing to stream config loader
3. Reuse existing `FieldType`, `DqRule` types where applicable

### 8.2 Phase 2: Gold SQL Generator

1. Create `apps/gold-etl/src/sql_gen.rs` following Silver patterns
2. Add continuous aggregate DDL generation
3. Add refresh policy DDL generation
4. Add cagg-specific idempotency patterns

### 8.3 Phase 3: Gold Schema Generator

1. Create `apps/gold-etl/src/schema_gen.rs`
2. Support CREATE MATERIALIZED VIEW WITH (timescaledb.continuous)
3. Support schema evolution (ALTER VIEW ADD COLUMN)
4. Support refresh policy management

### 8.4 Phase 4: Gold ETL Runner

1. Create `apps/gold-etl/src/etl.rs`
2. Orchestrate Silver -> Gold aggregation
3. Handle incremental refresh via TimescaleDB policies
4. Add DQ validation for aggregated data

---

## 9. Pattern Storage

This analysis should be stored as a reusable pattern for future reference.

**AgentDB Pattern Storage**:
```
domain: "architecture"
taskType: "architecture:etl-patterns"
tags: ["fe-001", "gold-layer", "sql-generation", "continuous-aggregates"]
```

---

## Appendix A: File References

| File | Lines | Purpose |
|------|-------|---------|
| `apps/silver-etl/src/sql_gen.rs` | 2030 | SQL generation from config |
| `apps/silver-etl/src/schema_gen.rs` | 506 | DDL generation |
| `apps/silver-etl/src/etl.rs` | 1549 | ETL orchestration |
| `core/src/config/silver_etl.rs` | 2071 | Configuration types |
| `deploy/pi/ddl-generator.sh` | 851 | Bash DDL alternative |
| `deploy/timescaledb/migrations/001_silver_schema.sql` | 270 | Manual migrations |
| `docs/architecture/CONFIG_DRIVEN_SILVER_ETL_DESIGN.md` | - | Design document |
| `config/base/streams/air-quality/config.yaml` | 318 | Example stream config |

## Appendix B: Related ADRs

- ADR-004: Config-Driven Silver ETL (existing)
- ADR-TBD: Gold Layer Continuous Aggregates (to be created)
- ADR-TBD: Schema Evolution for Materialized Views (to be created)
