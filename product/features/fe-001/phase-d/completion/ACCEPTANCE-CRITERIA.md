# FE-001 Phase D: Validation + Dashboard - Acceptance Criteria

> **Phase:** D (Validation + Dashboard)
> **Version:** 1.0
> **Created:** 2026-02-04
> **Last Updated:** 2026-02-04

---

## Overview

This document defines the acceptance criteria for Phase D, the **critical validation phase** that proves the V1.1 Gold Layer architecture works as designed. The centerpiece is the **Fast-Follower Test**: adding the `outdoor-air-quality` stream to the Gold layer using ONLY configuration changes.

**CRITICAL**: If the fast-follower test fails (requires code changes), Phase D is considered FAILED and architecture must be revised before proceeding to Phase E.

---

## Fast-Follower Test Acceptance Criteria

### AC-D-01: Fast-Follower Test Passes (v11-V01) - CRITICAL

**Given:** Phase C complete (3 streams in Gold layer)
**When:** Adding outdoor-air-quality stream via config-only changes
**Then:** New stream operational in Gold layer within 1 hour, with ZERO Rust code changes

**Verification:**
```bash
# STEP 1: Record start time
START_TIME=$(date +%s)
echo "Fast-follower test started at: $(date)"

# STEP 2: Verify outdoor-air-quality NOT in Gold (pre-condition)
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT view_name FROM timescaledb_information.continuous_aggregates
WHERE view_schema = 'gold' AND view_name LIKE '%outdoor_air%';
"
# Expected: 0 rows

# STEP 3: Create/edit outdoor-air-quality gold_etl config
# (This is the timed portion - see timing requirements below)

# STEP 4: Update domain config to add outdoor-air-quality as 4th stream

# STEP 5: Create manifest with gold-table and domain declarations

# STEP 6: Deploy
deploy/pi/deploy.sh apply .deploy/test/phase-d-fast-follower.manifest.json

# STEP 7: Verify outdoor-air-quality in Gold
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT view_name FROM timescaledb_information.continuous_aggregates
WHERE view_schema = 'gold' AND view_name LIKE '%outdoor_air%';
"
# Expected: outdoor_air_quality_hourly

# STEP 8: Verify in aligned view
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT column_name FROM information_schema.columns
WHERE table_schema = 'gold' AND table_name = 'indoor_air_quality_aligned'
AND column_name LIKE '%outdoor_air%'
LIMIT 5;
"
# Expected: outdoor_air_pm25_mean, etc.

# STEP 9: Record end time
END_TIME=$(date +%s)
ELAPSED=$((END_TIME - START_TIME))
echo "Fast-follower test completed in: $ELAPSED seconds"
# Expected: ELAPSED < 3600 (1 hour)
```

**Timing Checkpoints:**
| Checkpoint | Target Time | Cumulative |
|------------|-------------|------------|
| Read documentation | 10 min | 10 min |
| Create gold_etl config | 15 min | 25 min |
| Update domain config | 10 min | 35 min |
| Create manifest | 5 min | 40 min |
| Run deploy.sh apply | 5 min | 45 min |
| Verify in database | 5 min | 50 min |
| Update dashboard (optional) | 10 min | 60 min |

**Acceptance Checklist:**
- [ ] **CRITICAL**: Total time < 1 hour
- [ ] **CRITICAL**: Zero changes to `tools/ndp-gold-ddl/` source code
- [ ] **CRITICAL**: Zero changes to `deploy/pi/deploy.sh`
- [ ] **CRITICAL**: Zero changes to `core/` modules
- [ ] **CRITICAL**: Zero changes to any `.rs` files
- [ ] **CRITICAL**: All changes are JSON/YAML config files only
- [ ] gold.outdoor_air_quality_hourly created
- [ ] outdoor_air columns appear in aligned view
- [ ] Refresh policy attached automatically

**Owner:** ndp-tester

---

### AC-D-02: Git Diff Shows Config-Only Changes

**Given:** Fast-follower test completed
**When:** Running git diff
**Then:** Only config files modified, no code files

**Verification:**
```bash
# Check what files changed during fast-follower test
git diff --name-only HEAD~1

# Expected output (only these types of files):
# config/base/streams/outdoor-air-quality/config.yaml
# config/domains/indoor-air-quality/domain.yaml
# .deploy/releases/vX.Y.Z.manifest.json

# Verify NO code changes
git diff --name-only HEAD~1 | grep -E '\.(rs|sh)$'
# Expected: 0 matches (no Rust or shell script changes)
```

**Acceptance Checklist:**
- [ ] git diff shows only config files
- [ ] No .rs files changed
- [ ] No .sh files changed
- [ ] No Cargo.toml changes

**Owner:** ndp-tester

---

## Feature Computation Acceptance Criteria

### AC-D-03: Basic Feature Computation Works (v11-008)

**Given:** Gold continuous aggregates with data
**When:** Querying features
**Then:** Basic aggregate features (mean, std, min, max) available

**Verification:**
```bash
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT
    bucket,
    indoor_pm25_mean,
    indoor_pm25_std,
    indoor_pm25_min,
    indoor_pm25_max,
    indoor_pm25_p95
FROM gold.indoor_air_quality_aligned
WHERE bucket >= NOW() - INTERVAL '24 hours'
ORDER BY bucket DESC
LIMIT 5;
"
# Expected: All columns have values (not all NULL)
```

**Acceptance Checklist:**
- [ ] mean features computed correctly
- [ ] std features computed correctly
- [ ] min/max features computed correctly
- [ ] p95/p99 percentile features computed

**Owner:** ndp-rust-dev

---

### AC-D-04: Lag Feature Computation Works (v11-009)

**Given:** Gold aggregates with lag features enabled
**When:** Querying lag features
**Then:** t-1h, t-6h, t-24h lag values available

**Verification:**
```bash
# Check lag features in aggregate
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT
    bucket,
    pm25_mean as pm25_current,
    pm25_lag_1h,
    pm25_lag_6h,
    pm25_lag_24h
FROM gold.air_quality_hourly
WHERE bucket >= NOW() - INTERVAL '48 hours'
ORDER BY bucket DESC
LIMIT 10;
"
# Expected: lag columns populated (NULL for first N rows as expected)

# Verify lag values are actually lagged
docker exec timescaledb psql -U postgres -d ndp -c "
WITH lagged AS (
    SELECT
        bucket,
        pm25_mean,
        pm25_lag_1h,
        LAG(pm25_mean, 1) OVER (ORDER BY bucket) as computed_lag_1h
    FROM gold.air_quality_hourly
    WHERE bucket >= NOW() - INTERVAL '24 hours'
)
SELECT bucket, pm25_mean, pm25_lag_1h, computed_lag_1h
FROM lagged
WHERE pm25_lag_1h IS NOT NULL
LIMIT 5;
"
# Expected: pm25_lag_1h equals computed_lag_1h
```

**Acceptance Checklist:**
- [ ] pm25_lag_1h computed correctly
- [ ] pm25_lag_6h computed correctly
- [ ] pm25_lag_24h computed correctly
- [ ] co2 lag features computed (if configured)
- [ ] Lag values NULL for first N hours (expected behavior)

**Owner:** ndp-rust-dev

---

### AC-D-05: New Feature Type Test (v11-V02)

**Given:** Feature type registry with base types
**When:** Implementing a new feature type via trait
**Then:** New feature type generates correct SQL

**Verification:**
```bash
# Unit test demonstrates adding new feature type
cargo test -p ndp-gold-ddl test_custom_feature_type_registration -- --nocapture

# Test should show:
# 1. Define new struct implementing FeatureType trait
# 2. Register with registry
# 3. Verify generate_sql() produces expected SQL
```

**Acceptance Checklist:**
- [ ] New feature type implementable via trait only
- [ ] No modification to existing generator code
- [ ] Registration via registry works
- [ ] Generated SQL correct for new type

**Owner:** ndp-tester

---

## Data Dictionary Acceptance Criteria

### AC-D-06: Gold Layer Data Dictionary Complete (v11-010)

**Given:** All Gold objects created (Phases B-D)
**When:** Querying data dictionary
**Then:** All Gold tables, columns, and metadata queryable

**Verification:**
```bash
# List all Gold tables
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT table_name, table_type, stream_id, created_at
FROM data_dictionary.gold_tables
ORDER BY table_name;
"
# Expected: air_quality_hourly, air_quality_daily, outdoor_weather_hourly,
#           state_events_hourly, outdoor_air_quality_hourly (after fast-follower)

# List columns with metadata
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT table_name, column_name, data_type, is_metric, feature_type
FROM data_dictionary.gold_columns
WHERE table_name = 'air_quality_hourly'
ORDER BY ordinal_position;
"
# Expected: All columns with feature_type identified (aggregate, lag, rolling)

# Query specific metric lineage
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT column_name, source_table, source_column, transformation
FROM data_dictionary.gold_column_lineage
WHERE table_name = 'air_quality_hourly'
AND column_name = 'pm25_mean';
"
# Expected: source = silver.air_quality_observations, transformation = AVG(pm25)
```

**Acceptance Checklist:**
- [ ] `data_dictionary.gold_tables` has all Gold views
- [ ] `data_dictionary.gold_columns` has all columns
- [ ] Column metadata includes data_type, is_metric, feature_type
- [ ] Column lineage tracked (source table, transformation)
- [ ] Refresh policies documented
- [ ] Domain associations stored

**Owner:** ndp-analytics-engineer

---

### AC-D-07: Data Dictionary Queryable via SQL

**Given:** Data dictionary populated
**When:** Running discovery queries
**Then:** Useful information returned

**Verification:**
```bash
# List all metrics in a stream
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT column_name, feature_type
FROM data_dictionary.gold_columns
WHERE table_name = 'air_quality_hourly'
AND is_metric = true
ORDER BY column_name;
"
# Expected: Sorted list of metrics with types

# Find all lag features
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT table_name, column_name
FROM data_dictionary.gold_columns
WHERE feature_type = 'lag'
ORDER BY table_name, column_name;
"
# Expected: All lag columns across all streams
```

**Acceptance Checklist:**
- [ ] Query for all metrics in stream works
- [ ] Query for feature type works
- [ ] Query for lineage works
- [ ] Query for domain streams works
- [ ] Query for objectives works

**Owner:** ndp-analytics-engineer

---

## Dashboard Acceptance Criteria

### AC-D-08: Correlation-Ready Dashboard Operational (v11-011)

**Given:** Aligned view with data
**When:** Loading Grafana dashboard
**Then:** Dashboard shows correlated metrics from all streams

**Verification:**
```bash
# Check dashboard provisioned
ls deploy/grafana/dashboards/gold-correlation-ready.json
# Expected: File exists

# Check dashboard loads via API (if Grafana running)
curl -s http://localhost:3000/api/dashboards/uid/gold-correlation-ready | jq '.dashboard.title'
# Expected: "Gold Layer - Correlation Ready" or similar

# Load dashboard in browser
# Navigate to: http://<pi-ip>:3000/d/gold-correlation-ready
# Expected: Dashboard loads without errors
```

**Acceptance Checklist:**
- [ ] Dashboard JSON provisioned
- [ ] Dashboard loads in Grafana
- [ ] Panel shows indoor air quality metrics
- [ ] Panel shows outdoor weather context
- [ ] Panel shows state event counts
- [ ] Objective threshold lines displayed
- [ ] Time range selector works

**Owner:** ndp-grafana-dev

---

### AC-D-09: Dashboard Load Time Within Target

**Given:** Dashboard with 30 days of data
**When:** Loading dashboard in browser
**Then:** Dashboard loads in < 2 seconds

**Verification:**
```bash
# Use browser DevTools Network tab
# Or measure via API:
time curl -s "http://localhost:3000/api/ds/query" \
  -H "Content-Type: application/json" \
  -d '{"queries":[{"datasource":"TimescaleDB","rawSql":"SELECT * FROM gold.indoor_air_quality_aligned WHERE bucket >= NOW() - INTERVAL '\''30 days'\''","format":"table"}]}'
# Expected: real < 2.0s
```

**Acceptance Checklist:**
- [ ] Initial dashboard load < 2 seconds
- [ ] Panel queries use continuous aggregates
- [ ] No direct Silver table queries
- [ ] Time range changes respond quickly

**Owner:** ndp-grafana-dev

---

### AC-D-10: Dashboard Shows Objective Reference Lines

**Given:** Objectives defined in domain config
**When:** Viewing dashboard
**Then:** Threshold reference lines visible on metric panels

**Verification:**
```bash
# Check dashboard JSON includes thresholds
cat deploy/grafana/dashboards/gold-correlation-ready.json | jq '.panels[] | select(.title | contains("PM2.5")) | .fieldConfig.defaults.thresholds'
# Expected: Thresholds configured at 12 (healthy_pm25 objective)

# Verify visually in Grafana
# PM2.5 panel should show horizontal line at 12 ug/m3
# CO2 panel should show horizontal line at 800 ppm
```

**Acceptance Checklist:**
- [ ] CO2 threshold at 800 ppm displayed
- [ ] PM2.5 threshold at 12 ug/m3 displayed
- [ ] Thresholds from objectives config (not hardcoded)
- [ ] Color zones indicate healthy/unhealthy

**Owner:** ndp-grafana-dev

---

## Integration Acceptance Criteria

### AC-D-INT-01: Fast-Follower Manifest Idempotent

**Given:** Fast-follower deployment complete
**When:** Re-running the same manifest
**Then:** No errors, no changes

**Verification:**
```bash
# Run twice
deploy/pi/deploy.sh apply .deploy/test/phase-d-fast-follower.manifest.json
deploy/pi/deploy.sh apply .deploy/test/phase-d-fast-follower.manifest.json
# Expected: Both succeed with exit code 0
```

**Acceptance Checklist:**
- [ ] Second run exits cleanly
- [ ] No "already exists" errors
- [ ] Idempotent behavior confirmed

**Owner:** ndp-rust-dev

---

### AC-D-INT-02: Dashboard Auto-Refreshes with New Stream

**Given:** outdoor-air-quality added via fast-follower
**When:** Viewing dashboard after deployment
**Then:** New stream metrics visible without manual intervention

**Verification:**
```bash
# After fast-follower deployment, refresh dashboard
# Expected: outdoor_air metrics appear in aligned view panel
# May require dashboard panel configuration update (documented in fast-follower procedure)
```

**Acceptance Checklist:**
- [ ] Dashboard shows new stream after refresh
- [ ] No manual Grafana panel edits required (or minimal)
- [ ] Documentation covers any required steps

**Owner:** ndp-grafana-dev

---

## Performance Acceptance Criteria

### AC-D-PERF-01: Fast-Follower Deployment Time

**Given:** Config changes ready
**When:** Running deploy.sh apply
**Then:** Deployment completes in < 5 minutes

**Verification:**
```bash
time deploy/pi/deploy.sh apply .deploy/test/phase-d-fast-follower.manifest.json
# Expected: real < 5m
```

**Acceptance Checklist:**
- [ ] DDL generation < 30 seconds
- [ ] Object creation < 2 minutes
- [ ] Initial refresh < 2 minutes
- [ ] Total deployment < 5 minutes

**Owner:** ndp-tester

---

### AC-D-PERF-02: Aligned View Performance with 4 Streams

**Given:** Aligned view with 4 streams (after fast-follower)
**When:** Querying 30-day range
**Then:** Query < 100ms

**Verification:**
```bash
docker exec timescaledb psql -U postgres -d ndp -c "
EXPLAIN (ANALYZE, COSTS, TIMING)
SELECT bucket, indoor_pm25_mean, outdoor_temperature_c_mean, outdoor_air_pm25_mean
FROM gold.indoor_air_quality_aligned
WHERE bucket >= NOW() - INTERVAL '30 days';
"
# Expected: Execution Time: < 100ms
```

**Acceptance Checklist:**
- [ ] 4-stream aligned view query < 100ms
- [ ] No performance regression from 3-stream view
- [ ] JOIN strategy still efficient

**Owner:** ndp-tester

---

## Exit Criteria Summary

Phase D is complete when ALL of the following are true:

### Fast-Follower Test (CRITICAL)
- [ ] AC-D-01: **Fast-follower test passes (< 1 hour, zero code changes)**
- [ ] AC-D-02: Git diff shows config-only changes

### Feature Computation
- [ ] AC-D-03: Basic features (mean, std, min, max) work
- [ ] AC-D-04: Lag features (t-1h, t-6h, t-24h) work
- [ ] AC-D-05: New feature type addable via trait

### Data Dictionary
- [ ] AC-D-06: Gold layer data dictionary complete
- [ ] AC-D-07: Data dictionary queryable via SQL

### Dashboard
- [ ] AC-D-08: Correlation-ready dashboard operational
- [ ] AC-D-09: Dashboard loads < 2 seconds
- [ ] AC-D-10: Objective thresholds displayed

### Integration
- [ ] AC-D-INT-01: Fast-follower manifest idempotent
- [ ] AC-D-INT-02: Dashboard shows new stream

### Performance
- [ ] AC-D-PERF-01: Fast-follower deployment < 5 minutes
- [ ] AC-D-PERF-02: 4-stream aligned view query < 100ms

---

## CRITICAL: Phase D Failure Protocol

If AC-D-01 (Fast-Follower Test) FAILS:

1. **STOP Phase D immediately**
2. Document the failure in FAST-FOLLOWER-REPORT.md:
   - What code change was required?
   - Why was it required?
   - What architecture assumption was incorrect?
3. Return to Phase A-C to fix architecture
4. Re-run fast-follower test after fix
5. **ONLY proceed to Phase E after successful test**

---

## Phase D to Phase E Gate

Before starting Phase E:

- [ ] **CRITICAL**: AC-D-01 PASSED (fast-follower test)
- [ ] **CRITICAL**: Architecture validated as config-driven
- [ ] All feature types working
- [ ] Data dictionary complete
- [ ] Dashboard operational

---

## Sign-Off

| Role | Name | Date | Signature |
|------|------|------|-----------|
| ndp-tester | | | |
| ndp-grafana-dev | | | |
| ndp-analytics-engineer | | | |
| ndp-rust-dev | | | |

---

*Acceptance Criteria created: 2026-02-04 by ndp-tester*
