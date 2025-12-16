# Generic HTTP Polling Source - Usage Examples

This document demonstrates how to use the refactored `HttpPollingSource` with various data sources.

---

## Table of Contents

1. [Quick Start - AirGradient (Backward Compatible)](#quick-start---airgradient-backward-compatible)
2. [OpenWeatherMap API Integration](#openweathermap-api-integration)
3. [Custom IoT Device](#custom-iot-device)
4. [Multiple Endpoints](#multiple-endpoints)
5. [Advanced Configuration](#advanced-configuration)

---

## Quick Start - AirGradient (Backward Compatible)

The easiest way to get started - existing code works without changes:

```rust
use platform_core::sources::http_poll::{HttpPollingConfig, HttpPollingSource, SensorConfig};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Old API still works
    let config = HttpPollingConfig {
        sensors: vec![
            SensorConfig {
                serial_number: "001".to_string(),
                url: "http://airgradient_001.local/measures/current".to_string(),
            },
        ],
        poll_interval: Duration::from_secs(60),
        ..Default::default()
    };

    let mut source = HttpPollingSource::new_with_default_parser(config)?;
    source.start().await?;

    // Fetch data
    let points = source.fetch().await?;
    println!("Received {} data points", points.len());

    Ok(())
}
```

---

## OpenWeatherMap API Integration

### Step 1: Create a custom parser

```rust
use platform_core::{
    sources::http_poll::ResponseParser,
    error::{CoreError, CoreResult},
    traits::TimeSeriesPoint,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
struct OpenWeatherResponse {
    main: MainWeather,
    weather: Vec<WeatherDescription>,
    wind: Wind,
}

#[derive(Deserialize)]
struct MainWeather {
    temp: f64,
    humidity: f64,
    pressure: f64,
}

#[derive(Deserialize)]
struct WeatherDescription {
    description: String,
}

#[derive(Deserialize)]
struct Wind {
    speed: f64,
}

pub struct OpenWeatherParser;

impl ResponseParser for OpenWeatherParser {
    fn parse(
        &self,
        response_body: &str,
        location_id: &str,
        timestamp: DateTime<Utc>,
    ) -> CoreResult<Vec<TimeSeriesPoint>> {
        let data: OpenWeatherResponse = serde_json::from_str(response_body)
            .map_err(|e| CoreError::Source(format!("Failed to parse OpenWeather response: {}", e)))?;

        let mut points = Vec::new();

        // Temperature
        let mut tags = HashMap::new();
        tags.insert("metric".to_string(), "temperature".to_string());
        tags.insert("source".to_string(), "openweather".to_string());
        tags.insert("unit".to_string(), "celsius".to_string());

        points.push(TimeSeriesPoint {
            timestamp,
            location_id: location_id.to_string(),
            value: data.main.temp,
            tags,
        });

        // Humidity
        let mut tags = HashMap::new();
        tags.insert("metric".to_string(), "humidity".to_string());
        tags.insert("source".to_string(), "openweather".to_string());
        tags.insert("unit".to_string(), "percent".to_string());

        points.push(TimeSeriesPoint {
            timestamp,
            location_id: location_id.to_string(),
            value: data.main.humidity,
            tags,
        });

        // Pressure
        let mut tags = HashMap::new();
        tags.insert("metric".to_string(), "pressure".to_string());
        tags.insert("source".to_string(), "openweather".to_string());
        tags.insert("unit".to_string(), "hPa".to_string());

        points.push(TimeSeriesPoint {
            timestamp,
            location_id: location_id.to_string(),
            value: data.main.pressure,
            tags,
        });

        // Wind speed
        let mut tags = HashMap::new();
        tags.insert("metric".to_string(), "wind_speed".to_string());
        tags.insert("source".to_string(), "openweather".to_string());
        tags.insert("unit".to_string(), "m/s".to_string());

        points.push(TimeSeriesPoint {
            timestamp,
            location_id: location_id.to_string(),
            value: data.wind.speed,
            tags,
        });

        Ok(points)
    }

    fn name(&self) -> &'static str {
        "openweather"
    }
}
```

### Step 2: Configure and use

```rust
use platform_core::sources::http_poll::{
    HttpPollingConfig, HttpPollingSource, EndpointConfig,
    AuthMethod, ParserRegistry, RetryConfig
};
use std::time::Duration;
use std::sync::Arc;
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Set environment variable
    std::env::set_var("OPENWEATHER_API_KEY", "your_api_key_here");

    // Configure endpoint
    let config = HttpPollingConfig {
        endpoints: vec![
            EndpointConfig {
                id: "weather_boston".to_string(),
                url: "https://api.openweathermap.org/data/2.5/weather".to_string(),
                auth: AuthMethod::QueryParam {
                    param_name: "appid".to_string(),
                    value_env: "OPENWEATHER_API_KEY".to_string(),
                },
                parser_type: "openweather".to_string(),
                enabled: true,
                query_params: Some(HashMap::from([
                    ("q".to_string(), "Boston,US".to_string()),
                    ("units".to_string(), "metric".to_string()),
                ])),
            },
        ],
        poll_interval: Duration::from_secs(300), // 5 minutes
        timeout: Duration::from_secs(10),
        retry: RetryConfig {
            max_retries: 3,
            initial_delay_ms: 100,
            max_delay_ms: 30000,
            backoff_multiplier: 2.0,
            jitter: true,
        },
        ..Default::default()
    };

    // Register parser
    let mut registry = ParserRegistry::new();
    registry.register("openweather".to_string(), Arc::new(OpenWeatherParser));

    // Create and start source
    let mut source = HttpPollingSource::new(config, registry)?;
    source.start().await?;

    // Fetch data periodically
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;

        let points = source.fetch().await?;
        println!("Received {} weather data points", points.len());

        for point in points {
            println!("  {} @ {}: {}",
                point.tags.get("metric").unwrap(),
                point.location_id,
                point.value
            );
        }
    }
}
```

---

## Custom IoT Device

Example for a custom JSON API endpoint:

```rust
use platform_core::{
    sources::http_poll::ResponseParser,
    error::{CoreError, CoreResult},
    traits::TimeSeriesPoint,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::HashMap;

// Custom device response format
#[derive(Deserialize)]
struct CustomDeviceResponse {
    device_id: String,
    measurements: Vec<Measurement>,
}

#[derive(Deserialize)]
struct Measurement {
    sensor: String,
    value: f64,
    unit: String,
}

pub struct CustomDeviceParser;

impl ResponseParser for CustomDeviceParser {
    fn parse(
        &self,
        response_body: &str,
        location_id: &str,
        timestamp: DateTime<Utc>,
    ) -> CoreResult<Vec<TimeSeriesPoint>> {
        let data: CustomDeviceResponse = serde_json::from_str(response_body)
            .map_err(|e| CoreError::Source(format!("Failed to parse device response: {}", e)))?;

        let mut points = Vec::new();

        for measurement in data.measurements {
            let mut tags = HashMap::new();
            tags.insert("metric".to_string(), measurement.sensor.clone());
            tags.insert("unit".to_string(), measurement.unit.clone());
            tags.insert("device_id".to_string(), data.device_id.clone());
            tags.insert("source".to_string(), "custom_iot".to_string());

            points.push(TimeSeriesPoint {
                timestamp,
                location_id: location_id.to_string(),
                value: measurement.value,
                tags,
            });
        }

        Ok(points)
    }

    fn name(&self) -> &'static str {
        "custom_iot"
    }
}
```

### Configuration with Basic Auth

```rust
let config = HttpPollingConfig {
    endpoints: vec![
        EndpointConfig {
            id: "iot_device_001".to_string(),
            url: "https://iot.example.com/api/devices/001/data".to_string(),
            auth: AuthMethod::BasicAuth {
                username: "device_001".to_string(),
                password_env: "IOT_DEVICE_PASSWORD".to_string(),
            },
            parser_type: "custom_iot".to_string(),
            enabled: true,
            query_params: None,
        },
    ],
    poll_interval: Duration::from_secs(30),
    ..Default::default()
};
```

---

## Multiple Endpoints

Poll multiple data sources simultaneously:

```rust
let config = HttpPollingConfig {
    endpoints: vec![
        // Weather data
        EndpointConfig {
            id: "weather_boston".to_string(),
            url: "https://api.openweathermap.org/data/2.5/weather".to_string(),
            auth: AuthMethod::QueryParam {
                param_name: "appid".to_string(),
                value_env: "OPENWEATHER_API_KEY".to_string(),
            },
            parser_type: "openweather".to_string(),
            enabled: true,
            query_params: Some(HashMap::from([
                ("q".to_string(), "Boston,US".to_string()),
            ])),
        },

        // Air quality sensor 1
        EndpointConfig {
            id: "airgradient_001".to_string(),
            url: "http://airgradient_001.local/measures/current".to_string(),
            auth: AuthMethod::None,
            parser_type: "airgradient".to_string(),
            enabled: true,
            query_params: None,
        },

        // Air quality sensor 2
        EndpointConfig {
            id: "airgradient_002".to_string(),
            url: "http://airgradient_002.local/measures/current".to_string(),
            auth: AuthMethod::None,
            parser_type: "airgradient".to_string(),
            enabled: true,
            query_params: None,
        },

        // Custom IoT device
        EndpointConfig {
            id: "iot_device_001".to_string(),
            url: "https://iot.example.com/api/devices/001/data".to_string(),
            auth: AuthMethod::Header {
                header_name: "X-API-Key".to_string(),
                value_env: "IOT_API_KEY".to_string(),
            },
            parser_type: "custom_iot".to_string(),
            enabled: true,
            query_params: None,
        },
    ],
    poll_interval: Duration::from_secs(60),
    ..Default::default()
};

// Register all parsers
let mut registry = ParserRegistry::new();
registry.register("openweather".to_string(), Arc::new(OpenWeatherParser));
registry.register("airgradient".to_string(), Arc::new(AirGradientParser));
registry.register("custom_iot".to_string(), Arc::new(CustomDeviceParser));

let mut source = HttpPollingSource::new(config, registry)?;
source.start().await?;
```

---

## Advanced Configuration

### Custom Retry Policy

```rust
use platform_core::sources::http_poll::RetryConfig;

let retry_config = RetryConfig {
    max_retries: 5,                  // Try up to 5 times
    initial_delay_ms: 200,           // Start with 200ms
    max_delay_ms: 60000,             // Cap at 60 seconds
    backoff_multiplier: 2.5,         // Aggressive backoff
    jitter: true,                     // Add randomness
};

let config = HttpPollingConfig {
    retry: retry_config,
    ..Default::default()
};
```

### Conditional Endpoint Activation

```rust
let config = HttpPollingConfig {
    endpoints: vec![
        EndpointConfig {
            id: "prod_sensor".to_string(),
            url: "http://prod-sensor.local/data".to_string(),
            enabled: std::env::var("ENVIRONMENT").unwrap_or_default() == "production",
            ..Default::default()
        },
        EndpointConfig {
            id: "test_sensor".to_string(),
            url: "http://test-sensor.local/data".to_string(),
            enabled: std::env::var("ENVIRONMENT").unwrap_or_default() != "production",
            ..Default::default()
        },
    ],
    ..Default::default()
};
```

### Health Monitoring

```rust
use platform_core::traits::Source;

// Start source
let mut source = HttpPollingSource::new(config, registry)?;
source.start().await?;

// Monitor health
tokio::spawn(async move {
    loop {
        tokio::time::sleep(Duration::from_secs(30)).await;

        match source.health_check().await {
            Ok(health) => {
                if health.healthy {
                    println!("✓ Source healthy: {}", health.message);
                } else {
                    eprintln!("✗ Source unhealthy: {}", health.message);
                    eprintln!("  Details: {:?}", health.details);
                }
            }
            Err(e) => {
                eprintln!("Health check failed: {}", e);
            }
        }
    }
});
```

---

## Error Handling

The retry logic automatically classifies and handles errors:

- **Transient errors** (timeouts, 5xx): Automatically retried
- **Rate limiting** (429): Exponential backoff applied
- **Permanent errors** (4xx, parse errors): No retry

```rust
// Errors are logged automatically, but you can also handle them:
match source.fetch().await {
    Ok(points) => {
        println!("Got {} points", points.len());
    }
    Err(e) => {
        eprintln!("Fetch failed: {}", e);
        // Source will continue retrying in background
    }
}
```

---

## Best Practices

1. **Environment Variables for Secrets**: Always use environment variables for API keys
2. **Appropriate Poll Intervals**: Don't poll too frequently (respect API rate limits)
3. **Parser Testing**: Write unit tests for your custom parsers
4. **Health Monitoring**: Regularly check source health
5. **Retry Configuration**: Tune retry settings based on API characteristics
6. **Endpoint IDs**: Use descriptive IDs for easier debugging

---

## Migration from Old API

### Before
```rust
let config = HttpPollingConfig {
    sensors: vec![
        SensorConfig {
            serial_number: "001".to_string(),
            url: "http://sensor001.local/data".to_string(),
        },
    ],
    ..Default::default()
};
```

### After
```rust
let config = HttpPollingConfig {
    endpoints: vec![
        EndpointConfig {
            id: "sensor_001".to_string(),
            url: "http://sensor001.local/data".to_string(),
            auth: AuthMethod::None,
            parser_type: "your_parser".to_string(),
            enabled: true,
            query_params: None,
        },
    ],
    ..Default::default()
};
```

---

## Testing

### Unit Test Example

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_custom_parser() {
        let parser = CustomDeviceParser;
        let response = r#"{
            "device_id": "test001",
            "measurements": [
                {"sensor": "temp", "value": 22.5, "unit": "C"}
            ]
        }"#;

        let result = parser.parse(response, "test_location", Utc::now());
        assert!(result.is_ok());

        let points = result.unwrap();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].value, 22.5);
    }
}
```

---

## References

- [HTTP Polling Source Refactor Documentation](../architecture/HTTP_POLLING_SOURCE_REFACTOR.md)
- [AIR-005 Architecture Design](../architecture/AIR-005_INGESTION_COORDINATOR_DESIGN.md)
