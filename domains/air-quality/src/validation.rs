//! Validation rules for AirGradient ONE sensor data
//!
//! Based on spec v1.2.0 and sensor hardware specifications:
//! - CO2: SenseAir S8 (380-10,000 ppm)
//! - PM: PMS5003 (0-500 µg/m³)
//! - TVOC/NOx: SGP41 (index 1-500)
//! - Temperature: SHT40 (-10 to 50°C)
//! - Humidity: SHT40 (0-100%)

use crate::types::AirQualityReading;
use thiserror::Error;

/// Validation errors
#[derive(Debug, Error, PartialEq)]
pub enum ValidationError {
    #[error("CO2 out of range: {0} ppm (valid: 380-10000)")]
    Co2OutOfRange(i32),

    #[error("PM2.5 out of range: {0} µg/m³ (valid: 0-500)")]
    Pm25OutOfRange(f32),

    #[error("TVOC index out of range: {0} (valid: 1-500)")]
    TvocOutOfRange(i32),

    #[error("NOx index out of range: {0} (valid: 1-500)")]
    NoxOutOfRange(i32),

    #[error("Temperature out of range: {0}°C (valid: -10 to 50)")]
    TemperatureOutOfRange(f32),

    #[error("Humidity out of range: {0}% (valid: 0-100)")]
    HumidityOutOfRange(f32),

    #[error("PM value out of range: {0} µg/m³ (valid: 0-500)")]
    PmOutOfRange(f32),

    #[error("WiFi signal out of range: {0} dBm (valid: -100 to 0)")]
    WiFiOutOfRange(i32),

    #[error("Multiple validation errors: {0:?}")]
    MultipleErrors(Vec<ValidationError>),
}

/// Validation ranges (from spec)
pub mod ranges {
    /// CO2 valid range (ppm)
    pub const CO2_MIN: i32 = 380;
    pub const CO2_MAX: i32 = 10_000;

    /// PM concentrations valid range (µg/m³)
    pub const PM_MIN: f32 = 0.0;
    pub const PM_MAX: f32 = 500.0;

    /// TVOC/NOx index valid range
    pub const VOC_INDEX_MIN: i32 = 1;
    pub const VOC_INDEX_MAX: i32 = 500;

    /// Temperature valid range (°C)
    pub const TEMP_MIN: f32 = -10.0;
    pub const TEMP_MAX: f32 = 50.0;

    /// Humidity valid range (%)
    pub const HUMIDITY_MIN: f32 = 0.0;
    pub const HUMIDITY_MAX: f32 = 100.0;

    /// WiFi signal strength valid range (dBm)
    pub const WIFI_MIN: i32 = -100;
    pub const WIFI_MAX: i32 = 0;
}

/// Validate a complete air quality reading
///
/// Returns Ok(()) if all present values are within valid ranges.
/// Optional (None) values are not validated.
///
/// Collects all validation errors and returns them together.
pub fn validate_reading(reading: &AirQualityReading) -> Result<(), ValidationError> {
    let mut errors = Vec::new();

    // Validate CO2
    if let Some(co2) = reading.metrics.rco2 {
        if let Err(e) = validate_co2(co2) {
            errors.push(e);
        }
    }

    // Validate PM values
    if let Some(pm) = reading.particles.pm02 {
        if let Err(e) = validate_pm(pm, "pm02") {
            errors.push(e);
        }
    }
    if let Some(pm) = reading.particles.pm01 {
        if let Err(e) = validate_pm(pm, "pm01") {
            errors.push(e);
        }
    }
    if let Some(pm) = reading.particles.pm10 {
        if let Err(e) = validate_pm(pm, "pm10") {
            errors.push(e);
        }
    }

    // Validate gas indices
    if let Some(tvoc) = reading.gases.tvoc_index {
        if let Err(e) = validate_tvoc_index(tvoc) {
            errors.push(e);
        }
    }
    if let Some(nox) = reading.gases.nox_index {
        if let Err(e) = validate_nox_index(nox) {
            errors.push(e);
        }
    }

    // Validate environmental
    if let Some(temp) = reading.environment.atmp {
        if let Err(e) = validate_temperature(temp) {
            errors.push(e);
        }
    }
    if let Some(temp) = reading.environment.atmp_compensated {
        if let Err(e) = validate_temperature(temp) {
            errors.push(e);
        }
    }
    if let Some(hum) = reading.environment.rhum {
        if let Err(e) = validate_humidity(hum) {
            errors.push(e);
        }
    }
    if let Some(hum) = reading.environment.rhum_compensated {
        if let Err(e) = validate_humidity(hum) {
            errors.push(e);
        }
    }

    // Validate WiFi
    if let Some(wifi) = reading.device.wifi {
        if let Err(e) = validate_wifi(wifi) {
            errors.push(e);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else if errors.len() == 1 {
        Err(errors.into_iter().next().unwrap())
    } else {
        Err(ValidationError::MultipleErrors(errors))
    }
}

/// Validate CO2 value
fn validate_co2(value: i32) -> Result<(), ValidationError> {
    if value < ranges::CO2_MIN || value > ranges::CO2_MAX {
        Err(ValidationError::Co2OutOfRange(value))
    } else {
        Ok(())
    }
}

/// Validate PM concentration value
fn validate_pm(value: f32, _field: &str) -> Result<(), ValidationError> {
    if value < ranges::PM_MIN || value > ranges::PM_MAX {
        Err(ValidationError::PmOutOfRange(value))
    } else {
        Ok(())
    }
}

/// Validate TVOC index
fn validate_tvoc_index(value: i32) -> Result<(), ValidationError> {
    if value < ranges::VOC_INDEX_MIN || value > ranges::VOC_INDEX_MAX {
        Err(ValidationError::TvocOutOfRange(value))
    } else {
        Ok(())
    }
}

/// Validate NOx index
fn validate_nox_index(value: i32) -> Result<(), ValidationError> {
    if value < ranges::VOC_INDEX_MIN || value > ranges::VOC_INDEX_MAX {
        Err(ValidationError::NoxOutOfRange(value))
    } else {
        Ok(())
    }
}

/// Validate temperature value
fn validate_temperature(value: f32) -> Result<(), ValidationError> {
    if value < ranges::TEMP_MIN || value > ranges::TEMP_MAX {
        Err(ValidationError::TemperatureOutOfRange(value))
    } else {
        Ok(())
    }
}

/// Validate humidity value
fn validate_humidity(value: f32) -> Result<(), ValidationError> {
    if value < ranges::HUMIDITY_MIN || value > ranges::HUMIDITY_MAX {
        Err(ValidationError::HumidityOutOfRange(value))
    } else {
        Ok(())
    }
}

/// Validate WiFi signal strength
fn validate_wifi(value: i32) -> Result<(), ValidationError> {
    if value < ranges::WIFI_MIN || value > ranges::WIFI_MAX {
        Err(ValidationError::WiFiOutOfRange(value))
    } else {
        Ok(())
    }
}

// =============================================================================
// TESTS - London School TDD: Behavior verification through mocks and tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;
    use chrono::Utc;

    /// Mock reading creator for testing
    fn create_valid_reading() -> AirQualityReading {
        AirQualityReading {
            device: DeviceMetadata {
                wifi: Some(-50),
                serialno: "test-device".to_string(),
                boot_count: Some(1),
                boot: Some(1),
                led_mode: Some("co2".to_string()),
                firmware: Some("3.0.0".to_string()),
                model: Some("ONE".to_string()),
            },
            particles: ParticleData {
                pm01: Some(5.0),
                pm02: Some(12.5),
                pm10: Some(18.0),
                pm02_compensated: Some(11.8),
                pm01_standard: Some(5.2),
                pm02_standard: Some(12.8),
                pm10_standard: Some(18.5),
                pm003_count: Some(1000.0),
                pm005_count: Some(800.0),
                pm01_count: Some(400.0),
                pm02_count: Some(100.0),
                pm50_count: Some(50.0),
                pm10_count: Some(10.0),
            },
            gases: GasData {
                tvoc_index: Some(120),
                tvoc_raw: Some(30000.0),
                nox_index: Some(100),
                nox_raw: Some(25000.0),
            },
            environment: EnvironmentalData {
                atmp: Some(22.0),
                atmp_compensated: Some(21.5),
                rhum: Some(45.0),
                rhum_compensated: Some(44.0),
            },
            metrics: QualityMetrics { rco2: Some(450) },
            timestamp: Some(Utc::now()),
        }
    }

    // =========================================================================
    // CO2 Validation Tests
    // =========================================================================

    #[test]
    fn test_validate_co2_valid_range() {
        assert!(validate_co2(400).is_ok());
        assert!(validate_co2(380).is_ok()); // Min
        assert!(validate_co2(10_000).is_ok()); // Max
        assert!(validate_co2(5_000).is_ok()); // Mid
    }

    #[test]
    fn test_validate_co2_below_minimum() {
        let result = validate_co2(300);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ValidationError::Co2OutOfRange(300));
    }

    #[test]
    fn test_validate_co2_above_maximum() {
        let result = validate_co2(15_000);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ValidationError::Co2OutOfRange(15_000));
    }

    // =========================================================================
    // PM Validation Tests
    // =========================================================================

    #[test]
    fn test_validate_pm_valid_range() {
        assert!(validate_pm(0.0, "pm02").is_ok()); // Min
        assert!(validate_pm(12.5, "pm02").is_ok()); // Normal
        assert!(validate_pm(500.0, "pm02").is_ok()); // Max
        assert!(validate_pm(250.0, "pm10").is_ok()); // Mid
    }

    #[test]
    fn test_validate_pm_below_minimum() {
        let result = validate_pm(-5.0, "pm02");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ValidationError::PmOutOfRange(-5.0));
    }

    #[test]
    fn test_validate_pm_above_maximum() {
        let result = validate_pm(600.0, "pm02");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ValidationError::PmOutOfRange(600.0));
    }

    // =========================================================================
    // TVOC/NOx Index Validation Tests
    // =========================================================================

    #[test]
    fn test_validate_tvoc_index_valid_range() {
        assert!(validate_tvoc_index(1).is_ok()); // Min
        assert!(validate_tvoc_index(250).is_ok()); // Mid
        assert!(validate_tvoc_index(500).is_ok()); // Max
    }

    #[test]
    fn test_validate_tvoc_index_below_minimum() {
        let result = validate_tvoc_index(0);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ValidationError::TvocOutOfRange(0));
    }

    #[test]
    fn test_validate_tvoc_index_above_maximum() {
        let result = validate_tvoc_index(501);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), ValidationError::TvocOutOfRange(501));
    }

    #[test]
    fn test_validate_nox_index_valid_range() {
        assert!(validate_nox_index(1).is_ok());
        assert!(validate_nox_index(150).is_ok());
        assert!(validate_nox_index(500).is_ok());
    }

    #[test]
    fn test_validate_nox_index_out_of_range() {
        assert!(validate_nox_index(0).is_err());
        assert!(validate_nox_index(600).is_err());
    }

    // =========================================================================
    // Temperature Validation Tests
    // =========================================================================

    #[test]
    fn test_validate_temperature_valid_range() {
        assert!(validate_temperature(-10.0).is_ok()); // Min
        assert!(validate_temperature(20.0).is_ok()); // Normal
        assert!(validate_temperature(50.0).is_ok()); // Max
    }

    #[test]
    fn test_validate_temperature_below_minimum() {
        let result = validate_temperature(-15.0);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            ValidationError::TemperatureOutOfRange(-15.0)
        );
    }

    #[test]
    fn test_validate_temperature_above_maximum() {
        let result = validate_temperature(60.0);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            ValidationError::TemperatureOutOfRange(60.0)
        );
    }

    // =========================================================================
    // Humidity Validation Tests
    // =========================================================================

    #[test]
    fn test_validate_humidity_valid_range() {
        assert!(validate_humidity(0.0).is_ok()); // Min
        assert!(validate_humidity(50.0).is_ok()); // Mid
        assert!(validate_humidity(100.0).is_ok()); // Max
    }

    #[test]
    fn test_validate_humidity_below_minimum() {
        let result = validate_humidity(-5.0);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            ValidationError::HumidityOutOfRange(-5.0)
        );
    }

    #[test]
    fn test_validate_humidity_above_maximum() {
        let result = validate_humidity(105.0);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            ValidationError::HumidityOutOfRange(105.0)
        );
    }

    // =========================================================================
    // WiFi Validation Tests
    // =========================================================================

    #[test]
    fn test_validate_wifi_valid_range() {
        assert!(validate_wifi(-100).is_ok()); // Min
        assert!(validate_wifi(-50).is_ok()); // Mid
        assert!(validate_wifi(0).is_ok()); // Max
    }

    #[test]
    fn test_validate_wifi_out_of_range() {
        assert!(validate_wifi(-101).is_err());
        assert!(validate_wifi(1).is_err());
    }

    // =========================================================================
    // Complete Reading Validation Tests
    // =========================================================================

    #[test]
    fn test_validate_reading_all_valid() {
        let reading = create_valid_reading();
        let result = validate_reading(&reading);

        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_reading_with_none_values() {
        let mut reading = create_valid_reading();
        reading.particles.pm01 = None;
        reading.gases.tvoc_index = None;
        reading.environment.atmp_compensated = None;

        // Should still pass - None values are not validated
        assert!(validate_reading(&reading).is_ok());
    }

    #[test]
    fn test_validate_reading_invalid_co2() {
        let mut reading = create_valid_reading();
        reading.metrics.rco2 = Some(100); // Too low

        let result = validate_reading(&reading);
        assert!(result.is_err());

        match result.unwrap_err() {
            ValidationError::Co2OutOfRange(100) => (),
            _ => panic!("Expected Co2OutOfRange error"),
        }
    }

    #[test]
    fn test_validate_reading_invalid_pm25() {
        let mut reading = create_valid_reading();
        reading.particles.pm02 = Some(600.0); // Too high

        let result = validate_reading(&reading);
        assert!(result.is_err());

        match result.unwrap_err() {
            ValidationError::PmOutOfRange(600.0) => (),
            _ => panic!("Expected PmOutOfRange error"),
        }
    }

    #[test]
    fn test_validate_reading_multiple_errors() {
        let mut reading = create_valid_reading();
        reading.metrics.rco2 = Some(100); // Invalid
        reading.particles.pm02 = Some(600.0); // Invalid
        reading.gases.tvoc_index = Some(600); // Invalid

        let result = validate_reading(&reading);
        assert!(result.is_err());

        match result.unwrap_err() {
            ValidationError::MultipleErrors(errors) => {
                assert_eq!(errors.len(), 3);
            }
            _ => panic!("Expected MultipleErrors"),
        }
    }

    #[test]
    fn test_validate_reading_minimal_data() {
        let reading = AirQualityReading {
            device: DeviceMetadata {
                wifi: None,
                serialno: "minimal".to_string(),
                boot_count: None,
                boot: None,
                led_mode: None,
                firmware: None,
                model: None,
            },
            particles: ParticleData {
                pm01: None,
                pm02: Some(10.0),
                pm10: None,
                pm02_compensated: None,
                pm01_standard: None,
                pm02_standard: None,
                pm10_standard: None,
                pm003_count: None,
                pm005_count: None,
                pm01_count: None,
                pm02_count: None,
                pm50_count: None,
                pm10_count: None,
            },
            gases: GasData {
                tvoc_index: None,
                tvoc_raw: None,
                nox_index: None,
                nox_raw: None,
            },
            environment: EnvironmentalData {
                atmp: None,
                atmp_compensated: None,
                rhum: None,
                rhum_compensated: None,
            },
            metrics: QualityMetrics { rco2: Some(450) },
            timestamp: Some(Utc::now()),
        };

        // Should validate successfully with minimal valid data
        assert!(validate_reading(&reading).is_ok());
    }

    #[test]
    fn test_validation_error_display() {
        let error = ValidationError::Co2OutOfRange(100);
        let display = format!("{}", error);

        assert!(display.contains("CO2 out of range"));
        assert!(display.contains("100"));
        assert!(display.contains("380-10000"));
    }

    #[test]
    fn test_validation_ranges_constants() {
        use ranges::*;

        assert_eq!(CO2_MIN, 380);
        assert_eq!(CO2_MAX, 10_000);
        assert_eq!(PM_MIN, 0.0);
        assert_eq!(PM_MAX, 500.0);
        assert_eq!(VOC_INDEX_MIN, 1);
        assert_eq!(VOC_INDEX_MAX, 500);
        assert_eq!(TEMP_MIN, -10.0);
        assert_eq!(TEMP_MAX, 50.0);
        assert_eq!(HUMIDITY_MIN, 0.0);
        assert_eq!(HUMIDITY_MAX, 100.0);
        assert_eq!(WIFI_MIN, -100);
        assert_eq!(WIFI_MAX, 0);
    }
}
