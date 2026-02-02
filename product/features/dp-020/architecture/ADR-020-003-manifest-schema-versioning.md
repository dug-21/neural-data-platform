# ADR-020-003: Manifest Schema Versioning

**Status**: Proposed
**Date**: 2026-02-02
**Decision Makers**: NDP Architecture Team
**Feature**: dp-020 Declarative Deploy

---

## Context

The deployment manifest (`.deploy/manifest.json`) is a key interface between agents and the deployment system. As NDP evolves, the manifest schema will need to evolve to support:

1. New declaration types (e.g., `gold-table`, `alert-rule`)
2. New fields on existing types (e.g., `rollback_on_error` for migrations)
3. Renamed or deprecated fields
4. Structural changes

The schema versioning strategy must balance:
- **Backward compatibility** - Old manifests should work with new deploy.sh
- **Forward compatibility** - New manifests should gracefully degrade with old deploy.sh
- **Simplicity** - Operators shouldn't need complex migration tools
- **Explicit evolution** - Changes should be visible and documented

### Related Versioning in NDP

| Artifact | Versioning Strategy |
|----------|---------------------|
| Stream config | `config_version` field (currently 1) |
| JSON Schema | `schemas/stream-config.v1.1.schema.json` (file version) |
| Git releases | Tags (v1.0.0, v1.1.0) |
| Manifest | TBD (this ADR) |

---

## Decision

**Use a `version` field in the manifest with major.minor semver, and design for backward compatibility within the same major version.**

### Manifest Structure

```json
{
  "$schema": "./schemas/manifest.schema.json",
  "version": "1.0",
  "changes": [
    { "type": "stream", "id": "air-quality", "action": "update" },
    { "type": "silver-table", "stream_id": "air-quality", "action": "sync" }
  ]
}
```

### Version Field Semantics

| Version | Meaning |
|---------|---------|
| `1.0` | Initial schema |
| `1.1` | Minor addition (new optional fields, new declaration types) |
| `1.2` | Minor addition (more optional fields) |
| `2.0` | Breaking change (structural change, required field change) |

### Compatibility Rules

#### Within Same Major Version (1.x)

- **New optional fields**: Ignored by old deploy.sh
- **New declaration types**: Logged as warning, skipped by old deploy.sh
- **Removed fields**: Old deploy.sh continues to use defaults

#### Across Major Versions (1.x -> 2.x)

- **Breaking changes allowed**: Field renames, structural changes
- **Migration path required**: Document upgrade steps
- **Explicit opt-in**: deploy.sh checks major version before processing

### Implementation

```bash
# Version handling in deploy.sh

SUPPORTED_MANIFEST_VERSION_MAJOR=1
SUPPORTED_MANIFEST_VERSION_MINOR=2  # Supports 1.0, 1.1, 1.2

validate_manifest_version() {
    local manifest_file="$1"

    # Extract version
    local version=$(jq -r '.version // "1.0"' "$manifest_file")
    local major="${version%%.*}"
    local minor="${version#*.}"

    # Check major version compatibility
    if [ "$major" -gt "$SUPPORTED_MANIFEST_VERSION_MAJOR" ]; then
        error "Manifest version $version requires deploy.sh upgrade"
        error "This deploy.sh supports manifest version $SUPPORTED_MANIFEST_VERSION_MAJOR.x"
        return 1
    fi

    # Check minimum version
    if [ "$major" -lt 1 ]; then
        error "Invalid manifest version: $version"
        return 1
    fi

    # Warn on newer minor version
    if [ "$major" -eq "$SUPPORTED_MANIFEST_VERSION_MAJOR" ] && \
       [ "$minor" -gt "$SUPPORTED_MANIFEST_VERSION_MINOR" ]; then
        warn "Manifest version $version is newer than supported $SUPPORTED_MANIFEST_VERSION_MAJOR.$SUPPORTED_MANIFEST_VERSION_MINOR"
        warn "Some features may be ignored"
    fi

    log "Manifest version: $version (supported)"
    return 0
}
```

---

## Schema Evolution Examples

### Example 1: Adding a New Declaration Type (Minor Version Bump)

**Version 1.0**: Supports `stream`, `silver-table`, `migration`, `dimensions`, `dictionary`

**Version 1.1**: Adds `gold-table` type

```json
{
  "version": "1.1",
  "changes": [
    { "type": "gold-table", "id": "hourly-aqi", "action": "sync" }
  ]
}
```

**Old deploy.sh (1.0) behavior**:
```
[WARN] Unknown declaration type: gold-table (skipping)
```

**New deploy.sh (1.1) behavior**:
- Processes `gold-table` declaration

### Example 2: Adding Optional Field to Existing Type (Minor Version Bump)

**Version 1.0**: `stream` has `id`, `action`, `reload`

**Version 1.2**: `stream` adds `validate_only` field

```json
{
  "version": "1.2",
  "changes": [
    { "type": "stream", "id": "air-quality", "action": "update", "validate_only": true }
  ]
}
```

**Old deploy.sh (1.0) behavior**:
- Ignores `validate_only` field
- Processes normally (full sync)

**New deploy.sh (1.2) behavior**:
- Respects `validate_only: true`
- Validates without syncing

### Example 3: Breaking Change (Major Version Bump)

**Version 1.x**: `silver-table` uses `stream_id` to reference config

**Version 2.0**: `silver-table` uses `config_path` for explicit path

```json
// Version 1.x
{ "type": "silver-table", "stream_id": "air-quality", "action": "sync" }

// Version 2.0
{ "type": "silver-table", "config_path": "config/base/streams/air-quality/config.json", "action": "sync" }
```

**Migration path**:
1. Deploy v2 deploy.sh
2. Update manifest to version 2.0
3. Change `stream_id` to `config_path`

---

## JSON Schema Versioning

The manifest JSON Schema file follows the version:

```
schemas/
    manifest.v1.0.schema.json   # Initial schema
    manifest.v1.1.schema.json   # Added gold-table
    manifest.v1.2.schema.json   # Added validate_only
    manifest.schema.json        # Symlink to latest 1.x
```

### Schema Reference in Manifest

```json
{
  "$schema": "./schemas/manifest.schema.json",
  "version": "1.0",
  ...
}
```

The `$schema` field points to the latest compatible schema, while `version` indicates the manifest's actual version.

---

## Backward Compatibility Mechanisms

### 1. Unknown Fields Ignored (via JSON Schema)

```json
// schemas/manifest.schema.json
{
  "additionalProperties": false,  // At declaration type level
  // BUT schema uses oneOf, allowing future types
}
```

### 2. Unknown Declaration Types Skipped

```bash
dispatch_declaration() {
    local type=$(echo "$declaration" | jq -r '.type')

    case "$type" in
        stream|silver-table|migration|dimensions|dictionary)
            handle_$type "$declaration"
            ;;
        *)
            warn "Unknown declaration type: $type (skipping)"
            # Return success - don't fail on unknown types
            return 0
            ;;
    esac
}
```

### 3. Missing Optional Fields Use Defaults

```bash
handle_stream() {
    local json="$1"

    # Use defaults for optional fields
    local action=$(echo "$json" | jq -r '.action // "update"')
    local reload=$(echo "$json" | jq -r '.reload // "none"')
    local validate_only=$(echo "$json" | jq -r '.validate_only // false')

    # ...
}
```

### 4. Version-Specific Logic

```bash
handle_migration() {
    local json="$1"
    local version=$(echo "$MANIFEST_VERSION" | cut -d. -f1,2)

    # Feature added in 1.2
    if version_gte "$version" "1.2"; then
        local on_error=$(echo "$json" | jq -r '.on_error // "abort"')
        # Use on_error field
    else
        local on_error="abort"  # Default for older versions
    fi
}
```

---

## Consequences

### Positive

1. **Clear evolution path** - Version field makes compatibility explicit
2. **Graceful degradation** - Old deploy.sh handles new manifests safely
3. **No migration tool** - Minor versions just work
4. **Self-documenting** - Version in manifest indicates capabilities

### Negative

1. **Version maintenance** - Must update version when adding features
2. **Documentation burden** - Each version needs changelog
3. **Testing complexity** - Must test backward compatibility

### Neutral

1. **Schema file proliferation** - One file per version (can use symlinks)
2. **Error messages** - Must provide helpful upgrade guidance

---

## Alternatives Considered

### Alternative 1: No Version Field

Rely entirely on field defaults and ignore unknown fields.

**Rejected because**:
- Breaking changes would silently fail
- No way to detect incompatibility
- Hard to communicate required deploy.sh version

### Alternative 2: Git Tag as Version

Use git tag of the repo as manifest version.

**Rejected because**:
- Conflates platform version with manifest schema version
- Manifest format can evolve independently of code
- Harder to reason about compatibility

### Alternative 3: Full Semver (X.Y.Z)

Use three-part version (1.0.0, 1.0.1, 1.1.0).

**Rejected because**:
- Patch version adds no meaningful information for schema
- Simpler is better for operator understanding
- Two-part (major.minor) is sufficient

### Alternative 4: Date-Based Versioning (2026-02)

Use year-month as version identifier.

**Rejected because**:
- Doesn't communicate compatibility relationship
- Harder to determine if upgrade is breaking
- Semver is industry standard for APIs

---

## Version History Tracking

| Version | Date | Changes | Breaking |
|---------|------|---------|----------|
| 1.0 | 2026-02 | Initial release: stream, silver-table, migration, dimensions, dictionary | N/A |
| 1.1 | TBD | (Reserved for future additions) | No |
| 2.0 | TBD | (Reserved for structural changes) | Yes |

---

## Implementation Notes

### Manifest Template (.deploy/manifest.json)

```json
{
  "$schema": "./schemas/manifest.schema.json",
  "version": "1.0",
  "changes": []
}
```

### Version Check at Deploy Start

```bash
apply() {
    local manifest_file="${REPO_ROOT}/.deploy/manifest.json"

    # Phase 0: Version check
    log "Checking manifest version..."
    if ! validate_manifest_version "$manifest_file"; then
        error "Manifest version incompatible - upgrade deploy.sh or downgrade manifest"
        exit 1
    fi

    # Store version for handler use
    export MANIFEST_VERSION=$(jq -r '.version' "$manifest_file")

    # Continue with phases...
}
```

### Documentation Requirements

Each version bump requires:
1. Update `SUPPORTED_MANIFEST_VERSION_MINOR` in deploy.sh
2. Create new schema file (if structural changes)
3. Update version history table in this ADR
4. Document in CHANGELOG.md

---

## Related Decisions

- **ADR-020-001**: Extensible Handler Architecture (handler dispatch)
- **ADR-020-002**: DDL Generation Strategy (silver-table handler)
- **ADR-016-002**: Declarative Deploy (parent decision)

---

## References

- `/workspaces/neural-data-platform/product/features/dp-020/SCOPE.md` - Feature requirements
- `/workspaces/neural-data-platform/schemas/stream-config.v1.1.schema.json` - Example of versioned schema
- [Semantic Versioning 2.0.0](https://semver.org/) - Versioning standard

---

*ADR created: 2026-02-02*
*Feature: dp-020 Declarative Deploy*
