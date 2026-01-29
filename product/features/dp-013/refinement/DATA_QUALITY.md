# dp-013: CSV Data Quality Strategy

This document defines the data quality approach for CSV source types and dimension table loading, following NDP's [Layered DQ Strategy](../../../research/analyticplatforminfrastructure/04-LAYERED-DQ-STRATEGY.md).

---

## Context: CSV vs Streaming Sources

CSV sources have different DQ characteristics than streaming (HTTP/MQTT) sources:

| Aspect | Streaming Sources | CSV Sources |
|--------|-------------------|-------------|
| **Volume** | Continuous, low-volume per batch | Bulk, potentially large files |
| **Timing** | Real-time, individual records | Batch, all-at-once |
| **Error Impact** | Skip one row, continue | Abort entire file vs partial load |
| **Retry** | Automatic on next poll | Manual re-run required |
| **Use Case** | Live measurements | Historical backfill, reference data |

**Key principle**: CSV operations should be **explicit and predictable**. Users trigger imports manually and expect clear success/failure outcomes.

---

## 1. Validation Layers

### Layer 0: Pre-Load Validation (Config)

Before any data processing begins, validate prerequisites.

**Checks performed:**

| Check | Failure Behavior | Rationale |
|-------|------------------|-----------|
| CSV file exists | Fatal - abort | Cannot proceed without source |
| File is readable | Fatal - abort | Permission/lock issues |
| File has content | Warning - no-op | Empty file is valid but unusual |
| Required columns present | Fatal - abort | Schema mismatch before processing |
| Config schema valid | Fatal - abort | Invalid YAML/field definitions |

**Implementation:**

```rust
pub fn validate_csv_prerequisites(
    config: &CsvConfig,
    file_path: &Path,
) -> Result<PreLoadReport, CsvError> {
    // 1. File existence
    if !file_path.exists() {
        return Err(CsvError::FileNotFound(file_path.to_path_buf()));
    }

    // 2. Read headers
    let mut reader = csv::Reader::from_path(file_path)?;
    let headers: HashSet<_> = reader.headers()?.iter().collect();

    // 3. Check required columns
    let missing: Vec<_> = config.required_columns()
        .filter(|col| !headers.contains(col.as_str()))
        .collect();

    if !missing.is_empty() {
        return Err(CsvError::MissingColumns(missing));
    }

    Ok(PreLoadReport { row_estimate: count_lines(file_path)? })
}
```

### Layer 1: Parse-Time Validation (Row Level)

Applied during CSV parsing, before Bronze/Silver write.

**For Streams (CSV -> Bronze):**

| Check | Severity | Behavior |
|-------|----------|----------|
| Row parse failure | Configurable | Skip or abort based on `on_error` |
| Timestamp parse failure | Fatal per row | Skip row (cannot index without time) |
| Required field missing | Configurable | Skip or abort |
| Column count mismatch | Fatal per row | Skip row |

**For Dimensions (CSV -> Silver):**

| Check | Severity | Behavior |
|-------|----------|----------|
| Row parse failure | Fatal | Abort (dimensions must be complete) |
| Type conversion failure | Fatal | Abort with line number |
| Required field missing | Fatal | Abort with field name |
| Primary key null/empty | Fatal | Abort (referential integrity) |

**Implementation:**

```rust
pub struct RowValidationResult {
    pub valid: bool,
    pub row_number: usize,
    pub errors: Vec<RowError>,
    pub warnings: Vec<RowWarning>,
}

pub enum RowError {
    ParseFailed { line: usize, message: String },
    TypeConversion { field: String, value: String, expected: DataType },
    MissingRequired { field: String },
    InvalidTimestamp { field: String, value: String },
    NullPrimaryKey { field: String },
}
```

### Layer 2: Post-Load Validation (Table Level)

Applied after data is loaded, verifies aggregate constraints.

**For Dimensions:**

| Check | Severity | Behavior |
|-------|----------|----------|
| Primary key uniqueness | Fatal | Abort and rollback transaction |
| Row count verification | Warning | Log if differs from source |
| Referential integrity | Optional | Log foreign key violations |

**For Streams (after ETL to Silver):**

| Check | Severity | Behavior |
|-------|----------|----------|
| Timestamp range valid | Warning | Log unexpected range |
| Duplicate detection | Warning | Log duplicates (handled by upsert) |
| Completeness check | Info | Report field coverage |

---

## 2. Error Classification

### Error Severity Matrix

| Error Type | CSV Streams | Dimensions | Rationale |
|------------|-------------|------------|-----------|
| File not found | Fatal | Fatal | Cannot proceed |
| Malformed CSV (parse error) | Configurable | Fatal | Streams tolerate gaps; dimensions must be complete |
| Missing required column | Fatal | Fatal | Schema violation |
| Type conversion failure | Configurable | Fatal | Dimensions need integrity |
| Empty file | Warning | Warning | Valid but unusual |
| Partial read (I/O error) | Fatal | Fatal | Data integrity risk |
| Primary key violation | N/A | Fatal | Dimension integrity |
| Timestamp parse failure | Skip row | Fatal | Streams have fallback; dimensions don't |

### Error Type Definitions

```rust
#[derive(Debug, thiserror::Error)]
pub enum CsvError {
    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Malformed CSV at line {line}: {message}")]
    MalformedCsv { line: usize, message: String },

    #[error("Missing required columns: {0:?}")]
    MissingColumns(Vec<String>),

    #[error("Type conversion failed for field '{field}' at line {line}: cannot parse '{value}' as {expected}")]
    TypeConversion {
        line: usize,
        field: String,
        value: String,
        expected: String,
    },

    #[error("Empty file: {0}")]
    EmptyFile(PathBuf),

    #[error("Primary key violation: duplicate value '{value}' for key '{key}' at lines {first_line} and {second_line}")]
    PrimaryKeyViolation {
        key: String,
        value: String,
        first_line: usize,
        second_line: usize,
    },

    #[error("Timestamp parse failed at line {line}: cannot parse '{value}' with format '{format}'")]
    TimestampParse {
        line: usize,
        value: String,
        format: String,
    },
}
```

---

## 3. Error Reporting

### Log Format

Structured logging with consistent format for debugging and alerting.

**Row-level errors:**
```
[ERROR] dp-013/csv: Row 42: Type conversion failed for field 'temperature' - cannot parse 'N/A' as float
[ERROR] dp-013/csv: Row 55: Missing required field 'ndp_id'
[WARN]  dp-013/csv: Row 100: Skipped - timestamp parse failed for '2024-13-45'
```

**File-level errors:**
```
[ERROR] dp-013/csv: File not found: /path/to/missing.csv
[ERROR] dp-013/csv: Missing required columns: ['ndp_id', 'category']
[ERROR] dp-013/csv: Malformed CSV at line 1: unexpected number of fields (expected 6, got 4)
```

**Structured log format (JSON):**
```json
{
  "level": "ERROR",
  "target": "dp-013/csv",
  "message": "Type conversion failed",
  "row": 42,
  "field": "temperature",
  "value": "N/A",
  "expected_type": "float",
  "file": "/path/to/file.csv",
  "stream_id": "historical-aq"
}
```

### Summary Report

Generated after every CSV operation for CLI output.

**Success case:**
```
CSV Import Summary
==================
Source:      /data/imports/historical_readings.csv
Target:      Bronze (stream: historical-aq)
Duration:    2.3s

Rows:
  Total:     1,000
  Processed: 995
  Skipped:   5

Skipped rows:
  Row 42:  Type conversion failed (temperature)
  Row 55:  Missing required field (ndp_id)
  Row 100: Invalid timestamp format
  Row 234: Invalid timestamp format
  Row 567: Type conversion failed (humidity)

Status: SUCCESS (with warnings)
```

**Failure case:**
```
CSV Import Summary
==================
Source:      /data/imports/entity_context.csv
Target:      silver.entity_context (dimension)
Duration:    0.1s

ERROR: Import aborted

Reason: Missing required columns
Missing: ['category', 'ndp_id']
Found:   ['name', 'location', 'type']

Hint: Check column names match the dimension schema.
      Expected columns: ndp_id, category, friendly_name, location_path, correlates_with, orientation

Status: FAILED
```

### Report Structure

```rust
pub struct CsvImportReport {
    pub source_file: PathBuf,
    pub target: ImportTarget,
    pub duration: Duration,
    pub status: ImportStatus,

    // Row counts
    pub total_rows: usize,
    pub processed_rows: usize,
    pub skipped_rows: usize,

    // Error details
    pub row_errors: Vec<RowError>,
    pub file_error: Option<CsvError>,

    // For dimensions
    pub rows_before: Option<usize>,  // Pre-existing row count
    pub rows_after: Option<usize>,   // Post-load row count
    pub load_strategy: Option<LoadStrategy>,
}

pub enum ImportStatus {
    Success,
    SuccessWithWarnings,
    Failed,
}
```

---

## 4. Transparency Tables

### Dimension Load History

Track every dimension load operation for auditing and debugging.

```sql
CREATE TABLE silver.dimension_load_history (
    load_id         SERIAL PRIMARY KEY,
    dimension_id    TEXT NOT NULL,
    loaded_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    duration_ms     INTEGER,

    -- Source info
    source_file     TEXT NOT NULL,
    source_checksum TEXT,  -- SHA256 of source file

    -- Row counts
    rows_in_file    INTEGER NOT NULL,
    rows_loaded     INTEGER NOT NULL,
    rows_skipped    INTEGER DEFAULT 0,
    rows_before     INTEGER,  -- Pre-existing count
    rows_after      INTEGER,  -- Post-load count

    -- Load details
    strategy        TEXT NOT NULL,  -- 'truncate_and_load' or 'upsert'
    success         BOOLEAN NOT NULL,
    error_message   TEXT,

    -- Full report for debugging
    report_json     JSONB
);

-- Index for querying load history
CREATE INDEX idx_dim_load_history_dimension
ON silver.dimension_load_history (dimension_id, loaded_at DESC);

-- Index for finding failures
CREATE INDEX idx_dim_load_history_failures
ON silver.dimension_load_history (success, loaded_at DESC)
WHERE success = FALSE;
```

### CSV Stream Ingest History

Track CSV stream imports (distinct from regular streaming ingest).

```sql
CREATE TABLE bronze.csv_ingest_history (
    ingest_id       SERIAL PRIMARY KEY,
    stream_id       TEXT NOT NULL,
    ingested_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    duration_ms     INTEGER,

    -- Source info
    source_file     TEXT NOT NULL,
    source_checksum TEXT,

    -- Row counts
    rows_in_file    INTEGER NOT NULL,
    rows_written    INTEGER NOT NULL,
    rows_skipped    INTEGER DEFAULT 0,

    -- Config used
    on_error_policy TEXT NOT NULL,  -- 'skip' or 'abort'
    success         BOOLEAN NOT NULL,
    error_message   TEXT,

    -- Skipped row details (sample)
    skipped_rows    JSONB,  -- Array of {line, reason}

    report_json     JSONB
);

CREATE INDEX idx_csv_ingest_stream
ON bronze.csv_ingest_history (stream_id, ingested_at DESC);
```

### Querying Load History

```sql
-- Recent dimension loads
SELECT
    dimension_id,
    loaded_at,
    rows_loaded,
    strategy,
    success,
    error_message
FROM silver.dimension_load_history
WHERE loaded_at > NOW() - INTERVAL '7 days'
ORDER BY loaded_at DESC;

-- Failed loads in last 24 hours
SELECT
    dimension_id,
    loaded_at,
    error_message,
    source_file
FROM silver.dimension_load_history
WHERE success = FALSE
  AND loaded_at > NOW() - INTERVAL '24 hours';

-- Load history for specific dimension
SELECT
    loaded_at,
    rows_before,
    rows_after,
    rows_after - COALESCE(rows_before, 0) as net_change,
    strategy
FROM silver.dimension_load_history
WHERE dimension_id = 'entity-context'
ORDER BY loaded_at DESC
LIMIT 10;
```

---

## 5. `on_error` Configuration

### Configuration Options

| Setting | Behavior | Default For | Use Case |
|---------|----------|-------------|----------|
| `skip` | Log error, skip row, continue processing | Streams | Historical backfill where some bad data is acceptable |
| `abort` | Stop on first error, no partial load | Dimensions | Reference data must be complete and correct |

### Stream Config Example

```yaml
# config/base/streams/historical-aq.yaml
stream_id: historical-aq
source:
  type: csv
  path: data/imports/historical_readings.csv
  timestamp_field: timestamp
  timestamp_format: iso8601
  on_error: skip  # Default for streams
  max_errors: 100  # Abort if more than 100 rows fail
```

### Dimension Config Example

```yaml
# config/base/dimensions/entity_context.yaml
dimension_id: entity-context
source:
  type: csv
  path: config/dimensions/entity_context.csv
  on_error: abort  # Default for dimensions (cannot be changed to skip)
```

### Behavior Matrix

| Source Type | `on_error: skip` | `on_error: abort` |
|-------------|------------------|-------------------|
| **Stream CSV** | Skip bad rows, write good rows to Bronze, log errors | Stop at first error, no data written |
| **Dimension** | Not supported | Stop at first error, rollback transaction |

### Implementation

```rust
pub enum OnError {
    Skip { max_errors: Option<usize> },
    Abort,
}

impl Default for OnError {
    fn default() -> Self {
        // Different defaults per context
        OnError::Skip { max_errors: Some(100) }
    }
}

pub fn process_csv_row(
    row: &csv::StringRecord,
    line: usize,
    on_error: &OnError,
    error_count: &mut usize,
) -> Result<Option<ParsedRow>, CsvError> {
    match parse_row(row, line) {
        Ok(parsed) => Ok(Some(parsed)),
        Err(e) => match on_error {
            OnError::Skip { max_errors } => {
                *error_count += 1;
                tracing::warn!("Row {}: {} - skipping", line, e);

                if let Some(max) = max_errors {
                    if *error_count > *max {
                        return Err(CsvError::TooManyErrors {
                            count: *error_count,
                            max: *max,
                        });
                    }
                }
                Ok(None)  // Skip this row
            }
            OnError::Abort => Err(e),
        }
    }
}
```

---

## 6. DQ Integration with Existing Framework

### Relationship to Layered DQ Strategy

CSV data quality fits into the existing layered strategy:

```
CSV File
    │
    ├─── Pre-Load Validation (Layer 0) ─── File/schema checks
    │
    ▼
┌─────────────────────────────────────────────────────────────┐
│ LAYER 1: EXTRACT DQ (Parse-Time Validation)                  │
│ For Streams: Row-level validation during CSV → Bronze        │
│ Actions: skip, abort                                         │
└─────────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────────┐
│ BRONZE LAYER (Parquet)                                       │
│ Streams only - dimensions skip Bronze                        │
└─────────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────────┐
│ LAYER 2: TRANSFORM DQ                                        │
│ Bronze → Silver ETL (streams)                                │
│ CSV → Silver direct (dimensions)                             │
│ Actions: reject, flag, clamp, set_null                       │
└─────────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────────┐
│ SILVER LAYER (TimescaleDB)                                   │
│ Post-Load Validation for dimensions                          │
└─────────────────────────────────────────────────────────────┘
```

### DQ Rules for Dimensions

Dimensions can have DQ rules in their config (applied during load):

```yaml
# config/base/dimensions/entity_context.yaml
dimension_id: entity-context
schema:
  fields:
    - name: ndp_id
      data_type: text
      required: true
      dq_rules:
        - type: pattern
          pattern: "^[a-z][a-z0-9_]*$"
          action: reject
    - name: category
      data_type: text
      required: true
      dq_rules:
        - type: enum
          allowed: [door, window, sensor, device]
          action: reject
    - name: orientation
      data_type: text
      dq_rules:
        - type: enum
          allowed: [north, south, east, west, null]
          action: flag
```

---

## 7. CLI Output Examples

### Successful Stream Ingest

```bash
$ ndp stream ingest historical-aq

Validating config... OK
Reading /data/imports/historical_readings.csv...

Progress: [========================================] 1000/1000 rows

CSV Import Summary
==================
Stream:      historical-aq
Target:      Bronze (Parquet)
Duration:    2.3s

Rows:
  Total:     1,000
  Written:   995
  Skipped:   5

Skipped rows written to: /var/log/ndp/csv_errors_20260129_143022.log

Status: SUCCESS
```

### Successful Dimension Sync

```bash
$ ndp dimension sync entity-context

Validating config... OK
Reading config/dimensions/entity_context.csv...

Dimension Sync Summary
======================
Dimension:   entity-context
Target:      silver.entity_context
Strategy:    truncate_and_load
Duration:    0.4s

Rows:
  In file:   15
  Loaded:    15

Table state:
  Before:    12
  After:     15
  Net:       +3

Status: SUCCESS
```

### Failed Dimension Sync

```bash
$ ndp dimension sync entity-context

Validating config... OK
Reading config/dimensions/entity_context.csv...

ERROR at line 5: Type conversion failed
  Field: 'category'
  Value: 'DOOR' (expected lowercase enum)
  Allowed: door, window, sensor, device

Dimension Sync Summary
======================
Dimension:   entity-context
Target:      silver.entity_context
Strategy:    truncate_and_load
Duration:    0.1s

Status: FAILED (no changes made)

Hint: Fix the CSV file and re-run.
```

### Dry-Run Mode

```bash
$ ndp dimension sync entity-context --dry-run

Validating config... OK
Reading config/dimensions/entity_context.csv...

Dry-Run Summary
===============
Dimension:   entity-context
Target:      silver.entity_context
Strategy:    truncate_and_load

Would process:
  Rows:      15
  Current:   12
  Net:       +3

Validation:  PASSED (all rows valid)

No changes made (dry-run mode).
```

---

## Summary

| Aspect | CSV Streams | Dimensions |
|--------|-------------|------------|
| **Target** | Bronze (Parquet) | Silver (TimescaleDB) |
| **Default `on_error`** | `skip` | `abort` (forced) |
| **Partial Load** | Allowed | Not allowed |
| **Transparency** | `bronze.csv_ingest_history` | `silver.dimension_load_history` |
| **Use Case** | Historical backfill | Reference/lookup data |
| **Retry** | Re-run command | Fix file, re-run |

**Key Principles:**

1. **Dimensions must be complete** - no partial loads, abort on any error
2. **Streams can tolerate gaps** - skip bad rows, write good ones
3. **Every load is auditable** - transparency tables track all operations
4. **Fail fast, fail loud** - clear error messages with line numbers and context
