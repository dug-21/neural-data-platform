# BUG-002: Config-Driven Parsing Specification

**SPARC Phase:** Specification
**Status:** Draft
**Created:** 2025-12-18
**Feature:** dp-001 (Silver Layer Development)

---

## 1. Problem Statement

### 1.1 Current State (BROKEN)

The NDP ingestion system has **hardcoded parsers** that drop data and prevent the platform from being data-driven:

#### MQTT Parser (`core/src/sources/mqtt.rs`)
- **Currently:** Extracts ALL numeric fields dynamically with ORIGINAL names (GOOD)
- **Status:** ✅ Already implements config-driven approach
- **Example:** Correctly preserves `rco2`, `atmp`, `rhum`, `tvocIndex`, `noxIndex`, `pm01`, `pm10`, etc.

#### HTTP Poll Parser (`core/src/sources/http_poll.rs`)
- **Currently:** Hardcoded `CurrentMeasures` struct with specific fields
- **Status:** ❌ BROKEN - Drops fields not in struct definition
- **Problem:** Cannot ingest new sensor fields without code changes
- **Example:** Would drop `pm01Compensated`, `atmpCompensated`, `rhumCompensated` if sensor adds them

#### OpenWeatherMap Weather Parser (`core/src/sources/parsers/weather.rs`)
- **Currently:** Hardcoded `WeatherResponse` struct with nested structures
- **Status:** ❌ BROKEN - Cannot handle API changes or additional fields
- **Problem:** Tightly coupled to specific JSON structure (`main.temp`, `wind.speed`, etc.)
- **Example:** Cannot extract new fields like `dew_point`, `uvi` without code changes

#### OpenWeatherMap Air Pollution Parser (`core/src/sources/parsers/air_pollution.rs`)
- **Currently:** Hardcoded `AirPollutionResponse` with fixed component list
- **Status:** ❌ BROKEN - Cannot handle new pollutants
- **Problem:** API may add new pollutants (e.g., `pm1`, `bc` - black carbon)
- **Example:** Would drop new pollutant data without code changes

### 1.2 Why This is Broken

1. **Field Loss:** Parsers drop data that doesn't match hardcoded structs
2. **Rigid Schema:** Cannot adapt to API changes without recompiling
3. **Inconsistency:** MQTT is dynamic, HTTP is hardcoded
4. **Violation of Principle:** Bronze layer should preserve ALL raw data
5. **Operational Risk:** Adding a sensor field requires code deployment

### 1.3 Gap Analysis

| Parser | Current Approach | Desired Approach | Gap |
|--------|------------------|------------------|-----|
| MQTT | Dynamic field extraction (✅) | Config-driven field mapping | Add field mapping config |
| HTTP Poll | Hardcoded struct (❌) | Dynamic JSON extraction | Replace struct with `HashMap<String, Value>` |
| Weather API | Hardcoded nested structs (❌) | JSONPath field extraction | Add JSONPath parser |
| Air Pollution API | Hardcoded component list (❌) | Config-driven component mapping | Add field mapping config |

---

## 2. Requirements

### 2.1 Functional Requirements

#### FR-001: Dynamic Field Extraction
**Priority:** HIGH
**Description:** All parsers MUST extract fields dynamically based on configuration, not hardcoded structs.

**Acceptance Criteria:**
- Parser reads field list from YAML config, not Rust structs
- Adding a new field requires ONLY config change, no code change
- Unknown fields are either included (configurable) or logged as warnings

#### FR-002: JSONPath Support for Nested JSON
**Priority:** HIGH
**Description:** HTTP parsers MUST support JSONPath expressions to extract nested values.

**Acceptance Criteria:**
- Config specifies JSONPath like `main.temp`, `wind.speed`, `list[0].components.pm2_5`
- Parser evaluates JSONPath against response body
- Invalid JSONPath results in clear error message

#### FR-003: Field Renaming Configuration
**Priority:** MEDIUM
**Description:** Config MUST support mapping source field names to target field names.

**Acceptance Criteria:**
- Config specifies `source_field` → `target_field` mappings
- Example: `atmp` → `temperature`, `rhum` → `humidity` (for Silver layer)
- Renaming happens AFTER extraction, preserving raw Bronze layer data

#### FR-004: Field Exclusion Configuration
**Priority:** MEDIUM
**Description:** Config MUST support excluding metadata fields from ingestion.

**Acceptance Criteria:**
- Config lists fields to exclude (e.g., `serialno`, `firmware`, `model`)
- Excluded fields are not stored but may be logged for debugging
- Default behavior is include all numeric fields

#### FR-005: Type Coercion Configuration
**Priority:** MEDIUM
**Description:** Config MUST specify expected data types for validation.

**Acceptance Criteria:**
- Config declares field type (float, int, string, boolean)
- Parser validates extracted value matches expected type
- Type mismatches are logged and value is either coerced or dropped

#### FR-006: Unit Configuration
**Priority:** LOW
**Description:** Config SHOULD specify units for each field for metadata.

**Acceptance Criteria:**
- Config includes `unit` field (e.g., "celsius", "µg/m³", "ppm")
- Unit is added to TimeSeriesPoint tags
- Unit is purely metadata, does not affect parsing

### 2.2 Non-Functional Requirements

#### NFR-001: Backward Compatibility
**Category:** Compatibility
**Description:** Existing streams MUST continue working without modification.

**Measurement:**
- All existing integration tests pass
- air-quality stream ingests same fields as before
- outdoor-weather and outdoor-air-quality streams work unchanged

#### NFR-002: Performance
**Category:** Performance
**Description:** Config-driven parsing MUST NOT degrade throughput by >5%.

**Measurement:**
- Benchmark current MQTT ingestion: ~1000 msgs/sec
- Benchmark new HTTP dynamic parsing: >950 msgs/sec
- Parsing latency: <1ms per message p95

#### NFR-003: Error Handling
**Category:** Reliability
**Description:** Invalid config MUST be detected at startup, not runtime.

**Measurement:**
- Config validation runs on app start
- Invalid JSONPath expressions fail fast
- Clear error messages with line numbers

#### NFR-004: Observability
**Category:** Monitoring
**Description:** Parsing metrics MUST be exposed for monitoring.

**Measurement:**
- Counter: fields_extracted_total (by stream, field)
- Counter: fields_dropped_total (by stream, reason)
- Histogram: parsing_duration_seconds

---

## 3. Config Schema Design

### 3.1 Enhanced Stream Config Schema

```yaml
stream_id: "air-quality"
description: "AirGradient sensor readings from MQTT"
version: "1.0.0"
enabled: true

# Field schema with parsing configuration
fields:
  pm25:
    # Bronze layer (ingestion)
    source_path: "pm02"           # JSONPath or field name
    source_type: "float"          # Type validation
    nullable: false

    # Silver layer (transformation)
    target_name: "pm25"           # Renamed field (optional)
    target_type: "float"
    unit: "µg/m³"
    description: "Particulate Matter 2.5 micrometers"

    # Validation
    range: [0.0, 1000.0]          # Min/max bounds

  temperature:
    source_path: "atmp"           # Raw sensor name
    source_type: "float"
    target_name: "temperature"    # Friendly name
    unit: "celsius"
    nullable: true
    range: [-50.0, 60.0]

# Parser configuration
parser:
  type: "json"                    # json, csv, protobuf, etc.
  mode: "dynamic"                 # dynamic (extract all) or strict (only defined fields)

  # Field inclusion/exclusion
  include_undefined: false        # Include fields not in schema?
  exclude_fields:                 # Never extract these
    - "serialno"                  # Metadata only
    - "firmware"
    - "model"
    - "ledMode"

  # Nested JSON handling (for HTTP APIs)
  json_path_root: "$"             # Root path (default: "$")
  flatten_nested: true            # Flatten nested objects?
  array_handling: "first"         # first, last, all, index:N

# Source configuration
sources:
  - source_type: mqtt
    enabled: true
    parser_config:                # Source-specific parser config
      preserve_raw_names: true    # Keep original field names in Bronze
    params:
      broker_url: "mosquitto"
      port: 1883
      topic_pattern: "airgradient/readings/+"
```

### 3.2 Nested JSON Config (OpenWeatherMap Weather)

```yaml
stream_id: "outdoor-weather"
description: "OpenWeatherMap Current Weather API"
version: "1.0.0"
enabled: true

fields:
  temperature:
    source_path: "$.main.temp"         # JSONPath for nested field
    source_type: "float"
    target_name: "temperature"
    unit: "celsius"
    nullable: false
    range: [-50.0, 60.0]

  feels_like:
    source_path: "$.main.feels_like"
    source_type: "float"
    unit: "celsius"
    nullable: true

  wind_speed:
    source_path: "$.wind.speed"
    source_type: "float"
    unit: "m/s"
    nullable: true

  wind_gust:
    source_path: "$.wind.gust"         # Optional field
    source_type: "float"
    unit: "m/s"
    nullable: true

  rain_1h:
    source_path: "$.rain.1h"           # Handles special characters
    source_type: "float"
    unit: "mm"
    nullable: true
    default: 0.0                       # Default if missing

parser:
  type: "json"
  mode: "strict"                       # Only extract defined fields
  json_path_root: "$"

sources:
  - type: http_poll
    enabled: true
    parser_config:
      response_format: "json"
      root_path: "$"                   # Top-level object
    endpoints:
      - endpoint_id: openweathermap_weather
        url: "https://api.openweathermap.org/data/2.5/weather?lat=29.95838&lon=-81.30878&units=metric"
        auth_type: query_param
        auth_key: appid
        auth_value: "${OPENWEATHERMAP_API_KEY}"
```

### 3.3 Array Handling Config (OpenWeatherMap Air Pollution)

```yaml
stream_id: "outdoor-air-quality"
description: "OpenWeatherMap Air Pollution API"
version: "1.0.0"
enabled: true

fields:
  aqi:
    source_path: "$.list[0].main.aqi"  # Array indexing
    source_type: "int"
    target_type: "float"               # Type conversion
    unit: "1-5_scale"
    nullable: false
    range: [1.0, 5.0]

  pm2_5:
    source_path: "$.list[0].components.pm2_5"
    source_type: "float"
    unit: "µg/m³"
    nullable: false

  pm10:
    source_path: "$.list[0].components.pm10"
    source_type: "float"
    unit: "µg/m³"
    nullable: true

parser:
  type: "json"
  mode: "strict"
  json_path_root: "$"
  array_handling: "first"              # Use first element of list array

  # Fallback for dynamic component extraction
  dynamic_paths:
    - path: "$.list[0].components.*"   # Extract all components
      prefix: "pollutant_"             # Prefix for unknown components
      include: true
```

---

## 4. Architecture Changes

### 4.1 Config-Driven Parser Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Stream Config (YAML)                     │
│  - field definitions (source_path, target_name, type)        │
│  - parser config (mode, exclusions, JSONPath)                │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    ParserConfigLoader                        │
│  - Validates config at startup                               │
│  - Compiles JSONPath expressions                             │
│  - Creates FieldExtractor instances                          │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    DynamicJsonParser                         │
│  - parse(json: Value) → Vec<TimeSeriesPoint>                 │
│  - Uses FieldExtractor per field                             │
│  - Handles nested paths, arrays, type coercion               │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                     FieldExtractor                           │
│  - extract(json: &Value) → Option<f64>                       │
│  - JSONPath evaluation (jsonpath_lib crate)                  │
│  - Type validation and coercion                              │
│  - Range validation                                          │
└─────────────────────────────────────────────────────────────┘
```

### 4.2 Component Interactions

```
┌────────────┐        ┌──────────────────┐       ┌─────────────┐
│ MQTT/HTTP  │───────▶│ DynamicJsonParser│──────▶│ Bronze      │
│ Raw JSON   │        │ (config-driven)  │       │ Parquet     │
└────────────┘        └──────────────────┘       └─────────────┘
                             │
                             │ Uses
                             ▼
                      ┌──────────────┐
                      │ StreamConfig │
                      │ (fields,     │
                      │  parser)     │
                      └──────────────┘
```

---

## 5. Implementation Changes

### 5.1 New Rust Modules

#### `core/src/config/field_config.rs`
```rust
pub struct FieldConfig {
    pub source_path: String,        // JSONPath or field name
    pub source_type: FieldType,     // float, int, string, bool
    pub target_name: Option<String>,
    pub unit: Option<String>,
    pub nullable: bool,
    pub range: Option<(f64, f64)>,
    pub default: Option<serde_json::Value>,
}

pub enum FieldType {
    Float,
    Int,
    String,
    Boolean,
}
```

#### `core/src/config/parser_config.rs`
```rust
pub struct ParserConfig {
    pub parser_type: ParserType,    // json, csv, protobuf
    pub mode: ParserMode,            // dynamic, strict
    pub include_undefined: bool,
    pub exclude_fields: Vec<String>,
    pub json_path_root: String,
    pub array_handling: ArrayHandling,
}

pub enum ParserMode {
    Dynamic,  // Extract all fields
    Strict,   // Only extract defined fields
}

pub enum ArrayHandling {
    First,
    Last,
    All,
    Index(usize),
}
```

#### `core/src/parsers/dynamic_json.rs`
```rust
pub struct DynamicJsonParser {
    stream_id: String,
    fields: HashMap<String, FieldExtractor>,
    config: ParserConfig,
}

impl DynamicJsonParser {
    pub fn from_config(stream_config: &StreamConfig) -> CoreResult<Self> {
        // Build FieldExtractor for each field
        // Compile JSONPath expressions
        // Validate config
    }

    pub fn parse(&self, json: &Value, location_id: &str) -> CoreResult<Vec<TimeSeriesPoint>> {
        // Extract fields based on config
        // Handle dynamic vs strict mode
        // Apply exclusions
    }
}
```

#### `core/src/parsers/field_extractor.rs`
```rust
pub struct FieldExtractor {
    source_path: CompiledJsonPath,
    source_type: FieldType,
    target_name: String,
    validator: FieldValidator,
}

impl FieldExtractor {
    pub fn extract(&self, json: &Value) -> CoreResult<Option<f64>> {
        // Evaluate JSONPath
        // Validate type
        // Coerce to f64
        // Validate range
    }
}
```

### 5.2 Migration Path

#### Phase 1: Add Config Schema
1. Extend `StreamConfig` struct with `fields` and `parser` sections
2. Update YAML parser to read new fields
3. Validate config at app startup

#### Phase 2: Implement Dynamic Parser
1. Create `DynamicJsonParser` for JSON responses
2. Implement JSONPath evaluation (use `jsonpath_lib` crate)
3. Add type coercion and validation

#### Phase 3: Migrate MQTT Parser
1. Replace hardcoded skip_fields with config
2. Add field mapping support (atmp → temperature)
3. Keep dynamic extraction behavior

#### Phase 4: Migrate HTTP Parser
1. Replace `CurrentMeasures` struct with `HashMap<String, Value>`
2. Use `DynamicJsonParser` for field extraction
3. Add tests for dynamic field discovery

#### Phase 5: Migrate Weather Parser
1. Replace `WeatherResponse` structs with JSONPath config
2. Update config with nested paths (`$.main.temp`)
3. Handle optional fields (gust, rain, snow)

#### Phase 6: Migrate Air Pollution Parser
1. Replace `PollutionComponents` struct with config
2. Add array handling for `list[0]`
3. Support dynamic component discovery

---

## 6. Acceptance Criteria

### 6.1 Configuration Acceptance

- [ ] **AC-001:** YAML config includes `fields` section with `source_path`, `source_type`, `target_name`
- [ ] **AC-002:** YAML config includes `parser` section with `mode`, `exclude_fields`
- [ ] **AC-003:** Invalid JSONPath syntax fails at app startup with clear error
- [ ] **AC-004:** Missing required fields in config fail validation
- [ ] **AC-005:** Config supports nested JSONPath (e.g., `$.main.temp`)
- [ ] **AC-006:** Config supports array indexing (e.g., `$.list[0].components.pm2_5`)

### 6.2 Parsing Acceptance

- [ ] **AC-007:** MQTT parser extracts fields based on config, not hardcoded struct
- [ ] **AC-008:** HTTP parser extracts fields using JSONPath expressions
- [ ] **AC-009:** Unknown fields in "dynamic" mode are included with warning
- [ ] **AC-010:** Unknown fields in "strict" mode are logged and dropped
- [ ] **AC-011:** Excluded fields (serialno, firmware) are never ingested
- [ ] **AC-012:** Field renaming (atmp → temperature) happens correctly

### 6.3 Type Validation Acceptance

- [ ] **AC-013:** Float fields coerce integers (e.g., 42 → 42.0)
- [ ] **AC-014:** String fields that look numeric are rejected
- [ ] **AC-015:** Out-of-range values are logged and dropped
- [ ] **AC-016:** Nullable fields handle missing values gracefully
- [ ] **AC-017:** Non-nullable fields error if missing

### 6.4 Backward Compatibility Acceptance

- [ ] **AC-018:** Existing air-quality stream config works unchanged
- [ ] **AC-019:** Existing outdoor-weather stream config works unchanged
- [ ] **AC-020:** All existing integration tests pass
- [ ] **AC-021:** Bronze Parquet files contain same fields as before
- [ ] **AC-022:** Grafana dashboards continue to work

### 6.5 Performance Acceptance

- [ ] **AC-023:** MQTT ingestion throughput >950 msgs/sec (95% of baseline)
- [ ] **AC-024:** HTTP parsing latency <1ms p95
- [ ] **AC-025:** Config parsing overhead <10ms at startup
- [ ] **AC-026:** Memory usage increase <5%

### 6.6 Observability Acceptance

- [ ] **AC-027:** Metric `fields_extracted_total{stream, field}` exposed
- [ ] **AC-028:** Metric `fields_dropped_total{stream, reason}` exposed
- [ ] **AC-029:** Histogram `parsing_duration_seconds` exposed
- [ ] **AC-030:** Logs include field name, source path, and value on extraction

---

## 7. Constraints

### 7.1 Technical Constraints

1. **JSONPath Library:** Use `jsonpath_lib` or `serde_json_path` crate (battle-tested)
2. **Rust Type System:** TimeSeriesPoint.value is f64, all fields must coerce to float
3. **Parquet Schema:** Bronze layer schema is fixed (timestamp, location_id, value, tags)
4. **No Breaking Changes:** Existing deployments must work without config migration

### 7.2 Performance Constraints

1. **Latency:** Parsing must complete in <1ms p95 (single message)
2. **Throughput:** Must handle 1000+ messages/sec per stream
3. **Memory:** No unbounded growth from dynamic field discovery
4. **Startup Time:** Config validation must complete in <100ms

### 7.3 Operational Constraints

1. **Config Validation:** All errors must be caught at startup, not during ingestion
2. **Rollback Safety:** Invalid config must not crash running application
3. **Metrics:** All parsing failures must be observable via Prometheus metrics
4. **Documentation:** Config schema must be documented with examples

### 7.4 Compatibility Constraints

1. **YAML Version:** Config must work with current serde_yaml version
2. **Rust Version:** Must compile on Rust 1.70+ (current MSRV)
3. **No Schema Migration:** Bronze Parquet files remain unchanged
4. **etcd Compatibility:** Config must sync via existing etcd mechanism

---

## 8. Out of Scope

The following are explicitly OUT OF SCOPE for this bug fix:

1. **Silver Layer Transformation:** Field renaming happens in ETL, not ingestion
2. **Schema Evolution:** Parquet schema changes are separate effort
3. **Protocol Support:** Only JSON parsing, not CSV/Protobuf/etc.
4. **Field Derivation:** Computed fields (e.g., heat_index) are Silver layer concern
5. **Unit Conversion:** Converting celsius to fahrenheit is Silver layer concern
6. **Data Quality Rules:** Complex validation (e.g., pm25 < pm10) is separate
7. **Webhook/FileWatch:** Only MQTT and HTTP Poll sources in scope

---

## 9. Success Metrics

### 9.1 Development Success

- **Config Coverage:** 100% of fields defined in YAML config
- **Test Coverage:** >95% line coverage for DynamicJsonParser
- **Documentation:** Config schema documented with examples
- **Migration:** All 3 parsers migrated to config-driven approach

### 9.2 Operational Success

- **Zero Data Loss:** All numeric fields extracted from sensor payloads
- **Zero Downtime:** Deployment requires no service restart
- **Fast Rollback:** Config rollback completes in <30 seconds
- **Observability:** All parsing metrics available in Grafana

### 9.3 User Success

- **Operator Efficiency:** Adding new field takes <5 minutes (config edit)
- **Developer Efficiency:** No code changes for new sensor fields
- **System Reliability:** Parsing errors visible in metrics dashboard
- **Data Quality:** Field validation prevents bad data ingestion

---

## 10. Testing Strategy

### 10.1 Unit Tests

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_dynamic_parser_extracts_all_fields() {
        // Config with 5 fields
        // JSON with 7 fields (2 extra)
        // Assert: 5 fields extracted in strict mode
        // Assert: 7 fields extracted in dynamic mode
    }

    #[test]
    fn test_jsonpath_nested_extraction() {
        // Config: source_path = "$.main.temp"
        // JSON: {"main": {"temp": 20.5}}
        // Assert: value == 20.5
    }

    #[test]
    fn test_field_exclusion() {
        // Config: exclude_fields = ["serialno", "firmware"]
        // JSON: {"serialno": "ABC", "pm25": 12.5}
        // Assert: only pm25 extracted
    }

    #[test]
    fn test_type_coercion() {
        // Config: source_type = float
        // JSON: {"temp": 42}  (integer)
        // Assert: value == 42.0
    }

    #[test]
    fn test_range_validation() {
        // Config: range = [0.0, 100.0]
        // JSON: {"value": 150.0}
        // Assert: error or dropped
    }
}
```

### 10.2 Integration Tests

```rust
#[tokio::test]
async fn test_mqtt_config_driven_parsing() {
    // Load air-quality config
    // Parse real AirGradient payload
    // Assert: all fields extracted
    // Assert: excluded fields dropped
}

#[tokio::test]
async fn test_http_weather_jsonpath_parsing() {
    // Load outdoor-weather config
    // Parse real OpenWeatherMap response
    // Assert: nested fields extracted via JSONPath
    // Assert: optional fields handled correctly
}

#[tokio::test]
async fn test_http_air_pollution_array_handling() {
    // Load outdoor-air-quality config
    // Parse real Air Pollution API response
    // Assert: list[0] extracted correctly
    // Assert: all components extracted
}
```

### 10.3 End-to-End Tests

```bash
# Deploy with new config
./deploy/pi/deploy.sh sync

# Verify Bronze Parquet
duckdb bronze.db "SELECT DISTINCT metric FROM air_quality"
# Expected: pm01, pm02, pm10, rco2, atmp, rhum, tvocIndex, noxIndex, ...

# Verify metrics
curl localhost:9090/metrics | grep fields_extracted_total

# Verify Grafana dashboards still work
curl -s "http://grafana:3000/api/dashboards/uid/air-quality" | jq '.dashboard.panels'
```

---

## 11. Risks and Mitigations

### 11.1 Technical Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| JSONPath library performance | HIGH | LOW | Benchmark against baseline, cache compiled paths |
| Type coercion edge cases | MEDIUM | MEDIUM | Comprehensive test suite, strict validation |
| Config parsing overhead | LOW | LOW | Validate at startup, not per message |
| Memory leak from dynamic fields | HIGH | LOW | Limit field count, monitor memory metrics |

### 11.2 Operational Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Invalid config deployed | HIGH | MEDIUM | Config validation in CI/CD pipeline |
| Breaking change to existing streams | CRITICAL | LOW | Extensive backward compatibility tests |
| Performance regression | HIGH | LOW | Load testing before deployment |
| Field name collision | MEDIUM | LOW | Validate uniqueness in config |

### 11.3 Data Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Data loss from misconfigured exclusions | CRITICAL | MEDIUM | Default to include all fields, explicit exclusions |
| Incorrect field mapping | HIGH | MEDIUM | Validate mappings in tests, log raw values |
| Type mismatch causing data drop | MEDIUM | MEDIUM | Metrics on dropped fields, alerts on high drop rate |

---

## 12. Dependencies

### 12.1 External Dependencies

- **jsonpath_lib** or **serde_json_path:** JSONPath evaluation
  - Recommendation: `serde_json_path` (better maintained)
  - Version: 0.6+
  - License: MIT/Apache-2.0

### 12.2 Internal Dependencies

- **StreamConfig:** Must be extended with `fields` and `parser` sections
- **TimeSeriesPoint:** No changes (already supports arbitrary tags)
- **Bronze Storage:** No changes (schema is fixed)
- **etcd Sync:** Config changes must sync via existing mechanism

---

## 13. Documentation Requirements

### 13.1 User Documentation

- **Config Schema Reference:** Document all field config options
- **JSONPath Guide:** Examples for common patterns (nested, arrays)
- **Migration Guide:** How to migrate existing streams to new config
- **Troubleshooting:** Common config errors and solutions

### 13.2 Developer Documentation

- **Architecture Decision Record:** Why config-driven vs hardcoded
- **Parser Implementation:** How to add new parser types
- **Testing Guide:** How to test config changes
- **Performance Tuning:** Optimization tips for large payloads

---

## 14. Next Steps (Post-Specification)

After approval of this specification:

1. **Pseudocode Phase:** Design DynamicJsonParser algorithm
2. **Architecture Phase:** Create ADR for JSONPath library choice
3. **Refinement Phase:** TDD implementation with tests
4. **Completion Phase:** Deploy to staging, verify metrics

---

## Appendix A: Example Configs

### A.1 MQTT Dynamic Mode (Extract All)

```yaml
stream_id: "air-quality"
fields:
  # Define known fields for validation
  pm25:
    source_path: "pm02"
    source_type: "float"
    unit: "µg/m³"

parser:
  type: "json"
  mode: "dynamic"           # Extract ALL numeric fields
  include_undefined: true   # Include fields not in schema
  exclude_fields:
    - "serialno"
    - "firmware"
```

### A.2 HTTP Strict Mode (Only Defined Fields)

```yaml
stream_id: "outdoor-weather"
fields:
  temperature:
    source_path: "$.main.temp"
    source_type: "float"
    unit: "celsius"

parser:
  type: "json"
  mode: "strict"            # Only extract defined fields
  json_path_root: "$"
```

### A.3 Array Handling with Fallback

```yaml
stream_id: "outdoor-air-quality"
fields:
  aqi:
    source_path: "$.list[0].main.aqi"
    source_type: "int"

parser:
  type: "json"
  mode: "strict"
  array_handling: "first"

  # Fallback: extract unknown components
  dynamic_paths:
    - path: "$.list[0].components.*"
      include: true
```

---

## Appendix B: Performance Benchmarks

### B.1 Current Performance (Baseline)

```
MQTT Parser:
- Throughput: 1000 msgs/sec
- Latency p95: 0.5ms
- Memory: 10MB RSS

HTTP Parser:
- Throughput: 500 req/sec
- Latency p95: 0.8ms
- Memory: 12MB RSS
```

### B.2 Target Performance

```
Dynamic JSON Parser:
- Throughput: >950 msgs/sec (95% of baseline)
- Latency p95: <1ms
- Memory: <15MB RSS (50% increase acceptable)
```

---

## Revision History

| Date | Version | Author | Changes |
|------|---------|--------|---------|
| 2025-12-18 | 0.1 | Claude (SPARC Spec Agent) | Initial specification draft |

---

**Status:** Ready for Review
**Next Phase:** Pseudocode
**Assigned To:** ndp-rust-dev, ndp-architect
