# Silver Layer Research Synthesis

**Date**: 2026-01-05
**Status**: Complete
**Research Swarm**: Silver Layer Scope Refinement

---

## Executive Summary

This document synthesizes findings from four parallel research agents investigating the Silver layer design for the Neural Data Platform (NDP). The research confirms that a TimescaleDB-based Silver layer with DuckDB ETL is the optimal approach for Raspberry Pi 5 deployment.

### Key Decisions

| Decision Area | Recommendation | Confidence |
|--------------|----------------|------------|
| **Silver Scope** | 4 domain tables (indoor_air_quality, outdoor_weather, outdoor_air_quality, weather_forecast) | High |
| **ETL Approach** | DuckDB-based with hourly systemd timer | High |
| **Schema Design** | Denormalized hypertables with SI units | High |
| **Dashboard Migration** | Direct SQL translation with continuous aggregates | Medium |

### Resource Budget (Pi 5 16GB)

| Component | Memory | Status |
|-----------|--------|--------|
| Existing NDP services | ~750MB | Deployed |
| TimescaleDB | 256MB | Proposed |
| DuckDB ETL (peak) | 200MB | Proposed |
| **Total** | **~1.2GB** | **7.5% of 16GB** |

---

## 1. Silver Layer Scope

### Recommended Entities

| Entity | Source Streams | Update Frequency | Records/Day |
|--------|---------------|------------------|-------------|
| `silver.indoor_air_quality` | air-quality | ~1 min | 1,440 |
| `silver.outdoor_weather` | outdoor-weather, nws-observations | ~10 min | 288 |
| `silver.outdoor_air_quality` | outdoor-air-quality | ~10 min | 144 |
| `silver.weather_forecast` | nws-forecast-hourly, nws-gridpoints-forecast | ~1 hour | 22,464 |

### Stream-to-Entity Mapping

```
Bronze (7 streams)                 Silver (4 tables)
─────────────────                  ─────────────────
air-quality          ────────────► indoor_air_quality (1:1)

outdoor-weather      ──┬─────────► outdoor_weather (N:1)
nws-observations     ──┤           (source_provider column)
nws-station-obs      ──┘

outdoor-air-quality  ────────────► outdoor_air_quality (1:1)

nws-forecast-hourly  ──┬─────────► weather_forecast (1:N)
nws-gridpoints       ──┘           (explode 156 periods)
```

### Design Philosophy

**Denormalized wide tables** - Time-series data is append-only with read-heavy workloads. Wide tables optimize for:
- Query locality (all metrics in one row)
- TimescaleDB continuous aggregates
- Grafana dashboard performance
- ML feature extraction

---

## 2. ETL Approach: DuckDB with Systemd Timer

### Recommendation Rationale

| Factor | DuckDB ETL | Rust Native | Python Polars | pg_parquet FDW |
|--------|------------|-------------|---------------|----------------|
| Implementation | ~6 hours | ~28 hours | ~10 hours | ~8 hours |
| Memory (peak) | 200MB | 110MB | 300MB | 50MB |
| Performance | Excellent | Good | Good | Fair |
| Maintenance | Low | High | Medium | Low |
| **Decision** | **SELECTED** | Fallback | - | - |

### DuckDB ETL Architecture

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  Bronze Layer   │     │   DuckDB ETL    │     │  Silver Layer   │
│  (Parquet)      │────▶│  (SQL script)   │────▶│  (TimescaleDB)  │
└─────────────────┘     └─────────────────┘     └─────────────────┘
                               │
                        Hourly systemd timer
                        (5 minutes past hour)
```

### Scheduling

```ini
# /etc/systemd/system/ndp-etl.timer
[Timer]
OnCalendar=*:05:00
Persistent=true        # Catches up after downtime
RandomizedDelaySec=60  # Prevents thundering herd
```

### ETL SQL Pattern

```sql
-- Incremental load: new data since last watermark
INSERT INTO pg.silver.indoor_air_quality (...)
SELECT ...
FROM read_parquet('/data/raw/air-quality/**/*.parquet')
WHERE to_timestamp(timestamp / 1000000) > (
    SELECT COALESCE(MAX(observation_time), '1970-01-01')
    FROM pg.silver.indoor_air_quality
)
AND to_timestamp(timestamp / 1000000) <= current_timestamp - INTERVAL '5 minutes';
```

---

## 3. Data Dictionary Summary

### Type Standards

| Use Case | PostgreSQL Type | Example |
|----------|-----------------|---------|
| Temperatures | `DOUBLE PRECISION` | temperature_c |
| Percentages | `DOUBLE PRECISION` | humidity_pct |
| Indexes (AQI, TVOC) | `SMALLINT` | tvoc_index |
| CO2 | `SMALLINT` | co2 |
| Timestamps | `TIMESTAMPTZ` | observation_time |
| Identifiers | `TEXT` | ndp_id |
| DQ transparency | `TEXT[]` | dq_flags |

### Unit Standardization

| Measurement | Standard Unit | Source Conversions |
|-------------|---------------|-------------------|
| Temperature | Celsius (C) | OWM: Kelvin - 273.15 |
| Pressure | Pascals (Pa) | OWM: hPa × 100 |
| Wind Speed | km/h | OWM: m/s × 3.6 |
| Visibility | meters (m) | Direct |
| PM Concentration | ug/m3 | Direct |
| CO2 | ppm | Direct |

### Core Tables Schema Summary

**silver.indoor_air_quality**
- observation_time, ndp_id, location_path
- co2, pm25, pm25_compensated, pm10
- tvoc_index, nox_index
- temperature_c, humidity_pct
- dq_flags

**silver.outdoor_weather**
- observation_time, ndp_id, source_provider
- temperature_c, humidity_pct, pressure_pa
- wind_speed_kmh, wind_direction_deg
- visibility_m, cloud_cover_pct
- dq_flags

**silver.outdoor_air_quality**
- observation_time, ndp_id
- aqi_owm, aqi_epa (calculated)
- pm25, pm10, co, no, no2, o3, so2, nh3
- dq_flags

**silver.weather_forecast**
- issue_time, valid_time, ndp_id
- lead_time_hours (generated)
- temperature_c, humidity_pct
- wind_speed_kmh, precip_prob_pct
- dq_flags

---

## 4. Dashboard Integration

### Migration Strategy

| Current State | Target State |
|--------------|--------------|
| DuckDB datasource | TimescaleDB datasource |
| `read_parquet()` queries | Direct table queries |
| `${__from}::BIGINT * 1000` | `$__timeFilter(time)` |
| Real-time transformation | Pre-typed columns |

### SQL Translation Pattern

```sql
-- DuckDB (Bronze)
SELECT time_bucket(INTERVAL '10 minutes', to_timestamp(timestamp/1000000)) as time,
       AVG(CASE WHEN metric = 'pm02' THEN value END) as "PM2.5"
FROM read_parquet('/data/data/air-quality/**/*.parquet')
WHERE timestamp >= ${__from}::BIGINT * 1000
GROUP BY 1

-- TimescaleDB (Silver)
SELECT time_bucket('10 minutes', observation_time) as time,
       AVG(pm25) as "PM2.5"
FROM silver.indoor_air_quality
WHERE $__timeFilter(observation_time)
GROUP BY 1
```

### Performance Optimization

| Time Range | Query Target | Speedup |
|------------|--------------|---------|
| < 24h | Raw hypertable | 1x |
| 24h - 7d | Hourly aggregate | 10x |
| 7d - 90d | Daily aggregate | 100x |

### Connection Pool (Pi Optimized)

```yaml
maxOpenConns: 5      # Limit concurrent queries
maxIdleConns: 2      # Minimal idle footprint
connMaxLifetime: 14400  # 4 hours
```

### Alerting Thresholds

| Metric | Warning | Critical |
|--------|---------|----------|
| Indoor PM2.5 | 35 ug/m3 | 55 ug/m3 |
| Indoor CO2 | 1500 ppm | 2000 ppm |
| Outdoor AQI | 100 | 150 |
| Indoor Temp | >28°C | >32°C |

---

## 5. Implementation Roadmap

### Phase 1: Schema & Infrastructure (Week 1)

| Task | Effort | Dependency |
|------|--------|------------|
| Create TimescaleDB init scripts | 2h | None |
| Deploy TimescaleDB container | 1h | Init scripts |
| Configure Grafana datasource | 1h | Container |
| Create grafana_reader user | 0.5h | Container |

### Phase 2: ETL Development (Week 2)

| Task | Effort | Dependency |
|------|--------|------------|
| Write DuckDB ETL SQL script | 4h | Schema |
| Implement per-stream transforms | 2h | ETL script |
| Create systemd timer | 1h | ETL script |
| Add monitoring dashboard | 2h | ETL running |

### Phase 3: Dashboard Migration (Week 3)

| Task | Effort | Dependency |
|------|--------|------------|
| Migrate Indoor Air Quality dashboard | 2h | Silver data |
| Migrate Outdoor dashboards | 2h | Silver data |
| Migrate Forecast Accuracy dashboard | 3h | Silver data |
| Configure alerts | 2h | Dashboards |

### Phase 4: Validation & Cutover (Week 4)

| Task | Effort | Dependency |
|------|--------|------------|
| Backfill from Bronze history | 1h | ETL complete |
| Dual-run validation (DuckDB vs TimescaleDB) | 4h | Backfill |
| Performance tuning | 2h | Validation |
| Documentation update | 2h | Cutover |

**Total Estimated Effort**: ~30 hours

---

## 6. Open Questions Resolved

| Question | Resolution |
|----------|------------|
| Virtual vs Physical Silver? | Physical TimescaleDB; DuckDB views deprecated |
| Forecast explosion strategy? | DuckDB ETL handles array expansion |
| Stream deduplication? | nws-observations vs nws-station-observations - investigate later |
| ETL trigger? | Hourly systemd timer with Persistent=true |
| Backfill strategy? | Same ETL script with wider time filter |
| Unit standardization? | SI units in Silver (°C, km/h, Pa) |

---

## 7. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| DuckDB postgres extension issues | Low | Medium | Rust ETL fallback ready |
| TimescaleDB memory pressure | Low | Low | 256MB limit configured |
| Dashboard query timeout | Medium | Low | Continuous aggregates |
| ETL failure | Low | Medium | Persistent timer + alerting |

---

## 8. Success Criteria

| Metric | Target | Measurement |
|--------|--------|-------------|
| ETL latency | <30s | Timer logs |
| Dashboard query time | <2s for 24h | Grafana metrics |
| Memory usage | <300MB peak | Pi monitoring |
| Data freshness | <5 min lag | Grafana panel |
| Uptime | >99% | Systemd status |

---

## 9. Research Documents

| Document | Author | Key Contribution |
|----------|--------|------------------|
| `01-scope-definition.md` | ndp-architect | 4 entity model, stream mapping |
| `02-etl-alternatives.md` | ndp-timescale-dev | DuckDB recommendation, code templates |
| `03-data-dictionary.md` | ndp-analytics-engineer | Complete typed schemas |
| `04-dashboard-integration.md` | ndp-grafana-dev | SQL migration patterns |

---

## 10. Next Steps

1. **Create Feature**: `dp-006` for Silver Layer Implementation
2. **Specification Phase**: Formalize requirements from this research
3. **Architecture Phase**: ADRs for ETL approach and schema design
4. **Implementation**: Follow roadmap phases 1-4

---

*Synthesis completed: 2026-01-05*
*Research Swarm: 4 NDP agents (ndp-architect, ndp-timescale-dev, ndp-analytics-engineer, ndp-grafana-dev)*
