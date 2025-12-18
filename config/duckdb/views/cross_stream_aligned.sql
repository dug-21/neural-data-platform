-- Cross-Stream Aligned View
-- Feature: DP-001
-- Sources: Silver layer views (indoor air + outdoor weather + outdoor air quality)
-- Description: Time-aligned multi-stream data for correlation analysis
--
-- Alignment Strategy:
--   - 10-minute time buckets for all streams
--   - Indoor sensors: AVG() aggregation within bucket
--   - Outdoor APIs: FIRST() (already polled at bucket boundaries)
--   - FULL OUTER JOIN to preserve all data
--
-- Use Cases:
--   - Indoor/outdoor correlation analysis
--   - ML feature engineering
--   - Grafana dashboards with multiple streams
--
-- Performance: Optimized for 30-day queries (<15s target)

CREATE OR REPLACE VIEW cross_stream_aligned AS
WITH
    -- Indoor air quality aggregated to 10-minute buckets
    indoor_bucketed AS (
        SELECT
            time_bucket(INTERVAL '10 minutes', timestamp) as time_bucket,
            AVG(pm25) as indoor_pm25,
            AVG(pm10) as indoor_pm10,
            AVG(co2) as indoor_co2,
            AVG(temperature) as indoor_temp,
            AVG(humidity) as indoor_humidity,
            AVG(tvoc) as indoor_tvoc,
            AVG(nox) as indoor_nox,
            COUNT(*) as indoor_sample_count
        FROM silver_indoor_air
        WHERE timestamp IS NOT NULL
        GROUP BY time_bucket
    ),

    -- Outdoor weather (already at ~10-minute intervals)
    weather_bucketed AS (
        SELECT
            time_bucket(INTERVAL '10 minutes', timestamp) as time_bucket,
            FIRST(temperature) as outdoor_temp,
            FIRST(feels_like) as outdoor_feels_like,
            FIRST(pressure) as outdoor_pressure,
            FIRST(humidity) as outdoor_humidity,
            FIRST(wind_speed) as outdoor_wind_speed,
            FIRST(wind_deg) as outdoor_wind_deg,
            FIRST(wind_gust) as outdoor_wind_gust,
            FIRST(clouds) as outdoor_clouds,
            FIRST(visibility) as outdoor_visibility,
            FIRST(rain_1h) as outdoor_rain_1h,
            FIRST(snow_1h) as outdoor_snow_1h,
            COUNT(*) as weather_sample_count
        FROM silver_outdoor_weather
        WHERE timestamp IS NOT NULL
        GROUP BY time_bucket
    ),

    -- Outdoor air quality (already at ~10-minute intervals)
    air_quality_bucketed AS (
        SELECT
            time_bucket(INTERVAL '10 minutes', timestamp) as time_bucket,
            FIRST(aqi) as outdoor_aqi,
            FIRST(co) as outdoor_co,
            FIRST(no) as outdoor_no,
            FIRST(no2) as outdoor_no2,
            FIRST(o3) as outdoor_o3,
            FIRST(so2) as outdoor_so2,
            FIRST(pm2_5) as outdoor_pm2_5,
            FIRST(pm10) as outdoor_pm10,
            FIRST(nh3) as outdoor_nh3,
            COUNT(*) as air_quality_sample_count
        FROM silver_outdoor_air
        WHERE timestamp IS NOT NULL
        GROUP BY time_bucket
    )

-- Join all streams on time_bucket
SELECT
    -- Time bucket (primary key)
    COALESCE(
        indoor_bucketed.time_bucket,
        weather_bucketed.time_bucket,
        air_quality_bucketed.time_bucket
    ) as time_bucket,

    -- Indoor air quality measurements
    indoor_bucketed.indoor_pm25,
    indoor_bucketed.indoor_pm10,
    indoor_bucketed.indoor_co2,
    indoor_bucketed.indoor_temp,
    indoor_bucketed.indoor_humidity,
    indoor_bucketed.indoor_tvoc,
    indoor_bucketed.indoor_nox,
    indoor_bucketed.indoor_sample_count,

    -- Outdoor weather measurements
    weather_bucketed.outdoor_temp,
    weather_bucketed.outdoor_feels_like,
    weather_bucketed.outdoor_pressure,
    weather_bucketed.outdoor_humidity,
    weather_bucketed.outdoor_wind_speed,
    weather_bucketed.outdoor_wind_deg,
    weather_bucketed.outdoor_wind_gust,
    weather_bucketed.outdoor_clouds,
    weather_bucketed.outdoor_visibility,
    weather_bucketed.outdoor_rain_1h,
    weather_bucketed.outdoor_snow_1h,
    weather_bucketed.weather_sample_count,

    -- Outdoor air quality measurements
    air_quality_bucketed.outdoor_aqi,
    air_quality_bucketed.outdoor_co,
    air_quality_bucketed.outdoor_no,
    air_quality_bucketed.outdoor_no2,
    air_quality_bucketed.outdoor_o3,
    air_quality_bucketed.outdoor_so2,
    air_quality_bucketed.outdoor_pm2_5,
    air_quality_bucketed.outdoor_pm10,
    air_quality_bucketed.outdoor_nh3,
    air_quality_bucketed.air_quality_sample_count,

    -- Data completeness indicators
    CASE
        WHEN indoor_bucketed.time_bucket IS NOT NULL THEN 1
        ELSE 0
    END as has_indoor_data,

    CASE
        WHEN weather_bucketed.time_bucket IS NOT NULL THEN 1
        ELSE 0
    END as has_weather_data,

    CASE
        WHEN air_quality_bucketed.time_bucket IS NOT NULL THEN 1
        ELSE 0
    END as has_air_quality_data

FROM indoor_bucketed
FULL OUTER JOIN weather_bucketed
    USING (time_bucket)
FULL OUTER JOIN air_quality_bucketed
    USING (time_bucket)

-- Filter out NULL time buckets (should not happen but defensive)
WHERE COALESCE(
    indoor_bucketed.time_bucket,
    weather_bucketed.time_bucket,
    air_quality_bucketed.time_bucket
) IS NOT NULL

ORDER BY time_bucket DESC;

-- ============================================================================
-- View Metadata
-- ============================================================================
-- Source: Silver layer views (wide format after PIVOT)
-- Expected row count: ~144 buckets/day
-- Expected columns: 34 (time_bucket + 30 measurements + 3 flags)
-- Join strategy: FULL OUTER JOIN (preserves all data)
-- ============================================================================
