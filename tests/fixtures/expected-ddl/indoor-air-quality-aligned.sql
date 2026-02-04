-- Expected DDL for indoor-air-quality aligned view
-- Generated from domain config with full_outer join strategy

CREATE SCHEMA IF NOT EXISTS gold;

CREATE MATERIALIZED VIEW gold.indoor_air_quality_aligned
WITH (timescaledb.continuous) AS
SELECT
    COALESCE(indoor.bucket, outdoor.bucket, state.bucket) AS bucket,

    -- Indoor air quality fields (primary)
    indoor.ndp_id AS indoor_ndp_id,
    indoor.pm25_mean AS indoor_pm25_mean,
    indoor.pm25_std AS indoor_pm25_std,
    indoor.pm25_min AS indoor_pm25_min,
    indoor.pm25_max AS indoor_pm25_max,
    indoor.pm25_p95 AS indoor_pm25_p95,
    indoor.co2_mean AS indoor_co2_mean,
    indoor.co2_std AS indoor_co2_std,
    indoor.co2_min AS indoor_co2_min,
    indoor.co2_max AS indoor_co2_max,
    indoor.temperature_c_mean AS indoor_temperature_c_mean,
    indoor.temperature_c_min AS indoor_temperature_c_min,
    indoor.temperature_c_max AS indoor_temperature_c_max,
    indoor.humidity_pct_mean AS indoor_humidity_pct_mean,
    indoor.humidity_pct_min AS indoor_humidity_pct_min,
    indoor.humidity_pct_max AS indoor_humidity_pct_max,

    -- Outdoor weather fields (context)
    outdoor.ndp_id AS outdoor_ndp_id,
    outdoor.temperature_c_mean AS outdoor_temperature_c_mean,
    outdoor.humidity_pct_mean AS outdoor_humidity_pct_mean,
    outdoor.pressure_pa_mean AS outdoor_pressure_pa_mean,
    outdoor.wind_speed_kmh_mean AS outdoor_wind_speed_kmh_mean,

    -- State event fields (actuator) - LOCF applied
    state.ndp_id AS state_ndp_id,
    state.state AS state_last_state,

    indoor.sample_count AS indoor_sample_count,
    outdoor.sample_count AS outdoor_sample_count

FROM gold.air_quality_hourly indoor

FULL OUTER JOIN gold.weather_observations_hourly outdoor
    ON indoor.bucket = outdoor.bucket

FULL OUTER JOIN gold.state_events_hourly state
    ON indoor.bucket = state.bucket;

-- Note: For state events with null_handling: carry_forward,
-- the view should apply LOCF using window functions:
-- COALESCE(state.state, LAG(state.state) OVER (ORDER BY bucket))

SELECT add_continuous_aggregate_policy('gold.indoor_air_quality_aligned',
    start_offset => INTERVAL '4 hours',
    end_offset => INTERVAL '15 minutes',
    schedule_interval => INTERVAL '15 minutes'
);

-- Create index on bucket for efficient time-range queries
CREATE INDEX ON gold.indoor_air_quality_aligned (bucket DESC);

COMMENT ON MATERIALIZED VIEW gold.indoor_air_quality_aligned IS
'Aligned view joining indoor air quality with outdoor weather and state events for correlation analysis';
