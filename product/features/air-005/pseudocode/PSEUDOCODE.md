# AIR-005: Weather Data Integration - Pseudocode Document

**Version**: 1.1.0
**Last Updated**: 2025-12-16
**Status**: Design Phase
**SPARC Phase**: Pseudocode

---

## Table of Contents

1. [Overview](#overview)
2. [Core Abstractions](#core-abstractions)
3. [ResponseParser Trait](#responseparser-trait)
4. [Authentication Module](#authentication-module)
5. [Retry Logic](#retry-logic)
6. [HTTP Polling Source (Refactored)](#http-polling-source-refactored)
7. [Parser Implementations](#parser-implementations)
8. [Configuration Loading](#configuration-loading)
9. [Integration with Application](#integration-with-application)
10. [Complexity Analysis](#complexity-analysis)

---

## 1. Overview

This document provides pseudocode for refactoring `HttpPollingSource` from a hardcoded implementation to a generic, configuration-driven HTTP polling system. The key abstractions are:

1. **ResponseParser trait** - Pluggable parsing for different API formats
2. **AuthMethod enum** - Flexible authentication (query param, header, basic auth)
3. **RetryHandler** - Exponential backoff with jitter and error classification
4. **EndpointConfig** - Generic endpoint configuration replacing `SensorConfig`

---

## 2. Core Abstractions

### 2.1 Data Structures

```
STRUCT EndpointConfig:
    id: String                          # Unique endpoint identifier
    url: String                         # Base URL to poll
    auth: AuthMethod                    # Authentication configuration
    parser_type: String                 # Parser name (lookup key)
    enabled: Boolean                    # Enable/disable flag
    query_params: Map<String, String>   # URL query parameters

STRUCT HttpPollingConfig:
    poll_interval_secs: u64             # Seconds between polls
    timeout_secs: u64                   # HTTP request timeout
    buffer_capacity: usize              # Channel buffer size
    retry: RetryConfig                  # Retry settings
    endpoints: List<EndpointConfig>     # Endpoints to poll

STRUCT RetryConfig:
    max_retries: u32                    # Maximum retry attempts
    initial_delay_ms: u64               # First retry delay
    max_delay_ms: u64                   # Maximum delay cap
    backoff_multiplier: f64             # Exponential multiplier
    jitter: Boolean                     # Enable random jitter

ENUM ErrorType:
    Transient                           # Retry with backoff
    RateLimited { retry_after: u64 }    # Wait specific time
    Permanent                           # Don't retry

ENUM AuthMethod:
    None
    QueryParam { param_name, value_env }
    Header { header_name, value_env }
    BasicAuth { username, password_env }
```

---

## 3. ResponseParser Trait

### 3.1 Trait Definition

```
TRAIT ResponseParser:
    # Parse HTTP response body into TimeSeriesPoints
    FUNCTION parse(
        response_body: String,
        location_id: String,
        timestamp: DateTime<Utc>
    ) -> Result<List<TimeSeriesPoint>, Error>

    # Return parser name for logging/debugging
    FUNCTION name() -> String
```

### 3.2 Parser Registry

```
STRUCT ParserRegistry:
    parsers: Map<String, Arc<dyn ResponseParser>>

FUNCTION ParserRegistry::new() -> ParserRegistry:
    registry = empty map

    # Register built-in parsers
    registry.insert("openweather_current", WeatherParser)
    registry.insert("openweather_air_pollution", AirPollutionParser)

    RETURN ParserRegistry { parsers: registry }

FUNCTION ParserRegistry::get(parser_type: String) -> Option<ResponseParser>:
    RETURN parsers.get(parser_type).cloned()

FUNCTION ParserRegistry::register(name: String, parser: ResponseParser):
    parsers.insert(name, parser)
```

---

## 4. Authentication Module

### 4.1 Apply Authentication

```
FUNCTION apply_auth(request: RequestBuilder, auth: AuthMethod) -> RequestBuilder:
    MATCH auth:
        AuthMethod::None:
            RETURN request

        AuthMethod::QueryParam { param_name, value_env }:
            api_key = env::var(value_env)?
            RETURN request.query(&[(param_name, api_key)])

        AuthMethod::Header { header_name, value_env }:
            api_key = env::var(value_env)?
            RETURN request.header(header_name, api_key)

        AuthMethod::BasicAuth { username, password_env }:
            password = env::var(password_env)?
            RETURN request.basic_auth(username, Some(password))
```

### 4.2 Build Request with Query Params

```
FUNCTION build_request(
    client: HttpClient,
    endpoint: EndpointConfig
) -> Result<RequestBuilder, Error>:

    request = client.get(endpoint.url)

    # Apply authentication
    request = apply_auth(request, endpoint.auth)

    # Apply query parameters
    IF endpoint.query_params IS NOT empty:
        FOR (key, value) IN endpoint.query_params:
            # Expand environment variables in values
            expanded_value = expand_env_vars(value)
            request = request.query(&[(key, expanded_value)])

    RETURN request
```

---

## 5. Retry Logic

### 5.1 Error Classification

```
FUNCTION classify_error(status: Option<u16>, error: Error) -> ErrorType:
    MATCH status:
        Some(429):
            # Parse Retry-After header if available
            retry_after = parse_retry_after(error) OR 60
            RETURN ErrorType::RateLimited { retry_after }

        Some(401), Some(403):
            RETURN ErrorType::Permanent

        Some(404):
            RETURN ErrorType::Permanent

        Some(s) WHERE s >= 500:
            RETURN ErrorType::Transient

        Some(s) WHERE s >= 400:
            RETURN ErrorType::Permanent

        None:
            # Network error
            RETURN ErrorType::Transient

        _:
            RETURN ErrorType::Transient
```

### 5.2 Calculate Backoff Delay

```
FUNCTION calculate_delay(
    attempt: u32,
    config: RetryConfig
) -> Duration:

    # Exponential backoff: initial * multiplier^attempt
    delay_ms = config.initial_delay_ms * (config.backoff_multiplier ^ attempt)

    # Cap at maximum
    delay_ms = min(delay_ms, config.max_delay_ms)

    # Add jitter (0-10%) if enabled
    IF config.jitter:
        jitter = random(0.0, 0.1)
        delay_ms = delay_ms * (1.0 + jitter)

    RETURN Duration::from_millis(delay_ms as u64)
```

### 5.3 Execute with Retry

```
FUNCTION poll_with_retry(
    source: HttpPollingSource,
    endpoint: EndpointConfig
) -> Result<List<TimeSeriesPoint>, Error>:

    FOR attempt IN 0..=source.config.retry.max_retries:
        TRY:
            points = source.poll_endpoint(endpoint)
            RETURN Ok(points)

        CATCH error:
            status = extract_status_code(error)
            error_type = classify_error(status, error)

            MATCH error_type:
                ErrorType::Permanent:
                    log_error("Permanent error for {}: {}", endpoint.id, error)
                    RETURN Err(error)

                ErrorType::RateLimited { retry_after }:
                    IF attempt == source.config.retry.max_retries:
                        RETURN Err(error)
                    log_warn("Rate limited, waiting {}s", retry_after)
                    sleep(Duration::from_secs(retry_after))

                ErrorType::Transient:
                    IF attempt == source.config.retry.max_retries:
                        RETURN Err(error)
                    delay = calculate_delay(attempt, source.config.retry)
                    log_warn("Transient error, retry {} in {:?}", attempt + 1, delay)
                    sleep(delay)

    UNREACHABLE
```

---

## 6. HTTP Polling Source (Refactored)

### 6.1 Structure Definition

```
STRUCT HttpPollingSource:
    config: HttpPollingConfig
    client: HttpClient
    parser_registry: ParserRegistry
    receiver: Arc<Mutex<Receiver<TimeSeriesPoint>>>
    sender: Sender<TimeSeriesPoint>
    is_running: Arc<Mutex<Boolean>>
    last_successful_poll: Arc<Mutex<Map<String, DateTime>>>
    consecutive_errors: Arc<Mutex<Map<String, u32>>>
```

### 6.2 Constructor

```
FUNCTION HttpPollingSource::new(config: HttpPollingConfig) -> Result<Self, Error>:
    (sender, receiver) = channel(config.buffer_capacity)

    client = HttpClient::builder()
        .timeout(Duration::from_secs(config.timeout_secs))
        .https_only(true)
        .build()?

    parser_registry = ParserRegistry::new()

    RETURN HttpPollingSource {
        config,
        client,
        parser_registry,
        receiver: Arc::new(Mutex::new(receiver)),
        sender,
        is_running: Arc::new(Mutex::new(false)),
        last_successful_poll: Arc::new(Mutex::new(empty_map)),
        consecutive_errors: Arc::new(Mutex::new(empty_map)),
    }
```

### 6.3 Poll Single Endpoint

```
FUNCTION HttpPollingSource::poll_endpoint(
    endpoint: EndpointConfig
) -> Result<List<TimeSeriesPoint>, Error>:

    log_debug("Polling endpoint: {}", endpoint.id)

    # Build request with auth and query params
    request = build_request(self.client, endpoint)?

    # Execute request
    response = request.send().await?

    # Check for HTTP errors
    IF NOT response.status().is_success():
        RETURN Err(Error::Http(response.status()))

    # Get response body
    body = response.text().await?

    # Get parser for this endpoint
    parser = self.parser_registry.get(endpoint.parser_type)
        .ok_or(Error::UnknownParser(endpoint.parser_type))?

    # Parse response
    timestamp = Utc::now()
    location_id = endpoint.id.clone()

    points = parser.parse(body, location_id, timestamp)?

    log_debug("Parsed {} points from endpoint {}", points.len(), endpoint.id)

    RETURN Ok(points)
```

### 6.4 Poll All Endpoints

```
FUNCTION HttpPollingSource::poll_all_endpoints() -> Result<(), Error>:
    FOR endpoint IN self.config.endpoints:
        IF NOT endpoint.enabled:
            CONTINUE

        MATCH poll_with_retry(self, endpoint):
            Ok(points):
                # Update success tracking
                last_poll = self.last_successful_poll.lock()
                last_poll.insert(endpoint.id, Utc::now())

                errors = self.consecutive_errors.lock()
                errors.insert(endpoint.id, 0)

                # Send points to channel
                FOR point IN points:
                    IF self.sender.send(point).await.is_err():
                        log_warn("Channel full, dropping point")

            Err(error):
                # Track consecutive errors
                errors = self.consecutive_errors.lock()
                count = errors.get(endpoint.id).unwrap_or(0) + 1
                errors.insert(endpoint.id, count)

                log_error("Failed to poll {}: {} (consecutive: {})",
                    endpoint.id, error, count)

    RETURN Ok(())
```

### 6.5 Polling Loop

```
FUNCTION HttpPollingSource::polling_loop():
    interval = tokio::interval(Duration::from_secs(self.config.poll_interval_secs))

    WHILE self.is_running.lock().await == true:
        interval.tick().await

        IF let Err(e) = self.poll_all_endpoints().await:
            log_error("Polling cycle error: {}", e)

    log_info("Polling loop stopped")
```

### 6.6 Start and Stop

```
FUNCTION HttpPollingSource::start() -> Result<(), Error>:
    log_info("Starting HTTP polling source with {} endpoints",
        self.config.endpoints.len())

    enabled_count = self.config.endpoints.filter(|e| e.enabled).count()
    IF enabled_count == 0:
        RETURN Err(Error::NoEndpointsEnabled)

    self.is_running.lock().await = true

    # Clone for background task
    source_clone = self.clone()

    # Spawn background polling task
    tokio::spawn(async move {
        source_clone.polling_loop().await
    })

    # Initial poll
    self.poll_all_endpoints().await?

    log_info("HTTP polling source started, polling {} endpoints every {}s",
        enabled_count, self.config.poll_interval_secs)

    RETURN Ok(())

FUNCTION HttpPollingSource::stop() -> Result<(), Error>:
    log_info("Stopping HTTP polling source")
    self.is_running.lock().await = false
    RETURN Ok(())
```

### 6.7 Health Check

```
FUNCTION HttpPollingSource::health_check() -> Result<HealthStatus, Error>:
    is_running = self.is_running.lock().await

    details = empty_map
    details.insert("source_type", "http_polling")
    details.insert("is_running", is_running.to_string())

    IF NOT is_running:
        RETURN HealthStatus {
            healthy: false,
            message: "HTTP polling source not running",
            details
        }

    now = Utc::now()
    last_poll = self.last_successful_poll.lock().await
    errors = self.consecutive_errors.lock().await

    max_age = Duration::from_secs(self.config.poll_interval_secs * 2)

    unhealthy_endpoints = []
    degraded_endpoints = []

    FOR endpoint IN self.config.endpoints:
        IF NOT endpoint.enabled:
            CONTINUE

        # Check last successful poll time
        MATCH last_poll.get(endpoint.id):
            None:
                unhealthy_endpoints.push(endpoint.id)
            Some(time) WHERE (now - time) > max_age:
                degraded_endpoints.push(endpoint.id)
            _:
                # Healthy
                PASS

        # Check consecutive errors
        IF errors.get(endpoint.id).unwrap_or(0) >= 3:
            IF endpoint.id NOT IN degraded_endpoints:
                degraded_endpoints.push(endpoint.id)

    IF unhealthy_endpoints.is_empty() AND degraded_endpoints.is_empty():
        RETURN HealthStatus {
            healthy: true,
            message: "All endpoints operational",
            details
        }
    ELSE IF unhealthy_endpoints.len() == self.config.endpoints.len():
        details.insert("unhealthy", unhealthy_endpoints.join(","))
        RETURN HealthStatus {
            healthy: false,
            message: format!("All endpoints unhealthy: {:?}", unhealthy_endpoints),
            details
        }
    ELSE:
        details.insert("degraded", degraded_endpoints.join(","))
        details.insert("unhealthy", unhealthy_endpoints.join(","))
        RETURN HealthStatus {
            healthy: false,
            message: format!("Some endpoints degraded: {:?}", degraded_endpoints),
            details
        }
```

---

## 7. Parser Implementations

### 7.1 OpenWeatherMap Current Weather Parser

```
STRUCT WeatherParser

IMPLEMENT ResponseParser FOR WeatherParser:
    FUNCTION name() -> String:
        RETURN "openweather_current"

    FUNCTION parse(
        body: String,
        location_id: String,
        timestamp: DateTime
    ) -> Result<List<TimeSeriesPoint>, Error>:

        data = parse_json(body) as OpenWeatherResponse
        points = []

        # Helper to create point
        FUNCTION add_point(metric: String, value: f64):
            tags = {
                "metric": metric,
                "source": "openweather",
                "stream": "outdoor-weather"
            }
            points.push(TimeSeriesPoint {
                timestamp,
                location_id: location_id.clone(),
                value,
                tags
            })

        # Main weather data
        add_point("temperature", data.main.temp)
        add_point("feels_like", data.main.feels_like)
        add_point("pressure", data.main.pressure as f64)
        add_point("humidity", data.main.humidity as f64)

        # Optional min/max
        IF data.main.temp_min IS SOME:
            add_point("temp_min", data.main.temp_min)
        IF data.main.temp_max IS SOME:
            add_point("temp_max", data.main.temp_max)

        # Wind data
        add_point("wind_speed", data.wind.speed)
        IF data.wind.deg IS SOME:
            add_point("wind_deg", data.wind.deg as f64)
        IF data.wind.gust IS SOME:
            add_point("wind_gust", data.wind.gust)

        # Clouds
        add_point("clouds", data.clouds.all as f64)

        # Visibility (optional)
        IF data.visibility IS SOME:
            add_point("visibility", data.visibility as f64)

        # Rain (optional)
        IF data.rain IS SOME:
            IF data.rain["1h"] IS SOME:
                add_point("rain_1h", data.rain["1h"])
            IF data.rain["3h"] IS SOME:
                add_point("rain_3h", data.rain["3h"])

        # Snow (optional)
        IF data.snow IS SOME:
            IF data.snow["1h"] IS SOME:
                add_point("snow_1h", data.snow["1h"])
            IF data.snow["3h"] IS SOME:
                add_point("snow_3h", data.snow["3h"])

        RETURN Ok(points)
```

### 7.2 OpenWeatherMap Response Types

```
STRUCT OpenWeatherResponse:
    main: MainData
    wind: WindData
    clouds: CloudData
    rain: Option<PrecipData>
    snow: Option<PrecipData>
    visibility: Option<i32>
    dt: i64                     # Unix timestamp
    timezone: i32               # Timezone offset

STRUCT MainData:
    temp: f64
    feels_like: f64
    temp_min: Option<f64>
    temp_max: Option<f64>
    pressure: i32
    humidity: i32

STRUCT WindData:
    speed: f64
    deg: Option<i32>
    gust: Option<f64>

STRUCT CloudData:
    all: i32                    # Cloudiness %

STRUCT PrecipData:
    "1h": Option<f64>
    "3h": Option<f64>
```

### 7.3 OpenWeatherMap Air Pollution Parser

```
STRUCT AirPollutionParser

IMPLEMENT ResponseParser FOR AirPollutionParser:
    FUNCTION name() -> String:
        RETURN "openweather_air_pollution"

    FUNCTION parse(
        body: String,
        location_id: String,
        timestamp: DateTime
    ) -> Result<List<TimeSeriesPoint>, Error>:

        data = parse_json(body) as AirPollutionResponse

        IF data.list.is_empty():
            RETURN Err(Error::EmptyResponse)

        reading = data.list[0]
        points = []

        # Helper to create point
        FUNCTION add_point(metric: String, value: f64):
            tags = {
                "metric": metric,
                "source": "openweather",
                "stream": "outdoor-air-quality"
            }
            points.push(TimeSeriesPoint {
                timestamp,
                location_id: location_id.clone(),
                value,
                tags
            })

        # Air Quality Index (1-5 scale)
        add_point("aqi", reading.main.aqi as f64)

        # Pollutant concentrations (μg/m³)
        components = reading.components
        add_point("co", components.co)
        add_point("no", components.no)
        add_point("no2", components.no2)
        add_point("o3", components.o3)
        add_point("so2", components.so2)
        add_point("pm2_5", components.pm2_5)
        add_point("pm10", components.pm10)
        add_point("nh3", components.nh3)

        RETURN Ok(points)
```

### 7.4 Air Pollution Response Types

```
STRUCT AirPollutionResponse:
    list: List<AirPollutionReading>

STRUCT AirPollutionReading:
    main: AqiData
    components: PollutantData
    dt: i64                     # Unix timestamp

STRUCT AqiData:
    aqi: i32                    # 1-5 scale

STRUCT PollutantData:
    co: f64                     # Carbon monoxide
    no: f64                     # Nitrogen monoxide
    no2: f64                    # Nitrogen dioxide
    o3: f64                     # Ozone
    so2: f64                    # Sulphur dioxide
    pm2_5: f64                  # Fine particles
    pm10: f64                   # Coarse particles
    nh3: f64                    # Ammonia
```

---

## 8. Configuration Loading

### 8.1 Load from StreamConfig

```
FUNCTION load_http_config_from_stream(
    stream_config: StreamConfig
) -> Result<EndpointConfig, Error>:

    source = stream_config.sources
        .find(|s| s.type == "http_poll")
        .ok_or(Error::NoHttpSource)?

    # Build endpoint config
    endpoint = EndpointConfig {
        id: stream_config.stream_id,
        url: source.endpoint.url,
        auth: load_auth_method(source.endpoint.auth),
        parser_type: source.endpoint.parser_type,
        enabled: source.enabled AND stream_config.enabled,
        query_params: expand_query_params(source.endpoint.query_params),
    }

    RETURN Ok(endpoint)

FUNCTION load_auth_method(auth_config: AuthConfig) -> AuthMethod:
    MATCH auth_config.type:
        "none":
            RETURN AuthMethod::None

        "query_param":
            # Load API key from environment
            api_key = env::var(auth_config.value_env)?
            RETURN AuthMethod::QueryParam {
                param_name: auth_config.param_name,
                value: api_key,
            }

        "header":
            api_key = env::var(auth_config.value_env)?
            RETURN AuthMethod::Header {
                header_name: auth_config.header_name,
                value: api_key,
            }

        "basic":
            password = env::var(auth_config.password_env)?
            RETURN AuthMethod::BasicAuth {
                username: auth_config.username,
                password,
            }

FUNCTION expand_query_params(
    params: Map<String, String>
) -> Map<String, String>:

    result = empty_map

    FOR (key, value) IN params:
        # Expand ${VAR_NAME} patterns
        expanded = regex_replace(value, r"\$\{(\w+)\}", |m| {
            env::var(m.group(1)).unwrap_or("")
        })
        result.insert(key, expanded)

    RETURN result
```

### 8.2 Build HttpPollingConfig from Multiple Streams

```
FUNCTION build_polling_config(
    streams: List<StreamConfig>,
    global_config: WeatherConfig
) -> Result<HttpPollingConfig, Error>:

    endpoints = []

    FOR stream IN streams:
        # Only process streams with http_poll sources
        IF stream.has_http_poll_source():
            endpoint = load_http_config_from_stream(stream)?
            endpoints.push(endpoint)

    IF endpoints.is_empty():
        RETURN Err(Error::NoEndpoints)

    # Use first stream's config for global settings, or global defaults
    first_source = streams[0].get_http_poll_source()

    RETURN HttpPollingConfig {
        poll_interval_secs: first_source.poll_interval_secs OR global_config.poll_interval_secs,
        timeout_secs: first_source.timeout_secs OR 30,
        buffer_capacity: first_source.storage.buffer_capacity OR 1000,
        retry: first_source.retry OR RetryConfig::default(),
        endpoints,
    }
```

---

## 9. Integration with Application

### 9.1 Application Startup

```
FUNCTION start_weather_polling(
    stream_registry: StreamRegistry,
    tx: Sender<TimeSeriesPoint>
) -> Result<HttpPollingSource, Error>:

    # Load weather-related streams from registry
    weather_streams = stream_registry.list_streams()
        .filter(|s| s.has_http_poll_source())

    IF weather_streams.is_empty():
        log_info("No HTTP polling streams configured")
        RETURN Err(Error::NoStreams)

    # Build unified config
    config = build_polling_config(weather_streams, global_config)?

    # Create source
    source = HttpPollingSource::new(config)?

    # Replace internal sender with application sender
    source.sender = tx

    # Start polling
    source.start().await?

    RETURN Ok(source)
```

### 9.2 Main Application Integration

```
FUNCTION main():
    # ... existing setup ...

    # Create shared channel
    (tx, rx) = channel(1000)

    # Start MQTT source (existing - for indoor air quality)
    mqtt_source = MqttSource::new(mqtt_config)?
    mqtt_source.start(tx.clone()).await?

    # Start HTTP polling source (new - for outdoor weather)
    TRY:
        http_source = start_weather_polling(stream_registry, tx.clone()).await
        log_info("Weather polling started")
    CATCH e:
        log_warn("Weather polling not started: {}", e)
        # Not fatal - continue without weather data

    # Start storage writer (receives from all sources)
    storage_writer = StorageWriter::new(rx, parquet_store)
    storage_writer.start().await?

    # ... rest of application ...
```

### 9.3 Graceful Shutdown

```
FUNCTION shutdown():
    log_info("Shutting down...")

    # Stop HTTP polling
    IF http_source IS SOME:
        http_source.stop().await?

    # Stop MQTT
    mqtt_source.stop().await?

    # Flush storage
    storage_writer.flush().await?

    log_info("Shutdown complete")
```

---

## 10. Complexity Analysis

### 10.1 Time Complexity

```
HttpPollingSource::new():
    Time: O(1)
    - Configuration validation: O(1)
    - HTTP client creation: O(1)
    - Parser registry initialization: O(p) where p = registered parsers

HttpPollingSource::poll_endpoint():
    Time: O(m) where m = metrics in response
    - HTTP request: O(1) network operation
    - JSON parsing: O(r) where r = response size
    - Parser execution: O(m)
    Total: O(m)

poll_with_retry():
    Time: O(r * m) worst case
    - r = max_retries
    - m = metrics per endpoint
    - Backoff delays not counted (wall time)

poll_all_endpoints():
    Time: O(e * r * m)
    - e = enabled endpoints
    - r = max_retries per endpoint
    - m = metrics per endpoint

health_check():
    Time: O(e) where e = endpoints
    - Lock acquisitions: O(1) amortized
    - Endpoint iteration: O(e)
```

### 10.2 Space Complexity

```
HttpPollingSource:
    Space: O(c + p + e)
    - c = buffer_capacity (channel)
    - p = registered parsers
    - e = endpoint configurations

Per Endpoint Poll:
    Space: O(m) where m = metrics
    - Response buffer: O(r) response size
    - Parsed points: O(m)
    - Tags per point: O(1) constant

Maximum Memory Estimate:
    buffer_capacity = 1000
    TimeSeriesPoint ≈ 200 bytes
    Channel buffer: ~200 KB
    Parser registry: ~1 KB
    Endpoint configs: ~2 KB
    Total: ~205 KB per HttpPollingSource
```

### 10.3 Network Complexity

```
Requests per poll cycle:
    2 endpoints × 1 request each = 2 requests

Bandwidth per poll:
    Weather response: ~500 bytes
    Air pollution response: ~300 bytes
    Total: ~800 bytes per cycle

With 10-minute polling:
    Hourly: 6 cycles × 800 bytes = 4.8 KB
    Daily: 144 cycles × 800 bytes = 115 KB
    Monthly: ~3.4 MB

API Rate Limits (OpenWeatherMap Free Tier):
    Limit: 1,000 calls/day
    Usage: 288 calls/day (28.8% utilization)
```

---

## Summary

This pseudocode provides algorithms for a **generic HTTP polling system** with:

1. **ResponseParser trait** - Pluggable parsing for any JSON API
2. **ParserRegistry** - Dynamic parser lookup by name
3. **AuthMethod** - Flexible authentication (query param, header, basic)
4. **RetryHandler** - Exponential backoff with error classification
5. **EndpointConfig** - Generic endpoint configuration

**Key Design Decisions:**

- **Trait-based parsing**: Easily add new API integrations
- **Configuration-driven**: All settings from etcd/environment
- **Error classification**: Smart retry behavior for different failure modes
- **Health monitoring**: Per-endpoint tracking with degraded states
- **Thread safety**: Arc<Mutex> for async task coordination

---

## Document Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2025-12-16 | SPARC Agent | Initial pseudocode |
| 1.1.0 | 2025-12-16 | SPARC Agent | Refactored for generic HTTP polling; added ResponseParser trait, AuthMethod, RetryHandler; removed hardcoded references |

---

## References

- [AIR-005 Architecture](../architecture/ARCHITECTURE.md)
- [AIR-005 Refinement](../refinement/REFINEMENT.md)
- [Existing HttpPollingSource](../../../../core/src/sources/http_poll.rs)
