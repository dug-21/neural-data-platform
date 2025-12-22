# AIR-006: NWS Weather Data Integration - Complete Specification

**Feature ID**: air-006
**Phase**: Air Quality Monitoring
**SPARC Phase**: Specification
**Status**: Draft
**Created**: 2025-12-21
**Author**: NDP Specification Analyst

---

## 1. Executive Summary

This specification extends the config-driven parser architecture (BUG-002) to support National Weather Service (NWS) API integration. The implementation must handle:

1. **Array iteration** (FR-007): Process 156-period forecast arrays
2. **Response timestamp extraction** (FR-008): Use API timestamps, not poll time
3. **Metadata tags** (FR-009): Track forecast issue_time for verification
4. **String parsing** (FR-010): Extract numbers from "15 mph" format
5. **Enum mapping** (FR-011): Convert "NE" → 45.0 degrees
6. **Parser/Source integration** (FR-012): GenericHttpPollingSource uses Parser trait
7. **Legacy removal** (FR-013): Delete hardcoded parsers

### Key Goals

1. **Complete Config-Driven Parsing**: Remove all hardcoded parsers (weather.rs, air_pollution.rs)
2. **NWS API Support**: Integrate observations and hourly forecasts with tall format storage
3. **Forecast Verification**: Track forecast evolution with issue_time and forecast_valid_time
4. **Backward Compatibility**: All existing streams (air-quality, outdoor-weather, outdoor-air-quality) produce identical output

---

## 2. Requirements

### 2.1 Core Requirements from BUG-002

#### FR-001: Dynamic Field Extraction
**Priority:** HIGH
**Source:** BUG-002
**Description:** All parsers MUST extract fields dynamically based on configuration, not hardcoded structs.

**Acceptance Criteria:**
- Parser reads field list from YAML config, not Rust structs
- Adding a new field requires ONLY config change, no code change
- Unknown fields are either included (configurable) or logged as warnings

#### FR-002: JSONPath Support for Nested JSON
**Priority:** HIGH
**Source:** BUG-002
**Description:** HTTP parsers MUST support JSONPath expressions to extract nested values.

**Acceptance Criteria:**
- Config specifies JSONPath like `main.temp`, `wind.speed`, `list[0].components.pm2_5`
- Parser evaluates JSONPath against response body
- Invalid JSONPath results in clear error message

#### FR-003: Field Renaming Configuration
**Priority:** MEDIUM
**Source:** BUG-002
**Description:** Config MUST support mapping source field names to target field names.

**Acceptance Criteria:**
- Config specifies `source_field` → `target_field` mappings
- Renaming happens AFTER extraction, preserving raw Bronze layer data

#### FR-004: Field Exclusion Configuration
**Priority:** MEDIUM
**Source:** BUG-002
**Description:** Config MUST support excluding metadata fields from ingestion.

**Acceptance Criteria:**
- Config lists fields to exclude (e.g., `serialno`, `firmware`, `model`)
- Excluded fields are not stored but may be logged for debugging

#### FR-005: Type Coercion Configuration
**Priority:** MEDIUM
**Source:** BUG-002
**Description:** Config MUST specify expected data types for validation.

**Acceptance Criteria:**
- Config declares field type (float, int, string, boolean)
- Parser validates extracted value matches expected type
- Type mismatches are logged and value is either coerced or dropped

#### FR-006: Unit Configuration
**Priority:** LOW
**Source:** BUG-002
**Description:** Config SHOULD specify units for each field for metadata.

**Acceptance Criteria:**
- Config includes `unit` field (e.g., "celsius", "µg/m³", "ppm")
- Unit is added to TimeSeriesPoint tags

### 2.2 New Requirements for AIR-006

#### FR-007: Array Iteration
**Priority:** HIGH
**Source:** AIR-006 (NWS forecast requirement)
**Description:** Config must support iterating over JSON arrays to produce multiple TimeSeriesPoints.

**Acceptance Criteria:**
- Config specifies `array_path: "$.properties.periods"` to identify array
- Each array element produces N points (one per mapping)
- Example: 156-period NWS forecast produces 156 × M points (M = number of field mappings)
- Array iteration works with nested paths
- Empty arrays produce zero points without error

**Example Config:**
```yaml
parser:
  parser_type: json_path
  array_path: "$.properties.periods"
  element_mappings:
    - path: "temperature"
      metric_name: "temperature"
    - path: "windSpeed"
      metric_name: "wind_speed"
```

**Expected Output:** 2 periods × 2 metrics = 4 TimeSeriesPoints

#### FR-008: Response Timestamp Extraction
**Priority:** HIGH
**Source:** AIR-006 (forecast tracking requirement)
**Description:** Config must support using a response field as the timestamp instead of `Utc::now()`.

**Acceptance Criteria:**
- Config specifies `timestamp_field: "startTime"` to extract timestamp from response
- Parser parses ISO8601 timestamps from response
- Falls back to `Utc::now()` if field missing or parse fails (with warning)
- Supports both observations (single timestamp) and forecasts (per-element timestamps)

**Example:** TimeSeriesPoint.timestamp = 2025-12-21T11:00:00Z (not poll time)

#### FR-009: Response Metadata Tags
**Priority:** MEDIUM
**Source:** AIR-006 (forecast tracking requirement)
**Description:** Extract fields from response root as tags on ALL points.

**Acceptance Criteria:**
- Config specifies `metadata_tags` with JSONPath expressions
- Extracted values added to TimeSeriesPoint.tags
- Metadata tags applied to ALL points generated from response
- Example: `generatedAt` timestamp becomes `issue_time` tag on all forecast points

**Example:** All TimeSeriesPoints include `issue_time: "2025-12-21T09:15:00Z"`

#### FR-010: String Value Parsing
**Priority:** HIGH
**Source:** AIR-006 (NWS wind speed format)
**Description:** Parse numeric values from strings using regex patterns.

**Acceptance Criteria:**
- Config specifies `string_parse` with regex pattern and capture group
- Parser extracts numeric value from string like "10 mph" → 10.0
- Supports integer and float extraction
- Parse failures logged as warnings, field skipped

**Example:** "15 mph" → 15.0

**Edge Cases:**
- "10 to 20 mph" → Extract first number (10.0)
- "Variable" → null (logged as warning)
- "N/A" → null (logged as warning)

#### FR-011: Enum Mapping
**Priority:** MEDIUM
**Source:** AIR-006 (wind direction conversion)
**Description:** Map string values to numeric values via lookup table.

**Acceptance Criteria:**
- Config specifies `enum_map` with string → number mappings
- Parser looks up string value in map
- Unknown values logged as warnings, field skipped
- Case-insensitive matching supported

**Example:** "NE" → 45.0

#### FR-012: Parser/Source Integration
**Priority:** CRITICAL
**Source:** AIR-006 (architecture alignment)
**Description:** GenericHttpPollingSource MUST use Parser trait, not ResponseParser.

**Acceptance Criteria:**
- GenericHttpPollingSource receives `Box<dyn Parser>` in constructor
- ResponseParser trait usage removed from HTTP sources
- MqttSource already uses Parser trait (no change needed)
- SourceManager creates Parser from config and injects into source

#### FR-013: Legacy Removal
**Priority:** MEDIUM
**Source:** AIR-006 (cleanup)
**Description:** Delete legacy hardcoded parsers after migration.

**Acceptance Criteria:**
- `core/src/sources/parsers/weather.rs` deleted
- `core/src/sources/parsers/air_pollution.rs` deleted
- ResponseParser trait removed from registry
- All tests updated to use config-driven parsers

---

## 3. NWS-Specific Requirements

### 3.1 NWS Observations Stream

#### Key Features
- **Station:** KSGJ (Northeast Florida Regional Airport, St. Augustine)
- **Endpoint:** `/stations/KSGJ/observations/latest`
- **Poll Interval:** 10 minutes
- **Timestamp:** Extracted from response (`properties.timestamp`)
- **Location ID:** Station ID from response

#### Required Fields

| Field | Source Path | Unit | Transform | Notes |
|-------|-------------|------|-----------|-------|
| temperature | properties.temperature.value | celsius | - | Already in Celsius |
| dewpoint | properties.dewpoint.value | celsius | - | Already in Celsius |
| wind_speed | properties.windSpeed.value | m/s | - | Already in m/s |
| wind_direction | properties.windDirection.value | degrees | - | Numeric 0-360 |
| wind_gust | properties.windGust.value | m/s | - | Nullable |
| pressure | properties.barometricPressure.value | pa | - | Already in Pascals |
| visibility | properties.visibility.value | meters | - | Already in meters |
| humidity | properties.relativeHumidity.value | percent | - | 0-100 |

### 3.2 NWS Hourly Forecast Stream

#### Key Features
- **Grid Point:** JAX/79,49 (Jacksonville WFO, St. Augustine area)
- **Endpoint:** `/gridpoints/JAX/79,49/forecast/hourly`
- **Poll Interval:** 10 minutes
- **Array Iteration:** 156 forecast periods (FR-007)
- **Timestamp:** Per-period `startTime` (FR-008)
- **Issue Time:** Response `generatedAt` as metadata tag (FR-009)

#### Required Fields

| Field | Source Path | Unit | Parsing | Notes |
|-------|-------------|------|---------|-------|
| temperature | temperature | fahrenheit | - | Transform to Celsius |
| dewpoint | dewpoint.value | celsius | - | Already in Celsius |
| wind_speed | windSpeed | mph | String parse (FR-010) | "15 mph" → 15.0 |
| wind_direction | windDirection | degrees | Enum map (FR-011) | "NE" → 45.0 |
| precipitation_probability | probabilityOfPrecipitation.value | percent | - | 0-100 |
| humidity | relativeHumidity.value | percent | - | 0-100 |

#### Expected Output Volume
- **One poll:** 156 periods × 6 metrics = **936 TimeSeriesPoints**
- **Daily:** 144 polls × 936 points = **134,784 points/day**

---

## 4. Acceptance Criteria

### 4.1 Array Iteration (FR-007)

- [ ] **AC-007:** Config with `array_path` iterates over JSON arrays
- [ ] **AC-008:** Each array element produces N points (one per element_mapping)
- [ ] **AC-009:** NWS forecast with 156 periods produces 936 points (6 metrics)
- [ ] **AC-010:** Empty arrays produce zero points without error
- [ ] **AC-011:** Array iteration works with nested paths

### 4.2 Response Timestamp Extraction (FR-008)

- [ ] **AC-013:** Config `timestamp_field` extracts timestamp from response
- [ ] **AC-014:** Parser parses ISO8601 timestamps correctly
- [ ] **AC-015:** NWS observations use observation timestamp, not poll time
- [ ] **AC-016:** NWS forecasts use `startTime` from each period
- [ ] **AC-017:** Invalid timestamps fall back to `Utc::now()` with warning

### 4.3 Response Metadata Tags (FR-009)

- [ ] **AC-019:** Config `metadata_tags` extracts fields from response root
- [ ] **AC-020:** Metadata tags applied to ALL points from response
- [ ] **AC-021:** NWS forecast `issue_time` tag present on all 936 points
- [ ] **AC-022:** JSONPath expressions work for metadata tags

### 4.4 String Value Parsing (FR-010)

- [ ] **AC-024:** Config `string_parse` with regex pattern extracts numbers
- [ ] **AC-025:** "15 mph" → 15.0
- [ ] **AC-026:** "12.5 mph" → 12.5
- [ ] **AC-027:** "10 to 20 mph" → 10.0 (first number)
- [ ] **AC-028:** "Variable" → null with warning

### 4.5 Enum Mapping (FR-011)

- [ ] **AC-030:** Config `enum_map` maps strings to numbers
- [ ] **AC-031:** "NE" → 45.0 (wind direction mapping)
- [ ] **AC-032:** Unknown values logged as warnings, field skipped
- [ ] **AC-033:** Case-insensitive matching supported

### 4.6 Parser/Source Integration (FR-012)

- [ ] **AC-035:** GenericHttpPollingSource uses Parser trait
- [ ] **AC-036:** ResponseParser trait removed from HTTP sources
- [ ] **AC-037:** SourceManager creates Parser from config
- [ ] **AC-038:** Parser injected via constructor into sources

### 4.7 NWS Streams

- [ ] **AC-040:** `nws-observations` stream configured in etcd
- [ ] **AC-041:** NWS observations polling every 10 minutes
- [ ] **AC-042:** Observation timestamp extracted from response
- [ ] **AC-043:** All observation fields extracted correctly
- [ ] **AC-044:** `nws-forecast-hourly` stream configured in etcd
- [ ] **AC-045:** NWS forecast polling every 10 minutes
- [ ] **AC-046:** 156 forecast periods produce 936 points
- [ ] **AC-047:** `issue_time` tag present on all forecast points
- [ ] **AC-048:** Wind speed string parsing works
- [ ] **AC-049:** Wind direction enum mapping works
- [ ] **AC-050:** Forecast timestamps use `startTime`, not poll time

### 4.8 Backward Compatibility

- [ ] **AC-051:** Existing `air-quality` stream works unchanged
- [ ] **AC-052:** Existing `outdoor-weather` stream works unchanged
- [ ] **AC-053:** Existing `outdoor-air-quality` stream works unchanged
- [ ] **AC-054:** All existing integration tests pass

---

## 5. Non-Functional Requirements

### NFR-001: Performance
- Parsing latency: <1ms per message p95
- NWS forecast parsing (156 periods): <10ms p95
- No throughput degradation >5%

### NFR-002: Memory
- Max memory per forecast response: <5MB
- Parser instance overhead: <1KB
- No memory leaks during continuous operation

### NFR-003: Backward Compatibility
- All existing integration tests pass
- No config migration required for existing streams

### NFR-004: Error Handling
- Config validation at startup
- Invalid JSONPath fails fast
- Clear error messages

### NFR-005: Observability
- Counter: `fields_extracted_total{stream, field}`
- Counter: `fields_dropped_total{stream, reason}`
- Histogram: `parsing_duration_seconds`

---

## 6. Out of Scope

1. **Silver Layer Transformation:** Field renaming in ETL (dp-001)
2. **Schema Evolution:** Parquet schema changes
3. **Multiple NWS Stations:** Start with KSGJ only
4. **NWS Alerts/Warnings:** Defer to alerts phase (al-xxx)
5. **Forecast Accuracy Metrics:** Requires Silver layer (dp-002)

---

## 7. References

### NDP Internal
- [BUG-002 Specification](/workspaces/neural-data-platform/product/features/dp-001/bugs/BUG-002-CONFIG-DRIVEN-PARSING-SPEC.md)
- [BUG-002 Architecture](/workspaces/neural-data-platform/product/features/dp-001/bugs/BUG-002-CONFIG-DRIVEN-PARSING-ARCH.md)
- [Current JsonPathParser](/workspaces/neural-data-platform/core/src/parsers/json_path.rs)
- [Platform Architecture](/workspaces/neural-data-platform/docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md)

### External References
- [NWS API Documentation](https://www.weather.gov/documentation/services-web-api)
- [KSGJ Station Info](https://www.weather.gov/wrh/timeseries?site=KSGJ)
- [JAX Grid Point](https://api.weather.gov/points/29.9592,-81.3397)

---

## Revision History

| Date | Version | Author | Changes |
|------|---------|--------|---------|
| 2025-12-21 | 1.0 | NDP Specification Analyst | Initial comprehensive specification combining BUG-002 + AIR-006 |

---

**Status:** Ready for Review
**Next Phase:** Pseudocode
**Assigned To:** ndp-rust-dev, ndp-architect
