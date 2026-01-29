# dp-013: CSV Source Type & Dimension Tables - Specification

**SPARC Phase**: Specification
**Version**: 1.0.0
**Last Updated**: 2026-01-29
**Status**: Draft

---

## Table of Contents

1. [Requirements Analysis](#1-requirements-analysis)
2. [Use Cases](#2-use-cases)
3. [Data Models](#3-data-models)
4. [Interface Specifications](#4-interface-specifications)
5. [Acceptance Criteria Matrix](#5-acceptance-criteria-matrix)
6. [Dependencies](#6-dependencies)
7. [Constraints](#7-constraints)
8. [Edge Cases & Error Handling](#8-edge-cases--error-handling)

---

## 1. Requirements Analysis

### 1.1 Functional Requirements

#### FR-1: CSV Source Type for Stream Configs

| ID | Requirement | Priority | Rationale |
|----|-------------|----------|-----------|
| FR-1.1 | System SHALL recognize `source.type: csv` in stream configuration files | High | Enables batch data import via existing config pattern |
| FR-1.2 | CSV adapter SHALL read files from path specified in `source.path` | High | File location must be configurable |
| FR-1.3 | CSV adapter SHALL extract timestamps using `timestamp_field` and `timestamp_format` | High | Required for temporal alignment in Bronze |
| FR-1.4 | CSV adapter SHALL map columns to entity_schemas using existing `field_mappings` pattern | High | Reuse existing schema mapping mechanism |
| FR-1.5 | CSV adapter SHALL write records to Bronze layer as Parquet (same format as HTTP/MQTT) | High | Unified Bronze format regardless of source |
| FR-1.6 | CSV adapter SHALL support configurable delimiters (default: comma) | Medium | Handle TSV and other formats |
| FR-1.7 | CSV adapter SHALL support file encoding configuration (default: UTF-8) | Medium | Handle legacy files |
| FR-1.8 | CSV adapter SHALL handle error rows per `on_error` config (skip/abort) | High | Graceful degradation |

#### FR-2: Dimension Table Configs

| ID | Requirement | Priority | Rationale |
|----|-------------|----------|-----------|
| FR-2.1 | System SHALL recognize dimension config files in `config/base/dimensions/*.yaml` | High | Config-driven dimension management |
| FR-2.2 | Dimension config SHALL specify `dimension_id`, `target`, `source`, `schema`, and `load` sections | High | Complete dimension specification |
| FR-2.3 | System SHALL validate dimension schema against CSV header columns | High | Catch misconfigurations early |
| FR-2.4 | System SHALL support `truncate_and_load` strategy (DELETE + INSERT in transaction) | High | Default dimension refresh strategy |
| FR-2.5 | System SHALL support `upsert` strategy (INSERT ON CONFLICT UPDATE) | High | Incremental dimension updates |
| FR-2.6 | System SHALL auto-create target table from schema if not exists | Medium | Reduce manual setup |
| FR-2.7 | Dimensions SHALL load directly to Silver (skip Bronze) | High | Dimensions are metadata, not measurements |

#### FR-3: CLI Commands for Dimension Management

| ID | Requirement | Priority | Rationale |
|----|-------------|----------|-----------|
| FR-3.1 | `ndp dimension list` SHALL display all configured dimensions with status | High | Discoverability |
| FR-3.2 | `ndp dimension sync <id>` SHALL load specific dimension from CSV to target table | High | Targeted refresh |
| FR-3.3 | `ndp dimension sync --all` SHALL load all enabled dimensions | High | Bulk refresh |
| FR-3.4 | `ndp dimension sync --dry-run` SHALL validate without executing | High | Safe validation |
| FR-3.5 | CLI SHALL output summary: rows parsed, loaded, errors | High | Operational visibility |
| FR-3.6 | CLI SHALL return exit code 0 on success, non-zero on failure | High | CI/CD integration |

#### FR-4: Integration with deploy.sh

| ID | Requirement | Priority | Rationale |
|----|-------------|----------|-----------|
| FR-4.1 | `deploy.sh sync` SHALL process dimension configs after stream configs | High | Single deployment command |
| FR-4.2 | Dimension sync SHALL wait for TimescaleDB readiness before execution | High | Service dependency |
| FR-4.3 | deploy.sh SHALL report dimension sync status in summary | Medium | Operational visibility |

#### FR-5: Error Handling

| ID | Requirement | Priority | Rationale |
|----|-------------|----------|-----------|
| FR-5.1 | Malformed CSV SHALL abort with parse error including line number | High | Debuggability |
| FR-5.2 | Missing required columns SHALL fail validation before load | High | Fail-fast |
| FR-5.3 | Type conversion failures SHALL be logged with row context | High | Debuggability |
| FR-5.4 | File not found SHALL produce clear error with full path | High | Troubleshooting |
| FR-5.5 | Empty file SHALL produce warning and no-op result | Medium | Graceful handling |

---

### 1.2 Non-Functional Requirements

#### NFR-1: CSV Parsing Performance

| ID | Requirement | Metric | Rationale |
|----|-------------|--------|-----------|
| NFR-1.1 | CSV parser SHALL process at least 100,000 rows per second for simple schemas | 100k rows/sec | Reasonable batch import speed |
| NFR-1.2 | Memory usage SHALL be bounded (streaming parser, not load-all-at-once) | O(batch_size) memory | Handle large files |
| NFR-1.3 | Parser SHALL support files up to 10GB | 10GB max file size | Historical backfill support |

#### NFR-2: Transaction Safety

| ID | Requirement | Metric | Rationale |
|----|-------------|--------|-----------|
| NFR-2.1 | `truncate_and_load` SHALL execute DELETE and INSERT in single transaction | ACID guarantee | Prevent partial state |
| NFR-2.2 | On failure, database state SHALL roll back to pre-operation state | Rollback on error | Data integrity |
| NFR-2.3 | Concurrent dimension syncs to same table SHALL be serialized | Serialized access | Prevent race conditions |

#### NFR-3: Idempotency

| ID | Requirement | Metric | Rationale |
|----|-------------|--------|-----------|
| NFR-3.1 | Repeated `dimension sync` with same CSV SHALL produce identical table state | Idempotent | Safe re-runs |
| NFR-3.2 | Repeated `stream ingest` with same CSV SHALL not create duplicate Bronze records | Idempotent (within dedup window) | Safe re-runs |

---

## 2. Use Cases

### UC-1: Import Historical Timeseries CSV to Bronze

**ID**: UC-1
**Title**: Import Historical Air Quality Readings from CSV
**Actor**: Data Engineer
**Preconditions**:
- Stream config exists with `source.type: csv`
- CSV file exists at configured path
- Bronze storage is accessible

**Flow**:
1. Data Engineer executes `ndp stream ingest historical-aq`
2. System loads stream config from etcd/config file
3. System validates CSV file exists and is readable
4. System validates CSV header matches expected schema
5. System parses CSV rows, extracting timestamp and metric fields
6. System batches records and writes to Bronze as Parquet files
7. System reports: rows processed, written, errors skipped
8. Normal Silver ETL picks up Bronze data in next run

**Postconditions**:
- Bronze layer contains new Parquet files with ingested data
- Each row has proper `ndp_id`, `timestamp`, and `raw_payload`
- Ingestion metadata recorded (source: csv, file path, ingestion_time)

**Alternative Flows**:
- **A1**: CSV file not found
  - System reports error with full path
  - Exit code non-zero
  - No data written
- **A2**: Header mismatch
  - System reports missing/extra columns
  - Exit code non-zero
  - No data written
- **A3**: Row parse error with `on_error: skip`
  - System logs error with line number
  - System continues processing remaining rows
  - Summary includes skipped count
- **A4**: Row parse error with `on_error: abort`
  - System logs error with line number
  - System aborts immediately
  - Partial data may be written (up to last complete batch)

**Success Metrics**:
- Rows written equals rows in CSV minus any skipped
- Bronze Parquet schema matches stream schema
- Timestamps correctly parsed per format config

---

### UC-2: Load Entity Context Dimension Table

**ID**: UC-2
**Title**: Load Entity Context Reference Data
**Actor**: Data Engineer / Deploy Script
**Preconditions**:
- Dimension config exists in `config/base/dimensions/`
- CSV file exists at configured path
- TimescaleDB Silver schema accessible
- Target table exists OR auto-create enabled

**Flow**:
1. System loads dimension config from file
2. System validates dimension config schema
3. System validates CSV file exists and is readable
4. System validates CSV columns match dimension schema
5. System begins database transaction
6. For `truncate_and_load`:
   - System executes DELETE FROM target_table
   - System inserts all rows from CSV
7. For `upsert`:
   - System executes INSERT ON CONFLICT DO UPDATE for each row
8. System commits transaction
9. System reports: rows loaded, duration

**Postconditions**:
- Target table contains exactly the data from CSV (for truncate_and_load)
- Target table contains merged data (for upsert)
- All required fields populated
- Optional fields nullable

**Alternative Flows**:
- **A1**: Target table does not exist
  - System creates table from dimension schema
  - System proceeds with load
- **A2**: Transaction failure
  - System rolls back entire transaction
  - Original table state preserved
  - Error reported with cause

**Success Metrics**:
- Row count in target table matches expected
- Primary key constraint satisfied
- No orphaned partial loads

---

### UC-3: Sync Dimensions on Deployment

**ID**: UC-3
**Title**: Automatic Dimension Sync During Deployment
**Actor**: Deploy Script (automated)
**Preconditions**:
- `deploy.sh sync` invoked
- TimescaleDB running and healthy
- Dimension configs in `config/base/dimensions/`

**Flow**:
1. deploy.sh completes stream config sync to etcd
2. deploy.sh discovers dimension configs in directory
3. For each enabled dimension config:
   - deploy.sh invokes dimension sync logic
   - Sync follows UC-2 flow
4. deploy.sh reports summary of all dimension syncs

**Postconditions**:
- All enabled dimensions synced to current CSV state
- Deploy script exit code reflects overall success/failure

**Alternative Flows**:
- **A1**: One dimension fails, others succeed
  - Failed dimension logged with error
  - Other dimensions still synced
  - Overall exit code non-zero
  - Summary shows which failed

---

### UC-4: Dry-Run Validation

**ID**: UC-4
**Title**: Validate Dimension Sync Without Execution
**Actor**: Data Engineer
**Preconditions**:
- Dimension config exists
- CSV file exists

**Flow**:
1. Data Engineer executes `ndp dimension sync entity-context --dry-run`
2. System loads and validates dimension config
3. System validates CSV file exists and is readable
4. System validates CSV header matches schema
5. System parses all rows (validates type conversions)
6. System reports validation summary (no database changes)

**Postconditions**:
- No database modifications
- Validation report produced
- Exit code 0 if valid, non-zero if validation errors

**Success Metrics**:
- All validation errors reported before real execution would occur
- No side effects on database

---

## 3. Data Models

### 3.1 CSV Source Config Schema (Stream Extension)

Extends existing stream config with `source.type: csv`:

```yaml
# YAML Schema for CSV Source Type in Stream Config
# Location: config/base/streams/{stream-id}/config.yaml

stream_id: historical-aq                    # string, required, kebab-case
enabled: true                               # boolean, default: true
description: "Historical air quality data import"

source:
  type: csv                                 # enum: "csv" (new type)
  path: data/imports/historical_readings.csv  # string, required, relative or absolute

  # Timestamp extraction
  timestamp_field: timestamp                # string, required, column name
  timestamp_format: iso8601                 # string, default: "iso8601"
                                            # options: iso8601, epoch_seconds, epoch_millis
                                            #          or strftime format string

  # CSV parsing options
  delimiter: ","                            # string, default: ","
  encoding: utf-8                           # string, default: "utf-8"
  has_header: true                          # boolean, default: true
  skip_rows: 0                              # integer, default: 0 (skip initial rows)

  # Error handling
  on_error: skip                            # enum: "skip" | "abort", default: "skip"
  max_errors: 1000                          # integer, default: 1000 (abort if exceeded)

  # NDP context (written with each record)
  ndp_id: historical-import-001             # string, required, stable identifier
  context:                                  # object, optional metadata
    import_batch: "2024-Q1"
    source_system: "legacy-db-export"

# Schema mapping - reuses existing entity_schemas pattern
entity_schemas:
  - entity_type: air_quality
    fields:
      - name: pm25
        source_field: pm25                  # column name in CSV
        data_type: float
      - name: temperature
        source_field: temp_c
        data_type: float
      - name: humidity
        source_field: rh_percent
        data_type: float

# Bronze storage config
storage:
  batch_size: 1000
  target_partition: daily                   # how to partition output
```

#### Timestamp Format Options

| Format | Example Input | Description |
|--------|---------------|-------------|
| `iso8601` | `2024-01-15T10:30:00Z` | ISO 8601 with timezone |
| `epoch_seconds` | `1705315800` | Unix epoch seconds |
| `epoch_millis` | `1705315800000` | Unix epoch milliseconds |
| `%Y-%m-%d %H:%M:%S` | `2024-01-15 10:30:00` | Custom strftime format |

---

### 3.2 Dimension Config Schema

New configuration type for reference data:

```yaml
# YAML Schema for Dimension Config
# Location: config/base/dimensions/{dimension-id}.yaml

dimension_id: entity-context               # string, required, kebab-case
enabled: true                              # boolean, default: true
description: "Entity metadata for enrichment joins"

# Target specification
target:
  table: silver.entity_context             # string, required, fully-qualified table name
  schema: silver                           # string, extracted from table name
  primary_key: [ndp_id]                    # array of strings, required for upsert

# Source specification
source:
  type: csv                                # enum: "csv" (extensible for future: api, db)
  path: config/dimensions/entity_context.csv  # string, required
  delimiter: ","                           # string, default: ","
  encoding: utf-8                          # string, default: "utf-8"
  has_header: true                         # boolean, default: true

# Schema definition
schema:
  fields:
    - name: ndp_id                         # string, required, column name
      data_type: text                      # enum: text, integer, bigint, float, boolean,
                                           #       timestamptz, date, jsonb
      required: true                       # boolean, NOT NULL constraint
      primary_key: true                    # boolean, part of PK

    - name: category
      data_type: text
      required: true

    - name: friendly_name
      data_type: text
      required: false                      # nullable

    - name: location_path
      data_type: text
      required: false

    - name: correlates_with
      data_type: text
      required: false
      description: "ndp_id of correlated sensor"

    - name: orientation
      data_type: text
      required: false
      enum: [north, south, east, west]     # optional: allowed values

# Load strategy
load:
  strategy: truncate_and_load              # enum: "truncate_and_load" | "upsert"
  batch_size: 1000                         # integer, default: 1000

  # For upsert strategy only:
  conflict_columns: [ndp_id]               # columns for ON CONFLICT (usually = primary_key)
  update_columns: [category, friendly_name, location_path, correlates_with, orientation]
```

#### Data Type Mappings

| Config Type | PostgreSQL Type | Rust Type | Notes |
|-------------|-----------------|-----------|-------|
| `text` | TEXT | String | Unbounded string |
| `integer` | INTEGER | i32 | 4-byte signed |
| `bigint` | BIGINT | i64 | 8-byte signed |
| `float` | DOUBLE PRECISION | f64 | 8-byte IEEE 754 |
| `boolean` | BOOLEAN | bool | true/false |
| `timestamptz` | TIMESTAMPTZ | DateTime<Utc> | Timestamp with timezone |
| `date` | DATE | NaiveDate | Date without time |
| `jsonb` | JSONB | serde_json::Value | JSON blob |

---

### 3.3 Generated Table DDL

For dimension with auto-create enabled:

```sql
-- Auto-generated from dimension config: entity-context
CREATE TABLE IF NOT EXISTS silver.entity_context (
    ndp_id TEXT NOT NULL,
    category TEXT NOT NULL,
    friendly_name TEXT,
    location_path TEXT,
    correlates_with TEXT,
    orientation TEXT,
    _loaded_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    _dimension_version TEXT,
    PRIMARY KEY (ndp_id)
);

-- Metadata columns added automatically:
-- _loaded_at: when this row was loaded
-- _dimension_version: config version that loaded this
```

---

## 4. Interface Specifications

### 4.1 CLI Interface

#### Command: `ndp dimension list`

```
USAGE:
    ndp dimension list [OPTIONS]

OPTIONS:
    --format <FORMAT>    Output format: table (default), json, csv
    --enabled-only       Show only enabled dimensions
    -v, --verbose        Show additional details (file path, row count)

OUTPUT (table format):
    DIMENSION_ID      STATUS    TARGET_TABLE            LAST_SYNC
    entity-context    enabled   silver.entity_context   2024-01-15 10:30:00
    location-hierarchy disabled silver.locations        never

EXIT CODES:
    0    Success
    1    Configuration error
```

#### Command: `ndp dimension sync`

```
USAGE:
    ndp dimension sync <DIMENSION_ID> [OPTIONS]
    ndp dimension sync --all [OPTIONS]

ARGUMENTS:
    <DIMENSION_ID>    Specific dimension to sync

OPTIONS:
    --all             Sync all enabled dimensions
    --dry-run         Validate only, no database changes
    --force           Proceed even if target table has unknown columns
    -v, --verbose     Show row-level details

OUTPUT:
    [INFO] Loading dimension config: entity-context
    [INFO] Validating CSV: config/dimensions/entity_context.csv
    [INFO] CSV columns: ndp_id, category, friendly_name, location_path, correlates_with, orientation
    [INFO] Schema validation: OK
    [INFO] Beginning truncate_and_load to silver.entity_context
    [INFO] Deleted 10 existing rows
    [INFO] Inserted 15 rows in 0.05s
    [INFO] Sync complete: entity-context

    Summary:
      Dimensions synced: 1
      Total rows loaded: 15
      Duration: 0.12s

EXIT CODES:
    0    Success
    1    Configuration error
    2    File not found
    3    Schema validation error
    4    Database error
    5    Partial failure (--all mode, some dimensions failed)
```

#### Command: `ndp stream ingest`

```
USAGE:
    ndp stream ingest <STREAM_ID> [OPTIONS]

ARGUMENTS:
    <STREAM_ID>    Stream with CSV source to ingest

OPTIONS:
    --file <PATH>     Override source.path from config
    --dry-run         Validate only, no Bronze writes
    --limit <N>       Process only first N rows
    -v, --verbose     Show row-level details

OUTPUT:
    [INFO] Loading stream config: historical-aq
    [INFO] Source type: csv
    [INFO] Reading: data/imports/historical_readings.csv
    [INFO] Timestamp field: timestamp (format: iso8601)
    [INFO] Processing 50000 rows...
    [INFO] Written to Bronze: historical-aq/2024/01/15/batch_001.parquet
    [INFO] Written to Bronze: historical-aq/2024/01/15/batch_002.parquet

    Summary:
      Rows processed: 50000
      Rows written: 49985
      Rows skipped (errors): 15
      Parquet files created: 2
      Duration: 1.23s

EXIT CODES:
    0    Success
    1    Configuration error
    2    File not found
    3    Schema/header mismatch
    4    Storage error
    5    Too many parse errors (exceeded max_errors)
```

---

### 4.2 Config File Interface

#### Directory Structure

```
config/
  base/
    streams/
      historical-aq/
        config.yaml           # Stream config with source.type: csv
    dimensions/
      entity_context.yaml     # Dimension config
      locations.yaml          # Another dimension
    dimension_data/           # CSV files for dimensions (alternative location)
      entity_context.csv
      locations.csv
```

#### Config Validation Schema (JSON Schema)

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "ndp-dimension-config",
  "type": "object",
  "required": ["dimension_id", "target", "source", "schema", "load"],
  "properties": {
    "dimension_id": {
      "type": "string",
      "pattern": "^[a-z][a-z0-9-]{2,63}$"
    },
    "enabled": {
      "type": "boolean",
      "default": true
    },
    "target": {
      "type": "object",
      "required": ["table", "primary_key"],
      "properties": {
        "table": { "type": "string", "pattern": "^[a-z_]+\\.[a-z_]+$" },
        "primary_key": { "type": "array", "items": { "type": "string" } }
      }
    },
    "source": {
      "type": "object",
      "required": ["type", "path"],
      "properties": {
        "type": { "enum": ["csv"] },
        "path": { "type": "string" },
        "delimiter": { "type": "string", "default": "," },
        "encoding": { "type": "string", "default": "utf-8" }
      }
    },
    "schema": {
      "type": "object",
      "required": ["fields"],
      "properties": {
        "fields": {
          "type": "array",
          "items": {
            "type": "object",
            "required": ["name", "data_type"],
            "properties": {
              "name": { "type": "string" },
              "data_type": { "enum": ["text", "integer", "bigint", "float", "boolean", "timestamptz", "date", "jsonb"] },
              "required": { "type": "boolean", "default": false }
            }
          }
        }
      }
    },
    "load": {
      "type": "object",
      "required": ["strategy"],
      "properties": {
        "strategy": { "enum": ["truncate_and_load", "upsert"] },
        "batch_size": { "type": "integer", "default": 1000 }
      }
    }
  }
}
```

---

### 4.3 Integration with deploy.sh

Add to deploy.sh sync flow:

```bash
# In sync_config() function, after stream sync:

sync_dimensions() {
    log "Syncing dimension tables..."

    # Wait for TimescaleDB
    until dcx timescaledb pg_isready -U postgres -d ndp >/dev/null 2>&1; do
        warn "Waiting for TimescaleDB..."
        sleep 2
    done

    # Find all dimension configs
    local DIM_DIR="$REPO_ROOT/config/base/dimensions"
    if [ ! -d "$DIM_DIR" ]; then
        log "No dimension configs found, skipping"
        return 0
    fi

    local success=0
    local failed=0

    for config in "$DIM_DIR"/*.yaml; do
        [ -f "$config" ] || continue

        dim_id=$(basename "$config" .yaml)
        log "Syncing dimension: $dim_id"

        if ndp dimension sync "$dim_id" 2>&1; then
            success=$((success + 1))
        else
            warn "Failed to sync dimension: $dim_id"
            failed=$((failed + 1))
        fi
    done

    log "Dimension sync complete: $success succeeded, $failed failed"
    [ $failed -eq 0 ] || return 1
}
```

---

## 5. Acceptance Criteria Matrix

### 5.1 Part 1: CSV Source Type

| AC# | Criterion | Test Type | Pass Condition | Fail Condition |
|-----|-----------|-----------|----------------|----------------|
| AC-1.1 | `source.type: csv` recognized | Unit | Config parser returns SourceType::Csv | Parser error or wrong type |
| AC-1.2 | CSV adapter reads file | Integration | File opened, rows yielded | FileNotFound error |
| AC-1.3 | Timestamp parsing (iso8601) | Unit | `2024-01-15T10:30:00Z` -> correct DateTime | Parse error |
| AC-1.4 | Timestamp parsing (epoch_seconds) | Unit | `1705315800` -> correct DateTime | Parse error |
| AC-1.5 | Column mapping via entity_schemas | Unit | Column "temp_c" maps to field "temperature" | Mapping error |
| AC-1.6 | Bronze Parquet format matches other sources | Integration | Same schema as MQTT-sourced Parquet | Schema mismatch |
| AC-1.7 | Silver ETL promotes CSV-sourced data | Integration | Data appears in Silver table | No data in Silver |
| AC-1.8 | Invalid rows skipped (on_error: skip) | Integration | Good rows written, bad rows logged | Process aborts |
| AC-1.9 | Invalid rows abort (on_error: abort) | Integration | Process stops, error reported | Silent skip |
| AC-1.10 | `ndp stream ingest` triggers CSV ingest | E2E | Command executes, data in Bronze | Command fails |

### 5.2 Part 2: Dimension Table Configs

| AC# | Criterion | Test Type | Pass Condition | Fail Condition |
|-----|-----------|-----------|----------------|----------------|
| AC-2.1 | Dimension config schema validated | Unit | Valid config parses, invalid rejects | Wrong validation |
| AC-2.2 | Config files discovered in dimensions/ | Integration | All .yaml files found | Files missed |
| AC-2.3 | CSV source type for dimensions | Integration | CSV read and parsed | Read error |
| AC-2.4 | Schema validation (required fields) | Unit | Missing required field -> error | Silent failure |
| AC-2.5 | Schema validation (data types) | Unit | Type mismatch -> error | Silent coercion |
| AC-2.6 | truncate_and_load: DELETE + INSERT | Integration | Old data removed, new data inserted | Partial state |
| AC-2.7 | upsert: INSERT ON CONFLICT UPDATE | Integration | Existing updated, new inserted | Duplicate key error |
| AC-2.8 | Transaction rollback on failure | Integration | Table unchanged after error | Partial data |
| AC-2.9 | Auto-create table if not exists | Integration | Table created from schema | CREATE fails |
| AC-2.10 | deploy.sh sync processes dimensions | E2E | All dimensions synced | Dimensions skipped |

### 5.3 Part 3: CLI

| AC# | Criterion | Test Type | Pass Condition | Fail Condition |
|-----|-----------|-----------|----------------|----------------|
| AC-3.1 | `ndp dimension list` shows all | E2E | All dimensions displayed | Missing entries |
| AC-3.2 | `ndp dimension sync <id>` loads specific | E2E | Only specified loaded | Wrong dimension |
| AC-3.3 | `ndp dimension sync --all` loads all | E2E | All enabled loaded | Some skipped |
| AC-3.4 | `--dry-run` no side effects | E2E | DB unchanged after run | Data modified |
| AC-3.5 | Summary output accuracy | E2E | Counts match actual | Counts incorrect |
| AC-3.6 | Exit code 0 on success | E2E | Exit code 0 | Non-zero |
| AC-3.7 | Exit code non-zero on failure | E2E | Exit code > 0 | Zero on failure |

### 5.4 Error Handling

| AC# | Criterion | Test Type | Pass Condition | Fail Condition |
|-----|-----------|-----------|----------------|----------------|
| AC-4.1 | Malformed CSV line number | Unit | Error includes "line 47" | No line number |
| AC-4.2 | Missing required columns | Unit | Lists missing columns | Generic error |
| AC-4.3 | Type conversion error context | Unit | Shows column, value, expected type | Generic parse error |
| AC-4.4 | File not found shows path | Unit | Error includes full path | No path |
| AC-4.5 | Empty file warning | Unit | Warning logged, exit 0 | Error or silent |

---

## 6. Dependencies

### 6.1 Existing NDP Components

| Component | Location | Dependency Type | Notes |
|-----------|----------|-----------------|-------|
| Stream Config Parser | `core/src/types/stream_config.rs` | Extend | Add SourceType::Csv |
| Source Type Enum | `core/src/types/stream_config.rs` | Extend | Add Csv variant |
| Bronze Parquet Writer | `core/src/storage/parquet.rs` | Use | Write CSV data as Parquet |
| etcd Config Client | `config-client/` | Use | Read stream/dimension configs |
| Silver ETL Pipeline | `apps/silver-etl/` | Downstream | Consumes Bronze data |
| deploy.sh | `deploy/pi/deploy.sh` | Extend | Add dimension sync |
| Data Dictionary | `data_dictionary` schema | Extend | Register dimension metadata |

### 6.2 External Dependencies

| Dependency | Version | Purpose |
|------------|---------|---------|
| csv (Rust crate) | latest stable | CSV parsing |
| chrono | existing | Timestamp parsing |
| sqlx | existing | Database operations |
| tokio-postgres | existing | Alternative DB driver |

### 6.3 Service Dependencies

| Service | Required State | Used For |
|---------|----------------|----------|
| TimescaleDB | Running, ndp database exists | Dimension table storage |
| etcd | Running (optional) | Config storage |
| Bronze storage | Writable | CSV stream ingestion output |

---

## 7. Constraints

### 7.1 Technical Constraints

| ID | Constraint | Rationale |
|----|------------|-----------|
| TC-1 | CSV source is one-time ingest (not continuous polling) | Batch import use case |
| TC-2 | Dimensions skip Bronze layer | Not observational timeseries |
| TC-3 | Maximum file size 10GB | Memory and I/O practical limits |
| TC-4 | UTF-8 and common encodings only | Reasonable scope |

### 7.2 Business Constraints

| ID | Constraint | Rationale |
|----|------------|-----------|
| BC-1 | Must follow existing config-driven patterns | Architectural consistency |
| BC-2 | No new "loader" system - extensions only | Minimize new components |
| BC-3 | CLI naming follows existing `ndp` pattern | UX consistency |

### 7.3 Regulatory Constraints

| ID | Constraint | Rationale |
|----|------------|-----------|
| RC-1 | Audit trail for dimension changes | Data governance |
| RC-2 | Sensitive data not logged at row level | Privacy |

---

## 8. Edge Cases & Error Handling

### 8.1 CSV Parsing Edge Cases

| Edge Case | Expected Behavior |
|-----------|-------------------|
| Empty file | Warning logged, exit 0, no records written |
| Header only, no data rows | Info logged, exit 0, no records written |
| Quoted fields with embedded delimiters | Parse correctly per RFC 4180 |
| Quoted fields with embedded newlines | Parse correctly per RFC 4180 |
| Trailing empty columns | Treat as empty string or null based on schema |
| Leading/trailing whitespace in values | Trim unless quoted |
| BOM (byte order mark) at file start | Skip automatically |
| Mixed line endings (LF, CRLF) | Handle transparently |
| Duplicate column headers | Error on config validation |
| Column count mismatch in data row | Error with line number |

### 8.2 Dimension Load Edge Cases

| Edge Case | Expected Behavior |
|-----------|-------------------|
| Target table has extra columns not in schema | Warning, proceed (extra columns remain) |
| Target table missing columns from schema | Error on validation |
| Concurrent sync to same table | Serialize (lock or queue) |
| CSV has more rows than batch_size | Process in batches |
| Primary key violation on upsert | Update existing row |
| Foreign key violation | Error, rollback transaction |
| Null in required field | Error with row/column context |
| Enum value not in allowed list | Error with value and allowed list |

### 8.3 File System Edge Cases

| Edge Case | Expected Behavior |
|-----------|-------------------|
| Relative path resolution | Resolve relative to config directory |
| Symlink to file | Follow symlink |
| File locked by another process | Error with clear message |
| File modified during read | Complete with original content (buffered read) |
| Insufficient permissions | Error with permission details |
| Disk full during write | Error, clean up partial files |

---

## Appendix A: Example Configurations

### A.1 Complete CSV Stream Config

```yaml
# config/base/streams/historical-aq/config.yaml
stream_id: historical-aq
enabled: true
description: "Historical air quality readings import"
version: "1.0.0"

source:
  type: csv
  path: data/imports/historical_readings.csv
  timestamp_field: reading_time
  timestamp_format: "%Y-%m-%d %H:%M:%S"
  delimiter: ","
  encoding: utf-8
  on_error: skip
  max_errors: 100
  ndp_id: historical-import-2024q1
  context:
    import_batch: "2024-Q1"
    source_system: "legacy-export"

entity_schemas:
  - entity_type: air_quality
    fields:
      - name: pm25
        source_field: pm25
        data_type: float
      - name: pm10
        source_field: pm10
        data_type: float
      - name: temperature
        source_field: temp_c
        data_type: float
      - name: humidity
        source_field: rh_percent
        data_type: float
      - name: co2
        source_field: co2_ppm
        data_type: int

storage:
  batch_size: 1000
  batch_timeout_secs: 30
```

### A.2 Complete Dimension Config (Entity Context)

```yaml
# config/base/dimensions/entity_context.yaml
dimension_id: entity-context
enabled: true
description: "Entity metadata for JOIN enrichment"

target:
  table: silver.entity_context
  primary_key: [ndp_id]

source:
  type: csv
  path: config/dimensions/entity_context.csv
  delimiter: ","
  encoding: utf-8
  has_header: true

schema:
  fields:
    - name: ndp_id
      data_type: text
      required: true
      description: "Stable entity identifier"
    - name: category
      data_type: text
      required: true
      description: "Entity category (door, window, sensor)"
    - name: friendly_name
      data_type: text
      required: false
      description: "Human-readable name"
    - name: location_path
      data_type: text
      required: false
      description: "Hierarchical location path"
    - name: correlates_with
      data_type: text
      required: false
      description: "ndp_id of correlated sensor"
    - name: orientation
      data_type: text
      required: false
      description: "Cardinal direction"

load:
  strategy: truncate_and_load
  batch_size: 500
```

---

## Appendix B: Gold View Example

```sql
-- gold.events_with_context
-- Joins Silver events with entity context dimension

CREATE OR REPLACE VIEW gold.events_with_context AS
SELECT
    e.observation_time,
    e.ndp_id,
    e.event_type,
    e.new_state,
    e.old_state,
    -- Enrichment from dimension
    c.category,
    c.friendly_name,
    c.location_path,
    c.correlates_with,
    c.orientation
FROM silver.state_events e
LEFT JOIN silver.entity_context c
    ON e.ndp_id = c.ndp_id;

-- Usage:
-- SELECT * FROM gold.events_with_context
-- WHERE category = 'door'
-- AND observation_time > NOW() - INTERVAL '1 hour';
```

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2026-01-29 | SPARC Specification Agent | Initial specification |
