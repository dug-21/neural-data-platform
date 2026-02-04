# Changelog

All notable changes to the Neural Data Platform will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/dug-21/neural-data-platform/compare/v1.1.1...HEAD
[1.1.1]: https://github.com/dug-21/neural-data-platform/compare/v1.1.0...v1.1.1
[1.1.0]: https://github.com/dug-21/neural-data-platform/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/dug-21/neural-data-platform/releases/tag/v1.0.0
