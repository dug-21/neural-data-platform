-- Hourly Aggregation View for All Streams
-- Feature: DP-001 - Grafana Dashboard Support
-- Author: ndp-grafana-dev
-- Date: 2025-12-18
--
-- Purpose: Provide hourly aggregations across all streams for dashboard performance
--
-- Performance: Pre-aggregated hourly data for fast 7-30 day queries
--
-- Columns:
--   - bucket: Hourly time bucket (timestamp)
--   - stream_id: Stream identifier (air-quality, outdoor-conditions, outdoor-air-quality)
--   - Aggregated metrics: avg_*, max_*, min_* for each stream's fields

CREATE OR REPLACE VIEW readings_hourly AS
-- Indoor Air Quality Stream (air-quality)
SELECT
    time_bucket(INTERVAL '1 hour', timestamp) as bucket,
    'air-quality' as stream_id,

    -- PM2.5 aggregations
    ROUND(AVG(pm25), 1) as avg_pm25,
    ROUND(MAX(pm25), 1) as max_pm25,
    ROUND(MIN(pm25), 1) as min_pm25,

    -- PM10 aggregations
    ROUND(AVG(pm10), 1) as avg_pm10,
    ROUND(MAX(pm10), 1) as max_pm10,
    ROUND(MIN(pm10), 1) as min_pm10,

    -- CO2 aggregations
    ROUND(AVG(co2), 0) as avg_co2,
    ROUND(MAX(co2), 0) as max_co2,
    ROUND(MIN(co2), 0) as min_co2,

    -- Temperature aggregations
    ROUND(AVG(temperature), 1) as avg_temperature,
    ROUND(MAX(temperature), 1) as max_temperature,
    ROUND(MIN(temperature), 1) as min_temperature,

    -- Humidity aggregations
    ROUND(AVG(humidity), 1) as avg_humidity,
    ROUND(MAX(humidity), 1) as max_humidity,
    ROUND(MIN(humidity), 1) as min_humidity,

    -- TVOC aggregations
    ROUND(AVG(tvoc), 0) as avg_tvoc,
    ROUND(MAX(tvoc), 0) as max_tvoc,
    ROUND(MIN(tvoc), 0) as min_tvoc,

    -- NOx aggregations
    ROUND(AVG(nox), 0) as avg_nox,
    ROUND(MAX(nox), 0) as max_nox,
    ROUND(MIN(nox), 0) as min_nox,

    -- Outdoor weather placeholders (NULL for indoor stream)
    NULL as avg_apparent_temperature,
    NULL as avg_wind_speed,
    NULL as avg_pressure,
    NULL as avg_cloud_cover,

    -- Outdoor air quality placeholders (NULL for indoor stream)
    NULL as avg_pm2_5,
    NULL as avg_us_aqi,
    NULL as avg_no2,
    NULL as avg_o3,
    NULL as avg_so2,
    NULL as avg_co

FROM silver_indoor_air
WHERE timestamp IS NOT NULL
GROUP BY time_bucket(INTERVAL '1 hour', timestamp)

UNION ALL

-- Outdoor Weather Stream (outdoor-conditions)
SELECT
    time_bucket(INTERVAL '1 hour', timestamp) as bucket,
    'outdoor-conditions' as stream_id,

    -- Indoor air quality placeholders (NULL for outdoor weather stream)
    NULL as avg_pm25,
    NULL as max_pm25,
    NULL as min_pm25,
    NULL as avg_pm10,
    NULL as max_pm10,
    NULL as min_pm10,
    NULL as avg_co2,
    NULL as max_co2,
    NULL as min_co2,

    -- Temperature aggregations
    ROUND(AVG(temperature), 1) as avg_temperature,
    ROUND(MAX(temperature), 1) as max_temperature,
    ROUND(MIN(temperature), 1) as min_temperature,

    -- Humidity aggregations
    ROUND(AVG(humidity), 1) as avg_humidity,
    ROUND(MAX(humidity), 1) as max_humidity,
    ROUND(MIN(humidity), 1) as min_humidity,

    -- Indoor TVOC/NOx placeholders (NULL for outdoor stream)
    NULL as avg_tvoc,
    NULL as max_tvoc,
    NULL as min_tvoc,
    NULL as avg_nox,
    NULL as max_nox,
    NULL as min_nox,

    -- Feels Like temperature (corrected field name)
    ROUND(AVG(feels_like), 1) as avg_apparent_temperature,

    -- Wind speed
    ROUND(AVG(wind_speed), 2) as avg_wind_speed,

    -- Pressure
    ROUND(AVG(pressure), 1) as avg_pressure,

    -- Cloud cover (corrected field name)
    ROUND(AVG(clouds), 0) as avg_cloud_cover,

    -- Outdoor air quality placeholders (NULL for weather stream)
    NULL as avg_pm2_5,
    NULL as avg_us_aqi,
    NULL as avg_no2,
    NULL as avg_o3,
    NULL as avg_so2,
    NULL as avg_co

FROM silver_outdoor_weather
WHERE timestamp IS NOT NULL
GROUP BY time_bucket(INTERVAL '1 hour', timestamp)

UNION ALL

-- Outdoor Air Quality Stream (outdoor-air-quality)
SELECT
    time_bucket(INTERVAL '1 hour', timestamp) as bucket,
    'outdoor-air-quality' as stream_id,

    -- Indoor air quality placeholders (NULL for outdoor air stream)
    NULL as avg_pm25,
    NULL as max_pm25,
    NULL as min_pm25,
    NULL as avg_pm10,
    NULL as max_pm10,
    NULL as min_pm10,
    NULL as avg_co2,
    NULL as max_co2,
    NULL as min_co2,
    NULL as avg_temperature,
    NULL as max_temperature,
    NULL as min_temperature,
    NULL as avg_humidity,
    NULL as max_humidity,
    NULL as min_humidity,
    NULL as avg_tvoc,
    NULL as max_tvoc,
    NULL as min_tvoc,
    NULL as avg_nox,
    NULL as max_nox,
    NULL as min_nox,

    -- Weather placeholders (NULL for air quality stream)
    NULL as avg_apparent_temperature,
    NULL as avg_wind_speed,
    NULL as avg_pressure,
    NULL as avg_cloud_cover,

    -- Outdoor PM2.5 (different field name than indoor)
    ROUND(AVG(pm2_5), 1) as avg_pm2_5,

    -- US AQI (convert from OpenWeatherMap 1-5 scale to US AQI)
    -- 1=Good (0-50), 2=Moderate (51-100), 3=Unhealthy for Sensitive (101-150)
    -- 4=Unhealthy (151-200), 5=Very Unhealthy (201-300)
    ROUND(AVG(
        CASE
            WHEN aqi = 1 THEN 25
            WHEN aqi = 2 THEN 75
            WHEN aqi = 3 THEN 125
            WHEN aqi = 4 THEN 175
            WHEN aqi = 5 THEN 250
            ELSE NULL
        END
    ), 0) as avg_us_aqi,

    -- NO2
    ROUND(AVG(no2), 2) as avg_no2,

    -- O3
    ROUND(AVG(o3), 2) as avg_o3,

    -- SO2
    ROUND(AVG(so2), 2) as avg_so2,

    -- CO
    ROUND(AVG(co), 1) as avg_co

FROM silver_outdoor_air
WHERE timestamp IS NOT NULL
GROUP BY time_bucket(INTERVAL '1 hour', timestamp)

ORDER BY bucket DESC, stream_id;

-- ============================================================================
-- View Metadata
-- ============================================================================
-- Expected row count: ~24 rows/day per stream (72 total)
-- Expected columns: 32 (bucket, stream_id + 30 metric columns)
-- Performance: Fast aggregation for 7-30 day dashboard queries
-- ============================================================================
