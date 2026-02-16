-- ops-008: Layer 0 Foundation — Domain Configuration Tables
-- Source: deploy/pi/init-scripts/005_domain_objectives.sql (tables only, views/functions in 009)
-- Run order: 8th (depends on data_dictionary schema)
-- Idempotent: Yes (IF NOT EXISTS)

-- Domains table
CREATE TABLE IF NOT EXISTS data_dictionary.domains (
    domain_id           TEXT PRIMARY KEY,
    description         TEXT,
    stream_count        INTEGER,
    config_path         TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

COMMENT ON TABLE data_dictionary.domains IS
    'Domain configurations for cross-stream alignment and objectives';

-- Domain streams mapping
CREATE TABLE IF NOT EXISTS data_dictionary.domain_streams (
    domain_id           TEXT NOT NULL
                        REFERENCES data_dictionary.domains(domain_id)
                        ON DELETE CASCADE,
    stream_id           TEXT NOT NULL,
    alias               TEXT NOT NULL,
    role                TEXT NOT NULL
                        CHECK (role IN ('primary', 'context', 'actuator', 'constraint')),
    PRIMARY KEY (domain_id, stream_id)
);

COMMENT ON TABLE data_dictionary.domain_streams IS
    'Maps streams to domains with roles for alignment and pattern detection';

-- Objectives table
CREATE TABLE IF NOT EXISTS data_dictionary.objectives (
    objective_id        TEXT NOT NULL,
    domain_id           TEXT NOT NULL
                        REFERENCES data_dictionary.domains(domain_id)
                        ON DELETE CASCADE,
    description         TEXT,
    target_stream       TEXT NOT NULL,
    target_metric       TEXT NOT NULL,
    condition           TEXT NOT NULL
                        CHECK (condition IN ('<', '>', '<=', '>=', '==', '!=', 'between')),
    threshold           NUMERIC NOT NULL,
    threshold_upper     NUMERIC,
    unit                TEXT,
    priority            TEXT NOT NULL DEFAULT 'medium'
                        CHECK (priority IN ('low', 'medium', 'high', 'critical')),
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (domain_id, objective_id)
);

COMMENT ON TABLE data_dictionary.objectives IS
    'Target metrics to optimize toward — used for pattern detection and threshold crossing';

-- Constraints table
CREATE TABLE IF NOT EXISTS data_dictionary.constraints (
    constraint_id       TEXT NOT NULL,
    domain_id           TEXT NOT NULL
                        REFERENCES data_dictionary.domains(domain_id)
                        ON DELETE CASCADE,
    description         TEXT,
    constraint_stream   TEXT NOT NULL,
    constraint_metric   TEXT NOT NULL,
    condition           TEXT NOT NULL
                        CHECK (condition IN ('<', '>', '<=', '>=', '==', '!=')),
    threshold           NUMERIC NOT NULL,
    unit                TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (domain_id, constraint_id)
);

COMMENT ON TABLE data_dictionary.constraints IS
    'Conditions that must be met before taking actions (V1.3+ action framework)';

-- Indexes
CREATE INDEX IF NOT EXISTS idx_domain_streams_domain
    ON data_dictionary.domain_streams(domain_id);
CREATE INDEX IF NOT EXISTS idx_domain_streams_stream
    ON data_dictionary.domain_streams(stream_id);
CREATE INDEX IF NOT EXISTS idx_objectives_domain
    ON data_dictionary.objectives(domain_id);
CREATE INDEX IF NOT EXISTS idx_objectives_stream
    ON data_dictionary.objectives(target_stream);
CREATE INDEX IF NOT EXISTS idx_objectives_priority
    ON data_dictionary.objectives(priority);
CREATE INDEX IF NOT EXISTS idx_objectives_stream_metric
    ON data_dictionary.objectives(target_stream, target_metric);
CREATE INDEX IF NOT EXISTS idx_constraints_domain
    ON data_dictionary.constraints(domain_id);
CREATE INDEX IF NOT EXISTS idx_constraints_stream
    ON data_dictionary.constraints(constraint_stream);

-- Add domain-specific counters to sync_status if not present
DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'data_dictionary'
        AND table_name = 'sync_status'
        AND column_name = 'domains_synced'
    ) THEN
        ALTER TABLE data_dictionary.sync_status
        ADD COLUMN domains_synced INTEGER DEFAULT 0;
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = 'data_dictionary'
        AND table_name = 'sync_status'
        AND column_name = 'objectives_synced'
    ) THEN
        ALTER TABLE data_dictionary.sync_status
        ADD COLUMN objectives_synced INTEGER DEFAULT 0;
    END IF;
END $$;

DO $$ BEGIN
  RAISE NOTICE 'NDP init [008]: Domain tables created — domains, domain_streams, objectives, constraints';
END $$;
