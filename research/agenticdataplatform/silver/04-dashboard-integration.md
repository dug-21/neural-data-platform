# Dashboard Integration Research - Silver Layer

## Overview

This document provides research findings for migrating NDP Grafana dashboards from DuckDB (Bronze layer) to TimescaleDB (Silver layer). The goal is to enable efficient time-series visualization while respecting Pi 5 resource constraints.

## Current State Analysis

### Existing Data Sources

| Data Source | Type | Status | Purpose |
|-------------|------|--------|---------|
| NDP-DuckDB | motherduck-duckdb-datasource | Primary | Bronze Parquet queries via `read_parquet()` |
| NDP-TimescaleDB | postgres | Configured | Silver layer (not yet populated) |

### Current Dashboard Inventory

| Dashboard | UID | Data Source | Primary Metrics |
|-----------|-----|-------------|-----------------|
| Indoor Air Quality | ndp-indoor-air-quality | DuckDB | PM2.5, CO2, temperature, humidity |
| Outdoor Air Quality | ndp-outdoor-air-quality | DuckDB | AQI, PM2.5, PM10, pollutants |
| NWS Forecast Accuracy | ndp-forecast-accuracy | DuckDB | Temperature error, MAE by lead time |
| Indoor vs Outdoor | ndp-indoor-vs-outdoor | DuckDB | Comparison panels |
| Outdoor Conditions | ndp-outdoor-conditions | DuckDB | Weather metrics |
| NWS vs OWM Comparison | ndp-nws-vs-owm | DuckDB | Source comparison |
| Data Quality | ndp-data-quality | DuckDB | Completeness metrics |

### Bronze Streams (7 Total)

| Stream ID | Description | Key Metrics for Dashboards |
|-----------|-------------|---------------------------|
| air-quality | AirGradient MQTT sensors | pm25, co2, temperature, humidity, tvoc, nox |
| outdoor-air-quality | OpenWeatherMap Air Pollution | aqi, pm2_5, pm10, co, no2, o3, so2 |
| outdoor-weather | OpenWeatherMap Current Weather | temperature, humidity, wind_speed, pressure |
| nws-observations | NWS Station KSGJ | temperature, wind_speed, humidity, pressure |
| nws-forecast-hourly | NWS Hourly Forecast | temperature, wind_speed, probability_of_precipitation |
| nws-gridpoints-forecast | NWS Gridpoint Forecast | 40+ weather metrics |
| nws-station-observations | NWS Station Current | Real-time conditions |

---

## TimescaleDB Data Source Configuration

### Recommended Configuration

```yaml
# config/grafana/provisioning/datasources/timescaledb.yaml
apiVersion: 1

datasources:
  - name: NDP-TimescaleDB
    type: postgres
    uid: timescaledb-ndp
    url: pi5-timescaledb:5432
    database: ndp
    user: grafana_reader
    secureJsonData:
      password: ${GRAFANA_DB_PASSWORD}
    jsonData:
      sslmode: disable
      # Connection pooling - critical for Pi 5 memory
      maxOpenConns: 5          # Reduced from 10 for Pi constraints
      maxIdleConns: 2          # Keep minimal idle connections
      connMaxLifetime: 14400   # 4 hours
      # TimescaleDB-specific
      postgresVersion: 1500    # PostgreSQL 15
      timescaledb: true        # Enable time_bucket() functions
    isDefault: true            # Make Silver the default
    editable: false
```

### Database User Setup

```sql
-- 02-create-users.sql (add to init-scripts)
-- Read-only user for Grafana dashboards
CREATE USER grafana_reader WITH PASSWORD '${GRAFANA_DB_PASSWORD}';

-- Grant access to Silver schema
GRANT CONNECT ON DATABASE ndp TO grafana_reader;
GRANT USAGE ON SCHEMA silver TO grafana_reader;
GRANT SELECT ON ALL TABLES IN SCHEMA silver TO grafana_reader;
ALTER DEFAULT PRIVILEGES IN SCHEMA silver GRANT SELECT ON TABLES TO grafana_reader;

-- Grant access to continuous aggregates
GRANT SELECT ON ALL TABLES IN SCHEMA _timescaledb_internal TO grafana_reader;
```

### Connection Pool Sizing for Pi 5

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| maxOpenConns | 5 | Pi 5 has 8GB RAM, limit concurrent queries |
| maxIdleConns | 2 | Minimal memory footprint when idle |
| connMaxLifetime | 14400 | 4 hours, balance reconnection overhead |
| Query timeout | 30s | Prevent runaway queries on Pi |

---

## SQL Migration Patterns: DuckDB to TimescaleDB

### Key Differences

| Feature | DuckDB | TimescaleDB |
|---------|--------|-------------|
| Time bucketing | `time_bucket(INTERVAL '10 minutes', ...)` | `time_bucket('10 minutes', ...)` |
| Data source | `read_parquet('/path/**/*.parquet')` | Direct table queries |
| Timestamp conversion | `to_timestamp(timestamp/1000000)` | Native TIMESTAMPTZ |
| Grafana time filter | `${__from}::BIGINT * 1000` | `$__timeFilter(time)` |
| CASE expressions | Same syntax | Same syntax |
| Window functions | Full support | Full support |

### Migration Examples

#### Example 1: Current Value (Stat Panel)

**DuckDB (Bronze):**
```sql
SELECT AVG(value) as value
FROM (
    SELECT value
    FROM read_parquet('/data/data/air-quality/**/*.parquet')
    WHERE metric = 'pm02'
    ORDER BY timestamp DESC
    LIMIT 100
)
```

**TimescaleDB (Silver):**
```sql
SELECT pm25 as "PM2.5"
FROM silver.air_quality
ORDER BY time DESC
LIMIT 1
```

#### Example 2: Time Series (Graph Panel)

**DuckDB (Bronze):**
```sql
SELECT
    time_bucket(INTERVAL '10 minutes', to_timestamp(timestamp/1000000)) as time,
    AVG(CASE WHEN metric = 'pm02' THEN value END) as "PM2.5"
FROM read_parquet('/data/data/air-quality/**/*.parquet')
WHERE timestamp >= ${__from}::BIGINT * 1000
  AND timestamp <= ${__to}::BIGINT * 1000
GROUP BY 1
ORDER BY 1
```

**TimescaleDB (Silver):**
```sql
SELECT
    time_bucket('10 minutes', time) as time,
    AVG(pm25) as "PM2.5"
FROM silver.air_quality
WHERE $__timeFilter(time)
GROUP BY 1
ORDER BY 1
```

#### Example 3: Using Continuous Aggregates

**For long time ranges (7d+), use pre-computed aggregates:**

```sql
-- Hourly aggregate (faster for 7d-30d views)
SELECT
    bucket as time,
    avg_pm25 as "PM2.5 (avg)",
    max_pm25 as "PM2.5 (max)"
FROM silver.air_quality_hourly
WHERE $__timeFilter(bucket)
ORDER BY bucket
```

#### Example 4: Multi-Metric Panel

**DuckDB (Bronze):**
```sql
SELECT
    time_bucket(INTERVAL '10 minutes', to_timestamp(timestamp/1000000)) as time,
    AVG(CASE WHEN metric IN ('atmp', 'temperature') THEN value END) as "Temperature",
    AVG(CASE WHEN metric IN ('rhum', 'humidity') THEN value END) as "Humidity"
FROM read_parquet('/data/data/air-quality/**/*.parquet')
WHERE timestamp >= ${__from}::BIGINT * 1000
  AND timestamp <= ${__to}::BIGINT * 1000
GROUP BY 1
ORDER BY 1
```

**TimescaleDB (Silver):**
```sql
SELECT
    time_bucket('10 minutes', time) as time,
    AVG(temperature) as "Temperature",
    AVG(humidity) as "Humidity"
FROM silver.air_quality
WHERE $__timeFilter(time)
GROUP BY 1
ORDER BY 1
```

#### Example 5: Cross-Stream Comparison (Forecast vs Actual)

**DuckDB (Bronze):**
```sql
WITH forecast AS (
    SELECT
        time_bucket(INTERVAL '1 hour', to_timestamp(timestamp/1000000)) as hour,
        AVG(CASE WHEN metric = 'temperature' THEN (value-32)*5/9 END) as temp
    FROM read_parquet('/data/data/nws-forecast-hourly/**/*.parquet')
    WHERE timestamp >= ${__from}::BIGINT * 1000
      AND timestamp <= ${__to}::BIGINT * 1000
    GROUP BY 1
),
observations AS (
    SELECT
        time_bucket(INTERVAL '1 hour', to_timestamp(timestamp/1000000)) as hour,
        AVG(CASE WHEN metric = 'temperature' THEN value END) as temp
    FROM read_parquet('/data/data/nws-observations/**/*.parquet')
    WHERE timestamp >= ${__from}::BIGINT * 1000
      AND timestamp <= ${__to}::BIGINT * 1000
    GROUP BY 1
)
SELECT
    COALESCE(f.hour, o.hour) as time,
    f.temp as "Forecast",
    o.temp as "Actual"
FROM forecast f
FULL OUTER JOIN observations o ON f.hour = o.hour
ORDER BY time
```

**TimescaleDB (Silver):**
```sql
WITH forecast AS (
    SELECT
        time_bucket('1 hour', time) as hour,
        AVG(temperature) as temp
    FROM silver.nws_forecast_hourly
    WHERE $__timeFilter(time)
    GROUP BY 1
),
observations AS (
    SELECT
        time_bucket('1 hour', time) as hour,
        AVG(temperature) as temp
    FROM silver.nws_observations
    WHERE $__timeFilter(time)
    GROUP BY 1
)
SELECT
    COALESCE(f.hour, o.hour) as time,
    f.temp as "Forecast",
    o.temp as "Actual"
FROM forecast f
FULL OUTER JOIN observations o ON f.hour = o.hour
ORDER BY time
```

---

## Recommended Dashboard Panels for Silver Layer

### Dashboard 1: Overview Dashboard

**Purpose:** Single pane of glass for all NDP data streams

| Panel | Type | Data Source | Query Pattern |
|-------|------|-------------|---------------|
| Current Indoor PM2.5 | Stat | TimescaleDB | `SELECT pm25 FROM silver.air_quality ORDER BY time DESC LIMIT 1` |
| Current Outdoor AQI | Gauge | TimescaleDB | `SELECT aqi FROM silver.outdoor_air_quality ORDER BY time DESC LIMIT 1` |
| Current Temperature | Stat | TimescaleDB | `SELECT temperature FROM silver.outdoor_weather ORDER BY time DESC LIMIT 1` |
| 24h PM2.5 Trend | Time Series | TimescaleDB | Hourly aggregate, indoor + outdoor |
| Data Freshness | Table | TimescaleDB | `SELECT stream_id, MAX(time) FROM silver.* GROUP BY stream_id` |

### Dashboard 2: Air Quality Dashboard

**Purpose:** Indoor air quality monitoring with health thresholds

| Panel | Type | Thresholds | Continuous Aggregate |
|-------|------|------------|---------------------|
| PM2.5 Gauge | Gauge | 0-12 (good), 12-35 (moderate), 35-55 (unhealthy), 55+ (hazardous) | No |
| CO2 Gauge | Gauge | 0-1000 (good), 1000-2000 (elevated), 2000+ (high) | No |
| PM2.5 Trend | Time Series | Same thresholds | hourly for 7d+ |
| Indoor vs Outdoor PM2.5 | Time Series | - | hourly comparison |
| AQI Distribution | Heatmap | - | hourly buckets |

### Dashboard 3: Weather Dashboard

**Purpose:** Outdoor conditions from multiple sources

| Panel | Type | Data Streams |
|-------|------|--------------|
| Current Conditions | Stat row | outdoor-weather |
| Temperature Trend | Time Series | outdoor-weather + nws-observations |
| Wind Rose | Custom | nws-observations |
| Pressure Trend | Time Series | outdoor-weather |
| NWS vs OWM Comparison | Time Series | Both sources overlaid |

### Dashboard 4: Forecast Accuracy Dashboard

**Purpose:** Evaluate NWS forecast performance

| Panel | Type | Metric |
|-------|------|--------|
| Current Error | Stat | `ABS(forecast.temp - observed.temp)` |
| MAE by Lead Time | Bar Chart | Mean Absolute Error bucketed by forecast horizon |
| Accuracy % | Bar Chart | Percent within 2C threshold |
| Forecast vs Actual | Time Series | Dual-axis comparison |
| Error Distribution | Histogram | Error frequency |

---

## Performance Optimization for Pi 5

### Query Optimization Strategies

1. **Use Continuous Aggregates**
   - Create hourly and daily aggregates for all streams
   - Dashboard auto-selects based on time range
   - 10-100x faster for long time ranges

2. **Time Range Routing**
   | Time Range | Query Target |
   |------------|--------------|
   | < 24h | Raw hypertable |
   | 24h - 7d | Hourly aggregate |
   | 7d - 90d | Daily aggregate |

3. **Index Strategy**
   ```sql
   -- Silver tables should have
   CREATE INDEX ON silver.air_quality (time DESC);
   CREATE INDEX ON silver.air_quality (ndp_id, time DESC);
   ```

4. **Query Timeout**
   ```yaml
   # In Grafana datasource config
   jsonData:
     queryTimeout: "30s"
   ```

### Dashboard Refresh Rates

| Dashboard Type | Refresh Rate | Rationale |
|----------------|--------------|-----------|
| Overview | 1m | Near real-time, low query cost |
| Air Quality | 5m | AQI changes slowly |
| Weather | 5m | Weather changes slowly |
| Forecast Accuracy | 15m | Comparison data, higher cost |
| Historical Analysis | Manual | User-triggered, expensive |

### Memory Optimization

1. **Limit concurrent queries**
   - maxOpenConns: 5 (shared across all dashboards)
   - Dashboard panels should use shared queries where possible

2. **Use Grafana caching**
   ```ini
   # grafana.ini
   [caching]
   enabled = true
   ttl = 300  # 5 minutes
   ```

3. **Panel query reduction**
   - Use variables to share base queries
   - Prefer continuous aggregates over raw data
   - Limit time series point density

---

## Alerting Strategy with TimescaleDB

### Grafana Alerting Configuration

```yaml
# config/grafana/provisioning/alerting/air-quality-alerts.yaml
apiVersion: 1

groups:
  - name: AirQualityAlerts
    folder: NDP Alerts
    interval: 1m
    rules:
      # PM2.5 Alert - Unhealthy Level
      - uid: pm25-unhealthy
        title: PM2.5 Unhealthy Level
        condition: C
        data:
          - refId: A
            datasourceUid: timescaledb-ndp
            model:
              rawSql: |
                SELECT time, pm25 as value
                FROM silver.air_quality
                WHERE time > NOW() - INTERVAL '5 minutes'
                ORDER BY time DESC
                LIMIT 1
          - refId: B
            datasourceUid: __expr__
            model:
              type: reduce
              expression: A
              reducer: last
          - refId: C
            datasourceUid: __expr__
            model:
              type: threshold
              expression: B
              conditions:
                - evaluator:
                    type: gt
                    params: [35]
        for: 5m
        annotations:
          summary: "Indoor PM2.5 is elevated: {{ $values.B }} ug/m3"
          description: "PM2.5 has exceeded 35 ug/m3 for 5 minutes"
        labels:
          severity: warning
          stream: air-quality

      # CO2 Alert - Poor Ventilation
      - uid: co2-high
        title: CO2 High Level
        condition: C
        data:
          - refId: A
            datasourceUid: timescaledb-ndp
            model:
              rawSql: |
                SELECT time, co2 as value
                FROM silver.air_quality
                WHERE time > NOW() - INTERVAL '5 minutes'
                ORDER BY time DESC
                LIMIT 1
          - refId: B
            datasourceUid: __expr__
            model:
              type: reduce
              expression: A
              reducer: last
          - refId: C
            datasourceUid: __expr__
            model:
              type: threshold
              expression: B
              conditions:
                - evaluator:
                    type: gt
                    params: [1500]
        for: 10m
        annotations:
          summary: "CO2 is elevated: {{ $values.B }} ppm"
          description: "CO2 has exceeded 1500 ppm for 10 minutes - consider ventilation"
        labels:
          severity: warning
          stream: air-quality

      # Outdoor AQI Alert
      - uid: outdoor-aqi-unhealthy
        title: Outdoor AQI Unhealthy
        condition: C
        data:
          - refId: A
            datasourceUid: timescaledb-ndp
            model:
              rawSql: |
                SELECT time, aqi as value
                FROM silver.outdoor_air_quality
                WHERE time > NOW() - INTERVAL '10 minutes'
                ORDER BY time DESC
                LIMIT 1
          - refId: B
            datasourceUid: __expr__
            model:
              type: reduce
              expression: A
              reducer: last
          - refId: C
            datasourceUid: __expr__
            model:
              type: threshold
              expression: B
              conditions:
                - evaluator:
                    type: gt
                    params: [100]
        for: 15m
        annotations:
          summary: "Outdoor AQI is unhealthy: {{ $values.B }}"
          description: "Outdoor AQI has exceeded 100 for 15 minutes - limit outdoor activity"
        labels:
          severity: warning
          stream: outdoor-air-quality
```

### Alert Thresholds Reference

| Metric | Good | Moderate | Unhealthy | Alert Level |
|--------|------|----------|-----------|-------------|
| Indoor PM2.5 | 0-12 | 12-35 | 35+ | 35 (warning), 55 (critical) |
| Indoor CO2 | 0-1000 | 1000-1500 | 1500+ | 1500 (warning), 2000 (critical) |
| Outdoor AQI | 0-50 | 51-100 | 101+ | 100 (warning), 150 (critical) |
| Temperature (indoor) | 18-24 | 15-18, 24-28 | <15, >28 | 28 (warning) |

### Notification Channels

```yaml
# grafana/provisioning/alerting/contact-points.yaml
apiVersion: 1

contactPoints:
  - name: ndp-default
    receivers:
      - uid: email-notification
        type: email
        settings:
          addresses: "${NDP_ALERT_EMAIL}"
          singleEmail: true
      # Future: Add webhook for Home Assistant integration
```

---

## Migration Checklist

### Phase 1: Data Source Setup
- [ ] Update timescaledb.yaml with optimized connection settings
- [ ] Create grafana_reader user in 02-create-users.sql
- [ ] Test TimescaleDB connection from Grafana

### Phase 2: Dashboard Migration (per dashboard)
- [ ] Create copy of dashboard with `-silver` suffix
- [ ] Update datasource UID to `timescaledb-ndp`
- [ ] Migrate each panel query using patterns above
- [ ] Test with short time range (1h)
- [ ] Test with long time range (7d) - verify aggregates used
- [ ] Validate thresholds display correctly

### Phase 3: Alerting Setup
- [ ] Deploy alert provisioning YAML
- [ ] Configure notification channels
- [ ] Test alert firing with synthetic data
- [ ] Verify alert recovery

### Phase 4: Cutover
- [ ] Set NDP-TimescaleDB as default datasource
- [ ] Archive DuckDB-based dashboards (prefix with `[ARCHIVE]`)
- [ ] Update dashboard links and navigation
- [ ] Monitor query performance for 1 week

---

## Appendix: Grafana Variable Templates

### Stream Selector
```sql
-- Variable: stream_id
SELECT DISTINCT stream_id FROM data_dictionary.streams WHERE enabled = true ORDER BY stream_id
```

### Time Range Auto-Select
```sql
-- Variable: time_table (hidden)
-- Automatically selects raw vs aggregate based on time range
SELECT CASE
    WHEN $__range < INTERVAL '24 hours' THEN 'silver.air_quality'
    WHEN $__range < INTERVAL '7 days' THEN 'silver.air_quality_hourly'
    ELSE 'silver.air_quality_daily'
END as table_name
```

### Location Selector (Future)
```sql
-- Variable: ndp_id
SELECT DISTINCT ndp_id,
       COALESCE(context->>'friendly_name', ndp_id) as label
FROM silver.air_quality
ORDER BY label
```

---

## Related Documents

- `docs/architecture/ADR-003-silver-schema.md` - Silver layer schema decisions
- `product/features/dp-001/` - Data Platform foundation
- `config/grafana/` - Grafana configuration files

---

*Research completed: 2026-01-05*
*Author: NDP Grafana Developer Agent*
