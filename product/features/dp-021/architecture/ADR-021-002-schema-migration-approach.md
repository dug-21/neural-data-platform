# ADR-021-002: Schema Migration Approach - Shell+jq for v1.1 to v2.0

**Status**: Proposed
**Date**: 2026-02-02
**Decision Makers**: NDP Architecture Team
**Feature**: dp-021 Config Lifecycle & Release Management

---

## Context

dp-021 Phase 5 requires migrating stream configuration from v1.1 (transitional, with deprecated entity_schemas) to v2.0 (clean, entity_schemas forbidden). This is the first "breaking" schema change in NDP and establishes the migration pattern for future changes.

### Schema Version History

| Version | Format | entity_schemas | Enriched fields | Status |
|---------|--------|----------------|-----------------|--------|
| v1.0 | YAML | Required | Not supported | Retired |
| v1.1 | JSON | Deprecated (optional) | Supported | Current |
| v2.0 | JSON | Forbidden | Required | Target |

### Migration Complexity Analysis

The v1.1 to v2.0 migration is simple:

```
v1.1 Input:
{
  "config_version": 1.1,
  "fields": [
    {"name": "pm25", "type": "float", "description": "PM2.5 reading"}
  ],
  "entity_schemas": [
    {"name": "pm25", "description": "PM2.5 reading", "device_class": "sensor"}
  ]
}

v2.0 Output:
{
  "config_version": 2,
  "fields": [
    {"name": "pm25", "type": "float", "description": "PM2.5 reading"}
  ]
}
```

The transform is:
1. Remove `entity_schemas` section
2. Update `config_version` to 2

This is a deletion operation, not a data transformation. The enriched fields data was already copied during the dp-018 migration (Phase 0).

### Tooling Options

| Option | Complexity | Dependencies | Pi Compatible | Reusability |
|--------|------------|--------------|---------------|-------------|
| Shell+jq | Low | jq (available) | Yes | Migration-specific |
| Rust CLI | Medium | Cross-compile | Yes | Reusable framework |
| Python | Low | Python (not on Pi) | No | N/A |

---

## Decision

**Use shell+jq for the v1.1 to v2.0 migration. Future migrations with complex transforms will use Rust crates callable from CLI and MCP.**

### Rationale

1. **Simplicity matches complexity** - The migration is a simple `jq del(.entity_schemas)`, not a complex transform
2. **No new dependencies** - jq is already available on Pi
3. **One-time operation** - This migration runs once per deployment
4. **Pattern established** - dp-020 uses shell+jq for DDL generation successfully
5. **Future path clear** - Complex migrations warrant Rust; simple ones don't

### Implementation

#### Migration Script: `scripts/ndp-migrate-config.sh`

```bash
#!/usr/bin/env bash
set -euo pipefail

# dp-021: Config schema migration v1.1 -> v2.0
# Removes deprecated entity_schemas section

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CONFIG_DIR="${REPO_ROOT}/config/base/streams"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log() { echo -e "${GREEN}[migrate]${NC} $*"; }
warn() { echo -e "${YELLOW}[migrate]${NC} $*"; }
error() { echo -e "${RED}[migrate]${NC} $*" >&2; }

usage() {
    cat << EOF
Usage: $(basename "$0") [OPTIONS]

Migrate stream configs from v1.1 to v2.0 (removes entity_schemas)

Options:
  --from VERSION    Source version (default: 1.1)
  --to VERSION      Target version (default: 2)
  --dry-run         Preview changes without writing
  --stream ID       Migrate single stream (default: all)
  --help            Show this help

Examples:
  $(basename "$0") --dry-run              # Preview all migrations
  $(basename "$0") --stream air-quality   # Migrate one stream
  $(basename "$0")                        # Migrate all streams
EOF
}

# Transform v1.1 -> v2.0
transform_v1_1_to_v2() {
    local input="$1"
    jq '
        # Remove entity_schemas section
        del(.entity_schemas) |
        # Update version
        .config_version = 2
    ' "$input"
}

# Validate config has required enriched fields
validate_v2_ready() {
    local config_file="$1"
    local stream_id="$2"

    # Check all fields have descriptions (required for v2.0)
    local fields_without_desc
    fields_without_desc=$(jq -r '
        .fields[]
        | select(.description == null or .description == "")
        | .name
    ' "$config_file" | tr '\n' ', ')

    if [ -n "$fields_without_desc" ]; then
        error "Stream $stream_id has fields without description: ${fields_without_desc%,}"
        return 1
    fi

    return 0
}

# Migrate a single config file
migrate_config() {
    local config_file="$1"
    local dry_run="$2"
    local stream_id
    stream_id=$(basename "$(dirname "$config_file")")

    # Check current version
    local current_version
    current_version=$(jq -r '.config_version // 1' "$config_file")

    if [ "$current_version" = "2" ]; then
        log "$stream_id: Already at v2.0, skipping"
        return 0
    fi

    if [ "$current_version" != "1.1" ] && [ "$current_version" != "1" ]; then
        error "$stream_id: Unexpected version $current_version"
        return 1
    fi

    # Validate ready for v2.0
    if ! validate_v2_ready "$config_file" "$stream_id"; then
        error "$stream_id: Not ready for v2.0 migration"
        return 1
    fi

    if [ "$dry_run" = "true" ]; then
        log "$stream_id: Would migrate v$current_version -> v2.0"
        log "  Removing entity_schemas section"

        # Show diff
        local old_size new_size
        old_size=$(jq -c '.' "$config_file" | wc -c)
        new_size=$(transform_v1_1_to_v2 "$config_file" | wc -c)
        log "  Size: $old_size -> $new_size bytes"

        # Show what would be removed
        local entity_count
        entity_count=$(jq -r '.entity_schemas | length // 0' "$config_file")
        if [ "$entity_count" -gt 0 ]; then
            log "  Removing $entity_count entity_schemas entries"
        fi
    else
        log "$stream_id: Migrating v$current_version -> v2.0"

        # Create backup
        local backup_file="${config_file}.v${current_version}.bak"
        cp "$config_file" "$backup_file"
        log "  Backup: $backup_file"

        # Transform and write
        local temp_file="${config_file}.tmp"
        if ! transform_v1_1_to_v2 "$config_file" > "$temp_file"; then
            error "$stream_id: Transform failed"
            rm -f "$temp_file"
            return 1
        fi

        # Validate output against v2.0 schema
        if command -v ndp-validate &> /dev/null; then
            if ! ndp-validate --schema-version 2 "$temp_file"; then
                error "$stream_id: Output validation failed"
                rm -f "$temp_file"
                return 1
            fi
        fi

        # Atomic move
        mv "$temp_file" "$config_file"
        log "  Migrated successfully"
    fi

    return 0
}

# Main
main() {
    local from_version="1.1"
    local to_version="2"
    local dry_run="false"
    local stream_filter=""

    while [[ $# -gt 0 ]]; do
        case $1 in
            --from)
                from_version="$2"
                shift 2
                ;;
            --to)
                to_version="$2"
                shift 2
                ;;
            --dry-run)
                dry_run="true"
                shift
                ;;
            --stream)
                stream_filter="$2"
                shift 2
                ;;
            --help)
                usage
                exit 0
                ;;
            *)
                error "Unknown option: $1"
                usage
                exit 1
                ;;
        esac
    done

    # Validate version transition
    if [ "$from_version" != "1.1" ] || [ "$to_version" != "2" ]; then
        error "Only v1.1 -> v2.0 migration is currently supported"
        exit 1
    fi

    log "Config migration: v$from_version -> v$to_version"
    if [ "$dry_run" = "true" ]; then
        log "DRY RUN - no changes will be made"
    fi

    # Find configs to migrate
    local configs=()
    if [ -n "$stream_filter" ]; then
        local config_file="${CONFIG_DIR}/${stream_filter}/config.json"
        if [ ! -f "$config_file" ]; then
            error "Stream config not found: $config_file"
            exit 1
        fi
        configs+=("$config_file")
    else
        while IFS= read -r -d '' config_file; do
            configs+=("$config_file")
        done < <(find "$CONFIG_DIR" -name "config.json" -print0)
    fi

    if [ ${#configs[@]} -eq 0 ]; then
        warn "No configs found to migrate"
        exit 0
    fi

    log "Found ${#configs[@]} config(s) to process"

    # Migrate each config
    local success=0
    local failed=0
    local skipped=0

    for config_file in "${configs[@]}"; do
        if migrate_config "$config_file" "$dry_run"; then
            ((success++))
        else
            ((failed++))
        fi
    done

    # Summary
    echo ""
    log "Migration summary:"
    log "  Success: $success"
    log "  Failed:  $failed"

    if [ "$failed" -gt 0 ]; then
        exit 1
    fi
}

main "$@"
```

#### v2.0 JSON Schema: `schemas/stream-config.v2.schema.json`

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "stream-config.v2.schema.json",
  "title": "NDP Stream Configuration v2.0",
  "description": "Stream configuration schema v2.0 - entity_schemas removed",
  "type": "object",
  "properties": {
    "config_version": {
      "const": 2,
      "description": "Schema version (must be 2 for v2.0)"
    },
    "stream_id": {
      "type": "string",
      "pattern": "^[a-z][a-z0-9-]*$",
      "description": "Unique stream identifier"
    },
    "description": {
      "type": "string",
      "description": "Human-readable stream description"
    },
    "fields": {
      "type": "array",
      "items": {
        "$ref": "#/$defs/field"
      },
      "minItems": 1,
      "description": "Field definitions with required enrichment"
    },
    "sources": {
      "type": "array",
      "items": {
        "$ref": "#/$defs/source"
      },
      "description": "Data source configurations"
    },
    "silver_etl": {
      "$ref": "#/$defs/silver_etl",
      "description": "Silver layer ETL configuration"
    },
    "dq_rules": {
      "type": "array",
      "items": {
        "$ref": "#/$defs/dq_rule"
      },
      "description": "Data quality rules"
    }
  },
  "required": ["config_version", "stream_id", "fields"],
  "additionalProperties": false,
  "not": {
    "required": ["entity_schemas"]
  },
  "$defs": {
    "field": {
      "type": "object",
      "properties": {
        "name": {
          "type": "string",
          "description": "Field name"
        },
        "type": {
          "type": "string",
          "enum": ["float", "integer", "string", "boolean", "timestamp"],
          "description": "Field data type"
        },
        "nullable": {
          "type": "boolean",
          "default": true
        },
        "unit": {
          "type": "string",
          "description": "Unit of measurement"
        },
        "range": {
          "type": "array",
          "items": {"type": "number"},
          "minItems": 2,
          "maxItems": 2,
          "description": "Valid range [min, max]"
        },
        "description": {
          "type": "string",
          "minLength": 1,
          "description": "Human-readable field description (REQUIRED in v2.0)"
        },
        "device_class": {
          "type": "string",
          "description": "Device class for Grafana/Home Assistant integration"
        }
      },
      "required": ["name", "type", "description"],
      "additionalProperties": false
    },
    "source": {
      "type": "object",
      "properties": {
        "type": {
          "type": "string",
          "enum": ["mqtt", "http"]
        },
        "id": {"type": "string"},
        "config": {"type": "object"}
      },
      "required": ["type", "id", "config"]
    },
    "silver_etl": {
      "type": "object",
      "properties": {
        "target_table": {"type": "string"},
        "timestamp": {"type": "object"},
        "identity": {"type": "object"},
        "field_mappings": {"type": "array"}
      }
    },
    "dq_rule": {
      "type": "object",
      "properties": {
        "id": {"type": "string"},
        "expression": {"type": "string"},
        "severity": {
          "type": "string",
          "enum": ["info", "warning", "error"]
        }
      },
      "required": ["id", "expression"]
    }
  }
}
```

---

## Consequences

### Positive

1. **Simple solution for simple problem** - jq is perfect for JSON transforms
2. **No compilation needed** - Works immediately on any system with jq
3. **Transparent** - Easy to understand what the script does
4. **Reversible** - Backups created automatically
5. **Consistent with dp-020** - Uses same shell+jq pattern as DDL generation

### Negative

1. **Not reusable** - Script is specific to v1.1->v2.0 migration
2. **Limited validation** - jq doesn't provide rich error messages
3. **Shell complexity** - More complex migrations would be awkward in shell

### Mitigation

| Limitation | Mitigation |
|------------|------------|
| One-time use | Acceptable for simple migration; complex migrations use Rust |
| Limited validation | Use ndp-validate for post-migration validation |
| Shell complexity | Future complex migrations will use Rust crates |

---

## Alternatives Considered

### Alternative 1: Rust Migration CLI

Build `ndp-migrate-config` as a Rust binary with versioned transform functions.

```rust
pub fn migrate_v1_1_to_v2(config: &StreamConfigV1_1) -> StreamConfigV2 {
    StreamConfigV2 {
        config_version: 2,
        fields: config.fields.clone(),
        sources: config.sources.clone(),
        // entity_schemas omitted
    }
}
```

**Deferred because**:
- Cross-compilation adds build complexity
- Current migration is trivial (`del(.entity_schemas)`)
- Rust approach makes sense for future complex migrations
- Can implement when first complex migration is needed

**When to revisit**:
- Migration requires data transformation (not just deletion)
- Migration requires validation beyond JSON schema
- Multiple migration paths need support (v1.0->v2.0 vs v1.1->v2.0)

### Alternative 2: Python Migration Script

Use Python with jsonschema for migration and validation.

**Rejected because**:
- Python is not installed on Pi
- Would need to add Python dependency
- jq is already available and sufficient

### Alternative 3: Manual Migration

Document the changes and let operators migrate manually.

**Rejected because**:
- Error-prone for multiple streams
- No validation enforcement
- Inconsistent results

---

## Future Architecture: Rust Migration Crates

When complex migrations are needed, the architecture will be:

```
+------------------------------------------------------------------+
|                    ndp-config-migrate (Rust)                      |
+------------------------------------------------------------------+
|                                                                   |
|  CLI Usage:                                                       |
|    ndp-config-migrate --from 2.0 --to 3.0 config.json             |
|                                                                   |
|  MCP Tool Usage:                                                  |
|    migrate_config(from="2.0", to="3.0", stream_id="air-quality")  |
|                                                                   |
|  Library Usage (in air-quality-app):                              |
|    ndp_config_migrate::migrate(&config, "2.0", "3.0")?            |
|                                                                   |
+------------------------------------------------------------------+
|                                                                   |
|  Migration Registry:                                              |
|    v1.0 -> v1.1: add_enriched_fields()                            |
|    v1.1 -> v2.0: remove_entity_schemas()                          |
|    v2.0 -> v3.0: restructure_sources()  // future                 |
|                                                                   |
|  Features:                                                        |
|    - Type-safe transforms                                         |
|    - Chain migrations (v1.0 -> v1.1 -> v2.0)                      |
|    - Validation at each step                                      |
|    - Rollback support                                             |
|                                                                   |
+------------------------------------------------------------------+
```

This architecture is planned but deferred until a migration requires it.

---

## Implementation Notes

### Code Cleanup After Migration

After v2.0 migration is complete, these code changes are needed:

1. **Remove entity_schemas fallback in dictionary loader**:
```rust
// BEFORE (Phase 1 - supports both)
fn get_field_description(config: &StreamConfig, field: &str) -> Option<String> {
    // Try fields first
    if let Some(desc) = config.fields.iter()
        .find(|f| f.name == field)
        .and_then(|f| f.description.clone()) {
        return Some(desc);
    }
    // Fallback to entity_schemas
    config.entity_schemas.as_ref()
        .and_then(|es| es.iter().find(|e| e.name == field))
        .and_then(|e| e.description.clone())
}

// AFTER (Phase 5 - fields only)
fn get_field_description(config: &StreamConfig, field: &str) -> Option<String> {
    config.fields.iter()
        .find(|f| f.name == field)
        .and_then(|f| f.description.clone())
}
```

2. **Remove EntitySchema struct**:
```rust
// DELETE this struct
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EntitySchema {
    pub name: String,
    pub description: Option<String>,
    pub device_class: Option<String>,
}
```

3. **Update validator to reject entity_schemas**:
```rust
// In dp-019 validator
if config.contains_key("entity_schemas") {
    errors.push(ValidationError {
        path: "$.entity_schemas",
        message: "entity_schemas is forbidden in v2.0. Use description in fields instead.",
        severity: Severity::Error,
    });
}
```

---

## Related Decisions

- **ADR-021-001**: Hot-Reload Scope
- **ADR-021-003**: Release Methodology
- **ADR-016-001**: JSON Source of Truth (established versioning strategy)

---

## References

- `/workspaces/neural-data-platform/product/features/dp-021/SCOPE.md` - Phase 5 requirements
- `/workspaces/neural-data-platform/product/features/dp-016/IMPLEMENTATION-ROADMAP.md` - Schema versioning strategy
- `/workspaces/neural-data-platform/product/features/dp-018/` - v1.1 schema (current)

---

*ADR created: 2026-02-02*
*Feature: dp-021 Config Lifecycle & Release Management*
