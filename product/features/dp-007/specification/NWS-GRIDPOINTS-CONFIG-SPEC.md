# NWS Gridpoints Forecast Silver ETL Configuration Specification

**Document**: NWS-GRIDPOINTS-CONFIG-SPEC.md
**Version**: 1.0
**Date**: 2026-01-12
**Author**: ndp-meteorologist
**Status**: Proposed
**Feature**: DP-007 (NWS Gridpoints Silver ETL)

---

## 1. Stream Overview

### 1.1 Data Source Characteristics

The NWS Gridpoints API returns 7-day hourly forecasts with comprehensive meteorological data.

| Characteristic | Value | Notes |
|----------------|-------|-------|
| **API Endpoint** | `https://api.weather.gov/gridpoints/{office}/{gridX},{gridY}` | Example: JAX/80,50 |
| **Update Frequency** | Every 1-6 hours | NWS updates forecasts on variable schedule |
| **Forecast Horizon** | ~168 hours (7 days) | Maximum lead time |
| **Metrics Count** | ~40 primary metrics | Each with ~130-150 time values |
| **Data Points per Poll** | ~5,000-6,000 | 40 metrics x 150 values average |
| **Unit System** | WMO SI Units | degC, km/h, Pa, percent |

### 1.2 Stream Classification

Per ADR-006-006 (Stream Types), this stream is classified as:

| Property | Value | Rationale |
|----------|-------|-----------|
| **stream_type** | `forecasts` | Predictions for future conditions |
| **Primary Key** | `(issue_time, valid_time, ndp_id)` | Multiple forecasts per valid_time |
| **Temporal Pattern** | Prediction lifecycle | Same valid_time revised over time |

### 1.3 Key Temporal Semantics

Understanding NWS forecast timestamps is critical for accurate ETL:

| Term | NWS Source | Silver Column | Description |
|------|------------|---------------|-------------|
| **Issue Time** | `properties.updateTime` | `issue_time` | When NWS generated this forecast |
| **Valid Time** | Each metric's `validTime` start | `valid_time` | When prediction applies |
| **Valid Duration** | ISO 8601 duration in `validTime` | `valid_duration` | How long prediction is valid (PT1H, PT2H, PT6H) |
| **Lead Time** | Computed | `lead_time_hours` | `valid_time - issue_time` (key analysis dimension) |

#### Timestamp Format in NWS API

```json
{
  "updateTime": "2026-01-01T13:56:49+00:00",
  "temperature": {
    "uom": "wmoUnit:degC",
    "values": [
      {"validTime": "2026-01-01T07:00:00+00:00/PT2H", "value": 3.333}
    ]
  }
}
```

The `validTime` field uses ISO 8601 interval format: `<start>/<duration>`

---

## 2. Pre-Transform Configuration

### 2.1 Parser Type

The NWS gridpoints response requires special handling due to its column-oriented structure:

```yaml
parser:
  parser_type: column_oriented
  location_id_field: properties.gridId
  column_config:
    metrics_base_path: properties
    timestamp_format:
      type: iso8601_duration
```

### 2.2 Metrics Base Path

All forecast metrics are nested under `properties`:

```yaml
metrics_base_path: raw_payload.properties
```

### 2.3 Column Extraction List

#### Core Metrics (Dashboard-Critical)

| API Path | Bronze Field | Unit | Description |
|----------|--------------|------|-------------|
| `temperature` | temperature | degC | Air temperature |
| `dewpoint` | dewpoint | degC | Dewpoint temperature |
| `relativeHumidity` | relative_humidity | percent | Relative humidity |
| `windSpeed` | wind_speed | km/h | Wind speed |
| `windDirection` | wind_direction | degrees | Wind direction (0-360) |
| `windGust` | wind_gust | km/h | Wind gust speed |
| `probabilityOfPrecipitation` | probability_of_precipitation | percent | Precipitation probability |
| `skyCover` | sky_cover | percent | Cloud cover |
| `visibility` | visibility | meters | Visibility distance |

#### Comfort Metrics (Derived Conditions)

| API Path | Bronze Field | Unit | Description |
|----------|--------------|------|-------------|
| `apparentTemperature` | apparent_temperature | degC | Feels-like temperature |
| `heatIndex` | heat_index | degC | Heat index (when temp > 26C) |
| `windChill` | wind_chill | degC | Wind chill (when temp < 10C) |
| `wetBulbGlobeTemperature` | wet_bulb_globe_temperature | degC | WBGT (heat stress) |

#### Daily Extremes

| API Path | Bronze Field | Unit | Description |
|----------|--------------|------|-------------|
| `maxTemperature` | max_temperature | degC | Daily maximum temperature |
| `minTemperature` | min_temperature | degC | Daily minimum temperature |

#### Precipitation Quantities

| API Path | Bronze Field | Unit | Description |
|----------|--------------|------|-------------|
| `quantitativePrecipitation` | quantitative_precipitation | mm | Precipitation amount |
| `snowfallAmount` | snowfall_amount | mm | Snowfall amount |
| `iceAccumulation` | ice_accumulation | mm | Ice accumulation |

#### Fire Weather & Indices

| API Path | Bronze Field | Unit | Description |
|----------|--------------|------|-------------|
| `hainesIndex` | haines_index | index | Haines fire weather index (2-6) |
| `mixingHeight` | mixing_height | meters | Atmospheric mixing height |
| `probabilityOfThunder` | probability_of_thunder | percent | Thunderstorm probability |
| `redFlagThreatIndex` | red_flag_threat_index | index | Red flag threat level |

### 2.4 Timestamp Format Handling

NWS uses ISO 8601 duration intervals that require parsing:

```yaml
timestamp_format:
  type: iso8601_duration
  # Parses "2026-01-01T07:00:00+00:00/PT2H" into:
  #   valid_time: 2026-01-01T07:00:00Z
  #   valid_duration: PT2H (INTERVAL '2 hours')
```

---

## 3. Target Schema

### 3.1 Table Definition

```sql
-- Silver layer forecast table
CREATE TABLE silver.nws_forecasts (
    -- Audit/debugging
    ingestion_time          TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- DOMAIN KEYS (Critical for forecast evaluation)
    issue_time              TIMESTAMPTZ NOT NULL,    -- When forecast was generated
    valid_time              TIMESTAMPTZ NOT NULL,    -- When prediction applies
    valid_duration          INTERVAL,                 -- How long valid (PT1H, PT6H)

    -- Derived: Essential for accuracy analysis
    lead_time_hours         INTEGER GENERATED ALWAYS AS
                            (EXTRACT(EPOCH FROM valid_time - issue_time) / 3600) STORED,

    -- Location identifiers
    ndp_id                  TEXT NOT NULL,
    grid_office             TEXT,                     -- e.g., "JAX"
    grid_x                  INTEGER,
    grid_y                  INTEGER,

    -- Core Temperature Metrics
    temperature_c           DOUBLE PRECISION,
    dewpoint_c              DOUBLE PRECISION,

    -- Comfort Metrics
    apparent_temp_c         DOUBLE PRECISION,
    heat_index_c            DOUBLE PRECISION,
    wind_chill_c            DOUBLE PRECISION,

    -- Wind Metrics
    wind_speed_kmh          DOUBLE PRECISION,
    wind_direction_deg      DOUBLE PRECISION,
    wind_gust_kmh           DOUBLE PRECISION,

    -- Humidity & Precipitation
    humidity_pct            DOUBLE PRECISION,
    precip_probability_pct  DOUBLE PRECISION,
    precip_amount_mm        DOUBLE PRECISION,

    -- Sky & Visibility
    sky_cover_pct           DOUBLE PRECISION,
    visibility_m            DOUBLE PRECISION,

    -- DQ transparency
    dq_flags                TEXT[],

    -- Primary key for deduplication
    PRIMARY KEY (issue_time, valid_time, ndp_id)
);

-- Create hypertable partitioned by valid_time (enables join with observations)
SELECT create_hypertable('silver.nws_forecasts', 'valid_time',
    chunk_time_interval => INTERVAL '1 day');

-- Index for lead_time analysis (primary use case)
CREATE INDEX idx_nws_forecasts_lead_time
ON silver.nws_forecasts (lead_time_hours, valid_time);

-- Index for location queries
CREATE INDEX idx_nws_forecasts_ndp_id
ON silver.nws_forecasts (ndp_id, valid_time DESC);

-- Index for issue_time incremental processing
CREATE INDEX idx_nws_forecasts_issue_time
ON silver.nws_forecasts (issue_time DESC);
```

### 3.2 Column Definitions

| Column | Type | Nullable | Unit | Description |
|--------|------|----------|------|-------------|
| `ingestion_time` | TIMESTAMPTZ | NO | - | When row was inserted into Silver |
| `issue_time` | TIMESTAMPTZ | NO | - | When NWS generated this forecast |
| `valid_time` | TIMESTAMPTZ | NO | - | When prediction applies |
| `valid_duration` | INTERVAL | YES | - | How long prediction is valid |
| `lead_time_hours` | INTEGER | NO | hours | Computed: valid_time - issue_time |
| `ndp_id` | TEXT | NO | - | NDP source identifier |
| `grid_office` | TEXT | YES | - | NWS forecast office (JAX, MFL, etc.) |
| `grid_x` | INTEGER | YES | - | Grid X coordinate |
| `grid_y` | INTEGER | YES | - | Grid Y coordinate |
| `temperature_c` | DOUBLE PRECISION | YES | Celsius | Air temperature |
| `dewpoint_c` | DOUBLE PRECISION | YES | Celsius | Dewpoint temperature |
| `apparent_temp_c` | DOUBLE PRECISION | YES | Celsius | Feels-like temperature |
| `heat_index_c` | DOUBLE PRECISION | YES | Celsius | Heat index (when applicable) |
| `wind_chill_c` | DOUBLE PRECISION | YES | Celsius | Wind chill (when applicable) |
| `wind_speed_kmh` | DOUBLE PRECISION | YES | km/h | Wind speed |
| `wind_direction_deg` | DOUBLE PRECISION | YES | degrees | Wind direction (0-360) |
| `wind_gust_kmh` | DOUBLE PRECISION | YES | km/h | Wind gust speed |
| `humidity_pct` | DOUBLE PRECISION | YES | percent | Relative humidity (0-100) |
| `precip_probability_pct` | DOUBLE PRECISION | YES | percent | Precipitation probability (0-100) |
| `precip_amount_mm` | DOUBLE PRECISION | YES | mm | Quantitative precipitation |
| `sky_cover_pct` | DOUBLE PRECISION | YES | percent | Cloud cover (0-100) |
| `visibility_m` | DOUBLE PRECISION | YES | meters | Visibility distance |
| `dq_flags` | TEXT[] | YES | - | Array of DQ rule violations |

### 3.3 Column Naming Convention

Following NDP Silver conventions:

| Suffix | Meaning | Example |
|--------|---------|---------|
| `_c` | Celsius | `temperature_c` |
| `_pct` | Percent (0-100) | `humidity_pct` |
| `_kmh` | Kilometers per hour | `wind_speed_kmh` |
| `_deg` | Degrees (0-360) | `wind_direction_deg` |
| `_m` | Meters | `visibility_m` |
| `_mm` | Millimeters | `precip_amount_mm` |

---

## 4. Data Quality Rules

### 4.1 Range Checks (Physical Constraints)

| Column | Min | Max | Action | Rationale |
|--------|-----|-----|--------|-----------|
| `temperature_c` | -50.0 | 60.0 | flag | Physical limits for Earth surface |
| `dewpoint_c` | -50.0 | 40.0 | flag | Dewpoint cannot exceed temp |
| `apparent_temp_c` | -60.0 | 70.0 | flag | Extended range for wind chill/heat index |
| `heat_index_c` | 20.0 | 70.0 | flag | Only valid when temp > 26C |
| `wind_chill_c` | -70.0 | 10.0 | flag | Only valid when temp < 10C |
| `wind_speed_kmh` | 0.0 | 300.0 | flag | Hurricane-force max |
| `wind_direction_deg` | 0.0 | 360.0 | clamp | Wrap around at 360 |
| `wind_gust_kmh` | 0.0 | 400.0 | flag | Extreme gust max |
| `humidity_pct` | 0.0 | 100.0 | clamp | Physical constraint |
| `precip_probability_pct` | 0.0 | 100.0 | clamp | Probability constraint |
| `precip_amount_mm` | 0.0 | 500.0 | flag | Single period max |
| `sky_cover_pct` | 0.0 | 100.0 | clamp | Physical constraint |
| `visibility_m` | 0.0 | 50000.0 | flag | Clear day max |

### 4.2 Cross-Field Rules (Domain Logic)

#### Rule: Wind Gust >= Wind Speed

```yaml
- rule: cross_field_check
  name: wind_gust_gte_speed
  expression: "wind_gust_kmh IS NULL OR wind_speed_kmh IS NULL OR wind_gust_kmh >= wind_speed_kmh"
  message: "gust_less_than_sustained"
  action: flag
```

**Rationale**: By definition, a wind gust is a brief increase above sustained wind speed. A gust cannot be less than the sustained speed.

#### Rule: Valid Time After Issue Time

```yaml
- rule: cross_field_check
  name: valid_time_after_issue
  expression: "valid_time >= issue_time"
  message: "valid_time_before_issue"
  action: flag
```

**Rationale**: Forecasts predict future conditions. The valid_time (when the prediction applies) must be at or after the issue_time (when the forecast was generated).

#### Rule: Forecast Horizon Limit

```yaml
- rule: cross_field_check
  name: forecast_horizon_reasonable
  expression: "EXTRACT(EPOCH FROM (valid_time - issue_time)) <= 604800"
  message: "forecast_horizon_exceeds_7_days"
  action: flag
```

**Rationale**: NWS gridpoints forecasts extend only 7 days (168 hours = 604,800 seconds). Any lead_time exceeding this indicates data error.

#### Rule: Dewpoint <= Temperature

```yaml
- rule: cross_field_check
  name: dewpoint_lte_temp
  expression: "dewpoint_c IS NULL OR temperature_c IS NULL OR dewpoint_c <= temperature_c"
  message: "dewpoint_exceeds_temperature"
  action: flag
```

**Rationale**: Dewpoint physically cannot exceed air temperature (supersaturation would occur).

### 4.3 DQ Action Definitions

| Action | Behavior | Use Case |
|--------|----------|----------|
| `flag` | Keep value, add rule name to dq_flags array | Most range violations |
| `reject` | Set to NULL, add to dq_flags | Invalid data that shouldn't be used |
| `clamp` | Clamp to min/max, add to dq_flags | Bounded values (percentages) |
| `drop` | Drop entire row | Critical field violations |

---

## 5. Deduplication Strategy

### 5.1 Primary Key Selection

The primary key `(issue_time, valid_time, ndp_id)` is critical for forecast accuracy analysis:

```yaml
deduplication:
  enabled: true
  key_columns: [issue_time, valid_time, ndp_id]
  strategy: upsert
```

**Rationale**:

1. **issue_time**: Different forecasts are issued over time
2. **valid_time**: Each forecast covers multiple future time slots
3. **ndp_id**: Multiple locations may be tracked

### 5.2 Upsert Strategy

The `upsert` strategy handles:

- **Re-ingestion**: Same Bronze file processed again
- **Late arrivals**: Out-of-order data processing
- **Corrections**: NWS occasionally republishes corrections

```sql
-- UPSERT pattern
INSERT INTO silver.nws_forecasts (...)
VALUES (...)
ON CONFLICT (issue_time, valid_time, ndp_id) DO UPDATE SET
    ingestion_time = EXCLUDED.ingestion_time,
    temperature_c = EXCLUDED.temperature_c,
    -- ... other columns
    dq_flags = EXCLUDED.dq_flags;
```

### 5.3 Forecast Update Pattern

Understanding how forecasts evolve is key to proper deduplication:

```
Target: valid_time = 2026-01-02T12:00Z

issue_time=2026-01-01T06:00 -> lead_time=30h -> temp=22C (initial)
issue_time=2026-01-02T00:00 -> lead_time=12h -> temp=21C (revised)
issue_time=2026-01-02T06:00 -> lead_time=6h  -> temp=20C (updated)
```

Each row represents a **distinct forecast** for the same target time. All are kept because:
- Each has a different `issue_time`
- Lead time analysis requires comparing forecasts at different lead times

---

## 6. Sample Configuration YAML

The following complete `silver_etl` section can be added to the stream configuration:

```yaml
# NWS Gridpoints Forecast Silver ETL Configuration
# File: config/base/streams/nws-gridpoints-forecast/config.yaml
# Add to or replace existing silver_etl section

silver_etl:
  enabled: true
  target_table: silver.nws_forecasts
  target_schema: nws_forecasts_v1
  stream_type: forecasts

  # Pre-transform: Column-oriented array explosion
  pre_transform:
    parser_type: column_oriented
    metrics_base_path: raw_payload.properties
    issue_time_path: raw_payload.properties.updateTime

    # Array explosion configuration
    array_explosion:
      enabled: true
      # Each metric has a "values" array with {validTime, value} pairs
      values_path: values
      valid_time_field: validTime
      value_field: value
      # Parse ISO 8601 duration intervals
      timestamp_format: iso8601_duration

    # Columns to extract and explode
    columns:
      # Temperature Suite
      - metric_path: temperature
        field_name: temperature_c
        unit: celsius
      - metric_path: dewpoint
        field_name: dewpoint_c
        unit: celsius
      - metric_path: apparentTemperature
        field_name: apparent_temp_c
        unit: celsius
      - metric_path: heatIndex
        field_name: heat_index_c
        unit: celsius
      - metric_path: windChill
        field_name: wind_chill_c
        unit: celsius

      # Wind Suite
      - metric_path: windSpeed
        field_name: wind_speed_kmh
        unit: km/h
      - metric_path: windDirection
        field_name: wind_direction_deg
        unit: degrees
      - metric_path: windGust
        field_name: wind_gust_kmh
        unit: km/h

      # Humidity & Precipitation
      - metric_path: relativeHumidity
        field_name: humidity_pct
        unit: percent
      - metric_path: probabilityOfPrecipitation
        field_name: precip_probability_pct
        unit: percent
      - metric_path: quantitativePrecipitation
        field_name: precip_amount_mm
        unit: mm

      # Sky & Visibility
      - metric_path: skyCover
        field_name: sky_cover_pct
        unit: percent
      - metric_path: visibility
        field_name: visibility_m
        unit: meters

  # Primary timestamp mapping (issue_time)
  # Bronze timestamp is microseconds since epoch - same as all other streams
  timestamp:
    source_field: issue_time        # From pre-transform output (microseconds)
    target_field: issue_time
    transform: microseconds_to_timestamp

  # Identity fields
  identity_fields:
    - source: ndp_id
      target: ndp_id
    - source: context.source_type.grid_office
      target: grid_office
    - source: raw_payload.properties.gridX
      target: grid_x
    - source: raw_payload.properties.gridY
      target: grid_y

  # Field mappings with DQ rules
  field_mappings:
    # CRITICAL: valid_time timestamp handling
    # Pre-transform outputs valid_time as Unix SECONDS (from ColumnOrientedParser's forecast_valid_time tag)
    # Use unix_seconds transform to convert to TIMESTAMPTZ (consistent with all Silver streams)
    - source_path: valid_time         # From pre-transform output (unix seconds)
      target_column: valid_time
      type: timestamptz
      nullable: false
      transform:
        type: timestamp
        format: unix_seconds          # Generates: to_timestamp(valid_time) AS valid_time

    # Temperature Suite
    - source_path: temperature_c
      target_column: temperature_c
      type: double_precision
      nullable: true
      dq_rules:
        - rule: range_check
          min: -50.0
          max: 60.0
          action: flag

    - source_path: dewpoint_c
      target_column: dewpoint_c
      type: double_precision
      nullable: true
      dq_rules:
        - rule: range_check
          min: -50.0
          max: 40.0
          action: flag

    - source_path: apparent_temp_c
      target_column: apparent_temp_c
      type: double_precision
      nullable: true
      dq_rules:
        - rule: range_check
          min: -60.0
          max: 70.0
          action: flag

    - source_path: heat_index_c
      target_column: heat_index_c
      type: double_precision
      nullable: true
      dq_rules:
        - rule: range_check
          min: 20.0
          max: 70.0
          action: flag

    - source_path: wind_chill_c
      target_column: wind_chill_c
      type: double_precision
      nullable: true
      dq_rules:
        - rule: range_check
          min: -70.0
          max: 10.0
          action: flag

    # Wind Suite
    - source_path: wind_speed_kmh
      target_column: wind_speed_kmh
      type: double_precision
      nullable: true
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 300.0
          action: flag

    - source_path: wind_direction_deg
      target_column: wind_direction_deg
      type: double_precision
      nullable: true
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 360.0
          action: clamp

    - source_path: wind_gust_kmh
      target_column: wind_gust_kmh
      type: double_precision
      nullable: true
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 400.0
          action: flag

    # Humidity & Precipitation
    - source_path: humidity_pct
      target_column: humidity_pct
      type: double_precision
      nullable: true
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 100.0
          action: clamp

    - source_path: precip_probability_pct
      target_column: precip_probability_pct
      type: double_precision
      nullable: true
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 100.0
          action: clamp

    - source_path: precip_amount_mm
      target_column: precip_amount_mm
      type: double_precision
      nullable: true
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 500.0
          action: flag

    # Sky & Visibility
    - source_path: sky_cover_pct
      target_column: sky_cover_pct
      type: double_precision
      nullable: true
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 100.0
          action: clamp

    - source_path: visibility_m
      target_column: visibility_m
      type: double_precision
      nullable: true
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 50000.0
          action: flag

  # Cross-field DQ rules
  dq_rules:
    - rule: cross_field_check
      name: wind_gust_gte_speed
      expression: "wind_gust_kmh IS NULL OR wind_speed_kmh IS NULL OR wind_gust_kmh >= wind_speed_kmh"
      message: "gust_less_than_sustained"
      action: flag

    - rule: cross_field_check
      name: valid_time_after_issue
      expression: "valid_time >= issue_time"
      message: "valid_time_before_issue"
      action: flag

    - rule: cross_field_check
      name: forecast_horizon_reasonable
      expression: "EXTRACT(EPOCH FROM (valid_time - issue_time)) <= 604800"
      message: "forecast_horizon_exceeds_7_days"
      action: flag

    - rule: cross_field_check
      name: dewpoint_lte_temp
      expression: "dewpoint_c IS NULL OR temperature_c IS NULL OR dewpoint_c <= temperature_c"
      message: "dewpoint_exceeds_temperature"
      action: flag

  # DQ output configuration
  dq_output:
    enabled: true
    target_column: dq_flags
    include_rules: true
    include_values: false

  # Deduplication strategy
  deduplication:
    enabled: true
    key_columns: [issue_time, valid_time, ndp_id]
    strategy: upsert

  # Incremental processing
  incremental:
    enabled: true
    watermark_column: issue_time
    lag_interval: 1 hour
```

---

## 7. ETL Implementation Notes

### 7.1 Pre-Transform Using ColumnOrientedParser

The NWS gridpoints response has a column-oriented structure where each metric contains an array of values. This is handled by the existing `ColumnOrientedParser` from `neural-core` (see `core/src/parsers/column_oriented.rs`).

**Bronze Data Structure:**
```json
{
  "temperature": {
    "uom": "wmoUnit:degC",
    "values": [
      {"validTime": "2026-01-01T07:00:00+00:00/PT2H", "value": 3.333},
      {"validTime": "2026-01-01T09:00:00+00:00/PT2H", "value": 2.777}
    ]
  }
}
```

**Pre-Transform Output (FlattenedRow):**

| issue_time (µs) | valid_time (sec) | ndp_id | metric_name | value |
|-----------------|------------------|--------|-------------|-------|
| 1735729009000000 | 1735714800 | weather-nws-002 | temperature | 3.333 |
| 1735729009000000 | 1735722000 | weather-nws-002 | temperature | 2.777 |

**CRITICAL: Timestamp Formats**

| Field | Pre-Transform Output | Transform | Silver Column |
|-------|---------------------|-----------|---------------|
| `issue_time` | i64 microseconds | `microseconds_to_timestamp` | TIMESTAMPTZ |
| `valid_time` | i64 unix seconds | `unix_seconds` | TIMESTAMPTZ |

The `valid_time` uses Unix seconds because that's what `ColumnOrientedParser` produces in the `forecast_valid_time` tag (see `column_oriented.rs:285`):

```rust
tags.insert(
    "forecast_valid_time".to_string(),
    element_timestamp.timestamp().to_string(),  // Unix SECONDS
);
```

The existing silver-etl `TransformConfig::Timestamp { format: unix_seconds }` handles this conversion via:
```sql
to_timestamp(valid_time) AS valid_time  -- Converts unix seconds to TIMESTAMPTZ
```

**After DuckDB Transform (Silver Row):**

| issue_time | valid_time | ndp_id | temperature_c |
|------------|------------|--------|---------------|
| 2026-01-01T13:56:49Z | 2026-01-01T07:00:00Z | weather-nws-002 | 3.333 |
| 2026-01-01T13:56:49Z | 2026-01-01T09:00:00Z | weather-nws-002 | 2.777 |

### 7.2 DuckDB SQL Pattern

```sql
-- Array explosion using UNNEST
WITH bronze_data AS (
    SELECT
        timestamp as bronze_ts,
        ndp_id,
        json_extract(raw_payload, '$.properties.updateTime') as issue_time_str,
        json_extract(raw_payload, '$.properties.temperature.values') as temp_values,
        json_extract(raw_payload, '$.properties.dewpoint.values') as dewpoint_values
        -- ... more metrics
    FROM read_parquet('/data/raw/nws-gridpoints-forecast/**/*.parquet')
    WHERE to_timestamp(timestamp / 1000000) > :last_watermark
),
exploded AS (
    SELECT
        strptime(issue_time_str::VARCHAR, '%Y-%m-%dT%H:%M:%S%z') as issue_time,
        -- Parse ISO 8601 interval: split on '/', parse start, parse duration
        strptime(
            split_part(json_extract_string(t.value, '$.validTime'), '/', 1),
            '%Y-%m-%dT%H:%M:%S%z'
        ) as valid_time,
        split_part(json_extract_string(t.value, '$.validTime'), '/', 2) as valid_duration_str,
        ndp_id,
        json_extract(t.value, '$.value')::DOUBLE as temperature_c
    FROM bronze_data b,
    UNNEST(json_extract(temp_values, '$[*]')) as t(value)
)
SELECT * FROM exploded;
```

### 7.3 Variable Time Resolutions

Different metrics have different time resolutions:

| Metric | Typical Resolution | Array Size |
|--------|-------------------|------------|
| temperature | PT1H, PT2H | ~130 values |
| maxTemperature | P1D | ~8 values |
| minTemperature | P1D | ~8 values |
| probabilityOfPrecipitation | PT6H | ~30 values |
| weather | Variable | ~10 values |

The ETL must handle joining metrics with different time grids. Options:

1. **Keep as-is**: Store each metric at its native resolution
2. **Interpolate**: Resample to common resolution (introduces approximation)
3. **Separate tables**: Core hourly table + aggregated daily table

**Recommendation**: Option 1 (keep as-is) for Phase 1, with valid_duration column to indicate resolution.

---

## 8. Forecast Accuracy Integration

### 8.1 Join with Observations

The schema is designed to enable forecast accuracy analysis:

```sql
-- Join forecasts to observations
SELECT
    f.lead_time_hours,
    f.temperature_c as forecast_temp,
    o.temperature_c as observed_temp,
    ABS(f.temperature_c - o.temperature_c) as temp_error
FROM silver.nws_forecasts f
JOIN silver.weather_observations o
  ON f.valid_time = o.observation_time
 AND f.ndp_id = o.ndp_id
WHERE f.lead_time_hours BETWEEN 1 AND 168;
```

### 8.2 Accuracy by Lead Time

Primary analysis pattern:

```sql
SELECT
    lead_time_hours,
    COUNT(*) as sample_count,
    AVG(ABS(f.temperature_c - o.temperature_c)) as avg_temp_error,
    PERCENTILE_CONT(0.9) WITHIN GROUP (ORDER BY ABS(f.temperature_c - o.temperature_c)) as p90_error
FROM silver.nws_forecasts f
JOIN silver.weather_observations o
  ON f.valid_time = o.observation_time
 AND f.ndp_id = o.ndp_id
GROUP BY lead_time_hours
ORDER BY lead_time_hours;
```

### 8.3 Trustworthy Horizon

Determine maximum reliable forecast horizon:

```sql
WITH accuracy_by_lead AS (
    SELECT
        lead_time_hours,
        PERCENTILE_CONT(0.9) WITHIN GROUP (ORDER BY ABS(f.temperature_c - o.temperature_c)) as p90_error
    FROM silver.nws_forecasts f
    JOIN silver.weather_observations o
      ON f.valid_time = o.observation_time
     AND f.ndp_id = o.ndp_id
    WHERE f.valid_time > NOW() - INTERVAL '30 days'
    GROUP BY lead_time_hours
)
SELECT MAX(lead_time_hours) as max_trustworthy_hours
FROM accuracy_by_lead
WHERE p90_error <= 2.0;  -- 2C threshold
```

---

## 9. Related Documents

| Document | Location | Description |
|----------|----------|-------------|
| Weather Domain Model | `product/research/analyticplatforminfrastructure/02-WEATHER-DOMAIN-MODEL.md` | Domain entities and relationships |
| Forecast Evaluation Schema | `product/research/analyticplatforminfrastructure/05-FORECAST-EVALUATION-SCHEMA.md` | Schema design rationale |
| NWS Gridpoints Deep Dive | `product/research/analyticplatforminfrastructure/06-NWS-GRIDPOINTS-DEEP-DIVE.md` | API structure analysis |
| Config-Driven Silver ETL | `docs/architecture/CONFIG_DRIVEN_SILVER_ETL_DESIGN.md` | ETL configuration patterns |
| Stream Configuration | `config/base/streams/nws-gridpoints-forecast/config.yaml` | Existing Bronze config |

---

## 10. Open Questions

| Question | Status | Notes |
|----------|--------|-------|
| Array explosion implementation | Pending | Requires silver-etl enhancement |
| Extended metrics table | Deferred | Fire weather, marine metrics |
| Weather conditions (qualitative) | Deferred | Separate table for text conditions |
| Multi-location support | Future | Support multiple grid points |

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-12 | ndp-meteorologist | Initial specification |

---

## References

1. NWS API Documentation: https://www.weather.gov/documentation/services-web-api
2. WMO Unit Codes: https://codes.wmo.int/common/unit
3. ISO 8601 Duration Format: https://en.wikipedia.org/wiki/ISO_8601#Durations
4. TimescaleDB Hypertables: https://docs.timescale.com/timescaledb/latest/overview/core-concepts/hypertables/
