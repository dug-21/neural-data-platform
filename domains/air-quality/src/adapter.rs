//! Adapter for converting AirQualityReading to TimeSeriesPoint
//!
//! Implements the London School approach by defining the adapter through
//! its interactions with the core::traits::TimeSeriesPoint type.

use crate::types::AirQualityReading;
use chrono::Utc;
use platform_core::traits::TimeSeriesPoint;
use std::collections::HashMap;

/// Adapter for converting air quality readings to time series points
pub struct AirQualityAdapter;

impl AirQualityAdapter {
    /// Convert an AirQualityReading to a collection of TimeSeriesPoints
    ///
    /// Each numeric field becomes a separate time series point with:
    /// - timestamp: from reading or current time
    /// - location_id: device serial number
    /// - value: numeric value as f64
    /// - tags: metadata including metric name, firmware, model
    pub fn to_time_series_points(reading: &AirQualityReading) -> Vec<TimeSeriesPoint> {
        let mut points = Vec::new();

        let timestamp = reading.timestamp.unwrap_or_else(Utc::now);
        let location_id = reading.device.serialno.clone();

        // Build common tags
        let mut base_tags = HashMap::new();
        if let Some(fw) = &reading.device.firmware {
            base_tags.insert("firmware".to_string(), fw.clone());
        }
        if let Some(model) = &reading.device.model {
            base_tags.insert("model".to_string(), model.clone());
        }
        if let Some(led) = &reading.device.led_mode {
            base_tags.insert("led_mode".to_string(), led.clone());
        }

        // Helper to create tags with metric name
        let make_tags = |metric: &str| -> HashMap<String, String> {
            let mut tags = base_tags.clone();
            tags.insert("metric".to_string(), metric.to_string());
            tags
        };

        // CO2
        if let Some(co2) = reading.metrics.rco2 {
            points.push(TimeSeriesPoint {
                timestamp,
                location_id: location_id.clone(),
                value: co2 as f64,
                tags: make_tags("co2"),
            });
        }

        // PM values
        if let Some(pm01) = reading.particles.pm01 {
            points.push(TimeSeriesPoint {
                timestamp,
                location_id: location_id.clone(),
                value: pm01 as f64,
                tags: make_tags("pm1"),
            });
        }
        if let Some(pm02) = reading.particles.pm02 {
            points.push(TimeSeriesPoint {
                timestamp,
                location_id: location_id.clone(),
                value: pm02 as f64,
                tags: make_tags("pm25"),
            });
        }
        if let Some(pm10) = reading.particles.pm10 {
            points.push(TimeSeriesPoint {
                timestamp,
                location_id: location_id.clone(),
                value: pm10 as f64,
                tags: make_tags("pm10"),
            });
        }

        // PM compensated
        if let Some(pm02_comp) = reading.particles.pm02_compensated {
            points.push(TimeSeriesPoint {
                timestamp,
                location_id: location_id.clone(),
                value: pm02_comp as f64,
                tags: make_tags("pm25_compensated"),
            });
        }

        // Temperature
        if let Some(temp) = reading.environment.atmp {
            points.push(TimeSeriesPoint {
                timestamp,
                location_id: location_id.clone(),
                value: temp as f64,
                tags: make_tags("temperature"),
            });
        }
        if let Some(temp_comp) = reading.environment.atmp_compensated {
            points.push(TimeSeriesPoint {
                timestamp,
                location_id: location_id.clone(),
                value: temp_comp as f64,
                tags: make_tags("temperature_compensated"),
            });
        }

        // Humidity
        if let Some(hum) = reading.environment.rhum {
            points.push(TimeSeriesPoint {
                timestamp,
                location_id: location_id.clone(),
                value: hum as f64,
                tags: make_tags("humidity"),
            });
        }
        if let Some(hum_comp) = reading.environment.rhum_compensated {
            points.push(TimeSeriesPoint {
                timestamp,
                location_id: location_id.clone(),
                value: hum_comp as f64,
                tags: make_tags("humidity_compensated"),
            });
        }

        // TVOC
        if let Some(tvoc_idx) = reading.gases.tvoc_index {
            points.push(TimeSeriesPoint {
                timestamp,
                location_id: location_id.clone(),
                value: tvoc_idx as f64,
                tags: make_tags("tvoc_index"),
            });
        }
        if let Some(tvoc_raw) = reading.gases.tvoc_raw {
            points.push(TimeSeriesPoint {
                timestamp,
                location_id: location_id.clone(),
                value: tvoc_raw as f64,
                tags: make_tags("tvoc_raw"),
            });
        }

        // NOx
        if let Some(nox_idx) = reading.gases.nox_index {
            points.push(TimeSeriesPoint {
                timestamp,
                location_id: location_id.clone(),
                value: nox_idx as f64,
                tags: make_tags("nox_index"),
            });
        }
        if let Some(nox_raw) = reading.gases.nox_raw {
            points.push(TimeSeriesPoint {
                timestamp,
                location_id: location_id.clone(),
                value: nox_raw as f64,
                tags: make_tags("nox_raw"),
            });
        }

        // WiFi signal strength
        if let Some(wifi) = reading.device.wifi {
            points.push(TimeSeriesPoint {
                timestamp,
                location_id: location_id.clone(),
                value: wifi as f64,
                tags: make_tags("wifi_signal"),
            });
        }

        // Particle counts
        if let Some(count) = reading.particles.pm003_count {
            points.push(TimeSeriesPoint {
                timestamp,
                location_id: location_id.clone(),
                value: count as f64,
                tags: make_tags("pm003_count"),
            });
        }
        if let Some(count) = reading.particles.pm01_count {
            points.push(TimeSeriesPoint {
                timestamp,
                location_id: location_id.clone(),
                value: count as f64,
                tags: make_tags("pm01_count"),
            });
        }
        if let Some(count) = reading.particles.pm02_count {
            points.push(TimeSeriesPoint {
                timestamp,
                location_id: location_id.clone(),
                value: count as f64,
                tags: make_tags("pm02_count"),
            });
        }

        points
    }

    /// Extract a specific metric from a reading
    pub fn extract_metric(
        reading: &AirQualityReading,
        metric_name: &str,
    ) -> Option<TimeSeriesPoint> {
        let points = Self::to_time_series_points(reading);
        points
            .into_iter()
            .find(|p| p.tags.get("metric") == Some(&metric_name.to_string()))
    }

    /// Get all available metric names from a reading
    pub fn available_metrics(reading: &AirQualityReading) -> Vec<String> {
        Self::to_time_series_points(reading)
            .into_iter()
            .filter_map(|p| p.tags.get("metric").cloned())
            .collect()
    }
}

// =============================================================================
// TESTS - London School TDD: Test interactions and contracts
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;
    use chrono::TimeZone;

    /// Mock reading for testing adapter behavior
    fn create_test_reading() -> AirQualityReading {
        AirQualityReading {
            device: DeviceMetadata {
                wifi: Some(-50),
                serialno: "airgradient:test-123".to_string(),
                boot_count: Some(5),
                boot: Some(1),
                led_mode: Some("co2".to_string()),
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
                pm005_count: None,
                pm01_count: Some(456.0),
                pm02_count: Some(123.0),
                pm50_count: None,
                pm10_count: None,
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
            timestamp: Some(Utc.with_ymd_and_hms(2024, 1, 15, 10, 30, 0).unwrap()),
        }
    }

    /// Mock minimal reading
    fn create_minimal_reading() -> AirQualityReading {
        AirQualityReading {
            device: DeviceMetadata {
                wifi: None,
                serialno: "minimal-device".to_string(),
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
        }
    }

    // =========================================================================
    // Adapter Conversion Tests
    // =========================================================================

    #[test]
    fn test_adapter_converts_reading_to_multiple_points() {
        let reading = create_test_reading();
        let points = AirQualityAdapter::to_time_series_points(&reading);

        // Should create one point per available metric
        assert!(!points.is_empty());
        assert!(points.len() >= 10); // At minimum: co2, pm1, pm25, pm10, temp, hum, tvoc, nox, wifi
    }

    #[test]
    fn test_adapter_preserves_location_id() {
        let reading = create_test_reading();
        let points = AirQualityAdapter::to_time_series_points(&reading);

        // All points should have same location_id
        for point in points {
            assert_eq!(point.location_id, "airgradient:test-123");
        }
    }

    #[test]
    fn test_adapter_preserves_timestamp() {
        let reading = create_test_reading();
        let expected_time = reading.timestamp.unwrap();
        let points = AirQualityAdapter::to_time_series_points(&reading);

        // All points should have same timestamp
        for point in points {
            assert_eq!(point.timestamp, expected_time);
        }
    }

    #[test]
    fn test_adapter_includes_tags() {
        let reading = create_test_reading();
        let points = AirQualityAdapter::to_time_series_points(&reading);

        // Check first point has tags
        let point = &points[0];
        assert!(point.tags.contains_key("firmware"));
        assert_eq!(point.tags.get("firmware").unwrap(), "3.1.1");
        assert!(point.tags.contains_key("model"));
        assert_eq!(point.tags.get("model").unwrap(), "I-9PSL");
        assert!(point.tags.contains_key("led_mode"));
        assert_eq!(point.tags.get("led_mode").unwrap(), "co2");
        assert!(point.tags.contains_key("metric"));
    }

    #[test]
    fn test_adapter_creates_co2_point() {
        let reading = create_test_reading();
        let points = AirQualityAdapter::to_time_series_points(&reading);

        let co2_point = points
            .iter()
            .find(|p| p.tags.get("metric") == Some(&"co2".to_string()))
            .unwrap();
        assert_eq!(co2_point.value, 650.0);
        assert_eq!(co2_point.location_id, "airgradient:test-123");
    }

    #[test]
    fn test_adapter_creates_pm_points() {
        let reading = create_test_reading();
        let points = AirQualityAdapter::to_time_series_points(&reading);

        let pm25 = points
            .iter()
            .find(|p| p.tags.get("metric") == Some(&"pm25".to_string()))
            .unwrap();
        assert_eq!(pm25.value, 12.5);

        let pm1 = points
            .iter()
            .find(|p| p.tags.get("metric") == Some(&"pm1".to_string()))
            .unwrap();
        assert_eq!(pm1.value, 5.0);

        let pm10 = points
            .iter()
            .find(|p| p.tags.get("metric") == Some(&"pm10".to_string()))
            .unwrap();
        assert!((pm10.value - 18.3).abs() < 0.01);
    }

    #[test]
    fn test_adapter_creates_environmental_points() {
        let reading = create_test_reading();
        let points = AirQualityAdapter::to_time_series_points(&reading);

        let temp = points
            .iter()
            .find(|p| p.tags.get("metric") == Some(&"temperature".to_string()))
            .unwrap();
        assert_eq!(temp.value, 22.5);

        let temp_comp = points
            .iter()
            .find(|p| p.tags.get("metric") == Some(&"temperature_compensated".to_string()))
            .unwrap();
        assert!((temp_comp.value - 21.8).abs() < 0.01);

        let hum = points
            .iter()
            .find(|p| p.tags.get("metric") == Some(&"humidity".to_string()))
            .unwrap();
        assert!((hum.value - 45.2).abs() < 0.01);
    }

    #[test]
    fn test_adapter_creates_gas_points() {
        let reading = create_test_reading();
        let points = AirQualityAdapter::to_time_series_points(&reading);

        let tvoc_idx = points
            .iter()
            .find(|p| p.tags.get("metric") == Some(&"tvoc_index".to_string()))
            .unwrap();
        assert_eq!(tvoc_idx.value, 120.0);

        let tvoc_raw = points
            .iter()
            .find(|p| p.tags.get("metric") == Some(&"tvoc_raw".to_string()))
            .unwrap();
        assert_eq!(tvoc_raw.value, 32456.0);

        let nox_idx = points
            .iter()
            .find(|p| p.tags.get("metric") == Some(&"nox_index".to_string()))
            .unwrap();
        assert_eq!(nox_idx.value, 105.0);
    }

    #[test]
    fn test_adapter_handles_minimal_reading() {
        let reading = create_minimal_reading();
        let points = AirQualityAdapter::to_time_series_points(&reading);

        // Should have only 2 points: pm25 and co2
        assert_eq!(points.len(), 2);

        let metrics = AirQualityAdapter::available_metrics(&reading);
        assert!(metrics.contains(&"pm25".to_string()));
        assert!(metrics.contains(&"co2".to_string()));
    }

    #[test]
    fn test_adapter_handles_missing_timestamp() {
        let mut reading = create_test_reading();
        reading.timestamp = None;

        let points = AirQualityAdapter::to_time_series_points(&reading);

        // Should use current time
        assert!(!points.is_empty());
        for point in points {
            assert!(point.timestamp <= Utc::now());
        }
    }

    // =========================================================================
    // Extract Metric Tests
    // =========================================================================

    #[test]
    fn test_extract_specific_metric() {
        let reading = create_test_reading();
        let pm25 = AirQualityAdapter::extract_metric(&reading, "pm25");

        assert!(pm25.is_some());
        let point = pm25.unwrap();
        assert_eq!(point.tags.get("metric").unwrap(), "pm25");
        assert_eq!(point.value, 12.5);
    }

    #[test]
    fn test_extract_nonexistent_metric() {
        let reading = create_minimal_reading();
        let result = AirQualityAdapter::extract_metric(&reading, "pm1");

        assert!(result.is_none());
    }

    // =========================================================================
    // Available Metrics Tests
    // =========================================================================

    #[test]
    fn test_available_metrics_lists_all_present_metrics() {
        let reading = create_test_reading();
        let metrics = AirQualityAdapter::available_metrics(&reading);

        assert!(metrics.contains(&"co2".to_string()));
        assert!(metrics.contains(&"pm25".to_string()));
        assert!(metrics.contains(&"temperature".to_string()));
        assert!(metrics.contains(&"humidity".to_string()));
        assert!(metrics.contains(&"tvoc_index".to_string()));
    }

    #[test]
    fn test_available_metrics_minimal_reading() {
        let reading = create_minimal_reading();
        let metrics = AirQualityAdapter::available_metrics(&reading);

        assert_eq!(metrics.len(), 2);
        assert!(metrics.contains(&"pm25".to_string()));
        assert!(metrics.contains(&"co2".to_string()));
    }

    #[test]
    fn test_available_metrics_does_not_include_none_values() {
        let reading = create_minimal_reading();
        let metrics = AirQualityAdapter::available_metrics(&reading);

        // These should not be present
        assert!(!metrics.contains(&"pm1".to_string()));
        assert!(!metrics.contains(&"tvoc_index".to_string()));
        assert!(!metrics.contains(&"temperature".to_string()));
    }

    // =========================================================================
    // Contract Verification Tests (London School)
    // =========================================================================

    #[test]
    fn test_adapter_contract_all_points_have_required_fields() {
        let reading = create_test_reading();
        let points = AirQualityAdapter::to_time_series_points(&reading);

        for point in points {
            // Verify contract: every point must have these fields
            assert!(!point.location_id.is_empty());
            assert!(point.value.is_finite()); // Valid number
            assert!(point.tags.contains_key("metric"));
        }
    }

    #[test]
    fn test_adapter_contract_consistent_location_across_points() {
        let reading = create_test_reading();
        let points = AirQualityAdapter::to_time_series_points(&reading);

        let expected_location = "airgradient:test-123";
        for point in points {
            assert_eq!(point.location_id, expected_location);
        }
    }

    #[test]
    fn test_adapter_contract_unique_metrics() {
        let reading = create_test_reading();
        let points = AirQualityAdapter::to_time_series_points(&reading);

        let mut seen_metrics = std::collections::HashSet::new();
        for point in &points {
            let metric = point.tags.get("metric").unwrap();
            assert!(
                seen_metrics.insert(metric.clone()),
                "Duplicate metric: {}",
                metric
            );
        }
    }
}
