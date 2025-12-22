# AIR-006: NWS Weather Data Integration - Pseudocode Phase

**Feature ID**: air-006
**Feature Name**: NWS Weather Data Integration
**SPARC Phase**: Pseudocode (P)
**Version**: 1.0.0
**Status**: Draft
**Created**: 2025-12-21
**Author**: NDP Algorithm Designer

---

## Table of Contents

1. [Overview](#overview)
2. [Extension of BUG-002 Pattern](#extension-of-bug-002-pattern)
3. [New Parser Algorithms](#new-parser-algorithms)
4. [NWS Observations Parser](#nws-observations-parser)
5. [NWS Forecast Parser (Array Iteration)](#nws-forecast-parser-array-iteration)
6. [Timestamp Extraction Algorithms](#timestamp-extraction-algorithms)
7. [Unit Conversion Algorithms](#unit-conversion-algorithms)
8. [Enhanced Parser Factory](#enhanced-parser-factory)
9. [Complexity Analysis](#complexity-analysis)
10. [Error Handling Strategy](#error-handling-strategy)
11. [Testing Strategy](#testing-strategy)
12. [Integration with Existing System](#integration-with-existing-system)

---

## Overview

### Purpose

This document provides complete algorithmic pseudocode for implementing NWS weather data parsers in the Neural Data Platform. The design **extends** the config-driven parser architecture from BUG-002 with:

1. **Array iteration pattern** - For parsing forecast periods (tall format)
2. **Multi-timestamp extraction** - issue_time, forecast_valid_time, observation timestamps
3. **Complex unit conversions** - String wind speeds, cardinal directions, Fahrenheit to Celsius
4. **GeoJSON navigation** - Nested property extraction from NWS API responses

### Parser Types

| Parser | Type | Use Case | Extension of |
|--------|------|----------|--------------|
| `NwsObservationParser` | Flat GeoJSON | Current weather observations | JsonPathParser concept |
| `NwsForecastParser` | Array Iterator | Hourly forecast with evolution tracking | **NEW** - Array iteration pattern |

### Key Design Principles

1. **Absolute Timestamps**: issue_time and forecast_valid_time stored as Unix timestamps
2. **Tall Format**: One TimeSeriesPoint per metric per forecast period
3. **Stateless Parsing**: Zero-sized types, no mutable state
4. **Config-Driven**: Parsers configurable via YAML (future extension)

---

## Extension of BUG-002 Pattern

### What We Inherit from BUG-002

```
FROM BUG-002:
├─ Parser trait definition (parse, from_config, name)
├─ FlatJsonParser concept (extract all numeric fields)
├─ JsonPathParser concept (navigate nested JSON)
├─ Parser factory pattern
├─ Error handling strategy (CoreError mapping)
└─ Unit test patterns
```

### What We Add in AIR-006

```
NEW PATTERNS:
├─ ArrayIterationParser algorithm (for forecast periods)
├─ Multi-timestamp extraction (issue_time + forecast_valid_time)
├─ String parsing transforms:
│   ├─ parse_wind_speed("5 to 10 mph") → 7.5 m/s
│   └─ parse_wind_direction("NE") → 45.0°
├─ GeoJSON property navigation
├─ Generated field computation (lead_time_hours)
└─ Tall format data model (one row per period)
```

### Integration Strategy

```
ARCHITECTURE:
    ResponseParser trait (existing)
         ↓
    ┌────┴─────────────────────────┐
    │                              │
FlatJsonParser           NwsObservationParser (NEW)
    (BUG-002)                 (AIR-006)
    │                              │
JsonPathParser           NwsForecastParser (NEW)
    (BUG-002)                 (AIR-006)
                                   │
                      ArrayIterationParser pattern
                         (NEW algorithm)
```

**Key Insight**: NWS parsers are **specialized implementations** that could later be generalized into config-driven `ArrayIterationParser` for other APIs.

---

## New Parser Algorithms

### Algorithm 1: ArrayIterationParser Pattern

**Purpose**: Extract multiple TimeSeriesPoints from a JSON array with shared metadata.

**Use Case**: NWS hourly forecast returns array of 156 periods, each needing multiple metrics extracted.

**Core Concept**:
```
JSON Response:
{
  "properties": {
    "generatedAt": "2025-12-21T12:00:00Z",  ← Shared metadata
    "periods": [                            ← Array to iterate
      {"startTime": "...", "temperature": 72, ...},
      {"startTime": "...", "temperature": 73, ...},
      ...
    ]
  }
}

Output (Tall Format):
[
  TimeSeriesPoint { timestamp: poll_time, tags: { issue_time: "...", forecast_valid_time: "period[0].startTime", metric: "temperature" }, value: 72 },
  TimeSeriesPoint { timestamp: poll_time, tags: { issue_time: "...", forecast_valid_time: "period[0].startTime", metric: "humidity" }, value: 65 },
  ...
  TimeSeriesPoint { timestamp: poll_time, tags: { issue_time: "...", forecast_valid_time: "period[155].startTime", metric: "temperature" }, value: 68 },
]
```

**Pseudocode**:
```
ALGORITHM: parse_with_array_iteration
INPUT: json_response, array_path, shared_metadata_extractors, element_mappings
OUTPUT: Vec<TimeSeriesPoint>

BEGIN
    // Step 1: Parse JSON
    json_value ← serde_json::from_str(json_response)
    IF json_value is Error THEN
        RETURN CoreError::Source("Failed to parse JSON response")
    END IF

    // Step 2: Extract shared metadata (issue_time, grid_point, etc.)
    shared_metadata ← HashMap::new()
    FOR EACH (metadata_key, json_path) IN shared_metadata_extractors DO
        value ← extract_json_path(json_value, json_path)
        IF value is None THEN
            RETURN CoreError::Source("Missing required metadata: {metadata_key}")
        END IF
        shared_metadata.insert(metadata_key, value)
    END FOR

    // Step 3: Extract array at specified path
    array_value ← extract_json_path(json_value, array_path)
    IF array_value is None OR NOT array_value.is_array() THEN
        RETURN CoreError::Source("Array not found at path: {array_path}")
    END IF

    periods ← array_value.as_array()
    points ← Vec::new()

    // Step 4: Iterate over array elements
    FOR EACH element IN periods DO
        // Extract element-specific timestamp (forecast_valid_time)
        element_timestamp ← extract_timestamp(element, "startTime")
        IF element_timestamp is None THEN
            WARN("Missing timestamp in period, skipping")
            CONTINUE
        END IF

        // Step 5: Extract each metric from element
        FOR EACH mapping IN element_mappings DO
            // Navigate to field in element
            field_value ← extract_json_path(element, mapping.path)

            IF field_value is None THEN
                IF NOT mapping.optional THEN
                    RETURN CoreError::Source("Required field not found: {mapping.path}")
                ELSE
                    CONTINUE  // Skip optional missing field
                END IF
            END IF

            // Apply transformations (string_parse, enum_map, unit_conversion)
            transformed_value ← apply_transforms(field_value, mapping.transforms)

            IF transformed_value is None THEN
                IF NOT mapping.optional THEN
                    RETURN CoreError::Source("Transform failed for field: {mapping.path}")
                ELSE
                    CONTINUE
                END IF
            END IF

            // Create tags with shared metadata + element metadata
            tags ← HashMap::new()
            tags.insert("metric", mapping.metric_name)

            // Add shared metadata to tags
            FOR EACH (key, value) IN shared_metadata DO
                tags.insert(key, value.to_string())
            END FOR

            // Add element timestamp to tags
            tags.insert("forecast_valid_time", element_timestamp.timestamp().to_string())

            // Create TimeSeriesPoint
            point ← TimeSeriesPoint {
                timestamp: poll_timestamp,
                location_id: location_id.clone(),
                value: transformed_value,
                tags: tags
            }

            points.push(point)
        END FOR
    END FOR

    // Step 6: Validate result
    IF points.is_empty() THEN
        WARN("No points extracted from array")
    END IF

    INFO("Extracted {} points from {} periods", points.len(), periods.len())

    RETURN Ok(points)
END
```

**Complexity**:
- Time: O(n × m) where n = array size (156 periods), m = mappings per element (7 metrics)
- Space: O(n × m) for result vector (~1,092 points per poll)

---

## NWS Observations Parser

### Data Structure

```
STRUCT: NwsObservationParser
PURPOSE: Parse GeoJSON current observation response from NWS API

FIELDS:
    None (zero-sized type, stateless)

INVARIANTS:
    - Implements ResponseParser trait
    - Stateless (no mutable state between parse calls)
    - GeoJSON format: properties nested under "properties" key
```

### Parsing Algorithm

```
ALGORITHM: NwsObservationParser.parse
INPUT: response_body (JSON string), location_id (station ID), timestamp (poll time)
OUTPUT: Vec<TimeSeriesPoint> or CoreError

BEGIN
    // Step 1: Parse GeoJSON response
    geojson ← serde_json::from_str::<GeoJsonResponse>(response_body)
    IF geojson is Error THEN
        RETURN CoreError::Source("Failed to parse NWS observation GeoJSON: {error}")
    END IF

    // Step 2: Extract properties object
    IF geojson.properties is None THEN
        RETURN CoreError::Source("Missing 'properties' in GeoJSON response")
    END IF

    properties ← geojson.properties

    // Step 3: Extract observation timestamp
    observation_timestamp ← parse_rfc3339(properties.timestamp)
    IF observation_timestamp is Error THEN
        RETURN CoreError::Source("Invalid timestamp format: {properties.timestamp}")
    END IF

    observation_timestamp_utc ← observation_timestamp.with_timezone(&Utc)

    // Step 4: Initialize result vector
    points ← Vec::new()

    // Step 5: Extract each weather field with unit conversion
    CALL extract_quantity_field(
        properties.temperature,
        "temperature",
        "degC",
        observation_timestamp_utc,
        location_id,
        &mut points
    )

    CALL extract_quantity_field(
        properties.dewpoint,
        "dewpoint",
        "degC",
        observation_timestamp_utc,
        location_id,
        &mut points
    )

    CALL extract_quantity_field(
        properties.relative_humidity,
        "humidity",
        "percent",
        observation_timestamp_utc,
        location_id,
        &mut points
    )

    CALL extract_wind_fields(
        properties.wind_speed,
        properties.wind_direction,
        observation_timestamp_utc,
        location_id,
        &mut points
    )

    CALL extract_pressure_field(
        properties.barometric_pressure,
        observation_timestamp_utc,
        location_id,
        &mut points
    )

    CALL extract_quantity_field(
        properties.visibility,
        "visibility",
        "meters",
        observation_timestamp_utc,
        location_id,
        &mut points
    )

    // Step 6: Validate result
    IF points.is_empty() THEN
        WARN("No observation fields extracted from NWS response")
    END IF

    RETURN Ok(points)
END

SUBROUTINE: extract_quantity_field
INPUT: quantity_value (Option<QuantityValue>), metric_name, target_unit, timestamp, location_id, points
OUTPUT: None (mutates points vector)

BEGIN
    IF quantity_value is None THEN
        DEBUG("Optional field {metric_name} is null")
        RETURN
    END IF

    quantity ← quantity_value.unwrap()

    IF quantity.value is None THEN
        DEBUG("Field {metric_name} has null value")
        RETURN
    END IF

    raw_value ← quantity.value.unwrap()
    unit_code ← quantity.unit_code

    // Convert units if needed
    converted_value ← convert_nws_unit(raw_value, unit_code, target_unit)

    // Create tags
    tags ← HashMap::new()
    tags.insert("metric", metric_name)
    tags.insert("source", "nws")
    tags.insert("station", "KSGJ")
    tags.insert("unit", target_unit)

    // Create point
    point ← TimeSeriesPoint {
        timestamp: timestamp,
        location_id: location_id.to_string(),
        value: converted_value,
        tags: tags
    }

    points.push(point)
END

SUBROUTINE: extract_wind_fields
INPUT: wind_speed_qty, wind_direction_qty, timestamp, location_id, points
OUTPUT: None (mutates points vector)

BEGIN
    // Extract wind speed
    IF wind_speed_qty is Some AND wind_speed_qty.value is Some THEN
        raw_speed ← wind_speed_qty.value
        // NWS wind speed: km/h → convert to m/s
        speed_ms ← raw_speed / 3.6

        tags_speed ← HashMap::new()
        tags_speed.insert("metric", "wind_speed")
        tags_speed.insert("source", "nws")
        tags_speed.insert("station", "KSGJ")
        tags_speed.insert("unit", "m/s")

        point_speed ← TimeSeriesPoint {
            timestamp: timestamp,
            location_id: location_id.to_string(),
            value: speed_ms,
            tags: tags_speed
        }

        points.push(point_speed)
    END IF

    // Extract wind direction
    IF wind_direction_qty is Some AND wind_direction_qty.value is Some THEN
        direction_degrees ← wind_direction_qty.value

        tags_direction ← HashMap::new()
        tags_direction.insert("metric", "wind_direction")
        tags_direction.insert("source", "nws")
        tags_direction.insert("station", "KSGJ")
        tags_direction.insert("unit", "degrees")

        point_direction ← TimeSeriesPoint {
            timestamp: timestamp,
            location_id: location_id.to_string(),
            value: direction_degrees,
            tags: tags_direction
        }

        points.push(point_direction)
    END IF
END

SUBROUTINE: extract_pressure_field
INPUT: pressure_qty, timestamp, location_id, points
OUTPUT: None (mutates points vector)

BEGIN
    IF pressure_qty is None OR pressure_qty.value is None THEN
        RETURN
    END IF

    raw_pressure_pa ← pressure_qty.value  // Pascals
    pressure_hpa ← raw_pressure_pa / 100.0  // Convert to hectopascals

    tags ← HashMap::new()
    tags.insert("metric", "pressure")
    tags.insert("source", "nws")
    tags.insert("station", "KSGJ")
    tags.insert("unit", "hpa")

    point ← TimeSeriesPoint {
        timestamp: timestamp,
        location_id: location_id.to_string(),
        value: pressure_hpa,
        tags: tags
    }

    points.push(point)
END
```

**Complexity Analysis**:
- Time: O(n) where n = number of fields (typically 7-10)
- Space: O(n) for result vector
- JSON parsing: O(m) where m = response size (~2 KB)
- **Total**: O(m + n) ≈ O(m) dominated by JSON parsing

**Error Handling**:
- Invalid JSON → CoreError::Source with parse error
- Missing properties → CoreError::Source
- Invalid timestamp → CoreError::Source
- Null field values → DEBUG log, skip field (graceful)

---

## NWS Forecast Parser (Array Iteration)

### Data Structure

```
STRUCT: NwsForecastParser
PURPOSE: Parse GeoJSON hourly forecast response with array iteration (tall format)

FIELDS:
    None (zero-sized type, stateless)

INVARIANTS:
    - Implements ResponseParser trait
    - Generates multiple TimeSeriesPoints per forecast period
    - Stores issue_time and forecast_valid_time in tags
    - Tall format: one point per metric per period
```

### Parsing Algorithm

```
ALGORITHM: NwsForecastParser.parse
INPUT: response_body (JSON string), location_id (grid point ID), timestamp (poll time)
OUTPUT: Vec<TimeSeriesPoint> or CoreError

BEGIN
    // Step 1: Parse GeoJSON forecast response
    geojson ← serde_json::from_str::<ForecastGeoJsonResponse>(response_body)
    IF geojson is Error THEN
        RETURN CoreError::Source("Failed to parse NWS forecast GeoJSON: {error}")
    END IF

    properties ← geojson.properties

    // Step 2: Extract issue_time (generatedAt)
    issue_time_str ← properties.generated_at
    issue_time ← parse_rfc3339(issue_time_str)
    IF issue_time is Error THEN
        RETURN CoreError::Source("Invalid generatedAt timestamp: {issue_time_str}")
    END IF

    issue_time_utc ← issue_time.with_timezone(&Utc)
    issue_time_unix ← issue_time_utc.timestamp()

    // Step 3: Extract forecast periods array
    periods ← properties.periods
    IF periods.is_empty() THEN
        RETURN CoreError::Source("No forecast periods in response")
    END IF

    period_count ← periods.len()
    INFO("Processing {} forecast periods", period_count)

    // Validate period count (expect 156)
    IF period_count != 156 THEN
        WARN("Unexpected period count: {} (expected 156)", period_count)
    END IF

    // Step 4: Initialize result vector
    // Estimate capacity: periods × metrics_per_period
    points ← Vec::with_capacity(period_count * 7)

    // Step 5: Iterate over forecast periods
    FOR EACH period IN periods DO
        // Extract forecast_valid_time (startTime)
        valid_time_str ← period.start_time
        valid_time ← parse_rfc3339(valid_time_str)
        IF valid_time is Error THEN
            WARN("Invalid startTime in period: {valid_time_str}, skipping")
            CONTINUE
        END IF

        valid_time_utc ← valid_time.with_timezone(&Utc)
        valid_time_unix ← valid_time_utc.timestamp()

        // Calculate lead_time_hours
        lead_time_hours ← (valid_time_unix - issue_time_unix) / 3600

        // Create base tags (shared across all metrics in this period)
        base_tags ← HashMap::new()
        base_tags.insert("source", "nws")
        base_tags.insert("forecast_type", "hourly")
        base_tags.insert("grid_point", "JAX/79,49")
        base_tags.insert("issue_time", issue_time_unix.to_string())
        base_tags.insert("forecast_valid_time", valid_time_unix.to_string())
        base_tags.insert("lead_time_hours", lead_time_hours.to_string())

        // Extract temperature (Fahrenheit → Celsius)
        temperature_f ← period.temperature as f64
        temperature_c ← fahrenheit_to_celsius(temperature_f)

        CALL create_forecast_point(
            "temperature",
            temperature_c,
            "celsius",
            timestamp,
            location_id,
            base_tags.clone(),
            &mut points
        )

        // Extract dewpoint (already in Celsius)
        IF period.dewpoint is Some AND period.dewpoint.value is Some THEN
            dewpoint_c ← period.dewpoint.value

            CALL create_forecast_point(
                "dewpoint",
                dewpoint_c,
                "celsius",
                timestamp,
                location_id,
                base_tags.clone(),
                &mut points
            )
        END IF

        // Extract humidity
        IF period.relative_humidity is Some AND period.relative_humidity.value is Some THEN
            humidity ← period.relative_humidity.value

            CALL create_forecast_point(
                "humidity",
                humidity,
                "percent",
                timestamp,
                location_id,
                base_tags.clone(),
                &mut points
            )
        END IF

        // Parse wind speed (string format: "5 to 10 mph")
        wind_speed_ms ← parse_wind_speed(period.wind_speed)
        IF wind_speed_ms is Some THEN
            CALL create_forecast_point(
                "wind_speed",
                wind_speed_ms,
                "m/s",
                timestamp,
                location_id,
                base_tags.clone(),
                &mut points
            )
        END IF

        // Parse wind direction (cardinal → degrees)
        wind_dir_degrees ← parse_wind_direction(period.wind_direction)
        IF wind_dir_degrees is Some THEN
            CALL create_forecast_point(
                "wind_direction",
                wind_dir_degrees,
                "degrees",
                timestamp,
                location_id,
                base_tags.clone(),
                &mut points
            )
        END IF

        // Extract precipitation probability
        IF period.probability_of_precipitation is Some
           AND period.probability_of_precipitation.value is Some THEN
            precip_prob ← period.probability_of_precipitation.value

            CALL create_forecast_point(
                "precip_probability",
                precip_prob,
                "percent",
                timestamp,
                location_id,
                base_tags.clone(),
                &mut points
            )
        END IF

        // Note: short_forecast is string, not stored in TimeSeriesPoint
        // Could be added as metadata in future
    END FOR

    // Step 6: Validate result
    IF points.is_empty() THEN
        RETURN CoreError::Source("No forecast data extracted")
    END IF

    INFO("Extracted {} forecast points from {} periods", points.len(), period_count)

    RETURN Ok(points)
END

SUBROUTINE: create_forecast_point
INPUT: metric_name, value, unit, poll_timestamp, location_id, base_tags, points
OUTPUT: None (mutates points vector)

BEGIN
    tags ← base_tags.clone()
    tags.insert("metric", metric_name)
    tags.insert("unit", unit)

    point ← TimeSeriesPoint {
        timestamp: poll_timestamp,
        location_id: location_id.to_string(),
        value: value,
        tags: tags
    }

    points.push(point)
END
```

**Complexity Analysis**:
- Time: O(n × m) where n = periods (156), m = metrics per period (7)
- Space: O(n × m) = O(1,092) points per poll
- JSON parsing: O(r) where r = response size (~200 KB)
- **Total**: O(r + n × m) ≈ O(n × m) for processing

**Key Design Points**:
1. **Tall Format**: Each period generates 7 separate TimeSeriesPoints
2. **Tags Propagation**: issue_time, forecast_valid_time, lead_time_hours in every point
3. **Timestamp Strategy**: poll_timestamp is the TimeSeriesPoint.timestamp, valid_time in tags
4. **Pre-allocation**: Vec capacity pre-sized to 156 × 7 = 1,092

---

## Timestamp Extraction Algorithms

### Algorithm 1: RFC3339 Timestamp Parsing

```
ALGORITHM: parse_rfc3339
INPUT: timestamp_string (ISO 8601 format)
OUTPUT: DateTime<Utc> or Error

PURPOSE: Parse NWS timestamps with timezone offsets, convert to UTC

BEGIN
    // NWS formats:
    // "2025-12-21T12:00:00+00:00"  (UTC)
    // "2025-12-21T12:00:00-05:00"  (EST)
    // "2025-12-21T12:00:00-04:00"  (EDT)

    // Step 1: Parse with timezone information
    parsed ← DateTime::parse_from_rfc3339(timestamp_string)
    IF parsed is Error THEN
        RETURN Error("Invalid RFC3339 timestamp: {timestamp_string}")
    END IF

    // Step 2: Convert to UTC
    utc_timestamp ← parsed.with_timezone(&Utc)

    RETURN Ok(utc_timestamp)
END
```

**Test Cases**:
```
INPUT: "2025-12-21T12:00:00+00:00" → OUTPUT: 2025-12-21 12:00:00 UTC
INPUT: "2025-12-21T12:00:00-05:00" → OUTPUT: 2025-12-21 17:00:00 UTC
INPUT: "2025-12-21T12:00:00-04:00" → OUTPUT: 2025-12-21 16:00:00 UTC
INPUT: "invalid-date" → OUTPUT: Error
```

### Algorithm 2: Extract Issue Time and Valid Time

```
ALGORITHM: extract_forecast_timestamps
INPUT: forecast_properties (ForecastProperties object)
OUTPUT: (issue_time: DateTime<Utc>, periods_with_valid_times: Vec<(DateTime<Utc>, Period)>)

PURPOSE: Extract all timestamps from forecast response, validate consistency

BEGIN
    // Extract issue_time (when forecast was generated)
    issue_time ← parse_rfc3339(forecast_properties.generated_at)?

    periods_with_times ← Vec::new()

    FOR EACH period IN forecast_properties.periods DO
        // Extract forecast_valid_time (when conditions predicted for)
        valid_time ← parse_rfc3339(period.start_time)?

        // Validate: valid_time should be >= issue_time
        IF valid_time < issue_time THEN
            RETURN Error("Invalid forecast: valid_time < issue_time")
        END IF

        // Validate: valid_time should be in the future (relative to issue_time)
        lead_time_seconds ← valid_time.timestamp() - issue_time.timestamp()
        IF lead_time_seconds < 0 THEN
            RETURN Error("Negative lead time detected")
        END IF

        periods_with_times.push((valid_time, period))
    END FOR

    // Validate: periods should be chronologically ordered
    FOR i IN 1..periods_with_times.len() DO
        prev_time ← periods_with_times[i-1].0
        curr_time ← periods_with_times[i].0

        IF curr_time <= prev_time THEN
            WARN("Forecast periods not in chronological order at index {}", i)
        END IF
    END FOR

    RETURN Ok((issue_time, periods_with_times))
END
```

**Validation Checks**:
1. Issue time must be valid RFC3339
2. All period start times must be valid RFC3339
3. valid_time >= issue_time (causality check)
4. Periods should be chronologically ordered
5. Lead times should be positive

---

## Unit Conversion Algorithms

### Algorithm 1: Temperature Conversion

```
ALGORITHM: fahrenheit_to_celsius
INPUT: temperature_f (Fahrenheit value as f64)
OUTPUT: temperature_c (Celsius value as f64)

BEGIN
    temperature_c ← (temperature_f - 32.0) * 5.0 / 9.0
    RETURN temperature_c
END

EXAMPLES:
    fahrenheit_to_celsius(32.0) → 0.0
    fahrenheit_to_celsius(212.0) → 100.0
    fahrenheit_to_celsius(72.0) → 22.222...
```

### Algorithm 2: Wind Speed Parsing and Conversion

```
ALGORITHM: parse_wind_speed
INPUT: wind_speed_str (e.g., "10 mph", "5 to 10 mph")
OUTPUT: Option<f64> (meters per second)

PURPOSE: Parse NWS wind speed string format, handle ranges, convert to m/s

BEGIN
    // Step 1: Trim whitespace
    trimmed ← wind_speed_str.trim()

    // Step 2: Check for range format "X to Y mph"
    IF trimmed contains "to" THEN
        // Parse range: "5 to 10 mph"
        regex_pattern ← r"(\d+)\s+to\s+(\d+)\s+mph"
        matches ← regex::captures(trimmed, regex_pattern)

        IF matches is None THEN
            WARN("Failed to parse wind speed range: {}", trimmed)
            RETURN None
        END IF

        low ← matches[1].parse::<f64>()?
        high ← matches[2].parse::<f64>()?

        // Use average of range
        average_mph ← (low + high) / 2.0

        // Convert mph to m/s
        speed_ms ← average_mph * 0.44704
        RETURN Some(speed_ms)

    ELSE
        // Parse single value: "10 mph"
        regex_pattern ← r"(\d+)\s+mph"
        matches ← regex::captures(trimmed, regex_pattern)

        IF matches is None THEN
            WARN("Failed to parse wind speed: {}", trimmed)
            RETURN None
        END IF

        speed_mph ← matches[1].parse::<f64>()?

        // Convert mph to m/s
        speed_ms ← speed_mph * 0.44704
        RETURN Some(speed_ms)
    END IF
END

EXAMPLES:
    parse_wind_speed("10 mph") → Some(4.4704)
    parse_wind_speed("5 to 10 mph") → Some(3.3528)  # (7.5 mph * 0.44704)
    parse_wind_speed("15 mph") → Some(6.7056)
    parse_wind_speed("invalid") → None
```

**Conversion Factor**: 1 mph = 0.44704 m/s

**Error Handling**: Return None for unparseable formats, log warning

### Algorithm 3: Wind Direction Parsing

```
ALGORITHM: parse_wind_direction
INPUT: wind_direction_str (cardinal direction: "N", "NE", "E", etc.)
OUTPUT: Option<f64> (degrees, 0-360)

PURPOSE: Convert cardinal wind directions to numeric degrees

BEGIN
    trimmed ← wind_direction_str.trim().to_uppercase()

    // Map cardinal directions to degrees (meteorological convention)
    // 0° = North, 90° = East, 180° = South, 270° = West
    degrees ← MATCH trimmed AS
        CASE "N":   0.0
        CASE "NNE": 22.5
        CASE "NE":  45.0
        CASE "ENE": 67.5
        CASE "E":   90.0
        CASE "ESE": 112.5
        CASE "SE":  135.0
        CASE "SSE": 157.5
        CASE "S":   180.0
        CASE "SSW": 202.5
        CASE "SW":  225.0
        CASE "WSW": 247.5
        CASE "W":   270.0
        CASE "WNW": 292.5
        CASE "NW":  315.0
        CASE "NNW": 337.5
        CASE _:
            WARN("Unknown wind direction: {}", trimmed)
            RETURN None
    END MATCH

    RETURN Some(degrees)
END

EXAMPLES:
    parse_wind_direction("N") → Some(0.0)
    parse_wind_direction("NE") → Some(45.0)
    parse_wind_direction("S") → Some(180.0)
    parse_wind_direction("NW") → Some(315.0)
    parse_wind_direction("invalid") → None
```

**Cardinal Directions**: 16-point compass rose

**Edge Case**: "CALM" or empty string → Could map to None or special value

### Algorithm 4: Pressure Conversion

```
ALGORITHM: convert_pressure_pa_to_hpa
INPUT: pressure_pa (Pascals as f64)
OUTPUT: pressure_hpa (Hectopascals as f64)

BEGIN
    pressure_hpa ← pressure_pa / 100.0
    RETURN pressure_hpa
END

EXAMPLES:
    convert_pressure_pa_to_hpa(101325.0) → 1013.25  # Standard atmosphere
    convert_pressure_pa_to_hpa(100000.0) → 1000.0
```

**Note**: NWS API returns pressure in Pascals, we store in hectopascals (hPa) = millibars (mb)

---

## Enhanced Parser Factory

### Algorithm: Unified Parser Creation

**Extension of BUG-002 factory pattern with NWS parsers**

```
ALGORITHM: create_parser_from_config
INPUT: parser_name (String)
OUTPUT: Box<dyn ResponseParser> or CoreError

PURPOSE: Factory function to instantiate appropriate parser based on name

BEGIN
    // Step 1: Match on parser name
    parser ← MATCH parser_name AS
        CASE "flat_json":
            Box::new(FlatJsonParser::new())

        CASE "json_path":
            Box::new(JsonPathParser::new())

        CASE "nws_observations":
            Box::new(NwsObservationParser::new())

        CASE "nws_forecast_hourly":
            Box::new(NwsForecastParser::new())

        CASE unknown_name:
            RETURN CoreError::Config("Unknown parser name: {unknown_name}")
    END MATCH

    INFO("Created parser: {}", parser.name())

    RETURN Ok(parser)
END
```

**Parser Registry Pattern** (Alternative Design):

```
STRUCT: ParserRegistry
FIELDS:
    factories: HashMap<String, Box<dyn Fn() -> Box<dyn ResponseParser>>>

ALGORITHM: ParserRegistry.register
INPUT: parser_name (String), factory_fn (closure)

BEGIN
    factories.insert(parser_name, Box::new(factory_fn))
END

ALGORITHM: ParserRegistry.create
INPUT: parser_name (String)
OUTPUT: Box<dyn ResponseParser> or CoreError

BEGIN
    factory ← factories.get(parser_name)

    IF factory is None THEN
        RETURN CoreError::Config("Unknown parser: {parser_name}")
    END IF

    parser ← factory()
    RETURN Ok(parser)
END

USAGE:
    registry ← ParserRegistry::new()
    registry.register("nws_observations", || Box::new(NwsObservationParser::new()))
    registry.register("nws_forecast_hourly", || Box::new(NwsForecastParser::new()))

    parser ← registry.create("nws_observations")?
```

---

## Complexity Analysis

### NwsObservationParser

**Time Complexity**:
- JSON parsing: O(r) where r = response size (~2 KB)
- Field extraction: O(f) where f = number of fields (~7)
- Unit conversions: O(1) per field
- **Total**: O(r + f) ≈ O(r)

**Space Complexity**:
- Parsed JSON: O(r)
- Result vector: O(f)
- **Total**: O(r + f) ≈ O(r)

**Performance Estimate**:
- Response size: ~2 KB
- Parse time: <5 ms
- Total processing: <10 ms

### NwsForecastParser

**Time Complexity**:
- JSON parsing: O(r) where r = response size (~200 KB)
- Period iteration: O(n) where n = 156 periods
- Metrics per period: O(m) where m = 7 metrics
- String parsing (wind): O(s) per string, s = string length (~10 chars)
- **Total**: O(r + n × m × s) ≈ O(r + n × m)

**Space Complexity**:
- Parsed JSON: O(r)
- Result vector: O(n × m) = O(1,092) points
- **Total**: O(r + n × m)

**Performance Estimate**:
- Response size: ~200 KB
- Parse time: <50 ms
- Period processing: 156 × 7 × 2 ms ≈ 2.2 seconds (worst case)
- **Optimized estimate**: <500 ms with efficient allocations

**Optimization Notes**:
1. Pre-allocate result vector: `Vec::with_capacity(156 * 7)`
2. Reuse base_tags HashMap across periods (clone is cheap)
3. String parsing regex compiled once (lazy_static)
4. Avoid unnecessary allocations in tight loops

### Comparison with BUG-002 Parsers

| Parser | Time | Space | Notes |
|--------|------|-------|-------|
| FlatJsonParser (BUG-002) | O(r + f) | O(r + f) | f = all fields |
| JsonPathParser (BUG-002) | O(r + p × d) | O(r + p) | p = paths, d = depth |
| NwsObservationParser | O(r + f) | O(r + f) | Similar to Flat |
| NwsForecastParser | O(r + n × m) | O(r + n × m) | New: array iteration |

**Key Insight**: NwsForecastParser is most complex due to array iteration, but still linear in total data size.

---

## Error Handling Strategy

### Error Categories

```
ERROR TAXONOMY:

1. CONFIGURATION ERRORS (fail-fast at startup):
   - Unknown parser name in config
   - Invalid stream configuration
   - Parser registration conflicts

   ACTION: Return CoreError::Config, prevent source spawn

2. PARSING ERRORS (graceful degradation):
   - Malformed JSON response
   - Missing required fields (properties, generatedAt)
   - Invalid timestamp format
   - Type mismatches

   ACTION: Log error, skip message, continue polling

3. DATA QUALITY ERRORS (warn and skip):
   - Null values in optional fields
   - Unparseable wind speed strings
   - Unknown wind direction
   - Out-of-range values

   ACTION: Log warning, skip field, continue with other fields

4. VALIDATION ERRORS (warn but process):
   - Unexpected period count (!= 156)
   - Forecast periods not chronological
   - Negative lead times

   ACTION: Log warning, process available data

5. NETWORK ERRORS (retry with backoff):
   - HTTP timeout
   - Connection refused
   - 5xx server errors

   ACTION: Exponential backoff, retry up to 3 times
```

### Error Propagation

```
PARSE CALL CHAIN:

parse() → CoreResult<Vec<TimeSeriesPoint>>
    ├─ serde_json::from_str() → Result<GeoJson, serde_json::Error>
    │   └─ Map to CoreError::Source("Failed to parse JSON: {error}")
    │
    ├─ parse_rfc3339() → CoreResult<DateTime<Utc>>
    │   └─ Map to CoreError::Source("Invalid timestamp: {error}")
    │
    ├─ parse_wind_speed() → Option<f64>
    │   └─ None → WARN, skip field (no error)
    │
    └─ extract_quantity_field() → None (mutates vector)
        └─ Null value → DEBUG log, skip field

ERROR LOGGING STRATEGY:

ERROR level:
    - JSON parsing failures (malformed response)
    - Missing required fields (properties, generatedAt, timestamp)
    - Invalid timestamp formats

WARN level:
    - Unexpected period count (!= 156)
    - Unparseable wind speed/direction
    - Forecast periods not chronological
    - Negative lead times

INFO level:
    - Successful parse (point count)
    - Parser creation

DEBUG level:
    - Null optional fields
    - Field extraction details
```

### Retry Logic

```
ALGORITHM: poll_with_retry
INPUT: endpoint_config
OUTPUT: Vec<TimeSeriesPoint> or permanent failure

BEGIN
    max_attempts ← 3
    backoff_ms ← 1000  // Start with 1 second

    FOR attempt IN 1..=max_attempts DO
        result ← http_client.get(endpoint_config.url).send().await

        MATCH result AS
            CASE Ok(response):
                IF response.status.is_success() THEN
                    body ← response.text().await?
                    points ← parser.parse(body, location_id, timestamp)?
                    RETURN Ok(points)
                ELSE IF response.status.is_server_error() THEN
                    WARN("Server error (attempt {}/{}): {}", attempt, max_attempts, response.status)
                    // Retry on 5xx
                ELSE
                    // 4xx client error, don't retry
                    RETURN CoreError::Source("HTTP {}: {}", response.status, endpoint_config.url)
                END IF

            CASE Err(e):
                WARN("Request failed (attempt {}/{}): {}", attempt, max_attempts, e)
                // Retry on network errors
        END MATCH

        // Exponential backoff
        IF attempt < max_attempts THEN
            sleep(backoff_ms).await
            backoff_ms ← backoff_ms * 2
        END IF
    END FOR

    ERROR("Permanent failure after {} attempts", max_attempts)
    RETURN CoreError::Source("Max retries exceeded")
END
```

---

## Testing Strategy

### Unit Tests for NwsObservationParser

```
TEST SUITE: NwsObservationParser

TEST: parse_valid_observation_response
INPUT:
    {
      "properties": {
        "timestamp": "2025-12-21T12:00:00+00:00",
        "temperature": {"value": 20.5, "unitCode": "wmoUnit:degC"},
        "dewpoint": {"value": 15.2, "unitCode": "wmoUnit:degC"},
        "relativeHumidity": {"value": 65.0, "unitCode": "wmoUnit:percent"},
        "windSpeed": {"value": 15.0, "unitCode": "wmoUnit:km_h-1"},
        "windDirection": {"value": 180.0, "unitCode": "wmoUnit:degree_(angle)"},
        "barometricPressure": {"value": 101325.0, "unitCode": "wmoUnit:Pa"},
        "visibility": {"value": 16000.0, "unitCode": "wmoUnit:m"}
      }
    }
EXPECTED:
    - 7 TimeSeriesPoints (temperature, dewpoint, humidity, wind_speed, wind_direction, pressure, visibility)
    - temperature = 20.5
    - pressure = 1013.25 hPa (converted from 101325 Pa)
    - wind_speed = 4.167 m/s (converted from 15 km/h)
    - All tags include "source"="nws", "station"="KSGJ"

TEST: parse_observation_with_null_fields
INPUT:
    {
      "properties": {
        "timestamp": "2025-12-21T12:00:00+00:00",
        "temperature": {"value": 20.5, "unitCode": "wmoUnit:degC"},
        "dewpoint": null,
        "relativeHumidity": {"value": null, "unitCode": "wmoUnit:percent"},
        "windSpeed": null
      }
    }
EXPECTED:
    - 1 TimeSeriesPoint (only temperature)
    - Null fields skipped gracefully
    - No errors thrown

TEST: parse_observation_missing_timestamp
INPUT:
    {
      "properties": {
        "temperature": {"value": 20.5, "unitCode": "wmoUnit:degC"}
      }
    }
EXPECTED: CoreError::Source("Missing observation timestamp")

TEST: parse_observation_invalid_json
INPUT: "{ invalid json }"
EXPECTED: CoreError::Source("Failed to parse NWS observation GeoJSON")

TEST: parse_observation_missing_properties
INPUT: { "type": "Feature" }
EXPECTED: CoreError::Source("Missing 'properties' in GeoJSON response")
```

### Unit Tests for NwsForecastParser

```
TEST SUITE: NwsForecastParser

TEST: parse_valid_forecast_response
INPUT:
    {
      "properties": {
        "generatedAt": "2025-12-21T12:00:00+00:00",
        "periods": [
          {
            "startTime": "2025-12-21T13:00:00-05:00",
            "temperature": 72,
            "temperatureUnit": "F",
            "dewpoint": {"value": 16.0, "unitCode": "wmoUnit:degC"},
            "relativeHumidity": {"value": 65, "unitCode": "wmoUnit:percent"},
            "windSpeed": "10 mph",
            "windDirection": "S",
            "probabilityOfPrecipitation": {"value": 20, "unitCode": "wmoUnit:percent"},
            "shortForecast": "Partly Cloudy"
          },
          {
            "startTime": "2025-12-21T14:00:00-05:00",
            "temperature": 73,
            ...
          }
        ]
      }
    }
EXPECTED:
    - 14 TimeSeriesPoints (2 periods × 7 metrics)
    - temperature = 22.222 °C (converted from 72°F)
    - wind_speed = 4.4704 m/s (converted from 10 mph)
    - wind_direction = 180.0° (converted from "S")
    - All tags include "issue_time", "forecast_valid_time", "lead_time_hours"

TEST: parse_forecast_with_wind_speed_range
INPUT:
    "windSpeed": "5 to 10 mph"
EXPECTED:
    - wind_speed = 3.3528 m/s  # (7.5 mph average × 0.44704)

TEST: parse_forecast_with_all_wind_directions
INPUT: Array of periods with directions: N, NE, E, SE, S, SW, W, NW
EXPECTED: Correct degree mappings: 0, 45, 90, 135, 180, 225, 270, 315

TEST: parse_forecast_unexpected_period_count
INPUT: Response with 100 periods (instead of 156)
EXPECTED:
    - 700 TimeSeriesPoints (100 × 7)
    - Warning logged: "Unexpected period count: 100 (expected 156)"

TEST: parse_forecast_invalid_timestamp
INPUT:
    "generatedAt": "invalid-date"
EXPECTED: CoreError::Source("Invalid generatedAt timestamp")

TEST: parse_forecast_negative_lead_time
INPUT:
    "generatedAt": "2025-12-21T12:00:00Z"
    "periods[0].startTime": "2025-12-21T10:00:00Z"  (before generatedAt)
EXPECTED:
    - Warning logged
    - Period processed (graceful handling)

TEST: parse_forecast_empty_periods
INPUT:
    { "properties": { "generatedAt": "...", "periods": [] } }
EXPECTED: CoreError::Source("No forecast periods in response")
```

### Unit Tests for String Parsing Algorithms

```
TEST SUITE: parse_wind_speed

TEST: single_value_mph
INPUT: "10 mph"
EXPECTED: Some(4.4704)

TEST: range_value_mph
INPUT: "5 to 10 mph"
EXPECTED: Some(3.3528)  # Average: 7.5 mph

TEST: whitespace_handling
INPUT: "  15  mph  "
EXPECTED: Some(6.7056)

TEST: invalid_format
INPUT: "fast"
EXPECTED: None

TEST SUITE: parse_wind_direction

TEST: all_16_directions
INPUT: ["N", "NNE", "NE", "ENE", "E", "ESE", "SE", "SSE", "S", "SSW", "SW", "WSW", "W", "WNW", "NW", "NNW"]
EXPECTED: [0.0, 22.5, 45.0, 67.5, 90.0, 112.5, 135.0, 157.5, 180.0, 202.5, 225.0, 247.5, 270.0, 292.5, 315.0, 337.5]

TEST: lowercase_input
INPUT: "ne"
EXPECTED: Some(45.0)  # Normalized to uppercase

TEST: invalid_direction
INPUT: "X"
EXPECTED: None

TEST SUITE: fahrenheit_to_celsius

TEST: freezing_point
INPUT: 32.0
EXPECTED: 0.0

TEST: boiling_point
INPUT: 212.0
EXPECTED: 100.0

TEST: room_temperature
INPUT: 72.0
EXPECTED: 22.222...
```

### Integration Tests

```
TEST SUITE: End-to-End Integration

TEST: nws_observations_stream_ingestion
SETUP:
    - Mock NWS API endpoint with sample observation
    - Configure stream with NwsObservationParser
    - Start GenericHttpPollingSource

VERIFY:
    - HTTP request made with correct User-Agent header
    - Parser invoked with response body
    - 7 TimeSeriesPoints written to Parquet
    - Timestamps in UTC
    - Units converted correctly

TEST: nws_forecast_stream_ingestion
SETUP:
    - Mock NWS API endpoint with 156-period forecast
    - Configure stream with NwsForecastParser
    - Start GenericHttpPollingSource

VERIFY:
    - 1,092 TimeSeriesPoints written (156 periods × 7 metrics)
    - issue_time and forecast_valid_time in tags
    - lead_time_hours computed correctly
    - Parquet file size ~50 KB (compressed)

TEST: forecast_evolution_query
SETUP:
    - Ingest 3 forecast polls at different times
    - Same forecast_valid_time appears in all 3

VERIFY:
    - Query "WHERE forecast_valid_time = X" returns 3 forecasts
    - Sorted by issue_time (oldest to newest)
    - Temperature values show convergence

TEST: api_failure_recovery
SETUP:
    - Mock API returns 503 Server Error twice, then succeeds

VERIFY:
    - Retry logic invoked
    - Exponential backoff applied
    - Success on 3rd attempt
    - Data ingested successfully
```

---

## Integration with Existing System

### GenericHttpPollingSource Integration

```
ALGORITHM: GenericHttpPollingSource.run (EXISTING)
INPUT: None (reads from config)
OUTPUT: None (infinite loop)

BEGIN
    // Load stream configuration
    stream_config ← load_stream_config_from_etcd()

    // Create parser (NEW: supports NWS parsers)
    parser ← create_parser_from_config(stream_config.parser_name)?

    INFO("HTTP polling source started with parser: {}", parser.name())

    // Polling loop
    interval ← tokio::time::interval(stream_config.poll_interval)

    LOOP
        interval.tick().await

        FOR EACH endpoint IN stream_config.endpoints DO
            result ← CALL poll_endpoint_with_retry(endpoint)

            MATCH result AS
                CASE Ok(response_body):
                    // Parse response using configured parser
                    timestamp ← Utc::now()
                    points ← parser.parse(response_body, endpoint.location_id, timestamp)

                    MATCH points AS
                        CASE Ok(point_vec):
                            // Send all points to channel
                            FOR EACH point IN point_vec DO
                                tx.send(point).await?
                            END FOR

                            INFO("Ingested {} points from {}", point_vec.len(), endpoint.endpoint_id)

                        CASE Err(e):
                            ERROR("Parse error for {}: {}", endpoint.endpoint_id, e)
                            // Continue polling (graceful degradation)
                    END MATCH

                CASE Err(e):
                    ERROR("HTTP poll failed for {}: {}", endpoint.endpoint_id, e)
                    // Continue polling
            END MATCH
        END FOR
    END LOOP
END
```

**Key Points**:
1. GenericHttpPollingSource is **unchanged** - polymorphism via ResponseParser trait
2. Parser selection via `stream_config.parser_name` string
3. Error handling graceful (continue polling on parse errors)

### Stream Configuration Loading

```
YAML CONFIG (nws-observations.yaml):

sources:
  - type: http_poll
    enabled: true
    poll_interval_secs: 600
    timeout_secs: 30
    parser_name: nws_observations  ← Selects parser
    endpoints:
      - endpoint_id: ksgj_observations
        location_id: ksgj
        url: "https://api.weather.gov/stations/KSGJ/observations/latest"
        headers:
          User-Agent: "(NeuralDataPlatform, contact@example.com)"
          Accept: "application/geo+json"

RUST DESERIALIZATION:

STRUCT: SourceConfig
FIELDS:
    source_type: String        // "http_poll"
    enabled: bool
    poll_interval_secs: u64
    timeout_secs: u64
    parser_name: String        // "nws_observations" ← NEW field
    endpoints: Vec<EndpointConfig>

ALGORITHM: load_stream_config
INPUT: stream_id (String)
OUTPUT: StreamConfig or CoreError

BEGIN
    // Load from etcd
    yaml_str ← etcd_client.get(f"/streams/{stream_id}/config.yaml").await?

    // Deserialize
    config ← serde_yaml::from_str::<StreamConfig>(yaml_str)?

    // Validate parser exists
    parser ← create_parser_from_config(config.sources[0].parser_name)?

    RETURN Ok(config)
END
```

### IngestionCoordinator Flow

```
DATA FLOW:

NWS API
  ↓ HTTP GET (GenericHttpPollingSource)
  ↓
ResponseParser::parse() (NwsObservationParser or NwsForecastParser)
  ↓
Vec<TimeSeriesPoint>
  ↓ mpsc::Sender<TimeSeriesPoint>
  ↓
IngestionCoordinator (channel router)
  ↓ Route by stream_id in TimeSeriesPoint.tags
  ↓
ParquetStore::write_batch()
  ↓
Bronze layer Parquet files
```

**No changes required to**:
- IngestionCoordinator (polymorphic over TimeSeriesPoint)
- ParquetStore (generic storage)
- Channel infrastructure (mpsc)

**Only additions**:
1. New parser implementations (NwsObservationParser, NwsForecastParser)
2. Parser registry entries
3. Stream configurations (YAML files)

---

## Summary

### Algorithms Defined

| Algorithm | Purpose | Complexity |
|-----------|---------|------------|
| ArrayIterationParser | Parse forecast periods (tall format) | O(n × m) |
| NwsObservationParser.parse | Extract current weather observations | O(r + f) |
| NwsForecastParser.parse | Extract hourly forecast with evolution tracking | O(r + n × m) |
| parse_rfc3339 | Parse ISO 8601 timestamps with timezone | O(1) |
| extract_forecast_timestamps | Extract and validate issue_time + valid_times | O(n) |
| fahrenheit_to_celsius | Temperature unit conversion | O(1) |
| parse_wind_speed | Parse NWS wind speed strings | O(s) |
| parse_wind_direction | Convert cardinal directions to degrees | O(1) |
| convert_pressure_pa_to_hpa | Pressure unit conversion | O(1) |
| create_parser_from_config | Factory pattern for parser instantiation | O(1) |

### Extension of BUG-002

✅ **Inherited Patterns**:
- Parser trait definition
- FlatJson and JsonPath concepts
- Error handling strategy
- Factory pattern
- Unit test patterns

✅ **New Patterns**:
- Array iteration algorithm (forecast periods)
- Multi-timestamp extraction (issue_time, forecast_valid_time)
- String parsing transforms (wind speed, direction)
- GeoJSON navigation
- Tall format data model

### Data Model Highlights

1. **Absolute Timestamps**: issue_time and forecast_valid_time stored in tags as Unix timestamps
2. **Tall Format**: One TimeSeriesPoint per metric per forecast period (1,092 points per poll)
3. **Lead Time Computation**: `(forecast_valid_time - issue_time) / 3600` stored in tags
4. **Forecast Evolution**: Query `WHERE forecast_valid_time = X` returns all forecasts for that time

### Performance Characteristics

| Operation | Time | Space | Notes |
|-----------|------|-------|-------|
| Parse Observations | <10 ms | ~2 KB | 7 fields extracted |
| Parse Forecast | <500 ms | ~200 KB | 156 periods × 7 metrics |
| Storage per Day | ~40 MB | Parquet compressed | 144 polls × 1,092 points |

### Next Phase

**Refinement (TDD Implementation)**:
1. Implement `NwsObservationParser` with unit tests
2. Implement `NwsForecastParser` with unit tests
3. Implement string parsing helpers (wind speed, direction)
4. Register parsers in factory
5. Create stream configurations
6. Integration tests with mock HTTP responses
7. Deploy and verify with real NWS API

---

## References

### Internal Documents
- [BUG-002 Pseudocode](/workspaces/neural-data-platform/product/features/dp-001/bugs/BUG-002-CONFIG-DRIVEN-PARSING-PSEUDO.md)
- [AIR-006 Specification](/workspaces/neural-data-platform/product/features/air-006/specification/SPECIFICATION.md)
- [AIR-006 Scope](/workspaces/neural-data-platform/product/features/air-006/SCOPE.md)

### External References
- [NWS API Documentation](https://www.weather.gov/documentation/services-web-api)
- [GeoJSON Specification](https://geojson.org/)
- [RFC 3339 (ISO 8601)](https://datatracker.ietf.org/doc/html/rfc3339)

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0.0 | 2025-12-21 | NDP Algorithm Designer | Initial pseudocode extending BUG-002 patterns |

---

*This pseudocode follows the SPARC methodology and extends the config-driven parser architecture from BUG-002. Next phase: Architecture design for system integration.*
