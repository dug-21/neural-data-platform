//! Sample payloads for testing config-driven parsers
//!
//! These payloads demonstrate:
//! 1. Current firmware/API fields that exist today
//! 2. Future fields that might be added in newer versions
//! 3. Edge cases for parser behavior

use serde_json::json;
use serde_json::Value;

/// AirGradient payload with ALL current fields (firmware v3.x)
///
/// This represents the CURRENT AirGradient ONE firmware payload
pub fn airgradient_current() -> Value {
    json!({
        "serialno": "d83bda1cd074",
        "wifi": -67,
        "boot": 0,
        "firmware": "3.4.1",
        "model": "I-9PSL",
        "ledMode": "co2",
        "bootCount": 123,
        "pm01": 1.0,
        "pm02": 2.17,
        "pm10": 2.33,
        "rco2": 396.0,
        "atmp": 22.1,
        "rhum": 65.13,
        "tvocIndex": 42.0,
        "noxIndex": 2.0,
        "tvocRaw": 25420.0,
        "noxRaw": 16325.0
    })
}

/// AirGradient payload with FUTURE fields (simulated firmware v4.x)
///
/// This payload simulates what AirGradient might add in future firmware updates.
/// A truly config-driven parser should handle these WITHOUT code changes.
pub fn airgradient_future() -> Value {
    json!({
        "serialno": "d83bda1cd074",
        // Existing fields
        "pm01": 1.0,
        "pm02": 2.17,
        "pm10": 2.33,
        "rco2": 396.0,
        "atmp": 22.1,
        "rhum": 65.13,
        "tvocIndex": 42.0,
        "noxIndex": 2.0,
        // NEW fields added in v4.0
        "pm01Compensated": 0.8,
        "pm02Compensated": 1.9,
        "pm10Compensated": 2.1,
        "co2Compensated": 380.0,
        "vocRawValue": 2500.0,
        "noxRawValue": 150.0,
        "ambientLight": 128.0,
        "audioDbPeak": 45.5,
        "soilMoisture": 42.3,  // Hypothetical new sensor
        "uvIndex": 3.5         // Hypothetical new sensor
    })
}

/// OpenWeatherMap current weather response with FULL field set
///
/// This includes all optional fields that might be present
pub fn openweathermap_weather_full() -> Value {
    json!({
        "coord": {
            "lon": -122.4194,
            "lat": 37.7749
        },
        "weather": [{
            "id": 800,
            "main": "Clear",
            "description": "clear sky",
            "icon": "01d"
        }],
        "base": "stations",
        "main": {
            "temp": 22.5,
            "feels_like": 21.8,
            "temp_min": 20.0,
            "temp_max": 25.0,
            "pressure": 1013.0,
            "humidity": 65.0,
            "sea_level": 1013.0,
            "grnd_level": 1010.0
        },
        "visibility": 10000.0,
        "wind": {
            "speed": 3.5,
            "deg": 180.0,
            "gust": 5.2
        },
        "clouds": {
            "all": 20.0
        },
        "rain": {
            "1h": 0.5,
            "3h": 1.2
        },
        "snow": {
            "1h": 0.0,
            "3h": 0.0
        },
        "dt": 1702900000,
        "sys": {
            "type": 2,
            "id": 2001996,
            "country": "US",
            "sunrise": 1702882800,
            "sunset": 1702916400
        },
        "timezone": -28800,
        "id": 5391959,
        "name": "San Francisco",
        "cod": 200
    })
}

/// OpenWeatherMap weather with MINIMAL fields (what's guaranteed)
///
/// This tests that parsers handle missing optional fields
pub fn openweathermap_weather_minimal() -> Value {
    json!({
        "main": {
            "temp": 22.5,
            "feels_like": 21.8,
            "pressure": 1013.0,
            "humidity": 65.0
        },
        "wind": {
            "speed": 3.5,
            "deg": 180.0
        },
        "clouds": {
            "all": 20.0
        }
    })
}

/// OpenWeatherMap weather with EXTENDED fields (hypothetical API v3.0)
///
/// Simulates future API additions. Config-driven parser should handle these.
pub fn openweathermap_weather_future() -> Value {
    json!({
        "main": {
            "temp": 22.5,
            "feels_like": 21.8,
            "pressure": 1013.0,
            "humidity": 65.0,
            // Future additions
            "dew_point": 15.3,
            "heat_index": 23.1,
            "wind_chill": 21.5
        },
        "wind": {
            "speed": 3.5,
            "deg": 180.0,
            "gust": 5.2,
            // Future additions
            "speed_max_24h": 8.2,
            "gustiness_index": 1.5
        },
        "clouds": {
            "all": 20.0,
            // Future additions
            "low": 10.0,
            "mid": 5.0,
            "high": 5.0
        },
        "radiation": {
            "uv_index": 5.0,
            "solar": 850.0
        }
    })
}

/// OpenWeatherMap air pollution response
///
/// Standard air pollution API response
pub fn openweathermap_air_pollution() -> Value {
    json!({
        "coord": {
            "lon": -122.4194,
            "lat": 37.7749
        },
        "list": [{
            "main": {
                "aqi": 2
            },
            "components": {
                "co": 230.31,
                "no": 0.51,
                "no2": 15.34,
                "o3": 68.66,
                "so2": 3.73,
                "pm2_5": 8.59,
                "pm10": 12.15,
                "nh3": 0.92
            },
            "dt": 1702900000
        }]
    })
}

/// OpenWeatherMap air pollution with FUTURE fields (hypothetical API v3.0)
///
/// Simulates future pollutants that might be added
pub fn openweathermap_air_pollution_future() -> Value {
    json!({
        "list": [{
            "main": {
                "aqi": 2,
                // Future: more granular AQI
                "aqi_pm2_5": 1,
                "aqi_pm10": 2,
                "aqi_o3": 2
            },
            "components": {
                "co": 230.31,
                "no": 0.51,
                "no2": 15.34,
                "o3": 68.66,
                "so2": 3.73,
                "pm2_5": 8.59,
                "pm10": 12.15,
                "nh3": 0.92,
                // Future pollutants
                "pm1": 5.2,
                "pm4": 10.3,
                "black_carbon": 1.2,
                "voc": 25.5,
                "ch4": 1850.0,
                "benzene": 0.5
            },
            "dt": 1702900000
        }]
    })
}

/// Generic flat JSON with unknown fields
///
/// This tests that a flat parser can handle ANY numeric fields
pub fn generic_unknown_fields() -> Value {
    json!({
        "device_id": "sensor-xyz",
        "timestamp": "2024-01-01T12:00:00Z",
        "known_field_1": 42.0,
        "known_field_2": 99.9,
        "brand_new_field": 123.45,
        "another_unknown": 67.89,
        "future_sensor_reading": 11.11,
        "experimental_metric": 88.88,
        // Non-numeric (should be skipped by flat parser)
        "string_field": "value",
        "bool_field": true,
        "null_field": null,
        "object_field": {"nested": "value"}
    })
}

/// Nested JSON requiring JSONPath extraction
///
/// This tests path-based field extraction
pub fn nested_structure() -> Value {
    json!({
        "device": {
            "id": "sensor-001",
            "location": "outdoors"
        },
        "readings": {
            "environmental": {
                "temperature": 22.5,
                "humidity": 65.0,
                "pressure": 1013.0
            },
            "air_quality": {
                "pm25": 12.5,
                "pm10": 18.3,
                "co2": 425.0
            }
        },
        "metadata": {
            "firmware": "1.2.3",
            "uptime": 3600
        }
    })
}

/// Edge case: all numeric types
///
/// Tests that parsers handle integers, floats, scientific notation
pub fn numeric_types() -> Value {
    json!({
        "id": "test",
        "integer_field": 42,
        "float_field": 42.5,
        "scientific_notation": 1.23e-4,
        "large_number": 9999999999i64,
        "negative": -15.5,
        "zero": 0,
        "zero_float": 0.0
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_airgradient_current_has_expected_fields() {
        let payload = airgradient_current();
        assert!(payload.get("serialno").is_some());
        assert!(payload.get("pm02").is_some());
        assert!(payload.get("rco2").is_some());
    }

    #[test]
    fn test_airgradient_future_has_new_fields() {
        let payload = airgradient_future();
        // Verify future fields exist
        assert!(payload.get("pm01Compensated").is_some());
        assert!(payload.get("soilMoisture").is_some());
        assert!(payload.get("uvIndex").is_some());
    }

    #[test]
    fn test_generic_unknown_has_numeric_and_non_numeric() {
        let payload = generic_unknown_fields();
        // Numeric fields
        assert!(payload.get("brand_new_field").unwrap().is_f64());
        // Non-numeric fields
        assert!(payload.get("string_field").unwrap().is_string());
        assert!(payload.get("bool_field").unwrap().is_boolean());
    }

    #[test]
    fn test_nested_structure_requires_paths() {
        let payload = nested_structure();
        // Verify nested access is required
        assert!(payload.get("readings").unwrap().get("environmental").is_some());
        assert!(payload.get("readings").unwrap().get("air_quality").is_some());
    }
}
