-- Export DuckDB Views to SQLite for Grafana
-- Feature: DP-001 - Grafana Integration
-- Author: ndp-grafana-dev
-- Date: 2025-12-18
--
-- Purpose: Export hourly aggregated views to SQLite format for Grafana consumption
--
-- Usage:
--   duckdb /duckdb/neural_platform.db < /config/duckdb/export_to_sqlite.sql
--
-- Output: /duckdb/grafana.db (SQLite database for Grafana)
--
-- Schedule: Run this every 5 minutes via cron or systemd timer

-- Load SQLite extension
INSTALL sqlite;
LOAD sqlite;

-- Attach SQLite database (will create if doesn't exist)
ATTACH '/duckdb/grafana.db' AS grafana_db (TYPE SQLITE);

-- Drop and recreate readings_hourly table in SQLite
DROP TABLE IF EXISTS grafana_db.readings_hourly;

-- Create table structure
CREATE TABLE grafana_db.readings_hourly AS
SELECT * FROM readings_hourly
LIMIT 0;  -- Create empty table with correct schema

-- Insert recent data (last 30 days for dashboard queries)
INSERT INTO grafana_db.readings_hourly
SELECT * FROM readings_hourly
WHERE bucket >= current_timestamp - INTERVAL '30 days'
ORDER BY bucket DESC, stream_id;

-- Create indexes for faster queries
CREATE INDEX IF NOT EXISTS idx_bucket ON grafana_db.readings_hourly(bucket);
CREATE INDEX IF NOT EXISTS idx_stream ON grafana_db.readings_hourly(stream_id);
CREATE INDEX IF NOT EXISTS idx_bucket_stream ON grafana_db.readings_hourly(bucket, stream_id);

-- Export summary statistics
SELECT
    'Export completed successfully' as status,
    COUNT(*) as total_rows_exported,
    MIN(bucket) as earliest_data,
    MAX(bucket) as latest_data,
    current_timestamp as export_time
FROM grafana_db.readings_hourly;

-- Detach database
DETACH grafana_db;
