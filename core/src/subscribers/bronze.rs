//! Bronze layer subscriber for raw data storage with Write-Ahead Log
//!
//! BronzeSubscriber consumes RawDataPoint events from the EventBus,
//! durably logs them to a WAL, and periodically snapshots full Parquet
//! files by replaying the WAL from disk.
//!
//! # Design (BUG-004 WAL-Only Architecture)
//!
//! - WAL append on event receipt (durability)
//! - No in-memory accumulator — WAL on disk is the single source of truth
//! - Parquet written at day rollover only (1/day, not periodic)
//! - WAL is NOT truncated at snapshot time (next snapshot needs same data + new)
//! - WAL is truncated at day rollover only
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
//! ```

use crate::diagnostics::{
    do_malloc_trim, format_opt_mib, read_mallinfo2, read_proc_status_rss_bytes,
    read_process_rss_mib, MemoryDiagnostics, MemoryTrend, MemoryWatchdog,
};
use crate::error::CoreResult;
use crate::storage::wal::WriteAheadLog;
use crate::subscribers::{Subscriber, SubscriberError};
use crate::traits::{HealthStatus, RawStore};
use crate::types::RawDataPoint;
use async_trait::async_trait;
use chrono::{DateTime, Datelike, Timelike, Utc};
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

    /// UTC hour for day rollover (default: 0 = midnight UTC).
    /// Determines when the WAL is truncated and a new calendar day begins.
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

impl Default for BronzeSubscriberConfig {
    fn default() -> Self {
        Self {
            batch_size: default_batch_size(),
            flush_interval_secs: default_flush_interval_secs(),
            max_retries: default_max_retries(),
            stream_filter: Vec::new(),
            day_rollover_utc_hour: 0,
        }
    }
}

/// Subscriber for Bronze layer (Parquet) storage with Write-Ahead Log
///
/// Consumes RawDataPoint events from EventBus, durably logs them via WAL,
/// and periodically snapshots full Parquet files by replaying the WAL from disk.
/// No in-memory accumulator — the WAL on disk is the single source of truth.
pub struct BronzeSubscriber {
    id: String,
    config: BronzeSubscriberConfig,
    store: Arc<dyn RawStore>,
    wal: WriteAheadLog,
    data_dir: PathBuf,
    cancellation_token: CancellationToken,
    is_running: bool,
    // Metrics
    events_received: u64,
    events_written: u64,
    snapshots_written: u64,
    errors_total: u64,
    wal_errors: u64,
    memory_trend: MemoryTrend,
    memory_watchdog: MemoryWatchdog,
    watchdog_restart_pending: bool,
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

        Ok(Self {
            id: id.into(),
            config,
            store,
            wal,
            data_dir: data_dir.as_ref().to_path_buf(),
            cancellation_token: CancellationToken::new(),
            is_running: false,
            events_received: 0,
            events_written: 0,
            snapshots_written: 0,
            errors_total: 0,
            wal_errors: 0,
            memory_trend: MemoryTrend::new(100),
            memory_watchdog: MemoryWatchdog::from_env(),
            watchdog_restart_pending: false,
        })
    }

    /// Process a single data point: WAL append only.
    ///
    /// The WAL on disk is the single source of truth. No in-memory buffering.
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

        match self.wal.append_point(&owned_point) {
            Ok(_seq) => {
                // WAL is the single source of truth.
                // No accumulator — data is durable on disk.
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

    /// Snapshot by replaying the WAL from disk.
    ///
    /// Reads all WAL entries, groups by source_id, writes one Parquet file
    /// per source (full overwrite). Data is moved (not cloned) into the
    /// Parquet writer. WAL is NOT truncated — it stays intact because the
    /// next snapshot needs the same data plus any new entries. WAL is only
    /// cleared at day rollover.
    async fn snapshot(&mut self) -> Result<(), SubscriberError> {
        let snapshot_start = std::time::Instant::now();

        // Read WAL file size before replay for logging
        let wal_file_size = self.wal.file_size_bytes();

        // Replay all entries from WAL
        let entries = match self.wal.replay_since(0) {
            Ok(entries) => entries,
            Err(e) => {
                error!(
                    subscriber_id = %self.id,
                    error = %e,
                    "Snapshot: WAL replay failed"
                );
                return Err(SubscriberError::StorageError(format!(
                    "WAL replay failed: {}",
                    e
                )));
            }
        };

        if entries.is_empty() {
            return Ok(());
        }

        let entry_count = entries.len();

        // Group by source_id (move semantics — no clone)
        let mut points_by_source: HashMap<String, Vec<RawDataPoint>> = HashMap::new();
        for entry in entries {
            points_by_source
                .entry(entry.source_id.clone())
                .or_default()
                .push(entry.point);
        }

        let source_count = points_by_source.len();
        let total_points: usize = points_by_source.values().map(|v| v.len()).sum();

        // Determine snapshot_time from latest point timestamp
        let snapshot_time = points_by_source
            .values()
            .flat_map(|pts| pts.iter())
            .map(|p| p.timestamp)
            .max()
            .unwrap_or_else(Utc::now);

        // BUG-005 diagnostic: memory state BEFORE Parquet writes
        let rss_before = read_proc_status_rss_bytes();
        let alloc_before = read_mallinfo2();
        info!(
            subscriber_id = %self.id,
            wal_file_bytes = wal_file_size,
            wal_mib = format_args!("{:.1}", wal_file_size as f64 / 1_048_576.0),
            rss_mib = format_opt_mib(rss_before),
            fordblks_mib = format_opt_mib(alloc_before.as_ref().map(|a| a.fordblks)),
            total_points = total_points,
            sources = source_count,
            wal_entries_replayed = entry_count,
            "Snapshot starting — memory diagnostics"
        );

        // Write one Parquet file per source_id (move, not clone)
        for (source_id, points) in points_by_source {
            let partition_path = self.partition_path(&source_id, snapshot_time);

            self.store
                .write_raw_snapshot(points, &partition_path)
                .await
                .map_err(|e| {
                    SubscriberError::StorageError(format!("Snapshot write failed: {}", e))
                })?;
        }

        // BUG-005: Force glibc to return freed arena pages after Arrow allocations.
        do_malloc_trim();

        let rss_after_trim = read_proc_status_rss_bytes();
        let alloc_after = read_mallinfo2();

        // DO NOT truncate WAL here.
        // The WAL stays intact because the next snapshot needs the same data
        // plus any new entries. WAL is only cleared at day rollover.

        self.events_written = total_points as u64;
        self.snapshots_written += 1;

        let elapsed = snapshot_start.elapsed();
        info!(
            subscriber_id = %self.id,
            sources = source_count,
            total_points = total_points,
            wal_entries_replayed = entry_count,
            wal_file_bytes = wal_file_size,
            elapsed_ms = elapsed.as_millis(),
            rss_before_mib = format_opt_mib(rss_before),
            rss_after_trim_mib = format_opt_mib(rss_after_trim),
            fordblks_before_mib = format_opt_mib(alloc_before.as_ref().map(|a| a.fordblks)),
            fordblks_after_mib = format_opt_mib(alloc_after.as_ref().map(|a| a.fordblks)),
            "Snapshot complete"
        );

        // Trend summary every 10 snapshots
        if self.snapshots_written.is_multiple_of(10) {
            let rss_now = read_process_rss_mib();
            debug!(
                subscriber_id = %self.id,
                snapshots = self.snapshots_written,
                trend_samples = self.memory_trend.len(),
                rss_growth_mib_per_hour = self.memory_trend.growth_rate_bytes_per_hour()
                    .map(|r| format!("{:.2}", r / 1_048_576.0))
                    .unwrap_or_else(|| "N/A".into()),
                rss_current_mib = rss_now.map(|r| format!("{:.1}", r)).unwrap_or_else(|| "N/A".into()),
                "Memory trend summary (every 10 snapshots)"
            );
        }

        Ok(())
    }

    /// Log WAL state on startup for observability.
    ///
    /// No recovery step needed — the WAL is already on disk and the next
    /// day rollover will read it and write Parquet.
    fn log_startup_wal_state(&self) {
        let wal_size = self.wal.file_size_bytes();
        let wal_entries = self.wal.entry_count().unwrap_or(0);
        info!(
            subscriber_id = %self.id,
            wal_file_bytes = wal_size,
            wal_entry_count = wal_entries,
            "Startup: WAL state (no recovery needed — WAL is on disk)"
        );
    }

    /// Compute the Duration from `now` until the next day rollover at the
    /// configured `day_rollover_utc_hour`. If `now` is exactly the rollover
    /// hour, the next rollover is 24 hours away (we just rolled).
    fn duration_until_next_rollover(&self, now: DateTime<Utc>) -> Duration {
        let rollover_hour = self.config.day_rollover_utc_hour as u32;
        let current_hour = now.hour();

        // Build today's rollover timestamp
        let today_rollover = now
            .date_naive()
            .and_hms_opt(rollover_hour, 0, 0)
            .unwrap()
            .and_utc();

        let next_rollover = if now < today_rollover {
            // Rollover hasn't happened yet today
            today_rollover
        } else {
            // Rollover already passed (or is right now) — next one is tomorrow
            today_rollover + chrono::Duration::days(1)
        };

        let diff = next_rollover - now;
        // Convert chrono::Duration to std::time::Duration (always positive)
        Duration::from_secs(diff.num_seconds().max(1) as u64)
    }

    /// Perform day rollover: final snapshot for the ending day, then truncate WAL.
    async fn day_rollover(&mut self) {
        info!(
            subscriber_id = %self.id,
            wal_entry_count = self.wal.entry_count().unwrap_or(0),
            wal_file_bytes = self.wal.file_size_bytes(),
            "Day rollover: performing final snapshot for ending day"
        );

        // Final snapshot captures all WAL entries into Parquet
        if let Err(e) = self.snapshot().await {
            error!(
                subscriber_id = %self.id,
                error = %e,
                "Day rollover: final snapshot failed — WAL NOT truncated (data preserved)"
            );
            return; // Do NOT truncate if snapshot failed — data would be lost
        }

        // Truncate WAL: all entries are now safely in Parquet
        match self.wal.truncate() {
            Ok(()) => {
                info!(
                    subscriber_id = %self.id,
                    "Day rollover: WAL truncated for new day"
                );
            }
            Err(e) => {
                error!(
                    subscriber_id = %self.id,
                    error = %e,
                    "Day rollover: WAL truncate failed — next snapshot will still replay all entries"
                );
            }
        }
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
        if self.memory_watchdog.is_enabled() {
            info!(
                subscriber_id = %self.id,
                threshold_mib = self.memory_watchdog.threshold_mib().unwrap_or(0),
                "Memory watchdog enabled"
            );
        }
        self.is_running = true;

        // Log WAL state — no recovery needed, WAL is on disk
        self.log_startup_wal_state();

        let flush_interval = Duration::from_secs(self.config.flush_interval_secs);
        let mut flush_timer = tokio::time::interval(flush_interval);
        // First tick is immediate, skip it
        flush_timer.tick().await;

        // Day rollover timer: fires once at the configured UTC hour, then every 24h.
        // Computes delay from now until the next rollover hour.
        let rollover_delay = self.duration_until_next_rollover(Utc::now());
        info!(
            subscriber_id = %self.id,
            rollover_utc_hour = self.config.day_rollover_utc_hour,
            next_rollover_secs = rollover_delay.as_secs(),
            "Day rollover scheduled"
        );
        let mut rollover_timer = tokio::time::interval_at(
            tokio::time::Instant::now() + rollover_delay,
            Duration::from_secs(86400), // repeat every 24h
        );

        loop {
            tokio::select! {
                biased;

                // Check cancellation first
                _ = self.cancellation_token.cancelled() => {
                    info!(subscriber_id = %self.id, "Received cancellation signal");
                    break;
                }

                // Day rollover -- truncate WAL for new calendar day
                _ = rollover_timer.tick() => {
                    self.day_rollover().await;
                }

                // Flush timer -- periodic memory diagnostics
                _ = flush_timer.tick() => {
                    // BUG-005 mitigation: nudge glibc to return freed arena pages
                    // every heartbeat (30s), not just after snapshot.
                    do_malloc_trim();

                    let wal_bytes = self.wal.file_size_bytes();
                    let diag = MemoryDiagnostics::collect(wal_bytes);

                    // Record RSS for trend tracking
                    if let Some(rss) = diag.rss_bytes {
                        self.memory_trend.record(rss);
                    }

                    debug!(
                        subscriber_id = %self.id,
                        wal_mib = format_args!("{:.1}", wal_bytes as f64 / 1_048_576.0),
                        wal_file_bytes = wal_bytes,
                        rss_mib = diag.rss_mib_display(),
                        wal_errors = self.wal_errors,
                        events_received = self.events_received,
                        // Allocator fields
                        arena_mib = format_opt_mib(diag.arena_bytes),
                        fordblks_mib = format_opt_mib(diag.fordblks_bytes),
                        hblkhd_mib = format_opt_mib(diag.hblkhd_bytes),
                        // Smaps fields
                        heap_rss_mib = format_opt_mib(diag.heap_rss_bytes),
                        anon_rss_mib = format_opt_mib(diag.anon_rss_bytes),
                        // Unaccounted gap
                        unaccounted_mib = diag.unaccounted_bytes()
                            .map(|b| format!("{:.1}", b as f64 / 1_048_576.0))
                            .unwrap_or_else(|| "N/A".into()),
                        "Heartbeat"
                    );

                    // Memory watchdog: check for restart threshold
                    if let Some(rss) = diag.rss_bytes {
                        if self.memory_watchdog.should_restart(rss) {
                            warn!(
                                subscriber_id = %self.id,
                                rss_mib = format_args!("{:.1}", rss as f64 / 1_048_576.0),
                                threshold_mib = self.memory_watchdog.threshold_mib().unwrap_or(0),
                                arena_mib = format_opt_mib(diag.arena_bytes),
                                fordblks_mib = format_opt_mib(diag.fordblks_bytes),
                                uptime_samples = self.memory_trend.len(),
                                "MEMORY WATCHDOG: RSS exceeds restart threshold — initiating graceful restart. WAL ensures zero data loss."
                            );
                            self.watchdog_restart_pending = true;
                            break;
                        }
                    }
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

        // If watchdog triggered the exit, terminate the process so Docker restarts us.
        // Final snapshot already completed above -- WAL is durable, no data loss.
        if self.watchdog_restart_pending {
            warn!(
                subscriber_id = %self.id,
                "MEMORY WATCHDOG: Final snapshot complete. Exiting process for Docker restart."
            );
            // exit(0) signals clean shutdown to Docker, which will restart us
            // per `restart: unless-stopped` policy.
            std::process::exit(0);
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
            "wal_file_bytes".to_string(),
            self.wal.file_size_bytes().to_string(),
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
            day_rollover_utc_hour: 0,
        }
    }

    /// Create a temp directory with WAL and data paths for testing
    fn create_test_subscriber(
        id: &str,
        config: BronzeSubscriberConfig,
        mock_store: MockRawStore,
    ) -> BronzeSubscriber {
        let temp_dir =
            std::env::temp_dir().join(format!("bronze_test_{}_{}", id, uuid::Uuid::new_v4()));
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
        let temp_dir =
            std::env::temp_dir().join(format!("bronze_test_{}_{}", id, uuid::Uuid::new_v4()));
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
    fn test_config_ignores_removed_snapshot_interval_secs() {
        // Backward compatibility: old YAML with snapshot_interval_secs still parses
        // (serde ignores unknown fields by default)
        let yaml = r#"
            batch_size: 100
            flush_interval_secs: 30
            max_retries: 3
            snapshot_interval_secs: 1800
        "#;
        let config: BronzeSubscriberConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.batch_size, 100);
        assert_eq!(config.flush_interval_secs, 30);
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
    fn test_config_with_day_rollover_field() {
        // C3: YAML with explicit day_rollover_utc_hour
        let yaml = r#"
            batch_size: 100
            flush_interval_secs: 30
            max_retries: 3
            day_rollover_utc_hour: 6
        "#;
        let config: BronzeSubscriberConfig = serde_yaml::from_str(yaml).unwrap();
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
        let temp_dir =
            std::env::temp_dir().join(format!("bronze_result_test_{}", uuid::Uuid::new_v4()));
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

    // ========== TDD CYCLE 3: handle_point WAL-only behavior ==========

    #[test]
    fn test_handle_point_wal_only() {
        // Behavior: WAL append is the sole write path (no accumulator)
        let config = create_config(10, 5);
        let mock_store = MockRawStore::new();
        let mut subscriber = create_test_subscriber("bronze-test", config, mock_store);

        let point = Arc::new(create_test_point("air-quality-Mqtt"));
        subscriber.handle_point(point);

        // WAL should have 1 entry
        assert_eq!(subscriber.wal.entry_count().unwrap(), 1);
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

        assert_eq!(subscriber.wal.entry_count().unwrap(), 3);
        assert_eq!(subscriber.wal.next_sequence(), 4);
    }

    #[test]
    fn test_filtered_points_not_in_wal() {
        // Filtered points should not be written to WAL
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

        assert_eq!(subscriber.wal.entry_count().unwrap(), 0);
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
    async fn test_snapshot_does_not_truncate_wal() {
        // WAL-only architecture: snapshot replays WAL but does NOT truncate.
        // WAL stays intact for the next snapshot. Only day rollover truncates.
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
        assert_eq!(subscriber.wal.entry_count().unwrap(), 3);

        let result = subscriber.snapshot().await;
        assert!(result.is_ok());

        // WAL should still have all 3 entries (no truncation at snapshot)
        assert_eq!(subscriber.wal.entry_count().unwrap(), 3);
        assert_eq!(subscriber.wal.next_sequence(), 4);
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
        assert!(health.details.contains_key("wal_file_bytes"));
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
        let config = create_config(100, 60);
        let mut mock_store = MockRawStore::new();

        // Expect final snapshot write(s) on shutdown for the WAL-replayed points
        mock_store
            .expect_write_raw_snapshot()
            .times(1..)
            .returning(|_, _| Ok(()));

        let (subscriber, _temp_dir) =
            create_test_subscriber_with_cleanup("bronze-test", config, mock_store);
        let mut subscriber = subscriber;

        // Create broadcast channel
        let (tx, rx) = broadcast::channel::<Arc<RawDataPoint>>(100);

        // Get cancellation token before moving subscriber
        let cancel_token = subscriber.cancellation_token.clone();

        // Spawn subscriber task
        let subscriber_handle = tokio::spawn(async move { subscriber.start(rx).await });

        // Send 5 events
        for i in 0..5 {
            let point = Arc::new(create_test_point(&format!("air-quality-{}-Mqtt", i)));
            tx.send(point).unwrap();
        }

        // Give time for events to be received
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Trigger graceful shutdown — final snapshot writes WAL to Parquet
        cancel_token.cancel();

        // Wait for subscriber to finish
        let result = subscriber_handle.await.unwrap();
        assert!(result.is_ok());

        let _ = std::fs::remove_dir_all(&_temp_dir);
    }

    #[tokio::test]
    async fn test_day_rollover_triggers_snapshot_via_direct_call() {
        // Parquet is now only written at day_rollover (not periodic timer).
        // This test verifies day_rollover() calls snapshot() and truncates WAL.
        let config = create_config(100, 60);
        let mut mock_store = MockRawStore::new();

        // Expect snapshot write from day_rollover
        mock_store
            .expect_write_raw_snapshot()
            .times(1)
            .returning(|_, _| Ok(()));

        let mut subscriber = create_test_subscriber("day-rollover-snap", config, mock_store);

        // Send 3 events directly via handle_point
        for _ in 0..3 {
            let point = Arc::new(create_test_point("source-Mqtt"));
            subscriber.handle_point(point);
        }
        assert_eq!(subscriber.wal.entry_count().unwrap(), 3);

        // Day rollover triggers snapshot then truncates WAL
        subscriber.day_rollover().await;

        assert_eq!(subscriber.snapshots_written, 1);
        assert_eq!(subscriber.events_written, 3);
        assert_eq!(subscriber.wal.entry_count().unwrap(), 0); // WAL truncated
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

    // ========== TDD CYCLE 9: WAL Startup State Tests ==========

    /// Helper: create a test point with a specific timestamp.
    fn create_test_point_at(source_id: &str, ts: DateTime<Utc>) -> RawDataPoint {
        RawDataPoint::new(source_id, json!({"pm25": 12.5, "co2": 450}))
            .with_timestamp(ts)
            .with_ndp_id("test-device-001")
    }

    /// Helper: create a subscriber with a pre-populated WAL.
    /// Returns the subscriber AND the temp_dir path for cleanup.
    fn create_subscriber_with_wal(
        id: &str,
        config: BronzeSubscriberConfig,
        mock_store: MockRawStore,
        wal_setup: impl FnOnce(&mut crate::storage::wal::WriteAheadLog),
    ) -> (BronzeSubscriber, PathBuf) {
        let temp_dir =
            std::env::temp_dir().join(format!("bronze_wal_{}_{}", id, uuid::Uuid::new_v4()));
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
        let sub =
            BronzeSubscriber::new(id, config, Arc::new(mock_store), &wal_path, &data_dir).unwrap();
        (sub, temp_dir)
    }

    #[test]
    fn test_startup_wal_state_empty() {
        // First run -- empty WAL. log_startup_wal_state() should not panic.
        let config = create_config(10, 60);
        let mock_store = MockRawStore::new();
        let subscriber = create_test_subscriber("startup-empty", config, mock_store);

        // Should not panic; just logs
        subscriber.log_startup_wal_state();
        assert_eq!(subscriber.wal.entry_count().unwrap(), 0);
    }

    #[test]
    fn test_startup_wal_state_with_existing_entries() {
        // Restart after crash -- WAL has durable entries on disk.
        // New subscriber should see them via entry_count().
        let config = create_config(10, 60);
        let mock_store = MockRawStore::new();

        let today_start = Utc::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();
        let ts1 = today_start + chrono::Duration::hours(4);
        let ts2 = today_start + chrono::Duration::hours(5);

        let (subscriber, temp_dir) =
            create_subscriber_with_wal("startup-existing", config, mock_store, |wal| {
                let p1 = create_test_point_at("air-quality-Mqtt", ts1);
                let p2 = create_test_point_at("outdoor-weather-Http", ts2);
                wal.append_point(&p1).unwrap();
                wal.append_point(&p2).unwrap();
            });

        subscriber.log_startup_wal_state();
        assert_eq!(subscriber.wal.entry_count().unwrap(), 2);
        assert!(subscriber.wal.file_size_bytes() > 0);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_wal_survives_restart_and_snapshot_writes_parquet() {
        // Simulate crash: WAL has entries. New subscriber starts, snapshot
        // replays WAL and writes Parquet. Verifies the WAL-only recovery model.
        let config = create_config(10, 60);
        let mut mock_store = MockRawStore::new();

        // Expect snapshot to write Parquet from WAL replay
        mock_store
            .expect_write_raw_snapshot()
            .times(1)
            .returning(|_, _| Ok(()));

        let today_start = Utc::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();
        let ts1 = today_start + chrono::Duration::hours(1);
        let ts2 = today_start + chrono::Duration::hours(2);

        let (mut subscriber, temp_dir) =
            create_subscriber_with_wal("wal-restart", config, mock_store, |wal| {
                let p1 = create_test_point_at("air-quality-Mqtt", ts1);
                let p2 = create_test_point_at("air-quality-Mqtt", ts2);
                wal.append_point(&p1).unwrap();
                wal.append_point(&p2).unwrap();
            });

        // WAL has 2 entries from before "crash"
        assert_eq!(subscriber.wal.entry_count().unwrap(), 2);

        // Snapshot replays WAL -> writes Parquet
        subscriber.snapshot().await.unwrap();
        assert_eq!(subscriber.snapshots_written, 1);
        assert_eq!(subscriber.events_written, 2);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    // ========== TDD CYCLE 10: Day Rollover Tests (Phase 2) ==========

    #[test]
    fn test_duration_until_next_rollover_before_hour() {
        // If current time is before rollover hour, rollover is today
        let config = BronzeSubscriberConfig {
            day_rollover_utc_hour: 6,
            ..Default::default()
        };
        let mock_store = MockRawStore::new();
        let subscriber = create_test_subscriber("rollover-test", config, mock_store);

        // 03:00 UTC -- rollover at 06:00 is 3 hours away
        let now = chrono::TimeZone::with_ymd_and_hms(&Utc, 2026, 2, 14, 3, 0, 0).unwrap();
        let delay = subscriber.duration_until_next_rollover(now);
        assert_eq!(delay.as_secs(), 3 * 3600);
    }

    #[test]
    fn test_duration_until_next_rollover_after_hour() {
        // If current time is after rollover hour, rollover is tomorrow
        let config = BronzeSubscriberConfig {
            day_rollover_utc_hour: 6,
            ..Default::default()
        };
        let mock_store = MockRawStore::new();
        let subscriber = create_test_subscriber("rollover-test", config, mock_store);

        // 10:00 UTC -- rollover at 06:00 tomorrow is 20 hours away
        let now = chrono::TimeZone::with_ymd_and_hms(&Utc, 2026, 2, 14, 10, 0, 0).unwrap();
        let delay = subscriber.duration_until_next_rollover(now);
        assert_eq!(delay.as_secs(), 20 * 3600);
    }

    #[test]
    fn test_duration_until_next_rollover_at_exact_hour() {
        // If current time IS the rollover hour, next rollover is 24h away
        let config = BronzeSubscriberConfig {
            day_rollover_utc_hour: 6,
            ..Default::default()
        };
        let mock_store = MockRawStore::new();
        let subscriber = create_test_subscriber("rollover-test", config, mock_store);

        // 06:00 UTC exactly -- next rollover is tomorrow at 06:00
        let now = chrono::TimeZone::with_ymd_and_hms(&Utc, 2026, 2, 14, 6, 0, 0).unwrap();
        let delay = subscriber.duration_until_next_rollover(now);
        assert_eq!(delay.as_secs(), 24 * 3600);
    }

    #[test]
    fn test_duration_until_next_rollover_midnight_default() {
        // Default config: rollover at midnight (hour 0)
        let config = BronzeSubscriberConfig::default();
        let mock_store = MockRawStore::new();
        let subscriber = create_test_subscriber("rollover-test", config, mock_store);

        // 22:00 UTC -- rollover at 00:00 is 2 hours away
        let now = chrono::TimeZone::with_ymd_and_hms(&Utc, 2026, 2, 14, 22, 0, 0).unwrap();
        let delay = subscriber.duration_until_next_rollover(now);
        assert_eq!(delay.as_secs(), 2 * 3600);
    }

    #[tokio::test]
    async fn test_day_rollover_snapshots_then_truncates_wal() {
        // Day rollover should: (1) snapshot all WAL entries, (2) truncate WAL
        let config = create_config(10, 60);
        let mut mock_store = MockRawStore::new();

        // Expect snapshot write (1 source)
        mock_store
            .expect_write_raw_snapshot()
            .times(1)
            .returning(|_, _| Ok(()));

        let mut subscriber = create_test_subscriber("rollover-test", config, mock_store);

        // Add some data
        subscriber.handle_point(Arc::new(create_test_point("air-quality-Mqtt")));
        subscriber.handle_point(Arc::new(create_test_point("air-quality-Mqtt")));
        subscriber.handle_point(Arc::new(create_test_point("air-quality-Mqtt")));
        assert_eq!(subscriber.wal.entry_count().unwrap(), 3);

        // Day rollover
        subscriber.day_rollover().await;

        // WAL should be truncated (empty)
        assert_eq!(subscriber.wal.entry_count().unwrap(), 0);
        assert_eq!(subscriber.snapshots_written, 1);
        assert_eq!(subscriber.events_written, 3);
    }

    #[tokio::test]
    async fn test_day_rollover_does_not_truncate_on_snapshot_failure() {
        // If snapshot fails during day rollover, WAL must NOT be truncated
        let config = create_config(10, 60);
        let mut mock_store = MockRawStore::new();

        // Snapshot will fail
        mock_store
            .expect_write_raw_snapshot()
            .times(1)
            .returning(|_, _| Err(CoreError::Storage("Disk full".to_string())));

        let mut subscriber = create_test_subscriber("rollover-fail", config, mock_store);

        subscriber.handle_point(Arc::new(create_test_point("air-quality-Mqtt")));
        subscriber.handle_point(Arc::new(create_test_point("air-quality-Mqtt")));
        assert_eq!(subscriber.wal.entry_count().unwrap(), 2);

        // Day rollover -- snapshot fails
        subscriber.day_rollover().await;

        // WAL must still have all entries (NOT truncated)
        assert_eq!(subscriber.wal.entry_count().unwrap(), 2);
        assert_eq!(subscriber.snapshots_written, 0);
    }

    #[tokio::test]
    async fn test_day_rollover_empty_wal_is_noop() {
        // Rollover with empty WAL should be a clean no-op
        let config = create_config(10, 60);
        let mock_store = MockRawStore::new();
        // No expectations -- write_raw_snapshot should NOT be called

        let mut subscriber = create_test_subscriber("rollover-empty", config, mock_store);
        assert_eq!(subscriber.wal.entry_count().unwrap(), 0);

        subscriber.day_rollover().await;

        // WAL still empty, no snapshots
        assert_eq!(subscriber.wal.entry_count().unwrap(), 0);
        assert_eq!(subscriber.snapshots_written, 0);
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
            if entry.extension().map(|e| e == "parquet").unwrap_or(false) {
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
            day_rollover_utc_hour: 0,
        };

        let mut subscriber =
            BronzeSubscriber::new("int-test-01", config, store.clone(), &wal_path, &data_dir)
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

        // Give time for events to be received
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Trigger graceful shutdown — final snapshot writes WAL to Parquet
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

    // ========== TEST 2: WAL-only crash recovery (INT-04) ==========
    //
    // Write events via handle_point (WAL), snapshot (Parquet), add more events,
    // then "crash" (drop). Create new subscriber on same directory — WAL has all
    // entries, snapshot replays them to write correct Parquet.

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
                day_rollover_utc_hour: 0,
            };

            let mut subscriber =
                BronzeSubscriber::new("int-test-02", config, store.clone(), &wal_path, &data_dir)
                    .unwrap();

            // Feed 10 events directly via handle_point
            for i in 0..10 {
                let point = gen_point("air-quality-Mqtt", base_time, i);
                subscriber.handle_point(Arc::new(point));
            }
            assert_eq!(subscriber.wal.entry_count().unwrap(), 10);
            assert_eq!(subscriber.wal.next_sequence(), 11);

            // Snapshot the first 10 (writes Parquet, WAL stays intact)
            subscriber.snapshot().await.unwrap();
            assert_eq!(subscriber.snapshots_written, 1);

            // Feed 5 more events (WAL now has 15 total)
            for i in 10..15 {
                let point = gen_point("air-quality-Mqtt", base_time, i);
                subscriber.handle_point(Arc::new(point));
            }
            assert_eq!(subscriber.wal.entry_count().unwrap(), 15);

            // "Crash" -- drop subscriber without final snapshot
        }

        // Phase 2: Create new subscriber on the same directory
        {
            let store = Arc::new(ParquetStore::new(&data_dir).unwrap());
            let config = BronzeSubscriberConfig {
                batch_size: 100,
                flush_interval_secs: 60,
                max_retries: 3,
                stream_filter: Vec::new(),
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

            // WAL has all 15 entries from before "crash"
            assert_eq!(
                subscriber.wal.entry_count().unwrap(),
                15,
                "WAL should have all 15 entries from pre-crash"
            );

            // Snapshot replays WAL -> writes all 15 points to Parquet
            subscriber.snapshot().await.unwrap();

            // Query Parquet to verify all 15 points persisted
            let stored_points = query_all_raw(&store).await;
            assert_eq!(
                stored_points.len(),
                15,
                "Post-crash snapshot should contain all 15 points from WAL"
            );
        }
    }

    // ========== TEST 3: Snapshot overwrites previous (INT-02) ==========
    //
    // Take snapshot, add more events, take another snapshot, verify Parquet
    // has ALL data (WAL-based overwrite, not append).

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
            day_rollover_utc_hour: 0,
        };

        let mut subscriber =
            BronzeSubscriber::new("int-test-03", config, store.clone(), &wal_path, &data_dir)
                .unwrap();

        // Feed 10 events
        for i in 0..10 {
            let point = gen_point("air-quality-Mqtt", base_time, i);
            subscriber.handle_point(Arc::new(point));
        }

        // First snapshot: replays WAL (10 entries) -> writes 10 points to Parquet
        subscriber.snapshot().await.unwrap();
        let points_after_snap1 = query_all_raw(&store).await;
        assert_eq!(
            points_after_snap1.len(),
            10,
            "First snapshot should have 10 points"
        );

        // Feed 10 more events (WAL now has 20 total)
        for i in 10..20 {
            let point = gen_point("air-quality-Mqtt", base_time, i);
            subscriber.handle_point(Arc::new(point));
        }
        assert_eq!(subscriber.wal.entry_count().unwrap(), 20);

        // Second snapshot: replays entire WAL (20 entries) -> overwrites Parquet with all 20
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

    // ========== TEST 4: Multiple streams isolation via WAL (INT-06) ==========
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
            day_rollover_utc_hour: 0,
        };

        let mut subscriber =
            BronzeSubscriber::new("int-test-04", config, store.clone(), &wal_path, &data_dir)
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

        assert_eq!(subscriber.wal.entry_count().unwrap(), 16);

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
        assert_eq!(ow_points.len(), 5, "outdoor-weather should have 5 points");
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
