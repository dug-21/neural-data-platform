-- Silver Layer View: Outdoor Air Quality
-- Feature: DP-001
-- Source: /data/outdoor-air-quality/*.parquet
-- Description: OpenWeatherMap Air Pollution API data with validation
--
-- Quality Rules:
--   - Range validation per OpenWeatherMap API specs
--   - NULL handling for optional fields
--   - Rounding to air quality monitoring precision
--   - Timestamp validation (non-NULL required)
--
-- Performance: Optimized for 7-day queries (<5s target)

CREATE OR REPLACE VIEW silver_outdoor_air AS
SELECT
    -- Timestamp (required field)
    timestamp,

    -- Air Quality Index
    -- Range: 1-5 (OpenWeatherMap scale: 1=Good, 5=Very Poor)
    -- Precision: 0 decimals (discrete scale)
    CASE
        WHEN aqi >= 1 AND aqi <= 5
        THEN ROUND(aqi, 0)
        ELSE NULL
    END as aqi,

    -- Carbon Monoxide (CO)
    -- Range: 0-50000 µg/m³ (0 to extreme pollution)
    -- Precision: 1 decimal (monitoring equipment accuracy)
    CASE
        WHEN co >= 0 AND co <= 50000
        THEN ROUND(co, 1)
        ELSE NULL
    END as co,

    -- Nitric Oxide (NO)
    -- Range: 0-1000 µg/m³ (0 to extreme pollution)
    -- Precision: 2 decimals (monitoring equipment accuracy)
    CASE
        WHEN no >= 0 AND no <= 1000
        THEN ROUND(no, 2)
        ELSE NULL
    END as no,

    -- Nitrogen Dioxide (NO2)
    -- Range: 0-1000 µg/m³ (0 to extreme pollution)
    -- Precision: 2 decimals (monitoring equipment accuracy)
    CASE
        WHEN no2 >= 0 AND no2 <= 1000
        THEN ROUND(no2, 2)
        ELSE NULL
    END as no2,

    -- Ozone (O3)
    -- Range: 0-1000 µg/m³ (0 to extreme pollution)
    -- Precision: 2 decimals (monitoring equipment accuracy)
    CASE
        WHEN o3 >= 0 AND o3 <= 1000
        THEN ROUND(o3, 2)
        ELSE NULL
    END as o3,

    -- Sulfur Dioxide (SO2)
    -- Range: 0-1000 µg/m³ (0 to extreme pollution)
    -- Precision: 2 decimals (monitoring equipment accuracy)
    CASE
        WHEN so2 >= 0 AND so2 <= 1000
        THEN ROUND(so2, 2)
        ELSE NULL
    END as so2,

    -- Particulate Matter 2.5 µm (PM2.5)
    -- Range: 0-1000 µg/m³ (0 to extreme pollution)
    -- Precision: 1 decimal (monitoring equipment accuracy)
    CASE
        WHEN pm2_5 >= 0 AND pm2_5 <= 1000
        THEN ROUND(pm2_5, 1)
        ELSE NULL
    END as pm2_5,

    -- Particulate Matter 10 µm (PM10)
    -- Range: 0-1000 µg/m³ (0 to extreme pollution)
    -- Precision: 1 decimal (monitoring equipment accuracy)
    CASE
        WHEN pm10 >= 0 AND pm10 <= 1000
        THEN ROUND(pm10, 1)
        ELSE NULL
    END as pm10,

    -- Ammonia (NH3)
    -- Range: 0-200 µg/m³ (0 to extreme pollution)
    -- Precision: 2 decimals (monitoring equipment accuracy)
    CASE
        WHEN nh3 >= 0 AND nh3 <= 200
        THEN ROUND(nh3, 2)
        ELSE NULL
    END as nh3

FROM read_parquet(
    '/data/outdoor-air-quality/**/*.parquet',
    union_by_name = true,  -- Handle schema evolution
    filename = true        -- Include file path for debugging
)
WHERE
    -- Filter out records with invalid timestamps
    timestamp IS NOT NULL

    -- Optional: Filter to recent data only (improve query performance)
    -- Uncomment for production if only recent data is needed:
    -- AND timestamp >= current_timestamp - INTERVAL '90 days'

ORDER BY timestamp DESC;

-- ============================================================================
-- View Metadata
-- ============================================================================
-- Expected row count: ~144 rows/day (1 reading/10 minutes)
-- Expected columns: 10 (timestamp + 9 measurements)
-- Nullable columns: co, no, no2, o3, so2, pm10, nh3
-- Required columns: timestamp, aqi, pm2_5
-- ============================================================================
