# DP-006: Silver Layer Architecture Overview

**Feature**: dp-006 (Silver Layer Implementation)
**Status**: Architecture Complete
**Date**: 2026-01-10
**Author**: NDP Architect

---

## Executive Summary

This document summarizes the architectural decisions for dp-006, the Silver layer implementation that transforms raw Bronze Parquet data into typed TimescaleDB tables for analytics and dashboards.

### Key Decisions

| ADR | Decision | Rationale |
|-----|----------|-----------|
| ADR-006-001 | **duckdb-rs embedded** for ETL engine | Single binary, proven PostgreSQL writes, Pi 5 compatible |
| ADR-006-002 | **Separate binary** architecture | Protects Bronze reliability, follows "Bronze must succeed" principle |
| ADR-006-003 | **Flat silver.* schema** for Phase 1 | Simple queries, migration path documented for multi-domain |
| ADR-006-004 | **Four DQ actions** (flag/reject/clamp/drop) | Transparency by default, flexible per-field handling |
| ADR-006-005 | **systemd timer** scheduling | Persistent catch-up, standard Linux tooling, hourly cadence |
| ADR-006-006 | **stream_type field** distinction | Forward-compatible for events, explicit semantics |

---

## Architecture Diagram

```
┌──────────────────────────────────────────────────────────────────────────┐
│                          BRONZE LAYER (Existing)                          │
├──────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   ┌─────────────────────┐                                                │
│   │   air-quality-app   │    Sources → Channel → ParquetStore            │
│   │   (512MB limit)     │    Memory: 512MB | Uptime: Continuous          │
│   └──────────┬──────────┘                                                │
│              │                                                           │
│              │ writes Parquet files                                      │
│              ▼                                                           │
│   ┌─────────────────────────────────────────────────────────────┐       │
│   │ /data/raw/{stream-id}/year=/month=/day=/data.parquet        │       │
│   │                                                             │       │
│   │ Streams: air-quality, outdoor-weather, outdoor-air-quality, │       │
│   │          nws-observations, nws-forecast-hourly, ...         │       │
│   └─────────────────────────────────────────────────────────────┘       │
│                                                                          │
└──────────────────────────────────────────────────────────────────────────┘
                                    │
                                    │ reads Parquet (hourly via systemd timer)
                                    ▼
┌──────────────────────────────────────────────────────────────────────────┐
│                          SILVER LAYER (New - DP-006)                      │
├──────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   ┌─────────────────────────────────────────────────────────────┐       │
│   │              silver-etl (New Binary)                        │       │
│   │              Memory: 256MB | Schedule: Hourly               │       │
│   ├─────────────────────────────────────────────────────────────┤       │
│   │                                                             │       │
│   │  ┌─────────────────┐    ┌─────────────────────────────┐    │       │
│   │  │ ConfigLoader    │───▶│ config/base/streams/*.yaml  │    │       │
│   │  │ (etcd/YAML)     │    │ - silver_etl section        │    │       │
│   │  └────────┬────────┘    └─────────────────────────────┘    │       │
│   │           │                                                 │       │
│   │           ▼                                                 │       │
│   │  ┌─────────────────┐                                       │       │
│   │  │ SQL Generator   │    Generates DuckDB SQL from config   │       │
│   │  │ (DqRule,        │    - field_mappings → SELECT         │       │
│   │  │  Transforms)    │    - dq_rules → CASE expressions     │       │
│   │  └────────┬────────┘    - transforms → unit conversions    │       │
│   │           │                                                 │       │
│   │           ▼                                                 │       │
│   │  ┌─────────────────┐                                       │       │
│   │  │ DuckDB          │    read_parquet() → Transform → pg.   │       │
│   │  │ (embedded)      │    via postgres extension             │       │
│   │  └────────┬────────┘                                       │       │
│   │           │                                                 │       │
│   └───────────│─────────────────────────────────────────────────┘       │
│               │                                                          │
│               │ INSERT INTO silver.*                                     │
│               ▼                                                          │
│   ┌─────────────────────────────────────────────────────────────┐       │
│   │              TimescaleDB (256MB limit)                      │       │
│   ├─────────────────────────────────────────────────────────────┤       │
│   │                                                             │       │
│   │  silver.air_quality_observations    Hypertable              │       │
│   │  silver.weather_observations        Hypertable              │       │
│   │  silver.weather_forecasts           Hypertable              │       │
│   │  silver.outdoor_air_quality         Hypertable              │       │
│   │                                                             │       │
│   └─────────────────────────────────────────────────────────────┘       │
│                                                                          │
└──────────────────────────────────────────────────────────────────────────┘
                                    │
                                    │ SQL queries
                                    ▼
┌──────────────────────────────────────────────────────────────────────────┐
│                          PRESENTATION LAYER                               │
├──────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   ┌─────────────────────────────────────────────────────────────┐       │
│   │              Grafana (256MB limit)                          │       │
│   │              Dashboards query silver.* directly             │       │
│   └─────────────────────────────────────────────────────────────┘       │
│                                                                          │
└──────────────────────────────────────────────────────────────────────────┘
```

---

## ADR Summary

### ADR-006-001: ETL Engine Selection

**Decision**: Use **duckdb-rs embedded** in a Rust binary.

**Key Points**:
- Single binary deployment (no separate DuckDB container)
- Native Parquet support with zero-copy reads
- PostgreSQL extension for direct TimescaleDB writes
- ~200MB peak memory usage
- Proven on Raspberry Pi 5

**Fallback**: If postgres extension unreliable on ARM64, use DuckDB for reads + tokio-postgres for writes.

[Full ADR](./ADR-006-001-etl-engine-selection.md)

---

### ADR-006-002: Binary Architecture

**Decision**: Create **separate `silver-etl` binary** in `apps/silver-etl/`.

**Key Points**:
- Process isolation protects Bronze reliability
- Independent scheduling via systemd timer
- Separate memory budget (256MB)
- Can fail without impacting data capture
- Follows "Bronze must succeed, Silver is best-effort" principle

[Full ADR](./ADR-006-002-binary-architecture.md)

---

### ADR-006-003: Schema Naming Convention

**Decision**: Use **flat `silver.*` schema** for Phase 1.

**Key Points**:
- Simple queries: `SELECT * FROM silver.weather_observations`
- No schema switching for Grafana
- Easy cross-domain joins
- Migration path to `silver_aq.*`, `silver_weather.*` documented

**Naming Pattern**: `silver.{domain}_{entity_type}`
- `silver.air_quality_observations`
- `silver.weather_observations`
- `silver.weather_forecasts`
- `silver.outdoor_air_quality`

[Full ADR](./ADR-006-003-schema-naming-convention.md)

---

### ADR-006-004: DQ Rule Actions

**Decision**: Support **four DQ actions** with `flag` as default.

| Action | Value | Row | dq_flags |
|--------|-------|-----|----------|
| `flag` | Kept | Kept | Added |
| `reject` | NULL | Kept | Added |
| `clamp` | Clamped | Kept | Added |
| `drop` | - | Dropped | - |

**Key Points**:
- Transparency by default (flag, not reject)
- Different actions per field based on criticality
- All actions (except drop) recorded in `dq_flags` TEXT[] column
- Queryable audit trail

[Full ADR](./ADR-006-004-dq-rule-actions.md)

---

### ADR-006-005: Scheduling Mechanism

**Decision**: Use **systemd timer** running hourly at :05.

**Key Points**:
- `Persistent=true` catches up missed runs after reboot
- `RandomizedDelaySec=60` prevents thundering herd
- Integrated journal logging
- Restart on failure with backoff
- Standard `systemctl` tooling for operations

```ini
[Timer]
OnCalendar=*:05:00
Persistent=true
RandomizedDelaySec=60
```

[Full ADR](./ADR-006-005-scheduling-mechanism.md)

---

### ADR-006-006: Stream Type Distinction

**Decision**: Add **`stream_type` field** to stream configuration.

| Type | Description | PK Pattern |
|------|-------------|------------|
| `observations` | Continuous measurements | `(observation_time, ndp_id)` |
| `events` | Discrete state changes | `(event_time, ndp_id, event_type)` |
| `forecasts` | Future predictions | `(issue_time, valid_time, ndp_id)` |

**Key Points**:
- Forward-compatible for Home Assistant integration
- Explicit semantics guide ETL and query patterns
- Default `observations` for backward compatibility

[Full ADR](./ADR-006-006-stream-type-distinction.md)

---

## Config Schema

### silver_etl Section

```yaml
# config/base/streams/air-quality/config.yaml
stream_id: air-quality
stream_type: observations  # observations | events | forecasts

silver_etl:
  enabled: true
  target_table: silver.air_quality_observations

  timestamp:
    source_field: timestamp
    target_field: observation_time
    transform: microseconds_to_timestamp

  identity_fields:
    - source: ndp_id
      target: ndp_id

  field_mappings:
    - source_path: raw_payload.pm02
      target_column: pm25
      type: double_precision
      nullable: false
      transform: null
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 1000.0
          action: flag

    - source_path: raw_payload.atmp
      target_column: temperature_c
      type: double_precision
      transform:
        type: unit_conversion
        from: celsius
        to: celsius
        formula: { type: linear, scale: 1.0, offset: 0.0 }
      dq_rules:
        - rule: range_check
          min: -40.0
          max: 85.0
          action: flag

  dq_output:
    enabled: true
    target_column: dq_flags

  deduplication:
    enabled: true
    key_columns: [observation_time, ndp_id]
    strategy: upsert

  incremental:
    enabled: true
    watermark_column: observation_time
    lag_interval: 5 minutes
```

---

## Memory Budget

| Service | Memory Limit | Role |
|---------|-------------|------|
| mosquitto | 128MB | MQTT broker |
| etcd | 256MB | Configuration store |
| air-quality-app | 512MB | Bronze ingestion |
| TimescaleDB | 256MB | Silver storage |
| Grafana | 256MB | Dashboards |
| **silver-etl** | **256MB** | **ETL (new)** |
| **Total** | **1664MB** | **10.4% of 16GB** |

---

## Files to Create/Modify

### New Files

| Path | Purpose |
|------|---------|
| `apps/silver-etl/Cargo.toml` | ETL binary crate |
| `apps/silver-etl/src/main.rs` | Entry point |
| `apps/silver-etl/src/config.rs` | Config loader |
| `apps/silver-etl/src/etl.rs` | DuckDB ETL runner |
| `apps/silver-etl/src/sql_gen.rs` | SQL generator from config |
| `core/src/config/silver_etl.rs` | SilverEtlConfig types |
| `core/src/config/stream_type.rs` | StreamType enum |
| `deploy/pi/systemd/silver-etl.timer` | Scheduler |
| `deploy/pi/systemd/silver-etl.service` | Service unit |
| `deploy/pi/Dockerfile.silver-etl` | Container build |

### Modified Files

| Path | Change |
|------|--------|
| `core/Cargo.toml` | Add duckdb feature flag |
| `core/src/config/mod.rs` | Export silver_etl module |
| `config/base/streams/*/config.yaml` | Add `silver_etl` section |
| `deploy/pi/docker-compose.yml` | Add silver-etl service |

---

## Implementation Roadmap

### Phase 1: Foundation (Week 1)
- Create `apps/silver-etl/` crate structure
- Add duckdb-rs dependency
- Define `SilverEtlConfig` types in core
- Basic DuckDB connection test

### Phase 2: Config-Driven ETL (Week 2)
- SQL generator from config
- DQ rule SQL generation
- Transform SQL generation
- etcd config loader integration

### Phase 3: Integration (Week 3)
- Systemd timer setup
- Docker service configuration
- Monitoring dashboard
- Error handling and retry

### Phase 4: All Streams (Week 4)
- air-quality config + test
- outdoor-weather config + test
- nws-* streams config + test
- Historical backfill validation

---

## Success Criteria

| Metric | Target |
|--------|--------|
| ETL latency | < 60 seconds for hourly batch |
| Data freshness | < 5 minutes lag from Bronze |
| Memory usage | < 300MB peak for silver-etl |
| Config-only streams | Can add new stream with YAML only |
| DQ visibility | All violations in `dq_flags` column |

---

## Related Documents

| Document | Location |
|----------|----------|
| Feature Scope | `product/features/dp-006/SCOPE.md` |
| Config-Driven Design | `docs/architecture/CONFIG_DRIVEN_SILVER_ETL_DESIGN.md` |
| ETL Alternatives Research | `research/agenticdataplatform/silver/02-etl-alternatives.md` |
| Refined Synthesis | `research/agenticdataplatform/silver/06-refined-synthesis.md` |
| ETL Genericity Assessment | `research/agenticdataplatform/silver/09-etl-genericity-assessment.md` |

---

## AgentDB Patterns

The following patterns are stored in AgentDB for cross-agent discovery:

| Pattern Name | Domain | Tags |
|--------------|--------|------|
| `arch-dp-006-etl-engine` | architecture | dp-006, silver, etl, duckdb |
| `arch-dp-006-binary-separation` | architecture | dp-006, silver, binary |
| `arch-dp-006-schema-naming` | architecture | dp-006, silver, schema |
| `arch-dp-006-dq-actions` | architecture | dp-006, silver, dq |
| `arch-dp-006-scheduling` | architecture | dp-006, silver, systemd |
| `arch-dp-006-stream-types` | architecture | dp-006, silver, stream-type |

Use `get-pattern` with tags "dp-006" to discover all related patterns.

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-10 | NDP Architect | Initial architecture complete |
