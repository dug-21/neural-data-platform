# AIR-005 Specification: Outdoor Weather Data Integration

## Document Information

- **Feature ID**: AIR-005
- **Version**: 1.0.0
- **Status**: Draft
- **Created**: 2025-12-16
- **Last Updated**: 2025-12-16
- **Author**: SPARC Specification Agent

## 1. Feature Overview

### 1.1 Purpose

This specification defines the requirements for integrating OpenWeatherMap outdoor weather and air quality data into the Neural Data Platform. The system shall collect, normalize, and store weather metrics alongside existing neural network model performance data to enable environmental correlation analysis.

### 1.2 Scope

**In Scope:**
- Integration with OpenWeatherMap Current Weather API
- Integration with OpenWeatherMap Air Pollution API
- Automated polling every 10 minutes
- Configuration storage in etcd
- Timestamp normalization to system timezone
- Data storage in Parquet format via existing StorageWriter
- Health monitoring and error handling
- Extension of existing Domain Adapter pattern

**Out of Scope:**
- Historical weather data backfill
- Weather forecasting capabilities
- Multiple geographic location support (Phase 1)
- Real-time weather alerts
- Weather data visualization (separate feature)
- Modification of existing neural network data collection

### 1.3 Success Criteria

- Weather data successfully collected every 10 minutes with <5% missed polls
- All timestamps normalized to match existing platform timezone (UTC or configured)
- Configuration fully managed through etcd
- Zero impact on existing neural network data collection
- Health check endpoint reports weather data freshness
- Data queryable alongside existing neural metrics

## 2. Functional Requirements

### 2.1 Data Collection

#### FR-001: OpenWeatherMap Current Weather Integration
**Priority**: High
**Description**: System shall integrate with OpenWeatherMap Current Weather API v2.5

**Acceptance Criteria:**
- [ ] API endpoint: `https://api.openweathermap.org/data/2.5/weather`
- [ ] Request includes lat, lon, appid, units=metric parameters
- [ ] Response parsed for all documented fields
- [ ] HTTP timeout set to 30 seconds
- [ ] Retry logic: 3 attempts with exponential backoff (1s, 2s, 4s)
- [ ] API key stored securely in etcd at `/config/weather/api_key`

#### FR-002: OpenWeatherMap Air Pollution Integration
**Priority**: High
**Description**: System shall integrate with OpenWeatherMap Air Pollution API v2.5

**Acceptance Criteria:**
- [ ] API endpoint: `http://api.openweathermap.org/data/2.5/air_pollution`
- [ ] Request includes lat, lon, appid parameters
- [ ] Response parsed for AQI and all pollutant components
- [ ] Same timeout and retry logic as FR-001
- [ ] Uses same API key as Current Weather API

#### FR-003: Polling Schedule
**Priority**: High
**Description**: System shall poll both APIs every 10 minutes

**Acceptance Criteria:**
- [ ] Default poll interval: 600 seconds (10 minutes)
- [ ] Poll interval configurable via etcd at `/config/weather/poll_interval`
- [ ] Minimum allowed interval: 300 seconds (5 minutes)
- [ ] Maximum allowed interval: 3600 seconds (60 minutes)
- [ ] Both APIs polled atomically (same polling cycle)
- [ ] Polling schedule maintained across system restarts

#### FR-004: Geographic Configuration
**Priority**: High
**Description**: System shall support configurable geographic coordinates

**Acceptance Criteria:**
- [ ] Latitude stored in etcd at `/config/weather/latitude`
- [ ] Longitude stored in etcd at `/config/weather/longitude`
- [ ] Latitude validation: -90.0 to +90.0
- [ ] Longitude validation: -180.0 to +180.0
- [ ] Coordinates hot-reloadable without system restart
- [ ] Invalid coordinates trigger health check failure

### 2.2 Data Processing

#### FR-005: Timestamp Normalization
**Priority**: High
**Description**: System shall normalize all timestamps to platform timezone

**Acceptance Criteria:**
- [ ] OpenWeatherMap `dt` field (Unix timestamp) converted to platform timezone
- [ ] Platform timezone retrieved from etcd at `/config/system/timezone`
- [ ] Default timezone: UTC if not configured
- [ ] Sunrise/sunset times normalized to same timezone
- [ ] Timezone offset stored in metadata
- [ ] Timestamp precision: seconds (no sub-second)

#### FR-006: Data Normalization
**Priority**: Medium
**Description**: System shall normalize weather data to consistent units and schema

**Acceptance Criteria:**
- [ ] Temperature in Celsius (already metric from API)
- [ ] Pressure in hPa
- [ ] Wind speed in m/s
- [ ] Visibility in meters
- [ ] Precipitation in mm
- [ ] All pollutants in μg/m³
- [ ] Missing fields represented as NULL (not 0)
- [ ] Field names snake_case for consistency

#### FR-007: Stream Registration
**Priority**: High
**Description**: System shall register weather streams in existing Stream Registry

**Acceptance Criteria:**
- [ ] Stream ID: `weather_current` for current weather data
- [ ] Stream ID: `weather_air_quality` for air pollution data
- [ ] Stream config stored at `/streams/weather_current/config`
- [ ] Stream config stored at `/streams/weather_air_quality/config`
- [ ] Stream type: `weather_source`
- [ ] Schema versioning supported (v1.0.0 initial)

### 2.3 Data Storage

#### FR-008: Parquet Storage Integration
**Priority**: High
**Description**: System shall store weather data using existing ParquetStore

**Acceptance Criteria:**
- [ ] Data sent to StorageWriter via mpsc channel
- [ ] Parquet file path: `/data/weather/current/{date}.parquet`
- [ ] Parquet file path: `/data/weather/air_quality/{date}.parquet`
- [ ] Daily file rotation at midnight platform time
- [ ] Schema evolution supported via Parquet metadata
- [ ] Compression: SNAPPY (same as existing streams)

#### FR-009: Data Retention
**Priority**: Low
**Description**: System shall support configurable data retention

**Acceptance Criteria:**
- [ ] Default retention: 90 days
- [ ] Retention configurable via etcd at `/config/weather/retention_days`
- [ ] Cleanup job runs daily at 2 AM platform time
- [ ] Cleanup deletes files older than retention period
- [ ] Cleanup logs deletions for audit trail

### 2.4 Monitoring and Health

#### FR-010: Health Check Endpoint
**Priority**: High
**Description**: System shall provide health status for weather data collection

**Acceptance Criteria:**
- [ ] Health check method on WeatherSource trait
- [ ] Returns OK if last successful poll within 2x poll_interval
- [ ] Returns DEGRADED if API errors but data <24 hours old
- [ ] Returns FAILED if no data for >24 hours
- [ ] Includes last_poll_time, last_success_time, error_count in response
- [ ] Integrates with existing platform health monitoring

#### FR-011: Error Handling
**Priority**: High
**Description**: System shall handle API failures gracefully

**Acceptance Criteria:**
- [ ] Network errors logged with ERROR level
- [ ] API errors (4xx, 5xx) logged with context
- [ ] Rate limit errors (429) trigger backoff (60s)
- [ ] Invalid API key logs CRITICAL and stops polling
- [ ] Transient errors don't affect health check for 1 hour
- [ ] Circuit breaker after 10 consecutive failures

#### FR-012: Metrics Collection
**Priority**: Medium
**Description**: System shall collect operational metrics

**Acceptance Criteria:**
- [ ] Metric: `weather_api_calls_total` (counter by endpoint, status)
- [ ] Metric: `weather_api_duration_seconds` (histogram by endpoint)
- [ ] Metric: `weather_poll_errors_total` (counter by error_type)
- [ ] Metric: `weather_data_points_stored_total` (counter by stream)
- [ ] Metric: `weather_last_success_timestamp` (gauge)
- [ ] Metrics exposed via existing platform metrics endpoint

## 3. Non-Functional Requirements

### 3.1 Performance

#### NFR-001: API Response Time
**Category**: Performance
**Description**: Weather API calls shall complete within acceptable time limits

**Measurement**: p95 latency
**Target**: <2 seconds for 95% of API calls
**Validation**: Prometheus histogram analysis

#### NFR-002: Storage Throughput
**Category**: Performance
**Description**: Weather data shall be stored without blocking collection

**Measurement**: Channel buffer occupancy
**Target**: <10% buffer occupancy under normal operation
**Validation**: Runtime metrics monitoring

#### NFR-003: Memory Footprint
**Category**: Performance
**Description**: Weather source shall have minimal memory overhead

**Measurement**: RSS memory usage
**Target**: <50 MB additional memory for weather source
**Validation**: Memory profiling during load testing

### 3.2 Reliability

#### NFR-004: Data Collection Uptime
**Category**: Reliability
**Description**: Weather data collection shall maintain high availability

**Measurement**: Successful poll percentage
**Target**: >95% successful polls over 30-day period
**Validation**: SLI tracking dashboard

#### NFR-005: Data Consistency
**Category**: Reliability
**Description**: Weather data shall be stored without corruption

**Measurement**: Parquet file validation
**Target**: 100% valid Parquet files (readable schema, no truncation)
**Validation**: Automated file integrity checks

### 3.3 Scalability

#### NFR-006: Concurrent Stream Support
**Category**: Scalability
**Description**: Weather sources shall not degrade existing stream performance

**Measurement**: Existing stream latency before/after
**Target**: <5% latency increase on existing streams
**Validation**: Comparative load testing

### 3.4 Security

#### NFR-007: API Key Protection
**Category**: Security
**Description**: OpenWeatherMap API key shall be stored securely

**Measurement**: Security audit
**Target**: API key never logged, stored encrypted in etcd
**Validation**: Code review + etcd encryption verification

#### NFR-008: API Rate Limiting
**Category**: Security
**Description**: System shall respect OpenWeatherMap rate limits

**Measurement**: API call frequency
**Target**: <60 calls/minute per API (free tier: 1000/day)
**Validation**: Rate limit monitoring

### 3.5 Maintainability

#### NFR-009: Code Modularity
**Category**: Maintainability
**Description**: Weather integration shall follow existing architecture patterns

**Measurement**: Code review
**Target**: Domain Adapter pattern, <500 lines per module
**Validation**: Architecture review checklist

#### NFR-010: Configuration Flexibility
**Category**: Maintainability
**Description**: All operational parameters shall be configurable

**Measurement**: Configuration coverage
**Target**: 100% runtime parameters in etcd (no hardcoded values)
**Validation**: Configuration audit

### 3.6 Observability

#### NFR-011: Logging Standards
**Category**: Observability
**Description**: Weather source shall follow platform logging standards

**Measurement**: Log structure compliance
**Target**: Structured JSON logs with correlation IDs
**Validation**: Log aggregation pipeline compatibility

#### NFR-012: Tracing Integration
**Category**: Observability
**Description**: Weather API calls shall be traceable end-to-end

**Measurement**: Trace coverage
**Target**: 100% API calls with distributed traces
**Validation**: Jaeger trace inspection

## 4. Data Requirements

### 4.1 Current Weather Data Schema

```rust
// Parquet Schema for weather_current stream
struct CurrentWeather {
    // Primary timestamp (normalized to platform timezone)
    timestamp: i64,  // Unix timestamp in seconds

    // Location metadata
    location_name: String,      // City name
    location_country: String,   // Country code (e.g., "US")
    location_lat: f64,          // Latitude
    location_lon: f64,          // Longitude

    // Weather condition
    weather_id: i32,            // OpenWeatherMap weather condition ID
    weather_main: String,       // Group (Rain, Snow, Clear, etc.)
    weather_description: String, // Detailed description
    weather_icon: String,       // Icon code

    // Temperature (Celsius)
    temp: f64,                  // Current temperature
    feels_like: f64,            // Perceived temperature
    temp_min: f64,              // Minimum temperature
    temp_max: f64,              // Maximum temperature

    // Atmospheric pressure
    pressure: i32,              // Sea level pressure (hPa)
    pressure_sea_level: Option<i32>,  // Sea level pressure if available
    pressure_ground_level: Option<i32>, // Ground level pressure if available

    // Humidity
    humidity: i32,              // Humidity percentage (0-100)

    // Visibility
    visibility: Option<i32>,    // Visibility in meters

    // Wind
    wind_speed: f64,            // Wind speed (m/s)
    wind_deg: Option<i32>,      // Wind direction (degrees, 0-360)
    wind_gust: Option<f64>,     // Wind gust speed (m/s)

    // Precipitation
    rain_1h: Option<f64>,       // Rain volume last hour (mm)
    snow_1h: Option<f64>,       // Snow volume last hour (mm)

    // Cloudiness
    clouds_all: i32,            // Cloudiness percentage (0-100)

    // Sun times (Unix timestamps, normalized)
    sunrise: i64,               // Sunrise time
    sunset: i64,                // Sunset time

    // Metadata
    api_timestamp: i64,         // Original API dt field
    timezone_offset: i32,       // OpenWeatherMap timezone offset (seconds)
    data_source: String,        // "openweathermap_v2.5"
    schema_version: String,     // "1.0.0"
}
```

**Field Constraints:**
- All temperature fields: -100.0 to +70.0 Celsius
- Pressure: 800 to 1100 hPa
- Humidity: 0 to 100
- Wind speed: 0.0 to 150.0 m/s
- Wind direction: 0 to 360 degrees
- Clouds: 0 to 100 percent
- Visibility: 0 to 50000 meters

### 4.2 Air Quality Data Schema

```rust
// Parquet Schema for weather_air_quality stream
struct AirQuality {
    // Primary timestamp (normalized to platform timezone)
    timestamp: i64,  // Unix timestamp in seconds

    // Location metadata
    location_lat: f64,          // Latitude
    location_lon: f64,          // Longitude

    // Air Quality Index
    aqi: i32,                   // 1=Good, 2=Fair, 3=Moderate, 4=Poor, 5=Very Poor

    // Pollutant concentrations (μg/m³)
    co: f64,                    // Carbon monoxide
    no: f64,                    // Nitrogen monoxide
    no2: f64,                   // Nitrogen dioxide
    o3: f64,                    // Ozone
    so2: f64,                   // Sulphur dioxide
    pm2_5: f64,                 // Fine particles matter
    pm10: f64,                  // Coarse particulate matter
    nh3: f64,                   // Ammonia

    // Metadata
    api_timestamp: i64,         // Original API dt field
    data_source: String,        // "openweathermap_v2.5"
    schema_version: String,     // "1.0.0"
}
```

**Field Constraints:**
- AQI: 1 to 5 (integer)
- All pollutants: ≥ 0.0 μg/m³
- Typical ranges:
  - CO: 0-30000 μg/m³
  - NO: 0-500 μg/m³
  - NO2: 0-400 μg/m³
  - O3: 0-500 μg/m³
  - SO2: 0-1000 μg/m³
  - PM2.5: 0-500 μg/m³
  - PM10: 0-600 μg/m³
  - NH3: 0-400 μg/m³

### 4.3 Data Validation Rules

**Required Fields (cannot be NULL):**
- timestamp
- location_lat
- location_lon
- data_source
- schema_version

**Current Weather Required:**
- weather_id
- weather_main
- temp
- pressure
- humidity
- clouds_all

**Air Quality Required:**
- aqi
- All 8 pollutant fields (co, no, no2, o3, so2, pm2_5, pm10, nh3)

**Validation Actions:**
- Invalid required field: Log ERROR, drop record, increment error metric
- Out-of-range optional field: Log WARN, set to NULL, store record
- Missing optional field: Set to NULL, store record

## 5. Configuration Requirements

### 5.1 etcd Configuration Keys

```yaml
# Core weather configuration
/config/weather/enabled: bool                    # Enable/disable weather collection
/config/weather/api_key: string                  # OpenWeatherMap API key (encrypted)
/config/weather/poll_interval: int               # Polling interval in seconds (600)
/config/weather/latitude: float                  # Geographic latitude
/config/weather/longitude: float                 # Geographic longitude
/config/weather/retention_days: int              # Data retention period (90)

# API endpoints (allow override for testing)
/config/weather/api_base_url: string             # Base URL for OpenWeatherMap
/config/weather/current_weather_endpoint: string # Current weather path
/config/weather/air_pollution_endpoint: string   # Air pollution path

# Timeouts and retries
/config/weather/http_timeout_seconds: int        # HTTP request timeout (30)
/config/weather/max_retries: int                 # Max retry attempts (3)
/config/weather/retry_backoff_base_ms: int       # Exponential backoff base (1000)

# Circuit breaker
/config/weather/circuit_breaker_threshold: int   # Consecutive failures to open circuit (10)
/config/weather/circuit_breaker_timeout: int     # Seconds before retry (300)

# Rate limiting
/config/weather/max_calls_per_minute: int        # API rate limit (60)

# System integration
/config/system/timezone: string                  # Platform timezone (UTC)

# Stream registry
/streams/weather_current/config:
  stream_id: "weather_current"
  stream_type: "weather_source"
  schema_version: "1.0.0"
  enabled: true
  storage_path: "/data/weather/current"
  compression: "snappy"

/streams/weather_air_quality/config:
  stream_id: "weather_air_quality"
  stream_type: "weather_source"
  schema_version: "1.0.0"
  enabled: true
  storage_path: "/data/weather/air_quality"
  compression: "snappy"
```

### 5.2 Configuration Validation

**On Startup:**
- [ ] Validate all required config keys exist
- [ ] Validate numeric ranges (lat, lon, intervals)
- [ ] Test API key with health check call
- [ ] Verify etcd connectivity
- [ ] Create missing stream registry entries

**On Hot Reload:**
- [ ] Watch etcd keys for changes
- [ ] Validate new values before applying
- [ ] Log configuration changes
- [ ] Gracefully update polling schedule
- [ ] Don't drop in-flight API calls

### 5.3 Default Configuration

```toml
# config/weather_defaults.toml
[weather]
enabled = false  # Must be explicitly enabled
poll_interval = 600  # 10 minutes
http_timeout_seconds = 30
max_retries = 3
retry_backoff_base_ms = 1000
circuit_breaker_threshold = 10
circuit_breaker_timeout = 300
max_calls_per_minute = 60
retention_days = 90

[api]
base_url = "https://api.openweathermap.org"
current_weather_path = "/data/2.5/weather"
air_pollution_path = "/data/2.5/air_pollution"

# API key and coordinates have no defaults - must be configured
```

## 6. Integration Requirements

### 6.1 Domain Adapter Pattern Integration

#### INT-001: Source Trait Implementation
**Description**: Implement existing Source trait for weather data

**Requirements:**
- [ ] Implement `async fn fetch(&self) -> Result<Vec<DataPoint>>`
- [ ] Implement `async fn health_check(&self) -> HealthStatus`
- [ ] Use existing DataPoint type or extend for weather data
- [ ] Follow async/await patterns used in existing sources
- [ ] Return structured errors (WeatherError enum)

#### INT-002: Stream Registry Integration
**Description**: Register weather streams in existing StreamRegistry

**Requirements:**
- [ ] Use existing StreamRegistry API
- [ ] Store stream config in standard location
- [ ] Support schema evolution via version field
- [ ] Register both weather_current and weather_air_quality streams
- [ ] Enable/disable via stream config

### 6.2 Storage Pipeline Integration

#### INT-003: Channel Integration
**Description**: Send weather data through existing mpsc channel

**Requirements:**
- [ ] Use existing channel from Source to StorageWriter
- [ ] Respect channel buffer size limits
- [ ] Handle channel full scenarios gracefully
- [ ] Batch data points per poll cycle
- [ ] Tag data points with stream_id

#### INT-004: Parquet Storage Integration
**Description**: Store weather data using existing ParquetStore

**Requirements:**
- [ ] Use existing StorageWriter component
- [ ] Follow existing Parquet schema patterns
- [ ] Use same compression settings (SNAPPY)
- [ ] Follow same file rotation patterns
- [ ] Generate Parquet metadata for schema versioning

### 6.3 Configuration Integration

#### INT-005: etcd Integration
**Description**: Use existing etcd client for configuration

**Requirements:**
- [ ] Use platform's etcd client singleton
- [ ] Follow existing config key naming conventions
- [ ] Support hot reload via etcd watch
- [ ] Implement config validation on load
- [ ] Fall back to defaults gracefully

### 6.4 Monitoring Integration

#### INT-006: Metrics Integration
**Description**: Export metrics via existing platform metrics system

**Requirements:**
- [ ] Use existing Prometheus registry
- [ ] Follow metric naming conventions (`{subsystem}_{name}_{unit}`)
- [ ] Add weather-specific labels (endpoint, error_type)
- [ ] Export via existing /metrics HTTP endpoint
- [ ] Document metrics in platform metrics catalog

#### INT-007: Logging Integration
**Description**: Use existing structured logging system

**Requirements:**
- [ ] Use platform's logging framework (tracing/slog)
- [ ] Follow log level conventions (ERROR, WARN, INFO, DEBUG)
- [ ] Include correlation IDs for request tracing
- [ ] Structured logging with key-value pairs
- [ ] No PII in logs (sanitize location names if needed)

#### INT-008: Tracing Integration
**Description**: Instrument weather API calls with distributed tracing

**Requirements:**
- [ ] Use existing OpenTelemetry tracer
- [ ] Create span per API call
- [ ] Tag spans with endpoint, status_code, duration
- [ ] Link spans to parent transaction
- [ ] Export to existing Jaeger instance

## 7. Constraints and Assumptions

### 7.1 Technical Constraints

**CONST-001: Platform Architecture**
- Must run on Raspberry Pi 5 (ARM64 architecture)
- Maximum 8GB RAM available
- Limited CPU resources (shared with neural workloads)
- Storage on SD card or USB drive (limited IOPS)

**CONST-002: Rust Ecosystem**
- Must use Rust 1.70+ (stable)
- Async runtime: Tokio (existing platform choice)
- HTTP client: reqwest (existing platform choice)
- Parquet: arrow-rs (existing platform choice)

**CONST-003: Network Constraints**
- Residential internet connection (may be unstable)
- No static IP address
- Possible firewall restrictions
- DNS resolution required

**CONST-004: API Constraints**
- OpenWeatherMap Free Tier: 1,000 calls/day
- Rate limit: 60 calls/minute
- API updates every 10 minutes (no benefit to faster polling)
- Requires internet connectivity

### 7.2 Assumptions

**ASSUME-001: Configuration**
- Assumes etcd is running and accessible at platform startup
- Assumes valid OpenWeatherMap API key provided by user
- Assumes geographic coordinates don't change frequently
- Assumes platform timezone configured correctly

**ASSUME-002: Data Usage**
- Assumes weather data will be correlated with neural metrics in separate analysis
- Assumes 90-day retention sufficient for initial use cases
- Assumes single location sufficient (user's home)
- Assumes metric units preferred over imperial

**ASSUME-003: Operations**
- Assumes platform runs 24/7 (or near-24/7)
- Assumes system restarts are infrequent
- Assumes user monitors health dashboard
- Assumes log aggregation available for debugging

**ASSUME-004: Dependencies**
- Assumes OpenWeatherMap API remains stable (v2.5)
- Assumes existing platform components (etcd, Parquet, channels) are reliable
- Assumes no breaking changes to Domain Adapter pattern
- Assumes Rust ecosystem dependencies remain compatible

### 7.3 Dependencies

**External Dependencies:**
- OpenWeatherMap API (availability, rate limits, schema stability)
- Internet connectivity (DNS, HTTPS)
- System time synchronization (NTP)

**Internal Dependencies:**
- etcd cluster health
- StorageWriter availability
- Parquet storage capacity
- Platform metrics system

**Development Dependencies:**
- Rust toolchain (cargo, rustc)
- Testing framework (existing test harness)
- Mock API server (for integration tests)
- Parquet reader tools (for validation)

## 8. Acceptance Criteria

### 8.1 Functional Acceptance

**AC-F-001: End-to-End Data Flow**
```gherkin
Feature: Weather Data Collection

  Scenario: Successful weather data collection
    Given the weather source is enabled
    And valid API key is configured
    And coordinates are set to "40.7128,-74.0060" (NYC)
    When the polling cycle executes
    Then current weather data is fetched from OpenWeatherMap
    And air quality data is fetched from OpenWeatherMap
    And both datasets are normalized to platform timezone
    And data is sent to StorageWriter via channel
    And Parquet files are written to /data/weather/current/{date}.parquet
    And Parquet files are written to /data/weather/air_quality/{date}.parquet
    And metrics show successful API calls
    And health check returns OK status
```

**AC-F-002: Configuration Hot Reload**
```gherkin
Feature: Dynamic Configuration

  Scenario: Update polling interval without restart
    Given weather source is running with 600s interval
    When etcd key /config/weather/poll_interval is updated to 900
    Then the polling schedule is updated within 30 seconds
    And no in-flight API calls are dropped
    And next poll occurs at new interval
    And configuration change is logged
```

**AC-F-003: Error Handling**
```gherkin
Feature: Resilient Data Collection

  Scenario: API rate limit exceeded
    Given weather source is running
    When OpenWeatherMap returns HTTP 429 (Too Many Requests)
    Then the request is not retried immediately
    And backoff timer is set to 60 seconds
    And error is logged with WARN level
    And metric weather_poll_errors_total is incremented
    And health check remains OK if recent data available
    And next poll resumes after backoff period
```

**AC-F-004: Data Validation**
```gherkin
Feature: Data Quality

  Scenario: Out-of-range temperature value
    Given weather API returns temperature of 150.0 C
    When data validation runs
    Then the record is logged with WARN
    And temperature field is set to NULL
    And record is still stored (other fields valid)
    And metric validation_errors_total is incremented
    And error details include field name and value
```

### 8.2 Non-Functional Acceptance

**AC-NF-001: Performance**
- [ ] 95% of API calls complete in <2 seconds
- [ ] Weather source uses <50 MB RAM
- [ ] No impact on existing neural stream latency (within 5%)
- [ ] Channel buffer occupancy <10% during normal operation
- [ ] Parquet files written within 100ms of data receipt

**AC-NF-002: Reliability**
- [ ] >95% poll success rate over 30 days
- [ ] Zero data corruption in Parquet files
- [ ] Graceful degradation on API failures
- [ ] Automatic recovery after transient errors
- [ ] No memory leaks over 7-day run

**AC-NF-003: Security**
- [ ] API key never appears in logs
- [ ] API key encrypted at rest in etcd
- [ ] HTTPS used for all API calls
- [ ] No sensitive data in error messages
- [ ] Rate limiting enforced (60 calls/min)

**AC-NF-004: Maintainability**
- [ ] Code follows Rust style guide (rustfmt)
- [ ] All public APIs documented (rustdoc)
- [ ] Unit test coverage >80%
- [ ] Integration tests for happy path and error cases
- [ ] No clippy warnings at default level

**AC-NF-005: Observability**
- [ ] All metrics documented in metrics catalog
- [ ] Structured logs parseable by log aggregator
- [ ] Distributed traces link to API calls
- [ ] Health check provides actionable status
- [ ] Dashboard panels display key metrics

### 8.3 Integration Acceptance

**AC-I-001: Domain Adapter Compliance**
- [ ] WeatherSource implements Source trait
- [ ] fetch() method returns DataPoint Vec
- [ ] health_check() returns HealthStatus enum
- [ ] Follows async/await patterns
- [ ] Errors implement std::error::Error

**AC-I-002: Storage Pipeline Compliance**
- [ ] Data sent via existing mpsc channel
- [ ] DataPoints tagged with correct stream_id
- [ ] Timestamps in Unix epoch seconds
- [ ] Schema matches Parquet expectations
- [ ] Compression matches platform standard

**AC-I-003: Configuration Compliance**
- [ ] All config keys under /config/weather/
- [ ] Stream registry entries under /streams/
- [ ] Watches etcd for config changes
- [ ] Validates config before applying
- [ ] Defaults in config/weather_defaults.toml

### 8.4 User Acceptance

**AC-U-001: Setup Experience**
- [ ] User can enable weather collection by setting single config key
- [ ] User provides API key via secure method (not plaintext config file)
- [ ] User sees weather data in dashboard within 10 minutes of setup
- [ ] User can verify health via existing health check endpoint
- [ ] User documentation includes API key signup instructions

**AC-U-002: Operational Experience**
- [ ] User can change coordinates without code changes
- [ ] User can adjust polling interval for their use case
- [ ] User can disable weather collection without system restart
- [ ] User can query weather data alongside neural metrics
- [ ] User can troubleshoot via logs and metrics

## 9. Glossary

**AQI (Air Quality Index)**: Integer scale from 1 (Good) to 5 (Very Poor) representing overall air quality based on pollutant concentrations.

**Channel (mpsc)**: Multi-producer, single-consumer queue used for async message passing between Source and StorageWriter.

**Circuit Breaker**: Pattern that stops attempts to call failing API after threshold reached, preventing resource exhaustion.

**Domain Adapter**: Architectural pattern for integrating external data sources via standardized Source trait interface.

**etcd**: Distributed key-value store used for platform configuration and service discovery.

**Health Check**: Diagnostic endpoint returning OK/DEGRADED/FAILED status based on recent data collection success.

**Parquet**: Columnar storage format used for efficient time-series data storage and querying.

**Polling Cycle**: Periodic execution of API calls to collect weather data (default: every 10 minutes).

**Source Trait**: Rust trait defining interface for data sources with fetch() and health_check() methods.

**StorageWriter**: Platform component responsible for writing DataPoints to Parquet files on disk.

**Stream**: Named sequence of time-series data (e.g., weather_current, weather_air_quality).

**Stream Registry**: etcd-based catalog of active data streams with their configurations and schemas.

**Timestamp Normalization**: Converting API-provided Unix timestamps to platform's configured timezone.

**μg/m³ (Micrograms per cubic meter)**: Unit of measurement for air pollutant concentration.

## 10. References

**External Documentation:**
- OpenWeatherMap Current Weather API: https://openweathermap.org/current
- OpenWeatherMap Air Pollution API: https://openweathermap.org/api/air-pollution
- OpenWeatherMap API Authentication: https://openweathermap.org/appid

**Platform Documentation:**
- Neural Data Platform Architecture Overview: `/docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md`
- Domain Adapter Pattern: `/docs/architecture/COMPONENT_DEPENDENCY_MAP.md`
- Stream Registry Specification: (to be documented)
- Storage Pipeline Design: (to be documented)

**Project Artifacts:**
- Feature Scope: `/product/features/air-005/scope.md`
- Research Notes: `/product/features/air-005/research/`
- Implementation Plan: (to be created in Pseudocode phase)

## 11. Revision History

| Version | Date       | Author              | Changes                    |
|---------|------------|---------------------|----------------------------|
| 1.0.0   | 2025-12-16 | SPARC Spec Agent    | Initial specification      |

---

**Document Status**: DRAFT
**Next Phase**: Pseudocode (Algorithm Design)
**Approver**: Product Owner / Technical Lead

**Review Checklist**:
- [ ] All requirements testable
- [ ] Acceptance criteria clear
- [ ] Edge cases documented
- [ ] Performance metrics defined
- [ ] Security requirements specified
- [ ] Dependencies identified
- [ ] Constraints documented
- [ ] Stakeholder approval obtained
