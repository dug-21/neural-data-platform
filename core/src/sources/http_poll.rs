//! HTTP polling data source implementation
//!
//! Provides periodic data ingestion from HTTP endpoints with:
//! - Configurable poll intervals
//! - Request timeouts
//! - Generic response parsing via ResponseParser trait
//! - Multiple sensor/endpoint support
//! - Error handling and retries

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rand;
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

use crate::error::{CoreError, CoreResult};
use crate::parsers::Parser;
use crate::traits::{HealthStatus, Source, TimeSeriesPoint};

/// Authentication method for HTTP endpoints
#[derive(Debug, Clone)]
pub enum AuthMethod {
    /// No authentication required
    None,
    /// API key as query parameter (key, value)
    QueryParam { key: String, value: String },
    /// API key as header (header_name, value)
    Header { name: String, value: String },
    /// Bearer token authentication
    Bearer { token: String },
}

impl Default for AuthMethod {
    fn default() -> Self {
        AuthMethod::None
    }
}

/// Retry configuration with exponential backoff
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Initial delay between retries
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Backoff multiplier
    pub backoff_multiplier: f64,
    /// Add jitter to prevent thundering herd
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }
}

impl RetryConfig {
    /// Calculate delay for a given retry attempt (0-indexed)
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let base_delay =
            self.initial_delay.as_millis() as f64 * self.backoff_multiplier.powi(attempt as i32);
        let capped_delay = base_delay.min(self.max_delay.as_millis() as f64);

        let final_delay = if self.jitter {
            // Add up to 25% jitter
            let jitter_factor = 1.0 + (rand::random::<f64>() * 0.25);
            capped_delay * jitter_factor
        } else {
            capped_delay
        };

        Duration::from_millis(final_delay as u64)
    }
}

/// Error classification for retry decisions
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorClassification {
    /// Transient error - should retry (network timeout, 5xx, etc)
    Transient,
    /// Permanent error - should not retry (4xx except 429, parse error, etc)
    Permanent,
    /// Rate limited - should retry with backoff (429)
    RateLimited { retry_after: Option<Duration> },
}

/// HTTP polling error with classification
#[derive(Debug)]
pub struct PollingError {
    /// The underlying error message
    pub message: String,
    /// Classification for retry logic
    pub classification: ErrorClassification,
    /// HTTP status code if applicable
    pub status_code: Option<u16>,
    /// Endpoint that caused the error
    pub endpoint_id: String,
}

impl PollingError {
    pub fn transient(endpoint_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            classification: ErrorClassification::Transient,
            status_code: None,
            endpoint_id: endpoint_id.into(),
        }
    }

    pub fn permanent(endpoint_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            classification: ErrorClassification::Permanent,
            status_code: None,
            endpoint_id: endpoint_id.into(),
        }
    }

    pub fn rate_limited(endpoint_id: impl Into<String>, retry_after: Option<Duration>) -> Self {
        Self {
            message: "Rate limited".to_string(),
            classification: ErrorClassification::RateLimited { retry_after },
            status_code: Some(429),
            endpoint_id: endpoint_id.into(),
        }
    }

    pub fn from_status(
        endpoint_id: impl Into<String>,
        status: u16,
        body: impl Into<String>,
    ) -> Self {
        let endpoint_id = endpoint_id.into();
        let message = body.into();

        let classification = match status {
            429 => ErrorClassification::RateLimited { retry_after: None },
            400..=499 => ErrorClassification::Permanent,
            500..=599 => ErrorClassification::Transient,
            _ => ErrorClassification::Permanent,
        };

        Self {
            message,
            classification,
            status_code: Some(status),
            endpoint_id,
        }
    }

    /// Check if this error should be retried
    pub fn should_retry(&self) -> bool {
        matches!(
            self.classification,
            ErrorClassification::Transient | ErrorClassification::RateLimited { .. }
        )
    }
}

impl std::fmt::Display for PollingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.endpoint_id, self.message)
    }
}

impl std::error::Error for PollingError {}

/// Configuration for a single HTTP endpoint
#[derive(Debug, Clone)]
pub struct EndpointConfig {
    /// Unique identifier for this endpoint
    pub endpoint_id: String,
    /// Full URL to poll
    pub url: String,
    /// Location identifier for data points
    pub location_id: String,
    /// Name of the parser to use (registered in ParserRegistry)
    pub parser_name: String,
    /// Authentication method
    pub auth: AuthMethod,
    /// Custom headers to include
    pub headers: HashMap<String, String>,
    /// Whether this endpoint is enabled
    pub enabled: bool,
}

impl EndpointConfig {
    pub fn new(
        endpoint_id: impl Into<String>,
        url: impl Into<String>,
        location_id: impl Into<String>,
        parser_name: impl Into<String>,
    ) -> Self {
        Self {
            endpoint_id: endpoint_id.into(),
            url: url.into(),
            location_id: location_id.into(),
            parser_name: parser_name.into(),
            auth: AuthMethod::default(),
            headers: HashMap::new(),
            enabled: true,
        }
    }

    pub fn with_auth(mut self, auth: AuthMethod) -> Self {
        self.auth = auth;
        self
    }

    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(name.into(), value.into());
        self
    }
}

/// Registry for managing ResponseParser implementations
pub struct ParserRegistry {
    parsers: HashMap<String, Arc<dyn ResponseParser + Send + Sync>>,
}

impl ParserRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            parsers: HashMap::new(),
        }
    }

    /// Register a parser with the given name
    pub fn register<P: ResponseParser + Send + Sync + 'static>(
        &mut self,
        name: impl Into<String>,
        parser: P,
    ) {
        self.parsers.insert(name.into(), Arc::new(parser));
    }

    /// Get a parser by name
    pub fn get(&self, name: &str) -> Option<Arc<dyn ResponseParser + Send + Sync>> {
        self.parsers.get(name).cloned()
    }

    /// Check if a parser is registered
    pub fn contains(&self, name: &str) -> bool {
        self.parsers.contains_key(name)
    }

    /// Get all registered parser names
    pub fn parser_names(&self) -> Vec<String> {
        self.parsers.keys().cloned().collect()
    }

    /// Create a default registry with standard parsers pre-registered
    pub fn with_default_parsers() -> Self {
        let mut registry = Self::new();
        // Register OpenWeatherMap parsers
        registry.register(
            "openweathermap_current_weather",
            super::parsers::WeatherParser::new(),
        );
        registry.register(
            "openweathermap_air_pollution",
            super::parsers::AirPollutionParser::new(),
        );
        registry
    }
}

impl Default for ParserRegistry {
    fn default() -> Self {
        Self::with_default_parsers()
    }
}

/// Trait for parsing HTTP response bodies into time series points
///
/// Implement this trait to add support for new HTTP APIs.
/// The parser is responsible for converting API-specific JSON formats
/// into the platform's TimeSeriesPoint format.
pub trait ResponseParser: Send + Sync + 'static {
    /// Parse the response body into time series points
    ///
    /// # Arguments
    /// * `response_body` - The raw HTTP response body as a string
    /// * `location_id` - The location/sensor identifier
    /// * `timestamp` - The timestamp for the measurement
    ///
    /// # Returns
    /// A vector of time series points extracted from the response
    fn parse(
        &self,
        response_body: &str,
        location_id: &str,
        timestamp: DateTime<Utc>,
    ) -> CoreResult<Vec<TimeSeriesPoint>>;

    /// Name of this parser for logging
    fn name(&self) -> &'static str;
}

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

/// HTTP polling data source
pub struct HttpPollingSource {
    config: HttpPollingConfig,
    parser: Arc<dyn Parser + Send + Sync>,
    client: Client,
    receiver: Arc<Mutex<mpsc::Receiver<TimeSeriesPoint>>>,
    sender: mpsc::Sender<TimeSeriesPoint>,
    is_running: Arc<Mutex<bool>>,
    last_successful_poll: Arc<Mutex<HashMap<String, DateTime<Utc>>>>,
}

impl HttpPollingSource {
    /// Create a new HTTP polling source with injected parser
    pub fn new(
        config: HttpPollingConfig,
        parser: Box<dyn Parser + Send + Sync>,
    ) -> CoreResult<Self> {
        let (sender, receiver) = mpsc::channel(config.buffer_capacity);

        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| CoreError::Source(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            config,
            parser: Arc::from(parser),
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

        let body = response
            .text()
            .await
            .map_err(|e| CoreError::Source(format!("Failed to read response body: {}", e)))?;

        let json: Value = serde_json::from_str(&body)
            .map_err(|e| CoreError::Source(format!("Failed to parse JSON: {}", e)))?;

        let timestamp = Utc::now();
        self.parser.parse(&json, timestamp)
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
                    (now - *last_time).num_seconds()
                        > (self.config.poll_interval.as_secs() * 2) as i64
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
            details.insert(
                "unhealthy_sensors".to_string(),
                format!("{:?}", unhealthy_sensors),
            );
            Ok(HealthStatus {
                healthy: false,
                message: format!("All sensors unhealthy: {:?}", unhealthy_sensors),
                details,
            })
        } else {
            details.insert("status".to_string(), "some_sensors_unhealthy".to_string());
            details.insert(
                "unhealthy_sensors".to_string(),
                format!("{:?}", unhealthy_sensors),
            );
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
            parser: self.parser.clone(),
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

/// Generic HTTP polling configuration using new types
#[derive(Debug, Clone)]
pub struct GenericHttpPollingConfig {
    /// Endpoints to poll
    pub endpoints: Vec<EndpointConfig>,
    /// Poll interval
    pub poll_interval: Duration,
    /// HTTP request timeout
    pub timeout: Duration,
    /// Retry configuration
    pub retry_config: RetryConfig,
    /// Buffer capacity for internal channel
    pub buffer_capacity: usize,
}

impl Default for GenericHttpPollingConfig {
    fn default() -> Self {
        Self {
            endpoints: Vec::new(),
            poll_interval: Duration::from_secs(60),
            timeout: Duration::from_secs(30),
            retry_config: RetryConfig::default(),
            buffer_capacity: 1000,
        }
    }
}

/// Generic HTTP polling source with pluggable parsers
pub struct GenericHttpPollingSource {
    config: GenericHttpPollingConfig,
    client: Client,
    parser: Arc<dyn Parser + Send + Sync>,
    receiver: Arc<Mutex<mpsc::Receiver<TimeSeriesPoint>>>,
    sender: mpsc::Sender<TimeSeriesPoint>,
    is_running: Arc<Mutex<bool>>,
    last_successful_poll: Arc<Mutex<HashMap<String, DateTime<Utc>>>>,
    endpoint_retry_counts: Arc<Mutex<HashMap<String, u32>>>,
}

impl GenericHttpPollingSource {
    /// Create a new generic HTTP polling source with injected parser
    pub fn new(
        config: GenericHttpPollingConfig,
        parser: Box<dyn Parser + Send + Sync>,
    ) -> CoreResult<Self> {
        let (sender, receiver) = mpsc::channel(config.buffer_capacity);

        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| CoreError::Source(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            config,
            client,
            parser: Arc::from(parser),
            receiver: Arc::new(Mutex::new(receiver)),
            sender,
            is_running: Arc::new(Mutex::new(false)),
            last_successful_poll: Arc::new(Mutex::new(HashMap::new())),
            endpoint_retry_counts: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Create with default parser registry (DEPRECATED - use new() with parser injection)
    #[deprecated(since = "0.1.0", note = "Use new() with parser injection instead")]
    pub fn with_default_parsers(config: GenericHttpPollingConfig) -> CoreResult<Self> {
        // Provide backward compatibility with default FlatJson parser
        let parser_config = crate::parsers::ParserConfig {
            parser_type: crate::parsers::ParserType::FlatJson,
            location_id_field: "serialno".to_string(),
            default_location_id: Some("unknown".to_string()),
            skip_fields: vec!["serialno".to_string(), "firmware".to_string()],
            field_mappings: None,
            array_config: None,
            default_tags: [("source".to_string(), "http".to_string())].into(),
        };
        let parser = crate::parsers::FlatJsonParser::from_config(parser_config)
            .map_err(|e| CoreError::Config(format!("Failed to create default parser: {}", e)))?;
        Self::new(config, Box::new(parser))
    }

    /// Build the request with authentication
    fn build_request(
        &self,
        endpoint: &EndpointConfig,
    ) -> Result<reqwest::RequestBuilder, CoreError> {
        let mut url = endpoint.url.clone();

        // Handle query parameter auth
        if let AuthMethod::QueryParam { key, value } = &endpoint.auth {
            let separator = if url.contains('?') { "&" } else { "?" };
            url = format!("{}{}{}={}", url, separator, key, value);
        }

        let mut request = self.client.get(&url);

        // Add headers
        for (name, value) in &endpoint.headers {
            request = request.header(name, value);
        }

        // Handle header/bearer auth
        match &endpoint.auth {
            AuthMethod::Header { name, value } => {
                request = request.header(name, value);
            }
            AuthMethod::Bearer { token } => {
                request = request.header("Authorization", format!("Bearer {}", token));
            }
            _ => {}
        }

        Ok(request)
    }

    /// Poll a single endpoint with retry logic
    async fn poll_endpoint(
        &self,
        endpoint: &EndpointConfig,
    ) -> Result<Vec<TimeSeriesPoint>, PollingError> {
        let mut retry_count = 0;

        loop {
            debug!(
                "Polling endpoint: {} (attempt {})",
                endpoint.endpoint_id,
                retry_count + 1
            );

            let request = self.build_request(endpoint).map_err(|e| {
                PollingError::permanent(
                    &endpoint.endpoint_id,
                    format!("Failed to build request: {}", e),
                )
            })?;

            match request.send().await {
                Ok(response) => {
                    let status = response.status().as_u16();

                    if status >= 200 && status < 300 {
                        let body = response.text().await.map_err(|e| {
                            PollingError::transient(
                                &endpoint.endpoint_id,
                                format!("Failed to read response body: {}", e),
                            )
                        })?;

                        // Parse JSON response
                        let json: Value = serde_json::from_str(&body).map_err(|e| {
                            PollingError::permanent(
                                &endpoint.endpoint_id,
                                format!("Failed to parse JSON: {}", e),
                            )
                        })?;

                        let timestamp = Utc::now();

                        // Use injected parser (Parser trait)
                        let points = self.parser.parse(&json, timestamp).map_err(|e| {
                            PollingError::permanent(
                                &endpoint.endpoint_id,
                                format!("Failed to parse response: {}", e),
                            )
                        })?;

                        // Update successful poll time
                        let mut last_poll = self.last_successful_poll.lock().await;
                        last_poll.insert(endpoint.endpoint_id.clone(), timestamp);

                        // Reset retry count
                        let mut retry_counts = self.endpoint_retry_counts.lock().await;
                        retry_counts.insert(endpoint.endpoint_id.clone(), 0);

                        return Ok(points);
                    } else {
                        let body = response.text().await.unwrap_or_default();
                        let error = PollingError::from_status(&endpoint.endpoint_id, status, body);

                        if !error.should_retry()
                            || retry_count >= self.config.retry_config.max_retries
                        {
                            return Err(error);
                        }

                        // Wait before retry
                        let delay = self.config.retry_config.delay_for_attempt(retry_count);
                        warn!(
                            "Endpoint {} returned status {}, retrying in {:?}",
                            endpoint.endpoint_id, status, delay
                        );
                        tokio::time::sleep(delay).await;
                        retry_count += 1;
                    }
                }
                Err(e) => {
                    let error = if e.is_timeout() || e.is_connect() {
                        PollingError::transient(
                            &endpoint.endpoint_id,
                            format!("Request failed: {}", e),
                        )
                    } else {
                        PollingError::permanent(
                            &endpoint.endpoint_id,
                            format!("Request failed: {}", e),
                        )
                    };

                    if !error.should_retry() || retry_count >= self.config.retry_config.max_retries
                    {
                        return Err(error);
                    }

                    let delay = self.config.retry_config.delay_for_attempt(retry_count);
                    warn!(
                        "Endpoint {} request failed: {}, retrying in {:?}",
                        endpoint.endpoint_id, e, delay
                    );
                    tokio::time::sleep(delay).await;
                    retry_count += 1;
                }
            }
        }
    }

    /// Poll all enabled endpoints
    async fn poll_all_endpoints(&self) -> CoreResult<()> {
        for endpoint in &self.config.endpoints {
            if !endpoint.enabled {
                continue;
            }

            match self.poll_endpoint(endpoint).await {
                Ok(points) => {
                    debug!(
                        "Successfully polled endpoint {} - got {} points",
                        endpoint.endpoint_id,
                        points.len()
                    );

                    for point in points {
                        if let Err(e) = self.sender.send(point).await {
                            warn!("Failed to send point to channel: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to poll endpoint {}: {}", endpoint.endpoint_id, e);
                }
            }
        }

        Ok(())
    }

    /// Background polling loop
    async fn polling_loop(&self) -> CoreResult<()> {
        let mut interval = tokio::time::interval(self.config.poll_interval);

        while *self.is_running.lock().await {
            interval.tick().await;

            if let Err(e) = self.poll_all_endpoints().await {
                error!("Polling error: {}", e);
            }
        }

        Ok(())
    }

    /// Start the source
    pub async fn start(&mut self) -> CoreResult<()> {
        info!("Starting generic HTTP polling source");

        let enabled_count = self.config.endpoints.iter().filter(|e| e.enabled).count();
        if enabled_count == 0 {
            return Err(CoreError::Source(
                "No enabled endpoints configured".to_string(),
            ));
        }

        *self.is_running.lock().await = true;

        // Clone for background task
        let source_clone = Self {
            config: self.config.clone(),
            parser: self.parser.clone(),
            client: self.client.clone(),
            receiver: self.receiver.clone(),
            sender: self.sender.clone(),
            is_running: self.is_running.clone(),
            last_successful_poll: self.last_successful_poll.clone(),
            endpoint_retry_counts: self.endpoint_retry_counts.clone(),
        };

        tokio::spawn(async move {
            if let Err(e) = source_clone.polling_loop().await {
                error!("Generic HTTP polling loop failed: {}", e);
            }
        });

        // Initial poll
        self.poll_all_endpoints().await?;

        Ok(())
    }

    /// Stop the source
    pub async fn stop(&mut self) -> CoreResult<()> {
        info!("Stopping generic HTTP polling source");
        *self.is_running.lock().await = false;
        Ok(())
    }
}

#[async_trait]
impl Source for GenericHttpPollingSource {
    async fn fetch(&self) -> CoreResult<Vec<TimeSeriesPoint>> {
        let mut receiver = self.receiver.lock().await;
        let mut points = Vec::new();

        while let Ok(point) = receiver.try_recv() {
            points.push(point);
        }

        Ok(points)
    }

    async fn health_check(&self) -> CoreResult<HealthStatus> {
        let is_running = *self.is_running.lock().await;

        let mut details = HashMap::new();
        details.insert(
            "source_type".to_string(),
            "generic_http_polling".to_string(),
        );
        details.insert("is_running".to_string(), is_running.to_string());

        if !is_running {
            return Ok(HealthStatus {
                healthy: false,
                message: "Generic HTTP polling source not running".to_string(),
                details,
            });
        }

        let last_poll = self.last_successful_poll.lock().await;
        let now = Utc::now();

        let unhealthy_endpoints: Vec<_> = self
            .config
            .endpoints
            .iter()
            .filter(|ep| ep.enabled)
            .filter(|ep| {
                if let Some(last_time) = last_poll.get(&ep.endpoint_id) {
                    (now - *last_time).num_seconds()
                        > (self.config.poll_interval.as_secs() * 2) as i64
                } else {
                    true
                }
            })
            .map(|ep| ep.endpoint_id.clone())
            .collect();

        if unhealthy_endpoints.is_empty() {
            details.insert("status".to_string(), "all_endpoints_healthy".to_string());
            Ok(HealthStatus {
                healthy: true,
                message: "All endpoints operational".to_string(),
                details,
            })
        } else {
            details.insert("status".to_string(), "some_endpoints_unhealthy".to_string());
            details.insert(
                "unhealthy_endpoints".to_string(),
                format!("{:?}", unhealthy_endpoints),
            );
            Ok(HealthStatus {
                healthy: false,
                message: format!("Some endpoints unhealthy: {:?}", unhealthy_endpoints),
                details,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parsers::{FlatJsonParser, ParserConfig, ParserType};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn create_default_parser() -> Box<dyn Parser + Send + Sync> {
        let parser_config = ParserConfig {
            parser_type: ParserType::FlatJson,
            location_id_field: "serialno".to_string(),
            skip_fields: vec!["serialno", "firmware", "model", "ledMode"]
                .into_iter()
                .map(String::from)
                .collect(),
            default_tags: [("source".to_string(), "http".to_string())].into(),
            ..Default::default()
        };
        Box::new(FlatJsonParser::from_config(parser_config).unwrap())
    }

    #[tokio::test]
    async fn test_http_source_creation() {
        let config = HttpPollingConfig::default();
        let source = HttpPollingSource::new(config, create_default_parser());

        assert!(source.is_ok());
    }

    #[tokio::test]
    async fn test_health_check_not_running() {
        let config = HttpPollingConfig::default();
        let source = HttpPollingSource::new(config, create_default_parser()).unwrap();

        let health = source.health_check().await.unwrap();
        assert!(!health.healthy);
    }

    #[tokio::test]
    async fn test_parse_with_default_parser() {
        use crate::parsers::{FlatJsonParser, ParserConfig, ParserType};
        use chrono::Utc;
        use serde_json::json;

        let parser_config = ParserConfig {
            parser_type: ParserType::FlatJson,
            location_id_field: "serialno".to_string(),
            skip_fields: vec!["serialno".to_string(), "firmware".to_string()],
            default_tags: [("source".to_string(), "http".to_string())].into(),
            ..Default::default()
        };

        let parser = FlatJsonParser::from_config(parser_config).unwrap();

        let json = json!({
            "serialno": "ABC123",
            "firmware": "3.4.1",
            "pm02": 12.5,
            "rco2": 450.0,
            "atmp": 22.3,
            "rhum": 55.0,
            "wifi": -45,
            "pm10": 15.2,
            "pm01": 8.1,
            "tvocIndex": 42.0,
            "noxIndex": 1.5,
            "tvocRaw": 120.0,
            "noxRaw": 25.0
        });

        let points = parser.parse(&json, Utc::now()).unwrap();

        // Should extract 11 metrics (pm02, rco2, atmp, rhum, wifi, pm10, pm01, tvocIndex, noxIndex, tvocRaw, noxRaw)
        assert_eq!(points.len(), 11);

        // Check metrics with ORIGINAL field names
        let metric_names: Vec<String> = points
            .iter()
            .map(|p| p.tags.get("metric").unwrap().clone())
            .collect();

        assert!(metric_names.contains(&"pm02".to_string()));
        assert!(metric_names.contains(&"rco2".to_string())); // NOT "co2"
        assert!(metric_names.contains(&"atmp".to_string())); // NOT "temperature"
        assert!(metric_names.contains(&"rhum".to_string())); // NOT "humidity"
        assert!(metric_names.contains(&"pm10".to_string()));
        assert!(metric_names.contains(&"pm01".to_string()));
        assert!(metric_names.contains(&"tvocIndex".to_string()));
        assert!(metric_names.contains(&"noxIndex".to_string()));
        assert!(metric_names.contains(&"tvocRaw".to_string()));
        assert!(metric_names.contains(&"noxRaw".to_string()));

        // Verify no renamed fields
        assert!(!metric_names.contains(&"co2".to_string()));
        assert!(!metric_names.contains(&"temperature".to_string()));
        assert!(!metric_names.contains(&"humidity".to_string()));

        // Verify source ID
        assert!(points.iter().all(|p| p.location_id == "ABC123"));
        assert!(points
            .iter()
            .all(|p| p.tags.get("source") == Some(&"http".to_string())));
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
            "tvocIndex": 100
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

        let source = HttpPollingSource::new(config, create_default_parser()).unwrap();
        let points = source.poll_sensor(&sensor).await.unwrap();

        // Should extract 7 numeric fields (pm02, rco2, atmp, rhum, wifi, pm10, tvocIndex)
        assert_eq!(points.len(), 7);

        // Check fields use ORIGINAL names
        assert!(points
            .iter()
            .any(|p| p.tags.get("metric") == Some(&"pm02".to_string()) && p.value == 10.5));
        assert!(points
            .iter()
            .any(|p| p.tags.get("metric") == Some(&"rco2".to_string()) && p.value == 400.0));
        assert!(points
            .iter()
            .any(|p| p.tags.get("metric") == Some(&"atmp".to_string()) && p.value == 21.0));
        assert!(points
            .iter()
            .any(|p| p.tags.get("metric") == Some(&"rhum".to_string()) && p.value == 50.0));
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

        let source = HttpPollingSource::new(config, create_default_parser()).unwrap();
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

        let source = HttpPollingSource::new(config, create_default_parser()).unwrap();
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

        let source = HttpPollingSource::new(config, create_default_parser()).unwrap();
        let result = source.poll_sensor(&sensor).await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_start_no_sensors() {
        let config = HttpPollingConfig {
            sensors: vec![],
            ..Default::default()
        };

        let mut source = HttpPollingSource::new(config, create_default_parser()).unwrap();
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

        let source = HttpPollingSource::new(config, create_default_parser()).unwrap();
        assert_eq!(source.sender.capacity(), 500);
    }

    // Tests for AuthMethod
    #[test]
    fn test_auth_method_default() {
        let auth = AuthMethod::default();
        assert!(matches!(auth, AuthMethod::None));
    }

    #[test]
    fn test_auth_method_query_param() {
        let auth = AuthMethod::QueryParam {
            key: "api_key".to_string(),
            value: "secret123".to_string(),
        };

        if let AuthMethod::QueryParam { key, value } = auth {
            assert_eq!(key, "api_key");
            assert_eq!(value, "secret123");
        } else {
            panic!("Expected QueryParam variant");
        }
    }

    #[test]
    fn test_auth_method_header() {
        let auth = AuthMethod::Header {
            name: "X-API-Key".to_string(),
            value: "secret123".to_string(),
        };

        if let AuthMethod::Header { name, value } = auth {
            assert_eq!(name, "X-API-Key");
            assert_eq!(value, "secret123");
        } else {
            panic!("Expected Header variant");
        }
    }

    #[test]
    fn test_auth_method_bearer() {
        let auth = AuthMethod::Bearer {
            token: "token123".to_string(),
        };

        if let AuthMethod::Bearer { token } = auth {
            assert_eq!(token, "token123");
        } else {
            panic!("Expected Bearer variant");
        }
    }

    // Tests for RetryConfig
    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.initial_delay, Duration::from_secs(1));
        assert_eq!(config.max_delay, Duration::from_secs(60));
        assert_eq!(config.backoff_multiplier, 2.0);
        assert!(config.jitter);
    }

    #[test]
    fn test_retry_config_delay_for_attempt_no_jitter() {
        let config = RetryConfig {
            max_retries: 5,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            jitter: false,
        };

        // Attempt 0: 1s * 2^0 = 1s
        let delay0 = config.delay_for_attempt(0);
        assert_eq!(delay0, Duration::from_secs(1));

        // Attempt 1: 1s * 2^1 = 2s
        let delay1 = config.delay_for_attempt(1);
        assert_eq!(delay1, Duration::from_secs(2));

        // Attempt 2: 1s * 2^2 = 4s
        let delay2 = config.delay_for_attempt(2);
        assert_eq!(delay2, Duration::from_secs(4));

        // Attempt 3: 1s * 2^3 = 8s
        let delay3 = config.delay_for_attempt(3);
        assert_eq!(delay3, Duration::from_secs(8));
    }

    #[test]
    fn test_retry_config_delay_for_attempt_with_cap() {
        let config = RetryConfig {
            max_retries: 10,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(10),
            backoff_multiplier: 2.0,
            jitter: false,
        };

        // Attempt 10 would be 1024s, but should be capped at 10s
        let delay = config.delay_for_attempt(10);
        assert_eq!(delay, Duration::from_secs(10));
    }

    #[test]
    fn test_retry_config_delay_for_attempt_with_jitter() {
        let config = RetryConfig {
            max_retries: 3,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            jitter: true,
        };

        // With jitter enabled, delay should be between base_delay and base_delay * 1.25
        let delay = config.delay_for_attempt(0);
        assert!(delay >= Duration::from_secs(1));
        assert!(delay <= Duration::from_millis(1250)); // 1s * 1.25

        // Attempt 2: base is 4s, with jitter should be 4-5s
        let delay2 = config.delay_for_attempt(2);
        assert!(delay2 >= Duration::from_secs(4));
        assert!(delay2 <= Duration::from_secs(5));
    }

    #[test]
    fn test_retry_config_custom_backoff_multiplier() {
        let config = RetryConfig {
            max_retries: 3,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 3.0,
            jitter: false,
        };

        // Attempt 0: 1s * 3^0 = 1s
        assert_eq!(config.delay_for_attempt(0), Duration::from_secs(1));
        // Attempt 1: 1s * 3^1 = 3s
        assert_eq!(config.delay_for_attempt(1), Duration::from_secs(3));
        // Attempt 2: 1s * 3^2 = 9s
        assert_eq!(config.delay_for_attempt(2), Duration::from_secs(9));
    }

    // Tests for EndpointConfig
    #[test]
    fn test_endpoint_config_new() {
        let config = EndpointConfig::new(
            "endpoint1",
            "https://api.example.com/data",
            "location1",
            "json_parser",
        );

        assert_eq!(config.endpoint_id, "endpoint1");
        assert_eq!(config.url, "https://api.example.com/data");
        assert_eq!(config.location_id, "location1");
        assert_eq!(config.parser_name, "json_parser");
        assert!(matches!(config.auth, AuthMethod::None));
        assert!(config.headers.is_empty());
        assert!(config.enabled);
    }

    #[test]
    fn test_endpoint_config_with_auth() {
        let config = EndpointConfig::new(
            "endpoint1",
            "https://api.example.com/data",
            "location1",
            "json_parser",
        )
        .with_auth(AuthMethod::Bearer {
            token: "test_token".to_string(),
        });

        if let AuthMethod::Bearer { token } = config.auth {
            assert_eq!(token, "test_token");
        } else {
            panic!("Expected Bearer auth method");
        }
    }

    #[test]
    fn test_endpoint_config_with_header() {
        let config = EndpointConfig::new(
            "endpoint1",
            "https://api.example.com/data",
            "location1",
            "json_parser",
        )
        .with_header("Content-Type", "application/json")
        .with_header("Accept", "application/json");

        assert_eq!(config.headers.len(), 2);
        assert_eq!(
            config.headers.get("Content-Type").unwrap(),
            "application/json"
        );
        assert_eq!(config.headers.get("Accept").unwrap(), "application/json");
    }

    #[test]
    fn test_endpoint_config_builder_chain() {
        let config = EndpointConfig::new(
            "weather_api",
            "https://api.weather.com/v1/current",
            "station_001",
            "weather_parser",
        )
        .with_auth(AuthMethod::QueryParam {
            key: "apikey".to_string(),
            value: "secret123".to_string(),
        })
        .with_header("User-Agent", "WeatherApp/1.0")
        .with_header("Accept-Encoding", "gzip");

        assert_eq!(config.endpoint_id, "weather_api");
        assert_eq!(config.url, "https://api.weather.com/v1/current");
        assert_eq!(config.location_id, "station_001");
        assert_eq!(config.parser_name, "weather_parser");

        if let AuthMethod::QueryParam { key, value } = config.auth {
            assert_eq!(key, "apikey");
            assert_eq!(value, "secret123");
        } else {
            panic!("Expected QueryParam auth method");
        }

        assert_eq!(config.headers.len(), 2);
        assert_eq!(config.headers.get("User-Agent").unwrap(), "WeatherApp/1.0");
        assert_eq!(config.headers.get("Accept-Encoding").unwrap(), "gzip");
        assert!(config.enabled);
    }

    // ParserRegistry tests
    #[test]
    fn test_parser_registry_creation() {
        let registry = ParserRegistry::new();
        assert_eq!(registry.parser_names().len(), 0);
    }

    #[test]
    fn test_parser_registry_register_and_get() {
        use crate::sources::parsers::WeatherParser;

        let mut registry = ParserRegistry::new();
        let parser = WeatherParser::new();

        registry.register("weather", parser);

        assert!(registry.contains("weather"));
        assert_eq!(registry.parser_names().len(), 1);

        let retrieved = registry.get("weather");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name(), "openweathermap_current_weather");
    }

    #[test]
    fn test_parser_registry_get_nonexistent() {
        let registry = ParserRegistry::new();
        let result = registry.get("nonexistent");
        assert!(result.is_none());
    }

    #[test]
    fn test_parser_registry_contains() {
        use crate::sources::parsers::AirPollutionParser;

        let mut registry = ParserRegistry::new();
        registry.register("pollution", AirPollutionParser::new());

        assert!(registry.contains("pollution"));
        assert!(!registry.contains("weather"));
    }

    #[test]
    fn test_parser_registry_multiple_parsers() {
        use crate::sources::parsers::{AirPollutionParser, WeatherParser};

        let mut registry = ParserRegistry::new();
        registry.register("weather", WeatherParser::new());
        registry.register("pollution", AirPollutionParser::new());

        assert_eq!(registry.parser_names().len(), 2);
        assert!(registry.contains("weather"));
        assert!(registry.contains("pollution"));

        let names = registry.parser_names();
        assert!(names.contains(&"weather".to_string()));
        assert!(names.contains(&"pollution".to_string()));
    }

    #[test]
    fn test_parser_registry_default() {
        let registry = ParserRegistry::default();

        // Default registry should have pre-registered parsers
        assert!(registry.parser_names().len() >= 2);
        assert!(registry.contains("openweathermap_current_weather"));
        assert!(registry.contains("openweathermap_air_pollution"));
    }

    #[test]
    fn test_parser_registry_with_default_parsers() {
        let registry = ParserRegistry::with_default_parsers();

        assert!(registry.contains("openweathermap_current_weather"));
        assert!(registry.contains("openweathermap_air_pollution"));

        let weather_parser = registry.get("openweathermap_current_weather");
        assert!(weather_parser.is_some());
        assert_eq!(
            weather_parser.unwrap().name(),
            "openweathermap_current_weather"
        );

        let pollution_parser = registry.get("openweathermap_air_pollution");
        assert!(pollution_parser.is_some());
        assert_eq!(
            pollution_parser.unwrap().name(),
            "openweathermap_air_pollution"
        );
    }

    #[test]
    fn test_parser_registry_parser_names_returns_correct_names() {
        use crate::sources::parsers::{AirPollutionParser, WeatherParser};

        let mut registry = ParserRegistry::new();
        registry.register("custom_weather", WeatherParser::new());
        registry.register("custom_pollution", AirPollutionParser::new());

        let names = registry.parser_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"custom_weather".to_string()));
        assert!(names.contains(&"custom_pollution".to_string()));
    }

    #[test]
    fn test_parser_registry_overwrite_parser() {
        use crate::sources::parsers::WeatherParser;

        let mut registry = ParserRegistry::new();
        registry.register("parser1", WeatherParser::new());

        assert_eq!(registry.parser_names().len(), 1);

        // Registering with the same name should overwrite
        registry.register("parser1", WeatherParser::new());

        assert_eq!(registry.parser_names().len(), 1);
        assert!(registry.contains("parser1"));
    }

    // Error classification tests
    #[test]
    fn test_error_classification_variants() {
        // Test Transient variant
        let transient = ErrorClassification::Transient;
        assert!(matches!(transient, ErrorClassification::Transient));

        // Test Permanent variant
        let permanent = ErrorClassification::Permanent;
        assert!(matches!(permanent, ErrorClassification::Permanent));

        // Test RateLimited variant without retry_after
        let rate_limited = ErrorClassification::RateLimited { retry_after: None };
        assert!(matches!(
            rate_limited,
            ErrorClassification::RateLimited { retry_after: None }
        ));

        // Test RateLimited variant with retry_after
        let rate_limited_with_delay = ErrorClassification::RateLimited {
            retry_after: Some(Duration::from_secs(60)),
        };
        assert!(matches!(
            rate_limited_with_delay,
            ErrorClassification::RateLimited {
                retry_after: Some(_)
            }
        ));
    }

    #[test]
    fn test_polling_error_transient() {
        let error = PollingError::transient("endpoint1", "Network timeout");

        assert_eq!(error.endpoint_id, "endpoint1");
        assert_eq!(error.message, "Network timeout");
        assert_eq!(error.classification, ErrorClassification::Transient);
        assert_eq!(error.status_code, None);
        assert!(error.should_retry());
    }

    #[test]
    fn test_polling_error_permanent() {
        let error = PollingError::permanent("endpoint2", "Invalid credentials");

        assert_eq!(error.endpoint_id, "endpoint2");
        assert_eq!(error.message, "Invalid credentials");
        assert_eq!(error.classification, ErrorClassification::Permanent);
        assert_eq!(error.status_code, None);
        assert!(!error.should_retry());
    }

    #[test]
    fn test_polling_error_rate_limited() {
        let error = PollingError::rate_limited("endpoint3", Some(Duration::from_secs(30)));

        assert_eq!(error.endpoint_id, "endpoint3");
        assert_eq!(error.message, "Rate limited");
        assert!(matches!(
            error.classification,
            ErrorClassification::RateLimited {
                retry_after: Some(_)
            }
        ));
        assert_eq!(error.status_code, Some(429));
        assert!(error.should_retry());
    }

    #[test]
    fn test_polling_error_rate_limited_no_retry_after() {
        let error = PollingError::rate_limited("endpoint4", None);

        assert_eq!(error.endpoint_id, "endpoint4");
        assert_eq!(error.message, "Rate limited");
        assert!(matches!(
            error.classification,
            ErrorClassification::RateLimited { retry_after: None }
        ));
        assert_eq!(error.status_code, Some(429));
        assert!(error.should_retry());
    }

    #[test]
    fn test_polling_error_from_status_429() {
        let error = PollingError::from_status("endpoint5", 429, "Too Many Requests");

        assert_eq!(error.endpoint_id, "endpoint5");
        assert_eq!(error.message, "Too Many Requests");
        assert!(matches!(
            error.classification,
            ErrorClassification::RateLimited { retry_after: None }
        ));
        assert_eq!(error.status_code, Some(429));
        assert!(error.should_retry());
    }

    #[test]
    fn test_polling_error_from_status_4xx_permanent() {
        // Test various 4xx status codes (should all be Permanent except 429)
        let test_cases = vec![
            (400, "Bad Request"),
            (401, "Unauthorized"),
            (403, "Forbidden"),
            (404, "Not Found"),
            (422, "Unprocessable Entity"),
            (499, "Client Closed Request"),
        ];

        for (status, body) in test_cases {
            let error = PollingError::from_status("endpoint6", status, body);

            assert_eq!(error.endpoint_id, "endpoint6");
            assert_eq!(error.message, body);
            assert_eq!(error.classification, ErrorClassification::Permanent);
            assert_eq!(error.status_code, Some(status));
            assert!(!error.should_retry(), "Status {} should not retry", status);
        }
    }

    #[test]
    fn test_polling_error_from_status_5xx_transient() {
        // Test various 5xx status codes (should all be Transient)
        let test_cases = vec![
            (500, "Internal Server Error"),
            (502, "Bad Gateway"),
            (503, "Service Unavailable"),
            (504, "Gateway Timeout"),
            (599, "Network Connect Timeout Error"),
        ];

        for (status, body) in test_cases {
            let error = PollingError::from_status("endpoint7", status, body);

            assert_eq!(error.endpoint_id, "endpoint7");
            assert_eq!(error.message, body);
            assert_eq!(error.classification, ErrorClassification::Transient);
            assert_eq!(error.status_code, Some(status));
            assert!(error.should_retry(), "Status {} should retry", status);
        }
    }

    #[test]
    fn test_polling_error_from_status_other_permanent() {
        // Test status codes outside 4xx and 5xx ranges
        let test_cases = vec![
            (200, "OK"), // Should not create errors from success, but testing classification
            (300, "Multiple Choices"),
            (600, "Invalid Status"),
        ];

        for (status, body) in test_cases {
            let error = PollingError::from_status("endpoint8", status, body);

            assert_eq!(error.endpoint_id, "endpoint8");
            assert_eq!(error.message, body);
            assert_eq!(error.classification, ErrorClassification::Permanent);
            assert_eq!(error.status_code, Some(status));
            assert!(!error.should_retry(), "Status {} should not retry", status);
        }
    }

    #[test]
    fn test_polling_error_should_retry() {
        // Transient errors should retry
        let transient = PollingError::transient("ep1", "timeout");
        assert!(transient.should_retry());

        // Permanent errors should not retry
        let permanent = PollingError::permanent("ep2", "bad request");
        assert!(!permanent.should_retry());

        // Rate limited errors should retry
        let rate_limited = PollingError::rate_limited("ep3", None);
        assert!(rate_limited.should_retry());

        // 5xx should retry
        let server_error = PollingError::from_status("ep4", 503, "Service Unavailable");
        assert!(server_error.should_retry());

        // 4xx (except 429) should not retry
        let client_error = PollingError::from_status("ep5", 404, "Not Found");
        assert!(!client_error.should_retry());
    }

    #[test]
    fn test_polling_error_display() {
        let error = PollingError::transient("test-endpoint", "Connection timeout");
        let display_string = format!("{}", error);

        assert_eq!(display_string, "[test-endpoint] Connection timeout");
    }

    #[test]
    fn test_polling_error_debug() {
        let error = PollingError::transient("debug-endpoint", "Debug message");
        let debug_string = format!("{:?}", error);

        // Verify it contains key information
        assert!(debug_string.contains("PollingError"));
        assert!(debug_string.contains("debug-endpoint"));
        assert!(debug_string.contains("Debug message"));
    }

    #[test]
    fn test_error_classification_clone() {
        let original = ErrorClassification::RateLimited {
            retry_after: Some(Duration::from_secs(42)),
        };
        let cloned = original.clone();

        assert_eq!(original, cloned);
    }

    #[test]
    fn test_error_classification_partial_eq() {
        assert_eq!(
            ErrorClassification::Transient,
            ErrorClassification::Transient
        );
        assert_eq!(
            ErrorClassification::Permanent,
            ErrorClassification::Permanent
        );
        assert_ne!(
            ErrorClassification::Transient,
            ErrorClassification::Permanent
        );

        let rate1 = ErrorClassification::RateLimited { retry_after: None };
        let rate2 = ErrorClassification::RateLimited { retry_after: None };
        assert_eq!(rate1, rate2);

        let rate3 = ErrorClassification::RateLimited {
            retry_after: Some(Duration::from_secs(30)),
        };
        assert_ne!(rate1, rate3);
    }

    // Tests for GenericHttpPollingSource
    #[tokio::test]
    async fn test_generic_http_source_creation() {
        let config = GenericHttpPollingConfig::default();
        let parser = create_default_parser();
        let source = GenericHttpPollingSource::new(config, parser);

        assert!(source.is_ok());
    }

    #[tokio::test]
    #[allow(deprecated)]
    async fn test_generic_http_source_with_default_parsers() {
        let config = GenericHttpPollingConfig::default();
        let source = GenericHttpPollingSource::with_default_parsers(config);

        assert!(source.is_ok());
        let source = source.unwrap();

        // Verify parser was created
        assert_eq!(source.parser.name(), "flat_json");
    }

    #[test]
    fn test_generic_http_source_build_request_query_auth() {
        let config = GenericHttpPollingConfig::default();
        let parser = create_default_parser();
        let source = GenericHttpPollingSource::new(config, parser).unwrap();

        let endpoint = EndpointConfig::new(
            "test_endpoint",
            "https://api.example.com/data",
            "location1",
            "flat_json",
        )
        .with_auth(AuthMethod::QueryParam {
            key: "apikey".to_string(),
            value: "secret123".to_string(),
        });

        let request = source.build_request(&endpoint);
        assert!(request.is_ok());
    }

    #[test]
    fn test_generic_http_source_build_request_header_auth() {
        let config = GenericHttpPollingConfig::default();
        let parser = create_default_parser();
        let source = GenericHttpPollingSource::new(config, parser).unwrap();

        let endpoint = EndpointConfig::new(
            "test_endpoint",
            "https://api.example.com/data",
            "location1",
            "flat_json",
        )
        .with_auth(AuthMethod::Header {
            name: "X-API-Key".to_string(),
            value: "secret123".to_string(),
        });

        let request = source.build_request(&endpoint);
        assert!(request.is_ok());
    }

    #[test]
    fn test_generic_http_source_build_request_bearer_auth() {
        let config = GenericHttpPollingConfig::default();
        let parser = create_default_parser();
        let source = GenericHttpPollingSource::new(config, parser).unwrap();

        let endpoint = EndpointConfig::new(
            "test_endpoint",
            "https://api.example.com/data",
            "location1",
            "flat_json",
        )
        .with_auth(AuthMethod::Bearer {
            token: "bearer_token_123".to_string(),
        });

        let request = source.build_request(&endpoint);
        assert!(request.is_ok());
    }

    #[tokio::test]
    async fn test_generic_http_source_no_enabled_endpoints() {
        let mut config = GenericHttpPollingConfig::default();

        // Add a disabled endpoint
        let mut endpoint = EndpointConfig::new(
            "disabled",
            "https://api.example.com/data",
            "location1",
            "flat_json",
        );
        endpoint.enabled = false;
        config.endpoints.push(endpoint);

        let parser = create_default_parser();
        let mut source = GenericHttpPollingSource::new(config, parser).unwrap();
        let result = source.start().await;

        assert!(result.is_err());
        if let Err(CoreError::Source(msg)) = result {
            assert_eq!(msg, "No enabled endpoints configured");
        } else {
            panic!("Expected CoreError::Source");
        }
    }

    #[test]
    fn test_generic_http_polling_config_default() {
        let config = GenericHttpPollingConfig::default();

        assert_eq!(config.endpoints.len(), 0);
        assert_eq!(config.poll_interval, Duration::from_secs(60));
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert_eq!(config.retry_config.max_retries, 3);
        assert_eq!(config.buffer_capacity, 1000);
    }

    #[tokio::test]
    async fn test_generic_http_source_health_check_not_running() {
        let config = GenericHttpPollingConfig::default();
        let parser = create_default_parser();
        let source = GenericHttpPollingSource::new(config, parser).unwrap();

        let health = source.health_check().await.unwrap();
        assert!(!health.healthy);
        assert_eq!(health.message, "Generic HTTP polling source not running");
    }

    #[tokio::test]
    async fn test_generic_http_source_fetch_empty() {
        let config = GenericHttpPollingConfig::default();
        let parser = create_default_parser();
        let source = GenericHttpPollingSource::new(config, parser).unwrap();

        let points = source.fetch().await.unwrap();
        assert_eq!(points.len(), 0);
    }

    #[tokio::test]
    async fn test_generic_http_source_build_request_with_headers() {
        let config = GenericHttpPollingConfig::default();
        let parser = create_default_parser();
        let source = GenericHttpPollingSource::new(config, parser).unwrap();

        let endpoint = EndpointConfig::new(
            "test_endpoint",
            "https://api.example.com/data",
            "location1",
            "flat_json",
        )
        .with_header("User-Agent", "TestClient/1.0")
        .with_header("Accept", "application/json");

        let request = source.build_request(&endpoint);
        assert!(request.is_ok());
    }

    #[tokio::test]
    async fn test_generic_http_source_build_request_query_auth_with_existing_params() {
        let config = GenericHttpPollingConfig::default();
        let parser = create_default_parser();
        let source = GenericHttpPollingSource::new(config, parser).unwrap();

        // URL already has query parameters
        let endpoint = EndpointConfig::new(
            "test_endpoint",
            "https://api.example.com/data?foo=bar",
            "location1",
            "flat_json",
        )
        .with_auth(AuthMethod::QueryParam {
            key: "apikey".to_string(),
            value: "secret123".to_string(),
        });

        let request = source.build_request(&endpoint);
        assert!(request.is_ok());
    }

    #[tokio::test]
    async fn test_generic_http_source_buffer_capacity() {
        let config = GenericHttpPollingConfig {
            buffer_capacity: 500,
            ..Default::default()
        };
        let parser = create_default_parser();
        let source = GenericHttpPollingSource::new(config, parser).unwrap();

        assert_eq!(source.sender.capacity(), 500);
    }
}
