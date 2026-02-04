# FE-001 Phase E: Unified Event Abstraction - Acceptance Criteria

> **Phase:** E (Unified Event Abstraction)
> **Version:** 1.0
> **Created:** 2026-02-04
> **Last Updated:** 2026-02-04

---

## Overview

This document defines the acceptance criteria for Phase E, which completes the V1.1 Gold Layer Foundation by implementing the Unified Event Abstraction. This phase combines state transition events with threshold crossing events into a single queryable view that V1.2 Pattern Detection Engine will consume.

**V1.2 Handoff**: Phase E deliverables ARE the interface contract for V1.2.

---

## Feature Acceptance Criteria

### AC-E-01: Threshold Crossing Generator Works (v11-012)

**Given:** Objectives defined in domain config with thresholds
**When:** Gold aggregates contain data crossing thresholds
**Then:** Threshold crossing events are generated

**Verification:**
```bash
# Verify threshold crossing view exists
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT viewname
FROM pg_views
WHERE schemaname = 'gold' AND viewname LIKE '%threshold%crossing%';
"
# Expected: indoor_air_quality_threshold_crossings or similar

# Query for threshold crossing events
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT
    event_time,
    stream_id,
    entity_id,
    details->>'metric' as metric,
    details->>'threshold' as threshold,
    details->>'direction' as direction,
    details->>'value' as actual_value
FROM gold.events_unified
WHERE event_type = 'threshold_crossing'
AND event_time >= NOW() - INTERVAL '7 days'
ORDER BY event_time DESC
LIMIT 10;
"
# Expected: Rows showing threshold crossings with metric, threshold, direction

# Verify both rising and falling crossings
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT
    details->>'direction' as direction,
    COUNT(*) as count
FROM gold.events_unified
WHERE event_type = 'threshold_crossing'
GROUP BY details->>'direction';
"
# Expected: Both 'rising' and 'falling' present
```

**Acceptance Checklist:**
- [ ] Threshold crossing events generated for healthy_co2 objective
- [ ] Threshold crossing events generated for healthy_pm25 objective
- [ ] Rising crossings detected (value crosses above threshold)
- [ ] Falling crossings detected (value crosses below threshold)
- [ ] event_time is accurate (time of crossing)
- [ ] details contains metric, threshold, direction, value, previous_value
- [ ] details contains objective_id and condition

**Owner:** ndp-rust-dev

---

### AC-E-02: All Condition Types Supported

**Given:** Objectives with different condition types
**When:** Generating threshold crossings
**Then:** All condition types correctly evaluated

**Verification:**
```bash
# Test condition types via unit tests
cargo test -p ndp-gold-ddl threshold_crossing_conditions -- --nocapture

# Test should verify:
# - < (less than): crossing detected when value >= threshold then < threshold
# - <= (less than or equal): crossing detected when value > threshold then <= threshold
# - > (greater than): crossing detected when value <= threshold then > threshold
# - >= (greater than or equal): crossing detected when value < threshold then >= threshold
# - between (range): crossing in/out of range
```

**Acceptance Checklist:**
- [ ] Condition < works correctly
- [ ] Condition <= works correctly
- [ ] Condition > works correctly
- [ ] Condition >= works correctly
- [ ] Condition == works correctly
- [ ] Condition between works correctly (in-range and out-of-range crossings)

**Owner:** ndp-rust-dev

---

### AC-E-03: Unified Events View Combines Event Types (v11-013)

**Given:** State transitions (from Phase C) and threshold crossings
**When:** Querying unified events view
**Then:** Both event types available in single view with consistent schema

**Verification:**
```bash
# Verify unified events view exists
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT viewname
FROM pg_views
WHERE schemaname = 'gold' AND viewname = 'events_unified';
"
# Expected: events_unified

# Verify both event types present
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT event_type, COUNT(*) as count
FROM gold.events_unified
WHERE event_time >= NOW() - INTERVAL '7 days'
GROUP BY event_type;
"
# Expected: Both 'state_transition' and 'threshold_crossing' with counts

# Verify consistent schema
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT column_name, data_type
FROM information_schema.columns
WHERE table_schema = 'gold' AND table_name = 'events_unified'
ORDER BY ordinal_position;
"
# Expected: event_id, event_time, stream_id, entity_id, event_type, details
```

**Acceptance Checklist:**
- [ ] `gold.events_unified` view exists
- [ ] View has event_id (UUID) column
- [ ] View has event_time (TIMESTAMPTZ) column
- [ ] View has stream_id (TEXT) column
- [ ] View has entity_id (TEXT) column
- [ ] View has event_type (ENUM or TEXT) column
- [ ] View has details (JSONB) column
- [ ] State transitions included
- [ ] Threshold crossings included

**Owner:** ndp-timescale-dev

---

### AC-E-04: Event Schema Contract Met

**Given:** V1.2 Event Schema Contract (defined in PHASE-E-OVERVIEW.md)
**When:** Querying event details
**Then:** Details structure matches contract

**Verification:**
```bash
# Verify state_transition details structure
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT details
FROM gold.events_unified
WHERE event_type = 'state_transition'
LIMIT 1;
"
# Expected JSON structure:
# {
#   "from_state": "off",
#   "to_state": "on",
#   "duration_in_previous_ms": 3600000
# }

# Verify threshold_crossing details structure
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT details
FROM gold.events_unified
WHERE event_type = 'threshold_crossing'
LIMIT 1;
"
# Expected JSON structure:
# {
#   "metric": "co2",
#   "threshold": 800,
#   "direction": "rising",
#   "value": 812,
#   "previous_value": 795,
#   "objective_id": "healthy_co2",
#   "condition": "<"
# }
```

**Acceptance Checklist:**
- [ ] state_transition has from_state field
- [ ] state_transition has to_state field
- [ ] state_transition has duration_in_previous_ms field
- [ ] threshold_crossing has metric field
- [ ] threshold_crossing has threshold field
- [ ] threshold_crossing has direction field (rising/falling)
- [ ] threshold_crossing has value field
- [ ] threshold_crossing has previous_value field
- [ ] threshold_crossing has objective_id field
- [ ] threshold_crossing has condition field

**Owner:** ndp-rust-dev

---

### AC-E-05: Hourly Event Aggregate Available

**Given:** Unified events view
**When:** Querying hourly aggregates
**Then:** Event counts available by hour for aligned view join

**Verification:**
```bash
# Verify hourly events aggregate
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT
    bucket,
    state_transition_count,
    threshold_crossing_count,
    total_events
FROM gold.events_hourly
WHERE bucket >= NOW() - INTERVAL '7 days'
ORDER BY bucket DESC
LIMIT 10;
"
# Expected: Hourly buckets with event counts

# Verify joinable with aligned view
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT
    a.bucket,
    a.indoor_pm25_mean,
    e.total_events
FROM gold.indoor_air_quality_aligned a
LEFT JOIN gold.events_hourly e ON a.bucket = e.bucket
WHERE a.bucket >= NOW() - INTERVAL '24 hours'
ORDER BY a.bucket DESC
LIMIT 10;
"
# Expected: Joined results with event counts
```

**Acceptance Checklist:**
- [ ] `gold.events_hourly` aggregate exists
- [ ] Bucket column matches aligned view granularity (1 hour)
- [ ] state_transition_count column present
- [ ] threshold_crossing_count column present
- [ ] total_events column present
- [ ] Joinable with aligned view on bucket

**Owner:** ndp-timescale-dev

---

### AC-E-06: V1.2 Query Patterns Work

**Given:** Unified events view with data
**When:** Running V1.2 query patterns
**Then:** All patterns return expected results

**Verification:**
```bash
# Pattern 1: Get all events in time range
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT * FROM gold.events_unified
WHERE event_time BETWEEN NOW() - INTERVAL '24 hours' AND NOW()
ORDER BY event_time;
"
# Expected: Results ordered by time

# Pattern 2: Get events by type
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT * FROM gold.events_unified
WHERE event_type = 'threshold_crossing'
AND event_time >= NOW() - INTERVAL '24 hours';
"
# Expected: Only threshold_crossing events

# Pattern 3: Filter by objective
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT * FROM gold.events_unified
WHERE event_type = 'threshold_crossing'
AND details->>'objective_id' = 'healthy_co2';
"
# Expected: Only healthy_co2 threshold crossings

# Pattern 4: Join with aligned view
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT
    a.bucket,
    e.event_type,
    a.indoor_pm25_mean,
    a.indoor_co2_mean
FROM gold.indoor_air_quality_aligned a
LEFT JOIN gold.events_hourly e ON a.bucket = e.bucket
WHERE a.bucket >= NOW() - INTERVAL '24 hours'
ORDER BY a.bucket DESC;
"
# Expected: Joined results
```

**Acceptance Checklist:**
- [ ] Time range query works with ORDER BY
- [ ] Event type filter works
- [ ] Objective ID filter (JSONB) works
- [ ] Join with aligned view works
- [ ] Index on event_time exists
- [ ] Index on event_type exists (if using column, not JSONB)

**Owner:** ndp-tester

---

## Observability Acceptance Criteria (for Deferred Deduplication Decision)

### AC-E-07: Crossing Frequency Observable

**Given:** Threshold crossings generated
**When:** Analyzing crossing patterns
**Then:** Frequency can be determined for hysteresis decision

**Verification:**
```bash
# Query crossing frequency per objective per day
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT
    DATE_TRUNC('day', event_time) as day,
    details->>'objective_id' as objective,
    COUNT(*) as crossing_count
FROM gold.events_unified
WHERE event_type = 'threshold_crossing'
AND event_time >= NOW() - INTERVAL '7 days'
GROUP BY DATE_TRUNC('day', event_time), details->>'objective_id'
ORDER BY day DESC, objective;
"
# Expected: Daily counts by objective

# Check for oscillation patterns (multiple crossings in short window)
docker exec timescaledb psql -U postgres -d ndp -c "
WITH crossing_gaps AS (
    SELECT
        event_time,
        details->>'objective_id' as objective,
        EXTRACT(EPOCH FROM (event_time - LAG(event_time) OVER (
            PARTITION BY details->>'objective_id' ORDER BY event_time
        ))) / 60 as minutes_since_last
    FROM gold.events_unified
    WHERE event_type = 'threshold_crossing'
)
SELECT objective, COUNT(*) as rapid_crossings
FROM crossing_gaps
WHERE minutes_since_last < 60
GROUP BY objective;
"
# Expected: Count of crossings within 60 minutes of previous crossing
```

**Acceptance Checklist:**
- [ ] Daily crossing count queryable
- [ ] Per-objective crossing count queryable
- [ ] Oscillation patterns detectable (rapid successive crossings)
- [ ] previous_value enables post-hoc analysis
- [ ] Documentation notes this is for future hysteresis decision

**Owner:** ndp-analytics-engineer

---

### AC-E-08: Event Volume Monitoring

**Given:** Events generated continuously
**When:** Monitoring event volume
**Then:** Alerts can be configured for excessive events

**Verification:**
```bash
# Check hourly event volume
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT
    DATE_TRUNC('hour', event_time) as hour,
    COUNT(*) as event_count
FROM gold.events_unified
WHERE event_time >= NOW() - INTERVAL '24 hours'
GROUP BY DATE_TRUNC('hour', event_time)
ORDER BY hour DESC;
"
# Expected: Hourly counts for trend analysis

# Check if any hour exceeds threshold (e.g., 100 crossings/hour)
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT
    DATE_TRUNC('hour', event_time) as hour,
    details->>'objective_id' as objective,
    COUNT(*) as count
FROM gold.events_unified
WHERE event_type = 'threshold_crossing'
AND event_time >= NOW() - INTERVAL '24 hours'
GROUP BY DATE_TRUNC('hour', event_time), details->>'objective_id'
HAVING COUNT(*) > 100;
"
# Expected: Empty (or flagged hours if chattering)
```

**Acceptance Checklist:**
- [ ] Event volume queryable by hour
- [ ] Per-objective volume queryable
- [ ] Threshold for "excessive" documented (100/hour/objective)
- [ ] Query for alert trigger exists (even if not automated)

**Owner:** ndp-analytics-engineer

---

## Performance Acceptance Criteria

### AC-E-PERF-01: Unified Events Query Performance

**Given:** Unified events view with 30 days of data
**When:** Querying events
**Then:** Query completes in < 100ms

**Verification:**
```bash
docker exec timescaledb psql -U postgres -d ndp -c "
EXPLAIN (ANALYZE, COSTS, TIMING)
SELECT * FROM gold.events_unified
WHERE event_time >= NOW() - INTERVAL '30 days'
ORDER BY event_time;
"
# Expected: Execution Time: < 100ms
```

**Acceptance Checklist:**
- [ ] 30-day event query < 100ms
- [ ] Index on event_time used
- [ ] No sequential scan

**Owner:** ndp-tester

---

### AC-E-PERF-02: Threshold Crossing Detection Overhead

**Given:** Continuous aggregate refresh running
**When:** Threshold crossing detection executes
**Then:** Overhead < 5% of refresh time

**Verification:**
```bash
# Compare refresh times with/without crossing detection
# (Requires testing with crossing detection disabled for baseline)

# Monitor refresh job stats
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT
    j.config->>'mat_hypertable_id' as hypertable,
    js.total_duration,
    js.total_success
FROM timescaledb_information.job_stats js
JOIN timescaledb_information.jobs j ON js.job_id = j.job_id
WHERE j.proc_name = 'policy_refresh_continuous_aggregate';
"
# Expected: Duration acceptable
```

**Acceptance Checklist:**
- [ ] Crossing detection adds < 5% overhead
- [ ] No significant CPU spike during detection
- [ ] Memory usage within budget

**Owner:** ndp-tester

---

### AC-E-PERF-03: Resource Usage Within Budget

**Given:** Phase E objects on Pi 5
**When:** Monitoring during operation
**Then:** Resource usage within budget

**Verification:**
```bash
# Check events storage
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT
    pg_size_pretty(pg_total_relation_size('gold.events_unified')) as unified_size,
    pg_size_pretty(pg_total_relation_size('gold.events_hourly')) as hourly_size;
"
# Expected: < 50 MB total for events
```

**Acceptance Checklist:**
- [ ] events_unified < 30 MB (30 days)
- [ ] events_hourly < 10 MB (30 days)
- [ ] Total Phase E addition < 50 MB
- [ ] CPU overhead acceptable

**Owner:** ndp-tester

---

## Integration Acceptance Criteria

### AC-E-INT-01: Phase E Deployment Works

**Given:** Phase D complete
**When:** Deploying Phase E manifest
**Then:** All event objects created

**Verification:**
```bash
# Deploy Phase E manifest
deploy/pi/deploy.sh apply .deploy/test/phase-e-events.manifest.json

# Verify all objects created
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT viewname
FROM pg_views
WHERE schemaname = 'gold'
AND (viewname LIKE '%threshold%' OR viewname LIKE '%events%')
ORDER BY viewname;
"
# Expected: threshold_crossings, events_unified, events_hourly
```

**Acceptance Checklist:**
- [ ] Threshold crossing view created
- [ ] Unified events view created
- [ ] Hourly events aggregate created
- [ ] No manual intervention required

**Owner:** ndp-rust-dev

---

### AC-E-INT-02: Event Schema Backward Compatible

**Given:** Events already exist from Phase C (state transitions)
**When:** Deploying Phase E
**Then:** Existing state transitions preserved in unified view

**Verification:**
```bash
# Count state transitions before and after Phase E
# Should be preserved in unified view
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT
    event_type,
    MIN(event_time) as earliest,
    COUNT(*) as count
FROM gold.events_unified
GROUP BY event_type;
"
# Expected: state_transitions exist from Phase C timeframe
```

**Acceptance Checklist:**
- [ ] Phase C state transitions preserved
- [ ] No data loss during Phase E deployment
- [ ] Event schema consistent for all types

**Owner:** ndp-timescale-dev

---

## V1.2 Handoff Acceptance Criteria

### AC-E-V12-01: V1.2 Interface Contract Met

**Given:** Phase E complete
**When:** V1.2 Pattern Detection Engine connects
**Then:** All V1.2 requirements satisfied

**Verification:**
```bash
# Verify V1.2 requirements checklist
# 1. Unified event stream - gold.events_unified exists
# 2. Consistent schema - all events have same columns
# 3. Event type filter - event_type column works
# 4. Hourly aggregates - gold.events_hourly exists
# 5. Threshold context - objective_id in details
# 6. Direction info - direction in details

# Run V1.2 compatibility test
cargo test -p ndp-gold-ddl v12_compatibility -- --nocapture
```

**Acceptance Checklist:**
- [ ] gold.events_unified queryable
- [ ] Event schema matches V1.2 contract
- [ ] Event types distinguishable
- [ ] Hourly aggregates available
- [ ] Threshold context in details
- [ ] Direction information in details
- [ ] V1.2 team confirms compatibility

**Owner:** ndp-tester

---

### AC-E-V12-02: Documentation Complete for V1.2

**Given:** Phase E implementation complete
**When:** V1.2 development starts
**Then:** All necessary documentation available

**Verification:**
```bash
# Check documentation exists
ls -la product/features/fe-001/completion/
# Expected: FE-001-DONE-DEFINITION.md

ls -la docs/architecture/
# Expected: Gold layer architecture documented

# Check event schema documented
grep -r "events_unified" docs/
# Expected: Schema documented in architecture docs
```

**Acceptance Checklist:**
- [ ] Event schema documented
- [ ] V1.2 query patterns documented
- [ ] Threshold crossing behavior documented
- [ ] Deferred decision (hysteresis) documented
- [ ] Monitoring recommendations documented

**Owner:** ndp-tester

---

## Exit Criteria Summary

Phase E is complete when ALL of the following are true:

### Threshold Crossings
- [ ] AC-E-01: Threshold crossing generator works
- [ ] AC-E-02: All condition types supported

### Unified Events
- [ ] AC-E-03: Unified events view combines both types
- [ ] AC-E-04: Event schema contract met
- [ ] AC-E-05: Hourly event aggregate available
- [ ] AC-E-06: V1.2 query patterns work

### Observability
- [ ] AC-E-07: Crossing frequency observable
- [ ] AC-E-08: Event volume monitoring possible

### Performance
- [ ] AC-E-PERF-01: Unified events query < 100ms
- [ ] AC-E-PERF-02: Crossing detection overhead < 5%
- [ ] AC-E-PERF-03: Resource usage within budget

### Integration
- [ ] AC-E-INT-01: Phase E deployment works
- [ ] AC-E-INT-02: Event schema backward compatible

### V1.2 Handoff
- [ ] AC-E-V12-01: V1.2 interface contract met
- [ ] AC-E-V12-02: Documentation complete for V1.2

---

## V1.1 to V1.2 Handoff Gate

FE-001 is complete and V1.2 can begin when:

- [ ] All Phase E acceptance criteria pass
- [ ] FE-001-DONE-DEFINITION.md approved
- [ ] V1.2 team confirms Gold layer meets requirements
- [ ] All reflexion feedback recorded
- [ ] Patterns stored in AgentDB

---

## Sign-Off

| Role | Name | Date | Signature |
|------|------|------|-----------|
| ndp-rust-dev | | | |
| ndp-timescale-dev | | | |
| ndp-analytics-engineer | | | |
| ndp-tester | | | |

---

*Acceptance Criteria created: 2026-02-04 by ndp-tester*
