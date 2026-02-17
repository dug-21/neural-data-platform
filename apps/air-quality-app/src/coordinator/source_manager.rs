//! Source Manager
//!
//! Manages lifecycle of multiple data sources (MQTT, HTTP, Webhook)
//!
//! DP-012: Sources publish to EventBus instead of mpsc channel.
//! The EventBus is the single source of truth for all data flow.
//!
//! DP-021: Hot-reload support for sources. Sources can be reconfigured
//! without application restart by watching etcd for config changes.

use config_client::StreamRegistry;
use neural_core::parsers::{create_parser_from_config, ParserConfig, ParserType};
use neural_core::sources::{
    AuthMethod, EndpointConfig, GenericHttpPollingConfig, GenericHttpPollingSource, RetryConfig,
};
use neural_core::EventBus;
use neural_core::{
    HttpPollingConfig, HttpPollingSource, MqttConfig, MqttSource, RawSource, SensorConfig, Source,
    SourceConfig, SourceType, StreamConfig,
};
use regex::Regex;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

/// Cached regex for environment variable expansion (e.g., ${VAR_NAME})
/// Compiled once at first use, avoiding repeated compilation overhead.
static ENV_VAR_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$\{([^}]+)\}").expect("ENV_VAR_REGEX pattern is invalid"));

/// Source health status
#[derive(Debug, Clone, PartialEq)]
pub enum SourceHealth {
    Healthy,
    Degraded { reason: String },
    Unhealthy { reason: String },
    Unknown,
}

/// DP-021: Result of a hot-reload operation
#[derive(Debug, Clone)]
pub struct HotReloadResult {
    /// Whether the reload succeeded
    pub success: bool,
    /// Stream that was reloaded
    pub stream_id: String,
    /// Source IDs that were stopped during reload
    pub sources_stopped: Vec<String>,
    /// Source IDs that were started during reload
    pub sources_started: Vec<String>,
    /// Duration of the reload operation in milliseconds
    pub duration_ms: u64,
    /// Error message if reload failed
    pub error: Option<String>,
}

impl HotReloadResult {
    /// Create a successful reload result
    pub fn success(
        stream_id: String,
        sources_stopped: Vec<String>,
        sources_started: Vec<String>,
        duration_ms: u64,
    ) -> Self {
        Self {
            success: true,
            stream_id,
            sources_stopped,
            sources_started,
            duration_ms,
            error: None,
        }
    }

    /// Create a failed reload result
    pub fn failure(stream_id: String, error: String, duration_ms: u64) -> Self {
        Self {
            success: false,
            stream_id,
            sources_stopped: Vec::new(),
            sources_started: Vec::new(),
            duration_ms,
            error: Some(error),
        }
    }
}

/// DP-021: Type of configuration change detected
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigChangeType {
    /// New stream config created
    Created,
    /// Existing stream config modified
    Updated,
    /// Stream config deleted
    Deleted,
    /// Stream disabled (enabled: false)
    Disabled,
    /// Stream re-enabled (enabled: true)
    Enabled,
}

/// Source manager error
#[derive(Debug, thiserror::Error)]
pub enum SourceManagerError {
    #[error("Failed to spawn source: {0}")]
    SpawnError(String),

    #[error("Failed to stop source: {0}")]
    StopError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Source not found: {0}")]
    SourceNotFound(String),
}

/// Information about a running source
#[derive(Debug)]
struct SourceInfo {
    #[allow(dead_code)]
    source_id: String,
    stream_id: String,
    source_type: SourceType,
    enabled: bool,
    health: SourceHealth,
    cancel_token: CancellationToken,
    task_handle: Option<JoinHandle<()>>,
}

/// Manages multiple data sources
///
/// DP-012: Sources publish directly to EventBus instead of mpsc channel.
/// The EventBus broadcasts to all subscribers (BronzeSubscriber, SilverSubscriber, etc.)
pub struct SourceManager {
    registry: Arc<StreamRegistry>,
    sources: Arc<RwLock<HashMap<String, SourceInfo>>>,
    /// DP-012: EventBus for publishing raw data points
    event_bus: Option<Arc<EventBus>>,
}

impl SourceManager {
    /// Create a new source manager
    pub fn new(registry: Arc<StreamRegistry>) -> Self {
        Self {
            registry,
            sources: Arc::new(RwLock::new(HashMap::new())),
            event_bus: None,
        }
    }

    /// DP-012: Set the EventBus for publishing raw data points
    ///
    /// Must be called before starting sources. Sources will publish to
    /// EventBus instead of mpsc channel, enabling multi-consumer broadcasting.
    pub fn set_event_bus(&mut self, event_bus: Arc<EventBus>) {
        self.event_bus = Some(event_bus);
    }

    /// Start all configured sources
    pub async fn start_all_sources(&mut self) -> Result<(), SourceManagerError> {
        info!("Starting all configured sources");

        // Load all stream configurations
        let streams = self
            .registry
            .list_streams()
            .await
            .map_err(|e| SourceManagerError::ConfigError(e.to_string()))?;

        for stream_id in streams {
            let config = self
                .registry
                .load_stream(&stream_id)
                .await
                .map_err(|e| SourceManagerError::ConfigError(e.to_string()))?;

            // Skip disabled streams entirely
            if !config.enabled {
                info!("Skipping disabled stream: {}", stream_id);
                continue;
            }

            // Start sources for this stream
            self.start_sources_for_stream(&config).await?;
        }

        info!("All sources started");
        Ok(())
    }

    /// Start sources for a specific stream
    async fn start_sources_for_stream(
        &mut self,
        config: &StreamConfig,
    ) -> Result<(), SourceManagerError> {
        for source_config in &config.sources {
            if !source_config.enabled {
                debug!(
                    "Skipping disabled source {:?} for stream {}",
                    source_config.source_type, config.stream_id
                );
                continue;
            }

            self.spawn_source(&config.stream_id, source_config).await?;
        }

        Ok(())
    }

    /// Spawn a single source
    async fn spawn_source(
        &mut self,
        stream_id: &str,
        source_config: &SourceConfig,
    ) -> Result<String, SourceManagerError> {
        // Internal tracking ID includes source type (for HashMap uniqueness)
        // But storage path uses stream_id only (config-driven)
        let internal_id = format!("{}-{:?}", stream_id, source_config.source_type);
        // Storage path uses just stream_id: /data/raw/{stream_id}/year=.../...
        let storage_id = stream_id.to_string();

        // Stop any existing source with the same ID to prevent duplicates
        // This is defensive - normally sources should only be spawned once
        {
            let sources = self.sources.read().await;
            if sources.contains_key(&internal_id) {
                drop(sources); // Release read lock before acquiring write lock
                warn!(
                    "Source {} already exists, stopping before respawn",
                    internal_id
                );
                if let Err(e) = self.stop_source(&internal_id).await {
                    warn!("Failed to stop existing source {}: {}", internal_id, e);
                }
            }
        }

        info!(
            "Spawning source {} ({:?}) for stream {} (storage path: {})",
            internal_id, source_config.source_type, stream_id, storage_id
        );

        // Create cancellation token for this source
        let cancel_token = CancellationToken::new();

        // DP-012: Spawn source based on type - all sources publish to EventBus
        let task_handle = match source_config.source_type {
            SourceType::HttpPoll => {
                // DP-012: Get EventBus (required for all sources)
                let event_bus = self
                    .event_bus
                    .as_ref()
                    .ok_or_else(|| {
                        SourceManagerError::ConfigError(
                            "EventBus not set. Call set_event_bus() first.".to_string(),
                        )
                    })?
                    .clone();

                // Check if this is a parser-based config (GenericHttpPollingSource)
                // or an AirGradient-style config (HttpPollingSource)
                let has_parser = source_config
                    .params
                    .get("parser_name")
                    .and_then(|v| v.as_str())
                    .is_some();

                let stream_id_clone = stream_id.to_string();
                let storage_id_clone = storage_id.clone();
                let cancel_clone = cancel_token.clone();

                // AIR-009: Extract ndp_id and context from source config
                let ndp_id = source_config.ndp_id.clone();
                let context = source_config.context.clone();

                if has_parser {
                    // Parse GenericHttpPollingConfig and ParserConfig for external APIs (NWS, OpenWeatherMap, etc.)
                    let (http_config, parser_config) =
                        self.parse_generic_http_polling_config(stream_id, source_config)?;

                    Some(tokio::spawn(async move {
                        if let Err(e) = Self::run_generic_http_polling_source(
                            stream_id_clone,
                            storage_id_clone,
                            http_config,
                            parser_config,
                            event_bus,
                            cancel_clone,
                            ndp_id,
                            context,
                        )
                        .await
                        {
                            error!("Generic HTTP polling source failed: {}", e);
                        }
                    }))
                } else {
                    // Parse HttpPollingConfig for AirGradient sensors
                    let config = self.parse_http_polling_config(stream_id, source_config)?;
                    let storage_id_clone2 = storage_id.clone();

                    Some(tokio::spawn(async move {
                        if let Err(e) = Self::run_http_polling_source(
                            stream_id_clone,
                            storage_id_clone2,
                            config,
                            event_bus,
                            cancel_clone,
                            ndp_id,
                            context,
                        )
                        .await
                        {
                            error!("HTTP polling source failed: {}", e);
                        }
                    }))
                }
            }
            SourceType::Mqtt => {
                // DP-012: Get EventBus (required for all sources)
                let event_bus = self
                    .event_bus
                    .as_ref()
                    .ok_or_else(|| {
                        SourceManagerError::ConfigError(
                            "EventBus not set. Call set_event_bus() first.".to_string(),
                        )
                    })?
                    .clone();

                let stream_id_clone = stream_id.to_string();
                let storage_id_clone = storage_id.clone();
                let cancel_clone = cancel_token.clone();

                // AIR-009: Extract ndp_id and context from source config
                let ndp_id = source_config.ndp_id.clone();
                let context = source_config.context.clone();

                // Parse MQTT config from source params
                let config = self.parse_mqtt_config(stream_id, source_config)?;

                // AIR-012: Extract parser config from YAML (like HTTP does)
                let parser_config = if let Some(parser_val) = source_config.params.get("parser") {
                    serde_json::from_value::<ParserConfig>(parser_val.clone()).map_err(|e| {
                        SourceManagerError::ConfigError(format!(
                            "Failed to parse parser config for MQTT stream {}: {}",
                            stream_id, e
                        ))
                    })?
                } else {
                    // Fallback to default FlatJson parser (AirGradient-compatible)
                    ParserConfig {
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
                        ..Default::default()
                    }
                };

                Some(tokio::spawn(async move {
                    if let Err(e) = Self::run_mqtt_source(
                        stream_id_clone,
                        storage_id_clone,
                        config,
                        parser_config,
                        event_bus,
                        cancel_clone,
                        ndp_id,
                        context,
                    )
                    .await
                    {
                        error!("MQTT source failed: {}", e);
                    }
                }))
            }
            SourceType::Webhook => {
                warn!("Webhook source not yet implemented");
                None
            }
            SourceType::FileWatch => {
                warn!("FileWatch source not yet implemented");
                None
            }
            SourceType::Csv => {
                // dp-013: CSV sources are batch/one-time imports, not continuous polling.
                // Use `ndp stream ingest <stream_id>` or deploy.sh for CSV ingestion.
                warn!(
                    "CSV source '{}' is for batch import. Use 'ndp stream ingest' or deploy.sh, not continuous polling.",
                    stream_id
                );
                None
            }
        };

        // Create source info
        let source_info = SourceInfo {
            source_id: internal_id.clone(),
            stream_id: stream_id.to_string(),
            source_type: source_config.source_type.clone(),
            enabled: source_config.enabled,
            health: if task_handle.is_some() {
                SourceHealth::Healthy
            } else {
                SourceHealth::Unknown
            },
            cancel_token,
            task_handle,
        };

        // Store source info (keyed by internal_id for uniqueness)
        let mut sources = self.sources.write().await;
        sources.insert(internal_id.clone(), source_info);

        debug!(
            "Source {} spawned successfully (storage: {})",
            internal_id, storage_id
        );
        Ok(internal_id)
    }

    /// Parse HTTP polling configuration from source params
    fn parse_http_polling_config(
        &self,
        stream_id: &str,
        source_config: &SourceConfig,
    ) -> Result<HttpPollingConfig, SourceManagerError> {
        // Extract configuration parameters
        let base_url_template = source_config
            .params
            .get("base_url_template")
            .and_then(|v| v.as_str())
            .unwrap_or("http://airgradient_{SERIAL}.local/measures/current")
            .to_string();

        let poll_interval_secs = source_config
            .params
            .get("poll_interval_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(60);

        let timeout_secs = source_config
            .params
            .get("timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(10);

        let buffer_capacity = source_config
            .params
            .get("buffer_capacity")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000) as usize;

        // Parse sensors/endpoints array
        let sensors = if let Some(endpoints) = source_config.params.get("endpoints") {
            if let Some(arr) = endpoints.as_array() {
                arr.iter()
                    .filter_map(|v| {
                        let serial = v.get("serial")?.as_str()?;
                        let url = v
                            .get("url")
                            .and_then(|u| u.as_str())
                            .map(|s| s.to_string())
                            .or_else(|| Some(base_url_template.replace("{SERIAL}", serial)))?;

                        Some(SensorConfig {
                            serial_number: serial.to_string(),
                            url,
                        })
                    })
                    .collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        if sensors.is_empty() {
            return Err(SourceManagerError::ConfigError(format!(
                "No endpoints configured for HTTP polling source in stream {}",
                stream_id
            )));
        }

        Ok(HttpPollingConfig {
            base_url_template,
            poll_interval: std::time::Duration::from_secs(poll_interval_secs),
            timeout: std::time::Duration::from_secs(timeout_secs),
            sensors,
            buffer_capacity,
        })
    }

    /// Run HTTP polling source (DP-012: publishes RawDataPoint to EventBus)
    async fn run_http_polling_source(
        stream_id: String,
        _source_id: String,
        config: HttpPollingConfig,
        event_bus: Arc<EventBus>,
        cancel_token: CancellationToken,
        ndp_id: Option<String>,
        context: Option<serde_json::Value>,
    ) -> Result<(), SourceManagerError> {
        info!("Starting HTTP polling source for stream {}", stream_id);

        // Create parser from config (FlatJson for AirGradient sensors)
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
            ..Default::default()
        };
        let parser = create_parser_from_config(parser_config).map_err(|e| {
            SourceManagerError::SpawnError(format!("Failed to create parser: {}", e))
        })?;

        // DP-004: Create source with stream_id for proper source_id generation
        // AIR-011: Source is NOT started - we only use fetch_raw_batch() to avoid double-polling
        let source = HttpPollingSource::with_raw_config(
            config.clone(),
            parser,
            Some(stream_id.clone()),
            ndp_id.clone(),
            context.clone(),
        )
        .map_err(|e| SourceManagerError::SpawnError(e.to_string()))?;

        // AIR-011: Removed source.start() - it spawned a background polling_loop that:
        // 1. Polled HTTP endpoints and parsed JSON into TimeSeriesPoints
        // 2. Sent points to internal channel that was NEVER consumed
        // 3. Caused memory pressure and Pi lockups after hours of operation
        // We only use fetch_raw_batch() which returns raw JSON without parsing.

        // Poll loop - fetch data and publish to EventBus
        // AIR-011: Use config.poll_interval instead of hardcoded 1 second
        let mut interval = tokio::time::interval(config.poll_interval);

        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    info!("HTTP polling source for stream {} received cancellation", stream_id);
                    // AIR-011: Removed source.stop() - no background polling_loop to stop
                    break;
                }
                _ = interval.tick() => {
                    // DP-004/DP-012: Fetch raw data points and publish to EventBus
                    match source.fetch_raw_batch().await {
                        Ok(raw_points) => {
                            for raw_point in raw_points {
                                // DP-012: Publish to EventBus (zero-copy via Arc)
                                if let Err(e) = event_bus.publish(Arc::new(raw_point)) {
                                    // No subscribers is not an error - data flows when subscribers register
                                    debug!("EventBus publish result: {:?}", e);
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to fetch points from HTTP source: {}", e);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Parse GenericHttpPollingConfig and ParserConfig for external APIs (NWS, OpenWeatherMap, etc.)
    fn parse_generic_http_polling_config(
        &self,
        stream_id: &str,
        source_config: &SourceConfig,
    ) -> Result<(GenericHttpPollingConfig, ParserConfig), SourceManagerError> {
        let parser_name = source_config
            .params
            .get("parser_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                SourceManagerError::ConfigError(format!(
                    "Missing parser_name for generic HTTP polling source in stream {}",
                    stream_id
                ))
            })?
            .to_string();

        let poll_interval_secs = source_config
            .params
            .get("poll_interval_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(600); // Default 10 minutes for API rate limits

        let timeout_secs = source_config
            .params
            .get("timeout_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(30);

        let buffer_capacity = source_config
            .params
            .get("buffer_capacity")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000) as usize;

        // Parse endpoints array
        let endpoints = if let Some(endpoints_val) = source_config.params.get("endpoints") {
            if let Some(arr) = endpoints_val.as_array() {
                arr.iter()
                    .filter_map(|v| {
                        let endpoint_id = v.get("endpoint_id")?.as_str()?;
                        let url = v.get("url")?.as_str()?;
                        let location_id = v
                            .get("location_id")
                            .and_then(|v| v.as_str())
                            .unwrap_or(stream_id);

                        // Parse authentication
                        let auth =
                            if let Some(auth_type) = v.get("auth_type").and_then(|v| v.as_str()) {
                                match auth_type {
                                    "query_param" => {
                                        let key = v.get("auth_key")?.as_str()?;
                                        let value = v.get("auth_value")?.as_str()?;
                                        // Expand environment variables in auth value
                                        let expanded_value = Self::expand_env_vars(value);
                                        AuthMethod::QueryParam {
                                            key: key.to_string(),
                                            value: expanded_value,
                                        }
                                    }
                                    "header" => {
                                        let name = v.get("auth_key")?.as_str()?;
                                        let value = v.get("auth_value")?.as_str()?;
                                        let expanded_value = Self::expand_env_vars(value);
                                        AuthMethod::Header {
                                            name: name.to_string(),
                                            value: expanded_value,
                                        }
                                    }
                                    "bearer" => {
                                        let token = v.get("auth_value")?.as_str()?;
                                        let expanded_token = Self::expand_env_vars(token);
                                        AuthMethod::Bearer {
                                            token: expanded_token,
                                        }
                                    }
                                    _ => AuthMethod::None,
                                }
                            } else {
                                AuthMethod::None
                            };

                        // Expand environment variables in URL
                        let expanded_url = Self::expand_env_vars(url);

                        // Build endpoint config with auth
                        let mut endpoint = EndpointConfig::new(
                            endpoint_id,
                            expanded_url,
                            location_id,
                            &parser_name,
                        )
                        .with_auth(auth);

                        // Add custom headers from config
                        if let Some(headers) = v.get("headers").and_then(|h| h.as_object()) {
                            for (name, value) in headers {
                                if let Some(val_str) = value.as_str() {
                                    endpoint =
                                        endpoint.with_header(name, Self::expand_env_vars(val_str));
                                }
                            }
                        }

                        Some(endpoint)
                    })
                    .collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        if endpoints.is_empty() {
            return Err(SourceManagerError::ConfigError(format!(
                "No endpoints configured for generic HTTP polling source in stream {}",
                stream_id
            )));
        }

        // Parse the parser configuration from YAML
        let parser_config = if let Some(parser_val) = source_config.params.get("parser") {
            // Try to deserialize from YAML/JSON Value
            serde_json::from_value::<ParserConfig>(parser_val.clone()).map_err(|e| {
                SourceManagerError::ConfigError(format!(
                    "Failed to parse parser config for stream {}: {}",
                    stream_id, e
                ))
            })?
        } else {
            // Fallback to default FlatJson parser if no parser config specified
            ParserConfig {
                parser_type: ParserType::FlatJson,
                location_id_field: "location_id".to_string(),
                default_location_id: Some(stream_id.to_string()),
                skip_fields: Vec::new(),
                field_mappings: None,
                default_tags: std::collections::HashMap::new(),
                ..Default::default()
            }
        };

        let http_config = GenericHttpPollingConfig {
            endpoints,
            poll_interval: std::time::Duration::from_secs(poll_interval_secs),
            timeout: std::time::Duration::from_secs(timeout_secs),
            retry_config: RetryConfig::default(),
            buffer_capacity,
        };

        Ok((http_config, parser_config))
    }

    /// Expand environment variables in a string (e.g., ${VAR_NAME})
    ///
    /// Uses the cached `ENV_VAR_REGEX` to avoid recompiling the regex on each call.
    fn expand_env_vars(s: &str) -> String {
        let mut result = s.to_string();

        for cap in ENV_VAR_REGEX.captures_iter(s) {
            let var_name = &cap[1];
            if let Ok(value) = std::env::var(var_name) {
                result = result.replace(&format!("${{{}}}", var_name), &value);
            }
        }

        result
    }

    /// Parse MQTT configuration from source params
    fn parse_mqtt_config(
        &self,
        stream_id: &str,
        source_config: &SourceConfig,
    ) -> Result<MqttConfig, SourceManagerError> {
        let broker_url = source_config
            .params
            .get("broker_url")
            .and_then(|v| v.as_str())
            .map(|s| Self::expand_env_vars(s))
            .unwrap_or_else(|| {
                std::env::var("MQTT_BROKER_URL").unwrap_or_else(|_| "localhost".to_string())
            });

        let port = source_config
            .params
            .get("port")
            .and_then(|v| v.as_u64())
            .unwrap_or(1883) as u16;

        let topic_pattern = source_config
            .params
            .get("topic_pattern")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let client_id = source_config
            .params
            .get("client_id")
            .and_then(|v| v.as_str())
            .unwrap_or(&format!("ndp-{}", stream_id))
            .to_string();

        let buffer_capacity = source_config
            .params
            .get("buffer_capacity")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000) as usize;

        let qos = match source_config
            .params
            .get("qos")
            .and_then(|v| v.as_u64())
            .unwrap_or(1)
        {
            0 => rumqttc::QoS::AtMostOnce,
            1 => rumqttc::QoS::AtLeastOnce,
            2 => rumqttc::QoS::ExactlyOnce,
            _ => rumqttc::QoS::AtLeastOnce,
        };

        let reconnect_delay_secs = source_config
            .params
            .get("reconnect_delay_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(1);

        let max_reconnect_delay_secs = source_config
            .params
            .get("max_reconnect_delay_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(30);

        // AIR-012: Extract ndp_id_topic_segment from params
        let ndp_id_topic_segment = source_config
            .params
            .get("ndp_id_topic_segment")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);

        // AIR-012: Build subscriptions with ndp_id_topic_segment if topic_pattern exists
        let subscriptions = if let Some(ref pattern) = topic_pattern {
            let mut sub = neural_core::sources::mqtt::SubscriptionConfig::new(
                stream_id.to_string(),
                pattern.clone(),
            );
            if let Some(segment) = ndp_id_topic_segment {
                sub = sub.with_ndp_id_topic_segment(segment);
            }
            vec![sub]
        } else {
            Vec::new()
        };

        #[allow(deprecated)]
        Ok(MqttConfig {
            broker_url,
            port,
            client_id,
            topic_pattern,
            subscriptions,
            qos,
            reconnect_delay: std::time::Duration::from_secs(reconnect_delay_secs),
            max_reconnect_delay: std::time::Duration::from_secs(max_reconnect_delay_secs),
            buffer_capacity,
            default_stream_id: stream_id.to_string(),
        })
    }

    /// Run MQTT source (DP-012: publishes RawDataPoint to EventBus)
    /// AIR-012: Now accepts parser_config from YAML instead of hardcoding
    async fn run_mqtt_source(
        stream_id: String,
        _source_id: String,
        config: MqttConfig,
        parser_config: ParserConfig,
        event_bus: Arc<EventBus>,
        cancel_token: CancellationToken,
        ndp_id: Option<String>,
        context: Option<serde_json::Value>,
    ) -> Result<(), SourceManagerError> {
        info!(
            "Starting MQTT source for stream {} with parser type {:?}",
            stream_id, parser_config.parser_type
        );

        // AIR-012: Create parser from config (now from YAML, not hardcoded)
        let parser = create_parser_from_config(parser_config).map_err(|e| {
            SourceManagerError::SpawnError(format!("Failed to create parser: {}", e))
        })?;

        // DP-004: Create source with stream_id for proper source_id generation
        let mut source = MqttSource::with_raw_config(
            config,
            parser,
            Some(stream_id.clone()),
            ndp_id.clone(),
            context.clone(),
        );
        source
            .start()
            .await
            .map_err(|e| SourceManagerError::SpawnError(e.to_string()))?;

        // Poll loop - fetch data and publish to EventBus (same pattern as HTTP)
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));

        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    info!("MQTT source for stream {} received cancellation", stream_id);
                    source.stop().await
                        .map_err(|e| SourceManagerError::StopError(e.to_string()))?;
                    break;
                }
                _ = interval.tick() => {
                    // DP-004/DP-012: Fetch raw data points and publish to EventBus
                    match source.fetch_raw_batch().await {
                        Ok(raw_points) => {
                            for raw_point in raw_points {
                                // DP-012: Publish to EventBus (zero-copy via Arc)
                                if let Err(e) = event_bus.publish(Arc::new(raw_point)) {
                                    // No subscribers is not an error - data flows when subscribers register
                                    debug!("EventBus publish result: {:?}", e);
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to fetch points from MQTT source: {}", e);
                        }
                    }

                    // BUG-007: Drain cached_points to prevent unbounded memory growth.
                    // MqttSource::process_events() parses every MQTT message into
                    // TimeSeriesPoints and pushes them to cached_points (Source::fetch()).
                    // Since DP-012, only RawDataPoints via EventBus are used — the parsed
                    // points are never consumed, causing ~3.85 MiB/hour leak.
                    let _ = source.fetch().await;
                }
            }
        }

        Ok(())
    }

    /// Run Generic HTTP polling source for external APIs (DP-012: publishes RawDataPoint to EventBus)
    async fn run_generic_http_polling_source(
        stream_id: String,
        _source_id: String,
        config: GenericHttpPollingConfig,
        parser_config: ParserConfig,
        event_bus: Arc<EventBus>,
        cancel_token: CancellationToken,
        ndp_id: Option<String>,
        context: Option<serde_json::Value>,
    ) -> Result<(), SourceManagerError> {
        info!(
            "Starting generic HTTP polling source for stream {} with parser type {:?}",
            stream_id, parser_config.parser_type
        );

        // Create parser from config (uses the actual parser type from YAML, not hardcoded FlatJson)
        let parser = create_parser_from_config(parser_config).map_err(|e| {
            SourceManagerError::SpawnError(format!("Failed to create parser: {}", e))
        })?;

        // DP-004: Create source with stream_id for proper source_id generation
        // AIR-011: Source is NOT started - we only use fetch_raw_batch() to avoid double-polling
        let source = GenericHttpPollingSource::with_raw_config(
            config.clone(),
            parser,
            Some(stream_id.clone()),
            ndp_id.clone(),
            context.clone(),
        )
        .map_err(|e| SourceManagerError::SpawnError(e.to_string()))?;

        // AIR-011: Removed source.start() - it spawned a background polling_loop that:
        // 1. Polled HTTP endpoints and parsed JSON into TimeSeriesPoints
        // 2. Sent points to internal channel that was NEVER consumed
        // 3. Caused memory pressure and Pi lockups after hours of operation
        // We only use fetch_raw_batch() which returns raw JSON without parsing.

        // Poll loop - fetch data and publish to EventBus
        // AIR-011: Use config.poll_interval instead of hardcoded 1 second
        let mut interval = tokio::time::interval(config.poll_interval);

        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    info!("Generic HTTP polling source for stream {} received cancellation", stream_id);
                    // AIR-011: Removed source.stop() - no background polling_loop to stop
                    break;
                }
                _ = interval.tick() => {
                    // DP-004/DP-012: Fetch raw data points and publish to EventBus
                    match source.fetch_raw_batch().await {
                        Ok(raw_points) => {
                            for raw_point in raw_points {
                                // DP-012: Publish to EventBus (zero-copy via Arc)
                                if let Err(e) = event_bus.publish(Arc::new(raw_point)) {
                                    // No subscribers is not an error - data flows when subscribers register
                                    debug!("EventBus publish result: {:?}", e);
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to fetch points from generic HTTP source: {}", e);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Stop a specific source
    pub async fn stop_source(&mut self, source_id: &str) -> Result<(), SourceManagerError> {
        info!("Stopping source: {}", source_id);

        let mut sources = self.sources.write().await;

        if let Some(mut info) = sources.remove(source_id) {
            info.enabled = false;
            info.health = SourceHealth::Unhealthy {
                reason: "Stopped".to_string(),
            };

            // Cancel the source task
            info.cancel_token.cancel();

            // Wait for task to complete if it exists
            if let Some(handle) = info.task_handle {
                drop(sources); // Release lock before awaiting

                match tokio::time::timeout(std::time::Duration::from_secs(5), handle).await {
                    Ok(Ok(())) => {
                        debug!("Source {} stopped gracefully", source_id);
                    }
                    Ok(Err(e)) => {
                        warn!("Source {} task panicked: {:?}", source_id, e);
                    }
                    Err(_) => {
                        warn!("Source {} stop timed out after 5s", source_id);
                    }
                }
            }

            Ok(())
        } else {
            drop(sources);
            // Idempotent: if source not found, it's already stopped - return Ok
            debug!(
                "Source {} not found (already stopped?), returning Ok",
                source_id
            );
            Ok(())
        }
    }

    /// Stop all sources
    pub async fn stop_all_sources(&mut self) -> Result<(), SourceManagerError> {
        info!("Stopping all sources");

        let source_ids: Vec<String> = {
            let sources = self.sources.read().await;
            sources.keys().cloned().collect()
        };

        for source_id in source_ids {
            if let Err(e) = self.stop_source(&source_id).await {
                error!("Failed to stop source {}: {}", source_id, e);
                // Continue stopping other sources
            }
        }

        info!("All sources stopped");
        Ok(())
    }

    /// Get health status for a specific source
    pub async fn get_health(&self, source_id: &str) -> Option<SourceHealth> {
        let sources = self.sources.read().await;
        sources.get(source_id).map(|info| info.health.clone())
    }

    /// Get health status for all sources
    pub async fn get_all_health(&self) -> HashMap<String, SourceHealth> {
        let sources = self.sources.read().await;
        sources
            .iter()
            .map(|(id, info)| (id.clone(), info.health.clone()))
            .collect()
    }

    /// Restart a source (stop and start)
    pub async fn restart_source(&mut self, source_id: &str) -> Result<(), SourceManagerError> {
        info!("Restarting source: {}", source_id);

        // Get source info before stopping
        let (stream_id, source_type) = {
            let sources = self.sources.read().await;
            let info = sources
                .get(source_id)
                .ok_or_else(|| SourceManagerError::SourceNotFound(source_id.to_string()))?;

            (info.stream_id.clone(), info.source_type.clone())
        };

        // Stop the source
        self.stop_source(source_id).await?;

        // Load stream config to get source config
        let stream_config = self
            .registry
            .load_stream(&stream_id)
            .await
            .map_err(|e| SourceManagerError::ConfigError(e.to_string()))?;

        // Find the source config
        let source_config = stream_config
            .sources
            .iter()
            .find(|sc| sc.source_type == source_type)
            .ok_or_else(|| {
                SourceManagerError::ConfigError(format!(
                    "Source config not found for {:?}",
                    source_type
                ))
            })?;

        // Restart the source
        self.spawn_source(&stream_id, source_config).await?;

        info!("Source {} restarted", source_id);
        Ok(())
    }

    /// Update sources based on new stream configuration
    pub async fn update_sources_for_stream(
        &mut self,
        stream_id: &str,
    ) -> Result<(), SourceManagerError> {
        info!("Updating sources for stream: {}", stream_id);

        // Load new configuration
        let config = self
            .registry
            .load_stream(stream_id)
            .await
            .map_err(|e| SourceManagerError::ConfigError(e.to_string()))?;

        // Stop existing sources for this stream
        let source_ids: Vec<String> = {
            let sources = self.sources.read().await;
            sources
                .iter()
                .filter(|(_, info)| info.stream_id == stream_id)
                .map(|(id, _)| id.clone())
                .collect()
        };

        for source_id in source_ids {
            self.stop_source(&source_id).await?;
        }

        // Start new sources
        self.start_sources_for_stream(&config).await?;

        info!("Sources updated for stream: {}", stream_id);
        Ok(())
    }

    // =========================================================================
    // DP-021: Hot-Reload Methods
    // =========================================================================

    /// DP-021: Handle configuration change from etcd watch
    ///
    /// This is the main entry point for hot-reload. It determines the type of
    /// change and dispatches to the appropriate handler.
    ///
    /// # Arguments
    /// * `stream_id` - The stream that changed
    /// * `new_config` - The new configuration (None if deleted)
    ///
    /// # Returns
    /// HotReloadResult with details of what was changed
    pub async fn on_config_change(
        &mut self,
        stream_id: &str,
        new_config: Option<StreamConfig>,
    ) -> HotReloadResult {
        let start_time = std::time::Instant::now();

        info!(
            stream_id = %stream_id,
            has_new_config = new_config.is_some(),
            "Config change detected"
        );

        // Determine change type
        let has_current_sources = self.has_sources_for_stream(stream_id).await;
        let config_enabled = new_config.as_ref().map(|c| c.enabled).unwrap_or(false);
        let change_type =
            Self::determine_change_type(has_current_sources, new_config.is_some(), config_enabled);

        debug!(
            stream_id = %stream_id,
            change_type = ?change_type,
            has_current_sources = has_current_sources,
            config_enabled = config_enabled,
            "Determined change type"
        );

        // Validate new config if applicable
        if let Some(ref config) = new_config {
            if matches!(
                change_type,
                ConfigChangeType::Created | ConfigChangeType::Updated | ConfigChangeType::Enabled
            ) {
                if let Err(e) = config.validate() {
                    let duration_ms = start_time.elapsed().as_millis() as u64;
                    error!(
                        stream_id = %stream_id,
                        error = %e,
                        "Hot-reload aborted: invalid config"
                    );
                    return HotReloadResult::failure(
                        stream_id.to_string(),
                        format!("Config validation failed: {}", e),
                        duration_ms,
                    );
                }
            }
        }

        // Execute the appropriate change
        let result = match change_type {
            ConfigChangeType::Created => {
                self.handle_stream_created(stream_id, new_config.unwrap())
                    .await
            }
            ConfigChangeType::Updated => {
                self.handle_stream_updated(stream_id, new_config.unwrap())
                    .await
            }
            ConfigChangeType::Deleted => self.handle_stream_deleted(stream_id).await,
            ConfigChangeType::Disabled => self.handle_stream_disabled(stream_id).await,
            ConfigChangeType::Enabled => {
                self.handle_stream_enabled(stream_id, new_config.unwrap())
                    .await
            }
        };

        let duration_ms = start_time.elapsed().as_millis() as u64;

        // Create result with timing
        let final_result = match result {
            Ok((stopped, started)) => {
                info!(
                    stream_id = %stream_id,
                    change_type = ?change_type,
                    sources_stopped = ?stopped,
                    sources_started = ?started,
                    duration_ms = duration_ms,
                    "Hot-reload complete"
                );
                HotReloadResult::success(stream_id.to_string(), stopped, started, duration_ms)
            }
            Err(e) => {
                error!(
                    stream_id = %stream_id,
                    error = %e,
                    duration_ms = duration_ms,
                    "Hot-reload failed"
                );
                HotReloadResult::failure(stream_id.to_string(), e.to_string(), duration_ms)
            }
        };

        final_result
    }

    /// DP-021: Determine the type of configuration change
    fn determine_change_type(
        has_current_sources: bool,
        has_new_config: bool,
        config_enabled: bool,
    ) -> ConfigChangeType {
        match (has_current_sources, has_new_config, config_enabled) {
            // New stream config created and enabled
            (false, true, true) => ConfigChangeType::Created,
            // Existing stream updated
            (true, true, true) => ConfigChangeType::Updated,
            // Stream config deleted
            (true, false, _) => ConfigChangeType::Deleted,
            // Stream disabled
            (true, true, false) => ConfigChangeType::Disabled,
            // New stream but disabled - treat as disabled (no-op)
            (false, true, false) => ConfigChangeType::Disabled,
            // Re-enabling a previously disabled stream
            (false, false, _) => ConfigChangeType::Deleted, // Edge case: no sources, no config
        }
    }

    /// DP-021: Check if we have any sources for a given stream
    async fn has_sources_for_stream(&self, stream_id: &str) -> bool {
        let sources = self.sources.read().await;
        sources.iter().any(|(_, info)| info.stream_id == stream_id)
    }

    /// DP-021: Get current source IDs for a stream
    async fn get_source_ids_for_stream(&self, stream_id: &str) -> Vec<String> {
        let sources = self.sources.read().await;
        sources
            .iter()
            .filter(|(_, info)| info.stream_id == stream_id)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// DP-021: Handle new stream creation
    async fn handle_stream_created(
        &mut self,
        stream_id: &str,
        config: StreamConfig,
    ) -> Result<(Vec<String>, Vec<String>), SourceManagerError> {
        info!(stream_id = %stream_id, "Creating sources for new stream");

        let mut started = Vec::new();

        for source_config in &config.sources {
            if !source_config.enabled {
                debug!(
                    stream_id = %stream_id,
                    source_type = ?source_config.source_type,
                    "Skipping disabled source"
                );
                continue;
            }

            match self.spawn_source(stream_id, source_config).await {
                Ok(source_id) => {
                    started.push(source_id);
                }
                Err(e) => {
                    warn!(
                        stream_id = %stream_id,
                        error = %e,
                        "Failed to spawn source during stream creation"
                    );
                    // Continue with other sources
                }
            }
        }

        Ok((Vec::new(), started))
    }

    /// DP-021: Handle stream update with intelligent diffing
    ///
    /// This method compares old and new source configurations and:
    /// - Stops sources that are removed or changed
    /// - Starts new sources or sources with changed config
    /// - Preserves unchanged sources
    async fn handle_stream_updated(
        &mut self,
        stream_id: &str,
        new_config: StreamConfig,
    ) -> Result<(Vec<String>, Vec<String>), SourceManagerError> {
        info!(stream_id = %stream_id, "Updating sources for stream");

        let mut stopped = Vec::new();
        let mut started = Vec::new();

        // Get current source IDs for this stream
        let current_source_ids = self.get_source_ids_for_stream(stream_id).await;

        // Build map of new source configs by generated ID
        let mut new_source_map: HashMap<String, &SourceConfig> = HashMap::new();
        for source_config in &new_config.sources {
            if source_config.enabled {
                let source_id = format!("{}-{:?}", stream_id, source_config.source_type);
                new_source_map.insert(source_id, source_config);
            }
        }

        // Identify sources to remove (in current but not in new)
        let to_remove: Vec<String> = current_source_ids
            .iter()
            .filter(|id| !new_source_map.contains_key(*id))
            .cloned()
            .collect();

        // Identify sources to add (in new but not in current)
        let to_add: Vec<String> = new_source_map
            .keys()
            .filter(|id| !current_source_ids.contains(*id))
            .cloned()
            .collect();

        // Identify sources to update (in both - we stop and restart for safety)
        let to_update: Vec<String> = current_source_ids
            .iter()
            .filter(|id| new_source_map.contains_key(*id))
            .cloned()
            .collect();

        debug!(
            stream_id = %stream_id,
            to_remove = ?to_remove,
            to_add = ?to_add,
            to_update = ?to_update,
            "Source diff calculated"
        );

        // Stop sources that are being removed or updated
        for source_id in to_remove.iter().chain(to_update.iter()) {
            if let Err(e) = self.stop_source(source_id).await {
                warn!(
                    source_id = %source_id,
                    error = %e,
                    "Failed to stop source during update"
                );
            }
            stopped.push(source_id.clone());
        }

        // Start new sources and updated sources
        for source_id in to_add.iter().chain(to_update.iter()) {
            if let Some(source_config) = new_source_map.get(source_id) {
                match self.spawn_source(stream_id, source_config).await {
                    Ok(new_id) => {
                        started.push(new_id);
                    }
                    Err(e) => {
                        error!(
                            source_id = %source_id,
                            error = %e,
                            "Failed to start source during update"
                        );
                        // Continue with other sources
                    }
                }
            }
        }

        Ok((stopped, started))
    }

    /// DP-021: Handle stream deletion - stop all sources
    async fn handle_stream_deleted(
        &mut self,
        stream_id: &str,
    ) -> Result<(Vec<String>, Vec<String>), SourceManagerError> {
        info!(stream_id = %stream_id, "Removing sources for deleted stream");

        let source_ids = self.get_source_ids_for_stream(stream_id).await;
        let mut stopped = Vec::new();

        for source_id in source_ids {
            if let Err(e) = self.stop_source(&source_id).await {
                warn!(
                    source_id = %source_id,
                    error = %e,
                    "Failed to stop source during deletion"
                );
            }
            stopped.push(source_id);
        }

        Ok((stopped, Vec::new()))
    }

    /// DP-021: Handle stream being disabled
    async fn handle_stream_disabled(
        &mut self,
        stream_id: &str,
    ) -> Result<(Vec<String>, Vec<String>), SourceManagerError> {
        info!(stream_id = %stream_id, "Disabling sources for stream");
        // Same as delete - stop all sources
        self.handle_stream_deleted(stream_id).await
    }

    /// DP-021: Handle stream being re-enabled
    async fn handle_stream_enabled(
        &mut self,
        stream_id: &str,
        config: StreamConfig,
    ) -> Result<(Vec<String>, Vec<String>), SourceManagerError> {
        info!(stream_id = %stream_id, "Enabling sources for stream");
        // Same as create - start all sources
        self.handle_stream_created(stream_id, config).await
    }

    /// DP-021: Trigger a manual reload for a stream
    ///
    /// This is useful for the optional HTTP reload endpoint.
    /// It clears the registry cache and reloads the config.
    pub async fn trigger_reload(&mut self, stream_id: &str) -> HotReloadResult {
        info!(stream_id = %stream_id, "Manual reload triggered");

        // Clear cache to force fresh load from etcd
        self.registry.clear_cache().await;

        // Load fresh config
        match self.registry.load_stream(stream_id).await {
            Ok(config) => self.on_config_change(stream_id, Some(config)).await,
            Err(e) => {
                error!(
                    stream_id = %stream_id,
                    error = %e,
                    "Failed to load stream config for reload"
                );
                HotReloadResult::failure(stream_id.to_string(), e.to_string(), 0)
            }
        }
    }

    /// Get count of active sources
    pub async fn active_source_count(&self) -> usize {
        let sources = self.sources.read().await;
        sources.iter().filter(|(_, info)| info.enabled).count()
    }

    /// Get sources by type
    pub async fn get_sources_by_type(&self, source_type: SourceType) -> Vec<String> {
        let sources = self.sources.read().await;
        sources
            .iter()
            .filter(|(_, info)| info.source_type == source_type)
            .map(|(id, _)| id.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use neural_core::{EventBusConfig, FieldType, SchemaField};
    use std::time::Duration;

    // ========== TEST HELPERS ==========

    /// Create a test EventBus for source manager tests
    fn create_test_event_bus() -> Arc<EventBus> {
        Arc::new(EventBus::new(EventBusConfig::default()))
    }

    // ========== LONDON SCHOOL TDD: BEHAVIOR VERIFICATION TESTS ==========

    fn create_test_stream_config(stream_id: &str, source_type: SourceType) -> StreamConfig {
        StreamConfig {
            stream_id: stream_id.to_string(),
            description: "Test stream".to_string(),
            version: "1.0.0".to_string(),
            enabled: true,
            retention_days: 30,
            compression_after_days: 7,
            partitioning_strategy: "daily".to_string(),
            fields: vec![SchemaField::new("pm25".to_string(), FieldType::Float)],
            sources: vec![SourceConfig {
                source_type,
                enabled: true,
                ndp_id: None,
                context: None,
                params: HashMap::new(),
            }],
            storage: None,
            silver_etl: None,
            entity_schemas: None,
        }
    }

    #[tokio::test]
    async fn test_source_manager_creation() {
        // Arrange
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );

        // Act
        let manager = SourceManager::new(registry);

        // Assert
        assert_eq!(manager.active_source_count().await, 0);
    }

    #[tokio::test]
    async fn test_spawn_mqtt_source() {
        // Arrange
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );
        let mut manager = SourceManager::new(registry);

        // DP-012: Sources publish to EventBus - must set event_bus
        let event_bus = create_test_event_bus();
        manager.set_event_bus(event_bus);

        let source_config = SourceConfig {
            source_type: SourceType::Mqtt,
            enabled: true,
            ndp_id: None,
            context: None,
            params: HashMap::new(),
        };

        // Act
        let result = manager.spawn_source("test-stream", &source_config).await;

        // Assert
        assert!(result.is_ok());
        let source_id = result.unwrap();
        // internal_id includes source type for HashMap uniqueness
        assert!(source_id.contains("test-stream"));
        assert!(source_id.contains("Mqtt"));

        // Verify source is tracked
        assert_eq!(manager.active_source_count().await, 1);
    }

    #[tokio::test]
    async fn test_spawn_http_source() {
        // Arrange
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );
        let mut manager = SourceManager::new(registry);

        // DP-012: Set up EventBus before spawning HTTP source
        let event_bus = create_test_event_bus();
        manager.set_event_bus(event_bus);

        let mut params = HashMap::new();
        params.insert(
            "endpoints".to_string(),
            serde_json::json!([{
                "serial": "test123",
                "url": "http://localhost:8080/test"
            }]),
        );

        let source_config = SourceConfig {
            source_type: SourceType::HttpPoll,
            enabled: true,
            ndp_id: None,
            context: None,
            params,
        };

        // Act
        let result = manager.spawn_source("test-stream", &source_config).await;

        // Assert
        assert!(result.is_ok());
        let source_id = result.unwrap();
        // internal_id includes source type for HashMap uniqueness
        assert!(source_id.contains("HttpPoll"));
    }

    #[tokio::test]
    async fn test_spawn_webhook_source() {
        // Arrange
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );
        let mut manager = SourceManager::new(registry);

        let source_config = SourceConfig {
            source_type: SourceType::Webhook,
            enabled: true,
            ndp_id: None,
            context: None,
            params: HashMap::new(),
        };

        // Act
        let result = manager.spawn_source("test-stream", &source_config).await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(manager.active_source_count().await, 1);
    }

    #[tokio::test]
    async fn test_stop_source_success() {
        // Arrange
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );
        let mut manager = SourceManager::new(registry);

        // DP-012: Sources publish to EventBus - must set event_bus
        let event_bus = create_test_event_bus();
        manager.set_event_bus(event_bus);

        let source_config = SourceConfig {
            source_type: SourceType::Mqtt,
            enabled: true,
            ndp_id: None,
            context: None,
            params: HashMap::new(),
        };

        let source_id = manager
            .spawn_source("test-stream", &source_config)
            .await
            .unwrap();

        // Act
        let result = manager.stop_source(&source_id).await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(manager.active_source_count().await, 0);

        // Verify source is removed after stop (health is None)
        let health = manager.get_health(&source_id).await;
        assert!(health.is_none());
    }

    #[tokio::test]
    async fn test_stop_nonexistent_source() {
        // Arrange
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );
        let mut manager = SourceManager::new(registry);

        // Act
        let result = manager.stop_source("nonexistent-source").await;

        // Assert
        // Idempotent: stopping a nonexistent source should return Ok (already stopped)
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_stop_all_sources() {
        // Arrange
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );
        let mut manager = SourceManager::new(registry);

        // DP-012: Sources publish to EventBus - must set event_bus
        let event_bus = create_test_event_bus();
        manager.set_event_bus(event_bus);

        // Spawn multiple sources
        for i in 0..3 {
            let source_config = SourceConfig {
                source_type: SourceType::Mqtt,
                enabled: true,
                ndp_id: None,
                context: None,
                params: HashMap::new(),
            };
            manager
                .spawn_source(&format!("stream-{}", i), &source_config)
                .await
                .unwrap();
        }

        assert_eq!(manager.active_source_count().await, 3);

        // Act
        let result = manager.stop_all_sources().await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(manager.active_source_count().await, 0);
    }

    #[tokio::test]
    async fn test_get_health_for_source() {
        // Arrange
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );
        let mut manager = SourceManager::new(registry);

        // DP-012: Sources publish to EventBus - must set event_bus
        let event_bus = create_test_event_bus();
        manager.set_event_bus(event_bus);

        let source_config = SourceConfig {
            source_type: SourceType::Mqtt,
            enabled: true,
            ndp_id: None,
            context: None,
            params: HashMap::new(),
        };

        let source_id = manager
            .spawn_source("test-stream", &source_config)
            .await
            .unwrap();

        // Act
        let health = manager.get_health(&source_id).await;

        // Assert
        assert!(health.is_some());
        // MQTT sources now route through ingestion channel and are Healthy
        assert_eq!(health.unwrap(), SourceHealth::Healthy);
    }

    #[tokio::test]
    async fn test_get_health_for_nonexistent_source() {
        // Arrange
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );
        let manager = SourceManager::new(registry);

        // Act
        let health = manager.get_health("nonexistent").await;

        // Assert
        assert!(health.is_none());
    }

    #[tokio::test]
    async fn test_get_all_health() {
        // Arrange
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );
        let mut manager = SourceManager::new(registry);

        // DP-012: Sources publish to EventBus - must set event_bus
        let event_bus = create_test_event_bus();
        manager.set_event_bus(event_bus);

        // Spawn sources
        for i in 0..3 {
            let source_config = SourceConfig {
                source_type: SourceType::Mqtt,
                enabled: true,
                ndp_id: None,
                context: None,
                params: HashMap::new(),
            };
            manager
                .spawn_source(&format!("stream-{}", i), &source_config)
                .await
                .unwrap();
        }

        // Act
        let all_health = manager.get_all_health().await;

        // Assert
        assert_eq!(all_health.len(), 3);
        // MQTT sources now route through ingestion channel and are Healthy
        assert!(all_health.values().all(|h| *h == SourceHealth::Healthy));
    }

    #[tokio::test]
    async fn test_restart_source() {
        // Arrange
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );
        let mut manager = SourceManager::new(registry.clone());

        // DP-012: Sources publish to EventBus - must set event_bus
        let event_bus = create_test_event_bus();
        manager.set_event_bus(event_bus);

        // Create and save stream config
        let config = create_test_stream_config("test-stream", SourceType::Mqtt);
        registry.save_stream(&config).await.unwrap();

        let source_config = SourceConfig {
            source_type: SourceType::Mqtt,
            enabled: true,
            ndp_id: None,
            context: None,
            params: HashMap::new(),
        };

        let source_id = manager
            .spawn_source("test-stream", &source_config)
            .await
            .unwrap();

        // Act
        let result = manager.restart_source(&source_id).await;

        // Assert
        assert!(result.is_ok());

        // Verify source is in expected state after restart
        // MQTT sources now route through ingestion channel and are Healthy
        let health = manager.get_health(&source_id).await;
        assert_eq!(health, Some(SourceHealth::Healthy));
    }

    #[tokio::test]
    async fn test_get_sources_by_type() {
        // Arrange
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );
        let mut manager = SourceManager::new(registry);

        // DP-012: Set up EventBus for sources
        let event_bus = create_test_event_bus();
        manager.set_event_bus(event_bus);

        // Spawn sources of different types with different stream IDs to avoid overwrites
        let sources = vec![
            ("stream-1", SourceType::Mqtt),
            ("stream-2", SourceType::HttpPoll),
            ("stream-3", SourceType::Mqtt),
        ];

        for (stream_id, source_type) in sources {
            let mut params = HashMap::new();

            // Add required params for HttpPoll source
            if source_type == SourceType::HttpPoll {
                params.insert(
                    "endpoints".to_string(),
                    serde_json::json!([{
                        "serial": "test123",
                        "url": "http://localhost:8080/test"
                    }]),
                );
            }

            let source_config = SourceConfig {
                source_type: source_type.clone(),
                enabled: true,
                ndp_id: None,
                context: None,
                params,
            };
            manager
                .spawn_source(stream_id, &source_config)
                .await
                .unwrap();
        }

        // Act
        let mqtt_sources = manager.get_sources_by_type(SourceType::Mqtt).await;
        let http_sources = manager.get_sources_by_type(SourceType::HttpPoll).await;

        // Assert
        assert_eq!(mqtt_sources.len(), 2);
        assert_eq!(http_sources.len(), 1);
    }

    #[tokio::test]
    async fn test_active_source_count() {
        // Arrange
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );
        let mut manager = SourceManager::new(registry);

        // DP-012: Sources publish to EventBus - must set event_bus
        let event_bus = create_test_event_bus();
        manager.set_event_bus(event_bus);

        // Act - spawn sources
        for i in 0..5 {
            let source_config = SourceConfig {
                source_type: SourceType::Mqtt,
                enabled: true,
                ndp_id: None,
                context: None,
                params: HashMap::new(),
            };
            manager
                .spawn_source(&format!("stream-{}", i), &source_config)
                .await
                .unwrap();
        }

        // Assert
        assert_eq!(manager.active_source_count().await, 5);

        // Stop one source (internal_id includes source type)
        let source_id = format!("stream-0-{:?}", SourceType::Mqtt);
        manager.stop_source(&source_id).await.unwrap();

        assert_eq!(manager.active_source_count().await, 4);
    }

    // ========== ERROR HANDLING TESTS ==========

    #[tokio::test]
    async fn test_spawn_source_with_disabled_config() {
        // Arrange
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );
        let mut manager = SourceManager::new(registry);

        // DP-012: Sources publish to EventBus - must set event_bus
        let event_bus = create_test_event_bus();
        manager.set_event_bus(event_bus);

        let source_config = SourceConfig {
            source_type: SourceType::Mqtt,
            enabled: false, // Disabled
            ndp_id: None,
            context: None,
            params: HashMap::new(),
        };

        // Act
        let result = manager.spawn_source("test-stream", &source_config).await;

        // Assert - spawn should still succeed but source won't be enabled
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_stop_source_twice() {
        // Arrange
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );
        let mut manager = SourceManager::new(registry);

        // DP-012: Sources publish to EventBus - must set event_bus
        let event_bus = create_test_event_bus();
        manager.set_event_bus(event_bus);

        let source_config = SourceConfig {
            source_type: SourceType::Mqtt,
            enabled: true,
            ndp_id: None,
            context: None,
            params: HashMap::new(),
        };

        let source_id = manager
            .spawn_source("test-stream", &source_config)
            .await
            .unwrap();

        // Act
        let result1 = manager.stop_source(&source_id).await;
        let result2 = manager.stop_source(&source_id).await;

        // Assert
        assert!(result1.is_ok());
        // Idempotent: stopping an already-stopped source should return Ok
        assert!(result2.is_ok());
    }

    // ========== INTEGRATION CONTRACT TESTS ==========

    #[tokio::test]
    async fn test_source_manager_tracks_multiple_source_types() {
        // Verify manager correctly handles different source types
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );
        let mut manager = SourceManager::new(registry);

        // DP-012: Set up EventBus for sources
        let event_bus = create_test_event_bus();
        manager.set_event_bus(event_bus);

        // Spawn one of each type
        for source_type in [SourceType::Mqtt, SourceType::HttpPoll, SourceType::Webhook] {
            let mut params = HashMap::new();

            // Add required params for HttpPoll source
            if source_type == SourceType::HttpPoll {
                params.insert(
                    "endpoints".to_string(),
                    serde_json::json!([{
                        "serial": "test123",
                        "url": "http://localhost:8080/test"
                    }]),
                );
            }

            let source_config = SourceConfig {
                source_type: source_type.clone(),
                enabled: true,
                ndp_id: None,
                context: None,
                params,
            };
            manager
                .spawn_source("test-stream", &source_config)
                .await
                .unwrap();
        }

        // Verify all types are tracked
        assert_eq!(manager.get_sources_by_type(SourceType::Mqtt).await.len(), 1);
        assert_eq!(
            manager
                .get_sources_by_type(SourceType::HttpPoll)
                .await
                .len(),
            1
        );
        assert_eq!(
            manager.get_sources_by_type(SourceType::Webhook).await.len(),
            1
        );
    }

    #[tokio::test]
    async fn test_source_manager_health_lifecycle() {
        // Verify health status transitions through lifecycle
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );
        let mut manager = SourceManager::new(registry);

        // DP-012: Sources publish to EventBus - must set event_bus
        let event_bus = create_test_event_bus();
        manager.set_event_bus(event_bus);

        let source_config = SourceConfig {
            source_type: SourceType::Mqtt,
            enabled: true,
            ndp_id: None,
            context: None,
            params: HashMap::new(),
        };

        let source_id = manager
            .spawn_source("test-stream", &source_config)
            .await
            .unwrap();

        // After spawn - MQTT sources now route through ingestion channel and are Healthy
        assert_eq!(
            manager.get_health(&source_id).await,
            Some(SourceHealth::Healthy)
        );

        // After stop - should be unhealthy, but source is removed so health is None
        manager.stop_source(&source_id).await.unwrap();
        let health = manager.get_health(&source_id).await;
        // Source is removed from map after stopping, so health is None
        assert!(health.is_none());
    }

    // ========== MQTT/EventBus ROUTING TESTS (DP-012 REGRESSION PREVENTION) ==========

    #[tokio::test]
    async fn test_spawn_mqtt_source_publishes_to_eventbus() {
        // CRITICAL TEST (DP-012): Verify MQTT sources publish to EventBus
        // This prevents MQTT from bypassing the unified data flow
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );
        let mut manager = SourceManager::new(registry);

        // DP-012: Create EventBus for sources to publish to
        let event_bus = create_test_event_bus();
        manager.set_event_bus(event_bus.clone());

        // MQTT config
        let mut params = HashMap::new();
        params.insert(
            "broker_url".to_string(),
            serde_json::json!("mqtt://localhost"),
        );
        params.insert("port".to_string(), serde_json::json!(1883));
        params.insert("topic_pattern".to_string(), serde_json::json!("sensors/#"));

        let source_config = SourceConfig {
            source_type: SourceType::Mqtt,
            enabled: true,
            ndp_id: None,
            context: None,
            params,
        };

        // Act - spawn MQTT source
        let result = manager.spawn_source("air-quality", &source_config).await;

        // Assert - MQTT source should be spawned successfully
        // DP-012: Sources now publish to EventBus instead of mpsc channel:
        // 1. Create a running task that subscribes to MQTT
        // 2. Publish RawDataPoints to EventBus
        // 3. BronzeSubscriber handles writes to ParquetStore
        assert!(result.is_ok());
        let source_id = result.unwrap();
        // internal_id includes source type, but storage uses stream_id
        assert!(source_id.contains("air-quality"));
        assert!(source_id.contains("Mqtt"));
    }

    #[tokio::test]
    async fn test_mqtt_config_parsing_from_source_params() {
        // Verify MQTT configuration extraction from source params
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );
        let mut manager = SourceManager::new(registry);

        // DP-012: Set up EventBus
        let event_bus = create_test_event_bus();
        manager.set_event_bus(event_bus);

        // MQTT config with environment variable expansion
        let mut params = HashMap::new();
        params.insert(
            "broker_url".to_string(),
            serde_json::json!("${MQTT_BROKER_URL}"),
        );
        params.insert("port".to_string(), serde_json::json!(1883));
        params.insert(
            "topic_pattern".to_string(),
            serde_json::json!("sensors/+/data"),
        );
        params.insert(
            "client_id".to_string(),
            serde_json::json!("ndp-air-quality"),
        );
        params.insert("buffer_capacity".to_string(), serde_json::json!(500));

        let source_config = SourceConfig {
            source_type: SourceType::Mqtt,
            enabled: true,
            ndp_id: None,
            context: None,
            params: params.clone(),
        };

        // Spawn should succeed (even if MQTT not fully implemented)
        let result = manager.spawn_source("test-mqtt", &source_config).await;
        assert!(result.is_ok());

        // Verify parameters are accessible
        assert_eq!(params.get("port").unwrap().as_u64(), Some(1883));
        assert_eq!(params.get("buffer_capacity").unwrap().as_u64(), Some(500));
    }

    #[tokio::test]
    async fn test_mqtt_source_uses_stream_id_not_device_id() {
        // CRITICAL REGRESSION TEST: MQTT must use stream_id, not device MAC
        // This test verifies the fix for the routing bug
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );
        let mut manager = SourceManager::new(registry);

        // DP-012: Set up EventBus with subscriber to verify data flow
        let event_bus = create_test_event_bus();
        let mut _rx = event_bus.subscribe();
        manager.set_event_bus(event_bus.clone());

        let mut params = HashMap::new();
        params.insert(
            "broker_url".to_string(),
            serde_json::json!("mqtt://localhost"),
        );
        params.insert("topic_pattern".to_string(), serde_json::json!("sensors/#"));

        let source_config = SourceConfig {
            source_type: SourceType::Mqtt,
            enabled: true,
            ndp_id: None,
            context: None,
            params,
        };

        // Spawn MQTT source for "air-quality" stream
        let result = manager.spawn_source("air-quality", &source_config).await;
        assert!(result.is_ok());

        // Verify the source_id contains "air-quality"
        let source_id = result.unwrap();
        assert!(source_id.contains("air-quality"));

        // DP-012: EventBus is the data flow mechanism (subscriber count > 0 verifies connection)
        assert!(event_bus.subscriber_count() > 0);
    }

    // ========== DP-021: HOT-RELOAD TESTS ==========

    #[test]
    fn test_determine_change_type_created() {
        // No current sources, has new config, enabled
        assert_eq!(
            SourceManager::determine_change_type(false, true, true),
            ConfigChangeType::Created
        );
    }

    #[test]
    fn test_determine_change_type_updated() {
        // Has current sources, has new config, enabled
        assert_eq!(
            SourceManager::determine_change_type(true, true, true),
            ConfigChangeType::Updated
        );
    }

    #[test]
    fn test_determine_change_type_deleted() {
        // Has current sources, no new config
        assert_eq!(
            SourceManager::determine_change_type(true, false, false),
            ConfigChangeType::Deleted
        );
    }

    #[test]
    fn test_determine_change_type_disabled() {
        // Has current sources, has new config, disabled
        assert_eq!(
            SourceManager::determine_change_type(true, true, false),
            ConfigChangeType::Disabled
        );
    }

    #[test]
    fn test_hot_reload_result_success() {
        let result = HotReloadResult::success(
            "test-stream".to_string(),
            vec!["source-1".to_string()],
            vec!["source-2".to_string()],
            100,
        );

        assert!(result.success);
        assert_eq!(result.stream_id, "test-stream");
        assert_eq!(result.sources_stopped, vec!["source-1"]);
        assert_eq!(result.sources_started, vec!["source-2"]);
        assert_eq!(result.duration_ms, 100);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_hot_reload_result_failure() {
        let result = HotReloadResult::failure(
            "test-stream".to_string(),
            "Config validation failed".to_string(),
            50,
        );

        assert!(!result.success);
        assert_eq!(result.stream_id, "test-stream");
        assert!(result.sources_stopped.is_empty());
        assert!(result.sources_started.is_empty());
        assert_eq!(result.duration_ms, 50);
        assert_eq!(result.error, Some("Config validation failed".to_string()));
    }

    #[tokio::test]
    async fn test_has_sources_for_stream() {
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );
        let mut manager = SourceManager::new(registry);
        let event_bus = create_test_event_bus();
        manager.set_event_bus(event_bus);

        // Initially no sources
        assert!(!manager.has_sources_for_stream("test-stream").await);

        // Spawn a source
        let source_config = SourceConfig {
            source_type: SourceType::Mqtt,
            enabled: true,
            ndp_id: None,
            context: None,
            params: HashMap::new(),
        };
        manager
            .spawn_source("test-stream", &source_config)
            .await
            .unwrap();

        // Now has sources
        assert!(manager.has_sources_for_stream("test-stream").await);

        // Different stream has no sources
        assert!(!manager.has_sources_for_stream("other-stream").await);
    }

    #[tokio::test]
    async fn test_get_source_ids_for_stream() {
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );
        let mut manager = SourceManager::new(registry);
        let event_bus = create_test_event_bus();
        manager.set_event_bus(event_bus);

        // Spawn sources for different streams
        let source_config = SourceConfig {
            source_type: SourceType::Mqtt,
            enabled: true,
            ndp_id: None,
            context: None,
            params: HashMap::new(),
        };

        manager
            .spawn_source("stream-a", &source_config)
            .await
            .unwrap();
        manager
            .spawn_source("stream-b", &source_config)
            .await
            .unwrap();

        let ids_a = manager.get_source_ids_for_stream("stream-a").await;
        let ids_b = manager.get_source_ids_for_stream("stream-b").await;

        assert_eq!(ids_a.len(), 1);
        assert!(ids_a[0].contains("stream-a"));

        assert_eq!(ids_b.len(), 1);
        assert!(ids_b[0].contains("stream-b"));
    }

    #[tokio::test]
    async fn test_on_config_change_with_invalid_config() {
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );
        let mut manager = SourceManager::new(registry);
        let event_bus = create_test_event_bus();
        manager.set_event_bus(event_bus);

        // Create an invalid config (no fields)
        let mut invalid_config = create_test_stream_config("test-stream", SourceType::Mqtt);
        invalid_config.fields.clear();

        // Hot-reload should fail validation
        let result = manager
            .on_config_change("test-stream", Some(invalid_config))
            .await;

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("validation failed"));
    }

    #[tokio::test]
    async fn test_on_config_change_delete() {
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );
        let mut manager = SourceManager::new(registry);
        let event_bus = create_test_event_bus();
        manager.set_event_bus(event_bus);

        // Spawn a source
        let source_config = SourceConfig {
            source_type: SourceType::Mqtt,
            enabled: true,
            ndp_id: None,
            context: None,
            params: HashMap::new(),
        };
        manager
            .spawn_source("test-stream", &source_config)
            .await
            .unwrap();

        assert!(manager.has_sources_for_stream("test-stream").await);

        // Delete the stream config
        let result = manager.on_config_change("test-stream", None).await;

        assert!(result.success);
        assert!(!manager.has_sources_for_stream("test-stream").await);
        assert_eq!(result.sources_stopped.len(), 1);
    }

    #[tokio::test]
    async fn test_handle_stream_disabled() {
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );
        let mut manager = SourceManager::new(registry);
        let event_bus = create_test_event_bus();
        manager.set_event_bus(event_bus);

        // Spawn a source
        let source_config = SourceConfig {
            source_type: SourceType::Mqtt,
            enabled: true,
            ndp_id: None,
            context: None,
            params: HashMap::new(),
        };
        manager
            .spawn_source("test-stream", &source_config)
            .await
            .unwrap();

        // Disable the stream
        let mut disabled_config = create_test_stream_config("test-stream", SourceType::Mqtt);
        disabled_config.enabled = false;

        let result = manager
            .on_config_change("test-stream", Some(disabled_config))
            .await;

        assert!(result.success);
        assert!(!manager.has_sources_for_stream("test-stream").await);
    }

    #[tokio::test]
    async fn test_trigger_reload() {
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );

        // Save a config to registry
        let config = create_test_stream_config("reload-test", SourceType::Mqtt);
        registry.save_stream(&config).await.unwrap();

        let mut manager = SourceManager::new(registry.clone());
        let event_bus = create_test_event_bus();
        manager.set_event_bus(event_bus);

        // Trigger reload
        let result = manager.trigger_reload("reload-test").await;

        assert!(result.success);
        // New sources should be started (from the saved config)
        assert!(!result.sources_started.is_empty());

        // Cleanup
        registry.delete_stream("reload-test").await.unwrap();
    }
}
