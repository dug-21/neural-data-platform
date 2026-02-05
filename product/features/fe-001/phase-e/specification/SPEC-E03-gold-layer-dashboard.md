# SPEC-E03: Gold Layer Dashboard (v11-014)

> **Created:** 2026-02-05
> **Phase:** E (Unified Event Abstraction)
> **Feature ID:** v11-014
> **Priority:** High
> **Status:** Specification

---

## Executive Summary

Create a Grafana dashboard providing visibility into all Gold layer tables, including the unified events view built in Phase E. This dashboard serves as:
1. **Operational visibility** - See Gold layer data flowing correctly
2. **Architecture validation** - Prove the config-driven Gold layer works
3. **V1.2 foundation** - Dashboard patterns ready for correlation visualization

---

## User Stories

### US-E03-01: View Gold Layer Aggregates
**As a** platform operator
**I want** to see Gold layer continuous aggregates in Grafana
**So that** I can verify data is flowing through the Gold layer correctly

### US-E03-02: Monitor Unified Events
**As a** platform operator
**I want** to see state transitions and threshold crossings on a timeline
**So that** I can understand event patterns and validate Phase E functionality

### US-E03-03: View Objective Status
**As a** homeowner
**I want** to see whether my air quality objectives are being met
**So that** I can take action when thresholds are crossed

### US-E03-04: Explore Aligned Data
**As a** data analyst
**I want** to see all streams aligned on a single time axis
**So that** I can visually identify potential correlations

---

## Dashboard Design

### Dashboard: Gold Layer Overview

**Location:** `config/grafana/dashboards/gold-layer-overview.json`

#### Row 1: Air Quality Metrics (Gold Aggregates)

| Panel | Query Source | Visualization | Description |
|-------|-------------|---------------|-------------|
| **Indoor PM2.5** | `gold.air_quality_hourly` | Time series | pm25_mean with threshold line at 12 |
| **Indoor CO2** | `gold.air_quality_hourly` | Time series | co2_mean with threshold line at 800 |
| **Temperature & Humidity** | `gold.air_quality_hourly` | Dual-axis time series | temp_mean, humidity_mean |
| **Sample Quality** | `gold.air_quality_hourly` | Stat/Gauge | sample_count, data completeness % |

#### Row 2: Cross-Stream Alignment

| Panel | Query Source | Visualization | Description |
|-------|-------------|---------------|-------------|
| **Aligned Streams** | `gold.indoor_air_quality_aligned` | Time series (stacked) | Indoor + outdoor + weather overlaid |
| **Indoor vs Outdoor PM2.5** | `gold.indoor_air_quality_aligned` | Dual-axis time series | Compare indoor/outdoor pollution |
| **Weather Context** | `gold.outdoor_weather_hourly` | Time series | Temperature, humidity, pressure |
| **State Timeline** | `gold.home_assistant_state_hourly` | State timeline | Binary state visualization |

#### Row 3: Events & Objectives

| Panel | Query Source | Visualization | Description |
|-------|-------------|---------------|-------------|
| **Unified Events Timeline** | `gold.events_unified` | Annotations + bars | State transitions + threshold crossings |
| **Event Counts (Hourly)** | `gold.events_hourly` | Bar chart | Event counts by type |
| **Objective Gauges** | `gold.air_quality_hourly` | Gauge panel | Current values vs thresholds |
| **Threshold Crossings Log** | `gold.events_unified` | Table | Recent crossing events with details |

#### Row 4: Data Quality & Volume

| Panel | Query Source | Visualization | Description |
|-------|-------------|---------------|-------------|
| **Sample Counts** | `gold.*_hourly` | Bar chart | Samples per stream per hour |
| **Event Volume** | `gold.events_hourly` | Time series | Events over time |
| **Gold Layer Storage** | System query | Stat | Total size of Gold tables |
| **Refresh Status** | `timescaledb_information.jobs` | Table | CA refresh job status |

---

## SQL Queries for Dashboard

### Indoor Air Quality Time Series
```sql
SELECT
    bucket AS time,
    pm25_mean AS "PM2.5",
    co2_mean AS "CO2",
    temp_mean AS "Temperature",
    humidity_mean AS "Humidity"
FROM gold.air_quality_hourly
WHERE bucket >= $__timeFrom() AND bucket <= $__timeTo()
ORDER BY bucket;
```

### Aligned Multi-Stream View
```sql
SELECT
    bucket AS time,
    indoor_pm25_mean AS "Indoor PM2.5",
    outdoor_pm25_mean AS "Outdoor PM2.5",
    outdoor_temp_mean AS "Outdoor Temp",
    indoor_co2_mean / 100 AS "CO2 (÷100)"  -- Scale for overlay
FROM gold.indoor_air_quality_aligned
WHERE bucket >= $__timeFrom() AND bucket <= $__timeTo()
ORDER BY bucket;
```

### Unified Events for Annotations
```sql
SELECT
    event_time AS time,
    event_type AS title,
    CASE event_type
        WHEN 'state_transition' THEN
            details->>'from_state' || ' → ' || details->>'to_state'
        WHEN 'threshold_crossing' THEN
            details->>'metric' || ' ' || details->>'direction' || ' ' ||
            (details->>'threshold')::text
    END AS text,
    CASE event_type
        WHEN 'state_transition' THEN 'State'
        WHEN 'threshold_crossing' THEN 'Threshold'
    END AS tags
FROM gold.events_unified
WHERE event_time >= $__timeFrom() AND event_time <= $__timeTo()
ORDER BY event_time;
```

### Event Counts by Hour
```sql
SELECT
    bucket AS time,
    state_transition_count AS "State Transitions",
    threshold_crossing_count AS "Threshold Crossings",
    total_events AS "Total Events"
FROM gold.events_hourly
WHERE bucket >= $__timeFrom() AND bucket <= $__timeTo()
ORDER BY bucket;
```

### Objective Status Gauge
```sql
SELECT
    bucket AS time,
    co2_mean AS "CO2 ppm",
    pm25_mean AS "PM2.5 µg/m³"
FROM gold.air_quality_hourly
ORDER BY bucket DESC
LIMIT 1;
-- Gauge thresholds: CO2 < 800 (green), PM2.5 < 12 (green)
```

### Threshold Crossings Table
```sql
SELECT
    event_time AS "Time",
    details->>'metric' AS "Metric",
    details->>'threshold' AS "Threshold",
    details->>'direction' AS "Direction",
    details->>'value' AS "Value",
    details->>'objective_id' AS "Objective"
FROM gold.events_unified
WHERE event_type = 'threshold_crossing'
    AND event_time >= $__timeFrom()
ORDER BY event_time DESC
LIMIT 20;
```

### State Timeline
```sql
SELECT
    bucket AS time,
    last_state AS state,
    transitions_to_on AS "Opens",
    transitions_to_off AS "Closes"
FROM gold.home_assistant_state_hourly
WHERE bucket >= $__timeFrom() AND bucket <= $__timeTo()
ORDER BY bucket;
```

### Continuous Aggregate Refresh Status
```sql
SELECT
    j.config->>'hypertable_id' AS "Table",
    j.schedule_interval AS "Interval",
    js.last_run_status AS "Last Status",
    js.last_successful_finish AS "Last Success",
    js.total_runs AS "Total Runs"
FROM timescaledb_information.job_stats js
JOIN timescaledb_information.jobs j ON js.job_id = j.job_id
WHERE j.proc_name = 'policy_refresh_continuous_aggregate';
```

---

## Dashboard Variables

| Variable | Type | Query | Description |
|----------|------|-------|-------------|
| `stream` | Query | `SELECT DISTINCT stream_id FROM gold.events_unified` | Filter by stream |
| `entity` | Query | `SELECT DISTINCT entity_id FROM gold.events_unified WHERE stream_id = '$stream'` | Filter by entity |
| `event_type` | Custom | `state_transition, threshold_crossing` | Filter events |
| `objective` | Query | `SELECT DISTINCT details->>'objective_id' FROM gold.events_unified WHERE event_type = 'threshold_crossing'` | Filter by objective |

---

## Dashboard Configuration

### Data Source Configuration

The dashboard requires a PostgreSQL/TimescaleDB data source configured in Grafana:

```yaml
# config/grafana/provisioning/datasources/ndp-timescaledb.yaml
apiVersion: 1
datasources:
  - name: NDP-TimescaleDB
    type: postgres
    url: timescaledb:5432
    database: ndp
    user: grafana_reader
    secureJsonData:
      password: ${GRAFANA_DB_PASSWORD}
    jsonData:
      sslmode: disable
      maxOpenConns: 5
      maxIdleConns: 2
      connMaxLifetime: 14400
      postgresVersion: 1500
      timescaledb: true
```

### Dashboard Provisioning

```yaml
# config/grafana/provisioning/dashboards/ndp.yaml
apiVersion: 1
providers:
  - name: 'NDP Dashboards'
    orgId: 1
    folder: 'Neural Data Platform'
    type: file
    disableDeletion: false
    updateIntervalSeconds: 30
    options:
      path: /etc/grafana/provisioning/dashboards/ndp
```

---

## Threshold Annotations

Objective thresholds should be visualized as threshold lines on relevant panels:

| Metric | Threshold | Color | Source |
|--------|-----------|-------|--------|
| CO2 | 800 ppm | Red | healthy_co2 objective |
| PM2.5 | 12 µg/m³ | Red | healthy_pm25 objective |
| Humidity | 40-60% | Yellow band | comfortable_humidity objective |

These thresholds come from `config/domains/indoor-air-quality/domain.yaml`.

---

## Implementation Approach

### Option A: JSON Dashboard Export (Recommended)

1. Build dashboard manually in Grafana UI
2. Export as JSON
3. Store in `config/grafana/dashboards/gold-layer-overview.json`
4. Provision via Grafana provisioning

**Pros:** Visual editor, full Grafana features, portable
**Cons:** Large JSON file, manual updates

### Option B: Grafana Dashboard-as-Code

1. Use Grafana Terraform provider or Grafonnet
2. Generate dashboard from code

**Pros:** Version-controlled, reviewable
**Cons:** Learning curve, additional tooling

**Decision:** Use Option A (JSON export) for V1.1. Consider Option B for V1.2 if dashboards proliferate.

---

## Deployment

### Manifest Entry

```json
{
  "grafana-dashboard": {
    "gold-layer-overview": {
      "type": "grafana-dashboard",
      "path": "config/grafana/dashboards/gold-layer-overview.json",
      "folder": "Neural Data Platform",
      "datasource": "NDP-TimescaleDB"
    }
  }
}
```

### deploy.sh Handler

```bash
handle_grafana_dashboard() {
    local dashboard_id="$1"
    local dashboard_path="$2"

    # Validate JSON
    if ! jq empty "$dashboard_path" 2>/dev/null; then
        log_error "Invalid JSON: $dashboard_path"
        return 1
    fi

    # Import to Grafana via API
    curl -X POST \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer ${GRAFANA_API_KEY}" \
        -d @"$dashboard_path" \
        "http://localhost:3000/api/dashboards/db"
}
```

---

## Acceptance Criteria

### AC-E03-01: Dashboard Loads Successfully
- [ ] Dashboard imports without errors
- [ ] All panels render without "No Data" or errors
- [ ] Variables populate correctly

### AC-E03-02: Gold Aggregates Visible
- [ ] PM2.5 time series displays from gold.air_quality_hourly
- [ ] CO2 time series displays from gold.air_quality_hourly
- [ ] Outdoor weather displays from gold.outdoor_weather_hourly
- [ ] State events display from gold.home_assistant_state_hourly

### AC-E03-03: Aligned View Works
- [ ] Multi-stream alignment panel shows all 4 streams
- [ ] Time alignment is visually correct
- [ ] NULL values handled gracefully (gaps in lines)

### AC-E03-04: Unified Events Visible
- [ ] State transitions display on timeline
- [ ] Threshold crossings display on timeline
- [ ] Event table shows recent crossings with details
- [ ] Events filter by type works

### AC-E03-05: Objectives Visualization
- [ ] Threshold lines display on relevant panels
- [ ] Gauges show current values vs thresholds
- [ ] Color coding: green (good), yellow (warning), red (exceeded)

### AC-E03-06: Performance
- [ ] Dashboard loads in < 3 seconds
- [ ] 30-day range query completes in < 5 seconds
- [ ] No timeout errors on Pi 5

---

## File Inventory

### New Files
```
config/grafana/dashboards/gold-layer-overview.json
config/grafana/provisioning/datasources/ndp-timescaledb.yaml
config/grafana/provisioning/dashboards/ndp.yaml
```

### Modified Files
```
.deploy/releases/v1.1.6.manifest.json   # Add dashboard entry
deploy/pi/deploy.sh                      # Add handle_grafana_dashboard()
```

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| gold.air_quality_hourly | ✅ Deployed | Phase B |
| gold.outdoor_weather_hourly | ✅ Deployed | Phase D |
| gold.home_assistant_state_hourly | ✅ Deployed | Phase D |
| gold.indoor_air_quality_aligned | ✅ Deployed | Phase D |
| gold.events_unified | Phase E | v11-013 |
| gold.events_hourly | Phase E | v11-013 |
| Grafana container | ✅ Available | Docker stack |
| TimescaleDB datasource | To configure | Part of this feature |

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Dashboard JSON too large | Low | Low | Use panel references, avoid duplication |
| Grafana version mismatch | Low | Medium | Pin Grafana version, test upgrades |
| Query performance on Pi | Medium | Medium | Use continuous aggregates, add indexes |
| TimescaleDB features not in Grafana | Low | Low | Use raw SQL, not Grafana query builder |

---

## References

- [Grafana Dashboard JSON Model](https://grafana.com/docs/grafana/latest/dashboards/json-model/)
- [TimescaleDB Grafana Integration](https://docs.timescale.com/tutorials/latest/grafana/)
- [PHASE-E-OVERVIEW.md](./PHASE-E-OVERVIEW.md)
- [ACCEPTANCE-CRITERIA.md](../completion/ACCEPTANCE-CRITERIA.md)

---

*Specification created: 2026-02-05 by Claude*
