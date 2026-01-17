# DP-011 Architecture Overview

## ETL Run Statistics Persistence

This document provides an architectural overview of the dp-011 feature: persisting silver-etl run statistics to TimescaleDB for operational observability.

## Component Diagram

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                              silver-etl Container                                │
│                                                                                  │
│  ┌────────────────────────────────────────────────────────────────────────────┐ │
│  │                           DaemonRunner                                      │ │
│  │                                                                             │ │
│  │    ┌──────────────────┐    ┌──────────────────┐    ┌──────────────────┐   │ │
│  │    │  ConfigLoader    │    │  RealEtlExecutor │    │ EtlRunPersistence│   │ │
│  │    │  (etcd client)   │    │                  │    │ (NEW - dp-011)   │   │ │
│  │    └────────┬─────────┘    │  ┌────────────┐  │    └────────┬─────────┘   │ │
│  │             │              │  │ EtlRunner  │  │             │             │ │
│  │             │              │  │            │  │             │             │ │
│  │             │              │  │ ┌────────┐ │  │             │             │ │
│  │             │              │  │ │DuckDB  │ │  │             │             │ │
│  │             │              │  │ │+ATTACH │ │  │             │             │ │
│  │             │              │  │ │postgres│ │  │             │             │ │
│  │             │              │  │ └───┬────┘ │  │             │             │ │
│  │             │              │  └─────┼──────┘  │             │             │ │
│  │             │              └────────┼─────────┘             │             │ │
│  └─────────────┼───────────────────────┼───────────────────────┼─────────────┘ │
│                │                       │                       │               │
└────────────────┼───────────────────────┼───────────────────────┼───────────────┘
                 │                       │                       │
                 ▼                       │                       │
        ┌────────────────┐               │                       │
        │     etcd       │               │                       │
        │ (config store) │               │                       │
        └────────────────┘               │                       │
                                         │                       │
                 ┌───────────────────────┴───────────────────────┘
                 │
                 ▼
        ┌────────────────────────────────────────────────────────────┐
        │                      TimescaleDB                            │
        │  ┌──────────────────────┐    ┌──────────────────────────┐  │
        │  │   silver.*          │    │   silver.etl_runs         │  │
        │  │   (data tables)     │    │   (NEW - dp-011)          │  │
        │  │                     │    │                           │  │
        │  │ - air_quality_obs   │    │ - id (UUID)               │  │
        │  │ - weather_forecasts │    │ - stream_id               │  │
        │  │ - outdoor_air_qual  │    │ - started_at              │  │
        │  │ - ...               │    │ - completed_at            │  │
        │  │                     │    │ - status                  │  │
        │  │ (via DuckDB ATTACH) │    │ - rows_processed          │  │
        │  └──────────────────────┘    │ - watermarks              │  │
        │                              │ - error_message           │  │
        │                              │                           │  │
        │                              │ (via tokio-postgres)      │  │
        │                              └──────────────────────────┘  │
        └────────────────────────────────────────────────────────────┘
                                         │
                                         │
        ┌────────────────────────────────┼────────────────────────────┐
        │                                │                            │
        │                                ▼                            │
        │  ┌──────────────────┐    ┌──────────────────────────┐      │
        │  │   Grafana        │    │   ndp-mcp-server         │      │
        │  │   Dashboards     │    │   (Claude access)        │      │
        │  │                  │    │                          │      │
        │  │ ┌──────────────┐ │    │ - etl_status tool        │      │
        │  │ │ Prometheus   │ │    │ - etl_history tool       │      │
        │  │ │ datasource   │ │    │ - data_freshness tool    │      │
        │  │ └──────────────┘ │    │                          │      │
        │  │ ┌──────────────┐ │    │ (queries silver.etl_runs)│      │
        │  │ │ PostgreSQL   │ │    └──────────────────────────┘      │
        │  │ │ datasource   │ │                                      │
        │  │ └──────────────┘ │                                      │
        │  └──────────────────┘                                      │
        │           Observability Consumers                          │
        └────────────────────────────────────────────────────────────┘
```

## Data Flow

### ETL Execution Flow (with Persistence)

```
1. Daemon tick (every 5 minutes)
   │
   ├─► Generate daemon_cycle_id (UUID)
   │
   ├─► For each enabled stream:
   │     │
   │     ├─► [NEW] persistence.start_run(stream_id, mode, cycle_id)
   │     │         └─► INSERT INTO silver.etl_runs (status='running')
   │     │
   │     ├─► Load config from etcd
   │     │
   │     ├─► Execute ETL (DuckDB → PostgreSQL bulk insert)
   │     │
   │     ├─► [EXISTING] Update Prometheus metrics
   │     │
   │     └─► [NEW] persistence.complete_run(id, stats) or fail_run(id, error)
   │               └─► UPDATE silver.etl_runs (status='success'/'failed')
   │
   └─► Log cycle stats
```

### Query Flow (MCP Tools)

```
Agent Request                   ndp-mcp-server              TimescaleDB
    │                                │                          │
    │  "etl_status air-quality"      │                          │
    ├───────────────────────────────►│                          │
    │                                │  SELECT DISTINCT ON...   │
    │                                ├─────────────────────────►│
    │                                │                          │
    │                                │◄─────────────────────────┤
    │                                │  Run history + stats     │
    │◄───────────────────────────────┤                          │
    │  JSON response                 │                          │
```

## Scalability Considerations

### Current State (8 streams)

| Metric | Value | Notes |
|--------|-------|-------|
| Runs per day | ~2,304 | 8 streams x 288 runs (5-min interval) |
| Storage per month | ~14 MB | ~200 bytes/row |
| Query latency | <50ms | Index on (stream_id, started_at) |

### Future Scale (20+ streams)

| Metric | Value at 20 streams | Mitigation |
|--------|---------------------|------------|
| Runs per day | ~5,760 | Table size still small (~35 MB/month) |
| Index size | ~2 MB | Fits in Pi 5 memory |
| Query latency | <100ms | Add composite indexes if needed |

### Horizontal Scaling Path

If NDP moves beyond single-host deployment:

1. **Read replicas** - MCP tools query replica, writes go to primary
2. **Time-series partitioning** - Partition by month for faster retention cleanup
3. **Archive tier** - Move old data to cold storage (S3 + DuckDB for analysis)

Current design does not require these, but schema is compatible.

## Security Considerations

| Concern | Mitigation |
|---------|------------|
| Connection credentials | Use existing `TIMESCALE_URL` env var, same as EtlRunner |
| Error message exposure | `error_message` may contain paths/configs; MCP tools sanitize |
| SQL injection | Prepared statements in tokio-postgres implementation |
| Connection limits | Pool max_size=2, won't exhaust PostgreSQL connections |

## Monitoring

### Prometheus Metrics (Enhanced)

```
# Existing (no changes)
silver_etl_rows_processed_total{stream_id="..."}
silver_etl_rows_flagged_total{stream_id="..."}
silver_etl_rows_rejected_total{stream_id="..."}
silver_etl_duration_seconds_bucket{...}
silver_etl_runs_total

# New (dp-011)
silver_etl_last_success_timestamp{stream_id="..."}  # Unix epoch
silver_etl_last_run_status{stream_id="..."}         # 1=success, 0=failed
silver_etl_watermark_lag_seconds{stream_id="..."}   # now - watermark_after
silver_etl_persistence_errors_total                  # DB write failures
```

### Health Checks

```bash
# Existing: silver-etl container health
curl -f http://localhost:9090/health

# New: MCP tool health (queries silver.etl_runs)
curl -f http://localhost:9100/health
# Returns: {"etl_runs_accessible": true}
```

## Dependencies

### New Rust Dependencies

```toml
[dependencies]
tokio-postgres = { version = "0.7", features = ["with-uuid-1", "with-chrono-0_4", "with-serde_json-1"] }
bb8 = "0.8"
bb8-postgres = "0.8"
```

### Database Migration

```
deploy/timescaledb/migrations/
├── 001_silver_schema.sql        # Existing
├── 002_silver_indexes.sql       # Existing
└── 003_etl_runs.sql             # NEW (dp-011)
```

## Related Documents

| Document | Purpose |
|----------|---------|
| [ADR-001-persistence-strategy.md](./ADR-001-persistence-strategy.md) | Why dual-write (DB + Prometheus) |
| [ADR-002-connection-management.md](./ADR-002-connection-management.md) | How to manage DB connections |
| [ADR-003-run-lifecycle.md](./ADR-003-run-lifecycle.md) | When to INSERT vs UPDATE |
| [dp-010 ETL-STATUS-SPEC.md](../../dp-010/specification/ETL-STATUS-SPEC.md) | Schema and MCP tool specs |
| [SCOPE.md](../SCOPE.md) | Feature requirements |

## Implementation Phases

### Phase 1: Foundation (dp-011 scope)

- [ ] Migration `003_etl_runs.sql`
- [ ] `EtlRunPersistence` trait + PostgreSQL implementation
- [ ] Daemon integration (start_run, complete_run, fail_run)
- [ ] Startup orphan cleanup
- [ ] Unit tests with mocked persistence

### Phase 2: Observability (follow-on)

- [ ] Enhanced Prometheus metrics (gauges for last success, watermark lag)
- [ ] Grafana dashboard for ETL health
- [ ] Alertmanager rules for failed runs

### Phase 3: MCP Integration (dp-010 scope)

- [ ] `etl_status` tool implementation
- [ ] `etl_history` tool implementation
- [ ] `data_freshness` tool implementation

## Revision History

| Date | Author | Changes |
|------|--------|---------|
| 2026-01-16 | ndp-architect | Initial architecture design |
