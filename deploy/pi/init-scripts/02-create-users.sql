-- Create application users for DP-002

-- Read-only user for Grafana
DO $$
BEGIN
    IF NOT EXISTS (SELECT FROM pg_catalog.pg_roles WHERE rolname = 'grafana_reader') THEN
        CREATE USER grafana_reader WITH PASSWORD 'grafana_read_only';
    END IF;
END $$;

-- Grant access to data_dictionary schema
GRANT USAGE ON SCHEMA data_dictionary TO grafana_reader;
GRANT SELECT ON ALL TABLES IN SCHEMA data_dictionary TO grafana_reader;
ALTER DEFAULT PRIVILEGES IN SCHEMA data_dictionary GRANT SELECT ON TABLES TO grafana_reader;

-- Grant access to silver schema (for Pipeline Health dashboard)
GRANT USAGE ON SCHEMA silver TO grafana_reader;
GRANT SELECT ON ALL TABLES IN SCHEMA silver TO grafana_reader;
ALTER DEFAULT PRIVILEGES IN SCHEMA silver GRANT SELECT ON TABLES TO grafana_reader;

DO $$
BEGIN
    RAISE NOTICE 'Application users created successfully';
END $$;
