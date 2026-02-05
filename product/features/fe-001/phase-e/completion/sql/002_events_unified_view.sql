-- ============================================================================
-- MIGRATION: 002_events_unified_view.sql
-- Feature: v11-013 (Events Unified View) - SPEC-E02
-- Author: NDP TimescaleDB Developer
-- Date: 2026-02-05
--
-- Creates gold.events_unified view for V1.2 API backward compatibility.
-- This view formats the details JSONB for the original V1.2 contract.
--
-- Idempotent: CREATE OR REPLACE VIEW
-- ============================================================================

-- ============================================================================
-- VIEW: gold.events_unified
-- Purpose: V1.2 API compatibility view over events hypertable
-- ============================================================================

-- The unified view provides a backward-compatible interface.
-- It builds the `details` JSONB dynamically based on event_type.
-- V1.2 Pattern Detection Engine consumes this view.

CREATE OR REPLACE VIEW gold.events_unified AS
SELECT
    event_id,
    event_time,
    stream_id,
    entity_id,
    event_type,

    -- Build details JSONB for backward compatibility with V1.2 contract
    -- State transitions: from_state, to_state, duration
    -- Threshold crossings: metric, threshold, direction, values
    CASE event_type
        WHEN 'state_transition' THEN
            jsonb_build_object(
                'from_state', from_state,
                'to_state', to_state,
                'duration_in_previous_ms', duration_in_state_ms
            ) || COALESCE(details, '{}'::JSONB)
        WHEN 'threshold_crossing' THEN
            jsonb_build_object(
                'metric', metric,
                'threshold', threshold_value,
                'direction', crossing_direction,
                'value', metric_value,
                'previous_value', previous_metric_value,
                'objective_id', objective_id
            ) || COALESCE(details, '{}'::JSONB)
        ELSE
            details
    END AS details,

    -- Context is passed through directly
    context

FROM gold.events
ORDER BY event_time DESC, event_type, event_id;

-- ============================================================================
-- COMMENTS: Documentation for data dictionary
-- ============================================================================

COMMENT ON VIEW gold.events_unified IS
    'V1.2 API compatibility view over gold.events hypertable.
     Provides backward-compatible schema with JSONB details formatting.
     Use this view for V1.2 Pattern Detection Engine queries.
     Note: For direct column access, query gold.events table directly.';

-- ============================================================================
-- DOMAIN-SCOPED VIEW: Indoor Air Quality Events (Optional)
-- Purpose: Pre-filtered view for indoor air quality domain
-- ============================================================================

-- This view filters events to indoor air quality domain streams.
-- Useful for domain-specific dashboards and pattern detection.

CREATE OR REPLACE VIEW gold.indoor_air_quality_events AS
SELECT
    event_id,
    event_time,
    stream_id,
    entity_id,
    event_type,
    from_state,
    to_state,
    duration_in_state_ms,
    metric,
    threshold_value,
    crossing_direction,
    metric_value,
    previous_metric_value,
    objective_id,
    context,
    details
FROM gold.events
WHERE stream_id IN (
    'air-quality',
    'home-assistant-state',
    'outdoor-weather',
    'outdoor-air-quality'
)
ORDER BY event_time DESC;

COMMENT ON VIEW gold.indoor_air_quality_events IS
    'Domain-scoped view: Indoor air quality events only.
     Filters gold.events to streams in the indoor-air-quality domain.
     Use for domain-specific dashboards and pattern detection.';

-- ============================================================================
-- SUCCESS MESSAGE
-- ============================================================================

DO $$
BEGIN
    RAISE NOTICE 'gold.events_unified view created successfully (v11-013 SPEC-E02)';
    RAISE NOTICE 'V1.2 API contract: event_id, event_time, stream_id, entity_id, event_type, details (JSONB), context (JSONB)';
    RAISE NOTICE 'Domain view: gold.indoor_air_quality_events';
END $$;
