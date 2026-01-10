# Config-Driven Silver ETL Design

**Document**: CONFIG_DRIVEN_SILVER_ETL_DESIGN.md
**Version**: 1.0
**Date**: 2026-01-05
**Author**: NDP Architect
**Status**: Proposed
**Feature**: DP-006 (Silver Layer Implementation)

---

## Executive Summary

This document proposes extending NDP's existing config-driven patterns from Bronze ingestion to Silver ETL. By reusing the established configuration infrastructure (YAML files, etcd GitOps, field mappings, unit conversions), we can create a unified configuration schema that drives Bronze-to-Silver transformation with minimal new code.

### Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| **Extend existing config** | Reuse `field_mappings`, `UnitConversion` already proven in parsers |
| **YAML-driven transforms** | Same GitOps workflow, hot-reload via etcd |
| **DQ rules in config** | Transparent, auditable validation pipeline |
| **Silver schema in etcd** | Enables hot-reload schema evolution |

---

## 1. Current Bronze Config Patterns

### 1.1 Existing Field Mapping (Parser Config)

The Bronze layer already uses field mappings for the `Source` trait (Silver extraction):

```yaml
# config/base/streams/outdoor-weather/config.yaml
parser:
  parser_type: json_path
  field_mappings:
    - path: main.temp           # Source JSON path
      metric_name: temperature  # Target field name
      unit: celsius             # Expected unit
      transform: null           # Optional transform (unused today)
```

**Current `FieldMapping` struct** (`core/src/parsers/config.rs`):

```rust
pub struct FieldMapping {
    pub path: String,           // JSON path to extract
    pub metric_name: String,    // Output field name
    pub unit: Option<String>,   // Unit identifier
    pub transform: Option<String>, // Transform name (unused)
}
```

### 1.2 Existing Unit Conversion

The `UnitConversion` and `ConversionFormula` types exist but are underutilized:

```rust
// core/src/parsers/config.rs
pub struct UnitConversion {
    pub from: String,                    // e.g., "kelvin"
    pub to: String,                      // e.g., "celsius"
    pub factor: Option<f64>,             // Simple multiplication
    pub formula: Option<ConversionFormula>, // Complex conversion
}

pub enum ConversionFormula {
    Linear { scale: f64, offset: f64 },  // (value * scale) + offset
    Custom { code: String },             // Future: expression evaluation
}
```

### 1.3 Existing DQ in Schema Fields

Basic range validation exists in stream config `fields`:

```yaml
fields:
  - name: temperature
    type: float
    nullable: false
    unit: celsius
    range: [-50.0, 60.0]  # Validation range
```

### 1.4 Configuration Hierarchy

```
Priority 1: Stream Registry (/streams/{id}/config in etcd)
Priority 2: Legacy etcd (/config/{app}/*)
Priority 3: YAML files (config/*.yaml)
Priority 4: Code defaults
```

---

## 2. Proposed Silver ETL Config Schema

### 2.1 New Config Section: `silver_etl`

Extend each stream config with a `silver_etl` section that defines the transformation to Silver:

```yaml
# config/base/streams/outdoor-weather/config.yaml (extended)

stream_id: outdoor-weather
# ... existing Bronze config ...

# NEW: Silver layer ETL configuration
silver_etl:
  enabled: true
  target_table: silver.outdoor_weather
  target_schema: outdoor_weather_v1

  # Timestamp handling
  timestamp:
    source_field: timestamp          # Bronze column (microseconds)
    target_field: observation_time   # Silver column (TIMESTAMPTZ)
    transform: microseconds_to_timestamp

  # Identity fields (passthrough)
  identity_fields:
    - source: ndp_id
      target: ndp_id
    - source: context.source_type.provider
      target: source_provider

  # Field mappings with transforms and DQ rules
  field_mappings:
    - source_path: raw_payload.main.temp
      target_column: temperature_c
      type: double_precision
      nullable: false
      transform:
        type: unit_conversion
        from: kelvin
        to: celsius
        formula:
          type: linear
          scale: 1.0
          offset: -273.15
      dq_rules:
        - rule: range_check
          min: -50.0
          max: 60.0
          action: flag  # flag | reject | clamp

    - source_path: raw_payload.main.pressure
      target_column: pressure_pa
      type: double_precision
      nullable: true
      transform:
        type: unit_conversion
        from: hpa
        to: pa
        formula:
          type: linear
          scale: 100.0
          offset: 0.0
      dq_rules:
        - rule: range_check
          min: 80000
          max: 120000
          action: flag

    - source_path: raw_payload.wind.speed
      target_column: wind_speed_kmh
      type: double_precision
      nullable: true
      transform:
        type: unit_conversion
        from: m_s
        to: km_h
        formula:
          type: linear
          scale: 3.6
          offset: 0.0
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 300.0
          action: flag

    - source_path: raw_payload.main.humidity
      target_column: humidity_pct
      type: double_precision
      nullable: true
      transform: null  # No conversion needed
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 100.0
          action: clamp

  # DQ transparency output
  dq_output:
    enabled: true
    target_column: dq_flags
    include_rules: true      # Include rule names in flags
    include_values: false    # Don't include raw values (privacy)

  # Deduplication strategy
  deduplication:
    enabled: true
    key_columns: [observation_time, ndp_id]
    strategy: upsert  # upsert | skip | replace

  # Incremental load config
  incremental:
    enabled: true
    watermark_column: observation_time
    lag_interval: 5 minutes  # Safety buffer for late arrivals
```

### 2.2 Complete Example: Air Quality Stream

```yaml
# config/base/streams/air-quality/config.yaml (extended)

stream_id: air-quality
description: "AirGradient sensor readings from MQTT"
version: "1.0.0"
enabled: true

# ... existing Bronze config (sources, parser, storage) ...

silver_etl:
  enabled: true
  target_table: silver.air_quality_observations
  target_schema: air_quality_observations_v1

  timestamp:
    source_field: timestamp
    target_field: observation_time
    transform: microseconds_to_timestamp

  identity_fields:
    - source: ndp_id
      target: ndp_id
    - source: context.location.path
      target: location_path
    - source: context.location.type
      target: location_type
    - source: raw_payload.serialno
      target: device_serial
    - source: raw_payload.model
      target: device_model
    - source: raw_payload.firmware
      target: firmware_version

  field_mappings:
    # CO2 - integer sensor value
    - source_path: raw_payload.rco2
      target_column: co2
      type: smallint
      nullable: true
      transform: null
      dq_rules:
        - rule: range_check
          min: 380
          max: 10000
          action: flag

    # PM2.5 - primary metric
    - source_path: raw_payload.pm02
      target_column: pm25
      type: double_precision
      nullable: false
      transform: null
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 1000.0
          action: flag

    # PM2.5 Compensated (preferred)
    - source_path: raw_payload.pm02Compensated
      target_column: pm25_compensated
      type: double_precision
      nullable: true
      transform: null
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 1000.0
          action: flag

    # PM10
    - source_path: raw_payload.pm10
      target_column: pm10
      type: double_precision
      nullable: true
      transform: null
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 2000.0
          action: flag

    # Temperature - direct copy (already Celsius)
    - source_path: raw_payload.atmp
      target_column: temperature_c
      type: double_precision
      nullable: true
      transform: null
      dq_rules:
        - rule: range_check
          min: -40.0
          max: 85.0
          action: flag

    # Temperature Compensated (preferred)
    - source_path: raw_payload.atmpCompensated
      target_column: temperature_c_compensated
      type: double_precision
      nullable: true
      transform: null
      dq_rules:
        - rule: range_check
          min: -40.0
          max: 85.0
          action: flag

    # Humidity
    - source_path: raw_payload.rhum
      target_column: humidity_pct
      type: double_precision
      nullable: true
      transform: null
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 100.0
          action: clamp

    # Humidity Compensated
    - source_path: raw_payload.rhumCompensated
      target_column: humidity_pct_compensated
      type: double_precision
      nullable: true
      transform: null
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 100.0
          action: clamp

    # TVOC Index
    - source_path: raw_payload.tvocIndex
      target_column: tvoc_index
      type: smallint
      nullable: true
      transform: null
      dq_rules:
        - rule: range_check
          min: 1
          max: 500
          action: clamp

    # NOx Index
    - source_path: raw_payload.noxIndex
      target_column: nox_index
      type: smallint
      nullable: true
      transform: null
      dq_rules:
        - rule: range_check
          min: 1
          max: 500
          action: clamp

    # WiFi Signal (diagnostic)
    - source_path: raw_payload.wifi
      target_column: wifi_signal_dbm
      type: smallint
      nullable: true
      transform: null
      dq_rules:
        - rule: range_check
          min: -100
          max: 0
          action: flag

  dq_output:
    enabled: true
    target_column: dq_flags
    include_rules: true
    include_values: false

  deduplication:
    enabled: true
    key_columns: [observation_time, ndp_id]
    strategy: upsert

  incremental:
    enabled: true
    watermark_column: observation_time
    lag_interval: 5 minutes
```

---

## 3. Transform Definition System

### 3.1 Transform Types

```yaml
# Type 1: Unit Conversion (reuses existing UnitConversion)
transform:
  type: unit_conversion
  from: kelvin
  to: celsius
  formula:
    type: linear
    scale: 1.0
    offset: -273.15

# Type 2: Expression (future enhancement)
transform:
  type: expression
  expr: "(value - 32) * 5 / 9"  # Fahrenheit to Celsius

# Type 3: Lookup (for categorical mappings)
transform:
  type: lookup
  table:
    "1": "Good"
    "2": "Fair"
    "3": "Moderate"
    "4": "Poor"
    "5": "Very Poor"

# Type 4: JSON Extract (for nested payloads)
transform:
  type: json_extract
  path: "$.list[0].main.aqi"

# Type 5: Timestamp (various formats)
transform:
  type: timestamp
  format: microseconds_to_timestamp  # or iso8601, unix_seconds, nws_duration

# Type 6: Computed (references other columns)
transform:
  type: computed
  depends_on: [issue_time, valid_time]
  expr: "EXTRACT(EPOCH FROM valid_time - issue_time) / 3600"
```

### 3.2 Rust Config Types (Proposed Extension)

```rust
// core/src/config/silver_etl.rs

use serde::{Deserialize, Serialize};

/// Silver ETL configuration for a stream
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SilverEtlConfig {
    pub enabled: bool,
    pub target_table: String,
    pub target_schema: String,
    pub timestamp: TimestampMapping,
    #[serde(default)]
    pub identity_fields: Vec<IdentityField>,
    pub field_mappings: Vec<SilverFieldMapping>,
    #[serde(default)]
    pub dq_output: DqOutputConfig,
    #[serde(default)]
    pub deduplication: DeduplicationConfig,
    #[serde(default)]
    pub incremental: IncrementalConfig,
}

/// Mapping for timestamp field
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TimestampMapping {
    pub source_field: String,
    pub target_field: String,
    pub transform: TimestampTransform,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TimestampTransform {
    MicrosecondsToTimestamp,
    Iso8601,
    UnixSeconds,
    NwsDuration,
}

/// Identity field passthrough
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IdentityField {
    pub source: String,  // JSON path in Bronze
    pub target: String,  // Column name in Silver
}

/// Complete field mapping for Silver ETL
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SilverFieldMapping {
    pub source_path: String,
    pub target_column: String,
    pub type_name: String,  // PostgreSQL type
    #[serde(default)]
    pub nullable: bool,
    #[serde(default)]
    pub transform: Option<TransformConfig>,
    #[serde(default)]
    pub dq_rules: Vec<DqRule>,
}

/// Transform configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TransformConfig {
    UnitConversion {
        from: String,
        to: String,
        formula: ConversionFormula,
    },
    Expression {
        expr: String,
    },
    Lookup {
        table: std::collections::HashMap<String, String>,
    },
    JsonExtract {
        path: String,
    },
    Timestamp {
        format: TimestampTransform,
    },
    Computed {
        depends_on: Vec<String>,
        expr: String,
    },
}

/// Reuse existing ConversionFormula
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConversionFormula {
    Linear { scale: f64, offset: f64 },
    Custom { code: String },
}

/// Data quality rule
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "rule", rename_all = "snake_case")]
pub enum DqRule {
    RangeCheck {
        min: f64,
        max: f64,
        action: DqAction,
    },
    NotNull {
        action: DqAction,
    },
    Pattern {
        regex: String,
        action: DqAction,
    },
    OneOf {
        values: Vec<String>,
        action: DqAction,
    },
    Custom {
        name: String,
        expr: String,
        action: DqAction,
    },
}

/// Action to take when DQ rule triggers
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DqAction {
    Flag,    // Add to dq_flags array, keep value
    Reject,  // Set to NULL, add to dq_flags
    Clamp,   // Clamp to min/max, add to dq_flags
    Drop,    // Drop entire row
}

/// DQ output configuration
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct DqOutputConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_dq_column")]
    pub target_column: String,
    #[serde(default)]
    pub include_rules: bool,
    #[serde(default)]
    pub include_values: bool,
}

fn default_dq_column() -> String {
    "dq_flags".to_string()
}

/// Deduplication configuration
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct DeduplicationConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub key_columns: Vec<String>,
    #[serde(default)]
    pub strategy: DeduplicationStrategy,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DeduplicationStrategy {
    #[default]
    Upsert,
    Skip,
    Replace,
}

/// Incremental load configuration
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct IncrementalConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub watermark_column: String,
    #[serde(default)]
    pub lag_interval: String,  // e.g., "5 minutes"
}
```

---

## 4. DQ Rules in Config

### 4.1 Supported Rule Types

| Rule Type | Parameters | Action | Example |
|-----------|------------|--------|---------|
| `range_check` | `min`, `max` | flag/reject/clamp | Temperature -50 to 60 |
| `not_null` | - | flag/reject | Primary keys |
| `pattern` | `regex` | flag/reject | Serial number format |
| `one_of` | `values[]` | flag/reject | Status codes |
| `custom` | `name`, `expr` | flag/reject | Complex validation |

### 4.2 DQ Output Format

When `dq_output.enabled: true`, violations are collected:

```sql
-- Example dq_flags array content
['range_check:temperature_c:exceeded_max',
 'range_check:humidity_pct:clamped']
```

If `include_values: true` (not recommended for privacy):

```sql
['range_check:temperature_c:exceeded_max:65.2',
 'range_check:humidity_pct:clamped:105.0->100.0']
```

### 4.3 DQ Transparency Table (Optional)

For full audit trail, a separate DQ events table:

```yaml
# config/silver/dq_transparency.yaml
dq_transparency:
  enabled: true
  target_table: silver.dq_events
  retention_days: 90
  fields:
    - stream_id
    - observation_time
    - rule_name
    - column_name
    - original_value
    - action_taken
    - result_value
```

---

## 5. etcd-Driven Silver Schema

### 5.1 Schema Storage in etcd

Silver table schemas can be stored in etcd for hot-reload:

```
/silver/schemas/outdoor_weather_v1
/silver/schemas/air_quality_observations_v1
/silver/schemas/weather_forecasts_v1
```

### 5.2 Schema Definition Format

```yaml
# In etcd: /silver/schemas/outdoor_weather_v1
schema_name: outdoor_weather_v1
version: "1.0.0"
description: "Outdoor weather observations"
table_name: silver.outdoor_weather

columns:
  - name: ingestion_time
    type: timestamptz
    nullable: false
    default: now()
    description: "When row was inserted"

  - name: observation_time
    type: timestamptz
    nullable: false
    description: "When observation was taken"

  - name: ndp_id
    type: text
    nullable: false
    description: "NDP source identifier"

  - name: source_provider
    type: text
    nullable: true
    description: "Data provider (nws, owm)"

  - name: temperature_c
    type: double_precision
    nullable: false
    unit: celsius
    description: "Air temperature"

  - name: humidity_pct
    type: double_precision
    nullable: true
    unit: percent
    description: "Relative humidity"

  # ... more columns ...

  - name: dq_flags
    type: text[]
    nullable: true
    description: "Data quality violation flags"

primary_key: [observation_time, ndp_id]

hypertable:
  time_column: observation_time
  chunk_interval: 1 day

indexes:
  - name: idx_weather_ndp
    columns: [ndp_id, observation_time DESC]
  - name: idx_weather_provider
    columns: [source_provider, observation_time DESC]

retention_policy:
  enabled: true
  interval: 90 days
```

### 5.3 Schema Migration Workflow

```
1. Update schema YAML in etcd
2. ETL detects version change via watch
3. Generate migration SQL (ALTER TABLE)
4. Apply migration in transaction
5. Update schema version marker
6. Resume ETL with new schema
```

### 5.4 Version Management

```yaml
# Schema evolution tracking
/silver/migrations/outdoor_weather/
  - 001_initial.sql
  - 002_add_wind_gust.sql
  - 003_add_dq_flags.sql

/silver/versions/outdoor_weather
  - current: "1.0.3"
  - history:
    - version: "1.0.0"
      applied_at: "2026-01-01T00:00:00Z"
    - version: "1.0.1"
      applied_at: "2026-01-05T00:00:00Z"
      migration: "002_add_wind_gust.sql"
```

---

## 6. Generated DuckDB ETL SQL

### 6.1 SQL Generation from Config

The config can generate DuckDB SQL for execution:

```sql
-- Generated from config/base/streams/outdoor-weather/config.yaml
-- silver_etl section

INSERT INTO pg.silver.outdoor_weather (
    ingestion_time,
    observation_time,
    ndp_id,
    source_provider,
    temperature_c,
    humidity_pct,
    pressure_pa,
    wind_speed_kmh,
    dq_flags
)
WITH bronze_data AS (
    SELECT
        timestamp,
        ndp_id,
        json_extract(context, '$.source_type.provider') as source_provider,
        raw_payload
    FROM read_parquet('/data/raw/outdoor-weather/**/*.parquet')
    WHERE to_timestamp(timestamp / 1000000) > (
        SELECT COALESCE(MAX(observation_time), '1970-01-01'::TIMESTAMP)
        FROM pg.silver.outdoor_weather
    )
    AND to_timestamp(timestamp / 1000000) <= current_timestamp - INTERVAL '5 minutes'
),
transformed AS (
    SELECT
        current_timestamp as ingestion_time,
        to_timestamp(timestamp / 1000000) as observation_time,
        ndp_id,
        source_provider::TEXT,

        -- temperature_c: unit_conversion kelvin -> celsius, range_check -50 to 60
        CASE
            WHEN (json_extract(raw_payload, '$.main.temp')::FLOAT - 273.15)
                 NOT BETWEEN -50.0 AND 60.0
            THEN NULL
            ELSE json_extract(raw_payload, '$.main.temp')::FLOAT - 273.15
        END as temperature_c,

        -- humidity_pct: clamp 0-100
        LEAST(GREATEST(
            json_extract(raw_payload, '$.main.humidity')::FLOAT,
            0.0
        ), 100.0) as humidity_pct,

        -- pressure_pa: unit_conversion hpa -> pa
        json_extract(raw_payload, '$.main.pressure')::FLOAT * 100.0 as pressure_pa,

        -- wind_speed_kmh: unit_conversion m/s -> km/h
        json_extract(raw_payload, '$.wind.speed')::FLOAT * 3.6 as wind_speed_kmh,

        -- DQ flags collection
        ARRAY_AGG(
            CASE
                WHEN (json_extract(raw_payload, '$.main.temp')::FLOAT - 273.15)
                     NOT BETWEEN -50.0 AND 60.0
                THEN 'range_check:temperature_c:out_of_range'
            END
        ) FILTER (WHERE CASE
            WHEN (json_extract(raw_payload, '$.main.temp')::FLOAT - 273.15)
                 NOT BETWEEN -50.0 AND 60.0
            THEN TRUE ELSE FALSE END
        ) as dq_flags

    FROM bronze_data
    GROUP BY 1, 2, 3, 4, 5, 6, 7, 8
)
SELECT * FROM transformed
ON CONFLICT (observation_time, ndp_id) DO UPDATE SET
    ingestion_time = EXCLUDED.ingestion_time,
    source_provider = EXCLUDED.source_provider,
    temperature_c = EXCLUDED.temperature_c,
    humidity_pct = EXCLUDED.humidity_pct,
    pressure_pa = EXCLUDED.pressure_pa,
    wind_speed_kmh = EXCLUDED.wind_speed_kmh,
    dq_flags = EXCLUDED.dq_flags;
```

### 6.2 SQL Generator Trait

```rust
// core/src/etl/sql_generator.rs

pub trait SqlGenerator {
    fn generate_etl_sql(&self, config: &SilverEtlConfig) -> Result<String, CoreError>;
    fn generate_schema_ddl(&self, schema: &SilverSchema) -> Result<String, CoreError>;
    fn generate_dq_check(&self, rule: &DqRule, source_path: &str) -> String;
    fn generate_transform(&self, transform: &TransformConfig, source_path: &str) -> String;
}

pub struct DuckDbSqlGenerator;

impl SqlGenerator for DuckDbSqlGenerator {
    fn generate_transform(&self, transform: &TransformConfig, source_path: &str) -> String {
        match transform {
            TransformConfig::UnitConversion { from, to, formula } => {
                match formula {
                    ConversionFormula::Linear { scale, offset } => {
                        format!(
                            "(json_extract(raw_payload, '$.{}')::FLOAT * {} + {})",
                            source_path, scale, offset
                        )
                    }
                    _ => unimplemented!()
                }
            }
            TransformConfig::Timestamp { format } => {
                match format {
                    TimestampTransform::MicrosecondsToTimestamp => {
                        format!("to_timestamp({} / 1000000)", source_path)
                    }
                    _ => unimplemented!()
                }
            }
            _ => unimplemented!()
        }
    }

    // ... other implementations
}
```

---

## 7. Reusing Existing Patterns

### 7.1 Bronze Parser Config -> Silver ETL Config

| Bronze Config | Silver Config | Relationship |
|---------------|---------------|--------------|
| `parser.field_mappings[].path` | `field_mappings[].source_path` | Extend with full JSON path |
| `parser.field_mappings[].metric_name` | `field_mappings[].target_column` | Rename for clarity |
| `parser.field_mappings[].unit` | `field_mappings[].transform.to` | Make conversion explicit |
| `fields[].range` | `dq_rules[].range_check` | Move to DQ rules |
| N/A (new) | `dq_rules[].action` | Add action specification |
| N/A (new) | `dq_output` | Add transparency config |

### 7.2 Existing Types to Reuse

| Type | Location | Reuse For |
|------|----------|-----------|
| `FieldMapping` | `parsers/config.rs` | Extend for Silver |
| `UnitConversion` | `parsers/config.rs` | Transform definitions |
| `ConversionFormula` | `parsers/config.rs` | Linear/custom formulas |
| `SchemaField` | stream config | DQ range source |
| `StreamConfig` | `config.rs` | Add `silver_etl` field |

### 7.3 Backward Compatibility

The `silver_etl` section is optional:
- Streams without it continue Bronze-only operation
- Existing Bronze behavior unchanged
- Silver ETL activated when `silver_etl.enabled: true`

---

## 8. Implementation Roadmap

### Phase 1: Config Schema Definition (Week 1)

| Task | Effort | Output |
|------|--------|--------|
| Define Rust types for SilverEtlConfig | 4h | `core/src/config/silver_etl.rs` |
| Add to StreamConfig (optional field) | 2h | Updated `StreamConfig` |
| Write serde tests | 2h | Unit tests |
| Update config-client for Silver configs | 4h | etcd support |

### Phase 2: SQL Generator (Week 2)

| Task | Effort | Output |
|------|--------|--------|
| Implement DuckDbSqlGenerator trait | 8h | SQL generation |
| Unit tests for transform generation | 4h | Test coverage |
| DQ rule SQL generation | 4h | Validation SQL |
| Integration test with sample config | 4h | E2E test |

### Phase 3: ETL Orchestration (Week 3)

| Task | Effort | Output |
|------|--------|--------|
| Create ETL runner script | 4h | `etl/run_silver_etl.sh` |
| Systemd timer integration | 2h | Service files |
| Config hot-reload via etcd watch | 4h | Dynamic updates |
| Monitoring/alerting | 4h | Grafana dashboard |

### Phase 4: Schema Evolution (Week 4)

| Task | Effort | Output |
|------|--------|--------|
| Schema storage in etcd | 4h | Schema registry |
| Migration generator | 8h | ALTER TABLE SQL |
| Version tracking | 4h | Migration history |
| Documentation | 4h | Updated docs |

---

## 9. Alternatives Considered

### 9.1 Separate Silver Config Files

**Rejected**: Creating separate `config/silver/` directory duplicates stream definitions.

### 9.2 Pure SQL Templates

**Rejected**: SQL templates without config are harder to validate and maintain.

### 9.3 dbt-style YAML

**Considered**: dbt's schema.yml pattern is well-known but introduces new tooling.

**Decision**: Extend existing config pattern for consistency.

---

## 10. Open Questions

| Question | Status | Notes |
|----------|--------|-------|
| Should DQ transparency table be separate? | Open | Consider storage impact |
| How to handle schema evolution? | Proposed | etcd-driven migrations |
| Expression evaluation runtime? | Future | Consider rhai or similar |
| Multi-source merges (N:1)? | Deferred | Handle in separate config section |

---

## Appendix A: Complete Config Example

See full examples in:
- `/config/base/streams/outdoor-weather/config.yaml` (after extension)
- `/config/base/streams/air-quality/config.yaml` (after extension)

## Appendix B: Generated SQL Examples

See generated SQL templates in:
- `/etl/templates/outdoor_weather.sql`
- `/etl/templates/air_quality.sql`

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-05 | NDP Architect | Initial design |

---

## References

1. Existing parser config: `core/src/parsers/config.rs`
2. Stream config patterns: `config/base/streams/*/config.yaml`
3. Silver research: `research/agenticdataplatform/silver/`
4. Data dictionary: `research/agenticdataplatform/silver/03-data-dictionary.md`
5. ETL alternatives: `research/agenticdataplatform/silver/02-etl-alternatives.md`
