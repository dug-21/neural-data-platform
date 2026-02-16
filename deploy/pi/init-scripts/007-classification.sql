-- ops-008: Layer 0 Foundation — Stream Classification & Gold Table Metadata
-- Source: deploy/pi/init-scripts/004_stream_classification.sql (tables only, views/functions in 009)
-- Run order: 7th (depends on data_dictionary.streams FK)
-- Idempotent: Yes (IF NOT EXISTS)

-- Stream classification table
CREATE TABLE IF NOT EXISTS data_dictionary.stream_classification (
    stream_id           TEXT PRIMARY KEY
                        REFERENCES data_dictionary.streams(stream_id)
                        ON DELETE CASCADE,
    stream_type         TEXT NOT NULL
                        CHECK (stream_type IN ('observation', 'state_event', 'forecast', 'dimension')),
    correlation_role    TEXT NOT NULL
                        CHECK (correlation_role IN ('effect', 'cause', 'context', 'metadata')),
    null_handling       TEXT NOT NULL
                        CHECK (null_handling IN ('preserve', 'carry_forward')),
    description         TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

COMMENT ON TABLE data_dictionary.stream_classification IS
    'Stream type classifications for Gold layer correlation analysis';

-- Gold tables metadata
CREATE TABLE IF NOT EXISTS data_dictionary.gold_tables (
    table_name          TEXT PRIMARY KEY,
    object_type         TEXT NOT NULL DEFAULT 'continuous_aggregate',
    source_silver_table TEXT,
    source_stream_type  TEXT
                        CHECK (source_stream_type IS NULL OR
                               source_stream_type IN ('observation', 'state_event', 'forecast', 'dimension')),
    granularity         TEXT,
    description         TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Add source_stream_type column if it doesn't exist (for existing installations)
DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'data_dictionary'
        AND table_name = 'gold_tables'
        AND column_name = 'source_stream_type'
    ) THEN
        ALTER TABLE data_dictionary.gold_tables
        ADD COLUMN source_stream_type TEXT
            CHECK (source_stream_type IS NULL OR
                   source_stream_type IN ('observation', 'state_event', 'forecast', 'dimension'));
    END IF;
END $$;

COMMENT ON TABLE data_dictionary.gold_tables IS
    'Metadata for Gold layer tables and views';

-- Indexes
CREATE INDEX IF NOT EXISTS idx_stream_classification_type
    ON data_dictionary.stream_classification(stream_type);
CREATE INDEX IF NOT EXISTS idx_stream_classification_role
    ON data_dictionary.stream_classification(correlation_role);
CREATE INDEX IF NOT EXISTS idx_gold_tables_stream_type
    ON data_dictionary.gold_tables(source_stream_type);

DO $$ BEGIN
  RAISE NOTICE 'NDP init [007]: Classification tables created — stream_classification, gold_tables';
END $$;
