# Grafana Dashboard Queries

This directory contains SQL queries for Grafana dashboards monitoring the Neural Data Platform Silver layer.

## Query Files

### `state_events_health.sql`

Pipeline health queries specifically for the `silver.state_events` table (air-012: Home Assistant Integration).

**Important:** State events are **sparse data** - events only fire on state CHANGE, not at regular intervals. A window that stays closed for 2 days is normal behavior, not a pipeline failure.

**Sparse Data Thresholds:**
| Status | Threshold | Meaning |
|--------|-----------|---------|
| FRESH | < 18 hours | Recent state change detected |
| STALE | 18-36 hours | No recent changes, but within normal range |
| CRITICAL | > 36 hours | May indicate sensor disconnect |

**Queries Included:**
1. **Health Summary** - Per-entity status table
2. **Overall Health** - Aggregate worst-case status
3. **Freshness Gauge** - Seconds since last event (for gauge panel)
4. **Record Count** - Event volume (expected to be low)
5. **Recent Events** - Debugging table with entity context

### `unified_pipeline_health.sql`

Combined pipeline health view for ALL Silver layer streams with stream-type-appropriate thresholds.

**Stream Types:**

| Stream | Type | Thresholds (warn/crit) | Notes |
|--------|------|------------------------|-------|
| air_quality_observations | Regular | 90s / 180s | Indoor AQ sensors, ~30s intervals |
| weather_observations | Regular | 20m / 40m | NWS stations, ~10m intervals |
| outdoor_air_quality | Regular | 20m / 40m | OWM API, ~10m intervals |
| weather_forecasts | Regular | 2h / 4h | NWS gridpoints, ~1h intervals |
| state_events | Sparse | 18h / 36h | Home Assistant binary sensors |

**Queries Included:**
1. **Unified Status Grid** - All streams in one table with appropriate thresholds
2. **Aggregate Summary** - Overall health counts for stat panels

## Usage in Grafana

### Data Source

All queries target TimescaleDB with the `timescaledb-silver` datasource UID.

### Panel Types

| Query | Recommended Panel |
|-------|-------------------|
| Health Summary | Table |
| Overall Health | Stat |
| Freshness Gauge | Gauge |
| Record Count | Stat or Bar |
| Recent Events | Table |
| Unified Grid | Table |
| Aggregate Summary | Stat (multiple) |

### Status Color Mapping

Configure panel overrides for the "Status" column:
```json
{
  "mappings": [
    { "type": "value", "options": { "HEALTHY": { "color": "green" }, "FRESH": { "color": "green" } } },
    { "type": "value", "options": { "WARNING": { "color": "yellow" }, "STALE": { "color": "yellow" } } },
    { "type": "value", "options": { "CRITICAL": { "color": "red" } } },
    { "type": "value", "options": { "No Data": { "color": "dark-red" } } }
  ]
}
```

## Related Files

- Dashboard JSON: `/config/grafana/dashboards/pipeline-health.json`
- Silver Schema: `/deploy/timescaledb/init/001_silver_schema.sql`
- State Events Schema: `/deploy/timescaledb/init/002_state_events_schema.sql`
- Specification: `/product/features/air-012/specification/SPECIFICATION.md`
