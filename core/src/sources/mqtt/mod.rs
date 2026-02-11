//! MQTT-based data source implementation
//!
//! Provides real-time data ingestion from MQTT brokers with:
//! - Auto-reconnect with exponential backoff
//! - Backpressure handling with bounded queues
//! - Topic pattern substitution for multiple sensors
//! - Multi-subscription support with per-subscription parser configuration

mod router;
mod subscription;

// Re-export subscription types
pub use subscription::{SubscriptionConfig, SubscriptionError};
// Re-export router types
pub use router::{mqtt_pattern_to_regex, RouteEntry, RouterError, TopicRouter};
// ConfigError is re-exported below after its definition

use async_trait::async_trait;
use chrono::Utc;
use rumqttc::{AsyncClient, Event, EventLoop, MqttOptions, Packet, QoS};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

use crate::error::{CoreError, CoreResult};
use crate::parsers::{ParseContext, Parser};
use crate::traits::{HealthStatus, RawSource, Source, TimeSeriesPoint};
use crate::types::raw_data_point::RawDataPoint;

/// Configuration for MQTT source
///
/// Supports both new multi-subscription format and legacy single-topic format.
///
/// # New Format (Recommended)
///
/// ```yaml
/// broker_url: "mosquitto"
/// port: 1883
/// subscriptions:
///   - stream_id: air-quality
///     topic_pattern: "airgradient/readings/+"
///   - stream_id: homeassistant
///     topic_pattern: "homeassistant/+/+/state"
/// ```
///
/// # Legacy Format (Deprecated)
///
/// ```yaml
/// broker_url: "mosquitto"
/// port: 1883
/// topic_pattern: "airgradient/readings/{SERIAL_NUMBER}"
/// ```
#[derive(Debug, Clone)]
pub struct MqttConfig {
    pub broker_url: String,
    pub port: u16,
    pub client_id: String,
    /// Legacy topic pattern - deprecated, use subscriptions instead
    #[deprecated(since = "0.2.0", note = "Use subscriptions field instead")]
    pub topic_pattern: Option<String>,
    /// New multi-subscription support
    pub subscriptions: Vec<SubscriptionConfig>,
    pub qos: QoS,
    pub reconnect_delay: Duration,
    pub max_reconnect_delay: Duration,
    pub buffer_capacity: usize,
    /// Default stream ID for legacy topic_pattern
    pub default_stream_id: String,
}

/// Errors from MqttConfig validation
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigError {
    NoSubscriptions,
    DuplicateStreamId(String),
    InvalidSubscription { stream_id: String, error: String },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoSubscriptions => write!(
                f,
                "no subscriptions configured (use subscriptions array or legacy topic_pattern)"
            ),
            Self::DuplicateStreamId(id) => write!(f, "duplicate stream_id: {}", id),
            Self::InvalidSubscription { stream_id, error } => {
                write!(f, "invalid subscription '{}': {}", stream_id, error)
            }
        }
    }
}

impl std::error::Error for ConfigError {}

impl MqttConfig {
    /// Get effective subscriptions, handling both new and legacy formats.
    ///
    /// This method provides backward compatibility:
    /// - If `subscriptions` is non-empty, returns those
    /// - If `topic_pattern` is set (legacy), converts to a subscription
    /// - Logs deprecation warning for legacy format
    #[allow(deprecated)]
    pub fn get_subscriptions(&self) -> Vec<SubscriptionConfig> {
        if !self.subscriptions.is_empty() {
            return self.subscriptions.clone();
        }

        // Legacy fallback
        if let Some(ref pattern) = self.topic_pattern {
            warn!(
                topic_pattern = %pattern,
                "Using deprecated topic_pattern field. Migrate to subscriptions array."
            );

            // Convert {SERIAL_NUMBER} placeholder to MQTT wildcard
            let mqtt_pattern = pattern.replace("{SERIAL_NUMBER}", "+");

            vec![SubscriptionConfig::new(
                &self.default_stream_id,
                mqtt_pattern,
            )]
        } else {
            Vec::new()
        }
    }

    /// Validate the configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No subscriptions are configured (neither subscriptions nor topic_pattern)
    /// - Stream IDs are not unique
    /// - Any subscription is invalid
    #[allow(deprecated)]
    pub fn validate(&self) -> Result<(), ConfigError> {
        let subs = self.get_subscriptions();

        if subs.is_empty() {
            return Err(ConfigError::NoSubscriptions);
        }

        // Check for duplicate stream IDs
        let mut seen_ids = std::collections::HashSet::new();
        for sub in &subs {
            if !seen_ids.insert(&sub.stream_id) {
                return Err(ConfigError::DuplicateStreamId(sub.stream_id.clone()));
            }

            // Validate each subscription
            sub.validate()
                .map_err(|e| ConfigError::InvalidSubscription {
                    stream_id: sub.stream_id.clone(),
                    error: e.to_string(),
                })?;
        }

        Ok(())
    }
}

impl Default for MqttConfig {
    #[allow(deprecated)]
    fn default() -> Self {
        Self {
            broker_url: "localhost".to_string(),
            port: 1883,
            client_id: "neural-data-platform".to_string(),
            topic_pattern: Some("airgradient/readings/{SERIAL_NUMBER}".to_string()),
            subscriptions: Vec::new(),
            qos: QoS::AtLeastOnce,
            reconnect_delay: Duration::from_secs(1),
            max_reconnect_delay: Duration::from_secs(30),
            buffer_capacity: 1000,
            default_stream_id: "default".to_string(),
        }
    }
}

/// MQTT data source
pub struct MqttSource {
    config: MqttConfig,
    parser: Arc<dyn Parser + Send + Sync>,
    client: Option<AsyncClient>,
    #[allow(dead_code)]
    receiver: Arc<Mutex<mpsc::Receiver<TimeSeriesPoint>>>,
    #[allow(dead_code)]
    sender: mpsc::Sender<TimeSeriesPoint>,
    is_running: Arc<Mutex<bool>>,
    connection_healthy: Arc<Mutex<bool>>,
    cached_points: Arc<Mutex<Vec<TimeSeriesPoint>>>,
    /// DP-004: Cache for raw JSON payloads (Bronze layer)
    cached_raw_points: Arc<Mutex<Vec<RawDataPoint>>>,
    /// DP-004: Stream identifier for source_id generation
    stream_id: Option<String>,
    /// AIR-009: Stable source identifier
    ndp_id: Option<String>,
    /// AIR-009: Mutable context attributes
    context: Option<serde_json::Value>,
}

impl MqttSource {
    /// Create a new MQTT source with injected parser
    pub fn new(config: MqttConfig, parser: Box<dyn Parser + Send + Sync>) -> Self {
        Self::with_context(config, parser, None, None)
    }

    /// Create a new MQTT source with ndp_id and context (AIR-009)
    pub fn with_context(
        config: MqttConfig,
        parser: Box<dyn Parser + Send + Sync>,
        ndp_id: Option<String>,
        context: Option<serde_json::Value>,
    ) -> Self {
        Self::with_raw_config(config, parser, None, ndp_id, context)
    }

    /// Create a new MQTT source with full raw data configuration (DP-004)
    ///
    /// # Arguments
    /// * `config` - MQTT configuration
    /// * `parser` - Parser for TimeSeriesPoint extraction
    /// * `stream_id` - Stream identifier for source_id generation (e.g., "air-quality")
    /// * `ndp_id` - Stable source identifier from configuration
    /// * `context` - Mutable context metadata
    pub fn with_raw_config(
        config: MqttConfig,
        parser: Box<dyn Parser + Send + Sync>,
        stream_id: Option<String>,
        ndp_id: Option<String>,
        context: Option<serde_json::Value>,
    ) -> Self {
        let (sender, receiver) = mpsc::channel(config.buffer_capacity);

        Self {
            config,
            parser: Arc::from(parser),
            client: None,
            receiver: Arc::new(Mutex::new(receiver)),
            sender,
            is_running: Arc::new(Mutex::new(false)),
            connection_healthy: Arc::new(Mutex::new(false)),
            cached_points: Arc::new(Mutex::new(Vec::new())),
            cached_raw_points: Arc::new(Mutex::new(Vec::new())),
            stream_id,
            ndp_id,
            context,
        }
    }

    /// Get the generated source_id for this source (DP-004)
    ///
    /// Format: "{stream_id}-Mqtt" or "unknown-Mqtt" if stream_id not set
    pub fn source_id(&self) -> String {
        use crate::sources::generate_source_id;
        use crate::types::stream_config::SourceType;

        let stream_id = self.stream_id.as_deref().unwrap_or("unknown");
        generate_source_id(stream_id, &SourceType::Mqtt)
    }

    /// Parse MQTT payload into time series points using injected parser
    #[allow(dead_code)]
    fn parse_payload(&self, payload: &[u8]) -> CoreResult<Vec<TimeSeriesPoint>> {
        let json: Value = serde_json::from_slice(payload)
            .map_err(|e| CoreError::Source(format!("Failed to parse MQTT payload: {}", e)))?;

        let timestamp = Utc::now();
        let parse_context = ParseContext::new(self.ndp_id.clone(), self.context.clone());
        self.parser
            .parse_with_context(&json, timestamp, &parse_context)
    }

    /// Create a new connection and return the event loop
    fn create_connection(config: &MqttConfig) -> CoreResult<(AsyncClient, EventLoop)> {
        let mut mqtt_options = MqttOptions::new(&config.client_id, &config.broker_url, config.port);
        mqtt_options.set_keep_alive(Duration::from_secs(30));

        let (client, event_loop) = AsyncClient::new(mqtt_options, config.buffer_capacity);
        Ok((client, event_loop))
    }

    /// Process MQTT events - runs in a spawned task
    #[allow(clippy::too_many_arguments)]
    async fn process_events(
        config: MqttConfig,
        router: TopicRouter,
        parser: Arc<dyn Parser + Send + Sync>,
        mut event_loop: EventLoop,
        client: AsyncClient,
        cached_points: Arc<Mutex<Vec<TimeSeriesPoint>>>,
        cached_raw_points: Arc<Mutex<Vec<RawDataPoint>>>,
        is_running: Arc<Mutex<bool>>,
        connection_healthy: Arc<Mutex<bool>>,
        source_id: String,
        ndp_id: Option<String>,
        context: Option<serde_json::Value>,
    ) -> CoreResult<()> {
        let mut reconnect_attempt = 0_u32;

        // Subscribe to all topic patterns from router
        for pattern in router.topic_patterns() {
            client.subscribe(pattern, config.qos).await.map_err(|e| {
                CoreError::Source(format!("Failed to subscribe to {}: {}", pattern, e))
            })?;
            info!("Subscribed to topic pattern: {}", pattern);
        }

        while *is_running.lock().await {
            match event_loop.poll().await {
                Ok(Event::Incoming(Packet::Publish(publish))) => {
                    debug!("Received MQTT message on topic: {}", publish.topic);
                    reconnect_attempt = 0;

                    // Route topic to subscription
                    match router.route(&publish.topic) {
                        Some(route) => {
                            let timestamp = Utc::now();

                            // AIR-012: Try JSON first, fall back to raw text wrapper
                            // This enables Home Assistant integration with plain text payloads
                            let json = match serde_json::from_slice::<Value>(&publish.payload) {
                                Ok(json) => json,
                                Err(_) => {
                                    // Wrap raw text payload as JSON for downstream processing
                                    // Format: { "_raw_text": "on", "_topic": "homeassistant/..." }
                                    let raw_text = String::from_utf8_lossy(&publish.payload);
                                    debug!(
                                        topic = %publish.topic,
                                        raw_text = %raw_text,
                                        "Wrapping non-JSON payload as JSON object"
                                    );
                                    serde_json::json!({
                                        "_raw_text": raw_text.to_string(),
                                        "_topic": publish.topic.clone()
                                    })
                                }
                            };

                            // DP-004: Cache raw payload for Bronze layer (RawSource)
                            {
                                let mut raw_point =
                                    RawDataPoint::new(source_id.clone(), json.clone())
                                        .with_timestamp(timestamp);

                                // AIR-012: Dynamic ndp_id extraction from topic for event-oriented streams
                                // If ndp_id_topic_segment is configured, extract from topic path.
                                // Otherwise, fall back to static ndp_id from config.
                                let effective_ndp_id = route
                                    .extract_ndp_id_from_topic(&publish.topic)
                                    .or_else(|| ndp_id.clone());

                                if let Some(id) = effective_ndp_id {
                                    raw_point = raw_point.with_ndp_id(id);
                                }
                                if let Some(ref ctx) = context {
                                    raw_point = raw_point.with_context(ctx.clone());
                                }

                                let mut raw_cache = cached_raw_points.lock().await;
                                raw_cache.push(raw_point);
                            }

                            // Parse for Silver layer (legacy Source trait)
                            // AIR-012: Use dynamic ndp_id if extracted from topic
                            let effective_ndp_id_for_parse = route
                                .extract_ndp_id_from_topic(&publish.topic)
                                .or_else(|| ndp_id.clone());
                            let parse_context =
                                ParseContext::new(effective_ndp_id_for_parse, context.clone());
                            match parser.parse_with_context(&json, timestamp, &parse_context) {
                                Ok(mut points) => {
                                    // Tag points with stream_id and topic
                                    for point in &mut points {
                                        point.tags.insert(
                                            "stream_id".to_string(),
                                            route.stream_id.clone(),
                                        );
                                        point
                                            .tags
                                            .insert("topic".to_string(), publish.topic.clone());
                                    }
                                    // Add to cache for fetch()
                                    let mut cache = cached_points.lock().await;
                                    cache.extend(points);
                                }
                                Err(e) => {
                                    error!(
                                        topic = %publish.topic,
                                        stream_id = %route.stream_id,
                                        error = %e,
                                        "Failed to parse MQTT payload"
                                    );
                                }
                            }
                        }
                        None => {
                            // Dead letter - no route matched
                            warn!(
                                topic = %publish.topic,
                                "No route found for topic (dead letter)"
                            );
                        }
                    }
                }
                Ok(Event::Incoming(Packet::ConnAck(_))) => {
                    info!("Connected to MQTT broker");
                    *connection_healthy.lock().await = true;
                    reconnect_attempt = 0;
                }
                Ok(Event::Incoming(Packet::Disconnect)) => {
                    warn!("Disconnected from MQTT broker");
                    *connection_healthy.lock().await = false;

                    // Reconnect with exponential backoff
                    let delay = std::cmp::min(
                        config.reconnect_delay.as_secs() * 2_u64.pow(reconnect_attempt),
                        config.max_reconnect_delay.as_secs(),
                    );

                    warn!(
                        "Reconnecting to MQTT broker in {} seconds (attempt {})",
                        delay, reconnect_attempt
                    );

                    tokio::time::sleep(Duration::from_secs(delay)).await;
                    reconnect_attempt += 1;

                    // Create new connection
                    match Self::create_connection(&config) {
                        Ok((new_client, new_event_loop)) => {
                            event_loop = new_event_loop;
                            // Subscribe again to all patterns
                            for pattern in router.topic_patterns() {
                                if let Err(e) = new_client.subscribe(pattern, config.qos).await {
                                    error!(pattern = %pattern, "Failed to resubscribe: {}", e);
                                }
                            }
                            *connection_healthy.lock().await = true;
                        }
                        Err(e) => {
                            error!("Failed to reconnect: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("MQTT connection error: {}", e);
                    *connection_healthy.lock().await = false;

                    // Reconnect with exponential backoff
                    let delay = std::cmp::min(
                        config.reconnect_delay.as_secs() * 2_u64.pow(reconnect_attempt),
                        config.max_reconnect_delay.as_secs(),
                    );

                    warn!(
                        "Reconnecting to MQTT broker in {} seconds (attempt {})",
                        delay, reconnect_attempt
                    );

                    tokio::time::sleep(Duration::from_secs(delay)).await;
                    reconnect_attempt += 1;

                    // Create new connection
                    match Self::create_connection(&config) {
                        Ok((new_client, new_event_loop)) => {
                            event_loop = new_event_loop;
                            // Subscribe again to all patterns
                            for pattern in router.topic_patterns() {
                                if let Err(e) = new_client.subscribe(pattern, config.qos).await {
                                    error!(pattern = %pattern, "Failed to resubscribe: {}", e);
                                }
                            }
                            *connection_healthy.lock().await = true;
                        }
                        Err(e) => {
                            error!("Failed to reconnect: {}", e);
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Start the MQTT source
    pub async fn start(&mut self) -> CoreResult<()> {
        info!("Starting MQTT source: {}", self.config.client_id);

        *self.is_running.lock().await = true;

        // Create router from config subscriptions
        let subscriptions = self.config.get_subscriptions();
        let router = TopicRouter::new(subscriptions)
            .map_err(|e| CoreError::Config(format!("Failed to create topic router: {}", e)))?;

        // Create initial connection
        let (client, event_loop) = Self::create_connection(&self.config)?;
        self.client = Some(client.clone());

        // Clone data for background task
        let config = self.config.clone();
        let parser = self.parser.clone();
        let cached_points = self.cached_points.clone();
        let cached_raw_points = self.cached_raw_points.clone();
        let is_running = self.is_running.clone();
        let connection_healthy = self.connection_healthy.clone();
        let source_id = self.source_id();

        // Clone AIR-009 context for background task
        let ndp_id = self.ndp_id.clone();
        let context = self.context.clone();

        // Spawn background task for event processing
        tokio::spawn(async move {
            if let Err(e) = Self::process_events(
                config,
                router,
                parser,
                event_loop,
                client,
                cached_points,
                cached_raw_points,
                is_running,
                connection_healthy,
                source_id,
                ndp_id,
                context,
            )
            .await
            {
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

/// RawSource implementation for MqttSource (DP-004)
///
/// Returns raw JSON payloads as received from MQTT broker.
/// The payloads are cached during event processing and returned in batch.
#[async_trait]
impl RawSource for MqttSource {
    /// Fetch single raw data point from cache.
    ///
    /// Returns the first cached raw point, or error if cache is empty.
    /// For batch retrieval, use `fetch_raw_batch`.
    async fn fetch_raw(&self) -> CoreResult<RawDataPoint> {
        let mut cache = self.cached_raw_points.lock().await;
        cache
            .pop()
            .ok_or_else(|| CoreError::Source("No raw data points available in cache".to_string()))
    }

    /// Fetch all cached raw data points.
    ///
    /// Drains the entire raw payload cache and returns all points.
    /// Each point contains the exact JSON payload from the MQTT broker.
    async fn fetch_raw_batch(&self) -> CoreResult<Vec<RawDataPoint>> {
        let mut cache = self.cached_raw_points.lock().await;
        let points = cache.drain(..).collect();
        Ok(points)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::{FlatJsonParser, ParserConfig, ParserType};

    fn create_default_parser() -> Box<dyn Parser + Send + Sync> {
        let config = ParserConfig {
            parser_type: ParserType::FlatJson,
            location_id_field: "serialno".to_string(),
            default_location_id: Some("unknown".to_string()),
            skip_fields: vec![
                "serialno".to_string(),
                "firmware".to_string(),
                "model".to_string(),
                "ledMode".to_string(),
            ],
            field_mappings: None,
            array_config: None,
            column_config: None,
            default_tags: [("source".to_string(), "mqtt".to_string())]
                .into_iter()
                .collect(),
        };
        Box::new(FlatJsonParser::from_config(config).unwrap())
    }

    #[tokio::test]
    async fn test_mqtt_source_creation() {
        let config = MqttConfig::default();
        let _source = MqttSource::new(config.clone(), create_default_parser());
    }

    #[tokio::test]
    async fn test_health_check_before_start() {
        let config = MqttConfig::default();
        let source = MqttSource::new(config, create_default_parser());

        let health = source.health_check().await.unwrap();
        assert!(!health.healthy);
    }

    #[tokio::test]
    async fn test_parse_payload_success() {
        let config = MqttConfig::default();
        let source = MqttSource::new(config, create_default_parser());

        let payload = r#"{
            "serialno": "ABC123",
            "pm02": 12.5,
            "rco2": 450,
            "atmp": 22.3,
            "rhum": 55.0,
            "wifi": -45
        }"#;

        let points = source.parse_payload(payload.as_bytes()).unwrap();
        assert_eq!(points.len(), 5); // wifi is now included (numeric field)

        // Check PM2.5 - should use ORIGINAL field name
        let pm_point = points
            .iter()
            .find(|p| p.tags.get("metric") == Some(&"pm02".to_string()))
            .unwrap();
        assert_eq!(pm_point.value, 12.5);
        assert_eq!(pm_point.location_id, "ABC123");

        // Check CO2 - should use ORIGINAL field name (rco2, not co2)
        let co2_point = points
            .iter()
            .find(|p| p.tags.get("metric") == Some(&"rco2".to_string()))
            .unwrap();
        assert_eq!(co2_point.value, 450.0);

        // Check temperature - should use ORIGINAL field name (atmp, not temperature)
        let temp_point = points
            .iter()
            .find(|p| p.tags.get("metric") == Some(&"atmp".to_string()))
            .unwrap();
        assert_eq!(temp_point.value, 22.3);

        // Check humidity - should use ORIGINAL field name (rhum, not humidity)
        let hum_point = points
            .iter()
            .find(|p| p.tags.get("metric") == Some(&"rhum".to_string()))
            .unwrap();
        assert_eq!(hum_point.value, 55.0);
    }

    #[tokio::test]
    async fn test_parse_payload_invalid_json() {
        let config = MqttConfig::default();
        let source = MqttSource::new(config, create_default_parser());

        let payload = b"invalid json";
        let result = source.parse_payload(payload);

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_parse_payload_partial_data() {
        let config = MqttConfig::default();
        let source = MqttSource::new(config, create_default_parser());

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
    async fn test_parse_payload_all_fields() {
        let config = MqttConfig::default();
        let source = MqttSource::new(config, create_default_parser());

        // Real sensor payload with ALL fields
        let payload = r#"{
            "pm01": 0,
            "pm02": 2.17,
            "pm10": 2.33,
            "atmp": 22.1,
            "rhum": 65.13,
            "rco2": 396,
            "tvocIndex": 42,
            "noxIndex": 2,
            "tvocRaw": 123,
            "noxRaw": 456,
            "serialno": "d83bda1cd074"
        }"#;

        let points = source.parse_payload(payload.as_bytes()).unwrap();

        // Should extract all 9 numeric metrics (pm01, pm02, pm10, atmp, rhum, rco2, tvocIndex, noxIndex, tvocRaw, noxRaw)
        assert_eq!(points.len(), 10);

        // Verify all fields are present with ORIGINAL names
        let metric_names: Vec<String> = points
            .iter()
            .map(|p| p.tags.get("metric").unwrap().clone())
            .collect();

        assert!(metric_names.contains(&"pm01".to_string()));
        assert!(metric_names.contains(&"pm02".to_string()));
        assert!(metric_names.contains(&"pm10".to_string()));
        assert!(metric_names.contains(&"atmp".to_string()));
        assert!(metric_names.contains(&"rhum".to_string()));
        assert!(metric_names.contains(&"rco2".to_string()));
        assert!(metric_names.contains(&"tvocIndex".to_string()));
        assert!(metric_names.contains(&"noxIndex".to_string()));
        assert!(metric_names.contains(&"tvocRaw".to_string()));
        assert!(metric_names.contains(&"noxRaw".to_string()));

        // Verify no renamed fields (no "co2", "temperature", "humidity")
        assert!(!metric_names.contains(&"co2".to_string()));
        assert!(!metric_names.contains(&"temperature".to_string()));
        assert!(!metric_names.contains(&"humidity".to_string()));

        // All points should have same serial number
        assert!(points.iter().all(|p| p.location_id == "d83bda1cd074"));
    }

    #[tokio::test]
    async fn test_exponential_backoff_calculation() {
        let config = MqttConfig {
            reconnect_delay: Duration::from_secs(1),
            max_reconnect_delay: Duration::from_secs(30),
            ..Default::default()
        };

        let delays = vec![(0, 1), (1, 2), (2, 4), (3, 8), (4, 16), (5, 30), (6, 30)];

        for (attempt, expected) in delays {
            let delay = std::cmp::min(
                config.reconnect_delay.as_secs() * 2_u64.pow(attempt),
                config.max_reconnect_delay.as_secs(),
            );
            assert_eq!(delay, expected, "Failed for attempt {}", attempt);
        }
    }

    #[tokio::test]
    #[allow(deprecated)]
    async fn test_topic_pattern_substitution() {
        let config = MqttConfig {
            topic_pattern: Some("airgradient/readings/{SERIAL_NUMBER}".to_string()),
            ..Default::default()
        };

        let topic = config
            .topic_pattern
            .as_ref()
            .unwrap()
            .replace("{SERIAL_NUMBER}", "+");
        assert_eq!(topic, "airgradient/readings/+");
    }

    #[tokio::test]
    async fn test_fetch_returns_cached_points() {
        let config = MqttConfig::default();
        let source = MqttSource::new(config, create_default_parser());

        // Add some points to cache
        let mut cache = source.cached_points.lock().await;
        cache.push(TimeSeriesPoint {
            timestamp: Utc::now(),
            location_id: "test".to_string(),
            value: 12.5,
            tags: HashMap::new(),
            ndp_id: None,
            context: None,
        });
        drop(cache);

        let points = source.fetch().await.unwrap();
        assert_eq!(points.len(), 1);

        // Cache should be empty after fetch
        let cache = source.cached_points.lock().await;
        assert_eq!(cache.len(), 0);
    }

    // Additional comprehensive tests for dynamic field extraction

    #[tokio::test]
    async fn test_parse_payload_extracts_all_numeric_fields() {
        let config = MqttConfig::default();
        let source = MqttSource::new(config, create_default_parser());

        // Full AirGradient payload with ALL possible fields including compensated values
        let payload = r#"{
            "pm01": 1.0,
            "pm02": 2.17,
            "pm10": 2.33,
            "pm02Compensated": 1.27,
            "atmp": 22.1,
            "atmpCompensated": 22.1,
            "rhum": 65.13,
            "rhumCompensated": 65.13,
            "rco2": 396,
            "tvocIndex": 42,
            "tvocRaw": 31506.42,
            "noxIndex": 2,
            "noxRaw": 19013.92,
            "boot": 1568,
            "wifi": -29,
            "serialno": "d83bda1cd074",
            "firmware": "3.4.1",
            "model": "I-9PSL"
        }"#;

        let points = source.parse_payload(payload.as_bytes()).unwrap();

        // Should extract ALL 15 numeric fields, excluding string metadata
        assert_eq!(
            points.len(),
            15,
            "Expected 15 numeric fields to be extracted"
        );

        let metrics: Vec<&str> = points
            .iter()
            .map(|p| p.tags.get("metric").unwrap().as_str())
            .collect();

        // Verify ALL numeric fields present with ORIGINAL names
        assert!(metrics.contains(&"pm01"), "pm01 should be extracted");
        assert!(metrics.contains(&"pm02"), "pm02 should be extracted");
        assert!(metrics.contains(&"pm10"), "pm10 should be extracted");
        assert!(
            metrics.contains(&"pm02Compensated"),
            "pm02Compensated should be extracted"
        );
        assert!(
            metrics.contains(&"atmp"),
            "atmp should be extracted (NOT renamed to temperature)"
        );
        assert!(
            metrics.contains(&"atmpCompensated"),
            "atmpCompensated should be extracted"
        );
        assert!(
            metrics.contains(&"rhum"),
            "rhum should be extracted (NOT renamed to humidity)"
        );
        assert!(
            metrics.contains(&"rhumCompensated"),
            "rhumCompensated should be extracted"
        );
        assert!(
            metrics.contains(&"rco2"),
            "rco2 should be extracted (NOT renamed to co2)"
        );
        assert!(
            metrics.contains(&"tvocIndex"),
            "tvocIndex should be extracted"
        );
        assert!(metrics.contains(&"tvocRaw"), "tvocRaw should be extracted");
        assert!(
            metrics.contains(&"noxIndex"),
            "noxIndex should be extracted"
        );
        assert!(metrics.contains(&"noxRaw"), "noxRaw should be extracted");
        assert!(metrics.contains(&"boot"), "boot should be extracted");
        assert!(metrics.contains(&"wifi"), "wifi should be extracted");

        // Verify string metadata NOT extracted
        assert!(
            !metrics.contains(&"serialno"),
            "serialno is metadata, should not be extracted"
        );
        assert!(
            !metrics.contains(&"firmware"),
            "firmware is metadata, should not be extracted"
        );
        assert!(
            !metrics.contains(&"model"),
            "model is metadata, should not be extracted"
        );
    }

    #[tokio::test]
    async fn test_field_names_not_renamed_at_ingestion() {
        let config = MqttConfig::default();
        let source = MqttSource::new(config, create_default_parser());

        let payload = r#"{
            "rco2": 400,
            "atmp": 22.0,
            "rhum": 50.0,
            "tvocIndex": 100,
            "noxIndex": 5,
            "serialno": "test123"
        }"#;

        let points = source.parse_payload(payload.as_bytes()).unwrap();

        let metrics: Vec<&str> = points
            .iter()
            .map(|p| p.tags.get("metric").unwrap().as_str())
            .collect();

        // These SHOULD exist (original names)
        assert!(
            metrics.contains(&"rco2"),
            "rco2 MUST be preserved (not renamed to co2)"
        );
        assert!(
            metrics.contains(&"atmp"),
            "atmp MUST be preserved (not renamed to temperature)"
        );
        assert!(
            metrics.contains(&"rhum"),
            "rhum MUST be preserved (not renamed to humidity)"
        );
        assert!(
            metrics.contains(&"tvocIndex"),
            "tvocIndex MUST be preserved"
        );
        assert!(metrics.contains(&"noxIndex"), "noxIndex MUST be preserved");

        // These should NOT exist (they're renamed versions)
        assert!(
            !metrics.contains(&"co2"),
            "co2 should NOT exist - field should be named rco2"
        );
        assert!(
            !metrics.contains(&"temperature"),
            "temperature should NOT exist - field should be named atmp"
        );
        assert!(
            !metrics.contains(&"humidity"),
            "humidity should NOT exist - field should be named rhum"
        );
        assert!(
            !metrics.contains(&"tvoc"),
            "tvoc should NOT exist - field should be named tvocIndex"
        );
        assert!(
            !metrics.contains(&"nox"),
            "nox should NOT exist - field should be named noxIndex"
        );

        // Should have exactly 5 numeric fields
        assert_eq!(points.len(), 5, "Should extract exactly 5 numeric fields");
    }

    #[tokio::test]
    async fn test_non_metric_fields_excluded() {
        let config = MqttConfig::default();
        let source = MqttSource::new(config, create_default_parser());

        let payload = r#"{
            "pm02": 2.0,
            "serialno": "test123",
            "firmware": "3.4.1",
            "model": "I-9PSL",
            "ledMode": "co2",
            "wifi": -29,
            "boot": 100
        }"#;

        let points = source.parse_payload(payload.as_bytes()).unwrap();

        let metrics: Vec<&str> = points
            .iter()
            .map(|p| p.tags.get("metric").unwrap().as_str())
            .collect();

        // Only numeric fields should be extracted
        assert!(
            metrics.contains(&"pm02"),
            "pm02 is numeric, should be extracted"
        );
        assert!(
            metrics.contains(&"wifi"),
            "wifi is numeric, should be extracted"
        );
        assert!(
            metrics.contains(&"boot"),
            "boot is numeric, should be extracted"
        );

        // String metadata should NOT be extracted
        assert!(
            !metrics.contains(&"serialno"),
            "serialno is string, should not be extracted"
        );
        assert!(
            !metrics.contains(&"firmware"),
            "firmware is string, should not be extracted"
        );
        assert!(
            !metrics.contains(&"model"),
            "model is string, should not be extracted"
        );
        assert!(
            !metrics.contains(&"ledMode"),
            "ledMode is string, should not be extracted"
        );

        // Should have exactly 3 numeric metrics
        assert_eq!(points.len(), 3, "Should extract exactly 3 numeric fields");
    }

    #[tokio::test]
    async fn test_all_numeric_types_extracted() {
        let config = MqttConfig::default();
        let source = MqttSource::new(config, create_default_parser());

        // Test integer, float, negative values
        let payload = r#"{
            "intField": 100,
            "floatField": 22.5,
            "negativeInt": -29,
            "negativeFloat": -3.14,
            "zeroInt": 0,
            "zeroFloat": 0.0,
            "largeFloat": 31506.42,
            "serialno": "test"
        }"#;

        let points = source.parse_payload(payload.as_bytes()).unwrap();

        // Should extract ALL 7 numeric values regardless of type
        assert_eq!(
            points.len(),
            7,
            "Should extract all numeric types (int, float, negative, zero)"
        );

        let metrics: Vec<&str> = points
            .iter()
            .map(|p| p.tags.get("metric").unwrap().as_str())
            .collect();

        // Verify all numeric fields present
        assert!(metrics.contains(&"intField"));
        assert!(metrics.contains(&"floatField"));
        assert!(metrics.contains(&"negativeInt"));
        assert!(metrics.contains(&"negativeFloat"));
        assert!(metrics.contains(&"zeroInt"));
        assert!(metrics.contains(&"zeroFloat"));
        assert!(metrics.contains(&"largeFloat"));

        // Verify values are correctly parsed
        let int_point = points
            .iter()
            .find(|p| p.tags.get("metric") == Some(&"intField".to_string()))
            .unwrap();
        assert_eq!(int_point.value, 100.0);

        let neg_point = points
            .iter()
            .find(|p| p.tags.get("metric") == Some(&"negativeInt".to_string()))
            .unwrap();
        assert_eq!(neg_point.value, -29.0);
    }

    // ==========================================================================
    // MqttConfig Tests (Phase 3)
    // ==========================================================================

    #[tokio::test]
    #[allow(deprecated)]
    async fn test_config_get_subscriptions_from_new_format() {
        let config = MqttConfig {
            subscriptions: vec![
                SubscriptionConfig::new("air-quality", "airgradient/readings/+"),
                SubscriptionConfig::new("homeassistant", "homeassistant/+/+/state"),
            ],
            topic_pattern: None,
            ..Default::default()
        };

        let subs = config.get_subscriptions();
        assert_eq!(subs.len(), 2);
        assert_eq!(subs[0].stream_id, "air-quality");
        assert_eq!(subs[1].stream_id, "homeassistant");
    }

    #[tokio::test]
    #[allow(deprecated)]
    async fn test_config_get_subscriptions_from_legacy_format() {
        let config = MqttConfig {
            subscriptions: Vec::new(),
            topic_pattern: Some("airgradient/readings/{SERIAL_NUMBER}".to_string()),
            default_stream_id: "air-quality".to_string(),
            ..Default::default()
        };

        let subs = config.get_subscriptions();
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].stream_id, "air-quality");
        // {SERIAL_NUMBER} should be converted to +
        assert_eq!(subs[0].topic_pattern, "airgradient/readings/+");
    }

    #[tokio::test]
    #[allow(deprecated)]
    async fn test_config_get_subscriptions_prefers_new_format() {
        // When both are set, new format takes precedence
        let config = MqttConfig {
            subscriptions: vec![SubscriptionConfig::new("new-stream", "new/topic/+")],
            topic_pattern: Some("legacy/topic/+".to_string()),
            default_stream_id: "legacy-stream".to_string(),
            ..Default::default()
        };

        let subs = config.get_subscriptions();
        assert_eq!(subs.len(), 1);
        assert_eq!(subs[0].stream_id, "new-stream");
        assert_eq!(subs[0].topic_pattern, "new/topic/+");
    }

    #[tokio::test]
    #[allow(deprecated)]
    async fn test_config_get_subscriptions_empty_when_nothing_configured() {
        let config = MqttConfig {
            subscriptions: Vec::new(),
            topic_pattern: None,
            ..Default::default()
        };

        let subs = config.get_subscriptions();
        assert!(subs.is_empty());
    }

    #[tokio::test]
    #[allow(deprecated)]
    async fn test_config_validate_success_with_new_format() {
        let config = MqttConfig {
            subscriptions: vec![
                SubscriptionConfig::new("stream1", "topic/+"),
                SubscriptionConfig::new("stream2", "topic/#"),
            ],
            topic_pattern: None,
            ..Default::default()
        };

        assert!(config.validate().is_ok());
    }

    #[tokio::test]
    #[allow(deprecated)]
    async fn test_config_validate_success_with_legacy_format() {
        let config = MqttConfig {
            subscriptions: Vec::new(),
            topic_pattern: Some("airgradient/readings/+".to_string()),
            default_stream_id: "default".to_string(),
            ..Default::default()
        };

        assert!(config.validate().is_ok());
    }

    #[tokio::test]
    #[allow(deprecated)]
    async fn test_config_validate_error_no_subscriptions() {
        let config = MqttConfig {
            subscriptions: Vec::new(),
            topic_pattern: None,
            ..Default::default()
        };

        let result = config.validate();
        assert!(matches!(result, Err(ConfigError::NoSubscriptions)));
    }

    #[tokio::test]
    #[allow(deprecated)]
    async fn test_config_validate_error_duplicate_stream_id() {
        let config = MqttConfig {
            subscriptions: vec![
                SubscriptionConfig::new("same-id", "topic/a/+"),
                SubscriptionConfig::new("same-id", "topic/b/+"),
            ],
            topic_pattern: None,
            ..Default::default()
        };

        let result = config.validate();
        assert!(matches!(result, Err(ConfigError::DuplicateStreamId(id)) if id == "same-id"));
    }

    #[tokio::test]
    #[allow(deprecated)]
    async fn test_config_validate_error_invalid_subscription() {
        let config = MqttConfig {
            subscriptions: vec![SubscriptionConfig {
                stream_id: "".to_string(), // Invalid: empty
                topic_pattern: "test/+".to_string(),
                parser: None,
                enabled: true,
                ndp_id_topic_segment: None,
            }],
            topic_pattern: None,
            ..Default::default()
        };

        let result = config.validate();
        assert!(matches!(
            result,
            Err(ConfigError::InvalidSubscription { .. })
        ));
    }

    #[test]
    fn test_config_error_display() {
        let no_subs = ConfigError::NoSubscriptions;
        assert!(no_subs.to_string().contains("no subscriptions"));

        let duplicate = ConfigError::DuplicateStreamId("test".to_string());
        assert!(duplicate.to_string().contains("duplicate"));
        assert!(duplicate.to_string().contains("test"));

        let invalid = ConfigError::InvalidSubscription {
            stream_id: "bad".to_string(),
            error: "empty field".to_string(),
        };
        assert!(invalid.to_string().contains("invalid subscription"));
        assert!(invalid.to_string().contains("bad"));
        assert!(invalid.to_string().contains("empty field"));
    }
}
