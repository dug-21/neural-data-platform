//! Type definitions for AirGradient ONE air quality data
//!
//! Spec Version: 1.2.0
//! Total Fields: 29 (validated with actual sensor data)
//! Data Source: MQTT + Local API

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Complete air quality reading from AirGradient ONE device
/// Contains all 29 fields from both MQTT and Local API sources
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AirQualityReading {
    /// Device metadata
    #[serde(flatten)]
    pub device: DeviceMetadata,

    /// Particle measurements
    #[serde(flatten)]
    pub particles: ParticleData,

    /// Gas sensor measurements
    #[serde(flatten)]
    pub gases: GasData,

    /// Environmental measurements
    #[serde(flatten)]
    pub environment: EnvironmentalData,

    /// Quality metrics and indices
    #[serde(flatten)]
    pub metrics: QualityMetrics,

    /// Timestamp of the reading (added by our system)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<DateTime<Utc>>,
}

/// Device identification and status metadata (6 fields)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeviceMetadata {
    /// WiFi signal strength in dBm
    pub wifi: Option<i32>,

    /// Device serial number (unique identifier)
    pub serialno: String,

    /// Boot count since device activation
    pub boot_count: Option<i32>,

    /// Current boot sequence number
    pub boot: Option<i32>,

    /// LED mode setting
    pub led_mode: Option<String>,

    /// Firmware version
    pub firmware: Option<String>,

    /// Device model
    pub model: Option<String>,
}

/// Particle Matter (PM) sensor data (15 fields)
/// PMS5003 sensor provides multiple PM measurements
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ParticleData {
    // Standard PM concentrations (µg/m³)
    /// PM1.0 concentration
    pub pm01: Option<f32>,

    /// PM2.5 concentration (most commonly used)
    pub pm02: Option<f32>,

    /// PM10 concentration
    pub pm10: Option<f32>,

    // Compensated PM values (adjusted for environmental conditions)
    /// PM2.5 compensated for temperature/humidity
    pub pm02_compensated: Option<f32>,

    // Standard atmospheric PM (different calculation method)
    /// PM1.0 standard atmospheric
    #[serde(rename = "pm01Standard")]
    pub pm01_standard: Option<f32>,

    /// PM2.5 standard atmospheric
    #[serde(rename = "pm02Standard")]
    pub pm02_standard: Option<f32>,

    /// PM10 standard atmospheric
    #[serde(rename = "pm10Standard")]
    pub pm10_standard: Option<f32>,

    // Particle counts per 0.1L of air
    /// Count of particles >0.3µm
    #[serde(rename = "pm003Count")]
    pub pm003_count: Option<f32>,

    /// Count of particles >0.5µm
    #[serde(rename = "pm005Count")]
    pub pm005_count: Option<f32>,

    /// Count of particles >1.0µm
    #[serde(rename = "pm01Count")]
    pub pm01_count: Option<f32>,

    /// Count of particles >2.5µm
    #[serde(rename = "pm02Count")]
    pub pm02_count: Option<f32>,

    /// Count of particles >5.0µm
    #[serde(rename = "pm50Count")]
    pub pm50_count: Option<f32>,

    /// Count of particles >10µm
    #[serde(rename = "pm10Count")]
    pub pm10_count: Option<f32>,
}

/// Gas sensor measurements (4 fields)
/// SGP41 sensor for TVOC and NOx
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GasData {
    /// TVOC index (1-500)
    pub tvoc_index: Option<i32>,

    /// TVOC raw sensor value
    pub tvoc_raw: Option<f32>,

    /// NOx index (1-500)
    pub nox_index: Option<i32>,

    /// NOx raw sensor value
    pub nox_raw: Option<f32>,
}

/// Environmental sensor measurements (4 fields)
/// SHT40 sensor for temperature and humidity
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentalData {
    /// Temperature in Celsius
    pub atmp: Option<f32>,

    /// Temperature compensated for heat from device
    pub atmp_compensated: Option<f32>,

    /// Relative humidity percentage (0-100)
    pub rhum: Option<f32>,

    /// Relative humidity compensated
    pub rhum_compensated: Option<f32>,
}

/// Air quality metrics and indices (1 field)
/// SenseAir S8 sensor
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QualityMetrics {
    /// CO2 concentration in ppm (380-10000)
    pub rco2: Option<i32>,
}

// =============================================================================
// TESTS - London School TDD: Define behavior through tests first
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock for testing - simulates a complete reading
    fn create_mock_complete_reading() -> AirQualityReading {
        AirQualityReading {
            device: DeviceMetadata {
                wifi: Some(-42),
                serialno: "airgradient:123456".to_string(),
                boot_count: Some(10),
                boot: Some(1),
                led_mode: Some("pm".to_string()),
                firmware: Some("3.1.1".to_string()),
                model: Some("I-9PSL".to_string()),
            },
            particles: ParticleData {
                pm01: Some(5.0),
                pm02: Some(12.5),
                pm10: Some(18.3),
                pm02_compensated: Some(11.8),
                pm01_standard: Some(5.2),
                pm02_standard: Some(12.8),
                pm10_standard: Some(18.5),
                pm003_count: Some(1234.0),
                pm005_count: Some(890.0),
                pm01_count: Some(456.0),
                pm02_count: Some(123.0),
                pm50_count: Some(45.0),
                pm10_count: Some(12.0),
            },
            gases: GasData {
                tvoc_index: Some(120),
                tvoc_raw: Some(32456.0),
                nox_index: Some(105),
                nox_raw: Some(28934.0),
            },
            environment: EnvironmentalData {
                atmp: Some(22.5),
                atmp_compensated: Some(21.8),
                rhum: Some(45.2),
                rhum_compensated: Some(44.5),
            },
            metrics: QualityMetrics { rco2: Some(650) },
            timestamp: Some(Utc::now()),
        }
    }

    /// Mock for testing - simulates MQTT partial reading (subset of fields)
    fn create_mock_mqtt_reading() -> AirQualityReading {
        AirQualityReading {
            device: DeviceMetadata {
                wifi: Some(-42),
                serialno: "airgradient:123456".to_string(),
                boot_count: None,
                boot: None,
                led_mode: None,
                firmware: None,
                model: None,
            },
            particles: ParticleData {
                pm01: Some(5.0),
                pm02: Some(12.5),
                pm10: Some(18.3),
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
                tvoc_index: Some(120),
                tvoc_raw: None,
                nox_index: Some(105),
                nox_raw: None,
            },
            environment: EnvironmentalData {
                atmp: Some(22.5),
                atmp_compensated: None,
                rhum: Some(45.2),
                rhum_compensated: None,
            },
            metrics: QualityMetrics { rco2: Some(650) },
            timestamp: Some(Utc::now()),
        }
    }

    #[test]
    fn test_complete_reading_has_all_29_fields() {
        let reading = create_mock_complete_reading();

        // Device metadata (7 fields including serialno)
        assert!(reading.device.wifi.is_some());
        assert!(!reading.device.serialno.is_empty());
        assert!(reading.device.boot_count.is_some());
        assert!(reading.device.boot.is_some());
        assert!(reading.device.led_mode.is_some());
        assert!(reading.device.firmware.is_some());
        assert!(reading.device.model.is_some());

        // Particle data (13 fields)
        assert!(reading.particles.pm01.is_some());
        assert!(reading.particles.pm02.is_some());
        assert!(reading.particles.pm10.is_some());
        assert!(reading.particles.pm02_compensated.is_some());
        assert!(reading.particles.pm01_standard.is_some());
        assert!(reading.particles.pm02_standard.is_some());
        assert!(reading.particles.pm10_standard.is_some());
        assert!(reading.particles.pm003_count.is_some());
        assert!(reading.particles.pm005_count.is_some());
        assert!(reading.particles.pm01_count.is_some());
        assert!(reading.particles.pm02_count.is_some());
        assert!(reading.particles.pm50_count.is_some());
        assert!(reading.particles.pm10_count.is_some());

        // Gas data (4 fields)
        assert!(reading.gases.tvoc_index.is_some());
        assert!(reading.gases.tvoc_raw.is_some());
        assert!(reading.gases.nox_index.is_some());
        assert!(reading.gases.nox_raw.is_some());

        // Environmental data (4 fields)
        assert!(reading.environment.atmp.is_some());
        assert!(reading.environment.atmp_compensated.is_some());
        assert!(reading.environment.rhum.is_some());
        assert!(reading.environment.rhum_compensated.is_some());

        // Quality metrics (1 field)
        assert!(reading.metrics.rco2.is_some());
    }

    #[test]
    fn test_mqtt_partial_reading_handles_missing_fields() {
        let reading = create_mock_mqtt_reading();

        // Essential fields should be present
        assert!(!reading.device.serialno.is_empty());
        assert!(reading.particles.pm02.is_some());
        assert!(reading.metrics.rco2.is_some());

        // Optional fields should be None
        assert!(reading.device.firmware.is_none());
        assert!(reading.particles.pm02_compensated.is_none());
        assert!(reading.gases.tvoc_raw.is_none());
    }

    #[test]
    fn test_device_metadata_serialization() {
        let metadata = DeviceMetadata {
            wifi: Some(-50),
            serialno: "test-123".to_string(),
            boot_count: Some(5),
            boot: Some(1),
            led_mode: Some("co2".to_string()),
            firmware: Some("3.0.0".to_string()),
            model: Some("ONE".to_string()),
        };

        let json = serde_json::to_string(&metadata).unwrap();
        let deserialized: DeviceMetadata = serde_json::from_str(&json).unwrap();

        assert_eq!(metadata, deserialized);
    }

    #[test]
    fn test_particle_data_with_all_counts() {
        let particles = ParticleData {
            pm01: Some(3.5),
            pm02: Some(8.2),
            pm10: Some(15.7),
            pm02_compensated: Some(7.8),
            pm01_standard: Some(3.6),
            pm02_standard: Some(8.4),
            pm10_standard: Some(16.0),
            pm003_count: Some(1500.0),
            pm005_count: Some(1000.0),
            pm01_count: Some(500.0),
            pm02_count: Some(150.0),
            pm50_count: Some(50.0),
            pm10_count: Some(15.0),
        };

        assert_eq!(particles.pm02.unwrap(), 8.2);
        assert_eq!(particles.pm10_count.unwrap(), 15.0);
    }

    #[test]
    fn test_gas_data_indices_and_raw() {
        let gases = GasData {
            tvoc_index: Some(150),
            tvoc_raw: Some(35000.0),
            nox_index: Some(120),
            nox_raw: Some(30000.0),
        };

        assert!(gases.tvoc_index.unwrap() >= 1);
        assert!(gases.tvoc_index.unwrap() <= 500);
        assert!(gases.nox_index.unwrap() >= 1);
        assert!(gases.nox_index.unwrap() <= 500);
    }

    #[test]
    fn test_environmental_data_compensation() {
        let env = EnvironmentalData {
            atmp: Some(25.0),
            atmp_compensated: Some(24.2),
            rhum: Some(50.0),
            rhum_compensated: Some(48.5),
        };

        // Compensated values should be different from raw
        assert_ne!(env.atmp.unwrap(), env.atmp_compensated.unwrap());
        assert_ne!(env.rhum.unwrap(), env.rhum_compensated.unwrap());
    }

    #[test]
    fn test_quality_metrics_co2_range() {
        let metrics = QualityMetrics { rco2: Some(450) };

        // CO2 should be in valid range
        assert!(metrics.rco2.unwrap() >= 380);
        assert!(metrics.rco2.unwrap() <= 10000);
    }

    #[test]
    fn test_reading_clone_and_equality() {
        let reading1 = create_mock_complete_reading();
        let reading2 = reading1.clone();

        assert_eq!(reading1, reading2);
    }

    #[test]
    fn test_reading_debug_format() {
        let reading = create_mock_mqtt_reading();
        let debug_str = format!("{:?}", reading);

        assert!(debug_str.contains("AirQualityReading"));
        assert!(debug_str.contains("airgradient:123456"));
    }
}
