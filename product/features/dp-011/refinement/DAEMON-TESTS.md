# DP-011: Daemon Integration Tests Specification

**Feature ID**: dp-011
**Phase**: Refinement (SPARC R)
**Created**: 2026-01-16
**Test Type**: Unit (Mock-Based, Daemon Layer)

---

## Overview

Tests for daemon-level integration of ETL run persistence. These tests verify that the DaemonRunner correctly coordinates with both `EtlExecutor` and `EtlRunPersistence` using mocks for both dependencies.

---

## Test File Location

```
apps/silver-etl/src/daemon.rs

#[cfg(test)]
mod tests {
    // Existing tests for MockEtlExecutor
    // NEW: Tests for MockEtlRunPersistence integration
}
```

---

## Updated DaemonRunner (Conceptual)

```rust
/// Daemon runner with persistence support
pub struct DaemonRunner<E: EtlExecutor, P: EtlRunPersistence> {
    executor: Mutex<E>,
    persistence: P,
    config: DaemonConfig,
    shutdown_rx: watch::Receiver<bool>,
}

impl<E: EtlExecutor + 'static, P: EtlRunPersistence + 'static> DaemonRunner<E, P> {
    pub fn new(
        executor: E,
        persistence: P,
        config: DaemonConfig,
        shutdown_rx: watch::Receiver<bool>,
    ) -> Self {
        Self {
            executor: Mutex::new(executor),
            persistence,
            config,
            shutdown_rx,
        }
    }

    /// Execute a single ETL cycle with persistence
    fn run_cycle(&self) -> Result<DaemonCycleStats, DaemonError> {
        let cycle_id = Uuid::new_v4();  // Shared across all stream runs
        let start = std::time::Instant::now();
        let mut stats = DaemonCycleStats::default();

        let streams = self.get_streams()?;
        stats.streams_processed = streams.len();

        for stream_id in &streams {
            self.run_stream_with_persistence(stream_id, cycle_id, &mut stats);
        }

        stats.cycle_duration_ms = start.elapsed().as_millis() as u64;
        Ok(stats)
    }

    fn run_stream_with_persistence(
        &self,
        stream_id: &str,
        cycle_id: Uuid,
        stats: &mut DaemonCycleStats,
    ) {
        // 1. Start run record (may fail gracefully)
        let run_id = match self.persistence.start_run(
            stream_id,
            EtlRunMode::Daemon,
            Some(cycle_id),
        ) {
            Ok(id) => Some(id),
            Err(e) => {
                warn!(%e, %stream_id, "Failed to start run record");
                None
            }
        };

        // 2. Execute ETL
        let executor = self.executor.lock().unwrap();
        match executor.run_stream(stream_id) {
            Ok(etl_stats) => {
                stats.streams_succeeded += 1;
                stats.total_rows_processed += etl_stats.rows_processed;

                // 3a. Complete run record
                if let Some(id) = run_id {
                    if let Err(e) = self.persistence.complete_run(id, &etl_stats) {
                        warn!(%e, %stream_id, "Failed to complete run record");
                    }
                }
            }
            Err(e) => {
                stats.streams_failed += 1;

                // 3b. Fail run record
                if let Some(id) = run_id {
                    let context = serde_json::json!({
                        "error_type": "etl_execution",
                        "stream_id": stream_id,
                    });
                    if let Err(pe) = self.persistence.fail_run(id, &e.to_string(), Some(context)) {
                        warn!(%pe, %stream_id, "Failed to record run failure");
                    }
                }
            }
        }
    }
}
```

---

## Test Specifications

### Test 1: Daemon Persists Each Stream Run

```rust
/// Test: daemon run_cycle calls persistence for each stream
///
/// Verifies:
/// - start_run called once per enabled stream
/// - complete_run called once per successful stream
/// - Persistence operations happen for ALL streams
#[tokio::test]
async fn test_daemon_persists_each_stream_run() {
    // Arrange
    let mut mock_executor = MockEtlExecutor::new();
    let mut mock_persistence = MockEtlRunPersistence::new();

    // Two streams enabled
    mock_executor.expect_list_enabled_streams()
        .returning(|| Ok(vec!["stream-a".to_string(), "stream-b".to_string()]));

    // Both ETLs succeed
    mock_executor.expect_run_stream()
        .with(eq("stream-a"))
        .returning(|_| Ok(make_stats(100, 5, 0)));
    mock_executor.expect_run_stream()
        .with(eq("stream-b"))
        .returning(|_| Ok(make_stats(200, 10, 0)));

    // Expect start_run called twice (once per stream)
    mock_persistence.expect_start_run()
        .times(2)
        .returning(|_, _, _| Ok(Uuid::new_v4()));

    // Expect complete_run called twice (both succeed)
    mock_persistence.expect_complete_run()
        .times(2)
        .returning(|_, _| Ok(()));

    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    let config = DaemonConfig::default();
    let daemon = DaemonRunner::new(mock_executor, mock_persistence, config, shutdown_rx);

    // Act
    let result = daemon.run_cycle();

    // Assert
    assert!(result.is_ok());
    let stats = result.unwrap();
    assert_eq!(stats.streams_processed, 2);
    assert_eq!(stats.streams_succeeded, 2);
    assert_eq!(stats.streams_failed, 0);
}
```

### Test 2: Failed Stream Calls fail_run

```rust
/// Test: when ETL fails, fail_run is called with error details
///
/// Verifies:
/// - fail_run called instead of complete_run on ETL failure
/// - Error message passed to persistence
/// - Other streams still processed
#[tokio::test]
async fn test_failed_stream_persists_error() {
    // Arrange
    let mut mock_executor = MockEtlExecutor::new();
    let mut mock_persistence = MockEtlRunPersistence::new();

    mock_executor.expect_list_enabled_streams()
        .returning(|| Ok(vec!["stream-a".to_string(), "stream-b".to_string()]));

    // stream-a fails, stream-b succeeds
    mock_executor.expect_run_stream()
        .with(eq("stream-a"))
        .returning(|_| Err(DaemonError::Etl("Transform failed: column not found".into())));
    mock_executor.expect_run_stream()
        .with(eq("stream-b"))
        .returning(|_| Ok(make_stats(100, 0, 0)));

    // Both streams start
    let run_id_a = Uuid::new_v4();
    let run_id_b = Uuid::new_v4();

    mock_persistence.expect_start_run()
        .with(eq("stream-a"), always(), always())
        .times(1)
        .returning(move |_, _, _| Ok(run_id_a));
    mock_persistence.expect_start_run()
        .with(eq("stream-b"), always(), always())
        .times(1)
        .returning(move |_, _, _| Ok(run_id_b));

    // stream-a calls fail_run
    mock_persistence.expect_fail_run()
        .withf(|id, error, context| {
            error.contains("column not found") &&
            context.is_some()
        })
        .times(1)
        .returning(|_, _, _| Ok(()));

    // stream-b calls complete_run
    mock_persistence.expect_complete_run()
        .times(1)
        .returning(|_, _| Ok(()));

    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    let config = DaemonConfig::default();
    let daemon = DaemonRunner::new(mock_executor, mock_persistence, config, shutdown_rx);

    // Act
    let result = daemon.run_cycle();

    // Assert
    assert!(result.is_ok());
    let stats = result.unwrap();
    assert_eq!(stats.streams_succeeded, 1);
    assert_eq!(stats.streams_failed, 1);
}
```

### Test 3: daemon_cycle_id Links All Runs

```rust
/// Test: all runs in a cycle share the same daemon_cycle_id
///
/// Verifies:
/// - UUID generated once per cycle
/// - Same UUID passed to all start_run calls
#[tokio::test]
async fn test_daemon_cycle_id_shared() {
    // Arrange
    let mut mock_executor = MockEtlExecutor::new();
    let mut mock_persistence = MockEtlRunPersistence::new();
    let captured_cycle_ids = Arc::new(Mutex::new(Vec::new()));
    let captured_clone = captured_cycle_ids.clone();

    mock_executor.expect_list_enabled_streams()
        .returning(|| Ok(vec!["stream-a".to_string(), "stream-b".to_string(), "stream-c".to_string()]));

    mock_executor.expect_run_stream()
        .returning(|_| Ok(make_stats(100, 0, 0)));

    // Capture cycle_ids from all start_run calls
    mock_persistence.expect_start_run()
        .times(3)
        .returning(move |_, _, cycle_id| {
            if let Some(id) = cycle_id {
                captured_clone.lock().unwrap().push(id);
            }
            Ok(Uuid::new_v4())
        });

    mock_persistence.expect_complete_run()
        .returning(|_, _| Ok(()));

    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    let config = DaemonConfig::default();
    let daemon = DaemonRunner::new(mock_executor, mock_persistence, config, shutdown_rx);

    // Act
    let _ = daemon.run_cycle();

    // Assert: All cycle_ids should be the same
    let cycle_ids = captured_cycle_ids.lock().unwrap();
    assert_eq!(cycle_ids.len(), 3);
    assert_eq!(cycle_ids[0], cycle_ids[1]);
    assert_eq!(cycle_ids[1], cycle_ids[2]);
}
```

### Test 4: Persistence Failure Does Not Fail ETL

```rust
/// Test: ETL continues even when persistence fails
///
/// Verifies:
/// - Persistence errors are logged but not propagated
/// - ETL execution proceeds regardless
/// - Cycle stats reflect ETL success/failure, not persistence
#[tokio::test]
async fn test_persistence_failure_continues_etl() {
    // Arrange
    let mut mock_executor = MockEtlExecutor::new();
    let mut mock_persistence = MockEtlRunPersistence::new();

    mock_executor.expect_list_enabled_streams()
        .returning(|| Ok(vec!["air-quality".to_string()]));

    // ETL succeeds
    mock_executor.expect_run_stream()
        .times(1)
        .returning(|_| Ok(make_stats(500, 25, 3)));

    // Persistence always fails
    mock_persistence.expect_start_run()
        .times(1)
        .returning(|_, _, _| Err(PersistenceError::Database("Connection refused".into())));

    // complete_run should NOT be called (no run_id to complete)
    mock_persistence.expect_complete_run()
        .times(0);

    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    let config = DaemonConfig::default();
    let daemon = DaemonRunner::new(mock_executor, mock_persistence, config, shutdown_rx);

    // Act
    let result = daemon.run_cycle();

    // Assert: ETL cycle succeeds despite persistence failure
    assert!(result.is_ok());
    let stats = result.unwrap();
    assert_eq!(stats.streams_succeeded, 1);
    assert_eq!(stats.total_rows_processed, 500);
}
```

### Test 5: complete_run Failure Does Not Fail Cycle

```rust
/// Test: complete_run failure is logged but doesn't fail the cycle
///
/// Verifies:
/// - ETL success counted even if complete_run fails
/// - Stats reflect ETL outcome, not persistence outcome
#[tokio::test]
async fn test_complete_run_failure_logged() {
    // Arrange
    let mut mock_executor = MockEtlExecutor::new();
    let mut mock_persistence = MockEtlRunPersistence::new();

    mock_executor.expect_list_enabled_streams()
        .returning(|| Ok(vec!["air-quality".to_string()]));

    mock_executor.expect_run_stream()
        .returning(|_| Ok(make_stats(100, 0, 0)));

    // start_run succeeds
    mock_persistence.expect_start_run()
        .returning(|_, _, _| Ok(Uuid::new_v4()));

    // complete_run fails
    mock_persistence.expect_complete_run()
        .returning(|_, _| Err(PersistenceError::Database("Timeout".into())));

    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    let config = DaemonConfig::default();
    let daemon = DaemonRunner::new(mock_executor, mock_persistence, config, shutdown_rx);

    // Act
    let result = daemon.run_cycle();

    // Assert: Cycle succeeds, ETL counted as successful
    assert!(result.is_ok());
    let stats = result.unwrap();
    assert_eq!(stats.streams_succeeded, 1);
}
```

### Test 6: Run Mode is Daemon

```rust
/// Test: daemon passes EtlRunMode::Daemon to persistence
///
/// Verifies:
/// - run_mode is always 'daemon' in daemon context
#[tokio::test]
async fn test_run_mode_is_daemon() {
    // Arrange
    let mut mock_executor = MockEtlExecutor::new();
    let mut mock_persistence = MockEtlRunPersistence::new();

    mock_executor.expect_list_enabled_streams()
        .returning(|| Ok(vec!["test-stream".to_string()]));

    mock_executor.expect_run_stream()
        .returning(|_| Ok(make_stats(100, 0, 0)));

    // Verify run_mode is Daemon
    mock_persistence.expect_start_run()
        .withf(|_, mode, _| *mode == EtlRunMode::Daemon)
        .times(1)
        .returning(|_, _, _| Ok(Uuid::new_v4()));

    mock_persistence.expect_complete_run()
        .returning(|_, _| Ok(()));

    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    let config = DaemonConfig::default();
    let daemon = DaemonRunner::new(mock_executor, mock_persistence, config, shutdown_rx);

    // Act
    let _ = daemon.run_cycle();

    // Assert: Handled by mock expectation (withf predicate)
}
```

### Test 7: Stream Filter Respects Persistence

```rust
/// Test: stream_filter limits which streams get persistence calls
///
/// Verifies:
/// - Only filtered stream gets start_run/complete_run
/// - Other streams not touched
#[tokio::test]
async fn test_stream_filter_with_persistence() {
    // Arrange
    let mut mock_executor = MockEtlExecutor::new();
    let mut mock_persistence = MockEtlRunPersistence::new();

    // list_enabled_streams should NOT be called when filter is set
    mock_executor.expect_list_enabled_streams()
        .times(0);

    // Only outdoor-weather should be run
    mock_executor.expect_run_stream()
        .with(eq("outdoor-weather"))
        .times(1)
        .returning(|_| Ok(make_stats(50, 2, 0)));

    // Only one start_run for outdoor-weather
    mock_persistence.expect_start_run()
        .with(eq("outdoor-weather"), always(), always())
        .times(1)
        .returning(|_, _, _| Ok(Uuid::new_v4()));

    mock_persistence.expect_complete_run()
        .times(1)
        .returning(|_, _| Ok(()));

    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    let config = DaemonConfig {
        stream_filter: Some("outdoor-weather".to_string()),
        ..Default::default()
    };
    let daemon = DaemonRunner::new(mock_executor, mock_persistence, config, shutdown_rx);

    // Act
    let result = daemon.run_cycle();

    // Assert
    assert!(result.is_ok());
    assert_eq!(result.unwrap().streams_processed, 1);
}
```

### Test 8: Error Context Includes Stream ID

```rust
/// Test: fail_run error context includes useful debugging info
///
/// Verifies:
/// - error_context JSON includes stream_id
/// - error_context JSON includes error_type
#[tokio::test]
async fn test_error_context_structure() {
    // Arrange
    let mut mock_executor = MockEtlExecutor::new();
    let mut mock_persistence = MockEtlRunPersistence::new();

    mock_executor.expect_list_enabled_streams()
        .returning(|| Ok(vec!["failing-stream".to_string()]));

    mock_executor.expect_run_stream()
        .returning(|_| Err(DaemonError::Etl("Parse error".into())));

    mock_persistence.expect_start_run()
        .returning(|_, _, _| Ok(Uuid::new_v4()));

    // Verify error context structure
    mock_persistence.expect_fail_run()
        .withf(|_, _, context| {
            if let Some(ctx) = context {
                ctx.get("stream_id").is_some() &&
                ctx.get("error_type").is_some()
            } else {
                false
            }
        })
        .times(1)
        .returning(|_, _, _| Ok(()));

    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    let config = DaemonConfig::default();
    let daemon = DaemonRunner::new(mock_executor, mock_persistence, config, shutdown_rx);

    // Act
    let _ = daemon.run_cycle();

    // Assert: Handled by mock expectation
}
```

---

## Test Helper Functions

```rust
use std::sync::{Arc, Mutex};
use mockall::predicate::*;

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

/// Helper to create a mock persistence that always succeeds
fn success_persistence() -> MockEtlRunPersistence {
    let mut mock = MockEtlRunPersistence::new();
    mock.expect_start_run()
        .returning(|_, _, _| Ok(Uuid::new_v4()));
    mock.expect_complete_run()
        .returning(|_, _| Ok(()));
    mock.expect_fail_run()
        .returning(|_, _, _| Ok(()));
    mock
}
```

---

## Test Execution

```bash
# Run all daemon tests
cargo test --package silver-etl daemon::tests

# Run persistence-specific daemon tests
cargo test --package silver-etl daemon::tests::test_daemon_persist

# Run with output
cargo test --package silver-etl daemon::tests -- --nocapture
```

---

## Coverage Summary

| Test | Behavior Verified |
|------|-------------------|
| `test_daemon_persists_each_stream_run` | Basic persistence flow |
| `test_failed_stream_persists_error` | Error path handling |
| `test_daemon_cycle_id_shared` | Cycle linking |
| `test_persistence_failure_continues_etl` | Graceful degradation |
| `test_complete_run_failure_logged` | Non-blocking persistence |
| `test_run_mode_is_daemon` | Mode correctness |
| `test_stream_filter_with_persistence` | Filter interaction |
| `test_error_context_structure` | Error detail capture |

**Total: 8 daemon integration tests**

---

*Daemon tests specification created: 2026-01-16*
