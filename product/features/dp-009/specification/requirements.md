# DP-009: Silver Layer Data Dictionary - Requirements

**Feature**: dp-009 - Config-Driven Silver Layer Data Dictionary
**Version**: 1.0
**Status**: Draft
**Last Updated**: 2026-01-16

---

## 1. Overview

This document specifies the requirements for extending the existing PostgreSQL data dictionary to include Silver layer metadata. The goal is to enable users to query Silver table schemas, column definitions, Bronze-to-Silver lineage, and DQ rules through a unified, config-driven data dictionary.

---

## 2. Functional Requirements

### 2.1 Silver Table Metadata (FR-009-001)

**Requirement**: The system SHALL store metadata for each Silver layer table.

| Field | Description | Source |
|-------|-------------|--------|
| `table_name` | Silver table name (e.g., `air_quality_observations`) | `silver_etl.target_table` |
| `schema_name` | Database schema (always `silver`) | Derived |
| `description` | Human-readable description | `silver_etl.description` (new) |
| `grain` | Row granularity description | `silver_etl.grain` (new) |
| `source_streams` | Array of contributing Bronze streams | `stream_id` |
| `hypertable_column` | TimescaleDB time column | `silver_etl.timestamp.target_field` |
| `chunk_interval` | TimescaleDB chunk interval | Default or config |

**Acceptance Criteria**:
- AC-009-001-1: `SELECT COUNT(*) FROM data_dictionary.silver_tables` returns 4
- AC-009-001-2: Each row has non-null `table_name` and `schema_name`
- AC-009-001-3: `source_streams` array is populated for all tables

### 2.2 Silver Column Definitions (FR-009-002)

**Requirement**: The system SHALL store column-level metadata for each Silver table.

| Field | Description | Source |
|-------|-------------|--------|
| `table_name` | Parent table reference | FK to `silver_tables` |
| `column_name` | Column name | `silver_etl.field_mappings[].target_column` |
| `data_type` | PostgreSQL data type | `silver_etl.field_mappings[].type` |
| `unit` | Unit of measurement | `silver_etl.field_mappings[].unit` (new) |
| `description` | Column description | `silver_etl.field_mappings[].description` (new) |
| `nullable` | Nullable flag | `silver_etl.field_mappings[].nullable` |
| `is_primary_key` | Primary key indicator | Derived from `identity_fields` |
| `sort_order` | Display order | Array index |

**Acceptance Criteria**:
- AC-009-002-1: `air_quality_observations` has >= 7 columns documented
- AC-009-002-2: `weather_observations` has >= 10 columns documented
- AC-009-002-3: `weather_forecasts` has >= 8 columns documented
- AC-009-002-4: `outdoor_air_quality` has >= 5 columns documented
- AC-009-002-5: Each column has `data_type` populated
- AC-009-002-6: Measurement columns have `unit` populated

### 2.3 Bronze-to-Silver Lineage (FR-009-003)

**Requirement**: The system SHALL track the mapping from Bronze source fields to Silver target columns.

| Field | Description | Source |
|-------|-------------|--------|
| `silver_table` | Target Silver table | `silver_etl.target_table` |
| `silver_column` | Target column | `field_mappings[].target_column` |
| `source_stream` | Source Bronze stream | `stream_id` |
| `source_path` | JSONPath to source field | `field_mappings[].source_path` |
| `transformation` | Transformation applied | Derived from config |

**Acceptance Criteria**:
- AC-009-003-1: Query "where does pm25 come from?" returns `air-quality`, `raw_payload.pm02Compensated`
- AC-009-003-2: Query "where does temperature_c come from?" returns multiple sources (air-quality, outdoor-weather)
- AC-009-003-3: All Silver columns have at least one lineage record

### 2.4 DQ Rules Documentation (FR-009-004)

**Requirement**: The system SHALL expose DQ rules applied to each Silver column.

| Field | Description | Source |
|-------|-------------|--------|
| `silver_table` | Target Silver table | `silver_etl.target_table` |
| `silver_column` | Target column | `field_mappings[].target_column` |
| `rule_name` | Rule type | `dq_rules[].rule` |
| `rule_params` | Rule parameters as JSONB | `dq_rules[].*` (min, max, etc.) |
| `action` | Enforcement action | `dq_rules[].action` |

**Acceptance Criteria**:
- AC-009-004-1: Query "what rules apply to pm25?" returns `range_check` with min=0, max=1000
- AC-009-004-2: Query "what rules apply to temperature_c?" returns `range_check`
- AC-009-004-3: All rules have non-null `action`

### 2.5 Unified Dictionary View (FR-009-005)

**Requirement**: The system SHALL provide a unified view spanning Bronze and Silver layers.

```sql
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
```

**Acceptance Criteria**:
- AC-009-005-1: View includes rows with `layer = 'bronze'`
- AC-009-005-2: View includes rows with `layer = 'silver'`
- AC-009-005-3: Query `SELECT DISTINCT layer FROM v_complete_dictionary` returns 2 rows

### 2.6 Lineage View (FR-009-006)

**Requirement**: The system SHALL provide a view for lineage queries.

```sql
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

**Acceptance Criteria**:
- AC-009-006-1: `SELECT * FROM v_lineage WHERE silver_column = 'pm25'` returns result
- AC-009-006-2: View includes `silver_type` and `silver_unit` columns

### 2.7 Config-Driven Sync (FR-009-007)

**Requirement**: The system SHALL populate Silver dictionary tables from YAML config.

**Sync Process**:
1. Parse `silver_etl` sections from all stream configs in etcd
2. Extract `target_table`, `description`, `grain` for `silver_tables`
3. Extract `field_mappings[]` for `silver_columns` and `silver_lineage`
4. Extract `dq_rules[]` for `silver_dq_rules`
5. Upsert records (idempotent)

**Acceptance Criteria**:
- AC-009-007-1: Running `deploy.sh sync-dictionary` populates Silver tables
- AC-009-007-2: Running sync twice produces identical row counts
- AC-009-007-3: Adding new stream config and syncing adds new dictionary entries

### 2.8 Config Schema Extension (FR-009-008)

**Requirement**: The `silver_etl` config schema SHALL support optional documentation metadata.

**New optional fields** (backward compatible):

```yaml
silver_etl:
  description: "Human-readable table description"  # NEW
  grain: "One row per observation"                 # NEW

  field_mappings:
    - source_path: raw_payload.pm02
      target_column: pm25
      type: double_precision
      unit: "ug/m3"              # NEW
      description: "PM2.5 concentration"  # NEW
      nullable: false
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 1000.0
          action: flag
```

**Acceptance Criteria**:
- AC-009-008-1: Existing configs without new fields still sync successfully
- AC-009-008-2: Configs with new fields populate corresponding dictionary columns

---

## 3. Non-Functional Requirements

### 3.1 Performance (NFR-009-001)

| Requirement | Target |
|-------------|--------|
| Sync duration | < 30 seconds for all streams |
| Dictionary query latency | < 100ms for typical queries |
| View query latency | < 500ms for unified view |

### 3.2 Reliability (NFR-009-002)

| Requirement | Target |
|-------------|--------|
| Sync idempotency | Repeated syncs produce identical results |
| Transaction safety | Sync uses transactions with rollback on error |
| Error handling | Sync continues on individual stream errors, reports at end |

### 3.3 Maintainability (NFR-009-003)

| Requirement | Target |
|-------------|--------|
| Migration idempotency | `IF NOT EXISTS` for all DDL |
| Schema versioning | Migration number `03-` follows existing sequence |
| Documentation | SQL comments on all tables and columns |

### 3.4 Backward Compatibility (NFR-009-004)

| Requirement | Target |
|-------------|--------|
| Existing configs | Work without modification |
| Existing queries | Bronze dictionary queries unchanged |
| Existing sync | Bronze sync continues to work |

---

## 4. Data Flow

```
                         Stream Config (etcd)
                         ┌─────────────────────────────────────┐
                         │ silver_etl:                         │
                         │   target_table: silver.aq_obs       │
                         │   description: "AQ measurements"    │
                         │   grain: "Per-sensor reading"       │
                         │   field_mappings:                   │
                         │     - source_path: raw_payload.pm02 │
                         │       target_column: pm25           │
                         │       type: double_precision        │
                         │       unit: "ug/m3"                 │
                         │       dq_rules:                     │
                         │         - rule: range_check         │
                         │           min: 0, max: 1000         │
                         └──────────────┬──────────────────────┘
                                        │
                                        ▼
                              ┌─────────────────┐
                              │  sync-dictionary │
                              │    (deploy.sh)   │
                              └────────┬────────┘
                                       │
           ┌───────────────────────────┼───────────────────────────┐
           │                           │                           │
           ▼                           ▼                           ▼
┌─────────────────────┐   ┌─────────────────────┐   ┌─────────────────────┐
│   silver_tables     │   │   silver_columns    │   │   silver_lineage    │
├─────────────────────┤   ├─────────────────────┤   ├─────────────────────┤
│ table_name          │◄──│ table_name (FK)     │   │ silver_table        │
│ description         │   │ column_name         │   │ silver_column       │
│ grain               │   │ data_type           │   │ source_stream       │
│ source_streams[]    │   │ unit                │   │ source_path         │
│ hypertable_column   │   │ description         │   │ transformation      │
└─────────────────────┘   │ nullable            │   └─────────────────────┘
                          └─────────────────────┘
                                       │
                                       ▼
                          ┌─────────────────────┐
                          │   silver_dq_rules   │
                          ├─────────────────────┤
                          │ silver_table        │
                          │ silver_column       │
                          │ rule_name           │
                          │ rule_params (JSONB) │
                          │ action              │
                          └─────────────────────┘
```

---

## 5. User Stories

### US-009-001: Data Analyst Discovers Silver Schema

**As a** data analyst
**I want to** query the data dictionary for Silver table columns
**So that** I can understand what data is available for analysis

**Acceptance Test**:
```sql
SELECT column_name, data_type, unit, description
FROM data_dictionary.silver_columns
WHERE table_name = 'air_quality_observations'
ORDER BY sort_order;
```
Returns all AQ columns with types and units.

### US-009-002: Data Engineer Traces Lineage

**As a** data engineer
**I want to** trace where a Silver column comes from
**So that** I can debug data quality issues

**Acceptance Test**:
```sql
SELECT source_stream, source_path, transformation
FROM data_dictionary.v_lineage
WHERE silver_column = 'pm25';
```
Returns `air-quality`, `raw_payload.pm02Compensated`, transformation info.

### US-009-003: QA Engineer Reviews DQ Rules

**As a** QA engineer
**I want to** see what DQ rules apply to a column
**So that** I can verify data validation is correct

**Acceptance Test**:
```sql
SELECT rule_name, rule_params, action
FROM data_dictionary.silver_dq_rules
WHERE silver_column = 'temperature_c';
```
Returns `range_check`, `{"min": -40, "max": 85}`, `flag`.

### US-009-004: Platform Admin Views Complete Dictionary

**As a** platform administrator
**I want to** view all fields across Bronze and Silver
**So that** I can understand the full data landscape

**Acceptance Test**:
```sql
SELECT layer, entity, column_name, data_type, unit
FROM data_dictionary.v_complete_dictionary
ORDER BY layer, entity, column_name;
```
Returns rows from both Bronze and Silver layers.

### US-009-005: DevOps Engineer Syncs Dictionary

**As a** DevOps engineer
**I want to** sync the data dictionary from config
**So that** documentation stays current with schema changes

**Acceptance Test**:
```bash
./deploy/pi/deploy.sh sync-dictionary
# Verify no errors, check row counts
psql -c "SELECT COUNT(*) FROM data_dictionary.silver_tables"
# Returns 4
```

---

## 6. Success Criteria Mapping

| Success Criterion | Requirements | Acceptance Criteria |
|-------------------|--------------|---------------------|
| Silver tables queryable (4 rows) | FR-009-001 | AC-009-001-1 |
| Silver columns with types and units | FR-009-002 | AC-009-002-5, AC-009-002-6 |
| Lineage traceable (pm25 source) | FR-009-003 | AC-009-003-1 |
| DQ rules exposed (temperature_c rules) | FR-009-004 | AC-009-004-2 |
| Unified view works | FR-009-005 | AC-009-005-1, AC-009-005-2 |
| Config-driven | FR-009-007 | AC-009-007-1 |
| Sync idempotent | FR-009-007 | AC-009-007-2 |

---

## 7. Out of Scope

| Item | Reason |
|------|--------|
| Gold layer dictionary | Not yet implemented (dp-010+) |
| Automated schema drift detection | Enhancement for future |
| Grafana data dictionary dashboard | Separate feature |
| Version history for schema changes | Over-engineering for current needs |
| Cross-table relationship tracking | Not needed yet |

---

## 8. Dependencies

| Dependency | Type | Status |
|------------|------|--------|
| dp-002 Bronze Data Dictionary | Data | Complete |
| dp-006 Silver Layer | Data | Complete |
| `silver_etl` config sections | Config | Complete |
| `deploy.sh sync-dictionary` | Code | Exists (extend) |
| etcd config storage | Infrastructure | Complete |
| TimescaleDB | Infrastructure | Complete |

---

## 9. Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Config schema changes break existing | Low | Medium | New fields are optional |
| Sync complexity increases maintenance | Medium | Low | Modular sync functions |
| Lineage tracking incomplete for complex transforms | Low | Low | Document known gaps |

---

## 10. Glossary

| Term | Definition |
|------|------------|
| Bronze | Raw data layer (Parquet files) |
| Silver | Cleaned, typed data layer (TimescaleDB) |
| Lineage | Tracking data flow from source to target |
| DQ Rules | Data Quality rules for validation |
| Grain | Row-level granularity description |
| Hypertable | TimescaleDB time-partitioned table |

---

*Requirements documented: 2026-01-16*
*Author: ndp-scrum-master*
