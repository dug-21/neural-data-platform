# DP-011: Test Fixtures Specification

**Feature ID**: dp-011
**Phase**: Refinement (SPARC R)
**Created**: 2026-01-16
**Test Type**: Shared Test Utilities

---

## Overview

Common test fixtures, helpers, and mock factories for dp-011 ETL run statistics persistence testing. These utilities are shared across unit, daemon, and integration tests.

---

## File Location

```
apps/silver-etl/src/persistence.rs

// At the end of the file, inside #[cfg(test)]
#[cfg(test)]
pub(crate) mod test_fixtures {
    // All fixtures defined below
}
```

Or in a dedicated test utilities module:

```
apps/silver-etl/src/test_utils.rs   # If multiple modules need fixtures
```

---

## EtlStats Fixtures

### Basic Stats Factory

```rust
/// Create test EtlStats with specified row count
///
/// Default values:
/// - rows_with_dq_flags: 5% of rows
/// - rows_rejected: 1% of rows
/// - duration_ms: 100ms
/// - watermarks: None
pub fn make_test_stats(rows: u64) -> EtlStats {
    EtlStats {
        stream_id: "test-stream".to_string(),
        rows_processed: rows,
        rows_with_dq_flags: rows / 20,  // 5%
        rows_rejected: rows / 100,       // 1%
        duration_ms: 100,
        watermark_before: None,
        watermark_after: None,
    }
}

/// Create stats with custom stream_id
pub fn make_test_stats_for_stream(stream_id: &str, rows: u64) -> EtlStats {
    EtlStats {
        stream_id: stream_id.to_string(),
        rows_processed: rows,
        rows_with_dq_flags: rows / 20,
        rows_rejected: rows / 100,
        duration_ms: 100,
        watermark_before: None,
        watermark_after: None,
    }
}
```

### Stats with Watermarks

```rust
use chrono::{DateTime, Duration, Utc};

/// Create stats with watermark timestamps
///
/// Useful for testing incremental ETL scenarios
pub fn make_test_stats_with_watermarks(rows: u64) -> EtlStats {
    let now = Utc::now();
    EtlStats {
        stream_id: "test-stream".to_string(),
        rows_processed: rows,
        rows_with_dq_flags: 0,
        rows_rejected: 0,
        duration_ms: 100,
        watermark_before: Some(now - Duration::hours(1)),
        watermark_after: Some(now),
    }
}

/// Create stats with specific watermark window
pub fn make_test_stats_with_window(
    rows: u64,
    window_start: DateTime<Utc>,
    window_end: DateTime<Utc>,
) -> EtlStats {
    EtlStats {
        stream_id: "test-stream".to_string(),
        rows_processed: rows,
        rows_with_dq_flags: 0,
        rows_rejected: 0,
        duration_ms: (window_end - window_start).num_milliseconds() as u64,
        watermark_before: Some(window_start),
        watermark_after: Some(window_end),
    }
}
```

### Stats with DQ Flags

```rust
/// Create stats with high DQ flag rate (for testing quality scenarios)
pub fn make_test_stats_with_quality_issues(rows: u64) -> EtlStats {
    EtlStats {
        stream_id: "test-stream".to_string(),
        rows_processed: rows,
        rows_with_dq_flags: rows / 4,   // 25% flagged
        rows_rejected: rows / 10,        // 10% rejected
        duration_ms: 200,
        watermark_before: None,
        watermark_after: Some(Utc::now()),
    }
}

/// Create perfect stats (no quality issues)
pub fn make_test_stats_perfect(rows: u64) -> EtlStats {
    EtlStats {
        stream_id: "test-stream".to_string(),
        rows_processed: rows,
        rows_with_dq_flags: 0,
        rows_rejected: 0,
        duration_ms: 50,
        watermark_before: None,
        watermark_after: Some(Utc::now()),
    }
}
```

---

## Error Context Fixtures

### Standard Error Context

```rust
use serde_json::{json, Value};

/// Create error context for transform failures
pub fn make_test_error_context() -> Value {
    json!({
        "stage": "transform",
        "sql": "INSERT INTO silver.test_table...",
        "parquet_files": ["file1.parquet"]
    })
}

/// Create detailed transform error context
pub fn make_transform_error_context(
    sql: &str,
    parquet_files: Vec<&str>,
    duckdb_error: &str,
) -> Value {
    json!({
        "stage": "transform",
        "sql": sql,
        "parquet_files": parquet_files,
        "duckdb_error": duckdb_error,
        "timestamp": Utc::now().to_rfc3339()
    })
}

/// Create connection error context
pub fn make_connection_error_context(host: &str, error: &str) -> Value {
    json!({
        "stage": "connection",
        "host": host,
        "error_type": "connection_refused",
        "error_message": error,
        "retry_count": 3
    })
}

/// Create schema error context
pub fn make_schema_error_context(
    table: &str,
    missing_column: &str,
) -> Value {
    json!({
        "stage": "schema_validation",
        "table": table,
        "missing_column": missing_column,
        "available_columns": ["id", "timestamp", "value"],
        "error_type": "column_not_found"
    })
}
```

---

## Mock Factories

### MockEtlRunPersistence Factory

```rust
use mockall::predicate::*;

/// Create a mock persistence that always succeeds
pub fn success_persistence() -> MockEtlRunPersistence {
    let mut mock = MockEtlRunPersistence::new();
    mock.expect_start_run()
        .returning(|_, _, _| Ok(Uuid::new_v4()));
    mock.expect_complete_run()
        .returning(|_, _| Ok(()));
    mock.expect_fail_run()
        .returning(|_, _, _| Ok(()));
    mock
}

/// Create a mock persistence that fails on start_run
pub fn failing_start_persistence() -> MockEtlRunPersistence {
    let mut mock = MockEtlRunPersistence::new();
    mock.expect_start_run()
        .returning(|_, _, _| Err(PersistenceError::Database("Connection refused".into())));
    mock
}

/// Create a mock persistence that fails on complete_run
pub fn failing_complete_persistence() -> MockEtlRunPersistence {
    let mut mock = MockEtlRunPersistence::new();
    mock.expect_start_run()
        .returning(|_, _, _| Ok(Uuid::new_v4()));
    mock.expect_complete_run()
        .returning(|_, _| Err(PersistenceError::Database("Timeout".into())));
    mock
}

/// Create a mock persistence that tracks calls
pub fn tracking_persistence() -> (MockEtlRunPersistence, Arc<Mutex<Vec<PersistenceCall>>>) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let calls_clone = calls.clone();

    let mut mock = MockEtlRunPersistence::new();

    let calls_start = calls.clone();
    mock.expect_start_run()
        .returning(move |stream_id, mode, cycle_id| {
            calls_start.lock().unwrap().push(PersistenceCall::Start {
                stream_id: stream_id.to_string(),
                mode,
                cycle_id,
            });
            Ok(Uuid::new_v4())
        });

    let calls_complete = calls.clone();
    mock.expect_complete_run()
        .returning(move |id, stats| {
            calls_complete.lock().unwrap().push(PersistenceCall::Complete {
                id,
                rows_processed: stats.rows_processed,
            });
            Ok(())
        });

    let calls_fail = calls.clone();
    mock.expect_fail_run()
        .returning(move |id, error, _| {
            calls_fail.lock().unwrap().push(PersistenceCall::Fail {
                id,
                error: error.to_string(),
            });
            Ok(())
        });

    (mock, calls_clone)
}

/// Record of persistence calls for verification
#[derive(Debug, Clone)]
pub enum PersistenceCall {
    Start {
        stream_id: String,
        mode: EtlRunMode,
        cycle_id: Option<Uuid>,
    },
    Complete {
        id: Uuid,
        rows_processed: u64,
    },
    Fail {
        id: Uuid,
        error: String,
    },
}
```

### MockEtlExecutor Factory

```rust
/// Create a mock executor that succeeds for all streams
pub fn success_executor(streams: Vec<String>) -> MockEtlExecutor {
    let mut mock = MockEtlExecutor::new();
    let streams_clone = streams.clone();

    mock.expect_list_enabled_streams()
        .returning(move || Ok(streams_clone.clone()));

    mock.expect_run_stream()
        .returning(|stream_id| Ok(make_test_stats_for_stream(stream_id, 100)));

    mock
}

/// Create a mock executor where specific streams fail
pub fn partial_failure_executor(
    streams: Vec<String>,
    failing_streams: Vec<String>,
) -> MockEtlExecutor {
    let mut mock = MockEtlExecutor::new();
    let streams_clone = streams.clone();

    mock.expect_list_enabled_streams()
        .returning(move || Ok(streams_clone.clone()));

    let failing = failing_streams.clone();
    mock.expect_run_stream()
        .returning(move |stream_id| {
            if failing.contains(&stream_id.to_string()) {
                Err(DaemonError::Etl(format!("Simulated failure for {}", stream_id)))
            } else {
                Ok(make_test_stats_for_stream(stream_id, 100))
            }
        });

    mock
}

/// Create a mock executor that always fails
pub fn failing_executor() -> MockEtlExecutor {
    let mut mock = MockEtlExecutor::new();

    mock.expect_list_enabled_streams()
        .returning(|| Err(DaemonError::Config("Config unavailable".into())));

    mock
}
```

---

## UUID Fixtures

```rust
/// Well-known UUIDs for testing (deterministic)
pub mod test_uuids {
    use uuid::Uuid;

    /// UUID for test run 1
    pub fn run_1() -> Uuid {
        Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap()
    }

    /// UUID for test run 2
    pub fn run_2() -> Uuid {
        Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap()
    }

    /// UUID for test cycle
    pub fn cycle() -> Uuid {
        Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap()
    }
}
```

---

## Stream Configuration Fixtures

```rust
/// Test stream identifiers following NDP naming convention
pub mod test_streams {
    pub const AIR_QUALITY: &str = "air-quality";
    pub const OUTDOOR_WEATHER: &str = "outdoor-weather";
    pub const NWS_FORECAST: &str = "nws-gridpoints-forecast";
    pub const INDOOR_SENSORS: &str = "indoor-sensors";

    /// All test streams
    pub fn all() -> Vec<String> {
        vec![
            AIR_QUALITY.to_string(),
            OUTDOOR_WEATHER.to_string(),
            NWS_FORECAST.to_string(),
            INDOOR_SENSORS.to_string(),
        ]
    }

    /// Subset for quick tests
    pub fn minimal() -> Vec<String> {
        vec![AIR_QUALITY.to_string()]
    }
}
```

---

## Daemon Configuration Fixtures

```rust
/// Create daemon config for fast tests (short intervals)
pub fn fast_daemon_config() -> DaemonConfig {
    DaemonConfig {
        interval_secs: 1,
        stream_filter: None,
        max_consecutive_failures: 3,
        backoff_multiplier: 1.5,
    }
}

/// Create daemon config with specific stream filter
pub fn filtered_daemon_config(stream_id: &str) -> DaemonConfig {
    DaemonConfig {
        interval_secs: 1,
        stream_filter: Some(stream_id.to_string()),
        max_consecutive_failures: 3,
        backoff_multiplier: 2.0,
    }
}

/// Create daemon config with aggressive backoff (for testing failure handling)
pub fn aggressive_backoff_config() -> DaemonConfig {
    DaemonConfig {
        interval_secs: 1,
        stream_filter: None,
        max_consecutive_failures: 1,
        backoff_multiplier: 10.0,
    }
}
```

---

## Database Row Fixtures

```rust
use chrono::{DateTime, Utc};

/// Row structure matching silver.etl_runs table
#[derive(Debug, Clone)]
pub struct EtlRunRow {
    pub id: Uuid,
    pub stream_id: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<i64>,
    pub status: String,
    pub rows_processed: i64,
    pub rows_flagged: i64,
    pub rows_rejected: i64,
    pub watermark_before: Option<DateTime<Utc>>,
    pub watermark_after: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub error_context: Option<Value>,
    pub run_mode: String,
    pub daemon_cycle_id: Option<Uuid>,
}

impl EtlRunRow {
    /// Create a running state row
    pub fn running(stream_id: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            stream_id: stream_id.to_string(),
            started_at: Utc::now(),
            completed_at: None,
            duration_ms: None,
            status: "running".to_string(),
            rows_processed: 0,
            rows_flagged: 0,
            rows_rejected: 0,
            watermark_before: None,
            watermark_after: None,
            error_message: None,
            error_context: None,
            run_mode: "daemon".to_string(),
            daemon_cycle_id: Some(Uuid::new_v4()),
        }
    }

    /// Create a success state row
    pub fn success(stream_id: &str, stats: &EtlStats) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            stream_id: stream_id.to_string(),
            started_at: now - chrono::Duration::milliseconds(stats.duration_ms as i64),
            completed_at: Some(now),
            duration_ms: Some(stats.duration_ms as i64),
            status: "success".to_string(),
            rows_processed: stats.rows_processed as i64,
            rows_flagged: stats.rows_with_dq_flags as i64,
            rows_rejected: stats.rows_rejected as i64,
            watermark_before: stats.watermark_before,
            watermark_after: stats.watermark_after,
            error_message: None,
            error_context: None,
            run_mode: "daemon".to_string(),
            daemon_cycle_id: Some(Uuid::new_v4()),
        }
    }

    /// Create a failed state row
    pub fn failed(stream_id: &str, error: &str) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            stream_id: stream_id.to_string(),
            started_at: now - chrono::Duration::seconds(5),
            completed_at: Some(now),
            duration_ms: Some(5000),
            status: "failed".to_string(),
            rows_processed: 0,
            rows_flagged: 0,
            rows_rejected: 0,
            watermark_before: None,
            watermark_after: None,
            error_message: Some(error.to_string()),
            error_context: Some(make_test_error_context()),
            run_mode: "daemon".to_string(),
            daemon_cycle_id: Some(Uuid::new_v4()),
        }
    }
}
```

---

## Assertion Helpers

```rust
/// Assert EtlStats match expected values with tolerance
pub fn assert_stats_eq(actual: &EtlStats, expected: &EtlStats) {
    assert_eq!(actual.stream_id, expected.stream_id, "stream_id mismatch");
    assert_eq!(actual.rows_processed, expected.rows_processed, "rows_processed mismatch");
    assert_eq!(actual.rows_with_dq_flags, expected.rows_with_dq_flags, "rows_with_dq_flags mismatch");
    assert_eq!(actual.rows_rejected, expected.rows_rejected, "rows_rejected mismatch");
}

/// Assert persistence was called for all streams
pub fn assert_all_streams_persisted(
    calls: &[PersistenceCall],
    expected_streams: &[String],
) {
    let start_calls: Vec<&String> = calls.iter()
        .filter_map(|c| match c {
            PersistenceCall::Start { stream_id, .. } => Some(stream_id),
            _ => None,
        })
        .collect();

    for stream in expected_streams {
        assert!(
            start_calls.contains(&stream),
            "Expected start_run for stream '{}' but not found in calls",
            stream
        );
    }
}
```

---

## Usage Example

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::test_fixtures::*;

    #[tokio::test]
    async fn test_using_fixtures() {
        // Use mock factories
        let mock_persistence = success_persistence();
        let mock_executor = success_executor(test_streams::all());

        // Use config fixtures
        let config = fast_daemon_config();

        // Use stats fixtures
        let stats = make_test_stats_with_watermarks(500);

        // Use error context fixtures
        let context = make_transform_error_context(
            "INSERT INTO silver.test...",
            vec!["file.parquet"],
            "column not found"
        );

        // Use assertion helpers
        let expected = make_test_stats(500);
        assert_stats_eq(&stats, &expected);
    }
}
```

---

## Module Exports

```rust
#[cfg(test)]
pub(crate) mod test_fixtures {
    pub use super::{
        // Stats factories
        make_test_stats,
        make_test_stats_for_stream,
        make_test_stats_with_watermarks,
        make_test_stats_with_window,
        make_test_stats_with_quality_issues,
        make_test_stats_perfect,

        // Error context factories
        make_test_error_context,
        make_transform_error_context,
        make_connection_error_context,
        make_schema_error_context,

        // Mock factories
        success_persistence,
        failing_start_persistence,
        failing_complete_persistence,
        tracking_persistence,
        success_executor,
        partial_failure_executor,
        failing_executor,

        // Config factories
        fast_daemon_config,
        filtered_daemon_config,
        aggressive_backoff_config,

        // Fixed values
        test_uuids,
        test_streams,

        // Types
        PersistenceCall,
        EtlRunRow,

        // Assertion helpers
        assert_stats_eq,
        assert_all_streams_persisted,
    };
}
```

---

*Test fixtures specification created: 2026-01-16*
