# SQL Patterns for Dimension Tables

**Feature**: dp-013 - CSV Source Type & Dimension Tables
**Layer**: Silver (TimescaleDB)
**Status**: Draft

---

## Overview

This document defines SQL patterns for dimension tables in the NDP Silver layer. Dimension tables hold reference data that enriches time-series observations. Unlike hypertables (which use TimescaleDB's time-series optimizations), dimension tables are regular PostgreSQL tables with standard relational semantics.

**Key Distinction**:
- **Hypertables**: Time-series data from streams (observations, forecasts)
- **Dimension Tables**: Reference/lookup data loaded from CSV configs

---

## 1. Dimension Table DDL

### entity_context Table

The primary dimension table for air-012, linking `ndp_id` identifiers to human-readable context.

```sql
-- Create silver schema if not exists
CREATE SCHEMA IF NOT EXISTS silver;

-- Entity context dimension table
CREATE TABLE IF NOT EXISTS silver.entity_context (
    -- Primary identifier (matches ndp_id in time-series tables)
    ndp_id TEXT PRIMARY KEY,

    -- Classification
    category TEXT NOT NULL,  -- 'door', 'window', 'sensor', etc.

    -- Human-readable name
    friendly_name TEXT,

    -- Hierarchical location path
    location_path TEXT,      -- '/home/living', '/home/office'

    -- Correlation reference (links to related ndp_id)
    correlates_with TEXT,    -- References another ndp_id

    -- Physical attributes
    orientation TEXT,        -- 'north', 'south', 'east', 'west'

    -- Audit columns
    loaded_at TIMESTAMPTZ DEFAULT NOW(),
    source_file TEXT         -- Track which CSV populated this row
);

-- Index for common query patterns
CREATE INDEX IF NOT EXISTS idx_entity_context_category
    ON silver.entity_context (category);

CREATE INDEX IF NOT EXISTS idx_entity_context_location
    ON silver.entity_context (location_path);

COMMENT ON TABLE silver.entity_context IS
    'Dimension table for entity metadata. Loaded from config/dimensions/entity_context.csv';
```

### Generic Dimension Table Template

```sql
CREATE TABLE IF NOT EXISTS silver.{dimension_name} (
    -- Primary key column(s)
    {pk_column} {pk_type} PRIMARY KEY,

    -- Required fields
    {required_field} {type} NOT NULL,

    -- Optional fields
    {optional_field} {type},

    -- Standard audit columns
    loaded_at TIMESTAMPTZ DEFAULT NOW(),
    source_file TEXT
);
```

---

## 2. Auto-Create Table from Schema

When a dimension config specifies a table that does not exist, the system generates DDL from the schema definition.

### Type Mapping

| Config `data_type` | PostgreSQL Type |
|--------------------|-----------------|
| `text` | `TEXT` |
| `int` | `INTEGER` |
| `bigint` | `BIGINT` |
| `float` | `DOUBLE PRECISION` |
| `bool` | `BOOLEAN` |
| `timestamp` | `TIMESTAMPTZ` |
| `date` | `DATE` |
| `json` | `JSONB` |

### SQL Generation Pattern

Given dimension config:
```yaml
dimension_id: entity-context
target:
  table: silver.entity_context
  primary_key: [ndp_id]
schema:
  fields:
    - name: ndp_id
      data_type: text
      required: true
    - name: category
      data_type: text
      required: true
    - name: friendly_name
      data_type: text
```

Generated DDL:
```sql
CREATE TABLE IF NOT EXISTS silver.entity_context (
    ndp_id TEXT NOT NULL,
    category TEXT NOT NULL,
    friendly_name TEXT,
    loaded_at TIMESTAMPTZ DEFAULT NOW(),
    source_file TEXT,
    PRIMARY KEY (ndp_id)
);
```

### Rust SQL Generation

```rust
/// Generate CREATE TABLE DDL from dimension schema
fn generate_create_table_ddl(config: &DimensionConfig) -> String {
    let mut columns = Vec::new();

    for field in &config.schema.fields {
        let sql_type = match field.data_type.as_str() {
            "text" => "TEXT",
            "int" => "INTEGER",
            "bigint" => "BIGINT",
            "float" => "DOUBLE PRECISION",
            "bool" => "BOOLEAN",
            "timestamp" => "TIMESTAMPTZ",
            "date" => "DATE",
            "json" => "JSONB",
            _ => "TEXT",  // Default fallback
        };

        let nullable = if field.required { " NOT NULL" } else { "" };
        columns.push(format!("    {} {}{}", field.name, sql_type, nullable));
    }

    // Add audit columns
    columns.push("    loaded_at TIMESTAMPTZ DEFAULT NOW()".to_string());
    columns.push("    source_file TEXT".to_string());

    // Primary key constraint
    let pk_cols = config.target.primary_key.join(", ");
    columns.push(format!("    PRIMARY KEY ({})", pk_cols));

    format!(
        "CREATE TABLE IF NOT EXISTS {} (\n{}\n);",
        config.target.table,
        columns.join(",\n")
    )
}
```

---

## 3. Load Strategies SQL

### Strategy: Truncate and Load

Replaces all data atomically. Best for small dimension tables where full refresh is acceptable.

```sql
-- Truncate and Load (transactional)
BEGIN;

-- Clear existing data
DELETE FROM silver.entity_context;

-- Insert new data
INSERT INTO silver.entity_context (
    ndp_id,
    category,
    friendly_name,
    location_path,
    correlates_with,
    orientation,
    source_file
)
VALUES
    ('door_backslider', 'door', 'Back Door Slider', '/home/living', 'aq_airgradient_1', 'south', 'entity_context.csv'),
    ('door_officewindow', 'window', 'Office Window', '/home/office', 'aq_airgradient_1', 'east', 'entity_context.csv'),
    ('door_dinettewindow', 'window', 'Dinette Window', '/home/dining', 'aq_airgradient_1', 'west', 'entity_context.csv');

COMMIT;
```

**Advantages**:
- Simple to implement
- Guaranteed consistent state
- Removes orphaned records automatically

**Disadvantages**:
- Brief unavailability during reload
- Full table lock during transaction

### Strategy: Upsert (ON CONFLICT)

Merges new data with existing. Best for incremental updates or when you want to preserve existing records not in the current load.

```sql
-- Upsert with ON CONFLICT
INSERT INTO silver.entity_context (
    ndp_id,
    category,
    friendly_name,
    location_path,
    correlates_with,
    orientation,
    source_file
)
VALUES
    ('door_backslider', 'door', 'Back Door Slider', '/home/living', 'aq_airgradient_1', 'south', 'entity_context.csv')
ON CONFLICT (ndp_id) DO UPDATE SET
    category = EXCLUDED.category,
    friendly_name = EXCLUDED.friendly_name,
    location_path = EXCLUDED.location_path,
    correlates_with = EXCLUDED.correlates_with,
    orientation = EXCLUDED.orientation,
    loaded_at = NOW(),
    source_file = EXCLUDED.source_file;
```

**Advantages**:
- No downtime during reload
- Preserves records not in current load
- Row-level locking (minimal contention)

**Disadvantages**:
- Orphaned records remain (need separate cleanup)
- Slightly more complex SQL

### Batch Upsert Pattern

For loading multiple rows efficiently:

```sql
-- Batch upsert using VALUES list
INSERT INTO silver.entity_context (
    ndp_id, category, friendly_name, location_path, correlates_with, orientation, source_file
)
VALUES
    ($1, $2, $3, $4, $5, $6, $7),
    ($8, $9, $10, $11, $12, $13, $14),
    -- ... more rows
ON CONFLICT (ndp_id) DO UPDATE SET
    category = EXCLUDED.category,
    friendly_name = EXCLUDED.friendly_name,
    location_path = EXCLUDED.location_path,
    correlates_with = EXCLUDED.correlates_with,
    orientation = EXCLUDED.orientation,
    loaded_at = NOW(),
    source_file = EXCLUDED.source_file;
```

### Composite Primary Key Upsert

For dimension tables with multi-column primary keys:

```sql
-- Example: time-based dimension (e.g., holiday calendar)
CREATE TABLE IF NOT EXISTS silver.calendar_events (
    event_date DATE NOT NULL,
    event_type TEXT NOT NULL,
    event_name TEXT,
    is_holiday BOOLEAN DEFAULT FALSE,
    loaded_at TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (event_date, event_type)
);

-- Upsert with composite key
INSERT INTO silver.calendar_events (event_date, event_type, event_name, is_holiday)
VALUES ('2025-12-25', 'holiday', 'Christmas', TRUE)
ON CONFLICT (event_date, event_type) DO UPDATE SET
    event_name = EXCLUDED.event_name,
    is_holiday = EXCLUDED.is_holiday,
    loaded_at = NOW();
```

---

## 4. Gold View Pattern

Gold views join Silver hypertables (time-series) with dimension tables (reference data) to provide enriched, analysis-ready data.

### Basic Join Pattern

```sql
-- Gold view: Events enriched with entity context
CREATE OR REPLACE VIEW gold.events_with_context AS
SELECT
    e.observation_time,
    e.ndp_id,
    e.state,
    e.event_type,
    -- Dimension attributes
    c.category,
    c.friendly_name,
    c.location_path,
    c.correlates_with,
    c.orientation
FROM silver.state_events e
LEFT JOIN silver.entity_context c USING (ndp_id);

COMMENT ON VIEW gold.events_with_context IS
    'State events enriched with entity context. LEFT JOIN preserves events without context.';
```

### Air Quality with Location Context

```sql
-- Gold view: Air quality readings with location context
CREATE OR REPLACE VIEW gold.air_quality_with_location AS
SELECT
    o.observation_time,
    o.ndp_id,
    o.pm25,
    o.pm10,
    o.co2_ppm,
    o.temperature_c,
    o.humidity_pct,
    -- Location context
    c.friendly_name AS sensor_name,
    c.location_path,
    SPLIT_PART(c.location_path, '/', 2) AS zone,  -- e.g., 'home' from '/home/office'
    SPLIT_PART(c.location_path, '/', 3) AS room   -- e.g., 'office' from '/home/office'
FROM silver.air_quality_observations o
LEFT JOIN silver.entity_context c USING (ndp_id)
WHERE c.category = 'sensor' OR c.category IS NULL;
```

### Cross-Domain Correlation View

```sql
-- Gold view: Correlate door/window events with air quality
CREATE OR REPLACE VIEW gold.ventilation_impact AS
SELECT
    time_bucket('15 minutes', o.observation_time) AS bucket,
    o.ndp_id AS sensor_id,
    AVG(o.pm25) AS avg_pm25,
    AVG(o.co2_ppm) AS avg_co2,
    -- Count of open doors/windows in correlated entities
    COUNT(DISTINCT CASE
        WHEN e.state = 'on' AND ec.category IN ('door', 'window')
        THEN e.ndp_id
    END) AS open_vents_count
FROM silver.air_quality_observations o
LEFT JOIN silver.entity_context sc ON o.ndp_id = sc.ndp_id
LEFT JOIN silver.entity_context ec ON ec.correlates_with = o.ndp_id
LEFT JOIN silver.state_events e ON e.ndp_id = ec.ndp_id
    AND e.observation_time >= o.observation_time - INTERVAL '15 minutes'
    AND e.observation_time <= o.observation_time
GROUP BY bucket, o.ndp_id
ORDER BY bucket DESC;
```

### Materialized Gold View for Performance

For frequently-queried Gold views, consider materialization:

```sql
-- Materialized Gold view with refresh
CREATE MATERIALIZED VIEW gold.daily_sensor_summary AS
SELECT
    DATE_TRUNC('day', o.observation_time) AS day,
    o.ndp_id,
    c.friendly_name,
    c.location_path,
    COUNT(*) AS reading_count,
    AVG(o.pm25) AS avg_pm25,
    MAX(o.pm25) AS max_pm25,
    AVG(o.temperature_c) AS avg_temp
FROM silver.air_quality_observations o
LEFT JOIN silver.entity_context c USING (ndp_id)
GROUP BY DATE_TRUNC('day', o.observation_time), o.ndp_id, c.friendly_name, c.location_path;

-- Index for dashboard queries
CREATE INDEX ON gold.daily_sensor_summary (day DESC);
CREATE INDEX ON gold.daily_sensor_summary (ndp_id);

-- Refresh strategy (run daily via cron/systemd timer)
REFRESH MATERIALIZED VIEW gold.daily_sensor_summary;
```

---

## 5. Migration Strategy

Managing dimension table schema evolution requires coordination between config changes and database migrations.

### Adding Columns (Backwards Compatible)

Adding new columns is safe and does not require migration scripts.

**Step 1**: Update dimension config
```yaml
schema:
  fields:
    # ... existing fields
    - name: floor_number    # NEW FIELD
      data_type: int
```

**Step 2**: Apply schema change
```sql
-- Add column with NULL default (backwards compatible)
ALTER TABLE silver.entity_context
ADD COLUMN IF NOT EXISTS floor_number INTEGER;

-- Optional: Add comment
COMMENT ON COLUMN silver.entity_context.floor_number IS
    'Floor number for multi-story buildings. Added in dp-013.';
```

**Step 3**: Update CSV and reload
```bash
ndp dimension sync entity-context
```

### Removing Columns (Requires Coordination)

Removing columns requires coordination to avoid breaking dependent views/queries.

**Step 1**: Identify dependencies
```sql
-- Find views that reference the column
SELECT
    c.relname AS view_name,
    pg_get_viewdef(c.oid) AS view_definition
FROM pg_class c
JOIN pg_depend d ON c.oid = d.refobjid
JOIN pg_attribute a ON d.objid = a.attrelid AND d.objsubid = a.attnum
WHERE a.attname = 'column_to_remove'
  AND c.relkind = 'v';
```

**Step 2**: Update dependent views
```sql
-- Recreate views without the removed column
CREATE OR REPLACE VIEW gold.events_with_context AS
SELECT
    e.*,
    c.category,
    c.friendly_name,
    c.location_path
    -- removed: c.deprecated_column
FROM silver.state_events e
LEFT JOIN silver.entity_context c USING (ndp_id);
```

**Step 3**: Drop column
```sql
ALTER TABLE silver.entity_context DROP COLUMN deprecated_column;
```

**Step 4**: Update dimension config to remove field

### Changing Column Types (Migration Script)

Type changes require explicit migration with data conversion.

**Migration script pattern** (`migrations/20250129_entity_context_floor_to_text.sql`):
```sql
-- Migration: Change floor_number from INTEGER to TEXT
-- Reason: Support non-numeric floor identifiers (e.g., 'basement', 'mezzanine')

BEGIN;

-- Step 1: Add new column with target type
ALTER TABLE silver.entity_context
ADD COLUMN floor_name TEXT;

-- Step 2: Migrate data with transformation
UPDATE silver.entity_context
SET floor_name = CASE
    WHEN floor_number = 0 THEN 'ground'
    WHEN floor_number = -1 THEN 'basement'
    ELSE floor_number::TEXT
END;

-- Step 3: Drop old column
ALTER TABLE silver.entity_context DROP COLUMN floor_number;

-- Step 4: Rename new column (optional, if keeping same name)
ALTER TABLE silver.entity_context
RENAME COLUMN floor_name TO floor;

COMMIT;
```

### Renaming Columns

```sql
-- Simple rename (update config and dependent views)
ALTER TABLE silver.entity_context
RENAME COLUMN correlates_with TO related_sensor_id;
```

### Schema Evolution Best Practices

| Change Type | Risk Level | Approach |
|-------------|------------|----------|
| Add nullable column | Low | Direct ALTER TABLE |
| Add non-null column | Medium | Add as NULL, backfill, then set NOT NULL |
| Remove column | High | Identify deps, update views, then DROP |
| Change type | High | Migration script with data conversion |
| Rename column | Medium | Update config + views + ALTER |
| Add index | Low | CREATE INDEX CONCURRENTLY |
| Add constraint | Medium | Validate data first, then add |

---

## 6. Dry-Run Validation

Before loading, validate that CSV data matches the schema.

### Validation Query Pattern

```sql
-- Dry-run: Validate CSV data against schema
-- Use COPY ... TO STDOUT with validation

-- Check for NULL in required fields
SELECT
    line_number,
    ndp_id,
    category
FROM csv_staging
WHERE ndp_id IS NULL OR category IS NULL;

-- Check for duplicates on primary key
SELECT
    ndp_id,
    COUNT(*) AS occurrence_count
FROM csv_staging
GROUP BY ndp_id
HAVING COUNT(*) > 1;

-- Validate foreign key references (if applicable)
SELECT s.correlates_with
FROM csv_staging s
LEFT JOIN silver.entity_context c ON s.correlates_with = c.ndp_id
WHERE s.correlates_with IS NOT NULL AND c.ndp_id IS NULL;
```

### Rust Dry-Run Implementation

```rust
pub struct DryRunResult {
    pub valid: bool,
    pub row_count: usize,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<String>,
}

pub enum ValidationError {
    MissingRequired { row: usize, column: String },
    DuplicatePrimaryKey { row: usize, key: String },
    InvalidType { row: usize, column: String, value: String, expected: String },
    InvalidReference { row: usize, column: String, value: String },
}
```

---

## 7. Performance Considerations

### Dimension Table Size Guidelines

| Size | Rows | Recommendation |
|------|------|----------------|
| Small | < 1,000 | No special handling needed |
| Medium | 1,000 - 100,000 | Index foreign keys |
| Large | > 100,000 | Consider partitioning or denormalization |

### Index Strategy for Dimension Tables

```sql
-- Common query patterns need indexes
-- Filter by category
CREATE INDEX idx_entity_context_category ON silver.entity_context (category);

-- Filter by location hierarchy
CREATE INDEX idx_entity_context_location ON silver.entity_context (location_path);

-- Text search on friendly names
CREATE INDEX idx_entity_context_name_trgm ON silver.entity_context
    USING gin (friendly_name gin_trgm_ops);
```

### Join Performance

For large time-series tables joining to dimension tables:

```sql
-- Ensure dimension table primary key is indexed (automatic for PRIMARY KEY)
-- Consider hash join hints for large joins

-- Analyze tables after bulk loads
ANALYZE silver.entity_context;
ANALYZE silver.state_events;
```

---

## Related Documentation

- [SCOPE.md](../SCOPE.md) - Feature requirements
- [PLATFORM_ARCHITECTURE_OVERVIEW.md](/workspaces/neural-data-platform/docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md) - Overall architecture
- Pattern: `arch-data-lake-layers` - Bronze/Silver/Gold data flow
- Pattern: `data:upsert-idempotency` - UPSERT for idempotent writes
- Pattern: `analytics-silver-data-types` - PostgreSQL type selection guide

---

## Document History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2025-01-29 | Initial draft for dp-013 |
