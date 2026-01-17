# ETL Status Schema & MCP Tools Specification

**Feature ID**: dp-010 (specification for dp-011 implementation)
**Version**: 1.1.0
**Created**: 2026-01-16
**Updated**: 2026-01-17
**Author**: ndp-rust-dev

> **Note**: dp-011 is now complete. The `silver.etl_runs` table is operational.
> This specification has been updated to reflect the actual implementation.
> See: `deploy/timescaledb/migrations/003_etl_runs.sql`

---

## Overview

This specification defines the **ETL observability infrastructure** for the Neural Data Platform:

1. **`silver.etl_runs`** - Persistent table for ETL run history
2. **MCP Tools** - `etl_status`, `etl_history`, `data_freshness`
3. **Retention Policy** - Automatic cleanup of old run records

The current implementation (see `apps/silver-etl/src/etl.rs`) computes `EtlStats` but does not persist them. Prometheus metrics exist (`apps/silver-etl/src/metrics.rs`) but are ephemeral. This specification enables:

- Historical ETL run tracking for debugging
- Cross-run trend analysis
- Agent-accessible ETL status via MCP tools

---

## Part 1: Database Schema

### silver.etl_runs Table

```sql
-- =============================================================================
-- Neural Data Platform - ETL Run History Schema
-- =============================================================================
-- Migration: XXX_etl_runs.sql (dp-011)
-- Purpose: Persist ETL execution statistics for observability
-- =============================================================================

CREATE TABLE IF NOT EXISTS silver.etl_runs (
    -- Run identification
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    stream_id           TEXT NOT NULL,

    -- Timing
    started_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at        TIMESTAMPTZ,
    duration_ms         BIGINT,

    -- Status (enum-like TEXT for flexibility)
    -- Values: 'running', 'success', 'failed', 'partial'
    status              TEXT NOT NULL DEFAULT 'running'
                        CHECK (status IN ('running', 'success', 'failed', 'partial')),

    -- Statistics (from EtlStats struct)
    rows_processed      BIGINT NOT NULL DEFAULT 0,
    rows_flagged        BIGINT NOT NULL DEFAULT 0,
    rows_rejected       BIGINT NOT NULL DEFAULT 0,

    -- Watermarks (nullable for first runs / backfills)
    watermark_before    TIMESTAMPTZ,
    watermark_after     TIMESTAMPTZ,

    -- Error tracking
    error_message       TEXT,
    error_context       JSONB,

    -- Metadata
    run_mode            TEXT DEFAULT 'daemon'
                        CHECK (run_mode IN ('daemon', 'manual', 'backfill')),
    daemon_cycle_id     UUID,  -- Links runs from same daemon cycle

    -- Timestamps for housekeeping
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- =============================================================================
-- Indexes
-- =============================================================================

-- Primary query pattern: Get latest run(s) for a stream
CREATE INDEX IF NOT EXISTS idx_etl_runs_stream_started
    ON silver.etl_runs (stream_id, started_at DESC);

-- Query pattern: Filter by status (e.g., find failures)
-- Note: Includes started_at for efficient sorting of filtered results
CREATE INDEX IF NOT EXISTS idx_etl_runs_status
    ON silver.etl_runs (status, started_at DESC);

-- Query pattern: Link daemon cycle runs
CREATE INDEX IF NOT EXISTS idx_etl_runs_cycle
    ON silver.etl_runs (daemon_cycle_id) WHERE daemon_cycle_id IS NOT NULL;

-- =============================================================================
-- Comments (dp-011 implementation uses concise comments)
-- =============================================================================

COMMENT ON TABLE silver.etl_runs IS
    'Tracks Silver ETL execution statistics for operational observability (dp-011)';

COMMENT ON COLUMN silver.etl_runs.daemon_cycle_id IS
    'UUID linking all stream runs within a single daemon tick cycle';

COMMENT ON COLUMN silver.etl_runs.watermark_before IS
    'Max timestamp in target table before ETL run (for incremental tracking)';

COMMENT ON COLUMN silver.etl_runs.watermark_after IS
    'Max timestamp in target table after ETL run (for data freshness tracking)';

COMMENT ON COLUMN silver.etl_runs.error_context IS
    'Structured JSON containing stage, SQL, file list, etc. for debugging failures';
```

### Retention Policy

```sql
-- =============================================================================
-- Retention Policy: Delete runs older than 30 days
-- =============================================================================
-- Run as scheduled job (pg_cron or external scheduler)

-- Manual execution:
DELETE FROM silver.etl_runs
WHERE created_at < NOW() - INTERVAL '30 days';

-- pg_cron setup (if available):
-- SELECT cron.schedule('etl_runs_cleanup', '0 3 * * *',
--     $$DELETE FROM silver.etl_runs WHERE created_at < NOW() - INTERVAL '30 days'$$);

-- Estimated storage:
-- - ~200 bytes per row
-- - 8 streams x 288 runs/day (5-min interval) = 2,304 runs/day
-- - 30 days x 2,304 = 69,120 rows = ~14 MB
-- - Pi-friendly: Well within resource constraints
```

---

## Part 2: MCP Tool Specifications

### Tool 1: `etl_status`

**Purpose**: Get current/latest ETL status for one or all streams.

#### Input Schema (JSON Schema)

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "stream_id": {
      "type": "string",
      "description": "Stream identifier (e.g., 'air-quality'). If omitted, returns status for all streams."
    }
  },
  "additionalProperties": false
}
```

#### Output Schema

```json
{
  "type": "object",
  "required": ["success", "streams"],
  "properties": {
    "success": { "type": "boolean" },
    "streams": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["stream_id", "status", "last_run"],
        "properties": {
          "stream_id": { "type": "string" },
          "status": {
            "type": "string",
            "enum": ["running", "success", "failed", "partial", "never_run"]
          },
          "last_run": {
            "type": ["object", "null"],
            "properties": {
              "id": { "type": "string", "format": "uuid" },
              "started_at": { "type": "string", "format": "date-time" },
              "completed_at": { "type": ["string", "null"], "format": "date-time" },
              "duration_ms": { "type": ["integer", "null"] },
              "rows_processed": { "type": "integer" },
              "rows_flagged": { "type": "integer" },
              "rows_rejected": { "type": "integer" },
              "watermark_before": { "type": ["string", "null"], "format": "date-time" },
              "watermark_after": { "type": ["string", "null"], "format": "date-time" },
              "error_message": { "type": ["string", "null"] }
            }
          },
          "runs_last_24h": {
            "type": "object",
            "properties": {
              "total": { "type": "integer" },
              "succeeded": { "type": "integer" },
              "failed": { "type": "integer" }
            }
          }
        }
      }
    }
  }
}
```

#### Example Request/Response

**Request** (all streams):
```json
{}
```

**Request** (specific stream):
```json
{
  "stream_id": "air-quality"
}
```

**Response**:
```json
{
  "success": true,
  "streams": [
    {
      "stream_id": "air-quality",
      "status": "success",
      "last_run": {
        "id": "550e8400-e29b-41d4-a716-446655440000",
        "started_at": "2026-01-16T21:00:00Z",
        "completed_at": "2026-01-16T21:00:02Z",
        "duration_ms": 2150,
        "rows_processed": 288,
        "rows_flagged": 5,
        "rows_rejected": 0,
        "watermark_before": "2026-01-16T20:55:00Z",
        "watermark_after": "2026-01-16T21:00:00Z",
        "error_message": null
      },
      "runs_last_24h": {
        "total": 288,
        "succeeded": 287,
        "failed": 1
      }
    },
    {
      "stream_id": "nws-gridpoints-forecast",
      "status": "failed",
      "last_run": {
        "id": "550e8400-e29b-41d4-a716-446655440001",
        "started_at": "2026-01-16T21:00:00Z",
        "completed_at": "2026-01-16T21:00:05Z",
        "duration_ms": 5230,
        "rows_processed": 0,
        "rows_flagged": 0,
        "rows_rejected": 0,
        "watermark_before": "2026-01-16T18:00:00Z",
        "watermark_after": null,
        "error_message": "SQL execution failed: column \"wind_speed_kmh\" does not exist"
      },
      "runs_last_24h": {
        "total": 48,
        "succeeded": 45,
        "failed": 3
      }
    }
  ]
}
```

#### Query Implementation

```sql
-- For a specific stream:
WITH latest_run AS (
    SELECT DISTINCT ON (stream_id) *
    FROM silver.etl_runs
    WHERE stream_id = $1
    ORDER BY stream_id, started_at DESC
),
run_stats AS (
    SELECT
        stream_id,
        COUNT(*) AS total,
        COUNT(*) FILTER (WHERE status = 'success') AS succeeded,
        COUNT(*) FILTER (WHERE status = 'failed') AS failed
    FROM silver.etl_runs
    WHERE stream_id = $1
      AND started_at > NOW() - INTERVAL '24 hours'
    GROUP BY stream_id
)
SELECT
    lr.stream_id,
    lr.status,
    lr.id,
    lr.started_at,
    lr.completed_at,
    lr.duration_ms,
    lr.rows_processed,
    lr.rows_flagged,
    lr.rows_rejected,
    lr.watermark_before,
    lr.watermark_after,
    lr.error_message,
    rs.total AS runs_24h_total,
    rs.succeeded AS runs_24h_succeeded,
    rs.failed AS runs_24h_failed
FROM latest_run lr
LEFT JOIN run_stats rs ON lr.stream_id = rs.stream_id;

-- For all streams (no WHERE clause on stream_id)
```

---

### Tool 2: `etl_history`

**Purpose**: Retrieve historical ETL runs for trend analysis and debugging.

#### Input Schema (JSON Schema)

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "required": ["stream_id"],
  "properties": {
    "stream_id": {
      "type": "string",
      "description": "Stream identifier (required)"
    },
    "limit": {
      "type": "integer",
      "description": "Maximum number of runs to return (default: 10, max: 100)",
      "default": 10,
      "minimum": 1,
      "maximum": 100
    },
    "since": {
      "type": "string",
      "format": "date-time",
      "description": "Only return runs after this timestamp (ISO 8601)"
    },
    "status": {
      "type": "string",
      "enum": ["running", "success", "failed", "partial"],
      "description": "Filter by status"
    }
  },
  "additionalProperties": false
}
```

#### Output Schema

```json
{
  "type": "object",
  "required": ["success", "stream_id", "runs"],
  "properties": {
    "success": { "type": "boolean" },
    "stream_id": { "type": "string" },
    "runs": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "id": { "type": "string", "format": "uuid" },
          "started_at": { "type": "string", "format": "date-time" },
          "completed_at": { "type": ["string", "null"], "format": "date-time" },
          "duration_ms": { "type": ["integer", "null"] },
          "status": { "type": "string" },
          "rows_processed": { "type": "integer" },
          "rows_flagged": { "type": "integer" },
          "rows_rejected": { "type": "integer" },
          "watermark_before": { "type": ["string", "null"], "format": "date-time" },
          "watermark_after": { "type": ["string", "null"], "format": "date-time" },
          "error_message": { "type": ["string", "null"] },
          "error_context": { "type": ["object", "null"] },
          "run_mode": { "type": "string" }
        }
      }
    },
    "summary": {
      "type": "object",
      "properties": {
        "total_returned": { "type": "integer" },
        "total_available": { "type": "integer" },
        "time_range": {
          "type": "object",
          "properties": {
            "oldest": { "type": "string", "format": "date-time" },
            "newest": { "type": "string", "format": "date-time" }
          }
        }
      }
    }
  }
}
```

#### Example Request/Response

**Request**:
```json
{
  "stream_id": "air-quality",
  "limit": 5,
  "since": "2026-01-16T18:00:00Z"
}
```

**Response**:
```json
{
  "success": true,
  "stream_id": "air-quality",
  "runs": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "started_at": "2026-01-16T21:00:00Z",
      "completed_at": "2026-01-16T21:00:02Z",
      "duration_ms": 2150,
      "status": "success",
      "rows_processed": 288,
      "rows_flagged": 5,
      "rows_rejected": 0,
      "watermark_before": "2026-01-16T20:55:00Z",
      "watermark_after": "2026-01-16T21:00:00Z",
      "error_message": null,
      "error_context": null,
      "run_mode": "daemon"
    },
    {
      "id": "550e8400-e29b-41d4-a716-446655440001",
      "started_at": "2026-01-16T20:55:00Z",
      "completed_at": "2026-01-16T20:55:02Z",
      "duration_ms": 1980,
      "status": "success",
      "rows_processed": 285,
      "rows_flagged": 3,
      "rows_rejected": 0,
      "watermark_before": "2026-01-16T20:50:00Z",
      "watermark_after": "2026-01-16T20:55:00Z",
      "error_message": null,
      "error_context": null,
      "run_mode": "daemon"
    }
  ],
  "summary": {
    "total_returned": 5,
    "total_available": 36,
    "time_range": {
      "oldest": "2026-01-16T18:05:00Z",
      "newest": "2026-01-16T21:00:00Z"
    }
  }
}
```

#### Query Implementation

```sql
-- Main query
SELECT
    id, started_at, completed_at, duration_ms, status,
    rows_processed, rows_flagged, rows_rejected,
    watermark_before, watermark_after,
    error_message, error_context, run_mode
FROM silver.etl_runs
WHERE stream_id = $1
  AND ($2::TIMESTAMPTZ IS NULL OR started_at > $2)
  AND ($3::TEXT IS NULL OR status = $3)
ORDER BY started_at DESC
LIMIT $4;

-- Count query for summary
SELECT
    COUNT(*) AS total_available,
    MIN(started_at) AS oldest,
    MAX(started_at) AS newest
FROM silver.etl_runs
WHERE stream_id = $1
  AND ($2::TIMESTAMPTZ IS NULL OR started_at > $2)
  AND ($3::TEXT IS NULL OR status = $3);
```

---

### Tool 3: `data_freshness`

**Purpose**: Report data freshness across Bronze and Silver layers.

#### Input Schema (JSON Schema)

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "layer": {
      "type": "string",
      "enum": ["bronze", "silver", "all"],
      "default": "all",
      "description": "Which layer(s) to check (default: all)"
    }
  },
  "additionalProperties": false
}
```

#### Output Schema

```json
{
  "type": "object",
  "required": ["success", "freshness", "checked_at"],
  "properties": {
    "success": { "type": "boolean" },
    "checked_at": { "type": "string", "format": "date-time" },
    "freshness": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["layer", "identifier", "latest_timestamp", "age_seconds"],
        "properties": {
          "layer": { "type": "string", "enum": ["bronze", "silver"] },
          "identifier": {
            "type": "string",
            "description": "Stream ID (bronze) or table name (silver)"
          },
          "latest_timestamp": {
            "type": ["string", "null"],
            "format": "date-time"
          },
          "age_seconds": {
            "type": ["integer", "null"],
            "description": "Seconds since latest data (null if no data)"
          },
          "freshness_status": {
            "type": "string",
            "enum": ["fresh", "stale", "critical", "no_data"],
            "description": "fresh: <5min, stale: 5-30min, critical: >30min"
          },
          "row_count": {
            "type": ["integer", "null"],
            "description": "Total rows (Silver only)"
          },
          "last_etl_run": {
            "type": ["string", "null"],
            "format": "date-time",
            "description": "Most recent ETL completion (Silver only)"
          }
        }
      }
    },
    "summary": {
      "type": "object",
      "properties": {
        "bronze_streams": { "type": "integer" },
        "silver_tables": { "type": "integer" },
        "stale_count": { "type": "integer" },
        "critical_count": { "type": "integer" }
      }
    }
  }
}
```

#### Example Request/Response

**Request**:
```json
{
  "layer": "all"
}
```

**Response**:
```json
{
  "success": true,
  "checked_at": "2026-01-16T21:05:00Z",
  "freshness": [
    {
      "layer": "bronze",
      "identifier": "air-quality",
      "latest_timestamp": "2026-01-16T21:04:30Z",
      "age_seconds": 30,
      "freshness_status": "fresh"
    },
    {
      "layer": "bronze",
      "identifier": "nws-gridpoints-forecast",
      "latest_timestamp": "2026-01-16T20:00:00Z",
      "age_seconds": 3900,
      "freshness_status": "critical"
    },
    {
      "layer": "silver",
      "identifier": "air_quality_observations",
      "latest_timestamp": "2026-01-16T21:00:00Z",
      "age_seconds": 300,
      "freshness_status": "fresh",
      "row_count": 142857,
      "last_etl_run": "2026-01-16T21:00:02Z"
    },
    {
      "layer": "silver",
      "identifier": "weather_forecasts",
      "latest_timestamp": "2026-01-16T18:00:00Z",
      "age_seconds": 11100,
      "freshness_status": "critical",
      "row_count": 10240,
      "last_etl_run": "2026-01-16T18:05:00Z"
    }
  ],
  "summary": {
    "bronze_streams": 8,
    "silver_tables": 4,
    "stale_count": 0,
    "critical_count": 2
  }
}
```

#### Query Implementation

**Bronze Freshness** (from Parquet file metadata):
```rust
// Pseudo-code for Bronze freshness (existing ndp-bronze code)
for stream in enabled_streams {
    let files = glob(&format!("{}/{}/**/*.parquet", bronze_path, stream.id))?;
    let latest = files.iter()
        .filter_map(|f| f.metadata().modified().ok())
        .max();
    // OR read MAX(timestamp) from Parquet if available
}
```

**Silver Freshness** (from TimescaleDB):
```sql
-- Per-table freshness with watermark column awareness
SELECT
    'air_quality_observations' AS table_name,
    'observation_time' AS watermark_column,
    MAX(observation_time) AS latest_timestamp,
    COUNT(*) AS row_count
FROM silver.air_quality_observations
UNION ALL
SELECT
    'weather_observations',
    'observation_time',
    MAX(observation_time),
    COUNT(*)
FROM silver.weather_observations
UNION ALL
SELECT
    'weather_forecasts',
    'issue_time',  -- Note: Uses issue_time, not valid_time
    MAX(issue_time),
    COUNT(*)
FROM silver.weather_forecasts
UNION ALL
SELECT
    'outdoor_air_quality',
    'observation_time',
    MAX(observation_time),
    COUNT(*)
FROM silver.outdoor_air_quality;

-- Last ETL run per stream (join with etl_runs)
SELECT DISTINCT ON (stream_id)
    stream_id,
    completed_at AS last_etl_run
FROM silver.etl_runs
WHERE status = 'success'
ORDER BY stream_id, started_at DESC;
```

**Freshness Thresholds**:
| Status | Threshold |
|--------|-----------|
| `fresh` | age < 5 minutes (300s) |
| `stale` | 5 min <= age < 30 min |
| `critical` | age >= 30 minutes |
| `no_data` | No data in table/stream |

---

## Part 3: Implementation Guidelines for dp-011

### 3.1 Rust Changes to silver-etl

**File**: `apps/silver-etl/src/etl.rs`

```rust
// Add to EtlStats struct (already exists, no changes needed)
pub struct EtlStats {
    pub stream_id: String,
    pub rows_processed: u64,
    pub rows_with_dq_flags: u64,  // maps to rows_flagged
    pub rows_rejected: u64,
    pub duration_ms: u64,
    pub watermark_before: Option<DateTime<Utc>>,
    pub watermark_after: Option<DateTime<Utc>>,
}

// New: EtlRunRecord for persistence
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

#[derive(Debug, Clone, Copy)]
pub enum EtlRunStatus {
    Running,
    Success,
    Failed,
    Partial,
}

#[derive(Debug, Clone, Copy)]
pub enum EtlRunMode {
    Daemon,
    Manual,
    Backfill,
}
```

**File**: `apps/silver-etl/src/persistence.rs` (new)

```rust
// Trait for ETL run persistence
pub trait EtlRunPersistence: Send + Sync {
    /// Insert a new run record (status = Running)
    fn start_run(&self, stream_id: &str, run_mode: EtlRunMode,
                 daemon_cycle_id: Option<Uuid>) -> Result<Uuid, PersistenceError>;

    /// Update run with completion status and stats
    fn complete_run(&self, id: Uuid, stats: &EtlStats) -> Result<(), PersistenceError>;

    /// Update run with failure status and error
    fn fail_run(&self, id: Uuid, error: &str,
                context: Option<serde_json::Value>) -> Result<(), PersistenceError>;
}

// Implementation using DuckDB's PostgreSQL extension (already attached)
pub struct DuckDbRunPersistence<'a> {
    conn: &'a Connection,
}
```

**File**: `apps/silver-etl/src/daemon.rs`

```rust
// Modify run_cycle to persist run records
fn run_cycle(&self) -> Result<DaemonCycleStats, DaemonError> {
    let cycle_id = Uuid::new_v4();  // Shared across all stream runs

    for stream_id in &streams {
        // 1. Start run record
        let run_id = persistence.start_run(stream_id, EtlRunMode::Daemon, Some(cycle_id))?;

        // 2. Execute ETL
        match executor.run_stream(stream_id) {
            Ok(stats) => {
                // 3a. Complete run record
                persistence.complete_run(run_id, &stats)?;
            }
            Err(e) => {
                // 3b. Fail run record
                persistence.fail_run(run_id, &e.to_string(), None)?;
            }
        }
    }
}
```

### 3.2 MCP Server Changes

**File**: `mcp/ndp-bronze/src/tools/etl_status.rs` (new)

```rust
use crate::storage::SilverStorage;

pub async fn etl_status(
    storage: &dyn SilverStorage,
    stream_id: Option<String>,
) -> Result<EtlStatusResponse, McpError> {
    let streams = match stream_id {
        Some(id) => vec![storage.get_latest_run(&id).await?],
        None => storage.get_all_latest_runs().await?,
    };

    Ok(EtlStatusResponse {
        success: true,
        streams,
    })
}
```

### 3.3 Migration Deployment

```bash
# dp-011 deliverables:
deploy/timescaledb/migrations/003_etl_runs.sql    # Schema + indexes
deploy/timescaledb/migrations/003_etl_runs_down.sql  # Rollback

# Apply migration:
psql $TIMESCALE_URL -f deploy/timescaledb/migrations/003_etl_runs.sql

# Verify:
\d silver.etl_runs
SELECT COUNT(*) FROM silver.etl_runs;
```

---

## Part 4: Cross-References

### Current Source Code

| File | Contents |
|------|----------|
| `apps/silver-etl/src/etl.rs:893-914` | `EtlStats` struct definition |
| `apps/silver-etl/src/daemon.rs:79-88` | `DaemonCycleStats` struct |
| `apps/silver-etl/src/metrics.rs` | Prometheus metrics (ephemeral) |

### Current EtlStats Fields (to persist)

| Field | Type | Maps To |
|-------|------|---------|
| `stream_id` | `String` | `stream_id TEXT` |
| `rows_processed` | `u64` | `rows_processed BIGINT` |
| `rows_with_dq_flags` | `u64` | `rows_flagged BIGINT` |
| `rows_rejected` | `u64` | `rows_rejected BIGINT` |
| `duration_ms` | `u64` | `duration_ms BIGINT` |
| `watermark_before` | `Option<DateTime<Utc>>` | `watermark_before TIMESTAMPTZ` |
| `watermark_after` | `Option<DateTime<Utc>>` | `watermark_after TIMESTAMPTZ` |

### Related Patterns (from AgentDB)

- `arch-data-lake-layers`: Bronze -> Silver -> Gold architecture
- `arch-config-driven-silver-etl`: Configuration-driven ETL design
- `duckdb-timestamptz-arithmetic`: DuckDB timestamp handling

---

## Appendix A: SQL Quick Reference

### Create Table
```sql
\i deploy/timescaledb/migrations/003_etl_runs.sql
```

### Query Examples
```sql
-- Last run per stream
SELECT DISTINCT ON (stream_id) * FROM silver.etl_runs
ORDER BY stream_id, started_at DESC;

-- Failed runs in last 24h
SELECT * FROM silver.etl_runs
WHERE status = 'failed' AND started_at > NOW() - INTERVAL '24 hours'
ORDER BY started_at DESC;

-- Success rate by stream
SELECT
    stream_id,
    COUNT(*) AS total_runs,
    COUNT(*) FILTER (WHERE status = 'success') AS successful,
    ROUND(100.0 * COUNT(*) FILTER (WHERE status = 'success') / COUNT(*), 1) AS success_rate_pct
FROM silver.etl_runs
WHERE started_at > NOW() - INTERVAL '7 days'
GROUP BY stream_id;

-- Average duration by stream
SELECT
    stream_id,
    AVG(duration_ms)::INT AS avg_duration_ms,
    MAX(duration_ms) AS max_duration_ms
FROM silver.etl_runs
WHERE status = 'success'
GROUP BY stream_id;
```

---

*Specification created: 2026-01-16*
*dp-011 implementation target: Post dp-010 MCP tools*
