# dp-021: Config Lifecycle & MCP Administration

## Parent Initiative

This feature implements **Phases 4, 5, and 6** of [dp-016: Configuration Architecture Review](../dp-016/IMPLEMENTATION-ROADMAP.md).

---

## Problem Statement

After the foundation (dp-018), validation (dp-019), and declarative deploy (dp-020) are complete, several capabilities remain:

1. **No hot-reload** - Source config changes require full app restart
2. **Schema migration needed** - entity_schemas still exists (deprecated in v1.1, needs removal)
3. **No MCP write tools** - Agents can read config but not create/update streams

These are lower priority than the core fixes but complete the configuration architecture vision.

---

## Goals

1. **Hot-reload sources** - MQTT/HTTP source changes without restart
2. **Complete entity_schemas elimination** - v1.1 → v2.0 migration
3. **MCP administration** - Full CRUD for stream configs via MCP

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

### Phase 5: Schema Migration Tool (v1.1 → v2.0)

| ID | Task | Description | Acceptance Criteria |
|----|------|-------------|---------------------|
| 5.1 | Create migration framework | `tools/ndp-migrate-config/` with version transforms | Supports v1.1→v2, v2→v3, etc. |
| 5.2 | Create v2.0 JSON Schema | Schema WITHOUT entity_schemas; fields REQUIRED | Enforces merged structure |
| 5.3 | Implement v1.1→v2.0 migration | Remove entity_schemas (data already in fields) | Clean configs, no data loss |
| 5.4 | Remove entity_schemas fallback | Dictionary loader reads ONLY from fields | Code cleanup |
| 5.5 | Create migration CLI | `ndp-migrate-config --from 1.1 --to 2` | Transforms all configs |
| 5.6 | Add dry-run mode | Preview changes without writing | Safe migration testing |
| 5.7 | Update validator | Enforce v2.0 schema; reject entity_schemas | Clean break enforced |
| 5.8 | Update sync scripts | Remove entity_schemas handling | No legacy code paths |
| 5.9 | Remove deprecated structs | Remove `EntitySchema` from Rust code | Code cleanup |

### Phase 6: MCP Write Tools

| ID | Task | Description | Acceptance Criteria |
|----|------|-------------|---------------------|
| 6.1 | create_stream MCP tool | Create new stream config via MCP | Writes JSON, triggers deploy |
| 6.2 | update_stream MCP tool | Modify existing stream config | Validates before save |
| 6.3 | delete_stream MCP tool | Remove stream config | Cleans up etcd, optional table drop |
| 6.4 | validate_stream MCP tool | Dry-run validation | Returns validation errors |
| 6.5 | create_silver_table MCP tool | Generate and apply DDL | Creates table from config |
| 6.6 | reload_stream MCP tool | Trigger hot-reload | For source-level changes |

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

### v1.1 → v2.0 Migration

**Before (v1.1 from dp-018)**:
```json
{
  "config_version": 1.1,
  "fields": [
    {
      "name": "pm25",
      "type": "float",
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
      "description": "Particulate matter 2.5µm",
      "device_class": "sensor"
    }
  ]
}
```

**Migration Logic** (simple because dp-018 already enriched fields):
1. Verify all entity_schemas entries have corresponding enriched fields
2. Remove `entity_schemas` section entirely
3. Bump `config_version` to 2

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

### MCP Tool Flow

```
MCP Tool (create_stream)
    │
    ├── Validate JSON against schema (dp-019)
    ├── Write to config/base/streams/{id}/config.json
    ├── Update .deploy/manifest.json
    │
    ▼
Trigger deploy.sh apply (dp-020)
    │
    ▼
Git commit/push (backup)
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
| Migration tool | `tools/ndp-migrate-config/` | Rust CLI for schema migrations |
| v2.0 Schema | `schemas/stream-config.v2.schema.json` | Clean schema without entity_schemas |
| Updated validator | `tools/ndp-validate/` | Enforces v2.0 |

### Phase 6
| Deliverable | Location | Description |
|-------------|----------|-------------|
| MCP write tools | `apps/ndp-mcp-server/src/tools/` | 6 new MCP tools |
| Tool schemas | `apps/ndp-mcp-server/schemas/` | JSON schemas for tool inputs |

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

### Phase 6: MCP Write
1. **create_stream** creates valid config and triggers deploy
2. **update_stream** validates before saving
3. **delete_stream** cleans up etcd and optionally drops table
4. **All tools return** structured success/error responses

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

# Phase 6: MCP tools
# Via MCP client
mcp call create_stream '{"stream_id": "test-sensor", ...}'
mcp call validate_stream '{"stream_id": "test-sensor"}'
```

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| dp-018 | **REQUIRED** | JSON configs with v1.1 schema |
| dp-019 | **REQUIRED** | Validation pipeline |
| dp-020 | **REQUIRED** | Declarative deploy for MCP integration |
| dp-017 | **REQUIRED** | Integration environment |

---

## Phasing Options

This feature can be implemented incrementally:

| Option | Phases | Effort | Value |
|--------|--------|--------|-------|
| **Minimal** | 4 only | 2-3 days | Hot-reload for operations |
| **Core** | 4 + 5 | 5-7 days | Clean schema, hot-reload |
| **Full** | 4 + 5 + 6 | 10-14 days | Complete MCP administration |

**Recommendation**: Implement Phase 4 and 5 first. Phase 6 (MCP write) can be deferred until there's demand for agent-driven config creation.

---

## References

- [dp-016 IMPLEMENTATION-ROADMAP.md](../dp-016/IMPLEMENTATION-ROADMAP.md) - Phases 4, 5, 6 details
- [dp-016 HOT-RELOAD-FEASIBILITY.md](../dp-016/architecture/HOT-RELOAD-FEASIBILITY.md)
- [dp-016 MCP-ADMIN-ANALYSIS.md](../dp-016/architecture/MCP-ADMIN-ANALYSIS.md)
- [dp-018 SCOPE.md](../dp-018/SCOPE.md) - v1.1 schema foundation

---

*Scope created: 2026-02-01*
*Parent: dp-016 Configuration Architecture Review*
