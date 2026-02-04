-- Expected DDL for air-quality rolling feature views
-- Generated from gold_etl.features.rolling config

CREATE SCHEMA IF NOT EXISTS gold;

-- Rolling statistics for pm25 with 4-hour and 24-hour windows
CREATE MATERIALIZED VIEW gold.air_quality_pm25_rolling
WITH (timescaledb.continuous) AS
SELECT
    bucket,
    ndp_id,
    pm25_mean AS pm25_current,

    -- 4-hour rolling window (4 buckets at hourly granularity)
    AVG(pm25_mean) OVER (
        PARTITION BY ndp_id
        ORDER BY bucket
        ROWS BETWEEN 3 PRECEDING AND CURRENT ROW
    ) AS pm25_rolling_4h_mean,

    STDDEV(pm25_mean) OVER (
        PARTITION BY ndp_id
        ORDER BY bucket
        ROWS BETWEEN 3 PRECEDING AND CURRENT ROW
    ) AS pm25_rolling_4h_std,

    -- 24-hour rolling window (24 buckets at hourly granularity)
    AVG(pm25_mean) OVER (
        PARTITION BY ndp_id
        ORDER BY bucket
        ROWS BETWEEN 23 PRECEDING AND CURRENT ROW
    ) AS pm25_rolling_24h_mean,

    STDDEV(pm25_mean) OVER (
        PARTITION BY ndp_id
        ORDER BY bucket
        ROWS BETWEEN 23 PRECEDING AND CURRENT ROW
    ) AS pm25_rolling_24h_std,

    -- Deviation from rolling mean (useful for anomaly detection)
    pm25_mean - AVG(pm25_mean) OVER (
        PARTITION BY ndp_id
        ORDER BY bucket
        ROWS BETWEEN 23 PRECEDING AND CURRENT ROW
    ) AS pm25_deviation_from_24h_mean

FROM gold.air_quality_hourly;

COMMENT ON MATERIALIZED VIEW gold.air_quality_pm25_rolling IS
'PM2.5 rolling statistics (4h and 24h windows) for trend analysis and anomaly detection';
