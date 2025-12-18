# BUG-002: Config-Driven Parsing - Pseudocode Phase

**Status**: Pseudocode Design
**Created**: 2025-12-18
**Phase**: SPARC - Pseudocode (P)
**Related**: BUG-001 (Specification)

---

## Overview

This document provides complete algorithmic pseudocode for implementing config-driven parsers in the Neural Data Platform ingestion system. The design supports two parser types:

1. **FlatJsonParser** - For AirGradient MQTT sensors (extracts all numeric fields)
2. **JsonPathParser** - For OpenWeatherMap HTTP API (maps JSON paths to metrics)

---

## 1. Parser Trait Definition

```rust
TRAIT: Parser
PURPOSE: Unified interface for parsing source-specific payloads into time series points
LIFECYCLE: Thread-safe, sharable across async tasks

METHODS:
    parse(payload: &[u8], source_type: &str) -> CoreResult<Vec<TimeSeriesPoint>>
        PURPOSE: Parse raw payload into time series points
        INPUT:
            - payload: Raw bytes from source (JSON, CSV, etc.)
            - source_type: Source identifier for context ("mqtt", "http_poll")
        OUTPUT:
            - Vec<TimeSeriesPoint> on success
            - CoreError on parsing failure
        CONSTRAINTS:
            - Must handle malformed input gracefully
            - Must preserve original metric names (no renaming in Bronze layer)
            - Must extract location_id from configured field

    from_config(config: &ParserConfig) -> CoreResult<Self>
        PURPOSE: Factory method to construct parser from configuration
        INPUT:
            - config: Parser configuration with type and parameters
        OUTPUT:
            - Parser implementation instance
            - CoreError if config is invalid
        CONSTRAINTS:
            - Must validate all required config fields
            - Must fail fast on misconfiguration

    name() -> &'static str
        PURPOSE: Human-readable parser name for logging
        OUTPUT: Static string identifier ("flat_json", "json_path")

REQUIREMENTS:
    - Send + Sync for async runtime compatibility
    - Stateless design (no mutable state between parse calls)
    - Zero-copy parsing where possible
```

---

## 2. FlatJsonParser Algorithm

### 2.1 Data Structure

```
STRUCT: FlatJsonParser
PURPOSE: Extract all numeric fields from flat JSON objects (AirGradient sensors)

FIELDS:
    location_field: String
        - JSON field name containing location/sensor identifier
        - Example: "serialno" for AirGradient

    exclude_fields: HashSet<String>
        - Fields to skip during extraction (non-metric metadata)
        - Example: {"wifi", "boot", "firmware", "model", "ledMode", "bootCount"}

    source_tag: String
        - Value for "source" tag in TimeSeriesPoint
        - Example: "mqtt", "http_poll"

INVARIANTS:
    - location_field must exist in payload
    - exclude_fields must not include location_field
    - All numeric types supported: f64, i64, u64
```

### 2.2 Parsing Algorithm

```
ALGORITHM: FlatJsonParser.parse
INPUT: payload (raw bytes), source_type (string)
OUTPUT: Vec<TimeSeriesPoint> or CoreError

BEGIN
    // Step 1: Parse JSON
    json_value ← serde_json::from_slice(payload)
    IF json_value is Error THEN
        RETURN CoreError::Source("Failed to parse JSON: {error}")
    END IF

    // Step 2: Validate JSON structure
    IF NOT json_value.is_object() THEN
        RETURN CoreError::Source("Payload is not a JSON object")
    END IF

    json_object ← json_value.as_object()

    // Step 3: Extract location identifier
    location_value ← json_object.get(location_field)
    IF location_value is None THEN
        RETURN CoreError::Source("Missing location field: {location_field}")
    END IF

    IF NOT location_value.is_string() THEN
        RETURN CoreError::Source("Location field is not a string")
    END IF

    location_id ← location_value.as_str().to_string()

    // Step 4: Generate timestamp
    timestamp ← Utc::now()

    // Step 5: Initialize result vector
    points ← Vec::new()

    // Step 6: Extract all numeric fields
    FOR EACH (field_name, field_value) IN json_object DO
        // Skip location field (already extracted)
        IF field_name == location_field THEN
            CONTINUE
        END IF

        // Skip excluded fields (metadata)
        IF exclude_fields.contains(field_name) THEN
            CONTINUE
        END IF

        // Extract numeric value (supports f64, i64, u64)
        numeric_value ← CALL extract_numeric(field_value)

        IF numeric_value is None THEN
            // Skip non-numeric fields (e.g., string enums)
            CONTINUE
        END IF

        // Create tags map
        tags ← HashMap::new()
        tags.insert("metric", field_name.clone())  // ORIGINAL field name
        tags.insert("source", source_tag.clone())

        // Create TimeSeriesPoint
        point ← TimeSeriesPoint {
            timestamp: timestamp,
            location_id: location_id.clone(),
            value: numeric_value,
            tags: tags
        }

        points.push(point)
    END FOR

    // Step 7: Validate result
    IF points.is_empty() THEN
        warn!("No numeric fields extracted from payload")
    END IF

    RETURN Ok(points)
END

SUBROUTINE: extract_numeric
INPUT: json_value (serde_json::Value)
OUTPUT: Option<f64>

BEGIN
    // Try f64 (includes JSON numbers with decimals)
    IF json_value.is_f64() THEN
        RETURN Some(json_value.as_f64())
    END IF

    // Try i64 (JSON integers, may be negative)
    IF json_value.is_i64() THEN
        int_val ← json_value.as_i64()
        RETURN Some(int_val as f64)
    END IF

    // Try u64 (large positive integers)
    IF json_value.is_u64() THEN
        uint_val ← json_value.as_u64()
        RETURN Some(uint_val as f64)
    END IF

    // Not a numeric type
    RETURN None
END
```

### 2.3 Factory Method

```
ALGORITHM: FlatJsonParser.from_config
INPUT: config (ParserConfig)
OUTPUT: FlatJsonParser or CoreError

BEGIN
    // Step 1: Validate config type
    IF config.parser_type != "flat_json" THEN
        RETURN CoreError::Config("Expected parser type 'flat_json'")
    END IF

    // Step 2: Extract location field
    location_field ← config.params.get("location_field")
    IF location_field is None THEN
        RETURN CoreError::Config("Missing required param: location_field")
    END IF

    IF NOT location_field.is_string() THEN
        RETURN CoreError::Config("location_field must be a string")
    END IF

    // Step 3: Extract exclude_fields (optional)
    exclude_fields ← HashSet::new()
    IF config.params.contains_key("exclude_fields") THEN
        exclude_list ← config.params.get("exclude_fields")

        IF NOT exclude_list.is_array() THEN
            RETURN CoreError::Config("exclude_fields must be an array")
        END IF

        FOR EACH item IN exclude_list.as_array() DO
            IF NOT item.is_string() THEN
                RETURN CoreError::Config("exclude_fields items must be strings")
            END IF

            exclude_fields.insert(item.as_str().to_string())
        END FOR
    END IF

    // Step 4: Extract source tag (optional, default from source_type)
    source_tag ← config.params.get("source_tag")
        .and_then(|v| v.as_str())
        .unwrap_or(config.source_type)
        .to_string()

    // Step 5: Construct parser
    parser ← FlatJsonParser {
        location_field: location_field.as_str().to_string(),
        exclude_fields: exclude_fields,
        source_tag: source_tag
    }

    RETURN Ok(parser)
END
```

---

## 3. JsonPathParser Algorithm

### 3.1 Data Structure

```
STRUCT: JsonPathParser
PURPOSE: Extract specific fields from nested JSON using JSONPath expressions

FIELDS:
    location_id: String
        - Static location identifier for this source
        - Example: "home" for OpenWeatherMap

    mappings: Vec<JsonPathMapping>
        - List of JSONPath-to-metric mappings
        - Each mapping defines one metric extraction

    source_tag: String
        - Value for "source" tag in TimeSeriesPoint

STRUCT: JsonPathMapping
FIELDS:
    path: String
        - JSONPath expression (e.g., "$.main.temp")
        - Supports nested access with dot notation

    metric: String
        - Metric name to use in tags
        - Example: "temperature", "humidity"

    optional: bool
        - If true, missing path doesn't cause error
        - If false, missing path returns CoreError

INVARIANTS:
    - All paths must be valid JSONPath syntax
    - Metric names must be unique within parser
    - At least one mapping must exist
```

### 3.2 Parsing Algorithm

```
ALGORITHM: JsonPathParser.parse
INPUT: payload (raw bytes), source_type (string)
OUTPUT: Vec<TimeSeriesPoint> or CoreError

BEGIN
    // Step 1: Parse JSON
    json_value ← serde_json::from_slice(payload)
    IF json_value is Error THEN
        RETURN CoreError::Source("Failed to parse JSON: {error}")
    END IF

    // Step 2: Generate timestamp
    timestamp ← Utc::now()

    // Step 3: Initialize result vector
    points ← Vec::new()

    // Step 4: Process each mapping
    FOR EACH mapping IN mappings DO
        // Extract value using JSONPath
        extracted_value ← CALL extract_json_path(json_value, mapping.path)

        IF extracted_value is None THEN
            IF NOT mapping.optional THEN
                RETURN CoreError::Source("Required path not found: {mapping.path}")
            ELSE
                // Skip optional missing field
                CONTINUE
            END IF
        END IF

        // Convert to numeric
        numeric_value ← CALL extract_numeric(extracted_value)

        IF numeric_value is None THEN
            IF NOT mapping.optional THEN
                RETURN CoreError::Source("Path value is not numeric: {mapping.path}")
            ELSE
                CONTINUE
            END IF
        END IF

        // Create tags map
        tags ← HashMap::new()
        tags.insert("metric", mapping.metric.clone())
        tags.insert("source", source_tag.clone())

        // Create TimeSeriesPoint
        point ← TimeSeriesPoint {
            timestamp: timestamp,
            location_id: location_id.clone(),
            value: numeric_value,
            tags: tags
        }

        points.push(point)
    END FOR

    // Step 5: Validate result
    IF points.is_empty() THEN
        RETURN CoreError::Source("No fields extracted from payload")
    END IF

    RETURN Ok(points)
END

SUBROUTINE: extract_json_path
INPUT: json_value (serde_json::Value), path (String)
OUTPUT: Option<serde_json::Value>

PURPOSE: Navigate nested JSON using simple dot notation ($.main.temp)

BEGIN
    // Strip leading "$." if present
    normalized_path ← path.trim_start_matches("$.")

    // Split path into components
    components ← normalized_path.split(".")

    // Navigate JSON tree
    current_value ← json_value

    FOR EACH component IN components DO
        // Check if current value is an object
        IF NOT current_value.is_object() THEN
            RETURN None
        END IF

        // Get next level
        next_value ← current_value.get(component)

        IF next_value is None THEN
            RETURN None
        END IF

        current_value ← next_value
    END FOR

    RETURN Some(current_value.clone())
END
```

### 3.3 Factory Method

```
ALGORITHM: JsonPathParser.from_config
INPUT: config (ParserConfig)
OUTPUT: JsonPathParser or CoreError

BEGIN
    // Step 1: Validate config type
    IF config.parser_type != "json_path" THEN
        RETURN CoreError::Config("Expected parser type 'json_path'")
    END IF

    // Step 2: Extract location_id
    location_id ← config.params.get("location_id")
    IF location_id is None THEN
        RETURN CoreError::Config("Missing required param: location_id")
    END IF

    IF NOT location_id.is_string() THEN
        RETURN CoreError::Config("location_id must be a string")
    END IF

    // Step 3: Extract mappings
    mappings_value ← config.params.get("mappings")
    IF mappings_value is None THEN
        RETURN CoreError::Config("Missing required param: mappings")
    END IF

    IF NOT mappings_value.is_array() THEN
        RETURN CoreError::Config("mappings must be an array")
    END IF

    mappings ← Vec::new()
    metric_names ← HashSet::new()  // For uniqueness check

    FOR EACH mapping_obj IN mappings_value.as_array() DO
        IF NOT mapping_obj.is_object() THEN
            RETURN CoreError::Config("Each mapping must be an object")
        END IF

        // Extract path
        path ← mapping_obj.get("path")
        IF path is None OR NOT path.is_string() THEN
            RETURN CoreError::Config("Mapping missing required 'path' string")
        END IF

        // Extract metric
        metric ← mapping_obj.get("metric")
        IF metric is None OR NOT metric.is_string() THEN
            RETURN CoreError::Config("Mapping missing required 'metric' string")
        END IF

        // Check metric uniqueness
        metric_str ← metric.as_str().to_string()
        IF metric_names.contains(metric_str) THEN
            RETURN CoreError::Config("Duplicate metric name: {metric_str}")
        END IF
        metric_names.insert(metric_str.clone())

        // Extract optional flag (default: true)
        optional ← mapping_obj.get("optional")
            .and_then(|v| v.as_bool())
            .unwrap_or(true)

        // Validate path syntax
        path_str ← path.as_str().to_string()
        IF NOT is_valid_json_path(path_str) THEN
            RETURN CoreError::Config("Invalid JSONPath syntax: {path_str}")
        END IF

        // Create mapping
        mapping ← JsonPathMapping {
            path: path_str,
            metric: metric_str,
            optional: optional
        }

        mappings.push(mapping)
    END FOR

    IF mappings.is_empty() THEN
        RETURN CoreError::Config("At least one mapping required")
    END IF

    // Step 4: Extract source tag
    source_tag ← config.params.get("source_tag")
        .and_then(|v| v.as_str())
        .unwrap_or(config.source_type)
        .to_string()

    // Step 5: Construct parser
    parser ← JsonPathParser {
        location_id: location_id.as_str().to_string(),
        mappings: mappings,
        source_tag: source_tag
    }

    RETURN Ok(parser)
END

SUBROUTINE: is_valid_json_path
INPUT: path (String)
OUTPUT: bool

BEGIN
    // Simple validation for $.field.nested.path syntax

    // Must start with optional "$."
    normalized ← path.trim_start_matches("$.")

    // Must not be empty after normalization
    IF normalized.is_empty() THEN
        RETURN false
    END IF

    // Split into components
    components ← normalized.split(".")

    // Each component must be non-empty alphanumeric + underscore
    FOR EACH component IN components DO
        IF component.is_empty() THEN
            RETURN false
        END IF

        // Check valid identifier characters
        FOR EACH char IN component.chars() DO
            IF NOT (char.is_alphanumeric() OR char == '_') THEN
                RETURN false
            END IF
        END FOR
    END FOR

    RETURN true
END
```

---

## 4. Configuration Schema

### 4.1 Rust Type Definitions

```rust
TYPE DEFINITIONS:

STRUCT: ParserConfig
PURPOSE: Deserialized parser configuration from YAML
DERIVES: Debug, Clone, Deserialize

FIELDS:
    parser_type: String
        - Discriminator for parser type
        - Valid values: "flat_json", "json_path"

    params: HashMap<String, serde_json::Value>
        - Type-specific parameters
        - Flexible schema for different parser needs

STRUCT: SourceConfigWithParser
PURPOSE: Source configuration including parser config
DERIVES: Debug, Clone, Deserialize

FIELDS:
    source_type: String
        - "mqtt", "http_poll", etc.

    enabled: bool
        - Whether source is active

    parser: ParserConfig
        - Parser configuration for this source

    params: HashMap<String, serde_json::Value>
        - Source-specific parameters (broker URL, endpoints, etc.)
```

### 4.2 YAML Configuration Examples

```yaml
EXAMPLE 1: FlatJsonParser for MQTT (AirGradient)

sources:
  - source_type: mqtt
    enabled: true
    parser:
      parser_type: flat_json
      params:
        location_field: serialno
        exclude_fields:
          - wifi
          - boot
          - firmware
          - model
          - ledMode
          - bootCount
        source_tag: mqtt
    params:
      broker_url: mosquitto
      port: 1883
      topic_pattern: airgradient/readings/+
      qos: 1

EXAMPLE 2: JsonPathParser for HTTP (OpenWeatherMap)

sources:
  - source_type: http_poll
    enabled: true
    parser:
      parser_type: json_path
      params:
        location_id: home
        source_tag: http_poll
        mappings:
          - path: $.main.temp
            metric: temperature
            optional: false
          - path: $.main.humidity
            metric: humidity
            optional: false
          - path: $.main.pressure
            metric: pressure
            optional: true
          - path: $.wind.speed
            metric: wind_speed
            optional: true
          - path: $.wind.deg
            metric: wind_deg
            optional: true
          - path: $.wind.gust
            metric: wind_gust
            optional: true
          - path: $.clouds.all
            metric: clouds
            optional: true
          - path: $.visibility
            metric: visibility
            optional: true
          - path: $.rain.1h
            metric: rain_1h
            optional: true
          - path: $.snow.1h
            metric: snow_1h
            optional: true
    params:
      poll_interval_secs: 600
      timeout_secs: 30
      endpoints:
        - url: https://api.openweathermap.org/data/2.5/weather?lat=29.95838&lon=-81.30878&units=metric
          auth_type: query_param
          auth_key: appid
```

---

## 5. Parser Factory Function

```
ALGORITHM: create_parser_from_config
INPUT: config (ParserConfig)
OUTPUT: Box<dyn Parser> or CoreError

PURPOSE: Factory function to instantiate appropriate parser based on config type

BEGIN
    // Step 1: Match on parser type
    MATCH config.parser_type AS
        CASE "flat_json":
            parser ← FlatJsonParser::from_config(config)?
            RETURN Ok(Box::new(parser))

        CASE "json_path":
            parser ← JsonPathParser::from_config(config)?
            RETURN Ok(Box::new(parser))

        CASE unknown_type:
            RETURN CoreError::Config("Unknown parser type: {unknown_type}")
    END MATCH
END

ALTERNATIVE DESIGN: Parser Registry Pattern

STRUCT: ParserRegistry
FIELDS:
    factories: HashMap<String, ParserFactory>

TYPE: ParserFactory = Fn(&ParserConfig) -> CoreResult<Box<dyn Parser>>

ALGORITHM: ParserRegistry.register
INPUT: parser_type (String), factory (ParserFactory)

BEGIN
    factories.insert(parser_type, factory)
END

ALGORITHM: ParserRegistry.create
INPUT: config (ParserConfig)
OUTPUT: Box<dyn Parser> or CoreError

BEGIN
    factory ← factories.get(config.parser_type)

    IF factory is None THEN
        RETURN CoreError::Config("Unknown parser type: {config.parser_type}")
    END IF

    RETURN factory(config)
END

USAGE:
    registry ← ParserRegistry::new()
    registry.register("flat_json", |cfg| FlatJsonParser::from_config(cfg))
    registry.register("json_path", |cfg| JsonPathParser::from_config(cfg))

    parser ← registry.create(config)?
```

---

## 6. SourceManager Integration

### 6.1 Configuration Loading

```
ALGORITHM: SourceManager.load_source_configs
INPUT: config_path (String)
OUTPUT: Vec<SourceConfigWithParser> or CoreError

BEGIN
    // Step 1: Read YAML configuration
    yaml_content ← fs::read_to_string(config_path)?

    // Step 2: Parse YAML
    config ← serde_yaml::from_str::<StreamConfig>(yaml_content)?

    // Step 3: Validate sources section exists
    IF config.sources is None OR config.sources.is_empty() THEN
        RETURN CoreError::Config("No sources defined in config")
    END IF

    // Step 4: Return source configs
    RETURN Ok(config.sources)
END
```

### 6.2 Source Spawning with Parser

```
ALGORITHM: SourceManager.spawn_source
INPUT: source_config (SourceConfigWithParser), tx (mpsc::Sender)
OUTPUT: JoinHandle or CoreError

BEGIN
    // Step 1: Create parser from config
    parser ← create_parser_from_config(source_config.parser)?

    info!("Created parser: {}", parser.name())

    // Step 2: Match on source type and spawn
    MATCH source_config.source_type AS
        CASE "mqtt":
            mqtt_config ← extract_mqtt_config(source_config.params)?

            source ← MqttSource::new(
                mqtt_config,
                tx.clone(),
                parser  // Pass parser to source
            )

            handle ← tokio::spawn(async move {
                source.run().await
            })

            RETURN Ok(handle)

        CASE "http_poll":
            http_config ← extract_http_config(source_config.params)?

            source ← HttpPollSource::new(
                http_config,
                tx.clone(),
                parser  // Pass parser to source
            )

            handle ← tokio::spawn(async move {
                source.run().await
            })

            RETURN Ok(handle)

        CASE unknown_type:
            RETURN CoreError::Config("Unknown source type: {unknown_type}")
    END MATCH
END
```

### 6.3 MqttSource with Parser

```
STRUCT: MqttSource
FIELDS:
    config: MqttConfig
    tx: mpsc::Sender<TimeSeriesPoint>
    parser: Box<dyn Parser>  // Config-driven parser

ALGORITHM: MqttSource.run
BEGIN
    // Connect to MQTT broker
    client ← connect_mqtt_broker(config)?

    // Subscribe to topic pattern
    client.subscribe(config.topic_pattern, config.qos).await?

    info!("MQTT subscribed to {}", config.topic_pattern)

    // Message loop
    LOOP
        message ← client.next_message().await

        IF message is Error THEN
            error!("MQTT error: {}", message.error())
            CALL reconnect_with_backoff()
            CONTINUE
        END IF

        // Parse payload using configured parser
        result ← parser.parse(message.payload(), "mqtt")

        MATCH result AS
            CASE Ok(points):
                // Send all points to channel
                FOR EACH point IN points DO
                    IF tx.send(point).await is Error THEN
                        error!("Failed to send point to channel")
                    END IF
                END FOR

                debug!("Parsed {} points from MQTT message", points.len())

            CASE Err(e):
                error!("Parser error: {}", e)
                // Continue processing (don't crash on bad message)
        END MATCH
    END LOOP
END
```

### 6.4 HttpPollSource with Parser

```
STRUCT: HttpPollSource
FIELDS:
    config: HttpPollingConfig
    tx: mpsc::Sender<TimeSeriesPoint>
    parser: Box<dyn Parser>  // Config-driven parser
    client: reqwest::Client

ALGORITHM: HttpPollSource.run
BEGIN
    // Create HTTP client
    client ← reqwest::Client::builder()
        .timeout(config.timeout)
        .build()?

    info!("HTTP poller started, interval: {}s", config.poll_interval_secs)

    // Polling loop
    interval ← tokio::time::interval(config.poll_interval)

    LOOP
        interval.tick().await

        // Poll each endpoint
        FOR EACH endpoint IN config.endpoints DO
            result ← CALL poll_endpoint(endpoint)

            MATCH result AS
                CASE Ok(response_body):
                    // Parse response using configured parser
                    parse_result ← parser.parse(response_body.as_bytes(), "http_poll")

                    MATCH parse_result AS
                        CASE Ok(points):
                            FOR EACH point IN points DO
                                tx.send(point).await?
                            END FOR

                            debug!("Parsed {} points from HTTP response", points.len())

                        CASE Err(e):
                            error!("Parser error for {}: {}", endpoint.url, e)
                    END MATCH

                CASE Err(e):
                    error!("HTTP request failed for {}: {}", endpoint.url, e)
            END MATCH
        END FOR
    END LOOP
END

SUBROUTINE: poll_endpoint
INPUT: endpoint (EndpointConfig)
OUTPUT: CoreResult<String>

BEGIN
    // Build request with authentication
    request ← client.get(endpoint.url)

    MATCH endpoint.auth_type AS
        CASE "query_param":
            request ← request.query(&[(endpoint.auth_key, endpoint.auth_value)])

        CASE "header":
            request ← request.header(endpoint.auth_key, endpoint.auth_value)

        CASE "none":
            // No authentication
    END MATCH

    // Execute request
    response ← request.send().await?

    // Check status
    IF NOT response.status().is_success() THEN
        RETURN CoreError::Source("HTTP {}: {}", response.status(), endpoint.url)
    END IF

    // Read body
    body ← response.text().await?

    RETURN Ok(body)
END
```

---

## 7. Complexity Analysis

### 7.1 FlatJsonParser

**Time Complexity:**
- JSON parsing: O(n) where n = payload size
- Field iteration: O(m) where m = number of JSON fields
- Numeric extraction per field: O(1)
- **Total: O(n + m)** dominated by JSON parsing

**Space Complexity:**
- JSON value tree: O(n) for payload representation
- Result vector: O(p) where p = number of numeric fields
- **Total: O(n + p)**

**Optimization Notes:**
- Zero-copy string extraction where possible (use `&str` over `String`)
- Reuse HashMap allocations with `with_capacity(estimated_fields)`
- Consider serde_json streaming parser for very large payloads (>1MB)

### 7.2 JsonPathParser

**Time Complexity:**
- JSON parsing: O(n) where n = payload size
- Path extraction: O(k * d) where k = number of mappings, d = average path depth
- **Total: O(n + k*d)** dominated by JSON parsing for typical configs

**Space Complexity:**
- JSON value tree: O(n)
- Result vector: O(k) where k = number of mappings
- **Total: O(n + k)**

**Optimization Notes:**
- Cache parsed JSON value between mapping evaluations (single parse)
- Pre-compile path components during `from_config` (avoid runtime splits)
- Consider `jsonpath-rust` crate for complex path expressions (arrays, filters)

### 7.3 Parser Factory

**Time Complexity:**
- Config validation: O(c) where c = config size
- Parser construction: O(1) for simple parsers
- **Total: O(c)**

**Space Complexity:**
- Config deserialization: O(c)
- Parser struct: O(1) for FlatJsonParser, O(k) for JsonPathParser
- **Total: O(c + k)**

---

## 8. Error Handling Strategy

```
ERROR CATEGORIES:

1. Configuration Errors (fail-fast at startup):
   - Invalid parser type
   - Missing required params
   - Invalid JSONPath syntax
   - Duplicate metric names

   ACTION: Return CoreError::Config, prevent source spawn

2. Parsing Errors (graceful degradation):
   - Malformed JSON
   - Missing required fields
   - Type mismatches (non-numeric values)

   ACTION: Log error, skip message, continue processing

3. Runtime Errors (retry with backoff):
   - Network failures (MQTT disconnect, HTTP timeout)
   - Channel send failures (router down)

   ACTION: Exponential backoff reconnect

ERROR PROPAGATION:

parse() -> CoreResult<Vec<TimeSeriesPoint>>
    ├─ CoreError::Source("JSON parse failed")
    ├─ CoreError::Source("Missing location field")
    └─ CoreError::Source("No numeric fields found")

from_config() -> CoreResult<Self>
    ├─ CoreError::Config("Unknown parser type")
    ├─ CoreError::Config("Missing required param")
    └─ CoreError::Config("Invalid path syntax")

LOGGING STRATEGY:

ERROR level:
    - Configuration validation failures
    - Parsing failures (bad JSON structure)
    - Required field missing

WARN level:
    - Optional field missing
    - Non-numeric field skipped
    - Empty result set (no points extracted)

DEBUG level:
    - Successful parse (point count)
    - Field extraction details

INFO level:
    - Parser creation
    - Source startup with parser type
```

---

## 9. Testing Strategy

### 9.1 FlatJsonParser Tests

```
TEST SUITE: FlatJsonParser

TEST: parse_valid_airgradient_payload
INPUT:
    {
        "serialno": "84fce612a6dc",
        "wifi": -67,
        "rco2": 600,
        "pm02": 5.0,
        "atmp": 22.5,
        "rhum": 45.2,
        "tvocIndex": 120,
        "noxIndex": 1
    }
EXPECTED:
    - 5 points (excludes serialno, wifi)
    - location_id = "84fce612a6dc"
    - metrics: ["rco2", "pm02", "atmp", "rhum", "tvocIndex", "noxIndex"]
    - All have source="mqtt" tag

TEST: parse_missing_location_field
INPUT: { "pm02": 5.0 }
EXPECTED: CoreError::Source("Missing location field: serialno")

TEST: parse_non_json_payload
INPUT: b"not json!"
EXPECTED: CoreError::Source("Failed to parse JSON")

TEST: parse_all_non_numeric_fields
INPUT: { "serialno": "abc", "status": "ok", "mode": "auto" }
EXPECTED: Ok([]) - empty vector, warning logged

TEST: parse_mixed_numeric_types
INPUT: { "serialno": "abc", "int": 42, "float": 3.14, "uint": 255 }
EXPECTED: 3 points with correct f64 conversions

TEST: from_config_valid
INPUT:
    parser_type: flat_json
    params:
      location_field: serialno
      exclude_fields: [wifi, boot]
EXPECTED: FlatJsonParser with correct fields

TEST: from_config_missing_location_field
INPUT:
    parser_type: flat_json
    params: {}
EXPECTED: CoreError::Config("Missing required param: location_field")
```

### 9.2 JsonPathParser Tests

```
TEST SUITE: JsonPathParser

TEST: parse_openweathermap_response
INPUT:
    {
        "main": {
            "temp": 22.5,
            "humidity": 65.0,
            "pressure": 1013.2
        },
        "wind": {
            "speed": 3.5,
            "deg": 180
        }
    }
MAPPINGS:
    - path: $.main.temp, metric: temperature
    - path: $.main.humidity, metric: humidity
    - path: $.wind.speed, metric: wind_speed
EXPECTED:
    - 3 points
    - location_id = "home"
    - metrics: ["temperature", "humidity", "wind_speed"]

TEST: parse_missing_required_path
INPUT: { "main": { "temp": 22.5 } }
MAPPINGS:
    - path: $.main.humidity, metric: humidity, optional: false
EXPECTED: CoreError::Source("Required path not found: $.main.humidity")

TEST: parse_missing_optional_path
INPUT: { "main": { "temp": 22.5 } }
MAPPINGS:
    - path: $.main.humidity, metric: humidity, optional: true
EXPECTED: Ok([]) - empty vector

TEST: parse_nested_path_not_object
INPUT: { "main": "not_an_object" }
MAPPINGS:
    - path: $.main.temp, metric: temperature
EXPECTED: CoreError::Source (path navigation failed)

TEST: from_config_duplicate_metrics
INPUT:
    mappings:
      - path: $.a, metric: temp
      - path: $.b, metric: temp
EXPECTED: CoreError::Config("Duplicate metric name: temp")

TEST: from_config_invalid_json_path
INPUT:
    mappings:
      - path: $..invalid..path, metric: test
EXPECTED: CoreError::Config("Invalid JSONPath syntax")
```

---

## 10. Migration Path

### 10.1 Current State
- MQTT: Hardcoded `parse_payload()` in `MqttSource`
- HTTP: Hardcoded `OpenWeatherMapParser`

### 10.2 Migration Steps

```
PHASE 1: Implement Parser Trait
    1. Create parser module: core/src/parsers/mod.rs
    2. Define Parser trait
    3. Implement FlatJsonParser
    4. Implement JsonPathParser
    5. Implement parser factory
    6. Add unit tests

PHASE 2: Update Configuration Schema
    1. Add parser section to YAML schema
    2. Update serde models (SourceConfigWithParser)
    3. Validate against existing configs
    4. Update config documentation

PHASE 3: Refactor MqttSource
    1. Add parser field to MqttSource struct
    2. Replace parse_payload() with parser.parse()
    3. Update tests to inject mock parser
    4. Verify backward compatibility

PHASE 4: Refactor HttpPollSource
    1. Add parser field to HttpPollSource struct
    2. Remove OpenWeatherMapParser impl
    3. Use parser.parse() in poll loop
    4. Update tests

PHASE 5: Update Stream Configurations
    1. Add parser config to air-quality.yaml (MQTT)
    2. Add parser config to outdoor-weather.yaml (HTTP)
    3. Test with real deployments
    4. Document parser configuration

PHASE 6: Remove Legacy Code
    1. Delete hardcoded parse_payload() methods
    2. Delete OpenWeatherMapParser
    3. Update architecture documentation
    4. Close BUG-002
```

---

## 11. Future Extensions

### 11.1 Additional Parser Types

```
FUTURE: CsvParser
PURPOSE: Parse CSV payloads from legacy sensors

ALGORITHM:
    - Split payload by newline
    - Parse header row for field names
    - Parse data rows
    - Map columns to metrics based on config

FUTURE: ProtobufParser
PURPOSE: Parse Protocol Buffer messages

ALGORITHM:
    - Deserialize using configured .proto schema
    - Extract fields by protobuf field numbers
    - Support nested messages

FUTURE: RegexParser
PURPOSE: Extract metrics from unstructured text logs

ALGORITHM:
    - Apply regex patterns to payload
    - Extract named capture groups
    - Map groups to metrics
```

### 11.2 Advanced JSONPath Features

```
ENHANCEMENT: Array Access
SYNTAX: $.data[0].value
IMPLEMENTATION: Support bracket notation in extract_json_path()

ENHANCEMENT: Wildcard Iteration
SYNTAX: $.sensors[*].temperature
IMPLEMENTATION: Extract multiple points from array

ENHANCEMENT: Conditional Filtering
SYNTAX: $.data[?(@.type == 'temp')].value
IMPLEMENTATION: Use jsonpath-rust crate for complex queries
```

---

## Summary

This pseudocode document provides complete algorithmic specifications for:

1. **Parser Trait** - Unified interface for all parser types
2. **FlatJsonParser** - Dynamic field extraction from flat JSON (AirGradient)
3. **JsonPathParser** - Targeted field extraction using paths (OpenWeatherMap)
4. **Configuration Schema** - YAML structure and Rust types
5. **Parser Factory** - Dynamic parser instantiation
6. **SourceManager Integration** - End-to-end flow from config to ingestion

The design supports:
- **Zero configuration changes** to source implementations (dependency injection)
- **Type safety** through trait-based design
- **Extensibility** via factory pattern for future parser types
- **Error handling** with fail-fast config validation and graceful runtime degradation

**Next Phase**: Refinement (implement in Rust following this pseudocode)
