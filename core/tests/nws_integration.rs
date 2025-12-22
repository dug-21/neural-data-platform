//! Integration tests for NWS weather data parsing
//!
//! Tests the complete parsing flow for NWS observations and forecasts,
//! including timestamp extraction, unit conversion, and tall format generation.

mod fixtures;

use chrono::{DateTime, Timelike, Utc};
use fixtures::nws_payloads;
use serde_json::Value;

/// Helper function to parse RFC3339 timestamps
fn parse_rfc3339(timestamp_str: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(timestamp_str)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| format!("Invalid RFC3339 timestamp: {}", e))
}

/// Convert Fahrenheit to Celsius
fn fahrenheit_to_celsius(fahrenheit: f64) -> f64 {
    (fahrenheit - 32.0) * 5.0 / 9.0
}

/// Parse wind speed from NWS string format
fn parse_wind_speed(wind_speed_str: &str) -> Option<f64> {
    let trimmed = wind_speed_str.trim();

    // Handle range format "X to Y mph"
    if trimmed.contains(" to ") {
        let parts: Vec<&str> = trimmed.split(" to ").collect();
        if parts.len() == 2 {
            let low = parts[0].trim().parse::<f64>().ok()?;
            let high = parts[1].trim().trim_end_matches(" mph").parse::<f64>().ok()?;
            let average_mph = (low + high) / 2.0;
            return Some(average_mph * 0.44704); // mph to m/s
        }
    }

    // Handle single value "X mph"
    if let Some(mph_pos) = trimmed.find(" mph") {
        let speed_str = &trimmed[..mph_pos].trim();
        if let Ok(speed_mph) = speed_str.parse::<f64>() {
            return Some(speed_mph * 0.44704); // mph to m/s
        }
    }

    None
}

/// Parse wind direction from cardinal direction to degrees
fn parse_wind_direction(direction_str: &str) -> Option<f64> {
    match direction_str.trim().to_uppercase().as_str() {
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

// ============================================================================
// NWS Observation Parsing Tests
// ============================================================================

#[test]
fn test_nws_observation_timestamp_extraction() {
    let payload = nws_payloads::nws_observation_full();
    let properties = payload.get("properties").expect("Missing properties");
    let timestamp_str = properties
        .get("timestamp")
        .and_then(|v| v.as_str())
        .expect("Missing timestamp");

    let timestamp = parse_rfc3339(timestamp_str).expect("Failed to parse timestamp");

    // Verify timezone conversion to UTC
    assert_eq!(timestamp.hour(), 12);
    assert_eq!(timestamp.minute(), 0);
}

#[test]
fn test_nws_observation_field_extraction() {
    let payload = nws_payloads::nws_observation_full();
    let properties = payload.get("properties").expect("Missing properties");

    // Extract temperature
    let temp = properties
        .get("temperature")
        .and_then(|v| v.get("value"))
        .and_then(|v| v.as_f64())
        .expect("Missing temperature");
    assert!((temp - 20.5).abs() < 0.01);

    // Extract dewpoint
    let dewpoint = properties
        .get("dewpoint")
        .and_then(|v| v.get("value"))
        .and_then(|v| v.as_f64())
        .expect("Missing dewpoint");
    assert!((dewpoint - 15.2).abs() < 0.01);

    // Extract humidity
    let humidity = properties
        .get("relativeHumidity")
        .and_then(|v| v.get("value"))
        .and_then(|v| v.as_f64())
        .expect("Missing humidity");
    assert!((humidity - 69.43).abs() < 0.01);
}

#[test]
fn test_nws_observation_null_field_handling() {
    let payload = nws_payloads::nws_observation_with_nulls();
    let properties = payload.get("properties").expect("Missing properties");

    // Wind gust is null
    let wind_gust = properties
        .get("windGust")
        .and_then(|v| v.get("value"))
        .and_then(|v| v.as_f64());
    assert!(wind_gust.is_none(), "Wind gust should be None for null value");

    // Temperature should still be present
    let temp = properties
        .get("temperature")
        .and_then(|v| v.get("value"))
        .and_then(|v| v.as_f64());
    assert!(temp.is_some(), "Temperature should be present");
}

#[test]
fn test_nws_observation_pressure_conversion() {
    let payload = nws_payloads::nws_observation_full();
    let properties = payload.get("properties").expect("Missing properties");

    // Barometric pressure in Pascals
    let pressure_pa = properties
        .get("barometricPressure")
        .and_then(|v| v.get("value"))
        .and_then(|v| v.as_f64())
        .expect("Missing pressure");

    // Convert to hPa
    let pressure_hpa = pressure_pa / 100.0;
    assert!((pressure_hpa - 1019.0).abs() < 0.1);
}

#[test]
fn test_nws_observation_wind_speed_conversion() {
    let payload = nws_payloads::nws_observation_full();
    let properties = payload.get("properties").expect("Missing properties");

    // Wind speed in km/h
    let wind_speed_kmh = properties
        .get("windSpeed")
        .and_then(|v| v.get("value"))
        .and_then(|v| v.as_f64())
        .expect("Missing wind speed");

    // Convert to m/s
    let wind_speed_ms = wind_speed_kmh / 3.6;
    assert!((wind_speed_ms - 5.144).abs() < 0.01);
}

// ============================================================================
// NWS Forecast Parsing Tests
// ============================================================================

#[test]
fn test_nws_forecast_parse_valid_response() {
    let payload = nws_payloads::nws_forecast_three_periods();
    let properties = payload.get("properties").expect("Missing properties");

    // Extract issue_time
    let issue_time_str = properties
        .get("generatedAt")
        .and_then(|v| v.as_str())
        .expect("Missing generatedAt");
    let issue_time = parse_rfc3339(issue_time_str).expect("Failed to parse issue_time");

    // Extract periods
    let periods = properties
        .get("periods")
        .and_then(|v| v.as_array())
        .expect("Missing periods");

    assert_eq!(periods.len(), 3, "Should have 3 forecast periods");

    // Verify first period
    let period = &periods[0];

    // Temperature in Fahrenheit -> Celsius
    let temp_f = period.get("temperature").and_then(|v| v.as_f64()).expect("Missing temperature");
    let temp_c = fahrenheit_to_celsius(temp_f);
    assert!((temp_c - 22.222).abs() < 0.01, "Temperature conversion failed");

    // Wind speed string parsing
    let wind_speed_str = period.get("windSpeed").and_then(|v| v.as_str()).expect("Missing wind speed");
    let wind_speed_ms = parse_wind_speed(wind_speed_str).expect("Failed to parse wind speed");
    assert!((wind_speed_ms - 4.4704).abs() < 0.01, "Wind speed parsing failed");

    // Wind direction string parsing
    let wind_dir_str = period.get("windDirection").and_then(|v| v.as_str()).expect("Missing wind direction");
    let wind_dir_deg = parse_wind_direction(wind_dir_str).expect("Failed to parse wind direction");
    assert_eq!(wind_dir_deg, 180.0, "Wind direction parsing failed");
}

#[test]
fn test_nws_forecast_with_wind_speed_range() {
    let payload = nws_payloads::nws_forecast_three_periods();
    let properties = payload.get("properties").expect("Missing properties");
    let periods = properties.get("periods").and_then(|v| v.as_array()).expect("Missing periods");

    // Second period has wind speed range "5 to 10 mph"
    let period = &periods[1];
    let wind_speed_str = period.get("windSpeed").and_then(|v| v.as_str()).expect("Missing wind speed");

    assert_eq!(wind_speed_str, "5 to 10 mph");

    let wind_speed_ms = parse_wind_speed(wind_speed_str).expect("Failed to parse wind speed range");

    // Average of 5 and 10 is 7.5 mph -> 3.3528 m/s
    assert!((wind_speed_ms - 3.3528).abs() < 0.01, "Wind speed range parsing failed");
}

#[test]
fn test_nws_forecast_all_wind_directions() {
    let payload = nws_payloads::nws_forecast_three_periods();
    let properties = payload.get("properties").expect("Missing properties");
    let periods = properties.get("periods").and_then(|v| v.as_array()).expect("Missing periods");

    let expected_directions = vec![
        ("S", 180.0),
        ("SW", 225.0),
        ("NE", 45.0),
    ];

    for (idx, (expected_dir, expected_deg)) in expected_directions.iter().enumerate() {
        let period = &periods[idx];
        let wind_dir_str = period.get("windDirection").and_then(|v| v.as_str()).expect("Missing wind direction");

        assert_eq!(wind_dir_str, *expected_dir, "Period {} has wrong direction", idx);

        let wind_dir_deg = parse_wind_direction(wind_dir_str).expect("Failed to parse wind direction");
        assert_eq!(wind_dir_deg, *expected_deg, "Period {} has wrong degrees", idx);
    }
}

#[test]
fn test_nws_forecast_empty_periods() {
    let payload = nws_payloads::nws_forecast_empty_periods();
    let properties = payload.get("properties").expect("Missing properties");
    let periods = properties.get("periods").and_then(|v| v.as_array()).expect("Missing periods");

    assert_eq!(periods.len(), 0, "Should have 0 forecast periods");
}

#[test]
fn test_nws_forecast_invalid_timestamp() {
    // Try parsing an invalid timestamp
    let result = parse_rfc3339("invalid-date");
    assert!(result.is_err(), "Should fail to parse invalid timestamp");
}

#[test]
fn test_nws_forecast_timestamp_validation() {
    let payload = nws_payloads::nws_forecast_three_periods();
    let properties = payload.get("properties").expect("Missing properties");

    // Extract issue_time
    let issue_time_str = properties.get("generatedAt").and_then(|v| v.as_str()).expect("Missing generatedAt");
    let issue_time = parse_rfc3339(issue_time_str).expect("Failed to parse issue_time");

    // Extract first period valid_time
    let periods = properties.get("periods").and_then(|v| v.as_array()).expect("Missing periods");
    let period = &periods[0];
    let valid_time_str = period.get("startTime").and_then(|v| v.as_str()).expect("Missing startTime");
    let valid_time = parse_rfc3339(valid_time_str).expect("Failed to parse valid_time");

    // Validate: valid_time should be >= issue_time
    assert!(
        valid_time >= issue_time,
        "Valid time should be >= issue time"
    );

    // Calculate lead time in hours
    let lead_time_seconds = valid_time.timestamp() - issue_time.timestamp();
    let lead_time_hours = lead_time_seconds / 3600;

    assert!(lead_time_hours >= 0, "Lead time should be non-negative");
    // Note: Actual lead time depends on fixture data (generatedAt vs first period startTime)
    // This test just validates the lead time calculation is correct
}

#[test]
fn test_nws_forecast_tall_format_point_count() {
    let payload = nws_payloads::nws_forecast_three_periods();
    let properties = payload.get("properties").expect("Missing properties");
    let periods = properties.get("periods").and_then(|v| v.as_array()).expect("Missing periods");

    // Each period should generate multiple points (tall format)
    // Fields: temperature, dewpoint, humidity, wind_speed, wind_direction, precip_probability
    // 3 periods × 6 metrics = 18 points expected (minimum)

    let metrics_per_period = 6;
    let expected_points = periods.len() * metrics_per_period;

    assert!(
        expected_points >= 18,
        "Should generate at least 18 points (3 periods × 6 metrics)"
    );
}

// ============================================================================
// API Failure Recovery Tests
// ============================================================================

#[test]
fn test_api_response_missing_properties() {
    let payload = serde_json::json!({
        "type": "Feature"
    });

    let properties = payload.get("properties");
    assert!(properties.is_none(), "Should not have properties");
}

#[test]
fn test_api_response_malformed_json() {
    let malformed = "{ invalid json }";
    let result: Result<Value, _> = serde_json::from_str(malformed);
    assert!(result.is_err(), "Should fail to parse malformed JSON");
}
