-- ops-008: Layer 0 Foundation — Core Data Dictionary Tables
-- Source: deploy/pi/init-scripts/01-create-data-dictionary.sql (tables only, views in 009)
-- Run order: 5th (depends on data_dictionary schema)
-- Idempotent: Yes (IF NOT EXISTS)

-- 1. Streams table
CREATE TABLE IF NOT EXISTS data_dictionary.streams (
    stream_id           TEXT PRIMARY KEY,
    description         TEXT,
    version             TEXT NOT NULL DEFAULT '1.0.0',
    enabled             BOOLEAN NOT NULL DEFAULT true,
    retention_days      INTEGER DEFAULT 90,
    partitioning_strategy TEXT DEFAULT 'daily',
    compression_after_days INTEGER DEFAULT 7,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    metadata            JSONB
);

-- 2. Fields table (Bronze layer schema)
CREATE TABLE IF NOT EXISTS data_dictionary.fields (
    id                  SERIAL PRIMARY KEY,
    stream_id           TEXT NOT NULL REFERENCES data_dictionary.streams(stream_id) ON DELETE CASCADE,
    field_name          TEXT NOT NULL,
    field_type          TEXT NOT NULL,
    nullable            BOOLEAN NOT NULL DEFAULT true,
    unit                TEXT,
    description         TEXT,
    validation_min      DOUBLE PRECISION,
    validation_max      DOUBLE PRECISION,
    validation_pattern  TEXT,
    sort_order          INTEGER NOT NULL DEFAULT 0,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(stream_id, field_name)
);

-- 3. Sources table
CREATE TABLE IF NOT EXISTS data_dictionary.sources (
    id                  SERIAL PRIMARY KEY,
    stream_id           TEXT NOT NULL REFERENCES data_dictionary.streams(stream_id) ON DELETE CASCADE,
    source_id           TEXT NOT NULL,
    source_type         TEXT NOT NULL,
    enabled             BOOLEAN NOT NULL DEFAULT true,
    config              JSONB NOT NULL,
    parser_type         TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(stream_id, source_id)
);

-- 4. Entity schemas table
CREATE TABLE IF NOT EXISTS data_dictionary.entity_schemas (
    id                  SERIAL PRIMARY KEY,
    stream_id           TEXT NOT NULL REFERENCES data_dictionary.streams(stream_id) ON DELETE CASCADE,
    schema_name         TEXT NOT NULL,
    description         TEXT,
    device_class        TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(stream_id, schema_name)
);

-- 5. Entity schema attributes table
CREATE TABLE IF NOT EXISTS data_dictionary.entity_schema_attributes (
    id                  SERIAL PRIMARY KEY,
    schema_id           INTEGER NOT NULL REFERENCES data_dictionary.entity_schemas(id) ON DELETE CASCADE,
    attribute_name      TEXT NOT NULL,
    attribute_type      TEXT NOT NULL,
    unit                TEXT,
    description         TEXT,
    nullable            BOOLEAN DEFAULT true,
    range_min           DOUBLE PRECISION,
    range_max           DOUBLE PRECISION,
    sort_order          INTEGER NOT NULL DEFAULT 0,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(schema_id, attribute_name)
);

-- 6. Sync status table
CREATE TABLE IF NOT EXISTS data_dictionary.sync_status (
    id                  SERIAL PRIMARY KEY,
    sync_type           TEXT NOT NULL,
    started_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at        TIMESTAMPTZ,
    status              TEXT NOT NULL DEFAULT 'running',
    streams_synced      INTEGER DEFAULT 0,
    schemas_synced      INTEGER DEFAULT 0,
    attributes_synced   INTEGER DEFAULT 0,
    error_message       TEXT,
    etcd_revision       BIGINT
);

-- Indexes
CREATE INDEX IF NOT EXISTS idx_fields_stream_id ON data_dictionary.fields(stream_id);
CREATE INDEX IF NOT EXISTS idx_sources_stream_id ON data_dictionary.sources(stream_id);
CREATE INDEX IF NOT EXISTS idx_entity_schemas_stream_id ON data_dictionary.entity_schemas(stream_id);
CREATE INDEX IF NOT EXISTS idx_entity_schema_attrs_schema_id ON data_dictionary.entity_schema_attributes(schema_id);

DO $$ BEGIN
  RAISE NOTICE 'NDP init [005]: Data dictionary tables created — streams, fields, sources, entity_schemas, entity_schema_attributes, sync_status';
END $$;
