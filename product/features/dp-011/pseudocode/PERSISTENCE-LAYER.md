# Persistence Layer Pseudocode

**Feature**: dp-011 - Silver ETL Run Statistics Persistence
**Component**: Core Persistence Trait and Implementation
**Author**: ndp-rust-dev
**Created**: 2026-01-16

---

## Overview

This document defines the pseudocode for the ETL run persistence layer. The design follows NDP's Domain Adapter pattern with a trait-based abstraction for testability.

---

## Data Structures

### EtlRunStatus Enum

```
ENUM EtlRunStatus:
    Running     # ETL in progress
    Success     # Completed without errors
    Failed      # Completed with errors
    Partial     # Some streams in cycle failed (daemon mode)
```

### EtlRunMode Enum

```
ENUM EtlRunMode:
    Daemon      # Scheduled daemon execution
    Manual      # CLI manual run
    Backfill    # Historical data reprocessing
```

### EtlRunRecord Struct

```
STRUCT EtlRunRecord:
    id: UUID                              # Primary key
    stream_id: String                     # Stream identifier
    started_at: DateTime<Utc>             # Run start time
    completed_at: Option<DateTime<Utc>>   # Run completion time (null if running)
    duration_ms: Option<i64>              # Duration in milliseconds
    status: EtlRunStatus                  # Current status
    rows_processed: i64                   # Rows written to Silver
    rows_flagged: i64                     # Rows with DQ flags
    rows_rejected: i64                    # Rows rejected by DQ rules
    watermark_before: Option<DateTime>    # Watermark before ETL
    watermark_after: Option<DateTime>     # Watermark after ETL
    error_message: Option<String>         # Error message if failed
    error_context: Option<JSON>           # Structured error details
    run_mode: EtlRunMode                  # How run was triggered
    daemon_cycle_id: Option<UUID>         # Links runs in same cycle
```

---

## Trait Definition

### EtlRunPersistence Trait

```
TRAIT EtlRunPersistence (Send + Sync):

    # Insert a new run record with status='running'
    # Returns: UUID of the created run record
    FUNCTION start_run(
        stream_id: &str,
        run_mode: EtlRunMode,
        daemon_cycle_id: Option<UUID>
    ) -> Result<UUID, PersistenceError>

    # Update run with success status and stats
    FUNCTION complete_run(
        run_id: UUID,
        stats: &EtlStats
    ) -> Result<(), PersistenceError>

    # Update run with failure status and error
    FUNCTION fail_run(
        run_id: UUID,
        error_message: &str,
        error_context: Option<JSON>
    ) -> Result<(), PersistenceError>
```

---

## Implementation: DuckDbRunPersistence

Uses DuckDB's PostgreSQL extension (already attached in EtlRunner).

### Constructor

```
FUNCTION new(conn: &Connection) -> Self:
    1. VERIFY conn has postgres attached:
       result = conn.execute("SELECT 1 FROM pg.silver.etl_runs LIMIT 1")
       IF result.is_err() AND error.contains("does not exist"):
           RETURN Error(PersistenceError::TableNotFound("silver.etl_runs"))

    2. RETURN Self { conn }
```

### start_run Implementation

```
FUNCTION start_run(
    stream_id: &str,
    run_mode: EtlRunMode,
    daemon_cycle_id: Option<UUID>
) -> Result<UUID, PersistenceError>:

    1. Generate unique identifier:
       run_id = UUID::new_v4()

    2. Convert run_mode to string:
       mode_str = MATCH run_mode:
           Daemon => "daemon"
           Manual => "manual"
           Backfill => "backfill"

    3. Build INSERT statement:
       sql = """
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
       """

    4. Execute INSERT:
       TRY:
           conn.execute(sql, [
               run_id.to_string(),
               stream_id,
               mode_str,
               daemon_cycle_id.map(|id| id.to_string())
           ])
       CATCH e:
           error!(error = %e, stream_id = %stream_id, "Failed to start run record")
           RETURN Error(PersistenceError::InsertFailed(e.to_string()))

    5. Log success:
       debug!(
           run_id = %run_id,
           stream_id = %stream_id,
           run_mode = %mode_str,
           "Started ETL run record"
       )

    6. RETURN Ok(run_id)
```

### complete_run Implementation

```
FUNCTION complete_run(
    run_id: UUID,
    stats: &EtlStats
) -> Result<(), PersistenceError>:

    1. Build UPDATE statement:
       sql = """
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
       """

    2. Convert watermarks to RFC3339 strings (or NULL):
       wm_before = stats.watermark_before.map(|dt| dt.to_rfc3339())
       wm_after = stats.watermark_after.map(|dt| dt.to_rfc3339())

    3. Execute UPDATE:
       TRY:
           rows_affected = conn.execute(sql, [
               run_id.to_string(),
               stats.duration_ms as i64,
               stats.rows_processed as i64,
               stats.rows_with_dq_flags as i64,
               stats.rows_rejected as i64,
               wm_before,
               wm_after
           ])
       CATCH e:
           error!(error = %e, run_id = %run_id, "Failed to complete run record")
           RETURN Error(PersistenceError::UpdateFailed(e.to_string()))

    4. Verify row was updated:
       IF rows_affected == 0:
           warn!(run_id = %run_id, "No run record found to complete")
           RETURN Error(PersistenceError::RunNotFound(run_id))

    5. Log success:
       info!(
           run_id = %run_id,
           stream_id = %stats.stream_id,
           rows_processed = stats.rows_processed,
           duration_ms = stats.duration_ms,
           "Completed ETL run record"
       )

    6. RETURN Ok(())
```

### fail_run Implementation

```
FUNCTION fail_run(
    run_id: UUID,
    error_message: &str,
    error_context: Option<JSON>
) -> Result<(), PersistenceError>:

    1. Serialize error_context to JSON string (or NULL):
       context_json = error_context.map(|ctx| serde_json::to_string(&ctx))

    2. Build UPDATE statement:
       sql = """
           UPDATE pg.silver.etl_runs
           SET
               completed_at = NOW(),
               duration_ms = EXTRACT(EPOCH FROM (NOW() - started_at)) * 1000,
               status = 'failed',
               error_message = $2,
               error_context = $3::JSONB
           WHERE id = $1::UUID
       """

    3. Execute UPDATE:
       TRY:
           rows_affected = conn.execute(sql, [
               run_id.to_string(),
               error_message,
               context_json
           ])
       CATCH e:
           error!(error = %e, run_id = %run_id, "Failed to record run failure")
           RETURN Error(PersistenceError::UpdateFailed(e.to_string()))

    4. Verify row was updated:
       IF rows_affected == 0:
           warn!(run_id = %run_id, "No run record found to fail")
           RETURN Error(PersistenceError::RunNotFound(run_id))

    5. Log failure:
       error!(
           run_id = %run_id,
           error = %error_message,
           "Recorded ETL run failure"
       )

    6. RETURN Ok(())
```

---

## Error Types

```
ENUM PersistenceError:
    InsertFailed(String)      # INSERT operation failed
    UpdateFailed(String)      # UPDATE operation failed
    RunNotFound(UUID)         # Run ID doesn't exist
    TableNotFound(String)     # Table doesn't exist
    ConnectionError(String)   # Database connection issue
```

---

## Mock Implementation (for testing)

```
STRUCT MockRunPersistence:
    runs: Mutex<HashMap<UUID, EtlRunRecord>>

IMPL EtlRunPersistence FOR MockRunPersistence:

    FUNCTION start_run(...) -> Result<UUID, PersistenceError>:
        run_id = UUID::new_v4()
        record = EtlRunRecord {
            id: run_id,
            stream_id: stream_id.to_string(),
            started_at: Utc::now(),
            status: EtlRunStatus::Running,
            ...
        }
        self.runs.lock().insert(run_id, record)
        RETURN Ok(run_id)

    FUNCTION complete_run(run_id, stats) -> Result<(), PersistenceError>:
        IF let Some(record) = self.runs.lock().get_mut(&run_id):
            record.completed_at = Some(Utc::now())
            record.status = EtlRunStatus::Success
            record.rows_processed = stats.rows_processed as i64
            ...
            RETURN Ok(())
        ELSE:
            RETURN Error(PersistenceError::RunNotFound(run_id))

    FUNCTION fail_run(run_id, error, context) -> Result<(), PersistenceError>:
        IF let Some(record) = self.runs.lock().get_mut(&run_id):
            record.completed_at = Some(Utc::now())
            record.status = EtlRunStatus::Failed
            record.error_message = Some(error.to_string())
            record.error_context = context
            RETURN Ok(())
        ELSE:
            RETURN Error(PersistenceError::RunNotFound(run_id))
```

---

## Usage Pattern

```
# Typical usage in ETL flow:

persistence = DuckDbRunPersistence::new(&conn)?

# 1. Start run
run_id = persistence.start_run("air-quality", EtlRunMode::Daemon, Some(cycle_id))?

# 2. Execute ETL
TRY:
    stats = etl_runner.run_etl(&config, stream_id, bronze_path)?

    # 3a. Record success
    persistence.complete_run(run_id, &stats)?

CATCH error:
    # 3b. Record failure with context
    context = json!({
        "stage": "transform",
        "sql": sql_statement,
        "parquet_files": file_list
    })
    persistence.fail_run(run_id, &error.to_string(), Some(context))?

    # Re-throw for caller handling
    RETHROW error
```

---

## Considerations

### Thread Safety

- DuckDB Connection is NOT Sync, only Send
- Persistence methods take `&self` (immutable reference)
- DuckDB handles internal synchronization for single connection
- For multi-threaded access, wrap in `Mutex<Connection>`

### Connection Management

- Option A: Reuse EtlRunner's existing connection (tight coupling)
- Option B: Separate connection for persistence (isolation, recommended)
- Option C: Connection pool (overkill for Pi deployment)

Recommendation: **Option B** - Create persistence with dedicated connection for isolation.

### Transaction Handling

- Each operation is atomic (single statement)
- No explicit transactions needed
- If start_run succeeds but ETL fails before fail_run:
  - Record stays in 'running' state
  - Cleanup job can mark stale 'running' records as 'failed'

---

*Pseudocode created: 2026-01-16*
*Next: DAEMON-INTEGRATION.md*
