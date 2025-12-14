//! HTTP polling data source implementation
//!
//! Provides periodic data ingestion from HTTP endpoints with:
//! - Configurable poll intervals
//! - Request timeouts
//! - Multiple sensor support
//! - Error handling and retries

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

use crate::error::{CoreError, CoreResult};
use crate::traits::{HealthStatus, Source, TimeSeriesPoint};

/// Configuration for a single sensor
#[derive(Debug, Clone)]
pub struct SensorConfig {
    pub serial_number: String,
    pub url: String,
}

/// Configuration for HTTP polling source
#[derive(Debug, Clone)]
pub struct HttpPollingConfig {
    pub base_url_template: String,
    pub poll_interval: Duration,
    pub timeout: Duration,
    pub sensors: Vec<SensorConfig>,
    pub buffer_capacity: usize,
}

impl Default for HttpPollingConfig {
    fn default() -> Self {
        Self {
            base_url_template: "http://airgradient_{SERIAL}.local/measures/current".to_string(),
            poll_interval: Duration::from_secs(60),
            timeout: Duration::from_secs(10),
            sensors: Vec::new(),
            buffer_capacity: 1000,
        }
    }
}

/// AirGradient current measures response
#[derive(Debug, Clone, Deserialize)]
struct CurrentMeasures {
    #[serde(rename = "serialno")]
    serial_no: Option<String>,
    pm02: Option<f64>,
    #[serde(rename = "rco2")]
    co2: Option<f64>,
    #[serde(rename = "atmp")]
    temperature: Option<f64>,
    #[serde(rename = "rhum")]
    humidity: Option<f64>,
    #[serde(rename = "wifi")]
    wifi_strength: Option<i32>,
    // Extended fields not available in MQTT
    pm10: Option<f64>,
    pm01: Option<f64>,
    #[serde(rename = "tvoc")]
    tvoc: Option<f64>,
    #[serde(rename = "nox")]
    nox_index: Option<f64>,
}

/// HTTP polling data source
pub struct HttpPollingSource {
    config: HttpPollingConfig,
    client: Client,
    receiver: Arc<Mutex<mpsc::Receiver<TimeSeriesPoint>>>,
    sender: mpsc::Sender<TimeSeriesPoint>,
    is_running: Arc<Mutex<bool>>,
    last_successful_poll: Arc<Mutex<HashMap<String, DateTime<Utc>>>>,
}

impl HttpPollingSource {
    /// Create a new HTTP polling source
    pub fn new(config: HttpPollingConfig) -> CoreResult<Self> {
        let (sender, receiver) = mpsc::channel(config.buffer_capacity);

        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| CoreError::Source(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            config,
            client,
            receiver: Arc::new(Mutex::new(receiver)),
            sender,
            is_running: Arc::new(Mutex::new(false)),
            last_successful_poll: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Poll a single sensor
    async fn poll_sensor(&self, sensor: &SensorConfig) -> CoreResult<Vec<TimeSeriesPoint>> {
        debug!("Polling sensor: {}", sensor.serial_number);

        let response = self
            .client
            .get(&sensor.url)
            .send()
            .await
            .map_err(|e| CoreError::Source(format!("HTTP request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(CoreError::Source(format!(
                "HTTP request failed with status: {}",
                response.status()
            )));
        }

        let measures: CurrentMeasures = response
            .json()
            .await
            .map_err(|e| CoreError::Source(format!("Failed to parse response: {}", e)))?;

        self.parse_measures(measures, &sensor.serial_number)
    }

    /// Parse current measures into time series points
    fn parse_measures(
        &self,
        measures: CurrentMeasures,
        serial_number: &str,
    ) -> CoreResult<Vec<TimeSeriesPoint>> {
        let timestamp = Utc::now();
        let mut points = Vec::new();

        let location_id = measures
            .serial_no
            .as_ref()
            .unwrap_or(&serial_number.to_string())
            .clone();

        // Standard metrics (available in both MQTT and HTTP)
        if let Some(pm02) = measures.pm02 {
            let mut tags = HashMap::new();
            tags.insert("metric".to_string(), "pm02".to_string());
            tags.insert("source".to_string(), "http".to_string());

            points.push(TimeSeriesPoint {
                timestamp,
                location_id: location_id.clone(),
                value: pm02,
                tags,
            });
        }

        if let Some(co2) = measures.co2 {
            let mut tags = HashMap::new();
            tags.insert("metric".to_string(), "co2".to_string());
            tags.insert("source".to_string(), "http".to_string());

            points.push(TimeSeriesPoint {
                timestamp,
                location_id: location_id.clone(),
                value: co2,
                tags,
            });
        }

        if let Some(temp) = measures.temperature {
            let mut tags = HashMap::new();
            tags.insert("metric".to_string(), "temperature".to_string());
            tags.insert("source".to_string(), "http".to_string());

            points.push(TimeSeriesPoint {
                timestamp,
                location_id: location_id.clone(),
                value: temp,
                tags,
            });
        }

        if let Some(humidity) = measures.humidity {
            let mut tags = HashMap::new();
            tags.insert("metric".to_string(), "humidity".to_string());
            tags.insert("source".to_string(), "http".to_string());

            points.push(TimeSeriesPoint {
                timestamp,
                location_id: location_id.clone(),
                value: humidity,
                tags,
            });
        }

        if let Some(wifi) = measures.wifi_strength {
            let mut tags = HashMap::new();
            tags.insert("metric".to_string(), "wifi_strength".to_string());
            tags.insert("source".to_string(), "http".to_string());

            points.push(TimeSeriesPoint {
                timestamp,
                location_id: location_id.clone(),
                value: wifi as f64,
                tags,
            });
        }

        // Extended metrics (HTTP only)
        if let Some(pm10) = measures.pm10 {
            let mut tags = HashMap::new();
            tags.insert("metric".to_string(), "pm10".to_string());
            tags.insert("source".to_string(), "http".to_string());

            points.push(TimeSeriesPoint {
                timestamp,
                location_id: location_id.clone(),
                value: pm10,
                tags,
            });
        }

        if let Some(pm01) = measures.pm01 {
            let mut tags = HashMap::new();
            tags.insert("metric".to_string(), "pm01".to_string());
            tags.insert("source".to_string(), "http".to_string());

            points.push(TimeSeriesPoint {
                timestamp,
                location_id: location_id.clone(),
                value: pm01,
                tags,
            });
        }

        if let Some(tvoc) = measures.tvoc {
            let mut tags = HashMap::new();
            tags.insert("metric".to_string(), "tvoc".to_string());
            tags.insert("source".to_string(), "http".to_string());

            points.push(TimeSeriesPoint {
                timestamp,
                location_id: location_id.clone(),
                value: tvoc,
                tags,
            });
        }

        if let Some(nox) = measures.nox_index {
            let mut tags = HashMap::new();
            tags.insert("metric".to_string(), "nox_index".to_string());
            tags.insert("source".to_string(), "http".to_string());

            points.push(TimeSeriesPoint {
                timestamp,
                location_id: location_id.clone(),
                value: nox,
                tags,
            });
        }

        Ok(points)
    }

    /// Poll all sensors
    async fn poll_all_sensors(&self) -> CoreResult<()> {
        for sensor in &self.config.sensors {
            match self.poll_sensor(sensor).await {
                Ok(points) => {
                    debug!(
                        "Successfully polled sensor {} - got {} points",
                        sensor.serial_number,
                        points.len()
                    );

                    // Update last successful poll time
                    let mut last_poll = self.last_successful_poll.lock().await;
                    last_poll.insert(sensor.serial_number.clone(), Utc::now());

                    // Send points to channel
                    for point in points {
                        if let Err(e) = self.sender.send(point).await {
                            warn!("Failed to send point to channel: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to poll sensor {}: {}", sensor.serial_number, e);
                }
            }
        }

        Ok(())
    }

    /// Background polling task
    async fn polling_loop(&self) -> CoreResult<()> {
        let mut interval = tokio::time::interval(self.config.poll_interval);

        while *self.is_running.lock().await {
            interval.tick().await;

            if let Err(e) = self.poll_all_sensors().await {
                error!("Polling error: {}", e);
            }
        }

        Ok(())
    }
}

#[async_trait]
impl Source for HttpPollingSource {
    async fn fetch(&self) -> CoreResult<Vec<TimeSeriesPoint>> {
        let mut receiver = self.receiver.lock().await;
        let mut points = Vec::new();

        // Drain all available points from the channel
        while let Ok(point) = receiver.try_recv() {
            points.push(point);
        }

        Ok(points)
    }

    async fn health_check(&self) -> CoreResult<HealthStatus> {
        let is_running = *self.is_running.lock().await;

        let mut details = HashMap::new();
        details.insert("source_type".to_string(), "http_polling".to_string());
        details.insert("is_running".to_string(), is_running.to_string());

        if !is_running {
            return Ok(HealthStatus {
                healthy: false,
                message: "HTTP polling source not running".to_string(),
                details,
            });
        }

        let last_poll = self.last_successful_poll.lock().await;
        let now = Utc::now();

        // Check if any sensor has been polled recently
        let unhealthy_sensors: Vec<_> = self
            .config
            .sensors
            .iter()
            .filter(|sensor| {
                if let Some(last_time) = last_poll.get(&sensor.serial_number) {
                    (now - *last_time).num_seconds() > (self.config.poll_interval.as_secs() * 2) as i64
                } else {
                    true // Never polled
                }
            })
            .map(|s| s.serial_number.clone())
            .collect();

        if unhealthy_sensors.is_empty() {
            details.insert("status".to_string(), "all_sensors_healthy".to_string());
            Ok(HealthStatus {
                healthy: true,
                message: "All sensors operational".to_string(),
                details,
            })
        } else if unhealthy_sensors.len() == self.config.sensors.len() {
            details.insert("status".to_string(), "all_sensors_unhealthy".to_string());
            details.insert("unhealthy_sensors".to_string(), format!("{:?}", unhealthy_sensors));
            Ok(HealthStatus {
                healthy: false,
                message: format!("All sensors unhealthy: {:?}", unhealthy_sensors),
                details,
            })
        } else {
            details.insert("status".to_string(), "some_sensors_unhealthy".to_string());
            details.insert("unhealthy_sensors".to_string(), format!("{:?}", unhealthy_sensors));
            Ok(HealthStatus {
                healthy: false,
                message: format!("Some sensors unhealthy: {:?}", unhealthy_sensors),
                details,
            })
        }
    }
}

impl HttpPollingSource {
    /// Start the HTTP polling source
    pub async fn start(&mut self) -> CoreResult<()> {
        info!("Starting HTTP polling source");

        if self.config.sensors.is_empty() {
            return Err(CoreError::Source("No sensors configured".to_string()));
        }

        *self.is_running.lock().await = true;

        // Clone necessary data for background task
        let source_clone = Self {
            config: self.config.clone(),
            client: self.client.clone(),
            receiver: self.receiver.clone(),
            sender: self.sender.clone(),
            is_running: self.is_running.clone(),
            last_successful_poll: self.last_successful_poll.clone(),
        };

        // Spawn background polling task
        tokio::spawn(async move {
            if let Err(e) = source_clone.polling_loop().await {
                error!("HTTP polling loop failed: {}", e);
            }
        });

        // Initial poll
        self.poll_all_sensors().await?;

        Ok(())
    }

    /// Stop the HTTP polling source
    pub async fn stop(&mut self) -> CoreResult<()> {
        info!("Stopping HTTP polling source");
        *self.is_running.lock().await = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_http_source_creation() {
        let config = HttpPollingConfig::default();
        let source = HttpPollingSource::new(config);

        assert!(source.is_ok());
    }

    #[tokio::test]
    async fn test_health_check_not_running() {
        let config = HttpPollingConfig::default();
        let source = HttpPollingSource::new(config).unwrap();

        let health = source.health_check().await.unwrap();
        assert!(!health.healthy);
    }

    #[tokio::test]
    async fn test_parse_measures_full_data() {
        let config = HttpPollingConfig::default();
        let source = HttpPollingSource::new(config).unwrap();

        let measures = CurrentMeasures {
            serial_no: Some("ABC123".to_string()),
            pm02: Some(12.5),
            co2: Some(450.0),
            temperature: Some(22.3),
            humidity: Some(55.0),
            wifi_strength: Some(-45),
            pm10: Some(15.2),
            pm01: Some(8.1),
            tvoc: Some(120.0),
            nox_index: Some(1.5),
        };

        let points = source.parse_measures(measures, "ABC123").unwrap();
        assert_eq!(points.len(), 9);

        // Check standard metrics
        assert!(points.iter().any(|p| p.tags.get("metric") == Some(&"pm02".to_string())));
        assert!(points.iter().any(|p| p.tags.get("metric") == Some(&"co2".to_string())));
        assert!(points.iter().any(|p| p.tags.get("metric") == Some(&"temperature".to_string())));
        assert!(points.iter().any(|p| p.tags.get("metric") == Some(&"humidity".to_string())));
        assert!(points.iter().any(|p| p.tags.get("metric") == Some(&"wifi_strength".to_string())));

        // Check extended metrics (HTTP only)
        assert!(points.iter().any(|p| p.tags.get("metric") == Some(&"pm10".to_string())));
        assert!(points.iter().any(|p| p.tags.get("metric") == Some(&"pm01".to_string())));
        assert!(points.iter().any(|p| p.tags.get("metric") == Some(&"tvoc".to_string())));
        assert!(points.iter().any(|p| p.tags.get("metric") == Some(&"nox_index".to_string())));

        // Verify source ID
        assert!(points.iter().all(|p| p.location_id == "ABC123"));
    }

    #[tokio::test]
    async fn test_parse_measures_partial_data() {
        let config = HttpPollingConfig::default();
        let source = HttpPollingSource::new(config).unwrap();

        let measures = CurrentMeasures {
            serial_no: Some("ABC123".to_string()),
            pm02: Some(12.5),
            co2: None,
            temperature: None,
            humidity: None,
            wifi_strength: None,
            pm10: None,
            pm01: None,
            tvoc: None,
            nox_index: None,
        };

        let points = source.parse_measures(measures, "ABC123").unwrap();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].tags.get("metric").unwrap(), "pm02");
        assert_eq!(points[0].value, 12.5);
    }

    #[tokio::test]
    async fn test_poll_sensor_success() {
        let mock_server = MockServer::start().await;

        let response_body = r#"{
            "serialno": "TEST123",
            "pm02": 10.5,
            "rco2": 400,
            "atmp": 21.0,
            "rhum": 50.0,
            "wifi": -50,
            "pm10": 12.0,
            "tvoc": 100
        }"#;

        Mock::given(method("GET"))
            .and(path("/measures/current"))
            .respond_with(ResponseTemplate::new(200).set_body_string(response_body))
            .mount(&mock_server)
            .await;

        let sensor = SensorConfig {
            serial_number: "TEST123".to_string(),
            url: format!("{}/measures/current", mock_server.uri()),
        };

        let config = HttpPollingConfig {
            sensors: vec![sensor.clone()],
            ..Default::default()
        };

        let source = HttpPollingSource::new(config).unwrap();
        let points = source.poll_sensor(&sensor).await.unwrap();

        assert!(points.len() >= 6);
        assert!(points.iter().any(|p| p.tags.get("metric") == Some(&"pm02".to_string()) && p.value == 10.5));
        assert!(points.iter().any(|p| p.tags.get("metric") == Some(&"co2".to_string()) && p.value == 400.0));
    }

    #[tokio::test]
    async fn test_poll_sensor_timeout() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/measures/current"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_delay(Duration::from_secs(15)) // Longer than default timeout
                    .set_body_string("{}"),
            )
            .mount(&mock_server)
            .await;

        let sensor = SensorConfig {
            serial_number: "TEST123".to_string(),
            url: format!("{}/measures/current", mock_server.uri()),
        };

        let config = HttpPollingConfig {
            timeout: Duration::from_secs(1),
            sensors: vec![sensor.clone()],
            ..Default::default()
        };

        let source = HttpPollingSource::new(config).unwrap();
        let result = source.poll_sensor(&sensor).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_poll_sensor_network_error() {
        let sensor = SensorConfig {
            serial_number: "TEST123".to_string(),
            url: "http://non-existent-host.local/measures/current".to_string(),
        };

        let config = HttpPollingConfig {
            timeout: Duration::from_secs(1),
            sensors: vec![sensor.clone()],
            ..Default::default()
        };

        let source = HttpPollingSource::new(config).unwrap();
        let result = source.poll_sensor(&sensor).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_poll_sensor_http_error_status() {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/measures/current"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;

        let sensor = SensorConfig {
            serial_number: "TEST123".to_string(),
            url: format!("{}/measures/current", mock_server.uri()),
        };

        let config = HttpPollingConfig {
            sensors: vec![sensor.clone()],
            ..Default::default()
        };

        let source = HttpPollingSource::new(config).unwrap();
        let result = source.poll_sensor(&sensor).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_start_no_sensors() {
        let config = HttpPollingConfig {
            sensors: vec![],
            ..Default::default()
        };

        let mut source = HttpPollingSource::new(config).unwrap();
        let result = source.start().await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CoreError::Source(_)));
    }

    #[tokio::test]
    async fn test_poll_interval() {
        let config = HttpPollingConfig {
            poll_interval: Duration::from_millis(100),
            ..Default::default()
        };

        assert_eq!(config.poll_interval, Duration::from_millis(100));
    }

    #[tokio::test]
    async fn test_buffer_capacity() {
        let config = HttpPollingConfig {
            buffer_capacity: 500,
            ..Default::default()
        };

        let source = HttpPollingSource::new(config).unwrap();
        assert_eq!(source.sender.capacity(), 500);
    }
}
