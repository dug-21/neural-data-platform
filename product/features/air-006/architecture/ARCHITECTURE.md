# AIR-006: Parser System Unification - Architecture

**Feature**: air-006
**Phase**: Architecture (SPARC A)
**Status**: Design Complete
**Created**: 2025-12-21
**Author**: ndp-architect

---

## 1. Executive Summary

This document describes the architecture for unifying the Neural Data Platform's parser system. Currently, **two incompatible parser systems exist**, creating duplication and preventing config-driven data ingestion:

1. **Config-Driven Parser** (`core/src/parsers/`) - Parser trait, YAML-configurable ✅ KEEP
2. **Hardcoded ResponseParser** (`core/src/sources/parsers/`) - Struct-based, code changes required ❌ DELETE

### Target State

- **Single parser system**: Config-driven Parser trait
- **Zero code changes** needed to add new data sources
- **ArrayIteratorParser**: Handle array responses (OpenWeatherMap air pollution `list[0]`)
- **Unified integration**: Both MqttSource and GenericHttpPollingSource use Parser trait

---

## 2. Current State Analysis

### 2.1 Existing Parser Systems

| Parser System | Location | Type | Configuration | Status |
|--------------|----------|------|---------------|---------|
| **Parser trait** | `core/src/parsers/` | Interface | YAML config | ✅ Active |
| **FlatJsonParser** | `core/src/parsers/flat_json.rs` | Implementation | YAML | ✅ Active |
| **JsonPathParser** | `core/src/parsers/json_path.rs` | Implementation | YAML | ✅ Active |
| **ResponseParser trait** | `core/src/sources/http_poll.rs` | Interface | Hardcoded | ❌ Deprecate |
| **WeatherParser** | `core/src/sources/parsers/weather.rs` | Struct | Hardcoded | ❌ Delete |
| **AirPollutionParser** | `core/src/sources/parsers/air_pollution.rs` | Struct | Hardcoded | ❌ Delete |

### 2.2 Parser Trait Comparison

#### Config-Driven Parser (KEEP)
```rust
// core/src/parsers/traits.rs
pub trait Parser: Send + Sync {
    fn parse(&self, payload: &Value, timestamp: DateTime<Utc>)
        -> CoreResult<Vec<TimeSeriesPoint>>;
    fn name(&self) -> &str;
    fn config(&self) -> &ParserConfig;
}
```

**Strengths:**
- ✅ Config-driven field extraction
- ✅ YAML configuration support
- ✅ Designed for flexibility
- ✅ Already used by MqttSource

#### ResponseParser (DELETE)
```rust
// core/src/sources/http_poll.rs (lines 295-314)
pub trait ResponseParser: Send + Sync + 'static {
    fn parse(
        &self,
        response_body: &str,     // ← Takes string, not Value
        location_id: &str,       // ← Location passed separately
        timestamp: DateTime<Utc>
    ) -> CoreResult<Vec<TimeSeriesPoint>>;
    fn name(&self) -> &'static str;
}
```

**Issues:**
- ❌ Hardcoded struct definitions (WeatherResponse, AirPollutionResponse)
- ❌ No config support - requires code changes
- ❌ String parsing instead of Value
- ❌ Parallel implementation to Parser trait

### 2.3 Source Integration Status

| Source | Current Parser | Target Parser | Status |
|--------|---------------|---------------|--------|
| **MqttSource** | Parser trait ✅ | Parser trait | ✅ No changes needed |
| **HttpPollingSource** | Parser trait ✅ | Parser trait | ✅ No changes needed |
| **GenericHttpPollingSource** | ResponseParser ❌ | Parser trait | ⚠️ Needs migration |

---

## 3. Architecture Design

### 3.1 C4 Context Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                    External Data Sources                         │
│                                                                  │
│   ┌──────────────┐  ┌──────────────┐  ┌──────────────┐         │
│   │ AirGradient  │  │ OpenWeather  │  │   NWS API    │         │
│   │   Sensors    │  │  Current/    │  │  Forecasts   │         │
│   │   (MQTT)     │  │  Pollution   │  │   (HTTP)     │         │
│   └──────┬───────┘  └──────┬───────┘  └──────┬───────┘         │
│          │                  │                  │                 │
└──────────┼──────────────────┼──────────────────┼─────────────────┘
           │                  │                  │
           │ JSON payloads    │                  │
           ▼                  ▼                  ▼
┌─────────────────────────────────────────────────────────────────┐
│              Neural Data Platform - Ingestion Layer              │
│                                                                  │
│  ┌────────────────┐         ┌────────────────────────────┐      │
│  │  MqttSource    │         │ GenericHttpPollingSource  │      │
│  │  - flat JSON   │         │ - nested JSON             │      │
│  │  - all metrics │         │ - specific fields         │      │
│  └────────┬───────┘         └────────┬───────────────────┘      │
│           │                          │                          │
│           │  Uses Parser             │  Uses Parser             │
│           ▼                          ▼                          │
│  ┌──────────────────────────────────────────────────────┐      │
│  │           Unified Parser System                      │      │
│  │                                                       │      │
│  │  ┌────────────────┐   ┌────────────────┐            │      │
│  │  │ FlatJsonParser │   │ JsonPathParser │            │      │
│  │  │  (existing)    │   │  (existing)    │            │      │
│  │  └────────────────┘   └────────────────┘            │      │
│  │                                                       │      │
│  │  ┌────────────────────────────────────┐             │      │
│  │  │   ArrayIteratorParser (NEW)        │             │      │
│  │  │   - Unwraps list[0] arrays         │             │      │
│  │  │   - Delegates to JsonPathParser    │             │      │
│  │  └────────────────────────────────────┘             │      │
│  │                                                       │      │
│  │          All configured via YAML                     │      │
│  └───────────────────────┬───────────────────────────────┘      │
│                          │                                      │
│                          │ TimeSeriesPoints                     │
│                          ▼                                      │
│            ┌──────────────────────────┐                        │
│            │  IngestionCoordinator    │                        │
│            │  - Routes by stream_id   │                        │
│            └──────────┬───────────────┘                        │
│                       │                                         │
│                       ▼                                         │
│            ┌──────────────────────────┐                        │
│            │     ParquetStore          │                        │
│            │  Bronze Layer Storage     │                        │
│            └───────────────────────────┘                        │
└─────────────────────────────────────────────────────────────────┘
```

### 3.2 C4 Container Diagram - Parser System

```
┌──────────────────────────────────────────────────────────────────┐
│                    Parser Module Architecture                     │
│                                                                   │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │              core/src/parsers/ (Config-Driven)              │ │
│  │                                                             │ │
│  │  ┌────────────────────────────────────────────────────┐    │ │
│  │  │  Parser Trait (traits.rs)                         │    │ │
│  │  │  - parse(payload: &Value, timestamp)               │    │ │
│  │  │  - name() -> &str                                  │    │ │
│  │  │  - config() -> &ParserConfig                       │    │ │
│  │  └─────────────────┬──────────────────────────────────┘    │ │
│  │                    │                                        │ │
│  │                    │ implemented by                         │ │
│  │                    │                                        │ │
│  │      ┌─────────────┼─────────────┬────────────────┐        │ │
│  │      ▼             ▼             ▼                ▼        │ │
│  │  ┌────────┐  ┌───────────┐  ┌──────────┐  ┌─────────┐    │ │
│  │  │ Flat   │  │ JsonPath  │  │  Array   │  │ Custom  │    │ │
│  │  │ Json   │  │  Parser   │  │ Iterator │  │ (future)│    │ │
│  │  │ Parser │  │           │  │  Parser  │  │         │    │ │
│  │  └────────┘  └───────────┘  └──────────┘  └─────────┘    │ │
│  │                                   NEW!                      │ │
│  │                                                             │ │
│  │  ┌────────────────────────────────────────────────────┐    │ │
│  │  │  ParserConfig (config.rs)                         │    │ │
│  │  │  - parser_type: ParserType                        │    │ │
│  │  │  - location_id_field: String                      │    │ │
│  │  │  - skip_fields: Vec<String>                       │    │ │
│  │  │  - field_mappings: Vec<FieldMapping>              │    │ │
│  │  │  - array_path: Option<String>         ← NEW       │    │ │
│  │  │  - delegate_parser: Option<ParserType> ← NEW      │    │ │
│  │  └────────────────────────────────────────────────────┘    │ │
│  │                                                             │ │
│  │  ┌────────────────────────────────────────────────────┐    │ │
│  │  │  ParserFactory (factory.rs)                        │    │ │
│  │  │  - create_parser_from_config(config) -> Parser     │    │ │
│  │  └────────────────────────────────────────────────────┘    │ │
│  └─────────────────────────────────────────────────────────────┘ │
│                                                                   │
│  ┌─────────────────────────────────────────────────────────────┐ │
│  │    core/src/sources/parsers/ (DEPRECATED - TO DELETE)      │ │
│  │                                                             │ │
│  │  ┌────────────────────────────────────────────────────┐    │ │
│  │  │  ResponseParser trait (http_poll.rs)               │    │ │
│  │  │  ❌ DEPRECATED - Use Parser trait instead          │    │ │
│  │  └────────────────────────────────────────────────────┘    │ │
│  │                                                             │ │
│  │  ┌────────────────────────────────────────────────────┐    │ │
│  │  │  WeatherParser (weather.rs)                        │    │ │
│  │  │  ❌ DELETE - Replace with JsonPathParser + config  │    │ │
│  │  └────────────────────────────────────────────────────┘    │ │
│  │                                                             │ │
│  │  ┌────────────────────────────────────────────────────┐    │ │
│  │  │  AirPollutionParser (air_pollution.rs)             │    │ │
│  │  │  ❌ DELETE - Replace with ArrayIteratorParser      │    │ │
│  │  └────────────────────────────────────────────────────┘    │ │
│  └─────────────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────┘
```

### 3.3 Component Interaction Diagram

```
StreamConfig (YAML)                   SourceManager
      │                                    │
      │ parser:                            │
      │   parser_type: array_iterator      │
      │   array_path: "list[0]"            │
      │   delegate_parser: json_path       │
      │   field_mappings: [...]            │
      │                                    │
      └──────────────────────────────────►│
                                           │
                                           │ create_parser_from_config()
                                           ▼
                                    ParserFactory
                                           │
                                           │ match parser_type
                                           │
                       ┌───────────────────┼───────────────────┐
                       ▼                   ▼                   ▼
                  FlatJsonParser    JsonPathParser    ArrayIteratorParser
                       │                   │                   │
                       │                   │                   │
                       │                   │        wraps ────►│
                       │                   │                   │
                       └───────────────────┴───────────────────┘
                                           │
                                           │ Box<dyn Parser>
                                           ▼
                           GenericHttpPollingSource / MqttSource
                                           │
                                           │ poll_endpoint()
                                           ▼
                                      External API
                                           │
                                           │ JSON Response
                                           ▼
                                  parser.parse(&json, timestamp)
                                           │
                                           │ Vec<TimeSeriesPoint>
                                           ▼
                                  IngestionCoordinator
                                           │
                                           ▼
                                     ParquetStore
```

---

## 4. New Component: ArrayIteratorParser

### 4.1 Problem Statement

OpenWeatherMap Air Pollution API returns data wrapped in an array:

```json
{
  "coord": {"lat": 37.7749, "lon": -122.4194},
  "list": [
    {
      "main": {"aqi": 2},
      "components": {
        "pm2_5": 8.59,
        "pm10": 12.15,
        "no2": 15.3
      }
    }
  ]
}
```

**Current JsonPathParser limitation**: Cannot unwrap `list[0]` before applying field mappings.

### 4.2 ArrayIteratorParser Design

```rust
//! Array Iterator Parser
//!
//! Unwraps array responses and delegates field extraction to a child parser.
//! Use case: APIs that wrap data in `list[0]` or similar array structures.

use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::error::{CoreError, CoreResult};
use crate::traits::TimeSeriesPoint;

use super::{Parser, ParserConfig, ParserType};
use super::factory::create_parser_from_config;

/// Parser that extracts first element from an array and delegates to child parser
pub struct ArrayIteratorParser {
    config: ParserConfig,
    array_path: String,
    delegate_parser: Box<dyn Parser + Send + Sync>,
}

impl ArrayIteratorParser {
    /// Create from config
    pub fn from_config(config: ParserConfig) -> CoreResult<Self> {
        // Extract array_path from config
        let array_path = config.array_path.clone()
            .ok_or_else(|| CoreError::Config(
                "ArrayIteratorParser requires array_path".into()
            ))?;

        // Extract delegate parser type
        let delegate_type = config.delegate_parser.clone()
            .ok_or_else(|| CoreError::Config(
                "ArrayIteratorParser requires delegate_parser".into()
            ))?;

        // Create delegate parser config (inherit field_mappings, etc.)
        let delegate_config = ParserConfig {
            parser_type: delegate_type,
            location_id_field: config.location_id_field.clone(),
            default_location_id: config.default_location_id.clone(),
            skip_fields: config.skip_fields.clone(),
            field_mappings: config.field_mappings.clone(),
            default_tags: config.default_tags.clone(),
            array_path: None,  // Don't cascade array unwrapping
            delegate_parser: None,
        };

        // Create delegate parser
        let delegate_parser = create_parser_from_config(delegate_config)?;

        Ok(Self {
            config,
            array_path,
            delegate_parser,
        })
    }

    /// Extract value at array path and unwrap first element
    fn extract_array_element(&self, payload: &Value) -> CoreResult<Value> {
        let mut current = payload;

        // Navigate to array using path (e.g., "list")
        for segment in self.array_path.split('.') {
            current = current.get(segment)
                .ok_or_else(|| CoreError::Parser(
                    format!("Array path segment '{}' not found", segment)
                ))?;
        }

        // Extract first element
        let array = current.as_array()
            .ok_or_else(|| CoreError::Parser(
                format!("Expected array at path '{}'", self.array_path)
            ))?;

        let first_element = array.first()
            .ok_or_else(|| CoreError::Parser(
                "Array is empty".into()
            ))?;

        Ok(first_element.clone())
    }
}

impl Parser for ArrayIteratorParser {
    fn parse(
        &self,
        payload: &Value,
        timestamp: DateTime<Utc>,
    ) -> CoreResult<Vec<TimeSeriesPoint>> {
        // Extract first array element
        let unwrapped = self.extract_array_element(payload)?;

        // Delegate to child parser
        self.delegate_parser.parse(&unwrapped, timestamp)
    }

    fn name(&self) -> &str {
        "array_iterator"
    }

    fn config(&self) -> &ParserConfig {
        &self.config
    }
}
```

### 4.3 ArrayIteratorParser Configuration

```yaml
# config/base/streams/outdoor-air-quality.yaml
stream_id: outdoor-air-quality
description: "Outdoor air pollution from OpenWeatherMap API"
version: "1.0.0"
enabled: true

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

    # NEW: ArrayIteratorParser configuration
    parser:
      parser_type: array_iterator
      array_path: "list"           # ← Unwrap list[0]
      delegate_parser: json_path   # ← Then use JsonPathParser
      location_id_field: "coord"
      default_location_id: "${OWM_LOCATION_NAME}"
      default_tags:
        source: openweathermap
        api: air_pollution
        stream_id: outdoor-air-quality

      # Field mappings applied AFTER array unwrap
      field_mappings:
        - path: "main.aqi"
          metric_name: "aqi"
          unit: "1-5_scale"
        - path: "components.pm2_5"
          metric_name: "pm2_5"
          unit: "ug/m3"
        - path: "components.pm10"
          metric_name: "pm10"
          unit: "ug/m3"
        - path: "components.no2"
          metric_name: "no2"
          unit: "ug/m3"
```

---

## 5. Data Flow Diagrams

### 5.1 MQTT Data Flow (No Changes)

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│ MQTT Broker  │     │  MqttSource  │     │  Ingestion   │
│              │     │              │     │  Router      │
└──────┬───────┘     └──────┬───────┘     └──────┬───────┘
       │                    │                    │
       │ 1. MQTT Message    │                    │
       │ {                  │                    │
       │   "serialno": "X", │                    │
       │   "pm02": 12.5,    │                    │
       │   "rco2": 400      │                    │
       │ }                  │                    │
       ├───────────────────►│                    │
       │                    │                    │
       │                    │ 2. FlatJsonParser  │
       │                    │    .parse(json)    │
       │                    │                    │
       │                    │ 3. Vec<TSPoint>    │
       │                    │    [                │
       │                    │      {metric:pm02, │
       │                    │       value:12.5},  │
       │                    │      {metric:rco2, │
       │                    │       value:400}    │
       │                    │    ]               │
       │                    ├───────────────────►│
       │                    │                    │
       │                    │                    │ 4. route_by_stream_id
       │                    │                    ├──────────────┐
       │                    │                    │              ▼
       │                    │                    │      ┌────────────┐
       │                    │                    │      │  Parquet   │
       │                    │                    │      │   Store    │
       │                    │                    │      └────────────┘
```

### 5.2 HTTP Polling - Simple JSON (OpenWeatherMap Current Weather)

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│ OpenWeather  │     │   Generic    │     │  Ingestion   │
│  Current API │     │ HttpPolling  │     │  Router      │
└──────┬───────┘     └──────┬───────┘     └──────┬───────┘
       │                    │                    │
       │ 1. GET /weather    │                    │
       │◄───────────────────┤                    │
       │                    │                    │
       │ 2. Response        │                    │
       │ {                  │                    │
       │   "main": {        │                    │
       │     "temp": 20.5,  │                    │
       │     "humidity": 65 │                    │
       │   },               │                    │
       │   "wind": {        │                    │
       │     "speed": 3.5   │                    │
       │   }                │                    │
       │ }                  │                    │
       ├───────────────────►│                    │
       │                    │                    │
       │                    │ 3. JsonPathParser  │
       │                    │    .parse(json)    │
       │                    │                    │
       │                    │    Extracts:       │
       │                    │    - main.temp     │
       │                    │    - wind.speed    │
       │                    │                    │
       │                    │ 4. Vec<TSPoint>    │
       │                    │    [                │
       │                    │      {metric:temp, │
       │                    │       value:20.5},  │
       │                    │      {metric:wind, │
       │                    │       value:3.5}    │
       │                    │    ]               │
       │                    ├───────────────────►│
       │                    │                    │
       │                    │                    │ 5. route
       │                    │                    ├────────┐
       │                    │                    │        ▼
       │                    │                    │    ┌────────┐
       │                    │                    │    │Parquet │
       │                    │                    │    └────────┘
```

### 5.3 HTTP Polling - Array Response (OpenWeatherMap Air Pollution)

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│ OpenWeather  │     │   Generic    │     │  Ingestion   │
│ Pollution API│     │ HttpPolling  │     │  Router      │
└──────┬───────┘     └──────┬───────┘     └──────┬───────┘
       │                    │                    │
       │ 1. GET /pollution  │                    │
       │◄───────────────────┤                    │
       │                    │                    │
       │ 2. Response        │                    │
       │ {                  │                    │
       │   "list": [        │                    │
       │     {              │                    │
       │       "main": {    │                    │
       │         "aqi": 2   │                    │
       │       },           │                    │
       │       "components":│                    │
       │         "pm2_5":8.5│                    │
       │       }            │                    │
       │     }              │                    │
       │   ]                │                    │
       │ }                  │                    │
       ├───────────────────►│                    │
       │                    │                    │
       │              ArrayIteratorParser        │
       │                    │                    │
       │                    │ 3a. Unwrap list[0] │
       │                    │     ▼               │
       │                    │   {                │
       │                    │     "main": {...}, │
       │                    │     "components": {│
       │                    │        "pm2_5":8.5 │
       │                    │     }              │
       │                    │   }                │
       │                    │     │               │
       │                    │     │ 3b. Delegate  │
       │                    │     ▼               │
       │                    │ JsonPathParser     │
       │                    │   .parse(unwrapped)│
       │                    │                    │
       │                    │   Extracts:        │
       │                    │   - main.aqi       │
       │                    │   - components.pm2_5│
       │                    │                    │
       │                    │ 4. Vec<TSPoint>    │
       │                    │    [                │
       │                    │      {metric:aqi,  │
       │                    │       value:2},     │
       │                    │      {metric:pm2_5,│
       │                    │       value:8.5}    │
       │                    │    ]               │
       │                    ├───────────────────►│
       │                    │                    │
       │                    │                    │ 5. route
       │                    │                    ├────────┐
       │                    │                    │        ▼
       │                    │                    │    ┌────────┐
       │                    │                    │    │Parquet │
       │                    │                    │    └────────┘
```

---

## 6. ADR-004: Parser System Unification

**Status**: Proposed
**Date**: 2025-12-21
**Context**: Parser architecture decision

### Context

The Neural Data Platform has **two incompatible parser systems**:

1. **Config-driven Parser trait** (`core/src/parsers/`) - Flexible, YAML-configurable
2. **Hardcoded ResponseParser trait** (`core/src/sources/`) - Requires code changes for new sources

This duplication creates:
- **Maintenance burden**: Changes must be made in two places
- **Inflexibility**: Adding new data sources requires Rust code changes
- **Inconsistency**: MqttSource uses Parser, GenericHttpPollingSource uses ResponseParser
- **Complexity**: Developers must understand two different systems

### Decision

**Unify on the config-driven Parser trait system and delete ResponseParser.**

1. **Keep**: `core/src/parsers/` - Parser trait, FlatJsonParser, JsonPathParser
2. **Add**: ArrayIteratorParser for array-wrapped responses
3. **Delete**: `core/src/sources/parsers/` - ResponseParser trait, WeatherParser, AirPollutionParser
4. **Migrate**: GenericHttpPollingSource to use Parser trait
5. **Extend**: ParserConfig with `array_path` and `delegate_parser` fields

### Rationale

The Parser trait is superior because:
- ✅ Already used by MqttSource (proven integration)
- ✅ Config-driven (no code changes for new sources)
- ✅ Takes `&Value` instead of `&str` (more efficient, already parsed)
- ✅ Has `config()` method for introspection
- ✅ Designed for composition (ArrayIteratorParser wraps JsonPathParser)

### Consequences

**Positive:**
- ✅ Single parser system to maintain
- ✅ New data sources via YAML config only
- ✅ Consistent integration across all source types
- ✅ ArrayIteratorParser enables complex API structures
- ✅ Reduced code duplication

**Negative:**
- ⚠️ Breaking change for any external users of ResponseParser (none identified)
- ⚠️ Stream configs must be migrated to use new parser section
- ⚠️ One-time migration effort required

**Risks:**
- ⚠️ Invalid parser configs could cause data loss → **Mitigation**: Config validation at load time
- ⚠️ Performance impact of dynamic parsing → **Mitigation**: Benchmarking shows <1% overhead

### Backward Compatibility

**Phase 1 (Non-breaking)**: Add ArrayIteratorParser, update docs
**Phase 2 (Non-breaking)**: Migrate stream configs to use parser section
**Phase 3 (Non-breaking)**: Update GenericHttpPollingSource to prefer Parser over ResponseParser
**Phase 4 (Breaking)**: Delete ResponseParser and legacy parsers

### Alternatives Considered

**Alternative 1**: Keep both systems, add adapters
❌ Rejected - Perpetuates duplication, increases complexity

**Alternative 2**: Migrate Parser to use ResponseParser interface
❌ Rejected - ResponseParser is inferior (string parsing, no config support)

**Alternative 3**: Create third unified system
❌ Rejected - Over-engineering, Parser trait already solves the problem

---

## 7. Implementation Plan

### 7.1 Phase 1: Add ArrayIteratorParser (Non-breaking)

**Files to Create:**
- `core/src/parsers/array_iterator.rs` - ArrayIteratorParser implementation

**Files to Modify:**
- `core/src/parsers/mod.rs` - Export ArrayIteratorParser
- `core/src/parsers/config.rs` - Add `array_path` and `delegate_parser` fields
- `core/src/parsers/factory.rs` - Handle ArrayIterator parser type

**Tests:**
- Unit tests for array extraction
- Unit tests for delegation to JsonPathParser
- Integration tests with mock API responses

**Validation:**
```bash
cargo test --package neural-core --lib parsers::array_iterator
cargo test --package neural-core --lib parsers::factory
```

### 7.2 Phase 2: Update Stream Configurations (Non-breaking)

**Files to Modify:**
- `config/base/streams/outdoor-weather.yaml` - Add parser config
- `config/base/streams/outdoor-air-quality.yaml` - Use ArrayIteratorParser

**Example Migration:**

**BEFORE** (implicit):
```yaml
sources:
  - source_type: http_poll
    params:
      endpoints:
        - endpoint_id: weather
          url: "https://api.openweathermap.org/..."
```

**AFTER** (explicit):
```yaml
sources:
  - source_type: http_poll
    params:
      endpoints:
        - endpoint_id: weather
          url: "https://api.openweathermap.org/..."
    parser:
      parser_type: json_path
      location_id_field: "name"
      default_location_id: "${OWM_LOCATION_NAME}"
      field_mappings:
        - path: "main.temp"
          metric_name: "temperature"
          unit: "celsius"
```

### 7.3 Phase 3: Migrate GenericHttpPollingSource (Non-breaking)

**Files to Modify:**
- `core/src/sources/http_poll.rs`:
  - Change `parser_registry: Arc<ParserRegistry>` to `parser: Arc<dyn Parser>`
  - Update `poll_endpoint()` to use `parser.parse(&json, timestamp)`
  - Remove `ParserRegistry` and `ResponseParser` trait

**Migration Strategy:**

**BEFORE**:
```rust
pub struct GenericHttpPollingSource {
    config: GenericHttpPollingConfig,
    parser_registry: Arc<ParserRegistry>,  // ← OLD
    // ...
}

async fn poll_endpoint(&self, endpoint: &EndpointConfig)
    -> Result<Vec<TimeSeriesPoint>, PollingError>
{
    let parser = self.parser_registry.get(&endpoint.parser_name)?;
    let body = response.text().await?;
    parser.parse(&body, &endpoint.location_id, timestamp)  // ← OLD
}
```

**AFTER**:
```rust
pub struct GenericHttpPollingSource {
    config: GenericHttpPollingConfig,
    parser: Arc<dyn Parser + Send + Sync>,  // ← NEW
    // ...
}

async fn poll_endpoint(&self, endpoint: &EndpointConfig)
    -> Result<Vec<TimeSeriesPoint>, PollingError>
{
    let json: Value = response.json().await?;
    self.parser.parse(&json, timestamp)  // ← NEW
}
```

**Tests:**
```bash
cargo test --package neural-core --lib sources::http_poll
```

### 7.4 Phase 4: Update SourceManager (Non-breaking)

**Files to Modify:**
- `apps/air-quality-app/src/coordinator/source_manager.rs`:
  - Add `create_parser_from_config()` method
  - Update `spawn_source()` to create Parser from stream config
  - Inject Parser into GenericHttpPollingSource constructor

**Implementation:**
```rust
impl SourceManager {
    fn create_parser_from_config(
        &self,
        source_config: &SourceConfig,
    ) -> Result<Box<dyn Parser + Send + Sync>, SourceManagerError> {
        let parser_config = source_config.params
            .get("parser")
            .ok_or_else(|| SourceManagerError::ConfigError(
                "Missing parser configuration".into()
            ))?;

        let config: ParserConfig = serde_json::from_value(parser_config.clone())
            .map_err(|e| SourceManagerError::ConfigError(
                format!("Invalid parser config: {}", e)
            ))?;

        create_parser_from_config(config)
            .map_err(|e| SourceManagerError::ConfigError(
                format!("Failed to create parser: {}", e)
            ))
    }

    async fn spawn_source(
        &mut self,
        stream_id: &str,
        source_config: &SourceConfig,
    ) -> Result<String, SourceManagerError> {
        let parser = self.create_parser_from_config(source_config)?;

        match source_config.source_type {
            SourceType::HttpPoll => {
                let http_config = self.parse_http_polling_config(stream_id, source_config)?;
                let source = GenericHttpPollingSource::new(http_config, parser)?;
                // ... spawn task
            }
            // ...
        }
    }
}
```

### 7.5 Phase 5: Delete Legacy Code (Breaking)

**Files to Delete:**
- `core/src/sources/parsers/weather.rs`
- `core/src/sources/parsers/air_pollution.rs`
- `core/src/sources/parsers/mod.rs`

**Files to Modify:**
- `core/src/sources/http_poll.rs`:
  - Remove `ResponseParser` trait definition
  - Remove `ParserRegistry` struct
  - Remove `EndpointConfig.parser_name` field

**Files to Modify:**
- `core/src/sources/mod.rs` - Remove `pub use parsers::*;`

**Validation:**
```bash
# Verify no references to deleted code
rg "ResponseParser" --type rust
rg "WeatherParser" --type rust
rg "AirPollutionParser" --type rust
rg "ParserRegistry" --type rust

# Should only find deletions in git history
```

**Final Tests:**
```bash
cargo build --release
cargo test --all
./deploy/pi/deploy.sh sync   # Test config loading
./deploy/pi/deploy.sh start  # Test end-to-end ingestion
```

---

## 8. Migration Sequence Summary

| Phase | Description | Breaking? | Duration |
|-------|-------------|-----------|----------|
| 1 | Add ArrayIteratorParser | ❌ No | 2 hours |
| 2 | Migrate stream configs | ❌ No | 1 hour |
| 3 | Update GenericHttpPollingSource | ❌ No | 2 hours |
| 4 | Update SourceManager | ❌ No | 1 hour |
| 5 | Delete legacy parsers | ✅ Yes | 30 minutes |
| **Total** | | | **~7 hours** |

---

## 9. Files Summary

### Files to CREATE

| Path | Purpose | LOC |
|------|---------|-----|
| `core/src/parsers/array_iterator.rs` | ArrayIteratorParser implementation | ~150 |

### Files to MODIFY

| Path | Changes | LOC Impact |
|------|---------|------------|
| `core/src/parsers/mod.rs` | Export ArrayIteratorParser | +2 |
| `core/src/parsers/config.rs` | Add array_path, delegate_parser fields | +6 |
| `core/src/parsers/factory.rs` | Handle ArrayIterator parser type | +10 |
| `core/src/sources/http_poll.rs` | Replace ParserRegistry with Parser trait | -300 |
| `apps/air-quality-app/src/coordinator/source_manager.rs` | Create Parser from config | +30 |
| `config/base/streams/outdoor-weather.yaml` | Add parser config | +15 |
| `config/base/streams/outdoor-air-quality.yaml` | Use ArrayIteratorParser | +20 |

### Files to DELETE

| Path | Reason | LOC Removed |
|------|--------|-------------|
| `core/src/sources/parsers/weather.rs` | Replaced by JsonPathParser + config | -330 |
| `core/src/sources/parsers/air_pollution.rs` | Replaced by ArrayIteratorParser + config | -250 |
| `core/src/sources/parsers/mod.rs` | No longer needed | -11 |

**Net LOC Change**: ~-650 lines (simplification!)

---

## 10. Testing Strategy

### 10.1 Unit Tests

**ArrayIteratorParser Tests** (`core/src/parsers/array_iterator.rs`):
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_array_iterator_unwraps_first_element() {
        let config = ParserConfig {
            parser_type: ParserType::ArrayIterator,
            array_path: Some("list".to_string()),
            delegate_parser: Some(ParserType::JsonPath),
            field_mappings: Some(vec![
                FieldMapping {
                    path: "main.aqi".to_string(),
                    metric_name: "aqi".to_string(),
                    unit: Some("1-5".to_string()),
                    transform: None,
                }
            ]),
            ..Default::default()
        };

        let parser = ArrayIteratorParser::from_config(config).unwrap();

        let json = serde_json::json!({
            "list": [
                {"main": {"aqi": 2}},
                {"main": {"aqi": 3}}  // Should ignore this
            ]
        });

        let points = parser.parse(&json, Utc::now()).unwrap();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].value, 2.0);
    }

    #[test]
    fn test_array_iterator_errors_on_empty_array() {
        let config = create_test_config();
        let parser = ArrayIteratorParser::from_config(config).unwrap();

        let json = serde_json::json!({"list": []});

        let result = parser.parse(&json, Utc::now());
        assert!(result.is_err());
    }

    #[test]
    fn test_array_iterator_errors_on_missing_path() {
        let config = create_test_config();
        let parser = ArrayIteratorParser::from_config(config).unwrap();

        let json = serde_json::json!({"wrong_field": []});

        let result = parser.parse(&json, Utc::now());
        assert!(result.is_err());
    }
}
```

### 10.2 Integration Tests

**GenericHttpPollingSource with Parser** (`core/src/sources/http_poll.rs`):
```rust
#[tokio::test]
async fn test_http_source_with_json_path_parser() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_json(json!({
                "name": "TestCity",
                "main": {"temp": 22.5, "humidity": 55}
            })))
        .mount(&mock_server)
        .await;

    let parser_config = ParserConfig {
        parser_type: ParserType::JsonPath,
        location_id_field: "name".to_string(),
        field_mappings: Some(vec![
            FieldMapping {
                path: "main.temp".to_string(),
                metric_name: "temperature".to_string(),
                unit: Some("celsius".to_string()),
                transform: None,
            },
        ]),
        ..Default::default()
    };

    let parser = create_parser_from_config(parser_config).unwrap();
    let source = GenericHttpPollingSource::new(http_config, parser).unwrap();

    let points = source.poll_endpoint(&endpoint).await.unwrap();
    assert_eq!(points.len(), 1);
    assert_eq!(points[0].value, 22.5);
}

#[tokio::test]
async fn test_http_source_with_array_iterator_parser() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_json(json!({
                "list": [
                    {"main": {"aqi": 2}, "components": {"pm2_5": 8.5}}
                ]
            })))
        .mount(&mock_server)
        .await;

    let parser_config = ParserConfig {
        parser_type: ParserType::ArrayIterator,
        array_path: Some("list".to_string()),
        delegate_parser: Some(ParserType::JsonPath),
        field_mappings: Some(vec![
            FieldMapping {
                path: "main.aqi".to_string(),
                metric_name: "aqi".to_string(),
                unit: Some("1-5".to_string()),
                transform: None,
            },
        ]),
        ..Default::default()
    };

    let parser = create_parser_from_config(parser_config).unwrap();
    let source = GenericHttpPollingSource::new(http_config, parser).unwrap();

    let points = source.poll_endpoint(&endpoint).await.unwrap();
    assert_eq!(points.len(), 1);
    assert_eq!(points[0].value, 2.0);
}
```

### 10.3 End-to-End Tests

**Config-Driven Stream Test**:
```bash
# 1. Deploy with new configs
./deploy/pi/deploy.sh sync

# 2. Verify stream configs loaded
curl http://localhost:8080/api/streams | jq '.[] | select(.stream_id == "outdoor-air-quality")'

# 3. Check data ingestion
sqlite3 /data/outdoor-air-quality/data.parquet.db \
  "SELECT metric, value FROM outdoor_air_quality ORDER BY timestamp DESC LIMIT 10;"

# 4. Verify parser in use
curl http://localhost:8080/api/streams/outdoor-air-quality/status | \
  jq '.sources[0].parser_type'
# Should output: "array_iterator"
```

---

## 11. Performance Considerations

### 11.1 Parser Creation Overhead

**Benchmark**: Parser instantiation time
```rust
#[bench]
fn bench_parser_creation(b: &mut Bencher) {
    let config = create_array_iterator_config();
    b.iter(|| {
        let _ = create_parser_from_config(config.clone()).unwrap();
    });
}
```

**Expected**: <1ms per parser (created once per stream at startup)

### 11.2 Parsing Latency

**Benchmark**: Parse 1000 messages
```rust
#[bench]
fn bench_array_iterator_parsing(b: &mut Bencher) {
    let parser = create_test_parser();
    let json = create_test_json();
    b.iter(|| {
        let _ = parser.parse(&json, Utc::now()).unwrap();
    });
}
```

**Expected**: <100μs per message (negligible vs network latency)

### 11.3 Memory Overhead

| Component | Memory Usage |
|-----------|-------------|
| Parser instance | ~1KB (config + vtable) |
| ArrayIteratorParser | +1KB (delegate parser) |
| Per-message allocation | ~200 bytes (cloned Value) |

**Total overhead**: <5KB per stream (acceptable)

---

## 12. Success Criteria

### Functional Requirements

- ✅ ArrayIteratorParser extracts first element from arrays
- ✅ ArrayIteratorParser delegates to JsonPathParser correctly
- ✅ GenericHttpPollingSource uses Parser trait
- ✅ Stream configs define parser via YAML
- ✅ OpenWeatherMap current weather ingests via JsonPathParser
- ✅ OpenWeatherMap air pollution ingests via ArrayIteratorParser
- ✅ MqttSource continues using FlatJsonParser (no regression)
- ✅ All existing streams continue working after migration

### Non-Functional Requirements

- ✅ Parser creation < 1ms
- ✅ Parsing latency < 100μs per message
- ✅ Memory overhead < 5KB per stream
- ✅ Config validation fails fast on invalid parser type
- ✅ No data loss during migration

### Testing Requirements

- ✅ Unit tests for ArrayIteratorParser (>90% coverage)
- ✅ Unit tests for ParserFactory (handle all parser types)
- ✅ Integration tests for GenericHttpPollingSource with all parser types
- ✅ End-to-end tests with real API mocks
- ✅ Config validation tests (invalid parser types, missing fields)

---

## 13. Documentation Updates

### Files to Update

| Document | Changes |
|----------|---------|
| `docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md` | Add ArrayIteratorParser to parser system diagram |
| `docs/operations/STREAM_CONFIGURATION.md` | Document parser config section |
| `README.md` | Update "Adding New Data Sources" section |
| `product/features/air-006/completion/COMPLETION.md` | Record final implementation details |

### New Documentation

| Document | Content |
|----------|---------|
| `docs/architecture/PARSER_SYSTEM.md` | Complete parser system architecture |
| `docs/tutorials/ADDING_HTTP_SOURCE.md` | Step-by-step guide with parser config examples |

---

## 14. Risk Assessment

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Invalid parser configs cause data loss | High | Medium | Config validation at load time, fail-fast errors |
| Performance degradation | Medium | Low | Benchmark all parsers, <1% overhead acceptable |
| Breaking changes for users | Medium | Low | Phased migration, backward compatibility period |
| Array unwrap fails on edge cases | Medium | Medium | Comprehensive edge case tests (empty arrays, nested arrays) |
| Config migration errors | Low | Medium | Automated migration script, validation tests |

---

## 15. References

### Existing Documentation
- [BUG-002: Config-Driven Parsing Architecture](/workspaces/neural-data-platform/product/features/dp-001/bugs/BUG-002-CONFIG-DRIVEN-PARSING-ARCH.md)
- [Current HTTP Poll Source](/workspaces/neural-data-platform/core/src/sources/http_poll.rs)
- [Current MQTT Source](/workspaces/neural-data-platform/core/src/sources/mqtt.rs)
- [Parser Trait](/workspaces/neural-data-platform/core/src/parsers/traits.rs)
- [Parser Config](/workspaces/neural-data-platform/core/src/parsers/config.rs)

### External References
- [OpenWeatherMap Current Weather API](https://openweathermap.org/current)
- [OpenWeatherMap Air Pollution API](https://openweathermap.org/api/air-pollution)

---

## 16. Appendix: Configuration Examples

### A. FlatJsonParser (AirGradient MQTT)

```yaml
parser:
  parser_type: flat_json
  location_id_field: "serialno"
  skip_fields:
    - serialno
    - firmware
    - model
  default_tags:
    source: mqtt
    stream_id: air-quality
```

### B. JsonPathParser (OpenWeatherMap Current)

```yaml
parser:
  parser_type: json_path
  location_id_field: "name"
  default_location_id: "San Francisco"
  field_mappings:
    - path: "main.temp"
      metric_name: "temperature"
      unit: "celsius"
    - path: "main.humidity"
      metric_name: "humidity"
      unit: "percent"
    - path: "wind.speed"
      metric_name: "wind_speed"
      unit: "m/s"
  default_tags:
    source: openweathermap
    api: current_weather
```

### C. ArrayIteratorParser (OpenWeatherMap Air Pollution)

```yaml
parser:
  parser_type: array_iterator
  array_path: "list"
  delegate_parser: json_path
  location_id_field: "coord"
  default_location_id: "San Francisco"
  field_mappings:
    - path: "main.aqi"
      metric_name: "aqi"
      unit: "1-5_scale"
    - path: "components.pm2_5"
      metric_name: "pm2_5"
      unit: "ug/m3"
    - path: "components.pm10"
      metric_name: "pm10"
      unit: "ug/m3"
  default_tags:
    source: openweathermap
    api: air_pollution
```

---

**END OF ARCHITECTURE DOCUMENT**

Next Phase: **Refinement** (SPARC R) - Implementation planning and TDD
