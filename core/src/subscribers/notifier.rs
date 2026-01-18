//! EventNotifier for lightweight MQTT notifications (DP-012 Phase 3)
//!
//! This subscriber publishes lightweight notifications to MQTT when
//! RawDataPoint events are received. Unlike ProcessorSubscriber which runs
//! business logic, EventNotifier simply announces that data arrived.
//!
//! # Architecture
//!
//! ```text
//! EventBus (broadcast)
//!     |
//!     | RawDataPoint events
//!     v
//! EventNotifier
//!     |
//!     +-- accepts_stream()?
//!     |       |
//!     |       `-- Skip if false
//!     |
//!     +-- build_notification(event)
//!     |
//!     `-- mqtt_client.publish(topic, payload)
//!             |
//!             `-- QoS 0 (fire-and-forget)
//! ```
//!
//! # Use Cases
//!
//! - Dashboard real-time updates
//! - External system triggers
//! - Data arrival monitoring
//! - Debug/development tracing
//!
//! # Design Notes
//!
//! - Fire-and-forget (QoS 0) for minimal latency
//! - Configurable topic patterns
//! - Optional payload filtering (exclude large fields)
//! - Rate limiting to prevent MQTT floods

use crate::traits::HealthStatus;
use crate::types::RawDataPoint;

use super::{Subscriber, SubscriberError};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rumqttc::{AsyncClient, EventLoop, MqttOptions, QoS};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, Mutex};
use tracing::{debug, error, info, warn};

/// Configuration for EventNotifier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventNotifierConfig {
    /// Unique identifier for this subscriber
    #[serde(default = "default_subscriber_id")]
    pub subscriber_id: String,

    /// Stream IDs to notify for (empty = all streams)
    #[serde(default)]
    pub stream_filter: Vec<String>,

    /// MQTT broker hostname
    #[serde(default = "default_broker")]
    pub broker_url: String,

    /// MQTT broker port
    #[serde(default = "default_port")]
    pub port: u16,

    /// MQTT client ID
    #[serde(default = "default_client_id")]
    pub client_id: String,

    /// Topic prefix for notifications
    #[serde(default = "default_topic_prefix")]
    pub topic_prefix: String,

    /// Keep-alive interval in seconds
    #[serde(default = "default_keep_alive")]
    pub keep_alive_secs: u64,

    /// Include raw_payload in notification (can be large)
    #[serde(default)]
    pub include_payload: bool,

    /// Fields to include from raw_payload (if include_payload is false)
    #[serde(default)]
    pub payload_fields: Vec<String>,
}

fn default_subscriber_id() -> String {
    "event-notifier".to_string()
}

fn default_broker() -> String {
    "localhost".to_string()
}

fn default_port() -> u16 {
    1883
}

fn default_client_id() -> String {
    format!("ndp-notifier-{}", uuid::Uuid::new_v4())
}

fn default_topic_prefix() -> String {
    "ndp/events".to_string()
}

fn default_keep_alive() -> u64 {
    30
}

impl Default for EventNotifierConfig {
    fn default() -> Self {
        Self {
            subscriber_id: default_subscriber_id(),
            stream_filter: Vec::new(),
            broker_url: default_broker(),
            port: default_port(),
            client_id: default_client_id(),
            topic_prefix: default_topic_prefix(),
            keep_alive_secs: default_keep_alive(),
            include_payload: false,
            payload_fields: Vec::new(),
        }
    }
}

/// Notification payload sent via MQTT
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventNotification {
    /// Stream/source identifier
    pub stream_id: String,

    /// Event timestamp
    pub timestamp: DateTime<Utc>,

    /// Platform device ID if available
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ndp_id: Option<String>,

    /// Notification timestamp (when notification was created)
    pub notified_at: DateTime<Utc>,

    /// Optional payload excerpt
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
}

/// Subscriber state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventNotifierState {
    Idle,
    Connecting,
    Running,
    Stopped,
}

/// EventNotifier publishes lightweight MQTT notifications
pub struct EventNotifier {
    config: EventNotifierConfig,
    client: Option<AsyncClient>,
    is_connected: Arc<Mutex<bool>>,
    state: EventNotifierState,
    events_received: u64,
    notifications_sent: u64,
    notification_errors: u64,
    last_error: Option<String>,
    shutdown_signal: Option<tokio::sync::oneshot::Sender<()>>,
}

impl EventNotifier {
    /// Create a new EventNotifier
    pub fn new(config: EventNotifierConfig) -> Self {
        Self {
            config,
            client: None,
            is_connected: Arc::new(Mutex::new(false)),
            state: EventNotifierState::Idle,
            events_received: 0,
            notifications_sent: 0,
            notification_errors: 0,
            last_error: None,
            shutdown_signal: None,
        }
    }

    /// Get current state
    pub fn state(&self) -> EventNotifierState {
        self.state
    }

    /// Connect to MQTT broker
    pub async fn connect(&mut self) -> Result<(), SubscriberError> {
        self.state = EventNotifierState::Connecting;

        let mut mqtt_options = MqttOptions::new(
            &self.config.client_id,
            &self.config.broker_url,
            self.config.port,
        );
        mqtt_options.set_keep_alive(Duration::from_secs(self.config.keep_alive_secs));

        let (client, eventloop) = AsyncClient::new(mqtt_options, 100);
        self.client = Some(client);

        self.spawn_connection_handler(eventloop);

        info!(
            client_id = %self.config.client_id,
            broker = %self.config.broker_url,
            "EventNotifier connecting to MQTT"
        );

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
                        info!(client_id = %client_id, "EventNotifier connected");
                    }
                    Ok(rumqttc::Event::Incoming(rumqttc::Packet::Disconnect)) => {
                        *is_connected.lock().await = false;
                        warn!(client_id = %client_id, "EventNotifier disconnected");
                    }
                    Err(e) => {
                        *is_connected.lock().await = false;
                        debug!(error = %e, "MQTT connection error");
                        // Sleep briefly before reconnect attempt
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                    _ => {}
                }
            }
        });
    }

    /// Build notification from raw event
    fn build_notification(&self, raw: &RawDataPoint) -> EventNotification {
        let payload = if self.config.include_payload {
            Some(raw.raw_payload.clone())
        } else if !self.config.payload_fields.is_empty() {
            // Extract specific fields
            let mut extracted = serde_json::Map::new();
            if let Some(obj) = raw.raw_payload.as_object() {
                for field in &self.config.payload_fields {
                    if let Some(value) = obj.get(field) {
                        extracted.insert(field.clone(), value.clone());
                    }
                }
            }
            if extracted.is_empty() {
                None
            } else {
                Some(serde_json::Value::Object(extracted))
            }
        } else {
            None
        };

        EventNotification {
            stream_id: raw.source_id.clone(),
            timestamp: raw.timestamp,
            ndp_id: raw.ndp_id.clone(),
            notified_at: Utc::now(),
            payload,
        }
    }

    /// Build topic for notification
    fn build_topic(&self, raw: &RawDataPoint) -> String {
        format!("{}/{}", self.config.topic_prefix, raw.source_id)
    }

    /// Publish notification
    async fn publish_notification(&mut self, raw: &RawDataPoint) -> Result<(), SubscriberError> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| SubscriberError::Internal("Not connected".to_string()))?;

        let notification = self.build_notification(raw);
        let topic = self.build_topic(raw);

        let payload = serde_json::to_vec(&notification).map_err(|e| {
            SubscriberError::ProcessingError(format!("Serialization failed: {}", e))
        })?;

        // Fire-and-forget (QoS 0)
        client
            .publish(&topic, QoS::AtMostOnce, false, payload)
            .await
            .map_err(|e| SubscriberError::ProcessingError(format!("Publish failed: {}", e)))?;

        debug!(
            topic = %topic,
            stream = %raw.source_id,
            "Notification published"
        );

        Ok(())
    }

    /// Process a single event
    async fn process_event(&mut self, raw: Arc<RawDataPoint>) -> Result<(), SubscriberError> {
        // Check stream filter
        if !self.accepts_stream(&raw.source_id) {
            return Ok(());
        }

        self.events_received += 1;

        // Publish notification
        match self.publish_notification(&raw).await {
            Ok(()) => {
                self.notifications_sent += 1;
            }
            Err(e) => {
                self.notification_errors += 1;
                self.last_error = Some(e.to_string());
                // Log but continue - notifications are best-effort
                warn!(error = %e, "Notification failed");
            }
        }

        Ok(())
    }
}

#[async_trait]
impl Subscriber for EventNotifier {
    fn id(&self) -> &str {
        &self.config.subscriber_id
    }

    async fn start(
        &mut self,
        mut receiver: broadcast::Receiver<Arc<RawDataPoint>>,
    ) -> Result<(), SubscriberError> {
        info!(id = %self.id(), "Starting EventNotifier");

        // Connect to MQTT if not already connected
        if self.client.is_none() {
            self.connect().await?;
        }

        self.state = EventNotifierState::Running;

        // Create shutdown channel
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();
        self.shutdown_signal = Some(shutdown_tx);

        // Event processing loop
        loop {
            tokio::select! {
                // Check for shutdown signal
                _ = &mut shutdown_rx => {
                    info!(id = %self.id(), "Shutdown signal received");
                    break;
                }

                // Process events
                result = receiver.recv() => {
                    match result {
                        Ok(raw_point) => {
                            if let Err(e) = self.process_event(raw_point).await {
                                error!(error = %e, "Error processing event");
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!(lagged = n, "Receiver lagged, missed events");
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            info!("Event bus closed");
                            break;
                        }
                    }
                }
            }
        }

        self.state = EventNotifierState::Stopped;

        info!(
            id = %self.id(),
            events_received = self.events_received,
            notifications_sent = self.notifications_sent,
            notification_errors = self.notification_errors,
            "EventNotifier stopped"
        );

        Ok(())
    }

    async fn stop(&mut self) -> Result<(), SubscriberError> {
        info!(id = %self.id(), "Stopping EventNotifier");

        // Signal shutdown
        if let Some(tx) = self.shutdown_signal.take() {
            let _ = tx.send(());
        }

        // Disconnect client
        if let Some(client) = self.client.take() {
            if let Err(e) = client.disconnect().await {
                debug!(error = %e, "Error disconnecting MQTT client");
            }
        }

        self.state = EventNotifierState::Stopped;
        Ok(())
    }

    fn accepts_stream(&self, stream_id: &str) -> bool {
        if self.config.stream_filter.is_empty() {
            true
        } else {
            self.config.stream_filter.iter().any(|s| s == stream_id)
        }
    }

    async fn health_check(&self) -> HealthStatus {
        let mut details = HashMap::new();

        let is_connected = *self.is_connected.lock().await;
        let healthy = is_connected && self.state == EventNotifierState::Running;

        let message = if healthy {
            "Healthy".to_string()
        } else if !is_connected {
            "Not connected to MQTT".to_string()
        } else {
            "Not running".to_string()
        };

        details.insert("state".to_string(), format!("{:?}", self.state));
        details.insert("connected".to_string(), is_connected.to_string());
        details.insert("broker".to_string(), self.config.broker_url.clone());
        details.insert(
            "events_received".to_string(),
            self.events_received.to_string(),
        );
        details.insert(
            "notifications_sent".to_string(),
            self.notifications_sent.to_string(),
        );
        details.insert(
            "notification_errors".to_string(),
            self.notification_errors.to_string(),
        );

        if let Some(ref err) = self.last_error {
            details.insert("last_error".to_string(), err.clone());
        }

        HealthStatus {
            healthy,
            message,
            details,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create_test_config() -> EventNotifierConfig {
        EventNotifierConfig {
            subscriber_id: "test-notifier".to_string(),
            ..Default::default()
        }
    }

    fn create_raw_point(stream_id: &str) -> RawDataPoint {
        RawDataPoint::new(stream_id, json!({"pm25": 12.5, "temperature": 22.3}))
            .with_ndp_id("device-001")
    }

    #[test]
    fn test_config_default() {
        let config = EventNotifierConfig::default();
        assert_eq!(config.subscriber_id, "event-notifier");
        assert_eq!(config.broker_url, "localhost");
        assert_eq!(config.port, 1883);
        assert!(!config.include_payload);
    }

    #[test]
    fn test_notifier_new() {
        let config = create_test_config();
        let notifier = EventNotifier::new(config);

        assert_eq!(notifier.id(), "test-notifier");
        assert_eq!(notifier.state(), EventNotifierState::Idle);
        assert_eq!(notifier.events_received, 0);
    }

    #[test]
    fn test_accepts_stream_no_filter() {
        let config = EventNotifierConfig::default();
        let notifier = EventNotifier::new(config);

        assert!(notifier.accepts_stream("any-stream"));
        assert!(notifier.accepts_stream("another-stream"));
    }

    #[test]
    fn test_accepts_stream_with_filter() {
        let config = EventNotifierConfig {
            stream_filter: vec!["air-quality".to_string()],
            ..Default::default()
        };
        let notifier = EventNotifier::new(config);

        assert!(notifier.accepts_stream("air-quality"));
        assert!(!notifier.accepts_stream("outdoor-weather"));
    }

    #[test]
    fn test_build_notification_no_payload() {
        let config = EventNotifierConfig::default();
        let notifier = EventNotifier::new(config);

        let raw = create_raw_point("air-quality");
        let notification = notifier.build_notification(&raw);

        assert_eq!(notification.stream_id, "air-quality");
        assert_eq!(notification.ndp_id, Some("device-001".to_string()));
        assert!(notification.payload.is_none());
    }

    #[test]
    fn test_build_notification_with_payload() {
        let config = EventNotifierConfig {
            include_payload: true,
            ..Default::default()
        };
        let notifier = EventNotifier::new(config);

        let raw = create_raw_point("air-quality");
        let notification = notifier.build_notification(&raw);

        assert!(notification.payload.is_some());
        let payload = notification.payload.unwrap();
        assert_eq!(payload.get("pm25").unwrap().as_f64(), Some(12.5));
    }

    #[test]
    fn test_build_notification_with_selected_fields() {
        let config = EventNotifierConfig {
            payload_fields: vec!["pm25".to_string()],
            ..Default::default()
        };
        let notifier = EventNotifier::new(config);

        let raw = create_raw_point("air-quality");
        let notification = notifier.build_notification(&raw);

        assert!(notification.payload.is_some());
        let payload = notification.payload.unwrap();
        assert!(payload.get("pm25").is_some());
        assert!(payload.get("temperature").is_none());
    }

    #[test]
    fn test_build_topic() {
        let config = EventNotifierConfig {
            topic_prefix: "ndp/test".to_string(),
            ..Default::default()
        };
        let notifier = EventNotifier::new(config);

        let raw = create_raw_point("air-quality");
        let topic = notifier.build_topic(&raw);

        assert_eq!(topic, "ndp/test/air-quality");
    }

    #[tokio::test]
    async fn test_health_check_idle() {
        let config = create_test_config();
        let notifier = EventNotifier::new(config);

        let status = notifier.health_check().await;
        assert!(!status.healthy);
        assert!(status.details.contains_key("connected"));
    }
}
