//! Source lifecycle management
//!
//! Spawns and manages source instances based on SourceConfig type

use crate::error::{CoreError, CoreResult};
use crate::sources::{HttpPollingConfig, HttpPollingSource, MqttConfig, MqttSource, SensorConfig};
use crate::traits::{HealthStatus, Source, TimeSeriesPoint};
use crate::types::{SourceConfig, SourceType};
use rumqttc::QoS;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

/// Handle to a running source instance
pub struct SourceHandle {
    pub source_id: String,
    pub source_type: SourceType,
    stop_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl SourceHandle {
    /// Stop the source
    pub async fn stop(&mut self) -> CoreResult<()> {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
            info!("Stopped source: {}", self.source_id);
        }
        Ok(())
    }
}

/// Manages source lifecycle - spawning, stopping, and health checks
pub struct SourceManager {
    /// Active source handles
    sources: Arc<Mutex<HashMap<String, SourceHandle>>>,
    /// Channel for sending data points from sources
    tx: mpsc::Sender<TimeSeriesPoint>,
}

impl SourceManager {
    /// Create a new source manager
    pub fn new(tx: mpsc::Sender<TimeSeriesPoint>) -> Self {
        Self {
            sources: Arc::new(Mutex::new(HashMap::new())),
            tx,
        }
    }

    /// Spawn a source based on configuration
    pub async fn spawn_source(
        &self,
        source_id: String,
        config: SourceConfig,
    ) -> CoreResult<()> {
        if !config.enabled {
            debug!("Skipping disabled source: {}", source_id);
            return Ok(());
        }

        info!("Spawning source: {} (type: {:?})", source_id, config.source_type);

        match config.source_type {
            SourceType::Mqtt => self.spawn_mqtt_source(source_id, config).await,
            SourceType::HttpPoll => self.spawn_http_poll_source(source_id, config).await,
            SourceType::Webhook => {
                warn!("Webhook sources not yet implemented: {}", source_id);
                Err(CoreError::Source("Webhook sources not implemented".to_string()))
            }
            SourceType::FileWatch => {
                warn!("FileWatch sources not yet implemented: {}", source_id);
                Err(CoreError::Source("FileWatch sources not implemented".to_string()))
            }
        }
    }

    /// Spawn an MQTT source
    async fn spawn_mqtt_source(
        &self,
        source_id: String,
        config: SourceConfig,
    ) -> CoreResult<()> {
        let params = &config.params;

        // Extract MQTT configuration from params
        let broker_url = params
            .get("broker_url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CoreError::Config("Missing broker_url for MQTT source".to_string()))?
            .to_string();

        let port = params
            .get("port")
            .and_then(|v| v.as_u64())
            .unwrap_or(1883) as u16;

        let client_id = params
            .get("client_id")
            .and_then(|v| v.as_str())
            .unwrap_or(&source_id)
            .to_string();

        let topic_pattern = params
            .get("topic_pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CoreError::Config("Missing topic_pattern for MQTT source".to_string()))?
            .to_string();

        let qos = match params.get("qos").and_then(|v| v.as_u64()).unwrap_or(1) {
            0 => QoS::AtMostOnce,
            1 => QoS::AtLeastOnce,
            2 => QoS::ExactlyOnce,
            _ => QoS::AtLeastOnce,
        };

        let reconnect_delay_secs = params
            .get("reconnect_delay_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(5);

        let max_reconnect_delay_secs = params
            .get("max_reconnect_delay_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(300);

        let buffer_capacity = params
            .get("buffer_capacity")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000) as usize;

        let mqtt_config = MqttConfig {
            broker_url,
            port,
            client_id,
            topic_pattern,
            qos,
            reconnect_delay: std::time::Duration::from_secs(reconnect_delay_secs),
            max_reconnect_delay: std::time::Duration::from_secs(max_reconnect_delay_secs),
            buffer_capacity,
        };

        let mqtt_source = MqttSource::new(mqtt_config);

        // Create stop signal channel
        let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel();

        // Spawn background task that periodically fetches data and forwards to coordinator
        let tx = self.tx.clone();
        let source_id_clone = source_id.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                        // Fetch available points from source
                        match mqtt_source.fetch().await {
                            Ok(points) => {
                                for point in points {
                                    if let Err(e) = tx.send(point).await {
                                        error!("Failed to send point from MQTT source {}: {}", source_id_clone, e);
                                        break;
                                    }
                                }
                            }
                            Err(e) => {
                                error!("MQTT source {} fetch error: {}", source_id_clone, e);
                            }
                        }
                    }
                    _ = &mut stop_rx => {
                        info!("MQTT source {} received stop signal", source_id_clone);
                        break;
                    }
                }
            }
        });

        // Store handle
        let handle = SourceHandle {
            source_id: source_id.clone(),
            source_type: SourceType::Mqtt,
            stop_tx: Some(stop_tx),
        };

        let mut sources = self.sources.lock().await;
        sources.insert(source_id, handle);

        Ok(())
    }

    /// Spawn an HTTP polling source
    async fn spawn_http_poll_source(
        &self,
        source_id: String,
        config: SourceConfig,
    ) -> CoreResult<()> {
        let params = &config.params;

        // Extract HTTP polling configuration from params
        let base_url_template = params
            .get("base_url_template")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                CoreError::Config("Missing base_url_template for HTTP polling source".to_string())
            })?
            .to_string();

        let poll_interval_secs = params
            .get("poll_interval_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(60);

        let timeout_secs = params
            .get("timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(10);

        let buffer_capacity = params
            .get("buffer_capacity")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000) as usize;

        // Extract sensor configurations
        let sensors_json = params
            .get("sensors")
            .ok_or_else(|| CoreError::Config("Missing sensors for HTTP polling source".to_string()))?;

        let sensors_array = sensors_json
            .as_array()
            .ok_or_else(|| CoreError::Config("sensors must be an array".to_string()))?;

        let mut sensors = Vec::new();
        for sensor_value in sensors_array {
            let sensor_obj = sensor_value
                .as_object()
                .ok_or_else(|| CoreError::Config("Each sensor must be an object".to_string()))?;

            let serial_number = sensor_obj
                .get("serial_number")
                .and_then(|v| v.as_str())
                .ok_or_else(|| CoreError::Config("Missing serial_number in sensor".to_string()))?
                .to_string();

            let url = sensor_obj
                .get("url")
                .and_then(|v| v.as_str())
                .ok_or_else(|| CoreError::Config("Missing url in sensor".to_string()))?
                .to_string();

            sensors.push(SensorConfig { serial_number, url });
        }

        let http_config = HttpPollingConfig {
            base_url_template,
            poll_interval: std::time::Duration::from_secs(poll_interval_secs),
            timeout: std::time::Duration::from_secs(timeout_secs),
            sensors,
            buffer_capacity,
        };

        let mut http_source = HttpPollingSource::new(http_config)?;

        // Start the source
        http_source.start().await?;

        // Create stop signal channel
        let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel();

        // Spawn background task that periodically fetches data and forwards to coordinator
        let tx = self.tx.clone();
        let source_id_clone = source_id.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {
                        // Fetch available points from source
                        match http_source.fetch().await {
                            Ok(points) => {
                                for point in points {
                                    if let Err(e) = tx.send(point).await {
                                        error!("Failed to send point from HTTP source {}: {}", source_id_clone, e);
                                        break;
                                    }
                                }
                            }
                            Err(e) => {
                                error!("HTTP source {} fetch error: {}", source_id_clone, e);
                            }
                        }
                    }
                    _ = &mut stop_rx => {
                        info!("HTTP source {} received stop signal", source_id_clone);
                        let _ = http_source.stop().await;
                        break;
                    }
                }
            }
        });

        // Store handle
        let handle = SourceHandle {
            source_id: source_id.clone(),
            source_type: SourceType::HttpPoll,
            stop_tx: Some(stop_tx),
        };

        let mut sources = self.sources.lock().await;
        sources.insert(source_id, handle);

        Ok(())
    }

    /// Stop a specific source
    pub async fn stop_source(&self, source_id: &str) -> CoreResult<()> {
        let mut sources = self.sources.lock().await;
        if let Some(mut handle) = sources.remove(source_id) {
            handle.stop().await?;
            Ok(())
        } else {
            Err(CoreError::Source(format!("Source not found: {}", source_id)))
        }
    }

    /// Stop all sources
    pub async fn stop_all(&self) -> CoreResult<()> {
        info!("Stopping all sources");
        let mut sources = self.sources.lock().await;

        for (source_id, mut handle) in sources.drain() {
            if let Err(e) = handle.stop().await {
                error!("Error stopping source {}: {}", source_id, e);
            }
        }

        Ok(())
    }

    /// Get health status of all sources
    pub async fn health_check(&self) -> CoreResult<HashMap<String, HealthStatus>> {
        let sources = self.sources.lock().await;
        let mut health_statuses = HashMap::new();

        for (source_id, handle) in sources.iter() {
            let status = HealthStatus {
                healthy: true,
                message: format!("{:?} source running", handle.source_type),
                details: [
                    ("source_id".to_string(), source_id.clone()),
                    ("source_type".to_string(), format!("{:?}", handle.source_type)),
                ]
                .iter()
                .cloned()
                .collect(),
            };
            health_statuses.insert(source_id.clone(), status);
        }

        Ok(health_statuses)
    }

    /// Get count of active sources
    pub async fn active_source_count(&self) -> usize {
        self.sources.lock().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_source_manager_creation() {
        let (tx, _rx) = mpsc::channel(100);
        let manager = SourceManager::new(tx);
        assert_eq!(manager.active_source_count().await, 0);
    }

    #[tokio::test]
    async fn test_spawn_disabled_source() {
        let (tx, _rx) = mpsc::channel(100);
        let manager = SourceManager::new(tx);

        let config = SourceConfig {
            source_type: SourceType::Mqtt,
            enabled: false,
            params: HashMap::new(),
        };

        let result = manager.spawn_source("test-source".to_string(), config).await;
        assert!(result.is_ok());
        assert_eq!(manager.active_source_count().await, 0);
    }

    #[tokio::test]
    async fn test_spawn_mqtt_source_missing_params() {
        let (tx, _rx) = mpsc::channel(100);
        let manager = SourceManager::new(tx);

        let config = SourceConfig {
            source_type: SourceType::Mqtt,
            enabled: true,
            params: HashMap::new(),
        };

        let result = manager.spawn_source("test-mqtt".to_string(), config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_spawn_http_source_missing_params() {
        let (tx, _rx) = mpsc::channel(100);
        let manager = SourceManager::new(tx);

        let config = SourceConfig {
            source_type: SourceType::HttpPoll,
            enabled: true,
            params: HashMap::new(),
        };

        let result = manager.spawn_source("test-http".to_string(), config).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_spawn_webhook_source_not_implemented() {
        let (tx, _rx) = mpsc::channel(100);
        let manager = SourceManager::new(tx);

        let config = SourceConfig {
            source_type: SourceType::Webhook,
            enabled: true,
            params: HashMap::new(),
        };

        let result = manager.spawn_source("test-webhook".to_string(), config).await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CoreError::Source(_)));
    }

    #[tokio::test]
    async fn test_stop_nonexistent_source() {
        let (tx, _rx) = mpsc::channel(100);
        let manager = SourceManager::new(tx);

        let result = manager.stop_source("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_stop_all_empty() {
        let (tx, _rx) = mpsc::channel(100);
        let manager = SourceManager::new(tx);

        let result = manager.stop_all().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_health_check_empty() {
        let (tx, _rx) = mpsc::channel(100);
        let manager = SourceManager::new(tx);

        let health = manager.health_check().await.unwrap();
        assert!(health.is_empty());
    }

    #[test]
    fn test_source_handle_creation() {
        let (tx, _rx) = tokio::sync::oneshot::channel();
        let handle = SourceHandle {
            source_id: "test".to_string(),
            source_type: SourceType::Mqtt,
            stop_tx: Some(tx),
        };

        assert_eq!(handle.source_id, "test");
        assert_eq!(handle.source_type, SourceType::Mqtt);
    }

    #[tokio::test]
    async fn test_http_config_extraction() {
        let (tx, _rx) = mpsc::channel(100);
        let manager = SourceManager::new(tx);

        let mut params = HashMap::new();
        params.insert(
            "base_url_template".to_string(),
            json!("http://example.com/{SERIAL}"),
        );
        params.insert(
            "sensors".to_string(),
            json!([
                {
                    "serial_number": "TEST123",
                    "url": "http://example.com/TEST123"
                }
            ]),
        );

        let config = SourceConfig {
            source_type: SourceType::HttpPoll,
            enabled: true,
            params,
        };

        // This will fail to connect but should parse config correctly
        let result = manager.spawn_source("test-http".to_string(), config).await;
        // We expect it to spawn successfully (network errors happen later)
        assert!(result.is_ok());
    }
}
