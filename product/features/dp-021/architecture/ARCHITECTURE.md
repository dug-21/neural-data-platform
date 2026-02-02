# dp-021: Config Lifecycle & Release Management - Architecture

**Status**: Proposed
**Date**: 2026-02-02
**Decision Makers**: NDP Architecture Team
**Feature**: dp-021 Config Lifecycle & Release Management
**Parent ADRs**: ADR-016-001 (JSON Source of Truth), ADR-016-002 (Declarative Deploy)
**Prerequisites**: dp-018 (JSON Foundation), dp-019 (Validation), dp-020 (Declarative Deploy)

---

## Executive Summary

dp-021 completes the configuration lifecycle with three capabilities:

1. **Hot-Reload (Phase 4)**: MQTT/HTTP sources reconfigure without restart
2. **Schema Migration (Phase 5)**: v1.1 to v2.0 config schema transition
3. **Release Methodology (Phase R)**: Formalized versioning with manifest alignment

This architecture document covers the system design for all three phases.

---

## System Context

### Current State (After dp-020)

```
Developer                          Pi Device
    |                                  |
    +-- Edit config.json               |
    +-- Update manifest.json           |
    +-- git push ------------------>   |
    |                            git pull
    |                            ./deploy.sh apply
    |                                  |
    |                              (9-phase orchestration)
    |                                  |
    |                              RESTART REQUIRED
    |                              for config changes
    <-- Deployment complete -----------|
```

**Problems addressed by dp-021**:
- Source config changes require full app restart
- Schema still has deprecated entity_schemas
- No formal release versioning
- Manual manifest editing for every deploy

### Target State (After dp-021)

```
Developer                          Pi Device
    |                                  |
    +-- Edit config.json               |
    +-- Create release manifest        |
    +-- Create git tag v1.2.0          |
    +-- git push --tags ------------>  |
    |                            git pull
    |                            ./deploy.sh apply .deploy/releases/v1.2.0.manifest.json
    |                                  |
    |                              (9-phase orchestration)
    |                                  |
    |                              HOT-RELOAD for sources
    |                              (no restart needed)
    |                                  |
    |                              /var/ndp/deployed-version = v1.2.0
    <-- Deployment complete -----------|

Future (dp-023):
    +-- git push --tags ------------>  |
    |                            WEBHOOK TRIGGERS AUTO-DEPLOY
```

---

## Component Architecture

### Hot-Reload System (Phase 4)

```
+------------------------------------------------------------------+
|                      air-quality-app                              |
+------------------------------------------------------------------+
|                                                                   |
|  +-----------------------+      +--------------------------+      |
|  |    etcd Watch         |      |    SourceManager         |      |
|  |                       |      |                          |      |
|  | Watches:              |      | - Manages MQTT/HTTP      |      |
|  | /streams/*/config     |----->| - update_sources_for_    |      |
|  |                       |      |   stream(stream_id)      |      |
|  | On change:            |      | - Graceful reconnect     |      |
|  | - Parse new config    |      |                          |      |
|  | - Validate            |      |                          |      |
|  | - Notify SourceMgr    |      |                          |      |
|  +-----------------------+      +--------------------------+      |
|           |                              |                        |
|           |                              v                        |
|           |                     +------------------+              |
|           |                     | MqttSource       |              |
|           |                     | - disconnect()   |              |
|           |                     | - reconnect()    |              |
|           |                     +------------------+              |
|           |                              |                        |
|           |                     +------------------+              |
|           |                     | HttpPollingSource|              |
|           |                     | - update_interval|              |
|           |                     +------------------+              |
|           |                                                       |
+------------------------------------------------------------------+
            |
            v
+------------------------------------------------------------------+
|                           etcd                                    |
|  /streams/air-quality/config = { JSON blob }                      |
+------------------------------------------------------------------+
```

### Hot-Reload Data Flow

```
1. Config Change in etcd
   /streams/air-quality/config updated
           |
           v
2. etcd Watch Notification
   WatchResponse { key, value, mod_revision }
           |
           v
3. Config Validation
   Parse JSON, validate against v2.0 schema
           |
   +-------+-------+
   |               |
   v               v
   VALID        INVALID
   |               |
   v               v
4a. SourceManager::on_config_change()     4b. Log error, keep current config
           |
           v
5. Identify Changed Sources
   Compare old.sources vs new.sources
           |
   +-------+-------+-------+
   |       |       |       |
   v       v       v       v
 Added  Modified Removed Unchanged
   |       |       |       |
   v       v       v       v
 Start  Restart  Stop   No-op
```

### Scope Limitation: Sources Only

```
+------------------------------------------------------------------+
|                      Hot-Reload Scope                             |
+------------------------------------------------------------------+
|                                                                   |
|  IN SCOPE (Sources):                                              |
|  +-------------------+    +-------------------+                   |
|  | MqttSource        |    | HttpPollingSource |                   |
|  | - Topic change    |    | - URL change      |                   |
|  | - Broker change   |    | - Interval change |                   |
|  | - Auth change     |    | - Header change   |                   |
|  +-------------------+    +-------------------+                   |
|                                                                   |
|  OUT OF SCOPE (Subscribers):                                      |
|  +-------------------+    +-------------------+                   |
|  | BronzeSubscriber  |    | SilverSubscriber  |                   |
|  | - Owns Parquet    |    | - Owns TimescaleDB|                   |
|  |   writer state    |    |   writer state    |                   |
|  | - Requires        |    | - Requires        |                   |
|  |   coordinator     |    |   coordinator     |                   |
|  |   refactoring     |    |   refactoring     |                   |
|  +-------------------+    +-------------------+                   |
|                                                                   |
+------------------------------------------------------------------+
```

---

## Schema Migration System (Phase 5)

### Version Progression

```
v1.0 (YAML)         v1.1 (dp-018)           v2.0 (dp-021)
---------------     ------------------       ------------------
config_version: 1   config_version: 1.1      config_version: 2

entity_schemas:     entity_schemas:          entity_schemas:
  REQUIRED            DEPRECATED               FORBIDDEN

fields:             fields:                  fields:
  Basic               Enriched (desc,          Enriched
                      device_class)            REQUIRED

Status: RETIRED     Status: CURRENT          Status: TARGET
```

### Migration Script Architecture

```
+------------------------------------------------------------------+
|                 scripts/ndp-migrate-config.sh                     |
+------------------------------------------------------------------+
|                                                                   |
|  Input: Stream config.json (v1.1)                                 |
|                                                                   |
|  +------------------------------------------------------------+  |
|  | 1. Version Detection                                        |  |
|  |    jq '.config_version // 1'                                |  |
|  +------------------------------------------------------------+  |
|         |                                                         |
|         v                                                         |
|  +------------------------------------------------------------+  |
|  | 2. Validation                                               |  |
|  |    - Check all entity_schemas have corresponding fields     |  |
|  |    - Check fields have required enrichment                  |  |
|  +------------------------------------------------------------+  |
|         |                                                         |
|         v                                                         |
|  +------------------------------------------------------------+  |
|  | 3. Transform (v1.1 -> v2.0)                                 |  |
|  |    jq 'del(.entity_schemas) | .config_version = 2'          |  |
|  +------------------------------------------------------------+  |
|         |                                                         |
|         v                                                         |
|  +------------------------------------------------------------+  |
|  | 4. Schema Validation                                        |  |
|  |    Validate output against stream-config.v2.schema.json     |  |
|  +------------------------------------------------------------+  |
|         |                                                         |
|         v                                                         |
|  Output: Stream config.json (v2.0)                                |
|                                                                   |
+------------------------------------------------------------------+
```

### Migration Workflow

```
                     DRY-RUN MODE
                     (--dry-run)
                          |
                          v
+------------------------------------------------------------------+
|  ndp-migrate-config --from 1.1 --to 2 --dry-run                   |
+------------------------------------------------------------------+
|                                                                   |
|  For each config/base/streams/*/config.json:                      |
|    1. Load JSON                                                   |
|    2. Check version == 1.1                                        |
|    3. Generate v2.0 output (in memory)                            |
|    4. Validate against v2.0 schema                                |
|    5. Print diff (do not write)                                   |
|                                                                   |
+------------------------------------------------------------------+


                     APPLY MODE
                     (default)
                          |
                          v
+------------------------------------------------------------------+
|  ndp-migrate-config --from 1.1 --to 2                             |
+------------------------------------------------------------------+
|                                                                   |
|  For each config/base/streams/*/config.json:                      |
|    1. Load JSON                                                   |
|    2. Check version == 1.1                                        |
|    3. Create backup: config.json.v1.1.bak                         |
|    4. Transform to v2.0                                           |
|    5. Validate against v2.0 schema                                |
|    6. Write config.json                                           |
|    7. Log success                                                 |
|                                                                   |
+------------------------------------------------------------------+
```

---

## Release Methodology System (Phase R)

### Release Artifacts

```
Git Repository
|
+-- .deploy/
|   +-- releases/
|   |   +-- v1.0.0.manifest.json    <-- First release
|   |   +-- v1.1.0.manifest.json    <-- Added weather stream
|   |   +-- v1.1.1.manifest.json    <-- Bug fix
|   |   +-- v1.2.0.manifest.json    <-- Hot-reload feature
|   |   +-- v2.0.0.manifest.json    <-- Breaking: v2.0 schema
|   |   +-- TEMPLATE.manifest.json  <-- Template for new releases
|   +-- schemas/
|       +-- manifest.schema.json
|
+-- config/
|   +-- base/
|       +-- streams/
|           +-- air-quality/config.json
|           +-- outdoor-weather/config.json
|
+-- CHANGELOG.md
```

### Manifest-Version Alignment

```
+------------------------------------------------------------------+
|                    Release Alignment                              |
+------------------------------------------------------------------+

Git Tag                    Manifest                  Device State
---------                  --------                  ------------
v1.2.0  <----------------> v1.2.0.manifest.json <--> /var/ndp/deployed-version
   |                            |                           |
   |                            |                           |
   +-- Immutable                +-- Declares changes        +-- Queryable
   +-- Semantic version         +-- release_version: 1.2.0  +-- Current state
   +-- Points to commit         +-- description             +-- When deployed

Constraint: Git tag name MUST match manifest filename and release_version field
```

### Release Workflow

```
Developer Workstation                      Pi Device
        |                                      |
        |  1. Create/modify configs            |
        |                                      |
        |  2. Create release manifest:         |
        |     .deploy/releases/v1.2.0.manifest.json
        |     {                                |
        |       "release_version": "1.2.0",    |
        |       "description": "Add feature",  |
        |       "changes": [...]               |
        |     }                                |
        |                                      |
        |  3. Update CHANGELOG.md              |
        |                                      |
        |  4. Commit                           |
        |     git add .                        |
        |     git commit -m "Release v1.2.0"   |
        |                                      |
        |  5. Tag                              |
        |     git tag v1.2.0                   |
        |                                      |
        |  6. Push                             |
        |     git push origin main --tags      |
        |         |                            |
        |         +--------------------------->|
        |                                      |
        |                              7. Pull |
        |                                 git pull
        |                                      |
        |                              8. Deploy
        |                                 ./deploy.sh apply \
        |                                   .deploy/releases/v1.2.0.manifest.json
        |                                      |
        |                              9. Device state updated
        |                                 /var/ndp/deployed-version = v1.2.0
        |                                 /var/ndp/deployed-at = 2026-02-02T...
        |                                      |
        |<------------------------------------|
        |         Deployment complete          |
```

### Semantic Versioning Rules

```
MAJOR.MINOR.PATCH

+------------------------------------------------------------------+
| MAJOR (breaking change)                                           |
+------------------------------------------------------------------+
| - Config schema version change (v1.1 -> v2.0)                     |
| - API contract changes                                            |
| - Removed streams or fields                                       |
| Example: 1.x.x -> 2.0.0                                           |
+------------------------------------------------------------------+

+------------------------------------------------------------------+
| MINOR (new feature, backward compatible)                          |
+------------------------------------------------------------------+
| - New stream added                                                |
| - New field added to existing stream                              |
| - New MCP tool                                                    |
| - Hot-reload capability                                           |
| Example: 1.2.x -> 1.3.0                                           |
+------------------------------------------------------------------+

+------------------------------------------------------------------+
| PATCH (bug fix, backward compatible)                              |
+------------------------------------------------------------------+
| - Config value corrections                                        |
| - DQ rule fixes                                                   |
| - Documentation updates                                           |
| Example: 1.2.3 -> 1.2.4                                           |
+------------------------------------------------------------------+
```

---

## Manifest Schema Evolution

### Current (dp-020)

```json
{
  "$schema": "./schemas/manifest.schema.json",
  "version": "1.0",
  "changes": [...]
}
```

### Extended (dp-021)

```json
{
  "$schema": "./schemas/manifest.schema.json",
  "version": "1.1",
  "release_version": "1.2.0",
  "description": "Release v1.2.0: Add weather stream, enable hot-reload",
  "changes": [...]
}
```

The `release_version` field:
- Aligns with git tag
- Recorded in `/var/ndp/deployed-version`
- Required for release manifests
- Optional for ad-hoc manifests

---

## Webhook Foundation (Future dp-023)

```
+------------------------------------------------------------------+
|                   Webhook Architecture (dp-023)                   |
+------------------------------------------------------------------+

GitHub                                     Pi Device
   |                                           |
   | Tag push: v1.2.0                          |
   |                                           |
   | POST /webhooks/deploy                     |
   | {                                         |
   |   "ref": "refs/tags/v1.2.0",             |
   |   "repository": {...}                     |
   | }                                         |
   +------------------------------------------>|
   |                                           |
   |                                   +-------v--------+
   |                                   | Webhook Server |
   |                                   | (Rust, future) |
   |                                   +-------+--------+
   |                                           |
   |                                   1. Verify signature
   |                                   2. Extract tag: v1.2.0
   |                                   3. git pull
   |                                   4. Locate manifest:
   |                                      .deploy/releases/v1.2.0.manifest.json
   |                                   5. ./deploy.sh apply <manifest>
   |                                   6. Report status
   |                                           |
   |<------------------------------------------+
   |  Status: success/failure                  |
```

dp-021 establishes the foundation (manifest naming, version alignment) for dp-023 to implement.

---

## Integration Points

### With dp-019 (Validation)

```
Hot-Reload Flow:
  etcd change -> Parse JSON -> dp-019 Validator -> SourceManager
                                      |
                                 REJECT if invalid
                                 (keep current config)
```

### With dp-020 (Declarative Deploy)

```
Release Manifest:
  deploy.sh apply .deploy/releases/v1.2.0.manifest.json
           |
           v
  (dp-020 9-phase orchestration)
           |
           v
  (dp-021 device state update with release_version)
```

### With etcd Watch (Existing)

```
air-quality-app already has:
  - etcd client
  - Watch capability
  - ConfigSyncService

dp-021 adds:
  - SourceManager::on_config_change()
  - Graceful source reconnection
```

---

## Error Handling

### Hot-Reload Errors

| Error | Handling | Recovery |
|-------|----------|----------|
| Parse error | Log, keep current config | Admin fixes config |
| Validation error | Log, keep current config | Admin fixes config |
| MQTT connect fail | Log, retry with backoff | Eventual reconnect |
| HTTP poll fail | Log, retry with backoff | Eventual reconnect |

### Migration Errors

| Error | Handling | Recovery |
|-------|----------|----------|
| Validation fail | Abort migration | Fix source config |
| Write fail | Restore from backup | Manual intervention |
| Schema mismatch | Abort with details | Update migration logic |

### Release Errors

| Error | Handling | Recovery |
|-------|----------|----------|
| Missing manifest | Error, list available | Create manifest |
| Version mismatch | Error, show expected | Fix tag/manifest |
| Deploy fail | Partial state | Fix and re-deploy |

---

## Related ADRs

- **ADR-021-001**: Hot-Reload Scope (Sources Only)
- **ADR-021-002**: Schema Migration Approach (Shell+jq)
- **ADR-021-003**: Release Methodology (SemVer + Manifest Alignment)
- **ADR-016-001**: JSON Source of Truth (parent)
- **ADR-016-002**: Declarative Deploy (parent)
- **ADR-020-001**: Extensible Handlers (dp-020)

---

## References

- `/workspaces/neural-data-platform/product/features/dp-021/SCOPE.md` - Feature requirements
- `/workspaces/neural-data-platform/product/features/dp-016/IMPLEMENTATION-ROADMAP.md` - Parent roadmap
- `/workspaces/neural-data-platform/product/features/dp-020/architecture/ARCHITECTURE.md` - Deploy architecture

---

*Architecture document created: 2026-02-02*
*Feature: dp-021 Config Lifecycle & Release Management*
