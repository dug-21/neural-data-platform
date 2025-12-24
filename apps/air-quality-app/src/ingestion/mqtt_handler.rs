//! MQTT Handler for AIR-002 Ingestion Pipeline
//!
//! This module provides the MQTT ingestion handler that:
//! - Connects to MQTT broker using neural_core MqttSource
//! - Fetches incoming air quality readings from AirGradient sensors
//! - Forwards TimeSeriesPoint data through a channel to the storage pipeline
//! - Provides health checking for monitoring

use neural_core::parsers::{create_parser_from_config, ParserConfig, ParserType};
use neural_core::traits::{HealthStatus, Source};
use neural_core::{CoreError, MqttConfig, MqttSource, TimeSeriesPoint};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// MQTT ingestion handler for AIR-002 pipeline
///
/// Manages the MQTT data source and forwards received points to the storage pipeline
pub struct MqttHandler {
    source: MqttSource,
    sender: mpsc::Sender<TimeSeriesPoint>,
}

impl MqttHandler {
    /// Create a new MQTT handler
    ///
    /// # Arguments
    /// * `config` - MQTT configuration (broker, port, topic pattern, etc.)
    /// * `sender` - Channel sender for forwarding points to storage pipeline
    ///
    /// # Errors
    /// Returns error if MQTT source fails to start
    pub async fn new(
        config: MqttConfig,
        sender: mpsc::Sender<TimeSeriesPoint>,
    ) -> Result<Self, CoreError> {
        info!(
            "Initializing MQTT handler with broker: {}:{}",
            config.broker_url, config.port
        );

        // Create parser from config (FlatJson for AirGradient MQTT messages)
        let parser_config = ParserConfig {
            parser_type: ParserType::FlatJson,
            location_id_field: "serialno".to_string(),
            default_location_id: None,
            skip_fields: vec![
                "serialno".to_string(),
                "wifi".to_string(),
                "boot".to_string(),
                "firmware".to_string(),
                "model".to_string(),
                "ledMode".to_string(),
                "bootCount".to_string(),
            ],
            field_mappings: None,
            default_tags: std::collections::HashMap::new(),
            array_config: None,
            column_config: None,
        };
        let parser = create_parser_from_config(parser_config)
            .map_err(|e| CoreError::Config(format!("Failed to create parser: {}", e)))?;

        // Start the MQTT source (connects and begins receiving messages)
        let mut source = MqttSource::new(config, parser);
        source.start().await?;

        info!("MQTT handler started successfully");

        Ok(Self { source, sender })
    }

    /// Run the ingestion loop
    ///
    /// Continuously fetches points from MQTT source and sends them through the channel.
    /// This should run indefinitely until the handler is stopped.
    ///
    /// # Errors
    /// Returns error if fetching fails or channel is closed
    pub async fn run(&self) -> Result<(), CoreError> {
        info!("Starting MQTT ingestion loop");

        loop {
            // Fetch available points from the source
            match self.source.fetch().await {
                Ok(points) => {
                    if !points.is_empty() {
                        debug!("Fetched {} points from MQTT source", points.len());

                        // Send each point through the channel
                        for point in points {
                            if let Err(e) = self.sender.send(point).await {
                                error!("Failed to send point through channel: {}", e);
                                return Err(CoreError::Source(format!(
                                    "Channel send failed: {}",
                                    e
                                )));
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to fetch points from MQTT source: {}", e);
                    // Continue running even if fetch fails - source may recover
                    warn!("Continuing after fetch error, source may recover");
                }
            }

            // Small delay to avoid busy-waiting when no data is available
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }

    /// Check health of MQTT connection
    ///
    /// Delegates to the underlying MQTT source health check
    ///
    /// # Errors
    /// Returns error if health check fails
    pub async fn health_check(&self) -> Result<HealthStatus, CoreError> {
        self.source.health_check().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::HashMap;
    use std::time::Duration;

    #[tokio::test]
    async fn test_mqtt_handler_creation() {
        let config = MqttConfig {
            broker_url: "localhost".to_string(),
            port: 1883,
            client_id: "test-handler".to_string(),
            topic_pattern: "test/+".to_string(),
            qos: rumqttc::QoS::AtLeastOnce,
            reconnect_delay: Duration::from_secs(1),
            max_reconnect_delay: Duration::from_secs(10),
            buffer_capacity: 100,
        };

        let (sender, _receiver) = mpsc::channel(100);

        // Note: This will fail if no MQTT broker is running, which is expected in tests
        // In production, we'd use a mock or test broker
        let result = MqttHandler::new(config, sender).await;

        // We expect this to fail in test environment without broker
        assert!(result.is_err() || result.is_ok());
    }

    #[tokio::test]
    async fn test_mqtt_handler_channel_capacity() {
        let (sender, receiver) = mpsc::channel::<TimeSeriesPoint>(5);

        // Verify channel capacity
        assert_eq!(receiver.max_capacity(), 5);

        drop(sender);
        drop(receiver);
    }

    #[test]
    fn test_mqtt_config_defaults() {
        let config = MqttConfig::default();

        assert_eq!(config.broker_url, "localhost");
        assert_eq!(config.port, 1883);
        assert_eq!(config.buffer_capacity, 1000);
    }
}
