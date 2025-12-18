-- Silver Layer View: Indoor Air Quality
-- Feature: DP-001
-- Source: /data/air-quality/*.parquet
-- Description: AirGradient sensor readings with data quality validation
--
-- Quality Rules:
--   - Range validation for all numeric fields
--   - NULL handling for optional fields
--   - Rounding to appropriate precision
--   - Timestamp validation (non-NULL required)
--
-- Performance: Optimized for 7-day queries (<5s target)

CREATE OR REPLACE VIEW silver_indoor_air AS
SELECT
    -- Timestamp (required field)
    timestamp,

    -- Particulate Matter 2.5 µm (PM2.5)
    -- Range: 0-500 µg/m³ (EPA AQI scale max)
    -- Precision: 1 decimal (sensor accuracy ±10%)
    CASE
        WHEN pm25 >= 0 AND pm25 <= 500
        THEN ROUND(pm25, 1)
        ELSE NULL
    END as pm25,

    -- Particulate Matter 10 µm (PM10)
    -- Range: 0-1000 µg/m³ (typical max for indoor)
    -- Precision: 1 decimal (sensor accuracy ±10%)
    CASE
        WHEN pm10 >= 0 AND pm10 <= 1000
        THEN ROUND(pm10, 1)
        ELSE NULL
    END as pm10,

    -- Carbon Dioxide (CO2)
    -- Range: 400-5000 ppm (400 = outdoor, 5000 = OSHA limit)
    -- Precision: 0 decimals (sensor reports integers)
    CASE
        WHEN co2 >= 400 AND co2 <= 5000
        THEN ROUND(co2, 0)
        ELSE NULL
    END as co2,

    -- Temperature
    -- Range: -10 to 50°C (realistic indoor range)
    -- Precision: 1 decimal (sensor accuracy ±0.5°C)
    CASE
        WHEN temperature >= -10 AND temperature <= 50
        THEN ROUND(temperature, 1)
        ELSE NULL
    END as temperature,

    -- Relative Humidity
    -- Range: 0-100% (physical limits)
    -- Precision: 1 decimal (sensor accuracy ±2%)
    CASE
        WHEN humidity >= 0 AND humidity <= 100
        THEN ROUND(humidity, 1)
        ELSE NULL
    END as humidity,

    -- Total Volatile Organic Compounds (TVOC)
    -- Range: 0-60000 ppb (sensor max)
    -- Precision: 0 decimals (sensor reports integers)
    CASE
        WHEN tvoc >= 0 AND tvoc <= 60000
        THEN ROUND(tvoc, 0)
        ELSE NULL
    END as tvoc,

    -- Nitrogen Oxides (NOx)
    -- Range: 0-1000 ppb (typical indoor max)
    -- Precision: 0 decimals (sensor reports integers)
    CASE
        WHEN nox >= 0 AND nox <= 1000
        THEN ROUND(nox, 0)
        ELSE NULL
    END as nox

FROM read_parquet(
    '/data/air-quality/**/*.parquet',
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
-- Expected row count: ~1440 rows/day (1 reading/minute)
-- Expected columns: 8 (timestamp + 7 measurements)
-- Nullable columns: pm10, co2, temperature, humidity, tvoc, nox
-- Required columns: timestamp, pm25
-- ============================================================================
