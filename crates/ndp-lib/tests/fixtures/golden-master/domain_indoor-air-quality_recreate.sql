-- Aligned view for domain: indoor-air-quality
-- Streams: indoor, outdoor, state, outdoor_aqi
-- Mode: RECREATE (drop and create)

-- Drop existing view
DROP MATERIALIZED VIEW IF EXISTS gold.indoor_air_quality_aligned CASCADE;

-- Create aligned view
CREATE MATERIALIZED VIEW gold.indoor_air_quality_aligned AS
SELECT
            COALESCE(indoor.bucket, outdoor.bucket, state.bucket, outdoor_aqi.bucket) AS bucket,

    -- indoor (Observation)
            indoor.co2_mean AS indoor_co2_mean,
            indoor.co2_std AS indoor_co2_std,
            indoor.co2_min AS indoor_co2_min,
            indoor.co2_max AS indoor_co2_max,
            indoor.humidity_pct_mean AS indoor_humidity_pct_mean,
            indoor.humidity_pct_min AS indoor_humidity_pct_min,
            indoor.humidity_pct_max AS indoor_humidity_pct_max,
            indoor.nox_index_mean AS indoor_nox_index_mean,
            indoor.nox_index_max AS indoor_nox_index_max,
            indoor.pm10_mean AS indoor_pm10_mean,
            indoor.pm10_min AS indoor_pm10_min,
            indoor.pm10_max AS indoor_pm10_max,
            indoor.pm25_mean AS indoor_pm25_mean,
            indoor.pm25_std AS indoor_pm25_std,
            indoor.pm25_min AS indoor_pm25_min,
            indoor.pm25_max AS indoor_pm25_max,
            indoor.pm25_p95 AS indoor_pm25_p95,
            indoor.temperature_c_mean AS indoor_temperature_c_mean,
            indoor.temperature_c_min AS indoor_temperature_c_min,
            indoor.temperature_c_max AS indoor_temperature_c_max,
            indoor.tvoc_index_mean AS indoor_tvoc_index_mean,
            indoor.tvoc_index_max AS indoor_tvoc_index_max,
            indoor.sample_count AS indoor_sample_count,
            COALESCE(indoor.sample_count, 0) AS indoor_samples,

    -- outdoor (Observation)
            outdoor.cloud_cover_pct_mean AS outdoor_cloud_cover_pct_mean,
            outdoor.feels_like_c_mean AS outdoor_feels_like_c_mean,
            outdoor.feels_like_c_min AS outdoor_feels_like_c_min,
            outdoor.feels_like_c_max AS outdoor_feels_like_c_max,
            outdoor.humidity_pct_mean AS outdoor_humidity_pct_mean,
            outdoor.humidity_pct_min AS outdoor_humidity_pct_min,
            outdoor.humidity_pct_max AS outdoor_humidity_pct_max,
            outdoor.precipitation_mm_mean AS outdoor_precipitation_mm_mean,
            outdoor.precipitation_mm_max AS outdoor_precipitation_mm_max,
            outdoor.precipitation_mm_count AS outdoor_precipitation_mm_count,
            outdoor.pressure_pa_mean AS outdoor_pressure_pa_mean,
            outdoor.temperature_c_mean AS outdoor_temperature_c_mean,
            outdoor.temperature_c_min AS outdoor_temperature_c_min,
            outdoor.temperature_c_max AS outdoor_temperature_c_max,
            outdoor.visibility_m_mean AS outdoor_visibility_m_mean,
            outdoor.visibility_m_min AS outdoor_visibility_m_min,
            outdoor.wind_speed_kmh_mean AS outdoor_wind_speed_kmh_mean,
            outdoor.wind_speed_kmh_max AS outdoor_wind_speed_kmh_max,
            outdoor.sample_count AS outdoor_sample_count,
            COALESCE(outdoor.sample_count, 0) AS outdoor_samples,

    -- state (StateEvent)
            COALESCE(
        state.state_count,
        LAG(state.state_count, 1) OVER (ORDER BY COALESCE(indoor.bucket, outdoor.bucket, state.bucket, outdoor_aqi.bucket)),
        LAG(state.state_count, 2) OVER (ORDER BY COALESCE(indoor.bucket, outdoor.bucket, state.bucket, outdoor_aqi.bucket)),
        LAG(state.state_count, 3) OVER (ORDER BY COALESCE(indoor.bucket, outdoor.bucket, state.bucket, outdoor_aqi.bucket)),
        LAG(state.state_count, 4) OVER (ORDER BY COALESCE(indoor.bucket, outdoor.bucket, state.bucket, outdoor_aqi.bucket)),
        LAG(state.state_count, 5) OVER (ORDER BY COALESCE(indoor.bucket, outdoor.bucket, state.bucket, outdoor_aqi.bucket)),
        LAG(state.state_count, 6) OVER (ORDER BY COALESCE(indoor.bucket, outdoor.bucket, state.bucket, outdoor_aqi.bucket))
    ) AS state_state_count,
            COALESCE(
        state.state_first,
        LAG(state.state_first, 1) OVER (ORDER BY COALESCE(indoor.bucket, outdoor.bucket, state.bucket, outdoor_aqi.bucket)),
        LAG(state.state_first, 2) OVER (ORDER BY COALESCE(indoor.bucket, outdoor.bucket, state.bucket, outdoor_aqi.bucket)),
        LAG(state.state_first, 3) OVER (ORDER BY COALESCE(indoor.bucket, outdoor.bucket, state.bucket, outdoor_aqi.bucket)),
        LAG(state.state_first, 4) OVER (ORDER BY COALESCE(indoor.bucket, outdoor.bucket, state.bucket, outdoor_aqi.bucket)),
        LAG(state.state_first, 5) OVER (ORDER BY COALESCE(indoor.bucket, outdoor.bucket, state.bucket, outdoor_aqi.bucket)),
        LAG(state.state_first, 6) OVER (ORDER BY COALESCE(indoor.bucket, outdoor.bucket, state.bucket, outdoor_aqi.bucket))
    ) AS state_state_first,
            COALESCE(
        state.state_last,
        LAG(state.state_last, 1) OVER (ORDER BY COALESCE(indoor.bucket, outdoor.bucket, state.bucket, outdoor_aqi.bucket)),
        LAG(state.state_last, 2) OVER (ORDER BY COALESCE(indoor.bucket, outdoor.bucket, state.bucket, outdoor_aqi.bucket)),
        LAG(state.state_last, 3) OVER (ORDER BY COALESCE(indoor.bucket, outdoor.bucket, state.bucket, outdoor_aqi.bucket)),
        LAG(state.state_last, 4) OVER (ORDER BY COALESCE(indoor.bucket, outdoor.bucket, state.bucket, outdoor_aqi.bucket)),
        LAG(state.state_last, 5) OVER (ORDER BY COALESCE(indoor.bucket, outdoor.bucket, state.bucket, outdoor_aqi.bucket)),
        LAG(state.state_last, 6) OVER (ORDER BY COALESCE(indoor.bucket, outdoor.bucket, state.bucket, outdoor_aqi.bucket))
    ) AS state_state_last,
            COALESCE(
        state.sample_count,
        LAG(state.sample_count, 1) OVER (ORDER BY COALESCE(indoor.bucket, outdoor.bucket, state.bucket, outdoor_aqi.bucket)),
        LAG(state.sample_count, 2) OVER (ORDER BY COALESCE(indoor.bucket, outdoor.bucket, state.bucket, outdoor_aqi.bucket)),
        LAG(state.sample_count, 3) OVER (ORDER BY COALESCE(indoor.bucket, outdoor.bucket, state.bucket, outdoor_aqi.bucket)),
        LAG(state.sample_count, 4) OVER (ORDER BY COALESCE(indoor.bucket, outdoor.bucket, state.bucket, outdoor_aqi.bucket)),
        LAG(state.sample_count, 5) OVER (ORDER BY COALESCE(indoor.bucket, outdoor.bucket, state.bucket, outdoor_aqi.bucket)),
        LAG(state.sample_count, 6) OVER (ORDER BY COALESCE(indoor.bucket, outdoor.bucket, state.bucket, outdoor_aqi.bucket))
    ) AS state_sample_count,
            COALESCE(state.sample_count, 0) AS state_samples,

    -- outdoor_aqi (Observation)
            outdoor_aqi.aqi_epa_mean AS outdoor_aqi_aqi_epa_mean,
            outdoor_aqi.aqi_epa_min AS outdoor_aqi_aqi_epa_min,
            outdoor_aqi.aqi_epa_max AS outdoor_aqi_aqi_epa_max,
            outdoor_aqi.aqi_owm_mean AS outdoor_aqi_aqi_owm_mean,
            outdoor_aqi.aqi_owm_min AS outdoor_aqi_aqi_owm_min,
            outdoor_aqi.aqi_owm_max AS outdoor_aqi_aqi_owm_max,
            outdoor_aqi.co_ugm3_mean AS outdoor_aqi_co_ugm3_mean,
            outdoor_aqi.co_ugm3_max AS outdoor_aqi_co_ugm3_max,
            outdoor_aqi.no2_ugm3_mean AS outdoor_aqi_no2_ugm3_mean,
            outdoor_aqi.no2_ugm3_max AS outdoor_aqi_no2_ugm3_max,
            outdoor_aqi.o3_ugm3_mean AS outdoor_aqi_o3_ugm3_mean,
            outdoor_aqi.o3_ugm3_max AS outdoor_aqi_o3_ugm3_max,
            outdoor_aqi.pm10_mean AS outdoor_aqi_pm10_mean,
            outdoor_aqi.pm10_min AS outdoor_aqi_pm10_min,
            outdoor_aqi.pm10_max AS outdoor_aqi_pm10_max,
            outdoor_aqi.pm25_mean AS outdoor_aqi_pm25_mean,
            outdoor_aqi.pm25_std AS outdoor_aqi_pm25_std,
            outdoor_aqi.pm25_min AS outdoor_aqi_pm25_min,
            outdoor_aqi.pm25_max AS outdoor_aqi_pm25_max,
            outdoor_aqi.pm25_p95 AS outdoor_aqi_pm25_p95,
            outdoor_aqi.so2_ugm3_mean AS outdoor_aqi_so2_ugm3_mean,
            outdoor_aqi.so2_ugm3_max AS outdoor_aqi_so2_ugm3_max,
            outdoor_aqi.sample_count AS outdoor_aqi_sample_count,
            COALESCE(outdoor_aqi.sample_count, 0) AS outdoor_aqi_samples,

    -- Total samples
            COALESCE(indoor.sample_count, 0) + COALESCE(outdoor.sample_count, 0) + COALESCE(state.sample_count, 0) + COALESCE(outdoor_aqi.sample_count, 0) AS total_samples
FROM (SELECT bucket, AVG(co2_mean) AS co2_mean, AVG(co2_std) AS co2_std, MIN(co2_min) AS co2_min, MAX(co2_max) AS co2_max, AVG(humidity_pct_mean) AS humidity_pct_mean, MIN(humidity_pct_min) AS humidity_pct_min, MAX(humidity_pct_max) AS humidity_pct_max, AVG(nox_index_mean) AS nox_index_mean, MAX(nox_index_max) AS nox_index_max, AVG(pm10_mean) AS pm10_mean, MIN(pm10_min) AS pm10_min, MAX(pm10_max) AS pm10_max, AVG(pm25_mean) AS pm25_mean, AVG(pm25_std) AS pm25_std, MIN(pm25_min) AS pm25_min, MAX(pm25_max) AS pm25_max, MAX(pm25_p95) AS pm25_p95, AVG(temperature_c_mean) AS temperature_c_mean, MIN(temperature_c_min) AS temperature_c_min, MAX(temperature_c_max) AS temperature_c_max, AVG(tvoc_index_mean) AS tvoc_index_mean, MAX(tvoc_index_max) AS tvoc_index_max, SUM(sample_count) AS sample_count FROM gold.air_quality_hourly GROUP BY bucket) indoor
FULL OUTER JOIN (SELECT bucket, AVG(cloud_cover_pct_mean) AS cloud_cover_pct_mean, AVG(feels_like_c_mean) AS feels_like_c_mean, MIN(feels_like_c_min) AS feels_like_c_min, MAX(feels_like_c_max) AS feels_like_c_max, AVG(humidity_pct_mean) AS humidity_pct_mean, MIN(humidity_pct_min) AS humidity_pct_min, MAX(humidity_pct_max) AS humidity_pct_max, AVG(precipitation_mm_mean) AS precipitation_mm_mean, MAX(precipitation_mm_max) AS precipitation_mm_max, SUM(precipitation_mm_count) AS precipitation_mm_count, AVG(pressure_pa_mean) AS pressure_pa_mean, AVG(temperature_c_mean) AS temperature_c_mean, MIN(temperature_c_min) AS temperature_c_min, MAX(temperature_c_max) AS temperature_c_max, AVG(visibility_m_mean) AS visibility_m_mean, MIN(visibility_m_min) AS visibility_m_min, AVG(wind_speed_kmh_mean) AS wind_speed_kmh_mean, MAX(wind_speed_kmh_max) AS wind_speed_kmh_max, SUM(sample_count) AS sample_count FROM gold.outdoor_weather_hourly GROUP BY bucket) outdoor
    ON indoor.bucket = outdoor.bucket
FULL OUTER JOIN (SELECT bucket, SUM(state_count) AS state_count, MIN(state_first) AS state_first, MAX(state_last) AS state_last, SUM(sample_count) AS sample_count FROM gold.home_assistant_state_hourly GROUP BY bucket) state
    ON COALESCE(indoor.bucket, outdoor.bucket) = state.bucket
FULL OUTER JOIN (SELECT bucket, AVG(aqi_epa_mean) AS aqi_epa_mean, MIN(aqi_epa_min) AS aqi_epa_min, MAX(aqi_epa_max) AS aqi_epa_max, AVG(aqi_owm_mean) AS aqi_owm_mean, MIN(aqi_owm_min) AS aqi_owm_min, MAX(aqi_owm_max) AS aqi_owm_max, AVG(co_ugm3_mean) AS co_ugm3_mean, MAX(co_ugm3_max) AS co_ugm3_max, AVG(no2_ugm3_mean) AS no2_ugm3_mean, MAX(no2_ugm3_max) AS no2_ugm3_max, AVG(o3_ugm3_mean) AS o3_ugm3_mean, MAX(o3_ugm3_max) AS o3_ugm3_max, AVG(pm10_mean) AS pm10_mean, MIN(pm10_min) AS pm10_min, MAX(pm10_max) AS pm10_max, AVG(pm25_mean) AS pm25_mean, AVG(pm25_std) AS pm25_std, MIN(pm25_min) AS pm25_min, MAX(pm25_max) AS pm25_max, MAX(pm25_p95) AS pm25_p95, AVG(so2_ugm3_mean) AS so2_ugm3_mean, MAX(so2_ugm3_max) AS so2_ugm3_max, SUM(sample_count) AS sample_count FROM gold.outdoor_air_quality_hourly GROUP BY bucket) outdoor_aqi
    ON COALESCE(indoor.bucket, outdoor.bucket, state.bucket) = outdoor_aqi.bucket
WHERE COALESCE(indoor.bucket, outdoor.bucket, state.bucket, outdoor_aqi.bucket) >= NOW() - INTERVAL '90 days';

-- Index for efficient bucket queries
CREATE INDEX IF NOT EXISTS idx_indoor_air_quality_aligned_bucket
    ON gold.indoor_air_quality_aligned (bucket);

-- Scheduled refresh for aligned materialized view
-- Delete dependent jobs and DROP procedure for clean redeploy
DO $$
DECLARE
    _job_id INTEGER;
BEGIN
    FOR _job_id IN
        SELECT job_id FROM timescaledb_information.jobs
        WHERE proc_schema = 'gold' AND proc_name = 'refresh_indoor_air_quality_aligned'
    LOOP
        PERFORM delete_job(_job_id);
        RAISE NOTICE 'Deleted job % (gold.refresh_indoor_air_quality_aligned) before procedure replacement', _job_id;
    END LOOP;
END $$;

DROP PROCEDURE IF EXISTS gold.refresh_indoor_air_quality_aligned(integer, jsonb);

CREATE OR REPLACE PROCEDURE gold.refresh_indoor_air_quality_aligned(job_id INT, config JSONB)
LANGUAGE plpgsql AS $$
BEGIN
    REFRESH MATERIALIZED VIEW gold.indoor_air_quality_aligned;
    RAISE NOTICE 'Refreshed aligned view: gold.indoor_air_quality_aligned';
    COMMIT;
END;
$$;

-- Schedule refresh every 15 minutes (aligns with CA refresh intervals)
SELECT add_job(
    'gold.refresh_indoor_air_quality_aligned'::regproc,
    '15 minutes'::INTERVAL,
    config => '{}'::JSONB
);
