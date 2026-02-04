-- Expected DDL for air-quality lag feature views
-- Generated from gold_etl.features.lag config

CREATE SCHEMA IF NOT EXISTS gold;

-- Lag features for pm25
CREATE MATERIALIZED VIEW gold.air_quality_pm25_lags
WITH (timescaledb.continuous) AS
SELECT
    bucket,
    ndp_id,
    pm25_mean AS pm25_current,
    LAG(pm25_mean, 1) OVER (PARTITION BY ndp_id ORDER BY bucket) AS pm25_lag_1h,
    LAG(pm25_mean, 6) OVER (PARTITION BY ndp_id ORDER BY bucket) AS pm25_lag_6h,
    LAG(pm25_mean, 24) OVER (PARTITION BY ndp_id ORDER BY bucket) AS pm25_lag_24h,
    pm25_mean - LAG(pm25_mean, 1) OVER (PARTITION BY ndp_id ORDER BY bucket) AS pm25_diff_1h,
    pm25_mean - LAG(pm25_mean, 6) OVER (PARTITION BY ndp_id ORDER BY bucket) AS pm25_diff_6h,
    pm25_mean - LAG(pm25_mean, 24) OVER (PARTITION BY ndp_id ORDER BY bucket) AS pm25_diff_24h
FROM gold.air_quality_hourly;

-- Lag features for co2
CREATE MATERIALIZED VIEW gold.air_quality_co2_lags
WITH (timescaledb.continuous) AS
SELECT
    bucket,
    ndp_id,
    co2_mean AS co2_current,
    LAG(co2_mean, 1) OVER (PARTITION BY ndp_id ORDER BY bucket) AS co2_lag_1h,
    LAG(co2_mean, 6) OVER (PARTITION BY ndp_id ORDER BY bucket) AS co2_lag_6h,
    LAG(co2_mean, 24) OVER (PARTITION BY ndp_id ORDER BY bucket) AS co2_lag_24h,
    co2_mean - LAG(co2_mean, 1) OVER (PARTITION BY ndp_id ORDER BY bucket) AS co2_diff_1h,
    co2_mean - LAG(co2_mean, 6) OVER (PARTITION BY ndp_id ORDER BY bucket) AS co2_diff_6h,
    co2_mean - LAG(co2_mean, 24) OVER (PARTITION BY ndp_id ORDER BY bucket) AS co2_diff_24h
FROM gold.air_quality_hourly;

COMMENT ON MATERIALIZED VIEW gold.air_quality_pm25_lags IS
'PM2.5 lag features for time-series forecasting (1h, 6h, 24h)';

COMMENT ON MATERIALIZED VIEW gold.air_quality_co2_lags IS
'CO2 lag features for time-series forecasting (1h, 6h, 24h)';
