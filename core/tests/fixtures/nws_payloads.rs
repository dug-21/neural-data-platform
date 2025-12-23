//! NWS API mock payloads for testing
//!
//! These payloads are based on actual NWS API responses and are used
//! to test the ArrayIteratorParser and related parsing logic.

use serde_json::{json, Value};

/// Full NWS observation response with all fields populated
///
/// Based on: https://api.weather.gov/stations/KSGJ/observations/latest
pub fn nws_observation_full() -> Value {
    // Read from fixture file
    let fixture_str = include_str!("nws_observation_sample.json");
    serde_json::from_str(fixture_str).expect("Failed to parse observation fixture")
}

/// NWS observation response with null fields (edge case)
///
/// Tests graceful handling of missing optional data
pub fn nws_observation_with_nulls() -> Value {
    json!({
      "type": "Feature",
      "properties": {
        "timestamp": "2025-12-21T12:00:00+00:00",
        "temperature": {
          "unitCode": "wmoUnit:degC",
          "value": 20.5
        },
        "dewpoint": {
          "unitCode": "wmoUnit:degC",
          "value": null
        },
        "relativeHumidity": {
          "unitCode": "wmoUnit:percent",
          "value": 69.43
        },
        "windSpeed": {
          "unitCode": "wmoUnit:km_h-1",
          "value": null
        },
        "windGust": {
          "unitCode": "wmoUnit:km_h-1",
          "value": null
        },
        "windDirection": {
          "unitCode": "wmoUnit:degree_(angle)",
          "value": 180
        },
        "barometricPressure": {
          "unitCode": "wmoUnit:Pa",
          "value": null
        },
        "visibility": {
          "unitCode": "wmoUnit:m",
          "value": 16090
        }
      }
    })
}

/// NWS hourly forecast with 3 periods (condensed for testing)
///
/// Based on: https://api.weather.gov/gridpoints/JAX/79,49/forecast/hourly
/// This is a shortened version with 3 periods instead of 156
pub fn nws_forecast_three_periods() -> Value {
    // Read from fixture file
    let fixture_str = include_str!("nws_forecast_sample.json");
    serde_json::from_str(fixture_str).expect("Failed to parse forecast fixture")
}

/// NWS forecast with 156 periods (full response simulation)
///
/// Generates a realistic 156-period forecast for performance testing
pub fn nws_forecast_full_156_periods() -> Value {
    let mut periods = Vec::new();

    for i in 0..156 {
        let hour = 13 + i;
        let day_offset = hour / 24;
        let hour_in_day = hour % 24;

        periods.push(json!({
            "number": i + 1,
            "startTime": format!("2025-12-{:02}T{:02}:00:00-05:00", 21 + day_offset, hour_in_day),
            "endTime": format!("2025-12-{:02}T{:02}:00:00-05:00", 21 + day_offset, (hour_in_day + 1) % 24),
            "isDaytime": hour_in_day >= 6 && hour_in_day < 18,
            "temperature": 70 + (i % 10) as i64,
            "temperatureUnit": "F",
            "probabilityOfPrecipitation": {
                "unitCode": "wmoUnit:percent",
                "value": (i % 100) as i64
            },
            "dewpoint": {
                "unitCode": "wmoUnit:degC",
                "value": 15.0 + (i as f64 * 0.1)
            },
            "relativeHumidity": {
                "unitCode": "wmoUnit:percent",
                "value": 60 + (i % 20) as i64
            },
            "windSpeed": if i % 2 == 0 { "10 mph" } else { "5 to 10 mph" },
            "windDirection": match i % 8 {
                0 => "N",
                1 => "NE",
                2 => "E",
                3 => "SE",
                4 => "S",
                5 => "SW",
                6 => "W",
                _ => "NW",
            },
            "shortForecast": "Partly Cloudy"
        }));
    }

    json!({
        "type": "Feature",
        "properties": {
            "generatedAt": "2025-12-21T12:00:00+00:00",
            "periods": periods
        }
    })
}

/// NWS forecast with empty periods array (error case)
pub fn nws_forecast_empty_periods() -> Value {
    json!({
        "type": "Feature",
        "properties": {
            "generatedAt": "2025-12-21T12:00:00+00:00",
            "periods": []
        }
    })
}

/// NWS forecast with invalid timestamp (error case)
pub fn nws_forecast_invalid_timestamp() -> Value {
    json!({
        "type": "Feature",
        "properties": {
            "generatedAt": "invalid-date-format",
            "periods": [
                {
                    "startTime": "2025-12-21T13:00:00-05:00",
                    "temperature": 72
                }
            ]
        }
    })
}

/// NWS forecast with missing generatedAt (error case)
pub fn nws_forecast_missing_issue_time() -> Value {
    json!({
        "type": "Feature",
        "properties": {
            "periods": [
                {
                    "startTime": "2025-12-21T13:00:00-05:00",
                    "temperature": 72
                }
            ]
        }
    })
}

/// NWS forecast with period missing startTime (error case)
pub fn nws_forecast_missing_valid_time() -> Value {
    json!({
        "type": "Feature",
        "properties": {
            "generatedAt": "2025-12-21T12:00:00+00:00",
            "periods": [
                {
                    "temperature": 72,
                    "windSpeed": "10 mph"
                }
            ]
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nws_observation_full_has_expected_structure() {
        let payload = nws_observation_full();
        assert!(payload.get("properties").is_some());
        assert!(payload.get("type").is_some());

        let properties = payload.get("properties").unwrap();
        assert!(properties.get("timestamp").is_some());
        assert!(properties.get("temperature").is_some());
    }

    #[test]
    fn test_nws_observation_with_nulls_has_nulls() {
        let payload = nws_observation_with_nulls();
        let properties = payload.get("properties").unwrap();

        // Temperature should be present
        assert!(properties
            .get("temperature")
            .unwrap()
            .get("value")
            .unwrap()
            .is_f64());

        // Dewpoint should be null
        assert!(properties
            .get("dewpoint")
            .unwrap()
            .get("value")
            .unwrap()
            .is_null());
    }

    #[test]
    fn test_nws_forecast_three_periods_has_three_periods() {
        let payload = nws_forecast_three_periods();
        let properties = payload.get("properties").unwrap();
        let periods = properties.get("periods").unwrap().as_array().unwrap();

        assert_eq!(periods.len(), 3);
    }

    #[test]
    fn test_nws_forecast_full_156_periods_has_156_periods() {
        let payload = nws_forecast_full_156_periods();
        let properties = payload.get("properties").unwrap();
        let periods = properties.get("periods").unwrap().as_array().unwrap();

        assert_eq!(periods.len(), 156);
    }

    #[test]
    fn test_nws_forecast_empty_periods_has_zero_periods() {
        let payload = nws_forecast_empty_periods();
        let properties = payload.get("properties").unwrap();
        let periods = properties.get("periods").unwrap().as_array().unwrap();

        assert_eq!(periods.len(), 0);
    }
}
