-- Silver Layer View: Indoor Air Quality
-- Feature: DP-001
-- Source: /data/data/air-quality/**/*.parquet (Bronze layer - long format)
-- Description: AirGradient sensor readings PIVOTed to wide format with validation
--
-- Bronze Schema: timestamp, location_id, metric, value
-- Silver Schema: timestamp, location_id, pm25, pm10, co2, temperature, humidity, tvoc, nox
--
-- Quality Rules:
--   - Range validation for all numeric fields
--   - NULL for out-of-range values
--   - Rounding to appropriate precision
--
-- Performance: Optimized for 7-day queries (<5s target)

CREATE OR REPLACE VIEW silver_indoor_air AS
WITH bronze_data AS (
    SELECT
        -- Convert microseconds to timestamp
        to_timestamp(timestamp / 1000000) as ts,
        location_id,
        metric,
        value
    FROM read_parquet(
        '/data/data/air-quality/**/*.parquet',
        union_by_name = true,
        filename = true,
        hive_partitioning = true
    )
    WHERE timestamp IS NOT NULL
),

-- PIVOT from long format to wide format
-- Note: Maps from AirGradient sensor field names to Silver schema names
-- Sensor sends: pm02, rco2, atmp, rhum, tvocIndex, noxIndex, pm10
-- Silver exposes: pm25, co2, temperature, humidity, tvoc, nox, pm10
pivoted AS (
    SELECT
        ts as timestamp,
        location_id,
        -- pm02 in Bronze → pm25 in Silver (PM2.5 measurement)
        MAX(CASE WHEN metric = 'pm02' THEN value END) as pm25_raw,
        -- pm10 or pm10Standard → pm10
        COALESCE(
            MAX(CASE WHEN metric = 'pm10' THEN value END),
            MAX(CASE WHEN metric = 'pm10Standard' THEN value END)
        ) as pm10_raw,
        -- rco2 in Bronze → co2 in Silver (some parsers write as 'co2')
        COALESCE(
            MAX(CASE WHEN metric = 'rco2' THEN value END),
            MAX(CASE WHEN metric = 'co2' THEN value END)
        ) as co2_raw,
        -- atmp in Bronze → temperature in Silver (some parsers write as 'temperature')
        COALESCE(
            MAX(CASE WHEN metric = 'atmp' THEN value END),
            MAX(CASE WHEN metric = 'temperature' THEN value END)
        ) as temperature_raw,
        -- rhum in Bronze → humidity in Silver (some parsers write as 'humidity')
        COALESCE(
            MAX(CASE WHEN metric = 'rhum' THEN value END),
            MAX(CASE WHEN metric = 'humidity' THEN value END)
        ) as humidity_raw,
        -- tvocIndex in Bronze → tvoc in Silver
        MAX(CASE WHEN metric = 'tvocIndex' THEN value END) as tvoc_raw,
        -- noxIndex in Bronze → nox in Silver
        MAX(CASE WHEN metric = 'noxIndex' THEN value END) as nox_raw
    FROM bronze_data
    GROUP BY ts, location_id
)

-- Apply data quality validation
SELECT
    timestamp,
    location_id,

    -- PM2.5: 0-500 µg/m³ (EPA AQI scale max), 1 decimal precision
    CASE
        WHEN pm25_raw >= 0 AND pm25_raw <= 500
        THEN ROUND(pm25_raw, 1)
        ELSE NULL
    END as pm25,

    -- PM10: 0-1000 µg/m³, 1 decimal precision
    CASE
        WHEN pm10_raw >= 0 AND pm10_raw <= 1000
        THEN ROUND(pm10_raw, 1)
        ELSE NULL
    END as pm10,

    -- CO2: 400-5000 ppm (400=outdoor, 5000=OSHA limit), integer
    CASE
        WHEN co2_raw >= 400 AND co2_raw <= 5000
        THEN ROUND(co2_raw, 0)
        ELSE NULL
    END as co2,

    -- Temperature: -10 to 50°C (indoor range), 1 decimal
    CASE
        WHEN temperature_raw >= -10 AND temperature_raw <= 50
        THEN ROUND(temperature_raw, 1)
        ELSE NULL
    END as temperature,

    -- Humidity: 0-100%, 1 decimal
    CASE
        WHEN humidity_raw >= 0 AND humidity_raw <= 100
        THEN ROUND(humidity_raw, 1)
        ELSE NULL
    END as humidity,

    -- TVOC: 0-60000 ppb (sensor max), integer
    CASE
        WHEN tvoc_raw >= 0 AND tvoc_raw <= 60000
        THEN ROUND(tvoc_raw, 0)
        ELSE NULL
    END as tvoc,

    -- NOx: 0-1000 ppb, integer
    CASE
        WHEN nox_raw >= 0 AND nox_raw <= 1000
        THEN ROUND(nox_raw, 0)
        ELSE NULL
    END as nox

FROM pivoted
ORDER BY timestamp DESC;

-- ============================================================================
-- View Metadata
-- ============================================================================
-- Source: Bronze layer (long format with metric column)
-- Transform: PIVOT to wide format with validation
-- Expected columns: 9 (timestamp, location_id, + 7 measurements)
-- ============================================================================
