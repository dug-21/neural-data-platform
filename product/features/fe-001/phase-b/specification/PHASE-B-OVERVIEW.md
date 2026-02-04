# FE-001 Phase B: First Stream (air-quality) - Overview

> **Created:** 2026-02-04
> **Phase:** B (First Stream - Reference Implementation)
> **Target:** Week 3
> **Status:** Specification Complete
> **Release Version:** v1.1.1
> **Dependencies:** Phase A complete (v11-A01, v11-A02, v11-A03, v11-A05, v1.1.0 released)

---

## Executive Summary

Phase B applies the architecture foundation established in Phase A to the `air-quality` stream as the **reference implementation**. This phase validates that the declarative infrastructure works end-to-end, from config to operational Gold layer continuous aggregates with refresh policies.

**Key Principle**: The air-quality stream serves as the exemplar. Every pattern established here will be replicated for subsequent streams via config-only changes.

**Exit Criteria**: `gold.air_quality_hourly` and `gold.air_quality_daily` continuous aggregates operational, refresh policies running, at least one feature type (lag or rolling) working.

---

## Phase B Features

| ID | Feature | Priority | Specification |
|----|---------|----------|---------------|
| v11-001 | Stream Type Classification | High | [SPEC-B01](./SPEC-B01-stream-type-classification.md) |
| v11-002 | Classification Propagation | High | [SPEC-B02](./SPEC-B02-classification-propagation.md) |
| v11-003 | Per-Stream Continuous Aggregates | Critical | [SPEC-B03](./SPEC-B03-continuous-aggregates.md) |
| v11-004 | Aggregate Refresh Policy | High | [SPEC-B04](./SPEC-B04-refresh-policy.md) |

---

## Pre-Deployment Requirements

### Tool Build Requirement

Phase B is the first phase that **deploys** Gold layer objects to the Pi. The `ndp-gold-ddl` tool must be available on the target device.

**Gap Identified in v1.1.0:** Phase A added the tool code but did not include a mechanism to build it on the Pi.

**Solution Options:**

1. **Add `tool` declaration type** to deploy.sh:
   ```json
   {"type": "tool", "id": "ndp-gold-ddl", "action": "build"}
   ```

2. **Add cargo build step** to deploy.sh pre-flight:
   ```bash
   # In deploy.sh apply(), before Phase 5 (Gold Tables)
   cargo build --release -p ndp-gold-ddl
   ```

3. **Include in container build** process

**Chosen Approach:** Add cargo build step for tools in deploy.sh (simplest, maintains current architecture).

### Release Preparation (RELEASE-POLICY Compliance)

Phase B deployment to Pi requires proper release packaging per `docs/procedures/RELEASE-POLICY.md`:

| Artifact | Location | Status |
|----------|----------|--------|
| Manifest | `.deploy/releases/v1.1.1.manifest.json` | Create during Phase B |
| Git Tag | `v1.1.1` (annotated) | Create after implementation |
| Changelog | `CHANGELOG.md` | Update with v1.1.1 section |

**Version:** v1.1.1 (PATCH - builds on v1.1.0 Gold foundation)

---

## Dependency Graph

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         PHASE A (PREREQUISITES)                              │
│                                                                              │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐          │
│  │ v11-A01          │  │ v11-A02          │  │ v11-A05          │          │
│  │ Gold ETL Schema  │  │ Gold DDL Tool    │  │ Objectives       │          │
│  │ (COMPLETE)       │  │ (COMPLETE)       │  │ Schema           │          │
│  └────────┬─────────┘  └────────┬─────────┘  └──────────────────┘          │
│           │                     │                                            │
└───────────┼─────────────────────┼────────────────────────────────────────────┘
            │                     │
            ▼                     │
┌───────────────────────┐         │
│ v11-001               │         │
│ Stream Type           │         │
│ Classification        │         │
└───────────┬───────────┘         │
            │                     │
            ▼                     │
┌───────────────────────┐         │
│ v11-002               │         │
│ Classification        │◄────────┘
│ Propagation           │
└───────────┬───────────┘
            │
            ▼
┌───────────────────────┐
│ v11-003               │
│ Per-Stream Continuous │
│ Aggregates            │
│ (air-quality)         │
└───────────┬───────────┘
            │
            ▼
┌───────────────────────┐
│ v11-004               │
│ Aggregate Refresh     │
│ Policy                │
└───────────┬───────────┘
            │
            ▼
      PHASE B COMPLETE
            │
            ▼
      PHASE C READY
```

### Dependency Details

| Feature | Depends On | Blocking For |
|---------|------------|--------------|
| v11-001 | V1.0 stream config, v11-A01 schema | v11-002 |
| v11-002 | v11-001, v11-A02 | v11-003, Phase C alignment |
| v11-003 | v11-A02, v11-002 | v11-004, Phase C multi-stream |
| v11-004 | v11-003 | Phase B completion |

---

## Implementation Order

Based on dependencies, the recommended implementation order is:

### Day 1: Stream Classification

1. **v11-001: Stream Type Classification** (Day 1, AM)
   - Add `stream_type` field to air-quality config
   - Validates schema integration with v11-A01
   - Minimal change, validates config → schema flow

2. **v11-002: Classification Propagation** (Day 1, PM)
   - Ensure `stream_type` flows to Silver metadata
   - Update data dictionary with classification
   - Enables Phase C correlation role assignment

### Day 2-4: Continuous Aggregates

3. **v11-003: Per-Stream Continuous Aggregates** (Day 2-4)
   - Configure `gold_etl` section in air-quality config
   - Generate DDL via `ndp-gold-ddl`
   - Deploy `gold.air_quality_hourly` and `gold.air_quality_daily`
   - Most complex feature - validates full pipeline

### Day 5: Refresh Policy

4. **v11-004: Aggregate Refresh Policy** (Day 5)
   - Configure refresh policy in config
   - Deploy policy via `ndp-gold-ddl`
   - Verify automatic refresh on Pi

---

## Air Quality Config Extension

This is the target configuration for air-quality after Phase B completion:

```yaml
# config/base/streams/air-quality/config.yaml
# Additions for Phase B highlighted with comments

stream_id: "air-quality"
description: "AirGradient sensor readings from MQTT"
version: "1.1.0"                    # Version bump for Gold layer
enabled: true

# NEW: Stream type classification (v11-001)
stream_type: "observation"          # observation | state_event | forecast | dimension

# ... existing fields, sources, storage, entity_schemas ...

# Existing Silver ETL (unchanged)
silver_etl:
  enabled: true
  target_table: silver.air_quality_observations
  # ... rest of silver_etl unchanged ...

# NEW: Gold ETL configuration (v11-003, v11-004)
gold_etl:
  enabled: true
  description: "Hourly and daily aggregates for air quality metrics"

  aggregates:
    granularities: ["1 hour", "1 day"]
    default_metrics: ["mean", "count"]
    fields:
      pm25:
        metrics: ["mean", "std", "min", "max", "p95"]
      pm10:
        metrics: ["mean", "min", "max"]
      co2:
        metrics: ["mean", "std", "min", "max"]
      temperature_c:
        metrics: ["mean", "min", "max"]
      humidity_pct:
        metrics: ["mean", "min", "max"]
      tvoc_index:
        metrics: ["mean", "max"]
      nox_index:
        metrics: ["mean", "max"]

  features:
    lag:
      enabled: true
      lags_hours: [1, 6, 24]
      fields: ["pm25", "co2"]
    rolling:
      enabled: true
      windows: ["4 hours", "24 hours"]
      stats: ["mean", "std"]
      fields: ["pm25"]
    trend:
      enabled: true
      window: "4 hours"
      fields: ["pm25", "co2"]

  # Refresh policy (v11-004)
  refresh_policy:
    schedule_interval: "15 minutes"
    start_offset: "4 hours"
    end_offset: "15 minutes"
```

---

## Generated Artifacts

### TimescaleDB Objects

| Object | Type | Generated By |
|--------|------|--------------|
| `gold.air_quality_hourly` | Continuous Aggregate | v11-003 via ndp-gold-ddl |
| `gold.air_quality_daily` | Continuous Aggregate | v11-003 via ndp-gold-ddl |
| (policy for hourly) | Refresh Policy | v11-004 via ndp-gold-ddl |
| (policy for daily) | Refresh Policy | v11-004 via ndp-gold-ddl |

### Data Dictionary Entries

| Table | Entries |
|-------|---------|
| `data_dictionary.stream_classification` | air-quality, stream_type=observation |
| `data_dictionary.gold_tables` | gold.air_quality_hourly, gold.air_quality_daily |
| `data_dictionary.gold_columns` | All computed columns with feature_type |

---

## Shared Interfaces

### Generated SQL Templates

The following SQL will be generated by `ndp-gold-ddl`:

#### Hourly Continuous Aggregate

```sql
CREATE MATERIALIZED VIEW gold.air_quality_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', observation_time) AS bucket,
    ndp_id,

    -- PM2.5 metrics (pm25: mean, std, min, max, p95)
    AVG(pm25) AS pm25_mean,
    STDDEV(pm25) AS pm25_std,
    MIN(pm25) AS pm25_min,
    MAX(pm25) AS pm25_max,
    PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY pm25) AS pm25_p95,

    -- PM10 metrics (pm10: mean, min, max)
    AVG(pm10) AS pm10_mean,
    MIN(pm10) AS pm10_min,
    MAX(pm10) AS pm10_max,

    -- CO2 metrics (co2: mean, std, min, max)
    AVG(co2) AS co2_mean,
    STDDEV(co2) AS co2_std,
    MIN(co2) AS co2_min,
    MAX(co2) AS co2_max,

    -- Temperature metrics
    AVG(temperature_c) AS temperature_c_mean,
    MIN(temperature_c) AS temperature_c_min,
    MAX(temperature_c) AS temperature_c_max,

    -- Humidity metrics
    AVG(humidity_pct) AS humidity_pct_mean,
    MIN(humidity_pct) AS humidity_pct_min,
    MAX(humidity_pct) AS humidity_pct_max,

    -- TVOC metrics
    AVG(tvoc_index) AS tvoc_index_mean,
    MAX(tvoc_index) AS tvoc_index_max,

    -- NOx metrics
    AVG(nox_index) AS nox_index_mean,
    MAX(nox_index) AS nox_index_max,

    -- Sample count (default metric)
    COUNT(*) AS sample_count

FROM silver.air_quality_observations
GROUP BY bucket, ndp_id;
```

#### Refresh Policy

```sql
SELECT add_continuous_aggregate_policy('gold.air_quality_hourly',
    start_offset => INTERVAL '4 hours',
    end_offset => INTERVAL '15 minutes',
    schedule_interval => INTERVAL '15 minutes'
);
```

---

## Integration Test Requirements

### Test: End-to-End Gold Deployment

```bash
# Deploy Phase B to Pi
deploy.sh apply .deploy/test/phase-b-air-quality.manifest.json

# Verify continuous aggregate exists
dcx timescaledb psql -U postgres -d ndp -c "
SELECT view_name
FROM timescaledb_information.continuous_aggregates
WHERE view_schema = 'gold'
  AND view_name LIKE 'air_quality%';
"
# Expected: air_quality_hourly, air_quality_daily

# Verify refresh policy exists
dcx timescaledb psql -U postgres -d ndp -c "
SELECT view_name, schedule_interval, refresh_lag
FROM timescaledb_information.continuous_aggregate_stats
WHERE view_name = 'gold.air_quality_hourly';
"

# Verify data is being aggregated
dcx timescaledb psql -U postgres -d ndp -c "
SELECT bucket, pm25_mean, co2_mean, sample_count
FROM gold.air_quality_hourly
ORDER BY bucket DESC
LIMIT 5;
"
```

### Test: Config-Only Field Addition

```bash
# Add a new metric to existing field (config change only)
# Edit: gold_etl.aggregates.fields.pm25.metrics += "p99"

# Regenerate DDL
ndp-gold-ddl generate --stream air-quality --action recreate

# The action must be "recreate" because continuous aggregates cannot be altered
# Verify new column appears after recreate
```

### Test: Query Performance

```bash
# 30-day range query must complete in < 100ms on Pi
dcx timescaledb psql -U postgres -d ndp -c "
EXPLAIN ANALYZE
SELECT bucket, pm25_mean, co2_mean
FROM gold.air_quality_hourly
WHERE bucket >= NOW() - INTERVAL '30 days';
"
# Expected: Execution time < 100ms
```

---

## Manifest for Phase B (v1.1.1)

Per `docs/procedures/RELEASE-POLICY.md`, the release manifest:

**Location:** `.deploy/releases/v1.1.1.manifest.json`

```json
{
  "$schema": "../schemas/manifest.schema.json",
  "version": "1.0",
  "release_version": "1.1.1",
  "description": "Release v1.1.1: Gold Layer - air-quality continuous aggregates (Phase B)",
  "changes": [
    {
      "type": "tool",
      "id": "ndp-gold-ddl",
      "action": "build"
    },
    {
      "type": "stream",
      "id": "air-quality",
      "action": "update",
      "reload": "none"
    },
    {
      "type": "gold-tables",
      "stream_id": "air-quality",
      "action": "sync"
    },
    {
      "type": "dictionary",
      "action": "sync"
    }
  ]
}
```

**Manifest Changelog:**
- `tool` declaration builds `ndp-gold-ddl` (new in v1.1.1)
- `stream` updates air-quality config with `gold_etl.enabled: true`
- `gold-tables` creates continuous aggregates via ndp-gold-ddl
- `dictionary` updates data dictionary with Gold table metadata

---

## Exit Criteria Checklist

### Phase B Complete When:

**Feature Implementation:**
- [ ] **v11-001**: `stream_type: observation` present in air-quality config
- [ ] **v11-002**: `data_dictionary.stream_classification` contains air-quality entry
- [ ] **v11-003**: `gold.air_quality_hourly` continuous aggregate operational
- [ ] **v11-003**: `gold.air_quality_daily` continuous aggregate operational
- [ ] **v11-004**: Refresh policy running (every 15 min)
- [ ] Query performance < 100ms for 30-day range on Pi
- [ ] **Config-only change can modify aggregate fields** (architecture validation)

**Tool Build Infrastructure:**
- [ ] deploy.sh has mechanism to build `ndp-gold-ddl` tool
- [ ] `tool` declaration type implemented OR cargo build step added
- [ ] Tool available at `/opt/ndp/bin/ndp-gold-ddl` OR `target/release/ndp-gold-ddl`

**Release Preparation (RELEASE-POLICY):**
- [ ] `.deploy/releases/v1.1.1.manifest.json` created and valid
- [ ] `CHANGELOG.md` updated with v1.1.1 section
- [ ] Git tag `v1.1.1` created (annotated)
- [ ] Code and tag pushed to remote

### Architecture Validation Checkpoint:

Before proceeding to Phase C:
- [ ] Adding a new metric (e.g., p99) requires only config edit + manifest recreate
- [ ] No Rust code changes needed for metric addition
- [ ] Deploy.sh correctly calls ndp-gold-ddl
- [ ] Pi deployment successful via `deploy.sh apply .deploy/releases/v1.1.1.manifest.json`

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Continuous aggregate refresh too expensive on Pi | Medium | High | Profile refresh cost; adjust window to 30 min if needed |
| Percentile computation (p95) expensive | Medium | Medium | Test on Pi early; fall back to approximation if needed |
| Config regeneration doesn't trigger recreate | Low | High | Explicit test for config change scenario |
| Data dictionary sync misses Gold tables | Low | Medium | Verify sync function extension |

---

## Team Assignments

| Feature | Lead Agent | Supporting Agents |
|---------|------------|-------------------|
| v11-001 | `ndp-architect` | - |
| v11-002 | `ndp-timescale-dev` | `ndp-architect` |
| v11-003 | `ndp-rust-dev` | `ndp-timescale-dev`, `ndp-tester` |
| v11-004 | `ndp-timescale-dev` | `ndp-rust-dev` |

---

## References

- [SCOPE.md](../../SCOPE.md) - Full V1.1 scope definition (Phase B section)
- [DECISIONS.md](../../architecture/DECISIONS.md) - Architecture decisions
- [CONFIG-DEPLOYMENT-FLOW.md](../../architecture/CONFIG-DEPLOYMENT-FLOW.md) - Phase 5 Gold tables
- [ADR-FE001-001](../../architecture/DECISIONS.md#adr-fe001-001-gold-ddl-generation-in-rust) - Gold DDL in Rust
- [SPEC-A01](../phase-a/specification/SPEC-A01-gold-etl-schema.md) - Gold ETL schema reference
- [Air Quality Config](../../../../config/base/streams/air-quality/config.yaml) - Current stream config

---

*Phase B Specification created: 2026-02-04*
