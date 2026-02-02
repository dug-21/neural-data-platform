# ADR-021-003: Release Methodology - SemVer + Manifest Alignment + Git Tags

**Status**: Proposed
**Date**: 2026-02-02
**Decision Makers**: NDP Architecture Team
**Feature**: dp-021 Config Lifecycle & Release Management

---

## Context

With dp-020 establishing declarative deployment via manifests, a gap remains: there is no formal connection between code versions, manifest files, and what is actually deployed on devices. Operators cannot easily answer:

- "What version is deployed on the Pi?"
- "What changed between v1.1.0 and v1.2.0?"
- "Which manifest corresponds to git tag v1.2.0?"

Additionally, the forthcoming dp-023 (Webhook-Triggered Deployment) needs a predictable mapping from git tags to deployment manifests.

### Current State

```
Git                          Manifests                    Device
---                          ---------                    ------
No formal tags               .deploy/manifest.json        No version tracking
                             (single file, overwritten)
```

### Desired State

```
Git                          Manifests                         Device
---                          ---------                         ------
v1.0.0                  <--> .deploy/releases/v1.0.0.manifest.json
v1.1.0                  <--> .deploy/releases/v1.1.0.manifest.json
v1.2.0                  <--> .deploy/releases/v1.2.0.manifest.json
                                      |
                                      v
                             /var/ndp/deployed-version = v1.2.0
```

---

## Decision

**Adopt Semantic Versioning 2.0.0 with strict alignment between git tags, manifest filenames, and device-tracked versions. Manifests live in `.deploy/releases/` with names matching their version.**

### Three-Way Alignment

```
                   ALIGNMENT RULE
+--------------------------------------------------+
|                                                  |
|   Git Tag       Manifest Filename      Device    |
|   -------       -----------------      ------    |
|   v1.2.0   =    v1.2.0.manifest.json = v1.2.0   |
|                                                  |
+--------------------------------------------------+
```

### Semantic Versioning Rules for NDP

Following [SemVer 2.0.0](https://semver.org/):

```
MAJOR.MINOR.PATCH

MAJOR - Incompatible changes
MINOR - Backward-compatible features
PATCH - Backward-compatible fixes
```

#### NDP-Specific Version Bump Rules

| Change Type | Version Bump | Examples |
|-------------|--------------|----------|
| **Config schema change** | MAJOR | v1.1 -> v2.0 schema, remove field |
| **API breaking change** | MAJOR | MCP tool signature change |
| **New stream** | MINOR | Add weather-station stream |
| **New field** | MINOR | Add humidity to air-quality |
| **New MCP tool** | MINOR | Add reload_stream tool |
| **Config value fix** | PATCH | Correct DQ threshold |
| **Documentation** | PATCH | Update CHANGELOG |
| **Bug fix** | PATCH | Fix validation edge case |

### Release Artifacts

Each release creates/updates these artifacts:

#### 1. Release Manifest

Location: `.deploy/releases/v{MAJOR}.{MINOR}.{PATCH}.manifest.json`

```json
{
  "$schema": "../schemas/manifest.schema.json",
  "version": "1.1",
  "release_version": "1.2.0",
  "description": "Release v1.2.0: Add weather-station stream",
  "released_at": "2026-02-02T10:30:00Z",
  "changes": [
    {
      "type": "stream",
      "id": "weather-station",
      "action": "create"
    },
    {
      "type": "silver-table",
      "stream_id": "weather-station",
      "action": "create"
    },
    {
      "type": "dictionary",
      "action": "sync"
    },
    {
      "type": "container-restart",
      "target": "air-quality-app"
    }
  ]
}
```

#### 2. Git Tag

```bash
git tag -a v1.2.0 -m "Release v1.2.0: Add weather-station stream"
```

Tag message should match manifest description.

#### 3. CHANGELOG Entry

```markdown
## [1.2.0] - 2026-02-02

### Added
- weather-station stream with temperature, humidity, pressure fields
- Silver table `silver.weather_readings`

### Changed
- Updated data dictionary with weather fields
```

#### 4. Device State Files

After deployment:
- `/var/ndp/deployed-version` = `v1.2.0`
- `/var/ndp/deployed-at` = `2026-02-02T10:35:22Z`
- `/var/ndp/manifest-applied` = SHA256 of manifest

### Release Template

`.deploy/releases/TEMPLATE.manifest.json`:

```json
{
  "$schema": "../schemas/manifest.schema.json",
  "version": "1.1",
  "release_version": "X.Y.Z",
  "description": "Release vX.Y.Z: Brief description",
  "released_at": "YYYY-MM-DDTHH:MM:SSZ",
  "changes": [
    {
      "type": "stream|silver-table|migration|dimensions|dictionary|container-build|container-restart",
      "id": "stream-id or omit",
      "action": "create|update|sync|delete"
    }
  ]
}
```

---

## Release Workflow

### Standard Release Process

```
1. DEVELOP
   - Make changes to configs, code
   - Test in integration environment

2. PREPARE RELEASE
   - Determine version bump (major/minor/patch)
   - Copy TEMPLATE.manifest.json to v{X}.{Y}.{Z}.manifest.json
   - Fill in release_version, description, changes
   - Update CHANGELOG.md

3. COMMIT
   git add .deploy/releases/v1.2.0.manifest.json
   git add CHANGELOG.md
   git commit -m "Release v1.2.0: Add weather-station stream"

4. TAG
   git tag -a v1.2.0 -m "Release v1.2.0: Add weather-station stream"

5. PUSH
   git push origin main --tags

6. DEPLOY (on Pi)
   git pull
   ./deploy.sh apply .deploy/releases/v1.2.0.manifest.json

7. VERIFY
   cat /var/ndp/deployed-version  # Should show v1.2.0
```

### Hotfix Release Process

For urgent fixes to production:

```
1. BRANCH
   git checkout -b hotfix/v1.2.1 v1.2.0

2. FIX
   - Apply minimal fix
   - Create v1.2.1.manifest.json

3. COMMIT & TAG
   git commit -m "Hotfix v1.2.1: Fix DQ threshold"
   git tag -a v1.2.1 -m "Hotfix v1.2.1: Fix DQ threshold"

4. MERGE & PUSH
   git checkout main
   git merge hotfix/v1.2.1
   git push origin main --tags

5. DEPLOY
   ./deploy.sh apply .deploy/releases/v1.2.1.manifest.json
```

---

## Manifest Schema Update

### Current Schema (dp-020)

```json
{
  "version": "1.0",
  "changes": [...]
}
```

### Extended Schema (dp-021)

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "title": "NDP Deployment Manifest v1.1",
  "type": "object",
  "properties": {
    "version": {
      "type": "string",
      "enum": ["1.0", "1.1"],
      "description": "Manifest schema version"
    },
    "release_version": {
      "type": "string",
      "pattern": "^[0-9]+\\.[0-9]+\\.[0-9]+$",
      "description": "Release version (SemVer, without 'v' prefix)"
    },
    "description": {
      "type": "string",
      "description": "Human-readable release description"
    },
    "released_at": {
      "type": "string",
      "format": "date-time",
      "description": "ISO 8601 release timestamp"
    },
    "changes": {
      "type": "array",
      "items": { "$ref": "#/$defs/change" }
    }
  },
  "required": ["version", "changes"],
  "if": {
    "properties": {
      "version": { "const": "1.1" }
    }
  },
  "then": {
    "required": ["version", "release_version", "changes"]
  }
}
```

**Note**: `release_version` is required for v1.1 manifests (release manifests) but optional for v1.0 (ad-hoc manifests).

---

## Device State Tracking

### State Files

| File | Content | Purpose |
|------|---------|---------|
| `/var/ndp/deployed-version` | `v1.2.0` | Current deployed release |
| `/var/ndp/deployed-at` | ISO 8601 timestamp | When deployment completed |
| `/var/ndp/manifest-applied` | SHA256 hash | Detect manifest drift |

### Implementation

```bash
# In deploy.sh apply
update_device_state() {
    local manifest_file="$1"
    local state_dir="/var/ndp"

    mkdir -p "$state_dir"

    # Extract release version from manifest
    local release_version
    release_version=$(jq -r '.release_version // empty' "$manifest_file")

    if [ -z "$release_version" ]; then
        # Fallback to git describe for ad-hoc manifests
        release_version=$(git describe --tags --always 2>/dev/null || echo "unknown")
    fi

    # Write state files
    echo "v${release_version#v}" > "$state_dir/deployed-version"
    date -Iseconds > "$state_dir/deployed-at"
    sha256sum "$manifest_file" | cut -d' ' -f1 > "$state_dir/manifest-applied"

    log "Device state updated: v${release_version#v}"
}
```

### Querying Device State

```bash
# Check current version
cat /var/ndp/deployed-version
# Output: v1.2.0

# Check when deployed
cat /var/ndp/deployed-at
# Output: 2026-02-02T10:35:22+00:00

# Verify manifest integrity
sha256sum .deploy/releases/v1.2.0.manifest.json | cut -d' ' -f1
cat /var/ndp/manifest-applied
# Should match
```

---

## Webhook Foundation (dp-023)

This release methodology establishes the convention for dp-023 webhook-triggered deployment:

```
GitHub Tag Push Event
{
  "ref": "refs/tags/v1.2.0",
  ...
}
        |
        v
Webhook Handler (future)
        |
        +-- Extract version: v1.2.0
        +-- Locate manifest: .deploy/releases/v1.2.0.manifest.json
        +-- Execute: ./deploy.sh apply <manifest>
        +-- Report status to GitHub
```

The predictable mapping from tag to manifest enables automated deployment.

---

## Consequences

### Positive

1. **Clear versioning** - Everyone knows what's deployed
2. **Reproducible deployments** - Tag + manifest = exact state
3. **Audit trail** - Git history + manifests document all changes
4. **Webhook-ready** - Predictable tag-to-manifest mapping
5. **Rollback capability** - Deploy any previous manifest

### Negative

1. **Process overhead** - More steps for each release
2. **Manifest proliferation** - Many files in releases/
3. **Discipline required** - Team must follow process

### Mitigation

| Issue | Mitigation |
|-------|------------|
| Process overhead | Provide release checklist, template |
| Many manifest files | Archive old releases to `releases/archive/` |
| Discipline | CI checks for tag-manifest alignment |

---

## Alternatives Considered

### Alternative 1: Auto-Generate Manifest from Git Diff

Automatically create manifest by analyzing git diff between tags.

**Rejected because**:
- Hard to reliably detect change types from diffs
- May miss semantic changes (config value changes)
- Manual manifest ensures intentional declarations

### Alternative 2: Single Manifest with Version Field

Keep one `manifest.json` and track version within:

```json
{
  "version": "1.2.0",
  "history": [
    {"version": "1.1.0", "changes": [...]},
    {"version": "1.2.0", "changes": [...]}
  ]
}
```

**Rejected because**:
- File grows unboundedly
- Merge conflicts likely with concurrent work
- Harder to find specific version's changes

### Alternative 3: No Version Tracking

Continue current approach - deploy whatever is in git HEAD.

**Rejected because**:
- Cannot answer "what's deployed?"
- No rollback capability
- Webhook deployment impossible without version mapping

---

## Implementation Checklist

### Phase R Tasks

| ID | Task | Deliverable |
|----|------|-------------|
| R.1 | Define versioning standard | `docs/procedures/RELEASE-POLICY.md` |
| R.2 | Formalize manifest naming | Convention documented |
| R.3 | Create release checklist | Documented in RELEASE-POLICY.md |
| R.4 | Align git tags to manifests | Validation in CI (future) |
| R.5 | Add manifest version field | Updated schema |
| R.6 | Device deployed-version tracking | `update_device_state()` in deploy.sh |
| R.7 | Create release template | `.deploy/releases/TEMPLATE.manifest.json` |
| R.8 | Document webhook trigger spec | `docs/procedures/WEBHOOK-DEPLOYMENT-SPEC.md` |

---

## Related Decisions

- **ADR-021-001**: Hot-Reload Scope
- **ADR-021-002**: Schema Migration Approach
- **ADR-020-001**: Extensible Handlers (manifest processing)
- **ADR-020-003**: Manifest Schema Versioning

---

## References

- [Semantic Versioning 2.0.0](https://semver.org/)
- `/workspaces/neural-data-platform/product/features/dp-021/SCOPE.md` - Phase R requirements
- `/workspaces/neural-data-platform/product/features/dp-020/architecture/ARCHITECTURE.md` - Deploy architecture

---

*ADR created: 2026-02-02*
*Feature: dp-021 Config Lifecycle & Release Management*
