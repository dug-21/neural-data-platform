# Data Dictionary Architecture Analysis for Gold Layer Extension

**Feature:** fe-001 (Gold Layer Foundation V1.1)
**Author:** NDP Architect
**Date:** 2026-02-03
**Status:** Analysis Complete

---

## Executive Summary

This document analyzes the existing data dictionary implementation and recommends an approach for extending it to support the Gold layer. Based on the analysis of current patterns and V1.1 requirements, the recommended approach is:

**RECOMMENDATION: Extend existing `data_dictionary` schema with Gold-specific tables, following the established Silver layer pattern.**

This maintains architectural consistency, enables unified cross-layer queries, and preserves the proven config-driven metadata approach.

---

## 1. Current Data Dictionary Architecture

### 1.1 Schema Organization

The data dictionary uses a single PostgreSQL schema `data_dictionary` containing tables for both Bronze and Silver layers:

```
data_dictionary schema
├── BRONZE LAYER (DP-002)
│   ├── streams                        # Stream-level metadata
│   ├── fields                         # Bronze field definitions
│   ├── sources                        # Data source configurations
│   ├── entity_schemas                 # Entity type definitions
│   ├── entity_schema_attributes       # Entity attribute definitions
│   └── sync_status                    # Sync audit trail
│
├── SILVER LAYER (DP-009)
│   ├── silver_tables                  # Silver table metadata
│   ├── silver_columns                 # Silver column definitions
│   ├── silver_lineage                 # Bronze-to-Silver mappings
│   └── silver_dq_rules               # DQ rules per column
│
└── UNIFIED VIEWS
    ├── v_complete_dictionary          # Bronze UNION Silver columns
    ├── v_silver_table_overview        # Silver table summary
    ├── v_lineage                      # Bronze-to-Silver lineage
    ├── v_dq_rules_summary            # DQ rules with context
    └── v_column_search               # Cross-layer column search
```

### 1.2 Key Design Decisions (from ADR-009-001)

| Decision | Rationale |
|----------|-----------|
| **Single schema** | Simplifies cross-layer queries; unified sync mechanism |
| **Layer prefix on tables** | `silver_tables`, `silver_columns` disambiguate from Bronze |
| **Natural primary keys** | `table_name` as PK avoids surrogate key indirection |
| **Normalized tables** | Efficient indexing for column-level queries |
| **JSONB for rule params** | Flexible schema for DQ rule parameters |
| **Unified views** | Enable single-query across Bronze + Silver |

### 1.3 Metadata Tracked by Layer

| Layer | Metadata | Source | Tables |
|-------|----------|--------|--------|
| **Bronze** | Streams, fields, sources, entity schemas | `config/base/streams/*/config.yaml` | `streams`, `fields`, `sources` |
| **Silver** | Tables, columns, lineage, DQ rules | `silver_etl` section of stream config | `silver_tables`, `silver_columns`, `silver_lineage`, `silver_dq_rules` |

### 1.4 Population Mechanism

The data dictionary is populated via shell script sync (`deploy/pi/deploy.sh:sync_to_data_dictionary()`):

1. **Parses YAML configs** using `yaml_get()` and `yaml_array_get()` helpers
2. **Generates INSERT/UPSERT SQL** for each metadata type
3. **Executes against TimescaleDB** via psql
4. **Two-pass algorithm for Silver** (multiple streams can feed one Silver table)

**Key Insight**: This Bash-based sync mechanism will need to be extended (or replaced by Rust interpreter) for Gold layer.

---

## 2. MCP Tools for Data Dictionary Access

### 2.1 Available MCP Tools

The `ndp-mcp-server` provides dictionary access via the `DictionaryStore` trait:

| MCP Tool | Purpose | SQL Query Target |
|----------|---------|------------------|
| `query_dictionary` | Search columns by name/description | `v_complete_dictionary` |
| `describe_column` | Get detailed column metadata | `silver_columns` + `silver_lineage` |
| `trace_lineage` | Bronze-to-Silver lineage chain | `silver_lineage` + `fields` + `silver_dq_rules` |
| `list_dq_rules` | List DQ rules with filters | `silver_dq_rules` |
| `describe_silver_table` | Silver table metadata | `silver_tables` |
| `list_silver_tables` | List all Silver tables | `silver_tables` |

### 2.2 Trait Interface

From `/workspaces/neural-data-platform/core/ndp-mcp-server/src/storage/timescale_dictionary.rs`:

```rust
#[async_trait]
pub trait DictionaryStore {
    async fn search(&self, query: &str, layer: Option<String>)
        -> McpResult<Vec<DictionaryEntry>>;
    async fn describe_column(&self, table_or_stream: &str, column_name: &str)
        -> McpResult<ColumnDescription>;
    async fn trace_lineage(&self, silver_table: &str, silver_column: &str)
        -> McpResult<LineageTrace>;
    async fn list_dq_rules(&self, table: Option<String>, column: Option<String>)
        -> McpResult<Vec<DqRuleInfo>>;
}
```

---

## 3. Gold Layer Metadata Requirements

### 3.1 From V1.1 Roadmap Analysis

The Gold layer introduces new metadata concepts:

| Metadata Type | Description | Examples |
|---------------|-------------|----------|
| **Gold tables/views** | Continuous aggregates, aligned views | `gold.air_quality_hourly`, `gold.aligned_hourly` |
| **Gold columns** | Computed fields with formulas | `pm25_mean`, `pm25_lag_1h`, `co2_trend_4h` |
| **Feature definitions** | Computation specifications | Rolling windows, lag intervals, aggregation functions |
| **Lineage (Silver-to-Gold)** | Source columns and transformations | `silver.air_quality_observations.pm25` -> `gold.air_quality_hourly.pm25_mean` |
| **Event types** | State transitions, threshold crossings | `state_transition`, `threshold_crossing` |
| **Objectives** | Declared targets for metrics | `co2 < 800 ppm`, `pm25 < 12 ug/m3` |
| **Stream classification** | Type categorization | `observation`, `state_event`, `forecast`, `dimension` |

### 3.2 Gold-Specific Metadata Characteristics

| Characteristic | Bronze/Silver | Gold | Implication |
|----------------|---------------|------|-------------|
| **Source** | Raw data / cleaned data | Computed features | Lineage traces to Silver |
| **Schema definition** | Config YAML | Config JSON (gold_etl) | Different parser needed |
| **Refresh policy** | N/A | Continuous aggregate policies | Need to track refresh config |
| **Computation logic** | Direct mapping | Aggregations, windows, functions | Need formula storage |
| **View dependencies** | Simple | Complex (multi-table joins) | Need dependency graph |

---

## 4. Options Analysis

### 4.1 Option A: Extend Existing Tables with Layer Discriminator

Add a `layer` column to existing tables:

```sql
ALTER TABLE data_dictionary.silver_columns
ADD COLUMN layer TEXT DEFAULT 'silver';

-- Query becomes:
SELECT * FROM data_dictionary.silver_columns WHERE layer = 'gold';
```

**Pros:**
- Minimal schema changes
- Existing MCP tools mostly work

**Cons:**
- Column semantics differ between layers (Gold has formulas, Silver has lineage to Bronze)
- "silver_columns" name becomes misleading
- Index efficiency degrades
- Validation logic becomes complex

**Verdict: REJECTED** - Conflates different concepts; makes maintenance harder.

### 4.2 Option B: Separate `gold` Schema

Create entirely new schema:

```sql
CREATE SCHEMA gold_dictionary;

CREATE TABLE gold_dictionary.tables (...);
CREATE TABLE gold_dictionary.columns (...);
CREATE TABLE gold_dictionary.features (...);
```

**Pros:**
- Clean separation
- Gold-specific tables without compromises

**Cons:**
- Cross-schema joins needed for unified views
- Separate sync mechanism required
- Inconsistent with established pattern
- More complex MCP implementation

**Verdict: REJECTED** - Breaks established single-schema pattern.

### 4.3 Option C: Extend `data_dictionary` Schema with Gold-Specific Tables (RECOMMENDED)

Add new tables following Silver pattern:

```sql
-- In data_dictionary schema
CREATE TABLE data_dictionary.gold_tables (...);
CREATE TABLE data_dictionary.gold_columns (...);
CREATE TABLE data_dictionary.gold_features (...);
CREATE TABLE data_dictionary.gold_lineage (...);
CREATE TABLE data_dictionary.objectives (...);
CREATE TABLE data_dictionary.stream_classification (...);
CREATE TABLE data_dictionary.event_types (...);

-- Unified views extended
CREATE OR REPLACE VIEW data_dictionary.v_complete_dictionary AS
SELECT 'bronze' AS layer, ... FROM data_dictionary.fields
UNION ALL
SELECT 'silver' AS layer, ... FROM data_dictionary.silver_columns
UNION ALL
SELECT 'gold' AS layer, ... FROM data_dictionary.gold_columns;
```

**Pros:**
- Consistent with Silver layer pattern (proven)
- Single schema for all dictionary queries
- Clean table naming (`gold_*`)
- Unified views include all layers
- MCP trait can be extended naturally
- Sync mechanism follows established pattern

**Cons:**
- More tables in schema
- Need to extend sync scripts

**Verdict: RECOMMENDED** - Follows established patterns, enables unified queries, minimal disruption.

---

## 5. Recommended Schema Design

### 5.1 New Tables for Gold Layer

#### 5.1.1 `gold_tables` - Gold Layer Table/View Metadata

```sql
CREATE TABLE IF NOT EXISTS data_dictionary.gold_tables (
    -- Primary key: fully-qualified name (e.g., 'gold.air_quality_hourly')
    table_name          TEXT PRIMARY KEY,

    -- PostgreSQL schema name (always 'gold')
    schema_name         TEXT NOT NULL DEFAULT 'gold',

    -- Type: 'continuous_aggregate', 'materialized_view', 'view', 'table'
    object_type         TEXT NOT NULL,

    -- Human-readable description
    description         TEXT,

    -- What one row represents
    grain               TEXT,

    -- Source tables (Silver or other Gold)
    source_tables       TEXT[] NOT NULL DEFAULT '{}',

    -- For continuous aggregates: time column
    time_column         TEXT,

    -- For continuous aggregates: bucket interval
    bucket_interval     INTERVAL,

    -- Refresh policy: schedule_interval
    refresh_interval    INTERVAL,

    -- Refresh policy: start_offset (lookback)
    refresh_lookback    INTERVAL,

    -- Stream ID if stream-specific (NULL for cross-stream views)
    stream_id           TEXT,

    -- Stream type classification
    stream_type         TEXT,

    -- Audit columns
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

COMMENT ON TABLE data_dictionary.gold_tables IS
    'Metadata for Gold layer tables, views, and continuous aggregates';
COMMENT ON COLUMN data_dictionary.gold_tables.object_type IS
    'Type: continuous_aggregate, materialized_view, view, or table';
COMMENT ON COLUMN data_dictionary.gold_tables.source_tables IS
    'Source tables (Silver or other Gold) that feed this object';
```

#### 5.1.2 `gold_columns` - Gold Layer Column Definitions

```sql
CREATE TABLE IF NOT EXISTS data_dictionary.gold_columns (
    -- Surrogate primary key
    id                  SERIAL PRIMARY KEY,

    -- Reference to parent Gold table
    table_name          TEXT NOT NULL
                        REFERENCES data_dictionary.gold_tables(table_name)
                        ON DELETE CASCADE,

    -- Column name in Gold table
    column_name         TEXT NOT NULL,

    -- PostgreSQL data type
    data_type           TEXT NOT NULL,

    -- Measurement unit (inherited from source or computed)
    unit                TEXT,

    -- Human-readable description
    description         TEXT,

    -- Feature category: 'aggregate', 'lag', 'rolling', 'trend', 'computed', 'passthrough'
    feature_type        TEXT,

    -- Computation formula (human-readable, e.g., 'AVG(pm25)')
    formula             TEXT,

    -- Whether column allows NULL
    nullable            BOOLEAN NOT NULL DEFAULT true,

    -- Display ordering
    sort_order          INTEGER NOT NULL DEFAULT 0,

    -- Audit columns
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Natural key
    UNIQUE(table_name, column_name)
);

COMMENT ON TABLE data_dictionary.gold_columns IS
    'Column definitions for Gold layer tables including feature metadata';
COMMENT ON COLUMN data_dictionary.gold_columns.feature_type IS
    'Feature category: aggregate, lag, rolling, trend, computed, passthrough';
COMMENT ON COLUMN data_dictionary.gold_columns.formula IS
    'Human-readable computation formula (e.g., AVG(pm25), LAG(co2, 1))';
```

#### 5.1.3 `gold_features` - Feature Configuration Metadata

```sql
CREATE TABLE IF NOT EXISTS data_dictionary.gold_features (
    -- Surrogate primary key
    id                  SERIAL PRIMARY KEY,

    -- Reference to Gold column
    gold_table          TEXT NOT NULL,
    gold_column         TEXT NOT NULL,

    -- Feature type: 'lag', 'rolling', 'trend', 'percentile', 'aggregate'
    feature_type        TEXT NOT NULL,

    -- Feature parameters as JSONB
    -- For lag: {"lag_hours": [1, 6, 24]}
    -- For rolling: {"window": "4 hours", "stat": "mean"}
    -- For trend: {"window": "4 hours"}
    -- For aggregate: {"stat": "mean"}
    params              JSONB NOT NULL DEFAULT '{}',

    -- Source field(s) used in computation
    source_fields       TEXT[] NOT NULL,

    -- Audit columns
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    UNIQUE(gold_table, gold_column)
);

COMMENT ON TABLE data_dictionary.gold_features IS
    'Feature computation configuration for Gold layer columns';
```

#### 5.1.4 `gold_lineage` - Silver-to-Gold Mappings

```sql
CREATE TABLE IF NOT EXISTS data_dictionary.gold_lineage (
    -- Surrogate primary key
    id                  SERIAL PRIMARY KEY,

    -- Target Gold table (fully-qualified name)
    gold_table          TEXT NOT NULL,

    -- Target Gold column
    gold_column         TEXT NOT NULL,

    -- Source table (Silver or another Gold)
    source_table        TEXT NOT NULL,

    -- Source column
    source_column       TEXT NOT NULL,

    -- Transformation type: 'aggregate', 'lag', 'rolling', 'passthrough', 'computed'
    transformation      TEXT NOT NULL DEFAULT 'passthrough',

    -- Audit columns
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Natural key: one mapping per (gold_table, gold_column, source_table, source_column)
    UNIQUE(gold_table, gold_column, source_table, source_column)
);

COMMENT ON TABLE data_dictionary.gold_lineage IS
    'Silver-to-Gold and Gold-to-Gold lineage mappings';
```

#### 5.1.5 `objectives` - Declared Target Specifications

```sql
CREATE TABLE IF NOT EXISTS data_dictionary.objectives (
    -- Primary key: objective ID
    id                  TEXT PRIMARY KEY,

    -- Human-readable description
    description         TEXT,

    -- Objective targets as JSONB array
    -- [{"stream": "air-quality", "metric": "co2", "condition": "<", "threshold": 800, "unit": "ppm", "priority": "high"}]
    targets             JSONB NOT NULL DEFAULT '[]',

    -- Constraints as JSONB array
    -- [{"description": "...", "stream": "...", "metric": "...", "condition": "...", "threshold": ...}]
    constraints         JSONB NOT NULL DEFAULT '[]',

    -- Whether objective is active
    enabled             BOOLEAN NOT NULL DEFAULT true,

    -- Audit columns
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

COMMENT ON TABLE data_dictionary.objectives IS
    'Declared objectives with target metrics and constraints';
```

#### 5.1.6 `stream_classification` - Stream Type Metadata

```sql
CREATE TABLE IF NOT EXISTS data_dictionary.stream_classification (
    -- Primary key: stream_id
    stream_id           TEXT PRIMARY KEY
                        REFERENCES data_dictionary.streams(stream_id)
                        ON DELETE CASCADE,

    -- Stream type: 'observation', 'state_event', 'forecast', 'dimension'
    stream_type         TEXT NOT NULL,

    -- Correlation role: 'cause', 'effect', 'context', 'metadata'
    correlation_role    TEXT,

    -- Description of classification
    description         TEXT,

    -- Audit columns
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

COMMENT ON TABLE data_dictionary.stream_classification IS
    'Stream type classification for correlation analysis';
COMMENT ON COLUMN data_dictionary.stream_classification.stream_type IS
    'Type: observation (continuous), state_event (discrete), forecast, dimension';
COMMENT ON COLUMN data_dictionary.stream_classification.correlation_role IS
    'Role in correlation: cause, effect, context, or metadata';
```

#### 5.1.7 `event_types` - Event Type Definitions

```sql
CREATE TABLE IF NOT EXISTS data_dictionary.event_types (
    -- Primary key: event type name
    event_type          TEXT PRIMARY KEY,

    -- Description of event type
    description         TEXT,

    -- Source view/table for this event type
    source_view         TEXT,

    -- JSONB schema for details field
    details_schema      JSONB NOT NULL DEFAULT '{}',

    -- Whether event type is active
    enabled             BOOLEAN NOT NULL DEFAULT true,

    -- Version for V1.2+ additions
    version             TEXT NOT NULL DEFAULT '1.1',

    -- Audit columns
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

COMMENT ON TABLE data_dictionary.event_types IS
    'Event type definitions for unified events abstraction';

-- Seed V1.1 event types
INSERT INTO data_dictionary.event_types (event_type, description, source_view, details_schema, version)
VALUES
    ('state_transition', 'State change events from state_event streams', 'gold.state_transitions',
     '{"type": "object", "properties": {"from_state": {"type": "string"}, "to_state": {"type": "string"}, "duration_in_previous_ms": {"type": "number"}}}'::jsonb, '1.1'),
    ('threshold_crossing', 'Metric threshold crossing events', 'gold.threshold_crossings',
     '{"type": "object", "properties": {"metric": {"type": "string"}, "threshold": {"type": "number"}, "direction": {"type": "string"}, "value": {"type": "number"}, "objective_id": {"type": "string"}}}'::jsonb, '1.1')
ON CONFLICT (event_type) DO NOTHING;
```

### 5.2 Indexes

```sql
-- gold_tables
CREATE INDEX IF NOT EXISTS idx_gold_tables_stream
    ON data_dictionary.gold_tables(stream_id);
CREATE INDEX IF NOT EXISTS idx_gold_tables_type
    ON data_dictionary.gold_tables(object_type);

-- gold_columns
CREATE INDEX IF NOT EXISTS idx_gold_columns_table
    ON data_dictionary.gold_columns(table_name);
CREATE INDEX IF NOT EXISTS idx_gold_columns_feature_type
    ON data_dictionary.gold_columns(feature_type);
CREATE INDEX IF NOT EXISTS idx_gold_columns_column_name
    ON data_dictionary.gold_columns(column_name);

-- gold_features
CREATE INDEX IF NOT EXISTS idx_gold_features_table
    ON data_dictionary.gold_features(gold_table);
CREATE INDEX IF NOT EXISTS idx_gold_features_type
    ON data_dictionary.gold_features(feature_type);

-- gold_lineage
CREATE INDEX IF NOT EXISTS idx_gold_lineage_table
    ON data_dictionary.gold_lineage(gold_table);
CREATE INDEX IF NOT EXISTS idx_gold_lineage_source
    ON data_dictionary.gold_lineage(source_table);
CREATE INDEX IF NOT EXISTS idx_gold_lineage_column
    ON data_dictionary.gold_lineage(gold_column);

-- stream_classification
CREATE INDEX IF NOT EXISTS idx_stream_classification_type
    ON data_dictionary.stream_classification(stream_type);
```

### 5.3 Extended Unified Views

```sql
-- v_complete_dictionary: Add Gold layer
CREATE OR REPLACE VIEW data_dictionary.v_complete_dictionary AS

-- Bronze columns
SELECT
    'bronze' AS layer,
    stream_id AS entity,
    field_name AS column_name,
    field_type AS data_type,
    unit,
    description,
    nullable,
    validation_min AS range_min,
    validation_max AS range_max
FROM data_dictionary.fields

UNION ALL

-- Silver columns
SELECT
    'silver' AS layer,
    sc.table_name AS entity,
    sc.column_name,
    sc.data_type,
    sc.unit,
    sc.description,
    sc.nullable,
    (dr.rule_params->>'min')::DOUBLE PRECISION AS range_min,
    (dr.rule_params->>'max')::DOUBLE PRECISION AS range_max
FROM data_dictionary.silver_columns sc
LEFT JOIN data_dictionary.silver_dq_rules dr
    ON sc.table_name = dr.silver_table
    AND sc.column_name = dr.silver_column
    AND dr.rule_name = 'range_check'

UNION ALL

-- Gold columns (NEW)
SELECT
    'gold' AS layer,
    gc.table_name AS entity,
    gc.column_name,
    gc.data_type,
    gc.unit,
    gc.description,
    gc.nullable,
    NULL AS range_min,
    NULL AS range_max
FROM data_dictionary.gold_columns gc;

COMMENT ON VIEW data_dictionary.v_complete_dictionary IS
    'Unified view of Bronze, Silver, and Gold column definitions';
```

```sql
-- v_full_lineage: Cross-layer lineage
CREATE OR REPLACE VIEW data_dictionary.v_full_lineage AS

-- Bronze to Silver
SELECT
    'bronze_to_silver' AS lineage_type,
    sl.source_stream AS source_entity,
    sl.source_path AS source_column,
    sl.silver_table AS target_entity,
    sl.silver_column AS target_column,
    sl.transformation
FROM data_dictionary.silver_lineage sl

UNION ALL

-- Silver to Gold
SELECT
    'silver_to_gold' AS lineage_type,
    gl.source_table AS source_entity,
    gl.source_column,
    gl.gold_table AS target_entity,
    gl.gold_column AS target_column,
    gl.transformation
FROM data_dictionary.gold_lineage gl
WHERE gl.source_table LIKE 'silver.%'

UNION ALL

-- Gold to Gold (chained features)
SELECT
    'gold_to_gold' AS lineage_type,
    gl.source_table AS source_entity,
    gl.source_column,
    gl.gold_table AS target_entity,
    gl.gold_column AS target_column,
    gl.transformation
FROM data_dictionary.gold_lineage gl
WHERE gl.source_table LIKE 'gold.%';

COMMENT ON VIEW data_dictionary.v_full_lineage IS
    'Cross-layer lineage from Bronze through Silver to Gold';
```

```sql
-- v_gold_table_overview: Gold table summary
CREATE OR REPLACE VIEW data_dictionary.v_gold_table_overview AS
SELECT
    gt.table_name,
    gt.schema_name,
    gt.object_type,
    gt.description,
    gt.grain,
    gt.source_tables,
    gt.stream_id,
    gt.stream_type,
    gt.bucket_interval,
    gt.refresh_interval,
    COUNT(DISTINCT gc.id) AS column_count,
    COUNT(DISTINCT gl.id) AS lineage_count,
    COUNT(DISTINCT gf.id) AS feature_count,
    gt.created_at,
    gt.updated_at
FROM data_dictionary.gold_tables gt
LEFT JOIN data_dictionary.gold_columns gc ON gt.table_name = gc.table_name
LEFT JOIN data_dictionary.gold_lineage gl ON gt.table_name = gl.gold_table
LEFT JOIN data_dictionary.gold_features gf ON gt.table_name = gf.gold_table
GROUP BY gt.table_name, gt.schema_name, gt.object_type, gt.description, gt.grain,
         gt.source_tables, gt.stream_id, gt.stream_type, gt.bucket_interval,
         gt.refresh_interval, gt.created_at, gt.updated_at;

COMMENT ON VIEW data_dictionary.v_gold_table_overview IS
    'Gold table summary with column, lineage, and feature counts';
```

---

## 6. MCP Tool Extensions

### 6.1 New MCP Tools for Gold Layer

| Tool | Purpose | Query Target |
|------|---------|--------------|
| `list_gold_tables` | List all Gold layer objects | `gold_tables` |
| `describe_gold_table` | Get Gold table metadata | `gold_tables` |
| `search_features` | Search by feature type | `gold_columns`, `gold_features` |
| `trace_gold_lineage` | Silver-to-Gold lineage chain | `gold_lineage` + `silver_lineage` |
| `list_objectives` | List declared objectives | `objectives` |
| `get_objective` | Get single objective details | `objectives` |
| `list_target_metrics` | List all target metrics | `objectives` (parsed) |
| `get_stream_classification` | Get stream type metadata | `stream_classification` |
| `list_event_types` | List available event types | `event_types` |

### 6.2 Extended `DictionaryStore` Trait

```rust
#[async_trait]
pub trait DictionaryStore {
    // Existing methods
    async fn search(&self, query: &str, layer: Option<String>)
        -> McpResult<Vec<DictionaryEntry>>;
    async fn describe_column(&self, table_or_stream: &str, column_name: &str)
        -> McpResult<ColumnDescription>;
    async fn trace_lineage(&self, table: &str, column: &str)
        -> McpResult<LineageTrace>;
    async fn list_dq_rules(&self, table: Option<String>, column: Option<String>)
        -> McpResult<Vec<DqRuleInfo>>;

    // NEW: Gold layer methods
    async fn list_gold_tables(&self, stream_id: Option<String>)
        -> McpResult<Vec<GoldTableInfo>>;
    async fn describe_gold_table(&self, table_name: &str)
        -> McpResult<GoldTableDescription>;
    async fn search_features(&self, feature_type: Option<String>, query: &str)
        -> McpResult<Vec<FeatureInfo>>;
    async fn trace_full_lineage(&self, gold_table: &str, gold_column: &str)
        -> McpResult<FullLineageTrace>;
    async fn list_objectives(&self)
        -> McpResult<Vec<ObjectiveInfo>>;
    async fn get_objective(&self, id: &str)
        -> McpResult<ObjectiveDetails>;
    async fn get_stream_classification(&self, stream_id: &str)
        -> McpResult<StreamClassification>;
    async fn list_event_types(&self)
        -> McpResult<Vec<EventTypeInfo>>;
}
```

---

## 7. Sync Mechanism Options

### 7.1 Option A: Extend Bash Script (Not Recommended)

Extend existing `sync_to_data_dictionary()` in `deploy.sh`:

**Pros:**
- Consistent with current Bronze/Silver approach

**Cons:**
- Bash parsing of complex JSON is fragile
- Gold ETL config is more complex than Silver
- Feature computation metadata is hard to extract

### 7.2 Option B: Gold ETL Interpreter Generates Metadata (Recommended)

The Rust-based Gold ETL Interpreter (v11-A02) generates both:
1. SQL for creating Gold objects (views, aggregates)
2. SQL for populating data dictionary metadata

**Flow:**
```
gold_etl config JSON
        │
        ▼
┌───────────────────────────────┐
│ Gold ETL Interpreter (Rust)   │
│                               │
│ 1. Parse gold_etl config      │
│ 2. Generate CREATE VIEW SQL   │
│ 3. Generate dictionary INSERTs│
│ 4. Execute both atomically    │
└───────────────────────────────┘
        │
        ▼
┌───────────────────────────────┐
│ TimescaleDB                   │
│ ├── gold.* views/aggregates   │
│ └── data_dictionary.gold_*    │
└───────────────────────────────┘
```

**Pros:**
- Type-safe config parsing
- Complex feature metadata easily extracted
- Atomic: Gold objects and metadata created together
- Testable

**Cons:**
- Different mechanism than Bronze/Silver (transition period)

**Verdict: RECOMMENDED** - The Gold ETL Interpreter already needs to parse config; generating metadata is a natural extension.

---

## 8. Population Example

### 8.1 Example: Air Quality Hourly Aggregate

**Config:**
```json
{
  "gold_etl": {
    "enabled": true,
    "aggregates": {
      "granularities": ["1 hour"],
      "fields": {
        "pm25": { "metrics": ["mean", "std", "min", "max"] },
        "co2": { "metrics": ["mean", "std"] }
      }
    },
    "features": {
      "lag": {
        "enabled": true,
        "lags_hours": [1, 6, 24],
        "fields": ["pm25", "co2"]
      }
    }
  }
}
```

**Generated Metadata Inserts:**

```sql
-- gold_tables
INSERT INTO data_dictionary.gold_tables (
    table_name, schema_name, object_type, description, grain,
    source_tables, time_column, bucket_interval, refresh_interval, stream_id
) VALUES (
    'gold.air_quality_hourly',
    'gold',
    'continuous_aggregate',
    'Hourly aggregates for air-quality stream',
    'One row per hour per sensor',
    ARRAY['silver.air_quality_observations'],
    'bucket',
    INTERVAL '1 hour',
    INTERVAL '15 minutes',
    'air-quality'
) ON CONFLICT (table_name) DO UPDATE SET ...;

-- gold_columns (aggregates)
INSERT INTO data_dictionary.gold_columns (table_name, column_name, data_type, unit, description, feature_type, formula, sort_order)
VALUES
    ('gold.air_quality_hourly', 'bucket', 'TIMESTAMPTZ', NULL, 'Hour bucket timestamp', 'passthrough', 'time_bucket(''1 hour'', observation_time)', 1),
    ('gold.air_quality_hourly', 'ndp_id', 'TEXT', NULL, 'Sensor identifier', 'passthrough', NULL, 2),
    ('gold.air_quality_hourly', 'pm25_mean', 'DOUBLE PRECISION', 'ug/m3', 'PM2.5 hourly mean', 'aggregate', 'AVG(pm25)', 10),
    ('gold.air_quality_hourly', 'pm25_std', 'DOUBLE PRECISION', 'ug/m3', 'PM2.5 hourly standard deviation', 'aggregate', 'STDDEV(pm25)', 11),
    ('gold.air_quality_hourly', 'pm25_min', 'DOUBLE PRECISION', 'ug/m3', 'PM2.5 hourly minimum', 'aggregate', 'MIN(pm25)', 12),
    ('gold.air_quality_hourly', 'pm25_max', 'DOUBLE PRECISION', 'ug/m3', 'PM2.5 hourly maximum', 'aggregate', 'MAX(pm25)', 13),
    ('gold.air_quality_hourly', 'co2_mean', 'DOUBLE PRECISION', 'ppm', 'CO2 hourly mean', 'aggregate', 'AVG(co2)', 20),
    ('gold.air_quality_hourly', 'co2_std', 'DOUBLE PRECISION', 'ppm', 'CO2 hourly standard deviation', 'aggregate', 'STDDEV(co2)', 21),
    ('gold.air_quality_hourly', 'pm25_lag_1h', 'DOUBLE PRECISION', 'ug/m3', 'PM2.5 value 1 hour ago', 'lag', 'LAG(pm25_mean, 1)', 30),
    ('gold.air_quality_hourly', 'pm25_lag_6h', 'DOUBLE PRECISION', 'ug/m3', 'PM2.5 value 6 hours ago', 'lag', 'LAG(pm25_mean, 6)', 31),
    ('gold.air_quality_hourly', 'pm25_lag_24h', 'DOUBLE PRECISION', 'ug/m3', 'PM2.5 value 24 hours ago', 'lag', 'LAG(pm25_mean, 24)', 32),
    ('gold.air_quality_hourly', 'co2_lag_1h', 'DOUBLE PRECISION', 'ppm', 'CO2 value 1 hour ago', 'lag', 'LAG(co2_mean, 1)', 40),
    ('gold.air_quality_hourly', 'co2_lag_6h', 'DOUBLE PRECISION', 'ppm', 'CO2 value 6 hours ago', 'lag', 'LAG(co2_mean, 6)', 41),
    ('gold.air_quality_hourly', 'co2_lag_24h', 'DOUBLE PRECISION', 'ppm', 'CO2 value 24 hours ago', 'lag', 'LAG(co2_mean, 24)', 42)
ON CONFLICT (table_name, column_name) DO UPDATE SET ...;

-- gold_features
INSERT INTO data_dictionary.gold_features (gold_table, gold_column, feature_type, params, source_fields)
VALUES
    ('gold.air_quality_hourly', 'pm25_mean', 'aggregate', '{"stat": "mean"}'::jsonb, ARRAY['pm25']),
    ('gold.air_quality_hourly', 'pm25_std', 'aggregate', '{"stat": "std"}'::jsonb, ARRAY['pm25']),
    ('gold.air_quality_hourly', 'pm25_lag_1h', 'lag', '{"lag_hours": 1}'::jsonb, ARRAY['pm25_mean']),
    ('gold.air_quality_hourly', 'pm25_lag_6h', 'lag', '{"lag_hours": 6}'::jsonb, ARRAY['pm25_mean']),
    ('gold.air_quality_hourly', 'pm25_lag_24h', 'lag', '{"lag_hours": 24}'::jsonb, ARRAY['pm25_mean'])
ON CONFLICT (gold_table, gold_column) DO UPDATE SET ...;

-- gold_lineage
INSERT INTO data_dictionary.gold_lineage (gold_table, gold_column, source_table, source_column, transformation)
VALUES
    ('gold.air_quality_hourly', 'pm25_mean', 'silver.air_quality_observations', 'pm25', 'aggregate'),
    ('gold.air_quality_hourly', 'pm25_std', 'silver.air_quality_observations', 'pm25', 'aggregate'),
    ('gold.air_quality_hourly', 'pm25_lag_1h', 'gold.air_quality_hourly', 'pm25_mean', 'lag'),
    ('gold.air_quality_hourly', 'pm25_lag_6h', 'gold.air_quality_hourly', 'pm25_mean', 'lag'),
    ('gold.air_quality_hourly', 'pm25_lag_24h', 'gold.air_quality_hourly', 'pm25_mean', 'lag')
ON CONFLICT (gold_table, gold_column, source_table, source_column) DO UPDATE SET ...;
```

---

## 9. Migration Path

### 9.1 Migration Script Location

`deploy/pi/init-scripts/005_gold_data_dictionary.sql`

### 9.2 Migration Approach

1. **Create new tables** (IF NOT EXISTS)
2. **Add indexes**
3. **Update/replace unified views**
4. **Seed event types**
5. **Idempotent** (safe to re-run)

### 9.3 Deploy Order

1. Run `005_gold_data_dictionary.sql` migration
2. Deploy Gold ETL Interpreter
3. Apply gold_etl configs (interpreter populates metadata)
4. Verify via MCP tools

---

## 10. Summary

### 10.1 Recommended Approach

**Extend `data_dictionary` schema with Gold-specific tables**, following the established Silver layer pattern from ADR-009-001.

### 10.2 New Tables

| Table | Purpose |
|-------|---------|
| `gold_tables` | Gold table/view metadata |
| `gold_columns` | Gold column definitions with feature type |
| `gold_features` | Feature computation configuration |
| `gold_lineage` | Silver-to-Gold and Gold-to-Gold mappings |
| `objectives` | Declared target specifications |
| `stream_classification` | Stream type metadata for correlation |
| `event_types` | Event type definitions |

### 10.3 Key Benefits

1. **Consistency**: Follows proven Silver pattern
2. **Unified queries**: All layers in single schema
3. **Full lineage**: Bronze -> Silver -> Gold traceability
4. **Feature discovery**: Query features by type, formula, source
5. **Objective integration**: Targets visible alongside data
6. **Extensible**: Ready for V1.2 event types and V1.3 model metadata

### 10.4 Next Steps

1. **Create migration script**: `005_gold_data_dictionary.sql`
2. **Extend MCP trait**: Add Gold-specific methods
3. **Implement interpreter metadata generation**: Part of v11-A02
4. **Update sync_status**: Track Gold layer sync
5. **Test unified views**: Verify cross-layer queries work

---

## References

- [ADR-009-001: Silver Layer Data Dictionary Tables](/workspaces/neural-data-platform/product/features/dp-009/architecture/ADR-009-001-silver-dictionary-tables.md)
- [DP-016: Dictionary Flow Analysis](/workspaces/neural-data-platform/product/features/dp-016/architecture/DICTIONARY-FLOW-ANALYSIS.md)
- [FE-001: Gold Layer Foundation Scope](/workspaces/neural-data-platform/product/features/fe-001/SCOPE.md)
- [Gold Layer Feature Roadmap](/workspaces/neural-data-platform/product/features/gold-001/FEATURE-ROADMAP.md)
- [Bronze Data Dictionary DDL](/workspaces/neural-data-platform/deploy/pi/init-scripts/01-create-data-dictionary.sql)
- [Silver Data Dictionary DDL](/workspaces/neural-data-platform/deploy/pi/init-scripts/003_silver_data_dictionary.sql)
- [TimescaleDictionaryStore Implementation](/workspaces/neural-data-platform/core/ndp-mcp-server/src/storage/timescale_dictionary.rs)
