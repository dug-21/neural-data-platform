# ops-008: Database Bootstrap & Init-Script Consolidation

## Vision

A new NDP device (or integration environment) should be able to start from a blank PostgreSQL database and deterministically arrive at a working state from scratch. The app/deployment process for new streams should create all data structures dynamically through config post initial setup. The database needs some foundational structure in place before that dynamic capability takes over, such as db creation, user permissioning, etc. This scope is to focus on creating the full structure upon initial startup to implement the necessary structure automatically. This can be a deployment-declaration to initialize the database. This will be HEAVILY utilized in integration testing, and would also be useful for creation of new edge devices from scratch in prod.

Today this doesn't work. The init-scripts in `deploy/pi/init-scripts/` have grown organically across 15+ features with two incompatible naming conventions, broken sort ordering, missing schema creation steps, and implicit dependencies on scripts that don't exist. A `docker compose down -v && up -d` against a fresh database fails immediately because `002_state_events_schema.sql` tries to create `silver.state_events` but no script creates the `silver` schema. Some of these scripts are no longer needed, and created when the system was not fully configuration driven.

This blocks ops-007 (integration testbed framework) from running its clean-slate smoke test, and it means deploying NDP to a new Pi requires manual intervention to get the database into a working state.

## The Problem

### 1. Missing schema creation
No init-script creates `CREATE SCHEMA silver`. The `silver` schema was originally created by `001_silver_schema.sql` which lives at `deploy/timescaledb/init/` and `deploy/timescaledb/migrations/` — it was never copied into `deploy/pi/init-scripts/`. It is applied separately via `deploy.sh silver-migrate` which runs `silver-etl migrate`. This means:
- Init-scripts that reference `silver.*` tables fail on a fresh database
- `002_state_events_schema.sql` hard-fails (no `IF NOT EXISTS`, no schema guard)
- `04-dimension-tables.sql` checks for `silver` and throws an exception if missing
- `02-create-users.sql` grants permissions on `silver` schema that doesn't exist yet

Similarly, there's no `CREATE SCHEMA gold` until `04-dimension-tables.sql` (which fails before reaching that statement), and no `CREATE SCHEMA intelligence` at all.

**Correction from original scope**: The `silver` schema is NOT created at runtime by `air-quality-app`. Research confirms air-quality-app creates zero DB objects at runtime — it only performs INSERT/UPSERT DML. The silver schema is created by the `silver-etl migrate` CLI command, which is invoked by `deploy.sh`.

### 2. Broken sort ordering — halts entire init
Docker's `docker-entrypoint-initdb.d` runs scripts in C locale sort order. With `ON_ERROR_STOP=on` (PostgreSQL default in modern images), the **entire init process halts at the first failure**. The actual C locale sort order is:

```
#  File                          Outcome on blank DB
── ─────────────────────────── ───────────────────────────────────────────
1  00-create-extensions.sql     SUCCEEDS (no dependencies)
2  002_state_events_schema.sql  FAILS → silver schema missing → INIT HALTS
3  003_silver_data_dictionary.sql  (never reached)
4  004_stream_classification.sql   (never reached)
5  005_domain_objectives.sql       (never reached)
6  006_pgvector_extension.sql      (never reached)
7  01-create-data-dictionary.sql   (never reached)
8  02-create-users.sql             (never reached)
9  03-add-computed-columns.sql     (never reached)
10 04-dimension-tables.sql         (never reached)
```

The result: on a clean `docker compose up`, the database has only the `timescaledb` extension and nothing else. Even if Docker continued past failures, files 003/004/005 would fail because they depend on `data_dictionary.streams`, `data_dictionary.fields`, and `data_dictionary.sync_status` — all created by `01-create-data-dictionary.sql` which sorts AFTER them.

Two naming conventions (`NN-name.sql` and `NNN_name.sql`) interleave incorrectly. The `01-` series was the original set; the `002_`-`006_` series was added by later features without realizing `002_` sorts between `00-` and `01-` in C locale (because `-` ASCII 0x2D < `2` ASCII 0x32).

### 3. Ghost dependency on `001_silver_schema.sql`
Multiple scripts reference `001_silver_schema.sql` in comments ("Prerequisites: 001_silver_schema.sql must be run first") but this file does not exist in `deploy/pi/init-scripts/`. It exists at:
- `deploy/timescaledb/init/001_silver_schema.sql` (924 lines — full schema + tables + CAs + functions + roles)
- `deploy/timescaledb/migrations/001_silver_schema.sql` (270 lines — migration version)

This file creates: `silver` and `analytics` schemas, all Silver hypertables, Silver continuous aggregates, utility functions (AQI, mold risk), analytics views, `ndp_app` role, compression/retention policies. None of this is available at init-script time.

### 4. No-op and vestigial scripts
- `03-add-computed-columns.sql` is entirely commented out — a no-op placeholder that should be removed
- The `gold.events_with_context` view in `04-dimension-tables.sql` references `silver.air_quality_observations` which doesn't exist at init time
- `002_state_events_schema.sql` creates `silver.state_events` which is also created config-driven by `ddl-generator.sh` Phase 4 from `home-assistant-state` stream config — the init-script version is redundant

### 5. Production Dockerfile issues (out of scope, noted)
- `docker/timescaledb/Dockerfile` uses `apt-get` on an Alpine base image — won't build
- Contains `init-scripts/` from a different project (`neural_trader` schema) — vestigial
- Pi compose uses PG15 (`timescale/timescaledb:latest-pg15`) but Dockerfiles reference PG16
- The production Pi compose does NOT use this Dockerfile — it uses the pre-built image directly

---

## Research Findings

### Two-Layer Bootstrap Architecture

Research confirms the database bootstrap has two distinct layers:

**Layer 0 — Init-Scripts** (Docker entrypoint, first boot only):
Creates the foundational infrastructure that deploy.sh expects to exist. Runs once when the PostgreSQL data directory is empty.

**Layer 1 — deploy.sh apply** (every deploy):
Creates all config-driven objects from stream and domain configs. Runs idempotently on every deployment.

### What deploy.sh Creates Dynamically (Layer 1)

deploy.sh `apply` runs an 11-phase orchestration. The DB-touching phases are:

| Phase | What | How | Creates |
|-------|------|-----|---------|
| 3 | Migrations | `psql -f <file>` | One-time schema changes from `.sql` files |
| 4 | Silver Tables | `ddl-generator.sh` per stream config | `CREATE SCHEMA IF NOT EXISTS silver`, `CREATE TABLE`, hypertables, indexes, compression/retention, grants |
| 5 | Gold Tables | `ndp gold sync` per stream config | `CREATE SCHEMA IF NOT EXISTS gold`, continuous aggregates, refresh policies |
| 6 | Domains | `ndp gold generate --domain` | Aligned materialized views, events tables/CAs/procedures, intelligence tables (metric_embeddings, predictions, graph_nodes, graph_edges, reasoning_bank), refresh jobs |
| 8 | Dimensions | `ndp dimension sync` | `TRUNCATE` + `COPY FROM CSV` (requires tables to exist) |
| 9 | Dictionary | `ndp dictionary sync` | `DELETE` + `INSERT/UPSERT` into all `data_dictionary.*` tables |

**Key insight**: deploy.sh Phase 4 creates `silver` schema with `CREATE SCHEMA IF NOT EXISTS`. Phase 5 creates `gold` schema. Phase 6 creates intelligence objects in `gold.*` schema. Init-scripts do NOT need to create Silver tables, Gold CAs, or intelligence tables — deploy.sh handles all of that from config.

### What Init-Scripts MUST Provide (Layer 0)

For deploy.sh to work, init-scripts must provide:

1. **Extensions**: `timescaledb`, `vector` (pgvector)
2. **Schemas**: `data_dictionary` (deploy.sh Phase 9 assumes it exists; Phases 4-6 create `silver`/`gold` themselves)
3. **Roles**: `ndp_app` (grants in Phase 4 DDL), `grafana_reader` (grants in Phase 4 DDL)
4. **data_dictionary tables**: `streams`, `fields`, `sources`, `entity_schemas`, `entity_schema_attributes`, `sync_status` (Phase 9 populates these)
5. **data_dictionary silver extension tables**: `silver_tables`, `silver_columns`, `silver_lineage`, `silver_dq_rules` (Phase 9 populates these)
6. **data_dictionary classification tables**: `stream_classification`, `gold_tables` (Phase 9 populates these)
7. **data_dictionary domain tables**: `domains`, `domain_streams`, `objectives`, `constraints` (Phase 6 Step F populates these)
8. **data_dictionary views and functions**: All views/functions that query the above tables
9. **Grants**: Schema USAGE + SELECT on `data_dictionary` for `grafana_reader`

### What Init-Scripts Should NOT Contain

These are created dynamically by deploy.sh from config and should NOT be in init-scripts:

- Silver hypertables (`silver.air_quality_observations`, etc.) — Phase 4
- Silver continuous aggregates (`silver.air_quality_hourly`, etc.) — currently in `001_silver_schema.sql`, should migrate to config-driven
- Gold continuous aggregates (`gold.air_quality_hourly`, etc.) — Phase 5
- Gold aligned views (`gold.indoor_air_quality_aligned`) — Phase 6
- Gold events infrastructure (`gold.events`, etc.) — Phase 6
- Intelligence tables (`gold.metric_embeddings`, etc.) — Phase 6
- `silver.state_events` — Phase 4 from `home-assistant-state` config
- `gold.events_with_context` view — references Silver tables that don't exist at init
- Any table DATA (data dictionary content, dimension data) — Phases 8-9

### Dimension Tables — Special Case

`silver.entity_context` and `silver.dimension_sync_log` (from `04-dimension-tables.sql`) are dimension infrastructure. deploy.sh Phase 8 runs `TRUNCATE` + `COPY FROM CSV` which requires the tables to exist. Options:
- **Option A**: Include dimension table DDL in init-scripts (simple, but couples init to specific dimensions)
- **Option B**: Have `ndp dimension sync` create tables if missing via `ensure_table()` (already exists in Rust code behind feature flag)
- **Recommended**: Option B — make the dimension sync command idempotent with table creation. This keeps init-scripts config-free.

### Silver Utility Functions — Special Case

`001_silver_schema.sql` defines three utility functions used by continuous aggregates and views:
- `silver.linear_interpolate()` — AQI calculation helper
- `silver.calculate_aqi_pm25()` — EPA PM2.5 AQI
- `silver.calculate_mold_risk()` — mold risk index

These are domain-specific functions that Silver CAs and Gold views depend on. They need to exist before CAs are created. Options:
- **Option A**: Include in init-scripts (always available, but domain-specific in generic bootstrap)
- **Option B**: Include in a deploy.sh "functions" phase that runs before Phase 4
- **Recommended**: Option A for now — they're small, stable, and breaking them out of init-scripts adds complexity without clear benefit. Place them in a dedicated `003-silver-functions.sql`.

### Analytics Views — Special Case

`001_silver_schema.sql` defines `analytics.*` views that reference Silver tables:
- `analytics.forecast_accuracy` — joins forecasts to observations
- `analytics.indoor_outdoor_comparison` — hourly indoor/outdoor comparison
- `analytics.latest_readings` — latest from all Silver tables

These CANNOT be in init-scripts because they reference Silver tables that don't exist yet. They should be created by deploy.sh after Silver tables are created (new Phase or appended to Phase 4).

### Intelligence Schema Clarification

The original scope mentioned `CREATE SCHEMA intelligence`. Research shows there is NO separate `intelligence` schema. All intelligence objects (`metric_embeddings`, `predictions`, `graph_nodes`, `graph_edges`, `reasoning_bank`) live in the `gold` schema. The `006_pgvector_extension.sql` only creates the `vector` extension, not an intelligence schema. Init-scripts just need `CREATE EXTENSION IF NOT EXISTS vector`.

### Current Object Inventory

**Total init-script DDL objects today** (across all 10 files):
- 2 extensions, 2 schemas (`data_dictionary`, `gold`), 1 user
- 20 tables, 14 views, 12 functions, 1 trigger
- ~25 indexes, ~10 grants, 3 TimescaleDB policies

**After ops-008** (init-scripts only create foundation):
- 2 extensions, 1-2 schemas (`data_dictionary`, optionally `silver`/`gold` for safety)
- ~14 tables (data_dictionary only), ~12 views, ~10 functions
- ~15 indexes, ~5 grants, 0 TimescaleDB policies

---

## What Done Looks Like

After ops-008:
- `docker compose -f docker-compose.integration.yml down -v && up -d` succeeds with zero errors in TimescaleDB init logs
- Init-scripts create only the foundation: extensions, schemas, roles, `data_dictionary` tables, utility functions
- All Silver/Gold/Intelligence tables are created by `deploy.sh apply` from config (not hardcoded in SQL)
- Init-scripts use a single naming convention: `NNN-description.sql` (three-digit zero-padded, hyphen separator)
- No init-script depends on application-created objects — they are fully self-contained
- `./tests/integration/run-testbed.sh smoke` passes end-to-end from clean slate (init-scripts + deploy.sh apply)
- The same init-scripts work identically in production (Pi deploy) and integration
- `deploy.sh apply` handles Silver table creation, Gold generation, intelligence setup, dimension loading, dictionary sync — all config-driven and idempotent
- Vestigial scripts removed: `03-add-computed-columns.sql`, `002_state_events_schema.sql` (redundant with config-driven Phase 4)
- Analytics views migrated out of init-scripts into deploy.sh (post-Silver-creation)
- `001_silver_schema.sql` in `deploy/timescaledb/` is retired — its contents are split between init-scripts (functions) and deploy.sh (everything else)
- Dimension table creation is idempotent in the dimension sync command

---

## Scope

### New Init-Script Set

Replace all 10 current files with a clean, correctly-ordered set:

```
deploy/pi/init-scripts/
  001-extensions.sql        ← CREATE EXTENSION timescaledb, vector
  002-schemas.sql           ← CREATE SCHEMA data_dictionary, silver, gold
  003-silver-functions.sql  ← Utility functions (linear_interpolate, calculate_aqi_pm25, calculate_mold_risk)
  004-roles.sql             ← CREATE ROLE ndp_app, grafana_reader + schema grants
  005-data-dictionary.sql   ← Core data_dictionary tables (streams, fields, sources, entity_schemas, sync_status)
  006-silver-dictionary.sql ← Silver metadata tables (silver_tables, silver_columns, silver_lineage, silver_dq_rules)
  007-classification.sql    ← stream_classification, gold_tables
  008-domain-objectives.sql ← domains, domain_streams, objectives, constraints
  009-dictionary-views.sql  ← All data_dictionary views and functions
```

Naming convention: `NNN-description.sql` — three-digit zero-padded with hyphens. Sorts correctly in C locale.

### deploy.sh Changes

- **Phase 4 (Silver Tables)**: Already creates Silver tables from config via `ddl-generator.sh`. No changes needed — init-scripts now provide the schemas and roles it needs.
- **Analytics views**: Add creation of `analytics.*` views after Phase 4 Silver table creation (new step or migration).
- **Phase 8 (Dimensions)**: Make `ndp dimension sync` create tables if missing (activate `ensure_table()` — already exists in Rust behind feature flag). Remove dependency on dimension tables being in init-scripts.
- **Retire `001_silver_schema.sql`**: Its responsibilities are now split:
  - Functions → `003-silver-functions.sql` (init-script)
  - Schema creation → `002-schemas.sql` (init-script)
  - Tables/hypertables/CAs/policies → deploy.sh Phase 4 (config-driven)
  - Analytics views → deploy.sh new step (post-Silver)
  - Role creation → `004-roles.sql` (init-script)

### What Stays Config-Driven (deploy.sh)

- All Silver hypertables, indexes, compression, retention
- All Silver continuous aggregates
- All Gold continuous aggregates, state transitions
- All domain aligned views, events, intelligence
- Dimension table DDL (ensure_table)
- Dimension data loading
- Data dictionary population

### Integration Environment Focus

Implementation should focus on the integration environment first:
1. Write new init-scripts
2. Test with `docker compose -f docker-compose.integration.yml down -v && up -d`
3. Verify init-scripts complete without errors
4. Run `DEPLOY_ENV=integration deploy.sh apply <manifest>` to create dynamic objects
5. Run `./tests/integration/run-testbed.sh smoke` end-to-end
6. Once integration passes, verify production compatibility (same init-scripts)

### Out of Scope

- Production Dockerfile fix (`docker/timescaledb/Dockerfile` Alpine/apt-get bug) — tracked separately
- PG15 vs PG16 version alignment between production and integration
- Neural-trader vestigial artifacts in `docker/timescaledb/`
- Making Silver continuous aggregates config-driven (currently hardcoded in `001_silver_schema.sql` — can be a follow-up)
- Changes to the Rust applications (air-quality-app, ndp-intelligence-app) — they already assume DDL exists

### Open Questions

1. **Silver CAs**: `001_silver_schema.sql` hardcodes 4 Silver continuous aggregates (`silver.air_quality_hourly`, etc.). These are separate from the Gold CAs. Are they still needed alongside the Gold CAs? If yes, they should move to deploy.sh as a config-driven step. If no, they can be dropped. For ops-008, defer this — deploy.sh Phase 4 + `silver-etl migrate` already handles this pathway.

2. **`silver.dq_events` table**: Currently in `001_silver_schema.sql`. This is a cross-stream DQ event table, not config-driven per stream. Needs a home — either init-scripts (if considered foundational) or a deploy.sh migration.

3. **`silver.schema_version` table**: Used by migration tracking. Currently in `001_silver_schema.sql` and `04-dimension-tables.sql`. Needs a home in init-scripts if migration tracking is still used, or can be dropped if deploy.sh handles versioning through manifests.

## Tracking

https://github.com/dug-21/neural-data-platform/issues/22
