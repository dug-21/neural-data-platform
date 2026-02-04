# SPEC-A04: Alignment Interpreter

> **Feature ID:** v11-A04
> **Priority:** Critical
> **Status:** Specification
> **Dependencies:** v11-A03 (Alignment Schema), v11-A02 (Gold DDL Tool)
> **Blocks:** Phase C (Cross-Stream Aligned View)

---

## User Story

**As a** platform operator,
**I want** an interpreter that generates aligned view SQL from domain configuration,
**So that** I can create cross-stream correlation views declaratively.

---

## Goal

Create an alignment interpreter module within `ndp-gold-ddl` that:
1. Reads domain configuration
2. Generates TimescaleDB view SQL joining multiple streams
3. Handles NULL strategies per stream type
4. Supports forecast stream alignment on `issued_at`
5. Produces queryable aligned views for V1.2 pattern detection

---

## Functional Requirements

### FR-A04-001: Aligned View Generation

The interpreter SHALL generate a materialized view joining all domain streams:

```sql
CREATE MATERIALIZED VIEW gold.{view_name} AS
SELECT
    COALESCE({bucket_expressions}) AS bucket,
    {column_expressions}
FROM gold.{stream1}_hourly s1
{join_clauses}
WHERE COALESCE({bucket_expressions}) >= NOW() - INTERVAL '90 days';
```

### FR-A04-002: Column Expression Generation

For each stream in the domain, generate aliased column expressions:

| Source Column | Generated Column |
|---------------|------------------|
| `s1.pm25_mean` | `{alias}_pm25_mean` |
| `s1.co2_std` | `{alias}_co2_std` |

Pattern: `{alias}_{original_column}` (underscore-separated)

### FR-A04-003: Join Clause Generation

Based on `alignment.join_strategy`:

**full_outer (default)**:
```sql
FROM gold.{stream1}_hourly s1
FULL OUTER JOIN gold.{stream2}_hourly s2 ON s1.bucket = s2.bucket
FULL OUTER JOIN gold.{stream3}_hourly s3 ON COALESCE(s1.bucket, s2.bucket) = s3.bucket
```

**left**:
```sql
FROM gold.{primary_stream}_hourly s1
LEFT JOIN gold.{stream2}_hourly s2 ON s1.bucket = s2.bucket
LEFT JOIN gold.{stream3}_hourly s3 ON s1.bucket = s3.bucket
```

**inner**:
```sql
FROM gold.{stream1}_hourly s1
INNER JOIN gold.{stream2}_hourly s2 ON s1.bucket = s2.bucket
INNER JOIN gold.{stream3}_hourly s3 ON s1.bucket = s3.bucket
```

### FR-A04-004: Bucket Coalescing

For FULL OUTER JOIN, generate COALESCE for the bucket column:
```sql
COALESCE(s1.bucket, s2.bucket, s3.bucket) AS bucket
```

### FR-A04-005: NULL Handling by Stream Type

Generate NULL handling expressions based on stream type and configuration:

**preserve** (default for observation, forecast):
```sql
s1.pm25_mean AS indoor_pm25_mean  -- No transformation
```

**carry_forward** (default for state_event):
```sql
COALESCE(
    s3.window_state,
    LAG(s3.window_state) IGNORE NULLS OVER (ORDER BY bucket)
) AS state_window_state
```

**interpolate** (explicit only):
```sql
-- Linear interpolation between non-NULL values
CASE
    WHEN s1.temp_mean IS NOT NULL THEN s1.temp_mean
    ELSE (
        LAG(s1.temp_mean) IGNORE NULLS OVER (ORDER BY bucket) +
        LEAD(s1.temp_mean) IGNORE NULLS OVER (ORDER BY bucket)
    ) / 2
END AS indoor_temp_mean
```

### FR-A04-006: Forecast Stream Alignment

For streams with `stream_type: forecast`, use `issued_at` instead of `observation_time`:

```sql
LEFT JOIN LATERAL (
    SELECT * FROM gold.nws_forecast_hourly f
    WHERE f.issued_at <= COALESCE(s1.bucket, s2.bucket)
    ORDER BY f.issued_at DESC
    LIMIT 1
) forecast ON TRUE
```

This ensures forecasts are joined based on "information available at the time", not "prediction target time".

### FR-A04-007: Primary Stream Ordering

The stream with `role: primary` SHALL always be the first (leftmost) in JOINs. This ensures:
- LEFT JOIN preserves all primary stream rows
- Column ordering is predictable

### FR-A04-008: CLI Integration

The alignment interpreter SHALL be invoked via:
```bash
ndp-gold-ddl generate --domain <domain_id> [--action sync|recreate]
```

Output includes:
1. Aligned view DDL
2. Index on bucket column
3. Optional: Refresh materialized view command

### FR-A04-009: Column Selection

The interpreter SHALL select columns from Gold hourly aggregates:
- All aggregate columns (e.g., `pm25_mean`, `co2_std`)
- Feature columns if present (e.g., `pm25_lag_1h`)
- Transition columns for state_event streams

### FR-A04-010: Sample Count Column

Include a sample count column for data quality awareness:
```sql
COALESCE(s1.sample_count, 0) + COALESCE(s2.sample_count, 0) + ... AS total_samples
```

---

## Non-Functional Requirements

### NFR-A04-001: Query Performance

Generated aligned view queries SHALL:
- Execute in < 100ms for 30-day range
- Use bucket index effectively
- Avoid correlated subqueries where possible

### NFR-A04-002: View Materialization

The aligned view SHALL be a regular materialized view (not continuous aggregate):
- Continuous aggregates cannot reference other continuous aggregates
- Refresh command generated for manual/scheduled refresh:
  ```sql
  REFRESH MATERIALIZED VIEW gold.{view_name};
  ```

### NFR-A04-003: Error Handling

Generation SHALL fail with clear errors when:
- Referenced stream has no Gold layer configured
- Stream type unknown (cannot determine NULL handling)
- Circular dependencies detected

---

## Acceptance Criteria

### AC-A04-001: Basic Aligned View Generation

```gherkin
Scenario: Generate aligned view for indoor-air-quality domain
  Given a domain config with streams: air-quality, outdoor-weather, home-assistant-state
  And alignment.view_name = "indoor_air_quality_aligned"
  And alignment.granularity = "1 hour"
  When I run: ndp-gold-ddl generate --domain indoor-air-quality
  Then the output SHALL contain CREATE MATERIALIZED VIEW gold.indoor_air_quality_aligned
  And the output SHALL contain FULL OUTER JOIN
  And the output SHALL contain COALESCE for bucket
```

### AC-A04-002: Column Aliasing

```gherkin
Scenario: Columns are aliased by stream alias
  Given stream air-quality has alias "indoor"
  And stream outdoor-weather has alias "outdoor"
  When I generate the aligned view
  Then the output SHALL contain "indoor_pm25_mean"
  And the output SHALL contain "outdoor_temp_mean"
  And the output SHALL NOT contain "air_quality_pm25_mean"
```

### AC-A04-003: NULL Handling for State Events

```gherkin
Scenario: State event streams use carry-forward NULL handling
  Given home-assistant-state has stream_type = "state_event"
  And null_handling is not explicitly set
  When I generate the aligned view
  Then the window_state column SHALL use COALESCE with LAG
  And the output SHALL contain "LAG(s3.window_state) IGNORE NULLS OVER"
```

### AC-A04-004: Forecast Stream LATERAL Join

```gherkin
Scenario: Forecast streams join on issued_at
  Given nws-forecast-hourly has stream_type = "forecast"
  And the stream is included in the domain
  When I generate the aligned view
  Then the output SHALL contain LEFT JOIN LATERAL
  And the output SHALL contain "WHERE f.issued_at <="
  And the output SHALL contain "ORDER BY f.issued_at DESC"
```

### AC-A04-005: Left Join Strategy

```gherkin
Scenario: Left join preserves all primary stream rows
  Given alignment.join_strategy = "left"
  And air-quality has role = "primary"
  When I generate the aligned view
  Then the output SHALL contain "FROM gold.air_quality_hourly"
  And the output SHALL contain "LEFT JOIN gold.outdoor_weather_hourly"
  And air-quality SHALL be the first table in FROM
```

### AC-A04-006: Missing Gold Layer Error

```gherkin
Scenario: Stream without Gold layer fails generation
  Given outdoor-weather does not have gold_etl.enabled = true
  When I run: ndp-gold-ddl generate --domain indoor-air-quality
  Then the tool SHALL exit with code 1
  And stderr SHALL contain "Stream 'outdoor-weather' has no Gold layer configured"
  And stdout SHALL be empty
```

### AC-A04-007: Index Generation

```gherkin
Scenario: Index is created on bucket column
  When I generate the aligned view
  Then the output SHALL contain CREATE INDEX
  And the index SHALL be on the bucket column
```

---

## Generated SQL Example

For domain `indoor-air-quality` with streams:
- `air-quality` (alias: `indoor`, role: `primary`, type: `observation`)
- `outdoor-weather` (alias: `outdoor`, role: `context`, type: `observation`)
- `home-assistant-state` (alias: `state`, role: `actuator`, type: `state_event`)

```sql
-- Aligned view for domain: indoor-air-quality
-- Generated by ndp-gold-ddl

CREATE MATERIALIZED VIEW gold.indoor_air_quality_aligned AS
SELECT
    -- Bucket column (coalesced from all streams)
    COALESCE(indoor.bucket, outdoor.bucket, state.bucket) AS bucket,

    -- Indoor Air Quality (observation - primary)
    indoor.pm25_mean AS indoor_pm25_mean,
    indoor.pm25_std AS indoor_pm25_std,
    indoor.pm25_max AS indoor_pm25_max,
    indoor.co2_mean AS indoor_co2_mean,
    indoor.co2_std AS indoor_co2_std,
    indoor.temp_mean AS indoor_temp_mean,
    indoor.humidity_mean AS indoor_humidity_mean,
    indoor.sample_count AS indoor_samples,

    -- Outdoor Weather (observation - context)
    outdoor.temp_mean AS outdoor_temp_mean,
    outdoor.humidity_mean AS outdoor_humidity_mean,
    outdoor.wind_speed_mean AS outdoor_wind_speed_mean,
    outdoor.pressure_mean AS outdoor_pressure_mean,
    outdoor.sample_count AS outdoor_samples,

    -- Home Assistant State (state_event - actuator)
    -- NULL handling: carry_forward
    COALESCE(
        state.window_state,
        LAG(state.window_state) IGNORE NULLS OVER (ORDER BY COALESCE(indoor.bucket, outdoor.bucket, state.bucket))
    ) AS state_window_state,
    state.transition_count AS state_transitions,

    -- Total sample count
    COALESCE(indoor.sample_count, 0) + COALESCE(outdoor.sample_count, 0) AS total_samples

FROM gold.air_quality_hourly indoor
FULL OUTER JOIN gold.outdoor_weather_hourly outdoor
    ON indoor.bucket = outdoor.bucket
FULL OUTER JOIN gold.home_assistant_state_hourly state
    ON COALESCE(indoor.bucket, outdoor.bucket) = state.bucket

WHERE COALESCE(indoor.bucket, outdoor.bucket, state.bucket) >= NOW() - INTERVAL '90 days';

-- Index for efficient bucket queries
CREATE INDEX IF NOT EXISTS idx_indoor_air_quality_aligned_bucket
    ON gold.indoor_air_quality_aligned (bucket);

-- Refresh command (run manually or via scheduler)
-- REFRESH MATERIALIZED VIEW gold.indoor_air_quality_aligned;
```

---

## Module Structure

Within `tools/ndp-gold-ddl/`:

```
src/generators/
├── aligned_view.rs         # Main alignment interpreter
├── join_builder.rs         # Join clause generation
├── column_builder.rs       # Column expression generation
└── null_handler.rs         # NULL handling strategies
```

### aligned_view.rs Interface

```rust
pub struct AlignedViewGenerator {
    config_loader: Box<dyn ConfigLoader>,
    stream_resolver: Box<dyn StreamResolver>,
}

impl AlignedViewGenerator {
    pub fn generate(&self, domain_id: &str, action: Action) -> Result<String, GeneratorError>;
}

pub struct AlignedViewConfig {
    pub view_name: String,
    pub granularity: String,
    pub join_strategy: JoinStrategy,
    pub null_handling: NullHandling,
    pub streams: Vec<AlignedStream>,
}

pub struct AlignedStream {
    pub stream_id: String,
    pub alias: String,
    pub role: StreamRole,
    pub stream_type: StreamType,
    pub null_handling: Option<NullHandling>,
    pub gold_table: String,  // e.g., "gold.air_quality_hourly"
    pub columns: Vec<String>, // Available columns from Gold layer
}
```

---

## Integration Test Requirements

### Test: End-to-End Aligned View

```rust
#[test]
fn test_generate_indoor_air_quality_aligned() {
    let generator = AlignedViewGenerator::new(
        MockConfigLoader::with_domain("indoor-air-quality"),
        MockStreamResolver::with_streams(vec!["air-quality", "outdoor-weather", "home-assistant-state"]),
    );

    let sql = generator.generate("indoor-air-quality", Action::Sync).unwrap();

    assert!(sql.contains("CREATE MATERIALIZED VIEW gold.indoor_air_quality_aligned"));
    assert!(sql.contains("FULL OUTER JOIN"));
    assert!(sql.contains("indoor_pm25_mean"));
    assert!(sql.contains("outdoor_temp_mean"));
    assert!(sql.contains("LAG(state.window_state) IGNORE NULLS"));
}
```

### Test: SQL Execution

```bash
# Generate and execute
ndp-gold-ddl generate --domain indoor-air-quality | psql -U postgres -d ndp

# Verify view exists
psql -c "SELECT count(*) FROM pg_matviews WHERE matviewname = 'indoor_air_quality_aligned'"
# Expected: 1

# Verify columns
psql -c "SELECT column_name FROM information_schema.columns WHERE table_name = 'indoor_air_quality_aligned' AND column_name LIKE 'indoor_%' LIMIT 5"
```

---

## London TDD Interfaces

### Trait: JoinBuilder

```rust
pub trait JoinBuilder {
    fn build_joins(&self, streams: &[AlignedStream], strategy: JoinStrategy) -> String;
}

// Implementations:
// - FullOuterJoinBuilder
// - LeftJoinBuilder
// - InnerJoinBuilder
```

### Trait: NullHandler

```rust
pub trait NullHandler {
    fn wrap_column(&self, column: &str, alias: &str, table_alias: &str) -> String;
}

// Implementations:
// - PreserveNullHandler (passthrough)
// - CarryForwardNullHandler (LAG IGNORE NULLS)
// - InterpolateNullHandler (linear interpolation)
```

### Trait: ColumnBuilder

```rust
pub trait ColumnBuilder {
    fn build_select_columns(&self, stream: &AlignedStream, null_handler: &dyn NullHandler) -> Vec<String>;
}
```

---

## deploy.sh Integration

### New Function: handle_domain()

```bash
handle_domain() {
    local declaration="$1"
    local domain_id=$(echo "$declaration" | jq -r '.domain_id')
    local action=$(echo "$declaration" | jq -r '.action // "sync"')

    log "Domain: $domain_id (action=$action)"

    # Sync domain config to etcd
    local config_file="$CONFIG_DIR/domains/$domain_id/domain.yaml"
    if [ -f "$config_file" ]; then
        log "  Syncing domain config to etcd..."
        cat "$config_file" | dcx etcd etcdctl put "/domains/$domain_id/config"
    fi

    # Generate aligned view DDL using Rust tool
    local ddl=$(ndp-gold-ddl generate --domain "$domain_id" --action "$action" 2>&1)
    local exit_code=$?

    if [ $exit_code -ne 0 ]; then
        error "Domain DDL generation failed: $ddl"
        return 1
    fi

    # Apply DDL to TimescaleDB
    log "  Applying Domain DDL to TimescaleDB..."
    echo "$ddl" | dcx timescaledb psql -U postgres -d ndp

    return $?
}
```

---

## References

- [SPEC-A03](./SPEC-A03-alignment-schema.md) - Alignment schema definition
- [DECISIONS.md](../../architecture/DECISIONS.md) - Decision 8: Forecast Alignment on issued_at
- [DECISIONS.md](../../architecture/DECISIONS.md) - Decision 10: NULL Handling by Stream Type
- [SCOPE.md](../../SCOPE.md) - v11-005: Cross-Stream Aligned View
