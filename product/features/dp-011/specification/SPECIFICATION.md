# DP-011: Silver ETL Run Statistics Persistence - SPARC Specification

**Feature ID**: dp-011
**Version**: 1.0.0
**Created**: 2026-01-16
**Author**: ndp-rust-dev
**Status**: Specification Complete

---

## Executive Summary

This specification defines the requirements, interfaces, and contracts for persisting ETL run statistics to TimescaleDB. It enables queryable ETL operational history for the MCP `etl_status`, `etl_history`, and `data_freshness` tools defined in dp-010.

---

## Table of Contents

1. [Functional Requirements](#functional-requirements)
2. [Non-Functional Requirements](#non-functional-requirements)
3. [Interface Contracts](#interface-contracts)
4. [Data Contracts](#data-contracts)
5. [Error Handling](#error-handling)
6. [Dependencies](#dependencies)
7. [Implementation Guidelines](#implementation-guidelines)
8. [Acceptance Criteria](#acceptance-criteria)

---

## Functional Requirements

### FR-001: Persist All ETL Run Statistics

**Requirement**: The system SHALL persist all ETL run statistics to the `silver.etl_runs` table after each ETL execution.

**Rationale**: Enables historical tracking, debugging, and MCP tool queries.

**Verification**: `SELECT COUNT(*) FROM silver.etl_runs WHERE stream_id = 'air-quality'` returns non-zero after ETL runs.

---

### FR-002: Record Run Timing

**Requirement**: The system SHALL record:
- `started_at`: Timestamp when ETL execution began (TIMESTAMPTZ)
- `completed_at`: Timestamp when ETL execution completed (TIMESTAMPTZ, nullable for in-progress runs)
- `duration_ms`: Total execution time in milliseconds (BIGINT, computed on completion)

**Source Mapping**:
| Current Code | Target Column |
|--------------|---------------|
| `Instant::now()` at start | `started_at` |
| `Instant::now()` at end | `completed_at` |
| `start.elapsed().as_millis()` | `duration_ms` |

**Verification**: Query `SELECT started_at, completed_at, duration_ms FROM silver.etl_runs` shows valid timing data.

---

### FR-003: Record Run Status

**Requirement**: The system SHALL record the run status as one of:
- `running`: ETL execution in progress
- `success`: ETL completed without errors
- `failed`: ETL completed with errors
- `partial`: Daemon cycle completed but some streams failed (daemon-level status)

**State Transitions**:
```
start_run() -> status = 'running'
complete_run() -> status = 'success'
fail_run() -> status = 'failed'
```

**Verification**: `SELECT DISTINCT status FROM silver.etl_runs` returns valid enum values.

---

### FR-004: Record Row Statistics

**Requirement**: The system SHALL record row processing statistics:
- `rows_processed`: Total rows transformed (BIGINT)
- `rows_flagged`: Rows with DQ flags applied (BIGINT)
- `rows_rejected`: Rows rejected by DQ rules (BIGINT)

**Source Mapping** (from `EtlStats`):
| EtlStats Field | Target Column |
|----------------|---------------|
| `rows_processed` (u64) | `rows_processed` (BIGINT) |
| `rows_with_dq_flags` (u64) | `rows_flagged` (BIGINT) |
| `rows_rejected` (u64) | `rows_rejected` (BIGINT) |

**Verification**: Sum of `rows_processed` across runs matches Silver table row count.

---

### FR-005: Record Watermarks

**Requirement**: The system SHALL record incremental load watermarks:
- `watermark_before`: MAX(watermark_column) before ETL run (TIMESTAMPTZ, nullable)
- `watermark_after`: MAX(watermark_column) after ETL run (TIMESTAMPTZ, nullable)

**Source Mapping**:
| EtlStats Field | Target Column |
|----------------|---------------|
| `watermark_before` (Option<DateTime<Utc>>) | `watermark_before` |
| `watermark_after` (Option<DateTime<Utc>>) | `watermark_after` |

**Nullable Conditions**:
- First run for a stream (no prior data)
- Backfill runs with explicit time bounds
- Failed runs that didn't reach watermark query

**Verification**: Watermark progression is monotonically increasing for successful runs.

---

### FR-006: Record Errors

**Requirement**: The system SHALL record error information for failed runs:
- `error_message`: Human-readable error description (TEXT, nullable)
- `error_context`: Structured error details (JSONB, nullable)

**Error Context Schema**:
```json
{
  "stage": "transform" | "load" | "watermark" | "parquet_read",
  "sql": "SELECT ... (truncated)",
  "parquet_files": ["file1.parquet", "file2.parquet"],
  "underlying_error": "detailed error message"
}
```

**Verification**: Failed runs have non-null `error_message`.

---

### FR-007: Link Daemon Cycle Runs

**Requirement**: The system SHALL link runs from the same daemon cycle via `daemon_cycle_id` (UUID).

**Behavior**:
- Daemon generates a new UUID at cycle start
- All stream runs within that cycle share the same `daemon_cycle_id`
- Manual/backfill runs have `daemon_cycle_id = NULL`

**Source**: New field in `DaemonRunner::run_cycle()`.

**Verification**: `SELECT COUNT(DISTINCT stream_id) FROM silver.etl_runs WHERE daemon_cycle_id = '<uuid>'` equals expected stream count.

---

### FR-008: Support Run Modes

**Requirement**: The system SHALL record the execution mode:
- `daemon`: Automated periodic execution
- `manual`: Human-triggered single run
- `backfill`: Historical data reprocessing

**Default**: `daemon` (matches current production usage).

**Source Mapping**:
| Invocation | run_mode |
|------------|----------|
| `silver-etl daemon` | `daemon` |
| `silver-etl run --stream X` | `manual` |
| `silver-etl run --stream X --since/--until` | `backfill` |

**Verification**: `SELECT DISTINCT run_mode FROM silver.etl_runs` returns valid enum values.

---

## Non-Functional Requirements

### NFR-001: Non-Blocking Persistence

**Requirement**: Persistence operations SHALL NOT block ETL execution.

**Implementation Strategy**:
- Use fire-and-forget pattern for `start_run()`
- `complete_run()`/`fail_run()` execute after ETL data is committed
- Persistence failures are logged but do not fail the ETL run

**Rationale**: ETL data integrity is more important than observability metadata.

**Verification**: ETL duration with persistence enabled is within 5% of baseline.

---

### NFR-002: Graceful Degradation

**Requirement**: Persistence failures SHALL log warnings but NOT fail the ETL run.

**Error Handling**:
```rust
match persistence.complete_run(run_id, &stats) {
    Ok(()) => debug!("Run stats persisted"),
    Err(e) => warn!(error = %e, "Failed to persist run stats - continuing"),
}
```

**Rationale**: Observability is supplementary; ETL data delivery is primary.

**Verification**: ETL succeeds even when TimescaleDB is temporarily unavailable for stats writes.

---

### NFR-003: Scale Support

**Requirement**: The system SHALL support:
- 20+ streams
- 5-minute intervals (288 runs/day/stream)
- ~5,760 runs/day total

**Storage Estimate**:
- ~200 bytes per row
- 30 days retention: 172,800 rows = ~35 MB

**Verification**: System performs acceptably with 100,000+ rows in `silver.etl_runs`.

---

### NFR-004: 30-Day Retention

**Requirement**: Run records older than 30 days SHALL be automatically deleted.

**Implementation**: Scheduled DELETE or pg_cron job:
```sql
DELETE FROM silver.etl_runs WHERE created_at < NOW() - INTERVAL '30 days';
```

**Frequency**: Daily at 03:00 UTC (low-activity period).

**Verification**: `SELECT MIN(created_at) FROM silver.etl_runs` is within 30 days of NOW().

---

## Interface Contracts

### EtlRunPersistence Trait

The core trait for ETL run persistence, following the Domain Adapter pattern and London School TDD approach used throughout NDP.

```rust
//! ETL run persistence trait
//!
//! File: apps/silver-etl/src/persistence.rs
//!
//! Provides abstraction for persisting ETL run statistics.
//! Implementations: DuckDbRunPersistence (prod), MockEtlRunPersistence (test)

use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

use crate::etl::EtlStats;

/// Run execution mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtlRunMode {
    /// Automated periodic execution
    Daemon,
    /// Human-triggered single run
    Manual,
    /// Historical data reprocessing
    Backfill,
}

impl EtlRunMode {
    /// Convert to database string value
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Daemon => "daemon",
            Self::Manual => "manual",
            Self::Backfill => "backfill",
        }
    }
}

/// Run execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EtlRunStatus {
    /// ETL execution in progress
    Running,
    /// ETL completed without errors
    Success,
    /// ETL completed with errors
    Failed,
    /// Daemon cycle completed but some streams failed
    Partial,
}

impl EtlRunStatus {
    /// Convert to database string value
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Success => "success",
            Self::Failed => "failed",
            Self::Partial => "partial",
        }
    }
}

/// Persistence errors
#[derive(Debug, thiserror::Error)]
pub enum PersistenceError {
    /// Database connection error
    #[error("Database connection error: {0}")]
    Connection(String),

    /// SQL execution error
    #[error("SQL execution error: {0}")]
    SqlExecution(String),

    /// Run not found (invalid UUID)
    #[error("Run not found: {0}")]
    RunNotFound(Uuid),

    /// Serialization error (for error_context JSONB)
    #[error("Serialization error: {0}")]
    Serialization(String),
}

/// Trait for ETL run persistence (mockable for London TDD)
///
/// Implementations must be Send + Sync for use in async contexts.
/// Uses the Domain Adapter pattern - production uses DuckDB's postgres extension,
/// tests use MockEtlRunPersistence.
#[cfg_attr(test, mockall::automock)]
pub trait EtlRunPersistence: Send + Sync {
    /// Start a new run record
    ///
    /// Creates a record with status = 'running' and returns the run UUID.
    /// Called at the beginning of ETL execution.
    ///
    /// # Arguments
    /// * `stream_id` - The stream being processed (e.g., "air-quality")
    /// * `run_mode` - Execution mode (daemon, manual, backfill)
    /// * `daemon_cycle_id` - Optional UUID linking runs from same daemon cycle
    ///
    /// # Returns
    /// * `Ok(Uuid)` - The new run's unique identifier
    /// * `Err(PersistenceError)` - If INSERT fails
    fn start_run(
        &self,
        stream_id: &str,
        run_mode: EtlRunMode,
        daemon_cycle_id: Option<Uuid>,
    ) -> Result<Uuid, PersistenceError>;

    /// Complete a run with success status
    ///
    /// Updates the run record with completion timestamp, duration, and statistics.
    /// Called after successful ETL execution.
    ///
    /// # Arguments
    /// * `run_id` - UUID from start_run()
    /// * `stats` - ETL execution statistics
    ///
    /// # Returns
    /// * `Ok(())` - If UPDATE succeeds
    /// * `Err(PersistenceError)` - If UPDATE fails or run not found
    fn complete_run(&self, run_id: Uuid, stats: &EtlStats) -> Result<(), PersistenceError>;

    /// Mark a run as failed
    ///
    /// Updates the run record with failure status, error message, and optional context.
    /// Called when ETL execution fails.
    ///
    /// # Arguments
    /// * `run_id` - UUID from start_run()
    /// * `error_message` - Human-readable error description
    /// * `error_context` - Optional structured error details (JSONB)
    ///
    /// # Returns
    /// * `Ok(())` - If UPDATE succeeds
    /// * `Err(PersistenceError)` - If UPDATE fails or run not found
    fn fail_run(
        &self,
        run_id: Uuid,
        error_message: &str,
        error_context: Option<Value>,
    ) -> Result<(), PersistenceError>;
}
```

### NoOpPersistence Implementation

For backwards compatibility and testing scenarios where persistence is disabled:

```rust
/// No-op implementation for when persistence is disabled
pub struct NoOpPersistence;

impl EtlRunPersistence for NoOpPersistence {
    fn start_run(
        &self,
        _stream_id: &str,
        _run_mode: EtlRunMode,
        _daemon_cycle_id: Option<Uuid>,
    ) -> Result<Uuid, PersistenceError> {
        Ok(Uuid::new_v4())
    }

    fn complete_run(&self, _run_id: Uuid, _stats: &EtlStats) -> Result<(), PersistenceError> {
        Ok(())
    }

    fn fail_run(
        &self,
        _run_id: Uuid,
        _error_message: &str,
        _error_context: Option<Value>,
    ) -> Result<(), PersistenceError> {
        Ok(())
    }
}
```

---

## Data Contracts

### EtlRunRecord Struct

Complete record representation for database operations:

```rust
/// Complete ETL run record
///
/// Represents a single row in silver.etl_runs table.
/// Used for queries and result mapping.
#[derive(Debug, Clone)]
pub struct EtlRunRecord {
    /// Unique run identifier
    pub id: Uuid,

    /// Stream that was processed
    pub stream_id: String,

    /// When ETL execution began
    pub started_at: DateTime<Utc>,

    /// When ETL execution completed (None if still running)
    pub completed_at: Option<DateTime<Utc>>,

    /// Execution duration in milliseconds (None if still running)
    pub duration_ms: Option<i64>,

    /// Current run status
    pub status: EtlRunStatus,

    /// Total rows transformed
    pub rows_processed: i64,

    /// Rows with DQ flags applied
    pub rows_flagged: i64,

    /// Rows rejected by DQ rules
    pub rows_rejected: i64,

    /// MAX(watermark_column) before ETL run
    pub watermark_before: Option<DateTime<Utc>>,

    /// MAX(watermark_column) after ETL run
    pub watermark_after: Option<DateTime<Utc>>,

    /// Error description for failed runs
    pub error_message: Option<String>,

    /// Structured error details
    pub error_context: Option<Value>,

    /// Execution mode
    pub run_mode: EtlRunMode,

    /// UUID linking runs from same daemon cycle
    pub daemon_cycle_id: Option<Uuid>,
}
```

### EtlStats to EtlRunRecord Mapping

```rust
impl EtlRunRecord {
    /// Create a completed run record from EtlStats
    pub fn from_stats(
        run_id: Uuid,
        stats: &EtlStats,
        started_at: DateTime<Utc>,
        run_mode: EtlRunMode,
        daemon_cycle_id: Option<Uuid>,
    ) -> Self {
        let completed_at = Utc::now();
        let duration_ms = (completed_at - started_at).num_milliseconds();

        Self {
            id: run_id,
            stream_id: stats.stream_id.clone(),
            started_at,
            completed_at: Some(completed_at),
            duration_ms: Some(duration_ms),
            status: EtlRunStatus::Success,
            rows_processed: stats.rows_processed as i64,
            rows_flagged: stats.rows_with_dq_flags as i64,
            rows_rejected: stats.rows_rejected as i64,
            watermark_before: stats.watermark_before,
            watermark_after: stats.watermark_after,
            error_message: None,
            error_context: None,
            run_mode,
            daemon_cycle_id,
        }
    }
}
```

### Database Schema Contract

The following SQL schema is the contract that implementations must target:

```sql
-- silver.etl_runs table (from dp-010 ETL-STATUS-SPEC.md)
CREATE TABLE IF NOT EXISTS silver.etl_runs (
    -- Run identification
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    stream_id           TEXT NOT NULL,

    -- Timing
    started_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at        TIMESTAMPTZ,
    duration_ms         BIGINT,

    -- Status
    status              TEXT NOT NULL DEFAULT 'running'
                        CHECK (status IN ('running', 'success', 'failed', 'partial')),

    -- Statistics
    rows_processed      BIGINT NOT NULL DEFAULT 0,
    rows_flagged        BIGINT NOT NULL DEFAULT 0,
    rows_rejected       BIGINT NOT NULL DEFAULT 0,

    -- Watermarks
    watermark_before    TIMESTAMPTZ,
    watermark_after     TIMESTAMPTZ,

    -- Error tracking
    error_message       TEXT,
    error_context       JSONB,

    -- Metadata
    run_mode            TEXT DEFAULT 'daemon'
                        CHECK (run_mode IN ('daemon', 'manual', 'backfill')),
    daemon_cycle_id     UUID,

    -- Housekeeping
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

---

## Error Handling

### PersistenceError Variants

| Variant | Cause | Recovery |
|---------|-------|----------|
| `Connection` | TimescaleDB unreachable | Log warning, continue ETL |
| `SqlExecution` | INSERT/UPDATE failed | Log warning, continue ETL |
| `RunNotFound` | Invalid run_id in complete/fail | Log error (bug indicator) |
| `Serialization` | error_context JSONB serialization failed | Log warning, use None |

### Graceful Degradation Strategy

```rust
/// Execute ETL with optional persistence
fn run_etl_with_persistence(
    &self,
    config: &SilverEtlConfig,
    stream_id: &str,
    bronze_dir: &str,
    persistence: &dyn EtlRunPersistence,
    run_mode: EtlRunMode,
    daemon_cycle_id: Option<Uuid>,
) -> Result<EtlStats, EtlError> {
    // 1. Start run record (fire-and-forget on failure)
    let run_id = persistence
        .start_run(stream_id, run_mode, daemon_cycle_id)
        .unwrap_or_else(|e| {
            warn!(error = %e, stream_id, "Failed to start run record");
            Uuid::nil()  // Sentinel value indicating persistence failure
        });

    // 2. Execute actual ETL
    let result = self.run_etl(config, stream_id, bronze_dir);

    // 3. Record result (skip if start_run failed)
    if !run_id.is_nil() {
        match &result {
            Ok(stats) => {
                if let Err(e) = persistence.complete_run(run_id, stats) {
                    warn!(error = %e, run_id = %run_id, "Failed to complete run record");
                }
            }
            Err(e) => {
                let error_context = serde_json::json!({
                    "error_type": format!("{:?}", e),
                });
                if let Err(pe) = persistence.fail_run(run_id, &e.to_string(), Some(error_context)) {
                    warn!(error = %pe, run_id = %run_id, "Failed to record run failure");
                }
            }
        }
    }

    result
}
```

---

## Dependencies

### Required Crates

| Crate | Version | Purpose |
|-------|---------|---------|
| `uuid` | 1.x | Run ID generation |
| `chrono` | 0.4.x | Timestamp handling |
| `serde_json` | 1.x | error_context JSONB serialization |
| `thiserror` | 1.x | PersistenceError derive |
| `mockall` | 0.13.x | Test mock generation (dev) |

### Database Requirements

- **TimescaleDB**: Target for persistence writes
- **DuckDB postgres extension**: Used for writes from ETL process

### Connection Strategy

The implementation uses DuckDB's postgres extension (already loaded for Silver writes) to INSERT into `silver.etl_runs`:

```sql
-- Via DuckDB postgres extension
INSERT INTO postgres_db.silver.etl_runs (id, stream_id, status, run_mode, daemon_cycle_id)
VALUES ($1, $2, 'running', $3, $4);

UPDATE postgres_db.silver.etl_runs
SET completed_at = NOW(),
    duration_ms = $2,
    status = 'success',
    rows_processed = $3,
    rows_flagged = $4,
    rows_rejected = $5,
    watermark_before = $6,
    watermark_after = $7
WHERE id = $1;
```

---

## Implementation Guidelines

### File Organization

```
apps/silver-etl/src/
├── persistence.rs      # NEW: EtlRunPersistence trait + DuckDbRunPersistence
├── etl.rs              # MODIFY: Add persistence parameter to run_etl
├── daemon.rs           # MODIFY: Use persistence in run_cycle
├── main.rs             # MODIFY: Wire up persistence
└── lib.rs              # MODIFY: Export persistence module
```

### Integration Points

1. **EtlRunner::run_etl()** - Add optional persistence parameter
2. **DaemonRunner::run_cycle()** - Generate cycle_id, call persistence for each stream
3. **main.rs daemon command** - Instantiate DuckDbRunPersistence

### Testing Strategy

Following London School TDD with mockall:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::*;

    #[test]
    fn test_persistence_called_on_success() {
        let mut mock = MockEtlRunPersistence::new();

        // Expect start_run
        mock.expect_start_run()
            .with(eq("air-quality"), eq(EtlRunMode::Daemon), always())
            .times(1)
            .returning(|_, _, _| Ok(Uuid::new_v4()));

        // Expect complete_run
        mock.expect_complete_run()
            .times(1)
            .returning(|_, _| Ok(()));

        // Execute with mock
        // ...
    }

    #[test]
    fn test_etl_continues_on_persistence_failure() {
        let mut mock = MockEtlRunPersistence::new();

        mock.expect_start_run()
            .returning(|_, _, _| Err(PersistenceError::Connection("test".into())));

        // ETL should still succeed
        // ...
    }
}
```

---

## Acceptance Criteria

### AC-001: Run Records Created

**Given**: ETL daemon is running
**When**: A 5-minute cycle completes
**Then**: `silver.etl_runs` contains one row per enabled stream with `daemon_cycle_id` linking them

### AC-002: Statistics Accurate

**Given**: ETL processes 100 rows for `air-quality`
**When**: Run completes successfully
**Then**: `rows_processed = 100` in the corresponding `silver.etl_runs` row

### AC-003: Errors Captured

**Given**: ETL fails with SQL execution error
**When**: Run is marked failed
**Then**: `error_message` contains error text, `error_context` contains structured details

### AC-004: Graceful Degradation

**Given**: TimescaleDB is temporarily unavailable for stats writes
**When**: ETL executes
**Then**: ETL data is written successfully, warning logged for persistence failure

### AC-005: MCP Tools Functional

**Given**: `silver.etl_runs` contains data
**When**: MCP `etl_status` tool is invoked
**Then**: Tool returns latest run status and 24h summary (dp-010 dependency)

### AC-006: Retention Working

**Given**: 31-day old run records exist
**When**: Retention job runs
**Then**: Old records are deleted, recent records preserved

---

## References

- [dp-010 ETL-STATUS-SPEC.md](/workspaces/neural-data-platform/product/features/dp-010/specification/ETL-STATUS-SPEC.md) - Schema and MCP tools specification
- [dp-011 SCOPE.md](/workspaces/neural-data-platform/product/features/dp-011/SCOPE.md) - Feature scope definition
- [EtlStats struct](/workspaces/neural-data-platform/apps/silver-etl/src/etl.rs) - Current statistics implementation (line 893-914)
- [daemon.rs](/workspaces/neural-data-platform/apps/silver-etl/src/daemon.rs) - Current daemon implementation
- [etl-observability-schema pattern](#) - AgentDB pattern for schema design
- [mcp-tool-testing-pattern](#) - AgentDB pattern for mockall testing

---

*Specification created: 2026-01-16*
*Ready for: Pseudocode phase (SPARC P)*
