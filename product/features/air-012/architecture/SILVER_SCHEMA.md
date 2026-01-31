# air-012: Silver Layer Schema Design

## Overview

This document specifies the Silver layer schema for Home Assistant state events (window/door sensors). The design follows established NDP patterns from `001_silver_schema.sql` while adapting for the unique characteristics of state event data.

---

## Schema: silver.state_events

### DDL

```sql
-- =============================================================================
-- Silver Layer: State Events from Home Assistant
-- =============================================================================
-- Feature: air-012 - Home Assistant Integration
-- Source: Bronze home-assistant-state stream (MQTT)
-- Grain: One row per state change event
-- Use: Event log for window/door state, foundation for dp-014 SCD Gold layer
-- =============================================================================

CREATE TABLE silver.state_events (
    -- Audit/Metadata
    ingestion_time      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    event_time          TIMESTAMPTZ NOT NULL,  -- When NDP received the MQTT message
    source_stream       TEXT NOT NULL DEFAULT 'home-assistant-state',

    -- Identity
    ndp_id              TEXT NOT NULL,         -- 'door_backslider', 'door_officewindow', etc.
    source_entity_id    TEXT,                  -- 'binary_sensor.door_backslider' (from MQTT topic)

    -- State
    state               TEXT NOT NULL,         -- 'on' (open) / 'off' (closed)

    -- DQ Transparency
    dq_flags            TEXT[],                -- Array of rule violations

    -- Primary Key (matches existing Silver tables pattern)
    PRIMARY KEY (event_time, ndp_id)
);

-- Comments
COMMENT ON TABLE silver.state_events IS
    'State change events from Home Assistant binary sensors.
     Source: home-assistant-state Bronze stream (MQTT).
     Grain: One row per state change event (sparse - only fires on change).
     Use: Window/door state tracking, foundation for dp-014 SCD Gold layer.
     Note: SCD semantics (valid_from/valid_to) computed in Gold layer.';

COMMENT ON COLUMN silver.state_events.event_time IS
    'When NDP received the MQTT message. MQTT latency typically <100ms.';
COMMENT ON COLUMN silver.state_events.state IS
    'Binary state: "on" = open, "off" = closed (Home Assistant convention).';
COMMENT ON COLUMN silver.state_events.source_entity_id IS
    'Full Home Assistant entity ID extracted from MQTT topic path.';
```

### Hypertable Configuration

```sql
-- Convert to hypertable with 1-day chunks (consistent with other Silver tables)
SELECT create_hypertable('silver.state_events',
    'event_time',
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE
);
```

**Design Rationale:**
- 1-day chunks match existing Silver tables (Pi memory constraint ~256MB)
- `event_time` as partition column enables efficient time-range queries
- Sparse data (events only on state change) means chunks will be small

---

## Indexes

```sql
-- Primary query pattern: Latest state per entity
CREATE INDEX IF NOT EXISTS idx_state_events_ndp_id
    ON silver.state_events (ndp_id, event_time DESC);

-- Query pattern: Events with DQ issues
CREATE INDEX IF NOT EXISTS idx_state_events_dq_flags
    ON silver.state_events USING GIN (dq_flags)
    WHERE dq_flags IS NOT NULL AND array_length(dq_flags, 1) > 0;

-- Query pattern: Events by source entity (for HA debugging)
CREATE INDEX IF NOT EXISTS idx_state_events_source_entity
    ON silver.state_events (source_entity_id, event_time DESC);
```

**Index Strategy:**
| Index | Pattern | Use Case |
|-------|---------|----------|
| `idx_state_events_ndp_id` | `(ndp_id, event_time DESC)` | Dashboard: latest state per sensor |
| `idx_state_events_dq_flags` | GIN on `dq_flags` | DQ transparency queries |
| `idx_state_events_source_entity` | `(source_entity_id, event_time DESC)` | Debugging MQTT topic issues |

---

## Compression Policy

```sql
-- Enable compression for state events
ALTER TABLE silver.state_events SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'ndp_id',
    timescaledb.compress_orderby = 'event_time DESC'
);

-- Compress after 7 days (consistent with other Silver tables)
SELECT add_compression_policy('silver.state_events',
    INTERVAL '7 days',
    if_not_exists => TRUE
);
```

**Compression Notes:**
- State events compress well due to repeated `state` values ('on'/'off')
- Segment by `ndp_id` enables efficient per-sensor queries on compressed data
- Expected compression ratio: 10-20x for text columns

---

## Retention Policy

```sql
-- Keep raw Silver data for 90 days (can be rebuilt from Bronze)
SELECT add_retention_policy('silver.state_events',
    INTERVAL '90 days',
    if_not_exists => TRUE
);
```

**Retention Rationale:**
| Layer | Retention | Rationale |
|-------|-----------|-----------|
| Bronze (Parquet) | 365 days | Audit trail, full history |
| Silver (state_events) | 90 days | Queryable window, rebuilds from Bronze |
| Gold (dp-014 SCD view) | Indefinite | Computed from Silver, small volume |

---

## Comparison with air_quality_observations

| Aspect | air_quality_observations | state_events |
|--------|--------------------------|--------------|
| **Grain** | One row per reading (~1 min) | One row per state change (sparse) |
| **Data Volume** | ~1440 rows/day/sensor | ~5-20 rows/day/sensor |
| **Payload Complexity** | 10+ numeric fields | 1 text field (state) |
| **Timestamp Source** | Sensor timestamp | Ingestion time |
| **Primary Key** | `(observation_time, ndp_id)` | `(event_time, ndp_id)` |
| **Chunk Interval** | 1 day | 1 day |
| **Continuous Aggregates** | Hourly/daily | Not needed (sparse data) |
| **DQ Rules** | Range checks, rate-of-change | Value validation, gap detection |

### Why No Continuous Aggregates

State events are fundamentally different from time-series observations:
1. **Sparse**: Events only fire on change (not periodic)
2. **Categorical**: State is 'on'/'off', not numeric (no averages)
3. **SCD Pattern**: Meaningful aggregation is "time in state" (computed in Gold)

The dp-014 Gold layer will create materialized views for:
- `gold.state_periods` - SCD with `valid_from`/`valid_to`
- Time-in-state calculations
- Point-in-time joins with air quality data

---

## Migration Script Location

The DDL should be added to a new init script:

```
deploy/timescaledb/init/002_state_events_schema.sql
```

Or appended to `001_silver_schema.sql` in a new section:

```sql
-- =============================================================================
-- SECTION 12: silver.state_events (air-012)
-- =============================================================================
-- ... (DDL from above)
```

---

## Related Documents

- `SCOPE.md` - Feature requirements and acceptance criteria
- `SILVER_ETL_CONFIG.md` - Stream configuration for Bronze-to-Silver ETL
- `dp-014` - Config-driven Gold layer (will consume this table)
- `001_silver_schema.sql` - Existing Silver layer patterns
