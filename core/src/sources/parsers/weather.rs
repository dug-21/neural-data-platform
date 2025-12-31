//! OpenWeatherMap Current Weather API parser
//!
//! Parses responses from the OpenWeatherMap current weather endpoint:
//! https://api.openweathermap.org/data/2.5/weather

use crate::error::{CoreError, CoreResult};
use crate::traits::TimeSeriesPoint;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::HashMap;

// Import ResponseParser trait directly  - it's defined in http_poll module
use super::super::http_poll::ResponseParser;

/// OpenWeatherMap current weather response structure
#[derive(Debug, Clone, Deserialize)]
struct WeatherResponse {
    main: MainWeather,
    wind: Wind,
    clouds: Clouds,
    visibility: Option<f64>,
    rain: Option<Rain>,
    snow: Option<Snow>,
}

#[derive(Debug, Clone, Deserialize)]
struct MainWeather {
    temp: f64,
    feels_like: f64,
    pressure: f64,
    humidity: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct Wind {
    speed: f64,
    deg: f64,
    gust: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
struct Clouds {
    all: f64,
}

#[derive(Debug, Clone, Deserialize)]
struct Rain {
    #[serde(rename = "1h")]
    one_hour: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
struct Snow {
    #[serde(rename = "1h")]
    one_hour: Option<f64>,
}

/// Parser for OpenWeatherMap current weather API responses
pub struct WeatherParser;

impl WeatherParser {
    /// Create a new weather parser
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
        tags.insert("api".to_string(), "current_weather".to_string());
        tags.insert("unit".to_string(), unit.to_string());

        TimeSeriesPoint {
            timestamp,
            location_id: location_id.to_string(),
            value,
            tags,
            ndp_id: None,
            context: None,
        }
    }
}

impl Default for WeatherParser {
    fn default() -> Self {
        Self::new()
    }
}

impl ResponseParser for WeatherParser {
    fn parse(
        &self,
        response_body: &str,
        location_id: &str,
        timestamp: DateTime<Utc>,
    ) -> CoreResult<Vec<TimeSeriesPoint>> {
        let weather: WeatherResponse = serde_json::from_str(response_body)
            .map_err(|e| CoreError::Source(format!("Failed to parse weather response: {}", e)))?;

        let mut points = Vec::new();

        // Temperature metrics
        points.push(Self::create_point(
            location_id,
            "temperature",
            weather.main.temp,
            "celsius",
            timestamp,
        ));

        points.push(Self::create_point(
            location_id,
            "feels_like",
            weather.main.feels_like,
            "celsius",
            timestamp,
        ));

        // Atmospheric metrics
        points.push(Self::create_point(
            location_id,
            "pressure",
            weather.main.pressure,
            "hpa",
            timestamp,
        ));

        points.push(Self::create_point(
            location_id,
            "humidity",
            weather.main.humidity,
            "percent",
            timestamp,
        ));

        // Wind metrics
        points.push(Self::create_point(
            location_id,
            "wind_speed",
            weather.wind.speed,
            "m/s",
            timestamp,
        ));

        points.push(Self::create_point(
            location_id,
            "wind_deg",
            weather.wind.deg,
            "degrees",
            timestamp,
        ));

        if let Some(gust) = weather.wind.gust {
            points.push(Self::create_point(
                location_id,
                "wind_gust",
                gust,
                "m/s",
                timestamp,
            ));
        }

        // Cloud coverage
        points.push(Self::create_point(
            location_id,
            "clouds",
            weather.clouds.all,
            "percent",
            timestamp,
        ));

        // Visibility
        if let Some(visibility) = weather.visibility {
            points.push(Self::create_point(
                location_id,
                "visibility",
                visibility,
                "meters",
                timestamp,
            ));
        }

        // Precipitation
        if let Some(rain) = weather.rain {
            if let Some(rain_1h) = rain.one_hour {
                points.push(Self::create_point(
                    location_id,
                    "rain_1h",
                    rain_1h,
                    "mm",
                    timestamp,
                ));
            }
        }

        if let Some(snow) = weather.snow {
            if let Some(snow_1h) = snow.one_hour {
                points.push(Self::create_point(
                    location_id,
                    "snow_1h",
                    snow_1h,
                    "mm",
                    timestamp,
                ));
            }
        }

        Ok(points)
    }

    fn name(&self) -> &'static str {
        "openweathermap_current_weather"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weather_parser_creation() {
        let parser = WeatherParser::new();
        assert_eq!(std::mem::size_of_val(&parser), 0); // Zero-sized type
    }

    #[test]
    fn test_weather_parser_default() {
        let parser = WeatherParser::default();
        assert_eq!(std::mem::size_of_val(&parser), 0);
    }

    #[test]
    fn test_parse_full_weather_response() {
        let parser = WeatherParser::new();
        let json = r#"{
            "main": {
                "temp": 20.5,
                "feels_like": 19.8,
                "pressure": 1015.0,
                "humidity": 65.0
            },
            "wind": {
                "speed": 3.5,
                "deg": 180.0,
                "gust": 5.2
            },
            "clouds": {
                "all": 40.0
            },
            "visibility": 10000.0,
            "rain": {
                "1h": 0.5
            },
            "snow": {
                "1h": 0.0
            }
        }"#;

        let timestamp = Utc::now();
        let points = parser.parse(json, "test-location", timestamp).unwrap();

        // Should have: temp, feels_like, pressure, humidity, wind_speed, wind_deg,
        // wind_gust, clouds, visibility, rain_1h, snow_1h = 11 points
        assert_eq!(points.len(), 11);

        // Verify temperature point
        let temp_point = points
            .iter()
            .find(|p| p.tags.get("metric") == Some(&"temperature".to_string()))
            .unwrap();
        assert_eq!(temp_point.value, 20.5);
        assert_eq!(temp_point.location_id, "test-location");
        assert_eq!(temp_point.tags.get("unit"), Some(&"celsius".to_string()));
        assert_eq!(
            temp_point.tags.get("source"),
            Some(&"openweathermap".to_string())
        );
    }

    #[test]
    fn test_parse_weather_without_optional_fields() {
        let parser = WeatherParser::new();
        let json = r#"{
            "main": {
                "temp": 20.5,
                "feels_like": 19.8,
                "pressure": 1015.0,
                "humidity": 65.0
            },
            "wind": {
                "speed": 3.5,
                "deg": 180.0
            },
            "clouds": {
                "all": 40.0
            }
        }"#;

        let timestamp = Utc::now();
        let points = parser.parse(json, "test-location", timestamp).unwrap();

        // Should have only required fields: temp, feels_like, pressure, humidity,
        // wind_speed, wind_deg, clouds = 7 points
        assert_eq!(points.len(), 7);
    }

    #[test]
    fn test_parse_invalid_json() {
        let parser = WeatherParser::new();
        let json = r#"{ "invalid": "json" }"#;

        let timestamp = Utc::now();
        let result = parser.parse(json, "test-location", timestamp);
        assert!(result.is_err());

        match result {
            Err(CoreError::Source(msg)) => {
                assert!(msg.contains("Failed to parse weather response"));
            }
            _ => panic!("Expected CoreError::Source"),
        }
    }
}
