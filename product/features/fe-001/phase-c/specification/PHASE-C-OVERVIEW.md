# FE-001 Phase C: Cross-Stream + Alignment - Overview

> **Created:** 2026-02-04
> **Phase:** C (Cross-Stream + Alignment)
> **Target:** Week 4
> **Status:** Specification Complete

---

## Executive Summary

Phase C extends the Gold layer to three streams and introduces the cross-stream aligned view. This phase implements the JOIN complexity that enables V1.2 pattern detection by providing a single queryable view across all domain streams.

**Exit Criteria**: 3 streams in Gold layer (air-quality, outdoor-weather, home-assistant-state), aligned view operational, state transitions extractable, objectives stored in data dictionary.

**Deliberately Excluded**: `outdoor-air-quality` stream (reserved for Phase D fast-follower test).

---

## Phase C Features

| ID | Feature | Priority | Specification | Dependencies |
|----|---------|----------|---------------|--------------|
| v11-005 | Cross-Stream Aligned View | Critical | [SPEC-C01](./SPEC-C01-aligned-view.md) | Phase A (v11-A04), Phase B (v11-003) |
| v11-006 | State Transition Materializer | High | [SPEC-C02](./SPEC-C02-state-transitions.md) | Phase B (v11-003 for home-assistant-state) |
| v11-007 | Objectives Storage | Medium | [SPEC-C03](./SPEC-C03-objectives-storage.md) | Phase A (v11-A05) |
| v11-003 | Per-Stream Continuous Aggregates (outdoor-weather, state-events) | Critical | Phase B spec, extended | Phase A (v11-A02) |

---

## Dependencies from Prior Phases

### Phase A Dependencies (Architecture Foundation)

| Dependency | Required By | Status |
|------------|-------------|--------|
| v11-A03: Alignment JSON Schema | v11-005 Aligned View | Phase A delivers |
| v11-A04: Alignment Interpreter | v11-005 Aligned View | Phase A delivers |
| v11-A05: Objectives JSON Schema | v11-007 Objectives Storage | Phase A delivers |
| v11-A02: Gold DDL Tool | All Phase C features | Phase A delivers |

### Phase B Dependencies (First Stream)

| Dependency | Required By | Status |
|------------|-------------|--------|
| v11-003: air-quality continuous aggregate | v11-005 Aligned View | Phase B delivers |
| v11-A06: Feature Type Registry | Feature computation in aligned view | Phase B delivers |
| gold_etl config pattern | outdoor-weather, state-events Gold | Phase B establishes pattern |

---

## Dependency Graph

```
Phase A (Architecture)                    Phase B (First Stream)
┌─────────────────────┐                  ┌─────────────────────┐
│ v11-A03             │                  │ v11-003             │
│ Alignment Schema    │─────────────────►│ air-quality hourly  │
└─────────────────────┘                  │ (continuous agg)    │
         │                               └──────────┬──────────┘
         │                                          │
         ▼                                          │
┌─────────────────────┐                             │
│ v11-A04             │                             │
│ Alignment           │                             │
│ Interpreter         │─────────────────────────────┤
└─────────────────────┘                             │
                                                    │
Phase C (Cross-Stream)                              │
┌─────────────────────────────────────────────────────────────────┐
│                                                                  │
│  ┌─────────────────────┐    ┌─────────────────────┐             │
│  │ v11-003 (extended)  │    │ v11-003 (extended)  │             │
│  │ outdoor-weather     │    │ home-assistant-state│             │
│  │ hourly              │    │ hourly              │             │
│  └──────────┬──────────┘    └──────────┬──────────┘             │
│             │                          │                        │
│             │    ┌─────────────────────┘                        │
│             │    │                                              │
│             ▼    ▼                                              │
│  ┌─────────────────────────────────────────────────┐            │
│  │ v11-005: Cross-Stream Aligned View              │◄───────────┤
│  │ gold.indoor_air_quality_aligned                 │            │
│  └──────────────────────────────────────────────────┘           │
│                      │                                          │
│                      │                                          │
│  ┌───────────────────┴───────────────────┐                     │
│  │                                        │                     │
│  ▼                                        ▼                     │
│  ┌─────────────────────┐    ┌─────────────────────┐            │
│  │ v11-006             │    │ v11-007             │            │
│  │ State Transition    │    │ Objectives          │            │
│  │ Materializer        │    │ Storage             │            │
│  └─────────────────────┘    └─────────────────────┘            │
│                                                                  │
└──────────────────────────────────────────────────────────────────┘
```

---

## Streams in Phase C Aligned View

### 1. air-quality (observation)

| Property | Value |
|----------|-------|
| Stream Type | `observation` |
| Role | `primary` (what we optimize) |
| Source Silver Table | `silver.air_quality_observations` |
| Gold Aggregate | `gold.air_quality_hourly` |
| NULL Handling | Preserve NULL |
| Key Metrics | pm25, co2, temperature_c, humidity_pct |

### 2. outdoor-weather (observation)

| Property | Value |
|----------|-------|
| Stream Type | `observation` |
| Role | `context` |
| Source Silver Table | `silver.weather_observations` |
| Gold Aggregate | `gold.outdoor_weather_hourly` |
| NULL Handling | Preserve NULL |
| Key Metrics | temperature_c, humidity_pct, wind_speed_kmh, pressure_pa |

### 3. home-assistant-state (state_event)

| Property | Value |
|----------|-------|
| Stream Type | `state_event` |
| Role | `actuator` (potential causes) |
| Source Silver Table | `silver.state_events` |
| Gold Aggregate | `gold.state_events_hourly` |
| NULL Handling | Carry Forward (LOCF) |
| Key Metrics | window_open_count, door_open_count, state_changes_count |

---

## Domain Configuration

Phase C uses the domain configuration pattern established in ADR-FE001-002:

**Location**: `config/domains/indoor-air-quality/domain.yaml`

```yaml
domain:
  id: indoor-air-quality
  description: "Maintain healthy indoor air quality"

  streams:
    - stream_id: air-quality
      alias: indoor
      role: primary
    - stream_id: outdoor-weather
      alias: outdoor
      role: context
    - stream_id: home-assistant-state
      alias: state
      role: actuator

  alignment:
    view_name: indoor_air_quality_aligned
    granularity: "1 hour"
    join_strategy: full_outer
    null_handling: by_stream_type  # ADR-FE001-004

  objectives:
    - id: healthy_co2
      target:
        stream: air-quality
        metric: co2
        condition: "<"
        threshold: 800
        unit: ppm
      priority: high

    - id: healthy_pm25
      target:
        stream: air-quality
        metric: pm25
        condition: "<"
        threshold: 12
        unit: ug/m3
      priority: high
```

---

## Key Architecture Decisions Applied

### ADR-FE001-003: Forecast Alignment (Not Used in Phase C)

Phase C does not include forecast streams. The `nws-forecast-hourly` stream is reserved for Phase D to demonstrate the forecast alignment pattern (join on `issued_at`).

### ADR-FE001-004: NULL Handling by Stream Type

| Stream | Type | NULL Handling | Implementation |
|--------|------|---------------|----------------|
| air-quality | observation | Preserve NULL | Direct column reference |
| outdoor-weather | observation | Preserve NULL | Direct column reference |
| home-assistant-state | state_event | Carry Forward (LOCF) | `COALESCE(col, LAG(col) IGNORE NULLS OVER (...))` |

### Decision 7: One Aligned View Per Domain

Phase C creates `gold.indoor_air_quality_aligned` - scoped to the indoor-air-quality domain. This is NOT a platform-wide view.

### Decision 11: Idempotency via Manifest-Declared Actions

Manifest must declare:
- `action: sync` for new continuous aggregates
- `action: recreate` if `gold_etl` config changed

---

## Implementation Order

### Step 1: Extend Continuous Aggregates (outdoor-weather, state-events)

Before the aligned view can be created, all source streams need Gold aggregates:

| Stream | Gold Aggregate | Generated By |
|--------|----------------|--------------|
| outdoor-weather | `gold.outdoor_weather_hourly` | `ndp-gold-ddl generate --stream outdoor-weather` |
| home-assistant-state | `gold.state_events_hourly` | `ndp-gold-ddl generate --stream home-assistant-state` |

**Note**: `gold.air_quality_hourly` already exists from Phase B.

### Step 2: Create Domain Configuration

File: `config/domains/indoor-air-quality/domain.yaml`

### Step 3: Generate Aligned View

```bash
ndp-gold-ddl generate --domain indoor-air-quality
```

This generates:
1. `gold.indoor_air_quality_aligned` (materialized view)
2. `gold.state_transitions` (view for state change extraction)
3. Refresh policies

### Step 4: Store Objectives in Data Dictionary

Objectives are stored in `data_dictionary.objectives` table.

---

## Test Strategy

### Unit Tests

| Feature | Test File | Key Test Cases |
|---------|-----------|----------------|
| v11-005 | `aligned_view_test.rs` | JOIN generation, NULL handling, column aliasing |
| v11-006 | `state_transitions_test.rs` | Transition detection, `is_actual_transition`, duration |
| v11-007 | `objectives_storage_test.rs` | Insert, query, condition types |

### Integration Tests

| Test | Description | Features |
|------|-------------|----------|
| `aligned_view_query.sql` | Query aligned view for 30 days | v11-005 |
| `state_transition_extraction.sql` | Extract transitions from state stream | v11-006 |
| `objectives_sync.sh` | Sync objectives from config to data dictionary | v11-007 |

### Performance Tests

| Test | Target | Measurement |
|------|--------|-------------|
| Aligned view query (30 days) | < 100ms | `EXPLAIN ANALYZE` |
| Continuous aggregate refresh | < 5% CPU sustained | Pi 5 monitoring |
| State transition extraction | < 50ms | `EXPLAIN ANALYZE` |

---

## File Inventory

### New Files (Phase C Creates)

```
config/domains/
└── indoor-air-quality/
    └── domain.yaml                    # Domain configuration (v11-005, v11-007)

config/base/streams/
├── outdoor-weather/
│   └── config.yaml                    # Extended with gold_etl section
└── home-assistant-state/
    └── config.yaml                    # Extended with gold_etl section

tools/ndp-gold-ddl/src/generators/
└── state_transitions.rs               # State transition SQL generator (v11-006)
```

### Modified Files (Phase C Extends)

```
deploy/pi/deploy.sh                    # handle_domain() called with indoor-air-quality
tools/ndp-gold-ddl/src/generators/
├── aligned_view.rs                    # Generates indoor_air_quality_aligned
└── mod.rs                             # Register state_transitions generator
core/src/gold/config.rs                # DomainConfig, ObjectivesConfig types
```

### SQL Artifacts Generated

```
gold.outdoor_weather_hourly            # Continuous aggregate
gold.state_events_hourly               # Continuous aggregate
gold.indoor_air_quality_aligned        # Aligned view (materialized)
gold.state_transitions                 # State transition view
data_dictionary.objectives             # Objectives metadata
data_dictionary.domains                # Domain metadata
data_dictionary.domain_streams         # Domain-stream mappings
```

---

## Exit Criteria Checklist

### Phase C Complete When:

- [ ] `gold.outdoor_weather_hourly` continuous aggregate operational
- [ ] `gold.state_events_hourly` continuous aggregate operational
- [ ] `gold.indoor_air_quality_aligned` view returns data for all 3 streams
- [ ] NULL handling correct by stream type (preserve for observation, LOCF for state)
- [ ] `gold.state_transitions` extracts state changes with `is_actual_transition`
- [ ] Objectives stored in `data_dictionary.objectives`
- [ ] Aligned view query < 100ms for 30-day range
- [ ] All unit tests passing
- [ ] `outdoor-air-quality` NOT in Gold layer (reserved for Phase D)

### Review Checklist:

- [ ] Domain config validates against schema
- [ ] Aligned view uses correct JOIN strategy (FULL OUTER)
- [ ] State columns use LOCF NULL handling
- [ ] Observation columns preserve NULL
- [ ] State transition `is_actual_transition` filters noise correctly
- [ ] Objectives have all condition types covered

---

## Resource Constraints (Pi 5)

### Memory Budget

| Component | Allocation | Notes |
|-----------|------------|-------|
| outdoor-weather aggregate refresh | < 50 MB | Per-stream, sequential |
| state-events aggregate refresh | < 30 MB | Smaller dataset |
| Aligned view query | < 50 MB | For 30-day range |
| **Total Phase C addition** | < 130 MB | During peak operations |

### Storage Estimates

| Object | Size (30 days) | Growth Rate |
|--------|----------------|-------------|
| gold.outdoor_weather_hourly | ~3 MB | ~0.1 MB/day |
| gold.state_events_hourly | ~1 MB | ~0.05 MB/day |
| gold.indoor_air_quality_aligned | ~10 MB | ~0.3 MB/day |
| **Total Phase C addition** | ~14 MB | ~0.45 MB/day |

---

## V1.2 Handoff Requirements

Phase C enables V1.2 by providing:

| Requirement | Delivered By |
|-------------|--------------|
| Query all streams in single view | `gold.indoor_air_quality_aligned` |
| Hourly granularity for correlation | Aligned view buckets |
| NULL-honest representation | ADR-FE001-004 applied |
| State transitions as events | `gold.state_transitions` |
| Objectives for threshold crossings | `data_dictionary.objectives` |

---

## References

- [SCOPE.md](../../SCOPE.md) - Full V1.1 scope definition
- [DECISIONS.md](../../architecture/DECISIONS.md) - Architecture decisions
- [ADR-FE001-002](../../architecture/ADR-FE001-002-domain-centric-config.md) - Domain-centric configuration
- [ADR-FE001-003](../../architecture/ADR-FE001-003-forecast-alignment.md) - Forecast alignment
- [ADR-FE001-004](../../architecture/ADR-FE001-004-null-handling.md) - NULL handling by stream type
- [CONFIG-DEPLOYMENT-FLOW.md](../../architecture/CONFIG-DEPLOYMENT-FLOW.md) - Deployment flow
- [PHASE-A-OVERVIEW.md](../phase-a/specification/PHASE-A-OVERVIEW.md) - Phase A specifications
