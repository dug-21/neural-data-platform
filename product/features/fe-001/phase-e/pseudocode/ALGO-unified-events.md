# ALGO-unified-events: UNION ALL View Generation

> **Algorithm ID:** E02
> **Feature:** v11-013 (Unified Events View)
> **Phase:** E (Unified Event Abstraction)
> **Created:** 2026-02-04

---

## Purpose

Generate a unified events view that combines state transitions (Phase C) and threshold crossings (Phase E) into a single queryable interface for V1.2 Pattern Detection. This algorithm produces a UNION ALL view with consistent schema, optional hourly aggregates, and aligned view integration for holistic analysis.

---

## Algorithm: GenerateUnifiedEventsView

```
ALGORITHM: GenerateUnifiedEventsView
INPUT:
    domain_config: DomainConfig
    config_loader: ConfigLoader
    action: Action
OUTPUT: Result<String, GeneratorError>
REQUIRES:
    - domain_config.includes_events = true
    - At least one event source (transitions OR crossings) exists

BEGIN
    domain_id <- domain_config.id

    // 1. Identify available event sources
    event_sources <- IdentifyEventSources(domain_config, config_loader)

    IF event_sources.is_empty() THEN
        RETURN Err(GeneratorError::Code409_NoEventSources {
            domain_id: domain_id,
            message: "Domain has no state_event streams or objectives"
        })
    END IF

    // 2. Build UNION ALL components
    union_parts <- Vec::new()

    // 2a. Add state transition selects
    FOR EACH transition_source IN event_sources.state_transitions DO
        part <- GenerateStateTransitionSelect(transition_source, domain_id)
        union_parts.push(part)
    END FOR

    // 2b. Add threshold crossing select
    IF event_sources.has_threshold_crossings THEN
        part <- GenerateThresholdCrossingSelect(domain_id)
        union_parts.push(part)
    END IF

    // 3. Generate view name
    view_name <- format!(
        "gold.{}_events_unified",
        domain_id.replace("-", "_")
    )

    // 4. Build final SQL
    sql <- GenerateUnifiedViewSQL(
        view_name,
        union_parts,
        domain_id,
        action
    )

    RETURN Ok(sql)
END
```

---

## Algorithm: IdentifyEventSources

```
ALGORITHM: IdentifyEventSources
INPUT:
    domain_config: DomainConfig
    config_loader: ConfigLoader
OUTPUT: EventSources

BEGIN
    sources <- EventSources {
        state_transitions: Vec::new(),
        has_threshold_crossings: false
    }

    // Find state_event streams in domain
    FOR EACH stream_id IN domain_config.streams DO
        stream_config <- config_loader.load_stream(stream_id)?

        IF stream_config.stream_type == "state_event" THEN
            IF stream_config.gold_etl.transitions.enabled THEN
                transition_table <- format!(
                    "gold.{}_transitions",
                    stream_id.replace("-", "_")
                )
                sources.state_transitions.push(StateTransitionSource {
                    stream_id: stream_id,
                    table_name: transition_table,
                    state_field: stream_config.gold_etl.transitions.state_field
                })
            END IF
        END IF
    END FOR

    // Check for objectives (implies threshold crossings)
    IF domain_config.objectives.len() > 0 THEN
        // Verify threshold crossing view exists
        crossing_view <- format!(
            "gold.{}_threshold_crossings",
            domain_config.id.replace("-", "_")
        )
        sources.has_threshold_crossings <- true
        sources.threshold_crossing_view <- crossing_view
    END IF

    RETURN sources
END
```

---

## Algorithm: GenerateStateTransitionSelect

```
ALGORITHM: GenerateStateTransitionSelect
INPUT:
    source: StateTransitionSource
    domain_id: String
OUTPUT: String

BEGIN
    // State transitions need to be transformed to unified schema
    RETURN format!(r#"
-- State transitions from {stream_id}
SELECT
    gen_random_uuid() AS event_id,
    transition_time AS event_time,
    stream_id,
    entity_id,
    'state_transition'::TEXT AS event_type,
    jsonb_build_object(
        'from_state', from_state,
        'to_state', to_state,
        'duration_in_previous_ms',
            EXTRACT(EPOCH FROM duration_in_previous_state) * 1000,
        'state_field', '{state_field}'
    ) AS details
FROM {table_name}
WHERE is_actual_transition = TRUE"#,
        stream_id = source.stream_id,
        state_field = source.state_field,
        table_name = source.table_name
    )
END
```

---

## Algorithm: GenerateThresholdCrossingSelect

```
ALGORITHM: GenerateThresholdCrossingSelect
INPUT: domain_id: String
OUTPUT: String

BEGIN
    crossing_view <- format!(
        "gold.{}_threshold_crossings",
        domain_id.replace("-", "_")
    )

    // Threshold crossings already have the unified schema
    RETURN format!(r#"
-- Threshold crossings from domain objectives
SELECT
    event_id,
    event_time,
    stream_id,
    entity_id,
    event_type,
    details
FROM {crossing_view}"#,
        crossing_view = crossing_view
    )
END
```

---

## Algorithm: GenerateUnifiedViewSQL

```
ALGORITHM: GenerateUnifiedViewSQL
INPUT:
    view_name: String
    union_parts: Vec<String>
    domain_id: String
    action: Action
OUTPUT: String

BEGIN
    // Build UNION ALL
    union_sql <- union_parts.join("\n\nUNION ALL\n")

    // Generate full SQL
    sql <- format!(r#"
-- Unified Events View for domain: {domain_id}
-- Generated by ndp-gold-ddl
-- Combines state transitions and threshold crossings for V1.2

{drop_statement}

CREATE OR REPLACE VIEW {view_name} AS

{union_sql}

ORDER BY event_time, event_type, event_id;

-- Comment for documentation
COMMENT ON VIEW {view_name} IS
    'Unified events from {domain_id}: state transitions and threshold crossings. For V1.2 Pattern Detection.';
"#,
        domain_id = domain_id,
        drop_statement = IF action == Recreate THEN
            format!("DROP VIEW IF EXISTS {} CASCADE;", view_name)
        ELSE
            ""
        END IF,
        view_name = view_name,
        union_sql = union_sql
    )

    RETURN sql
END
```

---

## Algorithm: GenerateHourlyEventsAggregate

```
ALGORITHM: GenerateHourlyEventsAggregate
INPUT:
    domain_id: String
    action: Action
OUTPUT: Result<String, GeneratorError>
REQUIRES:
    - Unified events view exists for domain

BEGIN
    unified_view <- format!(
        "gold.{}_events_unified",
        domain_id.replace("-", "_")
    )

    aggregate_name <- format!(
        "gold.{}_events_hourly",
        domain_id.replace("-", "_")
    )

    sql <- format!(r#"
-- Hourly Events Aggregate for domain: {domain_id}
-- Generated by ndp-gold-ddl
-- Provides bucketed counts for aligned view integration

{drop_statement}

CREATE MATERIALIZED VIEW {aggregate_name}
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', event_time) AS bucket,

    -- Total event count
    COUNT(*) AS total_events,

    -- Per-type counts
    COUNT(*) FILTER (WHERE event_type = 'state_transition') AS state_transition_count,
    COUNT(*) FILTER (WHERE event_type = 'threshold_crossing') AS threshold_crossing_count,

    -- Distinct entities with events
    COUNT(DISTINCT entity_id) AS distinct_entities_with_events,

    -- First and last event times (for gap analysis)
    MIN(event_time) AS first_event_time,
    MAX(event_time) AS last_event_time

FROM {unified_view}
GROUP BY bucket;

-- Refresh policy: every 15 minutes, 4-hour lookback
SELECT add_continuous_aggregate_policy('{aggregate_name}',
    start_offset => INTERVAL '4 hours',
    end_offset => INTERVAL '15 minutes',
    schedule_interval => INTERVAL '15 minutes'
);

-- Index for time range queries
CREATE INDEX IF NOT EXISTS idx_{short_name}_bucket
    ON {aggregate_name} (bucket DESC);

COMMENT ON MATERIALIZED VIEW {aggregate_name} IS
    'Hourly event counts for {domain_id}. Auto-refreshes every 15 minutes.';
"#,
        domain_id = domain_id,
        drop_statement = IF action == Recreate THEN
            format!("DROP MATERIALIZED VIEW IF EXISTS {} CASCADE;", aggregate_name)
        ELSE
            ""
        END IF,
        aggregate_name = aggregate_name,
        unified_view = unified_view,
        short_name = aggregate_name.split(".").last().replace("_", "")
    )

    RETURN Ok(sql)
END
```

---

## Algorithm: GenerateAlignedViewExtension

```
ALGORITHM: GenerateAlignedViewExtension
INPUT:
    domain_id: String
    existing_aligned_sql: String
OUTPUT: String

DESCRIPTION:
    Modifies the aligned view SQL to include event counts from the hourly aggregate.
    Called during aligned view generation in Phase C.

BEGIN
    events_hourly <- format!(
        "gold.{}_events_hourly",
        domain_id.replace("-", "_")
    )

    // Add JOIN to events hourly
    join_clause <- format!(r#"
LEFT JOIN {events_hourly} eh ON COALESCE(a.bucket, b.bucket, c.bucket) = eh.bucket"#,
        events_hourly = events_hourly
    )

    // Add event columns to SELECT
    event_columns <- r#"
    -- Event counts from hourly aggregate
    COALESCE(eh.total_events, 0) AS total_events,
    COALESCE(eh.state_transition_count, 0) AS state_transitions,
    COALESCE(eh.threshold_crossing_count, 0) AS threshold_crossings"#

    // Inject into existing SQL
    extended_sql <- InjectAlignedExtension(
        existing_aligned_sql,
        join_clause,
        event_columns
    )

    RETURN extended_sql
END
```

---

## Algorithm: GenerateGlobalUnifiedView

```
ALGORITHM: GenerateGlobalUnifiedView
INPUT:
    domains: Vec<DomainConfig>
    action: Action
OUTPUT: Result<String, GeneratorError>

DESCRIPTION:
    Optional global view across all domains.
    Enables cross-domain event analysis.

BEGIN
    // Build UNION ALL of domain views
    domain_selects <- Vec::new()

    FOR EACH domain IN domains DO
        domain_view <- format!(
            "gold.{}_events_unified",
            domain.id.replace("-", "_")
        )
        domain_selects.push(format!(
            "SELECT * FROM {domain_view}"
        ))
    END FOR

    IF domain_selects.is_empty() THEN
        RETURN Err(GeneratorError::NoDomains {
            message: "No domains configured for global view"
        })
    END IF

    sql <- format!(r#"
-- Global Unified Events View
-- Generated by ndp-gold-ddl
-- Combines events from all domains

{drop_statement}

CREATE OR REPLACE VIEW gold.events_unified AS
{domain_union};

COMMENT ON VIEW gold.events_unified IS
    'Global unified events across all domains. Use domain-scoped views for better performance.';
"#,
        drop_statement = IF action == Recreate THEN
            "DROP VIEW IF EXISTS gold.events_unified CASCADE;"
        ELSE
            ""
        END IF,
        domain_union = domain_selects.join("\nUNION ALL\n")
    )

    RETURN Ok(sql)
END
```

---

## Data Types

```
STRUCT EventSources:
    state_transitions: Vec<StateTransitionSource>
    has_threshold_crossings: bool
    threshold_crossing_view: Option<String>

STRUCT StateTransitionSource:
    stream_id: String
    table_name: String
    state_field: String

STRUCT UnifiedEvent:
    event_id: UUID
    event_time: DateTime
    stream_id: String
    entity_id: String
    event_type: EventType
    details: JSONB

ENUM EventType:
    StateTransition
    ThresholdCrossing
    // Future types
    Anomaly
    TrendChange

STRUCT HourlyEventCounts:
    bucket: DateTime
    total_events: i64
    state_transition_count: i64
    threshold_crossing_count: i64
    distinct_entities_with_events: i64
    first_event_time: DateTime
    last_event_time: DateTime
```

---

## SQL Example: Indoor Air Quality Unified Events

```sql
-- Generated unified events view
CREATE OR REPLACE VIEW gold.indoor_air_quality_events_unified AS

-- State transitions from home-assistant-state
SELECT
    gen_random_uuid() AS event_id,
    transition_time AS event_time,
    stream_id,
    entity_id,
    'state_transition'::TEXT AS event_type,
    jsonb_build_object(
        'from_state', from_state,
        'to_state', to_state,
        'duration_in_previous_ms',
            EXTRACT(EPOCH FROM duration_in_previous_state) * 1000,
        'state_field', 'state'
    ) AS details
FROM gold.home_assistant_state_transitions
WHERE is_actual_transition = TRUE

UNION ALL

-- Threshold crossings from domain objectives
SELECT
    event_id,
    event_time,
    stream_id,
    entity_id,
    event_type,
    details
FROM gold.indoor_air_quality_threshold_crossings

ORDER BY event_time, event_type, event_id;

COMMENT ON VIEW gold.indoor_air_quality_events_unified IS
    'Unified events from indoor-air-quality: state transitions and threshold crossings. For V1.2 Pattern Detection.';
```

---

## SQL Example: Hourly Events Aggregate

```sql
-- Hourly events aggregate for aligned view integration
CREATE MATERIALIZED VIEW gold.indoor_air_quality_events_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', event_time) AS bucket,
    COUNT(*) AS total_events,
    COUNT(*) FILTER (WHERE event_type = 'state_transition') AS state_transition_count,
    COUNT(*) FILTER (WHERE event_type = 'threshold_crossing') AS threshold_crossing_count,
    COUNT(DISTINCT entity_id) AS distinct_entities_with_events,
    MIN(event_time) AS first_event_time,
    MAX(event_time) AS last_event_time
FROM gold.indoor_air_quality_events_unified
GROUP BY bucket;

SELECT add_continuous_aggregate_policy('gold.indoor_air_quality_events_hourly',
    start_offset => INTERVAL '4 hours',
    end_offset => INTERVAL '15 minutes',
    schedule_interval => INTERVAL '15 minutes'
);

CREATE INDEX IF NOT EXISTS idx_indoorairqualityeventshourly_bucket
    ON gold.indoor_air_quality_events_hourly (bucket DESC);
```

---

## SQL Example: Aligned View with Event Counts

```sql
-- Extended aligned view including event counts
CREATE OR REPLACE VIEW gold.indoor_air_quality_aligned AS
SELECT
    COALESCE(aq.bucket, ow.bucket, se.bucket) AS bucket,

    -- Air quality observations
    aq.pm25_mean,
    aq.pm10_mean,
    aq.co2_mean,

    -- Outdoor weather
    ow.temperature_mean,
    ow.humidity_mean,

    -- State events (carry-forward)
    COALESCE(se.window_state,
        LAG(se.window_state) IGNORE NULLS OVER (ORDER BY COALESCE(aq.bucket, ow.bucket, se.bucket))
    ) AS window_state,

    -- Event counts from hourly aggregate
    COALESCE(eh.total_events, 0) AS total_events,
    COALESCE(eh.state_transition_count, 0) AS state_transitions,
    COALESCE(eh.threshold_crossing_count, 0) AS threshold_crossings

FROM gold.air_quality_hourly aq
FULL OUTER JOIN gold.outdoor_weather_hourly ow ON aq.bucket = ow.bucket
FULL OUTER JOIN gold.home_assistant_state_hourly se ON aq.bucket = se.bucket
LEFT JOIN gold.indoor_air_quality_events_hourly eh
    ON COALESCE(aq.bucket, ow.bucket, se.bucket) = eh.bucket

ORDER BY bucket;
```

---

## Complexity Analysis

| Operation | Time Complexity | Space Complexity |
|-----------|-----------------|------------------|
| State transition select | O(n) | O(n) |
| Threshold crossing select | O(m) | O(m) |
| UNION ALL | O(n + m) | O(n + m) |
| ORDER BY | O((n+m) log(n+m)) | O(1) |
| Hourly aggregate | O((n+m) log(n+m)) | O(h) |
| Total view query | O((n+m) log(n+m)) | O(n + m) |

Where: n = state transitions, m = threshold crossings, h = hours

---

## Error Handling

```
ENUM GeneratorError:
    Code409_NoEventSources {
        domain_id: String,
        message: String
    }
    Code410_TransitionViewMissing {
        stream_id: String
    }
    Code411_CrossingViewMissing {
        domain_id: String
    }
    Code412_AlignedViewNotFound {
        domain_id: String
    }
    NoDomains {
        message: String
    }
```

---

## Invariants

1. **Unified Schema**: All events have identical column structure
2. **Event ID Uniqueness**: gen_random_uuid() generates unique IDs
3. **Ordered Output**: Events ordered by time, type, id for determinism
4. **Actual Transitions Only**: is_actual_transition = TRUE filter applied
5. **UNION ALL Preserves Duplicates**: No deduplication (by design for V1.1)
6. **LEFT JOIN for Counts**: Missing hours get COALESCE to 0, not NULL

---

## Test Cases (London TDD)

```
TRAITS TO MOCK:
    - ConfigLoader: Return test domain/stream configs
    - DatabaseExecutor: Capture generated SQL

TEST: UnifiedViewIncludesStateTransitions
    GIVEN domain with state_event stream "home-assistant-state"
    AND stream has transitions.enabled = true
    WHEN GenerateUnifiedEventsView() is called
    THEN SQL contains "FROM gold.home_assistant_state_transitions"
    AND SQL contains "WHERE is_actual_transition = TRUE"

TEST: UnifiedViewIncludesThresholdCrossings
    GIVEN domain with objectives defined
    WHEN GenerateUnifiedEventsView() is called
    THEN SQL contains "FROM gold.*_threshold_crossings"

TEST: UnifiedViewHasCorrectSchema
    GIVEN valid domain configuration
    WHEN GenerateUnifiedEventsView() is called
    THEN SQL SELECT includes: event_id, event_time, stream_id, entity_id, event_type, details

TEST: HourlyAggregateHasCorrectBuckets
    GIVEN unified events view exists
    WHEN GenerateHourlyEventsAggregate() is called
    THEN SQL contains "time_bucket('1 hour', event_time)"
    AND SQL contains "total_events"
    AND SQL contains "state_transition_count"
    AND SQL contains "threshold_crossing_count"

TEST: AlignedViewIncludesEventCounts
    GIVEN domain with events_hourly aggregate
    WHEN GenerateAlignedViewExtension() is called
    THEN SQL contains "LEFT JOIN gold.*_events_hourly"
    AND SQL contains "COALESCE(eh.total_events, 0)"

TEST: RejectDomainWithNoEventSources
    GIVEN domain with no state_event streams
    AND domain with no objectives
    WHEN GenerateUnifiedEventsView() is called
    THEN Err(Code409_NoEventSources) is returned

TEST: GlobalViewCombinesAllDomains
    GIVEN domains: ["indoor-air-quality", "energy-efficiency"]
    WHEN GenerateGlobalUnifiedView() is called
    THEN SQL contains "UNION ALL"
    AND SQL contains "gold.indoor_air_quality_events_unified"
    AND SQL contains "gold.energy_efficiency_events_unified"

TEST: EventOrderingDeterministic
    GIVEN events with same event_time
    WHEN unified view is queried
    THEN events are ordered by event_time, event_type, event_id

TEST: StateTransitionDetailsComplete
    GIVEN state transition event
    WHEN viewed in unified view
    THEN details contains from_state, to_state, duration_in_previous_ms, state_field

TEST: ThresholdCrossingDetailsPassthrough
    GIVEN threshold crossing from Phase E
    WHEN viewed in unified view
    THEN details are passed through unchanged
```

---

## V1.2 Query Pattern Examples

### Time Range Scan

```sql
-- V1.2 Pattern 1: Get recent events
SELECT * FROM gold.events_unified
WHERE event_time >= NOW() - INTERVAL '24 hours'
ORDER BY event_time;
```

### Type Filter

```sql
-- V1.2 Pattern 2: Get only threshold crossings
SELECT * FROM gold.events_unified
WHERE event_type = 'threshold_crossing'
  AND event_time >= NOW() - INTERVAL '7 days';
```

### Objective Filter

```sql
-- V1.2 Pattern 3: Get crossings for specific objective
SELECT * FROM gold.events_unified
WHERE event_type = 'threshold_crossing'
  AND details->>'objective_id' = 'healthy_co2';
```

### Entity Filter

```sql
-- V1.2 Pattern 4: Get events for specific entity
SELECT * FROM gold.events_unified
WHERE entity_id = 'window_backslider'
ORDER BY event_time;
```

### Combined Filter

```sql
-- V1.2 Pattern 5: Rising crossings in last 24 hours
SELECT * FROM gold.events_unified
WHERE event_time >= NOW() - INTERVAL '24 hours'
  AND event_type = 'threshold_crossing'
  AND details->>'direction' = 'rising';
```

### Hourly Summary with Metrics

```sql
-- V1.2 Pattern 6: Aligned data with event counts
SELECT
    a.bucket,
    a.pm25_mean,
    a.co2_mean,
    a.window_state,
    a.total_events,
    a.state_transitions,
    a.threshold_crossings
FROM gold.indoor_air_quality_aligned a
WHERE a.bucket >= NOW() - INTERVAL '7 days'
ORDER BY a.bucket;
```

---

## Monitoring Queries

### Event Distribution by Type

```sql
SELECT
    event_type,
    COUNT(*) AS count,
    COUNT(*) * 100.0 / SUM(COUNT(*)) OVER () AS percentage
FROM gold.events_unified
WHERE event_time >= NOW() - INTERVAL '7 days'
GROUP BY event_type;
```

### Hourly Event Density

```sql
SELECT
    bucket,
    total_events,
    state_transition_count,
    threshold_crossing_count,
    CASE
        WHEN total_events > 20 THEN 'high'
        WHEN total_events > 5 THEN 'medium'
        ELSE 'low'
    END AS density_level
FROM gold.indoor_air_quality_events_hourly
WHERE bucket >= NOW() - INTERVAL '24 hours'
ORDER BY bucket;
```

### Event Gaps (Hours with No Events)

```sql
WITH all_hours AS (
    SELECT generate_series(
        NOW() - INTERVAL '7 days',
        NOW(),
        INTERVAL '1 hour'
    ) AS bucket
)
SELECT ah.bucket
FROM all_hours ah
LEFT JOIN gold.indoor_air_quality_events_hourly eh ON ah.bucket = eh.bucket
WHERE eh.bucket IS NULL
ORDER BY ah.bucket;
```

---

## Index Strategy Notes

Since `gold.events_unified` is a VIEW (not a materialized view), indexes cannot be created directly on it. The indexing strategy relies on:

1. **Underlying Tables**: Indexes on `gold.*_transitions` and `gold.*_threshold_crossings`
2. **Hourly Aggregate**: Index on `bucket` column
3. **V1.2 Recommendations**: For heavy query loads, consider materializing the unified view

```sql
-- Indexes on underlying tables (Phase C and E)
CREATE INDEX idx_transitions_time ON gold.home_assistant_state_transitions(transition_time DESC);
CREATE INDEX idx_crossings_time ON gold.indoor_air_quality_threshold_crossings(event_time DESC);

-- For JSONB queries on details
CREATE INDEX idx_crossings_details ON gold.indoor_air_quality_threshold_crossings USING GIN (details);
```

---

## References

- [SPEC-E02-unified-events-view.md](../specification/SPEC-E02-unified-events-view.md)
- [SPEC-E01-threshold-crossings.md](../specification/SPEC-E01-threshold-crossings.md)
- [ALGO-threshold-crossing.md](./ALGO-threshold-crossing.md)
- [ALGO-state-transition-extract.md](../../phase-c/pseudocode/ALGO-state-transition-extract.md)
- [DECISIONS.md](../../architecture/DECISIONS.md)

---

*ALGO-E02 created: 2026-02-04 by pseudocode-agent*
