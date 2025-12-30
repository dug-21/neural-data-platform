-- Data Dictionary Schema for DP-002
-- Executed on container first start

-- Create schema
CREATE SCHEMA IF NOT EXISTS data_dictionary;

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

-- 4. Entity schemas table (for data dictionary)
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

-- Views
CREATE OR REPLACE VIEW data_dictionary.v_data_dictionary AS
SELECT
    s.stream_id,
    es.schema_name,
    es.description AS schema_description,
    es.device_class,
    esa.attribute_name,
    esa.attribute_type,
    esa.unit,
    esa.description AS attribute_description,
    esa.nullable,
    esa.range_min,
    esa.range_max
FROM data_dictionary.streams s
JOIN data_dictionary.entity_schemas es ON s.stream_id = es.stream_id
JOIN data_dictionary.entity_schema_attributes esa ON es.id = esa.schema_id
ORDER BY s.stream_id, es.schema_name, esa.sort_order;

CREATE OR REPLACE VIEW data_dictionary.stream_overview AS
SELECT
    s.stream_id,
    s.description,
    s.version,
    s.enabled,
    s.retention_days,
    COUNT(DISTINCT f.id) AS field_count,
    COUNT(DISTINCT src.id) AS source_count,
    COUNT(DISTINCT es.id) AS schema_count,
    s.created_at,
    s.updated_at
FROM data_dictionary.streams s
LEFT JOIN data_dictionary.fields f ON s.stream_id = f.stream_id
LEFT JOIN data_dictionary.sources src ON s.stream_id = src.stream_id
LEFT JOIN data_dictionary.entity_schemas es ON s.stream_id = es.stream_id
GROUP BY s.stream_id, s.description, s.version, s.enabled,
         s.retention_days, s.created_at, s.updated_at;

-- Success message
DO $$
BEGIN
    RAISE NOTICE 'Data Dictionary schema created successfully';
END $$;
