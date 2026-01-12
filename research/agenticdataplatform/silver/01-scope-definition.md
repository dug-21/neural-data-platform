# Silver Layer Scope Definition

**Author**: NDP Architect
**Date**: 2026-01-05
**Status**: Research - Draft
**Feature**: dp-006 (Silver Layer Implementation)

---

## Executive Summary

This document defines the scope for the NDP Silver layer, which transforms raw Bronze JSON data into queryable, typed TimescaleDB hypertables. The design prioritizes Pi resource constraints, domain-specific normalization, and future ML feature engineering.

### Key Recommendations

| Aspect | Recommendation |
|--------|----------------|
| **Entity Structure** | 4 domain tables + 1 data dictionary schema |
| **Normalization** | Denormalized for time-series (wide tables per domain) |
| **Field Priority** | Core analytics fields first; metadata/diagnostic fields deferred |
| **Stream Mapping** | Multiple Bronze streams may feed single Silver entity |

---

## 1. Current Bronze Layer State

### Active Streams (7 total)

| Stream ID | Source | Update Frequency | Payload Size | Purpose |
|-----------|--------|-----------------|--------------|---------|
| `air-quality` | AirGradient MQTT | ~60s | ~600 bytes | Indoor air quality monitoring |
| `outdoor-weather` | OpenWeatherMap API | 10 min | ~800 bytes | Current weather conditions |
| `outdoor-air-quality` | OpenWeatherMap API | 10 min | ~400 bytes | Outdoor pollution levels |
| `nws-forecast-hourly` | NWS API | ~1 hour | ~120KB | Hourly forecast (156 periods) |
| `nws-station-observations` | NWS API | ~30 min | ~4KB | Airport weather station |
| `nws-observations` | NWS API | ~30 min | ~4KB | NWS observations (duplicate?) |
| `nws-gridpoints-forecast` | NWS API | ~1 hour | ~260KB | Full gridpoint forecast (40+ metrics) |

### Bronze Schema (Current)

```
timestamp    | source_id  | ndp_id              | context              | raw_payload
DateTime     | String     | String (nullable)   | JSON (nullable)      | JSON
```

All parsing deferred to Silver layer per ADR-001 (dp-004).

---

## 2. Recommended Silver Layer Entities

### Entity Design Philosophy

**Key Principle**: Denormalize for time-series performance.

Traditional OLTP normalization (3NF) optimizes for write consistency. Time-series workloads are read-heavy with append-only writes. We optimize for:

1. **Query locality**: All fields for a measurement in one row
2. **Partition efficiency**: TimescaleDB hypertables partition by time
3. **Aggregation speed**: Continuous aggregates over single tables
4. **Join avoidance**: Denormalized data eliminates runtime joins

### Proposed Entities

#### Entity 1: `silver.indoor_air_quality`

**Purpose**: Indoor environmental monitoring from AirGradient sensors.

**Source Streams**: `air-quality`

| Column | Type | Unit | Source Path | Notes |
|--------|------|------|-------------|-------|
| `time` | TIMESTAMPTZ | - | Bronze timestamp | Hypertable dimension |
| `ndp_id` | TEXT | - | ndp_id | Indexed, stable ID |
| `location_path` | TEXT | - | context.location.path | e.g., "/beachhouse/livingroom" |
| `pm25` | DOUBLE PRECISION | ug/m3 | raw_payload.pm02 | Primary metric |
| `pm10` | DOUBLE PRECISION | ug/m3 | raw_payload.pm10 | Coarse particles |
| `pm01` | DOUBLE PRECISION | ug/m3 | raw_payload.pm01 | Ultrafine particles |
| `co2` | INTEGER | ppm | raw_payload.rco2 | CO2 concentration |
| `temperature` | DOUBLE PRECISION | C | raw_payload.atmp | Ambient temp |
| `humidity` | DOUBLE PRECISION | % | raw_payload.rhum | Relative humidity |
| `tvoc_index` | INTEGER | index | raw_payload.tvocIndex | TVOC index (1-500) |
| `nox_index` | INTEGER | index | raw_payload.noxIndex | NOx index (1-500) |

**Deferred Fields** (Phase 2):
- `pm003Count`, `pm005Count` - Particle counts by size
- `wifi` - Signal strength (diagnostic)
- `boot`, `bootCount` - Device reboot tracking
- `firmware`, `model`, `serialno` - Device metadata

**Validation Rules**:
- `pm25`: 0-500 ug/m3 (EPA AQI scale max)
- `co2`: 400-5000 ppm (outdoor baseline to OSHA limit)
- `temperature`: -10 to 50 C (indoor realistic)
- `humidity`: 0-100%

---

#### Entity 2: `silver.outdoor_weather`

**Purpose**: Current outdoor weather conditions.

**Source Streams**: `outdoor-weather`, `nws-station-observations`

**Design Decision**: Combine OpenWeatherMap and NWS observations into unified view, with `source_provider` column to distinguish.

| Column | Type | Unit | OWM Path | NWS Path | Notes |
|--------|------|------|----------|----------|-------|
| `time` | TIMESTAMPTZ | - | timestamp | timestamp | Hypertable dimension |
| `ndp_id` | TEXT | - | ndp_id | ndp_id | Indexed |
| `source_provider` | TEXT | - | 'owm' | 'nws' | Data provenance |
| `temperature` | DOUBLE PRECISION | C | main.temp - 273.15 | properties.temperature.value | OWM in Kelvin |
| `feels_like` | DOUBLE PRECISION | C | main.feels_like - 273.15 | (calculated) | Apparent temperature |
| `humidity` | DOUBLE PRECISION | % | main.humidity | properties.relativeHumidity.value | |
| `pressure` | DOUBLE PRECISION | hPa | main.pressure | properties.barometricPressure.value / 100 | NWS in Pa |
| `wind_speed` | DOUBLE PRECISION | m/s | wind.speed | properties.windSpeed.value / 3.6 | NWS in km/h |
| `wind_direction` | DOUBLE PRECISION | degrees | wind.deg | properties.windDirection.value | 0-360 |
| `wind_gust` | DOUBLE PRECISION | m/s | wind.gust | properties.windGust.value / 3.6 | Nullable |
| `visibility` | INTEGER | m | visibility | properties.visibility.value | |
| `cloud_cover` | INTEGER | % | clouds.all | (from cloudLayers) | |
| `weather_description` | TEXT | - | weather[0].description | properties.textDescription | |

**Unit Normalization Required**:
- OWM temperature: Kelvin -> Celsius
- NWS pressure: Pa -> hPa
- NWS wind: km/h -> m/s

---

#### Entity 3: `silver.outdoor_air_quality`

**Purpose**: Outdoor pollution monitoring.

**Source Streams**: `outdoor-air-quality`

| Column | Type | Unit | Source Path | Notes |
|--------|------|------|-------------|-------|
| `time` | TIMESTAMPTZ | - | timestamp | Hypertable dimension |
| `ndp_id` | TEXT | - | ndp_id | Indexed |
| `aqi` | INTEGER | 1-5 | list[0].main.aqi | OWM European AQI scale |
| `pm25` | DOUBLE PRECISION | ug/m3 | list[0].components.pm2_5 | |
| `pm10` | DOUBLE PRECISION | ug/m3 | list[0].components.pm10 | |
| `co` | DOUBLE PRECISION | ug/m3 | list[0].components.co | Carbon monoxide |
| `no` | DOUBLE PRECISION | ug/m3 | list[0].components.no | Nitric oxide |
| `no2` | DOUBLE PRECISION | ug/m3 | list[0].components.no2 | Nitrogen dioxide |
| `o3` | DOUBLE PRECISION | ug/m3 | list[0].components.o3 | Ozone |
| `so2` | DOUBLE PRECISION | ug/m3 | list[0].components.so2 | Sulfur dioxide |
| `nh3` | DOUBLE PRECISION | ug/m3 | list[0].components.nh3 | Ammonia |

**Derived Fields** (Gold layer):
- US EPA AQI (calculated from PM2.5/O3)
- Health advisory level

---

#### Entity 4: `silver.weather_forecast`

**Purpose**: Forecast data for prediction accuracy tracking.

**Source Streams**: `nws-forecast-hourly`, `nws-gridpoints-forecast`

**Design Challenge**: NWS forecasts contain 156 hourly periods per fetch. Need to explode into individual rows.

| Column | Type | Unit | Source | Notes |
|--------|------|------|--------|-------|
| `time` | TIMESTAMPTZ | - | Ingestion time | When forecast was made |
| `valid_time` | TIMESTAMPTZ | - | period.startTime | When forecast applies |
| `ndp_id` | TEXT | - | ndp_id | Indexed |
| `forecast_hour` | INTEGER | hours | Calculated | Hours from issue time |
| `temperature` | DOUBLE PRECISION | F->C | period.temperature | Convert from F |
| `dewpoint` | DOUBLE PRECISION | C | dewpoint value | |
| `humidity` | DOUBLE PRECISION | % | relativeHumidity | |
| `wind_speed` | DOUBLE PRECISION | m/s | windSpeed | Parse "10 mph" |
| `wind_direction` | TEXT | - | windDirection | "NW", "SSE" |
| `precipitation_prob` | DOUBLE PRECISION | % | probabilityOfPrecipitation | |
| `short_forecast` | TEXT | - | period.shortForecast | "Partly Cloudy" |
| `is_daytime` | BOOLEAN | - | period.isDaytime | |

**Forecast Verification Join**:
```sql
-- Compare forecast to actuals
SELECT
    f.valid_time,
    f.temperature as forecast_temp,
    a.temperature as actual_temp,
    ABS(f.temperature - a.temperature) as error
FROM silver.weather_forecast f
JOIN silver.outdoor_weather a ON
    a.time BETWEEN f.valid_time - INTERVAL '30 min'
                AND f.valid_time + INTERVAL '30 min'
WHERE f.forecast_hour BETWEEN 1 AND 24;
```

---

### Entity 5: `data_dictionary` Schema

Already specified in ADR-001 (dp-002). Contains:
- `streams` - Stream metadata
- `fields` - Field definitions
- `sources` - Source configurations
- `entity_schemas` - HomeAssistant patterns
- `sync_status` - etcd sync tracking

---

## 3. Stream-to-Entity Mapping

### Mapping Strategy

| Bronze Stream | Silver Entity | Mapping Type | Notes |
|--------------|---------------|--------------|-------|
| `air-quality` | `indoor_air_quality` | 1:1 | Direct mapping |
| `outdoor-weather` | `outdoor_weather` | N:1 | Merged with NWS |
| `nws-station-observations` | `outdoor_weather` | N:1 | Same entity, different source |
| `nws-observations` | `outdoor_weather` | N:1 | Appears duplicate of above |
| `outdoor-air-quality` | `outdoor_air_quality` | 1:1 | Direct mapping |
| `nws-forecast-hourly` | `weather_forecast` | 1:N | Explode 156 periods |
| `nws-gridpoints-forecast` | `weather_forecast` | 1:N | Alternative source |

### Deduplication Consideration

`nws-observations` and `nws-station-observations` appear to be the same data (both from station KSGJ). **Recommendation**: Investigate whether both are needed; consider disabling one.

---

## 4. Normalization Strategy

### Recommendation: Denormalized Wide Tables

**Rationale**:

| Approach | Pros | Cons | Fit for NDP |
|----------|------|------|-------------|
| **Normalized (3NF)** | Less redundancy, easier updates | Requires JOINs, slower queries | Poor |
| **Denormalized (Wide)** | Fast queries, simple aggregations | Data redundancy | Good |
| **Star Schema** | Balance of both | Complexity | Overkill |

Time-series data is append-only (no updates), so normalization benefits don't apply. Wide tables with all metrics in one row optimize for:
- TimescaleDB continuous aggregates
- Grafana dashboard queries
- ML feature extraction

### Context Storage

Per ADR-003 (air-009), context stored as JSONB:

```sql
-- Hybrid approach: common fields as columns + JSONB for extras
CREATE TABLE silver.indoor_air_quality (
    time TIMESTAMPTZ NOT NULL,
    ndp_id TEXT NOT NULL,
    location_path TEXT,           -- Promoted from context
    context JSONB DEFAULT '{}',   -- Full context for flexibility
    -- metrics...
);
```

**Promotion Criteria**: Promote context fields to columns when:
1. Queried in >50% of dashboard panels
2. Used in GROUP BY or filters
3. Required for partition key

---

## 5. Field Extraction Priorities

### Priority 1: Core Analytics (MVP)

Fields needed for primary use cases: dashboards, alerts, basic ML.

| Entity | Core Fields |
|--------|-------------|
| `indoor_air_quality` | pm25, co2, temperature, humidity |
| `outdoor_weather` | temperature, humidity, wind_speed, pressure |
| `outdoor_air_quality` | aqi, pm25, o3 |
| `weather_forecast` | temperature, precipitation_prob, wind_speed |

### Priority 2: Enhanced Analytics

Fields for advanced dashboards and correlation analysis.

| Entity | Enhanced Fields |
|--------|-----------------|
| `indoor_air_quality` | tvoc_index, nox_index, pm10, pm01 |
| `outdoor_weather` | feels_like, visibility, cloud_cover, weather_description |
| `outdoor_air_quality` | co, no2, so2, nh3 |
| `weather_forecast` | dewpoint, humidity, short_forecast |

### Priority 3: Diagnostic/Metadata

Fields for debugging and device health monitoring.

| Entity | Diagnostic Fields |
|--------|-------------------|
| `indoor_air_quality` | wifi, boot_count, firmware, particle_counts |
| `outdoor_weather` | raw API response codes, station metadata |
| `weather_forecast` | forecast generation time, update intervals |

---

## 6. Pi Resource Constraints

### Memory Budget

Current allocation (from PLATFORM_ARCHITECTURE_OVERVIEW.md):

| Service | Memory | Status |
|---------|--------|--------|
| mosquitto | 128MB | Active |
| etcd | 256MB | Active |
| air-quality-app | 512MB | Active |
| duckdb | 512MB | Active (Virtual Silver) |
| grafana | 256MB | Active |
| **TimescaleDB** | **256-512MB** | **Proposed** |
| **Total** | **~2GB** | **12.5% of 16GB** |

### Storage Projections

| Entity | Records/Day | Row Size | Daily Growth | Monthly |
|--------|-------------|----------|--------------|---------|
| `indoor_air_quality` | 1,440 (1/min) | ~200B | 280KB | 8.4MB |
| `outdoor_weather` | 288 (OWM + NWS) | ~150B | 42KB | 1.3MB |
| `outdoor_air_quality` | 144 (10min) | ~120B | 17KB | 0.5MB |
| `weather_forecast` | 22,464 (156 periods * 144/day) | ~100B | 2.2MB | 66MB |
| **Total** | | | **~2.5MB/day** | **~76MB/month** |

With 90-day retention: ~230MB raw data + indexes

### Compression Strategy

```sql
-- Enable TimescaleDB compression after 7 days
SELECT add_compression_policy('silver.indoor_air_quality',
    INTERVAL '7 days');

-- Expected compression ratio: 10-20x for time-series
-- 230MB raw -> ~15-25MB compressed
```

---

## 7. Open Questions for Refinement Phase

### Architecture Questions

1. **Virtual vs Physical Silver**: Should we keep DuckDB virtual views alongside TimescaleDB, or migrate fully?
   - **Recommendation**: Dual-write period, then evaluate performance

2. **Forecast Explosion Strategy**: How to efficiently transform 156 periods per NWS fetch?
   - **Options**: Rust ETL batch, PostgreSQL function, scheduled job
   - **Recommendation**: Rust async batch with configurable parallelism

3. **Stream Deduplication**: Are `nws-observations` and `nws-station-observations` truly duplicates?
   - **Action**: Compare raw payloads, consider disabling one

### Implementation Questions

4. **ETL Trigger**: Real-time (on Bronze write) vs batch (scheduled)?
   - **Recommendation**: Near-real-time with 1-minute batch window

5. **Backfill Strategy**: How to populate Silver from existing Bronze data?
   - **Recommendation**: Rust CLI tool with parallel Parquet scanning

6. **Schema Migration**: How to handle Silver schema changes?
   - **Recommendation**: TimescaleDB-native migrations with version tracking

### Domain Questions

7. **Unit Standardization**: SI units vs source-native units?
   - **Recommendation**: SI in Silver (Celsius, m/s, Pa), with unit metadata

8. **Forecast Horizon**: How many hours of forecast to retain?
   - **Recommendation**: 168 hours (7 days), aligned with NWS updates

9. **Observation Lag Handling**: NWS observations can be 30+ minutes old. Track observation time separately from ingestion time?
   - **Recommendation**: Yes, add `observation_time` column from source

---

## 8. Next Steps

### Phase 1: Schema Creation
1. Create TimescaleDB container configuration
2. Write DDL for 4 Silver entities
3. Create hypertables with appropriate chunk intervals
4. Set up compression policies

### Phase 2: ETL Implementation
1. Implement Bronze->Silver transformation in Rust
2. Add field mapping configuration (Bronze path -> Silver column)
3. Build unit conversion utilities
4. Create validation rules

### Phase 3: Migration
1. Backfill from existing Bronze data
2. Enable real-time ETL
3. Update Grafana dashboards to use Silver
4. Deprecate DuckDB virtual views (optional)

---

## Appendix A: Bronze Raw Payload Examples

### air-quality (AirGradient MQTT)

```json
{
  "atmp": 21.69,
  "atmpCompensated": 21.69,
  "boot": 15924,
  "bootCount": 15924,
  "firmware": "3.4.1",
  "ledMode": "co2",
  "model": "I-9PSL",
  "noxIndex": 1,
  "noxRaw": 18535.5,
  "pm003Count": 20,
  "pm005Count": 15.33,
  "pm01": 0,
  "pm02": 0,
  "pm10": 0,
  "rco2": 589,
  "rhum": 55.41,
  "serialno": "d83bda1cd074",
  "tvocIndex": 165,
  "tvocRaw": 30277.17,
  "wifi": -53
}
```

### outdoor-weather (OpenWeatherMap)

```json
{
  "main": {
    "temp": 286.26,
    "feels_like": 285.77,
    "pressure": 1019,
    "humidity": 88
  },
  "wind": {
    "speed": 4.12,
    "deg": 360
  },
  "clouds": {"all": 100},
  "visibility": 10000,
  "weather": [{"description": "overcast clouds"}]
}
```

### nws-station-observations

```json
{
  "properties": {
    "temperature": {"value": 13, "unitCode": "wmoUnit:degC"},
    "dewpoint": {"value": 12, "unitCode": "wmoUnit:degC"},
    "windSpeed": {"value": 14.832, "unitCode": "wmoUnit:km_h-1"},
    "windDirection": {"value": 360},
    "barometricPressure": {"value": 101964.16, "unitCode": "wmoUnit:Pa"},
    "visibility": {"value": 16093.44, "unitCode": "wmoUnit:m"},
    "relativeHumidity": {"value": 93.65},
    "textDescription": "Cloudy"
  }
}
```

---

## Appendix B: Existing Architecture Patterns

### Relevant ADRs

| ADR | Feature | Key Decision |
|-----|---------|--------------|
| ADR-001 (dp-004) | Bronze Raw JSON | raw_payload stores untransformed source data |
| ADR-003 (air-009) | Silver Schema | JSONB for context, typed columns for metrics |
| ADR-001 (dp-002) | TimescaleDB Schema | Normalized data dictionary tables |
| ADR-002 (dp-002) | Entity Schema Format | Glob patterns for HomeAssistant entities |

### Pattern References

| Pattern | Description |
|---------|-------------|
| `arch-silver-schema` | Hypertable schema with JSONB context |
| `arch-bronze-schema` | Wide raw JSON storage |
| `arch-data-lake-layers` | Bronze -> Silver -> Gold architecture |
| `arch-domain-adapter-pattern` | Hexagonal architecture for sources/stores |

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-01-05 | NDP Architect | Initial scope definition |
