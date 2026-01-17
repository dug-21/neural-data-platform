# ADR-001: ETL Run Statistics Persistence Strategy

## Status

Accepted

## Context

The silver-etl application computes `EtlStats` per stream (rows_processed, rows_flagged, duration_ms, watermarks) after each ETL run. Currently:

1. **Prometheus metrics exist** (`apps/silver-etl/src/metrics.rs`) - Counter and histogram metrics for rows processed, flagged, rejected, and duration
2. **Metrics are ephemeral** - Reset on container restart, no historical queries possible
3. **No database persistence** - Cannot query "when did ETL last run for stream X?"
4. **MCP tools need data source** - dp-010's `etl_status`, `etl_history`, and `data_freshness` tools require queryable run history

We need to decide the observability architecture:
- **Option A**: Database persistence only (TimescaleDB `silver.etl_runs`)
- **Option B**: Prometheus metrics only (current + enhancements)
- **Option C**: Dual-write (both database AND Prometheus)

## Decision

**Adopt Option C: Dual-Write Architecture** with database as primary and Prometheus as secondary.

### Architecture

```
                                          +-------------------+
                                          | Grafana Dashboard |
                                          | (Real-time view)  |
                                          +--------+----------+
                                                   |
                                                   v
+---------------+     +-----------------+    +-----------+
| silver-etl   |---->| Prometheus      |<---| Scrape    |
| daemon       |     | Metrics         |    | /metrics  |
|              |     | (ephemeral)     |    +-----------+
| EtlStats     |     +-----------------+
| after run    |
|              |     +-----------------+    +-------------------+
|              |---->| TimescaleDB     |<---| MCP Tools         |
|              |     | silver.etl_runs |    | (etl_status,      |
+---------------+    | (persistent)    |    |  etl_history,     |
                     +-----------------+    |  data_freshness)  |
                                            +-------------------+
```

### Metrics Classification

| Metric Type | Prometheus | Database | Rationale |
|-------------|------------|----------|-----------|
| **Real-time counters** | Primary | Secondary | Prometheus excels at rate() queries |
| **Run history** | N/A | Primary | Prometheus has 15-day retention, can't query by run_id |
| **Error details** | N/A | Primary | Prometheus labels have cardinality limits |
| **Dashboards (live)** | Primary | Secondary | Sub-second refresh for Grafana |
| **Dashboards (historical)** | N/A | Primary | 30-day trend analysis |
| **MCP access** | N/A | Primary | SQL queries required |
| **Alerting** | Primary | Secondary | Prometheus Alertmanager integration |

### Prometheus Metrics (Enhanced)

Retain existing metrics with enhancements:

```rust
// Existing (keep)
silver_etl_rows_processed_total{stream_id}     // Counter
silver_etl_rows_flagged_total{stream_id}       // Counter
silver_etl_rows_rejected_total{stream_id}      // Counter
silver_etl_duration_seconds                     // Histogram
silver_etl_runs_total                           // Counter

// New additions
silver_etl_last_success_timestamp{stream_id}   // Gauge (Unix epoch)
silver_etl_last_run_status{stream_id}          // Gauge (1=success, 0=failed)
silver_etl_watermark_lag_seconds{stream_id}    // Gauge (now - watermark_after)
```

### Database Schema (TimescaleDB)

Per dp-010 specification:

```sql
CREATE TABLE silver.etl_runs (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    stream_id           TEXT NOT NULL,
    started_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at        TIMESTAMPTZ,
    duration_ms         BIGINT,
    status              TEXT NOT NULL DEFAULT 'running',
    rows_processed      BIGINT NOT NULL DEFAULT 0,
    rows_flagged        BIGINT NOT NULL DEFAULT 0,
    rows_rejected       BIGINT NOT NULL DEFAULT 0,
    watermark_before    TIMESTAMPTZ,
    watermark_after     TIMESTAMPTZ,
    error_message       TEXT,
    error_context       JSONB,
    run_mode            TEXT DEFAULT 'daemon',
    daemon_cycle_id     UUID,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

## Consequences

### Benefits

1. **Complete observability** - Both real-time (Prometheus) and historical (database) views
2. **MCP tool support** - `etl_status`, `etl_history` can query `silver.etl_runs`
3. **Grafana flexibility** - Use Prometheus datasource for live, PostgreSQL for history
4. **Alerting path** - Prometheus Alertmanager for ops, database for debugging
5. **Error context preservation** - JSONB error_context for debugging (not possible in Prometheus)

### Costs

1. **Dual-write overhead** - Two persistence targets per ETL run (~1-5ms additional)
2. **Consistency risk** - Prometheus write succeeds but DB write fails (acceptable)
3. **Slightly more code** - Persistence module + metric updates

### Trade-offs Accepted

| Trade-off | Decision |
|-----------|----------|
| Write latency | Acceptable: Async DB write, sync metrics |
| Eventual consistency | Acceptable: Prometheus may show success but DB has partial write |
| Memory for gauges | Acceptable: 3 new gauges x streams << 1KB |

## Alternatives Considered

### Option A: Database Only

**Pros**: Single source of truth, simpler architecture
**Cons**: No real-time Grafana dashboards, no Alertmanager integration, re-implements what Prometheus does well

**Rejected**: Loses existing Prometheus infrastructure value.

### Option B: Prometheus Only

**Pros**: Existing infrastructure, excellent for real-time
**Cons**:
- Cannot store error_message/error_context (label cardinality explosion)
- Cannot query by run_id
- Limited retention (typically 15 days)
- MCP tools cannot query (would need PromQL translation layer)

**Rejected**: Cannot support MCP tool requirements or long-term history.

### Option C (Partial): Write-Ahead Log

**Considered**: Write to local file, async batch insert to database
**Pros**: Survives database outages
**Cons**: Adds complexity, NDP already handles Bronze WAL separately

**Rejected**: Over-engineering for observability use case.

## Implementation Notes

### Write Order

1. **Start of run**: INSERT to database (status='running'), update Prometheus gauges
2. **End of run**: UPDATE database row, update Prometheus counters/gauges
3. **On error**: Database write failure is logged but does not fail ETL

### Failure Modes

| Failure | Behavior |
|---------|----------|
| DB unavailable at start | Log warning, continue ETL, no run record |
| DB unavailable at end | Log error, Prometheus metrics still updated |
| Prometheus unavailable | Metrics silently dropped (Prometheus library behavior) |

### Resource Constraints (Pi 5)

- Database write: ~5ms per run, <1KB per row
- Prometheus update: <1ms per metric
- 30-day retention at 5-min intervals: ~14MB for 8 streams

## Related ADRs

- ADR-002: Connection Management
- ADR-003: Run Lifecycle (INSERT vs UPDATE timing)

## References

- [dp-010 ETL Status Specification](../../dp-010/specification/ETL-STATUS-SPEC.md)
- [Current metrics.rs](../../../../apps/silver-etl/src/metrics.rs)
- [Prometheus Best Practices](https://prometheus.io/docs/practices/naming/)
