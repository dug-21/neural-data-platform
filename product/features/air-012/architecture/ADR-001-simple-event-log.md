# ADR-001: Simple Event Log in Silver Layer

## Status

**Accepted** (2026-01-30)

## Context

air-012 integrates Home Assistant window/door state events into NDP. The original architecture (documented in DATA_MODEL.md) proposed a comprehensive Silver layer with:

- SCD semantics (valid_from/valid_to)
- Computed `previous_state` columns
- Current state materialized views
- Point-in-time query functions

This design front-loads complexity in the Silver layer. During scope review (2026-01-30), we recognized:

1. **Silver is for cleaned facts** - The layer's purpose is data cleaning and normalization, not feature computation
2. **Gold computes features** - SCD semantics are features, not raw facts
3. **Iteration risk** - Baking SCD logic into Silver schema makes iteration expensive (requires migrations)
4. **dp-014 planned** - A Config-Driven Gold Layer feature is on the roadmap

## Decision

**Implement a simple event log in Silver. Defer SCD semantics to Gold layer (dp-014).**

### Silver Schema (Implemented)

```sql
CREATE TABLE silver.state_events (
    event_time       TIMESTAMPTZ NOT NULL,  -- Ingestion time
    ndp_id           TEXT NOT NULL,         -- 'door_backslider'
    source_entity_id TEXT,                  -- 'binary_sensor.door_backslider'
    state            TEXT NOT NULL,         -- 'on' (open) / 'off' (closed)
    dq_flags         TEXT[],
    PRIMARY KEY (event_time, ndp_id)
);

SELECT create_hypertable('silver.state_events', 'event_time');
```

### Design Principles

| Principle | Application |
|-----------|-------------|
| Silver stores facts | Raw state events with ingestion timestamp |
| No denormalization | No `category` column - JOIN with dimension table |
| No computed columns | No `previous_state` - compute in Gold if needed |
| Minimal schema | Only essential columns for event log |

### Metadata via Dimension JOIN

Entity metadata (category, friendly_name, location, correlates_with) comes from `entity_context` dimension table:

```sql
SELECT e.event_time, e.ndp_id, e.state,
       ctx.category, ctx.friendly_name, ctx.location_path
FROM silver.state_events e
JOIN silver.entity_context ctx ON e.ndp_id = ctx.ndp_id
WHERE e.ndp_id = 'door_backslider';
```

### Future Gold Layer (dp-014)

SCD semantics will be computed as a materialized view in Gold:

```sql
-- Config-driven in dp-014
CREATE MATERIALIZED VIEW gold.state_periods AS
SELECT
    ndp_id,
    state,
    event_time AS valid_from,
    LEAD(event_time) OVER (PARTITION BY ndp_id ORDER BY event_time) AS valid_to
FROM silver.state_events;
```

This enables:
- Point-in-time queries: "What was window state at 2pm?"
- Duration calculations: "How long was window open?"
- ML feature engineering: "State at time of each air quality reading"

## Consequences

### Positive

- **Fast implementation** - Days instead of weeks
- **Schema flexibility** - Iterate on SCD logic without migrations
- **Separation of concerns** - Silver = facts, Gold = features
- **Consistent with NDP patterns** - Bronze (raw) -> Silver (clean) -> Gold (features)
- **Lower cognitive load** - Simple schema is easier to understand and debug

### Negative

- **No point-in-time queries until dp-014** - Cannot ask "what was state at X?"
- **Duration queries require window functions** - Not pre-computed
- **Two features to complete full functionality** - air-012 + dp-014

### Mitigations

| Concern | Mitigation |
|---------|------------|
| Point-in-time queries | Create temporary view if urgently needed before dp-014 |
| Query complexity | Document common query patterns in PROCEDURES |
| Gold layer delay | dp-014 scope created, ready to start after air-012 |

## Alternatives Considered

### 1. Full SCD in Silver (Original Design)

**Rejected because:**
- Over-engineering for MVP
- Violates Silver = facts principle
- Makes iteration expensive

### 2. Compute previous_state in ETL

**Rejected because:**
- Adds ETL complexity
- Requires stateful processing
- Can be computed in Gold with window functions

### 3. Store duration in Silver

**Rejected because:**
- Duration is a derived feature, not a fact
- Requires update of previous row (anti-pattern)
- Better computed in Gold with LEAD() function

## Related

- **SCOPE.md** - Barebones scope definition
- **dp-013** - Dimension table infrastructure (provides entity_context)
- **dp-014** - Config-Driven Gold Layer (will add SCD semantics)
- **DATA_MODEL.md** - Original comprehensive design (superseded for Silver, reference for Gold)

---

**Decision Date:** 2026-01-30
**Participants:** User, NDP Architect
