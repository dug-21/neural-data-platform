# DP-001: DuckDB Analytics Layer + Grafana Dashboards

**Feature ID**: dp-001
**Phase**: Data Platform
**Status**: Scoping
**Created**: 2025-12-17
**Author**: Human + Claude

---

## Executive Summary
This project is building a generic data platform, modularly, in an MVP type approach focusing on 1 domain and several streams to serve as the 1st.  Currently we have created a data ingestion layer -> bronze layer stored in parquet files.  This solution is working, and this phase is not to touch that working solution.

Add a query/analytics layer using DuckDB and visualization layer using Grafana to the Neural Data Platform. This enables dashboards and ad-hoc analytical queries against the existing Parquet data without modifying the Rust ingestion pipeline.

---

## Business Context

### Current State

The platform currently ingests data from multiple sources and stores it in Parquet format (Bronze layer):
- `air-quality` - Indoor air quality from AirGradient sensor (MQTT)
- `outdoor-weather` - Weather data from OpenWeatherMap API (HTTP polling)
- `outdoor-air-quality` - Air pollution data from OpenWeatherMap API (HTTP polling)

Data is stored but not easily queryable or visualized.

### Desired Outcome

- **Queryable data**: SQL-based analytics against Parquet files
- **Visual dashboards**: Real-time and historical visualization of all streams
- **Correlation visibility**: Indoor vs outdoor metrics on the same timeline
- **Ad-hoc analysis**: Support for analytical queries outside of Grafana

---

## Scope

### In Scope

#### 1. DuckDB Analytics Layer

| Item | Description |
|------|-------------|
| DuckDB container | Standalone container for analytical queries |
| Parquet integration | Query existing Parquet files directly (read-only) |
| Virtual Silver views | SQL views with DQ logic (null handling, range filtering, normalization) |
| Multi-stream queries | JOIN capability across all three data streams |
| Query interface | Ability to run ad-hoc SQL queries (not just Grafana) |

#### 2. Grafana Visualization Layer

| Item | Description |
|------|-------------|
| Grafana container | OSS Grafana deployment |
| DuckDB datasource | Connection to DuckDB for queries |
| Config-based dashboards | YAML/JSON provisioned dashboards (GitOps) |
| Dashboard editing | Ability to modify and save dashboards in production |
| No authentication | Open access on local network (home deployment) |

#### 3. Dashboard Requirements

| Dashboard | Description |
|-----------|-------------|
| Indoor Air Quality | PM2.5, CO2, temperature, humidity over time |
| Outdoor Conditions | Temperature, wind, pressure, weather conditions |
| Outdoor Air Quality | AQI, PM2.5, pollutant gases |
| Indoor vs Outdoor | Multiple charts on same page, aligned timeline for correlation analysis |

#### 4. Time Range Support

| Range | Requirement |
|-------|-------------|
| Default view | Last 7 days |
| Extended view | Last 30 days (nice to have) |
| Custom range | User-selectable time picker |

#### 5. Deployment Changes

| Item | Description |
|------|-------------|
| docker-compose.yml | Add DuckDB and Grafana services |
| Volume mounts | Read-only access to Parquet data directory |
| Deployment scripts | Update deploy.sh for new services |
| Configuration files | DuckDB init scripts, Grafana provisioning |

### Out of Scope

#### Explicitly Excluded

| Item | Reason |
|------|--------|
| **Rust code changes** | Phase rule: no modifications to neural-core or air-quality-app |
| **Ingestion validation improvements** | Requires Rust changes, defer to future phase |
| **Real-time streaming triggers** | Requires Rust changes |
| **Grafana alerting** | Defer to dedicated alerts phase (al-xxx) |
| **Authentication/authorization** | Not required for home Pi deployment |
| **Physical Silver layer** | Virtual views are sufficient; physical materialization deferred |
| **ruv-FANN integration** | Defer to ML phase (ml-xxx) |
| **Mobile-responsive dashboards** | Desktop-first for V1 |

#### Deferred to Future Phases

| Feature | Target Phase |
|---------|--------------|
| Ingestion-time DQ improvements | Future Rust phase |
| Grafana alerting/notifications | al-001 |
| Predictive models (ruv-FANN) | ml-001 |
| Complex correlation triggers | ml-001 or al-001 |
| Dashboard authentication | If needed later |

---

## Technical Constraints

### Deployment Target

| Constraint | Value |
|------------|-------|
| Hardware | Raspberry Pi 5, 16GB RAM |
| Runtime | Docker containers |
| Network | Local home network |
| Storage | Existing Parquet files in `/data/` |

### Resource Budget

| Service | Max Memory | Notes |
|---------|------------|-------|
| Existing stack | ~900MB | mosquitto + etcd + app |
| DuckDB | ~512MB | Analytical queries |
| Grafana | ~256MB | Visualization |
| **Total** | ~1.7GB | Well within 16GB |

### Integration Points

| Integration | Type | Notes |
|-------------|------|-------|
| Parquet files | Read-only | DuckDB reads from `/data/{stream}/*.parquet` |
| DuckDB → Grafana | Datasource | Grafana queries DuckDB |
| Config files | GitOps | Dashboards provisioned from YAML/JSON |

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────┐
│                 EXISTING (no changes)                       │
│                                                             │
│  Sensors/APIs → Rust App → Parquet Files                    │
│                               │                             │
│                               ├─→ /data/air-quality/        │
│                               ├─→ /data/outdoor-weather/    │
│                               └─→ /data/outdoor-air-quality/│
└─────────────────────────────────────────────────────────────┘
                                │
                                │ (read-only mount)
                                ▼
┌─────────────────────────────────────────────────────────────┐
│                 NEW (this feature)                          │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                    DuckDB                            │   │
│  │  ┌─────────────────────────────────────────────┐    │   │
│  │  │           Virtual Silver Views              │    │   │
│  │  │  • silver_indoor_air                        │    │   │
│  │  │  • silver_outdoor_weather                   │    │   │
│  │  │  • silver_outdoor_air                       │    │   │
│  │  │  • cross_stream_aligned                     │    │   │
│  │  └─────────────────────────────────────────────┘    │   │
│  └──────────────────────┬──────────────────────────────┘   │
│                         │                                   │
│                         │ SQL queries                       │
│                         ▼                                   │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                   Grafana                            │   │
│  │  • Indoor Air Quality Dashboard                     │   │
│  │  • Outdoor Conditions Dashboard                     │   │
│  │  • Indoor vs Outdoor Comparison Dashboard           │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## Data Streams Reference

### air-quality (Indoor)

| Field | Type | Unit | Description |
|-------|------|------|-------------|
| pm25 | float | μg/m³ | PM2.5 particulate matter |
| pm10 | float | μg/m³ | PM10 particulate matter |
| co2 | float | ppm | Carbon dioxide |
| tvoc | float | ppb | Total VOCs |
| temperature | float | °C | Indoor temperature |
| humidity | float | % | Relative humidity |

### outdoor-weather

| Field | Type | Unit | Description |
|-------|------|------|-------------|
| temperature | float | °C | Outdoor temperature |
| feels_like | float | °C | Apparent temperature |
| humidity | float | % | Relative humidity |
| pressure | float | hPa | Atmospheric pressure |
| wind_speed | float | m/s | Wind speed |
| wind_direction | float | degrees | Wind direction |
| clouds | float | % | Cloud cover |
| weather_main | string | - | Weather condition |

### outdoor-air-quality

| Field | Type | Unit | Description |
|-------|------|------|-------------|
| aqi | int | 1-5 | Air Quality Index |
| pm25 | float | μg/m³ | PM2.5 |
| pm10 | float | μg/m³ | PM10 |
| no2 | float | μg/m³ | Nitrogen dioxide |
| o3 | float | μg/m³ | Ozone |
| co | float | μg/m³ | Carbon monoxide |
| so2 | float | μg/m³ | Sulfur dioxide |

---

## Success Criteria

### Must Have (V1 Complete)

- [ ] DuckDB container running and queryable
- [ ] All three Parquet streams queryable via SQL
- [ ] Virtual Silver views created with basic DQ logic
- [ ] Grafana container running
- [ ] DuckDB datasource configured in Grafana
- [ ] Indoor Air Quality dashboard functional
- [ ] Outdoor Conditions dashboard functional
- [ ] Indoor vs Outdoor comparison dashboard with aligned timeline
- [ ] Dashboards provisioned via config files
- [ ] Dashboards editable and saveable in Grafana UI
- [ ] Default 7-day view working
- [ ] 30-day view working
- [ ] docker-compose.yml updated
- [ ] deploy.sh updated

### Nice to Have (V1 Stretch)

- [ ] Dashboard refresh rate configurable
- [ ] Hourly/daily aggregation views
- [ ] Query performance optimized for 30-day range

---

## File Structure (Expected)

```
NEW FILES:
config/
├── duckdb/
│   ├── init.sql                    # Bootstrap script
│   └── views/
│       ├── silver_indoor_air.sql
│       ├── silver_outdoor_weather.sql
│       ├── silver_outdoor_air.sql
│       └── cross_stream_aligned.sql
└── grafana/
    ├── grafana.ini                 # Grafana config
    ├── provisioning/
    │   ├── datasources/
    │   │   └── duckdb.yaml
    │   └── dashboards/
    │       ├── dashboard.yaml      # Provisioning config
    │       └── dashboards/
    │           ├── indoor-air-quality.json
    │           ├── outdoor-conditions.json
    │           └── indoor-vs-outdoor.json

MODIFIED FILES:
deploy/pi/
├── docker-compose.yml              # Add DuckDB, Grafana services
└── deploy.sh                       # Add setup commands

NEW SPARC DOCUMENTATION:
product/features/dp-001/
├── SCOPE.md                        # This file
├── STATUS.md                       # Progress tracking
├── specification/                  # Requirements
├── pseudocode/                     # Query/view design
├── architecture/                   # Integration design
├── refinement/                     # Implementation
└── completion/                     # Deployment verification
```

## Existing Technology 
You and the agents should research existing technology built by getting patterns and checking md and drawio fles under /docs.  Deployment architecture will remain consistent, and we are leveraging Gitops configuration deployment patterns that this effort should also rely upon.

---

## Open Questions

| Question | Status | Notes |
|----------|--------|-------|
| DuckDB persistence | TBD | Should DuckDB have persistent storage or ephemeral? Views are SQL files, but Grafana-edited dashboards need persistence |
| Grafana dashboard storage | TBD | Need volume for Grafana DB to persist dashboard edits |
| Parquet file pattern | Verify | Confirm exact file naming pattern for glob in views |

---

## References

- [Existing Architecture](../../../docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md)
- [Stream Configurations](../../../config/base/streams/)
- [DuckDB Parquet Integration](https://duckdb.org/docs/data/parquet/overview)
- [Grafana Provisioning](https://grafana.com/docs/grafana/latest/administration/provisioning/)
- [Grafana DuckDB Plugin](https://grafana.com/grafana/plugins/motherduck-duckdb-datasource/)
