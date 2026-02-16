-- ops-008: Layer 0 Foundation — Application Roles & Grants
-- Run order: 4th (depends on schemas)
-- Idempotent: Yes (IF NOT EXISTS, idempotent GRANT)

-- ndp_app role (used by Silver ETL, intelligence, etc.)
DO $$ BEGIN
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'ndp_app') THEN
    CREATE ROLE ndp_app WITH LOGIN PASSWORD 'ndp_app_default';
  END IF;
END $$;

-- grafana_reader role (read-only for dashboards)
DO $$ BEGIN
  IF NOT EXISTS (SELECT FROM pg_roles WHERE rolname = 'grafana_reader') THEN
    CREATE USER grafana_reader WITH PASSWORD 'grafana_read_only';
  END IF;
END $$;

-- Schema grants for ndp_app
GRANT USAGE ON SCHEMA silver TO ndp_app;
GRANT USAGE ON SCHEMA gold TO ndp_app;
GRANT USAGE ON SCHEMA analytics TO ndp_app;
GRANT USAGE ON SCHEMA data_dictionary TO ndp_app;

-- Default privileges for ndp_app (future tables auto-granted)
ALTER DEFAULT PRIVILEGES IN SCHEMA silver GRANT SELECT, INSERT, UPDATE ON TABLES TO ndp_app;
ALTER DEFAULT PRIVILEGES IN SCHEMA gold GRANT SELECT ON TABLES TO ndp_app;
ALTER DEFAULT PRIVILEGES IN SCHEMA analytics GRANT SELECT ON TABLES TO ndp_app;
GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA silver TO ndp_app;

-- Schema grants for grafana_reader
GRANT USAGE ON SCHEMA data_dictionary TO grafana_reader;
GRANT USAGE ON SCHEMA silver TO grafana_reader;
GRANT USAGE ON SCHEMA gold TO grafana_reader;
GRANT USAGE ON SCHEMA analytics TO grafana_reader;

-- Default privileges for grafana_reader (future tables auto-granted)
ALTER DEFAULT PRIVILEGES IN SCHEMA data_dictionary GRANT SELECT ON TABLES TO grafana_reader;
ALTER DEFAULT PRIVILEGES IN SCHEMA silver GRANT SELECT ON TABLES TO grafana_reader;
ALTER DEFAULT PRIVILEGES IN SCHEMA gold GRANT SELECT ON TABLES TO grafana_reader;
ALTER DEFAULT PRIVILEGES IN SCHEMA analytics GRANT SELECT ON TABLES TO grafana_reader;

DO $$ BEGIN
  RAISE NOTICE 'NDP init [004]: Roles created — ndp_app, grafana_reader (with default privileges)';
END $$;
