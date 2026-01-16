# DP-009: Extend Data Dictionary to Silver Layer

**Feature ID**: dp-009
**Title**: Config-Driven Silver Layer Data Dictionary
**Status**: Scope Definition
**Created**: 2026-01-16
**Depends On**: dp-002 (Bronze Data Dictionary), dp-006 (Silver Layer)

---

## Executive Summary

Extend the existing PostgreSQL data dictionary to include Silver layer metadata. The Bronze data dictionary (dp-002) established config-driven schema documentation for streams, fields, and entity_schemas. With the Silver layer now operational (dp-006), we need to document Silver tables, columns, types, units, and DQ rules in the same queryable data dictionary - maintaining the config-driven approach.

---

## Context

### Current State

**Bronze Data Dictionary (dp-002)** - Operational:
```
data_dictionary.streams              → Stream metadata
data_dictionary.fields               → Bronze Parquet columns
data_dictionary.sources              → Data source configs
data_dictionary.entity_schemas       → Logical data dictionary entries
data_dictionary.entity_schema_attributes → Attribute definitions
data_dictionary.sync_status          → Sync tracking
```

**Silver Layer (dp-006)** - Operational:
```
silver.air_quality_observations      → Indoor AQ from AirGradient
silver.weather_observations          → NWS + OWM merged weather
silver.weather_forecasts             → NWS forecasts with lead_time
silver.outdoor_air_quality           → OWM outdoor AQ
```

### Gap

| Aspect | Bronze | Silver |
|--------|--------|--------|
| Table metadata | ✅ `streams` | ❌ Not tracked |
| Column definitions | ✅ `fields` | ❌ Not tracked |
| Types & units | ✅ In config | ❌ Only in SQL/comments |
| DQ rules | ✅ `entity_schema_attributes.range_*` | ❌ Only in `silver_etl` config |
| Lineage | N/A | ❌ No Bronze→Silver mapping |

**Result**: Users can query "what Bronze fields exist?" but NOT "what Silver columns exist?", "what are the units?", or "where does this column come from?"

---

## Objectives

1. **Document Silver tables** in data dictionary with metadata (description, grain, use case)
2. **Document Silver columns** with type, unit, description, nullable
3. **Track Bronze→Silver lineage** showing source field(s) for each Silver column
4. **Expose DQ rules** applied during ETL transformation
5. **Maintain config-driven approach** - Silver metadata derived from `silver_etl` YAML config
6. **Enable unified queries** across Bronze and Silver layers

---

## Scope

### In Scope

#### 1. New Data Dictionary Tables

Add Silver-specific tables to `data_dictionary` schema:

| Table | Purpose |
|-------|---------|
| `silver_tables` | Silver table metadata (name, description, grain, hypertable config) |
| `silver_columns` | Column definitions (name, type, unit, description, nullable) |
| `silver_lineage` | Bronze→Silver field mappings (source_path → target_column) |
| `silver_dq_rules` | DQ rules applied per column (rule type, params, action) |

#### 2. Schema Design

```sql
-- Silver table metadata
CREATE TABLE data_dictionary.silver_tables (
    table_name          TEXT PRIMARY KEY,
    schema_name         TEXT NOT NULL DEFAULT 'silver',
    description         TEXT,
    grain               TEXT,           -- 'one row per sensor reading'
    source_streams      TEXT[],         -- ['air-quality']
    hypertable_column   TEXT,           -- 'observation_time'
    chunk_interval      INTERVAL,
    created_at          TIMESTAMPTZ DEFAULT NOW(),
    updated_at          TIMESTAMPTZ DEFAULT NOW()
);

-- Silver column definitions
CREATE TABLE data_dictionary.silver_columns (
    id                  SERIAL PRIMARY KEY,
    table_name          TEXT NOT NULL REFERENCES data_dictionary.silver_tables(table_name),
    column_name         TEXT NOT NULL,
    data_type           TEXT NOT NULL,  -- 'DOUBLE PRECISION', 'TIMESTAMPTZ', etc.
    unit                TEXT,           -- 'ug/m3', 'Celsius', '%'
    description         TEXT,
    nullable            BOOLEAN DEFAULT true,
    is_primary_key      BOOLEAN DEFAULT false,
    sort_order          INTEGER DEFAULT 0,
    UNIQUE(table_name, column_name)
);

-- Bronze → Silver lineage
CREATE TABLE data_dictionary.silver_lineage (
    id                  SERIAL PRIMARY KEY,
    silver_table        TEXT NOT NULL,
    silver_column       TEXT NOT NULL,
    source_stream       TEXT NOT NULL,
    source_path         TEXT NOT NULL,  -- 'raw_payload.pm02'
    transformation      TEXT,           -- 'direct', 'kelvin_to_celsius', etc.
    UNIQUE(silver_table, silver_column, source_stream)
);

-- DQ rules per Silver column
CREATE TABLE data_dictionary.silver_dq_rules (
    id                  SERIAL PRIMARY KEY,
    silver_table        TEXT NOT NULL,
    silver_column       TEXT NOT NULL,
    rule_name           TEXT NOT NULL,  -- 'range_check', 'not_null', etc.
    rule_params         JSONB,          -- {"min": 0, "max": 1000}
    action              TEXT NOT NULL,  -- 'flag', 'reject', 'clamp'
    UNIQUE(silver_table, silver_column, rule_name)
);
```

#### 3. Unified Views

Create views that span Bronze and Silver:

```sql
-- Complete data dictionary view (Bronze + Silver)
CREATE VIEW data_dictionary.v_complete_dictionary AS
SELECT
    'bronze' AS layer,
    stream_id AS entity,
    field_name AS column_name,
    field_type AS data_type,
    unit,
    description
FROM data_dictionary.fields
UNION ALL
SELECT
    'silver' AS layer,
    table_name AS entity,
    column_name,
    data_type,
    unit,
    description
FROM data_dictionary.silver_columns;

-- Lineage view: Bronze field → Silver column
CREATE VIEW data_dictionary.v_lineage AS
SELECT
    l.source_stream,
    l.source_path AS bronze_field,
    l.silver_table,
    l.silver_column,
    l.transformation,
    sc.data_type AS silver_type,
    sc.unit AS silver_unit
FROM data_dictionary.silver_lineage l
JOIN data_dictionary.silver_columns sc
    ON l.silver_table = sc.table_name
   AND l.silver_column = sc.column_name;
```

#### 4. Config Schema Extension

Extend `silver_etl` config to include documentation metadata:

```yaml
silver_etl:
  enabled: true
  target_table: silver.air_quality_observations
  description: "Indoor air quality measurements from AirGradient sensors"
  grain: "One row per sensor reading (~1 minute intervals)"

  field_mappings:
    - source_path: raw_payload.pm02
      target_column: pm25
      type: double_precision
      unit: "ug/m3"
      description: "PM2.5 particulate matter concentration"
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 1000.0
          action: flag
```

**New fields** (backward compatible - optional):
- `description` on `silver_etl` block
- `grain` on `silver_etl` block
- `unit` on field mappings
- `description` on field mappings

#### 5. Sync Mechanism Extension

Extend `deploy.sh sync-dictionary` to include Silver metadata:

1. Parse `silver_etl` sections from stream configs
2. Populate `silver_tables` from `target_table` + metadata
3. Populate `silver_columns` from `field_mappings`
4. Populate `silver_lineage` from source_path → target_column
5. Populate `silver_dq_rules` from `dq_rules` arrays

#### 6. Migration Script

Create migration `003_silver_data_dictionary.sql`:
- Add new tables to `data_dictionary` schema
- Create unified views
- Add indexes for common query patterns
- Maintain idempotency (IF NOT EXISTS)

#### 7. Update Existing Stream Configs

Add documentation metadata to the 4 active Silver streams:
- `air-quality` → `silver.air_quality_observations`
- `outdoor-weather` + `nws-observations` → `silver.weather_observations`
- `nws-forecast-hourly` + `nws-gridpoints-forecast` → `silver.weather_forecasts`
- `outdoor-air-quality` → `silver.outdoor_air_quality`

---

### Out of Scope

| Item | Reason | Target |
|------|--------|--------|
| Gold layer dictionary | Not yet implemented | dp-010+ |
| Automated schema drift detection | Enhancement | Future |
| Grafana data dictionary dashboard | Separate concern | dp-010+ |
| Version history for schema changes | Over-engineering | Future |
| Cross-table relationship tracking | Not needed yet | Future |

---

## Success Criteria

| Criterion | Validation |
|-----------|------------|
| Silver tables queryable | `SELECT * FROM data_dictionary.silver_tables` returns 4 rows |
| Silver columns documented | Each table has all columns with types and units |
| Lineage traceable | Can query "where does pm25 come from?" |
| DQ rules exposed | Can query "what rules apply to temperature_c?" |
| Unified view works | `v_complete_dictionary` shows Bronze + Silver |
| Config-driven | Adding new stream config populates dictionary |
| Sync idempotent | Running sync twice produces same result |

---

## Technical Approach

### Data Flow

```
Stream Config (YAML)          PostgreSQL Data Dictionary
┌─────────────────────┐       ┌─────────────────────────┐
│ silver_etl:         │       │ silver_tables           │
│   target_table: ... │──────▶│   table_name            │
│   description: ...  │       │   description           │
│   grain: ...        │       │   grain                 │
│                     │       │   source_streams        │
│   field_mappings:   │       └─────────────────────────┘
│     - source_path   │              │
│       target_column │──────▶┌──────┴──────────────────┐
│       type          │       │ silver_columns          │
│       unit          │       │   column_name           │
│       description   │       │   data_type             │
│       dq_rules:     │       │   unit                  │
│         - rule      │       │   description           │
│           min/max   │       └─────────────────────────┘
│           action    │              │
└─────────────────────┘       ┌──────┴──────────────────┐
                              │ silver_lineage          │
                              │   source_stream         │
                              │   source_path           │
                              │   silver_table          │
                              │   silver_column         │
                              └─────────────────────────┘
                                     │
                              ┌──────┴──────────────────┐
                              │ silver_dq_rules         │
                              │   rule_name             │
                              │   rule_params           │
                              │   action                │
                              └─────────────────────────┘
```

### Query Examples

After implementation, these queries will work:

```sql
-- "What columns are in the air quality Silver table?"
SELECT column_name, data_type, unit, description
FROM data_dictionary.silver_columns
WHERE table_name = 'air_quality_observations';

-- "Where does pm25 come from?"
SELECT source_stream, source_path, transformation
FROM data_dictionary.silver_lineage
WHERE silver_column = 'pm25';

-- "What DQ rules apply to temperature columns?"
SELECT silver_table, silver_column, rule_name, rule_params, action
FROM data_dictionary.silver_dq_rules
WHERE silver_column LIKE '%temperature%';

-- "Show me the complete data dictionary"
SELECT layer, entity, column_name, data_type, unit
FROM data_dictionary.v_complete_dictionary
ORDER BY layer, entity, column_name;
```

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| dp-002 Bronze Data Dictionary | ✅ Complete | Schema exists, sync works |
| dp-006 Silver Layer | ✅ Complete | 4 tables operational |
| `silver_etl` config sections | ✅ Exist | Already have field_mappings |
| Deploy script sync command | ✅ Exists | Extend for Silver |

---

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Config schema changes break existing | Low | Medium | New fields optional, backward compatible |
| Sync complexity increases | Medium | Low | Modular sync functions |
| Lineage tracking incomplete | Low | Low | Document known gaps |

---

## Deliverables

| Deliverable | Description |
|-------------|-------------|
| `003_silver_data_dictionary.sql` | Migration adding Silver tables/views |
| Updated stream configs | Add `unit`, `description` to field_mappings |
| Extended sync script | Parse Silver metadata, populate tables |
| Updated procedure docs | Document Silver data dictionary usage |

---

## References

- [dp-002 SCOPE](../dp-002/SCOPE.md) - Bronze Data Dictionary
- [dp-006 SCOPE](../dp-006/SCOPE.md) - Silver Layer Implementation
- [01-create-data-dictionary.sql](../../../deploy/pi/init-scripts/01-create-data-dictionary.sql) - Current schema
- [03-data-dictionary.md](../../../research/agenticdataplatform/silver/03-data-dictionary.md) - Silver schema research

---

*Scope defined: 2026-01-16*
