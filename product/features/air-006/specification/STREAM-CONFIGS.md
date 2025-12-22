# Stream Configuration Reference - air-006

**Feature**: National Weather Service Integration
**Date**: 2025-12-21
**Status**: Specification Phase

This document provides complete YAML configurations for all five data streams in the Neural Data Platform, including the two new NWS streams introduced in air-006.

---

## Table of Contents

1. [Stream 1: air-quality (MQTT - AirGradient)](#stream-1-air-quality)
2. [Stream 2: outdoor-weather (HTTP - OpenWeatherMap Current)](#stream-2-outdoor-weather)
3. [Stream 3: outdoor-air-quality (HTTP - OpenWeatherMap Pollution)](#stream-3-outdoor-air-quality)
4. [Stream 4: nws-observations (HTTP - NWS Current)](#stream-4-nws-observations) ⭐ NEW
5. [Stream 5: nws-forecast-hourly (HTTP - NWS Forecast)](#stream-5-nws-forecast-hourly) ⭐ NEW
6. [Parser Type Reference](#parser-type-reference)
7. [Validation Notes](#validation-notes)

---

## Stream 1: air-quality

**Type**: MQTT Push
**Source**: AirGradient indoor air quality sensors
**Parser**: `flat_json` - Direct mapping from JSON fields to metrics
**Location**: Dynamic (from `serialno` field)

### Complete Configuration

```yaml
# Air Quality Stream Configuration
# GitOps managed - synced to etcd at /streams/air-quality/*

stream_id: "air-quality"
description: "AirGradient sensor readings from MQTT"
version: "1.0.0"
enabled: true
retention_days: 365
compression_after_days: 7
partitioning_strategy: "daily"

# Data schema fields
fields:
  pm25:
    type: "float"
    unit: "µg/m³"
    description: "Particulate Matter 2.5 micrometers"
    nullable: false
  pm10:
    type: "float"
    unit: "µg/m³"
    description: "Particulate Matter 10 micrometers"
    nullable: true
  co2:
    type: "int"
    unit: "ppm"
    description: "Carbon Dioxide concentration"
    nullable: true
  temperature:
    type: "float"
    unit: "celsius"
    description: "Ambient temperature"
    nullable: true
  humidity:
    type: "float"
    unit: "percent"
    description: "Relative humidity"
    nullable: true
  tvoc:
    type: "int"
    unit: "ppb"
    description: "Total Volatile Organic Compounds"
    nullable: true
  nox:
    type: "int"
    unit: "ppb"
    description: "Nitrogen Oxides"
    nullable: true

# Data sources configuration (SourceManager format)
sources:
  - source_type: mqtt
    enabled: true
    params:
      broker_url: "mosquitto"
      port: 1883
      client_id: "air-quality-app"
      topic_pattern: "airgradient/readings/+"
      qos: 1
      reconnect_delay_secs: 1
      max_reconnect_delay_secs: 30
      buffer_capacity: 1000
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
        stream_id: air-quality

# Storage configuration
storage:
  batch_size: 100
  batch_timeout_secs: 5
  buffer_capacity: 1000
```

### Parser Behavior

**Type**: `flat_json`

- **Location ID**: Extracted from `serialno` field in JSON payload
- **Timestamp**: Uses current system time (sensors don't provide timestamps)
- **Field Mapping**: Direct 1:1 mapping from JSON keys to metric names
- **Skip Fields**: Metadata fields excluded from time-series storage
- **Tags**: All data points tagged with `source=mqtt` and `stream_id=air-quality`

**Example Input**:
```json
{
  "serialno": "84fce612f684",
  "pm25": 12.3,
  "co2": 654,
  "temperature": 22.5,
  "humidity": 45.2,
  "firmware": "3.0.4"
}
```

**Parsed Output**:
```
location_id: "84fce612f684"
timestamp: <current_time>
metrics: {pm25: 12.3, co2: 654, temperature: 22.5, humidity: 45.2}
tags: {source: "mqtt", stream_id: "air-quality"}
```

---

## Stream 2: outdoor-weather

**Type**: HTTP Poll
**Source**: OpenWeatherMap Current Weather API
**Parser**: `json_path` - JSONPath expressions to extract nested fields
**Location**: Static (`home`)

### Complete Configuration

```yaml
stream_id: outdoor-weather
description: Outdoor weather data from OpenWeatherMap Current Weather API
version: "1.0.0"
enabled: true
retention_days: 90
compression_after_days: 7
partitioning_strategy: daily

fields:
  - name: temperature
    type: float
    nullable: false
    unit: celsius
    range: [-50.0, 60.0]
  - name: feels_like
    type: float
    nullable: true
    unit: celsius
    range: [-50.0, 60.0]
  - name: pressure
    type: float
    nullable: true
    unit: hpa
    range: [800.0, 1200.0]
  - name: humidity
    type: float
    nullable: true
    unit: percent
    range: [0.0, 100.0]
  - name: wind_speed
    type: float
    nullable: true
    unit: m/s
    range: [0.0, 100.0]
  - name: wind_deg
    type: float
    nullable: true
    unit: degrees
    range: [0.0, 360.0]
  - name: wind_gust
    type: float
    nullable: true
    unit: m/s
    range: [0.0, 150.0]
  - name: clouds
    type: float
    nullable: true
    unit: percent
    range: [0.0, 100.0]
  - name: visibility
    type: float
    nullable: true
    unit: meters
    range: [0.0, 50000.0]
  - name: rain_1h
    type: float
    nullable: true
    unit: mm
    range: [0.0, 500.0]
  - name: snow_1h
    type: float
    nullable: true
    unit: mm
    range: [0.0, 500.0]

sources:
  - type: http_poll
    enabled: true
    poll_interval_secs: 600
    timeout_secs: 30
    parser_name: openweathermap_current_weather
    endpoints:
      - endpoint_id: openweathermap_weather
        location_id: home
        lat: 29.95838
        lon: -81.30878
        url: "https://api.openweathermap.org/data/2.5/weather?lat=29.95838&lon=-81.30878&units=metric"
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

storage:
  batch_size: 50
  batch_timeout_secs: 30
  buffer_capacity: 500
```

### Parser Behavior

**Type**: `json_path`

- **Location ID**: From `name` field (e.g., "St Augustine"), fallback to `${OWM_LOCATION_NAME}`
- **Timestamp**: Current time when API is polled
- **Field Mapping**: JSONPath expressions navigate nested JSON structure
- **Polling**: Every 10 minutes (600 seconds)

**Example Input**:
```json
{
  "name": "St Augustine",
  "main": {
    "temp": 24.5,
    "feels_like": 25.1,
    "pressure": 1013,
    "humidity": 65
  },
  "wind": {
    "speed": 3.2,
    "deg": 180,
    "gust": 5.1
  },
  "clouds": {"all": 20},
  "visibility": 10000
}
```

**Parsed Output**:
```
location_id: "home"
timestamp: <poll_time>
metrics: {
  temperature: 24.5,
  feels_like: 25.1,
  pressure: 1013,
  humidity: 65,
  wind_speed: 3.2,
  wind_deg: 180,
  wind_gust: 5.1,
  clouds: 20,
  visibility: 10000
}
tags: {source: "openweathermap", api: "current_weather", stream_id: "outdoor-weather"}
```

---

## Stream 3: outdoor-air-quality

**Type**: HTTP Poll
**Source**: OpenWeatherMap Air Pollution API
**Parser**: `json_path` - JSONPath with array indexing
**Location**: Static (`home`)

### Complete Configuration

```yaml
stream_id: outdoor-air-quality
description: Outdoor air quality data from OpenWeatherMap Air Pollution API
version: "1.0.0"
enabled: true
retention_days: 90
compression_after_days: 7
partitioning_strategy: daily

fields:
  - name: aqi
    type: float
    nullable: false
    unit: 1-5_scale
    range: [1.0, 5.0]
  - name: co
    type: float
    nullable: true
    unit: μg/m³
    range: [0.0, 50000.0]
  - name: no
    type: float
    nullable: true
    unit: μg/m³
    range: [0.0, 1000.0]
  - name: no2
    type: float
    nullable: true
    unit: μg/m³
    range: [0.0, 1000.0]
  - name: o3
    type: float
    nullable: true
    unit: μg/m³
    range: [0.0, 1000.0]
  - name: so2
    type: float
    nullable: true
    unit: μg/m³
    range: [0.0, 1000.0]
  - name: pm2_5
    type: float
    nullable: false
    unit: μg/m³
    range: [0.0, 1000.0]
  - name: pm10
    type: float
    nullable: true
    unit: μg/m³
    range: [0.0, 1000.0]
  - name: nh3
    type: float
    nullable: true
    unit: μg/m³
    range: [0.0, 200.0]

sources:
  - type: http_poll
    enabled: true
    poll_interval_secs: 600
    timeout_secs: 30
    parser_name: openweathermap_air_pollution
    endpoints:
      - endpoint_id: openweathermap_air_pollution
        location_id: home
        lat: 29.95838
        lon: -81.30878
        url: "https://api.openweathermap.org/data/2.5/air_pollution?lat=29.95838&lon=-81.30878"
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

storage:
  batch_size: 50
  batch_timeout_secs: 30
  buffer_capacity: 500
```

### Parser Behavior

**Type**: `json_path`

- **Location ID**: From `coord` field (lat/lon), fallback to `${OWM_LOCATION_NAME}`
- **Timestamp**: Current time when API is polled
- **Field Mapping**: JSONPath with array indexing (`list[0]`) to extract first element
- **Polling**: Every 10 minutes (600 seconds)

**Example Input**:
```json
{
  "coord": {"lon": -81.30878, "lat": 29.95838},
  "list": [
    {
      "main": {"aqi": 2},
      "components": {
        "co": 234.5,
        "no": 0.12,
        "no2": 8.45,
        "o3": 68.32,
        "so2": 1.23,
        "pm2_5": 12.8,
        "pm10": 18.4,
        "nh3": 0.56
      }
    }
  ]
}
```

**Parsed Output**:
```
location_id: "home"
timestamp: <poll_time>
metrics: {
  aqi: 2,
  co: 234.5,
  no: 0.12,
  no2: 8.45,
  o3: 68.32,
  so2: 1.23,
  pm2_5: 12.8,
  pm10: 18.4,
  nh3: 0.56
}
tags: {source: "openweathermap", api: "air_pollution", stream_id: "outdoor-air-quality"}
```

---

## Stream 4: nws-observations

⭐ **NEW** - Introduced in air-006

**Type**: HTTP Poll
**Source**: National Weather Service Station Observations API
**Parser**: `json_path` - Single observation with timestamp extraction
**Location**: Static (`ksgj` - St. Augustine Airport)

### Complete Configuration

```yaml
stream_id: nws-observations
description: Real-time weather observations from NWS station KSGJ
version: "1.0.0"
enabled: true
retention_days: 365
compression_after_days: 7
partitioning_strategy: daily

fields:
  - name: temperature
    type: float
    nullable: true
    unit: celsius
    range: [-50.0, 60.0]
    description: "Ambient air temperature"
  - name: dewpoint
    type: float
    nullable: true
    unit: celsius
    range: [-50.0, 60.0]
    description: "Dew point temperature"
  - name: wind_direction
    type: float
    nullable: true
    unit: degrees
    range: [0.0, 360.0]
    description: "Wind direction in degrees from north"
  - name: wind_speed
    type: float
    nullable: true
    unit: km/h
    range: [0.0, 300.0]
    description: "Wind speed"
  - name: wind_gust
    type: float
    nullable: true
    unit: km/h
    range: [0.0, 400.0]
    description: "Wind gust speed"
  - name: barometric_pressure
    type: float
    nullable: true
    unit: pa
    range: [80000.0, 110000.0]
    description: "Barometric pressure"
  - name: sea_level_pressure
    type: float
    nullable: true
    unit: pa
    range: [80000.0, 110000.0]
    description: "Sea level pressure"
  - name: visibility
    type: float
    nullable: true
    unit: meters
    range: [0.0, 50000.0]
    description: "Visibility distance"
  - name: max_temperature_24h
    type: float
    nullable: true
    unit: celsius
    range: [-50.0, 60.0]
    description: "Maximum temperature in last 24 hours"
  - name: min_temperature_24h
    type: float
    nullable: true
    unit: celsius
    range: [-50.0, 60.0]
    description: "Minimum temperature in last 24 hours"
  - name: precipitation_1h
    type: float
    nullable: true
    unit: meters
    range: [0.0, 1.0]
    description: "Precipitation in last hour"
  - name: precipitation_3h
    type: float
    nullable: true
    unit: meters
    range: [0.0, 1.0]
    description: "Precipitation in last 3 hours"
  - name: precipitation_6h
    type: float
    nullable: true
    unit: meters
    range: [0.0, 1.0]
    description: "Precipitation in last 6 hours"
  - name: relative_humidity
    type: float
    nullable: true
    unit: percent
    range: [0.0, 100.0]
    description: "Relative humidity"
  - name: wind_chill
    type: float
    nullable: true
    unit: celsius
    range: [-50.0, 60.0]
    description: "Wind chill temperature"
  - name: heat_index
    type: float
    nullable: true
    unit: celsius
    range: [-50.0, 60.0]
    description: "Heat index temperature"

sources:
  - type: http_poll
    enabled: true
    poll_interval_secs: 300  # 5 minutes (NWS updates hourly but we check frequently)
    timeout_secs: 30
    retry_attempts: 3
    retry_delay_secs: 5
    parser_name: nws_observations
    endpoints:
      - endpoint_id: nws_ksgj_observations
        location_id: ksgj
        station_id: KSGJ
        url: "https://api.weather.gov/stations/KSGJ/observations/latest"
        auth_type: none
        headers:
          User-Agent: "(neural-data-platform, contact@example.com)"
          Accept: "application/geo+json"
    parser:
      parser_type: json_path
      location_id_field: properties.station
      default_location_id: ksgj
      timestamp_field: properties.timestamp  # Use observation time, not poll time
      timestamp_format: iso8601
      default_tags:
        source: nws
        api: observations
        stream_id: nws-observations
        station_id: KSGJ
      field_mappings:
        - path: properties.temperature.value
          metric_name: temperature
          unit: celsius
        - path: properties.dewpoint.value
          metric_name: dewpoint
          unit: celsius
        - path: properties.windDirection.value
          metric_name: wind_direction
          unit: degrees
        - path: properties.windSpeed.value
          metric_name: wind_speed
          unit: km/h
        - path: properties.windGust.value
          metric_name: wind_gust
          unit: km/h
        - path: properties.barometricPressure.value
          metric_name: barometric_pressure
          unit: pa
        - path: properties.seaLevelPressure.value
          metric_name: sea_level_pressure
          unit: pa
        - path: properties.visibility.value
          metric_name: visibility
          unit: meters
        - path: properties.maxTemperatureLast24Hours.value
          metric_name: max_temperature_24h
          unit: celsius
        - path: properties.minTemperatureLast24Hours.value
          metric_name: min_temperature_24h
          unit: celsius
        - path: properties.precipitationLastHour.value
          metric_name: precipitation_1h
          unit: meters
        - path: properties.precipitationLast3Hours.value
          metric_name: precipitation_3h
          unit: meters
        - path: properties.precipitationLast6Hours.value
          metric_name: precipitation_6h
          unit: meters
        - path: properties.relativeHumidity.value
          metric_name: relative_humidity
          unit: percent
        - path: properties.windChill.value
          metric_name: wind_chill
          unit: celsius
        - path: properties.heatIndex.value
          metric_name: heat_index
          unit: celsius

storage:
  batch_size: 50
  batch_timeout_secs: 30
  buffer_capacity: 500
```

### Parser Behavior

**Type**: `json_path`

- **Location ID**: Extracted from `properties.station` (URL like `/stations/KSGJ`), fallback to `ksgj`
- **Timestamp**: Extracted from `properties.timestamp` (ISO 8601 format from observation)
- **Field Mapping**: All fields nested under `properties.<field>.value`
- **Polling**: Every 5 minutes (NWS updates hourly, but we check frequently for timely data)
- **Headers Required**: `User-Agent` with contact info (NWS requirement)

**Example Input**:
```json
{
  "properties": {
    "station": "https://api.weather.gov/stations/KSGJ",
    "timestamp": "2025-12-21T14:53:00+00:00",
    "temperature": {"value": 18.3, "unitCode": "wmoUnit:degC"},
    "dewpoint": {"value": 12.2, "unitCode": "wmoUnit:degC"},
    "windDirection": {"value": 180, "unitCode": "wmoUnit:degree_(angle)"},
    "windSpeed": {"value": 12.96, "unitCode": "wmoUnit:km_h-1"},
    "barometricPressure": {"value": 101325, "unitCode": "wmoUnit:Pa"},
    "relativeHumidity": {"value": 68.5, "unitCode": "wmoUnit:percent"}
  }
}
```

**Parsed Output**:
```
location_id: "ksgj"
timestamp: 2025-12-21T14:53:00Z  # From properties.timestamp
metrics: {
  temperature: 18.3,
  dewpoint: 12.2,
  wind_direction: 180,
  wind_speed: 12.96,
  barometric_pressure: 101325,
  relative_humidity: 68.5
}
tags: {source: "nws", api: "observations", stream_id: "nws-observations", station_id: "KSGJ"}
```

---

## Stream 5: nws-forecast-hourly

⭐ **NEW** - Introduced in air-006

**Type**: HTTP Poll
**Source**: National Weather Service Gridpoint Forecast API
**Parser**: `array_iterator` - Iterates array of forecast periods
**Location**: Static (`ksgj`)

### Complete Configuration

```yaml
stream_id: nws-forecast-hourly
description: Hourly weather forecast from NWS gridpoint forecast
version: "1.0.0"
enabled: true
retention_days: 30  # Forecasts are short-lived data
compression_after_days: 7
partitioning_strategy: daily

fields:
  - name: temperature
    type: float
    nullable: false
    unit: fahrenheit
    range: [-50.0, 130.0]
    description: "Forecast temperature"
  - name: dewpoint
    type: float
    nullable: true
    unit: celsius
    range: [-50.0, 60.0]
    description: "Forecast dew point"
  - name: relative_humidity
    type: float
    nullable: true
    unit: percent
    range: [0.0, 100.0]
    description: "Forecast relative humidity"
  - name: wind_speed
    type: float
    nullable: true
    unit: mph
    range: [0.0, 200.0]
    description: "Forecast wind speed"
  - name: wind_direction
    type: float
    nullable: true
    unit: degrees
    range: [0.0, 360.0]
    description: "Forecast wind direction"
  - name: short_forecast
    type: string
    nullable: true
    description: "Brief forecast description"
  - name: probability_of_precipitation
    type: float
    nullable: true
    unit: percent
    range: [0.0, 100.0]
    description: "Precipitation probability"

sources:
  - type: http_poll
    enabled: true
    poll_interval_secs: 3600  # 1 hour (NWS updates hourly)
    timeout_secs: 30
    retry_attempts: 3
    retry_delay_secs: 5
    parser_name: nws_forecast_hourly
    endpoints:
      - endpoint_id: nws_jax_79_49_forecast
        location_id: ksgj
        grid_office: JAX
        grid_x: 79
        grid_y: 49
        url: "https://api.weather.gov/gridpoints/JAX/79,49/forecast/hourly"
        auth_type: none
        headers:
          User-Agent: "(neural-data-platform, contact@example.com)"
          Accept: "application/geo+json"
    parser:
      parser_type: array_iterator
      location_id_field: properties.gridId  # Will be "JAX"
      default_location_id: ksgj
      array_path: properties.periods  # Array of forecast periods
      timestamp_field: startTime  # Each period has its own startTime
      timestamp_format: iso8601
      metadata_tags:
        - path: properties.generatedAt
          tag_name: forecast_generated_at
          value_type: timestamp
        - path: properties.updateTime
          tag_name: forecast_update_time
          value_type: timestamp
      default_tags:
        source: nws
        api: forecast_hourly
        stream_id: nws-forecast-hourly
        grid_office: JAX
        grid_x: "79"
        grid_y: "49"
      element_mappings:
        - path: temperature
          metric_name: temperature
          unit: fahrenheit
        - path: dewpoint.value
          metric_name: dewpoint
          unit: celsius
        - path: relativeHumidity.value
          metric_name: relative_humidity
          unit: percent
        - path: windSpeed
          metric_name: wind_speed
          string_parse:
            pattern: "^(\\d+)\\s*(?:to\\s*(\\d+)\\s*)?mph$"
            capture_group: 1  # Take first number from "5 to 10 mph"
            fallback_value: null
          unit: mph
        - path: windDirection
          metric_name: wind_direction
          enum_map:
            N: 0
            NNE: 22.5
            NE: 45
            ENE: 67.5
            E: 90
            ESE: 112.5
            SE: 135
            SSE: 157.5
            S: 180
            SSW: 202.5
            SW: 225
            WSW: 247.5
            W: 270
            WNW: 292.5
            NW: 315
            NNW: 337.5
          unit: degrees
        - path: shortForecast
          metric_name: short_forecast
          value_type: string
        - path: probabilityOfPrecipitation.value
          metric_name: probability_of_precipitation
          unit: percent

storage:
  batch_size: 156  # 156 hours = 6.5 days of forecast
  batch_timeout_secs: 60
  buffer_capacity: 200
```

### Parser Behavior

**Type**: `array_iterator` ⭐ NEW PARSER TYPE

- **Location ID**: From `properties.gridId` (e.g., "JAX"), fallback to `ksgj`
- **Timestamp**: Extracted from each period's `startTime` (not poll time!)
- **Array Iteration**: Extracts `properties.periods` array and processes each element
- **Metadata Tags**: Forecast generation/update times attached to all points
- **String Parsing**: Custom regex for wind speed ("5 to 10 mph" → 5.0)
- **Enum Mapping**: Cardinal directions to degrees (N → 0, E → 90, etc.)
- **Polling**: Every 1 hour (NWS updates hourly forecasts)

**Example Input**:
```json
{
  "properties": {
    "generatedAt": "2025-12-21T14:30:00+00:00",
    "updateTime": "2025-12-21T14:25:00+00:00",
    "gridId": "JAX",
    "periods": [
      {
        "number": 1,
        "startTime": "2025-12-21T15:00:00+00:00",
        "endTime": "2025-12-21T16:00:00+00:00",
        "temperature": 72,
        "temperatureUnit": "F",
        "dewpoint": {"value": 18.3, "unitCode": "wmoUnit:degC"},
        "relativeHumidity": {"value": 65, "unitCode": "wmoUnit:percent"},
        "windSpeed": "5 to 10 mph",
        "windDirection": "SE",
        "shortForecast": "Partly Cloudy",
        "probabilityOfPrecipitation": {"value": 20, "unitCode": "wmoUnit:percent"}
      },
      {
        "number": 2,
        "startTime": "2025-12-21T16:00:00+00:00",
        "endTime": "2025-12-21T17:00:00+00:00",
        "temperature": 73,
        "temperatureUnit": "F",
        "dewpoint": {"value": 18.9, "unitCode": "wmoUnit:degC"},
        "relativeHumidity": {"value": 63, "unitCode": "wmoUnit:percent"},
        "windSpeed": "10 mph",
        "windDirection": "S",
        "shortForecast": "Mostly Sunny",
        "probabilityOfPrecipitation": {"value": 10, "unitCode": "wmoUnit:percent"}
      }
      // ... 154 more periods (156 total for 6.5 days)
    ]
  }
}
```

**Parsed Output** (2 points from 2 periods):
```
# Point 1
location_id: "ksgj"
timestamp: 2025-12-21T15:00:00Z  # From periods[0].startTime
metrics: {
  temperature: 72,
  dewpoint: 18.3,
  relative_humidity: 65,
  wind_speed: 5,  # Parsed from "5 to 10 mph"
  wind_direction: 135,  # SE → 135 degrees
  short_forecast: "Partly Cloudy",
  probability_of_precipitation: 20
}
tags: {
  source: "nws",
  api: "forecast_hourly",
  stream_id: "nws-forecast-hourly",
  grid_office: "JAX",
  grid_x: "79",
  grid_y: "49",
  forecast_generated_at: "2025-12-21T14:30:00Z",
  forecast_update_time: "2025-12-21T14:25:00Z"
}

# Point 2
location_id: "ksgj"
timestamp: 2025-12-21T16:00:00Z  # From periods[1].startTime
metrics: {
  temperature: 73,
  dewpoint: 18.9,
  relative_humidity: 63,
  wind_speed: 10,  # Parsed from "10 mph"
  wind_direction: 180,  # S → 180 degrees
  short_forecast: "Mostly Sunny",
  probability_of_precipitation: 10
}
tags: {
  source: "nws",
  api: "forecast_hourly",
  stream_id: "nws-forecast-hourly",
  grid_office: "JAX",
  grid_x: "79",
  grid_y: "49",
  forecast_generated_at: "2025-12-21T14:30:00Z",
  forecast_update_time: "2025-12-21T14:25:00Z"
}
```

**Key Differences from Other Parsers**:
- **Multiple Points**: One API response → 156 time-series points (one per forecast hour)
- **Timestamp Extraction**: Each period provides its own `startTime` (future timestamps)
- **String Parsing**: Custom regex to extract numeric values from strings
- **Enum Mapping**: Text values (N, SE, W) converted to numeric degrees
- **Metadata Tags**: Forecast metadata attached to all points in the batch

---

## Parser Type Reference

### 1. flat_json

**Used By**: `air-quality`

**Behavior**:
- Assumes flat JSON structure (no nesting)
- Direct 1:1 mapping from JSON keys to metric names
- Location ID from specified field
- Timestamp from current system time

**When to Use**:
- Simple JSON payloads without nesting
- MQTT sensor data
- Direct field mapping

**Example**:
```yaml
parser:
  parser_type: flat_json
  location_id_field: serialno
  skip_fields: [metadata1, metadata2]
  default_tags:
    source: mqtt
```

### 2. json_path

**Used By**: `outdoor-weather`, `outdoor-air-quality`, `nws-observations`

**Behavior**:
- Uses JSONPath expressions to navigate nested JSON
- Supports array indexing (`list[0]`)
- Extracts values from deeply nested structures
- Timestamp from current time or extracted field

**When to Use**:
- Nested JSON structures
- REST API responses
- Single observation per response

**Example**:
```yaml
parser:
  parser_type: json_path
  location_id_field: name
  timestamp_field: properties.timestamp  # Optional
  timestamp_format: iso8601
  field_mappings:
    - path: main.temp
      metric_name: temperature
      unit: celsius
    - path: list[0].components.pm2_5
      metric_name: pm2_5
      unit: ug/m3
```

### 3. array_iterator

⭐ **NEW** - Used By: `nws-forecast-hourly`

**Behavior**:
- Extracts array from JSON response
- Iterates over each element
- Creates one time-series point per array element
- Each element provides its own timestamp
- Supports string parsing and enum mapping

**When to Use**:
- API returns array of observations/forecasts
- Each element represents a different time period
- Batch processing of multiple time points

**Advanced Features**:
- **String Parsing**: Regex extraction from text fields
- **Enum Mapping**: Convert categorical values to numeric
- **Metadata Tags**: Attach response-level metadata to all points

**Example**:
```yaml
parser:
  parser_type: array_iterator
  array_path: properties.periods
  timestamp_field: startTime
  timestamp_format: iso8601
  metadata_tags:
    - path: properties.generatedAt
      tag_name: forecast_generated_at
      value_type: timestamp
  element_mappings:
    - path: temperature
      metric_name: temperature
      unit: fahrenheit
    - path: windSpeed
      metric_name: wind_speed
      string_parse:
        pattern: "^(\\d+)\\s*(?:to\\s*(\\d+)\\s*)?mph$"
        capture_group: 1
      unit: mph
    - path: windDirection
      metric_name: wind_direction
      enum_map:
        N: 0
        E: 90
        S: 180
        W: 270
      unit: degrees
```

---

## Validation Notes

### Configuration File Locations

```
/workspaces/neural-data-platform/config/base/streams/
├── air-quality/
│   └── config.yaml               ✅ EXISTS
├── outdoor-weather/
│   └── config.yaml               ✅ EXISTS
├── outdoor-air-quality/
│   └── config.yaml               ✅ EXISTS
├── nws-observations/
│   └── config.yaml               ⭐ TO BE CREATED (air-006)
└── nws-forecast-hourly/
    └── config.yaml               ⭐ TO BE CREATED (air-006)
```

### Environment Variables Required

| Variable | Used By | Purpose |
|----------|---------|---------|
| `OPENWEATHERMAP_API_KEY` | outdoor-weather, outdoor-air-quality | API authentication |
| `OWM_LOCATION_NAME` | outdoor-weather, outdoor-air-quality | Fallback location ID |
| `MQTT_BROKER_URL` | air-quality | MQTT broker address (default: mosquitto) |

**Note**: NWS APIs do not require API keys, only a `User-Agent` header with contact info.

### Polling Intervals

| Stream | Interval | Rationale |
|--------|----------|-----------|
| air-quality | N/A (push) | MQTT publishes every 60 seconds |
| outdoor-weather | 600s (10 min) | OWM free tier limit, weather changes slowly |
| outdoor-air-quality | 600s (10 min) | OWM free tier limit, air quality changes slowly |
| nws-observations | 300s (5 min) | NWS updates hourly, check frequently for timely data |
| nws-forecast-hourly | 3600s (1 hour) | NWS updates hourly forecasts |

### Storage Configuration

| Stream | Batch Size | Timeout | Buffer | Rationale |
|--------|------------|---------|--------|-----------|
| air-quality | 100 | 5s | 1000 | High-frequency MQTT stream |
| outdoor-weather | 50 | 30s | 500 | Low-frequency polling |
| outdoor-air-quality | 50 | 30s | 500 | Low-frequency polling |
| nws-observations | 50 | 30s | 500 | Moderate frequency |
| nws-forecast-hourly | 156 | 60s | 200 | Batch of 156 forecast points |

### Retention Policies

| Stream | Retention | Compression After | Rationale |
|--------|-----------|-------------------|-----------|
| air-quality | 365 days | 7 days | Long-term indoor air quality analysis |
| outdoor-weather | 90 days | 7 days | Seasonal weather patterns |
| outdoor-air-quality | 90 days | 7 days | Seasonal air quality trends |
| nws-observations | 365 days | 7 days | Long-term weather history |
| nws-forecast-hourly | 30 days | 7 days | Forecasts are ephemeral (compare to actuals) |

### Unit Conversions

Different APIs use different unit systems. The parser configurations preserve original units:

| Metric | air-quality | outdoor-weather | outdoor-air-quality | nws-observations | nws-forecast-hourly |
|--------|-------------|-----------------|---------------------|------------------|---------------------|
| Temperature | celsius | celsius | - | celsius | **fahrenheit** |
| Wind Speed | - | m/s | - | km/h | **mph** |
| Pressure | - | hpa | - | **pa** | - |
| PM2.5 | µg/m³ | - | µg/m³ | - | - |

**Note**: Unit conversion happens in the Silver layer (TimescaleDB) with continuous aggregates.

### Parser Implementation Status

| Parser Type | Implementation Status | Used By |
|-------------|----------------------|---------|
| `flat_json` | ✅ Implemented | air-quality |
| `json_path` | ✅ Implemented | outdoor-weather, outdoor-air-quality, nws-observations |
| `array_iterator` | ⭐ **TO BE IMPLEMENTED** (air-006) | nws-forecast-hourly |

### New Parser Features Required for air-006

The `array_iterator` parser type requires these new capabilities:

1. **Array Extraction**: Extract array from JSONPath
2. **Element Iteration**: Process each array element as a separate point
3. **Timestamp Per Element**: Use element's timestamp field (not poll time)
4. **String Parsing**: Regex extraction from string fields
5. **Enum Mapping**: Convert categorical values to numeric
6. **Metadata Tags**: Attach response-level metadata to all points

**Implementation Files**:
- `/workspaces/neural-data-platform/core/src/sources/http_polling/parsers/array_iterator.rs` (NEW)
- `/workspaces/neural-data-platform/core/src/sources/http_polling/parsers/mod.rs` (UPDATE)

---

## Summary

This document provides complete, validated YAML configurations for all five data streams:

1. **air-quality**: MQTT push from AirGradient sensors (`flat_json`)
2. **outdoor-weather**: HTTP poll from OpenWeatherMap Current (`json_path`)
3. **outdoor-air-quality**: HTTP poll from OpenWeatherMap Pollution (`json_path`)
4. **nws-observations**: ⭐ NEW - HTTP poll from NWS Station API (`json_path`)
5. **nws-forecast-hourly**: ⭐ NEW - HTTP poll from NWS Forecast API (`array_iterator`)

**Next Steps**:
1. Implement `array_iterator` parser type
2. Create configuration files for NWS streams
3. Add NWS streams to SourceManager initialization
4. Test end-to-end data flow from NWS APIs to Parquet storage

---

**Document Status**: ✅ COMPLETE
**Last Updated**: 2025-12-21
**Feature Phase**: Specification (SPARC S)
