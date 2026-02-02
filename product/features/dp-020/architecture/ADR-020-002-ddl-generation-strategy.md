# ADR-020-002: DDL Generation Strategy

**Status**: Proposed
**Date**: 2026-02-02
**Decision Makers**: NDP Architecture Team
**Feature**: dp-020 Declarative Deploy

---

## Context

dp-020 requires automatic DDL generation for Silver tables based on stream configuration. When a `silver-table` declaration appears in the manifest, the system must:

1. Generate `CREATE TABLE IF NOT EXISTS` with correct columns and types
2. Generate appropriate indexes
3. Convert to TimescaleDB hypertable with compression
4. Apply retention policies
5. Grant permissions to appropriate roles
6. Handle schema evolution (ADD COLUMN for new fields)

### Existing Pattern: sync_to_data_dictionary()

The current `deploy.sh` already generates SQL from configuration in the `sync_to_data_dictionary()` function (lines 344-802). This function:

- Reads YAML config files
- Extracts field metadata
- Generates INSERT statements
- Executes via `psql`

This establishes a precedent for SQL generation within deploy.sh.

---

## Decision

**Use shell templates with a helper function for DDL generation, embedded in deploy.sh.**

### Architecture

```
+------------------------------------------------------------------+
|                    DDL Generation Flow                            |
+------------------------------------------------------------------+
|                                                                   |
|  Stream Config (JSON)                                             |
|       |                                                           |
|       v                                                           |
|  +------------------------------------------------------------+  |
|  | extract_silver_etl()                                        |  |
|  | - Parse silver_etl section                                  |  |
|  | - Extract target_table, timestamp, field_mappings           |  |
|  +------------------------------------------------------------+  |
|       |                                                           |
|       v                                                           |
|  +------------------------------------------------------------+  |
|  | map_type()                                                  |  |
|  | - Config type -> PostgreSQL type                            |  |
|  | - Handle all supported types                                |  |
|  +------------------------------------------------------------+  |
|       |                                                           |
|       v                                                           |
|  +------------------------------------------------------------+  |
|  | generate_ddl()                                              |  |
|  | - Generate CREATE TABLE                                     |  |
|  | - Generate indexes                                          |  |
|  | - Generate hypertable conversion                            |  |
|  | - Generate policies                                         |  |
|  | - Generate permissions                                      |  |
|  +------------------------------------------------------------+  |
|       |                                                           |
|       v                                                           |
|  +------------------------------------------------------------+  |
|  | detect_new_columns() (for existing tables)                  |  |
|  | - Query information_schema.columns                          |  |
|  | - Compare config columns to existing                        |  |
|  | - Generate ALTER TABLE ADD COLUMN                           |  |
|  +------------------------------------------------------------+  |
|       |                                                           |
|       v                                                           |
|  SQL DDL Script -> psql                                          |
|                                                                   |
+------------------------------------------------------------------+
```

### Type Mapping

```bash
# Type mapping function: config type -> PostgreSQL type
map_type() {
    local config_type="$1"
    case "$config_type" in
        double_precision)   echo "DOUBLE PRECISION" ;;
        real)               echo "REAL" ;;
        integer)            echo "INTEGER" ;;
        bigint)             echo "BIGINT" ;;
        smallint)           echo "SMALLINT" ;;
        text)               echo "TEXT" ;;
        varchar)            echo "VARCHAR(255)" ;;
        boolean)            echo "BOOLEAN" ;;
        timestamptz)        echo "TIMESTAMPTZ" ;;
        jsonb)              echo "JSONB" ;;
        "text[]")           echo "TEXT[]" ;;
        *)
            warn "Unknown type '$config_type', defaulting to TEXT"
            echo "TEXT"
            ;;
    esac
}
```

### DDL Generator Function

```bash
# Generate DDL for a Silver table from stream config
# Args: $1 = path to stream config.json
# Output: SQL DDL to stdout
generate_ddl() {
    local config_file="$1"

    # Validate silver_etl exists
    local silver_etl=$(jq -r '.silver_etl // empty' "$config_file")
    if [ -z "$silver_etl" ] || [ "$silver_etl" = "null" ]; then
        error "No silver_etl section in $config_file"
        return 1
    fi

    # Extract metadata
    local target_table=$(jq -r '.silver_etl.target_table' "$config_file")
    local schema_name="${target_table%%.*}"
    local table_name="${target_table##*.}"
    local timestamp_col=$(jq -r '.silver_etl.timestamp.target_field // "observation_time"' "$config_file")

    # Start DDL generation
    cat << EOF
-- DDL for $target_table
-- Generated: $(date -Iseconds)
-- Source: $config_file

-- ============================================
-- 3.4a: CREATE TABLE
-- ============================================
CREATE TABLE IF NOT EXISTS $target_table (
    $timestamp_col TIMESTAMPTZ NOT NULL,
    ndp_id TEXT NOT NULL,
EOF

    # Generate columns from field_mappings
    local field_count=$(jq -r '.silver_etl.field_mappings | length' "$config_file")
    for i in $(seq 0 $((field_count - 1))); do
        local col_name=$(jq -r ".silver_etl.field_mappings[$i].target_column" "$config_file")
        local col_type_raw=$(jq -r ".silver_etl.field_mappings[$i].type // \"text\"" "$config_file")
        local col_type=$(map_type "$col_type_raw")
        local nullable=$(jq -r ".silver_etl.field_mappings[$i].nullable // true" "$config_file")

        local null_clause=""
        if [ "$nullable" = "false" ]; then
            null_clause=" NOT NULL"
        fi

        echo "    $col_name $col_type$null_clause,"
    done

    # Standard metadata columns
    cat << 'EOF'
    dq_flags TEXT[],
    _bronze_id UUID,
    _ingested_at TIMESTAMPTZ DEFAULT NOW()
);

EOF

    # Generate indexes
    cat << EOF
-- ============================================
-- 3.4b: Indexes
-- ============================================
CREATE INDEX IF NOT EXISTS idx_${table_name}_time_id
    ON $target_table ($timestamp_col, ndp_id);
CREATE INDEX IF NOT EXISTS idx_${table_name}_dq_flags
    ON $target_table USING GIN (dq_flags);

EOF

    # Generate hypertable conversion
    cat << EOF
-- ============================================
-- 3.4c: Hypertable Conversion
-- ============================================
SELECT create_hypertable('$target_table', '$timestamp_col',
    chunk_time_interval => INTERVAL '1 day',
    if_not_exists => TRUE);

EOF

    # Generate policies
    cat << EOF
-- ============================================
-- 3.4d: Compression and Retention Policies
-- ============================================
SELECT add_compression_policy('$target_table',
    INTERVAL '7 days', if_not_exists => TRUE);
SELECT add_retention_policy('$target_table',
    INTERVAL '90 days', if_not_exists => TRUE);

EOF

    # Generate permissions
    cat << EOF
-- ============================================
-- 3.4e: Permissions
-- ============================================
GRANT SELECT, INSERT ON $target_table TO ndp_app;
GRANT SELECT ON $target_table TO grafana_reader;
EOF

    return 0
}
```

### ADD COLUMN Detection

```bash
# Detect new columns and generate ALTER TABLE ADD COLUMN
# Args: $1 = config file, $2 = target table (optional, extracted from config if not provided)
# Output: SQL ALTER TABLE statements to stdout (empty if no new columns)
generate_add_columns() {
    local config_file="$1"
    local target_table="${2:-$(jq -r '.silver_etl.target_table' "$config_file")}"

    local schema_name="${target_table%%.*}"
    local table_name="${target_table##*.}"

    # Get existing columns from database
    local existing_columns=$(dcx timescaledb psql -U postgres -d ndp -tAc \
        "SELECT column_name FROM information_schema.columns
         WHERE table_schema = '$schema_name' AND table_name = '$table_name'" 2>/dev/null | tr '\n' ' ')

    # Check each field_mapping
    local field_count=$(jq -r '.silver_etl.field_mappings | length' "$config_file")
    local has_new_columns=false

    for i in $(seq 0 $((field_count - 1))); do
        local col_name=$(jq -r ".silver_etl.field_mappings[$i].target_column" "$config_file")
        local col_type_raw=$(jq -r ".silver_etl.field_mappings[$i].type // \"text\"" "$config_file")
        local col_type=$(map_type "$col_type_raw")

        # Check if column exists
        if ! echo " $existing_columns " | grep -q " $col_name "; then
            if [ "$has_new_columns" = "false" ]; then
                echo ""
                echo "-- ============================================"
                echo "-- 3.4g: ADD COLUMN (Schema Evolution)"
                echo "-- ============================================"
                has_new_columns=true
            fi

            cat << EOF
DO \$\$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_schema = '$schema_name'
        AND table_name = '$table_name'
        AND column_name = '$col_name'
    ) THEN
        ALTER TABLE $target_table ADD COLUMN $col_name $col_type;
    END IF;
END \$\$;
EOF
        fi
    done

    return 0
}
```

### Full DDL Generation Flow

```bash
# Generate complete DDL (CREATE TABLE + ADD COLUMN as needed)
# Args: $1 = config file
# Output: SQL DDL to stdout
generate_full_ddl() {
    local config_file="$1"
    local target_table=$(jq -r '.silver_etl.target_table' "$config_file")

    # Check if table exists
    local schema_name="${target_table%%.*}"
    local table_name="${target_table##*.}"
    local table_exists=$(dcx timescaledb psql -U postgres -d ndp -tAc \
        "SELECT EXISTS(SELECT 1 FROM information_schema.tables
         WHERE table_schema = '$schema_name' AND table_name = '$table_name')" 2>/dev/null)

    if [ "$table_exists" = "t" ]; then
        # Table exists - only generate ADD COLUMN for new fields
        log "Table $target_table exists, checking for new columns..."
        generate_add_columns "$config_file" "$target_table"
    else
        # Table doesn't exist - generate full DDL
        log "Table $target_table does not exist, generating CREATE TABLE..."
        generate_ddl "$config_file"
    fi
}
```

---

## Idempotency Mechanisms

| DDL Operation | Idempotency Approach |
|---------------|---------------------|
| CREATE TABLE | `IF NOT EXISTS` |
| CREATE INDEX | `IF NOT EXISTS` |
| create_hypertable | `if_not_exists => TRUE` |
| add_compression_policy | `if_not_exists => TRUE` |
| add_retention_policy | `if_not_exists => TRUE` |
| GRANT | Safe to re-run (no-op if already granted) |
| ALTER TABLE ADD COLUMN | Wrapped in `IF NOT EXISTS` check |

---

## Consequences

### Positive

1. **Consistent with existing patterns** - Follows sync_to_data_dictionary() approach
2. **No new dependencies** - Uses existing jq, psql
3. **Readable DDL** - Clear comments, easy to audit
4. **Idempotent by design** - Safe to run multiple times
5. **Portable** - Works on any system with shell/jq/psql
6. **Debuggable** - Can generate DDL to file, review, then apply

### Negative

1. **Shell string manipulation** - More verbose than a dedicated tool
2. **Limited validation** - Relies on psql for SQL syntax errors
3. **Type mapping maintenance** - Must keep in sync with dp-019 research

### Neutral

1. **Performance** - Acceptable for deployment-time operations
2. **Testing** - Can test by generating DDL and comparing to expected

---

## Alternatives Considered

### Alternative 1: Rust DDL Generator Binary

A dedicated Rust tool (`ndp-ddl-gen`) that reads config and outputs DDL.

```bash
# Hypothetical usage
ndp-ddl-gen --config config.json --output ddl.sql
psql -f ddl.sql
```

**Rejected because**:
- Adds cross-compilation requirement for Pi
- Most DDL logic is template-based, not compute-intensive
- Would need to maintain Rust dependency alongside shell
- Current pattern (shell generation) works in sync_to_data_dictionary

**When to reconsider**:
- If DDL generation becomes significantly more complex
- If type system validation becomes critical
- If generation performance becomes a bottleneck

### Alternative 2: SQL Templates with Variable Substitution

Store SQL templates in files, use envsubst or similar:

```sql
-- templates/silver_table.sql.tmpl
CREATE TABLE IF NOT EXISTS ${TARGET_TABLE} (
    ${TIMESTAMP_COL} TIMESTAMPTZ NOT NULL,
    ...
);
```

**Rejected because**:
- Less flexible for dynamic column lists
- Harder to handle conditional logic (ADD COLUMN detection)
- Would still need shell logic for column iteration
- Current approach is more self-contained

### Alternative 3: Separate DDL Generation Tool (Python/jq-based)

Use Python or a jq-only script for DDL generation:

```bash
python3 scripts/generate_ddl.py config.json > ddl.sql
# or
jq -rf scripts/generate_ddl.jq config.json > ddl.sql
```

**Rejected because**:
- Python adds dependency (may not be on Pi)
- jq-only is extremely verbose for this use case
- Shell function integrates better with deploy.sh
- Keeps all deployment logic in one place

---

## Implementation Notes

### Error Handling

```bash
handle_silver_table() {
    local json="$1"
    local stream_id=$(echo "$json" | jq -r '.stream_id')
    local config_file="$REPO_ROOT/config/base/streams/$stream_id/config.json"

    # Validate config exists
    if [ ! -f "$config_file" ]; then
        error "Stream config not found: $config_file"
        return 1
    fi

    # Validate silver_etl section
    if ! jq -e '.silver_etl' "$config_file" > /dev/null 2>&1; then
        error "No silver_etl section in $stream_id config"
        return 1
    fi

    # Generate DDL to temp file
    local ddl_file="/tmp/ddl_${stream_id}_$$.sql"
    if ! generate_full_ddl "$config_file" > "$ddl_file"; then
        error "DDL generation failed for $stream_id"
        rm -f "$ddl_file"
        return 1
    fi

    # Check if DDL is empty (no changes needed)
    if [ ! -s "$ddl_file" ]; then
        log "No DDL changes needed for $stream_id"
        rm -f "$ddl_file"
        return 0
    fi

    # Apply DDL
    log "Applying DDL for $stream_id..."
    if dcx timescaledb psql -U postgres -d ndp < "$ddl_file" 2>&1; then
        log "DDL applied successfully for $stream_id"
        rm -f "$ddl_file"
        return 0
    else
        error "DDL execution failed for $stream_id"
        log "Failed DDL saved to: $ddl_file"
        return 1
    fi
}
```

### Testing Strategy

```bash
# Test DDL generation without applying
generate_ddl /path/to/config.json > /tmp/test_ddl.sql
cat /tmp/test_ddl.sql  # Review

# Test ADD COLUMN detection
generate_add_columns /path/to/config.json silver.air_quality_observations

# Dry run (validate SQL syntax without executing)
dcx timescaledb psql -U postgres -d ndp -c "SET check_function_bodies = false;" \
    -f /tmp/test_ddl.sql --set ON_ERROR_STOP=on -1
```

---

## Type Mapping Reference

| Config Type | PostgreSQL Type | udt_name (for validation) | Notes |
|-------------|-----------------|---------------------------|-------|
| `double_precision` | DOUBLE PRECISION | float8 | 8-byte float |
| `real` | REAL | float4 | 4-byte float |
| `integer` | INTEGER | int4 | 4-byte signed |
| `bigint` | BIGINT | int8 | 8-byte signed |
| `smallint` | SMALLINT | int2 | 2-byte signed |
| `text` | TEXT | text | Unlimited |
| `varchar` | VARCHAR(255) | varchar | Default 255 |
| `boolean` | BOOLEAN | bool | true/false |
| `timestamptz` | TIMESTAMPTZ | timestamptz | With timezone |
| `jsonb` | JSONB | jsonb | Binary JSON |
| `text[]` | TEXT[] | _text | Text array |

This mapping aligns with dp-019 SUPPORTED-VALUES-RESEARCH.md (Section 4).

---

## Related Decisions

- **ADR-020-001**: Extensible Handler Architecture (handler structure)
- **ADR-020-003**: Manifest Schema Versioning (manifest evolution)
- **dp-019 SUPPORTED-VALUES-RESEARCH.md**: Type mapping reference

---

## References

- `/workspaces/neural-data-platform/deploy/pi/deploy.sh` - sync_to_data_dictionary() pattern
- `/workspaces/neural-data-platform/product/features/dp-019/specification/SUPPORTED-VALUES-RESEARCH.md` - Type definitions
- `/workspaces/neural-data-platform/product/features/dp-019/specification/SILVER-VALIDATION-RESEARCH.md` - Type compatibility
- `/workspaces/neural-data-platform/product/features/dp-020/SCOPE.md` - DDL requirements (3.4a-3.4g)

---

*ADR created: 2026-02-02*
*Feature: dp-020 Declarative Deploy*
