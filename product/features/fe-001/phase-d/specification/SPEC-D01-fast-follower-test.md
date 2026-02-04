# SPEC-D01: Fast-Follower Stream Test (v11-V01)

**Feature ID**: v11-V01
**Feature Name**: Fast-Follower Stream Test
**Priority**: Critical
**Created**: 2026-02-04
**Status**: Draft

---

## 1. Overview

### 1.1 User Story

> As a **platform operator**, I want to add a new stream (`outdoor-air-quality`) to the Gold layer by **only editing configuration files**, so that I can extend the platform without requiring development resources or code changes.

### 1.2 Goal

Validate the V1.1 Gold Layer architecture by demonstrating that a 4th stream can be added to Gold layer aggregates, features, and aligned views in **under 1 hour** with **zero Rust code changes**.

### 1.3 Success Criteria Summary

| Criterion | Target |
|-----------|--------|
| Total time | < 60 minutes |
| Rust code changes | Zero |
| Config files modified | 2-4 files |
| Gold layer operational | All queries pass |

---

## 2. Functional Requirements

### 2.1 Pre-Conditions (FR-D01-PRE)

| ID | Requirement | Verification |
|----|-------------|--------------|
| FR-D01-PRE-001 | outdoor-air-quality Silver table exists and has data | `SELECT COUNT(*) FROM silver.outdoor_air_quality` > 0 |
| FR-D01-PRE-002 | Phase A-C architecture complete and deployed | All Phase C exit criteria met |
| FR-D01-PRE-003 | ndp-gold-ddl tool operational | `ndp-gold-ddl --version` succeeds |
| FR-D01-PRE-004 | 3 streams already in Gold layer | `gold.air_quality_hourly`, `gold.outdoor_weather_hourly`, `gold.state_events_hourly` exist |

### 2.2 Config Creation (FR-D01-CFG)

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| FR-D01-CFG-001 | Add `gold_etl` section to outdoor-air-quality config | P0 | JSON Schema validates |
| FR-D01-CFG-002 | Add `stream_type: observation` to outdoor-air-quality config | P0 | Classification propagates |
| FR-D01-CFG-003 | Update domain config with outdoor-air-quality stream | P0 | Domain includes 4th stream |
| FR-D01-CFG-004 | Create/update manifest with gold-table declaration | P0 | Manifest parses correctly |

### 2.3 Deployment (FR-D01-DEP)

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| FR-D01-DEP-001 | ndp-gold-ddl generates valid SQL for new stream | P0 | SQL executes without error |
| FR-D01-DEP-002 | deploy.sh apply creates continuous aggregate | P0 | `gold.outdoor_air_quality_hourly` exists |
| FR-D01-DEP-003 | Aligned view regenerates with 4th stream | P0 | View includes outdoor_aqi columns |
| FR-D01-DEP-004 | Data dictionary auto-populates | P0 | `data_dictionary.gold_tables` has new entries |

### 2.4 Verification (FR-D01-VER)

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| FR-D01-VER-001 | Continuous aggregate has data | P0 | `SELECT * FROM gold.outdoor_air_quality_hourly LIMIT 1` returns data |
| FR-D01-VER-002 | Aligned view includes new stream | P0 | `outdoor_pm25` column exists and has values |
| FR-D01-VER-003 | Query performance acceptable | P0 | 30-day query < 100ms |
| FR-D01-VER-004 | Refresh policy active | P0 | Policy visible in `timescaledb_information.jobs` |

---

## 3. Non-Functional Requirements

### 3.1 Timing (NFR-D01-TIME)

| ID | Requirement | Target | Measurement |
|----|-------------|--------|-------------|
| NFR-D01-TIME-001 | Documentation review | < 10 min | Stopwatch |
| NFR-D01-TIME-002 | Config file creation | < 15 min | Stopwatch |
| NFR-D01-TIME-003 | Domain config update | < 10 min | Stopwatch |
| NFR-D01-TIME-004 | Manifest creation | < 5 min | Stopwatch |
| NFR-D01-TIME-005 | Deployment execution | < 5 min | Stopwatch |
| NFR-D01-TIME-006 | Verification | < 10 min | Stopwatch |
| NFR-D01-TIME-007 | **Total time** | **< 60 min** | Cumulative |

### 3.2 Code Impact (NFR-D01-CODE)

| ID | Requirement | Target | Measurement |
|----|-------------|--------|-------------|
| NFR-D01-CODE-001 | Rust source code changes | Zero | `git diff --stat *.rs` |
| NFR-D01-CODE-002 | Shell script changes | Zero | `git diff --stat *.sh` |
| NFR-D01-CODE-003 | Python code changes | Zero | `git diff --stat *.py` |
| NFR-D01-CODE-004 | Config-only changes | 2-4 files | `git diff --stat` shows only JSON/YAML |

### 3.3 Performance (NFR-D01-PERF)

| ID | Requirement | Target | Measurement |
|----|-------------|--------|-------------|
| NFR-D01-PERF-001 | Continuous aggregate refresh | < 10 seconds | TimescaleDB job metrics |
| NFR-D01-PERF-002 | Aligned view query (30 days) | < 100ms | `EXPLAIN ANALYZE` |
| NFR-D01-PERF-003 | Memory usage during refresh | < 50MB additional | Pi memory monitoring |

---

## 4. Acceptance Criteria (Gherkin)

### 4.1 Fast-Follower Config Creation

```gherkin
Feature: Fast-Follower Stream Configuration

  Scenario: Add gold_etl section to outdoor-air-quality
    Given the outdoor-air-quality stream config exists at config/base/streams/outdoor-air-quality/config.json
    And the stream has a working silver_etl section
    When I add a gold_etl section with aggregates for pm25, aqi_owm, and aqi_epa
    And I set stream_type to "observation"
    Then the config should pass JSON Schema validation
    And ndp-gold-ddl validate --stream outdoor-air-quality should succeed

  Scenario: Update domain config with 4th stream
    Given the indoor-air-quality domain config exists
    And it includes air-quality, outdoor-weather, and home-assistant-state
    When I add outdoor-air-quality with role "constraint" and alias "outdoor_aqi"
    Then the domain config should pass JSON Schema validation
    And ndp-gold-ddl validate --domain indoor-air-quality should succeed
```

### 4.2 Fast-Follower Deployment

```gherkin
Feature: Fast-Follower Deployment

  Scenario: Deploy new stream to Gold layer
    Given the outdoor-air-quality config has a valid gold_etl section
    And the domain config includes outdoor-air-quality
    And a manifest exists with gold-table and domain declarations
    When I run deploy.sh apply phase-d-test.manifest.json
    Then the deployment should complete without errors
    And gold.outdoor_air_quality_hourly should exist in TimescaleDB
    And the continuous aggregate refresh policy should be active

  Scenario: Aligned view includes new stream
    Given gold.outdoor_air_quality_hourly exists
    And the domain config includes outdoor-air-quality
    When I run deploy.sh apply with domain declaration
    Then gold.indoor_air_quality_aligned should include outdoor_pm25 column
    And gold.indoor_air_quality_aligned should include outdoor_aqi column
    And NULL values should be preserved (observation stream type)
```

### 4.3 Fast-Follower Verification

```gherkin
Feature: Fast-Follower Verification

  Scenario: Continuous aggregate has data
    Given gold.outdoor_air_quality_hourly was just created
    And silver.outdoor_air_quality has at least 24 hours of data
    When the continuous aggregate refreshes
    Then gold.outdoor_air_quality_hourly should have at least 24 rows
    And all configured metrics (mean, std, min, max) should have values

  Scenario: Query performance is acceptable
    Given gold.indoor_air_quality_aligned exists with 4 streams
    When I query 30 days of aligned data
    Then the query should complete in less than 100ms
    And the query plan should use index scans

  Scenario: Zero code changes verified
    Given the fast-follower test is complete
    When I run git diff on all source files
    Then no .rs files should be modified
    And no .sh files should be modified
    And only JSON and YAML config files should be modified
```

---

## 5. Fast-Follower Test Procedure

### 5.1 Pre-Test Checklist

Execute before starting the timed test:

```bash
# 1. Verify Silver table exists and has data
docker exec ndp-timescaledb psql -U postgres -d ndp -c \
  "SELECT COUNT(*) as row_count,
          MIN(observation_time) as earliest,
          MAX(observation_time) as latest
   FROM silver.outdoor_air_quality;"

# 2. Verify 3 streams already in Gold
docker exec ndp-timescaledb psql -U postgres -d ndp -c \
  "SELECT view_name FROM timescaledb_information.continuous_aggregates
   WHERE view_schema = 'gold';"

# 3. Verify ndp-gold-ddl works
ndp-gold-ddl --version

# 4. Verify current config has NO gold_etl section
cat config/base/streams/outdoor-air-quality/config.json | jq '.gold_etl // "NOT_PRESENT"'

# 5. Create clean git state
git stash  # If needed
git status  # Should be clean or only untracked files
```

### 5.2 Timed Test Procedure

**START STOPWATCH NOW**

#### Step 1: Read Documentation (Target: 10 min, Checkpoint: 0:10)

```bash
# Read the gold_etl config example from air-quality
cat config/base/streams/air-quality/config.json | jq '.gold_etl'

# Read the domain config structure
cat config/domains/indoor-air-quality/domain.yaml  # or .json

# Note: outdoor-air-quality fields available in Silver
docker exec ndp-timescaledb psql -U postgres -d ndp -c \
  "SELECT column_name, data_type
   FROM information_schema.columns
   WHERE table_schema = 'silver' AND table_name = 'outdoor_air_quality'
   ORDER BY ordinal_position;"
```

**CHECKPOINT 1**: Record time ___:___ (Target: < 10:00)

#### Step 2: Create gold_etl Config (Target: 15 min, Checkpoint: 0:25)

Add the following `gold_etl` section to `config/base/streams/outdoor-air-quality/config.json`:

```json
{
  "stream_type": "observation",
  "gold_etl": {
    "enabled": true,
    "description": "Gold layer hourly aggregates for outdoor air quality",
    "aggregates": {
      "granularities": ["1 hour"],
      "default_metrics": ["mean", "std", "min", "max", "count"],
      "fields": {
        "pm25": {
          "metrics": ["mean", "std", "min", "max", "p95"],
          "description": "PM2.5 from OpenWeatherMap"
        },
        "pm10": {
          "metrics": ["mean", "std", "min", "max"],
          "description": "PM10 concentration"
        },
        "aqi_owm": {
          "metrics": ["mean", "min", "max"],
          "description": "OpenWeatherMap AQI (1-5 scale)"
        },
        "aqi_epa": {
          "metrics": ["mean", "min", "max"],
          "description": "EPA AQI (derived)"
        },
        "o3_ugm3": {
          "metrics": ["mean", "max"],
          "description": "Ozone concentration"
        },
        "no2_ugm3": {
          "metrics": ["mean", "max"],
          "description": "Nitrogen dioxide"
        }
      }
    },
    "features": {
      "lag": {
        "enabled": true,
        "lags_hours": [1, 6, 24],
        "fields": ["pm25", "aqi_epa"]
      },
      "rolling": {
        "enabled": true,
        "windows": ["4 hours", "24 hours"],
        "stats": ["mean", "std"],
        "fields": ["pm25"]
      }
    }
  }
}
```

Validate the config:

```bash
# Validate JSON syntax
cat config/base/streams/outdoor-air-quality/config.json | jq .

# Validate against schema
ndp-gold-ddl validate --stream outdoor-air-quality
```

**CHECKPOINT 2**: Record time ___:___ (Target: < 25:00)

#### Step 3: Update Domain Config (Target: 10 min, Checkpoint: 0:35)

Update `config/domains/indoor-air-quality/domain.yaml` to include outdoor-air-quality:

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
    # NEW: Add outdoor air quality as constraint
    - stream_id: outdoor-air-quality
      alias: outdoor_aqi
      role: constraint

  alignment:
    view_name: indoor_air_quality_aligned
    granularity: "1 hour"
    join_strategy: full_outer
    null_handling: preserve

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

  constraints:
    - id: outdoor_air_safe
      description: "Don't open window if outdoor air is bad"
      stream: outdoor-air-quality
      metric: pm25
      condition: "<"
      threshold: 35
```

Validate the domain config:

```bash
ndp-gold-ddl validate --domain indoor-air-quality
```

**CHECKPOINT 3**: Record time ___:___ (Target: < 35:00)

#### Step 4: Create Manifest (Target: 5 min, Checkpoint: 0:40)

Create `.deploy/test/phase-d-fast-follower.manifest.json`:

```json
{
  "version": "1.1.0-test",
  "description": "Phase D Fast-Follower Test - Add outdoor-air-quality to Gold",
  "created": "2026-02-XX",
  "declarations": {
    "etcd-config": [
      {
        "stream_id": "outdoor-air-quality",
        "path": "config/base/streams/outdoor-air-quality/config.json"
      }
    ],
    "gold-tables": [
      {
        "stream_id": "outdoor-air-quality",
        "action": "sync"
      }
    ],
    "domains": [
      {
        "domain_id": "indoor-air-quality",
        "action": "recreate"
      }
    ]
  }
}
```

**CHECKPOINT 4**: Record time ___:___ (Target: < 40:00)

#### Step 5: Run Deployment (Target: 5 min, Checkpoint: 0:45)

```bash
# Sync config to etcd first (if not handled by manifest)
./scripts/sync-streams-to-etcd.sh outdoor-air-quality

# Run deployment
./deploy/pi/deploy.sh apply .deploy/test/phase-d-fast-follower.manifest.json

# Watch for errors in output
```

**CHECKPOINT 5**: Record time ___:___ (Target: < 45:00)

#### Step 6: Verification (Target: 10 min, Checkpoint: 0:55)

```bash
# 6.1 Verify continuous aggregate exists
docker exec ndp-timescaledb psql -U postgres -d ndp -c \
  "SELECT view_name FROM timescaledb_information.continuous_aggregates
   WHERE view_schema = 'gold' AND view_name = 'outdoor_air_quality_hourly';"

# 6.2 Verify data exists
docker exec ndp-timescaledb psql -U postgres -d ndp -c \
  "SELECT COUNT(*), MIN(bucket), MAX(bucket)
   FROM gold.outdoor_air_quality_hourly;"

# 6.3 Verify aligned view has new columns
docker exec ndp-timescaledb psql -U postgres -d ndp -c \
  "SELECT column_name FROM information_schema.columns
   WHERE table_schema = 'gold'
     AND table_name = 'indoor_air_quality_aligned'
     AND column_name LIKE 'outdoor_aqi%'
   ORDER BY column_name;"

# 6.4 Verify query performance
docker exec ndp-timescaledb psql -U postgres -d ndp -c \
  "EXPLAIN ANALYZE
   SELECT * FROM gold.indoor_air_quality_aligned
   WHERE bucket >= NOW() - INTERVAL '30 days';"

# 6.5 Verify refresh policy
docker exec ndp-timescaledb psql -U postgres -d ndp -c \
  "SELECT job_id, schedule_interval, config->>'mat_hypertable_id' as hypertable
   FROM timescaledb_information.jobs
   WHERE proc_name = 'policy_refresh_continuous_aggregate';"

# 6.6 Verify data dictionary populated
docker exec ndp-timescaledb psql -U postgres -d ndp -c \
  "SELECT * FROM data_dictionary.gold_tables
   WHERE table_name LIKE '%outdoor_air_quality%';"
```

**CHECKPOINT 6**: Record time ___:___ (Target: < 55:00)

#### Step 7: Code Change Verification (Target: 5 min, Checkpoint: 1:00)

```bash
# Verify ZERO code changes
git diff --stat

# Expected output should show ONLY:
# config/base/streams/outdoor-air-quality/config.json
# config/domains/indoor-air-quality/domain.yaml (or .json)
# .deploy/test/phase-d-fast-follower.manifest.json (new file)

# Verify no Rust changes
git diff --stat -- '*.rs'
# Should show: 0 files changed

# Verify no shell script changes
git diff --stat -- '*.sh'
# Should show: 0 files changed
```

**STOP STOPWATCH**

**FINAL TIME**: ___:___ (Target: < 60:00)

### 5.3 Post-Test Documentation

Record results in `FAST-FOLLOWER-REPORT.md`:

```markdown
# Fast-Follower Test Report

## Test Execution
- Date: 2026-02-XX
- Tester: [name]
- Start Time: HH:MM
- End Time: HH:MM
- Total Duration: XX minutes

## Checkpoint Times
| Checkpoint | Target | Actual | Status |
|------------|--------|--------|--------|
| 1. Documentation | 10:00 | XX:XX | PASS/FAIL |
| 2. gold_etl config | 25:00 | XX:XX | PASS/FAIL |
| 3. Domain config | 35:00 | XX:XX | PASS/FAIL |
| 4. Manifest | 40:00 | XX:XX | PASS/FAIL |
| 5. Deployment | 45:00 | XX:XX | PASS/FAIL |
| 6. Verification | 55:00 | XX:XX | PASS/FAIL |
| 7. Code check | 60:00 | XX:XX | PASS/FAIL |

## Code Changes
- Rust files changed: X (target: 0)
- Shell files changed: X (target: 0)
- Config files changed: X (target: 2-4)

## Verification Results
- Continuous aggregate exists: YES/NO
- Data present: YES/NO (X rows)
- Aligned view updated: YES/NO
- Query performance: XXms (target: <100ms)
- Data dictionary: COMPLETE/INCOMPLETE

## Issues Encountered
[Document any issues, workarounds, or gaps discovered]

## Conclusion
**TEST STATUS**: PASS / FAIL

[If FAIL, document what code changes were required and why]
```

---

## 6. Expected Gold Layer Outputs

### 6.1 Continuous Aggregate: gold.outdoor_air_quality_hourly

```sql
-- Expected columns
bucket                  TIMESTAMPTZ  -- Hour bucket
ndp_id                  TEXT         -- Entity identifier
pm25_mean               DOUBLE PRECISION
pm25_std                DOUBLE PRECISION
pm25_min                DOUBLE PRECISION
pm25_max                DOUBLE PRECISION
pm25_p95                DOUBLE PRECISION
pm10_mean               DOUBLE PRECISION
pm10_std                DOUBLE PRECISION
pm10_min                DOUBLE PRECISION
pm10_max                DOUBLE PRECISION
aqi_owm_mean            DOUBLE PRECISION
aqi_owm_min             SMALLINT
aqi_owm_max             SMALLINT
aqi_epa_mean            DOUBLE PRECISION
aqi_epa_min             SMALLINT
aqi_epa_max             SMALLINT
o3_ugm3_mean            DOUBLE PRECISION
o3_ugm3_max             DOUBLE PRECISION
no2_ugm3_mean           DOUBLE PRECISION
no2_ugm3_max            DOUBLE PRECISION
sample_count            BIGINT       -- Number of samples in bucket
```

### 6.2 Aligned View Additions

The `gold.indoor_air_quality_aligned` view should gain these columns:

```sql
-- New columns from outdoor-air-quality (outdoor_aqi alias)
outdoor_aqi_pm25        DOUBLE PRECISION  -- outdoor_aqi.pm25_mean
outdoor_aqi_pm10        DOUBLE PRECISION  -- outdoor_aqi.pm10_mean
outdoor_aqi_owm         DOUBLE PRECISION  -- outdoor_aqi.aqi_owm_mean
outdoor_aqi_epa         DOUBLE PRECISION  -- outdoor_aqi.aqi_epa_mean
outdoor_aqi_o3          DOUBLE PRECISION  -- outdoor_aqi.o3_ugm3_mean
outdoor_aqi_no2         DOUBLE PRECISION  -- outdoor_aqi.no2_ugm3_mean
```

### 6.3 Feature Columns (if enabled)

```sql
-- Lag features
outdoor_aqi_pm25_lag_1h   DOUBLE PRECISION
outdoor_aqi_pm25_lag_6h   DOUBLE PRECISION
outdoor_aqi_pm25_lag_24h  DOUBLE PRECISION
outdoor_aqi_aqi_epa_lag_1h  DOUBLE PRECISION
outdoor_aqi_aqi_epa_lag_6h  DOUBLE PRECISION
outdoor_aqi_aqi_epa_lag_24h DOUBLE PRECISION

-- Rolling features
outdoor_aqi_pm25_roll_4h_mean   DOUBLE PRECISION
outdoor_aqi_pm25_roll_4h_std    DOUBLE PRECISION
outdoor_aqi_pm25_roll_24h_mean  DOUBLE PRECISION
outdoor_aqi_pm25_roll_24h_std   DOUBLE PRECISION
```

---

## 7. London TDD Interfaces

### 7.1 Interface: GoldEtlConfig Loading

```rust
/// This interface MUST work for outdoor-air-quality without code changes
#[async_trait]
pub trait GoldConfigLoader {
    /// Load gold_etl config for any stream
    async fn load_gold_etl(&self, stream_id: &str) -> Result<GoldEtlConfig, ConfigError>;
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_loads_outdoor_air_quality_gold_etl() {
        // GIVEN outdoor-air-quality config with gold_etl section
        let loader = EtcdConfigLoader::new(etcd_client);

        // WHEN loading gold_etl
        let config = loader.load_gold_etl("outdoor-air-quality").await.unwrap();

        // THEN config is valid
        assert!(config.enabled);
        assert!(config.aggregates.fields.contains_key("pm25"));
        assert!(config.aggregates.fields.contains_key("aqi_epa"));
    }
}
```

### 7.2 Interface: DDL Generation

```rust
/// This interface MUST generate valid SQL for outdoor-air-quality without code changes
pub trait ContinuousAggregateGenerator {
    fn generate(
        &self,
        config: &GoldEtlConfig,
        stream_id: &str,
        silver_table: &str
    ) -> Result<String, GeneratorError>;
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_generates_outdoor_air_quality_aggregate() {
        // GIVEN outdoor-air-quality gold_etl config
        let config = load_test_config("outdoor-air-quality");
        let generator = ContinuousAggregateGenerator::new();

        // WHEN generating SQL
        let sql = generator.generate(&config, "outdoor-air-quality", "silver.outdoor_air_quality").unwrap();

        // THEN SQL is valid and includes expected columns
        assert!(sql.contains("CREATE MATERIALIZED VIEW"));
        assert!(sql.contains("gold.outdoor_air_quality_hourly"));
        assert!(sql.contains("AVG(pm25) AS pm25_mean"));
        assert!(sql.contains("AVG(aqi_epa) AS aqi_epa_mean"));
    }
}
```

### 7.3 Interface: Aligned View Generation

```rust
/// This interface MUST include outdoor-air-quality in aligned view without code changes
pub trait AlignedViewGenerator {
    fn generate(&self, domain: &DomainConfig) -> Result<String, GeneratorError>;
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_aligned_view_includes_fourth_stream() {
        // GIVEN domain config with 4 streams
        let domain = load_test_domain("indoor-air-quality");
        assert_eq!(domain.streams.len(), 4);

        let generator = AlignedViewGenerator::new();

        // WHEN generating aligned view
        let sql = generator.generate(&domain).unwrap();

        // THEN SQL includes outdoor_aqi columns
        assert!(sql.contains("outdoor_aqi.pm25_mean AS outdoor_aqi_pm25"));
        assert!(sql.contains("FULL OUTER JOIN gold.outdoor_air_quality_hourly"));
    }
}
```

---

## 8. Troubleshooting Guide

### 8.1 Common Issues

| Issue | Likely Cause | Resolution |
|-------|--------------|------------|
| Schema validation fails | Missing required field in gold_etl | Check gold-etl.schema.json for requirements |
| DDL generation fails | Invalid field name reference | Verify field names match Silver columns |
| Deployment fails | TimescaleDB permission issue | Check postgres user permissions |
| No data in aggregate | Refresh not run yet | Wait for refresh or run `CALL refresh_continuous_aggregate()` |
| Missing aligned view columns | Domain config not updated | Verify domain includes new stream |

### 8.2 Debug Commands

```bash
# Check DDL that would be generated
ndp-gold-ddl generate --stream outdoor-air-quality --dry-run

# Check aligned view DDL
ndp-gold-ddl generate --domain indoor-air-quality --dry-run

# Force refresh continuous aggregate
docker exec ndp-timescaledb psql -U postgres -d ndp -c \
  "CALL refresh_continuous_aggregate('gold.outdoor_air_quality_hourly', NULL, NULL);"

# Check for errors in TimescaleDB logs
docker logs ndp-timescaledb --tail 100 | grep -i error
```

---

## 9. References

- [SCOPE.md](../../SCOPE.md) - V1.1 scope definition
- [DECISIONS.md](../../architecture/DECISIONS.md) - Architecture decisions
- [outdoor-air-quality config](../../../../config/base/streams/outdoor-air-quality/config.json) - Target stream
- [air-quality config](../../../../config/base/streams/air-quality/config.json) - Reference implementation

---

*Specification created: 2026-02-04*
*This is the critical validation specification for V1.1 architecture.*
