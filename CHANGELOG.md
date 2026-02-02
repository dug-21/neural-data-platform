# Changelog

All notable changes to the Neural Data Platform will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/dug-21/neural-data-platform/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/dug-21/neural-data-platform/releases/tag/v1.0.0
