-- DuckDB Silver Layer Initialization Script
-- Feature: DP-001 - Silver Layer Query Infrastructure
-- Author: ndp-parquet-dev
-- Date: 2025-12-18
--
-- Purpose: Bootstrap DuckDB database with Silver layer views
--
-- Usage:
--   duckdb /data/silver.duckdb < /config/duckdb/init.sql
--
-- Prerequisites:
--   - Parquet files exist in /data/{stream-id}/*.parquet
--   - DuckDB CLI or embedded library available
--
-- Idempotency: Safe to run multiple times (uses CREATE OR REPLACE VIEW)

-- ============================================================================
-- 1. Database Configuration
-- ============================================================================

-- Set memory limit for Raspberry Pi deployment
SET memory_limit = '512MB';

-- Enable parallel query execution (use all 4 cores)
SET threads = 4;

-- Configure Parquet reader for optimal performance
SET enable_object_cache = true;

-- Log initialization start
SELECT 'Starting Silver layer view initialization...' as status,
       current_timestamp as timestamp;

-- ============================================================================
-- 2. Silver Layer Views
-- ============================================================================

-- Indoor Air Quality View
-- Source: /data/air-quality/*.parquet
-- Quality: Range validation, NULL handling, rounding
.read /config/duckdb/views/silver_indoor_air.sql

SELECT 'Created view: silver_indoor_air' as status,
       current_timestamp as timestamp;

-- Outdoor Weather View
-- Source: /data/outdoor-weather/*.parquet
-- Quality: Range validation, NULL handling, rounding
.read /config/duckdb/views/silver_outdoor_weather.sql

SELECT 'Created view: silver_outdoor_weather' as status,
       current_timestamp as timestamp;

-- Outdoor Air Quality View
-- Source: /data/outdoor-air-quality/*.parquet
-- Quality: Range validation, NULL handling, rounding
.read /config/duckdb/views/silver_outdoor_air.sql

SELECT 'Created view: silver_outdoor_air' as status,
       current_timestamp as timestamp;

-- ============================================================================
-- 3. Cross-Stream Views
-- ============================================================================

-- Cross-Stream Aligned View
-- Sources: All three streams
-- Alignment: 10-minute time buckets with FULL OUTER JOIN
.read /config/duckdb/views/cross_stream_aligned.sql

SELECT 'Created view: cross_stream_aligned' as status,
       current_timestamp as timestamp;

-- Hourly Aggregations View (for Grafana dashboards)
-- Sources: All three streams
-- Aggregation: Hourly rollups for fast dashboard queries
.read /config/duckdb/views/readings_hourly.sql

SELECT 'Created view: readings_hourly' as status,
       current_timestamp as timestamp;

-- ============================================================================
-- 4. View Validation
-- ============================================================================

-- Validate silver_indoor_air
SELECT 'Validating silver_indoor_air...' as status;
SELECT
    COUNT(*) as total_rows,
    MIN(timestamp) as earliest_reading,
    MAX(timestamp) as latest_reading,
    COUNT(DISTINCT DATE_TRUNC('day', timestamp)) as days_with_data
FROM silver_indoor_air
WHERE timestamp >= current_timestamp - INTERVAL '7 days';

-- Validate silver_outdoor_weather
SELECT 'Validating silver_outdoor_weather...' as status;
SELECT
    COUNT(*) as total_rows,
    MIN(timestamp) as earliest_reading,
    MAX(timestamp) as latest_reading,
    COUNT(DISTINCT DATE_TRUNC('day', timestamp)) as days_with_data
FROM silver_outdoor_weather
WHERE timestamp >= current_timestamp - INTERVAL '7 days';

-- Validate silver_outdoor_air
SELECT 'Validating silver_outdoor_air...' as status;
SELECT
    COUNT(*) as total_rows,
    MIN(timestamp) as earliest_reading,
    MAX(timestamp) as latest_reading,
    COUNT(DISTINCT DATE_TRUNC('day', timestamp)) as days_with_data
FROM silver_outdoor_air
WHERE timestamp >= current_timestamp - INTERVAL '7 days';

-- Validate cross_stream_aligned
SELECT 'Validating cross_stream_aligned...' as status;
SELECT
    COUNT(*) as total_buckets,
    MIN(time_bucket) as earliest_bucket,
    MAX(time_bucket) as latest_bucket,
    COUNT(*) FILTER (WHERE indoor_pm25 IS NOT NULL) as indoor_rows,
    COUNT(*) FILTER (WHERE outdoor_temp IS NOT NULL) as weather_rows,
    COUNT(*) FILTER (WHERE outdoor_aqi IS NOT NULL) as air_quality_rows
FROM cross_stream_aligned
WHERE time_bucket >= current_timestamp - INTERVAL '7 days';

-- Validate readings_hourly
SELECT 'Validating readings_hourly...' as status;
SELECT
    COUNT(*) as total_buckets,
    MIN(bucket) as earliest_bucket,
    MAX(bucket) as latest_bucket,
    COUNT(*) FILTER (WHERE stream_id = 'air-quality') as indoor_rows,
    COUNT(*) FILTER (WHERE stream_id = 'outdoor-conditions') as weather_rows,
    COUNT(*) FILTER (WHERE stream_id = 'outdoor-air-quality') as air_quality_rows
FROM readings_hourly
WHERE bucket >= current_timestamp - INTERVAL '7 days';

-- ============================================================================
-- 5. Performance Benchmarks
-- ============================================================================

-- Benchmark 7-day query on indoor air
SELECT 'Benchmarking silver_indoor_air (7 days)...' as status;
.timer on
SELECT COUNT(*), AVG(pm25), AVG(temperature)
FROM silver_indoor_air
WHERE timestamp >= current_timestamp - INTERVAL '7 days';
.timer off

-- Benchmark 30-day query on cross-stream view
SELECT 'Benchmarking cross_stream_aligned (30 days)...' as status;
.timer on
SELECT COUNT(*), AVG(indoor_pm25), AVG(outdoor_temp)
FROM cross_stream_aligned
WHERE time_bucket >= current_timestamp - INTERVAL '30 days';
.timer off

-- ============================================================================
-- 6. Summary
-- ============================================================================

SELECT '=====================================' as separator;
SELECT 'Silver Layer Initialization Complete' as status,
       current_timestamp as timestamp;
SELECT '=====================================' as separator;

-- List all views
SELECT 'Available views:' as info;
SELECT table_name, table_type
FROM information_schema.tables
WHERE table_schema = 'main'
AND table_type = 'VIEW'
ORDER BY table_name;

-- ============================================================================
-- End of init.sql
-- ============================================================================
