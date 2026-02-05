# FE-001 Phase E: Unified Event Abstraction - Overview

> **Created:** 2026-02-04
> **Updated:** 2026-02-05
> **Phase:** E (Unified Event Abstraction)
> **Target:** Week 6
> **Status:** Specification Complete (Events Hypertable Approach)

---

## Executive Summary

Phase E completes the V1.1 Gold Layer Foundation by implementing the **Unified Event Abstraction**. This phase creates a dedicated **events hypertable** that stores state transitions (from Phase C) and threshold crossings (new in Phase E) with **environmental context captured at event time** for V1.2 correlation analysis.

**Primary Deliverable**: `gold.events` - a TimescaleDB hypertable storing all events with context snapshots.

**Key Architecture Decision (2026-02-05)**: Events are stored in a hypertable (not a UNION ALL view) to:
1. Enable continuous aggregates on events
2. Capture environmental context at event time
3. Support efficient V1.2 correlation queries

**V1.2 Handoff**: This phase is the bridge to V1.2. The events hypertable IS the interface contract that V1.2 consumes.

---

## Phase E Features

| ID | Feature | Priority | Specification |
|----|---------|----------|---------------|
| v11-012 | Threshold Crossing Generator | Critical | [SPEC-E01](./SPEC-E01-threshold-crossings.md) |
| v11-013 | Unified Events View | Critical | [SPEC-E02](./SPEC-E02-unified-events-view.md) |
| v11-014 | Gold Layer Dashboard | High | [SPEC-E03](./SPEC-E03-gold-layer-dashboard.md) |
| v11-V02 | New Feature Type Test | Medium | See Phase D exit criteria |

---

## Dependency Graph

```
                PHASE A-D (Prerequisites)
                         |
        +----------------+----------------+
        |                                 |
        v                                 v
+-------------------+           +-------------------+
| Phase C           |           | Phase C           |
| v11-006           |           | v11-007           |
| State Transition  |           | Objectives        |
| Materializer      |           | Storage           |
+--------+----------+           +---------+---------+
         |                                |
         |                                v
         |                      +-------------------+
         |                      | v11-012           |
         |                      | Threshold         |
         |                      | Crossing          |
         |                      | Generator         |
         |                      +---------+---------+
         |                                |
         +----------------+---------------+
                          |
                          v
                +-------------------+
                | v11-013           |
                | Unified Events    |
                | View              |
                +---------+---------+
                          |
                          v
                +-------------------+
                | V1.2 Pattern      |
                | Detection Engine  |
                | (Consumer)        |
                +-------------------+
```

### Dependency Details

| Feature | Depends On | Blocking For |
|---------|------------|--------------|
| v11-012 (Threshold Crossings) | v11-007 (Objectives Storage) | v11-013 (Unified Events) |
| v11-013 (Unified Events) | v11-006 (State Transitions), v11-012 | v11-014 (Dashboard), V1.2 Pattern Detection |
| v11-014 (Gold Layer Dashboard) | All Gold CAs, v11-013 | V1.1 Completion |

---

## V1.2 Handoff Requirements

### What V1.2 Expects

V1.2 Pattern Detection Engine requires:

| Requirement | Source | Phase E Deliverable |
|-------------|--------|---------------------|
| Unified event stream | Query single view | `gold.events_unified` |
| Consistent event schema | Same fields for all event types | Event schema with JSONB details |
| Event type classification | Filter by event type | `event_type` column |
| Hourly event aggregates | Join with aligned view | `gold.events_hourly` aggregate |
| Threshold context | Know which objective was crossed | `objective_id` in details |
| Direction information | Rising vs falling crossings | `direction` in details |

### V1.2 Query Patterns

V1.2 will query events using these patterns:

```sql
-- Pattern 1: Get all events in time range
SELECT * FROM gold.events_unified
WHERE event_time BETWEEN :start AND :end
ORDER BY event_time;

-- Pattern 2: Get events by type
SELECT * FROM gold.events_unified
WHERE event_type = 'threshold_crossing'
  AND event_time >= NOW() - INTERVAL '24 hours';

-- Pattern 3: Join events with aligned view for correlation
SELECT
    a.bucket,
    e.event_type,
    e.details,
    a.indoor_pm25,
    a.window_state
FROM gold.indoor_air_quality_aligned a
LEFT JOIN gold.events_hourly eh ON a.bucket = eh.bucket;

-- Pattern 4: Filter by objective
SELECT * FROM gold.events_unified
WHERE event_type = 'threshold_crossing'
  AND details->>'objective_id' = 'healthy_co2';
```

### V1.2 Contract Definition

The following schema is the **interface contract** between V1.1 and V1.2:

```sql
-- Event Schema Contract (DO NOT CHANGE WITHOUT V1.2 COORDINATION)
CREATE TYPE event_type_enum AS ENUM (
    'state_transition',
    'threshold_crossing'
    -- Future: 'anomaly', 'trend_change'
);

-- Unified Events View Schema
gold.events_unified (
    event_id        UUID PRIMARY KEY,
    event_time      TIMESTAMPTZ NOT NULL,
    stream_id       TEXT NOT NULL,
    entity_id       TEXT NOT NULL,  -- ndp_id
    event_type      event_type_enum NOT NULL,
    details         JSONB NOT NULL
);

-- Details Schema by Event Type
-- state_transition:
{
    "from_state": "off",
    "to_state": "on",
    "duration_in_previous_ms": 3600000
}

-- threshold_crossing:
{
    "metric": "co2",
    "threshold": 800,
    "direction": "rising",  -- or "falling"
    "value": 812,
    "previous_value": 795,
    "objective_id": "healthy_co2",
    "condition": "<"
}
```

---

## Deferred Decision: Threshold Crossing Deduplication

From [DECISIONS.md](../../architecture/DECISIONS.md):

> **Deferred: Threshold Crossing Deduplication**
>
> **Question**: When a metric oscillates around a threshold, it generates many crossing events. Should we dedupe? Apply hysteresis?
>
> **Decision**: **Deferred** - Wait until we observe the behavior in practice.
>
> **Revisit When**: After V1.1 Phase E is deployed and generating real threshold crossing events.

### Observable Behavior Requirements

To support future deduplication decision, Phase E SHALL:

1. **Log crossing frequency** - Record how often crossings occur per objective per day
2. **Include previous_value** - Enables post-hoc analysis of oscillation patterns
3. **Track objective_id** - Enables per-objective analysis
4. **Monitor event volume** - Alert if events exceed threshold (TBD)

### Monitoring Requirements

| Metric | Purpose | Alert Threshold |
|--------|---------|-----------------|
| `gold_threshold_crossings_per_hour` | Track crossing frequency | > 100/hour per objective |
| `gold_threshold_crossing_oscillation_rate` | Same threshold crossed within 1 hour | > 10/hour per objective |
| `gold_events_unified_total_per_day` | Total event volume | > 10,000/day |

### Future Hysteresis Options

If deduplication is needed, these options will be evaluated:

| Option | Description | Pros | Cons |
|--------|-------------|------|------|
| Time-based debounce | Ignore crossings within N minutes of last | Simple | May miss real events |
| Hysteresis band | Require crossing by X% before generating event | Natural | Requires tuning per metric |
| Majority voting | Require N consecutive readings beyond threshold | Robust | Adds latency |
| Configurable per objective | Add `hysteresis` config to objective | Flexible | Complex |

**These options are NOT implemented in V1.1. This section documents the design space for V1.2.**

---

## Implementation Strategy

### Phase E Implementation Order

1. **v11-012: Threshold Crossing Generator** (Day 1-2)
   - Implement crossing detection SQL
   - Generate events from objectives
   - Index for V1.2 query patterns

2. **v11-013: Unified Events View** (Day 3-4)
   - Combine state transitions + threshold crossings
   - Create hourly aggregate for aligned view
   - Implement event schema

3. **Integration Testing** (Day 5)
   - V1.2 query pattern validation
   - Performance testing on Pi
   - Monitoring setup

### SQL Generation Approach

Following ADR-FE001-001, the `ndp-gold-ddl` tool generates all SQL:

```bash
# Generate threshold crossing view
ndp-gold-ddl generate --domain indoor-air-quality --component threshold-crossings

# Generate unified events view
ndp-gold-ddl generate --domain indoor-air-quality --component unified-events

# Generate all Phase E components
ndp-gold-ddl generate --domain indoor-air-quality --phase events
```

---

## Exit Criteria Checklist

### Feature Completion

- [ ] **v11-012**: Threshold crossing events generated from objectives config
- [ ] **v11-012**: Rising and falling crossings detected correctly
- [ ] **v11-012**: All condition types supported (<, <=, >, >=, between)
- [ ] **v11-013**: `gold.events_unified` view operational
- [ ] **v11-013**: View combines state transitions + threshold crossings
- [ ] **v11-013**: Consistent event schema across all event types
- [ ] **v11-013**: `gold.events_hourly` aggregate available
- [ ] **v11-014**: Gold Layer Dashboard deployed to Grafana
- [ ] **v11-014**: Dashboard displays all Gold continuous aggregates
- [ ] **v11-014**: Dashboard displays aligned view and unified events
- [ ] **v11-014**: Objective thresholds visible as annotations/lines

### Performance

- [ ] Query `gold.events_unified` for 30 days < 100ms
- [ ] Threshold crossing detection adds < 5% overhead to refresh
- [ ] Pi resource usage within budget (< 50 MB for events)

### V1.2 Handoff

- [ ] V1.2 query patterns validated
- [ ] Event schema documented and frozen
- [ ] Monitoring metrics implemented
- [ ] Handoff documentation complete

### Observability (for Deferred Deduplication Decision)

- [ ] Crossing frequency logged
- [ ] Oscillation patterns detectable via queries
- [ ] Alert thresholds configured (but not enabled)

---

## Test Strategy

### Unit Tests

| Test | Description | Location |
|------|-------------|----------|
| `threshold_crossing_detection_test.rs` | Test crossing detection for all conditions | `tools/ndp-gold-ddl/tests/` |
| `unified_events_view_test.rs` | Test UNION of event types | `tools/ndp-gold-ddl/tests/` |
| `event_schema_test.rs` | Validate JSONB details structure | `tools/ndp-gold-ddl/tests/` |

### Integration Tests

| Test | Description | Validates |
|------|-------------|-----------|
| `phase_e_deploy.rs` | Deploy all Phase E components | Full flow |
| `v12_query_patterns.rs` | Execute V1.2 query patterns | V1.2 compatibility |
| `pi_performance.rs` | Performance on Pi | Resource constraints |

### Test Manifests

```
.deploy/test/
├── phase-e-events.manifest.json      # All Phase E components
└── phase-e-crossings-only.manifest.json  # Threshold crossings standalone
```

---

## File Inventory

### New Files (Phase E creates)

```
tools/ndp-gold-ddl/src/generators/
└── events.rs                         # v11-012, v11-013 generators

config/domains/{domain}/domain.yaml   # Extended with threshold_crossings config
```

### Modified Files (Phase E extends)

```
tools/ndp-gold-ddl/src/generators/mod.rs  # Import events module
core/src/gold/config.rs                   # ThresholdCrossingsConfig struct
```

### Generated SQL

```sql
-- v11-012: Threshold Crossing View
gold.{domain}_threshold_crossings

-- v11-013: Unified Events View
gold.events_unified

-- v11-013: Hourly Events Aggregate
gold.events_hourly
```

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Threshold crossing chattering | Medium | Medium | Defer hysteresis; monitor; document for V1.2 |
| Event schema doesn't support V1.2 needs | Low | High | Review with V1.2 requirements before finalizing |
| Unified view performance poor | Low | Medium | Index on event_time, event_type; use partitioning |
| Objective changes break crossings | Low | Medium | Re-generate crossing view on objective change |

---

## References

### Phase E Specifications

- [SPEC-E01: Threshold Crossings](./SPEC-E01-threshold-crossings.md)
- [SPEC-E02: Unified Events View](./SPEC-E02-unified-events-view.md)

### Prerequisites (Phase C)

- v11-006: State Transition Materializer
- v11-007: Objectives Storage

### FE-001 Documents

- [SCOPE.md](../../SCOPE.md) - Full V1.1 scope definition
- [DECISIONS.md](../../architecture/DECISIONS.md) - Architecture decisions including deferred decision
- [CONFIG-DEPLOYMENT-FLOW.md](../../architecture/CONFIG-DEPLOYMENT-FLOW.md) - Deployment flow
- [SPARC-COORDINATION.md](../../SPARC-COORDINATION.md) - Overall SPARC coordination

---

*Phase E Overview created: 2026-02-04 by specification-agent*
