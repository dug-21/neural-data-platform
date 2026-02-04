# SPEC-E02: Unified Events View

> **Feature ID:** v11-013
> **Priority:** Critical
> **Status:** Specification
> **Dependencies:** v11-006 (State Transitions), v11-012 (Threshold Crossings)
> **Blocks:** V1.2 Pattern Detection Engine

---

## User Story

**As a** pattern detection system (V1.2),
**I want** a single unified view combining all event types,
**So that** I can scan for patterns across different event categories without joining multiple views.

---

## Goal

Create `gold.events_unified` - a single SQL view that:
1. Combines state transition events (from Phase C v11-006)
2. Combines threshold crossing events (from Phase E v11-012)
3. Presents a consistent schema for all event types
4. Provides hourly event aggregates for the aligned view
5. Serves as the PRIMARY event interface for V1.2

**Key Insight**: V1.2 Pattern Detection needs to correlate "what happened" (state changes like window open) with "what changed" (metric crossed threshold). A unified event view enables this correlation in a single query.

---

## Functional Requirements

### FR-E02-001: Unified Event Schema

All events in `gold.events_unified` SHALL have this schema:

| Column | Type | Description | NOT NULL |
|--------|------|-------------|----------|
| `event_id` | UUID | Unique event identifier | Yes |
| `event_time` | TIMESTAMPTZ | When the event occurred | Yes |
| `stream_id` | TEXT | Source stream identifier | Yes |
| `entity_id` | TEXT | Entity identifier (ndp_id) | Yes |
| `event_type` | TEXT | Event type enum value | Yes |
| `details` | JSONB | Type-specific event payload | Yes |

### FR-E02-002: Event Type Enumeration

Supported event types for V1.1:

| Event Type | Source | Description |
|------------|--------|-------------|
| `state_transition` | v11-006 | State field value changed |
| `threshold_crossing` | v11-012 | Metric crossed objective threshold |

Future event types (V1.2+):
- `anomaly` - Statistical anomaly detected
- `trend_change` - Significant trend direction change

### FR-E02-003: State Transition Details

For `event_type = 'state_transition'`:

```json
{
  "from_state": "off",
  "to_state": "on",
  "duration_in_previous_ms": 3600000,
  "state_field": "state"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `from_state` | string | Previous state value |
| `to_state` | string | New state value |
| `duration_in_previous_ms` | number | Time in previous state (milliseconds) |
| `state_field` | string | Name of the state field |

### FR-E02-004: Threshold Crossing Details

For `event_type = 'threshold_crossing'`:

```json
{
  "metric": "co2",
  "threshold": 800,
  "direction": "rising",
  "value": 812,
  "previous_value": 795,
  "objective_id": "healthy_co2",
  "condition": "<",
  "unit": "ppm"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `metric` | string | Metric name from stream |
| `threshold` | number | Objective threshold value |
| `direction` | string | `rising`, `falling`, `entering_range`, `exiting_range_low`, `exiting_range_high` |
| `value` | number | Current metric value |
| `previous_value` | number | Previous metric value |
| `objective_id` | string | Reference to objective |
| `condition` | string | Objective condition operator |
| `unit` | string | Unit of measurement (optional) |

### FR-E02-005: View Composition

The unified events view SHALL be a UNION ALL of:
1. State transition events (filtered to actual transitions only)
2. Threshold crossing events (filtered to actual crossings only)

```sql
CREATE VIEW gold.events_unified AS
    SELECT ... FROM gold.{domain}_state_transitions WHERE is_actual_transition = TRUE
    UNION ALL
    SELECT ... FROM gold.{domain}_threshold_crossings
```

### FR-E02-006: Hourly Event Aggregate

Create a continuous aggregate for hourly event counts:

```sql
CREATE MATERIALIZED VIEW gold.events_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', event_time) AS bucket,
    COUNT(*) AS total_events,
    COUNT(*) FILTER (WHERE event_type = 'state_transition') AS state_transition_count,
    COUNT(*) FILTER (WHERE event_type = 'threshold_crossing') AS threshold_crossing_count
FROM gold.events_unified
GROUP BY bucket;
```

### FR-E02-007: Domain Scope

Events are scoped to a domain:
- Only streams included in the domain contribute events
- Domain boundaries prevent cross-domain event mixing
- View name follows pattern: `gold.{domain}_events_unified`

For convenience, a global view MAY also be created:
```sql
CREATE VIEW gold.events_unified AS
SELECT * FROM gold.indoor_air_quality_events_unified
UNION ALL
SELECT * FROM gold.energy_efficiency_events_unified
-- etc.
```

### FR-E02-008: Ordering

Events SHALL be ordered by:
1. `event_time` (primary)
2. `event_type` (secondary, for determinism)
3. `event_id` (tertiary, for determinism)

### FR-E02-009: Event ID Generation

Event IDs SHALL be:
- Generated via `gen_random_uuid()` for V1.1
- Deterministic replay NOT required for V1.1
- Future: May switch to content-based hashing for idempotency

### FR-E02-010: Aligned View Integration

Hourly event counts SHALL be included in the domain aligned view:

```sql
-- In gold.{domain}_aligned
SELECT
    a.bucket,
    -- ... other columns ...
    COALESCE(eh.total_events, 0) AS total_events,
    COALESCE(eh.state_transition_count, 0) AS state_transitions,
    COALESCE(eh.threshold_crossing_count, 0) AS threshold_crossings
FROM gold.{domain}_hourly a
LEFT JOIN gold.events_hourly eh ON a.bucket = eh.bucket
```

---

## Non-Functional Requirements

### NFR-E02-001: Query Performance

| Query | Target | Measured By |
|-------|--------|-------------|
| All events in 30-day range | < 100ms | pg_stat_statements |
| Events filtered by type | < 50ms | pg_stat_statements |
| Hourly aggregate query | < 20ms | pg_stat_statements |

### NFR-E02-002: V1.2 Query Pattern Support

The view SHALL support these V1.2 query patterns efficiently:

```sql
-- Pattern 1: Time range scan
SELECT * FROM gold.events_unified
WHERE event_time BETWEEN :start AND :end;

-- Pattern 2: Type filter
SELECT * FROM gold.events_unified
WHERE event_type = 'threshold_crossing';

-- Pattern 3: Objective filter
SELECT * FROM gold.events_unified
WHERE details->>'objective_id' = :objective_id;

-- Pattern 4: Entity filter
SELECT * FROM gold.events_unified
WHERE entity_id = :entity_id;

-- Pattern 5: Combined filter
SELECT * FROM gold.events_unified
WHERE event_time >= NOW() - INTERVAL '24 hours'
  AND event_type = 'threshold_crossing'
  AND details->>'direction' = 'rising';
```

### NFR-E02-003: Index Strategy

Create indexes to support V1.2 patterns:

```sql
-- Primary access pattern: time range
CREATE INDEX idx_events_unified_time
    ON gold.events_unified(event_time);

-- Filter by type
CREATE INDEX idx_events_unified_type
    ON gold.events_unified(event_type, event_time);

-- Filter by entity
CREATE INDEX idx_events_unified_entity
    ON gold.events_unified(entity_id, event_time);

-- JSONB details (GIN index for flexible queries)
CREATE INDEX idx_events_unified_details
    ON gold.events_unified USING GIN (details);
```

**Note**: Views cannot have indexes directly. These indexes are on the underlying tables/views that feed the UNION ALL.

### NFR-E02-004: Schema Stability

The unified events schema is the **V1.2 contract**:
- Adding new fields to `details` is allowed (additive)
- Removing fields from `details` requires V1.2 coordination
- Changing column types requires V1.2 coordination
- Adding new `event_type` values is allowed (V1.2 should handle unknown types)

---

## Acceptance Criteria

### AC-E02-001: Schema Compliance

```gherkin
Scenario: Unified events view has correct schema
  Given gold.events_unified view exists
  When I query the view schema
  Then it SHALL have columns: event_id, event_time, stream_id, entity_id, event_type, details
  And event_id SHALL be UUID type
  And event_time SHALL be TIMESTAMPTZ type
  And details SHALL be JSONB type
```

### AC-E02-002: State Transitions Included

```gherkin
Scenario: State transitions appear in unified view
  Given a state transition event in gold.{domain}_state_transitions
  And is_actual_transition = TRUE
  When I query gold.events_unified
  Then the state transition event SHALL appear
  And event_type = 'state_transition'
  And details SHALL contain from_state, to_state
```

### AC-E02-003: Threshold Crossings Included

```gherkin
Scenario: Threshold crossings appear in unified view
  Given a threshold crossing event in gold.{domain}_threshold_crossings
  When I query gold.events_unified
  Then the threshold crossing event SHALL appear
  And event_type = 'threshold_crossing'
  And details SHALL contain metric, threshold, direction
```

### AC-E02-004: False Transitions Excluded

```gherkin
Scenario: Non-actual transitions are excluded
  Given a state event where is_actual_transition = FALSE
  When I query gold.events_unified
  Then the event SHALL NOT appear
```

### AC-E02-005: Hourly Aggregate Works

```gherkin
Scenario: Hourly event counts are accurate
  Given 5 state transitions in hour 10:00
  And 3 threshold crossings in hour 10:00
  When I query gold.events_hourly for bucket 10:00
  Then total_events = 8
  And state_transition_count = 5
  And threshold_crossing_count = 3
```

### AC-E02-006: V1.2 Pattern 1 - Time Range

```gherkin
Scenario: V1.2 can query events by time range
  Given events spanning 7 days
  When V1.2 queries events for last 24 hours
  Then only events from last 24 hours SHALL be returned
  And query completes in < 100ms
```

### AC-E02-007: V1.2 Pattern 3 - Objective Filter

```gherkin
Scenario: V1.2 can filter by objective
  Given threshold crossings for objectives "healthy_co2" and "healthy_pm25"
  When V1.2 queries WHERE details->>'objective_id' = 'healthy_co2'
  Then only healthy_co2 crossings SHALL be returned
```

### AC-E02-008: Aligned View Integration

```gherkin
Scenario: Aligned view includes event counts
  Given events exist for hour 10:00
  When I query gold.{domain}_aligned for bucket 10:00
  Then total_events column SHALL have correct count
  And state_transitions column SHALL have correct count
  And threshold_crossings column SHALL have correct count
```

### AC-E02-009: Empty Hours Handled

```gherkin
Scenario: Hours with no events show zero counts
  Given no events exist for hour 10:00
  When I query gold.{domain}_aligned for bucket 10:00
  Then total_events = 0
  And state_transitions = 0
  And threshold_crossings = 0
```

---

## SQL Generation

### Domain-Scoped Unified Events View

```sql
-- Generated by ndp-gold-ddl for domain: indoor-air-quality
CREATE OR REPLACE VIEW gold.indoor_air_quality_events_unified AS

-- State transition events
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
        'state_field', state_field
    ) AS details
FROM gold.home_assistant_state_transitions
WHERE is_actual_transition = TRUE

UNION ALL

-- Threshold crossing events (already has event_id, details)
SELECT
    event_id,
    event_time,
    stream_id,
    entity_id,
    event_type,
    details
FROM gold.indoor_air_quality_threshold_crossings;
```

### Global Unified Events View (Optional)

```sql
-- Optional: Global view across all domains
CREATE OR REPLACE VIEW gold.events_unified AS
SELECT * FROM gold.indoor_air_quality_events_unified
-- UNION ALL SELECT * FROM gold.energy_efficiency_events_unified
-- Add more domains as needed
;
```

### Hourly Events Aggregate

```sql
-- Hourly event counts for aligned view integration
CREATE MATERIALIZED VIEW gold.events_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', event_time) AS bucket,
    COUNT(*) AS total_events,
    COUNT(*) FILTER (WHERE event_type = 'state_transition') AS state_transition_count,
    COUNT(*) FILTER (WHERE event_type = 'threshold_crossing') AS threshold_crossing_count,
    -- Per-entity counts for multi-entity analysis
    COUNT(DISTINCT entity_id) AS distinct_entities_with_events
FROM gold.events_unified
GROUP BY bucket;

-- Refresh policy
SELECT add_continuous_aggregate_policy('gold.events_hourly',
    start_offset => INTERVAL '4 hours',
    end_offset => INTERVAL '15 minutes',
    schedule_interval => INTERVAL '15 minutes'
);
```

### Aligned View Extension

```sql
-- Extend the aligned view to include event counts
-- This is added to the existing gold.{domain}_aligned view generation

SELECT
    a.bucket,
    -- ... existing columns from streams ...

    -- Event counts from hourly aggregate
    COALESCE(eh.total_events, 0) AS total_events,
    COALESCE(eh.state_transition_count, 0) AS state_transitions,
    COALESCE(eh.threshold_crossing_count, 0) AS threshold_crossings

FROM gold.air_quality_hourly a
FULL OUTER JOIN gold.outdoor_weather_hourly ow ON a.bucket = ow.bucket
FULL OUTER JOIN gold.state_events_hourly se ON a.bucket = se.bucket
LEFT JOIN gold.events_hourly eh ON a.bucket = eh.bucket;
```

---

## Configuration

### Deploy Manifest

Add unified events to Phase E manifest:

```json
{
  "version": "1.1.0-phase-e",
  "declarations": {
    "gold-tables": [
      { "stream_id": "air-quality", "action": "sync" },
      { "stream_id": "home-assistant-state", "action": "sync" }
    ],
    "domains": [
      {
        "domain_id": "indoor-air-quality",
        "action": "sync",
        "components": ["threshold-crossings", "unified-events", "events-hourly"]
      }
    ]
  }
}
```

### CLI Commands

```bash
# Generate unified events view
ndp-gold-ddl generate --domain indoor-air-quality --component unified-events

# Generate hourly aggregate
ndp-gold-ddl generate --domain indoor-air-quality --component events-hourly

# Generate all Phase E components
ndp-gold-ddl generate --domain indoor-air-quality --phase events

# Validate Phase E configuration
ndp-gold-ddl validate --domain indoor-air-quality --phase events
```

---

## V1.2 Handoff Documentation

### Query Interface Contract

V1.2 Pattern Detection Engine should use these query patterns:

```sql
-- 1. Get recent events for correlation analysis
SELECT * FROM gold.events_unified
WHERE event_time >= NOW() - INTERVAL '7 days'
ORDER BY event_time;

-- 2. Get events around a specific time window
SELECT * FROM gold.events_unified
WHERE event_time BETWEEN :window_start AND :window_end
ORDER BY event_time;

-- 3. Get events by type for type-specific analysis
SELECT * FROM gold.events_unified
WHERE event_type = :event_type
  AND event_time >= :since
ORDER BY event_time;

-- 4. Get events for specific entity
SELECT * FROM gold.events_unified
WHERE entity_id = :entity_id
  AND event_time >= :since
ORDER BY event_time;

-- 5. Get hourly event counts with aligned data
SELECT
    a.bucket,
    a.indoor_pm25,
    a.indoor_co2,
    a.window_state,
    a.total_events,
    a.state_transitions,
    a.threshold_crossings
FROM gold.indoor_air_quality_aligned a
WHERE a.bucket >= NOW() - INTERVAL '7 days'
ORDER BY a.bucket;
```

### Schema Contract

```typescript
// V1.2 should expect this structure
interface UnifiedEvent {
  event_id: string;       // UUID
  event_time: string;     // ISO 8601 timestamp
  stream_id: string;      // Source stream identifier
  entity_id: string;      // Entity (sensor) identifier
  event_type: 'state_transition' | 'threshold_crossing';
  details: StateTransitionDetails | ThresholdCrossingDetails;
}

interface StateTransitionDetails {
  from_state: string;
  to_state: string;
  duration_in_previous_ms: number;
  state_field: string;
}

interface ThresholdCrossingDetails {
  metric: string;
  threshold: number;
  direction: 'rising' | 'falling' | 'entering_range' | 'exiting_range_low' | 'exiting_range_high';
  value: number;
  previous_value: number;
  objective_id: string;
  condition: '<' | '<=' | '>' | '>=' | 'between';
  unit?: string;
}
```

### Handling Unknown Event Types

V1.2 should gracefully handle unknown event types (future-proofing):

```typescript
// V1.2 recommendation
function processEvent(event: UnifiedEvent): void {
  switch (event.event_type) {
    case 'state_transition':
      handleStateTransition(event);
      break;
    case 'threshold_crossing':
      handleThresholdCrossing(event);
      break;
    default:
      // Log and skip unknown event types
      logger.warn(`Unknown event type: ${event.event_type}`);
  }
}
```

---

## London TDD Interfaces

### Trait: UnifiedEventsGenerator

```rust
pub trait UnifiedEventsGenerator {
    /// Generate SQL for unified events view
    fn generate_unified_view(&self, domain: &DomainConfig) -> Result<String, GeneratorError>;

    /// Generate SQL for hourly events aggregate
    fn generate_hourly_aggregate(&self) -> Result<String, GeneratorError>;

    /// Generate SQL to extend aligned view with event counts
    fn generate_aligned_extension(&self, domain: &DomainConfig) -> Result<String, GeneratorError>;
}
```

### Struct: EventSchema

```rust
#[derive(Debug, Clone, Serialize)]
pub struct UnifiedEvent {
    pub event_id: Uuid,
    pub event_time: DateTime<Utc>,
    pub stream_id: String,
    pub entity_id: String,
    pub event_type: EventType,
    pub details: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum EventType {
    StateTransition,
    ThresholdCrossing,
}
```

---

## Integration Test Requirements

### Test: View Composition

```rust
#[test]
fn test_unified_view_includes_state_transitions() {
    // Setup: Insert state transition into source view
    let transition = create_test_state_transition("window", "off", "on");
    insert_state_transition(&db, &transition);

    // Query unified view
    let events = query_unified_events(&db, &TimeRange::last_24_hours());

    // Verify
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event_type, EventType::StateTransition);
    assert_eq!(events[0].details["to_state"], "on");
}

#[test]
fn test_unified_view_includes_threshold_crossings() {
    // Setup: Objectives and observations that produce crossing
    setup_objective("healthy_co2", "<", 800);
    insert_hourly_observation("10:00", 795);
    insert_hourly_observation("11:00", 812);

    // Query unified view
    let events = query_unified_events(&db, &TimeRange::last_24_hours());

    // Verify
    assert!(events.iter().any(|e| e.event_type == EventType::ThresholdCrossing));
}
```

### Test: SQL Generation

```rust
#[test]
fn test_generates_unified_view_sql() {
    let domain = create_test_domain();
    let generator = UnifiedEventsGenerator::new();

    let sql = generator.generate_unified_view(&domain).unwrap();

    assert!(sql.contains("CREATE OR REPLACE VIEW"));
    assert!(sql.contains("events_unified"));
    assert!(sql.contains("UNION ALL"));
    assert!(sql.contains("state_transition"));
    assert!(sql.contains("threshold_crossing"));
}

#[test]
fn test_generates_hourly_aggregate_sql() {
    let generator = UnifiedEventsGenerator::new();

    let sql = generator.generate_hourly_aggregate().unwrap();

    assert!(sql.contains("CREATE MATERIALIZED VIEW"));
    assert!(sql.contains("events_hourly"));
    assert!(sql.contains("time_bucket"));
    assert!(sql.contains("total_events"));
    assert!(sql.contains("state_transition_count"));
    assert!(sql.contains("threshold_crossing_count"));
}
```

### Test: V1.2 Query Patterns

```rust
#[test]
fn test_v12_pattern_time_range() {
    // Setup events across multiple days
    setup_events_for_week();

    // Execute V1.2 query pattern
    let sql = "SELECT * FROM gold.events_unified WHERE event_time >= NOW() - INTERVAL '24 hours'";
    let result = execute_query(&db, sql);

    // Verify only last 24 hours returned
    assert!(result.iter().all(|e| e.event_time >= now_minus_24h()));
}

#[test]
fn test_v12_pattern_objective_filter() {
    // Setup crossings for different objectives
    setup_crossing("healthy_co2", "rising");
    setup_crossing("healthy_pm25", "rising");

    // Execute V1.2 query pattern
    let sql = "SELECT * FROM gold.events_unified WHERE details->>'objective_id' = 'healthy_co2'";
    let result = execute_query(&db, sql);

    // Verify only healthy_co2 returned
    assert!(result.iter().all(|e| e.details["objective_id"] == "healthy_co2"));
}
```

---

## Future Event Types

### V1.2 Additions (Planned)

| Event Type | Source | Detection Method |
|------------|--------|------------------|
| `anomaly` | Statistical analysis | Z-score or IQR outlier detection |
| `trend_change` | Trend features | Sign change in trend slope |

### Schema Extension Path

When adding new event types:

1. Add to `event_type` enum (additive, non-breaking)
2. Document `details` schema for new type
3. Add to UNION ALL in unified view
4. Update V1.2 documentation
5. V1.2 handles unknown types gracefully (already specified)

---

## References

- [SCOPE.md](../../SCOPE.md) - v11-013 description
- [SPEC-E01](./SPEC-E01-threshold-crossings.md) - Threshold crossing events
- [DECISIONS.md](../../architecture/DECISIONS.md) - Architecture decisions
- [PHASE-E-OVERVIEW.md](./PHASE-E-OVERVIEW.md) - Phase E context and V1.2 handoff

---

*SPEC-E02 created: 2026-02-04 by specification-agent*
