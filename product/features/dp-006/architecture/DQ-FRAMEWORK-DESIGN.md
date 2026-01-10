# DP-006: Data Quality Framework Design

**Document**: DQ-FRAMEWORK-DESIGN.md
**Version**: 1.0
**Date**: 2026-01-10
**Author**: NDP Data Quality Engineer
**Status**: Proposed
**Feature**: DP-006 (Silver Layer Implementation)

---

## Executive Summary

This document defines the complete Data Quality (DQ) framework for the Silver ETL pipeline. The framework implements a "flag over reject" principle with full transparency, enabling investigation of data issues without data loss.

### Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| **Flag as default action** | Preserve data; transparency enables investigation |
| **dq_flags TEXT[]** | Array allows multiple violations per row |
| **Structured flag format** | `rule:field:reason[:value]` enables parsing |
| **Separate transparency table** | Detailed audit trail without bloating Silver tables |
| **Config-driven rules** | YAML-only changes to add/modify DQ rules |

---

## 1. DQ Rule Types

### 1.1 Rule Type Hierarchy

```
DQ Rule Types
├── Value-Level Rules (single field validation)
│   ├── range_check        - Numeric bounds validation
│   ├── null_check         - Required field validation
│   ├── enum_check         - Value in allowed set
│   └── pattern_check      - Regex validation
│
├── Temporal Rules (time-based validation)
│   ├── freshness_check    - Timestamp within expected window
│   ├── monotonic_check    - Values increase/decrease monotonically
│   └── rate_of_change     - Max delta between consecutive values
│
├── Cross-Field Rules (multi-field validation)
│   ├── cross_field_check  - Relationship between fields
│   └── conditional_check  - Value depends on another field
│
└── Batch-Level Rules (aggregate validation)
    ├── completeness_check - Minimum non-null percentage
    └── cardinality_check  - Expected distinct value count
```

### 1.2 Rule Type Definitions

#### 1.2.1 range_check

Validates numeric values fall within physical bounds.

```yaml
- rule: range_check
  field: pm25
  min: 0.0
  max: 1000.0
  action: flag           # flag | reject | clamp
  clamp_to_bounds: false # If true with clamp action, adjust to min/max
```

**Use Cases**:
- Temperature: -60 to 60 C (Earth surface range)
- Humidity: 0-100%
- PM2.5: 0-1000 ug/m3
- Pressure: 800-1200 hPa

**SQL Generation**:
```sql
CASE
  WHEN pm25 < 0.0 OR pm25 > 1000.0
  THEN 'range_check:pm25:out_of_bounds'
  ELSE NULL
END
```

#### 1.2.2 null_check

Validates required fields are present.

```yaml
- rule: null_check
  field: observation_time
  action: reject         # Required fields should reject
```

**Use Cases**:
- Primary key fields (observation_time, ndp_id)
- Critical measurements (pm25 for air quality)

**SQL Generation**:
```sql
CASE
  WHEN observation_time IS NULL
  THEN 'null_check:observation_time:missing'
  ELSE NULL
END
```

#### 1.2.3 enum_check

Validates value is in an allowed set.

```yaml
- rule: enum_check
  field: wind_direction_cardinal
  allowed_values: [N, NE, E, SE, S, SW, W, NW]
  case_sensitive: false
  action: flag
```

**Use Cases**:
- Cardinal directions (N, NE, E, etc.)
- Weather conditions (clear, cloudy, rain, etc.)
- AQI categories (Good, Moderate, Unhealthy, etc.)

**SQL Generation**:
```sql
CASE
  WHEN UPPER(wind_direction_cardinal) NOT IN ('N','NE','E','SE','S','SW','W','NW')
  THEN 'enum_check:wind_direction_cardinal:invalid_value'
  ELSE NULL
END
```

#### 1.2.4 pattern_check

Validates string values match a regex pattern.

```yaml
- rule: pattern_check
  field: device_serial
  pattern: "^[A-Z0-9]{8,12}$"
  action: flag
```

**Use Cases**:
- Device serial numbers
- Station identifiers
- ISO timestamps

**SQL Generation**:
```sql
CASE
  WHEN device_serial !~ '^[A-Z0-9]{8,12}$'
  THEN 'pattern_check:device_serial:pattern_mismatch'
  ELSE NULL
END
```

#### 1.2.5 freshness_check

Validates timestamp is within expected window relative to ingestion time.

```yaml
- rule: freshness_check
  field: observation_time
  max_age: "2 hours"       # Data cannot be older than this
  max_future: "10 minutes" # Data cannot be this far in future
  reference: ingestion_time
  action: flag
```

**Use Cases**:
- Stale sensor data detection
- Future-dated records (clock sync issues)
- Late-arriving data flagging

**SQL Generation**:
```sql
CASE
  WHEN observation_time < ingestion_time - INTERVAL '2 hours'
  THEN 'freshness_check:observation_time:stale'
  WHEN observation_time > ingestion_time + INTERVAL '10 minutes'
  THEN 'freshness_check:observation_time:future'
  ELSE NULL
END
```

#### 1.2.6 monotonic_check

Validates cumulative values increase (or decrease) monotonically.

```yaml
- rule: monotonic_check
  field: cumulative_rainfall
  direction: increasing   # increasing | decreasing | strict_increasing
  partition_by: [ndp_id]
  allow_reset: true       # Allow counter resets
  reset_threshold: 1000   # Values below this after high = reset
  action: flag
```

**Use Cases**:
- Cumulative precipitation
- Energy meter readings
- Event counters

**SQL Generation**:
```sql
-- Requires window function in ETL
CASE
  WHEN cumulative_rainfall < LAG(cumulative_rainfall) OVER (
    PARTITION BY ndp_id ORDER BY observation_time
  )
  AND LAG(cumulative_rainfall) OVER (...) < 1000  -- Not a reset
  THEN 'monotonic_check:cumulative_rainfall:decreased'
  ELSE NULL
END
```

#### 1.2.7 rate_of_change

Validates the rate of change between consecutive values is within bounds.

```yaml
- rule: rate_of_change
  field: temperature_c
  max_change_per_minute: 2.0  # Max 2 C per minute
  partition_by: [ndp_id]
  action: flag
```

**Use Cases**:
- Temperature (shouldn't change > 2C/min)
- PM2.5 (sudden spikes indicate sensor issues)
- Pressure (rapid changes = weather events)

**SQL Generation**:
```sql
CASE
  WHEN ABS(temperature_c - LAG(temperature_c) OVER (
    PARTITION BY ndp_id ORDER BY observation_time
  )) / NULLIF(
    EXTRACT(EPOCH FROM observation_time - LAG(observation_time) OVER (...)) / 60.0, 0
  ) > 2.0
  THEN 'rate_of_change:temperature_c:exceeded'
  ELSE NULL
END
```

#### 1.2.8 cross_field_check

Validates relationships between multiple fields.

```yaml
- rule: cross_field_check
  name: dew_point_below_temp
  expression: "dew_point_c <= temperature_c"
  message: "dew_point_above_temp"
  action: flag
```

**Use Cases**:
- Dew point <= Temperature (physical constraint)
- Wind gust >= Wind speed
- Feels-like temperature relative to actual temperature
- PM10 >= PM2.5 (larger particles include smaller)

**SQL Generation**:
```sql
CASE
  WHEN NOT (dew_point_c <= temperature_c)
  THEN 'cross_field_check:dew_point_above_temp'
  ELSE NULL
END
```

#### 1.2.9 conditional_check

Validates a field based on another field's value.

```yaml
- rule: conditional_check
  name: rain_requires_precip
  condition: "weather_condition = 'rain'"
  then_rule:
    rule: range_check
    field: precipitation_mm
    min: 0.1
    max: 500.0
  action: flag
```

**Use Cases**:
- If weather = 'rain', precipitation > 0
- If location_type = 'indoor', temperature in range 10-40 C
- If sensor_model = 'AG-PRO', VOC reading should exist

**SQL Generation**:
```sql
CASE
  WHEN weather_condition = 'rain'
    AND (precipitation_mm IS NULL OR precipitation_mm < 0.1)
  THEN 'conditional_check:rain_requires_precip'
  ELSE NULL
END
```

#### 1.2.10 completeness_check (Batch Level)

Validates batch-level completeness metrics.

```yaml
- rule: completeness_check
  level: batch
  field: temperature_c
  min_completeness: 0.95  # 95% non-null
  action: warn            # Batch rules typically warn/alert
```

**Use Cases**:
- Expect 95% of rows to have temperature
- Expect 80% of forecasts to have wind data

**SQL Generation** (post-ETL check):
```sql
SELECT
  CASE
    WHEN COUNT(temperature_c)::FLOAT / NULLIF(COUNT(*), 0) < 0.95
    THEN 'completeness_check:temperature_c:below_threshold'
    ELSE NULL
  END as batch_flag
FROM etl_batch
```

#### 1.2.11 cardinality_check (Batch Level)

Validates expected number of distinct values in a batch.

```yaml
- rule: cardinality_check
  level: batch
  field: ndp_id
  expected_range: [1, 10]  # Expect 1-10 distinct sources
  action: warn
```

**Use Cases**:
- Expect data from N sources per batch
- Expect forecasts for M time periods

**SQL Generation** (post-ETL check):
```sql
SELECT
  CASE
    WHEN COUNT(DISTINCT ndp_id) NOT BETWEEN 1 AND 10
    THEN 'cardinality_check:ndp_id:unexpected_count'
    ELSE NULL
  END as batch_flag
FROM etl_batch
```

---

## 2. DQ Actions

### 2.1 Action Definitions

| Action | Behavior | Value Written | dq_flags Entry | Use When |
|--------|----------|---------------|----------------|----------|
| `flag` | Keep original value | Original | Yes | Default; suspicious but possible |
| `reject` | Set to NULL | NULL | Yes | Invalid data that breaks queries |
| `clamp` | Adjust to bounds | Clamped value | Yes | Physical constraints (0-100%) |
| `drop` | Drop entire row | Row excluded | Yes (in transparency table) | Catastrophically invalid |

### 2.2 Action Selection Guidelines

```
┌─────────────────────────────────────────────────────────────┐
│                  DQ ACTION DECISION TREE                    │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  Is the value physically impossible?                        │
│  (e.g., humidity = 150%, negative PM2.5)                   │
│     │                                                       │
│     ├─ YES → Can the value be safely corrected?             │
│     │        │                                              │
│     │        ├─ YES → clamp (humidity → 100)               │
│     │        │                                              │
│     │        └─ NO  → reject (set NULL)                    │
│     │                                                       │
│     └─ NO  → Would keeping it break downstream?             │
│              │                                              │
│              ├─ YES → reject                               │
│              │                                              │
│              └─ NO  → flag (keep value, record issue)      │
│                                                             │
│  Is the entire record unusable?                             │
│  (e.g., all fields are NULL or timestamp invalid)          │
│     │                                                       │
│     ├─ YES → drop (record in transparency table)           │
│     │                                                       │
│     └─ NO  → Apply field-level rules above                 │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 2.3 Default Action by Rule Type

| Rule Type | Default Action | Rationale |
|-----------|----------------|-----------|
| range_check | flag | Values might be unusual but valid extreme |
| null_check | reject | Missing required data is unusable |
| enum_check | flag | Unknown values should be preserved for debugging |
| pattern_check | flag | Format issues don't necessarily invalidate data |
| freshness_check | flag | Late data is still data |
| monotonic_check | flag | Counter issues need investigation |
| rate_of_change | flag | Spikes might be real weather events |
| cross_field_check | flag | Relationship violations need investigation |
| conditional_check | flag | Context-dependent rules are informational |
| completeness_check | warn | Batch-level; doesn't affect individual rows |
| cardinality_check | warn | Batch-level; informational only |

---

## 3. dq_flags Column Structure

### 3.1 Column Definition

```sql
-- In Silver table DDL
dq_flags TEXT[] DEFAULT '{}'::TEXT[]
```

### 3.2 Flag Format Specification

```
{rule_type}:{field_name}:{reason}[:{value}]
```

| Component | Description | Example |
|-----------|-------------|---------|
| `rule_type` | DQ rule that triggered | `range_check`, `null_check` |
| `field_name` | Column that failed | `pm25`, `temperature_c` |
| `reason` | Specific violation | `out_of_bounds`, `missing`, `exceeded` |
| `value` | Optional: original/flagged value | `150.5` (if include_values enabled) |

### 3.3 Flag Examples

```sql
-- Single range violation
['range_check:pm25:out_of_bounds']

-- Multiple violations on same row
['range_check:temperature_c:exceeded_max', 'null_check:humidity_pct:missing']

-- With value (when dq_output.include_values: true)
['range_check:pm25:out_of_bounds:1500.5']

-- Clamp action
['range_check:humidity_pct:clamped:105.0->100.0']

-- Cross-field violation
['cross_field_check:dew_point_above_temp']

-- Rate of change
['rate_of_change:temperature_c:exceeded:5.2/min']
```

### 3.4 dq_flags Query Patterns

```sql
-- Find rows with any DQ issues
SELECT * FROM silver.weather_observations
WHERE array_length(dq_flags, 1) > 0;

-- Find rows with specific rule violation
SELECT * FROM silver.weather_observations
WHERE 'range_check:pm25:out_of_bounds' = ANY(dq_flags);

-- Count violations by rule
SELECT
  unnest(dq_flags) as flag,
  COUNT(*) as occurrences
FROM silver.air_quality_observations
WHERE observation_time > NOW() - INTERVAL '24 hours'
GROUP BY 1
ORDER BY 2 DESC;

-- Find rows with multiple issues
SELECT * FROM silver.weather_observations
WHERE array_length(dq_flags, 1) > 2;
```

---

## 4. DQ Configuration Schema

### 4.1 YAML Schema Definition

```yaml
# DQ Rules Config Schema (within silver_etl section)

dq_rules:
  # ─────────────────────────────────────────────────────────
  # Value-Level Rules
  # ─────────────────────────────────────────────────────────

  # range_check - Numeric bounds
  - rule: range_check
    field: <string>           # Required: column name
    min: <number>             # Optional: minimum value (inclusive)
    max: <number>             # Optional: maximum value (inclusive)
    action: flag | reject | clamp  # Default: flag
    clamp_to_bounds: <boolean>     # Default: true (when action=clamp)

  # null_check - Required field
  - rule: null_check
    field: <string>           # Required: column name
    action: flag | reject     # Default: reject

  # enum_check - Allowed values
  - rule: enum_check
    field: <string>           # Required: column name
    allowed_values: [<values>] # Required: list of valid values
    case_sensitive: <boolean>  # Default: false
    action: flag | reject      # Default: flag

  # pattern_check - Regex validation
  - rule: pattern_check
    field: <string>           # Required: column name
    pattern: <regex>          # Required: regex pattern
    action: flag | reject     # Default: flag

  # ─────────────────────────────────────────────────────────
  # Temporal Rules
  # ─────────────────────────────────────────────────────────

  # freshness_check - Timestamp validation
  - rule: freshness_check
    field: <string>           # Required: timestamp column
    max_age: <interval>       # Optional: e.g., "2 hours"
    max_future: <interval>    # Optional: e.g., "10 minutes"
    reference: <string>       # Default: "ingestion_time"
    action: flag | reject     # Default: flag

  # monotonic_check - Cumulative value validation
  - rule: monotonic_check
    field: <string>           # Required: column name
    direction: increasing | decreasing | strict_increasing  # Required
    partition_by: [<columns>] # Required: grouping columns
    allow_reset: <boolean>    # Default: false
    reset_threshold: <number> # Optional: value indicating reset
    action: flag              # Default: flag

  # rate_of_change - Delta validation
  - rule: rate_of_change
    field: <string>           # Required: column name
    max_change_per_minute: <number>  # Required: max delta rate
    partition_by: [<columns>] # Required: grouping columns
    action: flag              # Default: flag

  # ─────────────────────────────────────────────────────────
  # Cross-Field Rules
  # ─────────────────────────────────────────────────────────

  # cross_field_check - Multi-field relationship
  - rule: cross_field_check
    name: <string>            # Required: unique rule name
    expression: <sql_expr>    # Required: boolean SQL expression
    message: <string>         # Optional: custom message for flag
    action: flag | reject     # Default: flag

  # conditional_check - Conditional validation
  - rule: conditional_check
    name: <string>            # Required: unique rule name
    condition: <sql_expr>     # Required: when condition is true
    then_rule:                # Required: rule to apply if condition met
      rule: <rule_type>
      # ... rule-specific params
    action: flag | reject     # Default: flag

  # ─────────────────────────────────────────────────────────
  # Batch-Level Rules
  # ─────────────────────────────────────────────────────────

  # completeness_check - Batch completeness
  - rule: completeness_check
    level: batch              # Required: must be "batch"
    field: <string>           # Required: column name
    min_completeness: <float> # Required: 0.0-1.0
    action: warn | alert      # Default: warn

  # cardinality_check - Distinct value count
  - rule: cardinality_check
    level: batch              # Required: must be "batch"
    field: <string>           # Required: column name
    expected_range: [<min>, <max>]  # Required: expected distinct count range
    action: warn | alert      # Default: warn

# ─────────────────────────────────────────────────────────
# DQ Output Configuration
# ─────────────────────────────────────────────────────────

dq_output:
  enabled: true               # Default: true
  target_column: dq_flags     # Default: "dq_flags"
  include_rules: true         # Include rule name in flag
  include_values: false       # Include original values (privacy concern)

  # Transparency table output
  transparency:
    enabled: true             # Write to transparency table
    table: silver.dq_transparency
    include_sample_payload: true
    max_samples_per_rule: 10
```

### 4.2 Complete Config Example: Air Quality

```yaml
# config/base/streams/air-quality/config.yaml
stream_id: air-quality
# ... existing Bronze config ...

silver_etl:
  enabled: true
  target_table: silver.air_quality_observations

  field_mappings:
    - source_path: raw_payload.pm02
      target_column: pm25
      type: double_precision

    - source_path: raw_payload.rco2
      target_column: co2
      type: smallint

    - source_path: raw_payload.atmp
      target_column: temperature_c
      type: double_precision

    - source_path: raw_payload.rhum
      target_column: humidity_pct
      type: double_precision

  dq_rules:
    # Value-level rules
    - rule: range_check
      field: pm25
      min: 0.0
      max: 1000.0
      action: flag

    - rule: range_check
      field: co2
      min: 380
      max: 10000
      action: flag

    - rule: range_check
      field: temperature_c
      min: -40.0
      max: 85.0
      action: flag

    - rule: range_check
      field: humidity_pct
      min: 0.0
      max: 100.0
      action: clamp

    - rule: null_check
      field: pm25
      action: flag  # PM2.5 is critical but sensor might be warming up

    - rule: null_check
      field: observation_time
      action: reject

    # Temporal rules
    - rule: freshness_check
      field: observation_time
      max_age: "2 hours"
      max_future: "5 minutes"
      action: flag

    - rule: rate_of_change
      field: pm25
      max_change_per_minute: 100.0  # PM2.5 shouldn't jump >100 ug/m3/min
      partition_by: [ndp_id]
      action: flag

    - rule: rate_of_change
      field: temperature_c
      max_change_per_minute: 3.0
      partition_by: [ndp_id]
      action: flag

    # Cross-field rules
    - rule: cross_field_check
      name: pm10_gte_pm25
      expression: "pm10 IS NULL OR pm25 IS NULL OR pm10 >= pm25"
      message: "pm10_less_than_pm25"
      action: flag

    # Batch-level rules
    - rule: completeness_check
      level: batch
      field: pm25
      min_completeness: 0.95
      action: warn

  dq_output:
    enabled: true
    target_column: dq_flags
    include_rules: true
    include_values: false
    transparency:
      enabled: true
      table: silver.dq_transparency
      include_sample_payload: true
      max_samples_per_rule: 10
```

### 4.3 Complete Config Example: Weather Observations

```yaml
# config/base/streams/outdoor-weather/config.yaml
stream_id: outdoor-weather
# ... existing Bronze config ...

silver_etl:
  enabled: true
  target_table: silver.weather_observations

  field_mappings:
    - source_path: raw_payload.main.temp
      target_column: temperature_c
      type: double_precision
      transform:
        type: unit_conversion
        from: kelvin
        to: celsius
        formula:
          type: linear
          scale: 1.0
          offset: -273.15

    - source_path: raw_payload.main.humidity
      target_column: humidity_pct
      type: double_precision

    - source_path: raw_payload.main.pressure
      target_column: pressure_hpa
      type: double_precision

    - source_path: raw_payload.wind.speed
      target_column: wind_speed_kmh
      type: double_precision
      transform:
        type: unit_conversion
        from: m_s
        to: km_h
        formula:
          type: linear
          scale: 3.6
          offset: 0.0

    - source_path: raw_payload.wind.deg
      target_column: wind_direction_deg
      type: double_precision

  dq_rules:
    # Value-level rules
    - rule: range_check
      field: temperature_c
      min: -60.0
      max: 60.0
      action: flag

    - rule: range_check
      field: humidity_pct
      min: 0.0
      max: 100.0
      action: clamp

    - rule: range_check
      field: pressure_hpa
      min: 800.0
      max: 1200.0
      action: flag

    - rule: range_check
      field: wind_speed_kmh
      min: 0.0
      max: 400.0  # Fastest recorded wind ~408 km/h
      action: flag

    - rule: range_check
      field: wind_direction_deg
      min: 0.0
      max: 360.0
      action: clamp
      clamp_to_bounds: false  # Use modulo instead

    - rule: null_check
      field: observation_time
      action: reject

    - rule: null_check
      field: temperature_c
      action: flag

    # Temporal rules
    - rule: freshness_check
      field: observation_time
      max_age: "3 hours"
      max_future: "10 minutes"
      action: flag

    - rule: rate_of_change
      field: temperature_c
      max_change_per_minute: 2.0
      partition_by: [ndp_id]
      action: flag

    - rule: rate_of_change
      field: pressure_hpa
      max_change_per_minute: 5.0  # Rapid pressure change
      partition_by: [ndp_id]
      action: flag

    # Cross-field rules
    - rule: cross_field_check
      name: wind_gust_gte_speed
      expression: "wind_gust_kmh IS NULL OR wind_gust_kmh >= wind_speed_kmh"
      message: "gust_less_than_sustained"
      action: flag

    - rule: cross_field_check
      name: feels_like_reasonable
      expression: >
        feels_like_c IS NULL OR
        ABS(feels_like_c - temperature_c) <= 20
      message: "feels_like_unreasonable"
      action: flag

    # Batch rules
    - rule: completeness_check
      level: batch
      field: temperature_c
      min_completeness: 0.98
      action: warn

  dq_output:
    enabled: true
    target_column: dq_flags
    include_rules: true
    include_values: false
    transparency:
      enabled: true
      table: silver.dq_transparency
```

---

## 5. Transparency Tables

### 5.1 Row-Level Transparency Table

```sql
-- Records individual DQ violations for audit trail
CREATE TABLE silver.dq_transparency (
    id                  BIGSERIAL,
    check_time          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    stream_id           TEXT NOT NULL,
    batch_id            TEXT,                   -- ETL batch identifier
    rule_name           TEXT NOT NULL,          -- e.g., "range_check"
    rule_level          TEXT NOT NULL,          -- "row" or "batch"
    field_name          TEXT,                   -- Column that failed
    violation_type      TEXT NOT NULL,          -- "flag", "reject", "clamp", "drop"
    violation_reason    TEXT NOT NULL,          -- Specific reason
    row_count           INTEGER NOT NULL,       -- Rows affected
    original_value      TEXT,                   -- Original value (if enabled)
    result_value        TEXT,                   -- Value after action
    sample_payload      JSONB,                  -- Sample row for debugging
    context             JSONB,                  -- Additional metadata

    PRIMARY KEY (check_time, id)
);

-- Convert to hypertable for time-series queries
SELECT create_hypertable(
    'silver.dq_transparency',
    'check_time',
    chunk_time_interval => INTERVAL '7 days'
);

-- Indexes for dashboard queries
CREATE INDEX idx_dq_trans_stream_time
ON silver.dq_transparency (stream_id, check_time DESC);

CREATE INDEX idx_dq_trans_rule
ON silver.dq_transparency (rule_name, check_time DESC);

CREATE INDEX idx_dq_trans_violation
ON silver.dq_transparency (violation_type, check_time DESC);

-- Retention policy (90 days)
SELECT add_retention_policy(
    'silver.dq_transparency',
    INTERVAL '90 days'
);
```

### 5.2 Batch-Level Summary Table

```sql
-- Aggregated DQ metrics per ETL batch
CREATE TABLE silver.dq_batch_summary (
    batch_id            TEXT PRIMARY KEY,
    batch_time          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    stream_id           TEXT NOT NULL,

    -- Row counts
    total_rows          INTEGER NOT NULL,
    rows_with_flags     INTEGER NOT NULL,
    rows_rejected       INTEGER NOT NULL,
    rows_clamped        INTEGER NOT NULL,
    rows_dropped        INTEGER NOT NULL,

    -- Rule violation counts (JSONB for flexibility)
    rule_violations     JSONB NOT NULL DEFAULT '{}',
    -- Example: {"range_check:pm25": 12, "null_check:humidity": 3}

    -- Completeness metrics
    field_completeness  JSONB NOT NULL DEFAULT '{}',
    -- Example: {"pm25": 0.98, "temperature_c": 0.95}

    -- Batch-level rule results
    batch_rule_results  JSONB NOT NULL DEFAULT '{}',
    -- Example: {"completeness_check:pm25": "pass", "cardinality_check:ndp_id": "warn"}

    -- Duration
    etl_duration_ms     INTEGER,

    -- Status
    status              TEXT NOT NULL DEFAULT 'success'
    -- "success", "partial_failure", "failed"
);

-- Index for time-series queries
CREATE INDEX idx_dq_batch_stream_time
ON silver.dq_batch_summary (stream_id, batch_time DESC);
```

### 5.3 DQ Metrics Continuous Aggregate

```sql
-- Hourly DQ metrics for dashboards
CREATE MATERIALIZED VIEW silver.dq_metrics_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', check_time) AS hour,
    stream_id,
    rule_name,
    violation_type,
    COUNT(*) AS violation_batches,
    SUM(row_count) AS total_violations
FROM silver.dq_transparency
GROUP BY 1, 2, 3, 4;

-- Refresh policy
SELECT add_continuous_aggregate_policy(
    'silver.dq_metrics_hourly',
    start_offset => INTERVAL '3 hours',
    end_offset => INTERVAL '1 hour',
    schedule_interval => INTERVAL '1 hour'
);
```

---

## 6. SQL Generation Patterns

### 6.1 Row-Level DQ Check SQL Template

```sql
-- Generated DQ check expression for a single row
WITH dq_checks AS (
    SELECT
        -- Original fields
        observation_time,
        ndp_id,
        pm25,
        co2,
        temperature_c,
        humidity_pct,

        -- DQ flag expressions
        ARRAY_REMOVE(ARRAY[
            -- range_check: pm25
            CASE
                WHEN pm25 < 0.0 OR pm25 > 1000.0
                THEN 'range_check:pm25:out_of_bounds'
                ELSE NULL
            END,

            -- range_check: co2
            CASE
                WHEN co2 < 380 OR co2 > 10000
                THEN 'range_check:co2:out_of_bounds'
                ELSE NULL
            END,

            -- range_check: temperature_c
            CASE
                WHEN temperature_c < -40.0 OR temperature_c > 85.0
                THEN 'range_check:temperature_c:out_of_bounds'
                ELSE NULL
            END,

            -- null_check: observation_time
            CASE
                WHEN observation_time IS NULL
                THEN 'null_check:observation_time:missing'
                ELSE NULL
            END,

            -- freshness_check: observation_time
            CASE
                WHEN observation_time < ingestion_time - INTERVAL '2 hours'
                THEN 'freshness_check:observation_time:stale'
                WHEN observation_time > ingestion_time + INTERVAL '5 minutes'
                THEN 'freshness_check:observation_time:future'
                ELSE NULL
            END,

            -- cross_field_check: pm10_gte_pm25
            CASE
                WHEN NOT (pm10 IS NULL OR pm25 IS NULL OR pm10 >= pm25)
                THEN 'cross_field_check:pm10_less_than_pm25'
                ELSE NULL
            END

        ], NULL) AS dq_flags,

        -- Clamped values
        LEAST(GREATEST(humidity_pct, 0.0), 100.0) AS humidity_pct_clamped

    FROM bronze_data
)
SELECT
    observation_time,
    ndp_id,
    -- Apply reject action (set to NULL if null_check fails)
    CASE
        WHEN 'null_check:observation_time:missing' = ANY(dq_flags)
        THEN NULL
        ELSE observation_time
    END AS observation_time_final,
    pm25,
    co2,
    temperature_c,
    humidity_pct_clamped AS humidity_pct,
    dq_flags
FROM dq_checks
-- Apply drop action (exclude entire rows)
WHERE NOT ('drop_check:catastrophic' = ANY(dq_flags))
```

### 6.2 Temporal Rule SQL (Window Functions)

```sql
-- Rate of change check with window functions
WITH lagged AS (
    SELECT
        *,
        LAG(pm25) OVER w AS prev_pm25,
        LAG(observation_time) OVER w AS prev_time,
        LAG(temperature_c) OVER w AS prev_temp
    FROM bronze_data
    WINDOW w AS (PARTITION BY ndp_id ORDER BY observation_time)
),
with_rate AS (
    SELECT
        *,
        -- PM2.5 rate of change
        CASE
            WHEN prev_time IS NOT NULL
            THEN ABS(pm25 - prev_pm25) /
                 NULLIF(EXTRACT(EPOCH FROM observation_time - prev_time) / 60.0, 0)
            ELSE NULL
        END AS pm25_rate,
        -- Temperature rate of change
        CASE
            WHEN prev_time IS NOT NULL
            THEN ABS(temperature_c - prev_temp) /
                 NULLIF(EXTRACT(EPOCH FROM observation_time - prev_time) / 60.0, 0)
            ELSE NULL
        END AS temp_rate
    FROM lagged
)
SELECT
    *,
    ARRAY_REMOVE(ARRAY[
        CASE
            WHEN pm25_rate > 100.0
            THEN 'rate_of_change:pm25:exceeded'
            ELSE NULL
        END,
        CASE
            WHEN temp_rate > 3.0
            THEN 'rate_of_change:temperature_c:exceeded'
            ELSE NULL
        END
    ], NULL) AS temporal_flags
FROM with_rate
```

### 6.3 Batch-Level Rule SQL

```sql
-- Post-ETL batch validation
WITH batch_stats AS (
    SELECT
        COUNT(*) AS total_rows,
        COUNT(pm25) AS pm25_count,
        COUNT(temperature_c) AS temp_count,
        COUNT(DISTINCT ndp_id) AS ndp_count
    FROM silver.air_quality_observations
    WHERE batch_id = $1
)
SELECT
    ARRAY_REMOVE(ARRAY[
        -- completeness_check: pm25
        CASE
            WHEN pm25_count::FLOAT / NULLIF(total_rows, 0) < 0.95
            THEN 'completeness_check:pm25:below_threshold'
            ELSE NULL
        END,
        -- completeness_check: temperature_c
        CASE
            WHEN temp_count::FLOAT / NULLIF(total_rows, 0) < 0.90
            THEN 'completeness_check:temperature_c:below_threshold'
            ELSE NULL
        END,
        -- cardinality_check: ndp_id
        CASE
            WHEN ndp_count NOT BETWEEN 1 AND 10
            THEN 'cardinality_check:ndp_id:unexpected_count'
            ELSE NULL
        END
    ], NULL) AS batch_flags,
    total_rows,
    pm25_count::FLOAT / NULLIF(total_rows, 0) AS pm25_completeness,
    temp_count::FLOAT / NULLIF(total_rows, 0) AS temp_completeness,
    ndp_count
FROM batch_stats
```

---

## 7. Rust Implementation Types

### 7.1 DQ Rule Enum

```rust
// core/src/config/dq_rules.rs

use serde::{Deserialize, Serialize};

/// DQ rule action
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DqAction {
    #[default]
    Flag,
    Reject,
    Clamp,
    Drop,
    Warn,
    Alert,
}

/// DQ rule configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "rule", rename_all = "snake_case")]
pub enum DqRule {
    // Value-level rules
    RangeCheck {
        field: String,
        #[serde(default)]
        min: Option<f64>,
        #[serde(default)]
        max: Option<f64>,
        #[serde(default)]
        action: DqAction,
        #[serde(default = "default_true")]
        clamp_to_bounds: bool,
    },
    NullCheck {
        field: String,
        #[serde(default = "default_reject")]
        action: DqAction,
    },
    EnumCheck {
        field: String,
        allowed_values: Vec<String>,
        #[serde(default)]
        case_sensitive: bool,
        #[serde(default)]
        action: DqAction,
    },
    PatternCheck {
        field: String,
        pattern: String,
        #[serde(default)]
        action: DqAction,
    },

    // Temporal rules
    FreshnessCheck {
        field: String,
        #[serde(default)]
        max_age: Option<String>,
        #[serde(default)]
        max_future: Option<String>,
        #[serde(default = "default_ingestion_time")]
        reference: String,
        #[serde(default)]
        action: DqAction,
    },
    MonotonicCheck {
        field: String,
        direction: MonotonicDirection,
        partition_by: Vec<String>,
        #[serde(default)]
        allow_reset: bool,
        #[serde(default)]
        reset_threshold: Option<f64>,
        #[serde(default)]
        action: DqAction,
    },
    RateOfChange {
        field: String,
        max_change_per_minute: f64,
        partition_by: Vec<String>,
        #[serde(default)]
        action: DqAction,
    },

    // Cross-field rules
    CrossFieldCheck {
        name: String,
        expression: String,
        #[serde(default)]
        message: Option<String>,
        #[serde(default)]
        action: DqAction,
    },
    ConditionalCheck {
        name: String,
        condition: String,
        then_rule: Box<DqRule>,
        #[serde(default)]
        action: DqAction,
    },

    // Batch-level rules
    CompletenessCheck {
        #[serde(default = "default_batch")]
        level: String,
        field: String,
        min_completeness: f64,
        #[serde(default = "default_warn")]
        action: DqAction,
    },
    CardinalityCheck {
        #[serde(default = "default_batch")]
        level: String,
        field: String,
        expected_range: (i32, i32),
        #[serde(default = "default_warn")]
        action: DqAction,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MonotonicDirection {
    Increasing,
    Decreasing,
    StrictIncreasing,
}

/// DQ output configuration
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct DqOutputConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_dq_flags")]
    pub target_column: String,
    #[serde(default = "default_true")]
    pub include_rules: bool,
    #[serde(default)]
    pub include_values: bool,
    #[serde(default)]
    pub transparency: DqTransparencyConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct DqTransparencyConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_transparency_table")]
    pub table: String,
    #[serde(default = "default_true")]
    pub include_sample_payload: bool,
    #[serde(default = "default_max_samples")]
    pub max_samples_per_rule: usize,
}

// Default functions
fn default_true() -> bool { true }
fn default_reject() -> DqAction { DqAction::Reject }
fn default_warn() -> DqAction { DqAction::Warn }
fn default_batch() -> String { "batch".to_string() }
fn default_ingestion_time() -> String { "ingestion_time".to_string() }
fn default_dq_flags() -> String { "dq_flags".to_string() }
fn default_transparency_table() -> String { "silver.dq_transparency".to_string() }
fn default_max_samples() -> usize { 10 }
```

### 7.2 DQ Rule Registry (Trait-Based)

```rust
// core/src/etl/dq_registry.rs

use std::collections::HashMap;

/// Trait for DQ rule SQL generation
pub trait DqRuleSqlGenerator: Send + Sync {
    /// Rule type name
    fn rule_type(&self) -> &str;

    /// Generate SQL CASE expression for row-level check
    fn generate_check_sql(&self, config: &serde_json::Value) -> Result<String, DqError>;

    /// Generate flag string for this rule
    fn generate_flag_string(&self, config: &serde_json::Value) -> Result<String, DqError>;

    /// Does this rule require window functions?
    fn requires_window(&self) -> bool { false }

    /// Is this a batch-level rule?
    fn is_batch_level(&self) -> bool { false }
}

/// Registry of DQ rule generators
pub struct DqRuleRegistry {
    generators: HashMap<String, Box<dyn DqRuleSqlGenerator>>,
}

impl DqRuleRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            generators: HashMap::new(),
        };
        // Register built-in rules
        registry.register(Box::new(RangeCheckGenerator));
        registry.register(Box::new(NullCheckGenerator));
        registry.register(Box::new(EnumCheckGenerator));
        registry.register(Box::new(FreshnessCheckGenerator));
        registry.register(Box::new(RateOfChangeGenerator));
        registry.register(Box::new(CrossFieldCheckGenerator));
        registry.register(Box::new(CompletenessCheckGenerator));
        registry
    }

    pub fn register(&mut self, generator: Box<dyn DqRuleSqlGenerator>) {
        self.generators.insert(generator.rule_type().to_string(), generator);
    }

    pub fn get(&self, rule_type: &str) -> Option<&dyn DqRuleSqlGenerator> {
        self.generators.get(rule_type).map(|g| g.as_ref())
    }
}
```

---

## 8. Domain-Specific DQ Rules

### 8.1 Weather Domain

| Rule | Field | Min | Max | Action | Rationale |
|------|-------|-----|-----|--------|-----------|
| range_check | temperature_c | -60 | 60 | flag | Earth surface temperature range |
| range_check | humidity_pct | 0 | 100 | clamp | Physical constraint |
| range_check | pressure_hpa | 800 | 1200 | flag | Sea-level pressure range |
| range_check | wind_speed_kmh | 0 | 400 | flag | Fastest recorded ~408 km/h |
| range_check | wind_direction_deg | 0 | 360 | clamp | Circular, use modulo |
| cross_field | wind_gust >= wind_speed | - | - | flag | Gust must exceed sustained |
| cross_field | dew_point <= temp | - | - | flag | Physical constraint |
| rate_of_change | temperature_c | - | 2/min | flag | Realistic rate |
| rate_of_change | pressure_hpa | - | 5/min | flag | Rapid change = event |
| freshness | observation_time | -3h | +10m | flag | Reasonable data age |

### 8.2 Air Quality Domain

| Rule | Field | Min | Max | Action | Rationale |
|------|-------|-----|-----|--------|-----------|
| range_check | pm25 | 0 | 1000 | flag | Sensor range |
| range_check | pm10 | 0 | 2000 | flag | Sensor range |
| range_check | co2 | 380 | 10000 | flag | Atmospheric floor + sensor max |
| range_check | tvoc_index | 1 | 500 | clamp | Sensirion index range |
| range_check | nox_index | 1 | 500 | clamp | Sensirion index range |
| range_check | temperature_c | -40 | 85 | flag | Sensor operating range |
| range_check | humidity_pct | 0 | 100 | clamp | Physical constraint |
| cross_field | pm10 >= pm25 | - | - | flag | Larger particles include smaller |
| rate_of_change | pm25 | - | 100/min | flag | Sudden spike = sensor issue |
| rate_of_change | co2 | - | 500/min | flag | Ventilation event |
| freshness | observation_time | -2h | +5m | flag | MQTT should be fresh |
| null_check | pm25 | - | - | flag | Critical but may be warming up |
| null_check | observation_time | - | - | reject | Required for time-series |

---

## 9. Dashboard Queries

### 9.1 DQ Violations Summary (Last 24h)

```sql
SELECT
    stream_id,
    rule_name,
    violation_type,
    SUM(row_count) AS total_violations,
    COUNT(DISTINCT batch_id) AS batches_affected,
    MAX(check_time) AS last_seen
FROM silver.dq_transparency
WHERE check_time > NOW() - INTERVAL '24 hours'
GROUP BY 1, 2, 3
ORDER BY total_violations DESC
LIMIT 50;
```

### 9.2 DQ Trend by Hour

```sql
SELECT
    hour,
    stream_id,
    SUM(total_violations) AS violations
FROM silver.dq_metrics_hourly
WHERE hour > NOW() - INTERVAL '7 days'
GROUP BY 1, 2
ORDER BY 1, 2;
```

### 9.3 Field Completeness Dashboard

```sql
SELECT
    stream_id,
    batch_time,
    field_completeness->>'pm25' AS pm25_completeness,
    field_completeness->>'temperature_c' AS temp_completeness,
    field_completeness->>'co2' AS co2_completeness
FROM silver.dq_batch_summary
WHERE batch_time > NOW() - INTERVAL '24 hours'
ORDER BY batch_time DESC;
```

### 9.4 Rows with Multiple DQ Issues

```sql
SELECT
    observation_time,
    ndp_id,
    array_length(dq_flags, 1) AS flag_count,
    dq_flags
FROM silver.air_quality_observations
WHERE observation_time > NOW() - INTERVAL '24 hours'
  AND array_length(dq_flags, 1) > 2
ORDER BY flag_count DESC
LIMIT 100;
```

---

## 10. Key Principles Summary

1. **Flag over reject**: Default action is `flag` - preserve data, enable investigation
2. **Transparency is paramount**: Every DQ decision is auditable via `dq_flags` and transparency tables
3. **Config-driven**: All DQ rules defined in YAML; no code changes to add rules
4. **Domain-aware**: Rule thresholds based on physical constraints and domain knowledge
5. **Layered approach**: Extract DQ (Bronze), Transform DQ (Silver), Analytics DQ (monitoring)
6. **Bronze is sacred**: DQ happens on read/transform; raw data is never modified

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-10 | NDP DQ Engineer | Initial design |

---

## References

1. `product/features/dp-006/SCOPE.md` - Feature scope definition
2. `product/research/analyticplatforminfrastructure/04-LAYERED-DQ-STRATEGY.md` - Layered DQ strategy
3. `research/agenticdataplatform/silver/09-etl-genericity-assessment.md` - Genericity assessment
4. `docs/architecture/CONFIG_DRIVEN_SILVER_ETL_DESIGN.md` - Silver ETL design
5. `config/base/streams/air-quality/config.yaml` - Air quality stream config
6. `config/base/streams/outdoor-weather/config.yaml` - Weather stream config
