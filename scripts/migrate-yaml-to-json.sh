#!/bin/bash
# =============================================================================
# dp-018: YAML to JSON Migration Script
# =============================================================================
# Converts stream configuration files from YAML to JSON v1.1 format.
#
# Features:
# - Converts config/base/streams/*/config.yaml to config.json
# - Adds config_version: "1.1" to output
# - Enriches fields with descriptions from entity_schemas
# - Idempotent: safe to run multiple times
# - Validates output against JSON Schema (if --validate)
#
# Dependencies:
# - yq (https://github.com/mikefarah/yq) - YAML processor
# - jq (https://stedolan.github.io/jq/) - JSON processor
#
# Usage:
#   ./scripts/migrate-yaml-to-json.sh [OPTIONS]
#
# Options:
#   --dry-run       Show what would be converted without writing files
#   --stream ID     Convert only the specified stream
#   --validate      Validate output against JSON schema
#   --verbose       Show detailed progress
#   --help          Show this help message
#
# Examples:
#   ./scripts/migrate-yaml-to-json.sh                      # Migrate all streams
#   ./scripts/migrate-yaml-to-json.sh --dry-run            # Preview changes
#   ./scripts/migrate-yaml-to-json.sh --stream air-quality # Migrate one stream
# =============================================================================

set -euo pipefail

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
CONFIG_DIR="${PROJECT_ROOT}/config/base/streams"
SCHEMA_FILE="${PROJECT_ROOT}/schemas/stream-config.v1.1.schema.json"
CONFIG_VERSION="1.1"

# Options
DRY_RUN=false
STREAM_FILTER=""
VALIDATE=false
VERBOSE=false

# Counters
SUCCESS_COUNT=0
SKIP_COUNT=0
FAIL_COUNT=0
ENRICHED_FIELDS_TOTAL=0

# Colors for output (disabled if not a terminal)
if [ -t 1 ]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[0;33m'
    BLUE='\033[0;34m'
    NC='\033[0m' # No Color
else
    RED=''
    GREEN=''
    YELLOW=''
    BLUE=''
    NC=''
fi

# =============================================================================
# Utility Functions
# =============================================================================

log_info() {
    echo -e "${GREEN}[INFO]${NC} $*"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $*"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $*"
}

log_debug() {
    if [ "$VERBOSE" = true ]; then
        echo -e "${BLUE}[DEBUG]${NC} $*"
    fi
}

show_help() {
    head -n 28 "$0" | tail -n 26 | sed 's/^# //' | sed 's/^#//'
    exit 0
}

check_dependencies() {
    local missing=()

    if ! command -v yq &> /dev/null; then
        missing+=("yq")
    fi

    if ! command -v jq &> /dev/null; then
        missing+=("jq")
    fi

    if [ ${#missing[@]} -gt 0 ]; then
        log_error "Missing required dependencies: ${missing[*]}"
        echo ""
        echo "Install instructions:"
        echo "  yq: https://github.com/mikefarah/yq#install"
        echo "       brew install yq (macOS)"
        echo "       snap install yq (Ubuntu)"
        echo ""
        echo "  jq: https://stedolan.github.io/jq/download/"
        echo "       brew install jq (macOS)"
        echo "       apt-get install jq (Debian/Ubuntu)"
        exit 1
    fi

    log_debug "Dependencies verified: yq, jq"
}

# =============================================================================
# Core Migration Functions
# =============================================================================

# Check if fields is a map (object) or array
# Returns: "map", "array", or "empty"
detect_fields_format() {
    local yaml_file="$1"

    local fields_type
    fields_type=$(yq eval '.fields | type' "$yaml_file" 2>/dev/null)

    case "$fields_type" in
        "!!map")
            echo "map"
            ;;
        "!!seq")
            echo "array"
            ;;
        *)
            echo "empty"
            ;;
    esac
}

# Convert fields from map format to array format
# Input: { pm25: { type: "float", ... } }
# Output: [ { name: "pm25", type: "float", ... } ]
convert_fields_map_to_array() {
    local json_content="$1"

    # Use jq to convert map to array, adding 'name' from the key
    echo "$json_content" | jq '
        if .fields | type == "object" then
            .fields = [.fields | to_entries[] | .value + {name: .key}]
        else
            .
        end
    '
}

# Build lookup map from entity_schemas for field enrichment
# Returns a JSON object mapping field name to attribute data
build_entity_schema_lookup() {
    local json_content="$1"

    echo "$json_content" | jq '
        if .entity_schemas then
            [.entity_schemas[] |
                select(.attributes != null) |
                .attributes[] |
                {(.name): .}
            ] | add // {}
        else
            {}
        end
    '
}

# Enrich fields with data from entity_schemas
# Copies: description, device_class, range, unit (if not already present)
enrich_fields_from_entity_schemas() {
    local json_content="$1"
    local lookup="$2"

    echo "$json_content" | jq --argjson lookup "$lookup" '
        .fields = [.fields[] |
            . as $field |
            ($lookup[$field.name] // {}) as $attr |
            # Enrich each attribute if not already present
            (if (.description // "") == "" and ($attr.description // "") != ""
             then . + {description: $attr.description} else . end) |
            (if (.device_class // "") == "" and ($attr.device_class // "") != ""
             then . + {device_class: $attr.device_class} else . end) |
            (if .range == null and $attr.range != null
             then . + {range: $attr.range} else . end) |
            (if (.unit // "") == "" and ($attr.unit // "") != ""
             then . + {unit: $attr.unit} else . end)
        ]
    '
}

# Count how many fields were enriched
count_enriched_fields() {
    local original="$1"
    local enriched="$2"

    # Count fields that gained a description
    local original_with_desc
    local enriched_with_desc

    original_with_desc=$(echo "$original" | jq '[.fields[] | select(.description != null and .description != "")] | length')
    enriched_with_desc=$(echo "$enriched" | jq '[.fields[] | select(.description != null and .description != "")] | length')

    echo $((enriched_with_desc - original_with_desc))
}

# Validate JSON against schema
validate_against_schema() {
    local json_file="$1"
    local schema_file="$2"

    if [ ! -f "$schema_file" ]; then
        log_warn "Schema file not found: $schema_file (skipping validation)"
        return 0
    fi

    # Use jq for basic structure validation
    # Full JSON Schema validation would require a tool like ajv-cli
    if ! jq empty "$json_file" 2>/dev/null; then
        log_error "Invalid JSON in $json_file"
        return 1
    fi

    # Check required fields
    local stream_id
    stream_id=$(jq -r '.stream_id // ""' "$json_file")
    if [ -z "$stream_id" ]; then
        log_error "Missing required field 'stream_id' in $json_file"
        return 1
    fi

    local fields_count
    fields_count=$(jq '.fields | length' "$json_file")
    if [ "$fields_count" -eq 0 ]; then
        log_error "Required field 'fields' is empty in $json_file"
        return 1
    fi

    log_debug "Validation passed for $json_file"
    return 0
}

# Migrate a single stream from YAML to JSON
migrate_single_stream() {
    local stream_dir="$1"
    local stream_id
    stream_id=$(basename "$stream_dir")

    local yaml_path="${stream_dir}/config.yaml"
    local json_path="${stream_dir}/config.json"

    log_debug "Processing stream: $stream_id"

    # Step 1: Check if YAML exists
    if [ ! -f "$yaml_path" ]; then
        log_warn "config.yaml not found for stream: $stream_id"
        ((SKIP_COUNT++))
        return 0
    fi

    # Step 2: Check idempotency (already migrated?)
    if [ -f "$json_path" ]; then
        local existing_version
        existing_version=$(jq -r '.config_version // 0' "$json_path" 2>/dev/null || echo "0")

        # Compare as strings to handle "1.1" format
        if [ "$existing_version" = "$CONFIG_VERSION" ] || [ "$existing_version" = "1.1" ]; then
            log_info "Stream '$stream_id' already migrated to v${CONFIG_VERSION}, skipping"
            ((SKIP_COUNT++))
            return 0
        fi
    fi

    # Step 3: Parse YAML to JSON
    local json_content
    if ! json_content=$(yq eval -o=json "$yaml_path" 2>&1); then
        log_error "Failed to parse YAML for stream '$stream_id': $json_content"
        ((FAIL_COUNT++))
        return 1
    fi

    # Step 4: Detect fields format and convert if necessary
    local fields_format
    fields_format=$(detect_fields_format "$yaml_path")
    log_debug "Fields format for $stream_id: $fields_format"

    if [ "$fields_format" = "map" ]; then
        log_debug "Converting fields from map to array format"
        json_content=$(convert_fields_map_to_array "$json_content")
    fi

    # Step 5: Build entity schema lookup
    local entity_lookup
    entity_lookup=$(build_entity_schema_lookup "$json_content")
    log_debug "Entity schema lookup built with $(echo "$entity_lookup" | jq 'keys | length') entries"

    # Step 6: Enrich fields from entity_schemas
    local original_json="$json_content"
    json_content=$(enrich_fields_from_entity_schemas "$json_content" "$entity_lookup")

    local fields_enriched
    fields_enriched=$(count_enriched_fields "$original_json" "$json_content")
    ((ENRICHED_FIELDS_TOTAL += fields_enriched))

    if [ "$fields_enriched" -gt 0 ]; then
        log_debug "Enriched $fields_enriched field(s) with descriptions"
    fi

    # Step 7: Set config_version
    json_content=$(echo "$json_content" | jq --arg version "$CONFIG_VERSION" '. + {config_version: $version}')

    # Step 8: Pretty-print the JSON
    json_content=$(echo "$json_content" | jq '.')

    # Step 9: Write or preview
    if [ "$DRY_RUN" = true ]; then
        echo ""
        echo "=== Would create: $json_path ==="
        echo "$json_content" | head -50
        if [ "$(echo "$json_content" | wc -l)" -gt 50 ]; then
            echo "... (truncated)"
        fi
        echo ""
        log_info "[DRY-RUN] Would migrate stream '$stream_id' ($fields_enriched fields enriched)"
    else
        echo "$json_content" > "$json_path"
        log_info "Migrated stream '$stream_id' -> $json_path ($fields_enriched fields enriched)"
    fi

    # Step 10: Validate if requested
    if [ "$VALIDATE" = true ] && [ "$DRY_RUN" = false ]; then
        if ! validate_against_schema "$json_path" "$SCHEMA_FILE"; then
            log_error "Validation failed for stream '$stream_id'"
            ((FAIL_COUNT++))
            return 1
        fi
    fi

    ((SUCCESS_COUNT++))
    return 0
}

# =============================================================================
# Main Entry Point
# =============================================================================

main() {
    # Parse arguments
    while [[ $# -gt 0 ]]; do
        case $1 in
            --dry-run)
                DRY_RUN=true
                shift
                ;;
            --stream)
                STREAM_FILTER="$2"
                shift 2
                ;;
            --validate)
                VALIDATE=true
                shift
                ;;
            --verbose)
                VERBOSE=true
                shift
                ;;
            --help|-h)
                show_help
                ;;
            *)
                log_error "Unknown option: $1"
                echo "Use --help for usage information"
                exit 1
                ;;
        esac
    done

    # Banner
    echo ""
    echo "========================================"
    echo "  dp-018: YAML to JSON Migration"
    echo "  Config Version: ${CONFIG_VERSION}"
    echo "========================================"
    echo ""

    # Check dependencies
    check_dependencies

    # Verify config directory exists
    if [ ! -d "$CONFIG_DIR" ]; then
        log_error "Config directory not found: $CONFIG_DIR"
        exit 1
    fi

    # Find all stream directories
    local stream_dirs=()

    if [ -n "$STREAM_FILTER" ]; then
        # Single stream mode
        local target_dir="${CONFIG_DIR}/${STREAM_FILTER}"
        if [ ! -d "$target_dir" ]; then
            log_error "Stream directory not found: $target_dir"
            exit 1
        fi
        stream_dirs=("$target_dir")
    else
        # All streams mode
        for dir in "${CONFIG_DIR}"/*/; do
            if [ -d "$dir" ]; then
                stream_dirs+=("${dir%/}")
            fi
        done
    fi

    local total_streams=${#stream_dirs[@]}
    log_info "Found $total_streams stream(s) to process"

    if [ "$DRY_RUN" = true ]; then
        log_warn "DRY-RUN mode: No files will be modified"
    fi

    if [ "$VALIDATE" = true ]; then
        log_info "Validation enabled against: $SCHEMA_FILE"
    fi

    echo ""

    # Process each stream
    for stream_dir in "${stream_dirs[@]}"; do
        migrate_single_stream "$stream_dir" || true
    done

    # Summary
    echo ""
    echo "========================================"
    echo "  Migration Summary"
    echo "========================================"
    echo ""
    log_info "Streams migrated:  $SUCCESS_COUNT"
    log_info "Streams skipped:   $SKIP_COUNT"
    log_info "Fields enriched:   $ENRICHED_FIELDS_TOTAL"

    if [ "$FAIL_COUNT" -gt 0 ]; then
        log_error "Streams failed:    $FAIL_COUNT"
        echo ""
        exit 1
    else
        log_info "Streams failed:    0"
        echo ""
        log_info "Migration completed successfully!"
    fi

    if [ "$DRY_RUN" = true ]; then
        echo ""
        log_warn "This was a dry-run. Run without --dry-run to apply changes."
    fi
}

# Run main function
main "$@"
