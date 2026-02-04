# SPEC-B04: Aggregate Refresh Policy

> **Feature ID:** v11-004
> **Priority:** High
> **Status:** Specification
> **Dependencies:** v11-003 (Per-Stream Continuous Aggregates)
> **Blocks:** Phase B Completion, Phase C (Multi-Stream with Aligned Refresh)

---

## User Story

**As a** platform operator,
**I want** continuous aggregates to automatically refresh on a configurable schedule,
**So that** the Gold layer stays current without manual intervention and queries return recent data.

---

## Goal

Configure TimescaleDB continuous aggregate refresh policies for air-quality aggregates:
1. Auto-refresh every 15 minutes (configurable)
2. 4-hour lookback window for late-arriving data
3. 15-minute end offset to avoid incomplete buckets
4. Resource usage within Pi 5 constraints

---

## Background: TimescaleDB Refresh Policies

### Policy Parameters

| Parameter | Description | Air-Quality Setting |
|-----------|-------------|---------------------|
| `schedule_interval` | How often refresh runs | 15 minutes |
| `start_offset` | How far back to refresh (handles late data) | 4 hours |
| `end_offset` | How close to "now" to refresh (avoids incomplete buckets) | 15 minutes |

### Refresh Window

```
Timeline:
├────────────────────────────────────────────────────────────►
                  start_offset              end_offset
                  (4 hours)                 (15 min)
                      │                         │
                      ▼                         ▼
    ┌─────────────────────────────────────────────┐
    │         REFRESH WINDOW                       │
    │    (buckets that will be recomputed)        │
    └─────────────────────────────────────────────┘
    │                                             │
 NOW - 4h                                     NOW - 15m
```

### Why These Settings?

1. **4-hour start_offset**: Handles late-arriving MQTT data, network delays, and buffer flushes
2. **15-minute end_offset**: Ensures hourly buckets are complete before aggregating
3. **15-minute schedule_interval**: Balance between freshness and resource usage

---

## Functional Requirements

### FR-B04-001: Policy Generation

The `ndp-gold-ddl` tool SHALL generate refresh policy SQL:

```sql
SELECT add_continuous_aggregate_policy('gold.air_quality_hourly',
    start_offset => INTERVAL '4 hours',
    end_offset => INTERVAL '15 minutes',
    schedule_interval => INTERVAL '15 minutes'
);
```

### FR-B04-002: Config-Driven Policy

Refresh policy settings SHALL be configurable in `gold_etl`:

```yaml
gold_etl:
  refresh_policy:
    schedule_interval: "15 minutes"
    start_offset: "4 hours"
    end_offset: "15 minutes"
```

### FR-B04-003: Default Policy Values

If `refresh_policy` is not specified, use defaults:
- `schedule_interval`: 15 minutes
- `start_offset`: 4 hours (for hourly aggregates)
- `end_offset`: Matches the bucket size (1 hour for hourly)

### FR-B04-004: Idempotent Policy Creation

Policy creation SHALL be idempotent:
- Check if policy exists before adding
- Skip if policy already exists
- Option to update policy if settings changed

### FR-B04-005: Policy Per Granularity

Each granularity SHALL have its own policy:
- `gold.air_quality_hourly` - 15 min schedule, 4 hour lookback
- `gold.air_quality_daily` - 1 hour schedule, 24 hour lookback

### FR-B04-006: Policy Removal on Recreate

When `action: recreate`, existing policies SHALL be removed (CASCADE handles this).

---

## Non-Functional Requirements

### NFR-B04-001: Resource Constraints

Refresh operations on Pi 5 SHALL:
- Use < 100 MB peak memory
- Use < 10% CPU sustained during refresh
- Complete within 30 seconds for typical data volume

### NFR-B04-002: Data Freshness

After refresh, data SHALL be no older than:
- `schedule_interval + end_offset` (30 minutes for default settings)

### NFR-B04-003: No Data Loss

Late-arriving data within `start_offset` SHALL be included in aggregates after next refresh.

### NFR-B04-004: Monitoring

Policy execution SHALL be visible in:
- TimescaleDB job statistics
- PostgreSQL logs (at DEBUG level)

---

## Acceptance Criteria

### AC-B04-001: Policy Created

```gherkin
Scenario: Refresh policy is created for hourly aggregate
  Given gold.air_quality_hourly continuous aggregate exists
  When deploy.sh apply is executed with air-quality Gold table
  Then a continuous aggregate policy SHALL exist for gold.air_quality_hourly
  And the policy SHALL have schedule_interval = '15 minutes'
```

### AC-B04-002: Policy Uses Config Values

```gherkin
Scenario: Policy uses configured values
  Given gold_etl.refresh_policy.schedule_interval = "30 minutes"
  When ndp-gold-ddl generates policy SQL
  Then schedule_interval SHALL be '30 minutes'
```

### AC-B04-003: Policy Runs Automatically

```gherkin
Scenario: Policy refreshes aggregate automatically
  Given a refresh policy is configured
  And new data arrives in silver.air_quality_observations
  When 15 minutes pass (schedule_interval)
  Then gold.air_quality_hourly SHALL contain the new data
  (after the end_offset window)
```

### AC-B04-004: Idempotent Policy Creation

```gherkin
Scenario: Running deploy twice does not duplicate policy
  Given a refresh policy already exists for gold.air_quality_hourly
  When deploy.sh apply is executed again with action = "sync"
  Then only one policy SHALL exist
  And deployment SHALL not fail
```

### AC-B04-005: Daily Aggregate Has Appropriate Policy

```gherkin
Scenario: Daily aggregate has daily-appropriate policy
  Given gold.air_quality_daily continuous aggregate exists
  When deploy.sh apply is executed
  Then a policy SHALL exist for gold.air_quality_daily
  And schedule_interval SHALL be >= '1 hour'
  And start_offset SHALL be >= '24 hours'
```

### AC-B04-006: Resource Usage Within Limits

```gherkin
Scenario: Refresh does not exceed resource limits
  Given gold.air_quality_hourly has 30 days of data
  When a refresh is triggered
  Then peak memory usage SHALL be < 100 MB
  And CPU usage SHALL be < 10% sustained
```

---

## Generated SQL

### Policy for Hourly Aggregate

```sql
-- Generated by ndp-gold-ddl for stream: air-quality
-- Policy for: gold.air_quality_hourly

-- Check if policy already exists
DO $$
DECLARE
    policy_exists BOOLEAN;
BEGIN
    SELECT EXISTS (
        SELECT 1 FROM timescaledb_information.continuous_aggregate_stats
        WHERE view_name = 'gold.air_quality_hourly'
    ) INTO policy_exists;

    IF NOT policy_exists THEN
        -- Add refresh policy
        PERFORM add_continuous_aggregate_policy('gold.air_quality_hourly',
            start_offset => INTERVAL '4 hours',
            end_offset => INTERVAL '15 minutes',
            schedule_interval => INTERVAL '15 minutes'
        );
        RAISE NOTICE 'Added refresh policy for gold.air_quality_hourly';
    ELSE
        RAISE NOTICE 'Refresh policy already exists for gold.air_quality_hourly';
    END IF;
END $$;
```

### Policy for Daily Aggregate

```sql
-- Policy for: gold.air_quality_daily
DO $$
DECLARE
    policy_exists BOOLEAN;
BEGIN
    SELECT EXISTS (
        SELECT 1 FROM timescaledb_information.continuous_aggregate_stats
        WHERE view_name = 'gold.air_quality_daily'
    ) INTO policy_exists;

    IF NOT policy_exists THEN
        PERFORM add_continuous_aggregate_policy('gold.air_quality_daily',
            start_offset => INTERVAL '7 days',      -- Longer lookback for daily
            end_offset => INTERVAL '1 hour',        -- Match bucket size
            schedule_interval => INTERVAL '1 hour'  -- Hourly refresh sufficient
        );
        RAISE NOTICE 'Added refresh policy for gold.air_quality_daily';
    ELSE
        RAISE NOTICE 'Refresh policy already exists for gold.air_quality_daily';
    END IF;
END $$;
```

---

## Air-Quality Config Extension

### refresh_policy Section

```yaml
# config/base/streams/air-quality/config.yaml

gold_etl:
  enabled: true
  description: "Hourly and daily aggregates for air quality metrics"

  aggregates:
    granularities: ["1 hour", "1 day"]
    # ... fields config ...

  # Refresh policy configuration (v11-004)
  refresh_policy:
    # How often to run refresh
    schedule_interval: "15 minutes"
    # How far back to refresh (handles late data)
    start_offset: "4 hours"
    # How close to "now" to refresh (avoids incomplete buckets)
    end_offset: "15 minutes"

  # Optional: Override for daily aggregate
  refresh_policy_daily:
    schedule_interval: "1 hour"
    start_offset: "7 days"
    end_offset: "1 hour"
```

---

## Integration Test Requirements

### Test: Policy Exists

```bash
# Deploy air-quality Gold layer with policy
deploy.sh apply .deploy/test/phase-b-air-quality.manifest.json

# Verify policy exists
dcx timescaledb psql -U postgres -d ndp -c "
SELECT view_name, schedule_interval, refresh_lag
FROM timescaledb_information.continuous_aggregate_stats
WHERE view_name = 'gold.air_quality_hourly';
"
# Expected: schedule_interval = 00:15:00
```

### Test: Policy Configuration

```bash
# Check policy details
dcx timescaledb psql -U postgres -d ndp -c "
SELECT * FROM timescaledb_information.jobs
WHERE proc_name = 'policy_refresh_continuous_aggregate'
  AND config::text LIKE '%air_quality_hourly%';
"
```

### Test: Automatic Refresh

```bash
# Insert new data into Silver
dcx timescaledb psql -U postgres -d ndp -c "
INSERT INTO silver.air_quality_observations
    (observation_time, ndp_id, pm25, co2)
VALUES
    (NOW() - INTERVAL '30 minutes', 'test-sensor', 25.5, 750);
"

# Wait for refresh (or trigger manually for testing)
dcx timescaledb psql -U postgres -d ndp -c "
CALL refresh_continuous_aggregate('gold.air_quality_hourly',
    NOW() - INTERVAL '1 hour',
    NOW()
);
"

# Verify data appears in aggregate
dcx timescaledb psql -U postgres -d ndp -c "
SELECT bucket, pm25_mean, sample_count
FROM gold.air_quality_hourly
WHERE bucket >= NOW() - INTERVAL '2 hours'
ORDER BY bucket DESC;
"
```

### Test: Resource Usage

```bash
# Monitor during refresh
dcx timescaledb psql -U postgres -d ndp -c "
SELECT pid, usename, state, query, now() - query_start AS duration
FROM pg_stat_activity
WHERE query LIKE '%air_quality%';
"

# Check memory usage during refresh (on Pi)
free -h
```

### Test: Idempotency

```bash
# Run deploy twice
deploy.sh apply .deploy/test/phase-b-air-quality.manifest.json
deploy.sh apply .deploy/test/phase-b-air-quality.manifest.json

# Verify only one policy per view
dcx timescaledb psql -U postgres -d ndp -c "
SELECT view_name, COUNT(*)
FROM timescaledb_information.continuous_aggregate_stats
GROUP BY view_name;
"
# Expected: count = 1 for each view
```

---

## London TDD Interfaces

### Trait: RefreshPolicyGenerator

```rust
/// Generates DDL for continuous aggregate refresh policies
trait RefreshPolicyGenerator {
    /// Generate policy SQL for an aggregate
    fn generate_policy(
        &self,
        view_name: &str,
        config: &RefreshPolicyConfig,
    ) -> Result<String, GeneratorError>;

    /// Generate idempotent policy wrapper
    fn wrap_idempotent(&self, sql: &str, view_name: &str) -> String;

    /// Get default policy for a granularity
    fn get_default_policy(&self, granularity: &str) -> RefreshPolicyConfig;
}
```

### Struct: RefreshPolicyConfig

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshPolicyConfig {
    /// How often to run refresh (e.g., "15 minutes")
    pub schedule_interval: String,
    /// How far back to refresh (e.g., "4 hours")
    pub start_offset: String,
    /// How close to now to refresh (e.g., "15 minutes")
    pub end_offset: String,
}

impl Default for RefreshPolicyConfig {
    fn default() -> Self {
        Self {
            schedule_interval: "15 minutes".into(),
            start_offset: "4 hours".into(),
            end_offset: "15 minutes".into(),
        }
    }
}

impl RefreshPolicyConfig {
    /// Get default policy for hourly aggregate
    pub fn default_hourly() -> Self {
        Self::default()
    }

    /// Get default policy for daily aggregate
    pub fn default_daily() -> Self {
        Self {
            schedule_interval: "1 hour".into(),
            start_offset: "7 days".into(),
            end_offset: "1 hour".into(),
        }
    }

    /// Parse interval string to PostgreSQL INTERVAL
    pub fn to_pg_interval(interval: &str) -> Result<String, ParseError> {
        // Validate format: "N unit" where unit is minutes, hours, days
        let re = Regex::new(r"^(\d+)\s+(minute|hour|day)s?$")?;
        if re.is_match(interval) {
            Ok(format!("INTERVAL '{}'", interval))
        } else {
            Err(ParseError::InvalidInterval(interval.into()))
        }
    }
}
```

### Mock: RefreshPolicyGenerator

```rust
mock! {
    pub RefreshPolicyGenerator {}

    impl RefreshPolicyGenerator for RefreshPolicyGenerator {
        fn generate_policy(
            &self,
            view_name: &str,
            config: &RefreshPolicyConfig,
        ) -> Result<String, GeneratorError>;

        fn wrap_idempotent(&self, sql: &str, view_name: &str) -> String;
        fn get_default_policy(&self, granularity: &str) -> RefreshPolicyConfig;
    }
}
```

---

## Monitoring Queries

### Check Policy Status

```sql
-- View all continuous aggregate policies
SELECT
    view_schema || '.' || view_name AS aggregate,
    schedule_interval,
    config
FROM timescaledb_information.jobs j
JOIN timescaledb_information.continuous_aggregates ca
    ON j.hypertable_name = ca.materialization_hypertable_name
WHERE proc_name = 'policy_refresh_continuous_aggregate';
```

### Check Last Refresh

```sql
-- View refresh statistics
SELECT
    view_name,
    completed_threshold AS last_refresh,
    refresh_lag,
    total_runs,
    total_successes,
    total_failures
FROM timescaledb_information.continuous_aggregate_stats
WHERE view_name LIKE 'gold.air_quality%';
```

### Check Job History

```sql
-- View recent job runs
SELECT
    job_id,
    pid,
    start_time,
    finish_time,
    status,
    total_duration
FROM timescaledb_information.job_stats
WHERE job_id IN (
    SELECT job_id FROM timescaledb_information.jobs
    WHERE config::text LIKE '%air_quality%'
)
ORDER BY start_time DESC
LIMIT 10;
```

---

## Error Handling

### Error Codes

| Code | Name | Description | Recovery |
|------|------|-------------|----------|
| 420 | POLICY_EXISTS | Policy already exists for view | Skip (idempotent) |
| 421 | INVALID_INTERVAL | Interval format not recognized | Check format |
| 422 | VIEW_NOT_FOUND | Cannot add policy to non-existent view | Create view first |
| 423 | POLICY_CREATION_FAILED | Failed to add policy | Check TimescaleDB logs |

---

## Pi 5 Resource Guidelines

### Memory Budget

| Operation | Peak Memory | Duration |
|-----------|-------------|----------|
| Hourly refresh (1 day) | ~20 MB | ~5 sec |
| Hourly refresh (30 days) | ~50 MB | ~15 sec |
| Daily refresh (30 days) | ~30 MB | ~10 sec |

### CPU Budget

| Operation | CPU % | Duration |
|-----------|-------|----------|
| Hourly refresh | ~5-8% | ~5-15 sec |
| Daily refresh | ~3-5% | ~5-10 sec |

### Tuning Recommendations

If resource usage exceeds budget:
1. Increase `schedule_interval` (less frequent refresh)
2. Decrease `start_offset` (smaller refresh window)
3. Add `WITH NO DATA` option and refresh during low-usage periods

---

## References

- [SCOPE.md](../../SCOPE.md) - v11-004 Aggregate Refresh Policy
- [DECISIONS.md](../../architecture/DECISIONS.md) - Decision 5: SQL Generation Pattern
- [SPEC-B03](./SPEC-B03-continuous-aggregates.md) - Per-Stream Continuous Aggregates
- [TimescaleDB Refresh Policies](https://docs.timescale.com/api/latest/continuous-aggregates/add_continuous_aggregate_policy/)

---

*Specification created: 2026-02-04*
