//! Silver Subscriber for Bronze-to-Silver streaming transform
//!
//! This module implements SilverSubscriber which transforms RawDataPoint events
//! from the EventBus into SilverRecords and writes them to TimescaleDB.
//!
//! # Architecture (DP-012 Phase 2.3)
//!
//! ```text
//! EventBus (broadcast)
//!     |
//!     | RawDataPoint events
//!     v
//! SilverSubscriber
//!     |
//!     +-- On startup: catch-up from Bronze (if configured)
//!     |   `-- BronzeReader::read_since(high_water_mark)
//!     |
//!     +-- For each event:
//!     |   +-- transform_to_silver(config, raw_data)
//!     |   +-- evaluate_dq(record, rules)
//!     |   `-- output.write(record)
//!     |
//!     `-- Track high-water mark to avoid reprocessing
//! ```
//!
//! # London TDD Pattern
//!
//! - SilverOutput trait is mocked for unit tests
//! - BronzeReader trait is mocked for catch-up tests
//! - Configuration drives behavior, not hard-coded logic

use crate::config::SilverEtlConfig;
use crate::silver::outputs::{SilverOutput, SilverOutputError};
use crate::silver::types::SilverRecord;
use crate::traits::HealthStatus;
use crate::types::RawDataPoint;

use super::{BronzeReader, Subscriber, SubscriberError};

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

/// Configuration for SilverSubscriber
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SilverSubscriberConfig {
    /// Unique identifier for this subscriber
    #[serde(default = "default_subscriber_id")]
    pub subscriber_id: String,

    /// Stream IDs to process (empty = all streams)
    #[serde(default)]
    pub stream_filter: HashSet<String>,

    /// ETL configurations by stream ID
    /// Maps stream_id -> SilverEtlConfig
    #[serde(default)]
    pub etl_configs: std::collections::HashMap<String, SilverEtlConfig>,

    /// Catch-up configuration
    #[serde(default)]
    pub catch_up: CatchUpConfig,

    /// Batch size for processing
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,

    /// Flush interval in seconds
    #[serde(default = "default_flush_interval")]
    pub flush_interval_secs: u64,
}

fn default_subscriber_id() -> String {
    "silver-subscriber".to_string()
}

fn default_batch_size() -> usize {
    100
}

fn default_flush_interval() -> u64 {
    5
}

impl Default for SilverSubscriberConfig {
    fn default() -> Self {
        Self {
            subscriber_id: default_subscriber_id(),
            stream_filter: HashSet::new(),
            etl_configs: std::collections::HashMap::new(),
            catch_up: CatchUpConfig::default(),
            batch_size: default_batch_size(),
            flush_interval_secs: default_flush_interval(),
        }
    }
}

/// Catch-up configuration for recovering missed data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatchUpConfig {
    /// Whether catch-up is enabled
    #[serde(default)]
    pub enabled: bool,

    /// Maximum time window to catch up (default: 1 hour)
    #[serde(default = "default_catch_up_window_secs")]
    pub window_secs: u64,

    /// Batch size for catch-up reads
    #[serde(default = "default_catch_up_batch")]
    pub batch_size: usize,

    /// Path to high-water mark file
    #[serde(default)]
    pub watermark_file: Option<PathBuf>,
}

fn default_catch_up_window_secs() -> u64 {
    3600 // 1 hour
}

fn default_catch_up_batch() -> usize {
    1000
}

impl Default for CatchUpConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            window_secs: default_catch_up_window_secs(),
            batch_size: default_catch_up_batch(),
            watermark_file: None,
        }
    }
}

/// Silver Subscriber state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriberState {
    /// Not started
    Idle,
    /// Catching up from Bronze
    CatchingUp,
    /// Processing live events
    Running,
    /// Stopped
    Stopped,
}

/// SilverSubscriber transforms Bronze events to Silver records
///
/// # Lifecycle
/// 1. Create with config, output sink, and optional bronze reader
/// 2. On start(): perform catch-up if configured, then process events
/// 3. On stop(): flush pending writes, save watermark
pub struct SilverSubscriber<O, B>
where
    O: SilverOutput,
    B: BronzeReader,
{
    config: SilverSubscriberConfig,
    output: Arc<O>,
    bronze_reader: Option<Arc<B>>,
    state: SubscriberState,
    high_water_mark: Option<DateTime<Utc>>,
    records_processed: u64,
    records_dropped: u64,
    last_error: Option<String>,
    shutdown_signal: Option<tokio::sync::oneshot::Sender<()>>,
}

impl<O, B> SilverSubscriber<O, B>
where
    O: SilverOutput + 'static,
    B: BronzeReader + 'static,
{
    /// Create a new SilverSubscriber
    pub fn new(config: SilverSubscriberConfig, output: Arc<O>) -> Self {
        Self {
            config,
            output,
            bronze_reader: None,
            state: SubscriberState::Idle,
            high_water_mark: None,
            records_processed: 0,
            records_dropped: 0,
            last_error: None,
            shutdown_signal: None,
        }
    }

    /// Create with bronze reader for catch-up support
    pub fn with_bronze_reader(mut self, reader: Arc<B>) -> Self {
        self.bronze_reader = Some(reader);
        self
    }

    /// Get current state
    pub fn state(&self) -> SubscriberState {
        self.state
    }

    /// Get high water mark
    pub fn high_water_mark(&self) -> Option<DateTime<Utc>> {
        self.high_water_mark
    }

    /// Get records processed count
    pub fn records_processed(&self) -> u64 {
        self.records_processed
    }

    /// Get records dropped count
    pub fn records_dropped(&self) -> u64 {
        self.records_dropped
    }

    /// Perform catch-up from Bronze layer
    async fn catch_up(&mut self) -> Result<usize, SubscriberError> {
        if !self.config.catch_up.enabled {
            debug!("Catch-up disabled, skipping");
            return Ok(0);
        }

        let bronze_reader = match &self.bronze_reader {
            Some(r) => r.clone(),
            None => {
                warn!("Catch-up enabled but no BronzeReader configured");
                return Ok(0);
            }
        };

        self.state = SubscriberState::CatchingUp;
        info!("Starting catch-up from Bronze layer");

        // Determine catch-up window
        let catch_up_window = Duration::seconds(self.config.catch_up.window_secs as i64);
        let since = self
            .high_water_mark
            .unwrap_or_else(|| Utc::now() - catch_up_window);

        // Get stream filter
        let stream_filter = if self.config.stream_filter.is_empty() {
            None
        } else {
            // For simplicity, catch up first stream (could iterate all)
            self.config.stream_filter.iter().next().map(|s| s.as_str())
        };

        // Read from Bronze
        let raw_points = bronze_reader
            .read_since(since, stream_filter)
            .await
            .map_err(|e| SubscriberError::CatchUpError(e.to_string()))?;

        info!(
            count = raw_points.len(),
            since = %since,
            "Read raw points from Bronze for catch-up"
        );

        // Process in batches
        let mut processed = 0;
        for chunk in raw_points.chunks(self.config.catch_up.batch_size) {
            for raw_point in chunk {
                if let Some(record) = self.transform_point(raw_point)? {
                    if !record.should_drop() {
                        self.output
                            .write(&record)
                            .await
                            .map_err(|e| SubscriberError::StorageError(e.to_string()))?;
                        processed += 1;

                        // Update high water mark
                        if self
                            .high_water_mark
                            .map_or(true, |hwm| record.timestamp > hwm)
                        {
                            self.high_water_mark = Some(record.timestamp);
                        }
                    } else {
                        self.records_dropped += 1;
                    }
                }
            }
        }

        info!(processed = processed, "Catch-up completed");
        Ok(processed)
    }

    /// Transform a RawDataPoint to SilverRecord
    fn transform_point(&self, raw: &RawDataPoint) -> Result<Option<SilverRecord>, SubscriberError> {
        // Get ETL config for this stream
        let _etl_config = match self.config.etl_configs.get(&raw.source_id) {
            Some(cfg) => cfg,
            None => {
                debug!(stream_id = %raw.source_id, "No ETL config for stream, skipping");
                return Ok(None);
            }
        };

        // TODO: Full transform implementation would:
        // 1. Extract timestamp using config.timestamp mapping
        // 2. Extract identity fields
        // 3. Apply field mappings with transforms
        // 4. Evaluate DQ rules
        // 5. Build SilverRecord

        // For now, create a basic record from the raw data
        let timestamp = raw.timestamp;
        let mut record = SilverRecord::new(&raw.source_id, timestamp);

        // Set device ID from ndp_id if present
        if let Some(ref ndp_id) = raw.ndp_id {
            record = record.with_device_id(ndp_id);
        }

        // Copy fields from raw_payload
        if let Some(payload) = raw.raw_payload.as_object() {
            for (key, value) in payload {
                if key != "timestamp" {
                    record = record.with_field(key.clone(), value.clone());
                }
            }
        }

        Ok(Some(record))
    }

    /// Process a single event
    async fn process_event(&mut self, raw: Arc<RawDataPoint>) -> Result<(), SubscriberError> {
        // Check stream filter
        if !self.accepts_stream(&raw.source_id) {
            return Ok(());
        }

        // Transform
        let record = match self.transform_point(&raw)? {
            Some(r) => r,
            None => return Ok(()),
        };

        // Check if should drop
        if record.should_drop() {
            self.records_dropped += 1;
            return Ok(());
        }

        // Write to output
        self.output
            .write(&record)
            .await
            .map_err(|e| SubscriberError::StorageError(e.to_string()))?;

        self.records_processed += 1;

        // Update high water mark
        if self
            .high_water_mark
            .map_or(true, |hwm| record.timestamp > hwm)
        {
            self.high_water_mark = Some(record.timestamp);
        }

        Ok(())
    }

    /// Load high water mark from file
    fn load_watermark(&mut self) -> Result<(), SubscriberError> {
        if let Some(ref path) = self.config.catch_up.watermark_file {
            if path.exists() {
                let content = std::fs::read_to_string(path).map_err(|e| {
                    SubscriberError::ConfigError(format!("Failed to read watermark: {}", e))
                })?;

                let ts = content.trim().parse::<DateTime<Utc>>().map_err(|e| {
                    SubscriberError::ConfigError(format!("Invalid watermark format: {}", e))
                })?;

                self.high_water_mark = Some(ts);
                info!(watermark = %ts, "Loaded high water mark from file");
            }
        }
        Ok(())
    }

    /// Save high water mark to file
    fn save_watermark(&self) -> Result<(), SubscriberError> {
        if let (Some(ref path), Some(hwm)) =
            (&self.config.catch_up.watermark_file, self.high_water_mark)
        {
            std::fs::write(path, hwm.to_rfc3339()).map_err(|e| {
                SubscriberError::Internal(format!("Failed to save watermark: {}", e))
            })?;
            debug!(watermark = %hwm, "Saved high water mark to file");
        }
        Ok(())
    }
}

#[async_trait]
impl<O, B> Subscriber for SilverSubscriber<O, B>
where
    O: SilverOutput + 'static,
    B: BronzeReader + 'static,
{
    fn id(&self) -> &str {
        &self.config.subscriber_id
    }

    async fn start(
        &mut self,
        mut receiver: broadcast::Receiver<Arc<RawDataPoint>>,
    ) -> Result<(), SubscriberError> {
        info!(id = %self.id(), "Starting SilverSubscriber");

        // Load watermark if configured
        self.load_watermark()?;

        // Perform catch-up from Bronze
        if let Err(e) = self.catch_up().await {
            error!(error = %e, "Catch-up failed");
            // Continue with live processing even if catch-up fails
        }

        self.state = SubscriberState::Running;

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
                                self.last_error = Some(e.to_string());
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

        // Cleanup
        self.state = SubscriberState::Stopped;
        self.save_watermark()?;

        info!(
            id = %self.id(),
            processed = self.records_processed,
            dropped = self.records_dropped,
            "SilverSubscriber stopped"
        );

        Ok(())
    }

    async fn stop(&mut self) -> Result<(), SubscriberError> {
        info!(id = %self.id(), "Stopping SilverSubscriber");

        // Signal shutdown
        if let Some(tx) = self.shutdown_signal.take() {
            let _ = tx.send(());
        }

        // Flush output
        self.output
            .flush()
            .await
            .map_err(|e| SubscriberError::ShutdownFailed(e.to_string()))?;

        // Save watermark
        self.save_watermark()?;

        self.state = SubscriberState::Stopped;
        Ok(())
    }

    fn accepts_stream(&self, stream_id: &str) -> bool {
        if self.config.stream_filter.is_empty() {
            // Accept all streams if no filter configured
            true
        } else {
            self.config.stream_filter.contains(stream_id)
        }
    }

    async fn health_check(&self) -> HealthStatus {
        let mut details = std::collections::HashMap::new();
        let (healthy, message);

        // Check output health
        match self.output.health_check().await {
            Ok(true) => {
                healthy =
                    self.state == SubscriberState::Running || self.state == SubscriberState::Idle;
                message = if healthy {
                    "Healthy".to_string()
                } else {
                    "Not running".to_string()
                };
            }
            Ok(false) | Err(_) => {
                healthy = false;
                message = "Output sink unhealthy".to_string();
            }
        }

        // Add details
        details.insert("state".to_string(), format!("{:?}", self.state));
        details.insert(
            "records_processed".to_string(),
            self.records_processed.to_string(),
        );
        details.insert(
            "records_dropped".to_string(),
            self.records_dropped.to_string(),
        );

        if let Some(hwm) = self.high_water_mark {
            details.insert("high_water_mark".to_string(), hwm.to_rfc3339());
        }

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

impl From<SilverOutputError> for SubscriberError {
    fn from(err: SilverOutputError) -> Self {
        SubscriberError::StorageError(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CoreError;
    use crate::silver::outputs::InMemorySilverOutput;
    use chrono::TimeZone;
    use serde_json::json;

    // Test implementation of BronzeReader
    #[derive(Default)]
    pub struct TestBronzeReader {
        pub read_since_result: std::sync::Mutex<Vec<RawDataPoint>>,
        pub latest_timestamp: std::sync::Mutex<Option<DateTime<Utc>>>,
    }

    impl TestBronzeReader {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn with_data(self, data: Vec<RawDataPoint>) -> Self {
            *self.read_since_result.lock().unwrap() = data;
            self
        }
    }

    #[async_trait]
    impl BronzeReader for TestBronzeReader {
        async fn read_since(
            &self,
            _since: DateTime<Utc>,
            _stream_filter: Option<&str>,
        ) -> Result<Vec<RawDataPoint>, CoreError> {
            Ok(self.read_since_result.lock().unwrap().clone())
        }

        async fn get_latest_timestamp(
            &self,
            _stream_filter: Option<&str>,
        ) -> Result<Option<DateTime<Utc>>, CoreError> {
            Ok(*self.latest_timestamp.lock().unwrap())
        }
    }

    fn create_test_config() -> SilverSubscriberConfig {
        let mut config = SilverSubscriberConfig::default();
        config.subscriber_id = "test-silver".to_string();
        config.stream_filter.insert("air-quality".to_string());
        config
    }

    fn create_raw_point(stream_id: &str, ts: DateTime<Utc>) -> RawDataPoint {
        RawDataPoint {
            source_id: stream_id.to_string(),
            timestamp: ts,
            ndp_id: Some("device-001".to_string()),
            context: None,
            raw_payload: json!({
                "pm25": 12.5,
                "temperature_c": 22.3
            }),
        }
    }

    #[test]
    fn test_silver_subscriber_config_default() {
        let config = SilverSubscriberConfig::default();
        assert_eq!(config.subscriber_id, "silver-subscriber");
        assert!(config.stream_filter.is_empty());
        assert_eq!(config.batch_size, 100);
        assert!(!config.catch_up.enabled);
    }

    #[test]
    fn test_catch_up_config_default() {
        let config = CatchUpConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.window_secs, 3600);
        assert_eq!(config.batch_size, 1000);
    }

    #[tokio::test]
    async fn test_silver_subscriber_new() {
        let config = create_test_config();
        let output = Arc::new(InMemorySilverOutput::new());
        let subscriber: SilverSubscriber<InMemorySilverOutput, TestBronzeReader> =
            SilverSubscriber::new(config, output);

        assert_eq!(subscriber.id(), "test-silver");
        assert_eq!(subscriber.state(), SubscriberState::Idle);
        assert!(subscriber.high_water_mark().is_none());
    }

    #[tokio::test]
    async fn test_accepts_stream_with_filter() {
        let config = create_test_config();
        let output = Arc::new(InMemorySilverOutput::new());
        let subscriber: SilverSubscriber<InMemorySilverOutput, TestBronzeReader> =
            SilverSubscriber::new(config, output);

        assert!(subscriber.accepts_stream("air-quality"));
        assert!(!subscriber.accepts_stream("outdoor-weather"));
    }

    #[tokio::test]
    async fn test_accepts_stream_without_filter() {
        let mut config = SilverSubscriberConfig::default();
        config.stream_filter.clear(); // Empty = accept all

        let output = Arc::new(InMemorySilverOutput::new());
        let subscriber: SilverSubscriber<InMemorySilverOutput, TestBronzeReader> =
            SilverSubscriber::new(config, output);

        assert!(subscriber.accepts_stream("air-quality"));
        assert!(subscriber.accepts_stream("outdoor-weather"));
        assert!(subscriber.accepts_stream("any-stream"));
    }

    #[tokio::test]
    async fn test_transform_point_basic() {
        let mut config = create_test_config();
        // Add ETL config for air-quality stream
        config.etl_configs.insert(
            "air-quality".to_string(),
            SilverEtlConfig {
                enabled: true,
                target_table: "silver.air_quality".to_string(),
                timestamp: crate::config::TimestampMapping {
                    source_field: "timestamp".to_string(),
                    target_field: "observation_time".to_string(),
                    transform: crate::config::TimestampTransform::MicrosecondsToTimestamp,
                },
                ..Default::default()
            },
        );

        let output = Arc::new(InMemorySilverOutput::new());
        let subscriber: SilverSubscriber<InMemorySilverOutput, TestBronzeReader> =
            SilverSubscriber::new(config, output);

        let ts = Utc.with_ymd_and_hms(2026, 1, 18, 12, 0, 0).unwrap();
        let raw = create_raw_point("air-quality", ts);

        let result = subscriber.transform_point(&raw).unwrap();
        assert!(result.is_some());

        let record = result.unwrap();
        assert_eq!(record.stream_id, "air-quality");
        assert_eq!(record.timestamp, ts);
        assert_eq!(record.device_id, Some("device-001".to_string()));
    }

    #[tokio::test]
    async fn test_transform_point_no_config_returns_none() {
        let config = create_test_config(); // No ETL configs
        let output = Arc::new(InMemorySilverOutput::new());
        let subscriber: SilverSubscriber<InMemorySilverOutput, TestBronzeReader> =
            SilverSubscriber::new(config, output);

        let ts = Utc.with_ymd_and_hms(2026, 1, 18, 12, 0, 0).unwrap();
        let raw = create_raw_point("air-quality", ts);

        let result = subscriber.transform_point(&raw).unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_health_check_idle() {
        let config = create_test_config();
        let output = Arc::new(InMemorySilverOutput::new());
        let subscriber: SilverSubscriber<InMemorySilverOutput, TestBronzeReader> =
            SilverSubscriber::new(config, output);

        let status = subscriber.health_check().await;
        assert!(status.healthy);
        assert!(status.details.contains_key("state"));
    }

    #[tokio::test]
    async fn test_catch_up_disabled() {
        let config = create_test_config(); // catch_up.enabled = false by default
        let output = Arc::new(InMemorySilverOutput::new());
        let mut subscriber: SilverSubscriber<InMemorySilverOutput, TestBronzeReader> =
            SilverSubscriber::new(config, output);

        let result = subscriber.catch_up().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_catch_up_no_reader() {
        let mut config = create_test_config();
        config.catch_up.enabled = true;

        let output = Arc::new(InMemorySilverOutput::new());
        let mut subscriber: SilverSubscriber<InMemorySilverOutput, TestBronzeReader> =
            SilverSubscriber::new(config, output);
        // No bronze reader set

        let result = subscriber.catch_up().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_catch_up_with_test_reader() {
        let mut config = create_test_config();
        config.catch_up.enabled = true;
        config.catch_up.window_secs = 3600;

        // Add ETL config
        config.etl_configs.insert(
            "air-quality".to_string(),
            SilverEtlConfig {
                enabled: true,
                target_table: "silver.air_quality".to_string(),
                timestamp: crate::config::TimestampMapping {
                    source_field: "timestamp".to_string(),
                    target_field: "observation_time".to_string(),
                    transform: crate::config::TimestampTransform::MicrosecondsToTimestamp,
                },
                ..Default::default()
            },
        );

        let output = Arc::new(InMemorySilverOutput::new());

        // Setup test bronze reader with data
        let ts = Utc.with_ymd_and_hms(2026, 1, 18, 12, 0, 0).unwrap();
        let raw_points = vec![
            create_raw_point("air-quality", ts),
            create_raw_point("air-quality", ts + Duration::minutes(1)),
        ];

        let test_reader = TestBronzeReader::new().with_data(raw_points);

        let mut subscriber =
            SilverSubscriber::new(config, output.clone()).with_bronze_reader(Arc::new(test_reader));

        let result = subscriber.catch_up().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 2);

        // Verify records were written to output
        let records = output.get_records();
        assert_eq!(records.len(), 2);
    }

    #[tokio::test]
    async fn test_subscriber_id() {
        let mut config = SilverSubscriberConfig::default();
        config.subscriber_id = "custom-silver-sub".to_string();

        let output = Arc::new(InMemorySilverOutput::new());
        let subscriber: SilverSubscriber<InMemorySilverOutput, TestBronzeReader> =
            SilverSubscriber::new(config, output);

        assert_eq!(subscriber.id(), "custom-silver-sub");
    }
}
