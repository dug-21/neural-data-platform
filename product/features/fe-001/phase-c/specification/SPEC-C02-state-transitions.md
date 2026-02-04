# SPEC-C02: State Transition Materializer (v11-006)

> **Feature ID:** v11-006
> **Feature Name:** State Transition Materializer
> **Phase:** C (Cross-Stream + Alignment)
> **Priority:** High
> **Created:** 2026-02-04

---

## User Story

**As a** pattern detection system,
**I want** discrete state transition events extracted from continuous state data,
**So that** I can correlate state changes with observation metrics for causality analysis.

---

## Goal

Create a generic state transition extraction mechanism that:
1. Converts raw state events into explicit transition records
2. Calculates duration in previous state
3. Filters out "noise" transitions (same state repeated)
4. Works for ANY `state_event` stream via configuration

---

## Background

### The Problem

The `home-assistant-state` stream records state events as they arrive:

```
event_time          | ndp_id           | state
--------------------|------------------|-------
2026-02-04 10:00:00 | door_backslider  | off
2026-02-04 10:05:00 | door_backslider  | on     <- actual transition
2026-02-04 10:05:01 | door_backslider  | on     <- duplicate (noise)
2026-02-04 10:15:00 | door_backslider  | off    <- actual transition
```

For pattern detection, we need:
- **Transition events** (when state actually changed)
- **Duration** (how long was previous state held)
- **Direction** (from_state -> to_state)

### The Solution

A materialized view that:
1. Uses window functions to compare consecutive states
2. Filters to only actual transitions
3. Calculates duration using LAG on timestamps

---

## Functional Requirements

### FR-C02-001: State Transition Extraction

**Description:** Extract actual state transitions from raw state event data.

**Acceptance Criteria:**
- Transitions are identified when `state` differs from previous `state` for same `ndp_id`
- First event for each `ndp_id` is considered a transition (initial state)
- Duplicate state events are filtered out
- Works for any `state_event` type stream

**SQL Pattern:**
```sql
SELECT
    event_time AS transition_time,
    ndp_id AS entity_id,
    LAG(state) OVER w AS from_state,
    state AS to_state,
    CASE
        WHEN LAG(state) OVER w IS DISTINCT FROM state THEN TRUE
        WHEN LAG(state) OVER w IS NULL THEN TRUE  -- First event
        ELSE FALSE
    END AS is_actual_transition
FROM silver.state_events
WHERE stream_id = 'home-assistant-state'
WINDOW w AS (PARTITION BY ndp_id ORDER BY event_time)
```

---

### FR-C02-002: Duration Calculation

**Description:** Calculate how long the entity was in the previous state before transitioning.

**Acceptance Criteria:**
- Duration calculated as time between consecutive events per entity
- Duration is NULL for first event (no previous state)
- Duration stored as INTERVAL type
- Duration in milliseconds also available for numeric operations

**SQL Pattern:**
```sql
event_time - LAG(event_time) OVER w AS duration_in_previous_state,
EXTRACT(EPOCH FROM (event_time - LAG(event_time) OVER w)) * 1000 AS duration_ms
```

---

### FR-C02-003: Config-Driven Generation

**Description:** Transition view is generated from stream config's `gold_etl.transitions` section.

**Acceptance Criteria:**
- `ndp-gold-ddl generate --stream home-assistant-state` includes transition view
- View name follows pattern: `gold.{stream_id}_transitions`
- Config specifies which field contains state value
- Config specifies entity field for partitioning

**Config Example:**
```yaml
gold_etl:
  transitions:
    enabled: true
    state_field: state
    entity_field: ndp_id
    track_duration: true
    include_in_alignment: true
```

---

### FR-C02-004: is_actual_transition Filter

**Description:** Provide a boolean flag to easily filter to real transitions.

**Acceptance Criteria:**
- `is_actual_transition = TRUE` for state changes
- `is_actual_transition = FALSE` for duplicate state events
- Can query with `WHERE is_actual_transition = TRUE` for clean data
- Flag logic handles NULL (first event) correctly

---

### FR-C02-005: Transition Direction

**Description:** Capture the direction of state change for analysis.

**Acceptance Criteria:**
- `from_state` is previous state value (NULL for first event)
- `to_state` is current state value
- For binary states (on/off), derive direction: 'opening' vs 'closing'
- Direction logic configurable per stream

**SQL Pattern:**
```sql
CASE
    WHEN from_state = 'off' AND to_state = 'on' THEN 'opening'
    WHEN from_state = 'on' AND to_state = 'off' THEN 'closing'
    ELSE 'unknown'
END AS transition_direction
```

---

### FR-C02-006: Entity Metadata Preservation

**Description:** Preserve entity identification for grouping and filtering.

**Acceptance Criteria:**
- `ndp_id` or configured entity field carried through
- Source entity ID available if different from ndp_id
- Stream ID included for multi-stream scenarios
- Device type derivable from ndp_id pattern (e.g., `door_*`, `window_*`)

---

## Non-Functional Requirements

### NFR-C02-001: Query Performance

**Description:** Transition queries must be efficient.

**Acceptance Criteria:**
- Query for 30-day transitions < 50ms
- Index on (ndp_id, transition_time) exists
- View is materialized for performance

---

### NFR-C02-002: Real-Time Availability

**Description:** Transitions available shortly after state events arrive.

**Acceptance Criteria:**
- Materialized view refreshes with Gold layer (every 15 min)
- Latency from state event to transition availability < 20 min
- Refresh does not block queries

---

## Stream Configuration Example

**File:** `config/base/streams/home-assistant-state/config.yaml` (extended)

```yaml
stream_id: "home-assistant-state"
stream_type: "state_event"  # NEW: enables transition extraction
description: "Window/door state events from Home Assistant via MQTT"
# ... existing fields, sources, silver_etl ...

gold_etl:
  enabled: true
  description: "Gold layer transformation for state event stream"

  # Hourly aggregates for aligned view
  aggregates:
    granularities: ["1 hour"]
    fields:
      state:
        metrics: [count, first, last]
        derived:
          - name: window_open_count
            expression: "COUNT(*) FILTER (WHERE state = 'on' AND ndp_id LIKE 'door_%' OR ndp_id LIKE 'window_%')"
          - name: state_changes_count
            expression: "COUNT(DISTINCT state)"

  # State transition extraction
  transitions:
    enabled: true
    state_field: state
    entity_field: ndp_id
    track_duration: true
    include_in_alignment: true
    direction_mapping:
      off_to_on: "opening"
      on_to_off: "closing"
```

---

## Generated SQL Example

### Transition View

```sql
-- Generated by ndp-gold-ddl for stream: home-assistant-state
-- Transition extraction view

CREATE MATERIALIZED VIEW gold.state_events_transitions AS
SELECT
    event_time AS transition_time,
    ndp_id AS entity_id,
    'home-assistant-state' AS stream_id,

    -- State transition details
    LAG(state) OVER w AS from_state,
    state AS to_state,

    -- Is this an actual state change?
    CASE
        WHEN LAG(state) OVER w IS DISTINCT FROM state THEN TRUE
        WHEN LAG(state) OVER w IS NULL THEN TRUE
        ELSE FALSE
    END AS is_actual_transition,

    -- Duration in previous state
    event_time - LAG(event_time) OVER w AS duration_in_previous_state,
    EXTRACT(EPOCH FROM (event_time - LAG(event_time) OVER w)) * 1000 AS duration_ms,

    -- Transition direction (for binary on/off states)
    CASE
        WHEN LAG(state) OVER w = 'off' AND state = 'on' THEN 'opening'
        WHEN LAG(state) OVER w = 'on' AND state = 'off' THEN 'closing'
        WHEN LAG(state) OVER w IS NULL THEN 'initial'
        ELSE 'unknown'
    END AS transition_direction,

    -- Device type derived from ndp_id
    CASE
        WHEN ndp_id LIKE 'door_%' THEN 'door'
        WHEN ndp_id LIKE 'window_%' THEN 'window'
        ELSE 'other'
    END AS device_type

FROM silver.state_events
WINDOW w AS (PARTITION BY ndp_id ORDER BY event_time);

-- Index for efficient queries
CREATE INDEX IF NOT EXISTS idx_state_events_transitions_time
    ON gold.state_events_transitions (transition_time DESC);

CREATE INDEX IF NOT EXISTS idx_state_events_transitions_entity
    ON gold.state_events_transitions (entity_id, transition_time DESC);

-- Filtered view for only actual transitions
CREATE VIEW gold.state_transitions_actual AS
SELECT * FROM gold.state_events_transitions
WHERE is_actual_transition = TRUE;

COMMENT ON MATERIALIZED VIEW gold.state_events_transitions IS
    'State transitions extracted from home-assistant-state stream. Refresh with Gold layer.';
```

---

## Acceptance Criteria (Given/When/Then)

### Scenario: Extract Actual Transitions

```gherkin
Given silver.state_events has these records:
  | event_time | ndp_id          | state |
  | 10:00:00   | door_backslider | off   |
  | 10:05:00   | door_backslider | on    |
  | 10:05:01   | door_backslider | on    |
  | 10:15:00   | door_backslider | off   |
When I query gold.state_transitions_actual
Then I should see 3 transitions:
  | transition_time | from_state | to_state | is_actual_transition |
  | 10:00:00        | NULL       | off      | TRUE (initial)       |
  | 10:05:00        | off        | on       | TRUE                 |
  | 10:15:00        | on         | off      | TRUE                 |
And the 10:05:01 duplicate is excluded
```

### Scenario: Calculate Duration in Previous State

```gherkin
Given a transition from 'off' to 'on' at 10:05:00
And the previous event was at 10:00:00
When I query the transition record
Then duration_in_previous_state should be '5 minutes'
And duration_ms should be 300000
```

### Scenario: Partition by Entity

```gherkin
Given state events for door_backslider and window_office
When I query transitions
Then each entity has its own transition sequence
And from_state for entity A does not leak into entity B
```

### Scenario: Config-Driven Generation

```gherkin
Given home-assistant-state config has gold_etl.transitions.enabled = true
When I run ndp-gold-ddl generate --stream home-assistant-state
Then the output includes CREATE MATERIALIZED VIEW gold.state_events_transitions
And the state_field matches config (state)
And the entity_field matches config (ndp_id)
```

---

## London TDD Interfaces

### ITransitionGenerator (tools/ndp-gold-ddl)

```rust
/// Generates SQL for state transition extraction
pub trait ITransitionGenerator {
    /// Generate transition view DDL for a state_event stream
    fn generate_transitions_ddl(
        &self,
        stream_config: &StreamConfig,
    ) -> Result<String, GeneratorError>;

    /// Generate filtered view for actual transitions only
    fn generate_actual_transitions_view(
        &self,
        base_view: &str,
    ) -> Result<String, GeneratorError>;
}
```

### ITransitionConfig

```rust
/// Configuration for transition extraction
pub struct TransitionConfig {
    pub enabled: bool,
    pub state_field: String,
    pub entity_field: String,
    pub track_duration: bool,
    pub include_in_alignment: bool,
    pub direction_mapping: Option<HashMap<String, String>>,
}
```

### ITransitionEvent (core/src/gold)

```rust
/// Domain type for a state transition event
pub struct TransitionEvent {
    pub transition_time: chrono::DateTime<chrono::Utc>,
    pub entity_id: String,
    pub stream_id: String,
    pub from_state: Option<String>,
    pub to_state: String,
    pub is_actual_transition: bool,
    pub duration_in_previous_state: Option<chrono::Duration>,
    pub transition_direction: TransitionDirection,
}

pub enum TransitionDirection {
    Opening,
    Closing,
    Initial,
    Unknown,
}
```

---

## Dependencies

| Dependency | Type | Required By |
|------------|------|-------------|
| v11-003: state-events hourly aggregate | Phase B/C | Source data |
| stream_type field in config | Phase A | Identifying state_event streams |
| gold_etl.transitions config section | Phase A | Configuration |
| silver.state_events table | V1.0 | Source data |

---

## Test Cases

### Unit Tests (tools/ndp-gold-ddl/tests/state_transitions_test.rs)

| Test | Description | Expected |
|------|-------------|----------|
| `test_generate_transition_ddl` | Generate DDL for state stream | Valid CREATE MATERIALIZED VIEW |
| `test_is_actual_transition_logic` | Transition flag correctness | TRUE for changes, FALSE for dups |
| `test_duration_calculation` | Duration SQL | Correct LAG expression |
| `test_direction_mapping` | Direction derivation | Config-driven CASE expression |
| `test_entity_partitioning` | Window partition | PARTITION BY entity_field |
| `test_non_state_stream_rejection` | Non-state stream | Error or skip transitions |

### Integration Tests

| Test | Description | Verification |
|------|-------------|--------------|
| `test_transition_extraction_30d` | Query transitions for 30 days | Correct count, no duplicates |
| `test_duration_accuracy` | Verify duration calculations | Match expected intervals |
| `test_multi_entity_isolation` | Multiple entities | No cross-contamination |

---

## Hourly Aggregate Integration

Transitions are also aggregated hourly for the aligned view:

```sql
-- In gold.state_events_hourly (generated by v11-003)
SELECT
    time_bucket('1 hour', transition_time) AS bucket,

    -- Transition counts per hour
    COUNT(*) FILTER (WHERE is_actual_transition AND transition_direction = 'opening') AS opens_count,
    COUNT(*) FILTER (WHERE is_actual_transition AND transition_direction = 'closing') AS closes_count,
    COUNT(*) FILTER (WHERE is_actual_transition) AS total_transitions,

    -- Last known state at end of hour
    LAST(to_state, transition_time) AS state_at_hour_end,

    -- Average duration of states that ended this hour
    AVG(duration_ms) FILTER (WHERE is_actual_transition) AS avg_state_duration_ms

FROM gold.state_events_transitions
WHERE is_actual_transition = TRUE
GROUP BY bucket;
```

---

## Data Dictionary Integration

```sql
-- gold_tables entry for transitions view
INSERT INTO data_dictionary.gold_tables (
    table_name, object_type, source_silver_table, description
) VALUES (
    'state_events_transitions',
    'materialized_view',
    'silver.state_events',
    'State transitions extracted from home-assistant-state stream'
);

-- gold_columns entries
INSERT INTO data_dictionary.gold_columns (
    table_name, column_name, data_type, feature_type, description
) VALUES
    ('state_events_transitions', 'transition_time', 'timestamptz', 'raw', 'When state changed'),
    ('state_events_transitions', 'entity_id', 'text', 'raw', 'Device identifier'),
    ('state_events_transitions', 'from_state', 'text', 'derived', 'Previous state value'),
    ('state_events_transitions', 'to_state', 'text', 'raw', 'New state value'),
    ('state_events_transitions', 'is_actual_transition', 'boolean', 'derived', 'TRUE if state actually changed'),
    ('state_events_transitions', 'duration_in_previous_state', 'interval', 'derived', 'Time in previous state'),
    ('state_events_transitions', 'transition_direction', 'text', 'derived', 'opening/closing/initial/unknown');
```

---

## Unified Events Integration (Phase E)

State transitions will be included in the unified events view (v11-013):

```sql
-- In gold.events_unified (Phase E)
SELECT
    transition_time AS event_time,
    stream_id,
    entity_id,
    'state_transition'::text AS event_type,
    jsonb_build_object(
        'from_state', from_state,
        'to_state', to_state,
        'duration_ms', duration_ms,
        'direction', transition_direction,
        'device_type', device_type
    ) AS details
FROM gold.state_events_transitions
WHERE is_actual_transition = TRUE
```

---

## References

- [SCOPE.md - v11-006](/workspaces/neural-data-platform/product/features/fe-001/SCOPE.md)
- [DECISIONS.md](/workspaces/neural-data-platform/product/features/fe-001/architecture/DECISIONS.md)
- [home-assistant-state config](/workspaces/neural-data-platform/config/base/streams/home-assistant-state/config.yaml)
- [PHASE-C-OVERVIEW.md](./PHASE-C-OVERVIEW.md) - Phase C overview
