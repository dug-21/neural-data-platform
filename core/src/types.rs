use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AirQualityReading {
    pub timestamp: DateTime<Utc>,
    pub location_id: String,

    // Device info
    pub wifi: i8,
    pub firmware: String,
    pub model: String,
    pub boot_count: u32,

    // CO2
    pub rco2: u16,

    // PM values
    pub pm01: f32,
    pub pm02: f32,
    pub pm10: f32,
    pub pm02_compensated: f32,

    // PM standard
    pub pm01_standard: f32,
    pub pm02_standard: f32,
    pub pm10_standard: f32,

    // PM counts
    pub pm003_count: f32,
    pub pm005_count: f32,
    pub pm01_count: f32,
    pub pm02_count: f32,
    pub pm50_count: f32,
    pub pm10_count: f32,

    // Temperature and humidity
    pub atmp: f32,
    pub atmp_compensated: f32,
    pub rhum: f32,
    pub rhum_compensated: f32,

    // VOC and NOx
    pub tvoc_index: u16,
    pub tvoc_raw: f32,
    pub nox_index: u16,
    pub nox_raw: f32,

    // LED mode
    pub led_mode: String,

    // Quality metrics
    pub quality_score: f32,
    pub quality_flags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenericTimeSeriesPoint {
    pub timestamp: DateTime<Utc>,
    pub location_id: String,
    pub value: f64,
    pub metadata: std::collections::HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_air_quality_reading_creation() {
        let reading = AirQualityReading {
            timestamp: Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap(),
            location_id: "sensor-001".to_string(),
            wifi: -45,
            firmware: "3.0.1".to_string(),
            model: "ONE".to_string(),
            boot_count: 5,
            rco2: 450,
            pm01: 1.2,
            pm02: 2.5,
            pm10: 5.0,
            pm02_compensated: 2.3,
            pm01_standard: 1.1,
            pm02_standard: 2.4,
            pm10_standard: 4.9,
            pm003_count: 100.0,
            pm005_count: 80.0,
            pm01_count: 50.0,
            pm02_count: 30.0,
            pm50_count: 10.0,
            pm10_count: 5.0,
            atmp: 22.5,
            atmp_compensated: 22.3,
            rhum: 45.0,
            rhum_compensated: 44.5,
            tvoc_index: 100,
            tvoc_raw: 150.5,
            nox_index: 1,
            nox_raw: 10.2,
            led_mode: "co2".to_string(),
            quality_score: 0.95,
            quality_flags: vec!["VALID".to_string()],
        };

        assert_eq!(reading.location_id, "sensor-001");
        assert_eq!(reading.pm02, 2.5);
        assert_eq!(reading.quality_score, 0.95);
    }

    #[test]
    fn test_generic_time_series_point_creation() {
        let point = GenericTimeSeriesPoint {
            timestamp: Utc::now(),
            location_id: "test-location".to_string(),
            value: 42.0,
            metadata: std::collections::HashMap::new(),
        };

        assert_eq!(point.location_id, "test-location");
        assert_eq!(point.value, 42.0);
    }
}
