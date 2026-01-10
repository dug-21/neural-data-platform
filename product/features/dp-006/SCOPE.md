# DP-006: Silver Layer Implementation

**Feature ID**: dp-006
**Title**: Silver Layer - Config-Driven ETL to TimescaleDB
**Status**: Scope Definition
**Created**: 2026-01-05
**Research**: `research/agenticdataplatform/silver/`

---

## Executive Summary

Implement the Silver layer of the Neural Data Platform, transforming raw Bronze Parquet data into clean, typed TimescaleDB tables optimized for analytics and dashboard queries. The implementation follows a config-driven approach extending the existing GitOps patterns established in Bronze.

---

## Context

### Platform Vision

NDP is intended as a **generic data platform** capable of supporting N data domains. Weather and air quality is the **first domain**, with the primary use case being:

> *"When should I open/close the window for optimal indoor air quality?"*

This requires correlating:
- Indoor air quality (PM2.5, CO2, temperature, humidity)
- Outdoor air quality (PM2.5, ozone, AQI)
- Outdoor weather (temperature, humidity, wind)
- Weather forecasts (what's coming in 1-6 hours)
- Home state (window open/closed, HVAC mode) — **future stream**

### Current State

- **Bronze layer**: Operational with 7 streams writing to Parquet
- **DuckDB container**: Removed from Pi deployment
- **Config infrastructure**: etcd-based GitOps pattern proven
- **Parser types**: Generic `FieldMapping`, `UnitConversion`, `ConversionFormula` available

### Priority Stack

```
NOW:     Silver layer data pipeline (this feature)
NEXT:    Window trigger logic (Gold layer, dp-007+)
LATER:   Additional domains (Home Assistant, financial, observability)
FUTURE:  Agentic real-time capabilities on edge
```

---

## Scope

### In Scope

| Component | Description |
|-----------|-------------|
| **silver-etl binary** | Separate Rust binary for Bronze→Silver ETL |
| **duckdb-rs embedded** | DuckDB embedded in Rust for Parquet reading + PostgreSQL writes |
| **Config schema** | `silver_etl` section added to stream YAML configs |
| **4 Silver tables** | Initial weather/AQ domain tables in TimescaleDB |
| **DQ transparency** | Config-driven DQ rules with `dq_flags` column |
| **Systemd scheduling** | Timer-based hourly ETL execution |
| **Grafana datasource** | TimescaleDB connection for dashboard migration |

### Initial Silver Tables

| Table | Source Streams | Pattern |
|-------|----------------|---------|
| `silver.air_quality_observations` | air-quality | Observations |
| `silver.weather_observations` | outdoor-weather, nws-observations | Observations |
| `silver.weather_forecasts` | nws-forecast-hourly, nws-gridpoints-forecast | Observations |
| `silver.outdoor_air_quality` | outdoor-air-quality | Observations |

### Out of Scope (Future Features)

| Item | Reason | Target |
|------|--------|--------|
| Gold layer / feature store | Separate concern (trigger logic) | dp-007+ |
| Home Assistant integration | Requires new source type | dp-008+ |
| Financial/energy domains | Secondary priority | Future |
| Advanced transforms (delta, decimal) | Not needed for weather/AQ | Future |
| Multi-tenancy/namespacing | Over-engineering for now | Phase 3 |
| Dashboard migration | Separate effort after Silver stable | dp-007+ |
| Continuous aggregates | Optimization after baseline | dp-007+ |

---

## Principles

### 1. Config-Driven Extensibility

> Adding a new stream to Silver should require **YAML changes only**, no Rust code modifications.

The `silver_etl` section extends existing stream configs:

```yaml
stream_id: air-quality
# ... existing Bronze config ...

silver_etl:
  enabled: true
  target_table: silver.air_quality_observations
  field_mappings:
    - source_path: raw_payload.pm02
      target_column: pm25
      type: double_precision
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 1000.0
          action: flag
```

### 2. Bronze Must Succeed, Silver is Best-Effort

> Per ADR-001: Silver can be rebuilt from Bronze. Bronze reliability must not be compromised by Silver failures.

This mandates:
- Separate binary (process isolation)
- Independent failure handling
- No blocking of Bronze writes

### 3. Build for First Domain, Design for N

> Ship working Silver for weather/AQ. Don't over-engineer for hypothetical future domains.

But:
- Schema patterns should be generalizable
- Config schema should accommodate future stream types
- Document extensibility path for Phase 2

### 4. Observations vs Events

> Distinguish continuous time-series (observations) from discrete state changes (events).

Current streams are all observations. Future Home Assistant streams will be events. The config schema should accommodate both:

```yaml
stream_type: observations  # continuous measurements (default)
# or
stream_type: events        # discrete state changes (future)
```

### 5. DQ Transparency Over Rejection

> Prefer flagging bad data over rejecting it. Transparency enables investigation.

Default DQ action is `flag` (keep value, record violation). Rejection reserved for truly invalid data that would break downstream.

---

## Expectations

### Deliverables

| Deliverable | Description |
|-------------|-------------|
| `apps/silver-etl/` | New Rust binary crate |
| `core/src/config/silver_etl.rs` | Config types for Silver ETL |
| Stream config updates | `silver_etl` section for 4 streams |
| TimescaleDB init scripts | Schema creation, hypertables, indexes |
| Systemd units | `silver-etl.service`, `silver-etl.timer` |
| Grafana datasource | TimescaleDB provisioning config |

### Success Criteria

| Metric | Target |
|--------|--------|
| ETL latency | < 60 seconds for hourly batch |
| Data freshness | < 5 minutes lag from Bronze |
| Memory usage | < 300MB peak for silver-etl process |
| Config-only streams | Can add new stream with YAML only |
| DQ visibility | All violations captured in `dq_flags` |

### Non-Goals

- Real-time streaming ETL (batch is sufficient)
- Sub-minute data freshness
- Dashboard migration (separate effort)
- Trigger/notification logic (Gold layer)

---

## ADR Proposals

The following architectural decisions require formal documentation during specification:

### ADR-006-001: ETL Engine Selection

**Question**: Which engine for Bronze→Silver transformation?

| Option | Pros | Cons |
|--------|------|------|
| **duckdb-rs embedded** | Single binary, proven PostgreSQL writes, Pi-compatible | New dependency |
| Polars + tokio-postgres | Already in dependencies | More code, no direct DB writes |
| pg_parquet FDW | Minimal code | Lower performance, less flexible |

**Recommendation**: duckdb-rs embedded

### ADR-006-002: Binary Architecture

**Question**: Integrate Silver ETL into air-quality-app or separate binary?

| Option | Pros | Cons |
|--------|------|------|
| **Separate binary** | Process isolation, independent failures | Additional deployment artifact |
| Integrated | Single deployment | Silver failure could impact Bronze |

**Recommendation**: Separate binary (protects Bronze reliability)

### ADR-006-003: Schema Naming Convention

**Question**: How to name Silver tables for future multi-domain support?

| Option | Now | Future |
|--------|-----|--------|
| **Flat in silver schema** | `silver.air_quality_observations` | Migrate to `silver_aq.*` when needed |
| Domain schemas from start | `silver_aq.observations` | Already namespaced |

**Recommendation**: Flat `silver.*` for Phase 1, document migration path to domain schemas

### ADR-006-004: DQ Rule Actions

**Question**: What actions should DQ rules support?

| Action | Behavior | Use Case |
|--------|----------|----------|
| `flag` | Keep value, add to dq_flags | Default, transparency |
| `reject` | Set NULL, add to dq_flags | Invalid data that breaks queries |
| `clamp` | Clamp to bounds, add flag | Bounded metrics (0-100%) |
| `drop` | Drop entire row | Catastrophically invalid |

**Recommendation**: Support all four, default to `flag`

### ADR-006-005: Scheduling Mechanism

**Question**: How to trigger ETL execution?

| Option | Pros | Cons |
|--------|------|------|
| **Systemd timer** | Standard Linux, Persistent=true for catch-up | External to application |
| Embedded scheduler | Single binary | More code, state management |
| File watch trigger | Event-driven | Complex, timing issues |

**Recommendation**: Systemd timer (hourly, 5 minutes past hour)

### ADR-006-006: Stream Type Distinction

**Question**: Should config schema distinguish observations from events?

| Option | Pros | Cons |
|--------|------|------|
| **Add stream_type field** | Forward-compatible for Home Assistant | Slight over-design |
| Implicit from config | Simpler now | Refactor later |

**Recommendation**: Add `stream_type: observations | events` to schema

---

## Research References

| Document | Key Content |
|----------|-------------|
| `01-scope-definition.md` | 4-entity model, stream mapping |
| `02-etl-alternatives.md` | DuckDB vs Polars vs native comparison |
| `03-data-dictionary.md` | Complete typed schemas, unit standards |
| `04-dashboard-integration.md` | Grafana migration patterns |
| `05-synthesis.md` | Original research synthesis |
| `06-refined-synthesis.md` | Updated with duckdb-rs, config-driven approach |
| `07-ml-platform-assessment.md` | Multi-domain ML readiness |
| `08-platform-architecture-assessment.md` | Schema extensibility analysis |
| `09-etl-genericity-assessment.md` | Transform/DQ genericity gaps |
| `CONFIG_DRIVEN_SILVER_ETL_DESIGN.md` | Full design document |

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| TimescaleDB container | Not deployed | Needs docker-compose addition |
| duckdb-rs crate | Not in Cargo.toml | Add with `bundled` feature |
| PostgreSQL extension | Requires runtime install | DuckDB `INSTALL postgres` |
| etcd config sync | Operational | Reuse existing pattern |

---

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| duckdb-rs PostgreSQL extension issues on ARM64 | Low | High | Fallback to Polars + tokio-postgres |
| TimescaleDB memory pressure on Pi | Low | Medium | 256MB limit, monitoring |
| Config schema changes break Bronze | Low | High | Separate config sections, versioning |
| ETL takes too long for hourly cadence | Low | Medium | Optimize queries, increase interval |

---

## Next Steps

1. **Create STATUS.md** — Initialize feature tracking
2. **Specification phase** — Formalize requirements, finalize ADRs
3. **Architecture phase** — Write ADR documents for decisions above
4. **Implementation** — TDD development of silver-etl binary

---

*Scope defined: 2026-01-05*
*Research basis: Silver Layer Research Swarm (9 documents)*
