# Changelog

All notable changes to the Neural Data Platform will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.1.10] - 2026-02-06

Wire events generator into ndp-gold-ddl CLI and deployment pipeline.

### Added

- **`--events` CLI flag** for `ndp-gold-ddl generate --domain <id> --events`
  - Generates events hypertable, unified view, hourly CAs, detection procedure, and scheduled job
  - Config-driven via `events` section in `domain.json`
- **`events_hourly_by_entity` continuous aggregate** — per-entity/stream hourly event rollup
  - Groups by `entity_id`, `stream_id` with same 15-minute refresh schedule
  - Compound index on `(entity_id, bucket DESC)`
- **Events section in `domain.json`** — `enabled`, `chunk_interval`, `retention`, `detection_schedule`
- **deploy.sh events DDL generation** — `handle_domain()` now generates and applies events DDL
  - Uses `ON_ERROR_STOP=1` for reliable error detection (fixes v1.1.6 silent failure root cause)

### Changed

- **Manifest format** — replaced migration-based deployment with domain declaration + tool build
- **`EventsGenerator::from_domain_config()`** — now reads `events` config from domain when present

### Removed

- **008_events_hourly_refresh_policies.sql** — generator handles refresh policies natively

## [1.1.9] - 2026-02-06

Deployment Tooling Foundation (ops-001). Extracts dictionary and dimension sync from Bash into Rust, with London TDD.

### Added

- **ndp-lib crate** (`crates/ndp-lib/`)
  - `sync_dictionary()` - Replaces ~460 lines of Bash SQL generation with parameterized queries
  - `sync_dimension()` - CSV-to-Silver dimension import with batch INSERT
  - `DbClient` trait (query/execute/batch_execute) for mockable database access
  - `ConfigLoader` trait for source-agnostic config loading (files today, etcd later)
  - `SyncReport` structured output from all sync operations
  - 57 London TDD tests with MockDbClient

- **ndp-cli tool** (`tools/ndp-cli/`)
  - `ndp dictionary sync` - Sync data dictionary from stream configs
  - `ndp dimension sync <id>` - Sync dimension table from CSV
  - Entity/verb CLI structure designed for V1.1-V2.0 journey
  - `--dry-run` support on all commands
  - Environment-aware config resolution (DEPLOY_ENV=integration|pi)

- **Dimension config JSON migration** (ops-001-12)
  - `config/base/dimensions/entity_context.json` (YAML preserved as fallback)

- **deploy.sh integration** (ops-001-09)
  - `command -v ndp` fallback wrapper around `sync_to_data_dictionary()`
  - `command -v ndp` fallback wrapper around `sync_dimension()`
  - `ndp-cli` added to supported tool builds
  - JSON-priority glob for dimension configs (`*.json` before `*.yaml`)

### Fixed

- **Stale data dictionary bug** (ops-001-13) - Bash `sync_to_data_dictionary()` read legacy `.yaml` files instead of authoritative `.json` configs; Rust implementation reads `.json` only

### Technical Notes

- Functions take parsed structs, not file paths (source-agnostic for files/etcd)
- Parameterized SQL ($1, $2) replaces Bash string concatenation
- Bronze layer: DELETE + INSERT; Silver layer: UPSERT
- 600+ tests passing across all crates (ndp-lib: 57, ndp-types: 88, ndp-gold-ddl: 338, ndp-validate: 217)
- No changes to existing crate code - purely additive

## [1.1.8] - 2026-02-05

Domain Configuration Standardization (FE-002). Migrates domain configs from YAML to JSON, fixes schema format, and adds CLI validation.

### Changed

- **Domain Config Format** (GAP-001)
  - Migrated `domain.yaml` → `domain.json` with FLAT format
  - Updated `ndp-gold-ddl` loader to use `serde_json`
  - Removed `serde_yaml` dependency from ndp-gold-ddl

- **Schema Format Standardization** (GAP-004)
  - Fixed `domain.schema.json` to use FLAT format (no wrapper object)
  - Updated semantic validator to expect FLAT format
  - All NDP configs now consistently use flat format

### Added

- **Domain Validation CLI** (GAP-003)
  - `ndp-validate --domain <path>` - Validate single domain config
  - `ndp-validate --domain-all` - Validate all domain configs
  - Layer 1 (JSON Schema) + Layer 2 (Semantic) validation
  - 38+ new validation tests (217 total in ndp-validate)

- **Golden Master Tests**
  - 13 golden master tests for DDL output verification
  - 12 SQL baseline fixtures with SHA256 checksums
  - Normalized SQL comparison for non-deterministic ordering

- **Deploy.sh Integration**
  - `validate_domain_configs()` function in Phase 1
  - Domain config validation before deployment

### Fixed

- Non-deterministic column ordering in aligned view generator
  - Fields now sorted alphabetically before DDL generation

### Removed

- `config/domains/indoor-air-quality/domain.yaml` (replaced by domain.json)

### Technical Notes

- Resolves GitHub issues #11 (GAP-001) and #13 (GAP-003)
- 556 total tests passing (339 ndp-gold-ddl + 217 ndp-validate)
- Implements FE-002 SPARC specification

## [1.1.7] - 2026-02-05

Patch release fixing event detection procedure column mismatch.

### Fixed

- **Event Detection Procedure** - Column name mismatch in `gold.get_event_context()`
  - Fixed `state_last_state` → `state_state_last` to match aligned view schema
  - Events now correctly capture window state context

- **Dashboard Column Names** - Fixed queries using wrong column names
  - Aligned view: `outdoor_aqi_pm25_mean`, `outdoor_temperature_c_mean`
  - Weather context: `temperature_c_mean`, `humidity_pct_mean`, `pressure_pa_mean`
  - State timeline: `ndp_id`, `state_last`

- **ndp-gold-ddl Events Generator** - Fixed column references in generated SQL
  - `outdoor_temperature_mean` → `outdoor_temperature_c_mean`
  - `outdoor_aqi_mean` → `outdoor_aqi_pm25_mean`
  - Added `window_state` with correct `state_state_last` column

### Migration Notes

- Re-applies `004_detect_events_procedure.sql` (idempotent via CREATE OR REPLACE)
- Run `CALL gold.detect_events();` after deployment to detect historical events

## [1.1.6] - 2026-02-05

Gold Layer Foundation - Phase E: Unified Event Abstraction (FE-001). Events hypertable, threshold crossings, and Gold Layer Dashboard.

### Added

- **Events Hypertable** (v11-013)
  - `gold.events` TimescaleDB hypertable for event storage
  - 7-day chunk interval, 1-year retention, 30-day compression
  - All event types: state_transition, threshold_crossing
  - Context snapshot at event time for V1.2 correlation
  - V1.2 query pattern indexes (time, type+time, entity, objective, GIN on context/details)

- **Unified Events View** (v11-013)
  - `gold.events_unified` view for V1.2 API compatibility
  - JSONB details with backward-compatible schema
  - `gold.indoor_air_quality_events` domain-scoped view

- **Events Continuous Aggregates** (v11-013)
  - `gold.events_hourly` - hourly event counts by type
  - `gold.events_hourly_by_entity` - per-entity breakdown
  - 15-minute refresh policy

- **Event Detection Procedure** (v11-013)
  - `gold.detect_events()` procedure with idempotent detection
  - `gold.get_event_context()` helper for context capture
  - `gold.detect_events_for_range()` for manual backfill
  - TimescaleDB scheduled job (15-minute interval)
  - Management functions: pause, resume, set_interval

- **Threshold Crossing Generator** (v11-012)
  - `gold.detect_threshold_crossings()` function
  - All condition types: <, <=, >, >=, between
  - Crossing directions: rising, falling, entering_range, exiting_range_low, exiting_range_high
  - Dynamic objective lookup from `data_dictionary.objectives`
  - Monitoring views for oscillation analysis

- **Gold Layer Dashboard** (v11-014)
  - `config/grafana/dashboards/gold-layer-overview.json`
  - 20 panels across 4 rows
  - Row 1: Air Quality Metrics (PM2.5, CO2, Temp/Humidity)
  - Row 2: Cross-Stream Alignment (multi-stream overlay)
  - Row 3: Events & Objectives (timeline, gauges, tables)
  - Row 4: Data Quality & Volume
  - Threshold lines at 12 µg/m³ (PM2.5) and 800 ppm (CO2)
  - Dashboard variables for filtering

- **ndp-gold-ddl EventsGenerator**
  - `generators/events.rs` with 5 generation methods
  - `EventsConfig` struct for configuration
  - 34 new unit tests (311 total)

- **Acceptance Tests**
  - `acceptance_events_hypertable.sql` - hypertable schema tests
  - `acceptance_threshold_crossings.sql` - all condition type tests
  - `acceptance_unified_events.sql` - V1.2 query pattern tests
  - `acceptance_detection_job.sql` - job scheduling tests
  - `run_acceptance_tests.sh` - test runner script

### Changed

- STATUS.md: Phase E complete (95% overall progress)
- ndp-gold-ddl: Added events module to generators

### Technical Notes

- Events are stored in hypertable (not view) per Decision 12 in DECISIONS.md
- Enables continuous aggregates on events (impossible with UNION ALL view)
- Context captured from `gold.indoor_air_quality_aligned` at event time
- Detection job uses `last_successful_finish` for idempotency
- V1.2 Pattern Detection Engine can consume events via `gold.events_unified`

### Migration Notes

- No breaking changes from v1.1.x
- New tables/views are additive
- Detection job requires Silver layer tables to generate events
- Dashboard requires Grafana datasource configuration

## [1.1.2] - 2026-02-05

Gold Layer Foundation - Phase C (FE-001). Cross-stream alignment with 3 streams and objectives storage.

### Added

- **Cross-Stream Aligned View** (v11-005)
  - `gold.indoor_air_quality_aligned` materialized view
  - FULL OUTER JOIN across air-quality, outdoor-weather, home-assistant-state
  - NULL handling by stream type (ADR-FE001-004)
    - observation streams: preserve NULL
    - state_event streams: LOCF (Last Observation Carried Forward)
  - Column aliasing by stream alias (indoor_, outdoor_, state_)
  - COALESCE bucket across all streams

- **State Transition Materializer** (v11-006)
  - `gold.state_events_transitions` view
  - LAG window function for state change detection
  - `is_actual_transition` boolean for noise filtering
  - Duration in previous state calculation
  - Partitioned by entity (ndp_id)
  - Transition direction mapping (opening/closing)
  - 22 unit tests

- **Objectives Storage** (v11-007)
  - `data_dictionary.domains` table
  - `data_dictionary.domain_streams` mapping table
  - `data_dictionary.objectives` table with all condition types (<, >, <=, >=, ==, !=, between)
  - `data_dictionary.constraints` table for V1.3+ action framework
  - MCP queryable views: v_domain_overview, v_high_priority_objectives
  - `check_objective_violation()` function
  - deploy.sh commands: `sync-domains`, `list-domains`

- **Additional Stream Aggregates** (v11-003 extended)
  - `gold.outdoor_weather_hourly` - temperature, humidity, wind, pressure
  - `gold.outdoor_weather_daily` - daily aggregates
  - `gold.state_events_hourly` - state counts and changes
  - gold_etl configurations for outdoor-weather and home-assistant-state

- **Domain Configuration**
  - `config/domains/indoor-air-quality/domain.yaml`
  - 3 streams: air-quality (primary), outdoor-weather (context), home-assistant-state (actuator)
  - 4 objectives: healthy_co2, healthy_pm25, comfortable_humidity, comfortable_temperature

- **Unit Tests**
  - 87 new Phase C tests (279 total in ndp-gold-ddl)
  - aligned_view_tests.rs (31 tests)
  - state_transitions_tests.rs (27 tests)
  - objectives_tests.rs (29 tests)
  - Test fixtures with MockConfigLoader

### Changed

- outdoor-weather stream config: Added `stream_type: "observation"`, `gold_etl` section
- home-assistant-state stream config: Added `stream_type: "state_event"`, `gold_etl` section with transitions
- ndp-gold-ddl: Added `--transitions` CLI flag
- ndp-gold-ddl: State transitions generator module
- deploy.sh: Added sync-domains and list-domains commands

### Notes

- outdoor-air-quality stream deliberately **excluded** (reserved for Phase D fast-follower test)
- 3 streams in Gold layer aligned view
- Reference implementation for domain-centric configuration pattern
- V1.1.2 pattern detection ready with queryable aligned view

## [1.1.1] - 2026-02-04

Gold Layer Foundation - Phase B (FE-001). First Gold stream deployment with air-quality.

### Added

- **Stream Type Classification** (v11-001)
  - StreamType enum: observation, state_event, forecast, dimension
  - Correlation role derivation (effect, cause, context, metadata)
  - NULL handling strategy (preserve, carry_forward)
  - 14 unit tests

- **Classification Propagation** (v11-002)
  - SQL migration `004_stream_classification.sql`
  - `data_dictionary.stream_classification` table
  - `data_dictionary.gold_tables` catalog
  - Helper functions: derive_correlation_role(), derive_null_handling()
  - 17 unit tests

- **Per-Stream Continuous Aggregates** (v11-003)
  - `gold.air_quality_hourly` - Hourly aggregates
  - `gold.air_quality_daily` - Daily aggregates
  - 22 metrics across 7 fields (pm25, pm10, co2, temperature_c, humidity_pct, tvoc_index, nox_index)
  - Idempotent DDL generation with IF NOT EXISTS pattern

- **Aggregate Refresh Policy** (v11-004)
  - Granularity-aware defaults (hourly: 15min, daily: 1hr)
  - Idempotent policy creation with if_not_exists => TRUE
  - 38 unit tests

### Changed

- Air quality stream config: `gold_etl.enabled: true`
- Air quality stream config: Added `stream_type: "observation"`
- ndp-gold-ddl: 152 total tests (up from 113)

### Notes

- First **active** Gold layer deployment
- Reference implementation for future Gold stream rollouts
- Continuous aggregates refresh automatically on configured schedule

## [1.1.0] - 2026-02-04

Gold Layer Foundation - Phase A (FE-001). Tooling and schemas for Gold layer, no active deployment.

### Added

- **Gold DDL Tool** (`tools/ndp-gold-ddl/`)
  - Rust CLI for generating TimescaleDB continuous aggregate DDL
  - Aligned view generator with FULL OUTER JOIN strategy
  - Feature registry with extensible trait system (lag, rolling, trend)
  - 113 unit tests

- **Gold Layer JSON Schemas** (`config/schemas/`)
  - `gold-etl.schema.json` - Gold ETL configuration validation
  - `alignment.schema.json` - Cross-stream alignment rules
  - `objectives.schema.json` - Domain objectives and constraints
  - `domain.schema.json` - Domain configuration with embedded objectives

- **Gold Validation Extensions** (`tools/ndp-validate/`)
  - 9 Gold-specific error codes (400-408)
  - Semantic validation for `gold_etl` sections
  - Domain configuration validation
  - 179 total tests (up from 134)

- **Deploy.sh Gold Handlers**
  - New declaration types: `gold-tables`, `domains`
  - `handle_gold_table()` - Stream-level Gold DDL generation
  - `handle_domain()` - Domain-level aligned view DDL
  - Phases 5 (Gold Tables) and 6 (Domains) added to orchestration
  - Dry-run support for testing

- **Test Fixtures** (`tests/fixtures/`)
  - 27 test configuration files (valid + invalid)
  - Reference domain config at `config/domains/indoor-air-quality/`

### Changed

- Air quality stream config updated with `gold_etl` section (disabled until Phase B)
- Deploy.sh now has 11-phase orchestration (was 9)

### Documentation

- FE-001 Phase A SPARC artifacts (74 files)
- 5 Architecture Decision Records (ADR-FE001-001 through ADR-FE001-005)

### Notes

- Gold layer is **not active** in this release (`gold_etl.enabled: false`)
- Phase B (v1.1.1) will enable first Gold stream deployment

## [1.0.0] - 2026-02-02

First formal release establishing declarative deployment and release methodology.

### Added

- **Declarative Deployment System** (dp-020)
  - Manifest-driven deployment with `./deploy.sh apply`
  - 9-phase orchestration: validation, builds, migrations, silver tables, streams, dimensions, dictionary, restarts, device state
  - DDL generation from stream configuration (no manual SQL)
  - Device state tracking in `/var/ndp/`

- **Config Lifecycle & Release Management** (dp-021)
  - Hot-reload support for source configurations
  - Semantic versioning standard for NDP
  - Release manifest naming convention (`vX.Y.Z.manifest.json`)
  - Git tag to manifest alignment
  - Config schema migration v1.1 to v2.0

- **Configuration Validation Pipeline** (dp-019)
  - Two-layer validation (JSON Schema + Rust semantic)
  - 134 validation tests

### Changed

- Stream configs migrated to `config_version: 2` (removed deprecated `entity_schemas`)
- Air quality app rebuilt with hot-reload capability

### Documentation

- `docs/procedures/RELEASE-POLICY.md` - Versioning and release workflow
- `docs/procedures/DEPLOYMENT-DECLARATIVES.md` - Manifest format and declaration types
- `CLAUDE.md` - Added Release Methodology section for agents

---

[Unreleased]: https://github.com/dug-21/neural-data-platform/compare/v1.1.9...HEAD
[1.1.9]: https://github.com/dug-21/neural-data-platform/compare/v1.1.8...v1.1.9
[1.1.8]: https://github.com/dug-21/neural-data-platform/compare/v1.1.7...v1.1.8
[1.1.7]: https://github.com/dug-21/neural-data-platform/compare/v1.1.6...v1.1.7
[1.1.6]: https://github.com/dug-21/neural-data-platform/compare/v1.1.2...v1.1.6
[1.1.2]: https://github.com/dug-21/neural-data-platform/compare/v1.1.1...v1.1.2
[1.1.1]: https://github.com/dug-21/neural-data-platform/compare/v1.1.0...v1.1.1
[1.1.0]: https://github.com/dug-21/neural-data-platform/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/dug-21/neural-data-platform/releases/tag/v1.0.0
