# HttpPollingSource Refactoring - Generic and Configuration-Driven

**Date**: 2025-12-16
**Component**: `core/src/sources/http_poll.rs`
**Status**: ✅ IMPLEMENTATION COMPLETE
**Implementation Date**: 2025-12-16

---

## Overview

The HttpPollingSource has been refactored from a sensor-specific implementation to a **generic, configuration-driven HTTP polling system** that can work with any HTTP endpoint and response format.

**IMPLEMENTATION STATUS**: ✅ COMPLETE

All components described in this document have been fully implemented and tested:
- Generic HTTP polling with configurable endpoints
- Pluggable parser system via ResponseParser trait
- Flexible authentication (None, QueryParam, Header, Bearer)
- Retry logic with exponential backoff and jitter
- Two production parsers for OpenWeatherMap APIs
- Stream configurations for outdoor weather and air quality data

---

## Key Changes

### 1. ResponseParser Trait ✅ IMPLEMENTED

Added a generic trait for parsing HTTP responses:

**Location**: `core/src/sources/http_poll.rs` (lines 100-120)

```rust
pub trait ResponseParser: Send + Sync + 'static {
    fn parse(
        &self,
        response_body: &str,
        location_id: &str,
        timestamp: DateTime<Utc>,
    ) -> CoreResult<Vec<TimeSeriesPoint>>;

    fn name(&self) -> &'static str;
}
```

**Purpose**: Allows different data sources (weather APIs, air quality sensors, IoT devices) to provide custom parsing logic.

---

### 2. Authentication Methods ✅ IMPLEMENTED

Added flexible authentication support:

**Location**: `core/src/sources/http_poll.rs` (lines 25-42)

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthMethod {
    None,
    QueryParam { param_name: String, value_env: String },
    Header { header_name: String, value_env: String },
    BasicAuth { username: String, password_env: String },
}
```

**Benefits**:
- Secure (credentials from environment variables)
- Supports multiple auth patterns
- Easy to extend

---

### 3. Retry Configuration with Exponential Backoff ✅ IMPLEMENTED

Added robust retry logic:

**Location**: `core/src/sources/http_poll.rs` (lines 44-88)

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub backoff_multiplier: f64,
    pub jitter: bool,
}
```

**Features**:
- Exponential backoff (e.g., 100ms → 200ms → 400ms)
- Jitter to prevent thundering herd
- Error classification (Transient, RateLimited, Permanent)
- Only retries on transient errors

---

### 4. Endpoint Configuration ✅ IMPLEMENTED

Replaced sensor-specific config with generic endpoints:

**Location**: `core/src/sources/http_poll.rs` (lines 120-160)

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct EndpointConfig {
    pub id: String,
    pub url: String,
    pub auth: AuthMethod,
    pub parser_type: String,           // Registry lookup
    pub enabled: bool,
    pub query_params: Option<HashMap<String, String>>,
}
```

---

### 5. ParserRegistry ✅ IMPLEMENTED

Added a plugin-style parser registry:

**Location**: `core/src/sources/http_poll.rs` (lines 200-240)

```rust
pub struct ParserRegistry {
    parsers: HashMap<String, Arc<dyn ResponseParser>>,
}

impl ParserRegistry {
    pub fn new() -> Self;
    pub fn register(&mut self, parser_type: String, parser: Arc<dyn ResponseParser>);
    pub fn get(&self, parser_type: &str) -> Option<Arc<dyn ResponseParser>>;
}
```

**Usage**:
```rust
let mut registry = ParserRegistry::new();
registry.register("airgradient".to_string(), Arc::new(AirGradientParser));
registry.register("openweather".to_string(), Arc::new(OpenWeatherParser));

let source = HttpPollingSource::new(config, registry)?;
```

---

## API Changes

### Old API (Deprecated)
```rust
// Sensor-specific
let config = HttpPollingConfig {
    base_url_template: "http://airgradient_{SERIAL}.local/measures/current".to_string(),
    sensors: vec![SensorConfig { serial_number, url }],
    ..Default::default()
};

let source = HttpPollingSource::new(config)?;
```

### New API
```rust
// Generic endpoints
let config = HttpPollingConfig {
    endpoints: vec![
        EndpointConfig {
            id: "weather_boston".to_string(),
            url: "https://api.openweathermap.org/data/2.5/weather".to_string(),
            auth: AuthMethod::QueryParam {
                param_name: "appid".to_string(),
                value_env: "OPENWEATHER_API_KEY".to_string(),
            },
            parser_type: "openweather".to_string(),
            enabled: true,
            query_params: Some(HashMap::from([
                ("q".to_string(), "Boston,US".to_string()),
                ("units".to_string(), "metric".to_string()),
            ])),
        },
    ],
    poll_interval: Duration::from_secs(300), // 5 minutes
    retry: RetryConfig::default(),
    ..Default::default()
};

let mut registry = ParserRegistry::new();
registry.register("openweather".to_string(), Arc::new(OpenWeatherParser));

let source = HttpPollingSource::new(config, registry)?;
```

---

## Backward Compatibility

The old API is still supported through:

1. **Legacy fields** in HttpPollingConfig:
   ```rust
   pub struct HttpPollingConfig {
       pub sensors: Vec<SensorConfig>,  // DEPRECATED but still works
       pub endpoints: Vec<EndpointConfig>,  // NEW
       ...
   }
   ```

2. **Default parser constructor**:
   ```rust
   let source = HttpPollingSource::new_with_default_parser(config)?;
   // Automatically registers AirGradientParser
   ```

3. **Dual polling logic**: `poll_all_endpoints()` handles both endpoints and legacy sensors.

---

## Implementation Examples

### Example 1: Weather API Parser ✅ IMPLEMENTED

**Production Implementation**: `core/src/sources/parsers/weather.rs`

This parser is fully implemented and tested with comprehensive unit tests. It parses OpenWeatherMap Current Weather API responses and extracts:
- Temperature and feels_like temperature
- Atmospheric pressure and humidity
- Wind speed, direction, and gusts
- Cloud coverage and visibility
- Precipitation (rain and snow)

```rust
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
struct OpenWeatherResponse {
    main: MainWeather,
    weather: Vec<Weather>,
    wind: Wind,
}

#[derive(Deserialize)]
struct MainWeather {
    temp: f64,
    humidity: f64,
    pressure: f64,
}

#[derive(Deserialize)]
struct Weather {
    description: String,
}

#[derive(Deserialize)]
struct Wind {
    speed: f64,
}

struct OpenWeatherParser;

impl ResponseParser for OpenWeatherParser {
    fn parse(
        &self,
        response_body: &str,
        location_id: &str,
        timestamp: DateTime<Utc>,
    ) -> CoreResult<Vec<TimeSeriesPoint>> {
        let data: OpenWeatherResponse = serde_json::from_str(response_body)
            .map_err(|e| CoreError::Source(format!("Failed to parse: {}", e)))?;

        let mut points = Vec::new();

        // Temperature
        let mut tags = HashMap::new();
        tags.insert("metric".to_string(), "temperature".to_string());
        tags.insert("source".to_string(), "openweather".to_string());
        points.push(TimeSeriesPoint {
            timestamp,
            location_id: location_id.to_string(),
            value: data.main.temp,
            tags,
        });

        // Humidity
        let mut tags = HashMap::new();
        tags.insert("metric".to_string(), "humidity".to_string());
        tags.insert("source".to_string(), "openweather".to_string());
        points.push(TimeSeriesPoint {
            timestamp,
            location_id: location_id.to_string(),
            value: data.main.humidity,
            tags,
        });

        Ok(points)
    }

    fn name(&self) -> &'static str {
        "openweather"
    }
}
```

### Example 2: Multi-Endpoint Configuration

```rust
let config = HttpPollingConfig {
    endpoints: vec![
        // Weather data
        EndpointConfig {
            id: "weather_boston".to_string(),
            url: "https://api.openweathermap.org/data/2.5/weather".to_string(),
            auth: AuthMethod::QueryParam {
                param_name: "appid".to_string(),
                value_env: "OPENWEATHER_API_KEY".to_string(),
            },
            parser_type: "openweather".to_string(),
            enabled: true,
            query_params: Some(HashMap::from([
                ("q".to_string(), "Boston,US".to_string()),
            ])),
        },
        // Air quality sensor
        EndpointConfig {
            id: "sensor_001".to_string(),
            url: "http://airgradient_001.local/measures/current".to_string(),
            auth: AuthMethod::None,
            parser_type: "airgradient".to_string(),
            enabled: true,
            query_params: None,
        },
    ],
    poll_interval: Duration::from_secs(60),
    timeout: Duration::from_secs(10),
    retry: RetryConfig {
        max_retries: 3,
        initial_delay_ms: 100,
        max_delay_ms: 30000,
        backoff_multiplier: 2.0,
        jitter: true,
    },
    ..Default::default()
};
```

---

## Benefits

1. **Extensibility**: Add new data sources without modifying core code
2. **Configurability**: All behavior controlled through configuration
3. **Reliability**: Built-in retry logic with exponential backoff
4. **Security**: Credentials stored in environment variables
5. **Testability**: Easy to mock parsers for unit tests
6. **Backward Compatible**: Existing code continues to work

---

## Migration Path

### For Existing AirGradient Users
**No changes required!** The old API still works:

```rust
let config = HttpPollingConfig {
    sensors: vec![SensorConfig { serial_number, url }],
    ..Default::default()
};
let source = HttpPollingSource::new_with_default_parser(config)?;
```

### For New Users or Migration
1. Convert sensors to endpoints
2. Register appropriate parsers
3. Configure authentication
4. Set retry policy

---

## Testing

All existing tests pass with backward compatibility. New tests added:

- `test_parser_registry()` - Parser registration and lookup
- `test_endpoint_config_with_auth()` - Endpoint configuration
- `test_retry_config_default()` - Retry configuration

---

## Implemented Parsers

### WeatherParser ✅ COMPLETE
**Location**: `core/src/sources/parsers/weather.rs`
**API**: OpenWeatherMap Current Weather API
**Metrics**: temperature, feels_like, pressure, humidity, wind_speed, wind_deg, wind_gust, clouds, visibility, rain_1h, snow_1h
**Tests**: Comprehensive unit tests (lines 204-305)

### AirPollutionParser ✅ COMPLETE
**Location**: `core/src/sources/parsers/air_pollution.rs`
**API**: OpenWeatherMap Air Pollution API
**Metrics**: aqi, co, no, no2, o3, so2, pm2_5, pm10, nh3
**Tests**: Comprehensive unit tests (lines 191-305)

## Stream Configurations

### outdoor-weather.yaml ✅ COMPLETE
**Location**: `config/streams/outdoor-weather.yaml`
**Description**: Outdoor weather data from OpenWeatherMap
**Fields**: 11 weather metrics with ranges and units
**Poll Interval**: 600 seconds (10 minutes, respects free tier limits)
**Storage**: Parquet format, daily partitioning, 90-day retention

### outdoor-air-quality.yaml ✅ COMPLETE
**Location**: `config/streams/outdoor-air-quality.yaml`
**Description**: Outdoor air quality data from OpenWeatherMap
**Fields**: 9 air quality metrics (AQI, pollutants)
**Poll Interval**: 600 seconds (10 minutes, respects free tier limits)
**Storage**: Parquet format, daily partitioning, 90-day retention

## Future Enhancements

1. **Webhook support**: Push-based data ingestion
2. **GraphQL endpoints**: Support for GraphQL queries
3. **Streaming responses**: Handle chunked/streaming HTTP responses
4. **Circuit breaker**: Advanced failure detection
5. **Metrics**: Prometheus metrics for endpoint health
6. **Rate limiting**: Respect API rate limits
7. **Additional parsers**: AccuWeather, Weather Underground, PurpleAir

---

## Dependencies Added

- `rand = "0.8"` - For jitter in retry delays

---

## References

- [AIR-005 Design Document](./AIR-005_INGESTION_COORDINATOR_DESIGN.md)
- [Source Trait](../../core/src/traits.rs)
- [Error Handling](../../core/src/error.rs)
