# AIR-005: Weather Data Integration - Architecture Document

**Version**: 1.1.0
**Last Updated**: 2025-12-16
**Status**: Design Phase
**SPARC Phase**: Architecture

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Current State Analysis](#current-state-analysis)
3. [Generic HTTP Polling Design](#generic-http-polling-design)
4. [Data Flow Architecture](#data-flow-architecture)
5. [Configuration Architecture](#configuration-architecture)
6. [Integration with Existing Components](#integration-with-existing-components)
7. [Resource Considerations](#resource-considerations)
8. [Error Handling Strategy](#error-handling-strategy)
9. [Security Considerations](#security-considerations)
10. [Deployment Architecture](#deployment-architecture)

---

## 1. Architecture Overview

### 1.1 Executive Summary

AIR-005 extends the Neural Data Platform to ingest outdoor weather and air quality data from OpenWeatherMap APIs. This feature requires **refactoring the existing `HttpPollingSource`** from a hardcoded implementation to a generic, configuration-driven HTTP polling system.

### 1.2 Design Principles

1. **Generic HTTP Polling**: Refactor `HttpPollingSource` to support any HTTP API via configurable response parsers
2. **Configuration-Driven**: All API endpoints, coordinates, auth, and intervals stored in etcd
3. **Stream Registry Pattern**: Two new streams (`outdoor-weather`, `outdoor-air-quality`) using existing StreamConfig
4. **Backward Compatible**: Existing functionality preserved (though AirGradient uses MQTT, not HTTP polling)
5. **Minimal Code Changes**: Primarily abstraction and configuration additions

### 1.3 Architectural Constraints

- **Memory Budget**: Must stay within 512MB limit for air-quality-app container
- **API Rate Limits**: Free tier = 1000 calls/day (plan for 10-minute polls = 288 calls/day)
- **Network Reliability**: Handle transient network failures with retry logic
- **Timezone Consistency**: All timestamps normalized to UTC

---

## 2. Current State Analysis

### 2.1 Existing `HttpPollingSource` (core/src/sources/http_poll.rs)

**What EXISTS and can be reused:**

| Component | Status | Notes |
|-----------|--------|-------|
| `HttpPollingConfig` | ✅ | poll_interval, timeout, buffer_capacity |
| `HttpPollingSource` | ✅ | Implements `Source` trait |
| Polling loop | ✅ | tokio interval-based background task |
| HTTP client | ✅ | reqwest with configurable timeout |
| Health check | ✅ | Staleness detection (2x poll_interval) |
| Channel buffering | ✅ | mpsc with configurable capacity |
| Unit tests | ✅ | wiremock-based mocking |

**What MUST be added/refactored:**

| Component | Status | Required Change |
|-----------|--------|-----------------|
| Response parsing | ❌ Hardcoded | Add `ResponseParser` trait |
| Auth support | ❌ Missing | Add `AuthMethod` enum |
| Retry logic | ❌ Missing | Add exponential backoff |
| Rate limit handling | ❌ Missing | Handle 429 with Retry-After |
| Error classification | ❌ Missing | Transient vs Permanent errors |
| etcd integration | ❌ Missing | Load config from StreamRegistry |

### 2.2 Current Hardcoded Structure (to be removed/abstracted)

```rust
// CURRENT: Hardcoded for a specific API format
struct CurrentMeasures {
    pm02: Option<f64>,
    co2: Option<f64>,
    temperature: Option<f64>,
    // ... specific fields
}

// FUTURE: Generic parser trait
trait ResponseParser {
    fn parse(&self, response: &str, location_id: &str) -> CoreResult<Vec<TimeSeriesPoint>>;
}
```

---

## 3. Generic HTTP Polling Design

### 3.1 Component Diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│                        air-quality-app Container                    │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌───────────────────────────────────────────────────────────────┐ │
│  │                    Configuration Layer                        │ │
│  ├───────────────────────────────────────────────────────────────┤ │
│  │  StreamRegistry (config-client)                               │ │
│  │  - Loads: /streams/outdoor-weather/config                     │ │
│  │  - Loads: /streams/outdoor-air-quality/config                 │ │
│  │  - Watches for config updates (hot-reload)                    │ │
│  └─────────────────────────┬─────────────────────────────────────┘ │
│                            │                                        │
│                            ▼                                        │
│  ┌───────────────────────────────────────────────────────────────┐ │
│  │                   Source Layer (Refactored)                   │ │
│  ├───────────────────────────────────────────────────────────────┤ │
│  │                                                                │ │
│  │  ┌──────────────────┐      ┌──────────────────────────────┐  │ │
│  │  │  MqttSource      │      │  HttpPollingSource (Generic) │  │ │
│  │  │  (indoor air)    │      │                              │  │ │
│  │  │                  │      │  ┌────────────────────────┐  │  │ │
│  │  │                  │      │  │ ParserRegistry         │  │  │ │
│  │  │                  │      │  │ ├─ WeatherParser       │  │  │ │
│  │  │                  │      │  │ └─ AirPollutionParser  │  │  │ │
│  │  │                  │      │  └────────────────────────┘  │  │ │
│  │  │                  │      │                              │  │ │
│  │  │                  │      │  ┌────────────────────────┐  │  │ │
│  │  │                  │      │  │ RetryHandler           │  │  │ │
│  │  │                  │      │  │ - Exponential backoff  │  │  │ │
│  │  │                  │      │  │ - Rate limit handling  │  │  │ │
│  │  │                  │      │  └────────────────────────┘  │  │ │
│  │  └────────┬─────────┘      └──────────────┬───────────────┘  │ │
│  │           │                               │                  │ │
│  └───────────┼───────────────────────────────┼──────────────────┘ │
│              │                               │                    │
│              ├───────────────────────────────┤                    │
│              │    tokio::mpsc::channel       │                    │
│              │    (TimeSeriesPoint flow)     │                    │
│              └───────────────┬───────────────┘                    │
│                              │                                    │
│                              ▼                                    │
│  ┌───────────────────────────────────────────────────────────────┐ │
│  │                     Storage Layer                              │ │
│  ├───────────────────────────────────────────────────────────────┤ │
│  │  StorageWriter → ParquetStore                                  │ │
│  │  - /data/outdoor-weather/YYYY-MM-DD_HH.parquet                │ │
│  │  - /data/outdoor-air-quality/YYYY-MM-DD_HH.parquet            │ │
│  └───────────────────────────────────────────────────────────────┘ │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

### 3.2 New Type Definitions

#### 3.2.1 ResponseParser Trait

```rust
/// Trait for parsing HTTP API responses into TimeSeriesPoints
pub trait ResponseParser: Send + Sync + 'static {
    /// Parse raw JSON response into time series points
    fn parse(
        &self,
        response_body: &str,
        location_id: &str,
        timestamp: DateTime<Utc>,
    ) -> CoreResult<Vec<TimeSeriesPoint>>;

    /// Parser identifier for logging and config
    fn name(&self) -> &'static str;
}
```

#### 3.2.2 AuthMethod Enum

```rust
/// Authentication methods for HTTP APIs
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthMethod {
    /// No authentication
    None,
    /// API key as query parameter (e.g., ?appid=KEY)
    QueryParam {
        param_name: String,
        #[serde(skip_serializing)]
        value: String,  // Loaded from env var
    },
    /// API key in header (e.g., X-API-Key: KEY)
    Header {
        header_name: String,
        #[serde(skip_serializing)]
        value: String,
    },
    /// HTTP Basic Auth
    BasicAuth {
        username: String,
        #[serde(skip_serializing)]
        password: String,
    },
}
```

#### 3.2.3 RetryConfig

```rust
/// Configuration for retry behavior
#[derive(Debug, Clone, Deserialize)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Initial delay before first retry
    pub initial_delay_ms: u64,
    /// Maximum delay between retries
    pub max_delay_ms: u64,
    /// Multiplier for exponential backoff
    pub backoff_multiplier: f64,
    /// Add jitter to prevent thundering herd
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 1000,
            max_delay_ms: 60000,
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }
}
```

#### 3.2.4 EndpointConfig (replaces SensorConfig)

```rust
/// Configuration for a single HTTP endpoint
#[derive(Debug, Clone, Deserialize)]
pub struct EndpointConfig {
    /// Unique identifier for this endpoint
    pub id: String,
    /// Full URL to poll
    pub url: String,
    /// Authentication method
    pub auth: AuthMethod,
    /// Parser type to use (registered in ParserRegistry)
    pub parser_type: String,
    /// Whether this endpoint is enabled
    pub enabled: bool,
    /// Optional query parameters
    pub query_params: Option<HashMap<String, String>>,
}
```

#### 3.2.5 Updated HttpPollingConfig

```rust
/// Configuration for generic HTTP polling source
#[derive(Debug, Clone, Deserialize)]
pub struct HttpPollingConfig {
    /// Interval between polls
    pub poll_interval_secs: u64,
    /// HTTP request timeout
    pub timeout_secs: u64,
    /// Channel buffer capacity
    pub buffer_capacity: usize,
    /// Retry configuration
    pub retry: RetryConfig,
    /// Endpoints to poll
    pub endpoints: Vec<EndpointConfig>,
}
```

### 3.3 Parser Implementations

#### 3.3.1 OpenWeatherMap Current Weather Parser

```rust
pub struct WeatherParser;

impl ResponseParser for WeatherParser {
    fn name(&self) -> &'static str {
        "openweather_current"
    }

    fn parse(
        &self,
        response_body: &str,
        location_id: &str,
        timestamp: DateTime<Utc>,
    ) -> CoreResult<Vec<TimeSeriesPoint>> {
        let data: OpenWeatherResponse = serde_json::from_str(response_body)?;

        let mut points = Vec::new();

        // Temperature
        points.push(create_point(timestamp, location_id, "temperature", data.main.temp));
        points.push(create_point(timestamp, location_id, "feels_like", data.main.feels_like));
        points.push(create_point(timestamp, location_id, "pressure", data.main.pressure as f64));
        points.push(create_point(timestamp, location_id, "humidity", data.main.humidity as f64));

        // Wind
        points.push(create_point(timestamp, location_id, "wind_speed", data.wind.speed));
        if let Some(deg) = data.wind.deg {
            points.push(create_point(timestamp, location_id, "wind_deg", deg as f64));
        }
        if let Some(gust) = data.wind.gust {
            points.push(create_point(timestamp, location_id, "wind_gust", gust));
        }

        // Clouds & visibility
        points.push(create_point(timestamp, location_id, "clouds", data.clouds.all as f64));
        if let Some(vis) = data.visibility {
            points.push(create_point(timestamp, location_id, "visibility", vis as f64));
        }

        // Precipitation (optional)
        if let Some(rain) = &data.rain {
            if let Some(h1) = rain.h1 {
                points.push(create_point(timestamp, location_id, "rain_1h", h1));
            }
        }
        if let Some(snow) = &data.snow {
            if let Some(h1) = snow.h1 {
                points.push(create_point(timestamp, location_id, "snow_1h", h1));
            }
        }

        Ok(points)
    }
}
```

#### 3.3.2 OpenWeatherMap Air Pollution Parser

```rust
pub struct AirPollutionParser;

impl ResponseParser for AirPollutionParser {
    fn name(&self) -> &'static str {
        "openweather_air_pollution"
    }

    fn parse(
        &self,
        response_body: &str,
        location_id: &str,
        timestamp: DateTime<Utc>,
    ) -> CoreResult<Vec<TimeSeriesPoint>> {
        let data: AirPollutionResponse = serde_json::from_str(response_body)?;

        let reading = data.list.first()
            .ok_or_else(|| CoreError::Source("No air pollution data in response".into()))?;

        let mut points = Vec::new();
        let c = &reading.components;

        points.push(create_point(timestamp, location_id, "aqi", reading.main.aqi as f64));
        points.push(create_point(timestamp, location_id, "co", c.co));
        points.push(create_point(timestamp, location_id, "no", c.no));
        points.push(create_point(timestamp, location_id, "no2", c.no2));
        points.push(create_point(timestamp, location_id, "o3", c.o3));
        points.push(create_point(timestamp, location_id, "so2", c.so2));
        points.push(create_point(timestamp, location_id, "pm2_5", c.pm2_5));
        points.push(create_point(timestamp, location_id, "pm10", c.pm10));
        points.push(create_point(timestamp, location_id, "nh3", c.nh3));

        Ok(points)
    }
}
```

### 3.4 Parser Registry

```rust
use std::collections::HashMap;
use std::sync::Arc;

/// Registry of available response parsers
pub struct ParserRegistry {
    parsers: HashMap<String, Arc<dyn ResponseParser>>,
}

impl ParserRegistry {
    pub fn new() -> Self {
        let mut parsers: HashMap<String, Arc<dyn ResponseParser>> = HashMap::new();

        // Register built-in parsers
        parsers.insert(
            "openweather_current".to_string(),
            Arc::new(WeatherParser)
        );
        parsers.insert(
            "openweather_air_pollution".to_string(),
            Arc::new(AirPollutionParser)
        );

        Self { parsers }
    }

    pub fn get(&self, parser_type: &str) -> Option<Arc<dyn ResponseParser>> {
        self.parsers.get(parser_type).cloned()
    }

    pub fn register(&mut self, name: String, parser: Arc<dyn ResponseParser>) {
        self.parsers.insert(name, parser);
    }
}

impl Default for ParserRegistry {
    fn default() -> Self {
        Self::new()
    }
}
```

---

## 4. Data Flow Architecture

### 4.1 Weather Data Ingestion Flow

```
┌─────────────────────────────────────────────────────────────────────┐
│                      Polling Cycle (every 10 min)                    │
└─────────────────────────────────────────────────────────────────────┘
                                   │
                                   ▼
┌─────────────────────────────────────────────────────────────────────┐
│ 1. HttpPollingSource.poll_all_endpoints()                           │
│    - Iterate through enabled endpoints                               │
│    - For each endpoint:                                              │
└─────────────────────────────────────────────────────────────────────┘
                                   │
                                   ▼
┌─────────────────────────────────────────────────────────────────────┐
│ 2. Build HTTP Request                                                │
│    - Apply auth (QueryParam: ?appid=API_KEY)                        │
│    - Apply query params (?lat=X&lon=Y&units=metric)                 │
│    - Set timeout                                                     │
└─────────────────────────────────────────────────────────────────────┘
                                   │
                                   ▼
┌─────────────────────────────────────────────────────────────────────┐
│ 3. Execute with Retry                                                │
│    - Send request                                                    │
│    - On error: classify (Transient/RateLimited/Permanent)           │
│    - On 429: parse Retry-After, backoff                             │
│    - On transient: exponential backoff, retry                        │
│    - On permanent: log error, skip endpoint                          │
└─────────────────────────────────────────────────────────────────────┘
                                   │
                                   ▼
┌─────────────────────────────────────────────────────────────────────┐
│ 4. Parse Response                                                    │
│    - Lookup parser from ParserRegistry by endpoint.parser_type      │
│    - parser.parse(response_body, location_id, timestamp)            │
│    - Returns Vec<TimeSeriesPoint>                                   │
└─────────────────────────────────────────────────────────────────────┘
                                   │
                                   ▼
┌─────────────────────────────────────────────────────────────────────┐
│ 5. Send to Channel                                                   │
│    - sender.send(point).await for each point                        │
│    - Update last_successful_poll timestamp                           │
│    - Reset error counter on success                                  │
└─────────────────────────────────────────────────────────────────────┘
                                   │
                                   ▼
┌─────────────────────────────────────────────────────────────────────┐
│ 6. StorageWriter Receives                                            │
│    - Batches points (100 points or 5s timeout)                      │
│    - Routes to correct stream based on tags                          │
│    - Writes to Parquet                                               │
└─────────────────────────────────────────────────────────────────────┘
```

### 4.2 Multi-Stream Coordination

```
Time: T0 (00:00:00)
├─ MqttSource: Indoor air quality (continuous, ~1 msg/min)
└─ HttpPollingSource:
   ├─ outdoor-weather endpoint (every 600s)
   └─ outdoor-air-quality endpoint (every 600s)

All sources → Single mpsc::channel → StorageWriter → ParquetStore
                                                       ├─ /data/air-quality/        (existing)
                                                       ├─ /data/outdoor-weather/    (new)
                                                       └─ /data/outdoor-air-quality/ (new)
```

---

## 5. Configuration Architecture

### 5.1 etcd Key Structure

```
/streams/
├── outdoor-weather/
│   └── config              # Full stream config YAML
│
├── outdoor-air-quality/
│   └── config              # Full stream config YAML
│
├── air-quality/            # Existing indoor air quality
│   └── config
│
/config/
└── weather/
    ├── api_key_env         # Environment variable name (OPENWEATHERMAP_API_KEY)
    ├── latitude            # Shared coordinate
    ├── longitude           # Shared coordinate
    └── poll_interval_secs  # Default poll interval
```

### 5.2 StreamConfig: outdoor-weather

```yaml
stream_id: "outdoor-weather"
description: "Outdoor weather data from OpenWeatherMap"
version: "1.0.0"
enabled: true
retention_days: 365
compression_after_days: 30
partitioning_strategy: "daily"

fields:
  - name: "temperature"
    type: "float"
    unit: "celsius"
    range: [-50.0, 60.0]
    nullable: false

  - name: "feels_like"
    type: "float"
    unit: "celsius"
    range: [-50.0, 60.0]
    nullable: false

  - name: "pressure"
    type: "float"
    unit: "hPa"
    range: [800.0, 1100.0]
    nullable: false

  - name: "humidity"
    type: "float"
    unit: "percent"
    range: [0.0, 100.0]
    nullable: false

  - name: "wind_speed"
    type: "float"
    unit: "m/s"
    range: [0.0, 100.0]
    nullable: false

  - name: "wind_deg"
    type: "float"
    unit: "degrees"
    range: [0.0, 360.0]
    nullable: true

  - name: "wind_gust"
    type: "float"
    unit: "m/s"
    range: [0.0, 150.0]
    nullable: true

  - name: "clouds"
    type: "float"
    unit: "percent"
    range: [0.0, 100.0]
    nullable: false

  - name: "visibility"
    type: "float"
    unit: "meters"
    range: [0.0, 50000.0]
    nullable: true

  - name: "rain_1h"
    type: "float"
    unit: "mm"
    range: [0.0, 500.0]
    nullable: true

  - name: "snow_1h"
    type: "float"
    unit: "mm"
    range: [0.0, 500.0]
    nullable: true

sources:
  - type: "http_poll"
    enabled: true
    endpoint:
      id: "openweather-current"
      url: "https://api.openweathermap.org/data/2.5/weather"
      parser_type: "openweather_current"
      auth:
        type: "query_param"
        param_name: "appid"
        value_env: "OPENWEATHERMAP_API_KEY"
      query_params:
        lat: "${WEATHER_LATITUDE}"
        lon: "${WEATHER_LONGITUDE}"
        units: "metric"
    poll_interval_secs: 600
    timeout_secs: 30
    retry:
      max_retries: 3
      initial_delay_ms: 1000
      max_delay_ms: 60000
      backoff_multiplier: 2.0

storage:
  batch_size: 100
  batch_timeout_secs: 5
  buffer_capacity: 1000
```

### 5.3 StreamConfig: outdoor-air-quality

```yaml
stream_id: "outdoor-air-quality"
description: "Outdoor air quality data from OpenWeatherMap"
version: "1.0.0"
enabled: true
retention_days: 365
compression_after_days: 30
partitioning_strategy: "daily"

fields:
  - name: "aqi"
    type: "int"
    range: [1, 5]
    nullable: false
    description: "Air Quality Index (1=Good, 5=Very Poor)"

  - name: "co"
    type: "float"
    unit: "μg/m³"
    range: [0.0, 50000.0]
    nullable: false

  - name: "no"
    type: "float"
    unit: "μg/m³"
    range: [0.0, 1000.0]
    nullable: false

  - name: "no2"
    type: "float"
    unit: "μg/m³"
    range: [0.0, 1000.0]
    nullable: false

  - name: "o3"
    type: "float"
    unit: "μg/m³"
    range: [0.0, 1000.0]
    nullable: false

  - name: "so2"
    type: "float"
    unit: "μg/m³"
    range: [0.0, 1000.0]
    nullable: false

  - name: "pm2_5"
    type: "float"
    unit: "μg/m³"
    range: [0.0, 1000.0]
    nullable: false

  - name: "pm10"
    type: "float"
    unit: "μg/m³"
    range: [0.0, 1000.0]
    nullable: false

  - name: "nh3"
    type: "float"
    unit: "μg/m³"
    range: [0.0, 1000.0]
    nullable: false

sources:
  - type: "http_poll"
    enabled: true
    endpoint:
      id: "openweather-air-pollution"
      url: "https://api.openweathermap.org/data/2.5/air_pollution"
      parser_type: "openweather_air_pollution"
      auth:
        type: "query_param"
        param_name: "appid"
        value_env: "OPENWEATHERMAP_API_KEY"
      query_params:
        lat: "${WEATHER_LATITUDE}"
        lon: "${WEATHER_LONGITUDE}"
    poll_interval_secs: 600
    timeout_secs: 30
    retry:
      max_retries: 3
      initial_delay_ms: 1000
      max_delay_ms: 60000
      backoff_multiplier: 2.0

storage:
  batch_size: 100
  batch_timeout_secs: 5
  buffer_capacity: 1000
```

---

## 6. Integration with Existing Components

### 6.1 Changes Required

**File**: `core/src/sources/http_poll.rs`

| Change | Description | Lines Affected |
|--------|-------------|----------------|
| Add `ResponseParser` trait | New trait for parsing | +20 |
| Add `AuthMethod` enum | Auth configuration | +25 |
| Add `RetryConfig` struct | Retry settings | +20 |
| Replace `SensorConfig` with `EndpointConfig` | Generic endpoint config | +15, -10 |
| Add `ParserRegistry` | Parser lookup | +30 |
| Refactor `poll_sensor` → `poll_endpoint` | Use parser + auth | +50, -40 |
| Add `poll_with_retry` | Retry logic | +40 |
| Add error classification | Transient/Permanent | +20 |
| Update `HttpPollingConfig` | New fields | +10, -5 |

**New Files**:
- `core/src/sources/parsers/mod.rs` - Parser module
- `core/src/sources/parsers/weather.rs` - OpenWeatherMap current parser
- `core/src/sources/parsers/air_pollution.rs` - OpenWeatherMap air pollution parser

**Estimated Total**: ~250 lines added, ~55 lines removed

### 6.2 No Changes Required

- `core/src/traits.rs` - `Source` trait unchanged
- `core/src/storage/parquet.rs` - Already supports multiple streams
- `core/src/storage/wal.rs` - Works with any TimeSeriesPoint
- `config-client/src/stream/registry.rs` - Already supports multiple streams
- `deploy/pi/docker-compose.yml` - No changes needed

### 6.3 Component Interaction Matrix

| Component | MqttSource | HttpPollingSource | StorageWriter | ParquetStore | StreamRegistry |
|-----------|------------|-------------------|---------------|--------------|----------------|
| **MqttSource** | - | Independent | Sends to | - | Loads config |
| **HttpPollingSource** | Independent | - | Sends to | - | Loads config |
| **StorageWriter** | Receives from | Receives from | - | Writes to | - |
| **ParquetStore** | - | - | Receives from | - | - |
| **StreamRegistry** | Configures | Configures | - | - | - |

---

## 7. Resource Considerations

### 7.1 Memory Impact

**Current Memory Usage**:
- air-quality-app: ~200MB (limit: 512MB)

**Additional Memory for Weather**:
- 2x endpoint configs: ~2KB
- Parser registry: ~1KB
- Additional channel buffer: ~2MB (shared)
- HTTP response buffers: ~5KB per response

**Total Additional**: ~5MB

**Projected Usage**: ~205MB / 512MB (40% of limit)

### 7.2 Network Impact

**API Usage**:
- Weather API: 1 call every 10 min = 144 calls/day
- Air Pollution API: 1 call every 10 min = 144 calls/day
- **Total**: 288 calls/day (28.8% of 1000 free tier limit)

**Bandwidth**:
- Weather response: ~1KB
- Air pollution response: ~0.5KB
- **Daily**: 288 × 1.5KB = ~432KB/day

### 7.3 Storage Impact

**New Parquet Files**:
- Weather: ~18 fields × 144 points/day = ~2,592 values/day → ~50KB/day
- Air pollution: ~9 fields × 144 points/day = ~1,296 values/day → ~25KB/day
- **Total**: ~75KB/day, ~27MB/year

---

## 8. Error Handling Strategy

### 8.1 Error Classification

```rust
#[derive(Debug, Clone, Copy)]
pub enum ErrorType {
    /// Temporary errors that should be retried
    Transient,
    /// Rate limit exceeded - respect Retry-After
    RateLimited { retry_after_secs: u64 },
    /// Permanent errors - don't retry
    Permanent,
}

impl ErrorType {
    pub fn classify(status: Option<u16>, error: &CoreError) -> Self {
        match status {
            Some(429) => ErrorType::RateLimited { retry_after_secs: 60 },
            Some(401) | Some(403) => ErrorType::Permanent,
            Some(404) => ErrorType::Permanent,
            Some(s) if s >= 500 => ErrorType::Transient,
            Some(s) if s >= 400 => ErrorType::Permanent,
            None => ErrorType::Transient, // Network errors
            _ => ErrorType::Transient,
        }
    }
}
```

### 8.2 Retry Flow

```
Request Failed
    │
    ├─ Classify Error
    │   ├─ Permanent (401, 403, 404, 4xx) → Log error, skip endpoint
    │   ├─ RateLimited (429) → Parse Retry-After, wait, retry
    │   └─ Transient (5xx, network) → Exponential backoff, retry
    │
    ├─ If retrying:
    │   ├─ Calculate delay: min(initial × 2^attempt, max_delay)
    │   ├─ Add jitter: delay × (1 + random(0, 0.1))
    │   ├─ Sleep(delay)
    │   └─ Retry request
    │
    └─ If max retries exceeded → Log error, mark endpoint unhealthy
```

### 8.3 Health Check Enhancement

```rust
pub async fn health_check(&self) -> CoreResult<HealthStatus> {
    let mut details = HashMap::new();
    let now = Utc::now();

    let mut unhealthy = Vec::new();
    let mut degraded = Vec::new();

    for endpoint in &self.config.endpoints {
        if !endpoint.enabled {
            continue;
        }

        let last_poll = self.last_successful_poll.lock().await;
        let max_age = Duration::from_secs(self.config.poll_interval_secs * 2);

        match last_poll.get(&endpoint.id) {
            None => unhealthy.push(endpoint.id.clone()),
            Some(time) if now.signed_duration_since(*time) > max_age => {
                degraded.push(endpoint.id.clone());
            }
            _ => {}
        }
    }

    let healthy = unhealthy.is_empty() && degraded.is_empty();

    Ok(HealthStatus { healthy, message, details })
}
```

---

## 9. Security Considerations

### 9.1 API Key Management

```
┌────────────────────────────────────────────────────────────┐
│                  API Key Security Layers                   │
├────────────────────────────────────────────────────────────┤
│ 1. Environment Variable                                    │
│    - OPENWEATHERMAP_API_KEY in .env                        │
│    - .env in .gitignore                                    │
│    - File permissions: 600                                  │
│                                                            │
│ 2. Config Reference                                        │
│    - Stream config uses: value_env: "OPENWEATHERMAP_API_KEY"│
│    - API key never stored in etcd                          │
│    - Loaded at runtime from environment                    │
│                                                            │
│ 3. Runtime Protection                                      │
│    - Never log API key (redact in logs)                   │
│    - HTTPS only (reqwest with https_only)                 │
│    - Key not included in error messages                    │
└────────────────────────────────────────────────────────────┘
```

### 9.2 Request Security

```rust
// Enforce HTTPS
let client = Client::builder()
    .timeout(Duration::from_secs(config.timeout_secs))
    .https_only(true)
    .build()?;
```

---

## 10. Deployment Architecture

### 10.1 Deployment Flow

```
1. Update .env with OPENWEATHERMAP_API_KEY

2. Load stream configs into etcd:
   ./scripts/load-stream-config.sh outdoor-weather
   ./scripts/load-stream-config.sh outdoor-air-quality

3. Restart air-quality-app:
   docker compose restart air-quality-app

4. Verify:
   - Check logs for "Starting HTTP polling for endpoint: openweather-*"
   - Verify Parquet files created in /data/outdoor-weather/
   - Check health endpoint
```

### 10.2 Hot Reload Support

The StreamRegistry watches etcd for config changes. When enabled:
- New endpoints can be added without restart
- Poll intervals can be adjusted
- Endpoints can be enabled/disabled
- Parser types cannot be changed (requires restart)

### 10.3 Rollback

```bash
# Disable via etcd (immediate, no restart)
etcdctl put /streams/outdoor-weather/enabled "false"
etcdctl put /streams/outdoor-air-quality/enabled "false"

# Or delete entirely
etcdctl del /streams/outdoor-weather --prefix
etcdctl del /streams/outdoor-air-quality --prefix

# Restart to apply
docker compose restart air-quality-app
```

---

## Document Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2025-12-16 | SPARC Agent | Initial architecture |
| 1.1.0 | 2025-12-16 | SPARC Agent | Updated based on codebase analysis; added generic HTTP polling design with ResponseParser trait, AuthMethod, RetryConfig; removed AirGradient-specific references |

---

## References

- [AIR-005 Specification](../specification/SPECIFICATION.md)
- [AIR-005 Pseudocode](../pseudocode/PSEUDOCODE.md)
- [Existing HttpPollingSource](../../../../core/src/sources/http_poll.rs)
- [OpenWeatherMap Current Weather API](https://openweathermap.org/current)
- [OpenWeatherMap Air Pollution API](https://openweathermap.org/api/air-pollution)
