# AirGradient Sensor Field Analysis

**Analysis Date**: 2025-12-23
**Scope**: AirGradient ONE indoor air quality sensor field naming and data structure
**Purpose**: Understand field naming conventions to guide Silver layer schema design

---

## Executive Summary

AirGradient sensors emit **29 distinct fields** via both MQTT and Local HTTP API. The platform currently handles all fields correctly using **raw field names** (e.g., `pm02`, `atmp`, `rhum`, `rco2`) in Bronze layer, then **transforms them to semantic names** (e.g., `pm25`, `temperature`, `humidity`, `co2`) when converting to TimeSeriesPoints for Silver layer consumption.

**Key Finding**: The current dashboard queries have **field name inconsistencies** - some use raw names (`pm02`, `rco2`, `atmp`, `rhum`) while others use semantic names (`temperature`, `humidity`), causing query failures when fields don't match.

---

## AirGradient Field Inventory

### 1. Device Metadata (7 fields)

| Field Name | Type | Description | Category | Silver Recommendation |
|------------|------|-------------|----------|----------------------|
| `serialno` | String | Device serial number | Identifier | **KEEP** - Used as location_id |
| `wifi` | i32 | WiFi signal strength (dBm) | Diagnostic | **PROMOTE** - Connectivity monitoring |
| `boot` | i32 | Current boot sequence | Diagnostic | OPTIONAL - Boot tracking |
| `boot_count` | i32 | Total boot count | Diagnostic | OPTIONAL - Reliability metric |
| `firmware` | String | Firmware version | Metadata | **KEEP** - Tag in Silver |
| `model` | String | Device model (e.g., I-9PSL) | Metadata | **KEEP** - Tag in Silver |
| `led_mode` | String | LED display mode | Settings | OPTIONAL - Configuration tracking |

**Source**: All fields available via Local HTTP API, subset via MQTT

---

### 2. Particle Matter Data (13 fields)

#### Standard PM Concentrations (µg/m³)
| Field Name | Type | Description | Category | Silver Recommendation |
|------------|------|-------------|----------|----------------------|
| `pm01` | f32 | PM1.0 concentration | Raw | **PROMOTE** - Core air quality metric |
| `pm02` | f32 | PM2.5 concentration | Raw | **PROMOTE** - Most critical PM metric |
| `pm10` | f32 | PM10 concentration | Raw | **PROMOTE** - Core air quality metric |

**Sensor**: Plantower PMS5003

#### Compensated PM Values
| Field Name | Type | Description | Category | Silver Recommendation |
|------------|------|-------------|----------|----------------------|
| `pm02Compensated` | f32 | PM2.5 adjusted for temp/humidity | Compensated | **PROMOTE** - More accurate for indoor |

**Note**: Only PM2.5 has compensated variant. Temperature/humidity compensation improves accuracy.

#### Standard Atmospheric PM
| Field Name | Type | Description | Category | Silver Recommendation |
|------------|------|-------------|----------|----------------------|
| `pm01Standard` | f32 | PM1.0 standard atmospheric | Standard | OPTIONAL - Research/comparison |
| `pm02Standard` | f32 | PM2.5 standard atmospheric | Standard | OPTIONAL - Research/comparison |
| `pm10Standard` | f32 | PM10 standard atmospheric | Standard | OPTIONAL - Research/comparison |

**Context**: Different calculation method for outdoor/atmospheric conditions. Less relevant for indoor monitoring.

#### Particle Counts (per 0.1L air)
| Field Name | Type | Description | Category | Silver Recommendation |
|------------|------|-------------|----------|----------------------|
| `pm003Count` | f32 | Particles >0.3µm | Count | OPTIONAL - Advanced analysis |
| `pm005Count` | f32 | Particles >0.5µm | Count | OPTIONAL - Advanced analysis |
| `pm01Count` | f32 | Particles >1.0µm | Count | OPTIONAL - Advanced analysis |
| `pm02Count` | f32 | Particles >2.5µm | Count | OPTIONAL - Advanced analysis |
| `pm50Count` | f32 | Particles >5.0µm | Count | OPTIONAL - Advanced analysis |
| `pm10Count` | f32 | Particles >10µm | Count | OPTIONAL - Advanced analysis |

**Use Case**: Particle counts useful for research, filter analysis, but not typically needed for basic air quality monitoring.

---

### 3. Gas Sensor Data (4 fields)

| Field Name | Type | Description | Category | Silver Recommendation |
|------------|------|-------------|----------|----------------------|
| `tvocIndex` | i32 | TVOC index (1-500) | Index | **PROMOTE** - User-friendly metric |
| `tvocRaw` | f32 | TVOC raw sensor value | Raw | OPTIONAL - Debugging/calibration |
| `noxIndex` | i32 | NOx index (1-500) | Index | **PROMOTE** - User-friendly metric |
| `noxRaw` | f32 | NOx raw sensor value | Raw | OPTIONAL - Debugging/calibration |

**Sensor**: Sensirion SGP41

**Analysis**:
- **Index values** (1-500) are calibrated, user-friendly metrics suitable for dashboards
- **Raw values** are unprocessed sensor readings, useful for debugging sensor issues
- Recommend promoting index values to Silver, keep raw in Bronze for troubleshooting

---

### 4. Environmental Data (4 fields)

| Field Name | Type | Description | Category | Silver Recommendation |
|------------|------|-------------|----------|----------------------|
| `atmp` | f32 | Ambient temperature (°C) | Raw | **PROMOTE** - Core environmental metric |
| `atmpCompensated` | f32 | Temperature compensated for device heat | Compensated | **PROMOTE** - More accurate |
| `rhum` | f32 | Relative humidity (%) | Raw | **PROMOTE** - Core environmental metric |
| `rhumCompensated` | f32 | Humidity compensated | Compensated | **PROMOTE** - More accurate |

**Sensor**: Sensirion SHT40

**Analysis**:
- **Raw values** (`atmp`, `rhum`) include heat from device electronics
- **Compensated values** adjust for internal device heat, providing true ambient readings
- **Recommendation**: Promote BOTH to Silver - raw for historical compatibility, compensated for accuracy
- Dashboard queries should prefer compensated values when available

---

### 5. Air Quality Metrics (1 field)

| Field Name | Type | Description | Category | Silver Recommendation |
|------------|------|-------------|----------|----------------------|
| `rco2` | i32 | CO2 concentration (ppm) | Metric | **PROMOTE** - Critical indoor air quality |

**Sensor**: SenseAir S8
**Valid Range**: 380-10,000 ppm
**Analysis**: Essential for indoor air quality monitoring, ventilation control

---

## Field Naming Conventions

### Bronze Layer (Raw Field Names)
The platform correctly preserves **raw AirGradient field names** in Bronze (Parquet):

```
pm01, pm02, pm10                    # Particle concentrations
pm02Compensated                      # Compensated PM2.5
pm01Standard, pm02Standard, pm10Standard  # Atmospheric standards
pm003Count, pm01Count, pm02Count, etc.    # Particle counts
atmp, atmpCompensated                # Temperature
rhum, rhumCompensated                # Humidity
rco2                                 # CO2
tvocIndex, tvocRaw                   # TVOC
noxIndex, noxRaw                     # NOx
wifi, serialno, boot, firmware, etc. # Metadata
```

### Silver Layer (Semantic Names)
The `AirQualityAdapter` transforms to human-friendly names:

```rust
// From domains/air-quality/src/adapter.rs
pm02      → "pm25"                  // Renamed for clarity
pm01      → "pm1"
pm10      → "pm10"
pm02Compensated → "pm25_compensated"
atmp      → "temperature"
atmpCompensated → "temperature_compensated"
rhum      → "humidity"
rhumCompensated → "humidity_compensated"
rco2      → "co2"
tvocIndex → "tvoc_index"
tvocRaw   → "tvoc_raw"
noxIndex  → "nox_index"
noxRaw    → "nox_raw"
wifi      → "wifi_signal"
```

**Critical**: The adapter adds a `metric` tag with the semantic name, not the raw field name.

---

## Dashboard Query Issues

### Problem: Inconsistent Field Name Usage

Current dashboards query Bronze layer Parquet files using **BOTH** raw and semantic names:

#### ❌ WRONG - Using raw names that don't exist in Bronze
```sql
-- From indoor-air-quality.json
WHERE metric = 'pm02'      -- ❌ WRONG: Bronze has pm25
WHERE metric = 'rco2'      -- ❌ WRONG: Bronze has co2
WHERE metric IN ('atmp', 'temperature')  -- ❌ Mixing raw/semantic
WHERE metric IN ('rhum', 'humidity')     -- ❌ Mixing raw/semantic
```

#### ✅ CORRECT - Using semantic names
```sql
-- What dashboards SHOULD use
WHERE metric = 'pm25'         -- ✅ Semantic name from adapter
WHERE metric = 'co2'          -- ✅ Semantic name from adapter
WHERE metric = 'temperature'  -- ✅ Semantic name from adapter
WHERE metric = 'humidity'     -- ✅ Semantic name from adapter
```

### Root Cause

The `AirQualityAdapter::to_time_series_points()` function creates TimeSeriesPoints with **semantic names** in the `metric` tag:

```rust
// Line 48-54: CO2 uses semantic name
if let Some(co2) = reading.metrics.rco2 {
    points.push(TimeSeriesPoint {
        tags: make_tags("co2"),  // ← Uses "co2", not "rco2"
    });
}

// Line 66-72: PM2.5 uses semantic name
if let Some(pm02) = reading.particles.pm02 {
    points.push(TimeSeriesPoint {
        tags: make_tags("pm25"),  // ← Uses "pm25", not "pm02"
    });
}
```

But Bronze Parquet files store the **semantic name** in the `metric` column because they're created from TimeSeriesPoints.

---

## Data Flow Analysis

```
┌─────────────────────────────────────────────────────────────────┐
│ 1. MQTT/HTTP Source                                             │
│    Raw JSON with AirGradient field names                        │
│    { "pm02": 12.5, "rco2": 450, "atmp": 22.5, "rhum": 45.0 }   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ 2. Parser (domains/air-quality/src/parser.rs)                   │
│    Deserializes to AirQualityReading struct                     │
│    Preserves raw field names in struct fields                   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ 3. AirQualityAdapter (domains/air-quality/src/adapter.rs)       │
│    Transforms to TimeSeriesPoint with SEMANTIC names            │
│    pm02 → metric="pm25"                                         │
│    rco2 → metric="co2"                                          │
│    atmp → metric="temperature"                                  │
│    rhum → metric="humidity"                                     │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ 4. Bronze Layer (Parquet)                                       │
│    Stores TimeSeriesPoint with semantic metric names            │
│    metric | value | timestamp | location_id | tags             │
│    ─────────────────────────────────────────────────────────    │
│    pm25   | 12.5  | ...       | sensor123   | {...}            │
│    co2    | 450   | ...       | sensor123   | {...}            │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│ 5. Silver Layer (Future: TimescaleDB)                           │
│    Query using SEMANTIC names from Bronze                       │
│    SELECT * FROM metrics WHERE metric = 'pm25'                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Silver Layer Recommendations

### Core Metrics (MUST PROMOTE)

These fields are essential for indoor air quality monitoring and should be promoted to Silver layer with aggregations, retention policies, and continuous aggregates:

| Metric Category | Fields | Aggregation Strategy |
|----------------|--------|----------------------|
| **Particulate Matter** | `pm25`, `pm25_compensated`, `pm1`, `pm10` | Hourly/daily AVG, MIN, MAX |
| **CO2** | `co2` | Hourly/daily AVG, alerting thresholds |
| **Temperature** | `temperature`, `temperature_compensated` | Hourly/daily AVG, MIN, MAX |
| **Humidity** | `humidity`, `humidity_compensated` | Hourly/daily AVG, MIN, MAX |
| **Gas Indices** | `tvoc_index`, `nox_index` | Hourly/daily AVG |

**Total Core Metrics**: 11 fields

### Secondary Metrics (OPTIONAL)

Useful for diagnostics, research, or advanced analysis:

| Metric Category | Fields | Use Case |
|----------------|--------|----------|
| **Device Health** | `wifi_signal` | Connectivity monitoring |
| **Particle Counts** | `pm003_count`, `pm01_count`, `pm02_count` | Filter analysis, research |
| **Raw Gas Sensors** | `tvoc_raw`, `nox_raw` | Sensor calibration, debugging |
| **Standard PM** | `pm01_standard`, `pm02_standard`, `pm10_standard` | Outdoor/atmospheric comparison |

**Total Secondary Metrics**: 10 fields

### Metadata (TAG ONLY)

These fields should be stored as **tags** in TimescaleDB, not as separate metric rows:

- `location_id` (from `serialno`)
- `firmware`
- `model`
- `led_mode`

---

## Raw vs Compensated Values

### Temperature/Humidity Compensation

AirGradient devices generate internal heat from electronics (CPU, sensors, etc.). This affects raw readings:

| Field | Description | Typical Offset | Recommendation |
|-------|-------------|----------------|----------------|
| `atmp` | Raw temperature | +1-2°C warmer | Keep for historical compatibility |
| `atmpCompensated` | Compensated temperature | Accurate ambient | **PREFER for analysis** |
| `rhum` | Raw humidity | -2-3% lower | Keep for historical compatibility |
| `rhumCompensated` | Compensated humidity | Accurate ambient | **PREFER for analysis** |

**Dashboard Guidance**:
- Use compensated values for real-time displays
- Use raw values for historical trend comparison (if previously using raw)
- Store BOTH in Silver for flexibility

### PM2.5 Compensation

| Field | Description | Compensation Factor | Recommendation |
|-------|-------------|---------------------|----------------|
| `pm02` | Raw PM2.5 | None | Good for outdoor conditions |
| `pm02Compensated` | Compensated PM2.5 | Temperature/humidity adjusted | **PREFER for indoor** |

**Context**: PM sensors' accuracy varies with temperature and humidity. Compensated values adjust for environmental conditions.

---

## Field Categories Summary

### Ambient Measurements (What users care about)
- ✅ **PROMOTE**: `pm25`, `pm25_compensated`, `pm1`, `pm10`, `co2`, `temperature`, `temperature_compensated`, `humidity`, `humidity_compensated`, `tvoc_index`, `nox_index`
- 📊 Total: 11 fields

### Device Internal / Diagnostic
- ⚠️ **BRONZE ONLY**: `wifi_signal`, `boot`, `boot_count`
- 📊 Total: 3 fields

### Research / Advanced
- 🔬 **OPTIONAL**: Particle counts, standard PM, raw gas sensors
- 📊 Total: 10 fields

### Metadata
- 🏷️ **TAGS**: `firmware`, `model`, `led_mode`, `serialno` (location_id)
- 📊 Total: 4 fields

---

## Dashboard Query Fixes Required

### File: `config/grafana/dashboards/indoor-air-quality.json`

#### Current (BROKEN)
```sql
-- Line 23: PM2.5 query
WHERE metric = 'pm02'  -- ❌ Field doesn't exist

-- Line 56: CO2 query
WHERE metric = 'rco2'  -- ❌ Field doesn't exist

-- Line 88: Temperature query
WHERE metric IN ('atmp', 'temperature')  -- ⚠️ Only 'temperature' exists

-- Line 112: Humidity query
WHERE metric IN ('rhum', 'humidity')  -- ⚠️ Only 'humidity' exists
```

#### Recommended (FIXED)
```sql
-- PM2.5: Use compensated if available, fallback to raw
WHERE metric IN ('pm25_compensated', 'pm25')

-- CO2: Use semantic name
WHERE metric = 'co2'

-- Temperature: Use compensated if available, fallback to raw
WHERE metric IN ('temperature_compensated', 'temperature')

-- Humidity: Use compensated if available, fallback to raw
WHERE metric IN ('humidity_compensated', 'humidity')
```

### File: `config/grafana/dashboards/indoor-vs-outdoor.json`

Same issues in comparison queries - need to use semantic names consistently.

---

## Silver Layer Schema Proposal

### TimescaleDB Hypertable

```sql
CREATE TABLE air_quality_metrics (
  time         TIMESTAMPTZ NOT NULL,
  location_id  TEXT NOT NULL,
  metric       TEXT NOT NULL,
  value        DOUBLE PRECISION NOT NULL,
  firmware     TEXT,
  model        TEXT,
  PRIMARY KEY (time, location_id, metric)
);

-- Create hypertable with daily partitioning
SELECT create_hypertable('air_quality_metrics', 'time', chunk_time_interval => INTERVAL '1 day');

-- Create index for metric queries
CREATE INDEX idx_metric ON air_quality_metrics (metric, time DESC);
```

### Continuous Aggregates

```sql
-- Hourly aggregates
CREATE MATERIALIZED VIEW air_quality_hourly
WITH (timescaledb.continuous) AS
SELECT
  time_bucket('1 hour', time) AS hour,
  location_id,
  metric,
  AVG(value) AS avg_value,
  MIN(value) AS min_value,
  MAX(value) AS max_value,
  COUNT(*) AS sample_count
FROM air_quality_metrics
GROUP BY hour, location_id, metric;

-- Daily aggregates
CREATE MATERIALIZED VIEW air_quality_daily
WITH (timescaledb.continuous) AS
SELECT
  time_bucket('1 day', time) AS day,
  location_id,
  metric,
  AVG(value) AS avg_value,
  MIN(value) AS min_value,
  MAX(value) AS max_value,
  STDDEV(value) AS stddev_value,
  COUNT(*) AS sample_count
FROM air_quality_metrics
GROUP BY day, location_id, metric;
```

### Retention Policy

```sql
-- Keep raw data for 90 days
SELECT add_retention_policy('air_quality_metrics', INTERVAL '90 days');

-- Keep hourly aggregates for 2 years
SELECT add_retention_policy('air_quality_hourly', INTERVAL '2 years');

-- Keep daily aggregates indefinitely
```

---

## Migration Strategy

### Phase 1: Fix Dashboard Queries (Immediate)
1. Update all dashboard JSON files to use semantic field names
2. Test queries against existing Bronze Parquet data
3. Deploy updated dashboards

### Phase 2: Silver Layer Design (Current Phase)
1. Design TimescaleDB schema (this document)
2. Create ETL pipeline from Bronze to Silver
3. Implement continuous aggregates

### Phase 3: Production Deployment
1. Populate Silver layer from Bronze backfill
2. Switch dashboards to query Silver instead of Bronze
3. Monitor query performance and data accuracy

---

## References

### Documentation
- [AirGradient Firmware Versions](https://www.airgradient.com/documentation/firmwares/)
- [AirGradient MQTT Configuration](https://www.airgradient.com/support/kb-mqtt-conf/)
- [Jeff Geerling AirGradient Review](https://www.jeffgeerling.com/blog/2021/airgradient-diy-air-quality-monitor-co2-pm25)
- [OpenHAB AirGradient Binding](https://www.openhab.org/addons/bindings/airgradient/)

### Code References
- `/workspaces/neural-data-platform/domains/air-quality/src/types.rs` - Field definitions
- `/workspaces/neural-data-platform/domains/air-quality/src/adapter.rs` - Name transformations
- `/workspaces/neural-data-platform/domains/air-quality/src/parser.rs` - JSON parsing
- `/workspaces/neural-data-platform/config/base/streams/air-quality/config.yaml` - Stream config
- `/workspaces/neural-data-platform/config/grafana/dashboards/indoor-air-quality.json` - Dashboard queries

---

## Appendix: Complete Field Matrix

| AirGradient Field | Rust Type | Category | Bronze Name | Silver Name | Promote? | Reason |
|-------------------|-----------|----------|-------------|-------------|----------|---------|
| `serialno` | String | Metadata | serialno | location_id | ✅ TAG | Device identifier |
| `wifi` | i32 | Diagnostic | wifi | wifi_signal | ⚠️ | Connectivity monitoring |
| `boot` | i32 | Diagnostic | boot | boot | ❌ | Internal counter |
| `boot_count` | i32 | Diagnostic | boot_count | boot_count | ❌ | Internal counter |
| `firmware` | String | Metadata | firmware | firmware | ✅ TAG | Version tracking |
| `model` | String | Metadata | model | model | ✅ TAG | Device type |
| `led_mode` | String | Settings | led_mode | led_mode | ✅ TAG | Configuration |
| `pm01` | f32 | Particle | pm01 | pm1 | ✅ | Core PM metric |
| `pm02` | f32 | Particle | pm02 | pm25 | ✅ | Most critical PM |
| `pm10` | f32 | Particle | pm10 | pm10 | ✅ | Core PM metric |
| `pm02Compensated` | f32 | Particle | pm02Compensated | pm25_compensated | ✅ | More accurate indoor |
| `pm01Standard` | f32 | Particle | pm01Standard | pm1_standard | ⚠️ | Research/comparison |
| `pm02Standard` | f32 | Particle | pm02Standard | pm25_standard | ⚠️ | Research/comparison |
| `pm10Standard` | f32 | Particle | pm10Standard | pm10_standard | ⚠️ | Research/comparison |
| `pm003Count` | f32 | Count | pm003Count | pm003_count | ⚠️ | Advanced analysis |
| `pm005Count` | f32 | Count | pm005Count | pm005_count | ⚠️ | Advanced analysis |
| `pm01Count` | f32 | Count | pm01Count | pm01_count | ⚠️ | Advanced analysis |
| `pm02Count` | f32 | Count | pm02Count | pm02_count | ⚠️ | Advanced analysis |
| `pm50Count` | f32 | Count | pm50Count | pm50_count | ⚠️ | Advanced analysis |
| `pm10Count` | f32 | Count | pm10Count | pm10_count | ⚠️ | Advanced analysis |
| `tvocIndex` | i32 | Gas | tvocIndex | tvoc_index | ✅ | User-friendly metric |
| `tvocRaw` | f32 | Gas | tvocRaw | tvoc_raw | ⚠️ | Debugging |
| `noxIndex` | i32 | Gas | noxIndex | nox_index | ✅ | User-friendly metric |
| `noxRaw` | f32 | Gas | noxRaw | nox_raw | ⚠️ | Debugging |
| `atmp` | f32 | Environment | atmp | temperature | ✅ | Core metric |
| `atmpCompensated` | f32 | Environment | atmpCompensated | temperature_compensated | ✅ | More accurate |
| `rhum` | f32 | Environment | rhum | humidity | ✅ | Core metric |
| `rhumCompensated` | f32 | Environment | rhumCompensated | humidity_compensated | ✅ | More accurate |
| `rco2` | i32 | Quality | rco2 | co2 | ✅ | Critical indoor metric |

**Legend**:
- ✅ **PROMOTE** - Essential for Silver layer
- ⚠️ **OPTIONAL** - Useful for specific use cases
- ❌ **BRONZE ONLY** - Internal/diagnostic data
- 🏷️ **TAG** - Store as metadata tag, not metric row

---

## Conclusion

The AirGradient sensor provides rich, comprehensive air quality data with **29 fields** covering particles, gases, environment, and device metadata. The platform correctly handles these fields through a two-layer naming approach:

1. **Bronze Layer**: Preserves raw AirGradient field names in Rust structs
2. **Silver Layer**: Transforms to semantic names via `AirQualityAdapter`

The current issue is **dashboard queries using raw field names** that no longer exist after adapter transformation. Fixing these queries to use semantic names will resolve the immediate problem.

For the Silver layer design, recommend promoting **11 core metrics** with continuous aggregates for hourly/daily analysis, while keeping particle counts and raw sensor values optional for advanced use cases.
