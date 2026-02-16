# ops-008 Pseudocode

## Init-Script Pseudocode

### 001-extensions.sql
```sql
-- Header comment: ops-008, Layer 0 foundation
-- Run order: 1st (no dependencies)

CREATE EXTENSION IF NOT EXISTS timescaledb CASCADE;
CREATE EXTENSION IF NOT EXISTS vector;

-- Verification
DO $$ BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'timescaledb') THEN
    RAISE EXCEPTION 'timescaledb extension failed to install';
  END IF;
  IF NOT EXISTS (SELECT 1 FROM pg_extension WHERE extname = 'vector') THEN
    RAISE EXCEPTION 'vector extension failed to install';
  END IF;
  RAISE NOTICE 'Extensions installed: timescaledb, vector';
END $$;
```

### 002-schemas.sql
```sql
-- Header comment: ops-008, creates all schemas
-- Run order: 2nd (depends on extensions)

CREATE SCHEMA IF NOT EXISTS data_dictionary;
CREATE SCHEMA IF NOT EXISTS silver;
CREATE SCHEMA IF NOT EXISTS gold;
CREATE SCHEMA IF NOT EXISTS analytics;

-- Verification
DO $$ BEGIN
  RAISE NOTICE 'Schemas created: data_dictionary, silver, gold, analytics';
END $$;
```

### 003-silver-functions.sql
```sql
-- Header comment: ops-008, Silver utility functions
-- Source: deploy/timescaledb/init/001_silver_schema.sql Section 1
-- Run order: 3rd (depends on silver schema)

CREATE OR REPLACE FUNCTION silver.linear_interpolate(
    value DOUBLE PRECISION, bp_low DOUBLE PRECISION, bp_high DOUBLE PRECISION,
    aqi_low INTEGER, aqi_high INTEGER
) RETURNS SMALLINT AS $$ ... $$ LANGUAGE plpgsql IMMUTABLE PARALLEL SAFE;
-- (exact implementation from 001_silver_schema.sql)

CREATE OR REPLACE FUNCTION silver.calculate_aqi_pm25(pm25_value DOUBLE PRECISION)
RETURNS SMALLINT AS $$ ... $$ LANGUAGE plpgsql IMMUTABLE PARALLEL SAFE;
-- (exact EPA breakpoints from 001_silver_schema.sql)

CREATE OR REPLACE FUNCTION silver.calculate_mold_risk(
    temp_c DOUBLE PRECISION, humidity_pct DOUBLE PRECISION
) RETURNS TEXT AS $$ ... $$ LANGUAGE plpgsql IMMUTABLE PARALLEL SAFE;
-- (exact thresholds from 001_silver_schema.sql)
```

### 004-roles.sql
```sql
-- Header comment: ops-008, application roles and grants
-- Run order: 4th (depends on schemas)

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

-- Default privileges for grafana_reader
ALTER DEFAULT PRIVILEGES IN SCHEMA data_dictionary GRANT SELECT ON TABLES TO grafana_reader;
ALTER DEFAULT PRIVILEGES IN SCHEMA silver GRANT SELECT ON TABLES TO grafana_reader;
ALTER DEFAULT PRIVILEGES IN SCHEMA gold GRANT SELECT ON TABLES TO grafana_reader;
ALTER DEFAULT PRIVILEGES IN SCHEMA analytics GRANT SELECT ON TABLES TO grafana_reader;
```

### 005-data-dictionary.sql
```sql
-- Header comment: ops-008, core data dictionary tables
-- Source: deploy/pi/init-scripts/01-create-data-dictionary.sql (tables only)
-- Run order: 5th (depends on data_dictionary schema)

-- Tables only (views moved to 009-dictionary-views.sql):
CREATE TABLE IF NOT EXISTS data_dictionary.streams ( ... );
CREATE TABLE IF NOT EXISTS data_dictionary.fields ( ... );
CREATE TABLE IF NOT EXISTS data_dictionary.sources ( ... );
CREATE TABLE IF NOT EXISTS data_dictionary.entity_schemas ( ... );
CREATE TABLE IF NOT EXISTS data_dictionary.entity_schema_attributes ( ... );
CREATE TABLE IF NOT EXISTS data_dictionary.sync_status ( ... );

-- Indexes
CREATE INDEX IF NOT EXISTS idx_fields_stream_id ON data_dictionary.fields(stream_id);
CREATE INDEX IF NOT EXISTS idx_sources_stream_id ON data_dictionary.sources(stream_id);
CREATE INDEX IF NOT EXISTS idx_entity_schemas_stream_id ON data_dictionary.entity_schemas(stream_id);
CREATE INDEX IF NOT EXISTS idx_entity_schema_attrs_schema_id ON data_dictionary.entity_schema_attributes(schema_id);
```

### 006-silver-dictionary.sql
```sql
-- Header comment: ops-008, Silver layer data dictionary metadata
-- Source: deploy/pi/init-scripts/003_silver_data_dictionary.sql (tables only)
-- Run order: 6th (depends on data_dictionary.streams FK target)

-- Tables only (views moved to 009):
CREATE TABLE IF NOT EXISTS data_dictionary.silver_tables ( ... );
CREATE TABLE IF NOT EXISTS data_dictionary.silver_columns ( ... );  -- FK to silver_tables
CREATE TABLE IF NOT EXISTS data_dictionary.silver_lineage ( ... );
CREATE TABLE IF NOT EXISTS data_dictionary.silver_dq_rules ( ... );

-- Indexes (all from 003_silver_data_dictionary.sql)
CREATE INDEX IF NOT EXISTS idx_silver_columns_table ON data_dictionary.silver_columns(table_name);
-- ... (all indexes)

-- sync_status column additions (idempotent DO block)
DO $$ BEGIN
  IF NOT EXISTS (SELECT 1 FROM information_schema.columns
    WHERE table_schema='data_dictionary' AND table_name='sync_status'
    AND column_name='silver_tables_synced') THEN
    ALTER TABLE data_dictionary.sync_status ADD COLUMN silver_tables_synced INTEGER DEFAULT 0;
  END IF;
  -- silver_columns_synced ...
END $$;
```

### 007-classification.sql
```sql
-- Header comment: ops-008, stream classification and gold table metadata
-- Source: deploy/pi/init-scripts/004_stream_classification.sql (tables only)
-- Run order: 7th (depends on data_dictionary.streams FK)

-- Tables only (views/functions moved to 009):
CREATE TABLE IF NOT EXISTS data_dictionary.stream_classification ( ... );  -- FK to streams
CREATE TABLE IF NOT EXISTS data_dictionary.gold_tables ( ... );

-- Indexes
CREATE INDEX IF NOT EXISTS idx_stream_classification_type ON data_dictionary.stream_classification(stream_type);
CREATE INDEX IF NOT EXISTS idx_stream_classification_role ON data_dictionary.stream_classification(correlation_role);
CREATE INDEX IF NOT EXISTS idx_gold_tables_stream_type ON data_dictionary.gold_tables(source_stream_type);

-- source_stream_type column addition for existing gold_tables (idempotent)
DO $$ BEGIN ... END $$;
```

### 008-domain-objectives.sql
```sql
-- Header comment: ops-008, domain configuration tables
-- Source: deploy/pi/init-scripts/005_domain_objectives.sql (tables only)
-- Run order: 8th (depends on data_dictionary.domains self-contained)

-- Tables only (views/functions moved to 009):
CREATE TABLE IF NOT EXISTS data_dictionary.domains ( ... );
CREATE TABLE IF NOT EXISTS data_dictionary.domain_streams ( ... );  -- FK to domains
CREATE TABLE IF NOT EXISTS data_dictionary.objectives ( ... );       -- FK to domains
CREATE TABLE IF NOT EXISTS data_dictionary.constraints ( ... );      -- FK to domains

-- Indexes (all from 005_domain_objectives.sql)
-- ... all indexes ...

-- sync_status column additions (idempotent)
DO $$ BEGIN
  -- domains_synced, objectives_synced
END $$;
```

### 009-dictionary-views.sql
```sql
-- Header comment: ops-008, all data dictionary views and functions consolidated
-- Run order: 9th (LAST — depends on all tables from 005-008)

-- ===== Views from 01-create-data-dictionary.sql =====
CREATE OR REPLACE VIEW data_dictionary.v_data_dictionary AS ...;
CREATE OR REPLACE VIEW data_dictionary.stream_overview AS ...;

-- ===== Views from 003_silver_data_dictionary.sql =====
CREATE OR REPLACE VIEW data_dictionary.v_complete_dictionary AS ...;
CREATE OR REPLACE VIEW data_dictionary.v_silver_table_overview AS ...;
CREATE OR REPLACE VIEW data_dictionary.v_lineage AS ...;
CREATE OR REPLACE VIEW data_dictionary.v_dq_rules_summary AS ...;
CREATE OR REPLACE VIEW data_dictionary.v_column_search AS ...;

-- ===== Functions from 003_silver_data_dictionary.sql =====
CREATE OR REPLACE FUNCTION data_dictionary.get_column_lineage(...) ...;
CREATE OR REPLACE FUNCTION data_dictionary.get_column_dq_rules(...) ...;

-- ===== Views from 004_stream_classification.sql =====
CREATE OR REPLACE VIEW data_dictionary.v_stream_classification_summary AS ...;
CREATE OR REPLACE VIEW data_dictionary.v_correlation_candidates AS ...;

-- ===== Functions from 004_stream_classification.sql =====
CREATE OR REPLACE FUNCTION data_dictionary.derive_correlation_role(...) ...;
CREATE OR REPLACE FUNCTION data_dictionary.derive_null_handling(...) ...;
CREATE OR REPLACE FUNCTION data_dictionary.sync_stream_classification(...) ...;

-- ===== Views from 005_domain_objectives.sql =====
CREATE OR REPLACE VIEW data_dictionary.v_domain_overview AS ...;
CREATE OR REPLACE VIEW data_dictionary.v_objectives_with_context AS ...;
CREATE OR REPLACE VIEW data_dictionary.v_high_priority_objectives AS ...;

-- ===== Functions from 005_domain_objectives.sql =====
CREATE OR REPLACE FUNCTION data_dictionary.get_objectives_for_stream(...) ...;
CREATE OR REPLACE FUNCTION data_dictionary.check_objective_violation(...) ...;

-- Verification
DO $$ BEGIN
  RAISE NOTICE 'Data dictionary views and functions created';
END $$;
```

## deploy.sh Changes Pseudocode

### Analytics Views Migration (new file: deploy/pi/migrations/001-analytics-views.sql)
```sql
-- Run after Phase 4 Silver table creation
-- Idempotent: CREATE OR REPLACE VIEW

CREATE SCHEMA IF NOT EXISTS analytics;

-- Only create views if Silver tables exist
DO $$ BEGIN
  IF EXISTS (SELECT 1 FROM information_schema.tables
    WHERE table_schema='silver' AND table_name='weather_forecasts') THEN

    -- forecast_accuracy view
    CREATE OR REPLACE VIEW analytics.forecast_accuracy AS
    SELECT ... FROM silver.weather_forecasts f
    INNER JOIN silver.weather_observations o ON ...;

    -- indoor_outdoor_comparison view
    CREATE OR REPLACE VIEW analytics.indoor_outdoor_comparison AS
    WITH indoor AS (...), outdoor_aq AS (...), weather AS (...)
    SELECT ...;

    -- latest_readings view
    CREATE OR REPLACE VIEW analytics.latest_readings AS
    WITH latest_indoor AS (...), latest_outdoor_aq AS (...), latest_weather AS (...)
    SELECT ...;

    RAISE NOTICE 'Analytics views created';
  ELSE
    RAISE NOTICE 'Silver tables not yet created, skipping analytics views';
  END IF;
END $$;

-- Grants
GRANT SELECT ON ALL TABLES IN SCHEMA analytics TO grafana_reader;
GRANT SELECT ON ALL TABLES IN SCHEMA analytics TO ndp_app;
```

### DQ Events Migration (new file: deploy/pi/migrations/002-dq-events.sql)
```sql
-- Run after Phase 4 Silver table creation
-- Creates silver.dq_events hypertable with retention policy

CREATE TABLE IF NOT EXISTS silver.dq_events (
    event_time          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    source_stream       TEXT NOT NULL,
    observation_time    TIMESTAMPTZ NOT NULL,
    ndp_id              TEXT NOT NULL,
    column_name         TEXT NOT NULL,
    rule_name           TEXT NOT NULL,
    original_value      TEXT,
    action_taken        TEXT NOT NULL,
    result_value        TEXT,
    PRIMARY KEY (event_time, source_stream, ndp_id, column_name)
);

SELECT create_hypertable('silver.dq_events', 'event_time',
    chunk_time_interval => INTERVAL '7 days', if_not_exists => TRUE);

SELECT add_retention_policy('silver.dq_events',
    INTERVAL '30 days', if_not_exists => TRUE);

CREATE INDEX IF NOT EXISTS idx_dq_events_stream
    ON silver.dq_events (source_stream, event_time DESC);
CREATE INDEX IF NOT EXISTS idx_dq_events_rule
    ON silver.dq_events (rule_name, event_time DESC);

GRANT SELECT, INSERT ON silver.dq_events TO ndp_app;
GRANT SELECT ON silver.dq_events TO grafana_reader;
```

### deploy.sh Phase 3 Update
```bash
# In handle_migrations() or equivalent:
# Run all migration files in deploy/pi/migrations/ in sorted order
for migration in "$SCRIPT_DIR/migrations/"*.sql; do
  if [ -f "$migration" ]; then
    log "Running migration: $(basename "$migration")"
    psql -h "$DB_HOST" -U postgres -d ndp -f "$migration"
  fi
done
```

## Integration Test Sequence

```bash
# 1. Clean slate
docker compose -f docker-compose.integration.yml down -v

# 2. Start services (init-scripts run automatically)
docker compose -f docker-compose.integration.yml up -d

# 3. Wait for TimescaleDB to be ready
until docker compose -f docker-compose.integration.yml exec -T timescaledb pg_isready; do
  sleep 1
done

# 4. Check init-script logs for errors
docker compose -f docker-compose.integration.yml logs timescaledb 2>&1 | grep -i "ERROR"
# Expected: no output (zero errors)

# 5. Verify foundation objects
docker compose -f docker-compose.integration.yml exec -T timescaledb \
  psql -U postgres -d ndp -c "
    SELECT nspname FROM pg_namespace
    WHERE nspname IN ('data_dictionary','silver','gold','analytics')
    ORDER BY nspname;"
# Expected: all 4 schemas

# 6. Verify roles
docker compose -f docker-compose.integration.yml exec -T timescaledb \
  psql -U postgres -d ndp -c "
    SELECT rolname FROM pg_roles WHERE rolname IN ('ndp_app','grafana_reader');"
# Expected: both roles

# 7. Verify data_dictionary tables (count)
docker compose -f docker-compose.integration.yml exec -T timescaledb \
  psql -U postgres -d ndp -c "
    SELECT COUNT(*) FROM information_schema.tables
    WHERE table_schema = 'data_dictionary';"
# Expected: 16 tables

# 8. Verify Silver functions
docker compose -f docker-compose.integration.yml exec -T timescaledb \
  psql -U postgres -d ndp -c "
    SELECT proname FROM pg_proc
    WHERE pronamespace = (SELECT oid FROM pg_namespace WHERE nspname='silver')
    ORDER BY proname;"
# Expected: calculate_aqi_pm25, calculate_mold_risk, linear_interpolate

# 9. Verify NO Silver hypertables exist yet
docker compose -f docker-compose.integration.yml exec -T timescaledb \
  psql -U postgres -d ndp -c "
    SELECT count(*) FROM timescaledb_information.hypertables
    WHERE hypertable_schema = 'silver';"
# Expected: 0

# 10. Run deploy.sh apply
DEPLOY_ENV=integration ./deploy/pi/deploy.sh apply <manifest>

# 11. Verify Silver tables now exist
docker compose -f docker-compose.integration.yml exec -T timescaledb \
  psql -U postgres -d ndp -c "
    SELECT hypertable_name FROM timescaledb_information.hypertables
    WHERE hypertable_schema = 'silver';"
# Expected: air_quality_observations, weather_observations, etc.

# 12. Verify analytics views exist
docker compose -f docker-compose.integration.yml exec -T timescaledb \
  psql -U postgres -d ndp -c "
    SELECT table_name FROM information_schema.views
    WHERE table_schema = 'analytics';"
# Expected: forecast_accuracy, indoor_outdoor_comparison, latest_readings

# 13. Idempotency check: re-run init-scripts manually
for f in deploy/pi/init-scripts/*.sql; do
  docker compose -f docker-compose.integration.yml exec -T timescaledb \
    psql -U postgres -d ndp -f "/docker-entrypoint-initdb.d/$(basename $f)" 2>&1
done
# Expected: no errors (all IF NOT EXISTS / CREATE OR REPLACE)
```
