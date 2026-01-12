# DP-007 Specification: Pre-Transform Parser Integration for Silver ETL

**Feature**: DP-007
**Document**: SPECIFICATION.md
**Version**: 1.0
**Date**: 2026-01-12
**Author**: NDP Architect
**Status**: Draft

---

## 1. Executive Summary

This specification defines the requirements for integrating the existing `ColumnOrientedParser` from neural-core into the silver-etl pipeline. The integration enables pre-transformation of columnar array data structures (such as NWS gridpoints forecasts) before DuckDB SQL processing.

### Problem Statement

The NWS gridpoints forecast data has a columnar array structure where each metric (temperature, windSpeed, etc.) contains arrays of `{validTime, value}` pairs. The current silver-etl uses DuckDB SQL which cannot easily handle array explosion without complex UNNEST operations.

The `ColumnOrientedParser` already solves this problem for Bronze ingestion by exploding arrays into individual `TimeSeriesPoint` objects. However, Bronze stores the raw JSON payload, not the parsed points.

### Proposed Solution

Add an optional `pre_transform` configuration section to `SilverEtlConfig` that:
1. Parses `raw_payload` through `ColumnOrientedParser` before DuckDB processing
2. Outputs flattened rows with one row per metric per validTime
3. Enables standard DuckDB SQL to process the flattened data

---

## 2. Functional Requirements

### FR-001: Optional Pre-Transform Support

**Requirement**: silver-etl SHALL support optional pre-transformation of `raw_payload` before DuckDB processing.

**Rationale**: Columnar array data structures like NWS gridpoints forecasts cannot be processed with standard `json_extract` operations in DuckDB. Pre-transformation flattens the data structure.

**Acceptance Criteria**:
- Pre-transform is configurable per stream via YAML configuration
- Streams without `pre_transform` config continue to work unchanged
- Pre-transform can be enabled/disabled independently of ETL enabled status

---

### FR-002: Reuse ColumnOrientedParser

**Requirement**: Pre-transform SHALL reuse the existing `ColumnOrientedParser` from neural-core.

**Rationale**: The parser is already tested, handles NWS-specific timestamp formats (ISO 8601 duration), and supports unit conversions.

**Acceptance Criteria**:
- No new parser implementation required
- Parser configuration loaded from stream config `column_config` section
- Parser errors propagate with context to ETL pipeline

**Reference**: `/workspaces/neural-data-platform/core/src/parsers/column_oriented.rs`

---

### FR-003: Columnar Array Flattening

**Requirement**: Pre-transform SHALL flatten columnar arrays into individual rows.

**Rationale**: Each Bronze row contains ~40 metrics with ~156 values each (~6000+ data points). These must become individual Silver rows.

**Input Structure** (single Bronze row):
```json
{
  "properties": {
    "temperature": {
      "values": [
        {"validTime": "2026-01-12T00:00:00+00:00/PT1H", "value": 15.5},
        {"validTime": "2026-01-12T01:00:00+00:00/PT1H", "value": 14.8}
      ]
    },
    "windSpeed": {
      "values": [
        {"validTime": "2026-01-12T00:00:00+00:00/PT1H", "value": 20.0}
      ]
    }
  }
}
```

**Output Structure** (multiple flattened rows):
| issue_time | valid_time | metric | value |
|------------|------------|--------|-------|
| 2026-01-12T00:00:00Z | 2026-01-12T00:00:00Z | temperature | 15.5 |
| 2026-01-12T00:00:00Z | 2026-01-12T01:00:00Z | temperature | 14.8 |
| 2026-01-12T00:00:00Z | 2026-01-12T00:00:00Z | wind_speed | 20.0 |

**Acceptance Criteria**:
- Each array element becomes a separate row
- Parent Bronze timestamp preserved as `issue_time`
- Array element timestamp extracted as `valid_time`
- Metric name included for filtering/pivoting

---

### FR-004: Dual Timestamp Extraction

**Requirement**: Each flattened row SHALL contain both `issue_time` (Bronze timestamp) and `valid_time` (from array validTime field).

**Rationale**: Forecast data requires both timestamps for:
- `issue_time`: When the forecast was issued (Bronze ingestion time)
- `valid_time`: When the forecast applies (forecast horizon)

**Acceptance Criteria**:
- `issue_time` extracted from Bronze `timestamp` field (microseconds since epoch)
- `valid_time` parsed from array element's `validTime` field (ISO 8601 duration format)
- Both timestamps stored as `TIMESTAMPTZ` in Silver

**Timestamp Parsing Rules**:
- ISO 8601 duration format: `"2026-01-12T00:00:00+00:00/PT1H"` extracts `2026-01-12T00:00:00+00:00`
- Duration component (`/PT1H`) indicates validity period, not used for timestamp extraction

---

### FR-005: Configuration-Driven Activation

**Requirement**: Pre-transform activation SHALL be driven by `silver_etl.pre_transform` section in stream config.

**Rationale**: Maintains consistency with DP-006 config-driven approach.

**Proposed Configuration Schema**:
```yaml
silver_etl:
  enabled: true
  target_table: silver.nws_forecasts

  pre_transform:
    enabled: true
    parser_type: column_oriented

    # Reference to existing parser config in same stream
    parser_config_ref: sources[0].parser

    # Or inline parser config (alternative)
    column_config:
      metrics_base_path: properties
      timestamp_format:
        type: iso8601_duration
      columns:
        - metric_path: temperature
          field_name: temperature_c
          unit: celsius
        # ... more columns

    # Output schema for flattened data
    output:
      issue_time_field: issue_time
      valid_time_field: valid_time
      metric_name_field: metric
      value_field: value
```

**Acceptance Criteria**:
- Configuration validates against defined schema
- Invalid config produces clear error messages
- Config hot-reload via etcd supported (future enhancement)

---

### FR-006: Passthrough Mode for Non-Array Streams

**Requirement**: Streams without `pre_transform` configuration SHALL continue processing with existing DuckDB-only pipeline.

**Rationale**: Existing streams (air-quality, outdoor-weather) must not be affected by this enhancement.

**Acceptance Criteria**:
- All existing silver-etl tests pass without modification
- Streams with `pre_transform: null` or missing section use standard pipeline
- No performance regression for non-pre-transform streams

---

### FR-007: Identity Field Preservation

**Requirement**: Pre-transform SHALL preserve identity fields from Bronze for propagation to Silver.

**Rationale**: Fields like `ndp_id`, `location_id`, and context information must flow through pre-transform.

**Preserved Fields**:
- `ndp_id` - NDP source identifier
- `location_id` - Extracted location (e.g., `ksgj_gridpoints`)
- Bronze `timestamp` - Becomes `issue_time`
- Context fields as configured

**Acceptance Criteria**:
- All identity fields from Bronze row propagated to every flattened row
- Context JSON preserved or selectively extracted per config

---

### FR-008: Unit Conversion Support

**Requirement**: Pre-transform SHALL apply unit conversions as configured in `column_config.unit_conversions`.

**Rationale**: NWS data uses WMO SI units which may need conversion for downstream applications.

**Example**:
```yaml
column_config:
  unit_conversions:
    wind_speed_kmh:
      from: km/h
      to: m/s
      factor: 0.277778
```

**Acceptance Criteria**:
- Existing `UnitConversion` types from neural-core supported
- Conversions applied during pre-transform, before DuckDB
- Conversion errors logged with context, row not dropped

---

### FR-009: Error Handling and Logging

**Requirement**: Pre-transform errors SHALL be logged with context and optionally skip invalid entries.

**Rationale**: Single invalid array elements should not fail entire Bronze row processing.

**Error Handling Strategy**:
| Error Type | Behavior | Example |
|------------|----------|---------|
| Missing metric | Skip metric, log warning | `"properties.temperature"` not found |
| Invalid timestamp | Skip entry, log warning | `"validTime": "invalid"` |
| Invalid value | Skip entry, log warning | `"value": null` or `"value": "N/A"` |
| Parser error | Propagate to ETL | Missing required config |

**Acceptance Criteria**:
- All errors logged with stream_id, metric, and entry index
- Invalid entries counted in ETL metrics
- Configurable behavior: skip vs. fail

---

### FR-010: Metric-Level Output Option

**Requirement**: Pre-transform SHALL support output as either wide format (one column per metric) or long format (metric as dimension).

**Rationale**: Different query patterns prefer different formats.

**Wide Format** (default for backward compatibility):
| issue_time | valid_time | temperature_c | wind_speed_kmh | humidity_pct |
|------------|------------|---------------|----------------|--------------|
| ... | ... | 15.5 | 20.0 | 65.0 |

**Long Format** (for flexibility):
| issue_time | valid_time | metric | value |
|------------|------------|--------|-------|
| ... | ... | temperature_c | 15.5 |
| ... | ... | wind_speed_kmh | 20.0 |
| ... | ... | humidity_pct | 65.0 |

**Acceptance Criteria**:
- Output format configurable via `pre_transform.output_format: wide|long`
- Wide format creates one temp table row per unique (issue_time, valid_time)
- Long format creates one row per metric per (issue_time, valid_time)

---

## 3. Non-Functional Requirements

### NFR-001: Performance - Array Explosion Throughput

**Requirement**: Pre-transform SHALL handle ~6000 points per Bronze row efficiently.

**Context**: Each NWS gridpoints response contains:
- ~40 metric columns (temperature, wind, precipitation, etc.)
- ~156 forecast hours per metric (7-day hourly forecast)
- Total: ~6,240 data points per Bronze row

**Performance Target**:
- Pre-transform execution: < 100ms per Bronze row
- Memory usage: < 50MB for 1000-row batch
- No buffering entire Bronze dataset in memory

**Acceptance Criteria**:
- Stream processing model (iterator, not collect-all)
- Benchmark test validates performance targets
- Memory profiling shows bounded growth

---

### NFR-002: Memory Efficiency

**Requirement**: Pre-transform SHALL use streaming/iterator patterns to avoid loading all data in memory.

**Rationale**: Edge deployment target (Raspberry Pi 5) has limited memory.

**Implementation Guidelines**:
- Use iterators for Bronze row processing
- Write to temp Parquet file in chunks if needed
- Clear intermediate data structures after processing

**Acceptance Criteria**:
- Maximum memory overhead per Bronze row: < 100KB
- Total process memory stays under 500MB during batch ETL

---

### NFR-003: Backward Compatibility

**Requirement**: Existing streams without `pre_transform` config MUST continue working without modification.

**Verification**:
- All existing silver-etl integration tests pass
- air-quality stream loads to Silver unchanged
- outdoor-weather stream loads to Silver unchanged

**Acceptance Criteria**:
- Zero changes to existing stream configs required
- API contract for `run_etl()` unchanged
- CLI interface unchanged

---

### NFR-004: Observability

**Requirement**: Pre-transform operations SHALL emit metrics and structured logs.

**Metrics**:
| Metric | Type | Description |
|--------|------|-------------|
| `etl_pretransform_rows_in` | Counter | Bronze rows processed |
| `etl_pretransform_rows_out` | Counter | Flattened rows produced |
| `etl_pretransform_errors` | Counter | Parse/transform errors |
| `etl_pretransform_duration_ms` | Histogram | Processing time |

**Logging**:
- INFO: Pre-transform start/end with row counts
- DEBUG: Per-metric extraction details
- WARN: Skipped entries with reason
- ERROR: Unrecoverable parse failures

---

### NFR-005: Testability

**Requirement**: Pre-transform logic SHALL be unit testable without DuckDB or TimescaleDB.

**Rationale**: Fast feedback loop during development.

**Test Strategy**:
- Unit tests for `ColumnOrientedParser` (already exist)
- Unit tests for pre-transform orchestration (new)
- Integration tests with mock Bronze Parquet data

---

## 4. Acceptance Criteria

### AC-001: NWS Gridpoints Loads Successfully

**Scenario**: Run silver-etl for nws-gridpoints-forecast with `enabled: true`

**Given**: Bronze Parquet files exist for nws-gridpoints-forecast
**And**: Pre-transform configuration is valid
**When**: silver-etl runs for nws-gridpoints-forecast
**Then**: Data loads to Silver `nws_forecasts` table
**And**: Row count matches expected (Bronze rows x ~6000 points)

---

### AC-002: Dual Timestamp Queryable

**Scenario**: Query forecast data by issue_time and valid_time

**Given**: NWS forecast data loaded to Silver
**When**: Execute query:
```sql
SELECT issue_time, valid_time, temperature_c
FROM silver.nws_forecasts
WHERE ndp_id = 'weather-nws-002'
  AND issue_time = '2026-01-12 12:00:00+00'
ORDER BY valid_time;
```
**Then**: Results show forecast horizon (valid_time > issue_time)
**And**: Each row has distinct valid_time
**And**: Temperature values are reasonable (-50 to 60 C)

---

### AC-003: Existing Tests Pass

**Scenario**: Verify backward compatibility

**Given**: Existing silver-etl test suite
**When**: Run `cargo test -p silver-etl`
**Then**: All tests pass
**And**: No deprecation warnings related to pre-transform

---

### AC-004: Pre-Transform Disableable

**Scenario**: Disable pre-transform for a stream

**Given**: nws-gridpoints-forecast with `pre_transform.enabled: false`
**When**: silver-etl attempts to process stream
**Then**: ETL skips pre-transform step
**And**: Logs indicate "pre-transform disabled"
**And**: Standard DuckDB pipeline executes (may fail for array data)

---

### AC-005: Configuration Validation

**Scenario**: Invalid pre-transform config rejected

**Given**: Pre-transform config with invalid `parser_type`
**When**: silver-etl loads configuration
**Then**: Clear error message indicates invalid config
**And**: ETL does not start

---

### AC-006: Error Resilience

**Scenario**: Handle malformed array entries gracefully

**Given**: Bronze data with one invalid `validTime` in array
**When**: Pre-transform processes the row
**Then**: Invalid entry skipped with warning log
**And**: Valid entries in same row processed successfully
**And**: ETL metrics reflect skipped count

---

## 5. Data Flow Diagram

```
+------------------------------------------------------------------+
|                        CURRENT FLOW                              |
|                   (Fails for Array Data)                         |
+------------------------------------------------------------------+

  +-------------------+       +-------------------+       +----------+
  | Bronze Parquet    |       | DuckDB            |       | Silver   |
  | - raw_payload     |  -->  | - json_extract    |  -->  | Table    |
  |   (JSON with      |       |   (FAILS: can't   |       |          |
  |    arrays)        |       |    handle arrays) |       |          |
  +-------------------+       +-------------------+       +----------+


+------------------------------------------------------------------+
|                     PROPOSED FLOW                                |
|                 (With Pre-Transform)                             |
+------------------------------------------------------------------+

  +-------------------+       +-------------------+       +------------------+
  | Bronze Parquet    |       | Rust Pre-         |       | Flattened Temp   |
  | - raw_payload     |  -->  | Transform         |  -->  | Parquet/DuckDB   |
  |   (JSON with      |       | (ColumnOriented   |       | Table            |
  |    arrays)        |       |  Parser)          |       |                  |
  +-------------------+       +-------------------+       +------------------+
                                     |
                                     | Extracts:
                                     | - issue_time (Bronze ts)
                                     | - valid_time (array ts)
                                     | - metric name
                                     | - value
                                     v
                              +-------------------+       +----------+
                              | DuckDB SQL        |       | Silver   |
                              | - Standard field  |  -->  | Table    |
                              |   selection       |       |          |
                              | - DQ rules        |       |          |
                              +-------------------+       +----------+


+------------------------------------------------------------------+
|                    DETAILED PRE-TRANSFORM                        |
+------------------------------------------------------------------+

Bronze Row (1 row):
+-----------------------------------------------------------------------+
| timestamp (issue) | ndp_id      | raw_payload                        |
|-------------------|-------------|-------------------------------------|
| 1736668800000000  | nws-002     | {"properties": {                    |
|                   |             |   "temperature": {"values": [       |
|                   |             |     {"validTime":"T00:00/PT1H",     |
|                   |             |      "value": 15.5},                |
|                   |             |     {"validTime":"T01:00/PT1H",     |
|                   |             |      "value": 14.8}                 |
|                   |             |   ]},                               |
|                   |             |   "windSpeed": {"values": [         |
|                   |             |     {"validTime":"T00:00/PT1H",     |
|                   |             |      "value": 20.0}                 |
|                   |             |   ]}                                |
|                   |             | }}                                  |
+-----------------------------------------------------------------------+

                              |
                              v  ColumnOrientedParser
                              v  (array explosion)
                              |

Flattened Output (3 rows):
+-----------------------------------------------------------------------+
| issue_time        | valid_time  | ndp_id  | metric      | value      |
|-------------------|-------------|---------|-------------|------------|
| 2026-01-12T00:00Z | 2026-01-12T00:00Z | nws-002 | temperature | 15.5 |
| 2026-01-12T00:00Z | 2026-01-12T01:00Z | nws-002 | temperature | 14.8 |
| 2026-01-12T00:00Z | 2026-01-12T00:00Z | nws-002 | wind_speed  | 20.0 |
+-----------------------------------------------------------------------+
```

---

## 6. Configuration Schema

### 6.1 Complete Pre-Transform Configuration

```yaml
# config/base/streams/nws-gridpoints-forecast/config.yaml

stream_id: nws-gridpoints-forecast
# ... existing Bronze config ...

silver_etl:
  enabled: true  # Enable Silver ETL
  target_table: silver.nws_forecasts

  # NEW: Pre-transform configuration
  pre_transform:
    enabled: true
    parser_type: column_oriented  # Currently only supported type

    # Column-oriented parser configuration
    # Can reference existing parser OR inline
    column_config:
      metrics_base_path: properties
      timestamp_format:
        type: iso8601_duration
      columns:
        - metric_path: temperature
          field_name: temperature_c
          unit: celsius
        - metric_path: windSpeed
          field_name: wind_speed_kmh
          unit: km/h
        - metric_path: probabilityOfPrecipitation
          field_name: precip_probability_pct
          unit: percent
        # ... additional columns

      # Optional unit conversions
      unit_conversions:
        wind_speed_ms:
          from: km/h
          to: m/s
          factor: 0.277778

    # Output configuration
    output:
      format: long  # wide | long
      issue_time_source: timestamp  # Bronze field for issue_time
      valid_time_source: validTime  # Array field for valid_time

    # Error handling
    on_error: skip  # skip | fail

  # Timestamp mapping (for DuckDB stage)
  timestamp:
    source_field: issue_time  # From pre-transform output
    target_field: issue_time
    transform: passthrough  # Already TIMESTAMPTZ

  # Additional timestamp for valid_time
  additional_timestamps:
    - source_field: valid_time
      target_field: valid_time
      transform: passthrough

  # Identity fields
  identity_fields:
    - source: ndp_id
      target: ndp_id
    - source: metric
      target: metric_name

  # Field mappings (for value extraction)
  field_mappings:
    - source_path: value
      target_column: value
      type: double_precision
      nullable: true

  # DQ rules
  dq_rules:
    - rule: cross_field_check
      name: valid_time_after_issue
      expression: "valid_time >= issue_time"
      message: "valid_time_before_issue"
      action: flag

  # Deduplication
  deduplication:
    enabled: true
    key_columns: [issue_time, valid_time, ndp_id, metric_name]
    strategy: upsert

  # Incremental loading
  incremental:
    enabled: true
    watermark_column: issue_time
    lag_interval: 1 hour
```

### 6.2 Rust Configuration Types

```rust
// core/src/config/silver_etl.rs (extension)

/// Pre-transform configuration for columnar array data
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct PreTransformConfig {
    /// Whether pre-transform is enabled
    pub enabled: bool,

    /// Parser type to use (currently only "column_oriented")
    pub parser_type: PreTransformParserType,

    /// Column-oriented parser configuration
    #[serde(default)]
    pub column_config: Option<ColumnOrientedConfig>,

    /// Output format configuration
    #[serde(default)]
    pub output: PreTransformOutput,

    /// Error handling behavior
    #[serde(default)]
    pub on_error: ErrorBehavior,
}

/// Supported pre-transform parser types
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum PreTransformParserType {
    ColumnOriented,
    // Future: ArrayOriented, Custom, etc.
}

/// Pre-transform output configuration
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct PreTransformOutput {
    /// Output format: wide or long
    #[serde(default)]
    pub format: OutputFormat,

    /// Bronze field to use as issue_time
    #[serde(default = "default_issue_time_source")]
    pub issue_time_source: String,

    /// Array field to use as valid_time
    #[serde(default = "default_valid_time_source")]
    pub valid_time_source: String,
}

/// Output format for pre-transformed data
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    /// One column per metric (wide pivot table)
    Wide,
    /// Metric as dimension (EAV model)
    #[default]
    Long,
}

/// Error handling behavior
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ErrorBehavior {
    /// Skip invalid entries, continue processing
    #[default]
    Skip,
    /// Fail ETL on any error
    Fail,
}

fn default_issue_time_source() -> String {
    "timestamp".to_string()
}

fn default_valid_time_source() -> String {
    "validTime".to_string()
}
```

---

## 7. Interface Contracts

### 7.1 Pre-Transform Trait

```rust
/// Trait for pre-transform implementations
pub trait PreTransform: Send + Sync {
    /// Transform a single Bronze row into multiple flattened rows
    fn transform(
        &self,
        bronze_row: &BronzeRow,
    ) -> Result<Vec<FlattenedRow>, PreTransformError>;

    /// Get the output schema for flattened data
    fn output_schema(&self) -> Vec<ColumnDef>;

    /// Get configuration for debugging
    fn config(&self) -> &PreTransformConfig;
}

/// Bronze row representation
pub struct BronzeRow {
    pub timestamp: i64,          // Microseconds since epoch
    pub ndp_id: String,
    pub source_id: String,
    pub context: Option<Value>,
    pub raw_payload: Value,      // JSON payload
}

/// Flattened output row
pub struct FlattenedRow {
    pub issue_time: DateTime<Utc>,
    pub valid_time: DateTime<Utc>,
    pub ndp_id: String,
    pub metric: String,
    pub value: f64,
    pub tags: HashMap<String, String>,
}
```

### 7.2 Integration with EtlRunner

```rust
impl EtlRunner {
    /// Run ETL with optional pre-transform
    pub fn run_etl(
        &self,
        config: &SilverEtlConfig,
        stream_id: &str,
        bronze_path: &str,
    ) -> Result<EtlStats, EtlError> {
        // Check if pre-transform is enabled
        if let Some(ref pt_config) = config.pre_transform {
            if pt_config.enabled {
                return self.run_etl_with_pretransform(
                    config, pt_config, stream_id, bronze_path
                );
            }
        }

        // Existing path: direct DuckDB SQL
        self.run_etl_standard(config, stream_id, bronze_path)
    }

    /// ETL with pre-transform stage
    fn run_etl_with_pretransform(
        &self,
        config: &SilverEtlConfig,
        pt_config: &PreTransformConfig,
        stream_id: &str,
        bronze_path: &str,
    ) -> Result<EtlStats, EtlError> {
        // 1. Read Bronze Parquet files
        // 2. For each row, apply pre-transform
        // 3. Write flattened data to temp Parquet
        // 4. Execute DuckDB SQL on temp Parquet
        // 5. Return combined stats
    }
}
```

---

## 8. Dependencies

### Internal Dependencies

| Dependency | Purpose | Status |
|------------|---------|--------|
| `neural-core::parsers::column_oriented` | Parser implementation | Exists |
| `neural-core::parsers::config` | Parser configuration types | Exists |
| `neural-core::config::silver_etl` | SilverEtlConfig | Extend |
| `silver-etl::etl` | EtlRunner | Modify |

### External Dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `chrono` | 0.4 | Timestamp parsing |
| `serde_json` | 1.0 | JSON processing |
| `polars` | 0.35+ | Parquet read/write |
| `tracing` | 0.1 | Logging/metrics |

---

## 9. Risks and Mitigations

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Parser performance insufficient | High | Low | Already handles similar data in Bronze ingestion |
| Memory exhaustion on Pi | High | Medium | Implement streaming/iterator pattern |
| Breaking changes to existing streams | High | Low | Comprehensive backward compatibility tests |
| Complex config schema | Medium | Medium | Provide sensible defaults, clear docs |
| DuckDB temp table limitations | Medium | Low | Use temp Parquet file instead of in-memory table |

---

## 10. Out of Scope

The following are explicitly out of scope for DP-007:

1. **New parser implementations** - Only `ColumnOrientedParser` integration
2. **Changes to Bronze layer** - Bronze continues storing raw JSON
3. **Schema evolution** - No automatic schema migration
4. **Real-time streaming** - Batch ETL only
5. **Multi-source merges** - Single Bronze source per stream
6. **Custom expression language** - Use existing parser capabilities

---

## 11. References

### Code References

- **ColumnOrientedParser**: `/workspaces/neural-data-platform/core/src/parsers/column_oriented.rs`
- **Parser Config**: `/workspaces/neural-data-platform/core/src/parsers/config.rs`
- **SilverEtlConfig**: `/workspaces/neural-data-platform/core/src/config/silver_etl.rs`
- **EtlRunner**: `/workspaces/neural-data-platform/apps/silver-etl/src/etl.rs`
- **NWS Stream Config**: `/workspaces/neural-data-platform/config/base/streams/nws-gridpoints-forecast/config.yaml`

### Documentation References

- **SCOPE.md**: `/workspaces/neural-data-platform/product/features/dp-007/SCOPE.md`
- **CONFIG_DRIVEN_SILVER_ETL_DESIGN**: `/workspaces/neural-data-platform/docs/architecture/CONFIG_DRIVEN_SILVER_ETL_DESIGN.md`
- **DP-006 SPARC Specification**: `/workspaces/neural-data-platform/product/features/dp-006/specification/`

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-12 | NDP Architect | Initial specification |

---

## Approval

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Architect | NDP Architect | 2026-01-12 | Draft |
| Tech Lead | - | - | Pending |
| Product Owner | - | - | Pending |
