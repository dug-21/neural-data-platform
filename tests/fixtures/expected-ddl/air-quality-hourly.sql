-- Expected DDL for air-quality hourly continuous aggregate
-- Generated from gold_etl config with granularity "1 hour"

CREATE SCHEMA IF NOT EXISTS gold;

CREATE MATERIALIZED VIEW gold.air_quality_hourly
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 hour', observation_time) AS bucket,
    ndp_id,
    AVG(pm25) AS pm25_mean,
    STDDEV(pm25) AS pm25_std,
    MIN(pm25) AS pm25_min,
    MAX(pm25) AS pm25_max,
    PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY pm25) AS pm25_p95,
    AVG(co2) AS co2_mean,
    STDDEV(co2) AS co2_std,
    MIN(co2) AS co2_min,
    MAX(co2) AS co2_max,
    AVG(temperature_c) AS temperature_c_mean,
    MIN(temperature_c) AS temperature_c_min,
    MAX(temperature_c) AS temperature_c_max,
    AVG(humidity_pct) AS humidity_pct_mean,
    MIN(humidity_pct) AS humidity_pct_min,
    MAX(humidity_pct) AS humidity_pct_max,
    COUNT(*) AS sample_count
FROM silver.air_quality_observations
GROUP BY bucket, ndp_id;

SELECT add_continuous_aggregate_policy('gold.air_quality_hourly',
    start_offset => INTERVAL '4 hours',
    end_offset => INTERVAL '15 minutes',
    schedule_interval => INTERVAL '15 minutes'
);

-- Create index on bucket for efficient time-range queries
CREATE INDEX ON gold.air_quality_hourly (bucket DESC);

-- Create index on ndp_id for device-specific queries
CREATE INDEX ON gold.air_quality_hourly (ndp_id, bucket DESC);

COMMENT ON MATERIALIZED VIEW gold.air_quality_hourly IS
'Hourly aggregates for indoor air quality measurements from AirGradient sensors';
