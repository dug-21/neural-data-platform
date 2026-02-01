# dp-018: JSON Config Foundation

## Parent Initiative

This feature implements **Phases 0 and 1** of [dp-016: Configuration Architecture Review](../dp-016/IMPLEMENTATION-ROADMAP.md).

**Absorbs**: air-013 (Unified Config Source for Silver ETL)

---

## Problem Statement

The NDP configuration system has critical issues causing silent failures:

1. **Dual source of truth** - YAML files vs etcd, components disagree on which to read
2. **Silver ETL silent failure** - Discovers streams from etcd, but loads config from YAML files
3. **No JSON standard** - YAML doesn't match MCP/agent workflows, no schema validation
4. **Scattered field metadata** - `fields` and `entity_schemas` store overlapping data

These issues were documented in dp-016's pain points analysis (P-001, P-017, P-019).

---

## Goals

1. **Establish JSON as platform configuration standard**
2. **Fix Silver ETL config loading** (reads from etcd like Bronze does)
3. **Create unified ConfigLoader trait** for consistent config access
4. **Prepare for entity_schemas elimination** (enrich fields during migration)

---

## Scope

### In Scope

**Phase 0: JSON Migration**

| ID | Task | Description | Acceptance Criteria |
|----|------|-------------|---------------------|
| 0.1 | Create JSON Schemas (v1.1) | `stream-config.schema.json` supporting current AND enriched fields | Schema accepts both patterns |
| 0.2 | Create supporting schemas | `dimension-config.schema.json`, `manifest.schema.json` | All config types have schemas |
| 0.3 | Build migration script | `scripts/migrate-yaml-to-json.sh` (shell+yq+jq) | Idempotent, preserves all data |
| 0.4 | Migrate stream configs | Convert `config/base/streams/*/config.yaml` → `config.json` | All streams have valid JSON |
| 0.5 | Enrich fields with descriptions | Copy `description` from entity_schemas into fields | Fields have description attribute |
| 0.6 | Migrate dimension configs | Convert dimension YAML files to JSON | Dimensions validate against schema |
| 0.7 | Update .gitignore | Remove old YAML files after migration | Clean repository state |
| 0.8 | Update documentation | Update README, docs to reference JSON | No stale YAML references |

**Phase 1: Unified Config Loading**

| ID | Task | Description | Acceptance Criteria |
|----|------|-------------|---------------------|
| 1.1 | Create ConfigLoader trait | Unified trait in `neural-core` with `load_stream_config()` | Single interface for all config loading |
| 1.2 | Implement EtcdConfigLoader | JSON-native implementation using serde_json | Reads JSON from etcd, returns typed config |
| 1.3 | Fix Silver streaming | Update `load_silver_etl_config()` to use EtcdConfigLoader | Silver streaming reads from etcd |
| 1.4 | Fix Silver batch | Ensure batch ETL uses same loader | Consistent behavior |
| 1.5 | Fix data dictionary sync | Update sync to read from etcd | Dictionary sync uses same source |
| 1.5a | Update dictionary loader | Read from `fields.description` with fallback to `entity_schemas` | Works with both patterns |
| 1.6 | Add config source logging | Log which source config was loaded from | Clear audit trail |
| 1.7 | Promote sync errors | Change WARN → ERROR for sync failures | Failures are visible |

### Out of Scope

- JSON Schema validation at deploy time (dp-019)
- Semantic validation (dp-019)
- Declarative deploy manifest (dp-020)
- Hot-reload (dp-021)
- MCP write tools (dp-021)

---

## Technical Context

### Schema Versioning Strategy

| Version | State | entity_schemas | Enriched fields |
|---------|-------|----------------|-----------------|
| v1.0 | Current (YAML) | Required | Not supported |
| **v1.1** | **This feature** | Deprecated (optional) | Supported |
| v2.0 | Future (dp-021) | Forbidden | Required |

### v1.1 Schema Structure

```json
{
  "config_version": 1.1,
  "stream_id": "air-quality",
  "fields": [
    {
      "name": "pm25",
      "type": "float",
      "unit": "µg/m³",
      "description": "Particulate matter 2.5µm",  // NEW in v1.1
      "device_class": "sensor"                     // NEW in v1.1
    }
  ],
  "entity_schemas": [...]  // DEPRECATED in v1.1, still accepted
}
```

### ConfigLoader Trait

```rust
pub trait ConfigLoader: Send + Sync {
    async fn load_stream_config(&self, stream_id: &str) -> Result<StreamConfig>;
    async fn load_silver_etl_config(&self, stream_id: &str) -> Result<SilverEtlConfig>;
}

pub struct EtcdConfigLoader {
    client: etcd_client::Client,
}
```

### Platform Constraints

| Constraint | Impact | Decision |
|------------|--------|----------|
| No Python on Pi | Migration scripts can't be Python | Shell+yq+jq (runs on dev machine) |
| Rust-first platform | Tooling should match platform | ConfigLoader in Rust |

---

## Deliverables

| Deliverable | Location | Description |
|-------------|----------|-------------|
| JSON Schemas | `schemas/` | stream-config.v1.schema.json, dimension-config.schema.json |
| Migration script | `scripts/migrate-yaml-to-json.sh` | YAML → JSON converter |
| Migrated configs | `config/base/streams/*/config.json` | All streams in JSON format |
| ConfigLoader trait | `core/src/config/loader.rs` | Unified config loading interface |
| EtcdConfigLoader | `core/src/config/etcd_loader.rs` | JSON-native etcd implementation |

---

## Success Criteria

1. **All configs in JSON format** with v1.1 schema
2. **Silver ETL starts correctly** when stream is configured in etcd
3. **No YAML file reads** in runtime code paths (Silver, Dictionary)
4. **Config source logged** on every config load
5. **Sync failures are ERROR level**, not WARN
6. **Fields contain descriptions** (copied from entity_schemas)

### Verification Commands

```bash
# Validate all configs against schemas
./scripts/validate-configs.sh

# Start app, verify Silver ETL starts
DEPLOY_ENV=integration ./deploy.sh deploy
DEPLOY_ENV=integration ./deploy.sh status

# Check logs for config source
grep "config loaded from" logs/air-quality.log
```

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| dp-017 | **REQUIRED** | Integration environment for testing |
| dp-016 | Complete | Architecture decisions (ADR-016-001, ADR-016-002) |

---

## References

- [dp-016 IMPLEMENTATION-ROADMAP.md](../dp-016/IMPLEMENTATION-ROADMAP.md) - Phase 0 and Phase 1 details
- [ADR-016-001: Config Source of Truth](../dp-016/architecture/ADR-016-001-config-source-of-truth.md)
- [dp-016 PAIN-POINTS.md](../dp-016/specification/PAIN-POINTS.md) - P-001, P-017, P-019
- [air-013 SCOPE.md](../air-013/SCOPE.md) - Absorbed feature

---

*Scope created: 2026-02-01*
*Parent: dp-016 Configuration Architecture Review*
