# dp-018: JSON Config Foundation - SPARC Specification

**Document Type**: SPARC Specification (Phase S)
**Feature**: dp-018 JSON Config Foundation
**Version**: 1.1
**Date**: 2026-02-01
**Parent**: dp-016 Configuration Architecture Review
**Architecture**: ADR-018-001 JSON Pass-Through Architecture

---

## 1. Executive Summary

This specification defines the requirements for implementing Phases 0 and 1 of the dp-016 Configuration Architecture roadmap. The goal is to establish JSON as the platform configuration standard and fix the critical Silver ETL config loading issue that causes silent failures.

### Key Outcomes

1. All stream configurations migrated from YAML to JSON format (v1.1 schema)
2. Silver ETL subscriber loads config from etcd (same source as Bronze)
3. **JSON pass-through architecture** - no transformation between file and etcd
4. **Eliminate lossy transformation** - delete StreamConfigYaml and to_stream_config()
5. Fields enriched with descriptions (preparation for entity_schemas elimination)
6. Silent failures converted to visible errors

### Core Architecture Principle

**JSON file = etcd blob = runtime config**

The fundamental change is eliminating the lossy transformation pipeline:
- **BEFORE**: YAML -> StreamConfigYaml -> to_stream_config() -> StreamConfig -> etcd (LOSSY)
- **AFTER**: JSON -> validate -> StreamConfig -> etcd (PASS-THROUGH)

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

#### Phase 1: JSON Pass-Through Architecture

| ID | Requirement | Priority | Acceptance Criteria | Traces To |
|----|-------------|----------|---------------------|-----------|
| **FR-010** | Extend StreamConfig with silver_etl field | HIGH | Add `silver_etl: Option<SilverEtlConfig>` to StreamConfig struct in `core/src/types/stream_config.rs`. Field uses `#[serde(skip_serializing_if = "Option::is_none")]` | Task 1.1, ADR-018-001 |
| **FR-011** | Delete StreamConfigYaml struct | HIGH | Remove `StreamConfigYaml` from codebase. This struct with `#[serde(flatten)] extra: HashMap` caused lossy transformation | Task 1.2, ADR-018-001 |
| **FR-012** | Delete to_stream_config() function | HIGH | Remove the lossy transformation function. JSON deserializes directly to StreamConfig | Task 1.2, ADR-018-001 |
| **FR-013** | Simplify ConfigSyncService | CRITICAL | ConfigSyncService reads JSON, validates against schema, deserializes to StreamConfig, saves to etcd. No transformation step. Pass-through only | Task 1.3, ADR-018-001 |
| **FR-014** | Delete load_silver_etl_config() function | HIGH | Remove separate Silver config loader. Silver uses `registry.load_stream().silver_etl` like Bronze uses `registry.load_stream().sources` | Task 1.4, ADR-018-001 |
| **FR-015** | Fix Silver subscriber config loading | CRITICAL | Silver subscriber uses `StreamRegistry.load_stream()` to get config from etcd, then accesses `config.silver_etl`. Same pattern as Bronze | Task 1.5, P-001 |
| **FR-016** | Update dictionary loader for enriched fields | MEDIUM | Dictionary loader reads `description` from `fields.description` with fallback to `entity_schemas`. Works with both v1.0 and v1.1 configs | Task 1.6 |
| **FR-017** | Add config source logging | MEDIUM | Every config load logs which source the config was loaded from (etcd key path). Format: `"config loaded from etcd: /streams/{stream_id}/config"` | Task 1.7 |
| **FR-018** | Promote sync errors to ERROR level | HIGH | Config sync failures logged as ERROR (not WARN). Failed streams are explicitly listed. Application behavior configurable via `--strict` mode | Task 1.8, P-017 |

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

### 3.2 Phase 1: JSON Pass-Through Architecture - Definition of Done

```gherkin
Feature: JSON Pass-Through Architecture (Phase 1)

  Scenario: JSON file equals etcd blob (no transformation)
    Given a JSON config file at config/base/streams/air-quality/config.json
    When ConfigSyncService syncs the stream to etcd
    Then the etcd value is the same JSON (no transformation)
    And StreamConfig deserializes directly from the JSON
    And no StreamConfigYaml intermediate struct is used

  Scenario: StreamConfig includes silver_etl
    Given a JSON config with silver_etl section
    When I deserialize to StreamConfig
    Then config.silver_etl is Some(SilverEtlConfig)
    And all silver_etl fields are preserved (no data loss)

  Scenario: Silver subscriber uses StreamRegistry (same as Bronze)
    Given etcd contains stream config at /streams/air-quality/config
    And the config has silver_etl.enabled = true
    When air-quality-app starts
    Then SilverSubscriber calls registry.load_stream("air-quality")
    And SilverSubscriber accesses config.silver_etl
    And log contains "config loaded from etcd: /streams/air-quality/config"
    And no YAML file is read during startup
    And no load_silver_etl_config() function is called

  Scenario: Missing etcd config fails loudly
    Given etcd does not contain /streams/missing-stream/config
    When air-quality-app tries to load config for missing-stream
    Then ERROR is logged with message containing "config not found"
    And the stream is listed as failed at startup
    And application continues (does not crash)

  Scenario: ConfigSyncService does pass-through (no transformation)
    Given a JSON config file
    When ConfigSyncService.sync_stream() runs
    Then it reads JSON from file
    And validates against JSON Schema
    And deserializes directly to StreamConfig (not StreamConfigYaml)
    And saves StreamConfig to etcd
    And no to_stream_config() transformation occurs

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

  Scenario: Both Bronze and Silver use StreamRegistry.load_stream()
    Given StreamRegistry is connected to etcd
    When Bronze needs config for air-quality
    Then it calls registry.load_stream("air-quality")
    And accesses config.sources

    When Silver needs config for air-quality
    Then it calls registry.load_stream("air-quality")
    And accesses config.silver_etl
    And both use the SAME StreamConfig struct

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
| **Rust-first platform** | New tooling should be Rust | StreamConfig extension and ConfigSyncService simplification in Rust. Leverages existing `config-client` crate |
| **Existing config-client crate** | Must integrate with existing infrastructure | StreamRegistry already provides load_stream(). No new loader classes needed - just extend StreamConfig with silver_etl field |
| **etcd as runtime store** | etcd is the single runtime config source | All runtime config reads go through etcd via StreamRegistry. JSON files are source-of-record for version control only |
| **Pass-through architecture** | No transformation between JSON file and etcd | ConfigSyncService validates then passes through. StreamConfig is the single struct for file, etcd, and runtime |

### 4.2 Architectural Constraints

| Constraint | Description | Source |
|------------|-------------|--------|
| **Silver ETL is a subscriber** | Silver ETL is NOT a separate daemon. It is a subscriber component inside `air-quality-app` that subscribes to the event bus | SCOPE.md clarification |
| **Legacy silver-etl daemon deprecated** | The `apps/silver-etl/` component is obsolete. Do not modify or use it | dp-017 |
| **config-client is the foundation** | The `config-client` crate already provides `StreamRegistry` with etcd integration. StreamRegistry.load_stream() is the unified access pattern for both Bronze and Silver | Existing codebase |
| **ConfigSyncService does pass-through** | On startup, `ConfigSyncService` syncs JSON configs to etcd with NO transformation. JSON file content = etcd blob | ADR-018-001 |
| **One struct: StreamConfig** | Eliminate StreamConfigYaml. StreamConfig is the single struct used for JSON files, etcd storage, and runtime. Add silver_etl field to StreamConfig | ADR-018-001 |
| **No new traits or loaders** | Do NOT create ConfigLoader trait, EtcdConfigLoader, or SilverRegistry. Use existing StreamRegistry.load_stream() | ADR-018-001 |

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

### 7.1 Extended StreamConfig (core/src/types/stream_config.rs)

```rust
/// Unified stream configuration struct
///
/// This is the SINGLE struct used everywhere:
/// - JSON config files (source of truth)
/// - etcd storage (pass-through, no transformation)
/// - Runtime access by Bronze and Silver components
///
/// Adding silver_etl here eliminates the lossy transformation that
/// occurred in the old StreamConfigYaml.to_stream_config() pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamConfig {
    pub stream_id: String,
    pub description: String,
    pub version: String,
    pub enabled: bool,
    pub retention_days: u32,
    pub compression_after_days: u32,
    pub partitioning_strategy: String,
    pub fields: Vec<SchemaField>,
    pub sources: Vec<SourceConfig>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage: Option<StorageConfig>,

    /// Silver ETL configuration - now part of unified config
    /// Bronze ignores this; Silver uses it for ETL configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub silver_etl: Option<SilverEtlConfig>,

    /// Entity schemas (deprecated in v1.1, removed in v2.0)
    /// Use enriched fields instead
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_schemas: Option<Vec<EntitySchema>>,
}
```

### 7.2 Simplified ConfigSyncService (apps/air-quality-app/src/config_sync/service.rs)

```rust
/// Pass-through config sync - no transformation
///
/// BEFORE (lossy):
///   YAML -> StreamConfigYaml -> to_stream_config() -> StreamConfig -> etcd
///
/// AFTER (pass-through):
///   JSON -> validate -> StreamConfig -> etcd
impl ConfigSyncService {
    pub fn sync_stream(&self, json_path: &Path) -> Result<()> {
        let json = fs::read_to_string(json_path)?;

        // Validate against schema (catches errors early)
        validate_json_schema(&json, &self.schema)?;

        // Deserialize directly to StreamConfig (same struct everywhere)
        let config: StreamConfig = serde_json::from_str(&json)?;

        // Save to etcd (serializes same struct - no data loss)
        self.registry.save_stream(&config)?;

        info!("config synced to etcd: /streams/{}/config", config.stream_id);
        Ok(())
    }
}
```

### 7.3 Unified Config Access Pattern

```rust
// Bronze component (already correct)
let config = registry.load_stream("air-quality").await?;
let sources = &config.sources;  // Access Bronze-specific fields

// Silver component (now fixed - same pattern)
let config = registry.load_stream("air-quality").await?;
let silver_etl = config.silver_etl.as_ref()
    .ok_or_else(|| anyhow!("Stream {} has no silver_etl config", stream_id))?;
let target_table = &silver_etl.target_table;  // Access Silver-specific fields
```

### 7.4 Migration Script Interface

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

- [ ] StreamConfig extended with silver_etl field
- [ ] StreamConfigYaml struct deleted
- [ ] to_stream_config() function deleted
- [ ] load_silver_etl_config() function deleted
- [ ] ConfigSyncService simplified (pass-through, no transformation)
- [ ] Silver subscriber uses StreamRegistry.load_stream()
- [ ] JSON file = etcd blob (verified by test)
- [ ] Dictionary sync uses StreamRegistry with fallback for entity_schemas
- [ ] All config loads logged with source
- [ ] Sync failures logged as ERROR (not WARN)
- [ ] Integration tests pass with DEPLOY_ENV=integration

---

## 9. Glossary

| Term | Definition |
|------|------------|
| **Pass-through architecture** | Design principle where JSON config file content equals etcd blob equals runtime config. No transformation layer. See ADR-018-001 |
| **StreamConfig** | The SINGLE struct used everywhere: JSON files, etcd storage, and runtime. Extended with `silver_etl` field in dp-018. Located in `core/src/types/stream_config.rs` |
| **StreamConfigYaml** | DEPRECATED/DELETED. Former intermediate struct with `#[serde(flatten)] extra: HashMap` that caused lossy transformation. Eliminated in dp-018 |
| **to_stream_config()** | DEPRECATED/DELETED. Former transformation function that converted StreamConfigYaml to StreamConfig, losing silver_etl data. Eliminated in dp-018 |
| **load_silver_etl_config()** | DEPRECATED/DELETED. Former function for loading Silver config from YAML files. Replaced by `registry.load_stream().silver_etl` |
| **StreamRegistry** | Existing class in `config-client` crate that manages stream configs in etcd. The unified access point for both Bronze and Silver via `load_stream()` |
| **ConfigSyncService** | Service in `air-quality-app` that syncs JSON configs to etcd on startup. Simplified to pass-through (no transformation) in dp-018 |
| **v1.0 schema** | Legacy schema (YAML). entity_schemas required, enriched fields not supported. Pre-migration state |
| **v1.1 schema** | Transitional schema (JSON). entity_schemas deprecated but accepted, enriched fields supported. dp-018 target state |
| **v2.0 schema** | Future schema (dp-016 Phase 5). entity_schemas forbidden, enriched fields required. Breaking change with migration tool |
| **entity_schemas** | Legacy array in config defining field metadata (description, device_class). DEPRECATED in v1.1. Data should be in fields instead |
| **Enriched fields** | v1.1 pattern where `description` and `device_class` are properties of each field in the `fields` array, not in a separate `entity_schemas` section |
| **Silver subscriber** | Component in `air-quality-app` that subscribes to the event bus and writes to Silver tables. Uses `registry.load_stream().silver_etl` for config |
| **Bronze subscriber** | Component that writes raw data to Parquet files in the Bronze layer. Uses `registry.load_stream().sources` for config |
| **config-client** | Existing Rust crate providing `ConfigClient` and `StreamRegistry` for etcd access |
| **SilverEtlConfig** | Configuration for Silver ETL including target table, field mappings, and DQ rules. Now a field within StreamConfig (not separate) |
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
| Deleting StreamConfigYaml breaks callers | Low | Medium | Search for all usages before deletion; update all callers to use StreamConfig directly |
| to_stream_config() callers break | Low | Medium | Eliminate transformation; callers deserialize JSON directly to StreamConfig |

---

## 12. Success Metrics

| Metric | Current State | After dp-018 | Measurement |
|--------|---------------|--------------|-------------|
| Config format | YAML (v1.0) | JSON (v1.1) | File extension in config/base/streams/ |
| Config structs | 2 (StreamConfigYaml + StreamConfig) | 1 (StreamConfig only) | Grep codebase for struct definitions |
| Transformation functions | to_stream_config() exists | Deleted | Grep codebase for to_stream_config |
| Silver config source | YAML files | etcd via StreamRegistry | Grep logs for "config loaded from" |
| Bronze/Silver config pattern | Different (Bronze: etcd, Silver: YAML) | Same (both use StreamRegistry.load_stream()) | Code review |
| JSON = etcd blob | No (transformation) | Yes (pass-through) | Integration test comparison |
| Silent config failures | Common (P-001) | Zero | Grep logs for ERROR level config messages |
| Config sync failure level | WARN | ERROR | Log level in ConfigSyncService |
| Field metadata locations | 2 (fields + entity_schemas) | 2 (transitional) | Config structure (1 location in v2.0) |

---

## 13. References

| Document | Path | Relevance |
|----------|------|-----------|
| **ADR-018-001** | `product/features/dp-018/architecture/ADR-018-001-config-loader-design.md` | **JSON Pass-Through Architecture** - defines the core architectural change |
| dp-018 SCOPE.md | `product/features/dp-018/SCOPE.md` | Feature scope definition |
| dp-016 IMPLEMENTATION-ROADMAP.md | `product/features/dp-016/IMPLEMENTATION-ROADMAP.md` | Detailed task breakdown |
| dp-016 PAIN-POINTS.md | `product/features/dp-016/specification/PAIN-POINTS.md` | Problem catalog |
| ADR-016-001 | `product/features/dp-016/architecture/ADR-016-001-config-source-of-truth.md` | Architecture decision |
| air-013 SCOPE.md | `product/features/air-013/SCOPE.md` | Absorbed feature |
| config-client crate | `config-client/` | StreamRegistry - unified config access |
| StreamConfig | `core/src/types/stream_config.rs` | Single struct for all config (extended with silver_etl) |

---

*Specification created: 2026-02-01*
*Specification updated: 2026-02-01 (v1.1 - aligned with ADR-018-001 pass-through architecture)*
*SPARC Phase: Specification (S)*
*Next Phase: Pseudocode (P)*
