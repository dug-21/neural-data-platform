-- Silver Layer View: Indoor Air Quality
-- Feature: DP-001
-- Source: /data/data/air-quality/**/*.parquet (Bronze layer - long format)
-- Description: AirGradient sensor readings PIVOTed to wide format with ALL metrics exposed
--
-- Bronze Schema: timestamp, location_id, metric, value
-- Silver Schema: timestamp, location_id, + all available AirGradient metrics
--
-- Design Philosophy:
--   - Expose ALL available metrics (raw, compensated, counts, indexes)
--   - Let dashboards decide which to display
--   - Consistent snake_case naming
--   - Range validation with NULL for out-of-range values
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
-- Expose ALL AirGradient metrics with consistent snake_case naming
pivoted AS (
    SELECT
        ts as timestamp,
        location_id,

        -- ===== Particulate Matter (PM) =====
        -- PM1.0
        MAX(CASE WHEN metric = 'pm01' THEN value END) as pm01_raw,

        -- PM2.5 (main air quality indicator)
        MAX(CASE WHEN metric = 'pm02' THEN value END) as pm02_raw,
        MAX(CASE WHEN metric = 'pm02Compensated' THEN value END) as pm02_compensated_raw,
        MAX(CASE WHEN metric = 'pm02Standard' THEN value END) as pm02_standard_raw,

        -- PM10
        MAX(CASE WHEN metric = 'pm10' THEN value END) as pm10_raw,
        MAX(CASE WHEN metric = 'pm10Standard' THEN value END) as pm10_standard_raw,

        -- Particle counts (per 0.1L of air)
        MAX(CASE WHEN metric = 'pm003Count' THEN value END) as pm003_count_raw,
        MAX(CASE WHEN metric = 'pm005Count' THEN value END) as pm005_count_raw,
        MAX(CASE WHEN metric = 'pm01Count' THEN value END) as pm01_count_raw,
        MAX(CASE WHEN metric = 'pm02Count' THEN value END) as pm02_count_raw,
        MAX(CASE WHEN metric = 'pm50Count' THEN value END) as pm50_count_raw,

        -- ===== CO2 =====
        MAX(CASE WHEN metric = 'rco2' THEN value END) as co2_raw,

        -- ===== Temperature =====
        MAX(CASE WHEN metric IN ('atmp', 'temperature') THEN value END) as temperature_raw,
        MAX(CASE WHEN metric = 'atmpCompensated' THEN value END) as temperature_compensated_raw,

        -- ===== Humidity =====
        MAX(CASE WHEN metric IN ('rhum', 'humidity') THEN value END) as humidity_raw,
        MAX(CASE WHEN metric = 'rhumCompensated' THEN value END) as humidity_compensated_raw,

        -- ===== VOC (Volatile Organic Compounds) =====
        MAX(CASE WHEN metric = 'tvocIndex' THEN value END) as tvoc_index_raw,
        MAX(CASE WHEN metric = 'tvocRaw' THEN value END) as tvoc_raw_raw,

        -- ===== NOx (Nitrogen Oxides) =====
        MAX(CASE WHEN metric = 'noxIndex' THEN value END) as nox_index_raw,
        MAX(CASE WHEN metric = 'noxRaw' THEN value END) as nox_raw_raw,

        -- ===== WiFi Signal Strength =====
        MAX(CASE WHEN metric = 'wifi_strength' THEN value END) as wifi_strength_raw

    FROM bronze_data
    GROUP BY ts, location_id
)

-- Apply data quality validation
SELECT
    timestamp,
    location_id,

    -- ===== Particulate Matter (PM) - 0-500 ug/m3 range =====
    CASE WHEN pm01_raw >= 0 AND pm01_raw <= 500 THEN ROUND(pm01_raw, 1) END as pm01,

    CASE WHEN pm02_raw >= 0 AND pm02_raw <= 500 THEN ROUND(pm02_raw, 1) END as pm02,
    CASE WHEN pm02_compensated_raw >= 0 AND pm02_compensated_raw <= 500 THEN ROUND(pm02_compensated_raw, 1) END as pm02_compensated,
    CASE WHEN pm02_standard_raw >= 0 AND pm02_standard_raw <= 500 THEN ROUND(pm02_standard_raw, 1) END as pm02_standard,

    CASE WHEN pm10_raw >= 0 AND pm10_raw <= 1000 THEN ROUND(pm10_raw, 1) END as pm10,
    CASE WHEN pm10_standard_raw >= 0 AND pm10_standard_raw <= 1000 THEN ROUND(pm10_standard_raw, 1) END as pm10_standard,

    -- Particle counts (no upper limit validation - counts vary widely)
    CASE WHEN pm003_count_raw >= 0 THEN ROUND(pm003_count_raw, 0) END as pm003_count,
    CASE WHEN pm005_count_raw >= 0 THEN ROUND(pm005_count_raw, 0) END as pm005_count,
    CASE WHEN pm01_count_raw >= 0 THEN ROUND(pm01_count_raw, 0) END as pm01_count,
    CASE WHEN pm02_count_raw >= 0 THEN ROUND(pm02_count_raw, 0) END as pm02_count,
    CASE WHEN pm50_count_raw >= 0 THEN ROUND(pm50_count_raw, 0) END as pm50_count,

    -- ===== CO2: 400-5000 ppm (400=outdoor baseline, 5000=OSHA limit) =====
    CASE WHEN co2_raw >= 400 AND co2_raw <= 5000 THEN ROUND(co2_raw, 0) END as co2,

    -- ===== Temperature: -10 to 50C (indoor range) =====
    CASE WHEN temperature_raw >= -10 AND temperature_raw <= 50 THEN ROUND(temperature_raw, 1) END as temperature,
    CASE WHEN temperature_compensated_raw >= -10 AND temperature_compensated_raw <= 50 THEN ROUND(temperature_compensated_raw, 1) END as temperature_compensated,

    -- ===== Humidity: 0-100% =====
    CASE WHEN humidity_raw >= 0 AND humidity_raw <= 100 THEN ROUND(humidity_raw, 1) END as humidity,
    CASE WHEN humidity_compensated_raw >= 0 AND humidity_compensated_raw <= 100 THEN ROUND(humidity_compensated_raw, 1) END as humidity_compensated,

    -- ===== TVOC: Index 0-500, Raw 0-60000 =====
    CASE WHEN tvoc_index_raw >= 0 AND tvoc_index_raw <= 500 THEN ROUND(tvoc_index_raw, 0) END as tvoc_index,
    CASE WHEN tvoc_raw_raw >= 0 AND tvoc_raw_raw <= 60000 THEN ROUND(tvoc_raw_raw, 0) END as tvoc_raw,

    -- ===== NOx: Index 0-500, Raw 0-60000 =====
    CASE WHEN nox_index_raw >= 0 AND nox_index_raw <= 500 THEN ROUND(nox_index_raw, 0) END as nox_index,
    CASE WHEN nox_raw_raw >= 0 AND nox_raw_raw <= 60000 THEN ROUND(nox_raw_raw, 0) END as nox_raw,

    -- ===== WiFi: -100 to 0 dBm =====
    CASE WHEN wifi_strength_raw >= -100 AND wifi_strength_raw <= 0 THEN ROUND(wifi_strength_raw, 0) END as wifi_strength

FROM pivoted
ORDER BY timestamp DESC;

-- ============================================================================
-- View Metadata
-- ============================================================================
-- Source: Bronze layer (long format with metric column)
-- Transform: PIVOT to wide format with validation
-- Expected columns: 22 (timestamp, location_id, + 20 measurements)
--
-- Metric Categories:
--   - PM (6): pm01, pm02, pm02_compensated, pm02_standard, pm10, pm10_standard
--   - Particle Counts (5): pm003_count, pm005_count, pm01_count, pm02_count, pm50_count
--   - CO2 (1): co2
--   - Temperature (2): temperature, temperature_compensated
--   - Humidity (2): humidity, humidity_compensated
--   - TVOC (2): tvoc_index, tvoc_raw
--   - NOx (2): nox_index, nox_raw
--   - WiFi (1): wifi_strength
-- ============================================================================
