# NWS API Parser Design

**Feature:** air-006
**Component:** core/src/sources/parsers/nws.rs
**Author:** ndp-rust-dev
**Date:** 2025-12-21

## Overview

Design for ResponseParser trait implementations for National Weather Service (NWS) API responses. Two parsers are required:

1. **NwsObservationParser** - Current observations from weather stations
2. **NwsForecastParser** - Hourly gridpoint forecasts

Both parsers follow the established ResponseParser pattern used by OpenWeatherMap parsers.

---

## Pattern Reference

Based on existing parsers:
- `/workspaces/neural-data-platform/core/src/sources/parsers/weather.rs` (OpenWeatherMap current weather)
- `/workspaces/neural-data-platform/core/src/sources/parsers/air_pollution.rs` (OpenWeatherMap air pollution)

### Key Pattern Elements

1. **Zero-sized struct** with `new()` and `Default` trait
2. **ResponseParser trait** implementation with `parse()` and `name()` methods
3. **Helper function** `create_point()` for consistent TimeSeriesPoint creation
4. **Serde structs** matching API response structure
5. **Comprehensive tests** including happy path, edge cases, error handling

---

## 1. NWS Observation Parser

### API Endpoint
```
https://api.weather.gov/stations/{stationId}/observations/latest
```

### Example Response
```json
{
  "properties": {
    "timestamp": "2025-12-21T18:45:00+00:00",
    "temperature": {
      "value": 19.0,
      "unitCode": "wmoUnit:degC",
      "qualityControl": "qc:V"
    },
    "dewpoint": {
      "value": 9.0,
      "unitCode": "wmoUnit:degC"
    },
    "relativeHumidity": {
      "value": 52.28,
      "unitCode": "wmoUnit:percent"
    },
    "windSpeed": {
      "value": 20.376,
      "unitCode": "wmoUnit:km_h-1"
    },
    "windDirection": {
      "value": 360,
      "unitCode": "wmoUnit:degree_(angle)"
    },
    "barometricPressure": {
      "value": 102370.52,
      "unitCode": "wmoUnit:Pa"
    },
    "visibility": {
      "value": 16093,
      "unitCode": "wmoUnit:m"
    },
    "textDescription": "Partly Cloudy"
  }
}
```

### Rust Structs

```rust
use crate::error::{CoreError, CoreResult};
use crate::traits::TimeSeriesPoint;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::HashMap;

use super::super::http_poll::ResponseParser;

/// NWS observation response structure
#[derive(Debug, Clone, Deserialize)]
struct NwsObservationResponse {
    properties: NwsObservationProperties,
}

#[derive(Debug, Clone, Deserialize)]
struct NwsObservationProperties {
    timestamp: String,

    #[serde(default)]
    temperature: Option<NwsMeasurement>,

    #[serde(default)]
    dewpoint: Option<NwsMeasurement>,

    #[serde(default)]
    #[serde(rename = "relativeHumidity")]
    relative_humidity: Option<NwsMeasurement>,

    #[serde(default)]
    #[serde(rename = "windSpeed")]
    wind_speed: Option<NwsMeasurement>,

    #[serde(default)]
    #[serde(rename = "windDirection")]
    wind_direction: Option<NwsMeasurement>,

    #[serde(default)]
    #[serde(rename = "barometricPressure")]
    barometric_pressure: Option<NwsMeasurement>,

    #[serde(default)]
    visibility: Option<NwsMeasurement>,

    #[serde(default)]
    #[serde(rename = "textDescription")]
    text_description: Option<String>,
}

/// NWS measurement with value, unit, and optional quality control
#[derive(Debug, Clone, Deserialize)]
struct NwsMeasurement {
    value: Option<f64>,

    #[serde(rename = "unitCode")]
    unit_code: Option<String>,

    #[serde(default)]
    #[serde(rename = "qualityControl")]
    quality_control: Option<String>,
}

impl NwsMeasurement {
    /// Extract numeric value, returning None if null or missing
    fn get_value(&self) -> Option<f64> {
        self.value
    }

    /// Extract simplified unit (remove "wmoUnit:" prefix)
    fn get_unit(&self) -> String {
        self.unit_code
            .as_ref()
            .map(|u| u.strip_prefix("wmoUnit:").unwrap_or(u).to_string())
            .unwrap_or_else(|| "unknown".to_string())
    }
}
```

### Parser Implementation

```rust
/// Parser for NWS observation API responses
pub struct NwsObservationParser;

impl NwsObservationParser {
    /// Create a new NWS observation parser
    pub fn new() -> Self {
        Self
    }

    /// Create a time series point with the given metric and value
    fn create_point(
        location_id: &str,
        metric: &str,
        value: f64,
        unit: &str,
        timestamp: DateTime<Utc>,
        qc_flag: Option<&str>,
    ) -> TimeSeriesPoint {
        let mut tags = HashMap::new();
        tags.insert("metric".to_string(), metric.to_string());
        tags.insert("source".to_string(), "nws".to_string());
        tags.insert("api".to_string(), "observation".to_string());
        tags.insert("unit".to_string(), unit.to_string());

        if let Some(qc) = qc_flag {
            tags.insert("quality_control".to_string(), qc.to_string());
        }

        TimeSeriesPoint {
            timestamp,
            location_id: location_id.to_string(),
            value,
            tags,
        }
    }

    /// Parse ISO 8601 timestamp from NWS API
    fn parse_timestamp(timestamp_str: &str) -> CoreResult<DateTime<Utc>> {
        DateTime::parse_from_rfc3339(timestamp_str)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| CoreError::Source(format!("Failed to parse timestamp '{}': {}", timestamp_str, e)))
    }
}

impl Default for NwsObservationParser {
    fn default() -> Self {
        Self::new()
    }
}

impl ResponseParser for NwsObservationParser {
    fn parse(
        &self,
        response_body: &str,
        location_id: &str,
        _timestamp: DateTime<Utc>, // NWS provides its own timestamp
    ) -> CoreResult<Vec<TimeSeriesPoint>> {
        let obs: NwsObservationResponse = serde_json::from_str(response_body)
            .map_err(|e| CoreError::Source(format!("Failed to parse NWS observation response: {}", e)))?;

        // Parse NWS-provided timestamp
        let timestamp = Self::parse_timestamp(&obs.properties.timestamp)?;

        let mut points = Vec::new();

        // Temperature
        if let Some(temp) = &obs.properties.temperature {
            if let Some(value) = temp.get_value() {
                points.push(Self::create_point(
                    location_id,
                    "temperature",
                    value,
                    &temp.get_unit(),
                    timestamp,
                    temp.quality_control.as_deref(),
                ));
            }
        }

        // Dewpoint
        if let Some(dewpoint) = &obs.properties.dewpoint {
            if let Some(value) = dewpoint.get_value() {
                points.push(Self::create_point(
                    location_id,
                    "dewpoint",
                    value,
                    &dewpoint.get_unit(),
                    timestamp,
                    dewpoint.quality_control.as_deref(),
                ));
            }
        }

        // Relative Humidity
        if let Some(humidity) = &obs.properties.relative_humidity {
            if let Some(value) = humidity.get_value() {
                points.push(Self::create_point(
                    location_id,
                    "relative_humidity",
                    value,
                    &humidity.get_unit(),
                    timestamp,
                    humidity.quality_control.as_deref(),
                ));
            }
        }

        // Wind Speed
        if let Some(wind_speed) = &obs.properties.wind_speed {
            if let Some(value) = wind_speed.get_value() {
                points.push(Self::create_point(
                    location_id,
                    "wind_speed",
                    value,
                    &wind_speed.get_unit(),
                    timestamp,
                    wind_speed.quality_control.as_deref(),
                ));
            }
        }

        // Wind Direction
        if let Some(wind_dir) = &obs.properties.wind_direction {
            if let Some(value) = wind_dir.get_value() {
                points.push(Self::create_point(
                    location_id,
                    "wind_direction",
                    value,
                    &wind_dir.get_unit(),
                    timestamp,
                    wind_dir.quality_control.as_deref(),
                ));
            }
        }

        // Barometric Pressure
        if let Some(pressure) = &obs.properties.barometric_pressure {
            if let Some(value) = pressure.get_value() {
                points.push(Self::create_point(
                    location_id,
                    "barometric_pressure",
                    value,
                    &pressure.get_unit(),
                    timestamp,
                    pressure.quality_control.as_deref(),
                ));
            }
        }

        // Visibility
        if let Some(visibility) = &obs.properties.visibility {
            if let Some(value) = visibility.get_value() {
                points.push(Self::create_point(
                    location_id,
                    "visibility",
                    value,
                    &visibility.get_unit(),
                    timestamp,
                    visibility.quality_control.as_deref(),
                ));
            }
        }

        Ok(points)
    }

    fn name(&self) -> &'static str {
        "nws_observation"
    }
}
```

### Key Features

1. **Null handling**: All measurements are `Option<NwsMeasurement>`, values within are `Option<f64>`
2. **QC flags**: Quality control tags added when present
3. **Unit parsing**: Strips `wmoUnit:` prefix from unit codes
4. **NWS timestamp**: Uses timestamp from API response, not poll time
5. **Graceful degradation**: Missing fields don't fail the entire parse

---

## 2. NWS Forecast Parser

### API Endpoint
```
https://api.weather.gov/gridpoints/{office}/{gridX},{gridY}/forecast/hourly
```

### Example Response (truncated)
```json
{
  "properties": {
    "updated": "2025-12-21T18:30:00+00:00",
    "generatedAt": "2025-12-21T18:45:00+00:00",
    "periods": [
      {
        "number": 1,
        "name": "This Hour",
        "startTime": "2025-12-21T19:00:00-05:00",
        "endTime": "2025-12-21T20:00:00-05:00",
        "isDaytime": false,
        "temperature": 66,
        "temperatureUnit": "F",
        "windSpeed": "10 mph",
        "windDirection": "N",
        "shortForecast": "Partly Cloudy",
        "probabilityOfPrecipitation": {
          "value": 20
        },
        "dewpoint": {
          "value": 10.0,
          "unitCode": "wmoUnit:degC"
        },
        "relativeHumidity": {
          "value": 52
        }
      },
      // ... up to 156 periods
    ]
  }
}
```

### Rust Structs

```rust
/// NWS forecast response structure
#[derive(Debug, Clone, Deserialize)]
struct NwsForecastResponse {
    properties: NwsForecastProperties,
}

#[derive(Debug, Clone, Deserialize)]
struct NwsForecastProperties {
    #[serde(rename = "generatedAt")]
    generated_at: String,

    periods: Vec<NwsForecastPeriod>,
}

#[derive(Debug, Clone, Deserialize)]
struct NwsForecastPeriod {
    number: u32,

    #[serde(rename = "startTime")]
    start_time: String,

    #[serde(rename = "endTime")]
    end_time: String,

    #[serde(rename = "isDaytime")]
    is_daytime: bool,

    temperature: i32,

    #[serde(rename = "temperatureUnit")]
    temperature_unit: String, // "F" or "C"

    #[serde(rename = "windSpeed")]
    wind_speed: String, // "10 mph" or "5 to 10 mph"

    #[serde(rename = "windDirection")]
    wind_direction: String, // "N", "NW", etc.

    #[serde(rename = "shortForecast")]
    short_forecast: String,

    #[serde(default)]
    #[serde(rename = "probabilityOfPrecipitation")]
    probability_of_precipitation: Option<NwsProbability>,

    #[serde(default)]
    dewpoint: Option<NwsMeasurement>,

    #[serde(default)]
    #[serde(rename = "relativeHumidity")]
    relative_humidity: Option<NwsProbability>,
}

#[derive(Debug, Clone, Deserialize)]
struct NwsProbability {
    value: Option<i32>, // Percentage or None
}

impl NwsForecastPeriod {
    /// Parse wind speed string to numeric mph value
    /// "10 mph" -> 10.0
    /// "5 to 10 mph" -> 7.5 (average)
    /// "Calm" -> 0.0
    fn parse_wind_speed(&self) -> Option<f64> {
        let speed_str = self.wind_speed.trim();

        if speed_str.eq_ignore_ascii_case("calm") {
            return Some(0.0);
        }

        // Handle "5 to 10 mph" format
        if speed_str.contains(" to ") {
            let parts: Vec<&str> = speed_str.split(" to ").collect();
            if parts.len() == 2 {
                let low = parts[0].trim().parse::<f64>().ok()?;
                let high = parts[1].split_whitespace().next()?.parse::<f64>().ok()?;
                return Some((low + high) / 2.0);
            }
        }

        // Handle "10 mph" format
        speed_str
            .split_whitespace()
            .next()?
            .parse::<f64>()
            .ok()
    }

    /// Convert wind direction string to degrees
    /// "N" -> 0, "NE" -> 45, "E" -> 90, etc.
    fn parse_wind_direction(&self) -> Option<f64> {
        match self.wind_direction.trim() {
            "N" => Some(0.0),
            "NNE" => Some(22.5),
            "NE" => Some(45.0),
            "ENE" => Some(67.5),
            "E" => Some(90.0),
            "ESE" => Some(112.5),
            "SE" => Some(135.0),
            "SSE" => Some(157.5),
            "S" => Some(180.0),
            "SSW" => Some(202.5),
            "SW" => Some(225.0),
            "WSW" => Some(247.5),
            "W" => Some(270.0),
            "WNW" => Some(292.5),
            "NW" => Some(315.0),
            "NNW" => Some(337.5),
            _ => None,
        }
    }
}
```

### Parser Implementation

```rust
/// Parser for NWS forecast API responses
pub struct NwsForecastParser;

impl NwsForecastParser {
    /// Create a new NWS forecast parser
    pub fn new() -> Self {
        Self
    }

    /// Create a time series point with forecast-specific tags
    fn create_point(
        location_id: &str,
        metric: &str,
        value: f64,
        unit: &str,
        forecast_time: DateTime<Utc>,
        issue_time: DateTime<Utc>,
    ) -> TimeSeriesPoint {
        let mut tags = HashMap::new();
        tags.insert("metric".to_string(), metric.to_string());
        tags.insert("source".to_string(), "nws".to_string());
        tags.insert("api".to_string(), "forecast".to_string());
        tags.insert("unit".to_string(), unit.to_string());
        tags.insert("issue_time".to_string(), issue_time.to_rfc3339());

        TimeSeriesPoint {
            timestamp: forecast_time,
            location_id: location_id.to_string(),
            value,
            tags,
        }
    }

    /// Parse ISO 8601 timestamp from NWS API
    fn parse_timestamp(timestamp_str: &str) -> CoreResult<DateTime<Utc>> {
        DateTime::parse_from_rfc3339(timestamp_str)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| CoreError::Source(format!("Failed to parse timestamp '{}': {}", timestamp_str, e)))
    }
}

impl Default for NwsForecastParser {
    fn default() -> Self {
        Self::new()
    }
}

impl ResponseParser for NwsForecastParser {
    fn parse(
        &self,
        response_body: &str,
        location_id: &str,
        _timestamp: DateTime<Utc>,
    ) -> CoreResult<Vec<TimeSeriesPoint>> {
        let forecast: NwsForecastResponse = serde_json::from_str(response_body)
            .map_err(|e| CoreError::Source(format!("Failed to parse NWS forecast response: {}", e)))?;

        // Parse issue time from generatedAt
        let issue_time = Self::parse_timestamp(&forecast.properties.generated_at)?;

        let mut points = Vec::new();

        for period in &forecast.properties.periods {
            // Parse forecast valid time (startTime)
            let forecast_time = Self::parse_timestamp(&period.start_time)?;

            // Temperature (convert to Celsius if needed)
            let temp_celsius = if period.temperature_unit == "F" {
                (period.temperature as f64 - 32.0) * 5.0 / 9.0
            } else {
                period.temperature as f64
            };

            points.push(Self::create_point(
                location_id,
                "temperature_forecast",
                temp_celsius,
                "degC",
                forecast_time,
                issue_time,
            ));

            // Wind Speed
            if let Some(wind_speed) = period.parse_wind_speed() {
                points.push(Self::create_point(
                    location_id,
                    "wind_speed_forecast",
                    wind_speed,
                    "mph",
                    forecast_time,
                    issue_time,
                ));
            }

            // Wind Direction
            if let Some(wind_dir) = period.parse_wind_direction() {
                points.push(Self::create_point(
                    location_id,
                    "wind_direction_forecast",
                    wind_dir,
                    "degrees",
                    forecast_time,
                    issue_time,
                ));
            }

            // Probability of Precipitation
            if let Some(pop) = &period.probability_of_precipitation {
                if let Some(value) = pop.value {
                    points.push(Self::create_point(
                        location_id,
                        "precipitation_probability_forecast",
                        value as f64,
                        "percent",
                        forecast_time,
                        issue_time,
                    ));
                }
            }

            // Dewpoint
            if let Some(dewpoint) = &period.dewpoint {
                if let Some(value) = dewpoint.get_value() {
                    points.push(Self::create_point(
                        location_id,
                        "dewpoint_forecast",
                        value,
                        &dewpoint.get_unit(),
                        forecast_time,
                        issue_time,
                    ));
                }
            }

            // Relative Humidity
            if let Some(humidity) = &period.relative_humidity {
                if let Some(value) = humidity.value {
                    points.push(Self::create_point(
                        location_id,
                        "relative_humidity_forecast",
                        value as f64,
                        "percent",
                        forecast_time,
                        issue_time,
                    ));
                }
            }
        }

        Ok(points)
    }

    fn name(&self) -> &'static str {
        "nws_forecast"
    }
}
```

### Key Features

1. **Issue time tracking**: `issue_time` tag for forecast model run identification
2. **Wind parsing**: Handles "10 mph", "5 to 10 mph", "Calm" formats
3. **Direction conversion**: Cardinal directions → degrees
4. **Temperature conversion**: Fahrenheit → Celsius if needed
5. **Multiple periods**: Processes all ~156 hourly forecast periods
6. **Forecast-specific metrics**: `_forecast` suffix distinguishes from observations

---

## Testing Requirements

### NwsObservationParser Tests

```rust
#[cfg(test)]
mod observation_tests {
    use super::*;

    #[test]
    fn test_observation_parser_creation() {
        let parser = NwsObservationParser::new();
        assert_eq!(std::mem::size_of_val(&parser), 0); // Zero-sized type
    }

    #[test]
    fn test_parse_full_observation_response() {
        let parser = NwsObservationParser::new();
        let json = r#"{
            "properties": {
                "timestamp": "2025-12-21T18:45:00+00:00",
                "temperature": {"value": 19.0, "unitCode": "wmoUnit:degC"},
                "dewpoint": {"value": 9.0, "unitCode": "wmoUnit:degC"},
                "relativeHumidity": {"value": 52.28, "unitCode": "wmoUnit:percent"},
                "windSpeed": {"value": 20.376, "unitCode": "wmoUnit:km_h-1"},
                "windDirection": {"value": 360, "unitCode": "wmoUnit:degree_(angle)"},
                "barometricPressure": {"value": 102370.52, "unitCode": "wmoUnit:Pa"}
            }
        }"#;

        let timestamp = Utc::now();
        let points = parser.parse(json, "KSGJ", timestamp).unwrap();

        assert!(points.len() >= 6);

        // Verify temperature
        let temp = points.iter()
            .find(|p| p.tags.get("metric") == Some(&"temperature".to_string()))
            .unwrap();
        assert_eq!(temp.value, 19.0);
        assert_eq!(temp.tags.get("unit"), Some(&"degC".to_string()));
    }

    #[test]
    fn test_parse_with_null_values() {
        let parser = NwsObservationParser::new();
        let json = r#"{
            "properties": {
                "timestamp": "2025-12-21T18:45:00+00:00",
                "temperature": {"value": 19.0, "unitCode": "wmoUnit:degC"},
                "dewpoint": {"value": null, "unitCode": "wmoUnit:degC"},
                "windSpeed": {"value": null, "unitCode": "wmoUnit:km_h-1"}
            }
        }"#;

        let timestamp = Utc::now();
        let points = parser.parse(json, "KSGJ", timestamp).unwrap();

        // Should only have temperature (null values skipped)
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].tags.get("metric"), Some(&"temperature".to_string()));
    }

    #[test]
    fn test_parse_with_quality_control_flags() {
        let parser = NwsObservationParser::new();
        let json = r#"{
            "properties": {
                "timestamp": "2025-12-21T18:45:00+00:00",
                "temperature": {
                    "value": 19.0,
                    "unitCode": "wmoUnit:degC",
                    "qualityControl": "qc:V"
                }
            }
        }"#;

        let timestamp = Utc::now();
        let points = parser.parse(json, "KSGJ", timestamp).unwrap();

        assert_eq!(points.len(), 1);
        assert_eq!(points[0].tags.get("quality_control"), Some(&"qc:V".to_string()));
    }

    #[test]
    fn test_parse_invalid_json() {
        let parser = NwsObservationParser::new();
        let json = r#"{ "invalid": "json" }"#;

        let timestamp = Utc::now();
        let result = parser.parse(json, "KSGJ", timestamp);
        assert!(result.is_err());
    }
}
```

### NwsForecastParser Tests

```rust
#[cfg(test)]
mod forecast_tests {
    use super::*;

    #[test]
    fn test_forecast_parser_creation() {
        let parser = NwsForecastParser::new();
        assert_eq!(std::mem::size_of_val(&parser), 0);
    }

    #[test]
    fn test_parse_wind_speed_formats() {
        let period = NwsForecastPeriod {
            wind_speed: "10 mph".to_string(),
            // ... other fields
        };
        assert_eq!(period.parse_wind_speed(), Some(10.0));

        let period2 = NwsForecastPeriod {
            wind_speed: "5 to 10 mph".to_string(),
            // ...
        };
        assert_eq!(period2.parse_wind_speed(), Some(7.5));

        let period3 = NwsForecastPeriod {
            wind_speed: "Calm".to_string(),
            // ...
        };
        assert_eq!(period3.parse_wind_speed(), Some(0.0));
    }

    #[test]
    fn test_parse_wind_direction() {
        let period = NwsForecastPeriod {
            wind_direction: "N".to_string(),
            // ...
        };
        assert_eq!(period.parse_wind_direction(), Some(0.0));

        let period2 = NwsForecastPeriod {
            wind_direction: "SE".to_string(),
            // ...
        };
        assert_eq!(period2.parse_wind_direction(), Some(135.0));
    }

    #[test]
    fn test_parse_full_forecast_response() {
        let parser = NwsForecastParser::new();
        let json = r#"{
            "properties": {
                "generatedAt": "2025-12-21T18:45:00+00:00",
                "periods": [
                    {
                        "number": 1,
                        "startTime": "2025-12-21T19:00:00-05:00",
                        "endTime": "2025-12-21T20:00:00-05:00",
                        "isDaytime": false,
                        "temperature": 66,
                        "temperatureUnit": "F",
                        "windSpeed": "10 mph",
                        "windDirection": "N",
                        "shortForecast": "Partly Cloudy",
                        "probabilityOfPrecipitation": {"value": 20}
                    }
                ]
            }
        }"#;

        let timestamp = Utc::now();
        let points = parser.parse(json, "JAX/79,49", timestamp).unwrap();

        assert!(points.len() >= 3); // temp, wind_speed, wind_dir, pop

        // Verify issue_time tag present
        let temp = points.iter()
            .find(|p| p.tags.get("metric") == Some(&"temperature_forecast".to_string()))
            .unwrap();
        assert!(temp.tags.contains_key("issue_time"));
    }

    #[test]
    fn test_temperature_conversion() {
        // 66°F = 18.89°C
        let expected_celsius = (66.0 - 32.0) * 5.0 / 9.0;
        assert!((expected_celsius - 18.89).abs() < 0.1);
    }
}
```

---

## File Organization

```
core/src/sources/parsers/
├── mod.rs              # Add pub mod nws;
├── weather.rs          # Existing OpenWeatherMap weather
├── air_pollution.rs    # Existing OpenWeatherMap air pollution
└── nws.rs              # NEW: NWS observation + forecast parsers
```

---

## Integration Points

### In HttpPollingSource

```rust
use crate::sources::parsers::nws::{NwsObservationParser, NwsForecastParser};

// For observations
let parser = Box::new(NwsObservationParser::new());

// For forecasts
let parser = Box::new(NwsForecastParser::new());
```

### Stream Configuration

```yaml
# config/base/streams/weather-observations-nws.yaml
stream_id: weather-observations-nws
enabled: true
source_type: http_polling
parser: nws_observation
location_id: KSGJ
poll_interval_seconds: 300
endpoint: https://api.weather.gov/stations/KSGJ/observations/latest

# config/base/streams/weather-forecast-nws.yaml
stream_id: weather-forecast-nws
enabled: true
source_type: http_polling
parser: nws_forecast
location_id: JAX-79-49
poll_interval_seconds: 3600
endpoint: https://api.weather.gov/gridpoints/JAX/79,49/forecast/hourly
```

---

## Error Handling

Following existing parser patterns:

1. **JSON parsing errors** → `CoreError::Source` with descriptive message
2. **Timestamp parsing errors** → `CoreError::Source` with timestamp string
3. **Missing data** → Gracefully skip, don't fail entire parse
4. **Null values** → Skip the metric, continue with others

---

## Quality Control

NWS provides QC flags in observations:
- `qc:V` - Valid
- `qc:Z` - Estimated
- `qc:S` - Suspect

These are captured in the `quality_control` tag for downstream filtering.

---

## Metric Naming

| Observation | Metric Name | Unit |
|-------------|-------------|------|
| Temperature | `temperature` | degC |
| Dewpoint | `dewpoint` | degC |
| Humidity | `relative_humidity` | percent |
| Wind Speed | `wind_speed` | km_h-1 |
| Wind Direction | `wind_direction` | degree_(angle) |
| Pressure | `barometric_pressure` | Pa |
| Visibility | `visibility` | m |

| Forecast | Metric Name | Unit |
|----------|-------------|------|
| Temperature | `temperature_forecast` | degC |
| Wind Speed | `wind_speed_forecast` | mph |
| Wind Direction | `wind_direction_forecast` | degrees |
| Precipitation | `precipitation_probability_forecast` | percent |
| Dewpoint | `dewpoint_forecast` | degC |
| Humidity | `relative_humidity_forecast` | percent |

---

## Next Steps

1. Implement `/workspaces/neural-data-platform/core/src/sources/parsers/nws.rs`
2. Add `pub mod nws;` to `/workspaces/neural-data-platform/core/src/sources/parsers/mod.rs`
3. Run tests: `cargo test parsers::nws`
4. Update HttpPollingSource to support `parser: nws_observation` and `parser: nws_forecast`
5. Create stream configurations for KSGJ station and JAX gridpoint

---

## References

- **NWS API Docs:** https://www.weather.gov/documentation/services-web-api
- **Existing Parsers:**
  - `/workspaces/neural-data-platform/core/src/sources/parsers/weather.rs`
  - `/workspaces/neural-data-platform/core/src/sources/parsers/air_pollution.rs`
- **ResponseParser Trait:** `/workspaces/neural-data-platform/core/src/sources/http_poll.rs`
