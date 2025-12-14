# Changelog - Air Quality Feature (air-001)

All notable changes to the air-001 feature will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial GitHub workflow integration for air-001 feature
- CI/CD pipeline with Rust build, test, and lint checks
- Multi-architecture build preparation (amd64/arm64)
- Feature branch: `feature/air-001-implementation`
- Commit message template with conventional commits format
- Pre-commit hooks documentation

### Changed
- N/A

### Deprecated
- N/A

### Removed
- N/A

### Fixed
- N/A

### Security
- Added cargo-audit security scanning in CI/CD pipeline

---

## [0.1.0] - 2025-12-13

### Added
- SPARC methodology documentation for air-001 feature
- Specification document (01-specification.md) with complete AirGradient integration
- Architecture document (01-system-design.md) for Docker-first deployment
- Implementation roadmap (01-roadmap.md) with 5-phase plan
- Code review findings and corrections for MQTT integration

### Documentation
- Created comprehensive specification for AirGradient ONE sensor integration
- Documented 29+ field data model from live sensor testing
- MQTT topic pattern validation and correction
- Field availability matrix (MQTT vs Local API)

---

## Version History Legend

- **[Unreleased]**: Changes in development, not yet released
- **[0.1.0]**: Initial SPARC planning phase completion

## Scope Tags

When committing changes, use these scope tags in commit messages:
- `core`: Changes to core traits and types
- `domain`: Air quality domain-specific implementations
- `storage`: Parquet storage and data pipeline
- `ingestion`: MQTT and HTTP data sources
- `intelligence`: AQI, alerts, and forecasting
- `api`: REST API and MCP server
- `docker`: Container and deployment configuration
- `ci`: CI/CD pipeline and workflows
- `docs`: Documentation updates
- `tests`: Test suite additions or changes

## Example Commit Message

```
feat(ingestion): add MQTT source with reconnection logic

Implement MqttSource trait with automatic reconnection,
backpressure handling, and message parsing integration.

- Add rumqttc client configuration
- Implement exponential backoff on disconnect
- Add integration tests with testcontainers

Refs: #air-001, Phase 2.1
```

---

Last Updated: 2025-12-13
