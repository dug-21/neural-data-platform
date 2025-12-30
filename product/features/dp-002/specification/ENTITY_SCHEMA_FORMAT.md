# Entity Schema YAML Format Specification

**Document Type**: SPARC Specification
**Version**: 1.0.0
**Last Updated**: 2025-12-30
**Status**: Draft

---

## Overview

This document defines the standardized YAML format for `entity_schemas` in NDP stream configuration files. Entity schemas serve as the data dictionary definition mechanism, enabling:

- Documentation of expected data attributes
- Data quality validation
- Dynamic dashboard generation
- Pattern matching for HomeAssistant entities

---

## Design Principles

1. **Additive Only**: Entity schemas are ADDED to existing configs; `fields` sections remain unchanged
2. **Single Source of Truth**: `entity_schemas` is THE data dictionary definition
3. **Pattern Support**: Schema names can use wildcards for HomeAssistant entity matching
4. **Self-Documenting**: Every attribute includes description and units

---

## YAML Structure

### Top-Level Format

```yaml
# Existing stream configuration...
stream_id: {stream-id}
description: {stream description}
version: "{semver}"
enabled: true

# Existing fields section (DO NOT MODIFY)
fields:
  # ... existing field definitions ...

# NEW: Entity schemas section
entity_schemas:
  - schema_name: {unique-schema-name}
    description: {human-readable description}
    device_class: {optional device classification}
    attributes:
      - name: {attribute_name}
        type: {data_type}
        unit: {measurement_unit}
        description: {human-readable description}
        nullable: {true|false}
```

### Field Reference

#### Schema Definition

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `schema_name` | string | Yes | Unique identifier for this schema within the stream. Supports wildcards for pattern matching (e.g., `sensor.airgradient_*`) |
| `description` | string | Yes | Human-readable description of what this schema represents |
| `device_class` | string | No | HomeAssistant device classification (e.g., `air_quality`, `temperature`, `window`) |
| `attributes` | array | Yes | List of attribute definitions |

#### Attribute Definition

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Attribute name in snake_case format (1-64 chars) |
| `type` | string | Yes | Data type: `float`, `int`, `string`, `bool`, `json`, `timestamp` |
| `unit` | string | No | Measurement unit (e.g., `ug/m3`, `celsius`, `percent`) |
| `description` | string | Yes | Human-readable description of the attribute |
| `nullable` | boolean | No | Whether null values are allowed (default: `true`) |
| `range` | array | No | Valid value range as [min, max] for numeric types |

---

## Naming Conventions

### Schema Names

| Stream Type | Format | Example |
|-------------|--------|---------|
| Single-source streams | `{source-name}` | `airgradient`, `nws-weather` |
| Multi-source streams | `{source-name}` per source | `nws-observations`, `nws-hourly` |
| HomeAssistant entities | `{domain}.{pattern}` | `sensor.airgradient_*`, `binary_sensor.*_window*` |

### Attribute Names

- Use **snake_case** format
- Length: 1-64 characters
- Start with lowercase letter
- Allowed: lowercase letters, digits, underscores
- Examples: `pm25`, `temperature`, `wind_speed`, `co2_level`

### Unit Formats

Standard units used across NDP:

| Category | Units |
|----------|-------|
| Temperature | `celsius`, `fahrenheit`, `kelvin` |
| Concentration | `ug/m3`, `ppm`, `ppb` |
| Percentage | `percent` |
| Pressure | `hpa`, `pa`, `mbar` |
| Speed | `m/s`, `km/h`, `mph` |
| Distance | `meters`, `km`, `miles` |
| Direction | `degrees` |
| Time | `seconds`, `milliseconds`, `epoch_seconds` |
| Indices | `index`, `1-5_scale`, `aqi_scale` |

---

## Data Types

| Type | Description | Example Values |
|------|-------------|----------------|
| `float` | Floating-point number | `23.5`, `-5.0`, `1013.25` |
| `int` | Integer | `42`, `1500`, `0` |
| `string` | Text string | `"Partly Cloudy"`, `"open"` |
| `bool` | Boolean | `true`, `false` |
| `json` | JSON object/array | `{"key": "value"}` |
| `timestamp` | ISO 8601 timestamp | `"2025-12-30T15:00:00Z"` |

---

## Pattern Matching (HomeAssistant)

### Wildcard Syntax

Entity schemas support wildcard patterns for matching HomeAssistant entity IDs:

| Pattern | Matches | Example |
|---------|---------|---------|
| `*` | Any characters | `sensor.airgradient_*` matches `sensor.airgradient_abc123_pm25` |
| `?` | Single character | `sensor.temp_?` matches `sensor.temp_1`, `sensor.temp_2` |

### Pattern Examples

```yaml
entity_schemas:
  # Match all AirGradient sensors
  - schema_name: "sensor.airgradient_*"
    device_class: air_quality

  # Match all window binary sensors
  - schema_name: "binary_sensor.*_window*"
    device_class: window

  # Match temperature sensors by room
  - schema_name: "sensor.*_temperature"
    device_class: temperature

  # Match specific device pattern
  - schema_name: "sensor.aqara_*_humidity"
    device_class: humidity
```

### Matching Algorithm

When matching an entity ID to a schema:

1. Replace `*` with SQL `%` for LIKE matching
2. Replace `?` with SQL `_` for single-character matching
3. Match against all schemas for the stream
4. Return first matching schema (most specific wins)

```sql
-- Example: Match entity to schema
SELECT * FROM entity_schemas
WHERE 'sensor.airgradient_abc123_pm25' LIKE REPLACE(REPLACE(schema_name, '*', '%'), '?', '_');
```

---

## Complete Examples

### Example 1: AirGradient Indoor Air Quality

```yaml
# config/base/streams/air-quality/config.yaml

stream_id: air-quality
description: AirGradient sensor readings from MQTT
version: "1.0.0"
enabled: true
retention_days: 365
compression_after_days: 7
partitioning_strategy: daily

# Existing fields (DO NOT MODIFY)
fields:
  pm25:
    type: "float"
    unit: "ug/m3"
    description: "Particulate Matter 2.5 micrometers"
    nullable: false
  # ... other fields ...

# Entity schemas for data dictionary
entity_schemas:
  - schema_name: airgradient
    description: AirGradient indoor air quality sensors (AG One, AG Pro)
    device_class: air_quality
    attributes:
      - name: pm25
        type: float
        unit: ug/m3
        description: Particulate Matter 2.5 micrometers
        nullable: false
        range: [0, 1000]
      - name: pm10
        type: float
        unit: ug/m3
        description: Particulate Matter 10 micrometers
        nullable: true
        range: [0, 1000]
      - name: co2
        type: int
        unit: ppm
        description: Carbon Dioxide concentration
        nullable: true
        range: [400, 10000]
      - name: temperature
        type: float
        unit: celsius
        description: Ambient temperature
        nullable: true
        range: [-40, 85]
      - name: humidity
        type: float
        unit: percent
        description: Relative humidity
        nullable: true
        range: [0, 100]
      - name: tvoc
        type: int
        unit: ppb
        description: Total Volatile Organic Compounds
        nullable: true
        range: [0, 60000]
      - name: nox
        type: int
        unit: ppb
        description: Nitrogen Oxides index
        nullable: true
        range: [1, 500]
```

### Example 2: NWS Weather Observations

```yaml
# config/base/streams/nws-observations/config.yaml

entity_schemas:
  - schema_name: nws-observations
    description: Real-time weather observations from NWS station KSGJ
    attributes:
      - name: temperature
        type: float
        unit: celsius
        description: Ambient air temperature
        nullable: true
        range: [-50, 60]
      - name: dewpoint
        type: float
        unit: celsius
        description: Dew point temperature
        nullable: true
        range: [-50, 60]
      - name: wind_direction
        type: float
        unit: degrees
        description: Wind direction in degrees from north
        nullable: true
        range: [0, 360]
      - name: wind_speed
        type: float
        unit: km/h
        description: Wind speed
        nullable: true
        range: [0, 300]
      - name: wind_gust
        type: float
        unit: km/h
        description: Wind gust speed
        nullable: true
        range: [0, 400]
      - name: barometric_pressure
        type: float
        unit: pa
        description: Barometric pressure
        nullable: true
        range: [80000, 110000]
      - name: sea_level_pressure
        type: float
        unit: pa
        description: Sea level pressure
        nullable: true
        range: [80000, 110000]
      - name: visibility
        type: float
        unit: meters
        description: Visibility distance
        nullable: true
        range: [0, 50000]
      - name: relative_humidity
        type: float
        unit: percent
        description: Relative humidity
        nullable: true
        range: [0, 100]
      - name: precipitation_1h
        type: float
        unit: meters
        description: Precipitation in last hour
        nullable: true
        range: [0, 1]
      - name: heat_index
        type: float
        unit: celsius
        description: Heat index temperature
        nullable: true
        range: [-50, 60]
      - name: wind_chill
        type: float
        unit: celsius
        description: Wind chill temperature
        nullable: true
        range: [-50, 60]
```

### Example 3: Outdoor Air Quality (OpenWeatherMap)

```yaml
# config/base/streams/outdoor-air-quality/config.yaml

entity_schemas:
  - schema_name: airnow
    description: Outdoor air quality data from OpenWeatherMap Air Pollution API
    attributes:
      - name: aqi
        type: float
        unit: 1-5_scale
        description: Air Quality Index (1=Good, 5=Very Poor)
        nullable: false
        range: [1, 5]
      - name: co
        type: float
        unit: ug/m3
        description: Carbon Monoxide concentration
        nullable: true
        range: [0, 50000]
      - name: no
        type: float
        unit: ug/m3
        description: Nitrogen Monoxide concentration
        nullable: true
        range: [0, 1000]
      - name: no2
        type: float
        unit: ug/m3
        description: Nitrogen Dioxide concentration
        nullable: true
        range: [0, 1000]
      - name: o3
        type: float
        unit: ug/m3
        description: Ozone concentration
        nullable: true
        range: [0, 1000]
      - name: so2
        type: float
        unit: ug/m3
        description: Sulfur Dioxide concentration
        nullable: true
        range: [0, 1000]
      - name: pm2_5
        type: float
        unit: ug/m3
        description: Particulate Matter 2.5 micrometers
        nullable: false
        range: [0, 1000]
      - name: pm10
        type: float
        unit: ug/m3
        description: Particulate Matter 10 micrometers
        nullable: true
        range: [0, 1000]
      - name: nh3
        type: float
        unit: ug/m3
        description: Ammonia concentration
        nullable: true
        range: [0, 200]
```

### Example 4: NWS Hourly Forecast

```yaml
# config/base/streams/nws-forecast-hourly/config.yaml

entity_schemas:
  - schema_name: nws-hourly
    description: Hourly weather forecast from NWS gridpoint forecast
    attributes:
      - name: temperature
        type: float
        unit: fahrenheit
        description: Forecast temperature
        nullable: false
        range: [-50, 130]
      - name: dewpoint
        type: float
        unit: celsius
        description: Forecast dew point
        nullable: true
        range: [-50, 60]
      - name: relative_humidity
        type: float
        unit: percent
        description: Forecast relative humidity
        nullable: true
        range: [0, 100]
      - name: wind_speed
        type: float
        unit: mph
        description: Forecast wind speed
        nullable: true
        range: [0, 200]
      - name: wind_direction
        type: float
        unit: degrees
        description: Forecast wind direction
        nullable: true
        range: [0, 360]
      - name: short_forecast
        type: string
        description: Brief forecast description
        nullable: true
      - name: probability_of_precipitation
        type: float
        unit: percent
        description: Precipitation probability
        nullable: true
        range: [0, 100]
      - name: forecast_issue_time
        type: float
        unit: epoch_seconds
        description: Forecast issue timestamp as epoch seconds
        nullable: true
```

### Example 5: NWS Gridpoints (Comprehensive)

```yaml
# config/base/streams/nws-gridpoints-forecast/config.yaml

entity_schemas:
  - schema_name: nws-gridpoints
    description: Raw NWS gridpoint forecast data with 40+ comprehensive weather metrics
    attributes:
      # Temperature Suite
      - name: temperature
        type: float
        unit: celsius
        description: Forecast temperature
        nullable: true
        range: [-50, 60]
      - name: dewpoint
        type: float
        unit: celsius
        description: Forecast dewpoint temperature
        nullable: true
        range: [-50, 60]
      - name: max_temperature
        type: float
        unit: celsius
        description: Daily maximum temperature
        nullable: true
        range: [-50, 60]
      - name: min_temperature
        type: float
        unit: celsius
        description: Daily minimum temperature
        nullable: true
        range: [-50, 60]
      - name: apparent_temperature
        type: float
        unit: celsius
        description: Feels-like temperature
        nullable: true
        range: [-60, 70]
      - name: wet_bulb_globe_temperature
        type: float
        unit: celsius
        description: Wet bulb globe temperature (heat stress)
        nullable: true
        range: [-50, 60]
      - name: heat_index
        type: float
        unit: celsius
        description: Heat index (when applicable)
        nullable: true
        range: [-50, 70]
      - name: wind_chill
        type: float
        unit: celsius
        description: Wind chill (when applicable)
        nullable: true
        range: [-70, 20]
      # Wind Suite
      - name: wind_speed
        type: float
        unit: km/h
        description: Forecast wind speed
        nullable: true
        range: [0, 300]
      - name: wind_direction
        type: float
        unit: degrees
        description: Forecast wind direction
        nullable: true
        range: [0, 360]
      - name: wind_gust
        type: float
        unit: km/h
        description: Forecast wind gust speed
        nullable: true
        range: [0, 400]
      # Precipitation Suite
      - name: probability_of_precipitation
        type: float
        unit: percent
        description: Precipitation probability
        nullable: true
        range: [0, 100]
      - name: quantitative_precipitation
        type: float
        unit: mm
        description: Quantitative precipitation forecast
        nullable: true
        range: [0, 500]
      - name: snowfall_amount
        type: float
        unit: mm
        description: Snowfall amount
        nullable: true
        range: [0, 1000]
      # Sky & Visibility
      - name: sky_cover
        type: float
        unit: percent
        description: Cloud cover percentage
        nullable: true
        range: [0, 100]
      - name: visibility
        type: float
        unit: meters
        description: Visibility distance
        nullable: true
        range: [0, 50000]
      # Humidity
      - name: relative_humidity
        type: float
        unit: percent
        description: Relative humidity
        nullable: true
        range: [0, 100]
      # Fire Weather & Indices
      - name: probability_of_thunder
        type: float
        unit: percent
        description: Probability of thunderstorms
        nullable: true
        range: [0, 100]
      - name: haines_index
        type: float
        unit: index
        description: Haines index (fire weather)
        nullable: true
        range: [2, 6]
      # Marine (Coastal)
      - name: wave_height
        type: float
        unit: meters
        description: Wave height (coastal areas)
        nullable: true
        range: [0, 30]
      # ... additional fields as defined in config
```

### Example 6: HomeAssistant Stream with Pattern Matching

```yaml
# config/base/streams/homeassistant/config.yaml

stream_id: homeassistant
description: Home Assistant entity states via MQTT Statestream
version: "1.0.0"
enabled: false  # Enable when HA integration is active
retention_days: 365
compression_after_days: 7
partitioning_strategy: daily

# Generic Bronze layer schema (captures all HA entities)
fields:
  - name: entity_id
    type: string
    nullable: false
    description: Home Assistant entity identifier
  - name: state
    type: string
    nullable: false
    description: Current state value
  - name: last_changed
    type: timestamp
    nullable: true
    description: When state last changed
  - name: last_updated
    type: timestamp
    nullable: true
    description: When entity was last updated
  - name: attributes
    type: json
    nullable: true
    description: Entity-specific attributes

sources:
  - type: mqtt
    enabled: true
    params:
      broker_url: "mosquitto"
      port: 1883
      client_id: "ndp-homeassistant"
      topic_pattern: "homeassistant/+/+/state"
      qos: 1

# Entity schemas for specific device types
entity_schemas:
  # AirGradient sensors via Home Assistant
  - schema_name: "sensor.airgradient_*"
    description: AirGradient air quality sensors exposed through Home Assistant
    device_class: air_quality
    attributes:
      - name: pm02
        type: float
        unit: ug/m3
        description: Particulate Matter 2.5 (HA naming convention)
        nullable: true
        range: [0, 1000]
      - name: pm10
        type: float
        unit: ug/m3
        description: Particulate Matter 10
        nullable: true
        range: [0, 1000]
      - name: atmp
        type: float
        unit: celsius
        description: Ambient temperature
        nullable: true
        range: [-40, 85]
      - name: rhum
        type: float
        unit: percent
        description: Relative humidity
        nullable: true
        range: [0, 100]
      - name: rco2
        type: int
        unit: ppm
        description: CO2 concentration
        nullable: true
        range: [400, 10000]
      - name: tvoc
        type: int
        unit: ppb
        description: Total VOC index
        nullable: true
        range: [0, 60000]

  # Window/door contact sensors
  - schema_name: "binary_sensor.*_window*"
    description: Window contact sensors (open/closed)
    device_class: window
    attributes:
      - name: state
        type: string
        description: Window state (on=open, off=closed)
        nullable: false
      - name: device_class
        type: string
        description: Entity device class
        nullable: true
      - name: battery
        type: int
        unit: percent
        description: Battery level
        nullable: true
        range: [0, 100]

  # Temperature sensors
  - schema_name: "sensor.*_temperature"
    description: Temperature sensors from any device
    device_class: temperature
    attributes:
      - name: state
        type: float
        unit: celsius
        description: Temperature reading
        nullable: false
        range: [-40, 100]
      - name: unit_of_measurement
        type: string
        description: Unit of measurement
        nullable: true

  # Humidity sensors
  - schema_name: "sensor.*_humidity"
    description: Humidity sensors from any device
    device_class: humidity
    attributes:
      - name: state
        type: float
        unit: percent
        description: Humidity reading
        nullable: false
        range: [0, 100]
```

### Example 7: Outdoor Weather (OpenWeatherMap)

```yaml
# config/base/streams/outdoor-weather/config.yaml

entity_schemas:
  - schema_name: nws-weather
    description: Outdoor weather data from OpenWeatherMap Current Weather API
    attributes:
      - name: temperature
        type: float
        unit: celsius
        description: Current temperature
        nullable: false
        range: [-50, 60]
      - name: feels_like
        type: float
        unit: celsius
        description: Feels-like temperature
        nullable: true
        range: [-50, 60]
      - name: pressure
        type: float
        unit: hpa
        description: Atmospheric pressure at sea level
        nullable: true
        range: [800, 1200]
      - name: humidity
        type: float
        unit: percent
        description: Relative humidity
        nullable: true
        range: [0, 100]
      - name: wind_speed
        type: float
        unit: m/s
        description: Wind speed
        nullable: true
        range: [0, 100]
      - name: wind_deg
        type: float
        unit: degrees
        description: Wind direction in degrees
        nullable: true
        range: [0, 360]
      - name: wind_gust
        type: float
        unit: m/s
        description: Wind gust speed
        nullable: true
        range: [0, 150]
      - name: clouds
        type: float
        unit: percent
        description: Cloudiness percentage
        nullable: true
        range: [0, 100]
      - name: visibility
        type: float
        unit: meters
        description: Visibility distance
        nullable: true
        range: [0, 50000]
      - name: rain_1h
        type: float
        unit: mm
        description: Rain volume for last 1 hour
        nullable: true
        range: [0, 500]
      - name: snow_1h
        type: float
        unit: mm
        description: Snow volume for last 1 hour
        nullable: true
        range: [0, 500]
```

---

## Validation Rules

### Schema Validation

| Rule | Description |
|------|-------------|
| V1 | `schema_name` is required and unique within stream |
| V2 | `description` is required |
| V3 | `attributes` array must have at least one element |
| V4 | No duplicate attribute names within schema |

### Attribute Validation

| Rule | Description |
|------|-------------|
| V5 | `name` is required, 1-64 chars, snake_case |
| V6 | `type` is required, must be valid type |
| V7 | `description` is required |
| V8 | `range` must be [min, max] where min < max |
| V9 | `range` only valid for float/int types |

### Error Messages

```yaml
# Example validation errors
errors:
  - "Schema 'my-schema' is missing required field 'description'"
  - "Attribute 'Temperature' uses invalid format (must be snake_case)"
  - "Attribute 'pm25' has invalid range [100, 0] (min must be less than max)"
  - "Schema 'duplicate-schema' already exists in stream 'air-quality'"
```

---

## Migration Guide

### Adding Entity Schemas to Existing Stream

1. **Do NOT modify the `fields` section**
2. Add `entity_schemas:` after the `fields` section
3. Create one schema per logical data source
4. Include all attributes with proper types and units
5. Sync to data dictionary: `./deploy.sh sync-dictionary`
6. Verify in Grafana dashboard

### Example Migration Diff

```diff
 fields:
   pm25:
     type: "float"
     unit: "ug/m3"
     ...

+entity_schemas:
+  - schema_name: airgradient
+    description: AirGradient indoor air quality sensors
+    device_class: air_quality
+    attributes:
+      - name: pm25
+        type: float
+        unit: ug/m3
+        description: Particulate Matter 2.5 micrometers
+        nullable: false
```

---

## Relationship to `fields`

### Purpose Comparison

| Concept | Purpose | Used By |
|---------|---------|---------|
| `fields` | Bronze Parquet column schema | Ingestion engine (technical) |
| `entity_schemas` | Data dictionary entries | Data dictionary, DQ dashboard, documentation |

### Why Both Exist

- `fields` defines what the ingestion engine writes to Parquet
- `entity_schemas` documents what the data means for consumers
- They may have the same attributes but serve different purposes
- Accept temporary duplication for ingestion stability

### When They Differ

| Scenario | `fields` | `entity_schemas` |
|----------|----------|------------------|
| Ingestion adds metadata | Includes `source`, `stream_id` tags | Only documents data attributes |
| HomeAssistant stream | Generic: entity_id, state, attributes | Specific per device type |
| Multi-source stream | Union of all sources | One schema per source |

---

## Best Practices

1. **Document Everything**: Include descriptions for all attributes
2. **Use Standard Units**: Follow the unit conventions table
3. **Specify Ranges**: Help identify data quality issues
4. **Group Logically**: One schema per logical entity type
5. **Pattern Wisely**: Use wildcards only when needed
6. **Keep Consistent**: Follow existing stream patterns

---

*This document is part of the SPARC Specification phase for DP-002.*
