-- ============================================================================
-- Migration: 003_etl_runs.sql
-- Feature: dp-011 - Silver ETL Run Statistics Persistence
-- Description: Creates the silver.etl_runs table for tracking ETL execution
-- ============================================================================

-- Create the etl_runs table for tracking Silver ETL executions
-- This table stores run statistics, status, and error information for operational
-- observability and debugging.
CREATE TABLE IF NOT EXISTS silver.etl_runs (
    -- Primary identifier for each ETL run
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Stream being processed (e.g., 'air-quality', 'outdoor-weather')
    stream_id TEXT NOT NULL,

    -- Execution timestamps
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at TIMESTAMPTZ,
    duration_ms BIGINT,

    -- Run status: 'running', 'success', 'failed', 'partial'
    status TEXT NOT NULL DEFAULT 'running'
        CHECK (status IN ('running', 'success', 'failed', 'partial')),

    -- Row statistics
    rows_processed BIGINT NOT NULL DEFAULT 0,
    rows_flagged BIGINT NOT NULL DEFAULT 0,
    rows_rejected BIGINT NOT NULL DEFAULT 0,

    -- Watermark tracking for incremental loads
    watermark_before TIMESTAMPTZ,
    watermark_after TIMESTAMPTZ,

    -- Error information (populated on failure)
    error_message TEXT,
    error_context JSONB,

    -- Run mode: 'daemon' (scheduled), 'manual' (CLI), 'backfill' (historical)
    run_mode TEXT DEFAULT 'daemon'
        CHECK (run_mode IN ('daemon', 'manual', 'backfill')),

    -- Links all runs within the same daemon cycle
    daemon_cycle_id UUID,

    -- Audit timestamp
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for querying latest run per stream (most common query pattern)
CREATE INDEX IF NOT EXISTS idx_etl_runs_stream_started
    ON silver.etl_runs (stream_id, started_at DESC);

-- Index for daemon cycle grouping
CREATE INDEX IF NOT EXISTS idx_etl_runs_cycle
    ON silver.etl_runs (daemon_cycle_id)
    WHERE daemon_cycle_id IS NOT NULL;

-- Index for status queries (e.g., finding failed runs)
CREATE INDEX IF NOT EXISTS idx_etl_runs_status
    ON silver.etl_runs (status, started_at DESC);

-- Add comment for documentation
COMMENT ON TABLE silver.etl_runs IS
    'Tracks Silver ETL execution statistics for operational observability (dp-011)';

COMMENT ON COLUMN silver.etl_runs.daemon_cycle_id IS
    'UUID linking all stream runs within a single daemon tick cycle';

COMMENT ON COLUMN silver.etl_runs.watermark_before IS
    'Max timestamp in target table before ETL run (for incremental tracking)';

COMMENT ON COLUMN silver.etl_runs.watermark_after IS
    'Max timestamp in target table after ETL run (for data freshness tracking)';

COMMENT ON COLUMN silver.etl_runs.error_context IS
    'Structured JSON containing stage, SQL, file list, etc. for debugging failures';
