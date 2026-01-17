# DP-011: Silver ETL Run Statistics Persistence

**Feature ID**: dp-011
**Title**: Persist ETL Run Statistics to TimescaleDB
**Status**: Scope Definition
**Created**: 2026-01-16
**Depends On**: dp-006 (Silver Layer), dp-010 (MCP Specification)

---

## Executive Summary

Extend the silver-etl application to persist run statistics to TimescaleDB, enabling queryable ETL operational history. This feature implements the ETL statistics storage requirements defined in dp-010's MCP specification.

---

## Context

### Current State

The silver-etl application currently:
- Computes `EtlStats` per stream (rows_processed, rows_flagged, duration_ms, watermarks)
- Exposes Prometheus metrics (counters, histograms) - ephemeral, reset on restart
- Logs run statistics to stdout/journald
- **Does NOT persist run history to database**

### Problem

Without persistent ETL run history:
- Cannot query "when did ETL last run for stream X?"
- Cannot track error patterns over time
- Cannot build operational dashboards for ETL health
- MCP `etl_status` tool has no data source

### Solution

Add `silver.etl_runs` table and modify silver-etl to INSERT run statistics after each ETL execution.

---

## Scope

### In Scope

| Component | Description |
|-----------|-------------|
| **Migration** | `004_etl_runs.sql` - Create `silver.etl_runs` table |
| **silver-etl modification** | Persist `EtlStats` to database after each run |
| **Error capture** | Store error messages for failed runs |
| **Retention policy** | Auto-cleanup of old run records |

### Schema (Draft)

**Note**: Final schema defined by dp-010 specification.

```sql
CREATE TABLE silver.etl_runs (
    id                  BIGSERIAL PRIMARY KEY,
    stream_id           TEXT NOT NULL,
    started_at          TIMESTAMPTZ NOT NULL,
    completed_at        TIMESTAMPTZ,
    status              TEXT NOT NULL,  -- 'running', 'success', 'failed'
    rows_processed      BIGINT DEFAULT 0,
    rows_flagged        BIGINT DEFAULT 0,
    rows_rejected       BIGINT DEFAULT 0,
    duration_ms         BIGINT,
    watermark_before    TIMESTAMPTZ,
    watermark_after     TIMESTAMPTZ,
    error_message       TEXT,
    error_context       JSONB,

    -- Indexing for common queries
    CONSTRAINT valid_status CHECK (status IN ('running', 'success', 'failed'))
);

CREATE INDEX idx_etl_runs_stream_time ON silver.etl_runs (stream_id, started_at DESC);
CREATE INDEX idx_etl_runs_status ON silver.etl_runs (status) WHERE status != 'success';
```

### Out of Scope

| Item | Reason | Where |
|------|--------|-------|
| MCP tool implementation | Separate concern | dp-010 |
| Schema design decisions | Specification phase | dp-010 |
| Grafana ETL dashboard | Follow-on work | Future |
| Alerting on failures | Separate concern | Future |

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| dp-006 Silver Layer | ✅ Complete | Database operational |
| dp-010 MCP Specification | 🔲 In Progress | Defines schema requirements |

---

## Success Criteria

| Criterion | Validation |
|-----------|------------|
| ETL runs persisted | `SELECT * FROM silver.etl_runs` shows history |
| All streams tracked | Each enabled stream has run records |
| Errors captured | Failed runs include error_message |
| MCP queryable | dp-010 `etl_status` tool can query this table |
| Retention working | Old records auto-cleaned per policy |

---

## Implementation Notes

### Code Changes

1. **Add database writer to EtlRunner**
   - New method: `persist_stats(&self, stats: &EtlStats, status: &str, error: Option<&str>)`
   - Called at end of `run_etl()` in both success and error paths

2. **Daemon integration**
   - `RealEtlExecutor` gains database connection for stats persistence
   - Run record created at start (status='running'), updated on completion

3. **Connection management**
   - Reuse existing TimescaleDB connection from EtlRunner
   - Or: Separate connection for stats (isolation)

### Migration

- Idempotent (IF NOT EXISTS)
- Part of standard `deploy/timescaledb/migrations/` sequence
- Applied before silver-etl upgrade

---

## Deliverables

| Deliverable | Description |
|-------------|-------------|
| `004_etl_runs.sql` | Migration creating etl_runs table |
| `etl.rs` changes | Persist stats after run |
| `daemon.rs` changes | Track running state |
| Tests | Verify stats are persisted |

---

## References

- [dp-010 SCOPE](../dp-010/SCOPE.md) - MCP Specification (defines schema requirements)
- [silver-etl/src/etl.rs](../../../apps/silver-etl/src/etl.rs) - Current EtlStats implementation
- [silver-etl/src/metrics.rs](../../../apps/silver-etl/src/metrics.rs) - Prometheus metrics

---

*Scope defined: 2026-01-16*
*Implementation blocked on: dp-010 specification completion*
