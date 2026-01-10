# ADR-006-006: Stream Type Distinction

**Feature**: dp-006 (Silver Layer Implementation)
**Status**: Accepted
**Date**: 2026-01-10
**Author**: NDP Architect
**Supersedes**: None

---

## Context

The Neural Data Platform ingests different categories of time-series data:

1. **Observations** - Continuous measurements at regular intervals (e.g., temperature every minute)
2. **Events** - Discrete state changes at irregular intervals (e.g., door opened/closed)
3. **Forecasts** - Future predictions with valid time windows (e.g., hourly weather forecast)

Current streams are all observations. Future Home Assistant integration will introduce events. The config schema should accommodate both without breaking changes.

### Current Streams (All Observations)

| Stream | Pattern | Cadence |
|--------|---------|---------|
| air-quality | Observation | ~1 minute |
| outdoor-weather | Observation | ~15 minutes |
| outdoor-air-quality | Observation | ~1 hour |
| nws-observations | Observation | ~1 hour |
| nws-forecast-hourly | Forecast | ~6 hours |
| nws-gridpoints-forecast | Forecast | ~6 hours |

### Future Streams (Events)

| Stream | Pattern | Cadence |
|--------|---------|---------|
| home-assistant-states | Event | Irregular |
| home-assistant-history | Event | Irregular |

---

## Decision

**Add `stream_type` field to stream configuration** to distinguish observations from events.

```yaml
# config/base/streams/air-quality/config.yaml
stream_id: air-quality
stream_type: observations  # observations | events | forecasts
# ... rest of config
```

### Type Definitions

| Type | Description | Primary Key Pattern | Dedup Strategy |
|------|-------------|---------------------|----------------|
| `observations` | Continuous time-series measurements | `(observation_time, ndp_id)` | Last value wins |
| `events` | Discrete state changes | `(event_time, ndp_id, event_type)` | First in window |
| `forecasts` | Future predictions | `(issue_time, valid_time, ndp_id)` | Latest issue wins |

### Default Behavior

If `stream_type` is not specified, default to `observations` for backward compatibility.

---

## Consequences

### Positive

1. **Forward-compatible** - Schema ready for Home Assistant integration
2. **Explicit semantics** - Clear distinction guides ETL and query patterns
3. **Config-driven** - Type informs Silver table structure automatically
4. **Query optimization** - TimescaleDB can optimize based on type patterns
5. **Documentation** - Self-documenting configuration

### Negative

1. **Slight over-design** - Not immediately needed for Phase 1
2. **Additional field** - One more thing to configure per stream

### Neutral

1. **No runtime impact** - Just metadata for ETL decisions
2. **Backward compatible** - Existing configs work without change

---

## Type Specifications

### Type: `observations`

**Definition**: Continuous measurements at regular or semi-regular intervals.

**Characteristics**:
- Time-ordered sequence of measurements
- Same metric measured repeatedly
- Missing data indicates collection issue
- Aggregation: time_bucket averages make sense

**Silver Table Pattern**:
```sql
CREATE TABLE silver.{domain}_observations (
    observation_time    TIMESTAMPTZ NOT NULL,
    ingestion_time      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ndp_id              TEXT NOT NULL,
    -- measurement columns
    PRIMARY KEY (observation_time, ndp_id)
);

-- TimescaleDB hypertable on observation_time
SELECT create_hypertable('silver.{domain}_observations', 'observation_time');
```

**Deduplication**: On conflict, take latest ingestion (upsert).

**Example Streams**:
- air-quality, outdoor-weather, outdoor-air-quality
- nws-observations, nws-station-observations

---

### Type: `events`

**Definition**: Discrete state changes at irregular intervals.

**Characteristics**:
- Sparse data (may be hours between events)
- State changes, not measurements
- Missing events could be legitimate (nothing happened)
- Aggregation: counts and state duration make sense

**Silver Table Pattern**:
```sql
CREATE TABLE silver.{domain}_events (
    event_time          TIMESTAMPTZ NOT NULL,
    ingestion_time      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ndp_id              TEXT NOT NULL,
    event_type          TEXT NOT NULL,
    previous_state      TEXT,
    new_state           TEXT,
    -- event-specific columns
    attributes          JSONB DEFAULT '{}',
    PRIMARY KEY (event_time, ndp_id, event_type)
);

-- TimescaleDB hypertable on event_time
SELECT create_hypertable('silver.{domain}_events', 'event_time');
```

**Deduplication**: Within time window (e.g., 5 seconds), take first event only.

**Example Streams** (Future):
- home-assistant-states
- home-assistant-history
- motion-sensor-events

---

### Type: `forecasts`

**Definition**: Future predictions with validity windows.

**Characteristics**:
- Two time dimensions: issue time and valid time
- Multiple forecasts for same valid time (newer supersedes older)
- Historical forecasts useful for accuracy analysis
- Aggregation: Latest forecast per valid_time

**Silver Table Pattern**:
```sql
CREATE TABLE silver.{domain}_forecasts (
    issue_time          TIMESTAMPTZ NOT NULL,
    valid_time          TIMESTAMPTZ NOT NULL,
    ingestion_time      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ndp_id              TEXT NOT NULL,
    -- forecast columns
    lead_hours          SMALLINT GENERATED ALWAYS AS (
        EXTRACT(EPOCH FROM valid_time - issue_time) / 3600
    ) STORED,
    PRIMARY KEY (issue_time, valid_time, ndp_id)
);

-- TimescaleDB hypertable on valid_time (for dashboard queries)
SELECT create_hypertable('silver.{domain}_forecasts', 'valid_time');
```

**Deduplication**: Keep all issue times (for accuracy analysis). Query latest issue per valid_time.

**Example Streams**:
- nws-forecast-hourly
- nws-gridpoints-forecast

---

## Config Schema Addition

### Rust Type

```rust
// core/src/config/stream_type.rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StreamType {
    #[default]
    Observations,
    Events,
    Forecasts,
}

impl StreamType {
    /// Default primary key columns for this stream type
    pub fn default_pk_columns(&self) -> Vec<&'static str> {
        match self {
            StreamType::Observations => vec!["observation_time", "ndp_id"],
            StreamType::Events => vec!["event_time", "ndp_id", "event_type"],
            StreamType::Forecasts => vec!["issue_time", "valid_time", "ndp_id"],
        }
    }

    /// TimescaleDB time column for hypertable
    pub fn time_column(&self) -> &'static str {
        match self {
            StreamType::Observations => "observation_time",
            StreamType::Events => "event_time",
            StreamType::Forecasts => "valid_time",
        }
    }

    /// Suggested deduplication strategy
    pub fn default_dedup_strategy(&self) -> DeduplicationStrategy {
        match self {
            StreamType::Observations => DeduplicationStrategy::Upsert,
            StreamType::Events => DeduplicationStrategy::Skip,
            StreamType::Forecasts => DeduplicationStrategy::Replace,
        }
    }
}
```

### StreamConfig Extension

```rust
// core/src/config/stream.rs

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StreamConfig {
    pub stream_id: String,

    #[serde(default)]
    pub stream_type: StreamType,  // NEW FIELD

    pub description: Option<String>,
    pub version: Option<String>,
    pub enabled: bool,
    // ... existing fields
}
```

---

## ETL Behavior by Type

### Observations ETL

```sql
-- Upsert pattern for observations
INSERT INTO silver.air_quality_observations (...)
SELECT ...
FROM bronze_data
ON CONFLICT (observation_time, ndp_id) DO UPDATE SET
    temperature_c = EXCLUDED.temperature_c,
    -- ... other columns
    ingestion_time = EXCLUDED.ingestion_time;
```

### Events ETL

```sql
-- Deduplicate within time window, keep first
WITH deduplicated AS (
    SELECT *,
        ROW_NUMBER() OVER (
            PARTITION BY ndp_id, event_type,
                         time_bucket('5 seconds', event_time)
            ORDER BY event_time
        ) AS rn
    FROM bronze_data
)
INSERT INTO silver.home_events (...)
SELECT ... FROM deduplicated WHERE rn = 1
ON CONFLICT (event_time, ndp_id, event_type) DO NOTHING;
```

### Forecasts ETL

```sql
-- Insert all forecasts (keep history)
INSERT INTO silver.weather_forecasts (...)
SELECT ...
FROM bronze_data
ON CONFLICT (issue_time, valid_time, ndp_id) DO NOTHING;

-- Query for latest forecast uses:
-- WHERE issue_time = (SELECT MAX(issue_time) FROM ... WHERE valid_time = target)
```

---

## Migration Path

### Existing Streams

No change required. Default `stream_type: observations` matches current behavior.

### Adding New Events Stream

```yaml
# config/base/streams/home-assistant-states/config.yaml
stream_id: home-assistant-states
stream_type: events  # Explicit type
description: "Home Assistant entity state changes"

silver_etl:
  enabled: true
  target_table: silver.home_events

  # Event-specific config
  deduplication:
    enabled: true
    window: 5s
    strategy: skip  # Informed by stream_type
```

---

## Alternatives Considered

### Alternative 1: Implicit from Config

**Description**: Infer type from config structure (presence of `event_type` mapping implies events).

**Rejected because**: Implicit behavior is error-prone. Explicit is clearer for operators and documentation.

### Alternative 2: Metadata Only (No Behavior Change)

**Description**: Add field as documentation only, no ETL impact.

**Rejected because**: Loses value. Type should inform ETL decisions automatically.

### Alternative 3: Detailed Type Hierarchy

**Description**: More granular types like `time_series_regular`, `time_series_irregular`, `event_sparse`, etc.

**Rejected because**: Over-complicated. Three types cover 95% of use cases. Can extend later if needed.

---

## References

1. SCOPE.md: "Observations vs Events" principle
2. Pattern: `arch-bronze-schema` - Bronze schema handles all types uniformly
3. Research: `research/agenticdataplatform/silver/01-scope-definition.md`
4. Research: `research/agenticdataplatform/silver/09-etl-genericity-assessment.md`
5. Home Assistant state history: https://www.home-assistant.io/docs/configuration/state_object/

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-10 | NDP Architect | Initial decision |
