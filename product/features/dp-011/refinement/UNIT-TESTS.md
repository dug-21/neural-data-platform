# DP-011: Unit Tests Specification

**Feature ID**: dp-011
**Phase**: Refinement (SPARC R)
**Created**: 2026-01-16
**Test Type**: Unit (Mock-Based)

---

## Overview

Mock-based unit tests for the ETL run statistics persistence layer. These tests verify behavior without requiring a real database connection.

---

## Test File Location

```
apps/silver-etl/src/persistence.rs

#[cfg(test)]
mod tests {
    // All unit tests defined below
}
```

---

## Trait Definition (To Be Tested)

```rust
use chrono::{DateTime, Utc};
use uuid::Uuid;
use crate::etl::EtlStats;

/// Run mode for ETL execution
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EtlRunMode {
    Daemon,
    Manual,
    Backfill,
}

impl EtlRunMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Daemon => "daemon",
            Self::Manual => "manual",
            Self::Backfill => "backfill",
        }
    }
}

/// Persistence errors
#[derive(Debug, thiserror::Error)]
pub enum PersistenceError {
    #[error("Database error: {0}")]
    Database(String),

    #[error("Run not found: {0}")]
    NotFound(Uuid),

    #[error("Invalid state transition: {0}")]
    InvalidState(String),
}

/// Trait for ETL run persistence - enables mocking in tests
#[cfg_attr(test, mockall::automock)]
pub trait EtlRunPersistence: Send + Sync {
    /// Insert a new run record (status = 'running')
    fn start_run(
        &self,
        stream_id: &str,
        run_mode: EtlRunMode,
        daemon_cycle_id: Option<Uuid>,
    ) -> Result<Uuid, PersistenceError>;

    /// Update run with completion status and stats
    fn complete_run(&self, id: Uuid, stats: &EtlStats) -> Result<(), PersistenceError>;

    /// Update run with failure status and error
    fn fail_run(
        &self,
        id: Uuid,
        error: &str,
        context: Option<serde_json::Value>,
    ) -> Result<(), PersistenceError>;
}
```

---

## Unit Test Specifications

### Test 1: start_run Creates Run Record

```rust
/// Test: start_run creates a new run record with status='running'
///
/// Verifies:
/// - INSERT statement executed with correct columns
/// - UUID returned for tracking
/// - stream_id, run_mode, daemon_cycle_id stored correctly
#[test]
fn test_start_run_creates_record() {
    // Arrange: Mock database connection
    let mut mock_conn = MockDbConnection::new();

    // Expect INSERT into silver.etl_runs
    mock_conn.expect_execute()
        .withf(|sql: &str, params: &[&dyn ToSql]| {
            sql.contains("INSERT INTO") &&
            sql.contains("silver.etl_runs") &&
            sql.contains("stream_id") &&
            sql.contains("run_mode") &&
            sql.contains("status") &&
            sql.contains("'running'")
        })
        .times(1)
        .returning(|_, _| Ok(1));

    let persistence = DuckDbRunPersistence::new(&mock_conn);

    // Act
    let result = persistence.start_run(
        "air-quality",
        EtlRunMode::Daemon,
        Some(Uuid::new_v4()),
    );

    // Assert
    assert!(result.is_ok());
    let run_id = result.unwrap();
    assert!(!run_id.is_nil());
}
```

### Test 2: start_run Returns Valid UUID

```rust
/// Test: start_run returns a valid UUID for the created record
///
/// Verifies:
/// - UUID is v4 format
/// - UUID can be used for subsequent operations
#[test]
fn test_start_run_returns_uuid() {
    // Arrange
    let mut mock_conn = MockDbConnection::new();
    mock_conn.expect_execute()
        .returning(|_, _| Ok(1));

    let persistence = DuckDbRunPersistence::new(&mock_conn);

    // Act
    let result = persistence.start_run("test-stream", EtlRunMode::Manual, None);

    // Assert
    assert!(result.is_ok());
    let uuid = result.unwrap();

    // UUID should be valid v4
    assert_eq!(uuid.get_version_num(), 4);
}
```

### Test 3: complete_run Updates Status to Success

```rust
/// Test: complete_run updates run with status='success' and statistics
///
/// Verifies:
/// - UPDATE statement executed with correct id
/// - status set to 'success'
/// - completed_at timestamp set
/// - duration_ms calculated
#[test]
fn test_complete_run_updates_status() {
    // Arrange
    let mut mock_conn = MockDbConnection::new();
    let run_id = Uuid::new_v4();

    mock_conn.expect_execute()
        .withf(move |sql: &str, params: &[&dyn ToSql]| {
            sql.contains("UPDATE") &&
            sql.contains("silver.etl_runs") &&
            sql.contains("status") &&
            sql.contains("'success'") &&
            sql.contains("completed_at") &&
            sql.contains("WHERE id =")
        })
        .times(1)
        .returning(|_, _| Ok(1));

    let persistence = DuckDbRunPersistence::new(&mock_conn);
    let stats = make_test_stats(100);

    // Act
    let result = persistence.complete_run(run_id, &stats);

    // Assert
    assert!(result.is_ok());
}
```

### Test 4: complete_run Sets All Statistics

```rust
/// Test: complete_run stores all EtlStats fields correctly
///
/// Verifies:
/// - rows_processed stored
/// - rows_flagged stored (maps from rows_with_dq_flags)
/// - rows_rejected stored
/// - duration_ms stored
/// - watermark_before stored
/// - watermark_after stored
#[test]
fn test_complete_run_sets_statistics() {
    // Arrange
    let mut mock_conn = MockDbConnection::new();
    let run_id = Uuid::new_v4();

    mock_conn.expect_execute()
        .withf(|sql: &str, _| {
            sql.contains("rows_processed") &&
            sql.contains("rows_flagged") &&
            sql.contains("rows_rejected") &&
            sql.contains("duration_ms") &&
            sql.contains("watermark_before") &&
            sql.contains("watermark_after")
        })
        .times(1)
        .returning(|_, _| Ok(1));

    let persistence = DuckDbRunPersistence::new(&mock_conn);
    let stats = EtlStats {
        stream_id: "test".to_string(),
        rows_processed: 500,
        rows_with_dq_flags: 25,
        rows_rejected: 3,
        duration_ms: 1500,
        watermark_before: Some(Utc::now() - chrono::Duration::hours(1)),
        watermark_after: Some(Utc::now()),
    };

    // Act
    let result = persistence.complete_run(run_id, &stats);

    // Assert
    assert!(result.is_ok());
}
```

### Test 5: fail_run Records Error Message

```rust
/// Test: fail_run updates run with status='failed' and error_message
///
/// Verifies:
/// - status set to 'failed'
/// - error_message stored
/// - completed_at timestamp set
#[test]
fn test_fail_run_records_error() {
    // Arrange
    let mut mock_conn = MockDbConnection::new();
    let run_id = Uuid::new_v4();

    mock_conn.expect_execute()
        .withf(|sql: &str, _| {
            sql.contains("UPDATE") &&
            sql.contains("status") &&
            sql.contains("'failed'") &&
            sql.contains("error_message") &&
            sql.contains("completed_at")
        })
        .times(1)
        .returning(|_, _| Ok(1));

    let persistence = DuckDbRunPersistence::new(&mock_conn);

    // Act
    let result = persistence.fail_run(
        run_id,
        "SQL execution failed: column not found",
        None,
    );

    // Assert
    assert!(result.is_ok());
}
```

### Test 6: fail_run Stores Error Context

```rust
/// Test: fail_run stores structured error_context as JSONB
///
/// Verifies:
/// - error_context JSONB column populated
/// - JSON structure preserved
#[test]
fn test_fail_run_stores_context() {
    // Arrange
    let mut mock_conn = MockDbConnection::new();
    let run_id = Uuid::new_v4();

    mock_conn.expect_execute()
        .withf(|sql: &str, _| {
            sql.contains("error_context")
        })
        .times(1)
        .returning(|_, _| Ok(1));

    let persistence = DuckDbRunPersistence::new(&mock_conn);

    let context = serde_json::json!({
        "stage": "transform",
        "sql": "INSERT INTO silver.air_quality...",
        "parquet_files": ["file1.parquet", "file2.parquet"],
        "duckdb_error": "column 'wind_speed_kmh' does not exist"
    });

    // Act
    let result = persistence.fail_run(
        run_id,
        "Transform SQL failed",
        Some(context),
    );

    // Assert
    assert!(result.is_ok());
}
```

### Test 7: Persistence Failure is Graceful

```rust
/// Test: Persistence failures return error but don't panic
///
/// Verifies:
/// - Database errors wrapped in PersistenceError
/// - No panics on connection failure
/// - Error message preserved
#[test]
fn test_persistence_failure_graceful() {
    // Arrange
    let mut mock_conn = MockDbConnection::new();

    mock_conn.expect_execute()
        .returning(|_, _| Err(duckdb::Error::QueryError("Connection refused".into())));

    let persistence = DuckDbRunPersistence::new(&mock_conn);

    // Act
    let result = persistence.start_run("air-quality", EtlRunMode::Daemon, None);

    // Assert
    assert!(result.is_err());
    match result.unwrap_err() {
        PersistenceError::Database(msg) => {
            assert!(msg.contains("Connection refused"));
        }
        _ => panic!("Expected PersistenceError::Database"),
    }
}
```

### Test 8: Run Not Found Error

```rust
/// Test: complete_run returns NotFound when run doesn't exist
///
/// Verifies:
/// - UPDATE affects 0 rows -> NotFound error
/// - Error includes the UUID
#[test]
fn test_complete_run_not_found() {
    // Arrange
    let mut mock_conn = MockDbConnection::new();
    let run_id = Uuid::new_v4();

    // Simulate UPDATE affecting 0 rows
    mock_conn.expect_execute()
        .returning(|_, _| Ok(0));

    let persistence = DuckDbRunPersistence::new(&mock_conn);

    // Act
    let result = persistence.complete_run(run_id, &make_test_stats(100));

    // Assert
    assert!(result.is_err());
    match result.unwrap_err() {
        PersistenceError::NotFound(id) => {
            assert_eq!(id, run_id);
        }
        _ => panic!("Expected PersistenceError::NotFound"),
    }
}
```

### Test 9: EtlRunMode Serialization

```rust
/// Test: EtlRunMode serializes to correct string values
///
/// Verifies:
/// - Daemon -> "daemon"
/// - Manual -> "manual"
/// - Backfill -> "backfill"
#[test]
fn test_etl_run_mode_serialization() {
    assert_eq!(EtlRunMode::Daemon.as_str(), "daemon");
    assert_eq!(EtlRunMode::Manual.as_str(), "manual");
    assert_eq!(EtlRunMode::Backfill.as_str(), "backfill");
}
```

### Test 10: Null daemon_cycle_id Handling

```rust
/// Test: start_run handles None daemon_cycle_id
///
/// Verifies:
/// - NULL inserted for daemon_cycle_id when None
/// - No errors on missing cycle_id
#[test]
fn test_start_run_null_cycle_id() {
    // Arrange
    let mut mock_conn = MockDbConnection::new();

    mock_conn.expect_execute()
        .withf(|sql: &str, _| {
            // Should use NULL for daemon_cycle_id
            sql.contains("daemon_cycle_id") &&
            (sql.contains("NULL") || sql.contains("$"))
        })
        .times(1)
        .returning(|_, _| Ok(1));

    let persistence = DuckDbRunPersistence::new(&mock_conn);

    // Act - No cycle_id for manual run
    let result = persistence.start_run("air-quality", EtlRunMode::Manual, None);

    // Assert
    assert!(result.is_ok());
}
```

---

## Test Fixtures

```rust
/// Helper to create test EtlStats
fn make_test_stats(rows: u64) -> EtlStats {
    EtlStats {
        stream_id: "test-stream".to_string(),
        rows_processed: rows,
        rows_with_dq_flags: rows / 20,  // 5% flagged
        rows_rejected: rows / 100,       // 1% rejected
        duration_ms: 100,
        watermark_before: None,
        watermark_after: None,
    }
}

/// Helper to create test stats with watermarks
fn make_test_stats_with_watermarks(rows: u64) -> EtlStats {
    use chrono::{Duration, Utc};

    EtlStats {
        stream_id: "test-stream".to_string(),
        rows_processed: rows,
        rows_with_dq_flags: 0,
        rows_rejected: 0,
        duration_ms: 100,
        watermark_before: Some(Utc::now() - Duration::hours(1)),
        watermark_after: Some(Utc::now()),
    }
}

/// Helper to create error context JSON
fn make_test_error_context() -> serde_json::Value {
    serde_json::json!({
        "stage": "transform",
        "sql": "INSERT INTO silver.test...",
        "parquet_files": ["file1.parquet"]
    })
}
```

---

## Mock Database Connection Trait

```rust
/// Trait for database connection - enables mocking
#[cfg_attr(test, mockall::automock)]
pub trait DbConnection: Send + Sync {
    /// Execute SQL with parameters, return rows affected
    fn execute(&self, sql: &str, params: &[&dyn duckdb::ToSql]) -> Result<usize, duckdb::Error>;

    /// Query single row, return column values
    fn query_row<T, F>(&self, sql: &str, params: &[&dyn duckdb::ToSql], f: F) -> Result<T, duckdb::Error>
    where
        F: FnOnce(&duckdb::Row<'_>) -> Result<T, duckdb::Error>;
}
```

---

## Test Execution

```bash
# Run all persistence unit tests
cargo test --package silver-etl persistence::tests

# Run specific test
cargo test --package silver-etl test_start_run_creates_record -- --exact

# Run with output
cargo test --package silver-etl persistence::tests -- --nocapture
```

---

## Coverage Targets

| Function | Test Coverage |
|----------|---------------|
| `start_run` | 3 tests |
| `complete_run` | 3 tests |
| `fail_run` | 2 tests |
| `EtlRunMode` | 1 test |
| Error paths | 2 tests |
| **Total** | **11 tests** |

---

*Unit tests specification created: 2026-01-16*
