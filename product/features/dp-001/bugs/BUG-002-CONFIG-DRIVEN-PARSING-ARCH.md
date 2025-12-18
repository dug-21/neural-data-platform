# BUG-002: Config-Driven Parsing Architecture

**Bug ID**: BUG-002
**Feature**: dp-001
**Phase**: Architecture (SPARC A)
**Status**: Design Complete
**Created**: 2025-12-18
**Author**: ndp-architect

---

## 1. Executive Summary

This document describes the architecture for replacing hardcoded parser structs with a config-driven parser system. The current implementation has parsers tightly coupled to specific data formats (AirGradient, OpenWeatherMap), making it impossible to add new data sources without code changes.

### Problem Statement

The current ingestion layer has two issues:

1. **MQTT Parser**: Already uses dynamic JSON extraction (good), but field skip logic and location ID extraction are hardcoded
2. **HTTP Parsers**: Use hardcoded struct definitions that drop unknown fields

### Target State

- Parsers configured via YAML alongside stream definitions
- New data sources can be added without code changes
- Field extraction rules defined declaratively
- Parser behavior (flat extraction vs. JSON path) configurable per stream

---

## 2. Architecture Overview

### 2.1 C4 Context Diagram

```
                    +-------------------+
                    |   External APIs   |
                    | - OpenWeatherMap  |
                    | - Other APIs      |
                    +--------+----------+
                             |
                             | HTTP Responses (JSON)
                             v
+-------------------+    +-------------------+    +-------------------+
|  MQTT Broker      |    |  Config Store     |    |   Parquet Store   |
|  - AirGradient    |    |  - etcd           |    |   - Bronze Layer  |
|  - Other sensors  |    |  - YAML configs   |    +-------------------+
+--------+----------+    +--------+----------+              ^
         |                        |                         |
         | MQTT Messages          | Parser Configs          | TimeSeriesPoints
         v                        v                         |
+------------------------------------------------------------------+
|                      Neural Data Platform                         |
|                                                                   |
|  +------------------+     +------------------+     +------------+ |
|  |  Source Manager  | --> |  Parser Factory  | --> | Ingestion  | |
|  |  - MQTT Source   |     |  - Registry      |     | Router     | |
|  |  - HTTP Source   |     |  - Config Loader |     +-----+------+ |
|  +------------------+     +------------------+           |        |
|                                                          v        |
|                                              +-------------------+ |
|                                              |  Parquet Writer   | |
|                                              +-------------------+ |
+------------------------------------------------------------------+
```

### 2.2 C4 Container Diagram

```
+------------------------------------------------------------------+
|                     air-quality-app Container                     |
|                                                                   |
|  +------------------+     +----------------------------+          |
|  |  SourceManager   |     |      Parser Subsystem      |          |
|  |                  |     |                            |          |
|  | - spawn_source() |---->| +------------------------+ |          |
|  | - stop_source()  |     | |   ParserRegistry       | |          |
|  | - health_check() |     | | - register()           | |          |
|  +------------------+     | | - get() -> Parser      | |          |
|          |                | +------------------------+ |          |
|          |                |            |               |          |
|          v                |            v               |          |
|  +------------------+     | +------------------------+ |          |
|  |   MqttSource     |     | |    ParserFactory       | |          |
|  |   - parse()      |<----| | - from_config()        | |          |
|  +------------------+     | | - create_parser()      | |          |
|          |                | +------------------------+ |          |
|          |                |            |               |          |
|          v                |            v               |          |
|  +------------------+     | +------------------------+ |          |
|  | HttpPollingSource|     | |   Parser Trait         | |          |
|  |   - poll()       |<----| | impl: FlatJsonParser   | |          |
|  +------------------+     | | impl: JsonPathParser   | |          |
|                           | +------------------------+ |          |
|                           +----------------------------+          |
+------------------------------------------------------------------+
```

---

## 3. Parser Trait Design

### 3.1 Core Parser Trait

```rust
//! Parser trait for config-driven data extraction
//!
//! This trait defines the interface for all parsers in the NDP system.
//! Parsers convert raw JSON payloads into TimeSeriesPoint vectors.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::HashMap;

use crate::error::CoreResult;
use crate::traits::TimeSeriesPoint;

/// Configuration for a parser instance
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ParserConfig {
    /// Parser type identifier
    pub parser_type: ParserType,

    /// Field to use as location/sensor ID (JSON path)
    pub location_id_field: String,

    /// Default location ID if field not found
    pub default_location_id: Option<String>,

    /// Fields to skip during extraction (metadata fields)
    pub skip_fields: Vec<String>,

    /// For JsonPathParser: explicit field mappings
    pub field_mappings: Option<Vec<FieldMapping>>,

    /// Tags to add to all extracted points
    pub default_tags: HashMap<String, String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ParserType {
    /// Extract all numeric fields from flat JSON object
    FlatJson,
    /// Extract specific fields using JSON path expressions
    JsonPath,
    /// Custom parser (must be registered in code)
    Custom(String),
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct FieldMapping {
    /// JSON path to extract value (e.g., "main.temp", "list[0].components.pm2_5")
    pub path: String,
    /// Metric name for the extracted value
    pub metric_name: String,
    /// Optional unit for the metric
    pub unit: Option<String>,
    /// Optional transformation (e.g., kelvin_to_celsius)
    pub transform: Option<String>,
}

/// Main parser trait - all parsers must implement this
pub trait Parser: Send + Sync {
    /// Parse raw JSON payload into time series points
    fn parse(
        &self,
        payload: &Value,
        timestamp: DateTime<Utc>,
    ) -> CoreResult<Vec<TimeSeriesPoint>>;

    /// Return parser name for logging/debugging
    fn name(&self) -> &str;

    /// Return parser configuration for introspection
    fn config(&self) -> &ParserConfig;
}

/// Factory trait for creating parsers from config
pub trait ParserFactory: Send + Sync {
    /// Create a parser instance from configuration
    fn create(&self, config: ParserConfig) -> CoreResult<Box<dyn Parser>>;
}
```

### 3.2 Parser Type Hierarchy

```
Parser (trait)
    |
    +-- FlatJsonParser
    |       - Extracts ALL numeric fields from top-level JSON object
    |       - Uses skip_fields to exclude metadata
    |       - Preserves ORIGINAL field names (no renaming)
    |       - Used for: AirGradient sensors (MQTT & HTTP)
    |
    +-- JsonPathParser
    |       - Extracts fields using explicit JSON path mappings
    |       - Handles nested JSON structures
    |       - Supports array access (list[0].field)
    |       - Used for: OpenWeatherMap APIs
    |
    +-- CompositeParser (future)
            - Combines multiple parsers
            - Applies transformations
            - Used for: Complex APIs with multiple response formats
```

---

## 4. Parser Implementations

### 4.1 FlatJsonParser

```rust
//! Flat JSON Parser Implementation
//!
//! Extracts ALL numeric fields from a flat JSON object, preserving
//! original field names. This is the default parser for IoT sensors
//! that report multiple metrics in a single message.

use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::HashMap;

pub struct FlatJsonParser {
    config: ParserConfig,
}

impl FlatJsonParser {
    pub fn new(config: ParserConfig) -> Self {
        Self { config }
    }
}

impl Parser for FlatJsonParser {
    fn parse(
        &self,
        payload: &Value,
        timestamp: DateTime<Utc>,
    ) -> CoreResult<Vec<TimeSeriesPoint>> {
        let obj = payload.as_object()
            .ok_or_else(|| CoreError::Parser("Payload is not a JSON object".into()))?;

        // Extract location ID from configured field
        let location_id = self.extract_location_id(obj)?;

        let mut points = Vec::new();

        for (key, value) in obj {
            // Skip non-metric fields
            if self.config.skip_fields.contains(key) {
                continue;
            }

            // Extract numeric values (f64, i64, u64)
            let numeric_value = Self::extract_numeric(value);

            if let Some(num) = numeric_value {
                let mut tags = self.config.default_tags.clone();
                tags.insert("metric".to_string(), key.clone());

                points.push(TimeSeriesPoint {
                    timestamp,
                    location_id: location_id.clone(),
                    value: num,
                    tags,
                });
            }
        }

        Ok(points)
    }

    fn name(&self) -> &str {
        "flat_json"
    }

    fn config(&self) -> &ParserConfig {
        &self.config
    }
}

impl FlatJsonParser {
    fn extract_location_id(&self, obj: &serde_json::Map<String, Value>) -> CoreResult<String> {
        // Try to extract from configured field
        if let Some(value) = obj.get(&self.config.location_id_field) {
            if let Some(s) = value.as_str() {
                return Ok(s.to_string());
            }
        }

        // Fall back to default
        self.config.default_location_id.clone()
            .ok_or_else(|| CoreError::Parser(
                format!("Location ID field '{}' not found and no default configured",
                    self.config.location_id_field)
            ))
    }

    fn extract_numeric(value: &Value) -> Option<f64> {
        if let Some(num) = value.as_f64() {
            Some(num)
        } else if let Some(num) = value.as_i64() {
            Some(num as f64)
        } else if let Some(num) = value.as_u64() {
            Some(num as f64)
        } else {
            None
        }
    }
}
```

### 4.2 JsonPathParser

```rust
//! JSON Path Parser Implementation
//!
//! Extracts specific fields from nested JSON structures using path
//! expressions. This parser is used for external APIs with complex
//! response formats (e.g., OpenWeatherMap).

use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::HashMap;

pub struct JsonPathParser {
    config: ParserConfig,
}

impl JsonPathParser {
    pub fn new(config: ParserConfig) -> Self {
        Self { config }
    }

    /// Extract value at JSON path (e.g., "main.temp", "list[0].components.pm2_5")
    fn extract_at_path(&self, root: &Value, path: &str) -> Option<f64> {
        let mut current = root;

        for segment in path.split('.') {
            // Handle array access: field[0]
            if let Some(bracket_pos) = segment.find('[') {
                let field_name = &segment[..bracket_pos];
                let index_str = &segment[bracket_pos + 1..segment.len() - 1];
                let index: usize = index_str.parse().ok()?;

                current = current.get(field_name)?;
                current = current.get(index)?;
            } else {
                current = current.get(segment)?;
            }
        }

        // Extract numeric value
        if let Some(num) = current.as_f64() {
            Some(num)
        } else if let Some(num) = current.as_i64() {
            Some(num as f64)
        } else if let Some(num) = current.as_u64() {
            Some(num as f64)
        } else {
            None
        }
    }

    /// Apply transformation to value (e.g., unit conversion)
    fn apply_transform(&self, value: f64, transform: &str) -> f64 {
        match transform {
            "kelvin_to_celsius" => value - 273.15,
            "kelvin_to_fahrenheit" => (value - 273.15) * 9.0 / 5.0 + 32.0,
            "mps_to_mph" => value * 2.237,
            "mps_to_kmh" => value * 3.6,
            _ => value, // Unknown transform, return unchanged
        }
    }
}

impl Parser for JsonPathParser {
    fn parse(
        &self,
        payload: &Value,
        timestamp: DateTime<Utc>,
    ) -> CoreResult<Vec<TimeSeriesPoint>> {
        let mappings = self.config.field_mappings.as_ref()
            .ok_or_else(|| CoreError::Parser("JsonPathParser requires field_mappings".into()))?;

        // Extract location ID
        let location_id = self.extract_at_path(payload, &self.config.location_id_field)
            .map(|v| v.to_string())
            .or_else(|| payload.get(&self.config.location_id_field)?.as_str().map(String::from))
            .or_else(|| self.config.default_location_id.clone())
            .ok_or_else(|| CoreError::Parser("Could not extract location ID".into()))?;

        let mut points = Vec::new();

        for mapping in mappings {
            if let Some(mut value) = self.extract_at_path(payload, &mapping.path) {
                // Apply transformation if configured
                if let Some(transform) = &mapping.transform {
                    value = self.apply_transform(value, transform);
                }

                let mut tags = self.config.default_tags.clone();
                tags.insert("metric".to_string(), mapping.metric_name.clone());

                if let Some(unit) = &mapping.unit {
                    tags.insert("unit".to_string(), unit.clone());
                }

                points.push(TimeSeriesPoint {
                    timestamp,
                    location_id: location_id.clone(),
                    value,
                    tags,
                });
            }
        }

        Ok(points)
    }

    fn name(&self) -> &str {
        "json_path"
    }

    fn config(&self) -> &ParserConfig {
        &self.config
    }
}
```

---

## 5. Config Schema Design

### 5.1 Parser Configuration YAML

Parser configuration is embedded in the stream configuration under the `sources[].parser` key.

```yaml
# config/base/streams/air-quality.yaml
stream_id: air-quality
description: "Indoor air quality from AirGradient sensor"
version: "1.0.0"
enabled: true
retention_days: 90
compression_after_days: 7
partitioning_strategy: daily

fields:
  - name: pm01
    type: Float
    description: "PM1.0 particulate matter (ug/m3)"
  - name: pm02
    type: Float
    description: "PM2.5 particulate matter (ug/m3)"
  - name: pm10
    type: Float
    description: "PM10 particulate matter (ug/m3)"
  - name: rco2
    type: Float
    description: "CO2 concentration (ppm)"
  - name: atmp
    type: Float
    description: "Temperature (Celsius)"
  - name: rhum
    type: Float
    description: "Relative humidity (%)"
  - name: tvocIndex
    type: Float
    description: "TVOC index"
  - name: noxIndex
    type: Float
    description: "NOx index"

sources:
  - source_type: mqtt
    enabled: true
    params:
      broker_url: "${MQTT_BROKER_URL}"
      port: 1883
      topic_pattern: "airgradient/readings/+"
      client_id: "ndp-air-quality"
      qos: 1

    # NEW: Parser configuration
    parser:
      parser_type: flat_json
      location_id_field: serialno
      default_location_id: unknown
      skip_fields:
        - serialno
        - firmware
        - model
        - ledMode
      default_tags:
        source: mqtt
        stream_id: air-quality

  - source_type: http_poll
    enabled: true
    params:
      poll_interval_secs: 60
      timeout_secs: 10
      endpoints:
        - serial: "${AIRGRADIENT_SERIAL}"
          url: "http://airgradient_${AIRGRADIENT_SERIAL}.local/measures/current"

    parser:
      parser_type: flat_json
      location_id_field: serialno
      skip_fields:
        - serialno
        - firmware
        - model
        - ledMode
      default_tags:
        source: http
        stream_id: air-quality
```

### 5.2 OpenWeatherMap Configuration Example

```yaml
# config/base/streams/outdoor-weather.yaml
stream_id: outdoor-weather
description: "Outdoor weather from OpenWeatherMap API"
version: "1.0.0"
enabled: true
retention_days: 365
compression_after_days: 30
partitioning_strategy: daily

fields:
  - name: temperature
    type: Float
    description: "Temperature (Celsius)"
  - name: feels_like
    type: Float
    description: "Feels like temperature (Celsius)"
  - name: pressure
    type: Float
    description: "Atmospheric pressure (hPa)"
  - name: humidity
    type: Float
    description: "Relative humidity (%)"
  - name: wind_speed
    type: Float
    description: "Wind speed (m/s)"
  - name: wind_deg
    type: Float
    description: "Wind direction (degrees)"
  - name: clouds
    type: Float
    description: "Cloud coverage (%)"

sources:
  - source_type: http_poll
    enabled: true
    params:
      poll_interval_secs: 600
      timeout_secs: 30
      endpoints:
        - endpoint_id: weather
          url: "https://api.openweathermap.org/data/2.5/weather?lat=${OWM_LAT}&lon=${OWM_LON}&units=metric"
          auth_type: query_param
          auth_key: appid
          auth_value: "${OPENWEATHERMAP_API_KEY}"

    parser:
      parser_type: json_path
      location_id_field: name
      default_location_id: "${OWM_LOCATION_NAME}"
      default_tags:
        source: openweathermap
        api: current_weather
        stream_id: outdoor-weather
      field_mappings:
        - path: main.temp
          metric_name: temperature
          unit: celsius
        - path: main.feels_like
          metric_name: feels_like
          unit: celsius
        - path: main.pressure
          metric_name: pressure
          unit: hpa
        - path: main.humidity
          metric_name: humidity
          unit: percent
        - path: wind.speed
          metric_name: wind_speed
          unit: m/s
        - path: wind.deg
          metric_name: wind_deg
          unit: degrees
        - path: wind.gust
          metric_name: wind_gust
          unit: m/s
        - path: clouds.all
          metric_name: clouds
          unit: percent
        - path: visibility
          metric_name: visibility
          unit: meters
```

### 5.3 Air Pollution Configuration Example

```yaml
# config/base/streams/outdoor-air-quality.yaml
stream_id: outdoor-air-quality
description: "Outdoor air pollution from OpenWeatherMap API"
version: "1.0.0"
enabled: true
retention_days: 365
compression_after_days: 30
partitioning_strategy: daily

fields:
  - name: aqi
    type: Int
    description: "Air Quality Index (1-5)"
  - name: co
    type: Float
    description: "Carbon monoxide (ug/m3)"
  - name: no
    type: Float
    description: "Nitrogen monoxide (ug/m3)"
  - name: no2
    type: Float
    description: "Nitrogen dioxide (ug/m3)"
  - name: o3
    type: Float
    description: "Ozone (ug/m3)"
  - name: so2
    type: Float
    description: "Sulfur dioxide (ug/m3)"
  - name: pm2_5
    type: Float
    description: "PM2.5 (ug/m3)"
  - name: pm10
    type: Float
    description: "PM10 (ug/m3)"
  - name: nh3
    type: Float
    description: "Ammonia (ug/m3)"

sources:
  - source_type: http_poll
    enabled: true
    params:
      poll_interval_secs: 600
      timeout_secs: 30
      endpoints:
        - endpoint_id: air_pollution
          url: "https://api.openweathermap.org/data/2.5/air_pollution?lat=${OWM_LAT}&lon=${OWM_LON}"
          auth_type: query_param
          auth_key: appid
          auth_value: "${OPENWEATHERMAP_API_KEY}"

    parser:
      parser_type: json_path
      location_id_field: coord
      default_location_id: "${OWM_LOCATION_NAME}"
      default_tags:
        source: openweathermap
        api: air_pollution
        stream_id: outdoor-air-quality
      field_mappings:
        - path: list[0].main.aqi
          metric_name: aqi
          unit: 1-5_scale
        - path: list[0].components.co
          metric_name: co
          unit: ug/m3
        - path: list[0].components.no
          metric_name: no
          unit: ug/m3
        - path: list[0].components.no2
          metric_name: no2
          unit: ug/m3
        - path: list[0].components.o3
          metric_name: o3
          unit: ug/m3
        - path: list[0].components.so2
          metric_name: so2
          unit: ug/m3
        - path: list[0].components.pm2_5
          metric_name: pm2_5
          unit: ug/m3
        - path: list[0].components.pm10
          metric_name: pm10
          unit: ug/m3
        - path: list[0].components.nh3
          metric_name: nh3
          unit: ug/m3
```

---

## 6. Integration Points

### 6.1 Component Diagram

```
+------------------------------------------------------------------+
|                        SourceManager                              |
|                                                                   |
|  +--------------------+        +--------------------+             |
|  | StreamConfig       |------->| ParserFactory      |             |
|  | - sources[]        |        | - create(config)   |             |
|  | - parser (new!)    |        +----------+---------+             |
|  +--------------------+                   |                       |
|                                           v                       |
|  +--------------------+        +--------------------+             |
|  | MqttSource         |<-------| Parser (dyn trait) |             |
|  | - parser: Parser   |        | - FlatJsonParser   |             |
|  | - parse_payload()  |        | - JsonPathParser   |             |
|  +----------+---------+        +--------------------+             |
|             |                             ^                       |
|             |                             |                       |
|  +----------v---------+        +----------+---------+             |
|  | HttpPollingSource  |<-------| ParserRegistry     |             |
|  | - parser: Parser   |        | - get(name)        |             |
|  | - poll_endpoint()  |        | - register(parser) |             |
|  +--------------------+        +--------------------+             |
+------------------------------------------------------------------+
```

### 6.2 SourceManager Integration

The SourceManager needs to be updated to:

1. **Parse parser config from stream config**
2. **Create parser instance via ParserFactory**
3. **Inject parser into MqttSource/HttpPollingSource**

```rust
// apps/air-quality-app/src/coordinator/source_manager.rs

impl SourceManager {
    /// Create parser from source config
    fn create_parser_from_config(
        &self,
        source_config: &SourceConfig,
    ) -> Result<Box<dyn Parser>, SourceManagerError> {
        let parser_config = source_config.params
            .get("parser")
            .ok_or_else(|| SourceManagerError::ConfigError(
                "Missing parser configuration".to_string()
            ))?;

        let config: ParserConfig = serde_json::from_value(parser_config.clone())
            .map_err(|e| SourceManagerError::ConfigError(
                format!("Invalid parser config: {}", e)
            ))?;

        self.parser_factory.create(config)
    }

    /// Updated spawn_source to inject parser
    async fn spawn_source(
        &mut self,
        stream_id: &str,
        source_config: &SourceConfig,
    ) -> Result<String, SourceManagerError> {
        // Create parser from config
        let parser = self.create_parser_from_config(source_config)?;

        match source_config.source_type {
            SourceType::Mqtt => {
                let mqtt_config = self.parse_mqtt_config(stream_id, source_config)?;
                let source = MqttSource::new_with_parser(mqtt_config, parser);
                // ... spawn task
            }
            SourceType::HttpPoll => {
                let http_config = self.parse_http_polling_config(stream_id, source_config)?;
                let source = HttpPollingSource::new_with_parser(http_config, parser)?;
                // ... spawn task
            }
            // ...
        }
    }
}
```

### 6.3 MqttSource Changes

```rust
// core/src/sources/mqtt.rs

pub struct MqttSource {
    config: MqttConfig,
    parser: Box<dyn Parser>,  // NEW: injected parser
    // ... existing fields
}

impl MqttSource {
    /// Create with injected parser (preferred)
    pub fn new_with_parser(config: MqttConfig, parser: Box<dyn Parser>) -> Self {
        // ...
    }

    /// Parse payload using injected parser
    fn parse_payload(&self, payload: &[u8]) -> CoreResult<Vec<TimeSeriesPoint>> {
        let json: Value = serde_json::from_slice(payload)?;
        let timestamp = Utc::now();
        self.parser.parse(&json, timestamp)
    }
}
```

### 6.4 HttpPollingSource Changes

```rust
// core/src/sources/http_poll.rs

pub struct HttpPollingSource {
    config: HttpPollingConfig,
    parser: Box<dyn Parser>,  // NEW: injected parser
    // ... existing fields
}

impl HttpPollingSource {
    /// Create with injected parser
    pub fn new_with_parser(
        config: HttpPollingConfig,
        parser: Box<dyn Parser>,
    ) -> CoreResult<Self> {
        // ...
    }

    /// Poll sensor using injected parser
    async fn poll_sensor(&self, sensor: &SensorConfig) -> CoreResult<Vec<TimeSeriesPoint>> {
        let response = self.client.get(&sensor.url).send().await?;
        let json: Value = response.json().await?;
        let timestamp = Utc::now();
        self.parser.parse(&json, timestamp)
    }
}
```

---

## 7. Data Flow Diagrams

### 7.1 MQTT Data Flow

```
+----------------+     +----------------+     +----------------+
|   MQTT Broker  |     |   MqttSource   |     |   Ingestion    |
|                |     |                |     |   Router       |
+-------+--------+     +-------+--------+     +-------+--------+
        |                      |                      |
        | 1. MQTT Message      |                      |
        | (raw JSON)           |                      |
        +--------------------->|                      |
        |                      |                      |
        |                      | 2. parser.parse()    |
        |                      |    (config-driven)   |
        |                      |                      |
        |                      | 3. Vec<TSPoint>      |
        |                      |    with tags:        |
        |                      |    - metric: rco2    |
        |                      |    - source: mqtt    |
        |                      |    - stream_id: ...  |
        |                      +--------------------->|
        |                      |                      |
        |                      |                      | 4. Route by
        |                      |                      |    stream_id
        |                      |                      |
        |                      |                      v
        |                      |               +----------------+
        |                      |               | ParquetStore   |
        |                      |               | /data/{stream} |
        |                      |               +----------------+
```

### 7.2 HTTP Polling Data Flow

```
+----------------+     +----------------+     +----------------+
| External API   |     | HttpPollSource |     |   Ingestion    |
| (OpenWeather)  |     |                |     |   Router       |
+-------+--------+     +-------+--------+     +-------+--------+
        |                      |                      |
        | 1. HTTP Response     |                      |
        | (nested JSON)        |                      |
        <----------------------+                      |
        +--------------------->|                      |
        |                      |                      |
        |                      | 2. parser.parse()    |
        |                      |    JsonPathParser    |
        |                      |    extracts paths:   |
        |                      |    - main.temp       |
        |                      |    - wind.speed      |
        |                      |                      |
        |                      | 3. Vec<TSPoint>      |
        |                      |    with tags:        |
        |                      |    - metric: temp    |
        |                      |    - unit: celsius   |
        |                      |    - source: owm     |
        |                      +--------------------->|
        |                      |                      |
        |                      |                      | 4. Route by
        |                      |                      |    stream_id
        |                      |                      |
        |                      |                      v
        |                      |               +----------------+
        |                      |               | ParquetStore   |
        |                      |               | /data/{stream} |
        |                      |               +----------------+
```

### 7.3 Parser Selection Sequence

```
SourceManager             ParserFactory           StreamConfig
     |                         |                       |
     | 1. spawn_source()       |                       |
     |------------------------>|                       |
     |                         |                       |
     | 2. get parser config    |                       |
     |-------------------------------------------------|
     |                         |                       |
     | 3. create(ParserConfig) |                       |
     |------------------------>|                       |
     |                         |                       |
     |                         | 4. match parser_type  |
     |                         |    - flat_json        |
     |                         |    - json_path        |
     |                         |    - custom(name)     |
     |                         |                       |
     | 5. Box<dyn Parser>      |                       |
     |<------------------------|                       |
     |                         |                       |
     | 6. inject into Source   |                       |
     |    MqttSource::new_with_parser(config, parser)  |
     |                         |                       |
```

---

## 8. ADR: Config-Driven Parser Architecture

### ADR-003: Config-Driven Parser Selection

**Status**: Proposed
**Date**: 2025-12-18
**Context**: Parser implementation decision

---

#### Context

The Neural Data Platform ingests data from multiple source types:
- MQTT (AirGradient sensors) - flat JSON with many numeric fields
- HTTP APIs (OpenWeatherMap) - nested JSON with specific paths

Current implementation hardcodes parser logic:
- `AirGradientReading` struct drops unknown fields
- `WeatherResponse` struct requires code changes for new fields
- No way to add new sources without Rust code changes

#### Decision

Implement a config-driven parser system with two standard parser types:

1. **FlatJsonParser**: Extracts all numeric fields from flat JSON
2. **JsonPathParser**: Extracts specific paths from nested JSON

Parser selection and configuration is defined in stream YAML files alongside source configuration.

#### Consequences

**Positive:**
- New data sources can be added via config only
- Parser behavior is transparent and auditable
- Field extraction is explicit (no magic renaming)
- Easy to test parser configs against sample data

**Negative:**
- Additional configuration complexity
- JsonPath expressions may be unfamiliar to operators
- Custom parsers still require code (but rare)

**Risks:**
- Invalid parser configs could cause silent data loss
- Performance impact of dynamic parsing (mitigated by caching)

#### Implementation Notes

1. Parser trait must be object-safe for dynamic dispatch
2. ParserFactory creates correct parser type from config
3. Source implementations receive parser via constructor injection
4. Stream config schema extended with `parser:` section
5. Validation of parser config at stream load time

---

## 9. Migration Plan

### Phase 1: Add Parser Trait and Implementations (Non-breaking)

1. Create `core/src/parsers/mod.rs` module
2. Implement `Parser` trait
3. Implement `FlatJsonParser`
4. Implement `JsonPathParser`
5. Add comprehensive tests

### Phase 2: Update Sources to Accept Parser (Non-breaking)

1. Add `new_with_parser()` constructors to sources
2. Keep existing `new()` constructors for backward compatibility
3. Deprecate old parsing methods

### Phase 3: Update SourceManager (Non-breaking)

1. Add `ParserFactory` to SourceManager
2. Create parser from config when available
3. Fall back to default parser when config missing

### Phase 4: Update Stream Configs (Breaking for new features only)

1. Add `parser:` section to stream YAML files
2. Document parser configuration format
3. Validate parser config at load time

### Phase 5: Remove Legacy Parsers (Breaking)

1. Remove `AirGradientReading` struct
2. Remove `CurrentMeasures` struct
3. Remove hardcoded `WeatherParser`
4. Remove hardcoded `AirPollutionParser`

---

## 10. Testing Strategy

### 10.1 Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flat_json_parser_extracts_all_numeric_fields() {
        let config = ParserConfig {
            parser_type: ParserType::FlatJson,
            location_id_field: "serialno".to_string(),
            default_location_id: None,
            skip_fields: vec!["serialno".to_string(), "firmware".to_string()],
            field_mappings: None,
            default_tags: HashMap::new(),
        };

        let parser = FlatJsonParser::new(config);

        let payload = serde_json::json!({
            "serialno": "d83bda1cd074",
            "firmware": "3.4.1",
            "pm01": 1.0,
            "pm02": 2.17,
            "rco2": 396,
            "atmp": 22.1,
            "tvocIndex": 42
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();

        // Should extract 5 numeric fields (skip serialno, firmware)
        assert_eq!(points.len(), 5);

        let metrics: Vec<&str> = points.iter()
            .map(|p| p.tags.get("metric").unwrap().as_str())
            .collect();

        assert!(metrics.contains(&"pm01"));
        assert!(metrics.contains(&"pm02"));
        assert!(metrics.contains(&"rco2"));
        assert!(metrics.contains(&"atmp"));
        assert!(metrics.contains(&"tvocIndex"));

        // serialno and firmware should NOT be extracted
        assert!(!metrics.contains(&"serialno"));
        assert!(!metrics.contains(&"firmware"));
    }

    #[test]
    fn test_json_path_parser_extracts_nested_fields() {
        let config = ParserConfig {
            parser_type: ParserType::JsonPath,
            location_id_field: "name".to_string(),
            default_location_id: Some("test_location".to_string()),
            skip_fields: vec![],
            field_mappings: Some(vec![
                FieldMapping {
                    path: "main.temp".to_string(),
                    metric_name: "temperature".to_string(),
                    unit: Some("celsius".to_string()),
                    transform: None,
                },
                FieldMapping {
                    path: "wind.speed".to_string(),
                    metric_name: "wind_speed".to_string(),
                    unit: Some("m/s".to_string()),
                    transform: None,
                },
            ]),
            default_tags: HashMap::new(),
        };

        let parser = JsonPathParser::new(config);

        let payload = serde_json::json!({
            "name": "London",
            "main": {
                "temp": 20.5,
                "humidity": 65
            },
            "wind": {
                "speed": 3.5,
                "deg": 180
            }
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();

        // Should extract only configured fields
        assert_eq!(points.len(), 2);

        let temp_point = points.iter()
            .find(|p| p.tags.get("metric") == Some(&"temperature".to_string()))
            .unwrap();
        assert_eq!(temp_point.value, 20.5);
        assert_eq!(temp_point.tags.get("unit"), Some(&"celsius".to_string()));
    }

    #[test]
    fn test_json_path_parser_handles_array_access() {
        let config = ParserConfig {
            parser_type: ParserType::JsonPath,
            location_id_field: "coord".to_string(),
            default_location_id: Some("test".to_string()),
            skip_fields: vec![],
            field_mappings: Some(vec![
                FieldMapping {
                    path: "list[0].main.aqi".to_string(),
                    metric_name: "aqi".to_string(),
                    unit: Some("1-5_scale".to_string()),
                    transform: None,
                },
                FieldMapping {
                    path: "list[0].components.pm2_5".to_string(),
                    metric_name: "pm2_5".to_string(),
                    unit: Some("ug/m3".to_string()),
                    transform: None,
                },
            ]),
            default_tags: HashMap::new(),
        };

        let parser = JsonPathParser::new(config);

        let payload = serde_json::json!({
            "coord": {"lat": 51.5, "lon": -0.1},
            "list": [{
                "main": {"aqi": 2},
                "components": {
                    "pm2_5": 8.59,
                    "pm10": 12.15
                }
            }]
        });

        let points = parser.parse(&payload, Utc::now()).unwrap();

        assert_eq!(points.len(), 2);

        let aqi_point = points.iter()
            .find(|p| p.tags.get("metric") == Some(&"aqi".to_string()))
            .unwrap();
        assert_eq!(aqi_point.value, 2.0);
    }
}
```

### 10.2 Integration Tests

```rust
#[tokio::test]
async fn test_mqtt_source_with_flat_json_parser() {
    // Create parser from config
    let config = ParserConfig {
        parser_type: ParserType::FlatJson,
        location_id_field: "serialno".to_string(),
        default_location_id: Some("unknown".to_string()),
        skip_fields: vec!["serialno".to_string()],
        field_mappings: None,
        default_tags: [("source".to_string(), "mqtt".to_string())].into(),
    };

    let parser = Box::new(FlatJsonParser::new(config));

    // Create MQTT source with parser
    let mqtt_config = MqttConfig::default();
    let source = MqttSource::new_with_parser(mqtt_config, parser);

    // Simulate MQTT message
    let payload = r#"{"serialno": "abc123", "pm02": 12.5, "rco2": 400}"#;
    let points = source.parse_payload(payload.as_bytes()).unwrap();

    assert_eq!(points.len(), 2);
    assert!(points.iter().all(|p| p.tags.get("source") == Some(&"mqtt".to_string())));
}

#[tokio::test]
async fn test_http_source_with_json_path_parser() {
    let mock_server = wiremock::MockServer::start().await;

    // Set up mock response
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .respond_with(wiremock::ResponseTemplate::new(200)
            .set_body_json(serde_json::json!({
                "name": "TestCity",
                "main": {"temp": 22.5, "humidity": 55},
                "wind": {"speed": 2.5}
            })))
        .mount(&mock_server)
        .await;

    // Create parser from config
    let config = ParserConfig {
        parser_type: ParserType::JsonPath,
        location_id_field: "name".to_string(),
        default_location_id: None,
        skip_fields: vec![],
        field_mappings: Some(vec![
            FieldMapping {
                path: "main.temp".to_string(),
                metric_name: "temperature".to_string(),
                unit: Some("celsius".to_string()),
                transform: None,
            },
        ]),
        default_tags: HashMap::new(),
    };

    let parser = Box::new(JsonPathParser::new(config));

    // Create HTTP source with parser
    let http_config = HttpPollingConfig {
        sensors: vec![SensorConfig {
            serial_number: "test".to_string(),
            url: format!("{}/weather", mock_server.uri()),
        }],
        ..Default::default()
    };

    let source = HttpPollingSource::new_with_parser(http_config, parser).unwrap();

    // Poll and verify
    let points = source.poll_sensor(&source.config.sensors[0]).await.unwrap();

    assert_eq!(points.len(), 1);
    assert_eq!(points[0].value, 22.5);
    assert_eq!(points[0].location_id, "TestCity");
}
```

---

## 11. Files to Create/Modify

### New Files

| Path | Description |
|------|-------------|
| `core/src/parsers/mod.rs` | Parser module exports |
| `core/src/parsers/traits.rs` | Parser trait definition |
| `core/src/parsers/flat_json.rs` | FlatJsonParser implementation |
| `core/src/parsers/json_path.rs` | JsonPathParser implementation |
| `core/src/parsers/factory.rs` | ParserFactory implementation |
| `core/src/parsers/config.rs` | ParserConfig structs |

### Modified Files

| Path | Changes |
|------|---------|
| `core/src/lib.rs` | Export parsers module |
| `core/src/sources/mqtt.rs` | Add `new_with_parser()`, use injected parser |
| `core/src/sources/http_poll.rs` | Add `new_with_parser()`, use injected parser |
| `apps/air-quality-app/src/coordinator/source_manager.rs` | Create parsers from config |
| `config/base/streams/air-quality.yaml` | Add parser config |
| `config/base/streams/outdoor-weather.yaml` | Add parser config |
| `config/base/streams/outdoor-air-quality.yaml` | Add parser config |

### Files to Remove (Phase 5)

| Path | Reason |
|------|--------|
| `core/src/sources/parsers/weather.rs` | Replaced by JsonPathParser + config |
| `core/src/sources/parsers/air_pollution.rs` | Replaced by JsonPathParser + config |

---

## 12. Success Criteria

### Functional Requirements

- [ ] FlatJsonParser extracts all numeric fields from AirGradient payload
- [ ] FlatJsonParser preserves original field names (no renaming)
- [ ] JsonPathParser extracts configured paths from OpenWeatherMap responses
- [ ] JsonPathParser handles array access syntax (`list[0].field`)
- [ ] Parser config is loaded from stream YAML files
- [ ] Sources receive parser via constructor injection
- [ ] Unknown fields are NOT silently dropped

### Non-Functional Requirements

- [ ] Parser creation < 1ms
- [ ] Parsing latency < 100us per message
- [ ] Memory overhead < 1KB per parser instance
- [ ] Config validation fails fast on invalid parser type

### Testing Requirements

- [ ] Unit tests for FlatJsonParser
- [ ] Unit tests for JsonPathParser
- [ ] Integration tests for MQTT with parser injection
- [ ] Integration tests for HTTP with parser injection
- [ ] Config validation tests
- [ ] Backward compatibility tests (old sources without parser config)

---

## 13. References

- [Current MQTT Source](/workspaces/neural-data-platform/core/src/sources/mqtt.rs)
- [Current HTTP Poll Source](/workspaces/neural-data-platform/core/src/sources/http_poll.rs)
- [Current Weather Parser](/workspaces/neural-data-platform/core/src/sources/parsers/weather.rs)
- [Current Air Pollution Parser](/workspaces/neural-data-platform/core/src/sources/parsers/air_pollution.rs)
- [Source Manager](/workspaces/neural-data-platform/apps/air-quality-app/src/coordinator/source_manager.rs)
- [TimeSeriesPoint Trait](/workspaces/neural-data-platform/core/src/traits.rs)
- [DP-001 Feature Scope](/workspaces/neural-data-platform/product/features/dp-001/SCOPE.md)
