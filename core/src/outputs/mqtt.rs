//! MQTT Output Sink (DP-012 Phase 3)

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use rumqttc::{AsyncClient, EventLoop, MqttOptions, QoS};
use serde::Deserialize;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use super::{OutputError, OutputSink};
use crate::processors::ProcessorOutput;

/// Configuration for MQTT output sink
#[derive(Debug, Clone, Deserialize)]
pub struct MqttOutputConfig {
    #[serde(default = "default_name")]
    pub name: String,
    #[serde(default = "default_broker")]
    pub broker_url: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_client_id")]
    pub client_id: String,
    #[serde(default = "default_topic_prefix")]
    pub topic_prefix: String,
    #[serde(default)]
    pub qos: u8,
    #[serde(default = "default_keep_alive")]
    pub keep_alive_secs: u64,
}

fn default_name() -> String {
    "mqtt-output".to_string()
}

fn default_broker() -> String {
    "localhost".to_string()
}

fn default_port() -> u16 {
    1883
}

fn default_client_id() -> String {
    format!("ndp-output-{}", uuid::Uuid::new_v4())
}

fn default_topic_prefix() -> String {
    "ndp/outputs".to_string()
}

fn default_keep_alive() -> u64 {
    30
}

impl Default for MqttOutputConfig {
    fn default() -> Self {
        Self {
            name: default_name(),
            broker_url: default_broker(),
            port: default_port(),
            client_id: default_client_id(),
            topic_prefix: default_topic_prefix(),
            qos: 0,
            keep_alive_secs: default_keep_alive(),
        }
    }
}

/// MQTT Output Sink
pub struct MqttOutput {
    config: MqttOutputConfig,
    client: Option<AsyncClient>,
    is_connected: Arc<Mutex<bool>>,
}

impl MqttOutput {
    pub fn new(config: MqttOutputConfig) -> Self {
        Self {
            config,
            client: None,
            is_connected: Arc::new(Mutex::new(false)),
        }
    }

    pub async fn connect(&mut self) -> Result<(), OutputError> {
        let mut mqtt_options = MqttOptions::new(
            &self.config.client_id,
            &self.config.broker_url,
            self.config.port,
        );
        mqtt_options.set_keep_alive(Duration::from_secs(self.config.keep_alive_secs));

        let (client, eventloop) = AsyncClient::new(mqtt_options, 100);
        self.client = Some(client);

        self.spawn_connection_handler(eventloop);

        Ok(())
    }

    fn spawn_connection_handler(&self, mut eventloop: EventLoop) {
        let is_connected = self.is_connected.clone();
        let client_id = self.config.client_id.clone();

        tokio::spawn(async move {
            loop {
                match eventloop.poll().await {
                    Ok(rumqttc::Event::Incoming(rumqttc::Packet::ConnAck(_))) => {
                        *is_connected.lock().await = true;
                        info!(client_id = %client_id, "MqttOutput connected");
                    }
                    Ok(rumqttc::Event::Incoming(rumqttc::Packet::Disconnect)) => {
                        *is_connected.lock().await = false;
                        warn!(client_id = %client_id, "MqttOutput disconnected");
                    }
                    Err(e) => {
                        *is_connected.lock().await = false;
                        debug!(error = %e, "MQTT connection error");
                    }
                    _ => {}
                }
            }
        });
    }

    pub async fn publish(&self, topic: &str, payload: &[u8]) -> Result<(), OutputError> {
        let client = self.client.as_ref().ok_or(OutputError::NotConnected)?;

        let qos = match self.config.qos {
            0 => QoS::AtMostOnce,
            1 => QoS::AtLeastOnce,
            _ => QoS::ExactlyOnce,
        };

        client
            .publish(topic, qos, false, payload)
            .await
            .map_err(|e| OutputError::SendFailed(e.to_string()))
    }

    fn build_topic(&self, output: &ProcessorOutput) -> String {
        let suffix = match output {
            ProcessorOutput::Alert(alert) => format!("alerts/{}", alert.severity),
            ProcessorOutput::Metric(metric) => format!("metrics/{}", metric.name),
            ProcessorOutput::Event(event) => format!("events/{}", event.event_type),
        };
        format!("{}/{}", self.config.topic_prefix, suffix)
    }
}

#[async_trait]
impl OutputSink for MqttOutput {
    fn name(&self) -> String {
        self.config.name.clone()
    }

    async fn send(&self, output: &ProcessorOutput) -> Result<(), OutputError> {
        let topic = self.build_topic(output);

        let payload = serde_json::to_vec(output)
            .map_err(|e| OutputError::SerializationFailed(e.to_string()))?;

        self.publish(&topic, &payload).await?;

        debug!(sink = %self.config.name, topic = %topic, "Output published");

        Ok(())
    }

    async fn health_check(&self) -> Result<(), OutputError> {
        if *self.is_connected.lock().await {
            Ok(())
        } else {
            Err(OutputError::NotConnected)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = MqttOutputConfig::default();
        assert_eq!(config.name, "mqtt-output");
        assert_eq!(config.broker_url, "localhost");
        assert_eq!(config.port, 1883);
    }

    #[test]
    fn test_output_creation() {
        let output = MqttOutput::new(MqttOutputConfig::default());
        assert_eq!(output.name(), "mqtt-output");
    }

    #[tokio::test]
    async fn test_health_check_not_connected() {
        let output = MqttOutput::new(MqttOutputConfig::default());
        assert!(matches!(
            output.health_check().await,
            Err(OutputError::NotConnected)
        ));
    }
}
