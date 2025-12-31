-- Rollback for AIR-009: ndp_id and context columns
-- Migration: V002__add_ndp_id_and_context_columns_rollback.sql
--
-- WARNING: This will permanently remove ndp_id and context data from all records.
-- Ensure you have a backup before running this rollback.
--
-- Related: V002__add_ndp_id_and_context_columns.sql

-- ============================================================================
-- Drop indexes first (must exist before dropping columns)
-- ============================================================================

DROP INDEX IF EXISTS idx_air_quality_ndp_id;
DROP INDEX IF EXISTS idx_weather_ndp_id;
DROP INDEX IF EXISTS idx_air_quality_context;
DROP INDEX IF EXISTS idx_weather_context;

-- ============================================================================
-- Drop columns from air_quality_readings
-- ============================================================================

ALTER TABLE IF EXISTS air_quality_readings
    DROP COLUMN IF EXISTS ndp_id;

ALTER TABLE IF EXISTS air_quality_readings
    DROP COLUMN IF EXISTS context;

-- ============================================================================
-- Drop columns from weather_readings
-- ============================================================================

ALTER TABLE IF EXISTS weather_readings
    DROP COLUMN IF EXISTS ndp_id;

ALTER TABLE IF EXISTS weather_readings
    DROP COLUMN IF EXISTS context;

-- ============================================================================
-- Success message
-- ============================================================================

DO $$
BEGIN
    RAISE NOTICE 'AIR-009 rollback complete: ndp_id and context columns removed from Silver layer tables';
END $$;
