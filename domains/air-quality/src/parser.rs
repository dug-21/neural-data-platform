//! Parser for AirGradient ONE JSON payloads
//!
//! Supports both MQTT and Local API payload formats with graceful handling
//! of partial data (Option fields for all non-essential values).

use crate::types::*;
use chrono::Utc;
use serde_json::Value;
use thiserror::Error;

/// Parser errors
#[derive(Debug, Error)]
pub enum ParserError {
    #[error("JSON parse error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Invalid field type: {0}")]
    InvalidType(String),

    #[error("Invalid payload structure: {0}")]
    InvalidStructure(String),
}

/// Parse MQTT payload (typically contains subset of fields)
///
/// MQTT payloads usually include:
/// - wifi, serialno, rco2
/// - pm01, pm02, pm10
/// - atmp, rhum
/// - tvocIndex, noxIndex
pub fn parse_mqtt_payload(json: &str) -> Result<AirQualityReading, ParserError> {
    let value: Value = serde_json::from_str(json)?;

    // Extract required fields
    let serialno = value
        .get("serialno")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ParserError::MissingField("serialno".to_string()))?
        .to_string();

    // Build reading with available fields
    Ok(AirQualityReading {
        device: DeviceMetadata {
            wifi: value.get("wifi").and_then(|v| v.as_i64()).map(|v| v as i32),
            serialno,
            boot_count: value
                .get("bootCount")
                .and_then(|v| v.as_i64())
                .map(|v| v as i32),
            boot: value.get("boot").and_then(|v| v.as_i64()).map(|v| v as i32),
            led_mode: value
                .get("ledMode")
                .and_then(|v| v.as_str())
                .map(String::from),
            firmware: value
                .get("firmware")
                .and_then(|v| v.as_str())
                .map(String::from),
            model: value
                .get("model")
                .and_then(|v| v.as_str())
                .map(String::from),
        },
        particles: ParticleData {
            pm01: value.get("pm01").and_then(|v| v.as_f64()).map(|v| v as f32),
            pm02: value.get("pm02").and_then(|v| v.as_f64()).map(|v| v as f32),
            pm10: value.get("pm10").and_then(|v| v.as_f64()).map(|v| v as f32),
            pm02_compensated: value
                .get("pm02Compensated")
                .and_then(|v| v.as_f64())
                .map(|v| v as f32),
            pm01_standard: value
                .get("pm01Standard")
                .and_then(|v| v.as_f64())
                .map(|v| v as f32),
            pm02_standard: value
                .get("pm02Standard")
                .and_then(|v| v.as_f64())
                .map(|v| v as f32),
            pm10_standard: value
                .get("pm10Standard")
                .and_then(|v| v.as_f64())
                .map(|v| v as f32),
            pm003_count: value
                .get("pm003Count")
                .and_then(|v| v.as_f64())
                .map(|v| v as f32),
            pm005_count: value
                .get("pm005Count")
                .and_then(|v| v.as_f64())
                .map(|v| v as f32),
            pm01_count: value
                .get("pm01Count")
                .and_then(|v| v.as_f64())
                .map(|v| v as f32),
            pm02_count: value
                .get("pm02Count")
                .and_then(|v| v.as_f64())
                .map(|v| v as f32),
            pm50_count: value
                .get("pm50Count")
                .and_then(|v| v.as_f64())
                .map(|v| v as f32),
            pm10_count: value
                .get("pm10Count")
                .and_then(|v| v.as_f64())
                .map(|v| v as f32),
        },
        gases: GasData {
            tvoc_index: value
                .get("tvocIndex")
                .and_then(|v| v.as_i64())
                .map(|v| v as i32),
            tvoc_raw: value
                .get("tvocRaw")
                .and_then(|v| v.as_f64())
                .map(|v| v as f32),
            nox_index: value
                .get("noxIndex")
                .and_then(|v| v.as_i64())
                .map(|v| v as i32),
            nox_raw: value
                .get("noxRaw")
                .and_then(|v| v.as_f64())
                .map(|v| v as f32),
        },
        environment: EnvironmentalData {
            atmp: value.get("atmp").and_then(|v| v.as_f64()).map(|v| v as f32),
            atmp_compensated: value
                .get("atmpCompensated")
                .and_then(|v| v.as_f64())
                .map(|v| v as f32),
            rhum: value.get("rhum").and_then(|v| v.as_f64()).map(|v| v as f32),
            rhum_compensated: value
                .get("rhumCompensated")
                .and_then(|v| v.as_f64())
                .map(|v| v as f32),
        },
        metrics: QualityMetrics {
            rco2: value.get("rco2").and_then(|v| v.as_i64()).map(|v| v as i32),
        },
        timestamp: Some(Utc::now()),
    })
}

/// Parse Local API payload (contains all available fields)
///
/// Local API typically provides complete data including:
/// - All MQTT fields
/// - Compensated values
/// - Standard PM values
/// - Particle counts
/// - Raw gas sensor values
pub fn parse_local_api_payload(json: &str) -> Result<AirQualityReading, ParserError> {
    // Local API uses same structure as MQTT but with more fields populated
    parse_mqtt_payload(json)
}

// =============================================================================
// TESTS - London School TDD: Tests first, mocks for collaborators
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Test data - Complete MQTT payload (from actual sensor)
    const MQTT_COMPLETE_PAYLOAD: &str = r#"{
        "wifi": -42,
        "serialno": "airgradient:123456",
        "rco2": 650,
        "pm01": 5,
        "pm02": 12,
        "pm10": 18,
        "atmp": 22.5,
        "rhum": 45.2,
        "tvocIndex": 120,
        "noxIndex": 105
    }"#;

    /// Test data - Minimal MQTT payload (only required fields)
    const MQTT_MINIMAL_PAYLOAD: &str = r#"{
        "serialno": "airgradient:minimal"
    }"#;

    /// Test data - Complete Local API payload (all 29 fields)
    const LOCAL_API_COMPLETE_PAYLOAD: &str = r#"{
        "wifi": -45,
        "serialno": "airgradient:local-test",
        "rco2": 450,
        "pm01": 3.5,
        "pm02": 8.2,
        "pm10": 15.7,
        "pm02Compensated": 7.8,
        "pm01Standard": 3.6,
        "pm02Standard": 8.4,
        "pm10Standard": 16.0,
        "pm003Count": 1500,
        "pm005Count": 1000,
        "pm01Count": 500,
        "pm02Count": 150,
        "pm50Count": 50,
        "pm10Count": 15,
        "atmp": 25.0,
        "atmpCompensated": 24.2,
        "rhum": 50.0,
        "rhumCompensated": 48.5,
        "tvocIndex": 150,
        "tvocRaw": 35000,
        "noxIndex": 120,
        "noxRaw": 30000,
        "bootCount": 10,
        "boot": 1,
        "ledMode": "pm",
        "firmware": "3.1.1",
        "model": "I-9PSL"
    }"#;

    /// Test data - Invalid JSON
    const INVALID_JSON: &str = r#"{ "serialno": "test", invalid }"#;

    /// Test data - Missing required field
    const MISSING_SERIALNO: &str = r#"{
        "wifi": -50,
        "pm02": 10
    }"#;

    // =========================================================================
    // MQTT Parser Tests
    // =========================================================================

    #[test]
    fn test_parse_mqtt_complete_payload_success() {
        let result = parse_mqtt_payload(MQTT_COMPLETE_PAYLOAD);

        assert!(result.is_ok());
        let reading = result.unwrap();

        // Verify device metadata
        assert_eq!(reading.device.serialno, "airgradient:123456");
        assert_eq!(reading.device.wifi, Some(-42));

        // Verify particle data
        assert_eq!(reading.particles.pm01, Some(5.0));
        assert_eq!(reading.particles.pm02, Some(12.0));
        assert_eq!(reading.particles.pm10, Some(18.0));

        // Verify environmental data
        assert_eq!(reading.environment.atmp, Some(22.5));
        assert_eq!(reading.environment.rhum, Some(45.2));

        // Verify gas data
        assert_eq!(reading.gases.tvoc_index, Some(120));
        assert_eq!(reading.gases.nox_index, Some(105));

        // Verify quality metrics
        assert_eq!(reading.metrics.rco2, Some(650));

        // Verify timestamp was added
        assert!(reading.timestamp.is_some());
    }

    #[test]
    fn test_parse_mqtt_minimal_payload_success() {
        let result = parse_mqtt_payload(MQTT_MINIMAL_PAYLOAD);

        assert!(result.is_ok());
        let reading = result.unwrap();

        // Required field should be present
        assert_eq!(reading.device.serialno, "airgradient:minimal");

        // Optional fields should be None
        assert_eq!(reading.device.wifi, None);
        assert_eq!(reading.particles.pm02, None);
        assert_eq!(reading.metrics.rco2, None);
    }

    #[test]
    fn test_parse_mqtt_handles_partial_data_gracefully() {
        let partial = r#"{
            "serialno": "airgradient:partial",
            "pm02": 15.5,
            "atmp": 20.0
        }"#;

        let result = parse_mqtt_payload(partial);
        assert!(result.is_ok());

        let reading = result.unwrap();
        assert_eq!(reading.device.serialno, "airgradient:partial");
        assert_eq!(reading.particles.pm02, Some(15.5));
        assert_eq!(reading.environment.atmp, Some(20.0));

        // Fields not in payload should be None
        assert_eq!(reading.particles.pm01, None);
        assert_eq!(reading.gases.tvoc_index, None);
    }

    #[test]
    fn test_parse_mqtt_invalid_json_returns_error() {
        let result = parse_mqtt_payload(INVALID_JSON);

        assert!(result.is_err());
        match result {
            Err(ParserError::JsonError(_)) => (),
            _ => panic!("Expected JsonError"),
        }
    }

    #[test]
    fn test_parse_mqtt_missing_required_field_returns_error() {
        let result = parse_mqtt_payload(MISSING_SERIALNO);

        assert!(result.is_err());
        match result {
            Err(ParserError::MissingField(field)) => {
                assert_eq!(field, "serialno");
            }
            _ => panic!("Expected MissingField error"),
        }
    }

    #[test]
    fn test_parse_mqtt_handles_null_values() {
        let with_nulls = r#"{
            "serialno": "airgradient:null-test",
            "wifi": null,
            "pm02": null,
            "rco2": 400
        }"#;

        let result = parse_mqtt_payload(with_nulls);
        assert!(result.is_ok());

        let reading = result.unwrap();
        assert_eq!(reading.device.wifi, None);
        assert_eq!(reading.particles.pm02, None);
        assert_eq!(reading.metrics.rco2, Some(400));
    }

    // =========================================================================
    // Local API Parser Tests
    // =========================================================================

    #[test]
    fn test_parse_local_api_complete_payload_all_29_fields() {
        let result = parse_local_api_payload(LOCAL_API_COMPLETE_PAYLOAD);

        assert!(result.is_ok());
        let reading = result.unwrap();

        // Verify all device metadata fields (7 fields)
        assert_eq!(reading.device.serialno, "airgradient:local-test");
        assert_eq!(reading.device.wifi, Some(-45));
        assert_eq!(reading.device.boot_count, Some(10));
        assert_eq!(reading.device.boot, Some(1));
        assert_eq!(reading.device.led_mode, Some("pm".to_string()));
        assert_eq!(reading.device.firmware, Some("3.1.1".to_string()));
        assert_eq!(reading.device.model, Some("I-9PSL".to_string()));

        // Verify all particle data fields (13 fields)
        assert_eq!(reading.particles.pm01, Some(3.5));
        assert_eq!(reading.particles.pm02, Some(8.2));
        assert_eq!(reading.particles.pm10, Some(15.7));
        assert_eq!(reading.particles.pm02_compensated, Some(7.8));
        assert_eq!(reading.particles.pm01_standard, Some(3.6));
        assert_eq!(reading.particles.pm02_standard, Some(8.4));
        assert_eq!(reading.particles.pm10_standard, Some(16.0));
        assert_eq!(reading.particles.pm003_count, Some(1500.0));
        assert_eq!(reading.particles.pm005_count, Some(1000.0));
        assert_eq!(reading.particles.pm01_count, Some(500.0));
        assert_eq!(reading.particles.pm02_count, Some(150.0));
        assert_eq!(reading.particles.pm50_count, Some(50.0));
        assert_eq!(reading.particles.pm10_count, Some(15.0));

        // Verify all gas data fields (4 fields)
        assert_eq!(reading.gases.tvoc_index, Some(150));
        assert_eq!(reading.gases.tvoc_raw, Some(35000.0));
        assert_eq!(reading.gases.nox_index, Some(120));
        assert_eq!(reading.gases.nox_raw, Some(30000.0));

        // Verify all environmental data fields (4 fields)
        assert_eq!(reading.environment.atmp, Some(25.0));
        assert_eq!(reading.environment.atmp_compensated, Some(24.2));
        assert_eq!(reading.environment.rhum, Some(50.0));
        assert_eq!(reading.environment.rhum_compensated, Some(48.5));

        // Verify quality metrics (1 field)
        assert_eq!(reading.metrics.rco2, Some(450));

        // Total: 29 fields verified
    }

    #[test]
    fn test_parse_local_api_reuses_mqtt_parser() {
        // Local API should handle MQTT format too
        let result = parse_local_api_payload(MQTT_COMPLETE_PAYLOAD);

        assert!(result.is_ok());
        let reading = result.unwrap();
        assert_eq!(reading.device.serialno, "airgradient:123456");
    }

    // =========================================================================
    // Type Conversion Tests
    // =========================================================================

    #[test]
    fn test_parser_converts_numeric_types_correctly() {
        let json = r#"{
            "serialno": "test",
            "wifi": -50,
            "rco2": 450,
            "pm02": 12.5,
            "tvocIndex": 120
        }"#;

        let result = parse_mqtt_payload(json);
        assert!(result.is_ok());

        let reading = result.unwrap();
        assert_eq!(reading.device.wifi.unwrap(), -50_i32);
        assert_eq!(reading.metrics.rco2.unwrap(), 450_i32);
        assert_eq!(reading.particles.pm02.unwrap(), 12.5_f32);
        assert_eq!(reading.gases.tvoc_index.unwrap(), 120_i32);
    }

    #[test]
    fn test_parser_handles_float_as_integer_in_json() {
        let json = r#"{
            "serialno": "test",
            "pm01": 5,
            "pm02": 12,
            "pm10": 18
        }"#;

        let result = parse_mqtt_payload(json);
        assert!(result.is_ok());

        let reading = result.unwrap();
        assert_eq!(reading.particles.pm01, Some(5.0));
        assert_eq!(reading.particles.pm02, Some(12.0));
        assert_eq!(reading.particles.pm10, Some(18.0));
    }

    // =========================================================================
    // Edge Case Tests
    // =========================================================================

    #[test]
    fn test_parser_handles_empty_string_fields() {
        let json = r#"{
            "serialno": "",
            "ledMode": "",
            "firmware": ""
        }"#;

        let result = parse_mqtt_payload(json);
        assert!(result.is_ok());

        let reading = result.unwrap();
        assert_eq!(reading.device.serialno, "");
        assert_eq!(reading.device.led_mode, Some("".to_string()));
        assert_eq!(reading.device.firmware, Some("".to_string()));
    }

    #[test]
    fn test_parser_handles_large_particle_counts() {
        let json = r#"{
            "serialno": "test",
            "pm003Count": 99999.99,
            "pm10Count": 50000.5
        }"#;

        let result = parse_mqtt_payload(json);
        assert!(result.is_ok());

        let reading = result.unwrap();
        assert_eq!(reading.particles.pm003_count, Some(99999.99));
        assert_eq!(reading.particles.pm10_count, Some(50000.5));
    }

    #[test]
    fn test_parser_error_display() {
        let error = ParserError::MissingField("test_field".to_string());
        let display = format!("{}", error);

        assert!(display.contains("Missing required field"));
        assert!(display.contains("test_field"));
    }
}
