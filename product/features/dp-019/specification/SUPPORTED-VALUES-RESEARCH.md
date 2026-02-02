# NDP Supported Values Research

> **Feature:** dp-019 Config Validation Pipeline
> **Author:** Research Agent
> **Date:** 2026-02-02
> **Status:** Complete

## Executive Summary

This document catalogs all valid values for NDP stream configuration fields, derived from analysis of the Rust codebase (`core/src/`), JSON schema (`schemas/stream-config.v1.1.schema.json`), and existing stream configurations. These values form the foundation for the dp-019 config validation pipeline.

---

## 1. Field Types (`fields[].type`)

### Rust Enum: `FieldType`
**Location:** `/workspaces/neural-data-platform/core/src/types/stream_config.rs:31-39`

| Value | Rust Variant | Description | Supports Range | Supports Precision |
|-------|-------------|-------------|----------------|-------------------|
| `float` | `FieldType::Float` | Floating-point numeric | Yes | Yes |
| `int` | `FieldType::Int` | Integer numeric | Yes | No |
| `string` | `FieldType::String` | Text/string | No | No |
| `bool` | `FieldType::Bool` | Boolean true/false | No | No |
| `json` | `FieldType::Json` | Nested JSON object | No | No |

### JSON Schema Enum
**Location:** `/workspaces/neural-data-platform/schemas/stream-config.v1.1.schema.json:97-100`
```json
"enum": ["float", "int", "string", "bool", "json"]
```

### Validation Rules (from Rust)
- `string`, `bool`, `json`: Cannot have `range` or `display_precision`
- `int`: Cannot have `display_precision`
- `float`: Can have both `range` and `display_precision`

---

## 2. Source Types (`sources[].type`)

### Rust Enum: `SourceType`
**Location:** `/workspaces/neural-data-platform/core/src/types/stream_config.rs:183-191`

| Value | Rust Variant | Description | Required Params |
|-------|-------------|-------------|-----------------|
| `mqtt` | `SourceType::Mqtt` | MQTT subscription | `broker_url` |
| `http_poll` | `SourceType::HttpPoll` | HTTP polling | `poll_interval_secs` |
| `webhook` | `SourceType::Webhook` | HTTP push/webhook | - |
| `file_watch` | `SourceType::FileWatch` | File system watcher | - |
| `csv` | `SourceType::Csv` | CSV file source (dp-013) | `path`, `timestamp_field` |

### JSON Schema Enum
**Location:** `/workspaces/neural-data-platform/schemas/stream-config.v1.1.schema.json:141-144`
```json
"enum": ["mqtt", "http_poll", "http_push", "file_watch"]
```

**Note:** JSON schema uses `http_push` while Rust uses `Webhook`. CSV is defined in Rust but not yet in JSON schema.

---

## 3. Parser Types (`sources[].parser.parser_type`)

### Rust Enum: `ParserType`
**Location:** `/workspaces/neural-data-platform/core/src/parsers/config.rs:58-72`

| Value | Rust Variant | Description | Use Case |
|-------|-------------|-------------|----------|
| `flat_json` | `ParserType::FlatJson` | Extract all numeric fields from flat JSON | AirGradient, Home Assistant |
| `json_path` | `ParserType::JsonPath` | Extract specific fields via JSON path | OpenWeatherMap, NWS |
| `array_iterator` | `ParserType::ArrayIterator` | Iterate over JSON arrays | Nested array payloads |
| `column_oriented` | `ParserType::ColumnOriented` | Column-oriented data structures | NWS gridpoints forecasts |
| Custom(name) | `ParserType::Custom(String)` | Custom registered parser | Extension point |

### JSON Schema Enum
**Location:** `/workspaces/neural-data-platform/schemas/stream-config.v1.1.schema.json:339`
```json
"enum": ["flat_json", "json_path", "csv", "xml"]
```

**Note:** JSON schema includes `csv` and `xml` not in Rust enum; Rust has `array_iterator` and `column_oriented` not in schema.

---

## 4. Silver Layer PostgreSQL Types (`silver_etl.field_mappings[].type`)

### Valid Column Types
**Location:** `/workspaces/neural-data-platform/core/src/config/silver_etl.rs:251-264`

| Value | PostgreSQL Type | Typical Use |
|-------|-----------------|-------------|
| `double_precision` | DOUBLE PRECISION | High-precision floats (temperature, PM2.5) |
| `real` | REAL | Lower-precision floats |
| `smallint` | SMALLINT | Small integers (CO2 ppm, indices) |
| `integer` | INTEGER | Medium integers |
| `bigint` | BIGINT | Large integers |
| `text` | TEXT | Strings |
| `varchar` | VARCHAR | Fixed-length strings |
| `boolean` | BOOLEAN | Boolean values |
| `timestamptz` | TIMESTAMPTZ | Timestamps with timezone |
| `jsonb` | JSONB | JSON binary storage |
| `text[]` | TEXT[] | Text arrays |

### JSON Schema Enum
**Location:** `/workspaces/neural-data-platform/schemas/stream-config.v1.1.schema.json:624`
```json
"enum": ["double_precision", "real", "smallint", "integer", "bigint", "text", "boolean", "jsonb", "timestamptz"]
```

---

## 5. Timestamp Transforms (`silver_etl.timestamp.transform`)

### Rust Enum: `TimestampTransform`
**Location:** `/workspaces/neural-data-platform/core/src/config/silver_etl.rs:191-202`

| Value | Rust Variant | Description | Input Format |
|-------|-------------|-------------|--------------|
| `microseconds_to_timestamp` | `MicrosecondsToTimestamp` | Convert microseconds since epoch | Integer |
| `iso8601` | `Iso8601` | Parse ISO 8601 string | String |
| `unix_seconds` | `UnixSeconds` | Convert Unix seconds to timestamp | Integer |
| `nws_duration` | `NwsDuration` | Parse NWS duration format | String like "2024-01-01T00:00:00Z/PT1H" |

### JSON Schema Enum
**Location:** `/workspaces/neural-data-platform/schemas/stream-config.v1.1.schema.json:555-556`
```json
"enum": ["microseconds_to_timestamp", "milliseconds_to_timestamp", "seconds_to_timestamp", "iso8601_to_timestamp", "none"]
```

**Note:** JSON schema has `milliseconds_to_timestamp` and `seconds_to_timestamp`; Rust has `nws_duration`.

---

## 6. Field Transform Types (`silver_etl.field_mappings[].transform.type`)

### Rust Enum: `TransformConfig`
**Location:** `/workspaces/neural-data-platform/core/src/config/silver_etl.rs:291-317`

| Value | Rust Variant | Required Fields | Description |
|-------|-------------|-----------------|-------------|
| `unit_conversion` | `UnitConversion` | `from`, `to`, `formula` | Convert units (K->C, m/s->km/h) |
| `expression` | `Expression` | `expression` | SQL expression transform |
| `lookup` | `Lookup` | `table` | Categorical value mapping |
| `json_extract` | `JsonExtract` | `path` | Extract nested JSON value |
| `timestamp` | `Timestamp` | `format` | Timestamp format conversion |
| `computed` | `Computed` | `depends_on`, `expression` | Computed field from other columns |

### Conversion Formula Types
**Location:** `/workspaces/neural-data-platform/core/src/config/silver_etl.rs:320-328`

| Value | Description | Parameters |
|-------|-------------|------------|
| `linear` | (value * scale) + offset | `scale`, `offset` |
| `custom` | Custom code expression | `code` |

### JSON Schema Enum
**Location:** `/workspaces/neural-data-platform/schemas/stream-config.v1.1.schema.json:656-659`
```json
"enum": ["unit_conversion", "scale", "offset", "expression", "lookup", "coalesce"]
```

---

## 7. Data Quality Rules (`silver_etl.dq_rules[].rule`)

### Rust Enum: `DqRule`
**Location:** `/workspaces/neural-data-platform/core/src/config/silver_etl.rs:354-498`

#### Value-Level Rules

| Value | Rust Variant | Parameters | Description |
|-------|-------------|------------|-------------|
| `range_check` | `RangeCheck` | `field`, `min`, `max`, `clamp_to_bounds` | Validate numeric bounds |
| `null_check` | `NullCheck` | `field` | Validate non-null |
| `enum_check` | `EnumCheck` | `field`, `allowed_values`, `case_sensitive` | Validate allowed values |
| `pattern_check` | `PatternCheck` | `field`, `pattern` | Regex pattern validation |

#### Temporal Rules

| Value | Rust Variant | Parameters | Description |
|-------|-------------|------------|-------------|
| `freshness_check` | `FreshnessCheck` | `field`, `max_age`, `max_future`, `reference` | Timestamp recency |
| `monotonic_check` | `MonotonicCheck` | `field`, `direction`, `partition_by`, `allow_reset` | Monotonic values |
| `rate_of_change` | `RateOfChange` | `field`, `max_change_per_minute`, `partition_by` | Delta validation |

#### Cross-Field Rules

| Value | Rust Variant | Parameters | Description |
|-------|-------------|------------|-------------|
| `cross_field_check` | `CrossFieldCheck` | `name`, `expression`, `message` | Multi-field SQL expression |
| `conditional_check` | `ConditionalCheck` | `name`, `condition`, `then_rule` | Conditional validation |

#### Batch-Level Rules

| Value | Rust Variant | Parameters | Description |
|-------|-------------|------------|-------------|
| `completeness_check` | `CompletenessCheck` | `level`, `field`, `min_completeness` | Batch completeness ratio |
| `cardinality_check` | `CardinalityCheck` | `level`, `field`, `expected_range` | Distinct value count |

### JSON Schema Enum (Field-Level)
**Location:** `/workspaces/neural-data-platform/schemas/stream-config.v1.1.schema.json:720`
```json
"enum": ["range_check", "not_null", "enum_check", "regex_check", "length_check"]
```

### JSON Schema Enum (Batch-Level)
**Location:** `/workspaces/neural-data-platform/schemas/stream-config.v1.1.schema.json:766`
```json
"enum": ["cross_field_check", "freshness_check", "rate_of_change", "completeness_check", "uniqueness_check", "referential_check"]
```

---

## 8. DQ Actions (`silver_etl.dq_rules[].action`)

### Rust Enum: `DqAction`
**Location:** `/workspaces/neural-data-platform/core/src/config/silver_etl.rs:596-610`

| Value | Rust Variant | Description |
|-------|-------------|-------------|
| `flag` | `DqAction::Flag` | Keep value, add to dq_flags (default) |
| `reject` | `DqAction::Reject` | Set to NULL, add to dq_flags |
| `clamp` | `DqAction::Clamp` | Clamp to bounds, add to dq_flags |
| `drop` | `DqAction::Drop` | Drop entire row |
| `warn` | `DqAction::Warn` | Log warning (batch-level) |

### JSON Schema Enum
**Location:** `/workspaces/neural-data-platform/schemas/stream-config.v1.1.schema.json:748-750`
```json
"enum": ["flag", "reject", "clamp", "nullify", "warn"]
```

**Note:** JSON schema has `nullify` instead of `reject`.

---

## 9. Monotonic Direction (`monotonic_check.direction`)

### Rust Enum: `MonotonicDirection`
**Location:** `/workspaces/neural-data-platform/core/src/config/silver_etl.rs:587-593`

| Value | Description |
|-------|-------------|
| `increasing` | Values must not decrease |
| `decreasing` | Values must not increase |
| `strict_increasing` | Values must strictly increase |

---

## 10. Deduplication Strategy (`silver_etl.deduplication.strategy`)

### Rust Enum: `DeduplicationStrategy`
**Location:** `/workspaces/neural-data-platform/core/src/config/silver_etl.rs:688-698`

| Value | Rust Variant | Description |
|-------|-------------|-------------|
| `upsert` | `DeduplicationStrategy::Upsert` | Update existing row (default) |
| `skip` | `DeduplicationStrategy::Skip` | Skip if key exists |
| `replace` | `DeduplicationStrategy::Replace` | Replace entire row |

### JSON Schema Enum
**Location:** `/workspaces/neural-data-platform/schemas/stream-config.v1.1.schema.json:900`
```json
"enum": ["upsert", "skip", "last_wins", "first_wins"]
```

---

## 11. Device Classes (`entity_schemas.device_class`)

### Status: Freeform String

`device_class` is defined as a freeform `string` in both Rust and JSON schema. It is NOT constrained to an enum.

**Rust Location:** `/workspaces/neural-data-platform/core/src/types/stream_config.rs:384-385`
```rust
pub device_class: Option<String>,
```

**JSON Schema:** No enum constraint.

### Values Currently In Use

| Value | Source | Context |
|-------|--------|---------|
| `air_quality` | air-quality/config.json | AirGradient sensors |
| `binary_sensor` | home-assistant-state/config.json | Home Assistant binary sensors |

### Recommended Values (Home Assistant compatible)
Based on Home Assistant sensor device classes:
- `air_quality`, `binary_sensor`, `temperature`, `humidity`, `pressure`
- `weather`, `wind_speed`, `precipitation`, `illuminance`, `motion`
- `door`, `window`, `moisture`, `gas`, `smoke`, `co2`, `pm25`, `pm10`

---

## 12. Partitioning Strategy (`partitioning_strategy`)

### JSON Schema Enum
**Location:** `/workspaces/neural-data-platform/schemas/stream-config.v1.1.schema.json:49-51`

| Value | Description |
|-------|-------------|
| `daily` | Partition by day (default) |
| `hourly` | Partition by hour |
| `monthly` | Partition by month |

---

## 13. Authentication Types (`sources[].endpoints[].auth_type`)

### JSON Schema Enum
**Location:** `/workspaces/neural-data-platform/schemas/stream-config.v1.1.schema.json:318`

| Value | Description |
|-------|-------------|
| `none` | No authentication |
| `query_param` | API key in query parameter |
| `header` | API key in header |
| `basic` | HTTP Basic authentication |

---

## 14. CSV-Specific Enums (dp-013)

### Timestamp Format (`CsvSourceConfig.timestamp_format`)
**Location:** `/workspaces/neural-data-platform/core/src/types/stream_config.rs:228-240`

| Value | Description |
|-------|-------------|
| `iso8601` | ISO 8601 format (default) |
| `epoch_seconds` | Unix epoch in seconds |
| `epoch_millis` | Unix epoch in milliseconds |
| Custom(String) | Custom strftime pattern |

### On Error (`CsvSourceConfig.on_error`)
**Location:** `/workspaces/neural-data-platform/core/src/types/stream_config.rs:243-253`

| Value | Description |
|-------|-------------|
| `skip` | Skip invalid rows (default) |
| `fail` | Fail on first error |
| `log` | Log error and continue |

---

## 15. Freshness Reference (`freshness_check.reference`)

### Values

| Value | Description |
|-------|-------------|
| `ingestion_time` | Compare to when data was ingested (default) |
| `current_time` | Compare to current wall clock time |
| `batch_time` | Compare to batch processing time |

---

## Discrepancies Between Rust and JSON Schema

| Area | Rust Value | JSON Schema Value | Resolution |
|------|------------|-------------------|------------|
| Source type | `webhook` | `http_push` | Align to `http_push` in dp-019 |
| Source type | `csv` | Not present | Add to schema |
| Parser type | `array_iterator`, `column_oriented` | `csv`, `xml` | Add all to schema |
| Timestamp transform | `nws_duration` | `milliseconds_to_timestamp` | Add all to both |
| DQ action | `reject` | `nullify` | Align naming |
| Dedup strategy | `replace` | `last_wins`, `first_wins` | Add all |

---

## Validation Pipeline Implications

For dp-019 Config Validation Pipeline, the validator MUST:

1. **Accept all Rust-supported values** as the authoritative source
2. **Map JSON schema aliases** where discrepancies exist
3. **Warn on deprecated patterns** (entity_schemas, mqtt top-level)
4. **Validate cross-field constraints** (e.g., range only for numeric types)
5. **Support freeform strings** for device_class with recommended values

---

## References

| File | Purpose |
|------|---------|
| `/workspaces/neural-data-platform/core/src/types/stream_config.rs` | Rust type definitions |
| `/workspaces/neural-data-platform/core/src/config/silver_etl.rs` | Silver ETL configuration |
| `/workspaces/neural-data-platform/core/src/parsers/config.rs` | Parser configuration |
| `/workspaces/neural-data-platform/schemas/stream-config.v1.1.schema.json` | JSON Schema |
| `/workspaces/neural-data-platform/config/base/streams/air-quality/config.json` | Reference config |
| `/workspaces/neural-data-platform/config/base/streams/outdoor-weather/config.json` | Reference config |
