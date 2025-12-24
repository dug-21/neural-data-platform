# AIR-007: NWS Gridpoints Weather Data Expansion - Requirements Specification

**Feature ID**: AIR-007
**Version**: 1.0.0
**Status**: Draft
**Last Updated**: 2025-12-24
**Author**: sparc-coordinator

---

## Document Purpose

This document defines the detailed functional and non-functional requirements for expanding NWS weather data collection to include raw gridpoints forecast data and station observations. It serves as the definitive specification for implementation, testing, and acceptance.

---

## Requirements Overview

| Category | Count | Priority |
|----------|-------|----------|
| Functional Requirements | 10 | High |
| Non-Functional Requirements | 8 | High |
| Acceptance Criteria | 15 | High |

---

## Functional Requirements

### FR-1: NWS Raw Gridpoints Forecast Stream

**Priority**: High
**Component**: Stream Configuration

**Description**: Implement a new data stream that captures comprehensive forecast data from the NWS raw gridpoints API endpoint.

**Details**:
- **Stream ID**: `nws-gridpoints-forecast`
- **API Endpoint**: `GET https://api.weather.gov/gridpoints/JAX/79,49`
- **Location**: Jacksonville, FL grid (Office: JAX, Grid: 79,49)
- **Poll Interval**: 3600 seconds (1 hour)
- **Parser Type**: `ColumnOrientedParser` (new implementation)
- **Storage**: Bronze layer (Parquet)
- **Retention**: 90 days, compression after 7 days

**Required Fields** (40+ metrics):

#### Temperature Suite (8 fields)
| Field | Unit | Coverage | Validation Range |
|-------|------|----------|------------------|
| `temperature` | °C | 7+ days hourly | -40 to 50 |
| `dewpoint` | °C | 7+ days hourly | -40 to 40 |
| `max_temperature` | °C | Daily | -40 to 50 |
| `min_temperature` | °C | Daily | -40 to 50 |
| `apparent_temperature` | °C | Hourly | -50 to 60 |
| `wet_bulb_globe_temperature` | °C | Hourly | -40 to 50 |
| `heat_index` | °C | When applicable | -40 to 60 |
| `wind_chill` | °C | When applicable | -60 to 20 |

#### Wind Suite (7 fields)
| Field | Unit | Coverage | Validation Range |
|-------|------|----------|------------------|
| `wind_speed` | km/h | Hourly | 0 to 200 |
| `wind_direction` | degrees | Hourly | 0 to 360 |
| `wind_gust` | km/h | Hourly | 0 to 250 |
| `transport_wind_speed` | km/h | Hourly | 0 to 200 |
| `transport_wind_direction` | degrees | Hourly | 0 to 360 |
| `twenty_foot_wind_speed` | km/h | Fire weather | 0 to 200 |
| `twenty_foot_wind_direction` | degrees | Fire weather | 0 to 360 |

#### Precipitation Suite (4 fields)
| Field | Unit | Coverage | Validation Range |
|-------|------|----------|------------------|
| `probability_of_precipitation` | % | Hourly | 0 to 100 |
| `quantitative_precipitation` | mm | Hourly | 0 to 500 |
| `snowfall_amount` | mm | Hourly | 0 to 1000 |
| `ice_accumulation` | mm | Hourly | 0 to 100 |

#### Sky & Visibility Suite (4 fields)
| Field | Unit | Coverage | Validation Range |
|-------|------|----------|------------------|
| `sky_cover` | % | Hourly | 0 to 100 |
| `visibility` | m | Hourly | 0 to 50000 |
| `ceiling_height` | m | Hourly | 0 to 10000 |
| `weather` | JSON | Hourly | N/A (object array) |

#### Fire Weather & Indices (5 fields)
| Field | Unit | Coverage | Validation Range |
|-------|------|----------|------------------|
| `dispersion_index` | index | Hourly | 0 to 100 |
| `stability` | class | Hourly | N/A (categorical) |
| `low_visibility_occurrence_risk_index` | index | Hourly | 0 to 100 |
| `probability_of_thunder` | % | Hourly | 0 to 100 |
| `mixing_height` | m | Hourly | 0 to 5000 |

**API Headers**:
```yaml
User-Agent: "Neural-Data-Platform/1.0 (something@gmail.com)"
Accept: "application/geo+json"
```

**Rate Limiting**:
- NWS does not specify hard limits but recommends reasonable usage
- Implement 5-second retry delay on rate limit responses
- Use exponential backoff (max 3 retries)

---

### FR-2: NWS Station Observations Stream

**Priority**: High
**Component**: Stream Configuration

**Description**: Implement a new data stream that captures real-time weather station observations for ground-truth data.

**Details**:
- **Stream ID**: `nws-station-observations`
- **API Endpoint**: `GET https://api.weather.gov/stations/KSGJ/observations/latest`
- **Station**: KSGJ (NE Florida Regional Airport, 1.3 km from target location)
- **Poll Interval**: 900 seconds (15 minutes)
- **Parser Type**: `FlatJsonParser` or custom single-object parser
- **Storage**: Bronze layer (Parquet)
- **Retention**: 90 days, compression after 7 days

**Required Fields** (15+ metrics):

| Field | Unit | Nullable | Validation Range |
|-------|------|----------|------------------|
| `timestamp` | ISO 8601 | No | Valid timestamp |
| `temperature` | °C | No | -40 to 50 |
| `dewpoint` | °C | No | -40 to 40 |
| `relative_humidity` | % | No | 0 to 100 |
| `wind_direction` | degrees | Yes | 0 to 360 |
| `wind_speed` | km/h | No | 0 to 200 |
| `wind_gust` | km/h | Yes | 0 to 250 |
| `barometric_pressure` | Pa | No | 80000 to 110000 |
| `sea_level_pressure` | Pa | Yes | 80000 to 110000 |
| `visibility` | m | No | 0 to 50000 |
| `cloud_layers` | JSON | No | N/A (array) |
| `text_description` | string | No | N/A |
| `heat_index` | °C | Yes | -40 to 60 |
| `wind_chill` | °C | Yes | -60 to 20 |
| `precipitation_last_hour` | mm | Yes | 0 to 500 |
| `precipitation_last_3_hours` | mm | Yes | 0 to 500 |
| `precipitation_last_6_hours` | mm | Yes | 0 to 500 |

**Data Quality Note**: Observations have ~20 minute delay due to MADIS (Meteorological Assimilation Data Ingest System) quality control processing.

---

### FR-3: ColumnOrientedParser Implementation

**Priority**: High
**Component**: Parser (neural-core)

**Description**: Implement a generic parser that handles column-oriented JSON data structures where each metric has its own time-series array.

**Details**:
- **Trait**: Implements `ResponseParser` trait
- **Location**: `core/src/sources/parsers/column_oriented.rs`
- **Reusability**: Must support both NWS gridpoints and future Open-Meteo integration

**Parser Behavior**:

1. **Input Structure**:
```json
{
  "properties": {
    "temperature": {
      "uom": "wmoUnit:degC",
      "values": [
        {"validTime": "2025-12-24T02:00:00+00:00/PT3H", "value": 16.1},
        {"validTime": "2025-12-24T05:00:00+00:00/PT1H", "value": 15.6}
      ]
    },
    "skyCover": {
      "uom": "wmoUnit:percent",
      "values": [
        {"validTime": "2025-12-24T02:00:00+00:00/PT1H", "value": 5}
      ]
    }
  }
}
```

2. **Output**: `Vec<TimeSeriesPoint>` with flattened rows

3. **ISO 8601 Duration Parsing**:
   - Parse `PT1H` (1 hour), `PT3H` (3 hours), `PT6H` (6 hours)
   - Convert to Unix timestamp ranges
   - Handle overlapping time periods (use start timestamp)

4. **Configuration**:
```rust
pub struct ColumnOrientedConfig {
    pub root_path: String,              // "properties"
    pub field_mappings: HashMap<String, FieldMapping>,
    pub time_field_name: String,        // "validTime"
    pub value_field_name: String,       // "value"
    pub unit_field_name: Option<String>, // "uom"
}

pub struct FieldMapping {
    pub source_name: String,      // "temperature"
    pub target_name: String,      // "temperature"
    pub unit_conversion: Option<UnitConversion>,
}
```

5. **Unit Conversion**:
   - Support `wmoUnit:degC` → Celsius (passthrough)
   - Support `wmoUnit:km_h-1` → km/h (passthrough)
   - Support `wmoUnit:Pa` → Pascals (passthrough)
   - Support `wmoUnit:percent` → % (passthrough)
   - Future: Add conversion functions as needed

6. **Error Handling**:
   - Invalid JSON → return error with context
   - Missing required fields → skip metric, log warning
   - Invalid timestamp format → skip value, log warning
   - Null values → include as NULL in output

---

### FR-4: Stream Configuration Files

**Priority**: High
**Component**: Configuration

**Description**: Create YAML stream configuration files for both new streams following existing patterns.

**Details**:
- **Location**: `config/base/streams/nws-gridpoints-forecast/config.yaml`
- **Location**: `config/base/streams/nws-station-observations/config.yaml`
- **GitOps Sync**: ConfigSyncService will sync to etcd on application startup

**nws-gridpoints-forecast/config.yaml**:
```yaml
stream_id: nws-gridpoints-forecast
description: "NWS raw gridpoints forecast data with 40+ meteorological fields"
version: "1.0.0"
enabled: true
retention_days: 90
compression_after_days: 7
partitioning_strategy: daily

fields:
  # (Full schema with all 40+ fields)

sources:
  - source_type: HttpPoll
    enabled: true
    params:
      url: "https://api.weather.gov/gridpoints/JAX/79,49"
      method: GET
      headers:
        User-Agent: "Neural-Data-Platform/1.0 (contact@yourdomain.com)"
        Accept: "application/geo+json"
      poll_interval_seconds: 3600
      timeout_seconds: 30
      parser_name: column_oriented
      parser_config:
        root_path: "properties"
        time_field_name: "validTime"
        value_field_name: "value"
        field_mappings:
          # (Mapping configuration)
```

**nws-station-observations/config.yaml**:
```yaml
stream_id: nws-station-observations
description: "NWS weather station observations for ground-truth data"
version: "1.0.0"
enabled: true
retention_days: 90
compression_after_days: 7
partitioning_strategy: daily

fields:
  # (Schema with 15+ observation fields)

sources:
  - source_type: HttpPoll
    enabled: true
    params:
      url: "https://api.weather.gov/stations/KSGJ/observations/latest"
      method: GET
      headers:
        User-Agent: "Neural-Data-Platform/1.0 (contact@yourdomain.com)"
        Accept: "application/geo+json"
      poll_interval_seconds: 900
      timeout_seconds: 30
      parser_name: nws_observation
      parser_config:
        root_path: "properties"
```

---

### FR-5: Parser Registry Integration

**Priority**: High
**Component**: Parser Registry

**Description**: Register new parsers in the `ParserRegistry` for runtime lookup.

**Details**:
- **Location**: `core/src/sources/parsers/mod.rs`
- **Registration**: Add `column_oriented` and `nws_observation` parsers

**Implementation**:
```rust
impl ParserRegistry {
    pub fn new() -> Self {
        let mut parsers: HashMap<String, Box<dyn ResponseParser>> = HashMap::new();

        // Existing parsers
        parsers.insert("weather".to_string(), Box::new(WeatherParser));
        parsers.insert("air_pollution".to_string(), Box::new(AirPollutionParser));

        // New parsers
        parsers.insert("column_oriented".to_string(), Box::new(ColumnOrientedParser::default()));
        parsers.insert("nws_observation".to_string(), Box::new(NwsObservationParser));

        Self { parsers }
    }
}
```

---

### FR-7: Grafana Dashboard - NWS Gridpoint Forecast

**Priority**: Medium
**Component**: Grafana Dashboards

**Description**: Create a Grafana dashboard visualizing raw gridpoint forecast data.

**Details**:
- **Location**: `config/grafana/dashboards/nws-gridpoint-forecast.json`
- **Provisioning**: `config/grafana/provisioning/dashboards/default.yaml`

**Required Panels**:
1. **Sky Cover Timeline** - Line chart showing cloud coverage percentage
2. **Visibility Timeline** - Line chart showing visibility in meters
3. **Wind Gust Forecast** - Line chart showing predicted wind gusts
4. **Temperature Suite** - Multi-line chart (temp, dewpoint, apparent temp, heat index, wind chill)
5. **Precipitation Probability** - Area chart showing rain likelihood
6. **Fire Weather Indices** - Multi-stat panel (dispersion index, mixing height, probability of thunder)

**Time Range**: Default to 7 days forward (forecast horizon)

---

### FR-8: Grafana Dashboard - NWS Current Observations

**Priority**: Medium
**Component**: Grafana Dashboards

**Description**: Create a Grafana dashboard visualizing real-time weather station observations.

**Details**:
- **Location**: `config/grafana/dashboards/nws-current-observations.json`

**Required Panels**:
1. **Current Conditions** - Stat panel (temperature, humidity, wind speed)
2. **Temperature History** - Line chart (last 24 hours)
3. **Wind Conditions** - Wind rose or direction gauge
4. **Pressure Trend** - Line chart showing barometric pressure
5. **Visibility & Cloud Cover** - Combined chart
6. **Precipitation** - Bar chart (last hour, 3 hours, 6 hours)

**Time Range**: Default to 24 hours (observations are historical)

---

### FR-9: Grafana Dashboard - Forecast vs Observations Comparison

**Priority**: Low
**Component**: Grafana Dashboards

**Description**: Create a dashboard comparing NWS forecasts to actual observations for accuracy analysis.

**Details**:
- **Location**: `config/grafana/dashboards/nws-forecast-vs-observations.json`

**Required Panels**:
1. **Temperature Accuracy** - Overlay forecast vs observed temperature
2. **Wind Speed Accuracy** - Overlay forecast vs observed wind speed
3. **Precipitation Accuracy** - Compare forecast probability to actual precipitation
4. **Visibility Accuracy** - Overlay forecast vs observed visibility
5. **Forecast Error Metrics** - Stat panel (MAE, RMSE for temperature)

**Time Range**: Default to 7 days (requires forecast data + subsequent observations)

**Note**: This is a future enhancement and may require time-shift queries.

---

### FR-10: IngestionCoordinator Integration

**Priority**: High
**Component**: Application Startup

**Description**: Integrate new streams into the existing `IngestionCoordinator` for automatic lifecycle management.

**Details**:
- **No code changes required** - streams are auto-discovered via StreamRegistry
- **Verification**: Confirm streams appear in `/streams/` etcd prefix
- **Monitoring**: Ensure health checks and metrics are collected

**Expected Behavior**:
1. Application startup loads stream configs from etcd
2. `IngestionCoordinator` spawns `HttpPollingSource` for each stream
3. Data flows through existing pipeline (channel → router → storage)
4. Parquet files appear in `/data/nws-gridpoints-forecast/` and `/data/nws-station-observations/`

---

## Non-Functional Requirements

### NFR-1: Data Freshness

**Priority**: High
**Category**: Performance

**Description**: Data must be available in Parquet files within a defined time window after API polling.

**Requirement**:
- **Gridpoint Forecast**: Data written to Parquet within 5 minutes of poll completion
- **Station Observations**: Data written to Parquet within 5 minutes of poll completion
- **Grafana Visibility**: Dashboards query Parquet files directly (no ETL required)

**Measurement**:
- Log timestamp when API response received
- Log timestamp when Parquet file written
- Verify Grafana dashboard displays new data
- Alert if latency exceeds 10 minutes

---

### NFR-2: Parser Robustness

**Priority**: High
**Category**: Reliability

**Description**: The ColumnOrientedParser must handle variable time intervals and malformed data gracefully.

**Requirements**:
- **Variable Durations**: Support PT1H, PT3H, PT6H, PT12H, P1D formats
- **Missing Fields**: Continue processing other metrics if one metric is missing
- **Invalid Timestamps**: Skip invalid values, log warning, continue processing
- **Null Values**: Preserve NULLs in output (do not crash)
- **Overlapping Periods**: Use start timestamp when periods overlap

**Failure Modes**:
- Invalid JSON → return error, retry with backoff
- Empty response → return empty Vec, log warning
- Partial data → process available metrics, log missing fields

---

### NFR-3: Observation Data Delay Tolerance

**Priority**: Medium
**Category**: Data Quality

**Description**: System must account for the 20-minute MADIS processing delay in station observations.

**Requirements**:
- **Poll Interval**: 15 minutes (faster than delay, ensures no gaps)
- **Duplicate Detection**: Handle cases where same observation appears in consecutive polls
- **Timestamp Ordering**: Do not assume observations arrive in chronological order
- **Stale Data Handling**: Reject observations older than 2 hours

**Implementation**:
- Use observation timestamp (not poll timestamp) as primary timestamp
- Parquet store handles duplicate timestamps naturally (append-only)
- Grafana queries can deduplicate via `DISTINCT ON (timestamp)`

---

### NFR-4: API Rate Limiting Compliance

**Priority**: High
**Category**: External Dependency

**Description**: Respect NWS API rate limits and best practices to avoid blocking.

**Requirements**:
- **User-Agent Header**: Always include identifying User-Agent
- **Rate Limit Response**: Implement 5-second retry delay on HTTP 429
- **Exponential Backoff**: Max 3 retries with exponential backoff (5s, 10s, 20s)
- **Connection Pooling**: Reuse HTTP connections where possible
- **Request Timeout**: 30-second timeout per request

**Monitoring**:
- Log all HTTP 429 responses
- Track retry counts and success rates
- Alert if consecutive failures exceed 5

---

### NFR-5: Backward Compatibility

**Priority**: High
**Category**: System Stability

**Description**: New NWS streams must not break existing streams or cause regressions.

**Requirements**:
- **Existing Streams**: `air-quality`, `outdoor-weather`, `outdoor-air-quality` continue to function
- **Shared Components**: `HttpPollingSource`, `ParserRegistry` remain compatible
- **Storage**: No changes to existing Parquet schemas or directory structure
- **Configuration**: No changes to existing stream configs

**Verification**:
- Integration tests for all existing streams
- End-to-end test verifying all streams ingest simultaneously
- Grafana dashboards for existing streams remain functional

---

### NFR-6: Parser Extensibility

**Priority**: Medium
**Category**: Maintainability

**Description**: ColumnOrientedParser must be generic enough to support future Open-Meteo integration without major refactoring.

**Requirements**:
- **Configurable Field Mappings**: Support different source field names
- **Configurable JSON Paths**: Support different root paths (e.g., `properties` vs `hourly`)
- **Unit Conversion Framework**: Pluggable unit conversion functions
- **Time Format Variants**: Support both ISO 8601 durations and Unix timestamps

**Future Use Cases**:
- Open-Meteo hourly forecast API (similar column-oriented structure)
- Open-Meteo air quality API (similar column-oriented structure)
- Other weather APIs with columnar data

---

### NFR-7: Resource Constraints

**Priority**: High
**Category**: Performance

**Description**: New streams must operate within Raspberry Pi 5 memory and CPU constraints.

**Requirements**:
- **air-quality-app Memory**: No increase beyond existing 512MB limit
- **CPU Usage**: Parsing and ingestion should not exceed 10% CPU average
- **Disk I/O**: Parquet writes should batch to minimize I/O operations

**Measurement**:
- Monitor container memory usage via `docker stats`
- Track CPU usage during ingestion spikes
- Measure Parquet file sizes and write frequency

---

### NFR-8: Monitoring and Observability

**Priority**: Medium
**Category**: Operations

**Description**: New streams must provide observability for troubleshooting and performance analysis.

**Requirements**:
- **Health Checks**: Each stream reports health status to coordinator
- **Metrics**: Track poll success rate, parse errors, ingestion latency
- **Logging**: Log API errors, parser warnings, storage failures
- **Alerts**: Notify on consecutive failures (threshold: 5)

**Metrics to Track**:
- `nws_gridpoints_poll_success_rate` (gauge)
- `nws_gridpoints_parse_errors_total` (counter)
- `nws_observations_poll_latency_seconds` (histogram)
- `nws_observations_points_ingested_total` (counter)

---

## Acceptance Criteria

### AC-1: Gridpoint Forecast Ingestion

**Given**: The `nws-gridpoints-forecast` stream is enabled
**When**: The application polls the NWS API every hour
**Then**:
- [ ] API returns HTTP 200 with valid JSON response
- [ ] ColumnOrientedParser successfully parses all 40+ fields
- [ ] TimeSeriesPoint objects are created for each time period
- [ ] Data is written to `/data/nws-gridpoints-forecast/*.parquet`
- [ ] Grafana dashboard queries Parquet files successfully
- [ ] No errors logged during nominal operation

---

### AC-2: Station Observation Ingestion

**Given**: The `nws-station-observations` stream is enabled
**When**: The application polls the NWS station API every 15 minutes
**Then**:
- [ ] API returns HTTP 200 with latest observation
- [ ] NwsObservationParser successfully parses all 15+ fields
- [ ] TimeSeriesPoint object is created with observation timestamp
- [ ] Data is written to `/data/nws-station-observations/*.parquet`
- [ ] Grafana dashboard queries Parquet files successfully
- [ ] No errors logged during nominal operation

---

### AC-3: ColumnOrientedParser - Variable Time Periods

**Given**: A gridpoint response with mixed time intervals (PT1H, PT3H, PT6H)
**When**: The parser processes the response
**Then**:
- [ ] PT1H periods are parsed correctly (1-hour duration)
- [ ] PT3H periods are parsed correctly (3-hour duration)
- [ ] PT6H periods are parsed correctly (6-hour duration)
- [ ] All periods generate TimeSeriesPoint objects
- [ ] Overlapping periods use start timestamp
- [ ] No parsing errors occur

---

### AC-4: ColumnOrientedParser - Missing Field Handling

**Given**: A gridpoint response missing optional field `heat_index`
**When**: The parser processes the response
**Then**:
- [ ] Parser does not crash or return error
- [ ] Other fields (temperature, wind_speed, etc.) are parsed successfully
- [ ] Warning is logged for missing field
- [ ] Generated TimeSeriesPoints omit `heat_index` or include NULL

---

### AC-5: ColumnOrientedParser - Invalid Timestamp Handling

**Given**: A gridpoint response with malformed timestamp "INVALID-DATE"
**When**: The parser processes the response
**Then**:
- [ ] Invalid timestamp is skipped
- [ ] Warning is logged with specific value
- [ ] Other valid timestamps are processed successfully
- [ ] No panic or crash occurs

---

### AC-6: API Rate Limiting - HTTP 429 Retry

**Given**: NWS API returns HTTP 429 (rate limited)
**When**: The HttpPollingSource receives the response
**Then**:
- [ ] Source waits 5 seconds before retry
- [ ] Retry attempt is made (up to 3 times)
- [ ] Exponential backoff is applied (5s, 10s, 20s)
- [ ] Failure is logged if all retries exhausted
- [ ] Next scheduled poll continues normally

---

### AC-7: Data Quality - Range Validation

**Given**: A gridpoint forecast with temperature value of 999°C (invalid)
**When**: Grafana queries the Parquet files
**Then**:
- [ ] Invalid value is transformed to NULL in view
- [ ] Query returns NULL for that timestamp
- [ ] Valid temperature values in range [-40, 50] are preserved
- [ ] No database errors occur

---

### AC-8: Grafana Dashboard - Gridpoint Forecast

**Given**: The NWS Gridpoint Forecast dashboard is provisioned
**When**: A user opens the dashboard in Grafana
**Then**:
- [ ] Dashboard loads without errors
- [ ] Sky Cover panel displays data from `silver_nws_forecast`
- [ ] Visibility panel displays data from `silver_nws_forecast`
- [ ] Wind Gust panel displays data from `silver_nws_forecast`
- [ ] Temperature Suite panel displays multiple metrics
- [ ] Time range selector allows 7-day forward view
- [ ] Dashboard auto-refreshes every 5 minutes

---

### AC-9: Grafana Dashboard - Current Observations

**Given**: The NWS Current Observations dashboard is provisioned
**When**: A user opens the dashboard in Grafana
**Then**:
- [ ] Dashboard loads without errors
- [ ] Current Conditions panel shows latest values
- [ ] Temperature History panel displays 24-hour chart
- [ ] Wind Conditions panel displays direction and speed
- [ ] Pressure Trend panel displays barometric pressure
- [ ] Dashboard auto-refreshes every 5 minutes

---

### AC-10: Stream Configuration GitOps Sync

**Given**: Stream configs exist in `config/base/streams/nws-*/config.yaml`
**When**: Application starts with ConfigSyncService enabled
**Then**:
- [ ] ConfigSyncService discovers both YAML files
- [ ] YAML is parsed to `StreamConfig` structs
- [ ] Configs are saved to etcd `/streams/nws-gridpoints-forecast/config`
- [ ] Configs are saved to etcd `/streams/nws-station-observations/config`
- [ ] IngestionCoordinator loads streams from etcd
- [ ] Streams begin ingesting data

---

### AC-11: Backward Compatibility - Existing Streams

**Given**: AIR-007 is deployed with new NWS streams
**When**: The system is running
**Then**:
- [ ] `air-quality` stream continues ingesting MQTT data
- [ ] `outdoor-weather` stream continues polling OpenWeatherMap
- [ ] `outdoor-air-quality` stream continues polling OpenWeatherMap
- [ ] Existing Grafana dashboards remain functional
- [ ] No errors in logs related to existing streams

---

### AC-12: Resource Constraints - Memory Usage

**Given**: All streams are running (air-quality + outdoor-* + nws-*)
**When**: System is monitored for 24 hours
**Then**:
- [ ] `air-quality-app` container stays below 512MB
- [ ] Total platform memory usage stays below 2GB
- [ ] No OOM (Out of Memory) kills occur
- [ ] No memory leak detected (stable over 24h)

---

### AC-13: Parser Extensibility - Open-Meteo Compatibility

**Given**: ColumnOrientedParser is implemented
**When**: Configuration is provided for Open-Meteo format
**Then**:
- [ ] Parser can handle Open-Meteo's `hourly` root path (vs NWS `properties`)
- [ ] Parser can map different field names (`temperature_2m` vs `temperature`)
- [ ] Parser can handle Unix timestamp arrays (vs ISO 8601 durations)
- [ ] No code changes required, only configuration changes

*(Note: Actual Open-Meteo integration is out of scope for AIR-007)*

---

### AC-14: Monitoring - Health Check Endpoints

**Given**: NWS streams are configured and running
**When**: Health check is queried
**Then**:
- [ ] `/health` endpoint reports both streams as healthy
- [ ] `nws-gridpoints-forecast` shows last successful poll timestamp
- [ ] `nws-station-observations` shows last successful poll timestamp
- [ ] Consecutive failure count is tracked
- [ ] Status is "degraded" if last poll failed

---

### AC-15: End-to-End Integration Test

**Given**: Complete AIR-007 implementation is deployed
**When**: System runs for 2 hours
**Then**:
- [ ] At least 2 gridpoint forecast polls succeed (1-hour interval)
- [ ] At least 8 station observation polls succeed (15-minute interval)
- [ ] Parquet files are created in both stream directories
- [ ] Grafana dashboards query Parquet files for both streams
- [ ] Grafana dashboards display charts with data
- [ ] No critical errors logged
- [ ] All health checks pass

---

## Dependencies

### Internal Dependencies

| Component | Version | Purpose |
|-----------|---------|---------|
| `neural-core` | 1.4.0 | Core traits and types |
| `config-client` | 1.4.0 | etcd configuration client |
| `air-quality-app` | 1.4.0 | Application runtime |
| `HttpPollingSource` | 1.4.0 | HTTP data source |
| `ParserRegistry` | 1.4.0 | Parser management |
| `ParquetStore` | 1.4.0 | Bronze layer storage |

### External Dependencies

| Service | Version | Purpose |
|---------|---------|---------|
| NWS API | N/A | Weather data source |
| Grafana DuckDB Plugin | 1.1.3+ | Parquet file queries |
| Grafana | 11.4.0+ | Visualization |

### Reference Documents

| Document | Location | Purpose |
|----------|----------|---------|
| NWS Complete Analysis | `product/research/weatherresources/NWS-COMPLETE-ANALYSIS.md` | API research |
| Platform Architecture | `docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md` | System design |
| Domain Adapter Pattern | `docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md` | Trait definitions |
| How to Add New Stream | `docs/procedures/HOW_TO_ADD_NEW_STREAM.md` | Implementation guide |
| AIR-007 Scope | `product/features/air-007/SCOPE.md` | Feature scope |

---

## Out of Scope

The following items are explicitly **not** included in AIR-007:

1. **Open-Meteo Integration** - Parser should support it, but actual integration is future work
2. **Historical Backfill** - No importing of past NWS data from archive endpoints
3. **Multi-Location Support** - Hard-coded to Jacksonville area (JAX/79,49 and KSGJ)
4. **Air Quality Data from NWS** - NWS does not provide AQI data
5. **Marine Data** - Wave height/period available in gridpoints but not prioritized
6. **Custom Alerts** - Threshold-based alerting on NWS data (future feature)
7. **Machine Learning Features** - Feature engineering on NWS data (future DP-002+)
8. **TimescaleDB Migration** - Bronze layer stays in Parquet
9. **Forecast Accuracy Metrics** - Comparison dashboard is low priority, may be deferred
10. **Additional Weather Stations** - KSGJ only; no aggregation from multiple stations

---

## Risks and Mitigations

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| NWS API changes format | Low | High | Version API responses, implement schema validation |
| Parser complexity exceeds estimate | Medium | Medium | Start with minimal viable parser, iterate |
| Memory usage exceeds Pi limits | Low | High | Profile early, optimize batch sizes |
| ISO 8601 duration parsing edge cases | Medium | Low | Extensive unit tests, handle errors gracefully |
| MADIS delay causes gaps | Low | Low | Poll more frequently than delay (15 min vs 20 min) |
| Gridpoint update frequency varies | Medium | Low | Document variability, poll conservatively (1 hour) |

---

## Success Metrics

| Metric | Target | Measurement Method |
|--------|--------|-------------------|
| Gridpoint poll success rate | >95% | Monitor HTTP 200 responses |
| Observation poll success rate | >95% | Monitor HTTP 200 responses |
| Parser success rate | >99% | Track parse errors vs total responses |
| Data freshness | <5 min | Timestamp delta (API → Parquet) |
| Dashboard load time | <2 sec | Grafana query performance |
| Memory usage (air-quality-app) | <512MB | Docker stats monitoring |
| Zero regressions | 100% | Existing stream integration tests pass |

---

## Appendix A: Field Type Mappings

### NWS Units to Platform Types

| NWS Unit | Platform Type | Unit String | Range |
|----------|---------------|-------------|-------|
| `wmoUnit:degC` | Float | °C | -60 to 60 |
| `wmoUnit:km_h-1` | Float | km/h | 0 to 250 |
| `wmoUnit:degree_(angle)` | Float | degrees | 0 to 360 |
| `wmoUnit:Pa` | Float | Pa | 0 to 120000 |
| `wmoUnit:percent` | Float | % | 0 to 100 |
| `wmoUnit:m` | Float | m | 0 to 50000 |
| `wmoUnit:mm` | Float | mm | 0 to 1000 |

### Platform Field Naming Conventions

| NWS API Field | Platform Field | Rationale |
|---------------|----------------|-----------|
| `temperature` | `temperature` | Direct mapping |
| `dewpoint` | `dewpoint` | Direct mapping |
| `skyCover` | `sky_cover` | snake_case convention |
| `windSpeed` | `wind_speed` | snake_case convention |
| `windDirection` | `wind_direction` | snake_case convention |
| `relativeHumidity` | `relative_humidity` | snake_case convention |
| `maxTemperature` | `max_temperature` | snake_case convention |

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2025-12-24 | sparc-coordinator | Initial comprehensive requirements specification |

---

## References

1. National Weather Service API Documentation: https://www.weather.gov/documentation/services-web-api
2. ISO 8601 Duration Format: https://en.wikipedia.org/wiki/ISO_8601#Durations
3. MADIS Quality Control: https://madis.ncep.noaa.gov/
4. Neural Data Platform Architecture: `/docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md`
5. Domain Adapter Pattern: `/docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md#domain-adapter-pattern`
6. AIR-007 Scope: `/product/features/air-007/SCOPE.md`
7. NWS Complete Analysis: `/product/research/weatherresources/NWS-COMPLETE-ANALYSIS.md`
