-- ops-008: Layer 0 Foundation — Silver Layer Data Dictionary Metadata
-- Source: deploy/pi/init-scripts/003_silver_data_dictionary.sql (tables only, views in 009)
-- Run order: 6th (depends on data_dictionary.streams FK target)
-- Idempotent: Yes (IF NOT EXISTS)

-- Silver tables metadata
CREATE TABLE IF NOT EXISTS data_dictionary.silver_tables (
    table_name          TEXT PRIMARY KEY,
    schema_name         TEXT NOT NULL DEFAULT 'silver',
    description         TEXT,
    grain               TEXT,
    source_streams      TEXT[] NOT NULL DEFAULT '{}',
    hypertable_column   TEXT DEFAULT 'observation_time',
    chunk_interval      INTERVAL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

COMMENT ON TABLE data_dictionary.silver_tables IS
    'Metadata for Silver layer TimescaleDB tables';

-- Silver columns metadata
CREATE TABLE IF NOT EXISTS data_dictionary.silver_columns (
    id                  SERIAL PRIMARY KEY,
    table_name          TEXT NOT NULL
                        REFERENCES data_dictionary.silver_tables(table_name)
                        ON DELETE CASCADE,
    column_name         TEXT NOT NULL,
    data_type           TEXT NOT NULL,
    unit                TEXT,
    description         TEXT,
    nullable            BOOLEAN NOT NULL DEFAULT true,
    is_primary_key      BOOLEAN NOT NULL DEFAULT false,
    sort_order          INTEGER NOT NULL DEFAULT 0,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(table_name, column_name)
);

COMMENT ON TABLE data_dictionary.silver_columns IS
    'Column definitions for Silver layer tables including units and descriptions';

-- Silver lineage (Bronze-to-Silver field mappings)
CREATE TABLE IF NOT EXISTS data_dictionary.silver_lineage (
    id                  SERIAL PRIMARY KEY,
    silver_table        TEXT NOT NULL,
    silver_column       TEXT NOT NULL,
    source_stream       TEXT NOT NULL,
    source_path         TEXT NOT NULL,
    transformation      TEXT NOT NULL DEFAULT 'direct',
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(silver_table, silver_column, source_stream)
);

COMMENT ON TABLE data_dictionary.silver_lineage IS
    'Bronze-to-Silver field mappings for data lineage tracking';

-- Silver DQ rules
CREATE TABLE IF NOT EXISTS data_dictionary.silver_dq_rules (
    id                  SERIAL PRIMARY KEY,
    silver_table        TEXT NOT NULL,
    silver_column       TEXT,
    rule_name           TEXT NOT NULL,
    rule_params         JSONB NOT NULL DEFAULT '{}',
    action              TEXT NOT NULL DEFAULT 'flag',
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Unique index to handle NULL silver_column for cross-field rules
CREATE UNIQUE INDEX IF NOT EXISTS idx_silver_dq_rules_unique
    ON data_dictionary.silver_dq_rules(silver_table, COALESCE(silver_column, ''), rule_name);

COMMENT ON TABLE data_dictionary.silver_dq_rules IS
    'Data quality rules applied during Bronze-to-Silver ETL';

-- Indexes
CREATE INDEX IF NOT EXISTS idx_silver_columns_table
    ON data_dictionary.silver_columns(table_name);
CREATE INDEX IF NOT EXISTS idx_silver_columns_column_name
    ON data_dictionary.silver_columns(column_name);
CREATE INDEX IF NOT EXISTS idx_silver_lineage_table
    ON data_dictionary.silver_lineage(silver_table);
CREATE INDEX IF NOT EXISTS idx_silver_lineage_stream
    ON data_dictionary.silver_lineage(source_stream);
CREATE INDEX IF NOT EXISTS idx_silver_lineage_column
    ON data_dictionary.silver_lineage(silver_column);
CREATE INDEX IF NOT EXISTS idx_silver_dq_rules_table
    ON data_dictionary.silver_dq_rules(silver_table);
CREATE INDEX IF NOT EXISTS idx_silver_dq_rules_column
    ON data_dictionary.silver_dq_rules(silver_column);
CREATE INDEX IF NOT EXISTS idx_silver_dq_rules_name
    ON data_dictionary.silver_dq_rules(rule_name);
CREATE INDEX IF NOT EXISTS idx_silver_dq_rules_params
    ON data_dictionary.silver_dq_rules USING GIN (rule_params);

-- Add Silver-specific counters to sync_status if not present
DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'data_dictionary'
        AND table_name = 'sync_status'
        AND column_name = 'silver_tables_synced'
    ) THEN
        ALTER TABLE data_dictionary.sync_status
        ADD COLUMN silver_tables_synced INTEGER DEFAULT 0;
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'data_dictionary'
        AND table_name = 'sync_status'
        AND column_name = 'silver_columns_synced'
    ) THEN
        ALTER TABLE data_dictionary.sync_status
        ADD COLUMN silver_columns_synced INTEGER DEFAULT 0;
    END IF;
END $$;

DO $$ BEGIN
  RAISE NOTICE 'NDP init [006]: Silver dictionary tables created — silver_tables, silver_columns, silver_lineage, silver_dq_rules';
END $$;
