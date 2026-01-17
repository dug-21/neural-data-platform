# ADR-003: ETL Run Record Lifecycle

## Status

Accepted

## Context

When persisting ETL run statistics to `silver.etl_runs`, we need to decide:

1. **When to INSERT**: At start of run (status='running') or at end?
2. **When to UPDATE**: How to handle completion/failure?
3. **How to handle crashes**: What about orphaned 'running' records?
4. **How to link daemon cycles**: Multiple streams in one tick share a cycle ID?

### Current Daemon Flow

```rust
// daemon.rs run_cycle()
for stream_id in &streams {
    match executor.run_stream(stream_id) {
        Ok(stats) => { /* success handling */ }
        Err(e) => { /* failure handling */ }
    }
}
```

Each `run_stream()` call:
1. Loads config from etcd
2. Calls `runner.run_etl()`
3. Returns `EtlStats` or error

## Decision

**Adopt Two-Phase Lifecycle with daemon_cycle_id Linking**

### Phase 1: Start Record (Before ETL)

```sql
INSERT INTO silver.etl_runs (stream_id, status, run_mode, daemon_cycle_id)
VALUES ($1, 'running', $2, $3)
RETURNING id;
```

- Creates record with `status='running'`
- Captures `started_at` timestamp
- Links to `daemon_cycle_id` if in daemon mode
- Returns `id` for Phase 2 update

### Phase 2: Complete Record (After ETL)

**On Success:**
```sql
UPDATE silver.etl_runs
SET
    completed_at = NOW(),
    duration_ms = $2,
    status = 'success',
    rows_processed = $3,
    rows_flagged = $4,
    rows_rejected = $5,
    watermark_before = $6,
    watermark_after = $7
WHERE id = $1;
```

**On Failure:**
```sql
UPDATE silver.etl_runs
SET
    completed_at = NOW(),
    duration_ms = $2,
    status = 'failed',
    error_message = $3,
    error_context = $4
WHERE id = $1;
```

### Daemon Cycle Linking

Each daemon tick (run_cycle) generates a new `daemon_cycle_id`:

```rust
fn run_cycle(&self) -> Result<DaemonCycleStats, DaemonError> {
    let cycle_id = Uuid::new_v4();  // Shared across all streams in this tick

    for stream_id in &streams {
        let run_id = persistence.start_run(stream_id, EtlRunMode::Daemon, Some(cycle_id))?;
        // ... ETL execution ...
    }
}
```

Benefits:
- Query all runs from a single daemon tick: `WHERE daemon_cycle_id = $1`
- Correlate failures across streams
- Identify partial cycle completions

### State Diagram

```
                    ┌─────────────────────┐
                    │                     │
      INSERT        │     RUNNING         │
    ──────────────▶ │                     │
                    │ - started_at set    │
                    │ - daemon_cycle_id   │
                    └──────────┬──────────┘
                               │
                 ┌─────────────┴─────────────┐
                 │                           │
        ETL Success                     ETL Failure
                 │                           │
                 ▼                           ▼
    ┌────────────────────┐      ┌────────────────────┐
    │                    │      │                    │
    │     SUCCESS        │      │     FAILED         │
    │                    │      │                    │
    │ - completed_at     │      │ - completed_at     │
    │ - rows_processed   │      │ - error_message    │
    │ - watermarks       │      │ - error_context    │
    └────────────────────┘      └────────────────────┘


    Special: If stream A fails, stream B succeeds in same cycle:

    ┌────────────────────┐
    │                    │
    │     PARTIAL        │ ◄── Set on cycle stats (not individual run)
    │                    │
    │ (Informational)    │
    └────────────────────┘
```

### Handling Orphaned 'running' Records

**Problem**: If silver-etl crashes mid-execution, records may be left in 'running' status.

**Solution 1: Startup Cleanup**

```rust
// On daemon startup, clean up stale running records
impl PostgresRunPersistence {
    pub async fn cleanup_stale_runs(&self, stale_threshold: Duration) -> Result<u64, Error> {
        let query = "
            UPDATE silver.etl_runs
            SET status = 'failed',
                completed_at = NOW(),
                error_message = 'Process terminated unexpectedly (orphan cleanup)'
            WHERE status = 'running'
              AND started_at < NOW() - $1::INTERVAL
        ";
        // stale_threshold: 15 minutes (3x normal interval)
    }
}
```

**Solution 2: Scheduled Cleanup (pg_cron)**

```sql
-- Run hourly, mark any 'running' > 30 minutes as failed
SELECT cron.schedule('orphan_run_cleanup', '0 * * * *', $$
    UPDATE silver.etl_runs
    SET status = 'failed',
        completed_at = NOW(),
        error_message = 'Orphan cleanup: exceeded 30 minute timeout'
    WHERE status = 'running'
      AND started_at < NOW() - INTERVAL '30 minutes'
$$);
```

**Recommendation**: Implement both. Startup cleanup catches most cases; scheduled cleanup catches edge cases (e.g., daemon restarted on different host without accessing old DB).

### Run Modes

| Mode | Description | daemon_cycle_id |
|------|-------------|-----------------|
| `daemon` | Scheduled execution | Set (links runs in same tick) |
| `manual` | `silver-etl run <stream>` CLI | NULL |
| `backfill` | `silver-etl backfill <stream>` | NULL |

## Consequences

### Benefits

1. **Visibility into in-progress runs** - Query `WHERE status = 'running'` to see active ETL
2. **Accurate duration** - `completed_at - started_at` captures full execution time
3. **Failure forensics** - Even if ETL fails early, we have a record with start time
4. **Cycle correlation** - Link all stream runs from same daemon tick

### Costs

1. **Two database operations per run** - INSERT then UPDATE (vs single INSERT at end)
2. **Orphan management** - Need cleanup logic for crash scenarios
3. **Slightly more complex code** - Track run_id through execution

### Trade-offs

| Trade-off | Decision |
|-----------|----------|
| Two writes vs one | Accept: Better observability worth extra write |
| Orphan cleanup complexity | Accept: Simple startup hook + scheduled job |
| Status enum flexibility | Accept: TEXT allows future states without migration |

## Alternatives Considered

### Single INSERT at End

```rust
// Only write after ETL completes
let stats = runner.run_etl(...)?;
persistence.record_run(stream_id, &stats, "success")?;
```

**Pros**: Simpler, single write
**Cons**:
- Cannot see in-progress runs
- Cannot track started_at accurately (would be approximated)
- On crash, no record exists at all

**Rejected**: Loses valuable observability for in-flight operations.

### INSERT at Start, No UPDATE on Success

**Considered**: Always INSERT, but for success just leave status='running' and set stats.

**Rejected**: Confusing semantics - 'running' should mean in-progress, not "completed but we didn't update status".

### Separate Tables for Active vs Historical

**Considered**: `silver.etl_runs_active` for 'running', move to `silver.etl_runs` on completion.

**Rejected**: Over-engineering. Simple status column with index handles this efficiently.

## Implementation Notes

### Error Handling

```rust
// Persistence failure should NOT fail ETL
pub async fn run_stream(&self, stream_id: &str) -> Result<EtlStats, DaemonError> {
    // Phase 1: Try to record start
    let run_id = match self.persistence.start_run(stream_id, mode, cycle_id).await {
        Ok(id) => Some(id),
        Err(e) => {
            warn!(error = %e, "Failed to record run start, continuing without tracking");
            None
        }
    };

    // Execute ETL
    let result = self.runner.run_etl(&config, stream_id, &self.bronze_dir);

    // Phase 2: Try to record completion
    if let Some(id) = run_id {
        let _ = match &result {
            Ok(stats) => self.persistence.complete_run(id, stats).await,
            Err(e) => self.persistence.fail_run(id, &e.to_string(), None).await,
        };
    }

    result.map_err(|e| DaemonError::Etl(e.to_string()))
}
```

### Partial Status

The `partial` status is used at the **cycle level** in MCP responses, not stored per-run:

```rust
// MCP etl_status tool logic
if cycle_has_failures && cycle_has_successes {
    overall_status = "partial"
}
```

Individual runs are always `running`, `success`, or `failed`.

### Indexes for Queries

```sql
-- Find orphan runs
CREATE INDEX idx_etl_runs_running ON silver.etl_runs (started_at)
WHERE status = 'running';

-- Query by daemon cycle
CREATE INDEX idx_etl_runs_daemon_cycle ON silver.etl_runs (daemon_cycle_id)
WHERE daemon_cycle_id IS NOT NULL;
```

## Related ADRs

- ADR-001: Persistence Strategy (what data we persist)
- ADR-002: Connection Management (how we connect)

## References

- [dp-010 ETL Status Specification](../../dp-010/specification/ETL-STATUS-SPEC.md)
- [Current daemon.rs](../../../../apps/silver-etl/src/daemon.rs)
- [EtlStats struct](../../../../apps/silver-etl/src/etl.rs#L892-L914)
