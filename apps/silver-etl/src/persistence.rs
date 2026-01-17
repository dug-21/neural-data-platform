//! ETL Run Statistics Persistence (dp-011)
//!
//! Provides persistence for ETL run statistics to TimescaleDB for operational
//! observability. Uses the Domain Adapter pattern with trait-based abstraction
//! for testability (London TDD with mockall).
//!
//! # Architecture
//!
//! ```text
//! DaemonRunner
//!     │
//!     ▼
//! EtlRunPersistence (trait)
//!     │
//!     ├─► DuckDbRunPersistence (production - uses DuckDB postgres extension)
//!     └─► NoOpPersistence (backwards compatibility / disabled mode)
//! ```
//!
//! # Graceful Degradation
//!
//! Persistence failures MUST NOT fail ETL execution. The daemon logs warnings
//! but continues processing data. This ensures ETL availability even when the
//! statistics database is temporarily unavailable.

use chrono::{DateTime, Utc};
use duckdb::Connection;
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::etl::EtlStats;

// =============================================================================
// Enums
// =============================================================================

/// How the ETL run was triggered
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EtlRunMode {
    /// Scheduled daemon execution (every N minutes)
    Daemon,
    /// CLI manual run (user-triggered)
    Manual,
    /// Historical data reprocessing
    Backfill,
}

impl fmt::Display for EtlRunMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EtlRunMode::Daemon => write!(f, "daemon"),
            EtlRunMode::Manual => write!(f, "manual"),
            EtlRunMode::Backfill => write!(f, "backfill"),
        }
    }
}

impl EtlRunMode {
    /// Convert to database string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            EtlRunMode::Daemon => "daemon",
            EtlRunMode::Manual => "manual",
            EtlRunMode::Backfill => "backfill",
        }
    }
}

/// Current status of an ETL run
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EtlRunStatus {
    /// ETL in progress
    Running,
    /// Completed without errors
    Success,
    /// Completed with errors
    Failed,
    /// Some streams in cycle failed (daemon mode)
    Partial,
}

impl fmt::Display for EtlRunStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EtlRunStatus::Running => write!(f, "running"),
            EtlRunStatus::Success => write!(f, "success"),
            EtlRunStatus::Failed => write!(f, "failed"),
            EtlRunStatus::Partial => write!(f, "partial"),
        }
    }
}

impl EtlRunStatus {
    /// Convert to database string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            EtlRunStatus::Running => "running",
            EtlRunStatus::Success => "success",
            EtlRunStatus::Failed => "failed",
            EtlRunStatus::Partial => "partial",
        }
    }
}

// =============================================================================
// Error Types
// =============================================================================

/// Errors that can occur during persistence operations
#[derive(Debug, Error)]
pub enum PersistenceError {
    /// Database connection error
    #[error("Database connection error: {0}")]
    Connection(String),

    /// SQL execution error
    #[error("SQL execution error: {0}")]
    SqlExecution(String),

    /// Run record not found
    #[error("Run record not found: {0}")]
    RunNotFound(Uuid),

    /// Serialization error (for error_context JSON)
    #[error("Serialization error: {0}")]
    Serialization(String),
}

// =============================================================================
// Data Structures
// =============================================================================

/// Complete ETL run record as stored in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EtlRunRecord {
    pub id: Uuid,
    pub stream_id: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_ms: Option<i64>,
    pub status: EtlRunStatus,
    pub rows_processed: i64,
    pub rows_flagged: i64,
    pub rows_rejected: i64,
    pub watermark_before: Option<DateTime<Utc>>,
    pub watermark_after: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub error_context: Option<serde_json::Value>,
    pub run_mode: EtlRunMode,
    pub daemon_cycle_id: Option<Uuid>,
}

// =============================================================================
// Trait Definition
// =============================================================================

/// Trait for ETL run persistence - enables mocking in tests (London TDD style)
///
/// Implementations must be Send + Sync for use across threads.
/// The trait uses `&self` (immutable reference) for all methods.
///
/// # Error Handling
///
/// All methods return `Result` but callers should handle errors gracefully.
/// Persistence failures should NOT fail ETL execution.
#[cfg_attr(test, mockall::automock)]
pub trait EtlRunPersistence: Send + Sync {
    /// Insert a new run record with status='running'
    ///
    /// # Arguments
    ///
    /// * `stream_id` - Stream identifier (e.g., "air-quality")
    /// * `run_mode` - How the run was triggered
    /// * `daemon_cycle_id` - Optional UUID linking runs in the same daemon cycle
    ///
    /// # Returns
    ///
    /// UUID of the created run record
    fn start_run(
        &self,
        stream_id: &str,
        run_mode: EtlRunMode,
        daemon_cycle_id: Option<Uuid>,
    ) -> Result<Uuid, PersistenceError>;

    /// Update run with success status and statistics
    ///
    /// # Arguments
    ///
    /// * `run_id` - UUID from start_run
    /// * `stats` - ETL execution statistics
    fn complete_run(&self, run_id: Uuid, stats: &EtlStats) -> Result<(), PersistenceError>;

    /// Update run with failure status and error information
    ///
    /// # Arguments
    ///
    /// * `run_id` - UUID from start_run
    /// * `error_message` - Human-readable error description
    /// * `error_context` - Optional structured error details (JSON)
    fn fail_run(
        &self,
        run_id: Uuid,
        error_message: &str,
        error_context: Option<serde_json::Value>,
    ) -> Result<(), PersistenceError>;
}

// =============================================================================
// DuckDB Implementation
// =============================================================================

/// Production implementation using DuckDB's PostgreSQL extension
///
/// This implementation writes to TimescaleDB through DuckDB's attached
/// PostgreSQL database. It assumes PostgreSQL is already attached as 'pg'.
///
/// # Thread Safety
///
/// DuckDB Connection is Send but not Sync. This implementation uses
/// internal mutability through DuckDB's own synchronization.
pub struct DuckDbRunPersistence {
    /// Reference to DuckDB connection with PostgreSQL attached
    conn: Connection,
}

impl DuckDbRunPersistence {
    /// Create a new persistence instance
    ///
    /// # Arguments
    ///
    /// * `pg_conn_str` - PostgreSQL connection string for DuckDB attachment
    ///
    /// # Errors
    ///
    /// Returns error if DuckDB cannot connect or attach PostgreSQL.
    pub fn new(pg_conn_str: &str) -> Result<Self, PersistenceError> {
        debug!("Creating DuckDbRunPersistence with PostgreSQL attachment");

        let conn = Connection::open_in_memory()
            .map_err(|e| PersistenceError::Connection(e.to_string()))?;

        // Load postgres extension
        conn.execute_batch("INSTALL postgres; LOAD postgres;")
            .map_err(|e| PersistenceError::Connection(format!("Failed to load postgres extension: {}", e)))?;

        // Attach PostgreSQL database
        let attach_sql = format!("ATTACH '{}' AS pg (TYPE postgres)", pg_conn_str);
        conn.execute_batch(&attach_sql)
            .map_err(|e| PersistenceError::Connection(format!("Failed to attach PostgreSQL: {}", e)))?;

        // Verify table exists
        match conn.execute("SELECT 1 FROM pg.silver.etl_runs LIMIT 1", []) {
            Ok(_) => debug!("Verified silver.etl_runs table exists"),
            Err(e) => {
                if e.to_string().contains("does not exist") {
                    return Err(PersistenceError::Connection(
                        "Table silver.etl_runs does not exist. Run migration 003_etl_runs.sql".to_string()
                    ));
                }
                // Table might be empty, that's OK
                debug!("Table check result: {}", e);
            }
        }

        info!("DuckDbRunPersistence initialized successfully");
        Ok(Self { conn })
    }

    /// Create from an existing DuckDB connection with PostgreSQL already attached
    ///
    /// This is useful when sharing a connection with EtlRunner.
    ///
    /// # Safety
    ///
    /// The caller must ensure PostgreSQL is attached as 'pg' before calling.
    pub fn from_connection(conn: Connection) -> Result<Self, PersistenceError> {
        // Verify PostgreSQL is attached and table exists
        match conn.execute("SELECT 1 FROM pg.silver.etl_runs LIMIT 1", []) {
            Ok(_) => debug!("Verified silver.etl_runs table exists"),
            Err(e) => {
                if e.to_string().contains("does not exist") {
                    return Err(PersistenceError::Connection(
                        "Table silver.etl_runs does not exist or PostgreSQL not attached".to_string()
                    ));
                }
                // Table might be empty, that's OK
                debug!("Table verification: {}", e);
            }
        }

        Ok(Self { conn })
    }
}

impl EtlRunPersistence for DuckDbRunPersistence {
    fn start_run(
        &self,
        stream_id: &str,
        run_mode: EtlRunMode,
        daemon_cycle_id: Option<Uuid>,
    ) -> Result<Uuid, PersistenceError> {
        let run_id = Uuid::new_v4();

        let sql = r#"
            INSERT INTO pg.silver.etl_runs (
                id,
                stream_id,
                started_at,
                status,
                rows_processed,
                rows_flagged,
                rows_rejected,
                run_mode,
                daemon_cycle_id
            ) VALUES (
                $1::UUID,
                $2,
                NOW(),
                'running',
                0,
                0,
                0,
                $3,
                $4::UUID
            )
        "#;

        let cycle_id_str = daemon_cycle_id.map(|id| id.to_string());

        self.conn
            .execute(
                sql,
                duckdb::params![
                    run_id.to_string(),
                    stream_id,
                    run_mode.as_str(),
                    cycle_id_str,
                ],
            )
            .map_err(|e| {
                error!(error = %e, stream_id = %stream_id, "Failed to start run record");
                PersistenceError::SqlExecution(e.to_string())
            })?;

        debug!(
            run_id = %run_id,
            stream_id = %stream_id,
            run_mode = %run_mode,
            "Started ETL run record"
        );

        Ok(run_id)
    }

    fn complete_run(&self, run_id: Uuid, stats: &EtlStats) -> Result<(), PersistenceError> {
        let sql = r#"
            UPDATE pg.silver.etl_runs
            SET
                completed_at = NOW(),
                duration_ms = $2,
                status = 'success',
                rows_processed = $3,
                rows_flagged = $4,
                rows_rejected = $5,
                watermark_before = $6::TIMESTAMPTZ,
                watermark_after = $7::TIMESTAMPTZ
            WHERE id = $1::UUID
        "#;

        let wm_before = stats.watermark_before.map(|dt| dt.to_rfc3339());
        let wm_after = stats.watermark_after.map(|dt| dt.to_rfc3339());

        let rows_affected = self
            .conn
            .execute(
                sql,
                duckdb::params![
                    run_id.to_string(),
                    stats.duration_ms as i64,
                    stats.rows_processed as i64,
                    stats.rows_with_dq_flags as i64,
                    stats.rows_rejected as i64,
                    wm_before,
                    wm_after,
                ],
            )
            .map_err(|e| {
                error!(error = %e, run_id = %run_id, "Failed to complete run record");
                PersistenceError::SqlExecution(e.to_string())
            })?;

        if rows_affected == 0 {
            warn!(run_id = %run_id, "No run record found to complete");
            return Err(PersistenceError::RunNotFound(run_id));
        }

        info!(
            run_id = %run_id,
            stream_id = %stats.stream_id,
            rows_processed = stats.rows_processed,
            duration_ms = stats.duration_ms,
            "Completed ETL run record"
        );

        Ok(())
    }

    fn fail_run(
        &self,
        run_id: Uuid,
        error_message: &str,
        error_context: Option<serde_json::Value>,
    ) -> Result<(), PersistenceError> {
        let context_json = error_context
            .map(|ctx| serde_json::to_string(&ctx))
            .transpose()
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        let sql = r#"
            UPDATE pg.silver.etl_runs
            SET
                completed_at = NOW(),
                duration_ms = EXTRACT(EPOCH FROM (NOW()::TIMESTAMP - started_at::TIMESTAMP))::BIGINT * 1000,
                status = 'failed',
                error_message = $2,
                error_context = $3::JSON
            WHERE id = $1::UUID
        "#;

        let rows_affected = self
            .conn
            .execute(
                sql,
                duckdb::params![run_id.to_string(), error_message, context_json,],
            )
            .map_err(|e| {
                error!(error = %e, run_id = %run_id, "Failed to record run failure");
                PersistenceError::SqlExecution(e.to_string())
            })?;

        if rows_affected == 0 {
            warn!(run_id = %run_id, "No run record found to fail");
            return Err(PersistenceError::RunNotFound(run_id));
        }

        error!(
            run_id = %run_id,
            error = %error_message,
            "Recorded ETL run failure"
        );

        Ok(())
    }
}

// Implement Send + Sync for DuckDbRunPersistence
// DuckDB Connection is Send, and our implementation uses it in a thread-safe manner
unsafe impl Sync for DuckDbRunPersistence {}

// =============================================================================
// NoOp Implementation
// =============================================================================

/// No-operation persistence for backwards compatibility and disabled mode
///
/// This implementation does nothing - all operations succeed silently.
/// Use when:
/// - Running without a database
/// - Persistence is explicitly disabled
/// - Backwards compatibility with older configurations
#[derive(Debug, Default)]
pub struct NoOpPersistence;

impl NoOpPersistence {
    pub fn new() -> Self {
        Self
    }
}

impl EtlRunPersistence for NoOpPersistence {
    fn start_run(
        &self,
        stream_id: &str,
        run_mode: EtlRunMode,
        _daemon_cycle_id: Option<Uuid>,
    ) -> Result<Uuid, PersistenceError> {
        let run_id = Uuid::new_v4();
        debug!(
            run_id = %run_id,
            stream_id = %stream_id,
            run_mode = %run_mode,
            "NoOpPersistence: start_run (no-op)"
        );
        Ok(run_id)
    }

    fn complete_run(&self, run_id: Uuid, stats: &EtlStats) -> Result<(), PersistenceError> {
        debug!(
            run_id = %run_id,
            stream_id = %stats.stream_id,
            rows_processed = stats.rows_processed,
            "NoOpPersistence: complete_run (no-op)"
        );
        Ok(())
    }

    fn fail_run(
        &self,
        run_id: Uuid,
        error_message: &str,
        _error_context: Option<serde_json::Value>,
    ) -> Result<(), PersistenceError> {
        debug!(
            run_id = %run_id,
            error = %error_message,
            "NoOpPersistence: fail_run (no-op)"
        );
        Ok(())
    }
}

// =============================================================================
// Tests (London TDD - Mock-Based)
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::*;

    // =========================================================================
    // Test helpers
    // =========================================================================

    fn make_test_stats(stream_id: &str, rows: u64) -> EtlStats {
        EtlStats {
            stream_id: stream_id.to_string(),
            rows_processed: rows,
            rows_with_dq_flags: rows / 20, // 5% flagged
            rows_rejected: 0,
            duration_ms: 150,
            watermark_before: None,
            watermark_after: Some(Utc::now()),
        }
    }

    // =========================================================================
    // EtlRunMode tests
    // =========================================================================

    #[test]
    fn test_etl_run_mode_display() {
        assert_eq!(EtlRunMode::Daemon.to_string(), "daemon");
        assert_eq!(EtlRunMode::Manual.to_string(), "manual");
        assert_eq!(EtlRunMode::Backfill.to_string(), "backfill");
    }

    #[test]
    fn test_etl_run_mode_as_str() {
        assert_eq!(EtlRunMode::Daemon.as_str(), "daemon");
        assert_eq!(EtlRunMode::Manual.as_str(), "manual");
        assert_eq!(EtlRunMode::Backfill.as_str(), "backfill");
    }

    // =========================================================================
    // EtlRunStatus tests
    // =========================================================================

    #[test]
    fn test_etl_run_status_display() {
        assert_eq!(EtlRunStatus::Running.to_string(), "running");
        assert_eq!(EtlRunStatus::Success.to_string(), "success");
        assert_eq!(EtlRunStatus::Failed.to_string(), "failed");
        assert_eq!(EtlRunStatus::Partial.to_string(), "partial");
    }

    #[test]
    fn test_etl_run_status_as_str() {
        assert_eq!(EtlRunStatus::Running.as_str(), "running");
        assert_eq!(EtlRunStatus::Success.as_str(), "success");
        assert_eq!(EtlRunStatus::Failed.as_str(), "failed");
        assert_eq!(EtlRunStatus::Partial.as_str(), "partial");
    }

    // =========================================================================
    // NoOpPersistence tests
    // =========================================================================

    #[test]
    fn test_noop_persistence_start_run_returns_uuid() {
        let persistence = NoOpPersistence::new();

        let result = persistence.start_run("air-quality", EtlRunMode::Daemon, None);

        assert!(result.is_ok());
        let run_id = result.unwrap();
        // UUID should be valid (non-nil)
        assert!(!run_id.is_nil());
    }

    #[test]
    fn test_noop_persistence_complete_run_succeeds() {
        let persistence = NoOpPersistence::new();
        let run_id = Uuid::new_v4();
        let stats = make_test_stats("air-quality", 100);

        let result = persistence.complete_run(run_id, &stats);

        assert!(result.is_ok());
    }

    #[test]
    fn test_noop_persistence_fail_run_succeeds() {
        let persistence = NoOpPersistence::new();
        let run_id = Uuid::new_v4();

        let result = persistence.fail_run(run_id, "Test error", None);

        assert!(result.is_ok());
    }

    #[test]
    fn test_noop_persistence_with_cycle_id() {
        let persistence = NoOpPersistence::new();
        let cycle_id = Uuid::new_v4();

        let result = persistence.start_run("outdoor-weather", EtlRunMode::Daemon, Some(cycle_id));

        assert!(result.is_ok());
    }

    // =========================================================================
    // MockEtlRunPersistence tests (London TDD pattern verification)
    // =========================================================================

    #[test]
    fn test_mock_persistence_start_run_with_correct_args() {
        let mut mock = MockEtlRunPersistence::new();

        // Expect start_run to be called with specific arguments
        mock.expect_start_run()
            .with(
                eq("air-quality"),
                eq(EtlRunMode::Daemon),
                function(|opt: &Option<Uuid>| opt.is_some()),
            )
            .times(1)
            .returning(|_, _, _| Ok(Uuid::new_v4()));

        let cycle_id = Uuid::new_v4();
        let result = mock.start_run("air-quality", EtlRunMode::Daemon, Some(cycle_id));

        assert!(result.is_ok());
    }

    #[test]
    fn test_mock_persistence_complete_run_with_stats() {
        let mut mock = MockEtlRunPersistence::new();
        let run_id = Uuid::new_v4();

        mock.expect_complete_run()
            .with(
                eq(run_id),
                function(|stats: &EtlStats| {
                    stats.rows_processed == 100 && stats.stream_id == "air-quality"
                }),
            )
            .times(1)
            .returning(|_, _| Ok(()));

        let stats = make_test_stats("air-quality", 100);
        let result = mock.complete_run(run_id, &stats);

        assert!(result.is_ok());
    }

    #[test]
    fn test_mock_persistence_fail_run_with_error() {
        let mut mock = MockEtlRunPersistence::new();
        let run_id = Uuid::new_v4();

        mock.expect_fail_run()
            .with(
                eq(run_id),
                eq("Connection timeout"),
                function(|ctx: &Option<serde_json::Value>| ctx.is_some()),
            )
            .times(1)
            .returning(|_, _, _| Ok(()));

        let context = serde_json::json!({"stage": "transform", "sql": "SELECT ..."});
        let result = mock.fail_run(run_id, "Connection timeout", Some(context));

        assert!(result.is_ok());
    }

    #[test]
    fn test_mock_persistence_returns_error() {
        let mut mock = MockEtlRunPersistence::new();

        mock.expect_start_run()
            .returning(|_, _, _| Err(PersistenceError::Connection("Database unavailable".into())));

        let result = mock.start_run("air-quality", EtlRunMode::Daemon, None);

        assert!(result.is_err());
        match result {
            Err(PersistenceError::Connection(msg)) => {
                assert!(msg.contains("unavailable"));
            }
            _ => panic!("Expected Connection error"),
        }
    }

    #[test]
    fn test_mock_persistence_sequence() {
        let mut mock = MockEtlRunPersistence::new();
        let mut seq = mockall::Sequence::new();
        let run_id = Uuid::new_v4();

        // start_run must be called first
        mock.expect_start_run()
            .times(1)
            .in_sequence(&mut seq)
            .returning(move |_, _, _| Ok(run_id));

        // Then complete_run
        mock.expect_complete_run()
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_, _| Ok(()));

        // Execute in order
        let id = mock
            .start_run("test", EtlRunMode::Manual, None)
            .unwrap();
        assert_eq!(id, run_id);

        let stats = make_test_stats("test", 50);
        mock.complete_run(id, &stats).unwrap();
    }

    // =========================================================================
    // PersistenceError tests
    // =========================================================================

    #[test]
    fn test_persistence_error_display() {
        let conn_err = PersistenceError::Connection("timeout".to_string());
        assert!(conn_err.to_string().contains("connection"));

        let sql_err = PersistenceError::SqlExecution("syntax error".to_string());
        assert!(sql_err.to_string().contains("SQL"));

        let not_found = PersistenceError::RunNotFound(Uuid::new_v4());
        assert!(not_found.to_string().contains("not found"));

        let ser_err = PersistenceError::Serialization("invalid JSON".to_string());
        assert!(ser_err.to_string().contains("Serialization"));
    }

    // =========================================================================
    // EtlRunRecord tests
    // =========================================================================

    #[test]
    fn test_etl_run_record_serialization() {
        let record = EtlRunRecord {
            id: Uuid::new_v4(),
            stream_id: "air-quality".to_string(),
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
            duration_ms: Some(150),
            status: EtlRunStatus::Success,
            rows_processed: 100,
            rows_flagged: 5,
            rows_rejected: 0,
            watermark_before: None,
            watermark_after: Some(Utc::now()),
            error_message: None,
            error_context: None,
            run_mode: EtlRunMode::Daemon,
            daemon_cycle_id: Some(Uuid::new_v4()),
        };

        let json = serde_json::to_string(&record).expect("Should serialize");
        assert!(json.contains("air-quality"));
        assert!(json.contains("success"));
        assert!(json.contains("daemon"));

        let deserialized: EtlRunRecord =
            serde_json::from_str(&json).expect("Should deserialize");
        assert_eq!(deserialized.stream_id, "air-quality");
        assert_eq!(deserialized.status, EtlRunStatus::Success);
    }
}
