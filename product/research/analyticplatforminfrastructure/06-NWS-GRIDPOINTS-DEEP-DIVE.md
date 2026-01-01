# NWS Gridpoints API Deep Dive

## Overview

This document analyzes the NWS Gridpoints API response structure to inform Silver layer ETL design.

## API Endpoint

```
GET https://api.weather.gov/gridpoints/{office}/{gridX},{gridY}
Example: https://api.weather.gov/gridpoints/JAX/80,50
```

## Response Structure

```json
{
  "@context": ["https://geojson.org/geojson-ld/geojson-context.jsonld", {...}],
  "geometry": {
    "type": "Polygon",
    "coordinates": [[[-81.308, 29.9575], ...]]
  },
  "id": "https://api.weather.gov/gridpoints/JAX/80,50",
  "properties": {
    "@id": "https://api.weather.gov/gridpoints/JAX/80,50",
    "@type": "wx:Gridpoint",
    "updateTime": "2026-01-01T13:56:49+00:00",  // <-- ISSUE_TIME
    "validTimes": "2026-01-01T07:00:00+00:00/P7DT18H",
    "elevation": {"unitCode": "wmoUnit:m", "value": 0.9144},
    "forecastOffice": "https://api.weather.gov/offices/JAX",
    "gridId": "JAX",
    "gridX": 80,
    "gridY": 50,

    // ~60 metric objects, each with this structure:
    "temperature": {
      "uom": "wmoUnit:degC",
      "values": [
        {"validTime": "2026-01-01T07:00:00+00:00/PT2H", "value": 3.333...},
        {"validTime": "2026-01-01T09:00:00+00:00/PT2H", "value": 2.777...},
        // ~130 values covering 7+ days
      ]
    },
    "dewpoint": {...},
    "windSpeed": {...},
    // etc.
  }
}
```

## Key Observations

### 1. Timestamp Format: ISO 8601 Intervals

```
"validTime": "2026-01-01T07:00:00+00:00/PT2H"
             ├─────────────────────────────┤ ├──┤
                    Start Time              Duration
```

- Start time in RFC 3339 format
- Duration in ISO 8601 format (PT1H, PT2H, PT6H, P1D, etc.)
- Meaning: "This value is valid for 2 hours starting at 07:00"

### 2. Variable Granularity

Different metrics have different time resolutions:

| Metric | Typical Values | Granularity |
|--------|---------------|-------------|
| temperature | ~130 | Hourly (PT1H, PT2H) |
| maxTemperature | ~8 | Daily |
| minTemperature | ~8 | Daily |
| probabilityOfPrecipitation | ~30 | 6-hourly (PT6H) |
| quantitativePrecipitation | ~30 | 6-hourly |
| weather | ~10 | Variable (conditions change) |

### 3. Irregular Time Series

The `values` array does NOT have uniform time steps. Consecutive values might be:
- PT1H, PT1H, PT1H, PT2H, PT1H (variable)
- Or: PT6H, PT6H, PT6H (consistent for precip)

### 4. Null Values

Some metrics return `null` for certain periods:
```json
{"validTime": "2026-01-01T16:00:00+00:00/PT10H", "value": null}
```

This indicates the metric doesn't apply (e.g., wind chill when temperature > threshold).

### 5. Empty Arrays

Some metrics are empty for this location:
```json
"atmosphericDispersionIndex": {"values": []},
"primarySwellHeight": {"values": []},
```

These are location-dependent (marine metrics for coastal areas only).

### 6. Issue Time (updateTime)

```json
"updateTime": "2026-01-01T13:56:49+00:00"
```

This is when NWS generated the forecast. Critical for:
- Calculating lead_time
- Tracking forecast revisions
- Audit trail

### 7. WMO Units

```json
"uom": "wmoUnit:degC"
"uom": "wmoUnit:km_h-1"
"uom": "wmoUnit:percent"
```

Standard meteorological units. May need conversion for display.

### 8. Complex Weather Object

The `weather` property is qualitative, not numeric:

```json
"weather": {
  "values": [
    {
      "validTime": "2026-01-03T18:00:00+00:00/PT6H",
      "value": [
        {
          "coverage": "likely",
          "intensity": "moderate",
          "weather": "rain_showers",
          "visibility": {"unitCode": "wmoUnit:km", "value": null},
          "attributes": []
        }
      ]
    }
  ]
}
```

Note: `value` is an ARRAY of weather conditions (can have multiple simultaneous).

## Metrics Inventory

### Temperature Suite (8 metrics)
- temperature
- dewpoint
- maxTemperature
- minTemperature
- apparentTemperature
- wetBulbGlobeTemperature
- heatIndex
- windChill

### Wind Suite (7 metrics)
- windSpeed
- windDirection
- windGust
- transportWindSpeed
- transportWindDirection
- twentyFootWindSpeed
- twentyFootWindDirection

### Precipitation Suite (4 metrics)
- probabilityOfPrecipitation
- quantitativePrecipitation
- snowfallAmount
- iceAccumulation

### Sky & Visibility (3 metrics)
- skyCover
- visibility
- ceilingHeight

### Humidity (1 metric)
- relativeHumidity

### Fire Weather & Indices (~10 metrics)
- dispersionIndex
- lowVisibilityOccurrenceRiskIndex
- probabilityOfThunder
- mixingHeight
- hainesIndex
- davisStabilityIndex
- atmosphericDispersionIndex
- redFlagThreatIndex
- grasslandFireDangerIndex
- stability

### Marine (Often empty for inland, ~8 metrics)
- waveHeight
- wavePeriod
- waveDirection
- primarySwellHeight
- primarySwellDirection
- secondarySwellHeight
- secondarySwellDirection
- windWaveHeight

### Qualitative
- weather (complex nested structure)
- hazards

## ETL Considerations

### 1. Issue Time Extraction

```yaml
issue_time_path: properties.updateTime
```

### 2. Valid Time Parsing

Need to parse ISO 8601 intervals:
```rust
fn parse_interval(s: &str) -> (DateTime, Duration) {
    let parts: Vec<&str> = s.split('/').collect();
    let start = DateTime::parse_from_rfc3339(parts[0]);
    let duration = parse_iso8601_duration(parts[1]);
    (start, duration)
}
```

### 3. Metric Iteration

Config-driven: list which metrics to extract, map to column names.

```yaml
columns:
  - source_path: properties.temperature.values[*]
    target_column: temperature_c
    unit: celsius
  - source_path: properties.windSpeed.values[*]
    target_column: wind_speed_kmh
    unit: km/h
```

### 4. Weather Conditions

Separate extraction logic for qualitative data:
```yaml
weather_conditions:
  source_path: properties.weather.values[*]
  target_table: silver.weather_conditions
  columns:
    - source: value[*].coverage
      target: coverage
    - source: value[*].weather
      target: weather_type
```

### 5. Empty/Null Handling

- Empty arrays: Skip metric entirely (no rows)
- Null values: Load as NULL, or skip row (configurable)

### 6. Storage Estimate

Per API call:
- ~60 metrics × ~100 values average = ~6,000 data points
- Core table: ~20 columns × ~150 rows = ~3,000 rows (after dedup on time)
- Extended table: ~40 metrics × ~100 values = ~4,000 rows (sparse)

Per day (hourly polling):
- ~24 calls × ~3,000 rows = ~72,000 rows/day for core forecasts
- But most are updates to existing valid_times (handled by UPSERT)
