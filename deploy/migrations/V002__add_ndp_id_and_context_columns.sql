-- AIR-009: Add ndp_id and context columns to Silver layer
-- Migration: V002__add_ndp_id_and_context_columns.sql
--
-- Purpose: Add stable source identifier (ndp_id) and mutable context (JSON blob)
-- to support the Simple Blob architecture pattern.
--
-- Related:
--   - product/features/air-009/architecture/ADR-003-silver-layer-schema.md
--   - product/features/air-009/SCOPE.md
--
-- Design Decision: JSONB chosen for context to enable:
--   - Schema flexibility (no migrations for new context keys)
--   - Point-in-time accuracy (full context snapshot per record)
--   - Efficient GIN indexing for containment queries

-- ============================================================================
-- Add ndp_id column (stable source identifier)
-- Used for: Consistent identification across config changes
-- ============================================================================

ALTER TABLE IF EXISTS air_quality_readings
    ADD COLUMN IF NOT EXISTS ndp_id TEXT;

ALTER TABLE IF EXISTS weather_readings
    ADD COLUMN IF NOT EXISTS ndp_id TEXT;

-- ============================================================================
-- Add context column (mutable attributes as JSONB)
-- Used for: Room, floor, calibration status, device metadata
--
-- Context is stored as flattened JSONB:
--   {
--     "location.coordinates": [29.958, -81.308],
--     "location.type": "indoor",
--     "location.path": "home/upstairs/office",
--     "device_type": "airgradient",
--     "model": "ONE-V9"
--   }
-- ============================================================================

ALTER TABLE IF EXISTS air_quality_readings
    ADD COLUMN IF NOT EXISTS context JSONB;

ALTER TABLE IF EXISTS weather_readings
    ADD COLUMN IF NOT EXISTS context JSONB;

-- ============================================================================
-- Create indexes for common query patterns
-- ============================================================================

-- B-tree index for ndp_id equality queries (primary access pattern)
-- Partial index (WHERE ... IS NOT NULL) for efficiency during migration period
CREATE INDEX IF NOT EXISTS idx_air_quality_ndp_id
    ON air_quality_readings(ndp_id)
    WHERE ndp_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_weather_ndp_id
    ON weather_readings(ndp_id)
    WHERE ndp_id IS NOT NULL;

-- GIN index for JSONB context queries (supports @>, ?, ?& operators)
-- Using jsonb_path_ops for better performance on containment queries
CREATE INDEX IF NOT EXISTS idx_air_quality_context
    ON air_quality_readings USING GIN (context jsonb_path_ops)
    WHERE context IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_weather_context
    ON weather_readings USING GIN (context jsonb_path_ops)
    WHERE context IS NOT NULL;

-- ============================================================================
-- Add comments for documentation
-- ============================================================================

COMMENT ON COLUMN air_quality_readings.ndp_id IS
    'Stable source identifier from stream config (AIR-009). Immutable after assignment.';

COMMENT ON COLUMN air_quality_readings.context IS
    'Mutable attributes as flattened JSONB blob (AIR-009). Snapshot at write time for point-in-time accuracy.';

COMMENT ON COLUMN weather_readings.ndp_id IS
    'Stable source identifier from stream config (AIR-009). Immutable after assignment.';

COMMENT ON COLUMN weather_readings.context IS
    'Mutable attributes as flattened JSONB blob (AIR-009). Snapshot at write time for point-in-time accuracy.';

-- ============================================================================
-- Query examples for reference (not executed)
-- ============================================================================

-- Query by ndp_id (uses B-tree index):
--   SELECT time, pm25, temperature
--   FROM air_quality_readings
--   WHERE ndp_id = 'airgradient-office-001'
--     AND time > NOW() - INTERVAL '24 hours'
--   ORDER BY time DESC;

-- Query by context field (uses GIN index):
--   SELECT ndp_id, AVG(pm25) as avg_pm25
--   FROM air_quality_readings
--   WHERE context @> '{"location.type": "indoor"}'::jsonb
--     AND time > NOW() - INTERVAL '7 days'
--   GROUP BY ndp_id;

-- Query with context extraction:
--   SELECT time, pm25, context->>'location.path' as room
--   FROM air_quality_readings
--   WHERE context @> '{"device_type": "airgradient"}'::jsonb
--   ORDER BY time DESC
--   LIMIT 100;

-- ============================================================================
-- Success message
-- ============================================================================

DO $$
BEGIN
    RAISE NOTICE 'AIR-009 migration complete: ndp_id and context columns added to Silver layer tables';
END $$;
