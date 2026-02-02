# dp-021: Config Lifecycle & Release Management

## Parent Initiative

This feature implements **Phases 4 and 5** of [dp-016: Configuration Architecture Review](../dp-016/IMPLEMENTATION-ROADMAP.md), plus **Release Management** formalization.

**Note**: Phase 6 (MCP Write Tools) has been moved to [dp-022](../dp-022/SCOPE.md).

---

## Problem Statement

After the foundation (dp-018), validation (dp-019), and declarative deploy (dp-020) are complete, several capabilities remain:

1. **No hot-reload** - Source config changes require full app restart
2. **Schema migration needed** - entity_schemas still exists (deprecated in v1.1, needs removal)
3. **No release methodology** - Deployment manifests aren't formally tied to versioned releases
4. **No automated deployment trigger** - Manual `deploy.sh apply` required on every release

---

## Goals

1. **Hot-reload sources** - MQTT/HTTP source changes without restart
2. **Complete entity_schemas elimination** - v1.1 → v2.0 migration
3. **Formalize release methodology** - Opinionated versioning tied to deployment manifests
4. **Enable automated deployment** - Foundation for webhook-triggered deployments

---

## Scope

### Phase 4: Hot-Reload (Sources Only)

| ID | Task | Description | Acceptance Criteria |
|----|------|-------------|---------------------|
| 4.1 | Wire etcd watch | Connect existing watch to SourceManager | Watch triggers on config change |
| 4.2 | Implement source update | `SourceManager::update_sources_for_stream()` | Sources reconnect with new config |
| 4.3 | Handle MQTT reconnect | Graceful disconnect/reconnect for MQTT | No message loss during reload |
| 4.4 | Handle HTTP polling change | Update HTTP source polling interval | Immediate effect |
| 4.5 | Add reload endpoint | Optional HTTP endpoint to trigger reload | Manual reload capability |
| 4.6 | Integration test | Test source hot-reload end-to-end | Config change → source update |

**Scope Limitation**:
- **In Scope**: MQTT source reconnection, HTTP source reconfiguration
- **Out of Scope**: Bronze/Silver subscriber hot-reload (requires coordinator refactoring)

### Phase 5: Schema Migration (v1.1 → v2.0)

| ID | Task | Description | Acceptance Criteria |
|----|------|-------------|---------------------|
| 5.1 | Create migration script | `scripts/ndp-migrate-config.sh` (shell+jq) | Transforms v1.1→v2.0 |
| 5.2 | Create v2.0 JSON Schema | Schema WITHOUT entity_schemas; fields REQUIRED | Enforces merged structure |
| 5.3 | Implement v1.1→v2.0 transform | `jq 'del(.entity_schemas) \| .config_version = 2'` | Clean configs, no data loss |
| 5.4 | Remove entity_schemas fallback | Dictionary loader reads ONLY from fields | Code cleanup |
| 5.5 | Add dry-run mode | Preview changes without writing | Safe migration testing |
| 5.6 | Update validator | Enforce v2.0 schema; reject entity_schemas | Clean break enforced |
| 5.7 | Update sync scripts | Remove entity_schemas handling | No legacy code paths |
| 5.8 | Remove deprecated structs | Remove `EntitySchema` from Rust code | Code cleanup |

**Implementation note**: Shell+jq sufficient for this migration. Future architecture will use Rust crates callable from CLI and MCP (pending access control design).

### Phase R: Release Methodology (NEW)

| ID | Task | Description | Acceptance Criteria |
|----|------|-------------|---------------------|
| R.1 | Define versioning standard | Semantic versioning rules for NDP platform | Documented in RELEASE-POLICY.md |
| R.2 | Formalize manifest naming | `v{MAJOR}.{MINOR}.{PATCH}.manifest.json` convention | All releases follow convention |
| R.3 | Create release checklist | Steps for creating a versioned release | Documented procedure |
| R.4 | Align git tags to manifests | Git tag `vX.Y.Z` matches `vX.Y.Z.manifest.json` | 1:1 mapping enforced |
| R.5 | Add manifest version field | Manifest includes `release_version` field | Deploy knows exact version |
| R.6 | Device deployed-version tracking | `/var/ndp/deployed-version` updated on apply | Device state queryable |
| R.7 | Create release template | `.deploy/releases/TEMPLATE.manifest.json` | Consistent release creation |
| R.8 | Document webhook trigger spec | Specification for future automated deployment | Ready for dp-023 implementation |

---

## Release Methodology Design

### Versioning Standard

NDP follows **Semantic Versioning 2.0.0**:

```
MAJOR.MINOR.PATCH

MAJOR - Breaking changes (schema v2.0, API changes)
MINOR - New features (new stream, new MCP tool)
PATCH - Bug fixes, config corrections
```

### Release Artifacts

Each release consists of:

```
Git tag: v1.2.0
    │
    ├── .deploy/releases/v1.2.0.manifest.json  (what to deploy)
    ├── config/base/streams/*/config.json      (configurations)
    └── CHANGELOG.md entry                      (human description)
```

### Manifest-Version Alignment

```json
{
  "$schema": "../schemas/manifest.schema.json",
  "version": "1.0",
  "release_version": "1.2.0",
  "description": "Release v1.2.0: Add weather-station stream",
  "changes": [...]
}
```

### Release Workflow

```
1. Developer creates/modifies configs
2. Developer creates .deploy/releases/v{X}.{Y}.{Z}.manifest.json
3. Developer commits and creates git tag v{X}.{Y}.{Z}
4. Developer pushes code and tag
5. On Pi: git pull && ./deploy.sh apply .deploy/releases/v{X}.{Y}.{Z}.manifest.json
6. Device state updated: /var/ndp/deployed-version = v{X}.{Y}.{Z}
```

### Future: Webhook-Triggered Deployment (dp-023)

This release establishes the foundation for automated deployment:

```
GitHub webhook (tag push v*)
    │
    ▼
Pi webhook receiver (future dp-023)
    │
    ├── git pull
    ├── Locate .deploy/releases/v{tag}.manifest.json
    ├── ./deploy.sh apply <manifest>
    └── Report status back to GitHub
```

---

## Technical Context

### Schema Version Progression

```
v1.0 (YAML)     →  v1.1 (dp-018)        →  v2.0 (this feature)
                   JSON, transitional       JSON, clean

entity_schemas:    entity_schemas:         entity_schemas:
  REQUIRED           DEPRECATED              FORBIDDEN

enriched fields:   enriched fields:        enriched fields:
  NOT SUPPORTED      SUPPORTED               REQUIRED
```

### Hot-Reload Architecture

```
etcd watch (stream config change)
    │
    ▼
 SourceManager::on_config_change(stream_id)
    │
    ├── Parse new source config
    ├── Gracefully disconnect old sources
    ├── Create new sources with new config
    └── Log reload event
```

---

## Deliverables

### Phase 4
| Deliverable | Location | Description |
|-------------|----------|-------------|
| etcd watch wiring | `apps/air-quality-app/src/` | Watch → SourceManager connection |
| Source update logic | `core/src/sources/manager.rs` | Hot-reload implementation |
| Reload endpoint | `apps/air-quality-app/src/api/` | Optional HTTP trigger |

### Phase 5
| Deliverable | Location | Description |
|-------------|----------|-------------|
| Migration script | `scripts/ndp-migrate-config.sh` | Shell+jq for v1.1→v2.0 migration |
| v2.0 Schema | `schemas/stream-config.v2.schema.json` | Clean schema without entity_schemas |
| Updated validator | `tools/ndp-validate/` | Enforces v2.0 |

**Note**: Shell script is sufficient for v1.1→v2.0 (simple `jq` transform). Future architecture may consolidate to Rust crates callable from both CLI and MCP.

### Phase R (Release)
| Deliverable | Location | Description |
|-------------|----------|-------------|
| Release policy | `docs/procedures/RELEASE-POLICY.md` | Versioning standard |
| Release template | `.deploy/releases/TEMPLATE.manifest.json` | Template for new releases |
| Updated manifest schema | `schemas/manifest.schema.json` | Adds `release_version` field |
| Webhook spec | `docs/procedures/WEBHOOK-DEPLOYMENT-SPEC.md` | Foundation for dp-023 |

---

## Success Criteria

### Phase 4: Hot-Reload
1. **MQTT source reconnects** on config change without app restart
2. **HTTP polling interval** updates immediately
3. **No message loss** during MQTT reconnection

### Phase 5: Schema Migration
1. **All configs at v2.0** - entity_schemas removed
2. **Validator rejects** configs with entity_schemas
3. **No EntitySchema struct** in Rust code
4. **Migration is reversible** (dry-run mode)

### Phase R: Release Methodology
1. **All releases follow** `vX.Y.Z.manifest.json` naming
2. **Git tags align** with manifest versions
3. **Device reports** deployed version via `/var/ndp/deployed-version`
4. **Webhook spec documented** for future implementation

### Verification Commands

```bash
# Phase 4: Hot-reload test
DEPLOY_ENV=integration ./deploy.sh deploy
# Change MQTT topic in config
# Verify source reconnects without restart

# Phase 5: Migration
ndp-migrate-config --from 1.1 --to 2 --dry-run
ndp-migrate-config --from 1.1 --to 2
ndp-validate --all  # Should pass with v2.0 schema

# Phase R: Release
ls .deploy/releases/  # Should show v*.manifest.json files
cat /var/ndp/deployed-version  # Should show vX.Y.Z
git tag -l 'v*'  # Should match manifest files
```

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| dp-017 | **Complete** | Integration environment |
| dp-018 | **Complete** | JSON configs, etcd blob sync (simpler than planned) |
| dp-019 | **Complete** | Validation pipeline (134 tests) |
| dp-020 | **Complete** | Declarative deploy (9-phase orchestration) |

**All prerequisites are complete.** dp-021 can proceed.

---

## Out of Scope (Moved to dp-022)

The following MCP Write Tools are deferred to dp-022:

| Tool | Description |
|------|-------------|
| `create_stream` | Create new stream config via MCP |
| `update_stream` | Modify existing stream config |
| `delete_stream` | Remove stream config |
| `validate_stream` | Dry-run validation |
| `create_silver_table` | Generate and apply DDL |
| `reload_stream` | Trigger hot-reload |

---

## Phasing Options

| Option | Phases | Effort | Value |
|--------|--------|--------|-------|
| **Minimal** | 4 only | 2-3 days | Hot-reload for operations |
| **Core** | 4 + 5 | 5-7 days | Clean schema, hot-reload |
| **Full** | 4 + 5 + R | 8-10 days | Complete lifecycle management |

**Recommendation**: Implement all phases (4, 5, R) to establish the complete release methodology before adding MCP write capabilities in dp-022.

---

## Related Features

| Feature | Relationship |
|---------|--------------|
| dp-016 | Parent: Configuration Architecture Review |
| dp-018 | Prerequisite: JSON Config Foundation |
| dp-019 | Prerequisite: Validation Pipeline |
| dp-020 | Prerequisite: Declarative Deploy |
| dp-022 | Successor: MCP Write Tools |
| dp-023 | Future: Webhook-Triggered Deployment |

---

*Scope created: 2026-02-01*
*Scope updated: 2026-02-02 (MCP Write → dp-022, Release Methodology added)*
*Parent: dp-016 Configuration Architecture Review*
