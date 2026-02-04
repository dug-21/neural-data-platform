# FE-001 Phase B: First Stream (air-quality) - Acceptance Criteria

> **Phase:** B (First Stream - Reference Implementation)
> **Version:** 1.1
> **Release Version:** v1.1.1
> **Created:** 2026-02-04
> **Last Updated:** 2026-02-04

---

## Overview

This document defines the acceptance criteria for Phase B, which applies the Phase A architecture to the `air-quality` stream as the reference implementation. Success here validates that the declarative infrastructure works end-to-end.

---

## Feature Acceptance Criteria

### AC-B-01: Stream Type Classification Present (v11-001)

**Given:** The air-quality stream configuration file
**When:** The configuration is loaded
**Then:** The `stream_type` field is present and set to "observation"

**Verification:**
```bash
# Check stream_type in config
cat config/base/streams/air-quality/config.yaml | grep "stream_type"
# Expected: stream_type: "observation"

# Validate against schema
ndp-validate --config config/base/streams/air-quality/config.yaml
# Expected: Exit code 0, stream_type validated
```

**Acceptance Checklist:**
- [ ] `stream_type: "observation"` present in air-quality config
- [ ] Schema validates stream_type enum values
- [ ] All V1.0 streams have stream_type added

**Owner:** ndp-architect

---

### AC-B-02: Classification Propagates to Data Dictionary (v11-002)

**Given:** A stream with stream_type classification
**When:** The stream is deployed to Silver layer
**Then:** The data dictionary contains the classification

**Verification:**
```bash
# Query data dictionary for stream classification
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT stream_id, stream_type, description
FROM data_dictionary.stream_classification
WHERE stream_id = 'air-quality';
"
# Expected: Row with stream_type = 'observation'
```

**Acceptance Checklist:**
- [ ] `data_dictionary.stream_classification` table exists
- [ ] air-quality entry present with stream_type = observation
- [ ] Classification syncs on deploy

**Owner:** ndp-timescale-dev

---

### AC-B-03: Hourly Continuous Aggregate Operational (v11-003)

**Given:** air-quality stream with gold_etl enabled
**When:** Manifest is deployed with gold-table declaration
**Then:** `gold.air_quality_hourly` continuous aggregate is created and operational

**Verification:**
```bash
# Verify continuous aggregate exists
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT view_name, materialization_hypertable_name
FROM timescaledb_information.continuous_aggregates
WHERE view_schema = 'gold' AND view_name = 'air_quality_hourly';
"
# Expected: Row returned with view_name = 'air_quality_hourly'

# Verify schema contains expected columns
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT column_name, data_type
FROM information_schema.columns
WHERE table_schema = 'gold' AND table_name = 'air_quality_hourly'
ORDER BY ordinal_position;
"
# Expected: bucket, ndp_id, pm25_mean, pm25_std, pm25_min, pm25_max, pm25_p95, etc.

# Verify data is present (requires Silver data)
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT COUNT(*) as row_count FROM gold.air_quality_hourly;
"
# Expected: row_count > 0
```

**Acceptance Checklist:**
- [ ] `gold.air_quality_hourly` view exists
- [ ] View has `bucket` column with 1-hour granularity
- [ ] View has `ndp_id` column for entity grouping
- [ ] View has all configured metrics: pm25_mean, pm25_std, pm25_min, pm25_max, pm25_p95
- [ ] View has pm10_mean, pm10_min, pm10_max
- [ ] View has co2_mean, co2_std, co2_min, co2_max
- [ ] View has temperature_c_mean, temperature_c_min, temperature_c_max
- [ ] View has humidity_pct_mean, humidity_pct_min, humidity_pct_max
- [ ] View has tvoc_index_mean, tvoc_index_max
- [ ] View has nox_index_mean, nox_index_max
- [ ] View has sample_count column
- [ ] Data populates after Silver ETL runs

**Owner:** ndp-rust-dev

---

### AC-B-04: Daily Continuous Aggregate Operational (v11-003)

**Given:** air-quality stream with gold_etl enabled and daily granularity
**When:** Manifest is deployed
**Then:** `gold.air_quality_daily` continuous aggregate is created

**Verification:**
```bash
# Verify daily aggregate exists
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT view_name
FROM timescaledb_information.continuous_aggregates
WHERE view_schema = 'gold' AND view_name = 'air_quality_daily';
"
# Expected: Row with view_name = 'air_quality_daily'

# Verify daily granularity
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT bucket, pm25_mean
FROM gold.air_quality_daily
ORDER BY bucket DESC
LIMIT 5;
"
# Expected: bucket values are day boundaries (00:00:00)
```

**Acceptance Checklist:**
- [ ] `gold.air_quality_daily` view exists
- [ ] View has 1-day granularity buckets
- [ ] View contains same metric columns as hourly
- [ ] Data populates after refresh

**Owner:** ndp-rust-dev

---

### AC-B-05: Refresh Policy Operational (v11-004)

**Given:** Continuous aggregates with configured refresh policy
**When:** Policy interval elapses
**Then:** Aggregates are automatically refreshed

**Verification:**
```bash
# Verify refresh policy exists
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT view_name, schedule_interval, config
FROM timescaledb_information.jobs
WHERE application_name LIKE '%continuous_aggregate%'
  AND config::text LIKE '%air_quality_hourly%';
"
# Expected: schedule_interval = '00:15:00' (15 minutes)

# Verify refresh is occurring (check job stats)
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT hypertable_name, last_successful_run, next_scheduled_run
FROM timescaledb_information.job_stats js
JOIN timescaledb_information.jobs j ON js.job_id = j.job_id
WHERE j.config::text LIKE '%air_quality_hourly%';
"
# Expected: last_successful_run is recent
```

**Acceptance Checklist:**
- [ ] Hourly aggregate has refresh policy with 15-minute interval
- [ ] Daily aggregate has refresh policy with appropriate interval
- [ ] Policies use configured start_offset (4 hours)
- [ ] Policies use configured end_offset (15 minutes)
- [ ] Job stats show successful runs

**Owner:** ndp-timescale-dev

---

### AC-B-06: Query Performance Within Target (v11-003)

**Given:** Gold aggregate with 30 days of data
**When:** Querying the 30-day range
**Then:** Query completes in < 100ms on Raspberry Pi 5

**Verification:**
```bash
# Run performance test query
docker exec timescaledb psql -U postgres -d ndp -c "
EXPLAIN (ANALYZE, COSTS, TIMING)
SELECT bucket, pm25_mean, co2_mean, sample_count
FROM gold.air_quality_hourly
WHERE bucket >= NOW() - INTERVAL '30 days'
ORDER BY bucket;
"
# Expected: Execution Time: < 100ms

# Alternative: Use pg_stat_statements
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT mean_exec_time, calls
FROM pg_stat_statements
WHERE query LIKE '%gold.air_quality_hourly%'
ORDER BY calls DESC LIMIT 1;
"
# Expected: mean_exec_time < 100
```

**Acceptance Checklist:**
- [ ] 30-day query on gold.air_quality_hourly < 100ms
- [ ] 30-day query on gold.air_quality_daily < 50ms
- [ ] Query uses continuous aggregate (not underlying table)
- [ ] No sequential scan on underlying hypertable

**Owner:** ndp-tester

---

### AC-B-07: Config-Only Metric Addition Works

**Given:** Working gold.air_quality_hourly aggregate
**When:** Adding a new metric (p99) via config change only
**Then:** New column appears after manifest deploy with action=recreate

**Verification:**
```bash
# 1. Verify p99 NOT present initially
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT column_name FROM information_schema.columns
WHERE table_schema = 'gold' AND table_name = 'air_quality_hourly'
AND column_name LIKE '%p99%';
"
# Expected: 0 rows

# 2. Edit config to add p99 metric to pm25
# (manually edit config/base/streams/air-quality/config.yaml)

# 3. Deploy with recreate action
deploy/pi/deploy.sh apply .deploy/test/phase-b-recreate.manifest.json

# 4. Verify p99 now present
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT column_name FROM information_schema.columns
WHERE table_schema = 'gold' AND table_name = 'air_quality_hourly'
AND column_name LIKE '%p99%';
"
# Expected: 1 row (pm25_p99)
```

**Acceptance Checklist:**
- [ ] Config change adds metric without code change
- [ ] Manifest action=recreate triggers DDL regeneration
- [ ] Old view is dropped and recreated
- [ ] New column appears in schema
- [ ] Data repopulates on next refresh

**Owner:** ndp-tester

---

### AC-B-08: Data Dictionary Updated for Gold Tables (v11-010 partial)

**Given:** Gold continuous aggregates created
**When:** Data dictionary sync runs
**Then:** Gold table metadata appears in data dictionary

**Verification:**
```bash
# Check gold_tables entry
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT table_schema, table_name, table_type, stream_id
FROM data_dictionary.gold_tables
WHERE table_name LIKE 'air_quality%';
"
# Expected: Rows for air_quality_hourly and air_quality_daily

# Check gold_columns entries
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT table_name, column_name, data_type, is_metric
FROM data_dictionary.gold_columns
WHERE table_name = 'air_quality_hourly'
ORDER BY ordinal_position
LIMIT 10;
"
# Expected: Columns with is_metric = true for metrics
```

**Acceptance Checklist:**
- [ ] `data_dictionary.gold_tables` table exists
- [ ] Entry for gold.air_quality_hourly
- [ ] Entry for gold.air_quality_daily
- [ ] `data_dictionary.gold_columns` populated
- [ ] Metric columns marked appropriately

**Owner:** ndp-analytics-engineer

---

## Integration Acceptance Criteria

### AC-B-INT-01: Full Pipeline Works (Config to Operational Aggregate)

**Given:** Clean database state (no Gold objects)
**When:** Deploying Phase B manifest
**Then:** Complete Gold layer for air-quality is operational

**Verification:**
```bash
# Start from clean slate
docker exec timescaledb psql -U postgres -d ndp -c "DROP SCHEMA IF EXISTS gold CASCADE;"
docker exec timescaledb psql -U postgres -d ndp -c "CREATE SCHEMA gold;"

# Deploy Phase B manifest
deploy/pi/deploy.sh apply .deploy/test/phase-b-air-quality.manifest.json

# Verify all objects created
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT schemaname, matviewname
FROM pg_matviews
WHERE schemaname = 'gold';
"
# Expected: air_quality_hourly, air_quality_daily
```

**Acceptance Checklist:**
- [ ] deploy.sh apply creates Gold schema if needed
- [ ] Continuous aggregates created
- [ ] Refresh policies attached
- [ ] Data dictionary updated
- [ ] No manual intervention required

**Owner:** ndp-rust-dev

---

### AC-B-INT-02: Manifest Idempotency (Sync Action)

**Given:** Existing Gold aggregates
**When:** Re-running manifest with action=sync
**Then:** No changes made, no errors

**Verification:**
```bash
# Run deploy twice
deploy/pi/deploy.sh apply .deploy/test/phase-b-air-quality.manifest.json
deploy/pi/deploy.sh apply .deploy/test/phase-b-air-quality.manifest.json

# Both should succeed with no errors
# Expected: Exit code 0 both times
```

**Acceptance Checklist:**
- [ ] action=sync is idempotent
- [ ] No DROP/CREATE on re-run
- [ ] Exit code 0 on duplicate apply
- [ ] Log shows "already exists" message

**Owner:** ndp-rust-dev

---

## Tool Build Acceptance Criteria

### AC-B-TOOL-01: ndp-gold-ddl Tool Builds Successfully

**Given:** Phase B implementation complete
**When:** Running the tool build process
**Then:** `ndp-gold-ddl` binary is available on the Pi

**Verification:**
```bash
# Build the tool
cargo build --release -p ndp-gold-ddl

# Verify binary exists
ls -la target/release/ndp-gold-ddl
# Expected: Executable file present

# Verify it runs
./target/release/ndp-gold-ddl --help
# Expected: Help text displayed
```

**Acceptance Checklist:**
- [ ] `cargo build --release -p ndp-gold-ddl` succeeds
- [ ] Binary at `target/release/ndp-gold-ddl` is executable
- [ ] Binary runs without library errors on Pi (ARM64)
- [ ] deploy.sh can locate and invoke the tool

**Owner:** ndp-rust-dev

---

### AC-B-TOOL-02: deploy.sh Has Tool Build Mechanism

**Given:** deploy.sh apply command
**When:** Gold-tables declaration is processed
**Then:** ndp-gold-ddl tool is built or verified available before use

**Verification:**
```bash
# Run deploy with dry-run
deploy/pi/deploy.sh apply .deploy/releases/v1.1.1.manifest.json --dry-run

# Check for tool build step in output
# Expected: Log shows tool availability check or build step
```

**Acceptance Checklist:**
- [ ] deploy.sh checks for ndp-gold-ddl before Phase 5
- [ ] If missing, deploy.sh builds the tool OR fails with clear message
- [ ] Tool declaration type implemented OR cargo build step added
- [ ] Documentation updated for tool build mechanism

**Owner:** ndp-rust-dev

---

## Release Preparation Acceptance Criteria

### AC-B-REL-01: Manifest Created Per RELEASE-POLICY

**Given:** Phase B implementation complete
**When:** Preparing for release
**Then:** Manifest exists and is valid

**Verification:**
```bash
# Verify manifest exists
ls -la .deploy/releases/v1.1.1.manifest.json

# Validate manifest JSON
cat .deploy/releases/v1.1.1.manifest.json | jq .
# Expected: Valid JSON with release_version: "1.1.1"

# Verify required fields
cat .deploy/releases/v1.1.1.manifest.json | jq '.release_version, .description, .changes'
# Expected: All fields present
```

**Acceptance Checklist:**
- [ ] `.deploy/releases/v1.1.1.manifest.json` exists
- [ ] `version` field is "1.0"
- [ ] `release_version` field is "1.1.1"
- [ ] `description` summarizes Phase B changes
- [ ] `changes` array includes: tool, stream, gold-tables, dictionary

**Owner:** ndp-scrum-master

---

### AC-B-REL-02: CHANGELOG.md Updated

**Given:** Phase B release
**When:** Reviewing CHANGELOG.md
**Then:** v1.1.1 section documents all changes

**Verification:**
```bash
# Check for v1.1.1 section
grep -A 20 "## \[1.1.1\]" CHANGELOG.md
# Expected: Section with Added, Changed sections
```

**Acceptance Checklist:**
- [ ] `## [1.1.1] - YYYY-MM-DD` section exists
- [ ] Added section lists Gold layer features
- [ ] Changed section lists config changes
- [ ] Link references updated at bottom

**Owner:** ndp-scrum-master

---

### AC-B-REL-03: Git Tag Created

**Given:** Phase B code committed
**When:** Creating release
**Then:** Annotated tag v1.1.1 exists

**Verification:**
```bash
# Verify tag exists
git tag -l "v1.1.1"
# Expected: v1.1.1

# Verify tag is annotated
git show v1.1.1
# Expected: Shows tag message and commit info
```

**Acceptance Checklist:**
- [ ] Tag `v1.1.1` created with `-a` flag (annotated)
- [ ] Tag message describes Phase B changes
- [ ] Tag pushed to remote: `git push origin v1.1.1`

**Owner:** ndp-scrum-master

---

### AC-B-REL-04: Pi Deployment Successful

**Given:** v1.1.1 release pushed
**When:** Deploying to Pi
**Then:** Gold layer is operational

**Verification:**
```bash
# On Pi:
git pull
git describe --tags --exact-match
# Expected: v1.1.1

./deploy.sh apply .deploy/releases/v1.1.1.manifest.json
# Expected: Exit code 0

cat /var/ndp/deployed-version
# Expected: v1.1.1

# Verify Gold tables exist
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT view_name FROM timescaledb_information.continuous_aggregates
WHERE view_schema = 'gold';
"
# Expected: air_quality_hourly, air_quality_daily
```

**Acceptance Checklist:**
- [ ] `git pull` retrieves v1.1.1 code and tag
- [ ] `deploy.sh apply` completes without error
- [ ] `/var/ndp/deployed-version` shows v1.1.1
- [ ] Gold continuous aggregates operational
- [ ] Refresh policies running

**Owner:** ndp-tester

---

## Performance Acceptance Criteria

### AC-B-PERF-01: Refresh Policy Resource Usage Acceptable

**Given:** Refresh policy running on Pi 5
**When:** Monitoring during refresh window
**Then:** CPU usage < 5% sustained average

**Verification:**
```bash
# Monitor during refresh (on Pi)
top -b -n 10 -d 6 | grep postgres
# Expected: postgres CPU < 5% average over 60 seconds
```

**Acceptance Checklist:**
- [ ] Refresh policy CPU < 5% sustained
- [ ] Memory < 100 MB during refresh
- [ ] No I/O spikes causing delays

**Owner:** ndp-tester

---

### AC-B-PERF-02: Storage Within Budget

**Given:** 30 days of air-quality data
**When:** Checking Gold layer storage
**Then:** Storage < 20 MB total for air-quality Gold objects

**Verification:**
```bash
docker exec timescaledb psql -U postgres -d ndp -c "
SELECT
    hypertable_name,
    pg_size_pretty(total_bytes) as total_size
FROM hypertable_size_info
WHERE hypertable_name LIKE '%air_quality%'
AND hypertable_schema = 'gold';
"
# Expected: total_size < 20 MB per aggregate
```

**Acceptance Checklist:**
- [ ] gold.air_quality_hourly < 10 MB for 30 days
- [ ] gold.air_quality_daily < 5 MB for 30 days
- [ ] Growth rate documented

**Owner:** ndp-tester

---

## Exit Criteria Summary

Phase B is complete when ALL of the following are true:

### Stream Classification
- [ ] AC-B-01: stream_type: observation in air-quality config
- [ ] AC-B-02: Classification in data_dictionary.stream_classification

### Continuous Aggregates
- [ ] AC-B-03: gold.air_quality_hourly operational
- [ ] AC-B-04: gold.air_quality_daily operational
- [ ] AC-B-05: Refresh policies running (15-minute interval)
- [ ] AC-B-06: Query performance < 100ms for 30-day range

### Architecture Validation
- [ ] AC-B-07: Config-only metric addition works
- [ ] AC-B-08: Data dictionary updated

### Integration
- [ ] AC-B-INT-01: Full pipeline works (config to operational)
- [ ] AC-B-INT-02: Manifest idempotency verified

### Tool Build
- [ ] AC-B-TOOL-01: ndp-gold-ddl builds successfully
- [ ] AC-B-TOOL-02: deploy.sh has tool build mechanism

### Release Preparation (RELEASE-POLICY)
- [ ] AC-B-REL-01: Manifest `.deploy/releases/v1.1.1.manifest.json` created
- [ ] AC-B-REL-02: CHANGELOG.md updated with v1.1.1 section
- [ ] AC-B-REL-03: Git tag v1.1.1 created (annotated)
- [ ] AC-B-REL-04: Pi deployment successful

### Performance
- [ ] AC-B-PERF-01: Refresh CPU < 5% sustained
- [ ] AC-B-PERF-02: Storage < 20 MB for 30 days

---

## Phase B to Phase C Gate

Before starting Phase C, verify:

### Architecture Validation (CRITICAL)
- [ ] Adding a new metric requires ONLY config edit + manifest recreate
- [ ] No Rust code changes needed for metric addition
- [ ] deploy.sh correctly invokes ndp-gold-ddl
- [ ] air-quality serves as reference implementation for other streams

### Release Completion (CRITICAL)
- [ ] v1.1.1 released and pushed to remote
- [ ] Pi deployment verified operational
- [ ] `/var/ndp/deployed-version` shows `v1.1.1`

### Tool Infrastructure (CRITICAL)
- [ ] ndp-gold-ddl tool builds on Pi (ARM64)
- [ ] Tool build mechanism documented for future releases

---

## Sign-Off

| Role | Name | Date | Signature |
|------|------|------|-----------|
| ndp-rust-dev | | | |
| ndp-timescale-dev | | | |
| ndp-tester | | | |

---

*Acceptance Criteria created: 2026-02-04 by ndp-tester*
