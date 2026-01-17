# Daemon Integration Pseudocode

**Feature**: dp-011 - Silver ETL Run Statistics Persistence
**Component**: Daemon Runner Integration
**Author**: ndp-rust-dev
**Created**: 2026-01-16

---

## Overview

This document defines how the daemon integrates with the persistence layer. The daemon's `run_cycle` method is modified to track ETL runs with `start_run` -> ETL execution -> `complete_run`/`fail_run`.

---

## Current State Analysis

### Existing DaemonRunner Structure

```
# From apps/silver-etl/src/daemon.rs

STRUCT DaemonRunner<E: EtlExecutor>:
    executor: Mutex<E>           # ETL executor (mockable)
    config: DaemonConfig         # Interval, backoff settings
    shutdown_rx: watch::Receiver # Graceful shutdown signal

TRAIT EtlExecutor (Send):
    fn run_stream(stream_id) -> Result<EtlStats, DaemonError>
    fn list_enabled_streams() -> Result<Vec<String>, DaemonError>
```

### Current run_cycle Flow

```
FUNCTION run_cycle() -> Result<DaemonCycleStats, DaemonError>:
    1. Check shutdown signal
    2. Lock executor
    3. Get enabled streams (list or filter)
    4. FOR each stream:
       a. Check shutdown
       b. executor.run_stream(stream_id)
       c. Accumulate stats / handle errors
    5. Update Prometheus metrics
    6. Return DaemonCycleStats
```

---

## Modified Design

### Option A: Persistence in EtlExecutor (Tight Coupling)

```
# EtlExecutor gains persistence responsibility

TRAIT EtlExecutor (Send):
    fn run_stream(stream_id, run_mode, cycle_id) -> Result<EtlStats, DaemonError>
    fn list_enabled_streams() -> Result<Vec<String>, DaemonError>

# RealEtlExecutor handles persistence internally
```

**Pros**: Simple, single responsibility for executor
**Cons**: Can't mock persistence separately, harder to test

### Option B: Wrapper Pattern (Loose Coupling) - PREFERRED

```
# Persistence handled at daemon level, wrapping executor

STRUCT DaemonRunner<E: EtlExecutor, P: EtlRunPersistence>:
    executor: Mutex<E>
    persistence: P              # NEW: Persistence layer
    config: DaemonConfig
    shutdown_rx: watch::Receiver
```

**Pros**: Testable, separation of concerns, mockable persistence
**Cons**: Slightly more complex

---

## Modified DaemonRunner (Option B)

### Updated Structure

```
STRUCT DaemonRunner<E, P>
WHERE
    E: EtlExecutor + 'static,
    P: EtlRunPersistence + 'static,
{
    executor: Mutex<E>,
    persistence: P,
    config: DaemonConfig,
    shutdown_rx: watch::Receiver<bool>,
}
```

### Updated Constructor

```
FUNCTION new(
    executor: E,
    persistence: P,
    config: DaemonConfig,
    shutdown_rx: watch::Receiver<bool>
) -> Self:
    RETURN Self {
        executor: Mutex::new(executor),
        persistence,
        config,
        shutdown_rx,
    }
```

### Modified run_cycle

```
FUNCTION run_cycle() -> Result<DaemonCycleStats, DaemonError>:

    # 1. Initialize cycle tracking
    start = Instant::now()
    stats = DaemonCycleStats::default()
    cycle_id = UUID::new_v4()

    debug!(cycle_id = %cycle_id, "Starting daemon cycle")

    # 2. Check for shutdown before starting
    IF *self.shutdown_rx.borrow():
        RETURN Error(DaemonError::Shutdown)

    # 3. Lock executor for this cycle
    executor = self.executor.lock()
        .map_err(|e| DaemonError::Etl(format!("Mutex poisoned: {}", e)))?

    # 4. Get list of streams to process
    streams = MATCH &self.config.stream_filter:
        Some(stream_id) => vec![stream_id.clone()]
        None => executor.list_enabled_streams()?

    stats.streams_processed = streams.len()

    # 5. Process each stream WITH persistence tracking
    FOR stream_id IN &streams:

        # 5a. Check for shutdown between streams
        IF *self.shutdown_rx.borrow():
            RETURN Error(DaemonError::Shutdown)

        # 5b. Start run record in database
        run_id = MATCH self.persistence.start_run(
            stream_id,
            EtlRunMode::Daemon,
            Some(cycle_id)
        ):
            Ok(id) => id
            Err(e):
                # Log but continue - don't fail cycle due to persistence
                warn!(
                    stream_id = %stream_id,
                    error = %e,
                    "Failed to start run record, continuing without persistence"
                )
                None  # Track without ID

        # 5c. Execute ETL
        MATCH executor.run_stream(stream_id):

            Ok(etl_stats):
                # Success path
                stats.streams_succeeded += 1
                stats.total_rows_processed += etl_stats.rows_processed
                stats.total_rows_flagged += etl_stats.rows_with_dq_flags

                # Complete run record
                IF let Some(id) = run_id:
                    IF let Err(e) = self.persistence.complete_run(id, &etl_stats):
                        warn!(
                            run_id = %id,
                            error = %e,
                            "Failed to complete run record"
                        )

                # Update Prometheus metrics
                IF let Some(metrics) = EtlMetrics::get():
                    metrics.rows_processed
                        .with_label_values(&[stream_id])
                        .inc_by(etl_stats.rows_processed)
                    metrics.rows_flagged
                        .with_label_values(&[stream_id])
                        .inc_by(etl_stats.rows_with_dq_flags)

            Err(e):
                # Failure path
                stats.streams_failed += 1

                # Build error context
                error_context = json!({
                    "cycle_id": cycle_id.to_string(),
                    "stream_index": stats.streams_processed,
                    "error_type": format!("{:?}", e)
                })

                # Fail run record
                IF let Some(id) = run_id:
                    IF let Err(pe) = self.persistence.fail_run(
                        id,
                        &e.to_string(),
                        Some(error_context)
                    ):
                        warn!(
                            run_id = %id,
                            error = %pe,
                            "Failed to record run failure"
                        )

                warn!(stream_id = %stream_id, error = %e, "Stream ETL failed")

    # 6. Calculate cycle duration
    stats.cycle_duration_ms = start.elapsed().as_millis() as u64

    # 7. Update daemon-level metrics
    IF let Some(metrics) = EtlMetrics::get():
        metrics.runs_total.inc()
        metrics.duration_seconds.observe(start.elapsed().as_secs_f64())

    # 8. Log cycle completion
    info!(
        cycle_id = %cycle_id,
        streams_processed = stats.streams_processed,
        streams_succeeded = stats.streams_succeeded,
        streams_failed = stats.streams_failed,
        total_rows = stats.total_rows_processed,
        duration_ms = stats.cycle_duration_ms,
        "ETL cycle completed"
    )

    RETURN Ok(stats)
```

---

## RealEtlExecutor Updates

### Current RealEtlExecutor

```
STRUCT RealEtlExecutor:
    runner: EtlRunner
    config_loader: ConfigLoader
    bronze_dir: String
```

### No Changes Required

The executor remains focused on ETL execution. Persistence is handled at the daemon level, maintaining separation of concerns.

---

## Factory Function for Production

```
FUNCTION create_daemon_runner(
    pg_conn_str: &str,
    etcd_endpoints: Vec<String>,
    bronze_dir: String,
    config: DaemonConfig
) -> Result<DaemonRunner<RealEtlExecutor, DuckDbRunPersistence>, Error>:

    # 1. Create ETL runner with PostgreSQL connection
    runner = EtlRunner::with_postgres(pg_conn_str)?

    # 2. Create config loader
    config_loader = ConfigLoader::new(etcd_endpoints).await?

    # 3. Create real executor
    executor = RealEtlExecutor::new(runner, config_loader, bronze_dir)

    # 4. Create persistence layer (separate connection)
    persistence_conn = create_persistence_connection(pg_conn_str)?
    persistence = DuckDbRunPersistence::new(&persistence_conn)?

    # 5. Create shutdown channel
    (shutdown_tx, shutdown_rx) = watch::channel(false)

    # 6. Return configured daemon
    RETURN Ok(DaemonRunner::new(executor, persistence, config, shutdown_rx))
```

---

## Testing Strategy

### Mock Setup for Tests

```
#[tokio::test]
async fn test_daemon_persists_run_on_success():

    # Arrange
    mut mock_executor = MockEtlExecutor::new()
    mut mock_persistence = MockEtlRunPersistence::new()

    # Expect list_enabled_streams
    mock_executor.expect_list_enabled_streams()
        .returning(|| Ok(vec!["test-stream".to_string()]))

    # Expect run_stream to succeed
    mock_executor.expect_run_stream()
        .with(eq("test-stream"))
        .returning(|_| Ok(make_test_stats(100, 5, 0)))

    # Expect start_run to be called
    mock_persistence.expect_start_run()
        .with(eq("test-stream"), eq(EtlRunMode::Daemon), always())
        .returning(|_, _, _| Ok(Uuid::new_v4()))

    # Expect complete_run to be called with stats
    mock_persistence.expect_complete_run()
        .withf(|_, stats| stats.rows_processed == 100)
        .returning(|_, _| Ok(()))

    # Expect fail_run NOT to be called
    mock_persistence.expect_fail_run().times(0)

    # Create daemon
    (_, shutdown_rx) = watch::channel(false)
    daemon = DaemonRunner::new(
        mock_executor,
        mock_persistence,
        DaemonConfig::default(),
        shutdown_rx
    )

    # Act
    result = daemon.run_cycle()

    # Assert
    assert!(result.is_ok())
    stats = result.unwrap()
    assert_eq!(stats.streams_succeeded, 1)
```

### Test: Persistence Failure Doesn't Break Cycle

```
#[tokio::test]
async fn test_daemon_continues_when_persistence_fails():

    # Arrange
    mut mock_executor = MockEtlExecutor::new()
    mut mock_persistence = MockEtlRunPersistence::new()

    mock_executor.expect_list_enabled_streams()
        .returning(|| Ok(vec!["stream-1".to_string(), "stream-2".to_string()]))

    mock_executor.expect_run_stream()
        .returning(|_| Ok(make_test_stats(50, 0, 0)))

    # Persistence fails for start_run
    mock_persistence.expect_start_run()
        .returning(|_, _, _| Err(PersistenceError::ConnectionError("DB down".to_string())))

    # Create daemon
    (_, shutdown_rx) = watch::channel(false)
    daemon = DaemonRunner::new(mock_executor, mock_persistence, DaemonConfig::default(), shutdown_rx)

    # Act
    result = daemon.run_cycle()

    # Assert - Cycle completes despite persistence failure
    assert!(result.is_ok())
    stats = result.unwrap()
    assert_eq!(stats.streams_processed, 2)
    assert_eq!(stats.streams_succeeded, 2)
```

---

## Error Handling Philosophy

### Persistence Errors are Non-Fatal

```
# The daemon continues ETL execution even if persistence fails.
# Rationale:
# - Primary job is data transformation (ETL)
# - Observability is secondary
# - Don't fail data pipeline due to metrics database
# - Log warnings for ops visibility
```

### ETL Errors are Tracked

```
# Stream ETL failures are:
# 1. Logged with tracing
# 2. Recorded in database (if persistence works)
# 3. Counted in Prometheus metrics
# 4. Accumulated in DaemonCycleStats
# 5. NOT fatal to the cycle (other streams continue)
```

---

## Graceful Shutdown Considerations

```
# Shutdown can occur at these points:

1. Before starting any stream:
   - Return DaemonError::Shutdown immediately
   - No incomplete records

2. After start_run but before ETL:
   - Record stays in 'running' state
   - Cleanup job handles stale records

3. After ETL but before complete_run:
   - ETL data is written to Silver
   - Record stays 'running' but data is safe
   - Minor observability gap

4. During complete_run:
   - Data written, record may be inconsistent
   - Acceptable - next run will show correct state
```

---

## DaemonCycleStats Enhancement

```
# Consider adding cycle_id to stats for correlation

STRUCT DaemonCycleStats:
    cycle_id: UUID                    # NEW: Unique cycle identifier
    streams_processed: usize
    streams_succeeded: usize
    streams_failed: usize
    total_rows_processed: u64
    total_rows_flagged: u64
    cycle_duration_ms: u64
```

---

*Pseudocode created: 2026-01-16*
*Next: ETL-RUNNER-CHANGES.md*
