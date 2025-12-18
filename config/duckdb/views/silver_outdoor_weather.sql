-- Silver Layer View: Outdoor Weather
-- Feature: DP-001
-- Source: /data/data/outdoor-weather/**/*.parquet (Bronze layer - long format)
-- Description: OpenWeatherMap Current Weather API data PIVOTed to wide format
--
-- Bronze Schema: timestamp, location_id, metric, value
-- Silver Schema: timestamp, location_id, temperature, feels_like, pressure, humidity,
--                wind_speed, wind_deg, wind_gust, clouds, visibility, rain_1h, snow_1h
--
-- Quality Rules:
--   - Range validation per OpenWeatherMap API specs
--   - NULL handling for optional fields
--   - Rounding to meteorological precision standards
--
-- Performance: Optimized for 7-day queries (<5s target)

CREATE OR REPLACE VIEW silver_outdoor_weather AS
WITH bronze_data AS (
    SELECT
        -- Convert microseconds to timestamp
        to_timestamp(timestamp / 1000000) as ts,
        location_id,
        metric,
        value
    FROM read_parquet(
        '/data/outdoor-weather/**/*.parquet',
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
        MAX(CASE WHEN metric = 'temperature' THEN value END) as temperature_raw,
        MAX(CASE WHEN metric = 'feels_like' THEN value END) as feels_like_raw,
        MAX(CASE WHEN metric = 'pressure' THEN value END) as pressure_raw,
        MAX(CASE WHEN metric = 'humidity' THEN value END) as humidity_raw,
        MAX(CASE WHEN metric = 'wind_speed' THEN value END) as wind_speed_raw,
        MAX(CASE WHEN metric = 'wind_deg' THEN value END) as wind_deg_raw,
        MAX(CASE WHEN metric = 'wind_gust' THEN value END) as wind_gust_raw,
        MAX(CASE WHEN metric = 'clouds' THEN value END) as clouds_raw,
        MAX(CASE WHEN metric = 'visibility' THEN value END) as visibility_raw,
        MAX(CASE WHEN metric = 'rain_1h' THEN value END) as rain_1h_raw,
        MAX(CASE WHEN metric = 'snow_1h' THEN value END) as snow_1h_raw
    FROM bronze_data
    GROUP BY ts, location_id
)

-- Apply data quality validation
SELECT
    timestamp,
    location_id,

    -- Temperature: -50 to 60°C (global extremes), 1 decimal
    CASE
        WHEN temperature_raw >= -50 AND temperature_raw <= 60
        THEN ROUND(temperature_raw, 1)
        ELSE NULL
    END as temperature,

    -- Feels Like: -50 to 60°C, 1 decimal
    CASE
        WHEN feels_like_raw >= -50 AND feels_like_raw <= 60
        THEN ROUND(feels_like_raw, 1)
        ELSE NULL
    END as feels_like,

    -- Pressure: 800-1200 hPa, 1 decimal
    CASE
        WHEN pressure_raw >= 800 AND pressure_raw <= 1200
        THEN ROUND(pressure_raw, 1)
        ELSE NULL
    END as pressure,

    -- Humidity: 0-100%, 1 decimal
    CASE
        WHEN humidity_raw >= 0 AND humidity_raw <= 100
        THEN ROUND(humidity_raw, 1)
        ELSE NULL
    END as humidity,

    -- Wind Speed: 0-100 m/s, 2 decimals
    CASE
        WHEN wind_speed_raw >= 0 AND wind_speed_raw <= 100
        THEN ROUND(wind_speed_raw, 2)
        ELSE NULL
    END as wind_speed,

    -- Wind Direction: 0-360 degrees, integer
    CASE
        WHEN wind_deg_raw >= 0 AND wind_deg_raw <= 360
        THEN ROUND(wind_deg_raw, 0)
        ELSE NULL
    END as wind_deg,

    -- Wind Gust: 0-150 m/s, 2 decimals
    CASE
        WHEN wind_gust_raw >= 0 AND wind_gust_raw <= 150
        THEN ROUND(wind_gust_raw, 2)
        ELSE NULL
    END as wind_gust,

    -- Clouds: 0-100%, integer
    CASE
        WHEN clouds_raw >= 0 AND clouds_raw <= 100
        THEN ROUND(clouds_raw, 0)
        ELSE NULL
    END as clouds,

    -- Visibility: 0-50000 meters, integer
    CASE
        WHEN visibility_raw >= 0 AND visibility_raw <= 50000
        THEN ROUND(visibility_raw, 0)
        ELSE NULL
    END as visibility,

    -- Rain 1h: 0-500 mm, 2 decimals
    CASE
        WHEN rain_1h_raw >= 0 AND rain_1h_raw <= 500
        THEN ROUND(rain_1h_raw, 2)
        ELSE NULL
    END as rain_1h,

    -- Snow 1h: 0-500 mm, 2 decimals
    CASE
        WHEN snow_1h_raw >= 0 AND snow_1h_raw <= 500
        THEN ROUND(snow_1h_raw, 2)
        ELSE NULL
    END as snow_1h

FROM pivoted
ORDER BY timestamp DESC;

-- ============================================================================
-- View Metadata
-- ============================================================================
-- Source: Bronze layer (long format with metric column)
-- Transform: PIVOT to wide format with validation
-- Expected columns: 13 (timestamp, location_id, + 11 measurements)
-- ============================================================================
