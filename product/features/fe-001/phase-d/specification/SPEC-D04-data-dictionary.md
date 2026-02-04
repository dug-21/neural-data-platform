# SPEC-D04: Gold Layer Data Dictionary (v11-010)

**Feature ID**: v11-010
**Feature Name**: Gold Layer Data Dictionary
**Priority**: Medium
**Created**: 2026-02-04
**Status**: Draft

---

## 1. Overview

### 1.1 User Story

> As a **platform user**, I want to query metadata about Gold layer tables, columns, and their lineage from Silver, so that I can understand what data is available and how it was computed.

### 1.2 Goal

Extend the existing data dictionary (Bronze, Silver) to include Gold layer metadata. This enables:
- MCP tools to describe Gold layer objects
- Dashboard builders to discover available columns
- V1.2 pattern detection to understand feature semantics

### 1.3 Scope

| In Scope | Out of Scope |
|----------|--------------|
| Gold tables metadata | Query usage statistics |
| Gold columns with feature type | Column-level access control |
| Lineage from Silver to Gold | Cross-domain lineage |
| Stream classification metadata | Historical metadata versions |

---

## 2. Functional Requirements

### 2.1 Metadata Tables (FR-D04-META)

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| FR-D04-META-001 | Create `data_dictionary.gold_tables` table | P0 | Table exists with correct schema |
| FR-D04-META-002 | Create `data_dictionary.gold_columns` table | P0 | Table exists with correct schema |
| FR-D04-META-003 | Create `data_dictionary.stream_classification` table | P0 | Table exists with correct schema |
| FR-D04-META-004 | Create `data_dictionary.domains` table | P1 | Table exists with correct schema |
| FR-D04-META-005 | Create `data_dictionary.objectives` table | P1 | Table exists with correct schema |

### 2.2 Metadata Population (FR-D04-POP)

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| FR-D04-POP-001 | Populate gold_tables during ndp-gold-ddl execution | P0 | Tables auto-populated |
| FR-D04-POP-002 | Populate gold_columns with feature type metadata | P0 | feature_type column populated |
| FR-D04-POP-003 | Populate stream_classification from stream configs | P0 | All streams classified |
| FR-D04-POP-004 | Include source expression for computed columns | P1 | SQL expression stored |
| FR-D04-POP-005 | Sync on every deploy.sh apply | P0 | Metadata stays current |

### 2.3 Query Support (FR-D04-QRY)

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| FR-D04-QRY-001 | Query Gold tables by object type | P0 | Filter by continuous_aggregate, view |
| FR-D04-QRY-002 | Query Gold columns by feature type | P0 | Filter by aggregate, lag, rolling |
| FR-D04-QRY-003 | Query stream classification | P0 | Get stream_type and correlation_role |
| FR-D04-QRY-004 | Query lineage from Silver to Gold | P1 | Trace column sources |

---

## 3. Non-Functional Requirements

### 3.1 Performance (NFR-D04-PERF)

| ID | Requirement | Target | Measurement |
|----|-------------|--------|-------------|
| NFR-D04-PERF-001 | Metadata query response | < 10ms | Query timing |
| NFR-D04-PERF-002 | Metadata sync during deploy | < 5 seconds | Deploy timing |
| NFR-D04-PERF-003 | Storage overhead | < 1MB | pg_relation_size |

### 3.2 Data Quality (NFR-D04-DQ)

| ID | Requirement | Target | Measurement |
|----|-------------|--------|-------------|
| NFR-D04-DQ-001 | All Gold tables documented | 100% | Completeness check |
| NFR-D04-DQ-002 | All Gold columns documented | 100% | Completeness check |
| NFR-D04-DQ-003 | All streams classified | 100% | Classification coverage |

---

## 4. Acceptance Criteria (Gherkin)

### 4.1 Metadata Tables

```gherkin
Feature: Gold Layer Data Dictionary Tables

  Scenario: gold_tables metadata exists
    Given gold.air_quality_hourly continuous aggregate exists
    When data dictionary is populated
    Then data_dictionary.gold_tables should have a row for air_quality_hourly
    And object_type should be 'continuous_aggregate'
    And source_silver_table should be 'silver.air_quality_observations'

  Scenario: gold_columns metadata includes feature type
    Given gold.air_quality_hourly has pm25_mean and pm25_lag_1h columns
    When data dictionary is populated
    Then gold_columns should have:
      | column_name     | feature_type |
      | pm25_mean       | aggregate    |
      | pm25_lag_1h     | lag          |
      | sample_count    | aggregate    |
      | bucket          | dimension    |

  Scenario: stream_classification populated
    Given stream configs have stream_type field
    When data dictionary is populated
    Then stream_classification should show:
      | stream_id       | stream_type   | correlation_role |
      | air-quality     | observation   | effect           |
      | outdoor-weather | observation   | context          |
      | home-assistant-state | state_event | cause       |
```

### 4.2 Query Capabilities

```gherkin
Feature: Data Dictionary Queries

  Scenario: List all Gold continuous aggregates
    When I query "SELECT * FROM data_dictionary.gold_tables WHERE object_type = 'continuous_aggregate'"
    Then I should get all hourly aggregate views

  Scenario: List all lag features
    When I query "SELECT * FROM data_dictionary.gold_columns WHERE feature_type = 'lag'"
    Then I should get all lag feature columns across all Gold tables

  Scenario: Get stream correlation role
    When I query stream classification for 'home-assistant-state'
    Then correlation_role should be 'cause'
    And stream_type should be 'state_event'
```

---

## 5. Schema Definitions

### 5.1 gold_tables

```sql
CREATE TABLE data_dictionary.gold_tables (
    table_name          TEXT PRIMARY KEY,
    table_schema        TEXT NOT NULL DEFAULT 'gold',
    object_type         TEXT NOT NULL,  -- 'continuous_aggregate', 'view', 'materialized_view'
    source_silver_table TEXT,           -- REFERENCES silver_tables(table_name)
    source_stream_id    TEXT,           -- Stream that provides the data
    bucket_interval     INTERVAL,       -- e.g., '1 hour', '1 day'
    refresh_interval    INTERVAL,       -- e.g., '15 minutes'
    description         TEXT,
    created_at          TIMESTAMPTZ DEFAULT NOW(),
    updated_at          TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT valid_object_type CHECK (
        object_type IN ('continuous_aggregate', 'view', 'materialized_view')
    )
);

-- Index for common queries
CREATE INDEX idx_gold_tables_object_type ON data_dictionary.gold_tables(object_type);
CREATE INDEX idx_gold_tables_stream ON data_dictionary.gold_tables(source_stream_id);
```

### 5.2 gold_columns

```sql
CREATE TABLE data_dictionary.gold_columns (
    table_name          TEXT NOT NULL,
    column_name         TEXT NOT NULL,
    data_type           TEXT NOT NULL,
    feature_type        TEXT,           -- 'aggregate', 'lag', 'rolling', 'trend', 'dimension', 'identity'
    source_field        TEXT,           -- Original field name (e.g., 'pm25')
    source_expression   TEXT,           -- SQL expression (e.g., 'AVG(pm25)')
    metric_type         TEXT,           -- 'mean', 'std', 'min', 'max', etc.
    lag_hours           INTEGER,        -- For lag features
    window_hours        INTEGER,        -- For rolling features
    unit                TEXT,           -- Unit of measurement
    description         TEXT,
    created_at          TIMESTAMPTZ DEFAULT NOW(),

    PRIMARY KEY (table_name, column_name),
    FOREIGN KEY (table_name) REFERENCES data_dictionary.gold_tables(table_name)
        ON DELETE CASCADE
);

-- Indexes for common queries
CREATE INDEX idx_gold_columns_feature_type ON data_dictionary.gold_columns(feature_type);
CREATE INDEX idx_gold_columns_source_field ON data_dictionary.gold_columns(source_field);
```

### 5.3 stream_classification

```sql
CREATE TABLE data_dictionary.stream_classification (
    stream_id           TEXT PRIMARY KEY,
    stream_type         TEXT NOT NULL,  -- 'observation', 'state_event', 'forecast', 'dimension'
    correlation_role    TEXT,           -- 'cause', 'effect', 'context', 'metadata'
    description         TEXT,
    created_at          TIMESTAMPTZ DEFAULT NOW(),
    updated_at          TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT valid_stream_type CHECK (
        stream_type IN ('observation', 'state_event', 'forecast', 'dimension')
    ),
    CONSTRAINT valid_correlation_role CHECK (
        correlation_role IS NULL OR
        correlation_role IN ('cause', 'effect', 'context', 'metadata')
    )
);
```

### 5.4 domains

```sql
CREATE TABLE data_dictionary.domains (
    domain_id           TEXT PRIMARY KEY,
    description         TEXT,
    stream_ids          TEXT[],         -- Array of stream_ids in this domain
    alignment_view      TEXT,           -- Name of aligned view
    created_at          TIMESTAMPTZ DEFAULT NOW(),
    updated_at          TIMESTAMPTZ DEFAULT NOW()
);
```

### 5.5 objectives

```sql
CREATE TABLE data_dictionary.objectives (
    objective_id        TEXT PRIMARY KEY,
    domain_id           TEXT REFERENCES data_dictionary.domains(domain_id),
    description         TEXT,
    target_stream       TEXT NOT NULL,
    target_metric       TEXT NOT NULL,
    condition           TEXT NOT NULL,  -- '<', '>', '<=', '>=', '='
    threshold           DOUBLE PRECISION NOT NULL,
    unit                TEXT,
    priority            TEXT DEFAULT 'medium',
    created_at          TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT valid_condition CHECK (
        condition IN ('<', '>', '<=', '>=', '=')
    ),
    CONSTRAINT valid_priority CHECK (
        priority IN ('low', 'medium', 'high', 'critical')
    )
);
```

---

## 6. Feature Type Taxonomy

### 6.1 Feature Types

| Feature Type | Description | Example Columns |
|--------------|-------------|-----------------|
| `aggregate` | Statistical aggregate over bucket | pm25_mean, co2_std, sample_count |
| `lag` | Time-lagged value | pm25_lag_1h, co2_lag_24h |
| `rolling` | Rolling window statistic | pm25_roll_4h_mean |
| `trend` | Trend indicator | pm25_trend_4h |
| `dimension` | Time or grouping dimension | bucket |
| `identity` | Entity identifier | ndp_id |
| `raw` | Pass-through from Silver | (rare in Gold) |

### 6.2 Feature Type Detection

The data dictionary sync should auto-detect feature types based on column naming:

| Pattern | Detected Feature Type |
|---------|----------------------|
| `*_mean`, `*_std`, `*_min`, `*_max`, `*_p95`, `*_p99` | aggregate |
| `*_lag_*h` | lag |
| `*_roll_*h_*` | rolling |
| `*_trend_*` | trend |
| `bucket` | dimension |
| `ndp_id`, `*_ndp_id` | identity |
| `sample_count` | aggregate |

---

## 7. Population Mechanism

### 7.1 Integration with ndp-gold-ddl

The `ndp-gold-ddl` tool should generate metadata INSERT statements alongside DDL:

```bash
# Generate DDL + metadata
ndp-gold-ddl generate --stream air-quality --include-metadata

# Output includes:
# 1. CREATE MATERIALIZED VIEW ...
# 2. INSERT INTO data_dictionary.gold_tables ...
# 3. INSERT INTO data_dictionary.gold_columns ...
```

### 7.2 deploy.sh Integration

The `sync_to_data_dictionary()` function must be extended:

```bash
# In deploy.sh

sync_to_data_dictionary() {
    # ... existing Bronze/Silver sync ...

    # NEW: Sync stream_classification
    for config_dir in "$CONFIG_DIR"/streams/*/; do
        local config_file="$config_dir/config.json"
        local stream_id=$(jq -r '.stream_id' "$config_file")
        local stream_type=$(jq -r '.stream_type // "observation"' "$config_file")

        cat << SQL | dcx timescaledb psql -U postgres -d ndp
INSERT INTO data_dictionary.stream_classification (stream_id, stream_type)
VALUES ('$stream_id', '$stream_type')
ON CONFLICT (stream_id) DO UPDATE SET
    stream_type = EXCLUDED.stream_type,
    updated_at = NOW();
SQL
    done

    # NEW: Sync domains
    for domain_dir in "$CONFIG_DIR"/domains/*/; do
        # ... sync domain metadata ...
    done
}
```

### 7.3 Atomic Population

Gold metadata should be populated in the same transaction as DDL:

```sql
BEGIN;

-- Create continuous aggregate
CREATE MATERIALIZED VIEW gold.air_quality_hourly ...;

-- Populate metadata
INSERT INTO data_dictionary.gold_tables (...) VALUES (...);
INSERT INTO data_dictionary.gold_columns (...) VALUES (...), (...), ...;

COMMIT;
```

---

## 8. Query Examples

### 8.1 List All Gold Layer Objects

```sql
SELECT
    gt.table_name,
    gt.object_type,
    gt.source_stream_id,
    gt.bucket_interval,
    COUNT(gc.column_name) as column_count
FROM data_dictionary.gold_tables gt
LEFT JOIN data_dictionary.gold_columns gc ON gt.table_name = gc.table_name
GROUP BY gt.table_name, gt.object_type, gt.source_stream_id, gt.bucket_interval
ORDER BY gt.table_name;
```

### 8.2 List All Lag Features

```sql
SELECT
    gc.table_name,
    gc.column_name,
    gc.source_field,
    gc.lag_hours,
    gc.unit
FROM data_dictionary.gold_columns gc
WHERE gc.feature_type = 'lag'
ORDER BY gc.table_name, gc.lag_hours;
```

### 8.3 Get Stream Classification with Correlation Role

```sql
SELECT
    sc.stream_id,
    sc.stream_type,
    sc.correlation_role,
    gt.table_name as gold_table
FROM data_dictionary.stream_classification sc
LEFT JOIN data_dictionary.gold_tables gt ON sc.stream_id = gt.source_stream_id
ORDER BY sc.correlation_role, sc.stream_id;
```

### 8.4 Trace Column Lineage

```sql
SELECT
    gc.table_name as gold_table,
    gc.column_name as gold_column,
    gc.feature_type,
    gc.source_expression,
    gt.source_silver_table,
    sc.column_name as silver_column
FROM data_dictionary.gold_columns gc
JOIN data_dictionary.gold_tables gt ON gc.table_name = gt.table_name
LEFT JOIN data_dictionary.silver_columns sc
    ON gt.source_silver_table = sc.table_name
    AND gc.source_field = sc.column_name
WHERE gc.source_field IS NOT NULL
ORDER BY gc.table_name, gc.column_name;
```

### 8.5 Get Domain Objectives

```sql
SELECT
    d.domain_id,
    o.objective_id,
    o.target_stream,
    o.target_metric,
    o.condition || ' ' || o.threshold::text || ' ' || COALESCE(o.unit, '') as condition_display,
    o.priority
FROM data_dictionary.domains d
JOIN data_dictionary.objectives o ON d.domain_id = o.domain_id
ORDER BY d.domain_id, o.priority DESC;
```

---

## 9. London TDD Interfaces

### 9.1 Interface: Metadata Generator

```rust
/// Generate metadata INSERT statements for Gold layer objects
pub struct MetadataGenerator;

impl MetadataGenerator {
    pub fn new() -> Self;

    /// Generate gold_tables INSERT for a continuous aggregate
    pub fn generate_table_metadata(
        &self,
        stream_id: &str,
        table_name: &str,
        config: &GoldEtlConfig
    ) -> String;

    /// Generate gold_columns INSERTs for all columns
    pub fn generate_column_metadata(
        &self,
        table_name: &str,
        config: &GoldEtlConfig
    ) -> Vec<String>;
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_generates_table_metadata() {
        let generator = MetadataGenerator::new();

        let sql = generator.generate_table_metadata(
            "air-quality",
            "gold.air_quality_hourly",
            &test_config()
        );

        assert!(sql.contains("INSERT INTO data_dictionary.gold_tables"));
        assert!(sql.contains("'air_quality_hourly'"));
        assert!(sql.contains("'continuous_aggregate'"));
        assert!(sql.contains("'silver.air_quality_observations'"));
    }

    #[test]
    fn test_column_metadata_includes_feature_type() {
        let generator = MetadataGenerator::new();

        let inserts = generator.generate_column_metadata(
            "air_quality_hourly",
            &test_config_with_lag()
        );

        // Should have aggregate columns
        let aggregate_insert = inserts.iter().find(|s| s.contains("pm25_mean")).unwrap();
        assert!(aggregate_insert.contains("'aggregate'"));

        // Should have lag columns
        let lag_insert = inserts.iter().find(|s| s.contains("pm25_lag_1h")).unwrap();
        assert!(lag_insert.contains("'lag'"));
        assert!(lag_insert.contains("lag_hours"));
    }
}
```

### 9.2 Interface: Classification Sync

```rust
/// Sync stream classification from config
pub struct ClassificationSync;

impl ClassificationSync {
    /// Generate UPSERT for stream classification
    pub fn generate_classification_upsert(
        &self,
        stream_id: &str,
        stream_type: &str,
        correlation_role: Option<&str>
    ) -> String;
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_classification_upsert() {
        let sync = ClassificationSync::new();

        let sql = sync.generate_classification_upsert(
            "air-quality",
            "observation",
            Some("effect")
        );

        assert!(sql.contains("INSERT INTO data_dictionary.stream_classification"));
        assert!(sql.contains("ON CONFLICT"));
        assert!(sql.contains("'observation'"));
        assert!(sql.contains("'effect'"));
    }
}
```

---

## 10. Completeness Verification

### 10.1 Verification Queries

Run after each deploy to verify data dictionary completeness:

```sql
-- Check all Gold tables are documented
SELECT
    'MISSING: ' || v.view_name as issue
FROM timescaledb_information.continuous_aggregates v
LEFT JOIN data_dictionary.gold_tables gt
    ON v.view_name = gt.table_name
WHERE v.view_schema = 'gold'
    AND gt.table_name IS NULL;

-- Check all streams are classified
SELECT
    'UNCLASSIFIED: ' || s.stream_id as issue
FROM data_dictionary.streams s
LEFT JOIN data_dictionary.stream_classification sc
    ON s.stream_id = sc.stream_id
WHERE sc.stream_id IS NULL;

-- Check Gold columns match actual columns
SELECT
    'MISSING COLUMN: ' || c.table_name || '.' || c.column_name as issue
FROM information_schema.columns c
LEFT JOIN data_dictionary.gold_columns gc
    ON c.table_name = gc.table_name
    AND c.column_name = gc.column_name
WHERE c.table_schema = 'gold'
    AND gc.column_name IS NULL
    AND c.column_name NOT IN ('bucket', 'ndp_id');  -- Known system columns
```

### 10.2 Completeness Metrics

| Metric | Target | Query |
|--------|--------|-------|
| Gold table coverage | 100% | COUNT documented / COUNT actual |
| Gold column coverage | 100% | COUNT documented / COUNT actual |
| Stream classification coverage | 100% | COUNT classified / COUNT streams |
| Feature type assignment | 100% | COUNT with feature_type / COUNT columns |

---

## 11. References

- [SCOPE.md](../../SCOPE.md) - Feature v11-010 definition
- [DECISIONS.md](../../architecture/DECISIONS.md) - Decision 3: Data Dictionary Extension
- [data-dictionary-patterns.md](../../architecture/data-dictionary-patterns.md) - V1.0 patterns

---

*Specification created: 2026-02-04*
