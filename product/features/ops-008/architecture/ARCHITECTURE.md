# ops-008 Architecture Decisions

## ADR-001: Two-Layer Bootstrap Architecture

### Context

NDP's database bootstrap has grown organically across 15+ features. Init-scripts and deploy.sh both create database objects, with no clear boundary between them. This causes failures on fresh databases because init-scripts assume objects created by deploy.sh already exist, and vice versa.

The database needs a deterministic bootstrap path: blank DB -> foundational structure -> config-driven objects. Currently, this path is broken because `002_state_events_schema.sql` references the `silver` schema (never created by init-scripts), and files sort incorrectly in C locale.

### Decision

Establish two distinct bootstrap layers:

**Layer 0 — Init-Scripts** (`deploy/pi/init-scripts/`):
- Run once by Docker `docker-entrypoint-initdb.d` when the PostgreSQL data directory is empty
- Create ONLY foundational infrastructure: extensions, schemas, roles, data_dictionary tables, utility functions
- Zero dependencies on applications or config files
- Self-contained: no script references objects outside the init-scripts set
- Contract: after Layer 0 completes, `data_dictionary` schema is structurally complete (empty), `silver`/`gold`/`analytics` schemas exist, roles exist, extensions loaded

**Layer 1 — deploy.sh apply** (`deploy/pi/deploy.sh`):
- Runs on every deployment (idempotent)
- Creates all config-driven objects: Silver hypertables, Gold CAs, aligned views, intelligence tables, dimensions, dictionary data
- Assumes Layer 0 contract is met
- Uses `CREATE ... IF NOT EXISTS` and `ON CONFLICT` for idempotency

The boundary rule: if an object's DDL can be derived from stream/domain config, it belongs in Layer 1. If it is structural infrastructure that must exist before any config processing, it belongs in Layer 0.

### Consequences

- Fresh databases bootstrap deterministically: init-scripts -> deploy.sh apply
- Init-scripts never break when stream configs change (they know nothing about streams)
- deploy.sh can safely assume schemas and roles exist
- Silver tables, Gold CAs, and intelligence objects are always in sync with their configs
- Dimension tables are no longer pre-created; dimension sync must handle table creation
- Analytics views cannot be in init-scripts (they reference Silver tables)
- `001_silver_schema.sql` in `deploy/timescaledb/` is effectively retired — its contents are decomposed across both layers

---

## ADR-002: Init-Script Naming Convention

### Context

The current init-scripts use two incompatible naming conventions: `NN-description.sql` (e.g., `01-create-data-dictionary.sql`) and `NNN_description.sql` (e.g., `002_state_events_schema.sql`). Docker sorts files in C locale, where `-` (ASCII 0x2D) sorts before digits (ASCII 0x30+) and `_` (ASCII 0x5F) sorts after uppercase letters. This causes `002_state_events_schema.sql` to sort between `00-` and `01-`, breaking dependency order.

### Decision

All init-scripts use the naming convention: `NNN-description.sql`

- Three-digit zero-padded prefix (001-009)
- Hyphen separator (not underscore)
- Lowercase description words separated by hyphens
- Extension: `.sql`

C locale sort verification: `001-` < `002-` < ... < `009-` because all characters after the digit prefix are hyphens (0x2D), which is consistent and predictable.

This convention matches the one proposed in SCOPE.md and avoids the interleaving bugs caused by mixed `NN-` and `NNN_` prefixes.

### Consequences

- All 10 existing files must be replaced (not renamed) to ensure clean state
- Sort order is guaranteed correct in C locale for any number of scripts up to 999
- New scripts can be added between existing ones (e.g., `003-x.sql` sorts between `002-` and `004-`)
- The convention is simple to enforce in code review

---

## ADR-003: Dimension Table Creation Strategy

### Context

`04-dimension-tables.sql` currently creates `silver.entity_context` and `silver.dimension_sync_log` in init-scripts. It also creates dimension sync functions (truncate_and_load, start_dimension_sync, complete_dimension_sync). deploy.sh Phase 8 runs `ndp dimension sync` which does `TRUNCATE` + `COPY FROM CSV`, requiring these tables to exist.

Two options were considered:
- **Option A**: Keep dimension DDL in init-scripts (simple, but couples init to specific dimension schemas)
- **Option B**: Make `ndp dimension sync` create tables if missing via `ensure_table()` (already exists in Rust code behind a feature flag)

### Decision

**Option B**: Remove dimension tables from init-scripts. Activate `ensure_table()` in `ndp dimension sync` so it creates tables idempotently if they do not exist.

The dimension sync functions (`truncate_and_load_dimension`, `start_dimension_sync`, `complete_dimension_sync`, `update_entity_context_timestamp` trigger) are created by a deploy.sh migration that runs before Phase 8, or by the dimension sync command itself.

### Consequences

- Init-scripts are fully config-free — they know nothing about specific dimension schemas
- `ndp dimension sync` becomes fully self-bootstrapping
- The Rust feature flag for `ensure_table()` must be activated (minor code change)
- Dimension sync functions need a new home: either a deploy.sh migration SQL file or created by the Rust dimension sync command
- If the user adds new dimension types in the future, no init-script changes are needed

---

## ADR-004: DQ Events Table Placement

### Context

`silver.dq_events` is a cross-stream data quality event table defined in `001_silver_schema.sql`. It is a TimescaleDB hypertable with 7-day chunks and a 30-day retention policy. Silver ETL writes DQ events to this table during Bronze-to-Silver processing.

Options:
- Include in init-scripts as foundational Silver infrastructure
- Move to deploy.sh migration (runs after Silver tables exist)

### Decision

Move `silver.dq_events` to a deploy.sh migration rather than init-scripts.

Rationale:
1. It is a hypertable with retention policies — these are config-driven concerns managed by deploy.sh
2. No init-script object depends on `silver.dq_events`
3. Silver ETL runs only after deploy.sh has set up Silver tables, so dq_events does not need to exist at init time
4. Keeping init-scripts free of TimescaleDB policies (compression, retention) maintains the clean Layer 0/Layer 1 boundary

### Consequences

- Init-scripts contain zero `create_hypertable`, `add_compression_policy`, or `add_retention_policy` calls
- deploy.sh needs a new migration step that creates `silver.dq_events` as a hypertable with its policies
- The migration is idempotent (`IF NOT EXISTS`)
- Silver ETL's first run will succeed because deploy.sh runs before Silver ETL starts

---

## ADR-005: Schema Version Table Removal

### Context

`silver.schema_version` is used by `001_silver_schema.sql` and `04-dimension-tables.sql` to track which schema migrations have been applied. Rows include versions like `'1.0.0'` and `'003-dimensions'`.

NDP now uses deploy.sh manifests (`.deploy/releases/vX.Y.Z.manifest.json`) and git tags for version tracking. The in-database `schema_version` table is vestigial.

### Decision

Drop `silver.schema_version` from init-scripts. Do not create it, do not write to it. Existing production databases will retain the table (it is harmless), but new databases will not have it.

### Consequences

- Init-scripts are simpler (no version tracking logic)
- `04-dimension-tables.sql`'s prerequisite check for `schema_version` is eliminated
- Any code that reads `silver.schema_version` will need to handle its absence (grep confirms no Rust code reads it)
- Manifest-based versioning in `.deploy/releases/` is the single source of truth

---

## ADR-006: Silver Continuous Aggregates Deferral

### Context

`001_silver_schema.sql` defines 4 Silver continuous aggregates:
- `silver.air_quality_hourly`
- `silver.weather_observations_hourly`
- `silver.outdoor_air_quality_hourly`
- `silver.air_quality_daily`

These are separate from Gold CAs (created by deploy.sh Phase 5). They require the underlying Silver hypertables to exist, which means they cannot be in init-scripts (Layer 0 does not create Silver tables).

Options:
- Make Silver CAs config-driven in deploy.sh (significant effort)
- Keep them in `silver-etl migrate` (existing pathway)
- Drop them entirely (they may be redundant with Gold CAs)

### Decision

Defer Silver CAs to the existing `silver-etl migrate` pathway. This is out of scope for ops-008.

The `silver-etl migrate` command (invoked by `deploy.sh silver-migrate`) already creates these CAs. No changes needed in ops-008. Making them config-driven is a potential follow-up feature.

### Consequences

- ops-008 scope stays focused on init-scripts and the Layer 0/Layer 1 boundary
- Silver CAs continue to be created by `silver-etl migrate`
- A future feature could make Silver CAs config-driven (from stream config `silver.aggregates` section)
- The `001_silver_schema.sql` file in `deploy/timescaledb/` remains the source of truth for Silver CA definitions until that follow-up

---

## ADR-007: Analytics Views Migration Path

### Context

`001_silver_schema.sql` defines 3 analytics views and the `analytics` schema:
- `analytics.forecast_accuracy` (joins forecasts to observations)
- `analytics.indoor_outdoor_comparison` (hourly indoor/outdoor comparison)
- `analytics.latest_readings` (latest from all Silver tables)

These views reference Silver tables (`silver.weather_forecasts`, `silver.air_quality_observations`, `silver.outdoor_air_quality`, `silver.weather_observations`) that do not exist at init time. They cannot be in init-scripts.

### Decision

Create the `analytics` schema in init-scripts (`002-schemas.sql`), but move the analytics views to a deploy.sh migration SQL file that runs after Phase 4 (Silver table creation).

The migration file lives at `deploy/pi/migrations/001-analytics-views.sql` and is executed by deploy.sh Phase 3 (migrations) or a new post-Phase-4 step. Since migrations run via `psql -f <file>`, the views are created with `CREATE OR REPLACE VIEW` for idempotency.

### Consequences

- `analytics` schema exists from init (Layer 0) for future use
- Analytics views are created only after Silver tables exist (Layer 1)
- deploy.sh gets a new migration file for analytics views
- The views are always in sync with Silver table schema (if Silver schema changes, the migration can be updated)
- Grafana dashboards that use analytics views will work after `deploy.sh apply` completes

---

## Cross-References

- ADR-001 builds on ops-007 ADR-005 (clean-slate, Pattern ID 21) which established the need for deterministic DB bootstrap
- ADR-002 addresses the root cause identified in SCOPE.md Section 2 (broken sort ordering)
- ADR-003 relates to ops-007 ADR-006 (manifest-per-testbed, Pattern ID 22) which requires clean-slate capability
- ADR-007 references the existing deploy.sh Phase 3 migrations mechanism (Pattern ID 3, deploy-sh-ndp-dispatch)
