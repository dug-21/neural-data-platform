# FE-001 Phase C: Cross-Stream + Alignment - Acceptance Criteria

> **Phase:** C (Cross-Stream + Alignment)
> **Version:** 1.0
> **Created:** 2026-02-04
> **Last Updated:** 2026-02-04

---

## Overview

This document defines the acceptance criteria for Phase C, which extends the Gold layer to three streams and introduces the cross-stream aligned view. This phase validates the JOIN complexity required for V1.2 pattern detection.

**Deliberately Excluded:** `outdoor-air-quality` stream (reserved for Phase D fast-follower test).

---

## Feature Acceptance Criteria

### AC-C-01: Outdoor-Weather Continuous Aggregate Operational (v11-003 extended)

**Given:** outdoor-weather stream with gold_etl enabled
**When:** Manifest is deployed
**Then:** `gold.outdoor_weather_hourly` continuous aggregate is operational

**Verification:**
```bash
# Verify continuous aggregate exists
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT view_name
FROM timescaledb_information.continuous_aggregates
WHERE view_schema = 'gold' AND view_name = 'outdoor_weather_hourly';
"
# Expected: Row with view_name = 'outdoor_weather_hourly'

# Verify expected columns
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT column_name
FROM information_schema.columns
WHERE table_schema = 'gold' AND table_name = 'outdoor_weather_hourly'
ORDER BY ordinal_position;
"
# Expected: bucket, ndp_id, temperature_c_mean, humidity_pct_mean, wind_speed_kmh_mean, etc.

# Verify data present
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT COUNT(*) FROM gold.outdoor_weather_hourly WHERE bucket >= NOW() - INTERVAL '7 days';
"
# Expected: count > 0
```

**Acceptance Checklist:**
- [ ] `gold.outdoor_weather_hourly` view exists
- [ ] View has temperature_c metrics (mean, min, max)
- [ ] View has humidity_pct metrics (mean, min, max)
- [ ] View has wind_speed_kmh metrics
- [ ] View has pressure_pa metrics
- [ ] Refresh policy attached
- [ ] Data populates from Silver

**Owner:** ndp-rust-dev

---

### AC-C-02: Home-Assistant-State Continuous Aggregate Operational (v11-003 extended)

**Given:** home-assistant-state stream with gold_etl enabled
**When:** Manifest is deployed
**Then:** `gold.state_events_hourly` continuous aggregate is operational

**Verification:**
```bash
# Verify continuous aggregate exists
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT view_name
FROM timescaledb_information.continuous_aggregates
WHERE view_schema = 'gold' AND view_name = 'state_events_hourly';
"
# Expected: Row with view_name = 'state_events_hourly'

# Verify state-specific metrics
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT column_name
FROM information_schema.columns
WHERE table_schema = 'gold' AND table_name = 'state_events_hourly'
AND column_name LIKE '%count%' OR column_name LIKE '%changes%';
"
# Expected: window_open_count, door_open_count, state_changes_count
```

**Acceptance Checklist:**
- [ ] `gold.state_events_hourly` view exists
- [ ] View has state_changes_count metric
- [ ] View has window_open_count metric (if applicable)
- [ ] View has door_open_count metric (if applicable)
- [ ] stream_type = "state_event" in config
- [ ] Refresh policy attached

**Owner:** ndp-timescale-dev

---

### AC-C-03: Aligned View Joins All Three Streams (v11-005)

**Given:** Domain configuration with 3 streams (air-quality, outdoor-weather, home-assistant-state)
**When:** Domain DDL is generated and deployed
**Then:** `gold.indoor_air_quality_aligned` view returns data from all 3 streams

**Verification:**
```bash
# Verify aligned view exists
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT viewname
FROM pg_views
WHERE schemaname = 'gold' AND viewname = 'indoor_air_quality_aligned';
"
# Expected: Row with viewname = 'indoor_air_quality_aligned'

# Verify columns from all streams
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT column_name
FROM information_schema.columns
WHERE table_schema = 'gold' AND table_name = 'indoor_air_quality_aligned'
ORDER BY column_name;
"
# Expected: indoor_pm25_mean, outdoor_temperature_c_mean, state_window_open_count (aliased)

# Query aligned view for data from all streams
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT
    bucket,
    indoor_pm25_mean IS NOT NULL as has_indoor,
    outdoor_temperature_c_mean IS NOT NULL as has_outdoor,
    state_changes_count IS NOT NULL as has_state
FROM gold.indoor_air_quality_aligned
WHERE bucket >= NOW() - INTERVAL '1 day'
ORDER BY bucket DESC
LIMIT 10;
"
# Expected: Mix of TRUE values showing data from each stream
```

**Acceptance Checklist:**
- [ ] `gold.indoor_air_quality_aligned` view exists
- [ ] View has columns from air-quality stream (indoor_ prefix)
- [ ] View has columns from outdoor-weather stream (outdoor_ prefix)
- [ ] View has columns from home-assistant-state stream (state_ prefix)
- [ ] View uses FULL OUTER JOIN strategy
- [ ] Bucket column is common join key

**Owner:** ndp-analytics-engineer

---

### AC-C-04: NULL Handling Correct by Stream Type (ADR-FE001-004)

**Given:** Aligned view with multiple stream types
**When:** Querying rows where some streams have no data for a bucket
**Then:** NULL handling follows stream_type rules

**Verification:**
```bash
# Test observation streams preserve NULL (no carry-forward)
docker exec timescaledb psql -U postgres -d ndp -c "
-- Find buckets where air-quality has no data
WITH gaps AS (
    SELECT bucket
    FROM gold.indoor_air_quality_aligned
    WHERE indoor_pm25_mean IS NULL
    LIMIT 5
)
SELECT a.bucket, a.indoor_pm25_mean, a.outdoor_temperature_c_mean
FROM gold.indoor_air_quality_aligned a
JOIN gaps g ON a.bucket = g.bucket;
"
# Expected: indoor_pm25_mean IS NULL (not filled in)

# Test state_event streams use carry-forward (LOCF)
docker exec timescaledb psql -U postgres -d ndp -c "
-- State columns should carry forward last known value
SELECT bucket, state_window_open_count,
       LAG(state_window_open_count) OVER (ORDER BY bucket) as prev_state
FROM gold.indoor_air_quality_aligned
WHERE bucket >= NOW() - INTERVAL '6 hours'
ORDER BY bucket;
"
# Expected: state_window_open_count carries forward if no new events
```

**Acceptance Checklist:**
- [ ] observation streams (air-quality, outdoor-weather) preserve NULL
- [ ] state_event streams use COALESCE with LAG (LOCF pattern)
- [ ] NULL handling configurable via `null_handling: by_stream_type`
- [ ] View definition includes COALESCE for state columns

**Owner:** ndp-analytics-engineer

---

### AC-C-05: State Transitions Extracted (v11-006)

**Given:** home-assistant-state stream with state change events
**When:** Querying state transitions view
**Then:** Actual state changes are identifiable with `is_actual_transition`

**Verification:**
```bash
# Verify state transitions view exists
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT viewname
FROM pg_views
WHERE schemaname = 'gold' AND viewname LIKE '%state%transition%';
"
# Expected: state_transitions or similar

# Query for actual transitions
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT
    event_time,
    entity_id,
    from_state,
    to_state,
    duration_in_previous_ms,
    is_actual_transition
FROM gold.state_transitions
WHERE is_actual_transition = true
ORDER BY event_time DESC
LIMIT 10;
"
# Expected: Rows with real state changes (on->off, off->on)
```

**Acceptance Checklist:**
- [ ] `gold.state_transitions` view exists (or similar)
- [ ] View has from_state column
- [ ] View has to_state column
- [ ] View has duration_in_previous_ms column
- [ ] View has is_actual_transition boolean
- [ ] Filter is_actual_transition=true removes noise

**Owner:** ndp-timescale-dev

---

### AC-C-06: Objectives Stored in Data Dictionary (v11-007)

**Given:** Domain configuration with objectives section
**When:** Domain is deployed
**Then:** Objectives are queryable from data dictionary

**Verification:**
```bash
# Check objectives table
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT objective_id, domain_id, target_stream, target_metric, condition, threshold
FROM data_dictionary.objectives
WHERE domain_id = 'indoor-air-quality';
"
# Expected: Rows for healthy_co2, healthy_pm25

# Verify specific objective
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT *
FROM data_dictionary.objectives
WHERE objective_id = 'healthy_co2';
"
# Expected: condition = '<', threshold = 800, target_metric = 'co2'
```

**Acceptance Checklist:**
- [ ] `data_dictionary.objectives` table exists
- [ ] healthy_co2 objective stored (co2 < 800 ppm)
- [ ] healthy_pm25 objective stored (pm25 < 12 ug/m3)
- [ ] Objectives have priority field
- [ ] Objectives link to domain_id

**Owner:** ndp-rust-dev

---

### AC-C-07: Domain Metadata in Data Dictionary

**Given:** Domain configuration deployed
**When:** Querying data dictionary
**Then:** Domain and domain-stream mappings are queryable

**Verification:**
```bash
# Check domains table
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT domain_id, description, granularity
FROM data_dictionary.domains
WHERE domain_id = 'indoor-air-quality';
"
# Expected: Row with granularity = '1 hour'

# Check domain-stream mappings
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT domain_id, stream_id, role, alias
FROM data_dictionary.domain_streams
WHERE domain_id = 'indoor-air-quality'
ORDER BY role;
"
# Expected: 3 rows - air-quality (primary), outdoor-weather (context), home-assistant-state (actuator)
```

**Acceptance Checklist:**
- [ ] `data_dictionary.domains` table exists
- [ ] indoor-air-quality domain entry present
- [ ] `data_dictionary.domain_streams` table exists
- [ ] 3 stream mappings with correct roles
- [ ] Aliases stored (indoor, outdoor, state)

**Owner:** ndp-analytics-engineer

---

### AC-C-08: outdoor-air-quality NOT in Gold Layer

**Given:** Phase C complete
**When:** Checking Gold layer objects
**Then:** outdoor-air-quality stream is NOT present (reserved for Phase D)

**Verification:**
```bash
# Verify outdoor-air-quality not in Gold
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT view_name
FROM timescaledb_information.continuous_aggregates
WHERE view_schema = 'gold' AND view_name LIKE '%outdoor_air%';
"
# Expected: 0 rows

# Verify not in domain streams
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT stream_id
FROM data_dictionary.domain_streams
WHERE domain_id = 'indoor-air-quality' AND stream_id = 'outdoor-air-quality';
"
# Expected: 0 rows
```

**Acceptance Checklist:**
- [ ] No gold.outdoor_air_quality_* objects exist
- [ ] outdoor-air-quality not in domain_streams mapping
- [ ] Reserved for Phase D fast-follower test

**Owner:** ndp-tester

---

## Performance Acceptance Criteria

### AC-C-PERF-01: Aligned View Query Performance

**Given:** Aligned view with 30 days of data
**When:** Querying the aligned view
**Then:** Query completes in < 100ms on Pi 5

**Verification:**
```bash
docker exec timescaledb psql -U postgres -d ndp -c "
EXPLAIN (ANALYZE, COSTS, TIMING)
SELECT bucket, indoor_pm25_mean, outdoor_temperature_c_mean, state_changes_count
FROM gold.indoor_air_quality_aligned
WHERE bucket >= NOW() - INTERVAL '30 days'
ORDER BY bucket;
"
# Expected: Execution Time: < 100ms
```

**Acceptance Checklist:**
- [ ] 30-day aligned view query < 100ms
- [ ] Query uses continuous aggregate sources
- [ ] No sequential scan on underlying tables

**Owner:** ndp-tester

---

### AC-C-PERF-02: Resource Usage Within Budget

**Given:** Phase C objects on Pi 5
**When:** Monitoring during operation
**Then:** Resource usage within budget

**Verification:**
```bash
# Storage check
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT
    schemaname || '.' || matviewname as view_name,
    pg_size_pretty(pg_total_relation_size(schemaname || '.' || matviewname)) as size
FROM pg_matviews
WHERE schemaname = 'gold';
"
# Expected: Total < 50 MB

# Memory during refresh
# Monitor on Pi during refresh window
```

**Acceptance Checklist:**
- [ ] outdoor_weather_hourly < 10 MB (30 days)
- [ ] state_events_hourly < 5 MB (30 days)
- [ ] Aligned view materialization < 15 MB
- [ ] Total Phase C addition < 50 MB
- [ ] Refresh CPU < 5% sustained

**Owner:** ndp-tester

---

### AC-C-PERF-03: State Transition Query Performance

**Given:** state_transitions view with data
**When:** Querying for recent transitions
**Then:** Query completes in < 50ms

**Verification:**
```bash
docker exec timescaledb psql -U postgres -d ndp -c "
EXPLAIN (ANALYZE, COSTS, TIMING)
SELECT *
FROM gold.state_transitions
WHERE event_time >= NOW() - INTERVAL '7 days'
AND is_actual_transition = true
ORDER BY event_time DESC;
"
# Expected: Execution Time: < 50ms
```

**Acceptance Checklist:**
- [ ] 7-day transition query < 50ms
- [ ] Index on event_time exists
- [ ] is_actual_transition filter efficient

**Owner:** ndp-tester

---

## Integration Acceptance Criteria

### AC-C-INT-01: Multi-Stream Deployment Works

**Given:** Phase B complete (air-quality in Gold)
**When:** Deploying Phase C manifest
**Then:** All 3 streams operational with aligned view

**Verification:**
```bash
# Deploy Phase C manifest
deploy/pi/deploy.sh apply .deploy/test/phase-c-multi-stream.manifest.json

# Verify all objects created
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT view_name
FROM timescaledb_information.continuous_aggregates
WHERE view_schema = 'gold'
ORDER BY view_name;
"
# Expected: air_quality_hourly, air_quality_daily, outdoor_weather_hourly, state_events_hourly
```

**Acceptance Checklist:**
- [ ] deploy.sh handles multiple gold-table declarations
- [ ] deploy.sh handles domain declaration
- [ ] Objects created in correct order (aggregates before aligned view)
- [ ] No manual intervention required

**Owner:** ndp-rust-dev

---

### AC-C-INT-02: Domain Config Validation Works

**Given:** Invalid domain configuration
**When:** Running validation
**Then:** Errors caught with helpful messages

**Verification:**
```bash
# Test invalid stream reference
cat > /tmp/bad_domain.yaml << 'EOF'
domain:
  id: test-domain
  streams:
    - stream_id: nonexistent-stream
      role: primary
EOF

ndp-validate --schema config/schemas/domain.schema.json --config /tmp/bad_domain.yaml
# Expected: Error mentions unknown stream reference
```

**Acceptance Checklist:**
- [ ] Invalid stream references caught (error 402)
- [ ] Invalid role assignments caught
- [ ] Missing required streams caught
- [ ] Circular domain dependencies caught (error 407)

**Owner:** ndp-rust-dev

---

## Exit Criteria Summary

Phase C is complete when ALL of the following are true:

### Stream Aggregates
- [ ] AC-C-01: gold.outdoor_weather_hourly operational
- [ ] AC-C-02: gold.state_events_hourly operational

### Aligned View
- [ ] AC-C-03: gold.indoor_air_quality_aligned returns data from 3 streams
- [ ] AC-C-04: NULL handling correct by stream_type

### State Transitions
- [ ] AC-C-05: State transitions extractable with is_actual_transition

### Data Dictionary
- [ ] AC-C-06: Objectives stored in data_dictionary.objectives
- [ ] AC-C-07: Domain metadata in data dictionary

### Validation
- [ ] AC-C-08: outdoor-air-quality NOT in Gold layer (reserved)

### Performance
- [ ] AC-C-PERF-01: Aligned view query < 100ms
- [ ] AC-C-PERF-02: Resource usage within budget
- [ ] AC-C-PERF-03: State transition query < 50ms

### Integration
- [ ] AC-C-INT-01: Multi-stream deployment works
- [ ] AC-C-INT-02: Domain config validation works

---

## Phase C to Phase D Gate

Before starting Phase D:

- [ ] **CRITICAL**: 3 streams in aligned view operational
- [ ] **CRITICAL**: NULL handling validated for all stream types
- [ ] **CRITICAL**: outdoor-air-quality reserved for fast-follower test
- [ ] All objectives queryable from data dictionary

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
