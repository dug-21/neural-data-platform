# Phase D: Fast-Follower Validation Procedure

> **Phase:** D (Validation)
> **Purpose:** Timed validation procedure proving config-driven architecture
> **Target Time:** < 1 hour total
> **Critical Metric:** Zero Rust code changes
> **Created:** 2026-02-04

---

## Overview

This document provides the **step-by-step timed procedure** for the Fast-Follower Test (v11-V01). The test validates that a new stream can be added to the Gold layer using only configuration changes.

**Success Criteria**:
- Total time: < 1 hour
- Code changes: Zero `.rs` files modified
- Result: `outdoor-air-quality` operational in Gold layer

---

## Pre-Test Checklist

Before starting the timer, verify:

| Item | Check | Command |
|------|-------|---------|
| Phase A-C complete | All tests passing | `./scripts/test-phases-a-c.sh` |
| Git state clean | No uncommitted changes | `git status --porcelain` |
| outdoor-air-quality Silver exists | Has data | `dcx timescaledb psql -c "SELECT COUNT(*) FROM silver.outdoor_air_quality_observations"` |
| outdoor-air-quality NOT in Gold | No aggregate | `dcx timescaledb psql -c "SELECT * FROM timescaledb_information.continuous_aggregates WHERE view_name LIKE '%outdoor_air%'"` |
| Docker running | Services up | `docker compose ps` |
| Clock ready | Timer accessible | Start stopwatch |

---

## Timed Procedure

### START TIMER

Record start time: `_____________`

---

### Step 1: Read Documentation (Budget: 10 min)

**Checkpoint 1: ____:____ (target: 10:00)**

Review these documents to understand the config patterns:

1. Read `config/base/streams/air-quality/config.yaml` - Reference `gold_etl` section
2. Read `config/domains/indoor-air-quality/domain.yaml` - Domain structure
3. Read `.deploy/releases/` - Manifest format

**Key patterns to note:**
- `gold_etl.aggregates.granularities` format
- `gold_etl.aggregates.fields` structure
- Domain `streams` array structure
- Manifest `declarations` format

---

### Step 2: Create gold_etl Config (Budget: 15 min)

**Checkpoint 2: ____:____ (target: 25:00)**

Edit file: `config/base/streams/outdoor-air-quality/config.yaml`

Add `gold_etl` section (copy from air-quality, modify):

```yaml
# Add to existing config - do NOT replace existing content
# Add this section after existing silver_etl section

gold_etl:
  enabled: true
  description: "Outdoor air quality hourly aggregates"

  aggregates:
    granularities: ["1 hour"]
    default_metrics: ["mean", "count"]
    fields:
      pm25:
        metrics: ["mean", "std", "min", "max"]
      pm10:
        metrics: ["mean", "min", "max"]
      aqi:
        metrics: ["mean", "max"]
      # Add other fields from outdoor-air-quality schema

  features:
    lag:
      enabled: true
      lags_hours: [1, 24]
      fields: ["pm25", "aqi"]

  refresh_policy:
    schedule_interval: "15 minutes"
    start_offset: "4 hours"
    end_offset: "15 minutes"
```

**Validation command:**
```bash
ndp-validate --stream outdoor-air-quality
# Should output: "Valid"
```

---

### Step 3: Update Domain Config (Budget: 10 min)

**Checkpoint 3: ____:____ (target: 35:00)**

Edit file: `config/domains/indoor-air-quality/domain.yaml`

Add `outdoor-air-quality` to streams array:

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
    # ADD THIS:
    - stream_id: outdoor-air-quality
      alias: oaq
      role: context

  alignment:
    view_name: indoor_air_quality_aligned
    granularity: "1 hour"
    join_strategy: full_outer
    null_handling: by_stream_type

  # Existing objectives - no changes needed
```

**Validation command:**
```bash
ndp-gold-ddl validate --domain indoor-air-quality
# Should output: "Valid"
```

---

### Step 4: Create/Update Manifest (Budget: 5 min)

**Checkpoint 4: ____:____ (target: 40:00)**

Create file: `.deploy/releases/v1.1.1-fast-follower.manifest.json`

```json
{
  "version": "1.1.1",
  "description": "Phase D Fast-Follower: Add outdoor-air-quality to Gold layer",
  "release_date": "2026-02-XX",
  "declarations": [
    {
      "type": "etcd-config",
      "stream_id": "outdoor-air-quality",
      "action": "sync"
    },
    {
      "type": "gold-table",
      "stream_id": "outdoor-air-quality",
      "action": "sync"
    },
    {
      "type": "domain",
      "domain_id": "indoor-air-quality",
      "action": "recreate"
    },
    {
      "type": "dictionary",
      "action": "sync"
    }
  ]
}
```

---

### Step 5: Run deploy.sh apply (Budget: 5 min)

**Checkpoint 5: ____:____ (target: 45:00)**

Execute deployment:

```bash
# Dry run first
./deploy/pi/deploy.sh apply .deploy/releases/v1.1.1-fast-follower.manifest.json --dry-run

# Review generated SQL, then execute
./deploy/pi/deploy.sh apply .deploy/releases/v1.1.1-fast-follower.manifest.json
```

**Expected output:**
```
Phase 1: Validating manifest... OK
Phase 3: No migrations
Phase 4: Gold tables...
  - Creating gold.outdoor_air_quality_hourly... OK
  - Recreating gold.indoor_air_quality_aligned... OK
Phase 5: Syncing etcd configs... OK
Phase 7: Syncing data dictionary... OK
Phase 9: Updating device state... OK

Deployment complete.
```

---

### Step 6: Verify in Database (Budget: 5 min)

**Checkpoint 6: ____:____ (target: 50:00)**

Run verification queries:

```bash
# Verify continuous aggregate exists
dcx timescaledb psql -U postgres -d ndp -c "
SELECT view_schema, view_name
FROM timescaledb_information.continuous_aggregates
WHERE view_name LIKE '%outdoor_air%';
"
# Expected: gold | outdoor_air_quality_hourly

# Verify refresh policy
dcx timescaledb psql -U postgres -d ndp -c "
SELECT job_id, schedule_interval
FROM timescaledb_information.jobs
WHERE hypertable_name = 'outdoor_air_quality_hourly';
"
# Expected: Schedule interval present

# Verify aligned view has new columns
dcx timescaledb psql -U postgres -d ndp -c "
SELECT column_name
FROM information_schema.columns
WHERE table_schema = 'gold'
  AND table_name = 'indoor_air_quality_aligned'
  AND column_name LIKE 'oaq_%';
"
# Expected: oaq_pm25_mean, oaq_aqi_mean, etc.

# Verify data dictionary updated
dcx timescaledb psql -U postgres -d ndp -c "
SELECT table_name FROM data_dictionary.gold_tables
WHERE table_name LIKE '%outdoor_air%';
"
# Expected: gold.outdoor_air_quality_hourly
```

---

### Step 7: Update Dashboard (Optional, Budget: 10 min)

**Checkpoint 7: ____:____ (target: 60:00)**

If time permits, add outdoor-air-quality panel to dashboard:

Edit: `deploy/grafana/dashboards/gold-correlation.json`

Add panel querying `oaq_pm25_mean` from aligned view.

**Validation:**
```bash
# Restart Grafana to pick up changes
docker compose restart grafana

# Open dashboard and verify panel loads
```

---

### STOP TIMER

Record end time: `_____________`
Total duration: `_____________`

---

## Post-Test Verification

### Code Change Verification

**CRITICAL: Run immediately after stopping timer**

```bash
# List all changed files
git diff --name-only HEAD

# Expected output - ONLY config files:
# config/base/streams/outdoor-air-quality/config.yaml
# config/domains/indoor-air-quality/domain.yaml
# .deploy/releases/v1.1.1-fast-follower.manifest.json
# (optionally) deploy/grafana/dashboards/gold-correlation.json

# Filter for Rust files - MUST BE EMPTY
git diff --name-only HEAD | grep '\.rs$'
# Expected: NO OUTPUT

# If ANY .rs files appear, TEST FAILED
```

### Data Flow Verification

```bash
# Wait for aggregate refresh (up to 15 min)
# Then verify data exists

dcx timescaledb psql -U postgres -d ndp -c "
SELECT bucket, pm25_mean, aqi_mean, sample_count
FROM gold.outdoor_air_quality_hourly
ORDER BY bucket DESC
LIMIT 5;
"

# Verify aligned view has data from new stream
dcx timescaledb psql -U postgres -d ndp -c "
SELECT bucket, indoor_pm25, oaq_pm25_mean
FROM gold.indoor_air_quality_aligned
WHERE oaq_pm25_mean IS NOT NULL
ORDER BY bucket DESC
LIMIT 5;
"
```

---

## Result Documentation

### Fast-Follower Report Template

Create file: `product/features/fe-001/phase-d/reports/FAST-FOLLOWER-REPORT.md`

```markdown
# Fast-Follower Test Report

**Date:** YYYY-MM-DD
**Tester:** [Name]
**Result:** PASS / FAIL

## Timing Summary

| Step | Budget | Actual | Status |
|------|--------|--------|--------|
| 1. Read docs | 10 min | ___ min | OK/OVER |
| 2. gold_etl config | 15 min | ___ min | OK/OVER |
| 3. Domain config | 10 min | ___ min | OK/OVER |
| 4. Manifest | 5 min | ___ min | OK/OVER |
| 5. Deploy | 5 min | ___ min | OK/OVER |
| 6. Verify | 5 min | ___ min | OK/OVER |
| 7. Dashboard | 10 min | ___ min | OK/OVER/SKIP |
| **Total** | **60 min** | **___ min** | **PASS/FAIL** |

## Code Change Verification

```
$ git diff --name-only HEAD | grep '\.rs$'
[paste output - should be empty]
```

Rust files modified: 0 / FAIL: [list files]

## Database Verification

```sql
-- Continuous aggregate exists: YES / NO
-- Refresh policy exists: YES / NO
-- Aligned view columns: [count] / MISSING
-- Data dictionary: UPDATED / MISSING
```

## Issues Encountered

1. [Description of any issues]
2. [Workarounds applied]

## Architecture Gaps Found

[List any code changes that SHOULD have been config-driven but weren't]

## Recommendations

1. [Improvements for V1.2]
```

---

## Failure Handling

### If Time Budget Exceeded

1. Stop at 60 minutes
2. Document which step exceeded budget
3. Complete remaining steps without timer
4. Analyze bottleneck
5. File issue for architecture improvement

### If Code Changes Required

**This is a CRITICAL FAILURE**

1. STOP IMMEDIATELY
2. Document what code change was needed
3. DO NOT PROCEED to Phase E
4. Return to Phase A-C to fix architecture
5. File blocking issue: "Fast-follower requires code change: [description]"

### Recovery Procedure

If test fails and needs reset:

```bash
# Remove Gold artifacts
dcx timescaledb psql -U postgres -d ndp -c "
DROP MATERIALIZED VIEW IF EXISTS gold.outdoor_air_quality_hourly CASCADE;
"

# Revert config changes
git checkout config/base/streams/outdoor-air-quality/config.yaml
git checkout config/domains/indoor-air-quality/domain.yaml
rm .deploy/releases/v1.1.1-fast-follower.manifest.json

# Regenerate aligned view without new stream
./deploy/pi/deploy.sh apply --domain indoor-air-quality --action recreate
```

---

## Automated Verification Script

Save as `scripts/verify-fast-follower.sh`:

```bash
#!/bin/bash
set -e

echo "=== Fast-Follower Verification ==="

echo -n "Checking for Rust file changes... "
RUST_CHANGES=$(git diff --name-only HEAD | grep '\.rs$' | wc -l)
if [ "$RUST_CHANGES" -eq 0 ]; then
    echo "PASS (0 files)"
else
    echo "FAIL ($RUST_CHANGES files)"
    git diff --name-only HEAD | grep '\.rs$'
    exit 1
fi

echo -n "Checking continuous aggregate exists... "
CA_EXISTS=$(dcx timescaledb psql -U postgres -d ndp -t -c \
    "SELECT COUNT(*) FROM timescaledb_information.continuous_aggregates \
     WHERE view_name = 'outdoor_air_quality_hourly'")
if [ "$CA_EXISTS" -gt 0 ]; then
    echo "PASS"
else
    echo "FAIL"
    exit 1
fi

echo -n "Checking aligned view has new columns... "
NEW_COLS=$(dcx timescaledb psql -U postgres -d ndp -t -c \
    "SELECT COUNT(*) FROM information_schema.columns \
     WHERE table_schema = 'gold' \
       AND table_name = 'indoor_air_quality_aligned' \
       AND column_name LIKE 'oaq_%'")
if [ "$NEW_COLS" -gt 0 ]; then
    echo "PASS ($NEW_COLS columns)"
else
    echo "FAIL"
    exit 1
fi

echo -n "Checking data dictionary updated... "
DD_ENTRY=$(dcx timescaledb psql -U postgres -d ndp -t -c \
    "SELECT COUNT(*) FROM data_dictionary.gold_tables \
     WHERE table_name LIKE '%outdoor_air%'")
if [ "$DD_ENTRY" -gt 0 ]; then
    echo "PASS"
else
    echo "FAIL"
    exit 1
fi

echo ""
echo "=== All Verifications PASSED ==="
echo "Fast-follower test successful!"
```

---

## References

- [TEST-PLAN.md](./TEST-PLAN.md) - Phase D test plan
- [PHASE-D-OVERVIEW.md](../specification/PHASE-D-OVERVIEW.md) - Phase D specification
- [CONFIG-DEPLOYMENT-FLOW.md](../../architecture/CONFIG-DEPLOYMENT-FLOW.md) - Deployment flow

---

*Fast-Follower Validation Procedure created: 2026-02-04*
