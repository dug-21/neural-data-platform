-- =============================================================================
-- Unified Pipeline Health Query
-- =============================================================================
-- Feature: air-012 - Home Assistant Integration
--
-- This query provides a unified view of ALL Silver layer streams with
-- stream-type-appropriate freshness thresholds.
--
-- Stream Types:
-- 1. REGULAR STREAMS (observation-based, frequent updates)
--    - air_quality_observations: ~30s intervals, thresholds 90s/180s
--    - weather_observations: ~10min intervals, thresholds 20min/40min
--    - outdoor_air_quality: ~10min intervals, thresholds 20min/40min
--    - weather_forecasts: ~1h intervals, thresholds 2h/4h
--
-- 2. SPARSE STREAMS (event-driven, infrequent updates)
--    - state_events: only on change, thresholds 18h/36h
--
-- Usage: Single table panel showing health of entire Silver layer
-- =============================================================================

-- -----------------------------------------------------------------------------
-- UNIFIED STREAM STATUS GRID
-- Combines all stream types with appropriate thresholds
-- -----------------------------------------------------------------------------
WITH stream_health AS (
    -- ==========================================================================
    -- REGULAR STREAMS (seconds-based thresholds)
    -- ==========================================================================

    -- Air Quality Observations (indoor sensors, ~30s stream interval)
    SELECT
        'air_quality_observations' AS stream_name,
        'Air Quality (Indoor)' AS display_name,
        'observation' AS stream_type,
        MAX(observation_time) AS last_record_time,
        COUNT(*) AS total_records_24h,
        -- Thresholds in seconds
        90 AS yellow_threshold_seconds,
        180 AS red_threshold_seconds,
        -- Sparse flag
        FALSE AS is_sparse
    FROM silver.air_quality_observations
    WHERE observation_time >= NOW() - INTERVAL '24 hours'

    UNION ALL

    -- Weather Observations (NWS stations, ~10min updates)
    SELECT
        'weather_observations' AS stream_name,
        'Weather Observations (NWS)' AS display_name,
        'observation' AS stream_type,
        MAX(observation_time) AS last_record_time,
        COUNT(*) AS total_records_24h,
        1200 AS yellow_threshold_seconds,  -- 20 minutes
        2400 AS red_threshold_seconds,     -- 40 minutes
        FALSE AS is_sparse
    FROM silver.weather_observations
    WHERE observation_time >= NOW() - INTERVAL '24 hours'

    UNION ALL

    -- Outdoor Air Quality (OWM API, ~10min updates)
    SELECT
        'outdoor_air_quality' AS stream_name,
        'Outdoor Air Quality (OWM)' AS display_name,
        'observation' AS stream_type,
        MAX(observation_time) AS last_record_time,
        COUNT(*) AS total_records_24h,
        1200 AS yellow_threshold_seconds,  -- 20 minutes
        2400 AS red_threshold_seconds,     -- 40 minutes
        FALSE AS is_sparse
    FROM silver.outdoor_air_quality
    WHERE observation_time >= NOW() - INTERVAL '24 hours'

    UNION ALL

    -- Weather Forecasts (NWS gridpoints, ~1h updates)
    SELECT
        'weather_forecasts' AS stream_name,
        'Weather Forecasts (NWS)' AS display_name,
        'forecast' AS stream_type,
        MAX(ingestion_time) AS last_record_time,
        COUNT(*) AS total_records_24h,
        7200 AS yellow_threshold_seconds,   -- 2 hours
        14400 AS red_threshold_seconds,     -- 4 hours
        FALSE AS is_sparse
    FROM silver.weather_forecasts
    WHERE ingestion_time >= NOW() - INTERVAL '24 hours'

    UNION ALL

    -- ==========================================================================
    -- SPARSE STREAMS (hours-based thresholds)
    -- ==========================================================================

    -- State Events (Home Assistant binary sensors, event-driven)
    -- Events only fire on STATE CHANGE, not intervals
    -- Windows may stay closed for days - this is NORMAL
    SELECT
        'state_events' AS stream_name,
        'State Events (HA)' AS display_name,
        'event' AS stream_type,
        MAX(event_time) AS last_record_time,
        COUNT(*) AS total_records_24h,
        64800 AS yellow_threshold_seconds,  -- 18 hours
        129600 AS red_threshold_seconds,    -- 36 hours
        TRUE AS is_sparse
    FROM silver.state_events
    -- Note: No 24h filter for sparse streams - we need last event regardless of age
)
SELECT
    display_name AS "Stream",
    stream_type AS "Type",
    CASE
        WHEN is_sparse THEN 'Sparse'
        ELSE 'Regular'
    END AS "Data Pattern",
    COALESCE(TO_CHAR(last_record_time, 'YYYY-MM-DD HH24:MI:SS'), 'No Data') AS "Last Record",
    CASE
        WHEN last_record_time IS NULL THEN 'No Data'
        WHEN is_sparse THEN
            -- Hours for sparse streams
            CONCAT(
                ROUND(EXTRACT(EPOCH FROM (NOW() - last_record_time)) / 3600, 1),
                ' hours ago'
            )
        ELSE
            -- Minutes/seconds for regular streams
            CONCAT(
                EXTRACT(EPOCH FROM (NOW() - last_record_time))::INTEGER / 60,
                'm ',
                EXTRACT(EPOCH FROM (NOW() - last_record_time))::INTEGER % 60,
                's ago'
            )
    END AS "Time Since Last",
    total_records_24h AS "Records (24h)",
    CASE
        WHEN last_record_time IS NULL THEN 'CRITICAL'
        WHEN EXTRACT(EPOCH FROM (NOW() - last_record_time)) > red_threshold_seconds THEN 'CRITICAL'
        WHEN EXTRACT(EPOCH FROM (NOW() - last_record_time)) > yellow_threshold_seconds THEN 'WARNING'
        ELSE 'HEALTHY'
    END AS "Status",
    CASE
        WHEN is_sparse THEN
            CONCAT(
                yellow_threshold_seconds / 3600, 'h/',
                red_threshold_seconds / 3600, 'h'
            )
        ELSE
            CONCAT(
                yellow_threshold_seconds / 60, 'm/',
                red_threshold_seconds / 60, 'm'
            )
    END AS "Thresholds (warn/crit)"
FROM stream_health
ORDER BY
    -- Sort by status severity (critical first), then by stream type
    CASE
        WHEN last_record_time IS NULL THEN 0
        WHEN EXTRACT(EPOCH FROM (NOW() - last_record_time)) > red_threshold_seconds THEN 1
        WHEN EXTRACT(EPOCH FROM (NOW() - last_record_time)) > yellow_threshold_seconds THEN 2
        ELSE 3
    END,
    is_sparse,
    stream_name;


-- -----------------------------------------------------------------------------
-- AGGREGATE HEALTH SUMMARY
-- Quick overview: total streams, healthy/warning/critical counts
-- Use: Stat panels for dashboard header
-- -----------------------------------------------------------------------------
WITH stream_health AS (
    -- Air Quality
    SELECT
        MAX(observation_time) AS last_time,
        90 AS yellow_threshold,
        180 AS red_threshold
    FROM silver.air_quality_observations
    WHERE observation_time >= NOW() - INTERVAL '24 hours'

    UNION ALL

    -- Weather Observations
    SELECT
        MAX(observation_time),
        1200,
        2400
    FROM silver.weather_observations
    WHERE observation_time >= NOW() - INTERVAL '24 hours'

    UNION ALL

    -- Outdoor Air Quality
    SELECT
        MAX(observation_time),
        1200,
        2400
    FROM silver.outdoor_air_quality
    WHERE observation_time >= NOW() - INTERVAL '24 hours'

    UNION ALL

    -- Weather Forecasts
    SELECT
        MAX(ingestion_time),
        7200,
        14400
    FROM silver.weather_forecasts
    WHERE ingestion_time >= NOW() - INTERVAL '24 hours'

    UNION ALL

    -- State Events (sparse)
    SELECT
        MAX(event_time),
        64800,  -- 18 hours
        129600  -- 36 hours
    FROM silver.state_events
),
status_counts AS (
    SELECT
        CASE
            WHEN last_time IS NULL THEN 'CRITICAL'
            WHEN EXTRACT(EPOCH FROM (NOW() - last_time)) > red_threshold THEN 'CRITICAL'
            WHEN EXTRACT(EPOCH FROM (NOW() - last_time)) > yellow_threshold THEN 'WARNING'
            ELSE 'HEALTHY'
        END AS status
    FROM stream_health
)
SELECT
    COUNT(*) AS total_streams,
    COUNT(*) FILTER (WHERE status = 'HEALTHY') AS healthy_count,
    COUNT(*) FILTER (WHERE status = 'WARNING') AS warning_count,
    COUNT(*) FILTER (WHERE status = 'CRITICAL') AS critical_count,
    CASE
        WHEN COUNT(*) FILTER (WHERE status = 'CRITICAL') > 0 THEN 'CRITICAL'
        WHEN COUNT(*) FILTER (WHERE status = 'WARNING') > 0 THEN 'WARNING'
        ELSE 'HEALTHY'
    END AS overall_status
FROM status_counts;
