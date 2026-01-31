-- =============================================================================
-- Neural Data Platform - Silver Layer State Events Schema
-- =============================================================================
-- Feature: air-012 - Home Assistant Integration
-- Version: 1.0.0
-- Date: 2026-01-31
--
-- This script adds the state_events table for Home Assistant binary sensors
-- (window/door state tracking). Designed to integrate with existing Silver layer.
--
-- Prerequisites: 001_silver_schema.sql must be run first (creates silver schema)
-- Run order: 002 (second init script)
-- =============================================================================

-- =============================================================================
-- SECTION 1: silver.state_events
-- =============================================================================
-- Source: Bronze home-assistant-state stream (MQTT)
-- Grain: One row per state change event (sparse - only fires on change)
-- Use: Window/door state tracking, foundation for dp-014 SCD Gold layer

CREATE TABLE silver.state_events (
    -- Audit/Metadata
    ingestion_time      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    event_time          TIMESTAMPTZ NOT NULL,
    source_stream       TEXT NOT NULL DEFAULT 'home-assistant-state',

    -- Identity
    ndp_id              TEXT NOT NULL,
    source_entity_id    TEXT,

    -- State
    state               TEXT NOT NULL,

    -- DQ Transparency
    dq_flags            TEXT[],

    -- Primary Key
    PRIMARY KEY (event_time, ndp_id)
);

-- Convert to hypertable with 1-day chunks (Pi memory constraint)
SELECT create_hypertable('silver.state_events',
    'event_time',
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE
);

-- =============================================================================
-- SECTION 2: Indexes
-- =============================================================================

-- Primary query pattern: Latest state per entity
CREATE INDEX IF NOT EXISTS idx_state_events_ndp_id
    ON silver.state_events (ndp_id, event_time DESC);

-- Query pattern: Events with DQ issues
CREATE INDEX IF NOT EXISTS idx_state_events_dq_flags
    ON silver.state_events USING GIN (dq_flags)
    WHERE dq_flags IS NOT NULL AND array_length(dq_flags, 1) > 0;

-- Query pattern: Events by source entity (for HA debugging)
CREATE INDEX IF NOT EXISTS idx_state_events_source_entity
    ON silver.state_events (source_entity_id, event_time DESC);

-- =============================================================================
-- SECTION 3: Table and Column Comments
-- =============================================================================

COMMENT ON TABLE silver.state_events IS
    'State change events from Home Assistant binary sensors.
     Source: home-assistant-state Bronze stream (MQTT).
     Grain: One row per state change event (sparse - only fires on change).
     Use: Window/door state tracking, foundation for dp-014 SCD Gold layer.
     Note: SCD semantics (valid_from/valid_to) computed in Gold layer.';

COMMENT ON COLUMN silver.state_events.ingestion_time IS
    'When the row was inserted into TimescaleDB.';

COMMENT ON COLUMN silver.state_events.event_time IS
    'When NDP received the MQTT message. MQTT latency typically <100ms.';

COMMENT ON COLUMN silver.state_events.source_stream IS
    'Bronze stream identifier for ETL lineage.';

COMMENT ON COLUMN silver.state_events.ndp_id IS
    'NDP entity identifier: door_backslider, door_officewindow, etc.';

COMMENT ON COLUMN silver.state_events.source_entity_id IS
    'Full Home Assistant entity ID extracted from MQTT topic path.';

COMMENT ON COLUMN silver.state_events.state IS
    'Binary state: "on" = open, "off" = closed (Home Assistant convention).';

COMMENT ON COLUMN silver.state_events.dq_flags IS
    'Array of DQ rule violations detected during ETL. NULL = no issues.';

-- =============================================================================
-- SECTION 4: Compression Policy
-- =============================================================================
-- State events compress well due to repeated state values ('on'/'off')
-- Expected compression ratio: 10-20x for text columns

ALTER TABLE silver.state_events SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'ndp_id',
    timescaledb.compress_orderby = 'event_time DESC'
);

SELECT add_compression_policy('silver.state_events',
    INTERVAL '7 days',
    if_not_exists => TRUE
);

-- =============================================================================
-- SECTION 5: Retention Policy
-- =============================================================================
-- Keep raw Silver data for 90 days (can be rebuilt from Bronze)

SELECT add_retention_policy('silver.state_events',
    INTERVAL '90 days',
    if_not_exists => TRUE
);

-- =============================================================================
-- SECTION 6: Grant Permissions
-- =============================================================================
-- Use existing ndp_app role from 001_silver_schema.sql

GRANT SELECT, INSERT, UPDATE ON silver.state_events TO ndp_app;

-- =============================================================================
-- SECTION 7: Schema Version Tracking
-- =============================================================================

INSERT INTO silver.schema_version (version, description)
VALUES ('1.1.0', 'Add state_events table for air-012 Home Assistant integration')
ON CONFLICT (version) DO NOTHING;

-- =============================================================================
-- Summary
-- =============================================================================
-- Table created:
--   - silver.state_events (Home Assistant binary sensor state changes)
--
-- Indexes:
--   - idx_state_events_ndp_id (ndp_id, event_time DESC)
--   - idx_state_events_dq_flags (GIN on dq_flags, partial)
--   - idx_state_events_source_entity (source_entity_id, event_time DESC)
--
-- Policies:
--   - Compression: After 7 days, segmented by ndp_id
--   - Retention: 90 days
--
-- Note: No continuous aggregates for state_events (sparse, categorical data).
--       SCD semantics computed in dp-014 Gold layer.
-- =============================================================================
