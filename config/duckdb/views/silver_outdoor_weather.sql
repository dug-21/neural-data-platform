-- Silver Layer View: Outdoor Weather
-- Feature: DP-001
-- Source: /data/outdoor-weather/*.parquet
-- Description: OpenWeatherMap Current Weather API data with validation
--
-- Quality Rules:
--   - Range validation per OpenWeatherMap API specs
--   - NULL handling for optional fields
--   - Rounding to meteorological precision standards
--   - Timestamp validation (non-NULL required)
--
-- Performance: Optimized for 7-day queries (<5s target)

CREATE OR REPLACE VIEW silver_outdoor_weather AS
SELECT
    -- Timestamp (required field)
    timestamp,

    -- Temperature
    -- Range: -50 to 60°C (global extremes)
    -- Precision: 1 decimal (standard meteorological)
    CASE
        WHEN temperature >= -50 AND temperature <= 60
        THEN ROUND(temperature, 1)
        ELSE NULL
    END as temperature,

    -- Feels Like Temperature (wind chill / heat index)
    -- Range: -50 to 60°C (global extremes)
    -- Precision: 1 decimal (standard meteorological)
    CASE
        WHEN feels_like >= -50 AND feels_like <= 60
        THEN ROUND(feels_like, 1)
        ELSE NULL
    END as feels_like,

    -- Atmospheric Pressure
    -- Range: 800-1200 hPa (typhoon low to record high)
    -- Precision: 1 decimal (barometer accuracy)
    CASE
        WHEN pressure >= 800 AND pressure <= 1200
        THEN ROUND(pressure, 1)
        ELSE NULL
    END as pressure,

    -- Relative Humidity
    -- Range: 0-100% (physical limits)
    -- Precision: 1 decimal (standard meteorological)
    CASE
        WHEN humidity >= 0 AND humidity <= 100
        THEN ROUND(humidity, 1)
        ELSE NULL
    END as humidity,

    -- Wind Speed
    -- Range: 0-100 m/s (0 to hurricane force)
    -- Precision: 2 decimals (anemometer precision)
    CASE
        WHEN wind_speed >= 0 AND wind_speed <= 100
        THEN ROUND(wind_speed, 2)
        ELSE NULL
    END as wind_speed,

    -- Wind Direction
    -- Range: 0-360 degrees (compass bearing)
    -- Precision: 0 decimals (wind vane accuracy ±10°)
    CASE
        WHEN wind_deg >= 0 AND wind_deg <= 360
        THEN ROUND(wind_deg, 0)
        ELSE NULL
    END as wind_deg,

    -- Wind Gust
    -- Range: 0-150 m/s (0 to extreme event)
    -- Precision: 2 decimals (anemometer precision)
    CASE
        WHEN wind_gust >= 0 AND wind_gust <= 150
        THEN ROUND(wind_gust, 2)
        ELSE NULL
    END as wind_gust,

    -- Cloud Cover
    -- Range: 0-100% (clear to overcast)
    -- Precision: 0 decimals (oktas converted to percent)
    CASE
        WHEN clouds >= 0 AND clouds <= 100
        THEN ROUND(clouds, 0)
        ELSE NULL
    END as clouds,

    -- Visibility
    -- Range: 0-50000 meters (fog to clear)
    -- Precision: 0 decimals (observer accuracy ~100m)
    CASE
        WHEN visibility >= 0 AND visibility <= 50000
        THEN ROUND(visibility, 0)
        ELSE NULL
    END as visibility,

    -- Precipitation - Rain (1 hour)
    -- Range: 0-500 mm (0 to extreme rainfall)
    -- Precision: 2 decimals (rain gauge accuracy)
    CASE
        WHEN rain_1h >= 0 AND rain_1h <= 500
        THEN ROUND(rain_1h, 2)
        ELSE NULL
    END as rain_1h,

    -- Precipitation - Snow (1 hour)
    -- Range: 0-500 mm (0 to extreme snowfall)
    -- Precision: 2 decimals (snow gauge accuracy)
    CASE
        WHEN snow_1h >= 0 AND snow_1h <= 500
        THEN ROUND(snow_1h, 2)
        ELSE NULL
    END as snow_1h

FROM read_parquet(
    '/data/data/outdoor-weather/**/*.parquet',
    union_by_name = true,  -- Handle schema evolution
    filename = true,       -- Include file path for debugging
    hive_partitioning = true  -- Parse year/month/day from path
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
-- Expected columns: 12 (timestamp + 11 measurements)
-- Nullable columns: feels_like, pressure, humidity, wind_speed, wind_deg,
--                   wind_gust, clouds, visibility, rain_1h, snow_1h
-- Required columns: timestamp, temperature
-- ============================================================================
