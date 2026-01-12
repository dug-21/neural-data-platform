# Silver Layer Research Context

**Date**: 2026-01-05
**Research Swarm**: Silver Layer Scope Refinement
**Status**: In Progress

---

## Research Objective

Refine the scope and plan for building the Silver layer of the Neural Data Platform, focusing on:
1. Silver layer entity/table scope
2. ETL/ELT approach for Raspberry Pi constraints
3. Data dictionary refinement
4. Dashboard integration strategy

---

## Current State Summary

### Bronze Layer (Operational)

The NDP has a working Bronze layer with 7 active streams:

| Stream ID | Source | Data Type | Update Frequency | Typical File Size |
|-----------|--------|-----------|------------------|-------------------|
| air-quality | AirGradient MQTT | Indoor sensor | ~1 min | 94KB/day |
| outdoor-weather | OpenWeatherMap | Current weather | ~5 min | 7KB/day |
| outdoor-air-quality | OpenWeatherMap | Outdoor AQI | ~5 min | 5KB/day |
| nws-observations | NWS API | Station obs | ~5 min | 38KB/day |
| nws-station-observations | NWS API | Station current | ~5 min | 14KB/day |
| nws-forecast-hourly | NWS API | Hourly forecast | ~1 hour | 127KB/day |
| nws-gridpoints-forecast | NWS API | Raw gridpoint | ~1 hour | 261KB/day |

**Total Bronze throughput**: ~550KB/day

### Bronze Schema Pattern

All streams use the wide schema:
```
timestamp: TIMESTAMP (ingestion time)
source_id: STRING (source identifier)
ndp_id: STRING (platform-owned identifier)
context: JSON (config-derived metadata snapshot)
raw_payload: JSON (exact payload from source)
```

### Known Architecture Patterns (from AgentDB)

| Pattern | Description |
|---------|-------------|
| `arch-domain-adapter-pattern` | Hexagonal architecture with Source/Store traits |
| `arch-bronze-storage-trait` | BronzeStorage trait abstraction for MCP |
| `arch-data-lake-layers` | Bronze → Silver → Gold architecture |
| `arch-bronze-schema` | Wide raw JSON schema (DP-004) |
| `arch-silver-schema` | TimescaleDB hypertable schema (proposed) |
| `arch-dual-trait-source` | RawSource vs Source trait separation |

---

## Raw Payload Analysis

### air-quality (AirGradient Indoor Sensor)

**Key Metrics** (29 fields):
- Temperature: `atmp`, `atmpCompensated`
- Humidity: `rhum`, `rhumCompensated`
- PM Particles: `pm01`, `pm02`, `pm10`, `pm02Compensated`
- PM Counts: `pm003Count`, `pm005Count`, `pm01Count`, `pm02Count`, `pm10Count`, `pm50Count`
- PM Standards: `pm01Standard`, `pm02Standard`, `pm10Standard`
- CO2: `rco2`
- VOC: `tvocIndex`, `tvocRaw`
- NOx: `noxIndex`, `noxRaw`
- Device: `firmware`, `model`, `serialno`, `boot`, `bootCount`, `wifi`, `ledMode`

**Current Mapping** (7 fields mapped):
- humidity, temperature, nox, tvoc, pm25, pm10, co2

**Gap**: 22 unmapped fields (including particle counts, raw values, device metadata)

### outdoor-weather (OpenWeatherMap Current)

**Key Metrics** (11 fields mapped):
- temperature, feels_like, pressure, humidity
- wind_speed, wind_deg, wind_gust
- clouds, visibility
- rain_1h, snow_1h

### outdoor-air-quality (OpenWeatherMap Air Pollution)

**Key Metrics** (9 fields mapped):
- aqi (1-5 scale)
- Pollutants: co, no, no2, o3, so2, pm2_5, pm10, nh3

### nws-observations (NWS Station)

**Key Metrics** (deeply nested):
- temperature, dewpoint, relativeHumidity
- windSpeed, windDirection, windGust
- barometricPressure, visibility
- cloudLayers, textDescription, timestamp

### nws-gridpoints-forecast (NWS Comprehensive)

**Key Metrics** (39 mapped fields):
- Temperature variants: temperature, dewpoint, max/min, apparent, wet_bulb, heat_index, wind_chill
- Wind: speed, direction, gust, transport wind, twenty-foot wind
- Precipitation: probability, quantitative, snowfall, ice
- Visibility: sky_cover, visibility, ceiling_height
- Fire indices: dispersion, haines, davis_stability, red_flag
- Marine: wave_height, wave_period, wave_direction, swell data

---

## Key Research Questions

### Scope Definition
1. How should 7 Bronze streams map to Silver tables?
2. Should observations be combined into unified tables or kept separate?
3. What normalization strategy optimizes for Pi constraints?

### ETL/ELT Approach
1. DuckDB vs Rust-native vs Python for ETL?
2. Batch vs micro-batch vs streaming?
3. Memory budget for transformation operations?

### Data Dictionary
1. Which fields are essential vs nice-to-have?
2. Unit standardization approach?
3. Computed field definitions (AQI, heat index)?

### Dashboard Integration
1. How to migrate DuckDB SQL to TimescaleDB?
2. What continuous aggregates are needed?
3. Alerting thresholds and strategy?

---

## Constraints

- **Hardware**: Raspberry Pi 5, 16GB RAM
- **Current Usage**: ~750MB for existing services
- **Available**: ~15GB headroom
- **ETL Memory Budget**: Target <200MB additional
- **Stack**: Rust (core), TimescaleDB (Silver), DuckDB (optional)

---

## Research Files (in progress)

| File | Agent | Focus |
|------|-------|-------|
| `01-scope-definition.md` | ndp-architect | Silver entities & stream mapping |
| `02-etl-alternatives.md` | ndp-timescale-dev | ETL approach comparison |
| `03-data-dictionary.md` | ndp-analytics-engineer | Schema definitions |
| `04-dashboard-integration.md` | ndp-grafana-dev | Grafana/TimescaleDB |

---

## Previous Research Reference

The prior agentic data platform research (in parent directory) focused on:
- Agentic AI capabilities for data exploration
- DuckDB + agent integration patterns
- Edge AI frameworks and constraints
- Two-tier deployment (dev agents, production Pi)

This Silver layer research builds on that foundation with implementation-focused scope.

---

*Context document generated for synthesis phase*
