# AIR-006: Complete Config-Driven Stream Implementation

**Feature ID**: air-006
**Phase**: Air Quality Monitoring (Final Bronze Layer Enhancement)
**Status**: Specification
**Created**: 2025-12-21
**Author**: NDP Scrum Master

---

## Executive Summary

Complete the config-driven parser implementation started in BUG-002 by wiring the Parser trait into GenericHttpPollingSource and MqttSource, adding critical enhancements for array iteration, response timestamp extraction, response metadata tags, string value parsing, and enum mapping. Migrate ALL existing streams (air-quality, outdoor-weather, outdoor-air-quality) to the new config-driven system and add NWS observations and forecasts as the first fully config-driven streams.

This feature **completes the Bronze layer vision**: Zero Rust code changes required to add new HTTP/MQTT streams.

---

## Business Context

### Current State (Broken)

BUG-002 designed the config-driven parser architecture but **did not wire it into sources**:

| Component | Status | Problem |
|-----------|--------|---------|
| Parser Trait | ✅ Designed | Not integrated into sources |
| FlatJsonParser | ✅ Designed | Not used by MqttSource |
| JsonPathParser | ✅ Designed | Not used by GenericHttpPollingSource |
| NWS Integration | ❌ Blocked | Cannot add without config-driven parsers |

**Additional Gaps Discovered**:

| Feature | Status | Impact |
|---------|--------|--------|
| Array iteration | ❌ Missing | Cannot process NWS forecast periods array |
| Response timestamp extraction | ❌ Missing | Using poll time instead of observation time |
| Response metadata tags | ❌ Missing | Cannot track forecast `generatedAt` |
| String value parsing | ❌ Missing | Cannot extract "10 mph" → 10.0 |
| Enum mapping | ❌ Missing | Cannot convert "NE" → 45 degrees |

### Desired Outcome

- **Complete BUG-002**: Wire Parser trait into all sources
- **Add Array Iteration**: Process NWS forecast periods without code changes
- **Extract Response Timestamps**: Use observation time, not poll time
- **Capture Response Metadata**: Store forecast generation time as tags
- **Parse String Values**: Extract numeric values from formatted strings
- **Map Enums**: Convert categorical values to numeric (direction → degrees)
- **Migrate Existing Streams**: Zero changes to data output, all config-driven
- **Add NWS Streams**: First fully config-driven implementation

---

## Scope

### In Scope

#### 1. Complete BUG-002 Implementation

**Wire Parser Trait into Sources**:

| Source | Current Parser | New Parser | Change Required |
|--------|---------------|------------|-----------------|
| MqttSource | Hardcoded JSON extraction | FlatJsonParser (injected) | Add `new_with_parser()` constructor |
| GenericHttpPollingSource | No parser (direct JSON) | JsonPathParser (injected) | Add parser parameter, call `parser.parse()` |

**Integration with SourceManager**:

```rust
// apps/air-quality-app/src/coordinator/source_manager.rs

impl SourceManager {
    fn create_parser_from_config(&self, source_config: &SourceConfig) -> Box<dyn Parser> {
        // Read parser config from YAML
        // Call ParserFactory::create()
        // Return configured parser
    }

    async fn spawn_source(&mut self, stream_id: &str, source_config: &SourceConfig) {
        let parser = self.create_parser_from_config(source_config)?;

        match source_config.source_type {
            SourceType::Mqtt => {
                let source = MqttSource::new_with_parser(config, parser);
            }
            SourceType::HttpPoll => {
                let source = GenericHttpPollingSource::new_with_parser(config, parser)?;
            }
        }
    }
}
```

#### 2. Add Array Iteration Support

**New Config Feature**: `array_path` for iterating over JSON arrays

**Use Case**: NWS forecast API returns array of forecast periods:

```json
{
  "generatedAt": "2025-12-21T10:15:00Z",
  "periods": [
    {
      "number": 1,
      "startTime": "2025-12-21T11:00:00-05:00",
      "temperature": 72,
      "windSpeed": "10 mph",
      "windDirection": "NE"
    },
    {
      "number": 2,
      "startTime": "2025-12-21T12:00:00-05:00",
      "temperature": 74,
      "windSpeed": "12 mph",
      "windDirection": "E"
    }
  ]
}
```

**Config Example**:

```yaml
parser:
  parser_type: json_path

  # NEW: Iterate over array instead of single object
  array_path: "periods"

  # Extract timestamp from EACH array element
  timestamp_field: "startTime"
  timestamp_format: iso8601

  # Extract metadata from ROOT object (not array elements)
  root_tags:
    - path: "generatedAt"
      tag_name: "forecast_generated_at"

  field_mappings:
    - path: "temperature"
      metric_name: "temperature"
      unit: "fahrenheit"

    - path: "windSpeed"
      metric_name: "wind_speed"
      unit: "mph"
      value_parser: "regex"
      value_pattern: "^(\\d+)\\s+mph$"  # Extract "10" from "10 mph"

    - path: "windDirection"
      metric_name: "wind_direction"
      unit: "degrees"
      value_mapper: "cardinal_to_degrees"  # "NE" → 45
```

**Parser Behavior**:

1. Extract `periods` array from root
2. Iterate over each element in `periods`
3. For each element:
   - Extract timestamp from `startTime` field
   - Extract `generatedAt` from ROOT and add to tags
   - Extract temperature → numeric value
   - Extract wind_speed → parse "10 mph" → 10.0
   - Extract wind_direction → map "NE" → 45.0
   - Create TimeSeriesPoint with extracted data
4. Return Vec<TimeSeriesPoint> (one per array element)

**Result**: Poll NWS API once, get 156 TimeSeriesPoints (one per forecast hour)

#### 3. Add Response Timestamp Extraction

**Problem**: Currently using poll timestamp for ALL records, but observations have authoritative observation times.

**Solution**: Extract timestamp from response instead of using poll time.

**Config Example**:

```yaml
# NWS Observations
parser:
  parser_type: json_path

  # NEW: Extract timestamp from response
  timestamp_field: "properties.timestamp"
  timestamp_format: iso8601

  # OPTIONAL: Also store poll time as tag
  default_tags:
    poll_timestamp: "${POLL_TIME}"  # Variable substitution

  field_mappings:
    - path: "properties.temperature.value"
      metric_name: "temperature"
```

**Behavior**:

- If `timestamp_field` configured: Parse from response, use as TimeSeriesPoint.timestamp
- If `timestamp_field` missing: Fall back to poll timestamp (backward compatible)
- If parsing fails: Log error, fall back to poll timestamp

**OpenWeatherMap Migration**:

```yaml
# outdoor-weather.yaml (BEFORE)
# Uses poll time implicitly

# outdoor-weather.yaml (AFTER)
parser:
  timestamp_field: "dt"  # Extract from response
  timestamp_format: unix_epoch
```

#### 4. Add Response Metadata Tags

**Problem**: Cannot track forecast issue time or other response-level metadata.

**Solution**: Extract fields from response root and add as tags to ALL TimeSeriesPoints.

**Config Example**:

```yaml
parser:
  parser_type: json_path
  array_path: "periods"

  # NEW: Extract from root and add as tags
  root_tags:
    - path: "generatedAt"
      tag_name: "forecast_generated_at"

    - path: "updateTime"
      tag_name: "forecast_updated_at"

    - path: "gridId"
      tag_name: "grid_id"
```

**Behavior**:

- Before iterating array, extract root_tags from response
- Add root_tags to EVERY TimeSeriesPoint created from array elements
- Enables queries like: "Show all forecasts issued at 2025-12-21T10:00:00Z"

#### 5. Add String Value Parsing

**Problem**: NWS returns values like "10 mph", "5 to 10 mph", "N/A"

**Solution**: Regex extraction for numeric values from formatted strings.

**Config Examples**:

```yaml
field_mappings:
  # Simple numeric extraction
  - path: "windSpeed"
    metric_name: "wind_speed"
    value_parser: "regex"
    value_pattern: "^(\\d+)\\s+mph$"  # "10 mph" → 10.0

  # Range extraction (use first value)
  - path: "windSpeed"
    metric_name: "wind_speed"
    value_parser: "regex"
    value_pattern: "^(\\d+)\\s+to\\s+\\d+\\s+mph$"  # "5 to 10 mph" → 5.0

  # Handle null strings
  - path: "windGust"
    metric_name: "wind_gust"
    value_parser: "regex"
    value_pattern: "^(\\d+)\\s+mph$"
    nullable: true  # "N/A" → None
```

**Supported Parsers**:

| Parser Type | Description | Example |
|-------------|-------------|---------|
| `regex` | Extract numeric value using regex capture group | "10 mph" → 10.0 |
| `split` | Split string and extract value | "10,20,30" → 10.0 (first element) |
| `json` | Parse nested JSON string | '{"value":10}' → 10.0 |

#### 6. Add Enum Mapping

**Problem**: NWS returns categorical values like "NE", "Sunny", "Partly Cloudy"

**Solution**: Config-driven enum mapping to numeric values.

**Config Example**:

```yaml
field_mappings:
  # Cardinal direction → degrees
  - path: "windDirection"
    metric_name: "wind_direction"
    unit: "degrees"
    value_mapper: "cardinal_to_degrees"

  # Custom enum mapping
  - path: "weatherCondition"
    metric_name: "weather_code"
    value_mapper: "custom"
    value_map:
      "Clear": 0.0
      "Partly Cloudy": 1.0
      "Cloudy": 2.0
      "Rainy": 3.0
      "Stormy": 4.0
```

**Built-in Mappers**:

| Mapper | Input | Output |
|--------|-------|--------|
| `cardinal_to_degrees` | "N", "NE", "E", "SE", "S", "SW", "W", "NW" | 0, 45, 90, 135, 180, 225, 270, 315 |
| `custom` | Config-defined mapping | User-defined values |

#### 7. Migrate Existing Streams

**Critical Requirement**: ALL existing streams MUST produce IDENTICAL output after migration.

| Stream | Current Parser | New Config | Verification |
|--------|---------------|------------|--------------|
| air-quality (MQTT) | Hardcoded dynamic extraction | FlatJsonParser | Compare Parquet field counts |
| air-quality (HTTP) | Hardcoded CurrentMeasures | FlatJsonParser | Compare field names |
| outdoor-weather | Hardcoded WeatherParser | JsonPathParser | Compare temperature values |
| outdoor-air-quality | Hardcoded AirPollutionParser | JsonPathParser | Compare pm2_5 values |

**Migration Strategy**:

1. Create new parser configs in YAML
2. Test with sample responses
3. Deploy with parser injection
4. Verify data matches (compare last 24h before/after)
5. Delete legacy parser code

#### 8. Add NWS Streams

**First Fully Config-Driven Streams**:

| Stream ID | Description | Array Iteration | String Parsing | Enum Mapping |
|-----------|-------------|-----------------|----------------|--------------|
| `nws-observations` | Current weather from KSGJ | No | No | No |
| `nws-forecast-hourly` | Hourly forecast for JAX/79,49 | Yes (periods) | Yes (windSpeed) | Yes (windDirection) |

**Config Files**:

```
config/base/streams/
├── nws-observations.yaml          # Response timestamp extraction
└── nws-forecast-hourly.yaml       # Array iteration + string parsing + enum mapping
```

### Out of Scope

#### Silver Layer

| Feature | Reason |
|---------|--------|
| TimescaleDB schema | Separate feature (dp-001) |
| Bronze → Silver ETL | Separate feature (dp-002) |
| Forecast verification queries | Requires Silver layer |
| Continuous aggregates | Requires TimescaleDB |

#### Data Transformations

| Feature | Reason |
|---------|--------|
| Unit conversion (F→C) | Done in parser config (in scope as config) |
| Field renaming | Done in parser config field_mappings |
| Computed fields (heat_index) | Silver layer transformation |
| Data quality scoring | Future fe-xxx feature |

#### Additional Data Sources

| Feature | Reason |
|---------|--------|
| Multiple NWS stations | Future enhancement |
| NWS Alerts/Warnings | Alerts phase (al-xxx) |
| Radar data | Complex binary format, out of scope |
| Marine forecasts | Not needed for air quality |

---

## Success Criteria

### BUG-002 Completion

- [ ] **SC-001**: Parser trait integrated into MqttSource
- [ ] **SC-002**: Parser trait integrated into GenericHttpPollingSource
- [ ] **SC-003**: SourceManager creates parsers from YAML config
- [ ] **SC-004**: Parsers injected via constructor, not hardcoded
- [ ] **SC-005**: All existing tests pass with new parser system

### Array Iteration

- [ ] **SC-006**: `array_path` config extracts array from response
- [ ] **SC-007**: Parser iterates over array elements
- [ ] **SC-008**: One TimeSeriesPoint created per array element
- [ ] **SC-009**: NWS forecast API returns 156 points per poll

### Response Timestamp Extraction

- [ ] **SC-010**: `timestamp_field` config extracts timestamp from response
- [ ] **SC-011**: Timestamp format supports ISO8601 and Unix epoch
- [ ] **SC-012**: Falls back to poll time if extraction fails
- [ ] **SC-013**: NWS observations use observation time, not poll time

### Response Metadata Tags

- [ ] **SC-014**: `root_tags` config extracts metadata from response root
- [ ] **SC-015**: Root tags added to ALL TimeSeriesPoints in array
- [ ] **SC-016**: NWS forecast includes `forecast_generated_at` tag

### String Value Parsing

- [ ] **SC-017**: Regex parser extracts numeric value from string
- [ ] **SC-018**: "10 mph" → 10.0
- [ ] **SC-019**: "5 to 10 mph" → 5.0 (first value)
- [ ] **SC-020**: "N/A" → None (nullable)

### Enum Mapping

- [ ] **SC-021**: `cardinal_to_degrees` mapper converts directions
- [ ] **SC-022**: "NE" → 45.0
- [ ] **SC-023**: Custom enum mappings work from config
- [ ] **SC-024**: Unknown enum values log warning and use None

### Existing Stream Migration

- [ ] **SC-025**: air-quality (MQTT) produces identical Parquet output
- [ ] **SC-026**: air-quality (HTTP) produces identical Parquet output
- [ ] **SC-027**: outdoor-weather produces identical Parquet output
- [ ] **SC-028**: outdoor-air-quality produces identical Parquet output
- [ ] **SC-029**: All Grafana dashboards continue working

### NWS Streams

- [ ] **SC-030**: nws-observations stream configured in etcd
- [ ] **SC-031**: nws-forecast-hourly stream configured in etcd
- [ ] **SC-032**: NWS observations stored with observation timestamp
- [ ] **SC-033**: NWS forecasts stored in tall format (156 rows per poll)
- [ ] **SC-034**: Forecast metadata tags extracted (generatedAt)
- [ ] **SC-035**: Wind speed parsed from "10 mph" strings
- [ ] **SC-036**: Wind direction mapped from cardinal to degrees

### Legacy Code Removal

- [ ] **SC-037**: Delete `core/src/sources/parsers/weather.rs`
- [ ] **SC-038**: Delete `core/src/sources/parsers/air_pollution.rs`
- [ ] **SC-039**: Delete hardcoded MQTT skip_fields logic
- [ ] **SC-040**: Delete CurrentMeasures struct

---

## Architecture Overview

```
┌────────────────────────────────────────────────────────────────┐
│                         Stream Config (YAML)                    │
│                                                                 │
│  parser:                                                        │
│    parser_type: json_path                                      │
│    array_path: "periods"           # NEW: Array iteration      │
│    timestamp_field: "startTime"    # NEW: Response timestamp   │
│    root_tags:                      # NEW: Response metadata    │
│      - path: "generatedAt"                                     │
│        tag_name: "forecast_generated_at"                       │
│    field_mappings:                                             │
│      - path: "windSpeed"                                       │
│        value_parser: "regex"       # NEW: String parsing       │
│        value_pattern: "^(\\d+)"                                 │
│      - path: "windDirection"                                   │
│        value_mapper: "cardinal_to_degrees"  # NEW: Enum map    │
└─────────────────────────┬──────────────────────────────────────┘
                          │
                          ▼
┌────────────────────────────────────────────────────────────────┐
│                      SourceManager (UPDATED)                    │
│                                                                 │
│  fn create_parser_from_config() → Box<dyn Parser>              │
│    • Read parser config from YAML                              │
│    • Call ParserFactory::create()                              │
│    • Return configured parser instance                         │
│                                                                 │
│  fn spawn_source()                                             │
│    • Create parser from config                                 │
│    • Inject parser into source constructor                     │
│    • MqttSource::new_with_parser(config, parser)               │
│    • GenericHttpPollingSource::new_with_parser(config, parser) │
└─────────────────────────┬──────────────────────────────────────┘
                          │
         ┌────────────────┴────────────────┐
         │                                 │
         ▼                                 ▼
┌─────────────────────┐          ┌─────────────────────┐
│   MqttSource        │          │ GenericHttpPoll     │
│   (UPDATED)         │          │ Source (UPDATED)    │
│                     │          │                     │
│ parser: Box<Parser> │          │ parser: Box<Parser> │
│                     │          │                     │
│ fn parse_payload()  │          │ fn poll_endpoint()  │
│   parser.parse()    │          │   parser.parse()    │
└──────────┬──────────┘          └──────────┬──────────┘
           │                                │
           └────────────────┬───────────────┘
                            │
                            ▼
           ┌────────────────────────────────┐
           │   Parser (trait)               │
           │                                │
           │   parse(json, timestamp)       │
           │     → Vec<TimeSeriesPoint>     │
           └────────────────┬───────────────┘
                            │
           ┌────────────────┴────────────────┐
           │                                 │
           ▼                                 ▼
┌─────────────────────┐          ┌─────────────────────┐
│  FlatJsonParser     │          │  JsonPathParser     │
│                     │          │   (ENHANCED)        │
│  • Extract all      │          │                     │
│    numeric fields   │          │  • Array iteration  │
│  • Skip configured  │          │  • Timestamp extract│
│    fields           │          │  • Root tags        │
│  • Preserve names   │          │  • String parsing   │
│                     │          │  • Enum mapping     │
└─────────────────────┘          └─────────────────────┘
```

---

## Implementation Plan (SPARC Phases)

### Phase S: Specification

**Documents**:
- [ ] BUG-002 spec reviewed
- [ ] Air-006 scope (this document)
- [ ] API research for NWS endpoints
- [ ] Config schema design for new features

**Key Questions**:
- What regex patterns are needed for NWS wind speed?
- What timestamp formats does NWS use?
- How many forecast periods does hourly API return?

### Phase P: Pseudocode

**Algorithms**:
- [ ] Array iteration algorithm
- [ ] Timestamp extraction algorithm
- [ ] String value regex parsing algorithm
- [ ] Enum mapping lookup algorithm

### Phase A: Architecture

**Design Decisions**:
- [ ] Parser trait extension (backward compatible)
- [ ] Config schema for new features
- [ ] Integration points with SourceManager
- [ ] Migration strategy for existing streams

**ADRs**:
- [ ] ADR: Why array iteration vs. nested JSON
- [ ] ADR: Why response timestamp vs. poll timestamp
- [ ] ADR: Why config-driven enum mapping vs. hardcoded

### Phase R: Refinement (TDD)

**Implementation Order**:
1. Add parser injection to MqttSource (tests first)
2. Add parser injection to GenericHttpPollingSource (tests first)
3. Update SourceManager to create parsers (tests first)
4. Add array_path support to JsonPathParser (tests first)
5. Add timestamp_field extraction (tests first)
6. Add root_tags extraction (tests first)
7. Add value_parser regex support (tests first)
8. Add value_mapper enum support (tests first)

**Tests**:
- [ ] Unit tests for each parser enhancement
- [ ] Integration tests for source + parser
- [ ] Config validation tests
- [ ] Migration tests (compare old vs new output)

### Phase C: Completion

**Deployment**:
1. Deploy with new parser system
2. Migrate air-quality stream
3. Migrate outdoor-weather stream
4. Migrate outdoor-air-quality stream
5. Add nws-observations stream
6. Add nws-forecast-hourly stream
7. Delete legacy parser code

**Verification**:
- [ ] All existing streams produce identical output
- [ ] NWS streams ingesting data
- [ ] No errors in logs
- [ ] Grafana dashboards working
- [ ] Bronze Parquet files contain expected fields

---

## Key Design Decisions

### Decision 1: Parser Injection vs. Factory Method

**Choice**: Inject parser via constructor (`new_with_parser(config, parser)`), not factory method.

**Rationale**:
- Explicit dependency (easier to test)
- Follows Dependency Injection pattern
- SourceManager owns parser creation logic
- Sources remain agnostic to parser implementation

**Alternative Rejected**: Sources create own parsers from config (violates Single Responsibility Principle)

### Decision 2: Array Iteration in JsonPathParser

**Choice**: Add `array_path` config to JsonPathParser, iterate and create multiple TimeSeriesPoints.

**Rationale**:
- Tall format is standard for time-series data
- Simplifies Silver layer queries (no array unnesting)
- Consistent with existing TimeSeriesPoint model
- Enables efficient filtering on forecast_valid_time

**Alternative Rejected**: Nested JSON in single TimeSeriesPoint (complex to query, non-standard)

### Decision 3: Response Timestamp Extraction

**Choice**: Add `timestamp_field` config to extract timestamp from response, fall back to poll time.

**Rationale**:
- Observations have authoritative observation times
- Forecasts have authoritative issue times
- Enables forecast verification (JOIN on timestamps)
- Backward compatible (defaults to poll time)

**Alternative Rejected**: Always use poll time (loses temporal accuracy)

### Decision 4: Regex String Parsing

**Choice**: Add `value_parser: "regex"` with `value_pattern` config for numeric extraction from strings.

**Rationale**:
- NWS returns formatted strings ("10 mph")
- Regex is flexible for various formats
- Config-driven, no code changes for new patterns
- Fails gracefully (log warning, return None)

**Alternative Rejected**: Hardcoded parsing in parser (requires code changes)

### Decision 5: Config-Driven Enum Mapping

**Choice**: Add `value_mapper` with built-in mappers (cardinal_to_degrees) and custom mappings.

**Rationale**:
- Direction → degrees is common use case
- Config-driven for new enum types
- Type-safe (validates at config load time)
- Extensible for future mappers

**Alternative Rejected**: Store string values (loses queryability in Silver layer)

---

## Migration Strategy

### Existing Streams (Backward Compatible)

**air-quality (MQTT)**:

```yaml
# BEFORE: Hardcoded in MqttSource
# AFTER:
parser:
  parser_type: flat_json
  location_id_field: serialno
  skip_fields:
    - serialno
    - firmware
    - model
    - ledMode
  default_tags:
    source: mqtt
```

**outdoor-weather**:

```yaml
# BEFORE: Hardcoded WeatherParser
# AFTER:
parser:
  parser_type: json_path
  timestamp_field: "dt"           # NEW: Extract from response
  timestamp_format: unix_epoch
  location_id_field: "name"
  field_mappings:
    - path: "main.temp"
      metric_name: "temperature"
    - path: "wind.speed"
      metric_name: "wind_speed"
```

**outdoor-air-quality**:

```yaml
# BEFORE: Hardcoded AirPollutionParser
# AFTER:
parser:
  parser_type: json_path
  location_id_field: "coord"
  field_mappings:
    - path: "list[0].main.aqi"
      metric_name: "aqi"
    - path: "list[0].components.pm2_5"
      metric_name: "pm2_5"
```

### NWS Streams (New)

**nws-observations**:

```yaml
stream_id: nws-observations
description: "Current weather from NWS KSGJ station"
sources:
  - source_type: http_poll
    parser:
      parser_type: json_path
      timestamp_field: "properties.timestamp"  # NEW: Response timestamp
      timestamp_format: iso8601
      location_id_field: "properties.station"
      default_location_id: "KSGJ"
      field_mappings:
        - path: "properties.temperature.value"
          metric_name: "temperature"
          unit: "celsius"
```

**nws-forecast-hourly**:

```yaml
stream_id: nws-forecast-hourly
description: "Hourly forecast for St. Augustine area"
sources:
  - source_type: http_poll
    parser:
      parser_type: json_path

      # NEW: Array iteration
      array_path: "properties.periods"

      # NEW: Extract timestamp from each period
      timestamp_field: "startTime"
      timestamp_format: iso8601

      # NEW: Extract metadata from root
      root_tags:
        - path: "properties.generatedAt"
          tag_name: "forecast_generated_at"

      location_id_field: "properties.gridId"
      default_location_id: "JAX/79,49"

      field_mappings:
        - path: "temperature"
          metric_name: "temperature"
          unit: "fahrenheit"

        # NEW: String parsing
        - path: "windSpeed"
          metric_name: "wind_speed"
          value_parser: "regex"
          value_pattern: "^(\\d+)\\s+mph$"
          unit: "mph"

        # NEW: Enum mapping
        - path: "windDirection"
          metric_name: "wind_direction"
          value_mapper: "cardinal_to_degrees"
          unit: "degrees"
```

---

## File Structure

```
NEW RUST FILES:
core/src/parsers/
├── mod.rs                          # Export parsers module
├── traits.rs                       # Parser trait (from BUG-002)
├── flat_json.rs                    # FlatJsonParser (from BUG-002)
├── json_path.rs                    # JsonPathParser (ENHANCED)
├── factory.rs                      # ParserFactory
├── config.rs                       # ParserConfig structs
├── timestamp_extractor.rs          # NEW: Timestamp extraction logic
├── string_parser.rs                # NEW: Regex value parsing
└── enum_mapper.rs                  # NEW: Enum mapping logic

MODIFIED RUST FILES:
core/src/lib.rs                     # Export parsers module
core/src/sources/mqtt.rs            # Add new_with_parser()
core/src/sources/http_poll.rs       # Add new_with_parser()
apps/air-quality-app/src/coordinator/source_manager.rs  # Parser creation

NEW CONFIG FILES:
config/base/streams/
├── nws-observations.yaml           # NWS observations stream
└── nws-forecast-hourly.yaml        # NWS forecast stream

UPDATED CONFIG FILES:
config/base/streams/
├── air-quality.yaml                # Add parser config
├── outdoor-weather.yaml            # Add parser config
└── outdoor-air-quality.yaml        # Add parser config

DELETED FILES (after migration):
core/src/sources/parsers/
├── weather.rs                      # Replaced by JsonPathParser + config
└── air_pollution.rs                # Replaced by JsonPathParser + config

SPARC DOCUMENTATION:
product/features/air-006/
├── SCOPE.md                        # This file
├── STATUS.md                       # Progress tracking
├── specification/
│   ├── SPECIFICATION.md            # Requirements and API research
│   ├── NWS_API_RESEARCH.md         # NWS endpoint investigation
│   └── CONFIG_SCHEMA.md            # Parser config schema design
├── pseudocode/
│   ├── ARRAY_ITERATION.md          # Array iteration algorithm
│   ├── TIMESTAMP_EXTRACTION.md     # Timestamp parsing algorithm
│   ├── STRING_PARSING.md           # Regex value extraction
│   └── ENUM_MAPPING.md             # Enum lookup algorithm
├── architecture/
│   ├── ARCHITECTURE.md             # Integration design
│   ├── PARSER_ENHANCEMENTS.md      # New parser features
│   └── MIGRATION_PLAN.md           # Existing stream migration
├── refinement/
│   ├── IMPLEMENTATION.md           # TDD implementation log
│   └── TESTS.md                    # Test results
└── completion/
    ├── DEPLOYMENT.md               # Deployment verification
    └── VERIFICATION.md             # Data quality checks
```

---

## Constraints

### Technical Constraints

| Constraint | Impact |
|------------|--------|
| TimeSeriesPoint.value is f64 | All extracted values must coerce to float |
| Parquet schema is fixed | Bronze layer schema unchanged |
| Backward compatibility | Existing deployments work without config migration |
| No breaking changes | Existing parsers work until migration complete |

### Performance Constraints

| Constraint | Target |
|------------|--------|
| Parser creation | < 1ms |
| Parsing latency | < 1ms per message (p95) |
| Array iteration | 156 elements < 10ms |
| Regex extraction | < 100μs per field |

### Operational Constraints

| Constraint | Requirement |
|------------|-------------|
| Config validation | All errors caught at startup |
| Rollback safety | Invalid config must not crash app |
| Metrics | All parsing failures observable |
| Documentation | Config schema documented with examples |

---

## Open Questions

| Question | Status | Notes |
|----------|--------|-------|
| NWS forecast period count variability | TBD | Test API to confirm always 156 periods |
| Regex performance for 156 iterations | TBD | Benchmark regex parsing on NWS forecast |
| Poll interval for NWS | TBD | 10 min reasonable? Check NWS update frequency |
| NWS API rate limiting | TBD | Monitor during testing, add retry logic if needed |

---

## References

### BUG-002 Documents

- [BUG-002 Specification](/workspaces/neural-data-platform/product/features/dp-001/bugs/BUG-002-CONFIG-DRIVEN-PARSING-SPEC.md)
- [BUG-002 Architecture](/workspaces/neural-data-platform/product/features/dp-001/bugs/BUG-002-CONFIG-DRIVEN-PARSING-ARCH.md)

### NWS API

- [NWS API Documentation](https://www.weather.gov/documentation/services-web-api)
- [NWS API Specification](https://api.weather.gov/openapi.json)
- [KSGJ Station](https://api.weather.gov/stations/KSGJ)
- [JAX Gridpoint Hourly Forecast](https://api.weather.gov/gridpoints/JAX/79,49/forecast/hourly)

### NDP Architecture

- [Platform Architecture Overview](../../../docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md)
- [How to Add New Stream](../../../docs/procedures/HOW_TO_ADD_NEW_STREAM.md)
- [How to Add New Source](../../../docs/procedures/HOW_TO_ADD_NEW_SOURCE.md)
- [AIR-005 OpenWeatherMap Integration](../air-005/SCOPE.md)

### Related Features

- [DP-001 Silver Layer Development](../../dp-001/SCOPE.md)
- [AIR-001 through AIR-005](../) (Air Quality Phase)

---

## Approval

**Requires approval from**:
- [ ] Product Owner (scope alignment)
- [ ] ndp-architect (architecture review)
- [ ] ndp-rust-dev (implementation feasibility)

**Approval Date**: _________________

**Next Phase**: Specification (SPARC S)
