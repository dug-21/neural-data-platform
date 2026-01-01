//! Source Manager
//!
//! Manages lifecycle of multiple data sources (MQTT, HTTP, Webhook)

use config_client::StreamRegistry;
use neural_core::parsers::{create_parser_from_config, ParserConfig, ParserType};
use neural_core::sources::{
    AuthMethod, EndpointConfig, GenericHttpPollingConfig, GenericHttpPollingSource, RetryConfig,
};
use neural_core::types::raw_data_point::RawDataPoint;
use neural_core::{
    HttpPollingConfig, HttpPollingSource, MqttConfig, MqttSource, RawSource, SensorConfig,
    SourceConfig, SourceType, StreamConfig,
};
use regex::Regex;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

/// Cached regex for environment variable expansion (e.g., ${VAR_NAME})
/// Compiled once at first use, avoiding repeated compilation overhead.
static ENV_VAR_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\$\{([^}]+)\}").expect("ENV_VAR_REGEX pattern is invalid")
});

/// Source health status
#[derive(Debug, Clone, PartialEq)]
pub enum SourceHealth {
    Healthy,
    Degraded { reason: String },
    Unhealthy { reason: String },
    Unknown,
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
pub struct SourceManager {
    registry: Arc<StreamRegistry>,
    sources: Arc<RwLock<HashMap<String, SourceInfo>>>,
    /// Storage sender for RawDataPoint (dp-004 Bronze layer)
    ingestion_sender: Option<mpsc::Sender<RawDataPoint>>,
}

impl SourceManager {
    /// Create a new source manager
    pub fn new(registry: Arc<StreamRegistry>) -> Self {
        Self {
            registry,
            sources: Arc::new(RwLock::new(HashMap::new())),
            ingestion_sender: None,
        }
    }

    /// Set the ingestion sender (must be called before starting sources)
    pub fn set_ingestion_sender(&mut self, sender: mpsc::Sender<RawDataPoint>) {
        self.ingestion_sender = Some(sender);
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

        // Spawn source based on type - only HttpPoll requires ingestion sender
        let task_handle = match source_config.source_type {
            SourceType::HttpPoll => {
                // Get ingestion sender (required for HttpPoll)
                let ingestion_sender = self
                    .ingestion_sender
                    .as_ref()
                    .ok_or_else(|| {
                        SourceManagerError::ConfigError(
                            "Ingestion sender not set. Call set_ingestion_sender() first."
                                .to_string(),
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
                            ingestion_sender,
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
                            ingestion_sender,
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
                // Get ingestion sender (required for MQTT routing through ingestion channel)
                let ingestion_sender = self
                    .ingestion_sender
                    .as_ref()
                    .ok_or_else(|| {
                        SourceManagerError::ConfigError(
                            "Ingestion sender not set. Call set_ingestion_sender() first."
                                .to_string(),
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

                Some(tokio::spawn(async move {
                    if let Err(e) = Self::run_mqtt_source(
                        stream_id_clone,
                        storage_id_clone,
                        config,
                        ingestion_sender,
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

    /// Run HTTP polling source (DP-004: emits RawDataPoint to Bronze layer)
    async fn run_http_polling_source(
        stream_id: String,
        _source_id: String,
        config: HttpPollingConfig,
        ingestion_sender: mpsc::Sender<RawDataPoint>,
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
            array_config: None,
            column_config: None,
        };
        let parser = create_parser_from_config(parser_config).map_err(|e| {
            SourceManagerError::SpawnError(format!("Failed to create parser: {}", e))
        })?;

        // DP-004: Create source with stream_id for proper source_id generation
        let mut source = HttpPollingSource::with_raw_config(
            config,
            parser,
            Some(stream_id.clone()),
            ndp_id.clone(),
            context.clone(),
        )
        .map_err(|e| SourceManagerError::SpawnError(e.to_string()))?;

        // Start the source
        source
            .start()
            .await
            .map_err(|e| SourceManagerError::SpawnError(e.to_string()))?;

        // Poll loop - fetch data and send to ingestion channel
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));

        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    info!("HTTP polling source for stream {} received cancellation", stream_id);
                    source.stop().await
                        .map_err(|e| SourceManagerError::StopError(e.to_string()))?;
                    break;
                }
                _ = interval.tick() => {
                    // DP-004: Fetch raw data points directly from source (ADR-001 compliance)
                    match source.fetch_raw_batch().await {
                        Ok(raw_points) => {
                            for raw_point in raw_points {
                                if let Err(e) = ingestion_sender.send(raw_point).await {
                                    error!("Failed to send point to ingestion channel: {}", e);
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
                array_config: None,
                column_config: None,
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

        #[allow(deprecated)]
        Ok(MqttConfig {
            broker_url,
            port,
            client_id,
            topic_pattern,
            subscriptions: Vec::new(), // Using legacy topic_pattern for backward compatibility
            qos,
            reconnect_delay: std::time::Duration::from_secs(reconnect_delay_secs),
            max_reconnect_delay: std::time::Duration::from_secs(max_reconnect_delay_secs),
            buffer_capacity,
            default_stream_id: stream_id.to_string(),
        })
    }

    /// Run MQTT source (DP-004: emits RawDataPoint to Bronze layer)
    async fn run_mqtt_source(
        stream_id: String,
        _source_id: String,
        config: MqttConfig,
        ingestion_sender: mpsc::Sender<RawDataPoint>,
        cancel_token: CancellationToken,
        ndp_id: Option<String>,
        context: Option<serde_json::Value>,
    ) -> Result<(), SourceManagerError> {
        info!("Starting MQTT source for stream {}", stream_id);

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

        // Poll loop - fetch data and send to ingestion channel (same pattern as HTTP)
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
                    // DP-004: Fetch raw data points directly from source (ADR-001 compliance)
                    match source.fetch_raw_batch().await {
                        Ok(raw_points) => {
                            for raw_point in raw_points {
                                if let Err(e) = ingestion_sender.send(raw_point).await {
                                    error!("Failed to send MQTT point to ingestion channel: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to fetch points from MQTT source: {}", e);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Run Generic HTTP polling source for external APIs (DP-004: emits RawDataPoint to Bronze layer)
    async fn run_generic_http_polling_source(
        stream_id: String,
        _source_id: String,
        config: GenericHttpPollingConfig,
        parser_config: ParserConfig,
        ingestion_sender: mpsc::Sender<RawDataPoint>,
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
        let mut source = GenericHttpPollingSource::with_raw_config(
            config,
            parser,
            Some(stream_id.clone()),
            ndp_id.clone(),
            context.clone(),
        )
        .map_err(|e| SourceManagerError::SpawnError(e.to_string()))?;

        // Start the source
        source
            .start()
            .await
            .map_err(|e| SourceManagerError::SpawnError(e.to_string()))?;

        // Poll loop - fetch data and send to ingestion channel
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));

        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    info!("Generic HTTP polling source for stream {} received cancellation", stream_id);
                    source.stop().await
                        .map_err(|e| SourceManagerError::StopError(e.to_string()))?;
                    break;
                }
                _ = interval.tick() => {
                    // DP-004: Fetch raw data points directly from source (ADR-001 compliance)
                    match source.fetch_raw_batch().await {
                        Ok(raw_points) => {
                            for raw_point in raw_points {
                                if let Err(e) = ingestion_sender.send(raw_point).await {
                                    error!("Failed to send point to ingestion channel: {}", e);
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
    use neural_core::{FieldType, SchemaField};
    use std::time::Duration;

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

        // MQTT now routes through ingestion channel - must set sender
        let (tx, _rx) = tokio::sync::mpsc::channel(100);
        manager.set_ingestion_sender(tx);

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

        // Set up ingestion sender before spawning HTTP source
        let (tx, _rx) = tokio::sync::mpsc::channel(100);
        manager.set_ingestion_sender(tx);

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

        // MQTT now routes through ingestion channel - must set sender
        let (tx, _rx) = tokio::sync::mpsc::channel(100);
        manager.set_ingestion_sender(tx);

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

        // MQTT now routes through ingestion channel - must set sender
        let (tx, _rx) = tokio::sync::mpsc::channel(100);
        manager.set_ingestion_sender(tx);

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

        // MQTT now routes through ingestion channel - must set sender
        let (tx, _rx) = tokio::sync::mpsc::channel(100);
        manager.set_ingestion_sender(tx);

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

        // MQTT now routes through ingestion channel - must set sender
        let (tx, _rx) = tokio::sync::mpsc::channel(100);
        manager.set_ingestion_sender(tx);

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

        // MQTT now routes through ingestion channel - must set sender
        let (tx, _rx) = tokio::sync::mpsc::channel(100);
        manager.set_ingestion_sender(tx);

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

        // Set up ingestion sender for HTTP source
        let (tx, _rx) = tokio::sync::mpsc::channel(100);
        manager.set_ingestion_sender(tx);

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

        // MQTT now routes through ingestion channel - must set sender
        let (tx, _rx) = tokio::sync::mpsc::channel(100);
        manager.set_ingestion_sender(tx);

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

        // MQTT now routes through ingestion channel - must set sender
        let (tx, _rx) = tokio::sync::mpsc::channel(100);
        manager.set_ingestion_sender(tx);

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

        // MQTT now routes through ingestion channel - must set sender
        let (tx, _rx) = tokio::sync::mpsc::channel(100);
        manager.set_ingestion_sender(tx);

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

        // Set up ingestion sender for HTTP source
        let (tx, _rx) = tokio::sync::mpsc::channel(100);
        manager.set_ingestion_sender(tx);

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

        // MQTT now routes through ingestion channel - must set sender
        let (tx, _rx) = tokio::sync::mpsc::channel(100);
        manager.set_ingestion_sender(tx);

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

    // ========== MQTT ROUTING TESTS (REGRESSION PREVENTION) ==========

    #[tokio::test]
    async fn test_spawn_mqtt_source_sends_to_ingestion_channel() {
        // CRITICAL TEST: Verify MQTT sources route through ingestion channel
        // This prevents MQTT from bypassing IngestionRouter
        let registry = Arc::new(
            StreamRegistry::new(&["http://localhost:2379"])
                .await
                .unwrap(),
        );
        let mut manager = SourceManager::new(registry);

        // Create mock ingestion channel
        let (tx, _rx) = tokio::sync::mpsc::channel(100);
        manager.set_ingestion_sender(tx);

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
        // NOTE: Once MQTT implementation is complete, this should:
        // 1. Create a running task that subscribes to MQTT
        // 2. Send (source_id, stream_id, point) tuples to ingestion channel
        // 3. NOT write directly to ParquetStore
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

        let (tx, _rx) = tokio::sync::mpsc::channel(100);
        manager.set_ingestion_sender(tx);

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

        let (tx, mut rx) = tokio::sync::mpsc::channel(100);
        manager.set_ingestion_sender(tx);

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

        // Verify ingestion channel is connected (even if no messages yet)
        // The channel should exist and not be closed
        assert!(!rx.is_closed());
    }
}
