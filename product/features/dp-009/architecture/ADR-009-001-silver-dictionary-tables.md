# ADR-009-001: Silver Layer Data Dictionary Tables

**Feature**: dp-009 (Config-Driven Silver Layer Data Dictionary)
**Status**: Proposed
**Date**: 2026-01-16
**Author**: NDP Architect
**Depends On**: ADR-001 (dp-002 TimescaleDB Schema), ADR-006-003 (dp-006 Schema Naming)

---

## Context

The Bronze Data Dictionary (dp-002) successfully established config-driven schema documentation in PostgreSQL. The dictionary stores metadata about:

- **Streams**: High-level stream configuration (retention, partitioning)
- **Fields**: Bronze Parquet column definitions
- **Sources**: Data source configurations (MQTT, HTTP)
- **Entity Schemas**: Logical data dictionary for sensor types

With the Silver layer now operational (dp-006), users can query "what Bronze fields exist?" but cannot answer:

1. "What columns are in `silver.air_quality_observations`?"
2. "What is the unit for `temperature_c`?"
3. "Where does `pm25` come from?" (lineage)
4. "What DQ rules apply to `humidity_pct`?"

### Problem Statement

Silver layer metadata exists only in:
- SQL DDL files (column types)
- Code comments (units, descriptions)
- YAML configs (`silver_etl.field_mappings` has some metadata)

This scattered documentation prevents:
- Grafana dashboards from dynamically discovering Silver columns
- Self-service data discovery for analysts
- Automated lineage documentation
- DQ rule visibility

### Requirements

1. Document Silver tables with metadata (description, grain, hypertable config)
2. Document Silver columns with type, unit, description, nullable
3. Track Bronze-to-Silver lineage (source_path → target_column)
4. Expose DQ rules applied during ETL
5. Maintain config-driven approach (YAML as source of truth)
6. Enable unified queries across Bronze and Silver layers

---

## Decision

**Extend the `data_dictionary` schema with four new tables for Silver layer metadata.**

### Table Design

```
data_dictionary schema (existing)       data_dictionary schema (new)
┌─────────────────────────┐            ┌─────────────────────────┐
│ streams                 │            │ silver_tables           │
│ fields                  │            │ silver_columns          │
│ sources                 │            │ silver_lineage          │
│ entity_schemas          │            │ silver_dq_rules         │
│ entity_schema_attributes│            └─────────────────────────┘
│ sync_status             │
└─────────────────────────┘
```

### New Tables

#### 1. `silver_tables` - Table-Level Metadata

Stores metadata for each Silver table, mirroring `streams` for Bronze.

| Column | Type | Purpose |
|--------|------|---------|
| `table_name` | TEXT PK | Fully-qualified table name (`silver.weather_observations`) |
| `schema_name` | TEXT | PostgreSQL schema (`silver`) |
| `description` | TEXT | Human-readable table purpose |
| `grain` | TEXT | What one row represents ("One row per sensor reading") |
| `source_streams` | TEXT[] | Bronze streams that feed this table |
| `hypertable_column` | TEXT | Time column for hypertable (`observation_time`) |
| `chunk_interval` | INTERVAL | TimescaleDB chunk size |
| `created_at` | TIMESTAMPTZ | Record creation timestamp |
| `updated_at` | TIMESTAMPTZ | Last modification timestamp |

#### 2. `silver_columns` - Column-Level Metadata

Stores column definitions with units and descriptions.

| Column | Type | Purpose |
|--------|------|---------|
| `id` | SERIAL PK | Surrogate key |
| `table_name` | TEXT FK | Reference to silver_tables |
| `column_name` | TEXT | Column name (`temperature_c`) |
| `data_type` | TEXT | PostgreSQL type (`DOUBLE PRECISION`) |
| `unit` | TEXT | Measurement unit (`celsius`, `ug/m3`) |
| `description` | TEXT | Column purpose |
| `nullable` | BOOLEAN | Whether NULL is allowed |
| `is_primary_key` | BOOLEAN | Part of PK constraint |
| `sort_order` | INTEGER | Display ordering |

Unique constraint on `(table_name, column_name)`.

#### 3. `silver_lineage` - Bronze-to-Silver Mapping

Tracks where each Silver column's data originates.

| Column | Type | Purpose |
|--------|------|---------|
| `id` | SERIAL PK | Surrogate key |
| `silver_table` | TEXT | Target Silver table |
| `silver_column` | TEXT | Target column name |
| `source_stream` | TEXT | Source Bronze stream ID |
| `source_path` | TEXT | JSON path in raw_payload (`raw_payload.pm02`) |
| `transformation` | TEXT | Transform applied (`direct`, `kelvin_to_celsius`) |

Unique constraint on `(silver_table, silver_column, source_stream)`.

#### 4. `silver_dq_rules` - DQ Rules Per Column

Exposes data quality rules applied during ETL.

| Column | Type | Purpose |
|--------|------|---------|
| `id` | SERIAL PK | Surrogate key |
| `silver_table` | TEXT | Target Silver table |
| `silver_column` | TEXT | Target column (or NULL for cross-field) |
| `rule_name` | TEXT | Rule type (`range_check`, `null_check`) |
| `rule_params` | JSONB | Rule parameters (`{"min": 0, "max": 100}`) |
| `action` | TEXT | Action on violation (`flag`, `reject`, `clamp`) |

Unique constraint on `(silver_table, silver_column, rule_name)`.

### Relationship Diagram

```
data_dictionary.silver_tables
        │
        │ table_name (FK)
        ▼
data_dictionary.silver_columns ◄──── data_dictionary.silver_lineage
        │                                      │
        │ (table, column)                      │ (silver_table, silver_column)
        ▼                                      │
data_dictionary.silver_dq_rules ◄──────────────┘
```

### Index Strategy

```sql
-- Primary lookups
CREATE INDEX idx_silver_columns_table ON data_dictionary.silver_columns(table_name);
CREATE INDEX idx_silver_lineage_table ON data_dictionary.silver_lineage(silver_table);
CREATE INDEX idx_silver_lineage_stream ON data_dictionary.silver_lineage(source_stream);
CREATE INDEX idx_silver_dq_rules_table ON data_dictionary.silver_dq_rules(silver_table);

-- Column lookups for lineage queries
CREATE INDEX idx_silver_lineage_column ON data_dictionary.silver_lineage(silver_column);
CREATE INDEX idx_silver_dq_rules_column ON data_dictionary.silver_dq_rules(silver_column);
```

---

## Rationale

### Why Four Tables Instead of One

**Considered Alternative**: Single `silver_metadata` table with JSONB blob.

```sql
-- REJECTED: Single table approach
CREATE TABLE data_dictionary.silver_metadata (
    table_name TEXT PRIMARY KEY,
    columns JSONB,      -- [{name, type, unit, ...}]
    lineage JSONB,      -- [{source, target, ...}]
    dq_rules JSONB      -- [{rule, params, ...}]
);
```

**Rejected because**:
1. Cannot efficiently index individual columns for queries like "find all columns with unit 'celsius'"
2. No referential integrity between columns and lineage
3. Complex updates (must rewrite entire JSONB for any change)
4. Grafana Table panels work better with normalized data

**Normalized approach benefits**:
1. Efficient column-level queries with proper indexes
2. Joins enable lineage tracing
3. Partial updates without document replacement
4. Standard SQL compatible with all tools

### Why Extend data_dictionary Schema

**Considered Alternative**: Create separate `silver_dictionary` schema.

**Rejected because**:
1. Adds unnecessary schema complexity
2. Cross-schema joins needed for unified views
3. Bronze dictionary already in `data_dictionary` - logical to keep together
4. Single sync mechanism can handle both

### Why TEXT Primary Key for silver_tables

**Considered Alternative**: Surrogate key with unique constraint.

```sql
-- REJECTED: Surrogate key
CREATE TABLE silver_tables (
    id SERIAL PRIMARY KEY,
    table_name TEXT UNIQUE NOT NULL
);
```

**Natural key chosen because**:
1. `table_name` is inherently unique and stable
2. Eliminates join for FK lookups (can use table_name directly)
3. Matches Bronze `streams` pattern (stream_id as PK)
4. More readable in queries and views

### Why Store Fully-Qualified Table Name

Store `silver.air_quality_observations` not just `air_quality_observations`.

**Rationale**:
1. Future-proofs for multi-schema scenarios (per ADR-006-003 migration path)
2. Unambiguous reference in unified views
3. Matches `target_table` in YAML config (single source of truth)

---

## Consequences

### Positive

1. **Unified Discovery**: Single query can show Bronze + Silver metadata
2. **Lineage Visibility**: "Where does pm25 come from?" is a simple JOIN
3. **DQ Transparency**: Rules visible without reading YAML
4. **Config-Driven**: Sync from YAML maintains single source of truth
5. **Grafana Compatible**: Normalized tables work with Table panels
6. **Consistent Pattern**: Mirrors Bronze dictionary design

### Negative

1. **Migration Required**: New tables need DDL migration (003_silver_data_dictionary.sql)
2. **Sync Complexity**: Must parse `silver_etl` config sections
3. **Data Duplication**: Same info in YAML and PostgreSQL (sync manages consistency)

### Neutral

1. **No Breaking Changes**: Adds to existing schema, doesn't modify it
2. **Optional Adoption**: Dashboards can use when ready

---

## Alternatives Considered

### Alternative 1: Extend Existing Tables

Add Silver columns to existing `fields` table with layer discriminator.

```sql
-- REJECTED: Extend fields table
ALTER TABLE data_dictionary.fields
ADD COLUMN layer TEXT DEFAULT 'bronze'; -- 'bronze' or 'silver'
```

**Rejected because**:
1. Bronze fields have different attributes (validation_min/max vs dq_rules)
2. Silver has lineage, Bronze doesn't
3. Conflates different concepts (source fields vs derived columns)
4. Query complexity increases for layer filtering

### Alternative 2: Use PostgreSQL Information Schema

Derive Silver metadata from `information_schema.columns`.

```sql
-- REJECTED: Query information_schema
SELECT column_name, data_type
FROM information_schema.columns
WHERE table_schema = 'silver';
```

**Rejected because**:
1. No units, descriptions, or business metadata
2. No lineage information
3. No DQ rule visibility
4. Not config-driven (requires runtime introspection)

### Alternative 3: Metadata in Table Comments

Store metadata as PostgreSQL comments.

```sql
-- REJECTED: Comments-based approach
COMMENT ON COLUMN silver.weather_observations.temperature_c IS
  'unit=celsius|description=Ambient temperature';
```

**Rejected because**:
1. Limited queryability (no structured access)
2. Cannot track lineage
3. DQ rules don't fit comment format
4. Not config-driven

---

## Implementation Notes

### Migration Script

Place in `deploy/pi/init-scripts/003_silver_data_dictionary.sql`:

1. Create new tables with IF NOT EXISTS
2. Add indexes
3. Create unified views
4. Idempotent (re-runnable)

### Sync Integration

Extend `sync_to_data_dictionary()` in deploy.sh:

1. Parse `silver_etl.target_table` → `silver_tables`
2. Parse `silver_etl.field_mappings[]` → `silver_columns` + `silver_lineage`
3. Parse `silver_etl.field_mappings[].dq_rules[]` → `silver_dq_rules`
4. Parse `silver_etl.dq_rules[]` (cross-field) → `silver_dq_rules` with NULL column

### Data Flow

```
YAML Config                         PostgreSQL
┌────────────────────┐             ┌──────────────────────┐
│ silver_etl:        │             │ silver_tables        │
│   target_table: ...│────────────▶│   table_name         │
│   description: ... │             │   description        │
│   grain: ...       │             │   grain              │
│                    │             │   source_streams     │
│   field_mappings:  │             └──────────────────────┘
│     - source_path  │                      │
│       target_column│────────────▶┌────────┴─────────────┐
│       type         │             │ silver_columns       │
│       unit         │             │   column_name        │
│       description  │             │   data_type          │
│       dq_rules:    │             │   unit               │
│         - rule     │             │   description        │
│           min/max  │             └──────────────────────┘
│           action   │                      │
└────────────────────┘             ┌────────┴─────────────┐
                                   │ silver_lineage       │
                                   │   source_path        │
                                   │   transformation     │
                                   └──────────────────────┘
                                            │
                                   ┌────────┴─────────────┐
                                   │ silver_dq_rules      │
                                   │   rule_name          │
                                   │   rule_params        │
                                   │   action             │
                                   └──────────────────────┘
```

---

## Related Decisions

- **ADR-001 (dp-002)**: TimescaleDB Schema Design - Established Bronze dictionary pattern
- **ADR-003 (dp-002)**: Sync Mechanism - Shell script approach for YAML → PostgreSQL
- **ADR-006-003 (dp-006)**: Schema Naming Convention - `silver.*` flat schema
- **ADR-009-002**: Config Schema Extension - YAML changes for metadata
- **ADR-009-003**: Sync Mechanism Extension - Parsing silver_etl

---

## References

1. [dp-002 SCOPE](../../dp-002/SCOPE.md) - Bronze Data Dictionary scope
2. [dp-006 SCOPE](../../dp-006/SCOPE.md) - Silver Layer Implementation scope
3. [01-create-data-dictionary.sql](../../../../deploy/pi/init-scripts/01-create-data-dictionary.sql) - Current schema
4. [air-quality config.yaml](../../../../config/base/streams/air-quality/config.yaml) - Example silver_etl config

---

**Last Updated**: 2026-01-16
**Next Review**: After migration implementation and sync testing
