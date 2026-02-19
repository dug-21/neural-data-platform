# dp-Series Architecture Pattern Seed

**Generated**: 2026-02-19
**Source**: dp-001 through dp-023 SCOPE.md and architecture/*.md files
**Purpose**: AgentDB seed for foundational data platform patterns

---

## Deprecated Approaches (CRITICAL — Flag These)

### Pattern: DuckDB Eliminated

- **taskType**: deprecated:duckdb-analytics-layer
- **approach**: DuckDB was the ORIGINAL analytics layer planned in dp-001 (standalone container querying Parquet). It was REMOVED in dp-002 — TimescaleDB replaced it as the Silver layer foundation. DuckDB container no longer exists in production deployment. The Grafana DuckDB plugin for direct Bronze Parquet queries is retained separately (read-only), but the DuckDB container/service is gone. DO NOT re-introduce DuckDB as a container or ETL engine. Reference: dp-002 SCOPE.md, dp-006 SCOPE.md (ADR-006-001 selected duckdb-rs embedded but that was later superseded by the streaming subscriber model in dp-012).
- **successRate**: 0.0
- **tags**: deprecated, duckdb, analytics, eliminated, silver-layer
- **source**: dp-001, dp-002, dp-006
- **status**: deprecated

### Pattern: Polars Streaming Eliminated

- **taskType**: deprecated:polars-streaming-etl
- **approach**: Polars with streaming was considered as an ETL engine alternative during the Silver layer research phase (dp-006). It was NOT selected. The project uses TimescaleDB continuous aggregates and event-driven Silver subscriber instead. DO NOT use Polars streaming for ETL. Reference: dp-006 SCOPE.md (ADR-006-001 context).
- **successRate**: 0.0
- **tags**: deprecated, polars, streaming, eliminated
- **source**: dp-006
- **status**: deprecated

### Pattern: Batch Silver ETL (apps/silver-etl) Deprecated

- **taskType**: deprecated:silver-etl-batch-app
- **approach**: The `apps/silver-etl/` Rust binary was the original batch ETL approach (hourly, DuckDB-based). It was superseded by dp-012's unified event bus architecture where Silver ETL is an inline event subscriber in air-quality-app. The `apps/silver-etl/` crate is deprecated and must NOT be referenced or modified. Active Silver data path: `core/src/subscribers/silver.rs` (SilverSubscriber). Active DDL path: `deploy/pi/ddl-generator.sh`. Reference: dp-023 SCOPE.md "Architecture Clarification" section.
- **successRate**: 0.0
- **tags**: deprecated, silver-etl, batch, apps-silver-etl, duckdb
- **source**: dp-012, dp-023
- **status**: deprecated

---

## Architecture Patterns

### Pattern: Bronze→Silver→Gold Data Lake Architecture

- **taskType**: architecture:data-lake-pipeline
- **approach**: The platform uses a three-tier data lake: (1) **Bronze** = raw JSON stored as Parquet files with hive-style partitioning (`/data/raw/{stream_id}/year=YYYY/month=MM/day=DD/data.parquet`). Standard envelope: `{timestamp, source_id, ndp_id, context, raw_payload, day, month, year}`. Bronze MUST always succeed — it is the source of truth. (2) **Silver** = typed, DQ-flagged data in TimescaleDB hypertables via event-driven streaming subscriber. Silver is best-effort — can fail without breaking Bronze. (3) **Gold** = materialized views, continuous aggregates, feature views in PostgreSQL/TimescaleDB schema for ML and dashboards. Text fields use per-domain VIEWs (not materialized). Reference: dp-004, dp-006, dp-012, dp-023.
- **successRate**: 1.0
- **tags**: architecture, bronze, silver, gold, data-lake, timescaledb, parquet
- **source**: dp-004, dp-006, dp-012, dp-023
- **status**: current

### Pattern: Config-Driven Stream Definition (JSON v2)

- **taskType**: architecture:stream-config-format
- **approach**: All streams are defined in `config/base/streams/{stream_id}/config.json` using JSON format (v2). Key sections: `stream_id`, `stream_type` (observation/forecast/events), `fields[]` (Bronze schema with name, type, unit, description, nullable, range), `sources[]` (MQTT/HTTP config), `silver_etl` (ETL config with field_mappings, DQ rules, deduplication, incremental watermark), `gold_etl` (aggregates, features). Config is synced to etcd. etcd is the runtime source of truth. JSON was chosen over YAML: agent reliability (no indentation errors), MCP-native, JSON Schema validation, strict parsing. entity_schemas was deprecated in v1.1 and removed in v2.0 — use enriched `fields[]` with `description` instead. Config version tracking: `"config_version": 2`. Reference: dp-016 ADR-016-001, dp-018, dp-021.
- **successRate**: 1.0
- **tags**: config, json, stream-config, etcd, gitops, v2
- **source**: dp-016, dp-018, dp-021
- **status**: current

### Pattern: Unified Event Bus (Silver as Subscriber)

- **taskType**: architecture:event-bus-silver-subscriber
- **approach**: Since dp-012, all data flows through a broadcast channel event bus inside air-quality-app. Sources (MQTT, HTTP) emit `RawDataPoint` events. BronzeSubscriber writes Parquet. SilverSubscriber (active, streaming) writes to TimescaleDB via `core/src/subscribers/silver.rs` → `core/src/silver/transform.rs` → `core/src/silver/outputs/timescale.rs`. Silver ETL is event-driven (1-5 second latency), not batch (5+ minutes). Config-driven subscribers: enable/disable without code changes. Additional subscribers: alerts, MQTT event notifications. This architecture superseded the `apps/silver-etl/` batch approach. Reference: dp-012 SCOPE.md.
- **successRate**: 1.0
- **tags**: architecture, event-bus, subscriber, silver, streaming, real-time
- **source**: dp-012
- **status**: current

### Pattern: Silver ETL Field Mapping Config

- **taskType**: architecture:silver-etl-field-mapping
- **approach**: Silver ETL config in stream `config.json` under `silver_etl.field_mappings[]`. Each mapping: `source_path` (dot-notation into raw_payload, e.g., `"raw_payload.pm02Compensated"`), `target_column` (Silver column name), `type` (double_precision, smallint, text, jsonb, boolean), `unit`, `description`, `nullable`, `dq_rules[]`. DQ rules support: `range_check` (min/max with action flag/reject/clamp/drop), `cross_field_check`, `freshness_check`, `rate_of_change`, `completeness_check`. DQ flags land in `dq_flags TEXT[]` column. Default action is `flag` (transparency over rejection). Silver tables always include: `observation_time TIMESTAMPTZ`, `ndp_id TEXT`, `dq_flags TEXT[]`. Reference: dp-006 architecture, dp-023, air-quality config.json.
- **successRate**: 1.0
- **tags**: silver-etl, field-mapping, dq-rules, timescaledb, config-driven
- **source**: dp-006, dp-009, dp-023
- **status**: current

### Pattern: TimescaleDB Hypertable Schema Standard

- **taskType**: architecture:timescaledb-hypertable-schema
- **approach**: Silver tables are TimescaleDB hypertables with standard structure: `{observation_time TIMESTAMPTZ NOT NULL, ndp_id TEXT NOT NULL, ...domain_columns..., dq_flags TEXT[], _bronze_id UUID, _ingested_at TIMESTAMPTZ DEFAULT NOW()}`. Primary key: `(observation_time, ndp_id)` for observations; `(event_time, ndp_id, event_type)` for events; `(issue_time, valid_time, ndp_id)` for forecasts. DDL is generated by `deploy/pi/ddl-generator.sh` (NOT apps/silver-etl). Hypertable chunk_time_interval: 1 day. Compression after 7 days. Retention 90 days (configurable). Permissions: `ndp_app` (SELECT/INSERT), `grafana_reader` (SELECT). Indexes: `(observation_time, ndp_id)` standard + GIN on `dq_flags`. Reference: dp-006, dp-020.
- **successRate**: 1.0
- **tags**: timescaledb, hypertable, schema, silver, indexes, compression, retention
- **source**: dp-006, dp-020
- **status**: current

### Pattern: Stream Types Distinction

- **taskType**: architecture:stream-type-classification
- **approach**: Stream configs declare `stream_type` at top level. Three types: `"observation"` (continuous measurements with regular intervals, PK: observation_time + ndp_id), `"forecast"` (future predictions with issue_time and valid_time dual timestamps, PK: issue_time + valid_time + ndp_id), `"events"` (discrete state changes, PK: event_time + ndp_id + event_type). The distinction controls: ETL query patterns, primary key structure, Gold layer treatment (CAs for observations, SCDs for events, per-domain VIEWs for forecasts). Reference: dp-006 ADR-006-006.
- **successRate**: 1.0
- **tags**: stream-type, observation, forecast, events, schema
- **source**: dp-006, dp-014, dp-023
- **status**: current

### Pattern: Declarative Deploy with Manifest

- **taskType**: procedure:declarative-deploy
- **approach**: Deployment uses `./deploy.sh apply` with a manifest file at `.deploy/releases/vX.Y.Z.manifest.json`. Declaration types in manifest: `stream` (sync to etcd + reload), `silver-table` (DDL generation: CREATE TABLE or ADD COLUMN), `migration` (SQL file execution), `dimensions` (CSV→TimescaleDB sync), `dictionary` (data dictionary sync). Deploy orchestrates in correct order: validate → migrations → silver tables → stream sync → dictionary → dimension → reload → update device state. DDL is generated from `silver_etl.field_mappings` in stream configs. Idempotent (IF NOT EXISTS everywhere). Device state tracked at `/var/ndp/deployed-version`. See `docs/procedures/DEPLOYMENT-DECLARATIVES.md` for full reference. Reference: dp-020 SCOPE.md.
- **successRate**: 1.0
- **tags**: deployment, manifest, declarative, deploy.sh, ddl-generation
- **source**: dp-020, dp-021
- **status**: current

### Pattern: Config Validation Pipeline

- **taskType**: procedure:config-validation
- **approach**: Two-layer validation via `ndp-validate` tool (`tools/ndp-validate/`). Layer 1 (Schema): JSON Schema validation (jsonschema crate) — catches malformed JSON, missing required fields, unknown fields (additionalProperties: false), invalid enum values. Layer 2 (Semantic): Rust code validates application rules — valid field types against NDP-supported types, cross-reference validation (source_path must exist in fields), Silver table existence check in TimescaleDB, DQ rule syntax. Validation is gated in `deploy.sh sync` — bad config blocks deployment. Error output is structured JSON with path and actionable message. Supported types: float, string, integer, boolean, timestamp, json (Bronze); double_precision, smallint, bigint, text, jsonb, boolean, timestamptz (Silver). Reference: dp-019 SCOPE.md.
- **successRate**: 1.0
- **tags**: validation, json-schema, ndp-validate, config, deployment
- **source**: dp-019, dp-018
- **status**: current

### Pattern: Data Dictionary — Bronze and Silver

- **taskType**: architecture:data-dictionary
- **approach**: Queryable data dictionary in `data_dictionary` PostgreSQL schema. Bronze: `streams`, `fields`, `sources`, `entity_schemas` (deprecated v1.1+), `entity_schema_attributes`, `sync_status` tables. Silver extension (dp-009): `silver_tables` (table metadata, grain, source_streams), `silver_columns` (name, type, unit, description, nullable), `silver_lineage` (source_stream + source_path → silver_table + silver_column), `silver_dq_rules` (rule definitions). Unified view `v_complete_dictionary` spans Bronze + Silver. Populated by `deploy.sh sync-dictionary`. Config-driven: adding stream config auto-populates dictionary. Reference: dp-002, dp-009.
- **successRate**: 1.0
- **tags**: data-dictionary, metadata, bronze, silver, lineage, timescaledb
- **source**: dp-002, dp-009
- **status**: current

### Pattern: MCP Server for Data Exploration

- **taskType**: architecture:mcp-server-design
- **approach**: NDP MCP server (`apps/ndp-mcp-server/`) exposes platform data via HTTP JSON-RPC. Bronze tools: `list_streams`, `describe_schema` (modes: source/target/all), `validate_config`, `sample_data`. Silver tools: `list_silver_tables`, `describe_silver_table`, `sample_silver_data`, `silver_stats`. Dictionary tools: `query_dictionary`, `describe_column`, `trace_lineage`, `list_dq_rules`. Diagnostic tools: `etl_status`, `data_freshness`. Server architecture: axum HTTP → MCP handler → tool implementations → storage adapters (BronzeStorage trait for Parquet, SilverStorage trait for TimescaleDB, DictionaryStore trait for data_dictionary). Config via env vars. Cloud-portable design (same server works on Pi or cloud). Reference: dp-005, dp-010 SCOPE.md.
- **successRate**: 1.0
- **tags**: mcp, server, data-exploration, bronze, silver, data-dictionary
- **source**: dp-005, dp-010
- **status**: current

### Pattern: MCP Server Storage Adapter Traits

- **taskType**: architecture:mcp-storage-traits
- **approach**: MCP tools use trait abstractions for storage: `BronzeStorage` (list streams, introspect Parquet schema, sample rows), `SilverStorage` (list tables, describe schema, sample data, stats), `DictionaryStore` (search, column details, lineage, DQ rules). Each trait has impl for local (Pi): `LocalParquetStorage`, `TimescaleDbStorage`, `PostgresDictionaryStore`. Cloud impl deferred. Trait approach enables testability and portability. Bronze schema discovery uses Parquet introspection — no hardcoded expectations. Reference: dp-005, dp-010.
- **successRate**: 1.0
- **tags**: mcp, traits, storage-adapter, parquet, timescaledb, testability
- **source**: dp-005, dp-010
- **status**: current

### Pattern: Bronze Raw JSON Envelope Schema

- **taskType**: architecture:bronze-raw-json-schema
- **approach**: Bronze layer stores raw source payloads in a standard envelope (dp-004). Schema: `{timestamp INT64 (ms), source_id STRING, ndp_id STRING?, context JSON (config snapshot), raw_payload JSON (exact source payload untransformed), day INT, month INT, year INT}`. Domain fields (temperature, pm25) are INSIDE raw_payload as JSON, NOT as separate Parquet columns. Field extraction happens in Silver. Hive-style partitioning: `/data/raw/{stream_id}/year=YYYY/month=MM/day=DD/data.parquet`. One file per day, grows throughout day. Schema defined in `core/src/types/raw_data_point.rs`. Reference: dp-004 SCOPE.md.
- **successRate**: 1.0
- **tags**: bronze, parquet, raw-json, envelope, schema, hive-partitioning
- **source**: dp-004
- **status**: current

### Pattern: Config Source of Truth — JSON→etcd→Runtime

- **taskType**: architecture:config-source-of-truth
- **approach**: JSON files (`config/base/streams/*/config.json`) are the primary source of truth. etcd is the runtime cache. All runtime components MUST read from etcd, not from files directly. Flow: JSON file (git-versioned) → `deploy.sh sync` → etcd (stores JSON natively) → runtime components. Key pattern in etcd: `/streams/{stream_id}/config`. Silver subscriber (event-driven, streaming) reads config from etcd via ConfigLoader trait (`EtcdConfigLoader`). Data dictionary sync reads from etcd. Sync failures must be ERROR level (not WARN). Config loading must log which source was used. Violating this pattern causes silent failures (P-001 from dp-016). Reference: dp-016 ADR-016-001, dp-018.
- **successRate**: 1.0
- **tags**: config, etcd, json, source-of-truth, runtime, gitops
- **source**: dp-016, dp-018
- **status**: current

### Pattern: Config-Driven Silver Table Creation (DDL Generator)

- **taskType**: procedure:silver-table-ddl-generation
- **approach**: Silver tables are created from stream config — no manual SQL required. DDL generator script: `deploy/pi/ddl-generator.sh` (sourced by deploy.sh). Input: `silver_etl.field_mappings[]` from stream config. Type mapping: `float/double_precision → DOUBLE PRECISION`, `string/text/varchar → TEXT`, `integer/bigint → BIGINT`, `smallint → SMALLINT`, `boolean → BOOLEAN`, `timestamp → TIMESTAMPTZ`, `json/jsonb → JSONB`. Generated SQL: CREATE TABLE IF NOT EXISTS, indexes on (timestamp, ndp_id), GIN on dq_flags, create_hypertable(), compression policy (7 days), retention policy (90 days), GRANT to ndp_app and grafana_reader. Schema evolution: ADD COLUMN via `DO $$ IF NOT EXISTS` block. Idempotent: safe to run multiple times. Invoked via manifest `silver-table` declaration. Reference: dp-015, dp-020.
- **successRate**: 1.0
- **tags**: silver-table, ddl-generator, ddl, config-driven, timescaledb, schema-evolution
- **source**: dp-015, dp-019, dp-020
- **status**: current

### Pattern: MQTT Multi-Subscription Config

- **taskType**: architecture:mqtt-multi-subscription
- **approach**: MqttSource supports multiple topic subscriptions per broker connection (dp-003). Config format: `sources[].subscriptions[]` with `{stream_id, topic_pattern}` per subscription. One MQTT connection to Mosquitto, multiple topic patterns. Messages routed to correct stream based on topic pattern match. This enables HomeAssistant stream alongside air-quality stream without separate broker connections. Config in `sources[].broker_url`, `sources[].subscriptions[]`. Reference: dp-003 SCOPE.md.
- **successRate**: 1.0
- **tags**: mqtt, multi-subscription, config-driven, source-routing
- **source**: dp-003
- **status**: current

### Pattern: Silver Text and JSONB Field Types

- **taskType**: architecture:silver-non-numeric-fields
- **approach**: Silver supports non-numeric field types (dp-023). Text fields: `type: "text"` in field_mappings → PostgreSQL TEXT column. JSONB fields: `type: "jsonb"` → PostgreSQL JSONB column with explicit `::jsonb` cast in INSERT. Coercion in `core/src/silver/transform.rs` → `coerce_to_type()`. Text coercion: `Value::String → pass-through`. JSONB coercion: `Value::Object|Array → pass-through`, `Value::String → validate as JSON`, `Value::Null|Number|Bool → pass-through`. Mixed streams (numeric + text in one silver_etl config) are supported. No NLP/text processing in pipeline — text is pass-through. Gold layer: text reaches Gold via per-domain VIEWs using DISTINCT ON for latest value (not materialized, not CAs). Reference: dp-023.
- **successRate**: 1.0
- **tags**: text, jsonb, non-numeric, silver, field-types, mixed-streams
- **source**: dp-023
- **status**: current

### Pattern: Gold Text View Pattern

- **taskType**: architecture:gold-text-view
- **approach**: Gold layer uses per-domain VIEWs (not MATERIALIZED VIEWs) for text fields. Generator: `TextViewGenerator` in `crates/ndp-lib/src/gold/generators/text_view.rs`. View name: `gold.{domain_id}_text`. Schema: unpivoted `(observation_time, source_stream, field_name, value TEXT)`. Uses `DISTINCT ON (source_stream, field_name)` ordered by observation_time DESC to return latest text per field. Multiple text-bearing streams in same domain appear in UNION ALL. Config-driven: generator reads domain config to find streams with text/jsonb mappings. WHY VIEW not MATERIALIZED: always current, no refresh orchestration, no storage overhead on Pi. Grafana can filter by field_name. Intelligence apps (fe-005) query Gold text view when waking on existing gold_refresh NOTIFY. Reference: dp-023 ADR-003.
- **successRate**: 1.0
- **tags**: gold, text-view, distinct-on, per-domain, grafana, config-driven
- **source**: dp-023
- **status**: current

### Pattern: Dimension Tables Config and Load

- **taskType**: procedure:dimension-tables
- **approach**: Reference/lookup data (not timeseries) uses dimension table configs at `config/base/dimensions/*.yaml` (or .json in v2). Config fields: `dimension_id`, `target.table` (Silver table, e.g., `silver.entity_context`), `target.primary_key[]`, `source.type: csv`, `source.path`, `schema.fields[]` (name, data_type, required), `load.strategy` (truncate_and_load or upsert). Dimensions are NOT Bronze streams — they load directly to Silver. `deploy.sh sync` (or `dimensions` manifest declaration) processes dimension configs. CLI: `ndp dimension sync <id>`. Example: `silver.entity_context` with columns `ndp_id, category, friendly_name, location_path, correlates_with, orientation`. Gold views join Silver facts with Silver dimension tables. Reference: dp-013.
- **successRate**: 1.0
- **tags**: dimensions, reference-data, csv, silver, config-driven
- **source**: dp-013
- **status**: current

### Pattern: ETL Run Statistics Persistence

- **taskType**: architecture:etl-run-statistics
- **approach**: Silver ETL runs are persisted to `silver.etl_runs` table for operational history. Schema: `{id BIGSERIAL, stream_id TEXT, started_at TIMESTAMPTZ, completed_at TIMESTAMPTZ, status TEXT (running/success/failed), rows_processed BIGINT, rows_flagged BIGINT, rows_rejected BIGINT, duration_ms BIGINT, watermark_before TIMESTAMPTZ, watermark_after TIMESTAMPTZ, error_message TEXT, error_context JSONB}`. Indexes: `(stream_id, started_at DESC)` and partial index on failed status. Run record created at start with `status='running'`, updated on completion. MCP tool `etl_status` queries this table. Migration: `004_etl_runs.sql`. Reference: dp-011.
- **successRate**: 1.0
- **tags**: etl, statistics, silver, operational-monitoring, timescaledb
- **source**: dp-011
- **status**: current

### Pattern: Config-Driven Gold Layer (Materialized Views)

- **taskType**: architecture:gold-config-driven-views
- **approach**: Gold layer artifacts (materialized views, feature views) are defined in YAML/JSON config at `config/base/gold/*.yaml`. Config schema: `gold_view_id`, `type` (materialized_view/view/continuous_aggregate), `description`, `source.table` or `source.query`, `refresh.strategy` (on_demand/scheduled/continuous), `columns[]` (name, type, source, transform), `indexes[]`. DDL generator (`GoldDdlGenerator`) generates: CREATE MATERIALIZED VIEW, indexes, REFRESH command. Deploy integration: `./deploy.sh sync-gold-views`, `./deploy.sh refresh-gold-view <id>`. SCD Type 2 pattern for event streams: LEAD() OVER PARTITION BY to compute valid_from/valid_to. Reference: dp-014.
- **successRate**: 0.8
- **tags**: gold, materialized-view, config-driven, scd, feature-engineering
- **source**: dp-014
- **status**: current

### Pattern: Release Versioning and Manifest Naming

- **taskType**: procedure:release-methodology
- **approach**: NDP uses Semantic Versioning (MAJOR.MINOR.PATCH). MAJOR = breaking (schema v2.0, API changes). MINOR = new features (new stream, new MCP tool). PATCH = bug fixes, config corrections. Each release has three artifacts: (1) `.deploy/releases/vX.Y.Z.manifest.json` (what to deploy), (2) git tag `vX.Y.Z` (annotated), (3) `CHANGELOG.md` entry. Manifest includes `release_version` field matching git tag. Device state: `/var/ndp/deployed-version` updated after successful deploy. Release template: `.deploy/releases/TEMPLATE.manifest.json`. See `docs/procedures/RELEASE-POLICY.md` for full procedure. Reference: dp-021 Phase R.
- **successRate**: 1.0
- **tags**: release, semver, manifest, git-tag, changelog, deployment
- **source**: dp-021
- **status**: current

### Pattern: Integration Environment for Testing

- **taskType**: procedure:integration-environment
- **approach**: Local Docker integration environment mirrors production. Compose file: `docker-compose.integration.yml`. Enable via `DEPLOY_ENV=integration ./deploy.sh deploy`. Same services as production: etcd, mosquitto, timescaledb, air-quality-app, grafana, ndp-mcp-server. Test script: `scripts/integration-test.sh start/stop/clean/status`. All deployment changes MUST be validated in integration environment before Pi deployment. `DEPLOY_ENV=integration ./deploy.sh apply` runs full declarative deploy locally. Silver tables queryable at `docker exec integration-timescaledb psql -U postgres -d ndp`. Reference: dp-017, dp-020 testing expectations.
- **successRate**: 1.0
- **tags**: integration-test, docker-compose, testing, environment, local
- **source**: dp-017, dp-020
- **status**: current

### Pattern: Config Schema Versioning (JSON Schema)

- **taskType**: architecture:config-schema-versioning
- **approach**: Stream configs carry `"config_version": 2` field. Version history: v1.0 = YAML, entity_schemas required; v1.1 = JSON, entity_schemas deprecated, enriched fields supported; v2.0 = JSON, entity_schemas FORBIDDEN, enriched fields required. Schema files: `schemas/stream-config.schema.json` (v1/v2), `schemas/dimension-config.schema.json`, `schemas/manifest.schema.json`. Validation enforces version constraints. Migration script: `scripts/ndp-migrate-config.sh` (shell+jq) for v1.1→v2.0. v2.0 schema uses `additionalProperties: false` to catch unknown fields. Reference: dp-018, dp-021.
- **successRate**: 1.0
- **tags**: config-schema, versioning, json-schema, migration, entity_schemas
- **source**: dp-018, dp-021
- **status**: current

### Pattern: Hot-Reload for Sources (Not Subscribers)

- **taskType**: architecture:hot-reload-sources
- **approach**: Source config changes (MQTT topic, HTTP polling interval) trigger hot-reload via etcd watch without full app restart. Flow: etcd watch detects config change → SourceManager::on_config_change(stream_id) → graceful disconnect of old sources → create new sources with new config. Scope: MQTT source reconnects, HTTP source reconfiguration. NOT supported: Bronze/Silver subscriber hot-reload (requires coordinator refactoring — full restart needed). Optional HTTP reload endpoint for manual trigger. Reference: dp-021 Phase 4.
- **successRate**: 1.0
- **tags**: hot-reload, sources, etcd-watch, mqtt, http, source-manager
- **source**: dp-021
- **status**: current

### Pattern: MCP Write Tools Architecture (Future)

- **taskType**: architecture:mcp-write-tools
- **approach**: MCP write tools (dp-022) are BLOCKED until access controls are designed. Planned tools: `create_stream`, `update_stream`, `delete_stream`, `validate_stream`, `create_silver_table`, `reload_stream`. Flow: MCP tool → validate JSON → write config.json → trigger deploy.sh apply → git commit/push. Long-term architecture: Rust library crates callable from both CLI and MCP (no code duplication between ndp-validate CLI and MCP validate_stream tool). Access control required: authentication, RBAC (read-only vs admin), audit logging. Reference: dp-022.
- **successRate**: 0.5
- **tags**: mcp, write-tools, admin, access-control, future
- **source**: dp-022
- **status**: experimental

### Pattern: Grafana Dashboard Patterns for Silver Layer

- **taskType**: architecture:grafana-silver-dashboards
- **approach**: Three core Grafana dashboard patterns for Silver layer (dp-008): (1) **Pipeline Health** — config-driven (discovers streams from Silver tables dynamically, queries information_schema), freshness thresholds per stream type (MQTT every 30s, HTTP poll every 10 min), DQ flag summary. (2) **Forecast Accuracy** — JOIN weather_forecasts to weather_observations on valid_time ±30 min, compute error by lead time bucket (1h, 3h, 6h, 12h, 24h, 48h). (3) **Indoor Environment** — ventilation recommendation logic (CO2 > 800 AND outdoor temp 18-26°C AND humidity < 70% AND AQI < 50 AND precip < 20%). Temperature unit toggle via dashboard variable (°F/°C). TimescaleDB data source: `timescaledb-silver`. Reference: dp-008.
- **successRate**: 0.9
- **tags**: grafana, dashboards, silver, forecast-accuracy, ventilation, pipeline-health
- **source**: dp-008
- **status**: current

### Pattern: Pre-Transform Parser for Complex Data

- **taskType**: architecture:pre-transform-parser
- **approach**: For streams with columnar array data (NWS gridpoints forecasts), the Silver ETL supports a pre-transform step. Config: `silver_etl.pre_transform` section. Integrates `ColumnOrientedParser` from `neural-core` (`core/src/parsers/column_oriented.rs`). Flow: Bronze Parquet (raw JSON with arrays) → Rust pre-transform (ColumnOrientedParser flattens arrays to rows, one row per metric per validTime) → flattened temp table → DuckDB SQL (standard field extraction) → Silver TimescaleDB. This handles NWS gridpoints columnar structure where metrics contain `[{validTime, value}]` arrays. Reference: dp-007.
- **successRate**: 0.8
- **tags**: pre-transform, column-oriented-parser, nws-gridpoints, silver-etl
- **source**: dp-007
- **status**: current

### Pattern: Data Quality Transparency

- **taskType**: architecture:dq-transparency
- **approach**: DQ violations are flagged transparently, not silently rejected. Four actions: `flag` (keep value, add to dq_flags — DEFAULT), `reject` (set NULL, add to dq_flags), `clamp` (clamp to bounds, add flag), `drop` (drop entire row). All non-drop actions recorded in `dq_flags TEXT[]` column per row. DQ transparency table: `silver.dq_transparency` stores samples of violations with payload context. Rule types: `range_check` (min/max), `cross_field_check` (SQL expression), `freshness_check` (max_age, max_future), `rate_of_change` (max_change_per_minute, partition_by), `completeness_check` (min_completeness level). DQ flags enable downstream filtering without data loss. Reference: dp-006 ADR-006-004, air-quality config.json.
- **successRate**: 1.0
- **tags**: dq, data-quality, transparency, flags, silver, timescaledb
- **source**: dp-006
- **status**: current

### Pattern: Webhook-Triggered Deployment (Future)

- **taskType**: architecture:webhook-deployment
- **approach**: dp-023 establishes foundation for automated deployment. Future dp-023 webhook receiver on Pi: GitHub webhook (tag push v*) → Pi webhook receiver → git pull → locate `.deploy/releases/v{tag}.manifest.json` → `./deploy.sh apply <manifest>` → report status to GitHub. Device state after: `/var/ndp/deployed-version = vX.Y.Z`. Spec documented in `docs/procedures/WEBHOOK-DEPLOYMENT-SPEC.md`. Reference: dp-021 Phase R, dp-023.
- **successRate**: 0.5
- **tags**: webhook, deployment, automation, github, future
- **source**: dp-021
- **status**: experimental

---

## Key File Reference

| Pattern Area | Key Files |
|---|---|
| Bronze envelope schema | `core/src/types/raw_data_point.rs` |
| Silver subscriber (ACTIVE) | `core/src/subscribers/silver.rs` |
| Silver transform (ACTIVE) | `core/src/silver/transform.rs`, `coerce_to_type()` |
| Silver output (ACTIVE) | `core/src/silver/outputs/timescale.rs` |
| Silver DDL generator (ACTIVE) | `deploy/pi/ddl-generator.sh` |
| Stream configs | `config/base/streams/{stream_id}/config.json` |
| Dimension configs | `config/base/dimensions/*.json` |
| Gold DDL generators | `crates/ndp-lib/src/gold/generators/` |
| MCP server | `apps/ndp-mcp-server/src/` |
| Validation tool | `tools/ndp-validate/` |
| Deploy script | `deploy/pi/deploy.sh` |
| Release manifests | `.deploy/releases/vX.Y.Z.manifest.json` |
| Release template | `.deploy/releases/TEMPLATE.manifest.json` |
| Integration compose | `docker-compose.integration.yml` |
| Deployment procedures | `docs/procedures/DEPLOYMENT-DECLARATIVES.md` |
| Release policy | `docs/procedures/RELEASE-POLICY.md` |
| DEPRECATED batch ETL | `apps/silver-etl/` (DO NOT MODIFY) |
