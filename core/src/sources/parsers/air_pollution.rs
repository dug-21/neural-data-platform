//! OpenWeatherMap Air Pollution API parser
//!
//! Parses responses from the OpenWeatherMap air pollution endpoint:
//! https://api.openweathermap.org/data/2.5/air_pollution

use crate::error::{CoreError, CoreResult};
use crate::traits::TimeSeriesPoint;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::HashMap;

// Import ResponseParser trait directly - it's defined in http_poll module
use super::super::http_poll::ResponseParser;

/// OpenWeatherMap air pollution response structure
#[derive(Debug, Clone, Deserialize)]
struct AirPollutionResponse {
    list: Vec<AirPollutionData>,
}

#[derive(Debug, Clone, Deserialize)]
struct AirPollutionData {
    main: AirQualityIndex,
    components: PollutionComponents,
}

#[derive(Debug, Clone, Deserialize)]
struct AirQualityIndex {
    aqi: u8, // 1-5 scale (1=Good, 2=Fair, 3=Moderate, 4=Poor, 5=Very Poor)
}

#[derive(Debug, Clone, Deserialize)]
struct PollutionComponents {
    co: f64,    // Carbon monoxide (μg/m³)
    no: f64,    // Nitrogen monoxide (μg/m³)
    no2: f64,   // Nitrogen dioxide (μg/m³)
    o3: f64,    // Ozone (μg/m³)
    so2: f64,   // Sulphur dioxide (μg/m³)
    pm2_5: f64, // Fine particles (μg/m³)
    pm10: f64,  // Coarse particulate matter (μg/m³)
    nh3: f64,   // Ammonia (μg/m³)
}

/// Parser for OpenWeatherMap air pollution API responses
pub struct AirPollutionParser;

impl AirPollutionParser {
    /// Create a new air pollution parser
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
    ) -> TimeSeriesPoint {
        let mut tags = HashMap::new();
        tags.insert("metric".to_string(), metric.to_string());
        tags.insert("source".to_string(), "openweathermap".to_string());
        tags.insert("api".to_string(), "air_pollution".to_string());
        tags.insert("unit".to_string(), unit.to_string());

        TimeSeriesPoint {
            timestamp,
            location_id: location_id.to_string(),
            value,
            tags,
        }
    }
}

impl Default for AirPollutionParser {
    fn default() -> Self {
        Self::new()
    }
}

impl ResponseParser for AirPollutionParser {
    fn parse(
        &self,
        response_body: &str,
        location_id: &str,
        timestamp: DateTime<Utc>,
    ) -> CoreResult<Vec<TimeSeriesPoint>> {
        let pollution: AirPollutionResponse = serde_json::from_str(response_body).map_err(|e| {
            CoreError::Source(format!("Failed to parse air pollution response: {}", e))
        })?;

        if pollution.list.is_empty() {
            return Err(CoreError::Source(
                "Air pollution response contains no data".to_string(),
            ));
        }

        // Use the first (most recent) data point
        let data = &pollution.list[0];
        let mut points = Vec::new();

        // Air Quality Index
        points.push(Self::create_point(
            location_id,
            "aqi",
            data.main.aqi as f64,
            "1-5_scale",
            timestamp,
        ));

        // Carbon monoxide
        points.push(Self::create_point(
            location_id,
            "co",
            data.components.co,
            "μg/m³",
            timestamp,
        ));

        // Nitrogen monoxide
        points.push(Self::create_point(
            location_id,
            "no",
            data.components.no,
            "μg/m³",
            timestamp,
        ));

        // Nitrogen dioxide
        points.push(Self::create_point(
            location_id,
            "no2",
            data.components.no2,
            "μg/m³",
            timestamp,
        ));

        // Ozone
        points.push(Self::create_point(
            location_id,
            "o3",
            data.components.o3,
            "μg/m³",
            timestamp,
        ));

        // Sulphur dioxide
        points.push(Self::create_point(
            location_id,
            "so2",
            data.components.so2,
            "μg/m³",
            timestamp,
        ));

        // PM2.5
        points.push(Self::create_point(
            location_id,
            "pm2_5",
            data.components.pm2_5,
            "μg/m³",
            timestamp,
        ));

        // PM10
        points.push(Self::create_point(
            location_id,
            "pm10",
            data.components.pm10,
            "μg/m³",
            timestamp,
        ));

        // Ammonia
        points.push(Self::create_point(
            location_id,
            "nh3",
            data.components.nh3,
            "μg/m³",
            timestamp,
        ));

        Ok(points)
    }

    fn name(&self) -> &'static str {
        "openweathermap_air_pollution"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_air_pollution_parser_creation() {
        let parser = AirPollutionParser::new();
        assert_eq!(std::mem::size_of_val(&parser), 0); // Zero-sized type
    }

    #[test]
    fn test_air_pollution_parser_default() {
        let parser = AirPollutionParser::default();
        assert_eq!(std::mem::size_of_val(&parser), 0);
    }

    #[test]
    fn test_parse_full_air_pollution_response() {
        let parser = AirPollutionParser::new();
        let json = r#"{
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
                }
            }]
        }"#;

        let timestamp = Utc::now();
        let points = parser.parse(json, "test-location", timestamp).unwrap();

        // Should have 9 points: aqi, co, no, no2, o3, so2, pm2_5, pm10, nh3
        assert_eq!(points.len(), 9);

        // Verify AQI
        let aqi = points
            .iter()
            .find(|p| p.tags.get("metric") == Some(&"aqi".to_string()))
            .unwrap();
        assert_eq!(aqi.value, 2.0);
        assert_eq!(aqi.location_id, "test-location");
        assert_eq!(aqi.tags.get("unit"), Some(&"1-5_scale".to_string()));
        assert_eq!(aqi.tags.get("source"), Some(&"openweathermap".to_string()));
    }

    #[test]
    fn test_parse_empty_list() {
        let parser = AirPollutionParser::new();
        let json = r#"{ "list": [] }"#;

        let timestamp = Utc::now();
        let result = parser.parse(json, "test-location", timestamp);
        assert!(result.is_err());

        match result {
            Err(CoreError::Source(msg)) => {
                assert!(msg.contains("no data"));
            }
            _ => panic!("Expected CoreError::Source"),
        }
    }

    #[test]
    fn test_parse_invalid_json() {
        let parser = AirPollutionParser::new();
        let json = r#"{ "invalid": "json" }"#;

        let timestamp = Utc::now();
        let result = parser.parse(json, "test-location", timestamp);
        assert!(result.is_err());

        match result {
            Err(CoreError::Source(msg)) => {
                assert!(msg.contains("Failed to parse air pollution response"));
            }
            _ => panic!("Expected CoreError::Source"),
        }
    }

    #[test]
    fn test_all_points_have_correct_source_tags() {
        let parser = AirPollutionParser::new();
        let json = r#"{
            "list": [{
                "main": { "aqi": 2 },
                "components": {
                    "co": 230.31,
                    "no": 0.51,
                    "no2": 15.34,
                    "o3": 68.66,
                    "so2": 3.73,
                    "pm2_5": 8.59,
                    "pm10": 12.15,
                    "nh3": 0.92
                }
            }]
        }"#;

        let timestamp = Utc::now();
        let points = parser.parse(json, "test-location", timestamp).unwrap();

        for point in points {
            assert_eq!(
                point.tags.get("source"),
                Some(&"openweathermap".to_string())
            );
            assert_eq!(point.tags.get("api"), Some(&"air_pollution".to_string()));
            assert!(point.tags.contains_key("unit"));
            assert!(point.tags.contains_key("metric"));
        }
    }
}
