# dp-018: JSON Config Foundation - SPARC Specification

**Document Type**: SPARC Specification (Phase S)
**Feature**: dp-018 JSON Config Foundation
**Version**: 1.0
**Date**: 2026-02-01
**Parent**: dp-016 Configuration Architecture Review

---

## 1. Executive Summary

This specification defines the requirements for implementing Phases 0 and 1 of the dp-016 Configuration Architecture roadmap. The goal is to establish JSON as the platform configuration standard and fix the critical Silver ETL config loading issue that causes silent failures.

### Key Outcomes

1. All stream configurations migrated from YAML to JSON format (v1.1 schema)
2. Silver ETL subscriber loads config from etcd (same source as Bronze)
3. Unified `ConfigLoader` trait for consistent config access
4. Fields enriched with descriptions (preparation for entity_schemas elimination)
5. Silent failures converted to visible errors

---

## 2. Requirements Analysis

### 2.1 Functional Requirements

#### Phase 0: JSON Migration

| ID | Requirement | Priority | Acceptance Criteria | Traces To |
|----|-------------|----------|---------------------|-----------|
| **FR-001** | Create JSON Schema for stream configs (v1.1) | HIGH | Schema validates both current structure (entity_schemas) and enriched fields (description in fields). Schema stored at `schemas/stream-config.v1.schema.json` | Task 0.1 |
| **FR-002** | Create dimension config schema | MEDIUM | Schema validates dimension configurations. Schema stored at `schemas/dimension-config.schema.json` | Task 0.2 |
| **FR-003** | Create manifest schema | MEDIUM | Schema validates deployment manifests. Schema stored at `schemas/manifest.schema.json` | Task 0.2 |
| **FR-004** | Build YAML-to-JSON migration script | HIGH | Shell script using yq/jq converts YAML to JSON. Script is idempotent (safe to run multiple times). Located at `scripts/migrate-yaml-to-json.sh` | Task 0.3 |
| **FR-005** | Migrate all stream configs to JSON | HIGH | All `config/base/streams/*/config.yaml` files converted to `config.json`. All configs validate against v1.1 schema | Task 0.4 |
| **FR-006** | Enrich fields with descriptions | HIGH | During migration, copy `description` and `device_class` from `entity_schemas` entries into corresponding `fields` entries | Task 0.5 |
| **FR-007** | Migrate dimension configs | MEDIUM | All dimension YAML files converted to JSON and validate against schema | Task 0.6 |
| **FR-008** | Update .gitignore | LOW | Old YAML config files removed after successful migration and verification | Task 0.7 |
| **FR-009** | Update documentation | LOW | README and docs reference JSON format. No stale YAML references in documentation | Task 0.8 |

#### Phase 1: Unified Config Loading

| ID | Requirement | Priority | Acceptance Criteria | Traces To |
|----|-------------|----------|---------------------|-----------|
| **FR-010** | Create ConfigLoader trait | HIGH | Trait defined in `neural-core` with `load_stream_config()` and `load_silver_etl_config()` methods. Trait is `Send + Sync` | Task 1.1 |
| **FR-011** | Implement EtcdConfigLoader | HIGH | JSON-native implementation using `serde_json`. Reads config from etcd, returns typed `StreamConfig` or `SilverEtlConfig`. Located in `core/src/config/etcd_loader.rs` | Task 1.2 |
| **FR-012** | Fix Silver subscriber config loading | CRITICAL | Silver subscriber in `air-quality-app` uses `EtcdConfigLoader` to read config from etcd. Removes direct YAML file dependency | Task 1.3, P-001 |
| **FR-013** | Ensure batch ETL uses same loader | HIGH | If batch ETL exists, it uses `EtcdConfigLoader` for consistent behavior with streaming | Task 1.4, P-004 |
| **FR-014** | Fix data dictionary sync | HIGH | Dictionary sync reads from etcd, not YAML files | Task 1.5, P-013 |
| **FR-015** | Update dictionary loader for enriched fields | MEDIUM | Dictionary loader reads `description` from `fields.description` with fallback to `entity_schemas`. Works with both v1.0 and v1.1 configs | Task 1.5a |
| **FR-016** | Add config source logging | MEDIUM | Every config load logs which source the config was loaded from (etcd key path). Format: `"config loaded from etcd: /streams/{stream_id}/config"` | Task 1.6 |
| **FR-017** | Promote sync errors to ERROR level | HIGH | Config sync failures logged as ERROR (not WARN). Failed streams are explicitly listed. Application behavior configurable via `--strict` mode | Task 1.7, P-017 |

### 2.2 Non-Functional Requirements

| ID | Category | Requirement | Measurement | Traces To |
|----|----------|-------------|-------------|-----------|
| **NFR-001** | Reliability | No silent failures for config loading | Zero unlogged config load failures. All failures have corresponding ERROR log entry | P-001, P-017, P-019 |
| **NFR-002** | Compatibility | Backward compatible migration | Existing configs work without modification during transition. No data loss during YAML-to-JSON conversion | SCOPE.md |
| **NFR-003** | Performance | Config loading latency | Config load from etcd completes in <100ms for typical configs | - |
| **NFR-004** | Maintainability | Single source of truth | All runtime components read config from etcd only. No YAML file reads in production code paths | ADR-016-001 |
| **NFR-005** | Observability | Config state visibility | Config load source logged. Failed streams enumerated at startup | P-019, P-021 |
| **NFR-006** | Portability | No Python dependency | Migration scripts use shell+yq+jq only. No Python required on target device (Pi) | Platform Constraints |
| **NFR-007** | Testability | Integration test coverage | All config loading paths have integration tests runnable with `DEPLOY_ENV=integration` | dp-017 |

---

## 3. Acceptance Criteria

### 3.1 Phase 0: JSON Migration - Definition of Done

```gherkin
Feature: JSON Migration (Phase 0)

  Scenario: JSON Schema validates both config patterns
    Given the v1.1 JSON Schema at schemas/stream-config.v1.schema.json
    When I validate a config with only entity_schemas
    Then validation succeeds
    When I validate a config with enriched fields (description in fields)
    Then validation succeeds
    When I validate a config with both patterns
    Then validation succeeds
    When I validate a config with unknown fields
    Then validation fails with clear error message

  Scenario: Migration script converts YAML to JSON
    Given a stream config at config/base/streams/air-quality/config.yaml
    When I run scripts/migrate-yaml-to-json.sh
    Then config/base/streams/air-quality/config.json exists
    And the JSON file validates against v1.1 schema
    And all YAML content is preserved in JSON
    And running the script again produces identical output (idempotent)

  Scenario: Fields are enriched during migration
    Given a YAML config with entity_schemas containing descriptions
    When I run the migration script
    Then the resulting JSON has description in fields entries
    And the description matches the entity_schemas value
    And entity_schemas section is preserved (deprecated but present)

  Scenario: All streams migrated successfully
    Given the migration script has been run
    When I list config/base/streams/*/config.json
    Then every stream directory has a config.json file
    And every config.json validates against the schema
    And no config.yaml files remain (or are in .gitignore)
```

### 3.2 Phase 1: Unified Config Loading - Definition of Done

```gherkin
Feature: Unified Config Loading (Phase 1)

  Scenario: Silver subscriber loads config from etcd
    Given etcd contains stream config at /streams/air-quality/config
    And the config has silver_etl.enabled = true
    When air-quality-app starts
    Then SilverSubscriber is created for air-quality stream
    And log contains "config loaded from etcd: /streams/air-quality/config"
    And no YAML file is read during startup

  Scenario: Missing etcd config fails loudly
    Given etcd does not contain /streams/missing-stream/config
    When air-quality-app tries to load config for missing-stream
    Then ERROR is logged with message containing "config not found"
    And the stream is listed as failed at startup
    And application continues (does not crash)

  Scenario: Config sync failure is ERROR not WARN
    Given a stream config with validation error
    When ConfigSyncService.sync_all() runs
    Then ERROR is logged (not WARN)
    And the failed stream is explicitly named in the error
    And other valid streams continue to sync

  Scenario: Dictionary loader uses enriched fields with fallback
    Given a v1.1 config with description in fields
    When dictionary sync loads the config
    Then description is read from fields.description

    Given a v1.0 config with description only in entity_schemas
    When dictionary sync loads the config
    Then description is read from entity_schemas (fallback)
    And a deprecation warning is logged

  Scenario: ConfigLoader trait provides unified interface
    Given EtcdConfigLoader is initialized with etcd endpoints
    When I call load_stream_config("air-quality")
    Then I receive a typed StreamConfig
    And the config includes silver_etl section if present

  Scenario: Config source is always logged
    Given any component loading config from etcd
    When config is successfully loaded
    Then log contains source information: "config loaded from etcd: {key}"
    When config load fails
    Then log contains ERROR with source and reason
```

---

## 4. Constraints

### 4.1 Technical Constraints

| Constraint | Impact | Mitigation |
|------------|--------|------------|
| **No Python on Pi** | Migration scripts cannot use Python | Use shell+yq+jq for migration. Scripts run on dev machine, not Pi. Resulting JSON files are committed to git |
| **Rust-first platform** | New tooling should be Rust | ConfigLoader trait and EtcdConfigLoader implemented in Rust. Leverages existing `config-client` crate |
| **Existing config-client crate** | Must integrate with existing infrastructure | EtcdConfigLoader wraps/extends existing `ConfigClient` and `StreamRegistry`. Does not replace them |
| **etcd as runtime store** | etcd is the single runtime config source | All runtime config reads go through etcd. YAML files are source-of-record for version control only |

### 4.2 Architectural Constraints

| Constraint | Description | Source |
|------------|-------------|--------|
| **Silver ETL is a subscriber** | Silver ETL is NOT a separate daemon. It is a subscriber component inside `air-quality-app` that subscribes to the event bus | SCOPE.md clarification |
| **Legacy silver-etl daemon deprecated** | The `apps/silver-etl/` component is obsolete. Do not modify or use it | dp-017 |
| **config-client is the foundation** | The `config-client` crate already provides `StreamRegistry` with etcd integration. New code extends this | Existing codebase |
| **ConfigSyncService pushes to etcd** | On startup, `ConfigSyncService` syncs YAML configs to etcd. This is the "push" direction. ConfigLoader reads from etcd (the "pull" direction) | Existing codebase |

### 4.3 Business Constraints

| Constraint | Description |
|------------|-------------|
| **dp-017 is prerequisite** | Integration environment must be complete before testing dp-018 changes |
| **Backward compatibility required** | v1.1 schema must accept v1.0 configs. No breaking changes during migration |
| **No data loss** | Migration must preserve all configuration data. Validation before committing |

---

## 5. Stakeholder Concerns

### 5.1 Backward Compatibility

**Concern**: Existing deployments must continue working during the transition period.

**Mitigation**:
1. v1.1 schema accepts both `entity_schemas` (v1.0 pattern) and enriched fields
2. Dictionary loader implements fallback: reads `fields.description` first, falls back to `entity_schemas`
3. Migration script runs on dev machine; Pi receives already-migrated JSON files via git pull
4. `ConfigSyncService` continues to push configs to etcd; change is in how Silver subscriber reads them

**Verification**:
- All existing streams work after migration without config changes
- Integration tests cover both v1.0 and v1.1 config patterns

### 5.2 No Data Loss During Migration

**Concern**: YAML-to-JSON conversion must not lose or corrupt data.

**Mitigation**:
1. Migration script uses `yq -o=json` for reliable conversion
2. Enrichment step (copying descriptions) is additive, not destructive
3. Original entity_schemas preserved in v1.1 (deprecated but present)
4. Validation step compares field counts before/after migration
5. Git diff review before committing migrated files

**Verification**:
- Automated test compares YAML and JSON content
- Schema validation catches structural errors
- Manual review of first few migrated streams

### 5.3 Silent Failures Must Become Visible Errors

**Concern**: The root cause of air-012 was silent failures in config loading.

**Mitigation**:
1. All config sync failures logged as ERROR (not WARN) - FR-017
2. Failed streams explicitly listed at startup - FR-017
3. Every config load logs its source - FR-016
4. SilverSubscriber creation explicitly logged (or reason for not creating) - addresses P-019
5. Optional `--strict` mode fails startup on any config error

**Verification**:
- Integration test simulates config error, verifies ERROR log
- Integration test verifies "SilverSubscriber NOT created for stream X because Y" message
- Log review during deployment

### 5.4 Operational Simplicity

**Concern**: Operators need a clear, simple deployment process.

**Current State** (addressed by dp-018):
- Bronze reads from etcd, Silver reads from YAML files (inconsistent)
- Failures are silent, discovered only when data is missing

**After dp-018**:
- Both Bronze and Silver read from etcd (consistent)
- Failures are logged as ERROR with clear messages

**Not in dp-018 Scope** (future phases):
- Single-command deploy (dp-016 Phase 3)
- Hot-reload (dp-016 Phase 4)
- Automated validation at deploy time (dp-019)

---

## 6. Data Model Specification

### 6.1 v1.1 Stream Config Schema

The v1.1 schema is a **non-breaking extension** of v1.0:

```yaml
entities:
  StreamConfig:
    attributes:
      - config_version: number (1 or 1.1)
      - stream_id: string (required, kebab-case)
      - description: string (required)
      - version: string (semver, default "1.0.0")
      - enabled: boolean (default true)
      - retention_days: integer (default 0)
      - compression_after_days: integer (default 0)
      - partitioning_strategy: string (default "daily")
      - fields: array<SchemaField> (required, min 1)
      - sources: array<SourceConfig> (required, min 1)
      - storage: StorageConfig (optional)
      - entity_schemas: array<EntitySchema> (DEPRECATED in v1.1, optional)
      - silver_etl: SilverEtlConfig (optional)

  SchemaField:
    attributes:
      - name: string (required, snake_case)
      - type: enum [float, int, string, bool, json] (required)
      - unit: string (optional)
      - description: string (optional, NEW in v1.1)
      - device_class: string (optional, NEW in v1.1)
      - range: array<number> (optional, exactly 2 elements)
      - display_precision: integer (optional)
      - nullable: boolean (default true)
      - default: any (optional)

  EntitySchema:
    status: DEPRECATED in v1.1
    attributes:
      - name: string (required)
      - description: string (optional)
      - device_class: string (optional)
      - unit: string (optional)

  SilverEtlConfig:
    attributes:
      - enabled: boolean (required)
      - target_table: string (required, format "silver.{table_name}")
      - field_mappings: array<FieldMapping> (required)
      - dq_rules: array<DqRule> (optional)

  FieldMapping:
    attributes:
      - target_column: string (required)
      - source_path: string (required, format "raw_payload.{field}")
      - target_type: string (required)
      - transform: string (optional)
```

### 6.2 Schema Version Transitions

| Version | State | entity_schemas | Enriched fields | Enforced By |
|---------|-------|----------------|-----------------|-------------|
| v1.0 | Legacy (YAML) | Required | Not supported | N/A (pre-schema) |
| **v1.1** | **Current (dp-018)** | Deprecated (optional) | Supported | JSON Schema |
| v2.0 | Future (dp-016 Phase 5) | Forbidden | Required | JSON Schema + Validator |

---

## 7. Interface Specification

### 7.1 ConfigLoader Trait

```rust
/// Unified trait for configuration loading
///
/// All components needing config should use this trait, not direct etcd access.
/// This enables testing with mock implementations and ensures consistent behavior.
pub trait ConfigLoader: Send + Sync {
    /// Load complete stream configuration
    ///
    /// Returns the full StreamConfig including silver_etl if present.
    ///
    /// # Errors
    /// - `ConfigError::NotFound` if stream does not exist in etcd
    /// - `ConfigError::InvalidConfig` if JSON is malformed or fails validation
    async fn load_stream_config(&self, stream_id: &str) -> Result<StreamConfig, ConfigError>;

    /// Load Silver ETL configuration for a stream
    ///
    /// Convenience method that extracts silver_etl from StreamConfig.
    /// Returns None if stream exists but has no silver_etl section.
    ///
    /// # Errors
    /// - `ConfigError::NotFound` if stream does not exist in etcd
    async fn load_silver_etl_config(&self, stream_id: &str) -> Result<Option<SilverEtlConfig>, ConfigError>;

    /// List all stream IDs available in config store
    async fn list_streams(&self) -> Result<Vec<String>, ConfigError>;
}
```

### 7.2 EtcdConfigLoader Implementation

```rust
/// ConfigLoader implementation backed by etcd
///
/// Reads JSON configuration from etcd keys under /streams/{stream_id}/config.
/// Logs source of every config load for observability.
pub struct EtcdConfigLoader {
    registry: StreamRegistry,
}

impl EtcdConfigLoader {
    /// Create a new EtcdConfigLoader connected to etcd
    ///
    /// # Arguments
    /// * `endpoints` - etcd endpoint URLs (e.g., ["http://localhost:2379"])
    pub async fn new(endpoints: &[&str]) -> Result<Self, ConfigError>;
}
```

### 7.3 Migration Script Interface

```bash
# Usage: scripts/migrate-yaml-to-json.sh [options]
#
# Options:
#   --dry-run       Show what would be converted without writing files
#   --stream ID     Convert only the specified stream
#   --validate      Validate output against JSON schema
#   --verbose       Show detailed progress
#
# Examples:
#   ./scripts/migrate-yaml-to-json.sh                    # Migrate all streams
#   ./scripts/migrate-yaml-to-json.sh --dry-run          # Preview changes
#   ./scripts/migrate-yaml-to-json.sh --stream air-quality  # Migrate one stream
```

---

## 8. Validation Checklist

Before completing Phase 0:

- [ ] All stream configs converted to JSON
- [ ] All JSON configs validate against v1.1 schema
- [ ] Fields contain description (copied from entity_schemas)
- [ ] entity_schemas section preserved but marked deprecated
- [ ] Migration script is idempotent
- [ ] No data loss between YAML and JSON
- [ ] Documentation updated to reference JSON

Before completing Phase 1:

- [ ] ConfigLoader trait defined in neural-core
- [ ] EtcdConfigLoader implemented and tested
- [ ] Silver subscriber uses EtcdConfigLoader
- [ ] Dictionary sync uses EtcdConfigLoader with fallback
- [ ] All config loads logged with source
- [ ] Sync failures logged as ERROR (not WARN)
- [ ] Integration tests pass with DEPLOY_ENV=integration

---

## 9. Glossary

| Term | Definition |
|------|------------|
| **ConfigLoader** | Rust trait defining the interface for loading stream configurations. Implementations can read from etcd, files, or mock sources. Defined in `core/src/config/loader.rs` |
| **EtcdConfigLoader** | Implementation of ConfigLoader that reads JSON configs from etcd. Uses the existing `config-client` crate infrastructure |
| **StreamRegistry** | Existing class in `config-client` crate that manages stream configs in etcd. EtcdConfigLoader wraps/extends this |
| **ConfigSyncService** | Existing service in `air-quality-app` that syncs YAML configs to etcd on startup. Remains unchanged; pushes configs to etcd |
| **v1.0 schema** | Legacy schema (YAML). entity_schemas required, enriched fields not supported. Pre-migration state |
| **v1.1 schema** | Transitional schema (JSON). entity_schemas deprecated but accepted, enriched fields supported. dp-018 target state |
| **v2.0 schema** | Future schema (dp-016 Phase 5). entity_schemas forbidden, enriched fields required. Breaking change with migration tool |
| **entity_schemas** | Legacy array in config defining field metadata (description, device_class). DEPRECATED in v1.1. Data should be in fields instead |
| **Enriched fields** | v1.1 pattern where `description` and `device_class` are properties of each field in the `fields` array, not in a separate `entity_schemas` section |
| **Silver subscriber** | Component in `air-quality-app` that subscribes to the event bus and writes to Silver tables. NOT a separate daemon |
| **Bronze subscriber** | Component that writes raw data to Parquet files in the Bronze layer |
| **config-client** | Existing Rust crate providing `ConfigClient` and `StreamRegistry` for etcd access |
| **SilverEtlConfig** | Configuration for Silver ETL including target table, field mappings, and DQ rules. Part of StreamConfig |
| **DQ rules** | Data Quality rules defining validation checks (range, enum, not_null, etc.) applied during Silver ETL |

---

## 10. Dependencies and Prerequisites

| Dependency | Type | Status | Notes |
|------------|------|--------|-------|
| dp-017: Integration Environment | REQUIRED | Must complete first | Needed for testing dp-018 changes |
| dp-016: Architecture Review | Complete | ADR-016-001, ADR-016-002 define the target architecture |
| config-client crate | Existing | Available | Foundation for EtcdConfigLoader |
| yq (YAML processor) | Dev tool | Install required | Used by migration script on dev machine |
| jq (JSON processor) | Dev tool | Install required | Used by migration script on dev machine |

---

## 11. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Migration corrupts config data | Low | High | Validation at each step, git diff review, idempotent script |
| Silver subscriber breaks on deploy | Medium | High | Integration tests, phased rollout, rollback plan |
| Existing code breaks with JSON | Low | Medium | v1.1 schema is backward compatible; all existing patterns accepted |
| etcd connection failures | Low | High | Existing error handling in config-client; enhanced logging in dp-018 |
| Dictionary sync fails with new loader | Medium | Medium | Fallback logic for v1.0 configs; deprecation warnings |

---

## 12. Success Metrics

| Metric | Current State | After dp-018 | Measurement |
|--------|---------------|--------------|-------------|
| Config format | YAML (v1.0) | JSON (v1.1) | File extension in config/base/streams/ |
| Silver config source | YAML files | etcd | Grep logs for "config loaded from" |
| Silent config failures | Common (P-001) | Zero | Grep logs for ERROR level config messages |
| Config sync failure level | WARN | ERROR | Log level in ConfigSyncService |
| Field metadata locations | 2 (fields + entity_schemas) | 2 (transitional) | Config structure (1 location in v2.0) |

---

## 13. References

| Document | Path | Relevance |
|----------|------|-----------|
| dp-018 SCOPE.md | `product/features/dp-018/SCOPE.md` | Feature scope definition |
| dp-016 IMPLEMENTATION-ROADMAP.md | `product/features/dp-016/IMPLEMENTATION-ROADMAP.md` | Detailed task breakdown |
| dp-016 PAIN-POINTS.md | `product/features/dp-016/specification/PAIN-POINTS.md` | Problem catalog |
| ADR-016-001 | `product/features/dp-016/architecture/ADR-016-001-config-source-of-truth.md` | Architecture decision |
| air-013 SCOPE.md | `product/features/air-013/SCOPE.md` | Absorbed feature |
| config-client crate | `config-client/` | Existing infrastructure |
| StreamConfig | `core/src/types/stream_config.rs` | Current type definitions |

---

*Specification created: 2026-02-01*
*SPARC Phase: Specification (S)*
*Next Phase: Pseudocode (P)*
