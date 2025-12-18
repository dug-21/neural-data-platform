-- Silver Layer View: Outdoor Air Quality
-- Feature: DP-001
-- Source: /data/data/outdoor-air-quality/**/*.parquet (Bronze layer - long format)
-- Description: OpenWeatherMap Air Pollution API data PIVOTed to wide format
--
-- Bronze Schema: timestamp, location_id, metric, value
-- Silver Schema: timestamp, location_id, aqi, co, no, no2, o3, so2, pm2_5, pm10, nh3
--
-- Quality Rules:
--   - Range validation per OpenWeatherMap API specs
--   - NULL handling for optional fields
--   - Rounding to air quality monitoring precision
--
-- Performance: Optimized for 7-day queries (<5s target)

CREATE OR REPLACE VIEW silver_outdoor_air AS
WITH bronze_data AS (
    SELECT
        -- Convert microseconds to timestamp
        to_timestamp(timestamp / 1000000) as ts,
        location_id,
        metric,
        value
    FROM read_parquet(
        '/data/data/outdoor-air-quality/**/*.parquet',
        union_by_name = true,
        filename = true,
        hive_partitioning = true
    )
    WHERE timestamp IS NOT NULL
),

-- PIVOT from long format to wide format
pivoted AS (
    SELECT
        ts as timestamp,
        location_id,
        MAX(CASE WHEN metric = 'aqi' THEN value END) as aqi_raw,
        MAX(CASE WHEN metric = 'co' THEN value END) as co_raw,
        MAX(CASE WHEN metric = 'no' THEN value END) as no_raw,
        MAX(CASE WHEN metric = 'no2' THEN value END) as no2_raw,
        MAX(CASE WHEN metric = 'o3' THEN value END) as o3_raw,
        MAX(CASE WHEN metric = 'so2' THEN value END) as so2_raw,
        MAX(CASE WHEN metric = 'pm2_5' THEN value END) as pm2_5_raw,
        MAX(CASE WHEN metric = 'pm10' THEN value END) as pm10_raw,
        MAX(CASE WHEN metric = 'nh3' THEN value END) as nh3_raw
    FROM bronze_data
    GROUP BY ts, location_id
)

-- Apply data quality validation
SELECT
    timestamp,
    location_id,

    -- AQI: 1-5 (OpenWeatherMap scale), integer
    CASE
        WHEN aqi_raw >= 1 AND aqi_raw <= 5
        THEN ROUND(aqi_raw, 0)
        ELSE NULL
    END as aqi,

    -- CO: 0-50000 µg/m³, 1 decimal
    CASE
        WHEN co_raw >= 0 AND co_raw <= 50000
        THEN ROUND(co_raw, 1)
        ELSE NULL
    END as co,

    -- NO: 0-1000 µg/m³, 2 decimals
    CASE
        WHEN no_raw >= 0 AND no_raw <= 1000
        THEN ROUND(no_raw, 2)
        ELSE NULL
    END as no,

    -- NO2: 0-1000 µg/m³, 2 decimals
    CASE
        WHEN no2_raw >= 0 AND no2_raw <= 1000
        THEN ROUND(no2_raw, 2)
        ELSE NULL
    END as no2,

    -- O3: 0-1000 µg/m³, 2 decimals
    CASE
        WHEN o3_raw >= 0 AND o3_raw <= 1000
        THEN ROUND(o3_raw, 2)
        ELSE NULL
    END as o3,

    -- SO2: 0-1000 µg/m³, 2 decimals
    CASE
        WHEN so2_raw >= 0 AND so2_raw <= 1000
        THEN ROUND(so2_raw, 2)
        ELSE NULL
    END as so2,

    -- PM2.5: 0-1000 µg/m³, 1 decimal
    CASE
        WHEN pm2_5_raw >= 0 AND pm2_5_raw <= 1000
        THEN ROUND(pm2_5_raw, 1)
        ELSE NULL
    END as pm2_5,

    -- PM10: 0-1000 µg/m³, 1 decimal
    CASE
        WHEN pm10_raw >= 0 AND pm10_raw <= 1000
        THEN ROUND(pm10_raw, 1)
        ELSE NULL
    END as pm10,

    -- NH3: 0-200 µg/m³, 2 decimals
    CASE
        WHEN nh3_raw >= 0 AND nh3_raw <= 200
        THEN ROUND(nh3_raw, 2)
        ELSE NULL
    END as nh3

FROM pivoted
ORDER BY timestamp DESC;

-- ============================================================================
-- View Metadata
-- ============================================================================
-- Source: Bronze layer (long format with metric column)
-- Transform: PIVOT to wide format with validation
-- Expected columns: 11 (timestamp, location_id, + 9 measurements)
-- ============================================================================
