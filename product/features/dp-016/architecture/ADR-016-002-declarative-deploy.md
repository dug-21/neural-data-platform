# ADR-016-002: Declarative Deploy Architecture

**Status**: Accepted
**Date**: 2026-02-01
**Decision Makers**: Human + AI Architecture Review
**Feature**: dp-016 Configuration Architecture Review

---

## Context

Adding a new stream to NDP currently requires 8+ manual steps with 12+ failure points. Agents and operators must know the correct sequence: create config, write DDL, run migrations, sync to etcd, sync dictionary, sync dimensions, restart app. Missing or misordering steps causes silent failures.

**Problems with current approach**:
1. **Error-prone** - Easy to forget steps or run in wrong order
2. **Full reload risk** - Syncing everything can break working streams
3. **No isolation** - One bad config can take down unrelated streams
4. **Agent complexity** - Agents must know deployment internals
5. **No versioning** - No clear release/rollback mechanism

---

## Decision

**Implement Declarative Deploy with a manifest file.**

Agents declare what changed in a manifest. Deploy reads the manifest and executes only the necessary actions in the correct order. This inverts the responsibility: agents declare intent, deploy handles orchestration.

### Manifest Structure

The manifest is **part of the release** - it's versioned in git alongside the config it describes. Each release has its own manifest defining how to deploy that version.

```json
{
  "config_version": 1,
  "changes": [
    {
      "type": "stream",
      "id": "air-quality",
      "reload": "sources"
    },
    {
      "type": "stream",
      "id": "new-sensor",
      "reload": "full"
    },
    {
      "type": "silver-table",
      "stream": "new-sensor",
      "action": "create"
    },
    {
      "type": "migration",
      "file": "003_add_indexes.sql"
    },
    {
      "type": "dimensions",
      "id": "entity_context"
    },
    {
      "type": "dictionary",
      "description": "Sync data dictionary from config"
    }
  ]
}
```

**Manifest file**: `.deploy/manifest.json`

### Release Model

**Git is the release system.** Each commit/tag represents a release:

```
Git tag v1.2.0 (commit abc123)
├── config/base/streams/...      # Config AT this version (JSON files)
├── .deploy/manifest.json        # How to deploy THIS version
└── deploy/migrations/...        # Migrations for this version
```

- **v1.0.0** has its manifest (initial deployment)
- **v1.1.0** has its manifest (changes from v1.0.0)
- **v1.2.0** has its manifest (changes from v1.1.0)

**No separate versioning system.** Git tags/commits ARE the versions.

### Device-Local State

Each device tracks its own deployment state locally (NOT in git):

```
/var/ndp/
  deployed-version     # "v1.2.0" or commit SHA
  deployed-at          # timestamp of last deploy
```

This is device-specific operational state, not config.

### Deploy Workflow

```
Development workflow:
  1. Edit config files (streams, dimensions, migrations) - all JSON
  2. Update .deploy/manifest.json declaring what this release changes
  3. git add . && git commit -m "feat: add new-sensor stream"
  4. git tag v1.3.0 (optional, for milestone releases)
  5. git push && git push --tags

Device deployment (webhook or manual):
  1. git pull (or git checkout v1.3.0)
  2. Read .deploy/manifest.json
  3. Validate all declared changes
     - Parse JSON configs (serde_json - fast, strict)
     - Run Validator component (JSON Schema validation)
     - Check cross-references
  4. If validation fails → abort, report errors
  5. Execute changes in correct order:
     a. Migrations (silver-migrate)
     b. Silver tables (generate DDL, apply)
     c. Streams (sync JSON to etcd natively)
     d. Dictionary (sync to TimescaleDB)
     e. Dimensions (sync to TimescaleDB)
  6. Reload affected streams (sources or full)
  7. Update /var/ndp/deployed-version with current commit/tag
  8. Update /var/ndp/deployed-at with timestamp
```

**Note**: Deploy does NOT commit back to git. Git contains the release definition. Device state is local.

---

## Consequences

### Positive

1. **Agent simplicity** - Agents declare what changed, not how to deploy
2. **Safe incremental deploys** - Only declared changes are applied
3. **Per-stream isolation** - One stream change doesn't affect others
4. **Correct ordering** - Deploy handles dependencies (migrations before streams)
5. **Validation gate** - Bad config caught before affecting running system
6. **Auditable** - Manifest in git, history tracks all releases
7. **Rollback path** - History enables reverting to previous release
8. **Extensible** - New change types easy to add

### Negative

1. **New component** - Must build deploy orchestration logic
2. **Learning curve** - Agents must learn manifest format

### Neutral

1. **Manifest validation** - Need schema for manifest itself
2. **Webhook integration** - Optional automation on git push
3. **Device state separate** - Must manage /var/ndp/ on each device

---

## Declaration Types

| Type | Description | Actions |
|------|-------------|---------|
| `stream` | Stream config changed | validate → sync to etcd → reload |
| `silver-table` | Silver table needs DDL | generate DDL → apply to TimescaleDB |
| `migration` | SQL migration file | apply via silver-migrate |
| `dimensions` | Dimension data changed | sync CSV to TimescaleDB |
| `dictionary` | Data dictionary refresh | sync from config to TimescaleDB |

### Stream Reload Options

| Option | Behavior |
|--------|----------|
| `sources` | Hot-reload MQTT/HTTP sources (no restart) |
| `full` | Full app restart for this stream |
| `none` | Sync config only, no reload |

### Silver Table Actions

| Action | Behavior |
|--------|----------|
| `create` | Generate DDL from silver_etl, apply |
| `validate-only` | Check table exists, fail if missing |

---

## Validator Component

A dedicated Validator component gates all deployments:

```
Validator responsibilities:
  - JSON syntax validation (strict parsing, no ambiguity)
  - JSON Schema validation (mature tooling, IDE integration)
  - Keyword/type validation (enforced by schema)
  - Cross-reference validation (source_path vs fields)
  - Unknown field detection (JSON Schema additionalProperties: false)
  - Silver table existence check
  - DQ rule syntax validation

Available at:
  - Deploy time (gate deployment)
  - Runtime startup (defensive check)
  - MCP write operations (validate before save - JSON is MCP-native)
```

**JSON Schema Benefits**:
- IDE autocomplete and inline validation
- Pre-deploy validation catches errors early
- MCP tools can validate without conversion
- Agent output verification is reliable (strict format)

---

## Versioning Strategy

### Git Tags = Platform Versions

Git tags are the versioning system. No separate version fields needed.

```bash
git tag v1.0.0   # Initial release
git tag v1.1.0   # New features
git tag v1.2.0   # More features
git tag v1.2.1   # Bug fix
```

Semantic versioning via git tags:
- **Major** (v2.0.0): Breaking changes
- **Minor** (v1.1.0): New features, backward compatible
- **Patch** (v1.0.1): Bug fixes

### Config Schema Version

Each config file has `config_version` indicating its schema structure:

```json
{
  "config_version": 2,
  "stream_id": "air-quality",
  "description": "Air quality measurements from AirGradient sensors",
  "fields": [...]
}
```

- App/Validator only supports current schema version
- Migration tool transforms configs between schema versions
- This is separate from platform versioning (git tags)
- JSON format ensures strict, unambiguous versioning

### Manifest Schema Version

The manifest itself has a schema version:

```json
{
  "config_version": 1,
  "changes": [
    {"type": "stream", "id": "...", "reload": "..."}
  ]
}
```

This allows manifest format to evolve independently. JSON Schema (`manifest.schema.json`) validates the manifest structure.

---

## Example: Adding a New Stream

**Agent creates config**:
```
config/base/streams/new-sensor/config.json
```

**Agent updates manifest**:
```json
{
  "config_version": 1,
  "changes": [
    {
      "type": "stream",
      "id": "new-sensor",
      "reload": "full"
    },
    {
      "type": "silver-table",
      "stream": "new-sensor",
      "action": "create"
    },
    {
      "type": "dictionary",
      "description": "Sync data dictionary for new-sensor"
    }
  ]
}
```

**Agent commits and tags**:
```bash
git add config/base/streams/new-sensor/ .deploy/manifest.json
git commit -m "feat: Add new-sensor stream"
git tag v1.3.0
git push && git push --tags
```

**Device deploys** (webhook or manual):
```bash
# On Pi
git pull
./deploy.sh apply
```

**Deploy executes**:
1. Reads manifest.json from current commit (native JSON parsing)
2. Validates new-sensor config against JSON Schema
3. Generates Silver DDL from silver_etl
4. Applies DDL to TimescaleDB
5. Syncs new-sensor JSON to etcd (native format)
6. Syncs data dictionary
7. Restarts app (full reload for new-sensor)
8. Updates /var/ndp/deployed-version to current commit

---

## Rollback Strategy

Git is the rollback mechanism:

```bash
# View release history
git log --oneline --tags

# Check what's deployed on this device
cat /var/ndp/deployed-version

# Rollback to previous release
git checkout v1.2.0
./deploy.sh apply

# This executes v1.2.0's manifest.json:
# - Syncs v1.2.0 JSON config to etcd
# - Re-applies DDL if needed (idempotent)
# - Restarts app
# - Updates /var/ndp/deployed-version to v1.2.0
```

## Rebuild From Scratch

Deploy any version sequence to rebuild a device:

```bash
# Fresh device - deploy from scratch
git checkout v1.0.0 && ./deploy.sh apply
git checkout v1.1.0 && ./deploy.sh apply  # includes migrations
git checkout v1.2.0 && ./deploy.sh apply  # includes migrations

# Or jump directly to latest (must handle all migrations)
git checkout v1.2.0 && ./deploy.sh apply --full
```

Each release's manifest is the complete deployment instruction for that version.

---

## Implementation Phases

| Phase | Scope | Dependencies |
|-------|-------|--------------|
| 1 | Manifest schema + basic deploy.sh integration | None |
| 2 | Validator component (Rust, JSON Schema) | Phase 1 |
| 3 | Per-stream etcd sync (blob storage) | Phase 1, ADR-016-001 |
| 4 | Silver DDL generation from config | Phase 1, dp-015 |
| 5 | Source hot-reload wiring | Phase 3 |
| 6 | History + rollback | Phase 1 |

---

## Alternatives Considered

### Alternative 1: Git Diff Parsing

Deploy parses `git log` to determine what changed.

**Rejected because**:
- Complex to parse reliably
- Agents can't express intent (reload type, actions)
- No explicit versioning

### Alternative 2: Implicit Detection

Deploy scans all files, detects changes via checksums.

**Rejected because**:
- Still requires understanding what actions each file type needs
- No explicit declaration of intent
- Harder to audit

### Alternative 3: CI/CD Pipeline Only

Use GitHub Actions or similar for all deployment logic.

**Rejected because**:
- Pi deployment is local, not cloud-triggered
- Adds external dependency
- Manifest approach works with or without CI/CD

---

## Related Decisions

- **ADR-016-001**: Config Source of Truth (JSON primary, etcd cache, JSON as platform standard)
- **Q2**: Per-stream isolation
- **Q4**: Explicit silver-table declaration
- **Q5**: Hot-reload scope (sources + full)
- **Q8**: Config schema versioning (breaking + migration tool)

---

## References

- `product/features/dp-016/architecture/DECISION-QUESTIONS.md` - Full decision log
- `product/features/dp-016/specification/AS-IS-PROCESS.md` - Current 8-step manual process
- `product/features/dp-016/specification/PAIN-POINTS.md` - P-012 (manual DDL), P-015 (manual deployment)
