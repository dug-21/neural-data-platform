-- Expected DDL for air-quality trend feature views
-- Generated from gold_etl.features.trend config

CREATE SCHEMA IF NOT EXISTS gold;

-- Trend features for pm25 and co2 with 4-hour window
CREATE MATERIALIZED VIEW gold.air_quality_trends
WITH (timescaledb.continuous) AS
SELECT
    bucket,
    ndp_id,

    -- PM2.5 trend indicators
    pm25_mean AS pm25_current,
    LAG(pm25_mean, 4) OVER (PARTITION BY ndp_id ORDER BY bucket) AS pm25_4h_ago,
    pm25_mean - LAG(pm25_mean, 4) OVER (PARTITION BY ndp_id ORDER BY bucket) AS pm25_change_4h,

    CASE
        WHEN pm25_mean > LAG(pm25_mean, 4) OVER (PARTITION BY ndp_id ORDER BY bucket) * 1.1 THEN 'rising'
        WHEN pm25_mean < LAG(pm25_mean, 4) OVER (PARTITION BY ndp_id ORDER BY bucket) * 0.9 THEN 'falling'
        ELSE 'stable'
    END AS pm25_trend_direction,

    -- Rate of change (per hour)
    (pm25_mean - LAG(pm25_mean, 4) OVER (PARTITION BY ndp_id ORDER BY bucket)) / 4.0 AS pm25_rate_per_hour,

    -- CO2 trend indicators
    co2_mean AS co2_current,
    LAG(co2_mean, 4) OVER (PARTITION BY ndp_id ORDER BY bucket) AS co2_4h_ago,
    co2_mean - LAG(co2_mean, 4) OVER (PARTITION BY ndp_id ORDER BY bucket) AS co2_change_4h,

    CASE
        WHEN co2_mean > LAG(co2_mean, 4) OVER (PARTITION BY ndp_id ORDER BY bucket) * 1.1 THEN 'rising'
        WHEN co2_mean < LAG(co2_mean, 4) OVER (PARTITION BY ndp_id ORDER BY bucket) * 0.9 THEN 'falling'
        ELSE 'stable'
    END AS co2_trend_direction,

    -- Rate of change (per hour)
    (co2_mean - LAG(co2_mean, 4) OVER (PARTITION BY ndp_id ORDER BY bucket)) / 4.0 AS co2_rate_per_hour

FROM gold.air_quality_hourly;

COMMENT ON MATERIALIZED VIEW gold.air_quality_trends IS
'Trend features for PM2.5 and CO2 over 4-hour windows for predictive modeling';
