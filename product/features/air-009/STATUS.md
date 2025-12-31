# AIR-009: Source Identity and Context Configuration

## Current Phase
✅ **IMPLEMENTATION COMPLETE** - Ready for integration testing

## Progress
- [x] SCOPE.md created
- [x] SPARC Specification complete (4 documents)
- [x] SPARC Pseudocode complete (4 documents)
- [x] SPARC Architecture complete (4 documents)
- [x] SPARC Refinement complete (8 documents)
- [x] SPARC Completion (implementation)
- [x] All tests passing (529 tests: 344 platform-core + 118 air-quality-app + 67 air-quality)
- [x] Documentation updated
- [ ] Deployed to production

## Implementation Summary
The AIR-009 feature has been fully implemented using London TDD methodology with 8 parallel agents completing 11 TDD cycles.

### Completion Deliverables (Code Changes)

#### Core Types (platform-core)
- `core/src/types/stream_config.rs` - Added `ndp_id: Option<String>` and `context: Option<serde_json::Value>` to SourceConfig
- `core/src/traits.rs` - Added `ndp_id` and `context` fields to TimeSeriesPoint struct
- `core/src/parsers/traits.rs` - Added ParseContext struct for context injection
- `core/src/parsers/flat_json.rs` - Implemented `parse_with_context()` method
- `core/src/parsers/array_iterator.rs` - Implemented `parse_with_context()` method
- `core/src/parsers/column_oriented.rs` - Implemented `parse_with_context()` method
- `core/src/storage/parquet.rs` - Updated TimeSeriesPoint instantiations

#### Application Layer (air-quality-app)
- `apps/air-quality-app/src/config_sync/service.rs` - Added SourceYaml ndp_id/context parsing with proper field filtering
- `apps/air-quality-app/src/coordinator/source_manager.rs` - Updated all SourceConfig instantiations
- `apps/air-quality-app/src/coordinator/router.rs` - Updated all SourceConfig and TimeSeriesPoint usage
- `apps/air-quality-app/src/pipeline/storage_writer.rs` - Updated TimeSeriesPoint instantiations
- `apps/air-quality-app/src/api/handlers/readings.rs` - Updated mock TimeSeriesPoint usage
- `apps/air-quality-app/src/api/routes.rs` - Updated mock TimeSeriesPoint usage

#### Stream Configurations (7 files)
- `config/base/streams/air-quality/config.yaml` - Added ndp_id and context examples
- `config/base/streams/outdoor-weather/config.yaml`
- `config/base/streams/home-assistant/config.yaml`
- `config/base/streams/demo-sensors/config.yaml`
- `config/base/streams/weather-forecast/config.yaml`
- `config/base/streams/air-pollution/config.yaml`
- `config/base/streams/solar-radiation/config.yaml`

#### Database Migrations
- `deploy/migrations/V002__add_ndp_id_and_context_columns.sql` - Silver layer schema migration
- `deploy/migrations/V002__add_ndp_id_and_context_columns_rollback.sql` - Rollback script

### Specification Deliverables
- `specification/REQUIREMENTS.md` - 16 functional + 7 non-functional requirements
- `specification/ACCEPTANCE_CRITERIA.md` - Given/When/Then criteria
- `specification/USER_STORIES.md` - 16 user stories by epic
- `specification/GLOSSARY.md` - 40+ defined terms

### Pseudocode Deliverables
- `pseudocode/CONTEXT_FLATTENER.md` - Recursive flattening algorithm
- `pseudocode/CONFIG_PARSER.md` - YAML/etcd parsing with validation
- `pseudocode/RECORD_ENRICHER.md` - Attach context to parsed records
- `pseudocode/ETCD_KEY_GENERATOR.md` - Key path generation

### Architecture Deliverables
- `architecture/SYSTEM_DESIGN.md` - Overall system design with data flow diagrams
- `architecture/ADR-001-ndp-id-design.md` - Stable identifier design decision
- `architecture/ADR-002-context-flattening.md` - Context flattening approach
- `architecture/ADR-003-silver-layer-schema.md` - TimescaleDB JSONB schema decision

### Refinement Deliverables (London TDD)
- `refinement/TEST_STRATEGY.md` - London School TDD with test pyramid
- `refinement/TEST_CASES.md` - 50+ detailed test cases
- `refinement/MOCK_DEFINITIONS.md` - MockEtcdClient, MockParquetWriter, etc.
- `refinement/TDD_IMPLEMENTATION_ORDER.md` - 5-phase Red-Green-Refactor sequence
- `refinement/IMPLEMENTATION_PHASES.md` - 7 phases with LOC estimates
- `refinement/CODE_CHANGES.md` - File-by-file changes specification
- `refinement/MIGRATION_PLAN.md` - Backward compatibility strategy
- `refinement/INTEGRATION_CHECKLIST.md` - Pre/post deployment verification

Feature depends on DP-003 (MQTT Multi-Subscription Support) which is implementation complete and ready for integration testing.

## Scope Summary

This feature implements `ndp_id` (stable source identity) and `context` (mutable attributes) configuration across the NDP stack:

1. **Stream Configuration Updates** - Add ndp_id and context to all 6 active streams
2. **Configuration Sync (etcd)** - Ensure new fields sync properly
3. **Ingestion Pipeline Modifications** - Write ndp_id and flattened context with every record
4. **Data Dictionary (TimescaleDB Silver Layer)** - Add ndp_id column and context field schema

## Dependencies
| Dependency | Status | Notes |
|------------|--------|-------|
| DP-003: MQTT Multi-Subscription | Implementation Complete | Ready for integration testing |
| Existing stream configurations | Available | 6 streams in config/base/streams/ |
| ConfigSyncService | Operational | Needs verification for nested context |
| Bronze/Silver layer infrastructure | Operational | TimescaleDB added in DP-002 |

## Bugs
| ID | Status | Summary |
|----|--------|---------|

## Branch
`feature/air-009` (to be created)

## Last Updated
2025-12-31 (Implementation complete - 529 tests passing)

## Future Features (Out of Scope)
- Silver layer views for Grafana consumption
- Context field extraction helpers
- Continuous aggregates with context dimensions
