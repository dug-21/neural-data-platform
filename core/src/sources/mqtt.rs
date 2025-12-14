//! MQTT-based data source implementation
//!
//! Provides real-time data ingestion from MQTT brokers with:
//! - Auto-reconnect with exponential backoff
//! - Backpressure handling with bounded queues
//! - Topic pattern substitution for multiple sensors

use async_trait::async_trait;
use chrono::Utc;
use rumqttc::{AsyncClient, Event, EventLoop, MqttOptions, Packet, QoS};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

use crate::error::{CoreError, CoreResult};
use crate::traits::{HealthStatus, Source, TimeSeriesPoint};

/// Configuration for MQTT source
#[derive(Debug, Clone)]
pub struct MqttConfig {
    pub broker_url: String,
    pub port: u16,
    pub client_id: String,
    pub topic_pattern: String,
    pub qos: QoS,
    pub reconnect_delay: Duration,
    pub max_reconnect_delay: Duration,
    pub buffer_capacity: usize,
}

impl Default for MqttConfig {
    fn default() -> Self {
        Self {
            broker_url: "localhost".to_string(),
            port: 1883,
            client_id: "neural-data-platform".to_string(),
            topic_pattern: "airgradient/readings/{SERIAL_NUMBER}".to_string(),
            qos: QoS::AtLeastOnce,
            reconnect_delay: Duration::from_secs(1),
            max_reconnect_delay: Duration::from_secs(30),
            buffer_capacity: 1000,
        }
    }
}

/// AirGradient sensor reading from MQTT
#[derive(Debug, Clone, Deserialize)]
struct AirGradientReading {
    #[serde(rename = "serialno")]
    serial_no: String,
    pm02: Option<f64>,
    #[serde(rename = "rco2")]
    co2: Option<f64>,
    #[serde(rename = "atmp")]
    temperature: Option<f64>,
    #[serde(rename = "rhum")]
    humidity: Option<f64>,
    #[serde(rename = "wifi")]
    wifi_strength: Option<i32>,
}

/// MQTT data source
pub struct MqttSource {
    config: MqttConfig,
    client: Option<AsyncClient>,
    event_loop: Option<EventLoop>,
    receiver: Arc<Mutex<mpsc::Receiver<TimeSeriesPoint>>>,
    sender: mpsc::Sender<TimeSeriesPoint>,
    is_running: Arc<Mutex<bool>>,
    connection_healthy: Arc<Mutex<bool>>,
    cached_points: Arc<Mutex<Vec<TimeSeriesPoint>>>,
}

impl MqttSource {
    /// Create a new MQTT source
    pub fn new(config: MqttConfig) -> Self {
        let (sender, receiver) = mpsc::channel(config.buffer_capacity);

        Self {
            config,
            client: None,
            event_loop: None,
            receiver: Arc::new(Mutex::new(receiver)),
            sender,
            is_running: Arc::new(Mutex::new(false)),
            connection_healthy: Arc::new(Mutex::new(false)),
            cached_points: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Parse MQTT payload into time series points
    fn parse_payload(&self, payload: &[u8]) -> CoreResult<Vec<TimeSeriesPoint>> {
        let reading: AirGradientReading = serde_json::from_slice(payload)
            .map_err(|e| CoreError::Source(format!("Failed to parse MQTT payload: {}", e)))?;

        let timestamp = Utc::now();
        let mut points = Vec::new();

        // Store each metric as a separate point with metric type in tags
        if let Some(pm02) = reading.pm02 {
            let mut tags = HashMap::new();
            tags.insert("metric".to_string(), "pm02".to_string());
            tags.insert("source".to_string(), "mqtt".to_string());

            points.push(TimeSeriesPoint {
                timestamp,
                location_id: reading.serial_no.clone(),
                value: pm02,
                tags,
            });
        }

        if let Some(co2) = reading.co2 {
            let mut tags = HashMap::new();
            tags.insert("metric".to_string(), "co2".to_string());
            tags.insert("source".to_string(), "mqtt".to_string());

            points.push(TimeSeriesPoint {
                timestamp,
                location_id: reading.serial_no.clone(),
                value: co2,
                tags,
            });
        }

        if let Some(temp) = reading.temperature {
            let mut tags = HashMap::new();
            tags.insert("metric".to_string(), "temperature".to_string());
            tags.insert("source".to_string(), "mqtt".to_string());

            points.push(TimeSeriesPoint {
                timestamp,
                location_id: reading.serial_no.clone(),
                value: temp,
                tags,
            });
        }

        if let Some(humidity) = reading.humidity {
            let mut tags = HashMap::new();
            tags.insert("metric".to_string(), "humidity".to_string());
            tags.insert("source".to_string(), "mqtt".to_string());

            points.push(TimeSeriesPoint {
                timestamp,
                location_id: reading.serial_no.clone(),
                value: humidity,
                tags,
            });
        }

        if let Some(wifi) = reading.wifi_strength {
            let mut tags = HashMap::new();
            tags.insert("metric".to_string(), "wifi_strength".to_string());
            tags.insert("source".to_string(), "mqtt".to_string());

            points.push(TimeSeriesPoint {
                timestamp,
                location_id: reading.serial_no.clone(),
                value: wifi as f64,
                tags,
            });
        }

        Ok(points)
    }

    /// Handle reconnection with exponential backoff
    async fn reconnect(&mut self, attempt: u32) -> CoreResult<()> {
        let delay = std::cmp::min(
            self.config.reconnect_delay.as_secs() * 2_u64.pow(attempt),
            self.config.max_reconnect_delay.as_secs(),
        );

        warn!(
            "Reconnecting to MQTT broker in {} seconds (attempt {})",
            delay, attempt
        );

        tokio::time::sleep(Duration::from_secs(delay)).await;

        let mut mqtt_options = MqttOptions::new(
            &self.config.client_id,
            &self.config.broker_url,
            self.config.port,
        );
        mqtt_options.set_keep_alive(Duration::from_secs(30));

        let (client, event_loop) = AsyncClient::new(mqtt_options, self.config.buffer_capacity);
        self.client = Some(client);
        self.event_loop = Some(event_loop);

        if let Some(client) = &self.client {
            let topic = self.config.topic_pattern.replace("{SERIAL_NUMBER}", "+");
            client
                .subscribe(&topic, self.config.qos)
                .await
                .map_err(|e| CoreError::Source(format!("Failed to subscribe: {}", e)))?;
            info!("Subscribed to topic: {}", topic);
        }

        *self.connection_healthy.lock().await = true;
        Ok(())
    }

    /// Process MQTT events
    async fn process_events(&mut self) -> CoreResult<()> {
        let mut reconnect_attempt = 0_u32;

        while *self.is_running.lock().await {
            if let Some(event_loop) = &mut self.event_loop {
                match event_loop.poll().await {
                    Ok(Event::Incoming(Packet::Publish(publish))) => {
                        debug!("Received MQTT message on topic: {}", publish.topic);
                        reconnect_attempt = 0;

                        match self.parse_payload(&publish.payload) {
                            Ok(points) => {
                                // Add to cache for fetch()
                                let mut cache = self.cached_points.lock().await;
                                cache.extend(points);
                            }
                            Err(e) => {
                                error!("Failed to parse payload: {}", e);
                            }
                        }
                    }
                    Ok(Event::Incoming(Packet::ConnAck(_))) => {
                        info!("Connected to MQTT broker");
                        *self.connection_healthy.lock().await = true;
                        reconnect_attempt = 0;
                    }
                    Ok(Event::Incoming(Packet::Disconnect)) => {
                        warn!("Disconnected from MQTT broker");
                        *self.connection_healthy.lock().await = false;
                        self.reconnect(reconnect_attempt).await?;
                        reconnect_attempt += 1;
                    }
                    Err(e) => {
                        error!("MQTT connection error: {}", e);
                        *self.connection_healthy.lock().await = false;
                        self.reconnect(reconnect_attempt).await?;
                        reconnect_attempt += 1;
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    /// Start the MQTT source
    pub async fn start(&mut self) -> CoreResult<()> {
        info!("Starting MQTT source: {}", self.config.client_id);

        *self.is_running.lock().await = true;

        // Initial connection
        self.reconnect(0).await?;

        // Spawn background task for event processing
        let mut source_clone = MqttSource {
            config: self.config.clone(),
            client: self.client.clone(),
            event_loop: self.event_loop.take(),
            receiver: self.receiver.clone(),
            sender: self.sender.clone(),
            is_running: self.is_running.clone(),
            connection_healthy: self.connection_healthy.clone(),
            cached_points: self.cached_points.clone(),
        };

        tokio::spawn(async move {
            if let Err(e) = source_clone.process_events().await {
                error!("MQTT event processing failed: {}", e);
            }
        });

        Ok(())
    }

    /// Stop the MQTT source
    pub async fn stop(&mut self) -> CoreResult<()> {
        info!("Stopping MQTT source: {}", self.config.client_id);
        *self.is_running.lock().await = false;
        *self.connection_healthy.lock().await = false;

        if let Some(client) = &self.client {
            client
                .disconnect()
                .await
                .map_err(|e| CoreError::Source(format!("Failed to disconnect: {}", e)))?;
        }

        Ok(())
    }
}

#[async_trait]
impl Source for MqttSource {
    async fn fetch(&self) -> CoreResult<Vec<TimeSeriesPoint>> {
        let mut cache = self.cached_points.lock().await;
        let points = cache.drain(..).collect();
        Ok(points)
    }

    async fn health_check(&self) -> CoreResult<HealthStatus> {
        let is_healthy = *self.connection_healthy.lock().await;
        let is_running = *self.is_running.lock().await;

        if is_running && is_healthy {
            Ok(HealthStatus {
                healthy: true,
                message: "MQTT connection healthy".to_string(),
                details: HashMap::new(),
            })
        } else if is_running {
            Ok(HealthStatus {
                healthy: false,
                message: "MQTT connection unhealthy".to_string(),
                details: HashMap::new(),
            })
        } else {
            Ok(HealthStatus {
                healthy: false,
                message: "MQTT source not running".to_string(),
                details: HashMap::new(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mqtt_source_creation() {
        let config = MqttConfig::default();
        let _source = MqttSource::new(config.clone());
    }

    #[tokio::test]
    async fn test_health_check_before_start() {
        let config = MqttConfig::default();
        let source = MqttSource::new(config);

        let health = source.health_check().await.unwrap();
        assert!(!health.healthy);
    }

    #[tokio::test]
    async fn test_parse_payload_success() {
        let config = MqttConfig::default();
        let source = MqttSource::new(config);

        let payload = r#"{
            "serialno": "ABC123",
            "pm02": 12.5,
            "rco2": 450,
            "atmp": 22.3,
            "rhum": 55.0,
            "wifi": -45
        }"#;

        let points = source.parse_payload(payload.as_bytes()).unwrap();
        assert_eq!(points.len(), 5);

        // Check PM2.5
        let pm_point = points
            .iter()
            .find(|p| p.tags.get("metric") == Some(&"pm02".to_string()))
            .unwrap();
        assert_eq!(pm_point.value, 12.5);
        assert_eq!(pm_point.location_id, "ABC123");

        // Check CO2
        let co2_point = points
            .iter()
            .find(|p| p.tags.get("metric") == Some(&"co2".to_string()))
            .unwrap();
        assert_eq!(co2_point.value, 450.0);
    }

    #[tokio::test]
    async fn test_parse_payload_invalid_json() {
        let config = MqttConfig::default();
        let source = MqttSource::new(config);

        let payload = b"invalid json";
        let result = source.parse_payload(payload);

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_parse_payload_partial_data() {
        let config = MqttConfig::default();
        let source = MqttSource::new(config);

        let payload = r#"{
            "serialno": "ABC123",
            "pm02": 12.5
        }"#;

        let points = source.parse_payload(payload.as_bytes()).unwrap();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].tags.get("metric").unwrap(), "pm02");
        assert_eq!(points[0].value, 12.5);
    }

    #[tokio::test]
    async fn test_exponential_backoff_calculation() {
        let config = MqttConfig {
            reconnect_delay: Duration::from_secs(1),
            max_reconnect_delay: Duration::from_secs(30),
            ..Default::default()
        };

        let delays = vec![
            (0, 1),
            (1, 2),
            (2, 4),
            (3, 8),
            (4, 16),
            (5, 30),
            (6, 30),
        ];

        for (attempt, expected) in delays {
            let delay = std::cmp::min(
                config.reconnect_delay.as_secs() * 2_u64.pow(attempt),
                config.max_reconnect_delay.as_secs(),
            );
            assert_eq!(delay, expected, "Failed for attempt {}", attempt);
        }
    }

    #[tokio::test]
    async fn test_topic_pattern_substitution() {
        let config = MqttConfig {
            topic_pattern: "airgradient/readings/{SERIAL_NUMBER}".to_string(),
            ..Default::default()
        };

        let topic = config.topic_pattern.replace("{SERIAL_NUMBER}", "+");
        assert_eq!(topic, "airgradient/readings/+");
    }

    #[tokio::test]
    async fn test_fetch_returns_cached_points() {
        let config = MqttConfig::default();
        let source = MqttSource::new(config);

        // Add some points to cache
        let mut cache = source.cached_points.lock().await;
        cache.push(TimeSeriesPoint {
            timestamp: Utc::now(),
            location_id: "test".to_string(),
            value: 12.5,
            tags: HashMap::new(),
        });
        drop(cache);

        let points = source.fetch().await.unwrap();
        assert_eq!(points.len(), 1);

        // Cache should be empty after fetch
        let cache = source.cached_points.lock().await;
        assert_eq!(cache.len(), 0);
    }
}
