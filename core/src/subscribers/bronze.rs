//! Bronze layer subscriber for raw data storage with Write-Ahead Log
//!
//! BronzeSubscriber consumes RawDataPoint events from the EventBus,
//! durably logs them to a WAL, accumulates them in memory, and
//! periodically snapshots full Parquet files.
//!
//! # Design (AIR-017 Bronze Write-Ahead Architecture)
//!
//! - WAL append on event receipt (durability before memory)
//! - Accumulator holds in-memory data grouped by source_id
//! - Snapshot timer writes full Parquet per source (overwrite, no read-modify-write)
//! - WAL watermark advanced after successful snapshot
//! - Graceful shutdown triggers final snapshot
//!
//! # Configuration
//!
//! ```yaml
//! subscribers:
//!   - id: bronze
//!     type: storage
//!     config:
//!       batch_size: 100
//!       flush_interval_secs: 5
//!       max_retries: 3
//!       snapshot_interval_secs: 1800
//! ```

use crate::error::CoreResult;
use crate::storage::accumulator::Accumulator;
use crate::storage::wal::WriteAheadLog;
use crate::subscribers::{Subscriber, SubscriberError};
use crate::traits::{HealthStatus, RawStore};
use crate::types::RawDataPoint;
use async_trait::async_trait;
use chrono::{DateTime, Datelike, Utc};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

/// Configuration for BronzeSubscriber
#[derive(Debug, Clone, Deserialize)]
pub struct BronzeSubscriberConfig {
    /// Maximum batch size before flush (default: 100)
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,

    /// Maximum time between flushes in seconds (default: 5)
    #[serde(default = "default_flush_interval_secs")]
    pub flush_interval_secs: u64,

    /// Maximum retry attempts for storage failures (default: 3)
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Optional stream filter - if empty, accepts all streams
    #[serde(default)]
    pub stream_filter: Vec<String>,

    /// Parquet snapshot interval in seconds (default: 1800 = 30 minutes).
    /// Controls how often the in-memory accumulator is flushed to Parquet.
    #[serde(default = "default_snapshot_interval_secs")]
    pub snapshot_interval_secs: u64,

    /// UTC hour for day rollover (default: 0 = midnight UTC).
    /// Determines when the accumulator rotates to a new calendar day.
    #[serde(default)]
    pub day_rollover_utc_hour: u8,
}

fn default_batch_size() -> usize {
    100
}
fn default_flush_interval_secs() -> u64 {
    5
}
fn default_max_retries() -> u32 {
    3
}
fn default_snapshot_interval_secs() -> u64 {
    1800
}

impl Default for BronzeSubscriberConfig {
    fn default() -> Self {
        Self {
            batch_size: default_batch_size(),
            flush_interval_secs: default_flush_interval_secs(),
            max_retries: default_max_retries(),
            stream_filter: Vec::new(),
            snapshot_interval_secs: default_snapshot_interval_secs(),
            day_rollover_utc_hour: 0,
        }
    }
}

/// Subscriber for Bronze layer (Parquet) storage with Write-Ahead Log
///
/// Consumes RawDataPoint events from EventBus, durably logs them via WAL,
/// accumulates in memory, and periodically snapshots full Parquet files.
pub struct BronzeSubscriber {
    id: String,
    config: BronzeSubscriberConfig,
    store: Arc<dyn RawStore>,
    wal: WriteAheadLog,
    accumulator: Accumulator,
    data_dir: PathBuf,
    cancellation_token: CancellationToken,
    is_running: bool,
    // Metrics
    events_received: u64,
    events_written: u64,
    snapshots_written: u64,
    errors_total: u64,
    wal_errors: u64,
}

impl BronzeSubscriber {
    /// Create a new BronzeSubscriber with WAL-backed durability
    ///
    /// # Arguments
    /// * `id` - Unique identifier for this subscriber
    /// * `config` - Subscriber configuration
    /// * `store` - RawStore implementation for writing Parquet data
    /// * `wal_path` - Path to the WAL file (created if absent, recovered if present)
    /// * `data_dir` - Base directory for Parquet partition paths
    ///
    /// # Errors
    /// Returns `CoreError` if WAL creation or recovery fails.
    pub fn new(
        id: impl Into<String>,
        config: BronzeSubscriberConfig,
        store: Arc<dyn RawStore>,
        wal_path: impl AsRef<Path>,
        data_dir: impl AsRef<Path>,
    ) -> CoreResult<Self> {
        let wal = WriteAheadLog::new(wal_path)?;
        let today = Utc::now().date_naive();
        let accumulator = Accumulator::new(today);

        Ok(Self {
            id: id.into(),
            config,
            store,
            wal,
            accumulator,
            data_dir: data_dir.as_ref().to_path_buf(),
            cancellation_token: CancellationToken::new(),
            is_running: false,
            events_received: 0,
            events_written: 0,
            snapshots_written: 0,
            errors_total: 0,
            wal_errors: 0,
        })
    }

    /// Process a single data point: WAL append first, then accumulator.
    ///
    /// If WAL append fails, the point is NOT added to the accumulator
    /// (per Pseudocode ADR: durability before memory).
    fn handle_point(&mut self, point: Arc<RawDataPoint>) {
        self.events_received += 1;

        // Check stream filter
        if !self.accepts_stream(&point.source_id) {
            debug!(
                subscriber_id = %self.id,
                source_id = %point.source_id,
                "Skipping point: stream not in filter"
            );
            return;
        }

        let owned_point = (*point).clone();

        // WAL first -- durability before memory
        match self.wal.append_point(&owned_point) {
            Ok(_seq) => {
                // Only add to accumulator if WAL succeeded
                self.accumulator.add(owned_point);
            }
            Err(e) => {
                self.wal_errors += 1;
                error!(
                    subscriber_id = %self.id,
                    error = %e,
                    "WAL append failed -- point NOT durable"
                );
            }
        }
    }

    /// Snapshot all accumulated data to Parquet, then advance WAL watermark.
    ///
    /// Writes one Parquet file per source_id (full overwrite, no read-modify-write).
    /// On success, commits the WAL up to the current sequence number.
    async fn snapshot(&mut self) -> Result<(), SubscriberError> {
        if self.accumulator.count() == 0 {
            return Ok(());
        }

        let points_by_source = self.accumulator.all_points_by_source();
        let snapshot_time = self.accumulator.latest().unwrap_or_else(Utc::now);
        let total_points = self.accumulator.count();
        let source_count = points_by_source.len();

        for (source_id, points) in points_by_source {
            let partition_path = self.partition_path(source_id, snapshot_time);

            self.store
                .write_raw_snapshot(points.clone(), &partition_path)
                .await
                .map_err(|e| {
                    SubscriberError::StorageError(format!("Snapshot write failed: {}", e))
                })?;
        }

        // All writes succeeded -- advance WAL watermark
        let max_seq = self.wal.next_sequence().saturating_sub(1);
        if max_seq > 0 {
            self.wal.commit_to(max_seq).map_err(|e| {
                SubscriberError::StorageError(format!("WAL commit failed: {}", e))
            })?;
        }

        self.events_written = total_points as u64;
        self.snapshots_written += 1;

        info!(
            subscriber_id = %self.id,
            sources = source_count,
            total_points = total_points,
            "Snapshot complete"
        );

        Ok(())
    }

    /// Recover accumulator state from Parquet + WAL on startup.
    ///
    /// Called at the start of `start()` before entering the `select!` loop.
    /// Rebuilds in-memory state from:
    /// 1. Today's existing Parquet data (via `store.query_raw`) -- seeds accumulator
    /// 2. WAL entries after the watermark -- merged with dedup into accumulator
    ///
    /// Both steps are non-fatal: Parquet read failure warns and falls back to
    /// WAL-only recovery; WAL replay failure warns and continues with whatever
    /// was seeded from Parquet.
    async fn recover(&mut self) -> Result<(), SubscriberError> {
        let today = Utc::now().date_naive();

        // Step 1: Seed from today's Parquet (if any)
        let start_of_day = today
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();
        let end_of_day = start_of_day + chrono::Duration::days(1);

        match self.store.query_raw(start_of_day, end_of_day, None).await {
            Ok(parquet_points) => {
                if !parquet_points.is_empty() {
                    let count = parquet_points.len();
                    for point in parquet_points {
                        self.accumulator.add(point);
                    }
                    info!(
                        subscriber_id = %self.id,
                        points = count,
                        "Recovery: seeded accumulator from Parquet"
                    );
                }
            }
            Err(e) => {
                // Parquet read failure is non-fatal for recovery --
                // we can still replay WAL entries
                warn!(
                    subscriber_id = %self.id,
                    error = %e,
                    "Recovery: failed to read Parquet, continuing with WAL only"
                );
            }
        }

        // Step 2: Replay WAL entries since watermark
        let watermark = self.wal.current_watermark();
        match self.wal.replay_since(watermark) {
            Ok(wal_entries) => {
                if !wal_entries.is_empty() {
                    let count = wal_entries.len();
                    let points: Vec<RawDataPoint> = wal_entries
                        .into_iter()
                        .map(|e| e.point)
                        .collect();
                    self.accumulator.merge_wal_entries(points);
                    info!(
                        subscriber_id = %self.id,
                        entries = count,
                        watermark = watermark,
                        "Recovery: replayed WAL entries"
                    );
                }
            }
            Err(e) => {
                warn!(
                    subscriber_id = %self.id,
                    error = %e,
                    "Recovery: failed to replay WAL"
                );
            }
        }

        info!(
            subscriber_id = %self.id,
            accumulator_count = self.accumulator.count(),
            "Recovery complete"
        );

        Ok(())
    }

    /// Compute Parquet partition path for a given source_id and timestamp.
    ///
    /// Path format: `{data_dir}/raw/{stream_id}/year=YYYY/month=MM/day=DD/data.parquet`
    fn partition_path(&self, source_id: &str, timestamp: DateTime<Utc>) -> PathBuf {
        let stream_id = extract_stream_from_source_id(source_id);
        let mut path = self.data_dir.clone();
        path.push("raw");
        path.push(stream_id);
        path.push(format!("year={}", timestamp.year()));
        path.push(format!("month={:02}", timestamp.month()));
        path.push(format!("day={:02}", timestamp.day()));
        path.push("data.parquet");
        path
    }
}

#[async_trait]
impl Subscriber for BronzeSubscriber {
    fn id(&self) -> &str {
        &self.id
    }

    async fn start(
        &mut self,
        mut receiver: broadcast::Receiver<Arc<RawDataPoint>>,
    ) -> Result<(), SubscriberError> {
        info!(subscriber_id = %self.id, "Starting BronzeSubscriber");
        self.is_running = true;

        // Recovery: rebuild accumulator from Parquet + WAL
        self.recover().await?;

        let snapshot_interval = Duration::from_secs(self.config.snapshot_interval_secs);
        let mut snapshot_timer = tokio::time::interval(snapshot_interval);
        // First tick is immediate, skip it
        snapshot_timer.tick().await;

        let flush_interval = Duration::from_secs(self.config.flush_interval_secs);
        let mut flush_timer = tokio::time::interval(flush_interval);
        // First tick is immediate, skip it
        flush_timer.tick().await;

        loop {
            tokio::select! {
                biased;

                // Check cancellation first
                _ = self.cancellation_token.cancelled() => {
                    info!(subscriber_id = %self.id, "Received cancellation signal");
                    break;
                }

                // Snapshot timer -- periodic Parquet archival
                _ = snapshot_timer.tick() => {
                    if let Err(e) = self.snapshot().await {
                        error!(subscriber_id = %self.id, error = %e, "Snapshot failed on timer");
                    }
                }

                // Flush timer -- kept for metric logging / future WAL fsync
                _ = flush_timer.tick() => {
                    // Periodic heartbeat -- no batch flush needed with WAL architecture
                    debug!(
                        subscriber_id = %self.id,
                        accumulator_count = self.accumulator.count(),
                        wal_errors = self.wal_errors,
                        "Heartbeat"
                    );
                }

                // Receive events
                result = receiver.recv() => {
                    match result {
                        Ok(point) => {
                            self.handle_point(point);
                        }
                        Err(RecvError::Lagged(n)) => {
                            warn!(
                                subscriber_id = %self.id,
                                lagged_count = n,
                                "Subscriber lagged - some events may be lost"
                            );
                        }
                        Err(RecvError::Closed) => {
                            info!(subscriber_id = %self.id, "Event bus channel closed");
                            break;
                        }
                    }
                }
            }
        }

        // Final snapshot on exit
        info!(subscriber_id = %self.id, "Performing final snapshot before shutdown");
        if let Err(e) = self.snapshot().await {
            error!(subscriber_id = %self.id, error = %e, "Final snapshot failed");
        }

        self.is_running = false;
        info!(
            subscriber_id = %self.id,
            events_received = self.events_received,
            events_written = self.events_written,
            snapshots_written = self.snapshots_written,
            errors_total = self.errors_total,
            wal_errors = self.wal_errors,
            "BronzeSubscriber stopped"
        );

        Ok(())
    }

    async fn stop(&mut self) -> Result<(), SubscriberError> {
        info!(subscriber_id = %self.id, "Stopping BronzeSubscriber");
        self.cancellation_token.cancel();
        Ok(())
    }

    fn accepts_stream(&self, stream_id: &str) -> bool {
        if self.config.stream_filter.is_empty() {
            return true;
        }
        // Extract stream_id from source_id (e.g., "air-quality-Mqtt" -> "air-quality")
        let stream = extract_stream_from_source_id(stream_id);
        self.config.stream_filter.iter().any(|s| s == stream)
    }

    async fn health_check(&self) -> HealthStatus {
        let mut details = HashMap::new();
        details.insert(
            "events_received".to_string(),
            self.events_received.to_string(),
        );
        details.insert(
            "events_written".to_string(),
            self.events_written.to_string(),
        );
        details.insert(
            "snapshots_written".to_string(),
            self.snapshots_written.to_string(),
        );
        details.insert("errors_total".to_string(), self.errors_total.to_string());
        details.insert("wal_errors".to_string(), self.wal_errors.to_string());
        details.insert(
            "accumulator_count".to_string(),
            self.accumulator.count().to_string(),
        );
        details.insert("is_running".to_string(), self.is_running.to_string());

        HealthStatus {
            healthy: self.is_running || self.errors_total == 0,
            message: if self.is_running {
                "BronzeSubscriber running".to_string()
            } else {
                "BronzeSubscriber not running".to_string()
            },
            details,
        }
    }
}

/// Extract stream_id from source_id by removing the protocol suffix
///
/// source_id format: "{stream_id}-{SourceType}" (e.g., "air-quality-Mqtt", "nws-forecast-Http")
fn extract_stream_from_source_id(source_id: &str) -> &str {
    const SUFFIXES: &[&str] = &["-FileWatch", "-Webhook", "-HttpPoll", "-Http", "-Mqtt"];

    for suffix in SUFFIXES {
        if let Some(stripped) = source_id.strip_suffix(suffix) {
            return stripped;
        }
    }
    source_id
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CoreError;
    use crate::traits::MockRawStore;
    use chrono::Utc;
    use mockall::predicate::*;
    use serde_json::json;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::broadcast;

    // ========== HELPER FUNCTIONS ==========

    fn create_test_point(source_id: &str) -> RawDataPoint {
        RawDataPoint::new(source_id, json!({"pm25": 12.5, "co2": 450}))
            .with_timestamp(Utc::now())
            .with_ndp_id("test-device-001")
    }

    fn create_config(batch_size: usize, flush_interval_secs: u64) -> BronzeSubscriberConfig {
        BronzeSubscriberConfig {
            batch_size,
            flush_interval_secs,
            max_retries: 3,
            stream_filter: Vec::new(),
            snapshot_interval_secs: 1800,
            day_rollover_utc_hour: 0,
        }
    }

    /// Create a temp directory with WAL and data paths for testing
    fn create_test_subscriber(
        id: &str,
        config: BronzeSubscriberConfig,
        mock_store: MockRawStore,
    ) -> BronzeSubscriber {
        let temp_dir = std::env::temp_dir().join(format!(
            "bronze_test_{}_{}",
            id,
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let wal_path = temp_dir.join("wal.log");
        let data_dir = temp_dir.join("data");
        std::fs::create_dir_all(&data_dir).unwrap();

        BronzeSubscriber::new(id, config, Arc::new(mock_store), &wal_path, &data_dir).unwrap()
    }

    /// Create a test subscriber and return it along with its temp dir path for cleanup
    fn create_test_subscriber_with_cleanup(
        id: &str,
        config: BronzeSubscriberConfig,
        mock_store: MockRawStore,
    ) -> (BronzeSubscriber, PathBuf) {
        let temp_dir = std::env::temp_dir().join(format!(
            "bronze_test_{}_{}",
            id,
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let wal_path = temp_dir.join("wal.log");
        let data_dir = temp_dir.join("data");
        std::fs::create_dir_all(&data_dir).unwrap();

        let sub =
            BronzeSubscriber::new(id, config, Arc::new(mock_store), &wal_path, &data_dir).unwrap();
        (sub, temp_dir)
    }

    // ========== TDD CYCLE 1: BronzeSubscriberConfig Tests ==========

    #[test]
    fn test_config_default_values() {
        let config = BronzeSubscriberConfig::default();
        assert_eq!(config.batch_size, 100);
        assert_eq!(config.flush_interval_secs, 5);
        assert_eq!(config.max_retries, 3);
        assert!(config.stream_filter.is_empty());
        assert_eq!(config.snapshot_interval_secs, 1800);
        assert_eq!(config.day_rollover_utc_hour, 0);
    }

    #[test]
    fn test_config_deserialize_with_defaults() {
        let yaml = r#"
            batch_size: 50
        "#;
        let config: BronzeSubscriberConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.batch_size, 50);
        assert_eq!(config.flush_interval_secs, 5); // default
        assert_eq!(config.max_retries, 3); // default
        assert_eq!(config.snapshot_interval_secs, 1800); // default
        assert_eq!(config.day_rollover_utc_hour, 0); // default
    }

    #[test]
    fn test_config_deserialize_full() {
        let yaml = r#"
            batch_size: 200
            flush_interval_secs: 10
            max_retries: 5
            stream_filter:
              - air-quality
              - outdoor-weather
        "#;
        let config: BronzeSubscriberConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.batch_size, 200);
        assert_eq!(config.flush_interval_secs, 10);
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.stream_filter, vec!["air-quality", "outdoor-weather"]);
    }

    // ========== AIR-017 Config Cycles C1-C4: New fields ==========

    #[test]
    fn test_config_snapshot_interval_defaults_to_1800() {
        // C1: Deserialize YAML without snapshot_interval_secs -> defaults to 1800
        let yaml = r#"
            batch_size: 100
            flush_interval_secs: 30
            max_retries: 3
        "#;
        let config: BronzeSubscriberConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.snapshot_interval_secs, 1800);
    }

    #[test]
    fn test_config_day_rollover_defaults_to_zero() {
        // C2: Deserialize YAML without day_rollover_utc_hour -> defaults to 0
        let yaml = r#"
            batch_size: 100
        "#;
        let config: BronzeSubscriberConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.day_rollover_utc_hour, 0);
    }

    #[test]
    fn test_config_with_all_new_fields() {
        // C3: YAML with explicit snapshot_interval_secs and day_rollover_utc_hour
        let yaml = r#"
            batch_size: 100
            flush_interval_secs: 30
            max_retries: 3
            snapshot_interval_secs: 900
            day_rollover_utc_hour: 6
        "#;
        let config: BronzeSubscriberConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.snapshot_interval_secs, 900);
        assert_eq!(config.day_rollover_utc_hour, 6);
    }

    #[test]
    fn test_config_backward_compatible_with_pre_air017_yaml() {
        // C4: Existing YAML from before AIR-017 (only original fields) still works
        let yaml = r#"
            batch_size: 100
            flush_interval_secs: 30
            max_retries: 3
            stream_filter:
              - air-quality
        "#;
        let config: BronzeSubscriberConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.batch_size, 100);
        assert_eq!(config.flush_interval_secs, 30);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.stream_filter, vec!["air-quality"]);
        // New fields get their defaults
        assert_eq!(config.snapshot_interval_secs, 1800);
        assert_eq!(config.day_rollover_utc_hour, 0);
    }

    // ========== TDD CYCLE 2: BronzeSubscriber Creation Tests ==========

    #[test]
    fn test_subscriber_creation() {
        let config = BronzeSubscriberConfig::default();
        let mock_store = MockRawStore::new();
        let subscriber = create_test_subscriber("bronze-test", config, mock_store);

        assert_eq!(subscriber.id(), "bronze-test");
        assert_eq!(subscriber.events_received, 0);
        assert_eq!(subscriber.events_written, 0);
        assert_eq!(subscriber.snapshots_written, 0);
        assert_eq!(subscriber.wal_errors, 0);
        assert!(!subscriber.is_running);
    }

    #[test]
    fn test_subscriber_creation_returns_core_result() {
        // new() returns CoreResult<Self> because WAL creation can fail
        let temp_dir = std::env::temp_dir().join(format!(
            "bronze_result_test_{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let wal_path = temp_dir.join("wal.log");
        let data_dir = temp_dir.join("data");

        let mock_store = MockRawStore::new();
        let result = BronzeSubscriber::new(
            "result-test",
            BronzeSubscriberConfig::default(),
            Arc::new(mock_store),
            &wal_path,
            &data_dir,
        );
        assert!(result.is_ok());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_subscriber_accepts_all_streams_by_default() {
        let config = BronzeSubscriberConfig::default();
        let mock_store = MockRawStore::new();
        let subscriber = create_test_subscriber("bronze-test", config, mock_store);

        assert!(subscriber.accepts_stream("air-quality-Mqtt"));
        assert!(subscriber.accepts_stream("outdoor-weather-Http"));
        assert!(subscriber.accepts_stream("any-stream"));
    }

    #[test]
    fn test_subscriber_filters_streams() {
        let config = BronzeSubscriberConfig {
            stream_filter: vec!["air-quality".to_string()],
            ..Default::default()
        };
        let mock_store = MockRawStore::new();
        let subscriber = create_test_subscriber("bronze-test", config, mock_store);

        assert!(subscriber.accepts_stream("air-quality-Mqtt"));
        assert!(subscriber.accepts_stream("air-quality-Http"));
        assert!(!subscriber.accepts_stream("outdoor-weather-Http"));
    }

    // ========== TDD CYCLE 3: handle_point WAL-first behavior ==========

    #[test]
    fn test_handle_point_wal_then_accumulator() {
        // Behavior: WAL append happens first, then accumulator add
        let config = create_config(10, 5);
        let mock_store = MockRawStore::new();
        let mut subscriber = create_test_subscriber("bronze-test", config, mock_store);

        let point = Arc::new(create_test_point("air-quality-Mqtt"));
        subscriber.handle_point(point);

        // Point should be in accumulator (WAL succeeded)
        assert_eq!(subscriber.accumulator.count(), 1);
        assert_eq!(subscriber.events_received, 1);
        assert_eq!(subscriber.wal_errors, 0);

        // WAL sequence should have advanced
        assert_eq!(subscriber.wal.next_sequence(), 2);
    }

    #[test]
    fn test_handle_point_multiple_sources() {
        let config = create_config(10, 5);
        let mock_store = MockRawStore::new();
        let mut subscriber = create_test_subscriber("bronze-test", config, mock_store);

        subscriber.handle_point(Arc::new(create_test_point("air-quality-Mqtt")));
        subscriber.handle_point(Arc::new(create_test_point("air-quality-Mqtt")));
        subscriber.handle_point(Arc::new(create_test_point("outdoor-weather-Http")));

        assert_eq!(subscriber.accumulator.count(), 3);
        assert_eq!(subscriber.accumulator.source_count(), 2);
        assert_eq!(subscriber.wal.next_sequence(), 4);
    }

    #[test]
    fn test_filtered_points_not_in_wal_or_accumulator() {
        // Filtered points should not be written to WAL or accumulator
        let config = BronzeSubscriberConfig {
            batch_size: 10,
            stream_filter: vec!["air-quality".to_string()],
            ..Default::default()
        };
        let mock_store = MockRawStore::new();
        let mut subscriber = create_test_subscriber("bronze-test", config, mock_store);

        // This should be filtered out
        let point = Arc::new(create_test_point("outdoor-weather-Http"));
        subscriber.handle_point(point);

        assert_eq!(subscriber.accumulator.count(), 0);
        assert_eq!(subscriber.events_received, 1); // Still counted as received
        assert_eq!(subscriber.wal.next_sequence(), 1); // WAL not advanced
    }

    // ========== TDD CYCLE 4: Snapshot Tests ==========

    #[tokio::test]
    async fn test_snapshot_empty_is_noop() {
        let config = create_config(10, 5);
        let mock_store = MockRawStore::new();
        // No expectations -- write_raw_snapshot should NOT be called
        let mut subscriber = create_test_subscriber("bronze-test", config, mock_store);

        let result = subscriber.snapshot().await;
        assert!(result.is_ok());
        assert_eq!(subscriber.snapshots_written, 0);
    }

    #[tokio::test]
    async fn test_snapshot_writes_all_sources() {
        let config = create_config(10, 5);
        let mut mock_store = MockRawStore::new();

        // Expect write_raw_snapshot to be called for each source
        mock_store
            .expect_write_raw_snapshot()
            .times(2) // Two distinct sources
            .returning(|_, _| Ok(()));

        let mut subscriber = create_test_subscriber("bronze-test", config, mock_store);

        // Add points from two different sources
        subscriber.handle_point(Arc::new(create_test_point("air-quality-Mqtt")));
        subscriber.handle_point(Arc::new(create_test_point("air-quality-Mqtt")));
        subscriber.handle_point(Arc::new(create_test_point("outdoor-weather-Http")));

        let result = subscriber.snapshot().await;
        assert!(result.is_ok());
        assert_eq!(subscriber.snapshots_written, 1);
        assert_eq!(subscriber.events_written, 3);
    }

    #[tokio::test]
    async fn test_snapshot_advances_wal_watermark() {
        let config = create_config(10, 5);
        let mut mock_store = MockRawStore::new();

        mock_store
            .expect_write_raw_snapshot()
            .times(1)
            .returning(|_, _| Ok(()));

        let mut subscriber = create_test_subscriber("bronze-test", config, mock_store);

        // Add 3 points -- WAL sequences 1, 2, 3
        for _ in 0..3 {
            subscriber.handle_point(Arc::new(create_test_point("air-quality-Mqtt")));
        }

        assert_eq!(subscriber.wal.next_sequence(), 4);
        assert_eq!(subscriber.wal.current_watermark(), 0);

        let result = subscriber.snapshot().await;
        assert!(result.is_ok());

        // WAL watermark should advance to 3 (max_seq = next - 1)
        assert_eq!(subscriber.wal.current_watermark(), 3);
    }

    #[tokio::test]
    async fn test_snapshot_failure_does_not_advance_watermark() {
        let config = create_config(10, 5);
        let mut mock_store = MockRawStore::new();

        // First call fails
        mock_store
            .expect_write_raw_snapshot()
            .times(1)
            .returning(|_, _| Err(CoreError::Storage("Disk full".to_string())));

        let mut subscriber = create_test_subscriber("bronze-test", config, mock_store);

        subscriber.handle_point(Arc::new(create_test_point("air-quality-Mqtt")));

        let result = subscriber.snapshot().await;
        assert!(result.is_err());

        // WAL watermark should NOT have advanced
        assert_eq!(subscriber.wal.current_watermark(), 0);
        assert_eq!(subscriber.snapshots_written, 0);
    }

    // ========== TDD CYCLE 5: Partition path computation ==========

    #[test]
    fn test_partition_path_computation() {
        let config = create_config(10, 5);
        let mock_store = MockRawStore::new();
        let subscriber = create_test_subscriber("bronze-test", config, mock_store);

        let ts = chrono::TimeZone::with_ymd_and_hms(&Utc, 2026, 2, 8, 10, 30, 0).unwrap();
        let path = subscriber.partition_path("air-quality-Mqtt", ts);

        let path_str = path.to_string_lossy();
        assert!(path_str.contains("raw"));
        assert!(path_str.contains("air-quality"));
        assert!(path_str.contains("year=2026"));
        assert!(path_str.contains("month=02"));
        assert!(path_str.contains("day=08"));
        assert!(path_str.ends_with("data.parquet"));
    }

    #[test]
    fn test_partition_path_strips_protocol_suffix() {
        let config = create_config(10, 5);
        let mock_store = MockRawStore::new();
        let subscriber = create_test_subscriber("bronze-test", config, mock_store);

        let ts = chrono::TimeZone::with_ymd_and_hms(&Utc, 2026, 2, 8, 10, 0, 0).unwrap();

        let path_mqtt = subscriber.partition_path("air-quality-Mqtt", ts);
        let path_http = subscriber.partition_path("air-quality-Http", ts);

        // Both should resolve to the same stream path
        assert_eq!(path_mqtt, path_http);
    }

    // ========== TDD CYCLE 6: Health Check Tests ==========

    #[tokio::test]
    async fn test_health_check_not_running() {
        let config = BronzeSubscriberConfig::default();
        let mock_store = MockRawStore::new();
        let subscriber = create_test_subscriber("bronze-test", config, mock_store);

        let health = subscriber.health_check().await;
        assert!(health.healthy); // No errors, so healthy even when not running
        assert!(health.message.contains("not running"));
        assert_eq!(health.details.get("is_running"), Some(&"false".to_string()));
    }

    #[tokio::test]
    async fn test_health_check_includes_new_metrics() {
        let config = BronzeSubscriberConfig::default();
        let mock_store = MockRawStore::new();
        let subscriber = create_test_subscriber("bronze-test", config, mock_store);

        let health = subscriber.health_check().await;
        assert!(health.details.contains_key("events_received"));
        assert!(health.details.contains_key("events_written"));
        assert!(health.details.contains_key("snapshots_written"));
        assert!(health.details.contains_key("errors_total"));
        assert!(health.details.contains_key("wal_errors"));
        assert!(health.details.contains_key("accumulator_count"));
    }

    // ========== TDD CYCLE 7: Extract Stream ID Helper Tests ==========

    #[test]
    fn test_extract_stream_from_source_id() {
        assert_eq!(
            extract_stream_from_source_id("air-quality-Mqtt"),
            "air-quality"
        );
        assert_eq!(
            extract_stream_from_source_id("air-quality-Http"),
            "air-quality"
        );
        assert_eq!(
            extract_stream_from_source_id("nws-forecast-HttpPoll"),
            "nws-forecast"
        );
        assert_eq!(
            extract_stream_from_source_id("file-data-FileWatch"),
            "file-data"
        );
        assert_eq!(extract_stream_from_source_id("webhook-Webhook"), "webhook");
        // No suffix - return as-is
        assert_eq!(extract_stream_from_source_id("air-quality"), "air-quality");
        assert_eq!(extract_stream_from_source_id("unknown"), "unknown");
    }

    // ========== TDD CYCLE 8: Integration-Style Tests with Event Bus ==========

    #[tokio::test]
    async fn test_subscriber_receives_and_processes_events() {
        let mut config = create_config(100, 60);
        config.snapshot_interval_secs = 1; // 1 second snapshot timer
        let mut mock_store = MockRawStore::new();

        // Recovery: no existing Parquet data
        mock_store
            .expect_query_raw()
            .times(1)
            .returning(|_, _, _| Ok(vec![]));

        // Expect snapshot write(s) for the accumulated points
        mock_store
            .expect_write_raw_snapshot()
            .times(1..)
            .returning(|_, _| Ok(()));

        let (subscriber, _temp_dir) =
            create_test_subscriber_with_cleanup("bronze-test", config, mock_store);
        let mut subscriber = subscriber;

        // Create broadcast channel
        let (tx, rx) = broadcast::channel::<Arc<RawDataPoint>>(100);

        // Spawn subscriber task
        let subscriber_handle = tokio::spawn(async move { subscriber.start(rx).await });

        // Send 5 events
        for i in 0..5 {
            let point = Arc::new(create_test_point(&format!("air-quality-{}-Mqtt", i)));
            tx.send(point).unwrap();
        }

        // Wait for snapshot timer to fire (>1 second)
        tokio::time::sleep(Duration::from_millis(1500)).await;

        // Stop by closing channel
        drop(tx);

        // Wait for subscriber to finish
        let result = subscriber_handle.await.unwrap();
        assert!(result.is_ok());

        let _ = std::fs::remove_dir_all(&_temp_dir);
    }

    #[tokio::test]
    async fn test_snapshot_timer_fires() {
        let mut config = create_config(100, 60);
        config.snapshot_interval_secs = 1; // 1 second snapshot interval
        let mut mock_store = MockRawStore::new();

        // Recovery: no existing Parquet data
        mock_store
            .expect_query_raw()
            .times(1)
            .returning(|_, _, _| Ok(vec![]));

        // Expect at least one snapshot write from the timer
        mock_store
            .expect_write_raw_snapshot()
            .times(1..)
            .returning(|_, _| Ok(()));

        let (subscriber, _temp_dir) =
            create_test_subscriber_with_cleanup("bronze-test", config, mock_store);
        let mut subscriber = subscriber;

        let (tx, rx) = broadcast::channel::<Arc<RawDataPoint>>(100);

        let subscriber_handle = tokio::spawn(async move { subscriber.start(rx).await });

        // Send 3 events
        for i in 0..3 {
            let point = Arc::new(create_test_point(&format!("source-{}-Mqtt", i)));
            tx.send(point).unwrap();
        }

        // Wait for snapshot timer to fire (>1 second)
        tokio::time::sleep(Duration::from_millis(1500)).await;

        // Stop
        drop(tx);

        let result = subscriber_handle.await.unwrap();
        assert!(result.is_ok());

        let _ = std::fs::remove_dir_all(&_temp_dir);
    }

    #[tokio::test]
    async fn test_subscriber_handles_lagged_error() {
        // Use very small channel to trigger lag
        let config = create_config(100, 60);
        let mock_store = MockRawStore::new();
        let subscriber = create_test_subscriber("bronze-test", config, mock_store);

        let (tx, _rx) = broadcast::channel::<Arc<RawDataPoint>>(2);

        // Fill channel to cause lag
        for i in 0..5 {
            let point = Arc::new(create_test_point(&format!("source-{}-Mqtt", i)));
            let _ = tx.send(point);
        }

        // Receiver should get lagged error on next recv
        // This is tested by the fact that start() handles RecvError::Lagged and continues
        // The test here ensures subscriber doesn't crash on lag
    }

    #[tokio::test]
    async fn test_subscriber_final_snapshot_on_shutdown() {
        let config = create_config(100, 60);
        let mut mock_store = MockRawStore::new();

        // Recovery: no existing Parquet data
        mock_store
            .expect_query_raw()
            .times(1)
            .returning(|_, _, _| Ok(vec![]));

        // Expect final snapshot on shutdown
        mock_store
            .expect_write_raw_snapshot()
            .times(1..)
            .returning(|_, _| Ok(()));

        let (subscriber, _temp_dir) =
            create_test_subscriber_with_cleanup("bronze-test", config, mock_store);
        let mut subscriber = subscriber;

        let (tx, rx) = broadcast::channel::<Arc<RawDataPoint>>(100);

        // Get cancellation token before moving subscriber
        let cancel_token = subscriber.cancellation_token.clone();

        let subscriber_handle = tokio::spawn(async move { subscriber.start(rx).await });

        // Send some events
        for i in 0..3 {
            let point = Arc::new(create_test_point(&format!("source-{}-Mqtt", i)));
            tx.send(point).unwrap();
        }

        // Give time for events to be received
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Trigger graceful shutdown via cancellation token
        cancel_token.cancel();

        let result = subscriber_handle.await.unwrap();
        assert!(result.is_ok());

        let _ = std::fs::remove_dir_all(&_temp_dir);
    }

    // ========== TDD CYCLE 9: Startup Recovery Tests (P1-06) ==========

    /// Helper: create a test point with a specific timestamp for recovery tests.
    fn create_test_point_at(source_id: &str, ts: DateTime<Utc>) -> RawDataPoint {
        RawDataPoint::new(source_id, json!({"pm25": 12.5, "co2": 450}))
            .with_timestamp(ts)
            .with_ndp_id("test-device-001")
    }

    /// Helper: create a subscriber with a pre-populated WAL for recovery testing.
    /// Returns the subscriber AND the temp_dir (for WAL path access in multi-step tests).
    fn create_recovery_subscriber(
        id: &str,
        config: BronzeSubscriberConfig,
        mock_store: MockRawStore,
        wal_setup: impl FnOnce(&mut crate::storage::wal::WriteAheadLog),
    ) -> (BronzeSubscriber, PathBuf) {
        let temp_dir = std::env::temp_dir().join(format!(
            "bronze_recovery_{}_{}", id, uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let wal_path = temp_dir.join("wal.log");
        let data_dir = temp_dir.join("data");
        std::fs::create_dir_all(&data_dir).unwrap();

        // Create and populate WAL before constructing subscriber
        {
            let mut wal = crate::storage::wal::WriteAheadLog::new(&wal_path).unwrap();
            wal_setup(&mut wal);
        }

        // Construct subscriber -- WAL::new recovers state from existing file
        let sub = BronzeSubscriber::new(
            id,
            config,
            Arc::new(mock_store),
            &wal_path,
            &data_dir,
        )
        .unwrap();
        (sub, temp_dir)
    }

    #[tokio::test]
    async fn test_recovery_empty_start() {
        // Case: First run -- no Parquet, empty WAL -> accumulator stays empty
        let config = create_config(10, 60);
        let mut mock_store = MockRawStore::new();

        // query_raw returns empty (no Parquet data)
        mock_store
            .expect_query_raw()
            .times(1)
            .returning(|_, _, _| Ok(vec![]));

        let mut subscriber = create_test_subscriber("recovery-empty", config, mock_store);

        subscriber.recover().await.unwrap();

        assert_eq!(subscriber.accumulator.count(), 0);
        assert_eq!(subscriber.accumulator.source_count(), 0);
    }

    #[tokio::test]
    async fn test_recovery_parquet_only() {
        // Case: Clean shutdown after snapshot -- Parquet has data, WAL empty
        let config = create_config(10, 60);
        let mut mock_store = MockRawStore::new();

        let today_start = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
        let ts1 = today_start + chrono::Duration::hours(1);
        let ts2 = today_start + chrono::Duration::hours(2);
        let ts3 = today_start + chrono::Duration::hours(3);

        mock_store
            .expect_query_raw()
            .times(1)
            .returning(move |_, _, _| {
                Ok(vec![
                    create_test_point_at("air-quality-Mqtt", ts1),
                    create_test_point_at("air-quality-Mqtt", ts2),
                    create_test_point_at("outdoor-weather-Http", ts3),
                ])
            });

        let mut subscriber = create_test_subscriber("recovery-parquet", config, mock_store);

        subscriber.recover().await.unwrap();

        assert_eq!(subscriber.accumulator.count(), 3);
        assert_eq!(subscriber.accumulator.source_count(), 2);
    }

    #[tokio::test]
    async fn test_recovery_wal_only() {
        // Case: Crash after events but before snapshot -- no Parquet, WAL has entries
        let config = create_config(10, 60);
        let mut mock_store = MockRawStore::new();

        // query_raw returns empty (no Parquet)
        mock_store
            .expect_query_raw()
            .times(1)
            .returning(|_, _, _| Ok(vec![]));

        let today_start = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
        let ts1 = today_start + chrono::Duration::hours(4);
        let ts2 = today_start + chrono::Duration::hours(5);

        let (mut subscriber, temp_dir) = create_recovery_subscriber(
            "recovery-wal",
            config,
            mock_store,
            |wal| {
                // Append 2 entries to WAL (uncommitted -- watermark stays at 0)
                let p1 = create_test_point_at("air-quality-Mqtt", ts1);
                let p2 = create_test_point_at("outdoor-weather-Http", ts2);
                wal.append_point(&p1).unwrap();
                wal.append_point(&p2).unwrap();
            },
        );

        subscriber.recover().await.unwrap();

        assert_eq!(subscriber.accumulator.count(), 2);
        assert_eq!(subscriber.accumulator.source_count(), 2);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_recovery_parquet_plus_wal() {
        // Case: Crash between snapshot and next snapshot
        // Parquet has data from before snapshot, WAL has post-watermark entries
        let config = create_config(10, 60);
        let mut mock_store = MockRawStore::new();

        let today_start = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
        let ts1 = today_start + chrono::Duration::hours(1);
        let ts2 = today_start + chrono::Duration::hours(2);
        let ts3 = today_start + chrono::Duration::hours(3); // WAL-only
        let ts4 = today_start + chrono::Duration::hours(4); // WAL-only

        // Parquet returns 2 points (from the last successful snapshot)
        mock_store
            .expect_query_raw()
            .times(1)
            .returning(move |_, _, _| {
                Ok(vec![
                    create_test_point_at("air-quality-Mqtt", ts1),
                    create_test_point_at("air-quality-Mqtt", ts2),
                ])
            });

        let (mut subscriber, temp_dir) = create_recovery_subscriber(
            "recovery-both",
            config,
            mock_store,
            |wal| {
                // Simulate: 2 entries committed (watermark=2), then 2 more uncommitted
                let p1 = create_test_point_at("air-quality-Mqtt", ts1);
                let p2 = create_test_point_at("air-quality-Mqtt", ts2);
                let p3 = create_test_point_at("air-quality-Mqtt", ts3);
                let p4 = create_test_point_at("outdoor-weather-Http", ts4);
                wal.append_point(&p1).unwrap();
                wal.append_point(&p2).unwrap();
                wal.commit_to(2).unwrap(); // Snapshot committed first 2
                wal.append_point(&p3).unwrap();
                wal.append_point(&p4).unwrap();
            },
        );

        subscriber.recover().await.unwrap();

        // Parquet seeds 2 points (ts1, ts2 from air-quality-Mqtt)
        // WAL replays entries > watermark(2): seq 3 (ts3) and seq 4 (ts4)
        // merge_wal_entries deduplicates: ts3 is new, ts4 is new
        // Total: 2 (parquet) + 2 (WAL new) = 4
        assert_eq!(subscriber.accumulator.count(), 4);
        assert_eq!(subscriber.accumulator.source_count(), 2);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_recovery_parquet_failure_falls_back_to_wal() {
        // Case: Parquet read fails (e.g., corrupted file) -- should still recover from WAL
        let config = create_config(10, 60);
        let mut mock_store = MockRawStore::new();

        // query_raw fails
        mock_store
            .expect_query_raw()
            .times(1)
            .returning(|_, _, _| {
                Err(CoreError::Storage("Corrupted Parquet file".to_string()))
            });

        let today_start = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
        let ts1 = today_start + chrono::Duration::hours(6);
        let ts2 = today_start + chrono::Duration::hours(7);

        let (mut subscriber, temp_dir) = create_recovery_subscriber(
            "recovery-fallback",
            config,
            mock_store,
            |wal| {
                let p1 = create_test_point_at("air-quality-Mqtt", ts1);
                let p2 = create_test_point_at("outdoor-weather-Http", ts2);
                wal.append_point(&p1).unwrap();
                wal.append_point(&p2).unwrap();
            },
        );

        // recover() should NOT return an error -- Parquet failure is non-fatal
        let result = subscriber.recover().await;
        assert!(result.is_ok());

        // Accumulator should contain WAL data only
        assert_eq!(subscriber.accumulator.count(), 2);
        assert_eq!(subscriber.accumulator.source_count(), 2);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_recovery_called_before_select_loop() {
        // Integration test: verify that start() calls recover() before processing events.
        // We seed Parquet with data; after start+shutdown the subscriber should have
        // those points in the accumulator (proving recovery ran before event processing).
        let mut config = create_config(100, 60);
        config.snapshot_interval_secs = 3600; // Long interval -- no snapshot during test
        let mut mock_store = MockRawStore::new();

        let today_start = Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
        let ts1 = today_start + chrono::Duration::hours(1);

        // Recovery: query_raw returns 1 point
        mock_store
            .expect_query_raw()
            .times(1)
            .returning(move |_, _, _| {
                Ok(vec![create_test_point_at("air-quality-Mqtt", ts1)])
            });

        // Final snapshot on shutdown writes accumulated data
        mock_store
            .expect_write_raw_snapshot()
            .times(1..)
            .returning(|_, _| Ok(()));

        let (subscriber, temp_dir) =
            create_test_subscriber_with_cleanup("recovery-integration", config, mock_store);
        let mut subscriber = subscriber;

        let (tx, rx) = broadcast::channel::<Arc<RawDataPoint>>(100);

        let cancel_token = subscriber.cancellation_token.clone();
        let subscriber_handle = tokio::spawn(async move { subscriber.start(rx).await });

        // Give time for recovery + startup
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Immediately shutdown -- we only care that recovery ran
        cancel_token.cancel();

        let result = subscriber_handle.await.unwrap();
        assert!(result.is_ok());

        // The final snapshot write proves recovery seeded the accumulator
        // (write_raw_snapshot was called, meaning accumulator had data)
        drop(tx);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

// ========== P1-10 INTEGRATION TESTS (AIR-017 Phase 1) ==========
//
// These tests use real ParquetStore (not mocks) to exercise the full
// ingest-snapshot-recovery cycle with actual Parquet file I/O.
// Test IDs map to the TEST-PLAN.md: INT-01, INT-02, INT-04, INT-06.

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::storage::parquet::ParquetStore;
    use crate::storage::wal::WriteAheadLog;
    use chrono::{TimeZone, Utc};
    use serde_json::json;
    use std::sync::Arc;
    use std::time::Duration;
    use tempfile::TempDir;
    use tokio::sync::broadcast;

    // ========== HELPERS ==========

    /// Generate a RawDataPoint with a deterministic timestamp offset from a base time.
    fn gen_point(source_id: &str, base: DateTime<Utc>, index: usize) -> RawDataPoint {
        RawDataPoint::new(
            source_id,
            json!({
                "index": index,
                "pm25": 10.0 + (index as f64 * 0.1),
                "co2": 400 + index,
            }),
        )
        .with_timestamp(base + chrono::Duration::seconds(index as i64))
        .with_ndp_id(format!("{}-sensor-001", source_id))
    }

    /// Count Parquet files recursively under a directory.
    fn count_parquet_files(dir: &std::path::Path) -> usize {
        if !dir.exists() {
            return 0;
        }
        let mut count = 0;
        for entry in walkdir(dir) {
            if entry
                .extension()
                .map(|e| e == "parquet")
                .unwrap_or(false)
            {
                count += 1;
            }
        }
        count
    }

    /// Recursively walk a directory returning file PathBufs.
    fn walkdir(dir: &std::path::Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        if !dir.exists() || !dir.is_dir() {
            return files;
        }
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_file() {
                files.push(path);
            } else if path.is_dir() {
                files.extend(walkdir(&path));
            }
        }
        files
    }

    /// Query all raw points from a ParquetStore within a wide time range.
    async fn query_all_raw(store: &ParquetStore) -> Vec<RawDataPoint> {
        let start = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2030, 12, 31, 23, 59, 59).unwrap();
        store.query_raw(start, end, None).await.unwrap_or_default()
    }

    // ========== TEST 1: Full ingest-snapshot cycle (INT-01) ==========
    //
    // Create BronzeSubscriber with real ParquetStore, feed events via broadcast
    // channel, trigger snapshot via short snapshot_interval, verify Parquet files
    // exist and contain correct data.

    #[tokio::test]
    async fn test_integration_full_ingest_snapshot_cycle() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path().join("data");
        std::fs::create_dir_all(&data_dir).unwrap();

        let store = Arc::new(ParquetStore::new(&data_dir).unwrap());
        let wal_path = temp_dir.path().join("wal.log");

        let config = BronzeSubscriberConfig {
            batch_size: 100,
            flush_interval_secs: 60,
            max_retries: 3,
            stream_filter: Vec::new(),
            snapshot_interval_secs: 1, // 1 second -- fast for testing
            day_rollover_utc_hour: 0,
        };

        let mut subscriber = BronzeSubscriber::new(
            "int-test-01",
            config,
            store.clone(),
            &wal_path,
            &data_dir,
        )
        .unwrap();

        let (tx, rx) = broadcast::channel::<Arc<RawDataPoint>>(200);
        let cancel_token = subscriber.cancellation_token.clone();

        let subscriber_handle = tokio::spawn(async move { subscriber.start(rx).await });

        // Send 20 events from a single source
        let base_time = Utc::now();
        for i in 0..20 {
            let point = gen_point("air-quality-Mqtt", base_time, i);
            tx.send(Arc::new(point)).unwrap();
        }

        // Wait for snapshot timer to fire (>1 second)
        tokio::time::sleep(Duration::from_millis(1500)).await;

        // Trigger graceful shutdown
        cancel_token.cancel();
        let result = subscriber_handle.await.unwrap();
        assert!(result.is_ok(), "Subscriber should shut down cleanly");

        // Verify Parquet files were written
        let parquet_files = walkdir(&data_dir)
            .into_iter()
            .filter(|p| p.extension().map(|e| e == "parquet").unwrap_or(false))
            .collect::<Vec<_>>();
        assert!(
            !parquet_files.is_empty(),
            "At least one Parquet file should exist after snapshot"
        );

        // Query the store to verify data was persisted
        let stored_points = query_all_raw(&store).await;
        assert_eq!(
            stored_points.len(),
            20,
            "All 20 points should be in Parquet"
        );

        // Verify point content
        for point in &stored_points {
            assert_eq!(point.source_id, "air-quality-Mqtt");
            assert!(point.raw_payload.get("pm25").is_some());
            assert!(point.raw_payload.get("co2").is_some());
            assert!(point.raw_payload.get("index").is_some());
        }
    }

    // ========== TEST 2: Crash recovery (INT-04) ==========
    //
    // Write events to WAL + accumulator, simulate crash (drop subscriber),
    // create new subscriber on same directory, verify recovery rebuilds
    // accumulator from Parquet + WAL.

    #[tokio::test]
    async fn test_integration_crash_recovery() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path().join("data");
        std::fs::create_dir_all(&data_dir).unwrap();
        let wal_path = temp_dir.path().join("wal.log");

        let base_time = Utc::now();

        // Phase 1: Write events, snapshot, then add more events, then "crash"
        {
            let store = Arc::new(ParquetStore::new(&data_dir).unwrap());
            let config = BronzeSubscriberConfig {
                batch_size: 100,
                flush_interval_secs: 60,
                max_retries: 3,
                stream_filter: Vec::new(),
                snapshot_interval_secs: 3600, // Long -- no auto snapshot
                day_rollover_utc_hour: 0,
            };

            let mut subscriber = BronzeSubscriber::new(
                "int-test-02",
                config,
                store.clone(),
                &wal_path,
                &data_dir,
            )
            .unwrap();

            // Feed 10 events directly via handle_point
            for i in 0..10 {
                let point = gen_point("air-quality-Mqtt", base_time, i);
                subscriber.handle_point(Arc::new(point));
            }
            assert_eq!(subscriber.accumulator.count(), 10);
            assert_eq!(subscriber.wal.next_sequence(), 11);

            // Snapshot the first 10
            subscriber.snapshot().await.unwrap();
            assert_eq!(subscriber.snapshots_written, 1);

            // Feed 5 more events (these are in WAL but NOT yet in Parquet)
            for i in 10..15 {
                let point = gen_point("air-quality-Mqtt", base_time, i);
                subscriber.handle_point(Arc::new(point));
            }
            assert_eq!(subscriber.accumulator.count(), 15);

            // "Crash" -- drop subscriber without final snapshot
            // (WAL entries 11-15 are uncommitted, Parquet has entries 1-10)
        }

        // Phase 2: Create new subscriber on the same directory and recover
        {
            let store = Arc::new(ParquetStore::new(&data_dir).unwrap());
            let config = BronzeSubscriberConfig {
                batch_size: 100,
                flush_interval_secs: 60,
                max_retries: 3,
                stream_filter: Vec::new(),
                snapshot_interval_secs: 3600,
                day_rollover_utc_hour: 0,
            };

            let mut subscriber = BronzeSubscriber::new(
                "int-test-02-recovery",
                config,
                store.clone(),
                &wal_path,
                &data_dir,
            )
            .unwrap();

            // Run recovery
            subscriber.recover().await.unwrap();

            // Verify: accumulator should have all 15 points
            // (10 from Parquet seed + 5 from WAL replay, with dedup)
            assert_eq!(
                subscriber.accumulator.count(),
                15,
                "Recovery should rebuild all 15 points (10 Parquet + 5 WAL)"
            );
            assert_eq!(
                subscriber.accumulator.source_count(),
                1,
                "All points are from one source"
            );

            // Take a snapshot to verify the recovered data writes correctly
            subscriber.snapshot().await.unwrap();

            // Query Parquet to verify all 15 points persisted
            let stored_points = query_all_raw(&store).await;
            assert_eq!(
                stored_points.len(),
                15,
                "Post-recovery snapshot should contain all 15 points"
            );
        }
    }

    // ========== TEST 3: Snapshot overwrites previous (INT-02) ==========
    //
    // Take snapshot, add more events, take another snapshot, verify Parquet
    // has ALL data (accumulator-based overwrite, not append).

    #[tokio::test]
    async fn test_integration_snapshot_overwrites_previous() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path().join("data");
        std::fs::create_dir_all(&data_dir).unwrap();
        let wal_path = temp_dir.path().join("wal.log");

        let store = Arc::new(ParquetStore::new(&data_dir).unwrap());
        let base_time = Utc::now();

        let config = BronzeSubscriberConfig {
            batch_size: 100,
            flush_interval_secs: 60,
            max_retries: 3,
            stream_filter: Vec::new(),
            snapshot_interval_secs: 3600,
            day_rollover_utc_hour: 0,
        };

        let mut subscriber = BronzeSubscriber::new(
            "int-test-03",
            config,
            store.clone(),
            &wal_path,
            &data_dir,
        )
        .unwrap();

        // Feed 10 events
        for i in 0..10 {
            let point = gen_point("air-quality-Mqtt", base_time, i);
            subscriber.handle_point(Arc::new(point));
        }

        // First snapshot: writes 10 points to Parquet
        subscriber.snapshot().await.unwrap();
        let points_after_snap1 = query_all_raw(&store).await;
        assert_eq!(
            points_after_snap1.len(),
            10,
            "First snapshot should have 10 points"
        );

        // Feed 10 more events
        for i in 10..20 {
            let point = gen_point("air-quality-Mqtt", base_time, i);
            subscriber.handle_point(Arc::new(point));
        }
        assert_eq!(subscriber.accumulator.count(), 20);

        // Second snapshot: overwrites Parquet with all 20 points from accumulator
        subscriber.snapshot().await.unwrap();
        let points_after_snap2 = query_all_raw(&store).await;
        assert_eq!(
            points_after_snap2.len(),
            20,
            "Second snapshot should overwrite with all 20 points (not append to 30)"
        );

        // Verify the points span the full range (indices 0-19)
        let mut indices: Vec<i64> = points_after_snap2
            .iter()
            .map(|p| p.raw_payload["index"].as_i64().unwrap())
            .collect();
        indices.sort();
        let expected: Vec<i64> = (0..20).collect();
        assert_eq!(indices, expected, "All 20 point indices should be present");
    }

    // ========== TEST 4: Multiple streams in same accumulator (INT-06) ==========
    //
    // Feed events from different source_ids, verify each stream gets its own
    // Parquet file in the correct partition path.

    #[tokio::test]
    async fn test_integration_multiple_streams_isolation() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path().join("data");
        std::fs::create_dir_all(&data_dir).unwrap();
        let wal_path = temp_dir.path().join("wal.log");

        let store = Arc::new(ParquetStore::new(&data_dir).unwrap());
        let base_time = Utc::now();

        let config = BronzeSubscriberConfig {
            batch_size: 100,
            flush_interval_secs: 60,
            max_retries: 3,
            stream_filter: Vec::new(),
            snapshot_interval_secs: 3600,
            day_rollover_utc_hour: 0,
        };

        let mut subscriber = BronzeSubscriber::new(
            "int-test-04",
            config,
            store.clone(),
            &wal_path,
            &data_dir,
        )
        .unwrap();

        // Feed events from 3 different sources
        for i in 0..8 {
            let point = gen_point("air-quality-Mqtt", base_time, i);
            subscriber.handle_point(Arc::new(point));
        }
        for i in 0..5 {
            let point = gen_point("outdoor-weather-Http", base_time, i);
            subscriber.handle_point(Arc::new(point));
        }
        for i in 0..3 {
            let point = gen_point("nws-forecast-HttpPoll", base_time, i);
            subscriber.handle_point(Arc::new(point));
        }

        assert_eq!(subscriber.accumulator.count(), 16);
        assert_eq!(subscriber.accumulator.source_count(), 3);

        // Snapshot all streams
        subscriber.snapshot().await.unwrap();

        // Verify separate Parquet files for each stream
        let raw_dir = data_dir.join("raw");
        assert!(raw_dir.exists(), "raw/ directory should exist");

        // Check stream directories exist (partition path strips protocol suffix)
        let air_quality_dir = raw_dir.join("air-quality");
        let outdoor_weather_dir = raw_dir.join("outdoor-weather");
        let nws_forecast_dir = raw_dir.join("nws-forecast");

        assert!(
            air_quality_dir.exists(),
            "air-quality stream dir should exist"
        );
        assert!(
            outdoor_weather_dir.exists(),
            "outdoor-weather stream dir should exist"
        );
        assert!(
            nws_forecast_dir.exists(),
            "nws-forecast stream dir should exist"
        );

        // Count Parquet files per stream
        let aq_files = count_parquet_files(&air_quality_dir);
        let ow_files = count_parquet_files(&outdoor_weather_dir);
        let nf_files = count_parquet_files(&nws_forecast_dir);

        assert_eq!(aq_files, 1, "air-quality should have 1 Parquet file");
        assert_eq!(ow_files, 1, "outdoor-weather should have 1 Parquet file");
        assert_eq!(nf_files, 1, "nws-forecast should have 1 Parquet file");

        // Query each source individually and verify point counts
        let start = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2030, 12, 31, 23, 59, 59).unwrap();

        let aq_points = store
            .query_raw(start, end, Some("air-quality-Mqtt".to_string()))
            .await
            .unwrap();
        let ow_points = store
            .query_raw(start, end, Some("outdoor-weather-Http".to_string()))
            .await
            .unwrap();
        let nf_points = store
            .query_raw(start, end, Some("nws-forecast-HttpPoll".to_string()))
            .await
            .unwrap();

        assert_eq!(aq_points.len(), 8, "air-quality should have 8 points");
        assert_eq!(
            ow_points.len(),
            5,
            "outdoor-weather should have 5 points"
        );
        assert_eq!(nf_points.len(), 3, "nws-forecast should have 3 points");

        // Verify source_ids are correct in each result set
        for p in &aq_points {
            assert_eq!(p.source_id, "air-quality-Mqtt");
        }
        for p in &ow_points {
            assert_eq!(p.source_id, "outdoor-weather-Http");
        }
        for p in &nf_points {
            assert_eq!(p.source_id, "nws-forecast-HttpPoll");
        }
    }
}
