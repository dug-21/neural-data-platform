# dp-016: Implementation Roadmap

**Feature**: Configuration Architecture Review
**Date**: 2026-02-01
**Status**: Ready for Implementation

---

## Executive Summary

This roadmap implements the configuration architecture decisions from ADR-016-001 (JSON Source of Truth) and ADR-016-002 (Declarative Deploy). The work is organized into 6 phases with clear dependencies.

**Primary Goals**:
1. Fix the broken Silver/Dictionary config loading (reads files directly instead of etcd)
2. Establish JSON as platform configuration standard
3. Implement declarative deploy with manifest
4. Add JSON Schema validation pipeline

**Estimated Scope**: 6 phases, ~45 discrete work items

---

## Platform Constraints

| Constraint | Impact | Decision |
|------------|--------|----------|
| **No Python on Pi** | Migration/validation scripts can't be Python | Use Rust CLI or shell+jq |
| **Rust-first platform** | Tooling should match platform language | Validator = Rust, migrations = Rust or shell |
| **Edge deployment** | Minimize dependencies | No new runtime dependencies |
| **Ubuntu 25.04** | Modern shell, jq available | Shell scripts viable for simple transforms |

### Tooling Language Decisions

| Tool | Language | Rationale |
|------|----------|-----------|
| `ndp-validate` | **Rust** | Complex validation, reusable in app startup |
| `ndp-migrate-config` | **Rust** or shell+jq | Rust if complex transforms; shell+jq if simple |
| `deploy.sh` | **Shell** | Already bash, orchestration only |
| YAML→JSON migration | **Shell+jq+yq** | One-time migration, yq can be installed on dev machine |

**Note**: One-time migrations (Phase 0) can run on dev machine, not Pi. Runtime tools must work on Pi.

---

## Phase Overview

```
Phase 0: JSON Migration (Foundation)
    │
    ▼
Phase 1: Unified Config Loading (Critical Fix)
    │
    ▼
Phase 2: JSON Schema Validation
    │
    ├──────────────────┐
    ▼                  ▼
Phase 3: Declarative  Phase 4: Hot-Reload
Deploy (Manifest)     (Sources Only)
    │                  │
    └────────┬─────────┘
             ▼
Phase 5: MCP Write Tools (Future)
```

---

## Phase 0: JSON Migration (Foundation)

**Goal**: Convert all existing YAML configs to JSON, establish platform standard, and prepare schema for fields/entity_schemas merge.

**Priority**: HIGH - Foundation for all other work
**Effort**: 3-4 days
**Dependencies**: None

### Tasks

| ID | Task | Description | Acceptance Criteria |
|----|------|-------------|---------------------|
| 0.1 | Create JSON Schemas (v1) | Define `stream-config.schema.json` supporting BOTH current structure AND enriched fields | Schema accepts entity_schemas (current) AND description/device_class in fields (future) |
| 0.2 | Create supporting schemas | `dimension-config.schema.json`, `manifest.schema.json` | All config types have schemas |
| 0.3 | Build migration script | `scripts/migrate-yaml-to-json.sh` (shell+yq+jq) converts YAML → JSON | Idempotent, preserves all data |
| 0.4 | Migrate stream configs | Convert `config/base/streams/*/config.yaml` → `config.json` | All streams have valid JSON configs |
| 0.5 | **Enrich fields with descriptions** | Copy `description` from entity_schemas into corresponding fields entries | Fields have description attribute (prepares for Q6 merge) |
| 0.6 | Migrate dimension configs | Convert dimension YAML files to JSON | Dimensions validate against schema |
| 0.7 | Update .gitignore | Remove old YAML files after migration | Clean repository state |
| 0.8 | Update documentation | Update README, docs to reference JSON | No stale YAML references |

### Schema Versioning Strategy (Q8 in Action)

This migration demonstrates our schema versioning approach:

| Version | State | entity_schemas | Enriched fields | Notes |
|---------|-------|----------------|-----------------|-------|
| **v1.0** | Current (YAML) | Required | Not supported | Pre-migration state |
| **v1.1** | Transitional | Deprecated (optional) | Supported | Non-breaking, enables gradual adoption |
| **v2.0** | Target | Forbidden | Required | Breaking change, requires migration tool |

### v1.1 Schema (Phase 0 Deliverable)

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "config_version": {"enum": [1, 1.1]},
    "stream_id": {"type": "string"},
    "description": {"type": "string"},
    "fields": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "name": {"type": "string"},
          "type": {"type": "string"},
          "nullable": {"type": "boolean"},
          "unit": {"type": "string"},
          "range": {"type": "array"},
          "description": {"type": "string"},
          "device_class": {"type": "string"}
        },
        "required": ["name", "type"]
      }
    },
    "entity_schemas": {
      "type": "array",
      "deprecated": true,
      "description": "DEPRECATED in v1.1. Will be removed in v2.0. Use description/device_class in fields instead."
    }
  }
}
```

**Key Points**:
- v1.1 is a **non-breaking** change (accepts both patterns)
- Task 0.5 populates enriched fields during JSON migration
- Apps/loaders updated in Phase 1 to prefer fields, fall back to entity_schemas
- v2.0 is the **breaking** change requiring migration tool (Phase 5)

### Deliverables
- `schemas/stream-config.v1.schema.json` (supports both patterns)
- `schemas/dimension-config.schema.json`
- `schemas/manifest.schema.json`
- `scripts/migrate-yaml-to-json.sh` (shell+yq+jq, runs on dev machine)
- All configs converted to JSON with enriched fields

**Note**: The YAML→JSON migration runs on the development machine (where yq can be installed), not on the Pi. The resulting JSON files are committed to git.

### Validation
```bash
# Validate all configs against schemas
./scripts/validate-configs.sh
```

---

## Phase 1: Unified Config Loading (Critical Fix)

**Goal**: All components read config from etcd consistently. Fixes air-013 root cause.

**Priority**: CRITICAL - Fixes silent Silver ETL failures
**Effort**: 3-4 days
**Dependencies**: Phase 0 (JSON format established)

### Tasks

| ID | Task | Description | Acceptance Criteria |
|----|------|-------------|---------------------|
| 1.1 | Create ConfigLoader trait | Unified trait in `neural-core` with `load_stream_config()` | Single interface for all config loading |
| 1.2 | Implement EtcdConfigLoader | JSON-native implementation using serde_json | Reads JSON from etcd, returns typed config |
| 1.3 | Fix Silver streaming | Update `load_silver_etl_config()` to use EtcdConfigLoader | Silver streaming reads from etcd, not files |
| 1.4 | Fix Silver batch | Ensure batch ETL uses same loader | Consistent behavior with streaming |
| 1.5 | Fix data dictionary sync | Update sync to read from etcd or integrate into deploy | Dictionary sync uses same source as runtime |
| 1.5a | Update dictionary loader for fields | Dictionary loader reads from `fields.description` with fallback to `entity_schemas` | Prepares for Q6 merge; works with both patterns |
| 1.6 | Add config source logging | Log which source config was loaded from | Clear audit trail for debugging |
| 1.7 | Promote sync errors | Change WARN → ERROR for sync failures | Failures are visible, not silent |

### Code Changes

**New trait** (`core/src/config/loader.rs`):
```rust
pub trait ConfigLoader: Send + Sync {
    async fn load_stream_config(&self, stream_id: &str) -> Result<StreamConfig>;
    async fn load_silver_etl_config(&self, stream_id: &str) -> Result<SilverEtlConfig>;
}

pub struct EtcdConfigLoader {
    client: etcd_client::Client,
}
```

**Fix Silver streaming** (`apps/air-quality-app/src/silver/mod.rs`):
```rust
// BEFORE (broken):
let config = load_from_yaml_file(&path)?;

// AFTER (fixed):
let config = config_loader.load_silver_etl_config(stream_id).await?;
```

### Validation
```bash
# Start app, verify Silver ETL starts correctly
cargo run --bin air-quality-app

# Check logs for config source
grep "config loaded from" /var/log/ndp/air-quality.log
```

---

## Phase 2: Validation Pipeline

**Goal**: Validate all configs before they reach etcd or runtime.

**Priority**: HIGH - Prevents bad configs from causing runtime failures
**Effort**: 4-5 days
**Dependencies**: Phase 0, Phase 1

### Two Validation Layers

| Layer | What | How | Example |
|-------|------|-----|---------|
| **Schema Validation** | Structure, types, required fields | JSON Schema (declarative) | "fields must be an array" |
| **Semantic Validation** | Application-specific rules, valid values | Rust code (programmatic) | "type 'decimal' not supported by NDP" |

**Key Insight**: Some semantic validation COULD be injected into JSON Schema (enums, patterns), but requires research to discover what NDP actually supports today.

### Tasks

| ID | Task | Description | Acceptance Criteria |
|----|------|-------------|---------------------|
| **Research** |
| 2.0 | **Research: NDP-supported values** | Audit codebase to discover valid types, device_classes, source patterns, DQ operators | Documented list of valid values per field |
| **Layer 1: Schema Validation** |
| 2.1 | Create Validator component | Rust binary with two-layer validation | Returns structured errors |
| 2.2 | JSON syntax validation | Catch malformed JSON early | Clear error messages with line numbers |
| 2.3 | JSON Schema validation | Validate structure, types, required fields | Uses `jsonschema` crate |
| 2.4 | Unknown field detection | Fail on unexpected fields (additionalProperties: false) | No silent field capture |
| **Layer 2: Semantic Validation** |
| 2.5 | Valid `type` values | Check field types against NDP-supported types | Rejects unsupported types |
| 2.6 | Valid `device_class` values | Check against NDP-recognized device classes (if constrained) | Warns or errors on unknown |
| 2.7 | Cross-reference validation | Validate `source_path` references exist in `fields` | Catches P-005 (invalid source_path) |
| 2.8 | Silver table existence check | Verify target table exists in TimescaleDB | Catches P-006 before runtime |
| 2.9 | DQ rule syntax validation | Validate data quality rule expressions against supported operators | Catches invalid rules at deploy time |
| 2.10 | Source config validation | Validate MQTT/HTTP source configs have required fields | Catches misconfigured sources |
| **Integration** |
| 2.11 | Integrate into deploy.sh | Validation gates deployment | Bad config = deploy failure |
| 2.12 | Runtime startup validation | Defensive check at app startup | Defense in depth |
| 2.13 | Decide: Schema vs Code | After research, determine which semantic rules can be JSON Schema enums | Balance declarative vs programmatic |

### Research Task (2.0) - Deep Dive Required

This task requires auditing the NDP codebase to discover:

| Field | Question | Where to Look |
|-------|----------|---------------|
| `fields[].type` | What types does Bronze/Silver actually support? | `core/src/models/`, Parquet writers, TimescaleDB DDL generators |
| `fields[].device_class` | Is this constrained or freeform? | entity_schemas usage, Grafana integration |
| `sources[].type` | What source types exist? (mqtt, http, ...) | `core/src/sources/`, SourceManager |
| `silver_etl.field_mappings[].transform` | What transforms are supported? | Silver ETL code, transform functions |
| `dq_rules[].expression` | What DQ operators/syntax is valid? | DQ evaluation code |
| `storage.format` | What storage formats? (parquet, ...) | BronzeSubscriber, storage handlers |

**Output**: `docs/config/SUPPORTED-VALUES.md` documenting all valid values

### Research Task (2.0a) - DDL Generation (from dp-015)

dp-015 identified these DDL-related research questions:

| Topic | Question | Output |
|-------|----------|--------|
| **Type Mapping** | What PostgreSQL type for each config type? | Type mapping table in SUPPORTED-VALUES.md |
| **Index Strategy** | Auto-create on (timestamp, ndp_id)? Additional indexes from DQ rules? | Index generation rules |
| **Hypertable Config** | What chunk_time_interval? Compression settings? | Default hypertable settings |
| **Permissions** | What roles need access? (ndp_app, grafana_reader) | Permission grant templates |

**Proposed Type Mapping** (to be validated):

| Config Type | PostgreSQL Type | Notes |
|-------------|-----------------|-------|
| `string` | `TEXT` | |
| `float` | `DOUBLE PRECISION` | |
| `integer` | `BIGINT` | Safe default for sensor data |
| `boolean` | `BOOLEAN` | |
| `timestamp` | `TIMESTAMPTZ` | Always timezone-aware |
| `json` | `JSONB` | For nested/dynamic data |

**Index Strategy** (to be validated):
- Primary: `(timestamp, ndp_id)` - standard for all Silver tables
- GIN index on `dq_flags` array column (if exists)
- Additional indexes derived from `dq_rules` WHERE clauses?

**Output**: `docs/config/DDL-GENERATION.md` documenting type mapping and index strategy

### Schema vs Code Decision (2.13)

After research, decide for each semantic rule:

| Rule | JSON Schema? | Reasoning |
|------|--------------|-----------|
| Valid `type` values | Maybe (enum) | If list is small and stable, use enum |
| Valid `device_class` | Probably not | Likely freeform or extensible |
| `source_path` exists | No | Requires cross-reference logic |
| Table exists | No | Requires database query |
| DQ syntax | No | Requires expression parser |

### Validator CLI

```bash
# Validate single config (both layers)
ndp-validate config/base/streams/air-quality/config.json

# Schema validation only (fast, no DB)
ndp-validate --schema-only config.json

# Full validation with database checks
ndp-validate --check-tables --check-source-paths config.json

# Validate all configs
ndp-validate --all
```

### Error Output
```json
{
  "valid": false,
  "errors": [
    {
      "layer": "schema",
      "path": "$.fields[0].type",
      "message": "must be one of: float, integer, string, boolean, timestamp",
      "severity": "error"
    },
    {
      "layer": "semantic",
      "path": "$.silver_etl.field_mappings[2].source_path",
      "message": "source_path 'raw_payload.typo_field' not found in fields",
      "severity": "error"
    },
    {
      "layer": "semantic",
      "path": "$.silver_etl.target_table",
      "message": "table 'air_quality_readings' does not exist in TimescaleDB",
      "severity": "error"
    }
  ]
}
```

### Deliverables
- `docs/config/SUPPORTED-VALUES.md` - Research output
- `tools/ndp-validate/` - Validator binary (two-layer)
- Updated JSON Schemas with enums (where applicable)
- Integration with `deploy.sh`
- Runtime validation in app startup

---

## Phase 3: Declarative Deploy

**Goal**: Agents declare what changed; deploy executes in correct order.

**Priority**: HIGH - Reduces manual steps from 8+ to 1
**Effort**: 5-7 days
**Dependencies**: Phase 1, Phase 2

### Tasks

| ID | Task | Description | Acceptance Criteria |
|----|------|-------------|---------------------|
| 3.1 | Define manifest schema | `manifest.schema.json` with all declaration types | Schema validates all manifest patterns |
| 3.2 | Create manifest parser | Parse and validate `.deploy/manifest.json` | Typed Rust structs for manifest |
| 3.3 | Implement stream sync | Sync declared streams to etcd | Per-stream atomic updates |
| **3.4** | **Implement silver-table action (dp-015)** | Generate DDL from silver_etl config | Creates tables from config |
| 3.4a | DDL generator: CREATE TABLE | Generate column definitions using type mapping from 2.0a | Correct PostgreSQL types |
| 3.4b | DDL generator: Indexes | Generate indexes (timestamp, ndp_id) + DQ-derived | Standard + custom indexes |
| 3.4c | DDL generator: Hypertable | Convert to hypertable with chunk_time_interval | Compression-ready |
| 3.4d | DDL generator: Policies | Apply compression and retention policies | Matches existing tables |
| 3.4e | DDL generator: Permissions | Grant to ndp_app, grafana_reader roles | Consistent with existing |
| 3.4f | Idempotent execution | IF NOT EXISTS everywhere, safe to re-run | No errors on re-apply |
| 3.5 | Implement migration action | Run SQL migrations in order | Tracks applied migrations |
| 3.6 | Implement dimensions action | Sync dimension CSV to TimescaleDB | Dimensions updated atomically |
| 3.7 | Implement dictionary action | Sync data dictionary from config | Dictionary reflects current config |
| 3.8 | Implement reload logic | Hot-reload sources or full restart | Respects declared reload type |
| 3.9 | Create deploy.sh v2 | Orchestrates all actions from manifest | Single command deployment |
| 3.10 | Add device state tracking | `/var/ndp/deployed-version`, `/var/ndp/deployed-at` | Device knows what's deployed |

### Silver-Table DDL Generation (3.4 - from dp-015)

This is the core of dp-015, now integrated into declarative deploy:

**Input**: `silver_etl` section from stream config
```json
{
  "silver_etl": {
    "target_table": "silver.air_quality_readings",
    "field_mappings": [
      {"target_column": "pm25", "source_path": "raw_payload.pm25", "target_type": "float"},
      {"target_column": "temperature", "source_path": "raw_payload.temp", "target_type": "float"}
    ]
  }
}
```

**Output**: Generated DDL
```sql
-- 3.4a: CREATE TABLE
CREATE TABLE IF NOT EXISTS silver.air_quality_readings (
    timestamp TIMESTAMPTZ NOT NULL,
    ndp_id TEXT NOT NULL,
    pm25 DOUBLE PRECISION,
    temperature DOUBLE PRECISION,
    dq_flags TEXT[],
    _bronze_id UUID,
    _ingested_at TIMESTAMPTZ DEFAULT NOW()
);

-- 3.4b: Indexes
CREATE INDEX IF NOT EXISTS idx_air_quality_readings_time_id
    ON silver.air_quality_readings (timestamp, ndp_id);
CREATE INDEX IF NOT EXISTS idx_air_quality_readings_dq_flags
    ON silver.air_quality_readings USING GIN (dq_flags);

-- 3.4c: Hypertable
SELECT create_hypertable('silver.air_quality_readings', 'timestamp',
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE);

-- 3.4d: Policies
SELECT add_compression_policy('silver.air_quality_readings', INTERVAL '7 days', if_not_exists => TRUE);
SELECT add_retention_policy('silver.air_quality_readings', INTERVAL '90 days', if_not_exists => TRUE);

-- 3.4e: Permissions
GRANT SELECT, INSERT ON silver.air_quality_readings TO ndp_app;
GRANT SELECT ON silver.air_quality_readings TO grafana_reader;
```

**Depends on**: Phase 2 research (2.0a) for type mapping and index strategy

### Declaration Types

| Type | Actions | Description |
|------|---------|-------------|
| `stream` | validate → sync → reload | Stream config changed |
| `silver-table` | generate DDL → apply | Create Silver table from config |
| `migration` | apply SQL file | Run database migration |
| `dimensions` | sync CSV → TimescaleDB | Update dimension data |
| `dictionary` | sync config → data_dictionary | Refresh data dictionary |

### Deploy Flow

```bash
# Development
vi config/base/streams/new-sensor/config.json
vi .deploy/manifest.json
git add . && git commit -m "feat: add new-sensor"
git push

# Device (webhook or manual)
git pull
./deploy.sh apply

# Deploy reads manifest.json and executes:
# 1. Validate all declared changes
# 2. Run migrations (if any)
# 3. Create silver tables (if any)
# 4. Sync streams to etcd
# 5. Sync dictionary
# 6. Sync dimensions
# 7. Reload affected streams
# 8. Update /var/ndp/deployed-version
```

### Deliverables
- `.deploy/manifest.json` template
- `deploy.sh` v2 with manifest support
- Per-action implementation modules
- Device state files

---

## Phase 4: Hot-Reload (Sources Only)

**Goal**: MQTT/HTTP sources can be reconfigured without restart.

**Priority**: MEDIUM - Quality of life improvement
**Effort**: 2-3 days
**Dependencies**: Phase 1, Phase 3

### Tasks

| ID | Task | Description | Acceptance Criteria |
|----|------|-------------|---------------------|
| 4.1 | Wire etcd watch | Connect existing watch to SourceManager | Watch triggers on config change |
| 4.2 | Implement source update | `SourceManager::update_sources_for_stream()` | Sources reconnect with new config |
| 4.3 | Handle MQTT reconnect | Graceful disconnect/reconnect for MQTT | No message loss during reload |
| 4.4 | Handle HTTP polling change | Update HTTP source polling interval | Immediate effect |
| 4.5 | Add reload endpoint | Optional HTTP endpoint to trigger reload | Manual reload capability |
| 4.6 | Integration test | Test source hot-reload end-to-end | Config change → source update verified |

### Scope Limitation

**In Scope (Phase 4)**:
- MQTT source reconnection
- HTTP source reconfiguration
- Source-level hot-reload

**Out of Scope (Future)**:
- Bronze subscriber hot-reload (needs coordinator refactoring)
- Silver subscriber hot-reload (ownership model blocks this)
- DDL changes (schema migrations always require restart)

### Manifest Integration

```json
{
  "changes": [
    {
      "type": "stream",
      "id": "air-quality",
      "reload": "sources"  // Hot-reload sources only
    }
  ]
}
```

---

## Phase 5: Config Schema Migration Tool (v1.1 → v2.0)

**Goal**: Complete the entity_schemas elimination with a breaking schema change. Demonstrates Q8 (schema versioning with migration tool).

**Priority**: MEDIUM - First breaking change, validates our versioning strategy
**Effort**: 3-4 days
**Dependencies**: Phase 0 (v1.1 schema), Phase 1 (dictionary loader supports fields)

### Version Progression

```
v1.0 (YAML)     →  v1.1 (JSON, transitional)  →  v2.0 (JSON, clean)
                   Phase 0                        Phase 5

entity_schemas:    entity_schemas:               entity_schemas:
  REQUIRED           DEPRECATED (optional)         FORBIDDEN

enriched fields:   enriched fields:              enriched fields:
  NOT SUPPORTED      SUPPORTED                     REQUIRED
```

### Tasks

| ID | Task | Description | Acceptance Criteria |
|----|------|-------------|---------------------|
| 5.1 | Create migration framework | `tools/ndp-migrate-config/` (Rust CLI) with version transforms | Supports v1.1→v2, v2→v3, etc. |
| 5.2 | Create v2.0 JSON Schema | Schema WITHOUT entity_schemas; fields REQUIRED to have description | Enforces merged structure |
| 5.3 | Implement v1.1→v2.0 migration | Remove entity_schemas section (data already in fields from Phase 0) | Clean configs, no data loss |
| 5.4 | Remove entity_schemas fallback | Update dictionary loader to read ONLY from fields | entity_schemas code paths removed |
| 5.5 | Create migration CLI | `ndp-migrate-config --from 1.1 --to 2` | Transforms all configs |
| 5.6 | Add dry-run mode | Preview changes without writing | Safe migration testing |
| 5.7 | Update validator | Enforce v2.0 schema; reject configs with entity_schemas | Clean break enforced |
| 5.8 | Update sync scripts | Remove entity_schemas handling from deploy.sh | No legacy code paths |
| 5.9 | Remove deprecated structs | Remove `EntitySchema` struct from Rust code | Code cleanup |

### v1.1 → v2.0 Migration Details

**Before (v1.1 - from Phase 0)**:
```json
{
  "config_version": 1.1,
  "fields": [
    {
      "name": "pm25",
      "type": "float",
      "unit": "µg/m³",
      "range": [0, 500],
      "description": "Particulate matter 2.5µm",
      "device_class": "sensor"
    }
  ],
  "entity_schemas": [
    {"name": "pm25", "description": "Particulate matter 2.5µm", "device_class": "sensor"}
  ]
}
```

**After (v2.0)**:
```json
{
  "config_version": 2,
  "fields": [
    {
      "name": "pm25",
      "type": "float",
      "unit": "µg/m³",
      "range": [0, 500],
      "description": "Particulate matter 2.5µm",
      "device_class": "sensor"
    }
  ]
}
```

**Migration Logic** (simpler because Phase 0 already enriched fields):
1. Verify all entity_schemas entries have corresponding enriched fields (validation)
2. Remove `entity_schemas` section entirely
3. Bump `config_version` to 2

**Note**: The heavy lifting (copying data) happened in Phase 0. This migration is primarily cleanup.

### Migration Workflow

```bash
# Check current versions
ndp-migrate-config --status

# Preview migration (shows what would change)
ndp-migrate-config --from 1 --to 2 --dry-run

# Apply migration
ndp-migrate-config --from 1 --to 2

# Validate migrated configs
./scripts/validate-configs.sh --schema-version 2

# Commit migrated configs
git add config/ schemas/
git commit -m "chore: migrate configs to v2 (merge entity_schemas into fields)"
```

### Dictionary Loader Update (5.4)

**Before** (Phase 1 - supports both):
```rust
fn get_field_description(config: &StreamConfig, field_name: &str) -> Option<String> {
    // Try fields first (new pattern)
    if let Some(desc) = config.fields.iter()
        .find(|f| f.name == field_name)
        .and_then(|f| f.description.clone()) {
        return Some(desc);
    }
    // Fallback to entity_schemas (old pattern)
    config.entity_schemas.as_ref()
        .and_then(|es| es.iter().find(|e| e.name == field_name))
        .and_then(|e| e.description.clone())
}
```

**After** (Phase 5 - fields only):
```rust
fn get_field_description(config: &StreamConfig, field_name: &str) -> Option<String> {
    config.fields.iter()
        .find(|f| f.name == field_name)
        .and_then(|f| f.description.clone())
}
```

---

## Phase 6: MCP Write Tools (Future)

**Goal**: Full MCP administration capability.

**Priority**: LOW - Future enhancement
**Effort**: 5-7 days
**Dependencies**: All previous phases

### Tasks

| ID | Task | Description | Acceptance Criteria |
|----|------|-------------|---------------------|
| 6.1 | create_stream MCP tool | Create new stream config via MCP | Writes JSON, triggers deploy |
| 6.2 | update_stream MCP tool | Modify existing stream config | Validates before save |
| 6.3 | delete_stream MCP tool | Remove stream config | Cleans up etcd, optional table drop |
| 6.4 | validate_stream MCP tool | Dry-run validation | Returns validation errors |
| 6.5 | create_silver_table MCP tool | Generate and apply DDL | Creates table from config |
| 6.6 | reload_stream MCP tool | Trigger hot-reload | For source-level changes |

### MCP Flow

```
MCP Tool (create_stream)
    │
    ├── Validate JSON against schema
    ├── Write to config/base/streams/{id}/config.json
    ├── Update .deploy/manifest.json
    │
    ▼
Trigger deploy.sh apply
    │
    ▼
Git commit/push (backup)
```

---

## Implementation Order Summary

| Phase | Priority | Effort | Key Deliverable | Absorbs |
|-------|----------|--------|-----------------|---------|
| **0: JSON Migration** | HIGH | 3-4 days | All configs in JSON (v1.1), schemas support enriched fields | - |
| **1: Unified Loading** | CRITICAL | 4-5 days | Silver reads from etcd, dictionary loader supports fields | **air-013** |
| **2: Validation** | HIGH | 5-6 days | Two-layer validation, type mapping research, DDL research | dp-015 research |
| **3: Declarative Deploy** | HIGH | 6-8 days | Manifest-driven deployment, DDL generation | **dp-015** |
| **4: Hot-Reload** | MEDIUM | 2-3 days | Source hot-reload working | - |
| **5: Schema Migration** | MEDIUM | 3-4 days | v1.1→v2.0 migration (entity_schemas removed) | - |
| **6: MCP Write** | LOW | 5-7 days | Full MCP administration | - |

**Total Estimated Effort**: 28-37 days

**Note**: Effort increased slightly because dp-015 DDL generation is now explicit (was implicit in "silver-table action").

---

## Quick Wins (Can Start Immediately)

1. **Create JSON Schemas** (0.1) - No code changes, enables IDE validation immediately
2. **Enrich fields with descriptions** (0.5) - Prepares for Q6 merge during initial migration
3. **Promote sync errors to ERROR** (1.7) - One-line change, immediate visibility
4. **Add config source logging** (1.6) - Easy debugging improvement
5. **Create manifest schema** (3.1) - Documentation, enables validation

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| JSON migration breaks existing tooling | Phase 0 includes validation; old YAML kept until verified |
| Silver fix introduces regressions | Phase 1 includes comprehensive testing |
| Declarative deploy complexity | Start with subset of declaration types |
| Hot-reload edge cases | Phase 4 scoped to sources only; subscribers deferred |

---

## Success Metrics

| Metric | Current | Phase 0 | Phase 2 | Phase 5 |
|--------|---------|---------|---------|---------|
| Silent config failures | Common | Reduced | Rare | Zero |
| Manual deploy steps | 8+ | 8+ | 8+ | 1 |
| Schema validation coverage | 0% | 70% | 95% | 95% |
| Semantic validation coverage | 0% | 0% | 80%+ | 90%+ |
| Config format | YAML (v1.0) | JSON (v1.1) | JSON (v1.1) | JSON (v2.0) |
| Field metadata locations | 2 | 2 (transitional) | 2 (transitional) | 1 (fields only) |
| Schema versioning | None | Demonstrated | Demonstrated | Proven |
| NDP-supported values documented | No | No | Yes | Yes |
| MCP write capability | 0 tools | 0 tools | 0 tools | 6 tools |
| Hot-reload support | None | None | None | Sources |

---

## Feature Integration (air-013, dp-015)

These features were identified BEFORE the dp-016 architecture decisions. Now that we have a unified plan, they are formally absorbed.

### air-013: Unified Config Source for Silver ETL → **ABSORBED into Phase 1**

| air-013 Scope | dp-016 Coverage | Task |
|---------------|-----------------|------|
| Add silver_etl to StreamConfig | Already in scope | 1.2, 1.3 |
| Update ConfigSyncService to include silver_etl | Covered by unified loader | 1.2 |
| Update SilverSubscriber to read from etcd | Core fix | 1.3 |
| Remove YAML file dependency | Result of Phase 1 | 1.3, 1.4 |

**air-013 estimated effort was 4 hours. Phase 1 is larger (4-5 days) because it includes:**
- JSON format (not just fixing the loading path)
- ConfigLoader trait abstraction
- Dictionary loader update for fields/entity_schemas transition
- Comprehensive logging and error handling

**Recommendation**: Close air-013 as "Absorbed by dp-016 Phase 1" after Phase 1 completes.

---

### dp-015: Config-Driven Silver Table Creation → **ABSORBED into Phase 3**

| dp-015 Scope | dp-016 Coverage | Task |
|--------------|-----------------|------|
| Generate DDL from silver_etl config | Manifest `silver-table` action | 3.4 |
| Idempotent table creation (IF NOT EXISTS) | Part of DDL generation | 3.4 |
| Type mapping (JSON type → PostgreSQL) | **Gap: Needs research** | NEW 3.4a |
| Index strategy (timestamp, ndp_id) | **Gap: Needs research** | NEW 3.4b |
| Hypertable conversion | Part of DDL generation | 3.4 |
| Compression/retention policies | Part of DDL generation | 3.4 |
| Permissions (ndp_app role) | Part of DDL generation | 3.4 |

**Gaps identified from dp-015**:
- **Type mapping research**: What PostgreSQL type for each JSON type?
- **Index strategy**: Auto-create on (timestamp, ndp_id)? Parse DQ rules for additional indexes?
- **Schema evolution**: What happens when field_mappings change? (dp-015 marked this out of scope)

**Recommendation**:
1. Add type mapping/index research to Phase 2 (alongside NDP-supported values research)
2. Close dp-015 as "Absorbed by dp-016 Phase 3" after Phase 3 completes

---

### Integration Summary

| Feature | Status | Absorbed Into | Close When |
|---------|--------|---------------|------------|
| **air-013** | Fully absorbed | Phase 1 | Phase 1 complete |
| **dp-015** | Mostly absorbed (gaps filled) | Phase 2 (research), Phase 3 (implementation) | Phase 3 complete |

---

## External Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| **dp-017: Integration Environment** | **PREREQUISITE** | Must complete before dp-016 begins |
| Future MCP UI | Phase 6 (MCP write tools) | Enables full administration |
| Gold layer design | Out of scope | Q7 deferred cross-stream to Gold |

---

## Decision Traceability

All decisions from DECISION-QUESTIONS.md are mapped to implementation tasks:

| Decision | Description | Phase | Tasks |
|----------|-------------|-------|-------|
| **Q1: Source of Truth** | JSON primary, etcd as runtime cache | 0, 1 | 0.1-0.8 (JSON), 1.1-1.7 (etcd loading) |
| **Q2: Per-stream isolation** | Changes to one stream don't affect others | 1, 3 | 1.2 (blob storage), 3.3 (per-stream sync) |
| **Q3: Storage Format** | JSON per stream (native etcd format) | 0, 1 | 0.1-0.4 (JSON migration), 1.2 (JSON in etcd) |
| **Q4: Silver Table DDL** | Explicit declaration in manifest | 2, 3 | 2.0a (DDL research), 3.4-3.4f (DDL generation) |
| **Q5: Hot-Reload Scope** | Sources hot-reload; subscribers require restart | 3, 4 | 3.8 (reload logic), 4.1-4.6 (source reload) |
| **Q6: Merge fields/entity_schemas** | Eliminate entity_schemas section | 0, 1, 5 | 0.1 (v1.1 schema), 0.5 (enrich fields), 1.5a (loader fallback), 5.2-5.9 (v2.0 migration) |
| **Q7: Silver cross-stream** | Defer to Gold layer | - | Out of scope (correctly deferred) |
| **Q8: Config schema versioning** | Breaking changes + migration tool | 0, 5 | **Demonstrated by Q6**: v1.0→v1.1 (non-breaking), v1.1→v2.0 (breaking + migration) |
| **Emergent: Declarative Deploy** | Manifest-driven deployment | 3 | 3.1-3.10 (full implementation) |
| **Emergent: Validator** | Two-layer validation (schema + semantic) | 2 | 2.0 (research), 2.1-2.4 (schema), 2.5-2.10 (semantic), 2.11-2.13 (integration) |
| **Emergent: JSON Standard** | JSON as platform configuration format | 0 | 0.1-0.8 (all JSON work) |
| **air-013** | Unified config source for Silver ETL | 1 | 1.2-1.4 (Silver reads from etcd) |
| **dp-015** | Config-driven Silver table creation | 2, 3 | 2.0a (type/index research), 3.4-3.4f (DDL generation) |

### Q6 Implementation Path (fields/entity_schemas merge)

This decision spans multiple phases with a careful transition, demonstrating Q8 (schema versioning):

```
Phase 0: v1.0 → v1.1 (Non-Breaking)
├── 0.1: v1.1 schema supports BOTH patterns
├── 0.5: Copy descriptions from entity_schemas INTO fields
├── Result: Configs at v1.1, data duplicated (fields enriched, entity_schemas still present)
└── Benefit: All readers work, gradual adoption possible

Phase 1: Code Transition
├── 1.5a: Dictionary loader reads from fields FIRST, falls back to entity_schemas
├── Result: New code prefers enriched fields
└── Benefit: Codebase ready for v2.0

Phase 5: v1.1 → v2.0 (Breaking Change)
├── 5.2: v2.0 schema requires enriched fields, forbids entity_schemas
├── 5.3: Migration removes entity_schemas section (data already in fields)
├── 5.4: Remove entity_schemas fallback from loader
├── 5.7: Validator rejects configs with entity_schemas
├── 5.9: Remove EntitySchema struct from Rust code
└── Result: Clean codebase, single source of field metadata
```

**This is Q8 in action**: Breaking change (v2.0) with migration tool, non-breaking intermediate (v1.1).

---

## Next Steps

1. Review and approve this roadmap
2. Create GitHub issues for Phase 0 and Phase 1 tasks
3. Begin Phase 0: JSON Migration
4. Parallel: Update STATUS.md to track progress

---

## Prerequisite: dp-017 Integration Environment

**dp-016 depends on dp-017 completing first.**

### Why This Is a Prerequisite

Building a declarative deployment system (Phase 3) without a matching test environment means testing in production. Given the scope of dp-016's refactoring, this is too risky.

### dp-017 Scope (Separate Feature)

| Task | Description |
|------|-------------|
| Align integration compose | Match `docker-compose.integration.yml` to production (Pi) |
| Remove silver-etl-daemon | Obsolete - ETL now in air-quality-app |
| Add missing services | grafana, ndp-mcp-server |
| Fix mosquitto config | Remove deprecated options |
| Fix init script paths | Auto-create schemas on startup |
| Clean up root compose files | Consolidate or remove docker-compose.prod.yml |
| Create test harness | `scripts/integration-test.sh` for spin-up/down |
| Evaluate apps/silver-etl/ | Remove if fully migrated |

### Implementation Order

```
dp-017: Integration Environment  →  dp-016: Config Architecture
        (1-2 days)                        (28-37 days)
```

**Recommendation**: Create dp-017 SCOPE.md, complete it, then begin dp-016 Phase 0

---

*Roadmap created: 2026-02-01*
*Based on: ADR-016-001, ADR-016-002, DECISION-QUESTIONS.md*
