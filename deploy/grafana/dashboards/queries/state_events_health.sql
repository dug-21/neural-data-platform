-- =============================================================================
-- Pipeline Health Query: State Events (Sparse Data)
-- =============================================================================
-- Feature: air-012 - Home Assistant Integration
-- Table: silver.state_events
--
-- IMPORTANT: State events are EVENT-DRIVEN (sparse data)
-- Events only fire on state CHANGE, not on intervals.
-- Windows may remain closed for days, so regular 5min/15min thresholds
-- would cause false alarms.
--
-- Sparse Data Thresholds:
--   FRESH:    < 18 hours since last event (green)
--   STALE:    18-36 hours since last event (yellow)
--   CRITICAL: > 36 hours since last event (red)
--
-- Usage: Include in Pipeline Health dashboard alongside regular stream queries
-- =============================================================================

-- -----------------------------------------------------------------------------
-- QUERY 1: State Events Health Summary (per entity)
-- Use: Table panel showing health of each binary sensor
-- -----------------------------------------------------------------------------
SELECT
    'state_events' AS stream,
    ndp_id,
    MAX(event_time) AS last_event,
    NOW() - MAX(event_time) AS age,
    EXTRACT(EPOCH FROM (NOW() - MAX(event_time))) / 3600 AS hours_since_event,
    CASE
        WHEN MAX(event_time) IS NULL THEN 'CRITICAL'
        WHEN NOW() - MAX(event_time) < INTERVAL '18 hours' THEN 'FRESH'
        WHEN NOW() - MAX(event_time) < INTERVAL '36 hours' THEN 'STALE'
        ELSE 'CRITICAL'
    END AS status
FROM silver.state_events
GROUP BY ndp_id
ORDER BY hours_since_event DESC;


-- -----------------------------------------------------------------------------
-- QUERY 2: State Events Overall Health (aggregate)
-- Use: Stat panel showing worst-case health across all sensors
-- Returns: Worst status among all tracked entities
-- -----------------------------------------------------------------------------
WITH entity_health AS (
    SELECT
        ndp_id,
        MAX(event_time) AS last_event,
        EXTRACT(EPOCH FROM (NOW() - MAX(event_time))) / 3600 AS hours_since,
        CASE
            WHEN MAX(event_time) IS NULL THEN 3
            WHEN NOW() - MAX(event_time) < INTERVAL '18 hours' THEN 1
            WHEN NOW() - MAX(event_time) < INTERVAL '36 hours' THEN 2
            ELSE 3
        END AS status_severity
    FROM silver.state_events
    GROUP BY ndp_id
)
SELECT
    CASE MAX(status_severity)
        WHEN 1 THEN 'FRESH'
        WHEN 2 THEN 'STALE'
        ELSE 'CRITICAL'
    END AS overall_status,
    COUNT(DISTINCT ndp_id) AS entity_count,
    COUNT(DISTINCT ndp_id) FILTER (WHERE status_severity = 3) AS critical_count,
    COUNT(DISTINCT ndp_id) FILTER (WHERE status_severity = 2) AS stale_count,
    COUNT(DISTINCT ndp_id) FILTER (WHERE status_severity = 1) AS fresh_count,
    ROUND(MAX(hours_since)::NUMERIC, 1) AS max_hours_since_event
FROM entity_health;


-- -----------------------------------------------------------------------------
-- QUERY 3: State Events Freshness Gauge (seconds)
-- Use: Gauge panel for state_events freshness
-- Thresholds: Green <64800s (18h), Yellow 64800-129600s, Red >129600s (36h)
-- -----------------------------------------------------------------------------
SELECT
    COALESCE(
        EXTRACT(EPOCH FROM (NOW() - MAX(event_time))),
        999999  -- Return high value if no data
    )::INTEGER AS "Seconds Since Last Event"
FROM silver.state_events;


-- -----------------------------------------------------------------------------
-- QUERY 4: State Events Record Count (24h)
-- Use: Bar chart or stat panel showing event volume
-- Note: Low volume is EXPECTED for sparse data (events only on state change)
-- -----------------------------------------------------------------------------
SELECT
    'State Events' AS stream,
    COUNT(*) AS record_count,
    COUNT(DISTINCT ndp_id) AS unique_entities
FROM silver.state_events
WHERE event_time >= NOW() - INTERVAL '24 hours';


-- -----------------------------------------------------------------------------
-- QUERY 5: Recent State Events (last 7 days)
-- Use: Table panel for debugging/verification
-- Shows most recent state changes with entity context
-- -----------------------------------------------------------------------------
SELECT
    se.event_time,
    se.ndp_id,
    COALESCE(ec.friendly_name, se.ndp_id) AS entity_name,
    se.state,
    se.source_entity_id,
    CASE se.state
        WHEN 'on' THEN 'OPEN'
        WHEN 'off' THEN 'CLOSED'
        ELSE se.state
    END AS state_display,
    ec.category,
    ec.orientation
FROM silver.state_events se
LEFT JOIN silver.entity_context ec ON se.ndp_id = ec.ndp_id
WHERE se.event_time >= NOW() - INTERVAL '7 days'
ORDER BY se.event_time DESC
LIMIT 100;
