//! Daemon mode for Silver ETL
//!
//! Provides continuous ETL execution with configurable intervals,
//! graceful shutdown handling, and health monitoring.
//!
//! ## Usage
//!
//! ```bash
//! silver-etl daemon --interval 300  # Run every 5 minutes
//! silver-etl daemon --interval 60 --stream air-quality  # Specific stream
//! ```
//!
//! ## Architecture
//!
//! The daemon follows the same pattern as air-quality-app:
//! - `tokio::time::interval` for periodic execution
//! - `tokio::signal::ctrl_c()` for graceful shutdown
//! - `tokio::select!` to handle both concurrently

use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::watch;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::config::ConfigLoader;
use crate::etl::{EtlRunner, EtlStats};
use crate::metrics::EtlMetrics;
use crate::persistence::{EtlRunMode, EtlRunPersistence, NoOpPersistence};

/// Trait for ETL execution - enables mocking in tests (London TDD style)
///
/// Note: We use Send only (not Sync) because DuckDB Connection is not Sync.
/// The daemon uses interior mutability (Mutex) for thread-safe access.
#[cfg_attr(test, mockall::automock)]
pub trait EtlExecutor: Send {
    /// Run ETL for a single stream
    fn run_stream(&self, stream_id: &str) -> Result<EtlStats, DaemonError>;

    /// Get list of enabled streams
    fn list_enabled_streams(&self) -> Result<Vec<String>, DaemonError>;
}

/// Daemon-specific errors
#[derive(Debug, thiserror::Error)]
pub enum DaemonError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("ETL execution error: {0}")]
    Etl(String),

    #[error("Shutdown signal received")]
    Shutdown,
}

/// Configuration for daemon mode
#[derive(Debug, Clone)]
pub struct DaemonConfig {
    /// Interval between ETL runs in seconds
    pub interval_secs: u64,
    /// Optional specific stream to process (None = all enabled)
    pub stream_filter: Option<String>,
    /// Maximum consecutive failures before backoff
    pub max_consecutive_failures: u32,
    /// Backoff multiplier on failure
    pub backoff_multiplier: f64,
    /// ETL run mode for persistence tracking (dp-011)
    pub run_mode: EtlRunMode,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            interval_secs: 300,
            stream_filter: None,
            max_consecutive_failures: 3,
            backoff_multiplier: 2.0,
            run_mode: EtlRunMode::Daemon,
        }
    }
}

/// Statistics from a daemon run cycle
#[derive(Debug, Default, Clone)]
pub struct DaemonCycleStats {
    pub streams_processed: usize,
    pub streams_succeeded: usize,
    pub streams_failed: usize,
    pub total_rows_processed: u64,
    pub total_rows_flagged: u64,
    pub cycle_duration_ms: u64,
}

/// Daemon runner for continuous ETL execution
///
/// Supports optional persistence of run statistics (dp-011).
/// When persistence is enabled, each stream run is tracked with:
/// - `daemon_cycle_id`: Links all runs within the same cycle
/// - Statistics: rows processed, flagged, rejected
/// - Watermarks: for incremental load tracking
/// - Error context: for debugging failures
pub struct DaemonRunner<E: EtlExecutor, P: EtlRunPersistence = NoOpPersistence> {
    executor: Mutex<E>,
    config: DaemonConfig,
    shutdown_rx: watch::Receiver<bool>,
    /// Optional persistence for run statistics (dp-011)
    persistence: P,
}

impl<E: EtlExecutor + 'static> DaemonRunner<E, NoOpPersistence> {
    /// Create a new daemon runner without persistence (backwards compatible)
    pub fn new(executor: E, config: DaemonConfig, shutdown_rx: watch::Receiver<bool>) -> Self {
        Self {
            executor: Mutex::new(executor),
            config,
            shutdown_rx,
            persistence: NoOpPersistence::new(),
        }
    }
}

impl<E: EtlExecutor + 'static, P: EtlRunPersistence + 'static> DaemonRunner<E, P> {
    /// Create a new daemon runner with persistence (dp-011)
    pub fn with_persistence(
        executor: E,
        config: DaemonConfig,
        shutdown_rx: watch::Receiver<bool>,
        persistence: P,
    ) -> Self {
        Self {
            executor: Mutex::new(executor),
            config,
            shutdown_rx,
            persistence,
        }
    }

    /// Run the daemon loop
    ///
    /// This runs until a shutdown signal is received or a fatal error occurs.
    pub async fn run(&mut self) -> Result<(), DaemonError> {
        let interval_duration = Duration::from_secs(self.config.interval_secs);
        let mut interval = tokio::time::interval(interval_duration);
        let mut consecutive_failures: u32 = 0;

        info!(
            interval_secs = self.config.interval_secs,
            stream_filter = ?self.config.stream_filter,
            "Starting daemon mode"
        );

        // Run immediately on start, then on interval
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    match self.run_cycle() {
                        Ok(stats) => {
                            consecutive_failures = 0;
                            info!(
                                streams_processed = stats.streams_processed,
                                streams_succeeded = stats.streams_succeeded,
                                streams_failed = stats.streams_failed,
                                total_rows = stats.total_rows_processed,
                                duration_ms = stats.cycle_duration_ms,
                                "ETL cycle completed"
                            );
                        }
                        Err(DaemonError::Shutdown) => {
                            info!("Shutdown requested, exiting daemon loop");
                            return Ok(());
                        }
                        Err(e) => {
                            consecutive_failures += 1;
                            error!(
                                error = %e,
                                consecutive_failures = consecutive_failures,
                                "ETL cycle failed"
                            );

                            // Apply backoff if too many failures
                            if consecutive_failures >= self.config.max_consecutive_failures {
                                let backoff_secs = (self.config.interval_secs as f64
                                    * self.config.backoff_multiplier) as u64;
                                warn!(
                                    backoff_secs = backoff_secs,
                                    "Too many consecutive failures, applying backoff"
                                );
                                tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
                            }
                        }
                    }
                }
                _ = self.wait_for_shutdown() => {
                    info!("Shutdown signal received, stopping daemon");
                    return Ok(());
                }
            }
        }
    }

    /// Execute a single ETL cycle
    ///
    /// Integrates with persistence layer (dp-011) to track run statistics.
    /// Persistence failures are handled gracefully - they log warnings but
    /// do NOT fail the ETL cycle.
    fn run_cycle(&self) -> Result<DaemonCycleStats, DaemonError> {
        let start = std::time::Instant::now();
        let mut stats = DaemonCycleStats::default();

        // Generate cycle ID for linking all stream runs in this cycle (dp-011)
        let daemon_cycle_id = Uuid::new_v4();
        debug!(cycle_id = %daemon_cycle_id, "Starting ETL cycle");

        // Check for shutdown before starting
        if *self.shutdown_rx.borrow() {
            return Err(DaemonError::Shutdown);
        }

        // Lock executor for this cycle
        let executor = self
            .executor
            .lock()
            .map_err(|e| DaemonError::Etl(format!("Mutex poisoned: {}", e)))?;

        // Get list of streams to process
        let streams = match &self.config.stream_filter {
            Some(stream_id) => vec![stream_id.clone()],
            None => executor.list_enabled_streams()?,
        };

        stats.streams_processed = streams.len();

        // Process each stream with persistence tracking
        for stream_id in &streams {
            // Check for shutdown between streams
            if *self.shutdown_rx.borrow() {
                return Err(DaemonError::Shutdown);
            }

            // Start run record (dp-011) - graceful degradation on failure
            let run_id = match self.persistence.start_run(
                stream_id,
                self.config.run_mode,
                Some(daemon_cycle_id),
            ) {
                Ok(id) => Some(id),
                Err(e) => {
                    // CRITICAL: Persistence failure must NOT fail ETL
                    warn!(
                        stream_id = %stream_id,
                        error = %e,
                        "Failed to start run record - continuing without persistence"
                    );
                    None
                }
            };

            match executor.run_stream(stream_id) {
                Ok(etl_stats) => {
                    stats.streams_succeeded += 1;
                    stats.total_rows_processed += etl_stats.rows_processed;
                    stats.total_rows_flagged += etl_stats.rows_with_dq_flags;

                    // Complete run record (dp-011) - graceful degradation
                    if let Some(id) = run_id {
                        if let Err(e) = self.persistence.complete_run(id, &etl_stats) {
                            warn!(
                                run_id = %id,
                                stream_id = %stream_id,
                                error = %e,
                                "Failed to complete run record"
                            );
                        }
                    }

                    // Update Prometheus metrics
                    if let Some(metrics) = EtlMetrics::get() {
                        metrics
                            .rows_processed
                            .with_label_values(&[stream_id])
                            .inc_by(etl_stats.rows_processed);
                        metrics
                            .rows_flagged
                            .with_label_values(&[stream_id])
                            .inc_by(etl_stats.rows_with_dq_flags);
                    }
                }
                Err(e) => {
                    stats.streams_failed += 1;
                    warn!(stream_id = %stream_id, error = %e, "Stream ETL failed");

                    // Fail run record (dp-011) - graceful degradation
                    if let Some(id) = run_id {
                        let context = serde_json::json!({
                            "stage": "etl_execution",
                            "stream_id": stream_id,
                        });
                        if let Err(persist_err) =
                            self.persistence.fail_run(id, &e.to_string(), Some(context))
                        {
                            warn!(
                                run_id = %id,
                                stream_id = %stream_id,
                                error = %persist_err,
                                "Failed to record run failure"
                            );
                        }
                    }
                }
            }
        }

        stats.cycle_duration_ms = start.elapsed().as_millis() as u64;

        debug!(
            cycle_id = %daemon_cycle_id,
            streams_processed = stats.streams_processed,
            streams_succeeded = stats.streams_succeeded,
            duration_ms = stats.cycle_duration_ms,
            "ETL cycle completed"
        );

        // Update daemon-level metrics
        if let Some(metrics) = EtlMetrics::get() {
            metrics.runs_total.inc();
            metrics
                .duration_seconds
                .observe(start.elapsed().as_secs_f64());
        }

        Ok(stats)
    }

    /// Wait for shutdown signal
    async fn wait_for_shutdown(&mut self) {
        while !*self.shutdown_rx.borrow() {
            if self.shutdown_rx.changed().await.is_err() {
                // Channel closed, treat as shutdown
                break;
            }
        }
    }
}

/// Real implementation of EtlExecutor that wraps EtlRunner
pub struct RealEtlExecutor {
    runner: EtlRunner,
    config_loader: ConfigLoader,
    bronze_dir: String,
}

impl RealEtlExecutor {
    pub fn new(runner: EtlRunner, config_loader: ConfigLoader, bronze_dir: String) -> Self {
        Self {
            runner,
            config_loader,
            bronze_dir,
        }
    }
}

impl EtlExecutor for RealEtlExecutor {
    fn run_stream(&self, stream_id: &str) -> Result<EtlStats, DaemonError> {
        // Load config synchronously (blocking call in async context)
        let config = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { self.config_loader.load_stream_config(stream_id).await })
        })
        .map_err(|e| DaemonError::Config(e.to_string()))?;

        self.runner
            .run_etl(&config, stream_id, &self.bronze_dir)
            .map_err(|e| DaemonError::Etl(e.to_string()))
    }

    fn list_enabled_streams(&self) -> Result<Vec<String>, DaemonError> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(async { self.config_loader.load_all_enabled().await })
        })
        .map_err(|e| DaemonError::Config(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::{MockEtlRunPersistence, PersistenceError};
    use mockall::predicate::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::time::timeout;

    /// Helper to create test EtlStats
    fn make_stats(rows_processed: u64, rows_with_dq_flags: u64, rows_rejected: u64) -> EtlStats {
        EtlStats {
            stream_id: "test-stream".to_string(),
            rows_processed,
            rows_with_dq_flags,
            rows_rejected,
            duration_ms: 100,
            watermark_before: None,
            watermark_after: None,
        }
    }

    // =========================================================================
    // London TDD Tests: Focus on behavior verification through mocks
    // =========================================================================

    #[tokio::test]
    async fn test_daemon_calls_etl_on_interval() {
        // Arrange: Create mock executor
        let mut mock_executor = MockEtlExecutor::new();

        // Expect list_enabled_streams to be called
        mock_executor
            .expect_list_enabled_streams()
            .times(1..)
            .returning(|| Ok(vec!["air-quality".to_string()]));

        // Expect run_stream to be called with "air-quality"
        mock_executor
            .expect_run_stream()
            .with(eq("air-quality"))
            .times(1..)
            .returning(|_| Ok(make_stats(100, 5, 0)));

        // Create shutdown channel
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        // Create daemon with short interval for testing
        let config = DaemonConfig {
            interval_secs: 1, // 1 second for fast test
            ..Default::default()
        };

        let mut daemon = DaemonRunner::new(mock_executor, config, shutdown_rx);

        // Act: Run daemon for 1.5 seconds then shutdown
        let daemon_handle = tokio::spawn(async move { daemon.run().await });

        // Wait a bit to allow at least one cycle, then shutdown
        tokio::time::sleep(Duration::from_millis(1500)).await;
        shutdown_tx.send(true).unwrap();

        // Assert: Daemon should exit gracefully
        let result = timeout(Duration::from_secs(2), daemon_handle).await;
        assert!(result.is_ok(), "Daemon should have stopped");
        assert!(
            result.unwrap().unwrap().is_ok(),
            "Daemon should exit without error"
        );
    }

    #[tokio::test]
    async fn test_daemon_processes_specific_stream_when_filtered() {
        // Arrange
        let mut mock_executor = MockEtlExecutor::new();

        // Should NOT call list_enabled_streams when stream filter is set
        mock_executor.expect_list_enabled_streams().times(0);

        // Should call run_stream with the specific stream
        mock_executor
            .expect_run_stream()
            .with(eq("outdoor-weather"))
            .times(1..)
            .returning(|_| Ok(make_stats(50, 2, 0)));

        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let config = DaemonConfig {
            interval_secs: 1,
            stream_filter: Some("outdoor-weather".to_string()),
            ..Default::default()
        };

        let mut daemon = DaemonRunner::new(mock_executor, config, shutdown_rx);

        // Act
        let daemon_handle = tokio::spawn(async move { daemon.run().await });

        tokio::time::sleep(Duration::from_millis(1500)).await;
        shutdown_tx.send(true).unwrap();

        // Assert
        let result = timeout(Duration::from_secs(2), daemon_handle).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_daemon_handles_graceful_shutdown() {
        // Arrange
        let mut mock_executor = MockEtlExecutor::new();

        mock_executor
            .expect_list_enabled_streams()
            .returning(|| Ok(vec!["test-stream".to_string()]));

        mock_executor
            .expect_run_stream()
            .returning(|_| Ok(make_stats(0, 0, 0)));

        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let config = DaemonConfig {
            interval_secs: 60, // Long interval
            ..Default::default()
        };

        let mut daemon = DaemonRunner::new(mock_executor, config, shutdown_rx);

        // Act: Start daemon and immediately request shutdown
        let daemon_handle = tokio::spawn(async move { daemon.run().await });

        // Small delay to let daemon start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Send shutdown
        shutdown_tx.send(true).unwrap();

        // Assert: Should stop quickly
        let result = timeout(Duration::from_secs(1), daemon_handle).await;
        assert!(
            result.is_ok(),
            "Daemon should stop within 1 second after shutdown signal"
        );
    }

    #[tokio::test]
    async fn test_daemon_continues_after_stream_failure() {
        // Arrange
        let mut mock_executor = MockEtlExecutor::new();
        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();

        mock_executor
            .expect_list_enabled_streams()
            .returning(|| Ok(vec!["stream-a".to_string(), "stream-b".to_string()]));

        // First stream fails, second succeeds
        mock_executor
            .expect_run_stream()
            .returning(move |stream_id| {
                call_count_clone.fetch_add(1, Ordering::SeqCst);
                if stream_id == "stream-a" {
                    Err(DaemonError::Etl("Simulated failure".to_string()))
                } else {
                    Ok(EtlStats {
                        stream_id: "stream-b".to_string(),
                        rows_processed: 10,
                        rows_with_dq_flags: 0,
                        rows_rejected: 0,
                        duration_ms: 100,
                        watermark_before: None,
                        watermark_after: None,
                    })
                }
            });

        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        let config = DaemonConfig {
            interval_secs: 1,
            ..Default::default()
        };

        let mut daemon = DaemonRunner::new(mock_executor, config, shutdown_rx);

        // Act
        let daemon_handle = tokio::spawn(async move { daemon.run().await });

        tokio::time::sleep(Duration::from_millis(1500)).await;
        shutdown_tx.send(true).unwrap();

        let result = timeout(Duration::from_secs(2), daemon_handle).await;

        // Assert: Both streams should have been attempted
        assert!(result.is_ok());
        assert!(
            call_count.load(Ordering::SeqCst) >= 2,
            "Both streams should be processed even if one fails"
        );
    }

    #[tokio::test]
    async fn test_daemon_cycle_returns_correct_stats() {
        // Arrange
        let mut mock_executor = MockEtlExecutor::new();

        mock_executor
            .expect_list_enabled_streams()
            .returning(|| Ok(vec!["stream-1".to_string(), "stream-2".to_string()]));

        mock_executor
            .expect_run_stream()
            .with(eq("stream-1"))
            .returning(|_| Ok(make_stats(100, 5, 1)));

        mock_executor
            .expect_run_stream()
            .with(eq("stream-2"))
            .returning(|_| Ok(make_stats(200, 10, 2)));

        let (_shutdown_tx, shutdown_rx) = watch::channel(false);

        let config = DaemonConfig::default();
        let daemon = DaemonRunner::new(mock_executor, config, shutdown_rx);

        // Act
        let stats = daemon.run_cycle().unwrap();

        // Assert
        assert_eq!(stats.streams_processed, 2);
        assert_eq!(stats.streams_succeeded, 2);
        assert_eq!(stats.streams_failed, 0);
        assert_eq!(stats.total_rows_processed, 300);
        assert_eq!(stats.total_rows_flagged, 15);
    }

    #[test]
    fn test_daemon_config_default_values() {
        let config = DaemonConfig::default();

        assert_eq!(config.interval_secs, 300);
        assert!(config.stream_filter.is_none());
        assert_eq!(config.max_consecutive_failures, 3);
        assert_eq!(config.backoff_multiplier, 2.0);
        assert_eq!(config.run_mode, EtlRunMode::Daemon);
    }

    // =========================================================================
    // dp-011: Persistence Integration Tests (London TDD)
    // =========================================================================

    #[test]
    fn test_daemon_with_persistence_calls_start_run() {
        // Arrange
        let mut mock_executor = MockEtlExecutor::new();
        let mut mock_persistence = MockEtlRunPersistence::new();

        mock_executor
            .expect_list_enabled_streams()
            .returning(|| Ok(vec!["air-quality".to_string()]));

        mock_executor
            .expect_run_stream()
            .returning(|_| Ok(make_stats(100, 5, 0)));

        // Verify start_run is called with correct stream_id and mode
        mock_persistence
            .expect_start_run()
            .with(
                eq("air-quality"),
                eq(EtlRunMode::Daemon),
                function(|opt: &Option<Uuid>| opt.is_some()), // cycle_id present
            )
            .times(1)
            .returning(|_, _, _| Ok(Uuid::new_v4()));

        // Verify complete_run is called on success
        mock_persistence
            .expect_complete_run()
            .times(1)
            .returning(|_, _| Ok(()));

        let (_shutdown_tx, shutdown_rx) = watch::channel(false);
        let config = DaemonConfig::default();
        let daemon = DaemonRunner::with_persistence(mock_executor, config, shutdown_rx, mock_persistence);

        // Act
        let stats = daemon.run_cycle().unwrap();

        // Assert
        assert_eq!(stats.streams_succeeded, 1);
    }

    #[test]
    fn test_daemon_with_persistence_calls_fail_run_on_error() {
        // Arrange
        let mut mock_executor = MockEtlExecutor::new();
        let mut mock_persistence = MockEtlRunPersistence::new();

        mock_executor
            .expect_list_enabled_streams()
            .returning(|| Ok(vec!["air-quality".to_string()]));

        mock_executor
            .expect_run_stream()
            .returning(|_| Err(DaemonError::Etl("Connection refused".to_string())));

        mock_persistence
            .expect_start_run()
            .returning(|_, _, _| Ok(Uuid::new_v4()));

        // Verify fail_run is called with error message
        mock_persistence
            .expect_fail_run()
            .with(
                always(),
                function(|msg: &str| msg.contains("Connection refused")),
                function(|ctx: &Option<serde_json::Value>| ctx.is_some()),
            )
            .times(1)
            .returning(|_, _, _| Ok(()));

        let (_shutdown_tx, shutdown_rx) = watch::channel(false);
        let config = DaemonConfig::default();
        let daemon = DaemonRunner::with_persistence(mock_executor, config, shutdown_rx, mock_persistence);

        // Act
        let stats = daemon.run_cycle().unwrap();

        // Assert: ETL failed but cycle succeeded
        assert_eq!(stats.streams_failed, 1);
    }

    #[test]
    fn test_daemon_continues_when_persistence_start_fails() {
        // Arrange: CRITICAL - persistence failure must NOT fail ETL
        let mut mock_executor = MockEtlExecutor::new();
        let mut mock_persistence = MockEtlRunPersistence::new();

        mock_executor
            .expect_list_enabled_streams()
            .returning(|| Ok(vec!["air-quality".to_string()]));

        // ETL should succeed even if persistence fails
        mock_executor
            .expect_run_stream()
            .returning(|_| Ok(make_stats(100, 0, 0)));

        // start_run fails (database unavailable)
        mock_persistence
            .expect_start_run()
            .returning(|_, _, _| Err(PersistenceError::Connection("Database unavailable".into())));

        // complete_run should NOT be called (no run_id)
        mock_persistence.expect_complete_run().times(0);

        let (_shutdown_tx, shutdown_rx) = watch::channel(false);
        let config = DaemonConfig::default();
        let daemon = DaemonRunner::with_persistence(mock_executor, config, shutdown_rx, mock_persistence);

        // Act
        let result = daemon.run_cycle();

        // Assert: ETL cycle succeeds despite persistence failure
        assert!(result.is_ok(), "Cycle should succeed when persistence fails");
        let stats = result.unwrap();
        assert_eq!(stats.streams_succeeded, 1);
        assert_eq!(stats.total_rows_processed, 100);
    }

    #[test]
    fn test_daemon_continues_when_persistence_complete_fails() {
        // Arrange
        let mut mock_executor = MockEtlExecutor::new();
        let mut mock_persistence = MockEtlRunPersistence::new();

        mock_executor
            .expect_list_enabled_streams()
            .returning(|| Ok(vec!["air-quality".to_string()]));

        mock_executor
            .expect_run_stream()
            .returning(|_| Ok(make_stats(100, 0, 0)));

        mock_persistence
            .expect_start_run()
            .returning(|_, _, _| Ok(Uuid::new_v4()));

        // complete_run fails
        mock_persistence
            .expect_complete_run()
            .returning(|_, _| Err(PersistenceError::SqlExecution("Disk full".into())));

        let (_shutdown_tx, shutdown_rx) = watch::channel(false);
        let config = DaemonConfig::default();
        let daemon = DaemonRunner::with_persistence(mock_executor, config, shutdown_rx, mock_persistence);

        // Act
        let result = daemon.run_cycle();

        // Assert: ETL cycle succeeds despite persistence failure
        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.streams_succeeded, 1);
    }

    #[test]
    fn test_daemon_shares_cycle_id_across_streams() {
        // Arrange
        let mut mock_executor = MockEtlExecutor::new();
        let mut mock_persistence = MockEtlRunPersistence::new();
        let captured_cycle_ids = Arc::new(std::sync::Mutex::new(Vec::<Uuid>::new()));
        let capture_clone = captured_cycle_ids.clone();

        mock_executor
            .expect_list_enabled_streams()
            .returning(|| Ok(vec!["stream-a".to_string(), "stream-b".to_string()]));

        mock_executor
            .expect_run_stream()
            .returning(|_| Ok(make_stats(50, 0, 0)));

        // Capture cycle_ids from both start_run calls
        mock_persistence
            .expect_start_run()
            .times(2)
            .returning(move |_, _, cycle_id| {
                if let Some(id) = cycle_id {
                    capture_clone.lock().unwrap().push(id);
                }
                Ok(Uuid::new_v4())
            });

        mock_persistence
            .expect_complete_run()
            .times(2)
            .returning(|_, _| Ok(()));

        let (_shutdown_tx, shutdown_rx) = watch::channel(false);
        let config = DaemonConfig::default();
        let daemon = DaemonRunner::with_persistence(mock_executor, config, shutdown_rx, mock_persistence);

        // Act
        daemon.run_cycle().unwrap();

        // Assert: Both streams share the same cycle_id
        let cycle_ids = captured_cycle_ids.lock().unwrap();
        assert_eq!(cycle_ids.len(), 2);
        assert_eq!(cycle_ids[0], cycle_ids[1], "All streams in cycle should share the same cycle_id");
    }

    #[test]
    fn test_daemon_uses_configured_run_mode() {
        // Arrange
        let mut mock_executor = MockEtlExecutor::new();
        let mut mock_persistence = MockEtlRunPersistence::new();

        mock_executor
            .expect_list_enabled_streams()
            .returning(|| Ok(vec!["air-quality".to_string()]));

        mock_executor
            .expect_run_stream()
            .returning(|_| Ok(make_stats(100, 0, 0)));

        // Verify Manual mode is passed to persistence
        mock_persistence
            .expect_start_run()
            .with(always(), eq(EtlRunMode::Manual), always())
            .times(1)
            .returning(|_, _, _| Ok(Uuid::new_v4()));

        mock_persistence
            .expect_complete_run()
            .returning(|_, _| Ok(()));

        let (_shutdown_tx, shutdown_rx) = watch::channel(false);
        let config = DaemonConfig {
            run_mode: EtlRunMode::Manual,
            ..Default::default()
        };
        let daemon = DaemonRunner::with_persistence(mock_executor, config, shutdown_rx, mock_persistence);

        // Act & Assert (mock expectations verify behavior)
        daemon.run_cycle().unwrap();
    }
}
