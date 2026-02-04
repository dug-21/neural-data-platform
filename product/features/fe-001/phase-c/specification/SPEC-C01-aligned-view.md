# SPEC-C01: Cross-Stream Aligned View (v11-005)

> **Feature ID:** v11-005
> **Feature Name:** Cross-Stream Aligned View
> **Phase:** C (Cross-Stream + Alignment)
> **Priority:** Critical
> **Created:** 2026-02-04

---

## User Story

**As a** data scientist preparing data for pattern detection,
**I want** a single view that joins all domain streams on hourly buckets,
**So that** I can query cross-stream correlations without manual JOIN logic.

---

## Goal

Create `gold.indoor_air_quality_aligned` - a materialized view that:
1. Joins air-quality, outdoor-weather, and home-assistant-state streams
2. Uses hourly time buckets for correlation granularity
3. Applies NULL handling rules per stream type (ADR-FE001-004)
4. Is generated from domain configuration (no hardcoded SQL)

---

## Functional Requirements

### FR-C01-001: View Generation from Domain Config

**Description:** The `ndp-gold-ddl` tool generates aligned view SQL from domain.yaml configuration.

**Acceptance Criteria:**
- `ndp-gold-ddl generate --domain indoor-air-quality` produces valid SQL
- Generated SQL creates `gold.indoor_air_quality_aligned` view
- View name matches `domain.alignment.view_name` from config
- Changes to domain config regenerate the view (via `action: recreate`)

---

### FR-C01-002: FULL OUTER JOIN Strategy

**Description:** All streams are joined using FULL OUTER JOIN to preserve rows even when some streams have no data.

**Acceptance Criteria:**
- View includes rows where any stream has data
- Rows where one stream is missing show NULL for that stream's columns
- No rows are dropped due to missing stream data
- Join key is the hourly bucket timestamp

**SQL Pattern:**
```sql
FROM gold.air_quality_hourly aq
FULL OUTER JOIN gold.outdoor_weather_hourly ow
    ON aq.bucket = ow.bucket
FULL OUTER JOIN gold.state_events_hourly se
    ON COALESCE(aq.bucket, ow.bucket) = se.bucket
```

---

### FR-C01-003: Bucket Time Alignment

**Description:** All streams align on 1-hour time buckets as specified in domain config.

**Acceptance Criteria:**
- `bucket` column is the primary time key
- All timestamps are truncated to hour boundary
- Granularity matches `domain.alignment.granularity` (1 hour)
- Bucket uses `COALESCE` across all stream buckets for FULL OUTER JOIN

**SQL Pattern:**
```sql
SELECT
    COALESCE(aq.bucket, ow.bucket, se.bucket) AS bucket,
    ...
```

---

### FR-C01-004: NULL Handling by Stream Type

**Description:** NULL values are handled according to ADR-FE001-004 based on stream_type.

**Acceptance Criteria:**
- `observation` streams (air-quality, outdoor-weather): NULL preserved
- `state_event` streams (home-assistant-state): NULL filled via LOCF (Last Observation Carried Forward)
- `forecast` streams: NULL preserved (not in Phase C, but pattern established)
- `dimension` streams: NULL filled via LOCF (not in Phase C)

**SQL Pattern for LOCF:**
```sql
-- State columns use carry-forward
COALESCE(
    se.window_state,
    LAG(se.window_state) IGNORE NULLS OVER (ORDER BY bucket)
) AS window_state
```

**SQL Pattern for Observation (preserve NULL):**
```sql
-- Observation columns preserve NULL
aq.pm25_mean AS indoor_pm25
```

---

### FR-C01-005: Column Aliasing by Stream Role

**Description:** Columns are aliased based on stream alias and role from domain config.

**Acceptance Criteria:**
- Column names follow pattern: `{alias}_{metric}` or semantic names
- air-quality (alias: indoor): `indoor_pm25`, `indoor_co2`, etc.
- outdoor-weather (alias: outdoor): `outdoor_temp`, `outdoor_humidity`, etc.
- home-assistant-state (alias: state): `window_state`, `state_changes`, etc.
- No column name collisions

---

### FR-C01-006: Refresh Policy

**Description:** The aligned view refreshes automatically to incorporate new data.

**Acceptance Criteria:**
- View is a materialized view (not a regular view) for performance
- Refresh policy runs every 15 minutes (matches source aggregates)
- Lookback period of 4 hours catches late-arriving data
- Refresh does not block queries

**Note:** If TimescaleDB continuous aggregate limitations prevent this, use a scheduled refresh approach documented in the implementation.

---

## Non-Functional Requirements

### NFR-C01-001: Query Performance

**Description:** Aligned view queries must meet performance targets.

**Acceptance Criteria:**
- Query for 30-day range completes in < 100ms
- Query for 7-day range completes in < 50ms
- `EXPLAIN ANALYZE` shows index usage

**Measurement:** `pg_stat_statements` latency metrics

---

### NFR-C01-002: Memory Efficiency

**Description:** Aligned view queries stay within Pi 5 memory budget.

**Acceptance Criteria:**
- Peak memory during query < 50 MB
- No out-of-memory errors for 30-day queries
- Works within 2GB RAM constraint

---

### NFR-C01-003: Data Completeness

**Description:** Aligned view includes all available hourly buckets.

**Acceptance Criteria:**
- Coverage metric: % of buckets with data from each stream
- No data loss during alignment
- Gaps in source data appear as NULL (observation) or LOCF (state)

---

## Domain Configuration Example

**File:** `config/domains/indoor-air-quality/domain.yaml`

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
    null_handling: by_stream_type

  # Objectives defined but used in SPEC-C03
  objectives: [...]
```

---

## Generated SQL Example

The `ndp-gold-ddl generate --domain indoor-air-quality` command generates:

```sql
-- Generated by ndp-gold-ddl for domain: indoor-air-quality
-- Do not edit manually - regenerate from config

CREATE MATERIALIZED VIEW gold.indoor_air_quality_aligned AS
SELECT
    -- Time bucket (COALESCE for FULL OUTER JOIN)
    COALESCE(aq.bucket, ow.bucket, se.bucket) AS bucket,

    -- Indoor Air Quality (observation - preserve NULL)
    aq.pm25_mean AS indoor_pm25,
    aq.pm25_std AS indoor_pm25_std,
    aq.co2_mean AS indoor_co2,
    aq.co2_std AS indoor_co2_std,
    aq.temperature_c_mean AS indoor_temp,
    aq.humidity_pct_mean AS indoor_humidity,
    aq.sample_count AS indoor_samples,

    -- Outdoor Weather (observation - preserve NULL)
    ow.temperature_c_mean AS outdoor_temp,
    ow.humidity_pct_mean AS outdoor_humidity,
    ow.wind_speed_kmh_mean AS outdoor_wind_speed,
    ow.pressure_pa_mean AS outdoor_pressure,
    ow.cloud_cover_pct_mean AS outdoor_clouds,
    ow.sample_count AS outdoor_samples,

    -- State Events (state_event - LOCF for NULL)
    COALESCE(
        se.window_open_count,
        LAG(se.window_open_count) IGNORE NULLS OVER w
    ) AS window_opens,
    COALESCE(
        se.door_open_count,
        LAG(se.door_open_count) IGNORE NULLS OVER w
    ) AS door_opens,
    COALESCE(
        se.state_changes_count,
        LAG(se.state_changes_count) IGNORE NULLS OVER w
    ) AS state_changes,
    COALESCE(
        se.last_window_state,
        LAG(se.last_window_state) IGNORE NULLS OVER w
    ) AS last_window_state

FROM gold.air_quality_hourly aq
FULL OUTER JOIN gold.outdoor_weather_hourly ow
    ON aq.bucket = ow.bucket
FULL OUTER JOIN gold.state_events_hourly se
    ON COALESCE(aq.bucket, ow.bucket) = se.bucket

WINDOW w AS (ORDER BY COALESCE(aq.bucket, ow.bucket, se.bucket))

ORDER BY bucket;

-- Create index for time-based queries
CREATE INDEX IF NOT EXISTS idx_indoor_air_quality_aligned_bucket
    ON gold.indoor_air_quality_aligned (bucket DESC);

-- Refresh policy (if using scheduled refresh)
-- Note: Materialized views require manual or scheduled refresh
-- Schedule via pg_cron or deploy.sh cron job
COMMENT ON MATERIALIZED VIEW gold.indoor_air_quality_aligned IS
    'Aligned hourly view for indoor-air-quality domain. Refresh every 15 min.';
```

---

## Acceptance Criteria (Given/When/Then)

### Scenario: Query Aligned View for Recent Data

```gherkin
Given the aligned view exists with data from all 3 streams
When I query for the last 24 hours
Then I should see hourly buckets with columns from all streams
And observation columns should show NULL where data is missing
And state columns should show carried-forward values where data is missing
```

### Scenario: FULL OUTER JOIN Preserves All Data

```gherkin
Given air-quality has data for hours 1-10
And outdoor-weather has data for hours 5-15
And home-assistant-state has data for hours 8-12
When I query the aligned view
Then I should see buckets for hours 1-15 (union of all)
And hours 1-4 should have NULL for outdoor and state columns
And hours 13-15 should have NULL for indoor columns
```

### Scenario: LOCF Applies to State Columns

```gherkin
Given home-assistant-state has window_state='open' at hour 5
And no state events between hours 6-10
When I query the aligned view for hours 5-10
Then window_state should be 'open' for all hours 5-10
And this is because LOCF carries forward the last known state
```

### Scenario: View Regeneration on Config Change

```gherkin
Given the aligned view exists
When I modify domain.yaml to add a new stream reference
And I run ndp-gold-ddl generate --domain indoor-air-quality
Then the generated SQL should include the new stream
And deploy.sh apply with action: recreate should update the view
```

---

## London TDD Interfaces

### IAlignedViewGenerator (tools/ndp-gold-ddl)

```rust
/// Generates SQL for domain-aligned views
pub trait IAlignedViewGenerator {
    /// Generate CREATE MATERIALIZED VIEW statement
    fn generate_view_ddl(&self, domain: &DomainConfig) -> Result<String, GeneratorError>;

    /// Generate index DDL for the aligned view
    fn generate_index_ddl(&self, domain: &DomainConfig) -> Result<String, GeneratorError>;

    /// Generate full DDL (view + indexes + comments)
    fn generate_full_ddl(&self, domain: &DomainConfig) -> Result<String, GeneratorError>;
}
```

### IJoinStrategyResolver

```rust
/// Resolves JOIN strategy for each stream
pub trait IJoinStrategyResolver {
    /// Get JOIN clause for a stream based on its type and role
    fn resolve_join(&self, stream: &StreamRef, base_alias: &str) -> JoinClause;
}

pub enum JoinClause {
    FullOuter { on_condition: String },
    LeftJoin { on_condition: String },
    Lateral { subquery: String },  // For forecast streams
}
```

### INullHandlingResolver

```rust
/// Resolves NULL handling strategy per stream type
pub trait INullHandlingResolver {
    /// Get column expression with appropriate NULL handling
    fn resolve_column(&self, column: &str, stream_type: StreamType, alias: &str) -> String;
}
```

---

## Dependencies

| Dependency | Type | Required By |
|------------|------|-------------|
| v11-A04: Alignment Interpreter | Phase A | This feature |
| v11-003: air-quality hourly aggregate | Phase B | Source data |
| v11-003: outdoor-weather hourly aggregate | Phase C | Source data |
| v11-003: state-events hourly aggregate | Phase C | Source data |
| ADR-FE001-004: NULL Handling | Architecture | NULL strategy |

---

## Test Cases

### Unit Tests (tools/ndp-gold-ddl/tests/aligned_view_test.rs)

| Test | Description | Expected |
|------|-------------|----------|
| `test_generate_view_ddl_basic` | Generate DDL for 3-stream domain | Valid SQL, correct view name |
| `test_full_outer_join_clause` | JOIN generation | FULL OUTER JOIN for all streams |
| `test_bucket_coalesce` | Bucket time handling | COALESCE across all streams |
| `test_null_handling_observation` | Observation columns | Direct column reference (preserve NULL) |
| `test_null_handling_state_event` | State columns | LOCF expression generated |
| `test_column_aliasing` | Column naming | Matches `{alias}_{metric}` pattern |
| `test_index_generation` | Index DDL | Creates bucket index |

### Integration Tests

| Test | Description | SQL |
|------|-------------|-----|
| `test_aligned_view_query_30d` | Query performance | `EXPLAIN ANALYZE SELECT * FROM gold.indoor_air_quality_aligned WHERE bucket >= NOW() - INTERVAL '30 days'` |
| `test_full_outer_coverage` | Data preservation | Count rows vs union of source rows |
| `test_locf_correctness` | LOCF verification | Check state columns fill correctly |

---

## Manifest Declaration

**File:** `.deploy/releases/vX.Y.Z.manifest.json`

```json
{
  "declarations": {
    "domains": [
      {
        "domain_id": "indoor-air-quality",
        "action": "sync"
      }
    ]
  }
}
```

For config changes, use `"action": "recreate"`:

```json
{
  "declarations": {
    "domains": [
      {
        "domain_id": "indoor-air-quality",
        "action": "recreate"
      }
    ]
  }
}
```

---

## deploy.sh Integration

```bash
handle_domain() {
    local declaration="$1"
    local domain_id=$(echo "$declaration" | jq -r '.domain_id')
    local action=$(echo "$declaration" | jq -r '.action // "sync"')

    log "Domain: $domain_id (action=$action)"

    case "$action" in
        sync)
            # Check if view exists, create if not
            local ddl=$(ndp-gold-ddl generate --domain "$domain_id" --mode sync 2>&1)
            echo "$ddl" | dcx timescaledb psql -U postgres -d ndp
            ;;
        recreate)
            # Drop and recreate
            local ddl=$(ndp-gold-ddl generate --domain "$domain_id" --mode recreate 2>&1)
            echo "$ddl" | dcx timescaledb psql -U postgres -d ndp
            ;;
        validate-only)
            ndp-gold-ddl validate --domain "$domain_id"
            ;;
    esac

    # Store objectives in data dictionary
    sync_objectives_to_data_dictionary "$domain_id"
}
```

---

## Data Dictionary Integration

When the aligned view is created, metadata is stored:

```sql
-- gold_tables entry
INSERT INTO data_dictionary.gold_tables (
    table_name, object_type, bucket_interval, description
) VALUES (
    'indoor_air_quality_aligned',
    'materialized_view',
    '1 hour',
    'Aligned hourly view for indoor-air-quality domain'
);

-- gold_columns entries (one per column)
INSERT INTO data_dictionary.gold_columns (
    table_name, column_name, data_type, feature_type, source_expression
) VALUES
    ('indoor_air_quality_aligned', 'bucket', 'timestamptz', 'raw', 'COALESCE(aq.bucket, ow.bucket, se.bucket)'),
    ('indoor_air_quality_aligned', 'indoor_pm25', 'double precision', 'aggregate', 'aq.pm25_mean'),
    ('indoor_air_quality_aligned', 'window_state', 'text', 'locf', 'COALESCE(se.window_state, LAG(...))');
```

---

## References

- [ADR-FE001-002: Domain-Centric Configuration](/workspaces/neural-data-platform/product/features/fe-001/architecture/ADR-FE001-002-domain-centric-config.md)
- [ADR-FE001-004: NULL Handling by Stream Type](/workspaces/neural-data-platform/product/features/fe-001/architecture/ADR-FE001-004-null-handling.md)
- [DECISIONS.md - Decision 7](/workspaces/neural-data-platform/product/features/fe-001/architecture/DECISIONS.md) - One aligned view per domain
- [PHASE-C-OVERVIEW.md](./PHASE-C-OVERVIEW.md) - Phase C overview
