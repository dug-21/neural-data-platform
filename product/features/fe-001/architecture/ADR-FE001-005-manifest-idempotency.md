# ADR-FE001-005: Manifest-Declared Idempotency

**Status**: Accepted
**Date**: 2026-02-04
**Decision Makers**: NDP Architecture Team
**Feature**: FE-001 Gold Layer Foundation
**Parent ADRs**: ADR-016-002 (Declarative Deploy), ADR-FE001-001 (Gold DDL Rust)

---

## Context

### The Problem

TimescaleDB continuous aggregates need idempotent deployment:
- `CREATE MATERIALIZED VIEW` fails if view already exists
- No `CREATE OR REPLACE` for materialized views
- Continuous aggregates cannot have columns added via `ALTER TABLE`

The question: How to achieve idempotent Gold layer deployment?

### The Constraint (Decision 9)

**TimescaleDB continuous aggregates cannot be altered in place.** Adding a new metric to `gold_etl.aggregates.fields` requires:

1. DROP the existing continuous aggregate
2. CREATE new continuous aggregate with updated columns
3. Wait for refresh to repopulate data

This is a platform limitation, not a design choice.

### Two Idempotency Strategies

| Strategy | Description | Complexity |
|----------|-------------|------------|
| **Deploy-Time Detection** | At deploy time, compare config vs existing schema, decide action | Complex runtime logic |
| **Manifest-Time Declaration** | At manifest creation time, explicitly declare action | Simple, explicit |

---

## Decision

**Manifest explicitly declares `action` for each Gold table. Detection happens at manifest creation, not deploy time.**

### Manifest Format

```json
{
  "version": "1.1.0",
  "declarations": {
    "gold-tables": [
      { "stream_id": "air-quality", "action": "sync" },
      { "stream_id": "outdoor-weather", "action": "recreate" }
    ],
    "domains": [
      { "domain_id": "indoor-air-quality", "action": "sync" }
    ]
  }
}
```

### Action Semantics

| Action | When to Use | What `ndp-gold-ddl` Generates |
|--------|-------------|-------------------------------|
| `sync` | First deploy, or no config changes | Check exists, CREATE if not |
| `recreate` | ANY change to `gold_etl` config | DROP IF EXISTS, CREATE |

### Generated SQL by Action

**For `action: sync`**:

```sql
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM timescaledb_information.continuous_aggregates
        WHERE view_schema = 'gold' AND view_name = 'air_quality_hourly'
    ) THEN
        CREATE MATERIALIZED VIEW gold.air_quality_hourly
        WITH (timescaledb.continuous) AS
        SELECT
            time_bucket('1 hour', time) AS bucket,
            AVG(pm25) AS pm25_mean,
            STDDEV(pm25) AS pm25_std,
            MIN(pm25) AS pm25_min,
            MAX(pm25) AS pm25_max
        FROM silver.air_quality
        GROUP BY bucket;

        SELECT add_continuous_aggregate_policy('gold.air_quality_hourly',
            start_offset => INTERVAL '4 hours',
            end_offset => INTERVAL '15 minutes',
            schedule_interval => INTERVAL '15 minutes'
        );
    ELSE
        RAISE NOTICE 'gold.air_quality_hourly already exists, skipping';
    END IF;
END $$;
```

**For `action: recreate`**:

```sql
-- Explicitly drop and recreate
DROP MATERIALIZED VIEW IF EXISTS gold.outdoor_weather_hourly CASCADE;

CREATE MATERIALIZED VIEW gold.outdoor_weather_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', time) AS bucket,
    AVG(temp) AS temp_mean,
    AVG(humidity) AS humidity_mean,
    -- New columns added
    MIN(temp) AS temp_min,
    MAX(temp) AS temp_max
FROM silver.outdoor_weather
GROUP BY bucket;

-- Re-add policies (dropped with CASCADE)
SELECT add_continuous_aggregate_policy('gold.outdoor_weather_hourly',
    start_offset => INTERVAL '4 hours',
    end_offset => INTERVAL '15 minutes',
    schedule_interval => INTERVAL '15 minutes'
);
```

---

## Consequences

### Positive

1. **Explicit Intent** - Manifest declares exactly what should happen; no runtime guessing
2. **Predictable Behavior** - Same manifest always produces same result
3. **Simple Deploy.sh** - deploy.sh just executes what manifest declares
4. **Audit Trail** - Git history shows when recreate was used and why
5. **Review Safety** - PR reviewers can see `recreate` action and understand implications
6. **No Runtime Schema Diffing** - Avoids complex runtime comparison logic

### Negative

1. **Manual Decision Required** - Operator must know when to use `recreate`
2. **Easy to Forget** - Using `sync` when config changed leads to stale schema
3. **Documentation Burden** - Must clearly document when `recreate` is required
4. **No Automation (Initially)** - Future tooling could auto-detect changes

### Neutral

1. **Data Reprocessing** - `recreate` causes historical data to be recomputed from Silver (not lost)
2. **Downtime** - Brief gap while view is dropped and recreated
3. **Future Automation** - Can add manifest generation tool that auto-detects changes

---

## Alternatives Considered

### Alternative 1: Deploy-Time Schema Diffing

Compare deployed schema vs config at deploy time, auto-determine action.

```bash
# In deploy.sh
current_schema=$(get_view_columns gold.air_quality_hourly)
new_schema=$(ndp-gold-ddl schema --stream air-quality)
if [ "$current_schema" != "$new_schema" ]; then
    action="recreate"
else
    action="sync"
fi
```

**Rejected because:**
- Adds complexity to deploy.sh
- Requires database connection before deployment decisions
- Schema comparison logic is non-trivial (column order, types, expressions)
- Harder to audit ("why did this get recreated?")
- deploy.sh should be simple executor, not decision maker

### Alternative 2: Always Recreate

Every deployment drops and recreates all Gold views.

```json
{
  "gold-tables": [
    { "stream_id": "air-quality" }  // No action, always recreate
  ]
}
```

**Rejected because:**
- Unnecessary data reprocessing when config unchanged
- Wasteful of compute resources on Pi
- Longer deployment times
- Brief data unavailability on every deploy

### Alternative 3: Version-Based Detection

Compare version numbers in config to determine if recreate needed.

```json
{
  "gold_etl": {
    "version": "1.2.0",  // Increment when schema changes
    ...
  }
}
```

**Rejected because:**
- Adds another thing to maintain (version number)
- Easy to forget to increment version
- Doesn't provide more safety than explicit action
- Version doesn't clearly communicate "breaking change"

### Alternative 4: CREATE OR REPLACE (If Supported)

Use hypothetical `CREATE OR REPLACE MATERIALIZED VIEW`.

**Not available:**
- PostgreSQL and TimescaleDB do not support this for materialized views
- This alternative would be ideal but is not possible with current technology

---

## Implementation

### Procedural Requirement

> **ANY change to `gold_etl` config requires `action: recreate` in the manifest.**
>
> Unlike Silver tables (which support `ADD COLUMN`), Gold continuous aggregates cannot be altered in place. If you change metrics, granularities, or any `gold_etl` field, you MUST use `recreate`. Using `sync` with changed config will result in the old schema remaining in place.

### Action Selection Guide

| Scenario | Action | Why |
|----------|--------|-----|
| First deploy of Gold for stream | `sync` | Creates new aggregate |
| Re-deploy, `gold_etl` unchanged | `sync` | Idempotent, skips if exists |
| **ANY change to `gold_etl`** | **`recreate`** | **Required - cannot alter in place** |
| Add new stream with Gold | `sync` | New aggregate |
| Remove Gold from stream | `drop` (future) | Explicit removal |

### Domain Actions

Domains also support actions:

| Action | When to Use |
|--------|-------------|
| `sync` | First deploy or no alignment changes |
| `recreate` | Alignment strategy changed, streams added/removed |

```json
{
  "domains": [
    { "domain_id": "indoor-air-quality", "action": "sync" },
    { "domain_id": "energy-efficiency", "action": "recreate" }
  ]
}
```

### Future Automation

A future manifest generation tool could:

1. Load current deployed state from etcd
2. Compare with new config
3. Auto-set action based on diff
4. Generate manifest with correct actions

```bash
# Future tool (not in FE-001 scope)
ndp-manifest generate --compare-deployed > manifest.json
# Outputs manifest with action: recreate where config changed
```

### deploy.sh Integration

```bash
handle_gold_table() {
    local declaration="$1"
    local stream_id=$(echo "$declaration" | jq -r '.stream_id')
    local action=$(echo "$declaration" | jq -r '.action // "sync"')

    log "Gold Table: $stream_id (action=$action)"

    # Rust tool generates SQL based on action
    local ddl=$(ndp-gold-ddl generate --stream "$stream_id" --action "$action" 2>&1)
    if [ $? -ne 0 ]; then
        error "Gold DDL generation failed: $ddl"
        return 1
    fi

    log "  Applying Gold DDL to TimescaleDB..."
    echo "$ddl" | dcx timescaledb psql -U postgres -d ndp
}
```

---

## Related Decisions

- **Decision 9 (DECISIONS.md)**: Gold Schema Evolution Requires DROP/RECREATE - constraint this decision addresses
- **Decision 11 (DECISIONS.md)**: Idempotency via Manifest-Declared Actions - source decision
- **ADR-FE001-001**: Gold DDL Generation in Rust - tool that implements this
- **ADR-016-002**: Declarative Deploy - manifest-based deployment pattern

---

## References

- `/workspaces/neural-data-platform/product/features/fe-001/architecture/DECISIONS.md` - Source decisions (9, 11)
- `/workspaces/neural-data-platform/product/features/fe-001/architecture/CONFIG-DEPLOYMENT-FLOW.md` - Deployment integration
- `/workspaces/neural-data-platform/docs/procedures/DEPLOYMENT-DECLARATIVES.md` - Manifest format
- TimescaleDB documentation: Continuous aggregate limitations

---

*Architecture decision created: 2026-02-04*
*Feature: FE-001 Gold Layer Foundation*
