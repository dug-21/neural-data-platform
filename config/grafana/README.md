# Grafana Configuration - Neural Data Platform

## Overview

This directory contains Grafana configuration for the Neural Data Platform (DP-001).

## Architecture

```
DuckDB (Views) → SQLite Export → Grafana SQLite Datasource → Dashboards
```

### Data Flow

1. **Bronze Layer**: Parquet files in `/data/{stream-id}/*.parquet`
2. **Silver Layer**: DuckDB views (`silver_indoor_air`, `silver_outdoor_weather`, `silver_outdoor_air`)
3. **Aggregation Layer**: `readings_hourly` view (hourly rollups)
4. **Export Layer**: SQLite database at `/duckdb/grafana.db` (updated every 5 minutes)
5. **Visualization**: Grafana dashboards query SQLite database

## Files

```
config/grafana/
├── grafana.ini                      # Grafana server configuration
├── provisioning/
│   ├── datasources/
│   │   └── duckdb.yaml             # SQLite datasource pointing to DuckDB export
│   └── dashboards/
│       └── dashboards.yaml         # Dashboard provider configuration
├── dashboards/
│   ├── indoor-air-quality.json     # Indoor AirGradient sensor dashboard
│   ├── outdoor-air-quality.json    # OpenWeatherMap air quality dashboard
│   ├── outdoor-conditions.json     # Weather conditions dashboard
│   └── indoor-vs-outdoor.json      # Comparison dashboard
└── README.md                        # This file
```

## Datasource Configuration

### DuckDB → SQLite Export

The DuckDB container runs two processes:

1. **HTTP API**: `duckdb-http` on port 9090 (for API access)
2. **Export Script**: Runs every 5 minutes, exports `readings_hourly` view to `/duckdb/grafana.db`

Export script: `config/duckdb/export_to_sqlite.sql`

### Grafana SQLite Datasource

- **Type**: `frser-sqlite-datasource`
- **UID**: `duckdb-ndp`
- **Path**: `/data/duckdb/grafana.db`
- **Mode**: Read-only
- **Update Frequency**: 5 minutes (via export script)

## Dashboard Queries

All dashboards query the `readings_hourly` view, which contains hourly aggregations for:

### Indoor Air Quality Stream (`air-quality`)
- `avg_pm25`, `max_pm25`, `min_pm25`
- `avg_pm10`, `max_pm10`, `min_pm10`
- `avg_co2`, `max_co2`, `min_co2`
- `avg_temperature`, `max_temperature`, `min_temperature`
- `avg_humidity`, `max_humidity`, `min_humidity`
- `avg_tvoc`, `max_tvoc`, `min_tvoc`
- `avg_nox`, `max_nox`, `min_nox`

### Outdoor Weather Stream (`outdoor-conditions`)
- `avg_temperature`, `max_temperature`, `min_temperature`
- `avg_humidity`, `max_humidity`, `min_humidity`
- `avg_apparent_temperature` (feels like)
- `avg_wind_speed`
- `avg_pressure`
- `avg_cloud_cover`

### Outdoor Air Quality Stream (`outdoor-air-quality`)
- `avg_pm2_5` (note: different field name than indoor `pm25`)
- `avg_pm10`
- `avg_us_aqi` (converted from OpenWeatherMap 1-5 scale)
- `avg_no2`, `avg_o3`, `avg_so2`, `avg_co`

## Example Queries

### Current PM2.5 (Indoor)
```sql
SELECT
    avg_pm25 AS "value",
    bucket AS "time"
FROM readings_hourly
WHERE stream_id = 'air-quality'
ORDER BY bucket DESC
LIMIT 1
```

### 7-Day PM2.5 Trend
```sql
SELECT
    bucket AS time,
    avg_pm25 AS "PM2.5 (µg/m³)"
FROM readings_hourly
WHERE
    stream_id = 'air-quality'
    AND bucket >= datetime('now', '-7 days')
ORDER BY bucket
```

### Indoor vs Outdoor Temperature Comparison
```sql
SELECT
    bucket AS time,
    avg_temperature AS "Indoor Temperature (°C)"
FROM readings_hourly
WHERE stream_id = 'air-quality'
  AND bucket >= datetime('now', '-7 days')

UNION ALL

SELECT
    bucket AS time,
    avg_temperature AS "Outdoor Temperature (°C)"
FROM readings_hourly
WHERE stream_id = 'outdoor-conditions'
  AND bucket >= datetime('now', '-7 days')

ORDER BY time
```

## Performance

- **Query Window**: 7-30 days (configurable per dashboard)
- **Data Freshness**: 5 minutes (export script interval)
- **Query Speed**: <1s for hourly aggregated data
- **Storage**: ~50MB for 30 days of hourly data (3 streams)

## Thresholds & Alerts

### PM2.5 (EPA AQI)
- Green: 0-12 µg/m³ (Good)
- Yellow: 12-35 µg/m³ (Moderate)
- Orange: 35-55 µg/m³ (Unhealthy for Sensitive Groups)
- Red: 55+ µg/m³ (Unhealthy)

### CO2
- Green: 0-1000 ppm (Good)
- Yellow: 1000-2000 ppm (Moderate)
- Red: 2000+ ppm (Poor)

### US AQI
- Green: 0-50 (Good)
- Yellow: 51-100 (Moderate)
- Orange: 101-150 (Unhealthy for Sensitive)
- Red: 151-200 (Unhealthy)
- Purple: 201-300 (Very Unhealthy)
- Maroon: 301+ (Hazardous)

## Deployment

### Initial Setup

1. Start services:
   ```bash
   cd deploy/pi
   docker compose up -d
   ```

2. Wait for DuckDB to initialize views (check logs):
   ```bash
   docker compose logs duckdb
   ```

3. Wait for first SQLite export (5 minutes max)

4. Access Grafana:
   ```
   http://localhost:3000
   Username: admin
   Password: (set via GRAFANA_ADMIN_PASSWORD env var, default: admin)
   ```

### Troubleshooting

#### No Data in Dashboards

1. Check DuckDB export logs:
   ```bash
   docker compose logs duckdb | grep "Export completed"
   ```

2. Verify SQLite database exists:
   ```bash
   docker compose exec duckdb ls -lh /duckdb/grafana.db
   ```

3. Check Grafana datasource connection:
   - Go to Configuration → Data Sources → DuckDB
   - Click "Test & Save"

#### Slow Dashboard Queries

1. Check SQLite indexes:
   ```bash
   docker compose exec duckdb sqlite3 /duckdb/grafana.db ".indexes readings_hourly"
   ```

2. Verify export script is running:
   ```bash
   docker compose exec duckdb ps aux | grep export_to_sqlite
   ```

3. Reduce query time window (e.g., 7 days instead of 30 days)

#### Missing Metrics

1. Check which streams have data:
   ```bash
   docker compose exec duckdb duckdb /duckdb/neural_platform.db \
     "SELECT stream_id, COUNT(*) FROM readings_hourly GROUP BY stream_id"
   ```

2. Verify source Parquet files exist:
   ```bash
   docker compose exec air-quality-app ls -lh /data/*/
   ```

## Development

### Adding New Dashboards

1. Create dashboard JSON in `config/grafana/dashboards/`
2. Use datasource UID: `duckdb-ndp`
3. Query `readings_hourly` view with appropriate `stream_id` filter
4. Restart Grafana: `docker compose restart grafana`

### Adding New Metrics

1. Add metric to Silver layer view (e.g., `silver_indoor_air.sql`)
2. Add aggregation to `readings_hourly.sql`
3. Update export script to include new field
4. Restart DuckDB: `docker compose restart duckdb`
5. Wait for next export cycle (5 minutes)

### Changing Export Frequency

Edit `docker-compose.yml`, change `sleep 300` (5 minutes) to desired interval in seconds.

## References

- [Grafana SQLite Datasource](https://github.com/fr-ser/grafana-sqlite-datasource)
- [DuckDB SQLite Extension](https://duckdb.org/docs/extensions/sqlite)
- [EPA AQI Scale](https://www.airnow.gov/aqi/aqi-basics/)
- DP-001 Feature Documentation: `product/features/dp-001/`
