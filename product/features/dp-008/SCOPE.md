# DP-008: Silver Layer Grafana Dashboards

## Overview

Build Grafana dashboards that visualize data from the Silver layer (TimescaleDB), replacing the deprecated Bronze layer dashboards. Focus on operational monitoring, forecast accuracy analysis, and indoor environment management.

## Goals

1. **Operational Visibility**: Know that data collection is working across all streams
2. **Forecast Trust Calibration**: Understand NWS forecast accuracy at different lead times
3. **Indoor Environment Intelligence**: Combine indoor AQ, outdoor weather/AQ, and forecast data for actionable insights

## Pre-requisites

- DP-006 Silver ETL operational with data flowing to TimescaleDB
- Grafana instance running with access to TimescaleDB

## Scope

### Pre-work: Grafana Configuration

1. **Add TimescaleDB Data Source**
   - Type: PostgreSQL
   - Name: `timescaledb-silver`
   - Connection to Silver layer database
   - Read-only credentials

2. **Remove Bronze Dashboards**
   - Delete all 12 existing dashboard JSON files in `config/grafana/dashboards/`
   - **Keep** the DuckDB/Bronze data source (may be used for future validation views)

### Dashboard 1: Pipeline Health

**Purpose**: "Is everything still working?"

**Requirements**:
- **Config-driven**: Dynamically discovers streams from Silver tables (no manual updates as streams are added)
- Query `information_schema` or Silver tables directly to enumerate available streams

**Panels**:
| Panel | Description |
|-------|-------------|
| Stream Status Grid | One row per stream: name, last record time, status indicator |
| Data Freshness Gauges | Time since last record per stream |
| Record Volume (24h) | Count of records ingested in last 24 hours per stream |
| DQ Flag Summary | Percentage of records with DQ flags per stream |
| Ingestion Timeline | Stacked area chart showing records over time by stream |

**Freshness Thresholds**:
| Stream Type | Expected Interval | Yellow (Warning) | Red (Critical) |
|-------------|-------------------|------------------|----------------|
| air-quality (MQTT) | ~30 seconds | >1 minute | >2 minutes |
| outdoor-weather (HTTP poll) | 10 minutes | >20 minutes | >40 minutes |
| outdoor-air-quality (HTTP poll) | 10 minutes | >20 minutes | >40 minutes |
| nws-observations (HTTP poll) | 5 minutes | >10 minutes | >20 minutes |
| nws-gridpoints-forecast | 1 hour | >2 hours | >4 hours |

**Time Range**: Default 24h, selectable: 6h, 24h, 7d

### Dashboard 2: Forecast Accuracy

**Purpose**: "How much can I trust the forecast?"

**Data Sources**:
- `silver.weather_forecasts` (NWS gridpoints with issue_time/valid_time)
- `silver.weather_observations` (NWS observations)

**Accuracy Calculation**:
- Join forecast `valid_time` to closest `observation_time` (within ±30 minutes)
- Calculate error = forecast - observed

**Lead Time Buckets**:
| Bucket | Range | Use Case |
|--------|-------|----------|
| 1 hour | 0-1.5h | Immediate decisions |
| 3 hours | 1.5-4.5h | Near-term planning |
| 6 hours | 4.5-9h | Same-day planning |
| 12 hours | 9-18h | Next-day planning |
| 24 hours | 18-36h | Tomorrow |
| 48 hours | 36-60h | Weekend planning |

**Panels**:
| Panel | Description |
|-------|-------------|
| Temperature MAE by Lead Time | Bar chart: Mean Absolute Error per bucket |
| Temperature Bias | Bar chart: Mean Error (positive = forecast too warm) |
| Accuracy % (within 2°C) | Percentage of forecasts within 2°C of actual |
| Forecast vs Actual Overlay | Time series: both lines overlaid for visual comparison |
| Wind Speed Accuracy | MAE for wind speed by lead time |
| Humidity Accuracy | MAE for humidity by lead time |
| Trustworthy Horizon | Single stat: "Forecasts reliable up to X hours" |

**Time Range**: Default 7d (need sufficient samples), selectable: 24h, 7d, 30d

### Dashboard 3: Indoor Environment + Outdoor Context + Ventilation

**Purpose**: "What's my indoor air quality and should I open windows?"

**Data Sources**:
- `silver.air_quality_observations` (indoor AirGradient)
- `silver.weather_observations` (outdoor weather)
- `silver.outdoor_air_quality` (outdoor AQ)
- `silver.weather_forecasts` (upcoming conditions)

**Panels - Current Status Row**:
| Panel | Source | Description |
|-------|--------|-------------|
| Indoor Temperature | air_quality_observations | Current + 6h sparkline |
| Indoor Humidity | air_quality_observations | Current + comfort range indicator |
| Indoor CO2 | air_quality_observations | Current + threshold coloring (green <800, yellow 800-1200, red >1200) |
| Indoor PM2.5 | air_quality_observations | Current + EPA threshold coloring |
| Outdoor Temperature | weather_observations | Current + feels-like |
| Outdoor Humidity | weather_observations | Current value |
| Outdoor AQI | outdoor_air_quality | EPA AQI with category color |

**Panels - Ventilation Decision**:
| Panel | Description |
|-------|-------------|
| Ventilation Recommendation | Large indicator: "OPEN WINDOWS" (green) / "KEEP CLOSED" (red) with reason |
| Ventilation Factors | Table showing each factor's status (indoor CO2, outdoor temp, humidity, AQI, rain forecast) |
| Upcoming Conditions | Next 6-12h forecast summary (temp range, precip probability) |

**Ventilation Logic**:
```
OPEN WINDOWS when ALL conditions met:
- Indoor CO2 > 800 ppm (would benefit from fresh air)
- Outdoor temp: 18-26°C (65-79°F) comfort range
- Outdoor humidity < 70%
- Outdoor AQI < 50 (Good)
- Precipitation probability < 20% in next 2 hours
```

**Panels - Trends**:
| Panel | Description |
|-------|-------------|
| Indoor vs Outdoor Temperature | Dual-axis time series |
| Indoor vs Outdoor PM2.5 | Comparison chart with ratio |
| CO2 Trend | 24h trend with threshold lines |
| Humidity Comparison | Indoor vs outdoor with 60% indoor threshold |

**Time Range**: Default 24h, selectable: 6h, 24h, 7d

### Cross-Dashboard Feature: Temperature Unit Toggle

**Requirement**: All dashboards with temperature fields include a variable to switch between Fahrenheit and Celsius.

**Implementation**:
- Dashboard variable: `temp_unit` with options: `Celsius`, `Fahrenheit`
- SQL queries use conditional conversion:
  ```sql
  CASE
    WHEN '${temp_unit}' = 'Fahrenheit'
    THEN (temperature_c * 9/5) + 32
    ELSE temperature_c
  END as temperature
  ```
- Panel units dynamically set based on variable

## Out of Scope

- Continuous aggregates (using raw data with appropriate time ranges)
- Historical analysis dashboard (future feature)
- Alert/notification configuration (future feature)
- Mobile-optimized views

## Silver Layer Tables Reference

| Table | Key Columns | Metrics |
|-------|-------------|---------|
| `silver.air_quality_observations` | observation_time, ndp_id | pm25, pm10, co2, temperature_c, humidity_pct, tvoc_index, nox_index, dq_flags |
| `silver.weather_observations` | observation_time, ndp_id | temperature_c, feels_like_c, dewpoint_c, humidity_pct, pressure_pa, wind_speed_kmh, wind_direction_deg, wind_gust_kmh, visibility_m, heat_index_c, wind_chill_c, dq_flags |
| `silver.outdoor_air_quality` | observation_time, ndp_id | aqi_owm, aqi_epa, pm25, pm10, co_ugm3, no_ugm3, no2_ugm3, o3_ugm3, so2_ugm3, nh3_ugm3, dq_flags |
| `silver.weather_forecasts` | issue_time, valid_time, ndp_id | temperature_c, dewpoint_c, apparent_temp_c, heat_index_c, wind_chill_c, wind_speed_kmh, wind_direction_deg, wind_gust_kmh, precip_probability_pct, precip_amount_mm, humidity_pct, sky_cover_pct, visibility_m, dq_flags |

## Success Criteria

1. **Pipeline Health**: Can see at-a-glance that all streams are operational or identify which are failing
2. **Forecast Accuracy**: Can determine how many hours out the NWS forecast remains reliable
3. **Indoor/Outdoor**: Can make informed ventilation decisions based on combined indoor/outdoor data
4. **Temperature Toggle**: Can switch between °F and °C on any temperature-containing dashboard
5. **Config-driven**: Adding a new stream to Silver layer automatically appears in Pipeline Health dashboard

## Dependencies

- DP-006: Silver ETL (data source)
- TimescaleDB running with Silver schema
- Grafana with PostgreSQL plugin

## Risks

| Risk | Mitigation |
|------|------------|
| Forecast-observation join complexity | Start with hourly bucket matching, refine to nearest-neighbor if needed |
| Dashboard query performance | Use appropriate time range limits, add indexes if needed |
| Ventilation logic edge cases | Start simple, iterate based on real-world usage |
