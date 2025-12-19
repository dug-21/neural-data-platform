-- Export DuckDB Views to SQLite for Grafana
-- Feature: DP-001 - Grafana Integration
-- Author: ndp-grafana-dev
-- Date: 2025-12-18
--
-- Purpose: Export hourly aggregated views to SQLite format for Grafana consumption
--
-- Usage:
--   duckdb /var/duckdb/neural_platform.db < /config/duckdb/export_to_sqlite.sql
--
-- Output: /var/duckdb/grafana.db (SQLite database for Grafana)
--
-- Schedule: Run this every 5 minutes via cron or systemd timer

-- Load SQLite extension
INSTALL sqlite;
LOAD sqlite;

-- Attach SQLite database (will create if doesn't exist)
ATTACH '/var/duckdb/grafana.db' AS grafana_db (TYPE SQLITE);

-- Drop and recreate readings_hourly table in SQLite
DROP TABLE IF EXISTS grafana_db.readings_hourly;

-- Export data with epoch milliseconds for Grafana time series
-- Grafana's SQLite plugin requires numeric timestamps (epoch ms)
CREATE TABLE grafana_db.readings_hourly AS
SELECT
    -- Convert timestamp to epoch milliseconds for Grafana
    epoch_ms(bucket) as time,
    stream_id,
    avg_pm25, max_pm25, min_pm25,
    avg_pm10, max_pm10, min_pm10,
    avg_co2, max_co2, min_co2,
    avg_temperature, max_temperature, min_temperature,
    avg_humidity, max_humidity, min_humidity,
    avg_tvoc, max_tvoc, min_tvoc,
    avg_nox, max_nox, min_nox,
    avg_apparent_temperature,
    avg_wind_speed,
    avg_pressure,
    avg_cloud_cover,
    avg_pm2_5,
    avg_us_aqi,
    avg_no2,
    avg_o3,
    avg_so2,
    avg_co
FROM readings_hourly
WHERE bucket >= current_timestamp - INTERVAL '30 days'
ORDER BY bucket DESC, stream_id;

-- Create indexes for faster queries
CREATE INDEX IF NOT EXISTS idx_time ON grafana_db.readings_hourly(time);
CREATE INDEX IF NOT EXISTS idx_stream ON grafana_db.readings_hourly(stream_id);
CREATE INDEX IF NOT EXISTS idx_time_stream ON grafana_db.readings_hourly(time, stream_id);

-- Export summary statistics
SELECT
    'Export completed successfully' as status,
    COUNT(*) as total_rows_exported,
    MIN(time) as earliest_data_epoch_ms,
    MAX(time) as latest_data_epoch_ms,
    current_timestamp as export_time
FROM grafana_db.readings_hourly;

-- Detach database
DETACH grafana_db;
