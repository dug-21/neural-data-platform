# ops-008 Specification: Database Bootstrap & Init-Script Consolidation

## Objective

Replace the 10 broken init-scripts in `deploy/pi/init-scripts/` with 9 correctly-ordered, self-contained SQL scripts that deterministically bootstrap a blank PostgreSQL database for NDP. Establish a clean two-layer bootstrap architecture where init-scripts provide the foundation (Layer 0) and `deploy.sh apply` creates all config-driven objects (Layer 1).

## Requirements

### R-01: Init-Script Replacement

Replace all 10 existing files:
- `00-create-extensions.sql`
- `002_state_events_schema.sql`
- `003_silver_data_dictionary.sql`
- `004_stream_classification.sql`
- `005_domain_objectives.sql`
- `006_pgvector_extension.sql`
- `01-create-data-dictionary.sql`
- `02-create-users.sql`
- `03-add-computed-columns.sql`
- `04-dimension-tables.sql`

With 9 new files using `NNN-description.sql` naming (three-digit zero-padded, hyphen separator):
- `001-extensions.sql`
- `002-schemas.sql`
- `003-silver-functions.sql`
- `004-roles.sql`
- `005-data-dictionary.sql`
- `006-silver-dictionary.sql`
- `007-classification.sql`
- `008-domain-objectives.sql`
- `009-dictionary-views.sql`

### R-02: C Locale Sort Order Correctness

All 9 scripts must sort correctly in C locale (`LC_ALL=C sort`), matching their numeric prefix order. Docker's `docker-entrypoint-initdb.d` runs scripts in this order. No script may depend on objects created by a later-sorting script.

### R-03: Self-Contained Foundation

Init-scripts must create ONLY foundational objects:
- **Extensions**: `timescaledb`, `vector`
- **Schemas**: `data_dictionary`, `silver`, `gold`
- **Roles**: `ndp_app`, `grafana_reader`
- **Tables**: All `data_dictionary.*` tables (core, silver metadata, classification, domain objectives)
- **Views**: All `data_dictionary.*` views
- **Functions**: `data_dictionary.*` functions + `silver.*` utility functions (AQI, mold risk, interpolation)
- **Grants**: Schema USAGE + SELECT for `grafana_reader` on `data_dictionary`

Init-scripts must NOT create:
- Silver hypertables (deploy.sh Phase 4)
- Silver continuous aggregates (deploy.sh / silver-etl migrate)
- Gold continuous aggregates (deploy.sh Phase 5)
- Gold aligned views, events, intelligence (deploy.sh Phase 6)
- `silver.state_events` (deploy.sh Phase 4 from config)
- `gold.events_with_context` view (references non-existent Silver tables)
- Any table DATA (populated by deploy.sh Phases 8-9)
- Dimension tables (`silver.entity_context`, `silver.dimension_sync_log`) -- see R-07

### R-04: Idempotency

Every DDL statement must use `IF NOT EXISTS`, `CREATE OR REPLACE`, or `DO $$ ... END $$` guards. Re-running init-scripts on an existing database must be safe (no errors, no data loss).

### R-05: Integration Environment First

Implementation targets the integration environment:
1. Write new init-scripts
2. `docker compose -f docker-compose.integration.yml down -v && up -d`
3. Verify zero errors in TimescaleDB init logs
4. `DEPLOY_ENV=integration deploy.sh apply <manifest>` creates dynamic objects
5. Smoke test passes end-to-end

### R-06: Production Compatibility

The same init-scripts must work identically on Pi production (PG15 TimescaleDB). No integration-specific SQL.

### R-07: Dimension Table Strategy (Resolved)

**Decision**: Dimension tables (`silver.entity_context`, `silver.dimension_sync_log`) are NOT included in init-scripts. The `ndp dimension sync` command will create tables if missing via its existing `ensure_table()` code path (currently behind a feature flag). This keeps init-scripts config-free and makes dimension sync fully idempotent.

The dimension sync functions (`truncate_and_load_dimension`, `start_dimension_sync`, `complete_dimension_sync`) and trigger (`update_entity_context_timestamp`) move to deploy.sh as a migration or are created by the dimension sync command itself.

### R-08: Analytics Views Migration (Resolved)

**Decision**: Analytics views (`analytics.forecast_accuracy`, `analytics.indoor_outdoor_comparison`, `analytics.latest_readings`) are NOT in init-scripts because they reference Silver tables that do not exist at init time. They move to a deploy.sh migration script that runs after Phase 4 (Silver table creation). The `analytics` schema is still created by init-scripts (in `002-schemas.sql`).

### R-09: DQ Events Table (Resolved)

**Decision**: `silver.dq_events` is a cross-stream foundational table (not config-driven per stream). It is included in init-scripts as part of `003-silver-functions.sql` since it relates to the Silver layer infrastructure. However, it cannot be a hypertable at init time because init-scripts should not create TimescaleDB policies on tables that may need to be managed by deploy.sh. Instead, it is created as a regular table with a comment noting that deploy.sh can convert it to a hypertable when Silver tables are created.

**Update**: After further analysis, `silver.dq_events` is better placed in a deploy.sh migration rather than init-scripts, because: (a) it is a hypertable with retention policies, which are config-driven concerns, and (b) no init-script object depends on it. The Silver ETL code that writes DQ events runs after deploy.sh has set up Silver tables. This keeps init-scripts purely foundational DDL.

### R-10: Schema Version Table (Resolved)

**Decision**: `silver.schema_version` is DROPPED from init-scripts. Version tracking is handled by deploy.sh manifests and git tags, not an in-database table. The existing `schema_version` rows are vestigial from the pre-deploy.sh era.

### R-11: Silver Continuous Aggregates (Deferred)

Silver CAs (`silver.air_quality_hourly`, `silver.weather_observations_hourly`, `silver.outdoor_air_quality_hourly`, `silver.air_quality_daily`) are NOT in scope for ops-008. They currently live in `001_silver_schema.sql` and are created by `silver-etl migrate`. Making them config-driven is a follow-up task. For ops-008, the existing `silver-etl migrate` pathway continues to handle them.

### R-12: Retire `deploy/timescaledb/init/001_silver_schema.sql` Dependency

Init-scripts no longer depend on `001_silver_schema.sql`. Its responsibilities are decomposed:
- Schema creation -> `002-schemas.sql`
- Utility functions -> `003-silver-functions.sql`
- Role creation -> `004-roles.sql`
- Silver tables/hypertables/CAs/policies -> deploy.sh Phase 4 / silver-etl migrate
- Analytics views -> deploy.sh migration (post-Phase 4)
- DQ events table -> deploy.sh migration

## Acceptance Criteria

| AC-ID | Description | Verification |
|-------|-------------|-------------|
| AC-01 | `docker compose -f docker-compose.integration.yml down -v && up -d` succeeds with zero errors in TimescaleDB init logs | shell: check logs for ERROR |
| AC-02 | All 10 old init-scripts removed from `deploy/pi/init-scripts/` | file-check: old files absent |
| AC-03 | 9 new init-scripts present with correct naming (`NNN-description.sql`) | file-check: ls shows 9 files |
| AC-04 | C locale sort order matches numeric order (001 < 002 < ... < 009) | shell: `LC_ALL=C ls` output |
| AC-05 | Init-scripts create `data_dictionary`, `silver`, `gold` schemas | shell: psql query pg_namespace |
| AC-06 | Init-scripts create `ndp_app` and `grafana_reader` roles | shell: psql query pg_roles |
| AC-07 | All data_dictionary tables exist after init (streams, fields, sources, entity_schemas, entity_schema_attributes, sync_status, silver_tables, silver_columns, silver_lineage, silver_dq_rules, stream_classification, gold_tables, domains, domain_streams, objectives, constraints) | shell: psql query information_schema |
| AC-08 | Silver utility functions exist (linear_interpolate, calculate_aqi_pm25, calculate_mold_risk) | shell: psql query pg_proc |
| AC-09 | No init-script creates Silver hypertables, Gold CAs, or intelligence tables | grep: no CREATE TABLE silver. (except functions), no create_hypertable |
| AC-10 | No init-script references `001_silver_schema.sql` | grep: no mention in comments |
| AC-11 | `deploy.sh apply` succeeds after fresh init (creates Silver tables, Gold, dimensions) | shell: run deploy.sh apply |
| AC-12 | `03-add-computed-columns.sql` (no-op) removed | file-check: absent |
| AC-13 | `002_state_events_schema.sql` (redundant) removed | file-check: absent |
| AC-14 | Re-running init-scripts on existing DB produces no errors | shell: psql re-execute test |
| AC-15 | Grafana reader has SELECT on data_dictionary schema | shell: psql has_schema_privilege |
| AC-16 | Integration smoke test passes end-to-end | shell: run-testbed.sh smoke |

## Interface Contracts

### Layer 0 -> Layer 1 Contract

Init-scripts (Layer 0) guarantee to deploy.sh (Layer 1):
1. `data_dictionary` schema exists with all tables empty but structurally complete
2. `silver` schema exists (empty)
3. `gold` schema exists (empty)
4. `analytics` schema exists (empty)
5. `ndp_app` role exists with LOGIN
6. `grafana_reader` role exists with LOGIN
7. `timescaledb` extension loaded
8. `vector` extension loaded
9. Silver utility functions available in `silver` schema

deploy.sh (Layer 1) does NOT assume:
- Any Silver tables exist
- Any Gold objects exist
- Any data in data_dictionary tables
- Any dimension tables exist
- `silver.dq_events` exists (created by migration)
- `silver.schema_version` exists (dropped)

### Naming Convention

- File: `NNN-description.sql` (three-digit zero-padded, hyphen-separated words)
- C locale sort: `0` < `1` < ... < `9`, `-` (0x2D) < `0` (0x30)
- No underscore-prefixed files (avoids `_` ASCII 0x5F sorting issues)
