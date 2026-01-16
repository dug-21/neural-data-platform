# ADR-009-003: Silver Data Dictionary Sync Mechanism

**Feature**: dp-009 (Config-Driven Silver Layer Data Dictionary)
**Status**: Proposed
**Date**: 2026-01-16
**Author**: NDP Architect
**Depends On**: ADR-003 (dp-002 Sync Mechanism), ADR-009-001, ADR-009-002

---

## Context

The Bronze data dictionary sync (ADR-003, dp-002) established a pattern:

```bash
deploy.sh sync
    └── sync_to_data_dictionary()
        └── Parse YAML → Generate SQL → Execute in transaction
```

To populate Silver dictionary tables (ADR-009-001), we need to:

1. Parse `silver_etl` sections from stream configs
2. Generate SQL for `silver_tables`, `silver_columns`, `silver_lineage`, `silver_dq_rules`
3. Integrate with existing sync mechanism
4. Maintain idempotency and atomicity

### Challenges

1. **Multiple Streams → One Silver Table**: `outdoor-weather` and `nws-observations` both feed `silver.weather_observations`
2. **Cross-Field DQ Rules**: Rules at `silver_etl.dq_rules[]` level (not per-column)
3. **Transform Detection**: Identify transformation type from config
4. **Duplicate Prevention**: Same Silver table may appear in multiple configs

---

## Decision

**Extend the existing `sync_to_data_dictionary()` function with modular Silver sync functions.**

### Architecture

```
sync_to_data_dictionary()
    │
    ├── sync_bronze_metadata()         # Existing Bronze sync
    │   ├── generate_streams_sql()
    │   ├── generate_fields_sql()
    │   ├── generate_sources_sql()
    │   └── generate_entity_schemas_sql()
    │
    └── sync_silver_metadata()         # NEW Silver sync
        ├── collect_silver_configs()   # Gather from all streams
        ├── dedupe_silver_tables()     # Handle multi-stream tables
        ├── generate_silver_tables_sql()
        ├── generate_silver_columns_sql()
        ├── generate_silver_lineage_sql()
        └── generate_silver_dq_rules_sql()
```

### SQL Generation Strategy

#### 1. Collect Silver Configs First

Before generating SQL, scan all configs to identify unique Silver tables:

```bash
collect_silver_configs() {
    declare -A SILVER_TABLES  # table_name -> [stream_ids]

    for config_file in config/base/streams/*/config.yaml; do
        if yq eval '.silver_etl.enabled // false' "$config_file" | grep -q true; then
            local target_table=$(yq eval '.silver_etl.target_table' "$config_file")
            local stream_id=$(yq eval '.stream_id' "$config_file")

            # Accumulate streams per table
            SILVER_TABLES[$target_table]+="$stream_id "
        fi
    done
}
```

#### 2. Generate Silver Tables SQL

One row per unique Silver table:

```bash
generate_silver_tables_sql() {
    for target_table in "${!SILVER_TABLES[@]}"; do
        local streams="${SILVER_TABLES[$target_table]}"
        local first_stream=$(echo "$streams" | awk '{print $1}')
        local config_file="config/base/streams/$first_stream/config.yaml"

        local schema_name=$(echo "$target_table" | cut -d. -f1)
        local description=$(yq eval '.silver_etl.description // ""' "$config_file" | sql_escape)
        local grain=$(yq eval '.silver_etl.grain // ""' "$config_file" | sql_escape)
        local hypertable_col=$(yq eval '.silver_etl.timestamp.target_field // "observation_time"' "$config_file")

        # Convert stream list to PostgreSQL array
        local stream_array=$(echo "$streams" | xargs -n1 | sort -u |
            awk '{printf "\x27%s\x27,", $1}' | sed 's/,$//')

        cat <<EOF
INSERT INTO data_dictionary.silver_tables
    (table_name, schema_name, description, grain, source_streams, hypertable_column)
VALUES
    ('$target_table', '$schema_name', '$description', '$grain',
     ARRAY[$stream_array], '$hypertable_col')
ON CONFLICT (table_name) DO UPDATE SET
    description = EXCLUDED.description,
    grain = EXCLUDED.grain,
    source_streams = EXCLUDED.source_streams,
    hypertable_column = EXCLUDED.hypertable_column,
    updated_at = NOW();
EOF
    done
}
```

#### 3. Generate Silver Columns SQL

Parse `field_mappings` from each stream config:

```bash
generate_silver_columns_sql() {
    local config_file="$1"
    local target_table=$(yq eval '.silver_etl.target_table' "$config_file")

    local mapping_count=$(yq eval '.silver_etl.field_mappings | length' "$config_file")

    for i in $(seq 0 $((mapping_count - 1))); do
        local column_name=$(yq eval ".silver_etl.field_mappings[$i].target_column" "$config_file")
        local data_type=$(yq eval ".silver_etl.field_mappings[$i].type // 'text'" "$config_file" | to_postgres_type)
        local unit=$(yq eval ".silver_etl.field_mappings[$i].unit // null" "$config_file")
        local description=$(yq eval ".silver_etl.field_mappings[$i].description // ''" "$config_file" | sql_escape)
        local nullable=$(yq eval ".silver_etl.field_mappings[$i].nullable // true" "$config_file")

        cat <<EOF
INSERT INTO data_dictionary.silver_columns
    (table_name, column_name, data_type, unit, description, nullable, sort_order)
VALUES
    ('$target_table', '$column_name', '$data_type', $(sql_null "$unit"),
     '$description', $nullable, $i)
ON CONFLICT (table_name, column_name) DO UPDATE SET
    data_type = EXCLUDED.data_type,
    unit = EXCLUDED.unit,
    description = EXCLUDED.description,
    nullable = EXCLUDED.nullable,
    sort_order = EXCLUDED.sort_order;
EOF
    done

    # Also add standard columns (observation_time, ndp_id, dq_flags)
    generate_standard_columns_sql "$target_table"
}

generate_standard_columns_sql() {
    local target_table="$1"

    cat <<EOF
-- Standard columns for $target_table
INSERT INTO data_dictionary.silver_columns (table_name, column_name, data_type, description, nullable, is_primary_key, sort_order)
VALUES
    ('$target_table', 'observation_time', 'TIMESTAMPTZ', 'Observation timestamp', false, true, -3),
    ('$target_table', 'ndp_id', 'TEXT', 'NDP source identifier', false, true, -2),
    ('$target_table', 'ingestion_time', 'TIMESTAMPTZ', 'Silver layer ingestion timestamp', false, false, -1),
    ('$target_table', 'dq_flags', 'TEXT[]', 'Data quality violation flags', true, false, 9999)
ON CONFLICT (table_name, column_name) DO NOTHING;
EOF
}
```

#### 4. Generate Silver Lineage SQL

Track source → target mappings:

```bash
generate_silver_lineage_sql() {
    local config_file="$1"
    local stream_id=$(yq eval '.stream_id' "$config_file")
    local target_table=$(yq eval '.silver_etl.target_table' "$config_file")

    local mapping_count=$(yq eval '.silver_etl.field_mappings | length' "$config_file")

    for i in $(seq 0 $((mapping_count - 1))); do
        local source_path=$(yq eval ".silver_etl.field_mappings[$i].source_path" "$config_file")
        local target_column=$(yq eval ".silver_etl.field_mappings[$i].target_column" "$config_file")

        # Detect transformation type
        local transform="direct"
        if yq eval ".silver_etl.field_mappings[$i].transform" "$config_file" | grep -q "type:"; then
            transform=$(yq eval ".silver_etl.field_mappings[$i].transform.type" "$config_file")
        fi

        cat <<EOF
INSERT INTO data_dictionary.silver_lineage
    (silver_table, silver_column, source_stream, source_path, transformation)
VALUES
    ('$target_table', '$target_column', '$stream_id', '$source_path', '$transform')
ON CONFLICT (silver_table, silver_column, source_stream) DO UPDATE SET
    source_path = EXCLUDED.source_path,
    transformation = EXCLUDED.transformation;
EOF
    done
}
```

#### 5. Generate Silver DQ Rules SQL

Parse both column-level and cross-field rules:

```bash
generate_silver_dq_rules_sql() {
    local config_file="$1"
    local target_table=$(yq eval '.silver_etl.target_table' "$config_file")

    # Column-level DQ rules
    local mapping_count=$(yq eval '.silver_etl.field_mappings | length' "$config_file")

    for i in $(seq 0 $((mapping_count - 1))); do
        local target_column=$(yq eval ".silver_etl.field_mappings[$i].target_column" "$config_file")
        local rule_count=$(yq eval ".silver_etl.field_mappings[$i].dq_rules | length // 0" "$config_file")

        for j in $(seq 0 $((rule_count - 1))); do
            local rule_name=$(yq eval ".silver_etl.field_mappings[$i].dq_rules[$j].rule" "$config_file")
            local action=$(yq eval ".silver_etl.field_mappings[$i].dq_rules[$j].action" "$config_file")
            local rule_params=$(yq eval ".silver_etl.field_mappings[$i].dq_rules[$j] | del(.rule) | del(.action)" "$config_file" -o=json)

            cat <<EOF
INSERT INTO data_dictionary.silver_dq_rules
    (silver_table, silver_column, rule_name, rule_params, action)
VALUES
    ('$target_table', '$target_column', '$rule_name', '$rule_params'::jsonb, '$action')
ON CONFLICT (silver_table, silver_column, rule_name) DO UPDATE SET
    rule_params = EXCLUDED.rule_params,
    action = EXCLUDED.action;
EOF
        done
    done

    # Cross-field DQ rules (at silver_etl.dq_rules level)
    local xfield_rule_count=$(yq eval '.silver_etl.dq_rules | length // 0' "$config_file")

    for k in $(seq 0 $((xfield_rule_count - 1))); do
        local rule_name=$(yq eval ".silver_etl.dq_rules[$k].rule" "$config_file")
        local rule_id=$(yq eval ".silver_etl.dq_rules[$k].name // .silver_etl.dq_rules[$k].rule" "$config_file")
        local action=$(yq eval ".silver_etl.dq_rules[$k].action" "$config_file")
        local rule_params=$(yq eval ".silver_etl.dq_rules[$k] | del(.rule) | del(.action) | del(.name)" "$config_file" -o=json)

        cat <<EOF
INSERT INTO data_dictionary.silver_dq_rules
    (silver_table, silver_column, rule_name, rule_params, action)
VALUES
    ('$target_table', NULL, '${rule_name}:${rule_id}', '$rule_params'::jsonb, '$action')
ON CONFLICT (silver_table, silver_column, rule_name) DO UPDATE SET
    rule_params = EXCLUDED.rule_params,
    action = EXCLUDED.action;
EOF
    done
}
```

### Idempotency Approach

Use `INSERT ... ON CONFLICT ... DO UPDATE` (UPSERT) instead of TRUNCATE:

| Approach | Bronze Sync | Silver Sync |
|----------|-------------|-------------|
| **Strategy** | TRUNCATE + INSERT | UPSERT (ON CONFLICT) |
| **Reason** | Simple, full refresh | Multiple streams → same table |
| **Atomicity** | Full transaction | Full transaction |
| **Idempotent** | Yes | Yes |

**Why UPSERT for Silver**:

If `outdoor-weather` and `nws-observations` both feed `silver.weather_observations`:
- TRUNCATE approach would process one stream, truncate, then lose that data when processing the second
- UPSERT accumulates from all streams without losing prior data

### Transaction Management

```sql
BEGIN;

-- Record sync start
INSERT INTO data_dictionary.sync_status (sync_type, status)
VALUES ('full', 'running');

-- Bronze sync (existing - uses TRUNCATE)
TRUNCATE data_dictionary.streams CASCADE;
-- ... INSERT Bronze data ...

-- Silver sync (new - uses UPSERT)
-- Note: No TRUNCATE for Silver tables - UPSERT handles updates
-- ... UPSERT Silver data ...

-- Update sync status
UPDATE data_dictionary.sync_status
SET completed_at = NOW(),
    status = 'success',
    streams_synced = (SELECT COUNT(*) FROM data_dictionary.streams),
    schemas_synced = (SELECT COUNT(*) FROM data_dictionary.silver_tables)
WHERE status = 'running' AND completed_at IS NULL;

COMMIT;
```

### Error Handling

```bash
sync_silver_metadata() {
    local SQL_FILE="/tmp/silver_dictionary_sync_$$.sql"
    local ERROR_FILE="/tmp/silver_dictionary_error_$$.log"

    {
        echo "-- Silver Data Dictionary Sync"
        echo "-- Generated: $(date -Iseconds)"
        echo ""

        # Collect all Silver configs first
        collect_silver_configs

        # Generate SQL for each unique Silver table
        generate_silver_tables_sql

        # Generate columns, lineage, DQ rules from each stream
        for config_file in config/base/streams/*/config.yaml; do
            if yq eval '.silver_etl.enabled // false' "$config_file" | grep -q true; then
                generate_silver_columns_sql "$config_file"
                generate_silver_lineage_sql "$config_file"
                generate_silver_dq_rules_sql "$config_file"
            fi
        done
    } > "$SQL_FILE"

    # Execute with error capture
    docker exec -i pi5-timescaledb psql -U postgres -d ndp \
        -v ON_ERROR_STOP=1 < "$SQL_FILE" 2> "$ERROR_FILE"

    local exit_code=$?

    if [ $exit_code -ne 0 ]; then
        echo "ERROR: Silver Dictionary sync failed"
        cat "$ERROR_FILE"
        return 1
    fi

    rm -f "$SQL_FILE" "$ERROR_FILE"
    echo "Silver Dictionary sync successful"
}
```

---

## Rationale

### Why UPSERT Over TRUNCATE + INSERT

**Bronze pattern (TRUNCATE)**:
- One stream = one row in `streams` table
- No multi-source complexity
- Clean slate approach works

**Silver pattern (UPSERT)**:
- Multiple streams can feed one Silver table
- Processing one stream must not wipe another's contributions
- Accumulated metadata is correct after processing all streams

### Why Separate Bronze and Silver Sync Functions

**Considered Alternative**: Single unified sync function.

**Rejected because**:
1. Bronze uses TRUNCATE; Silver uses UPSERT
2. Different parsing logic (fields vs field_mappings)
3. Cleaner error handling per layer
4. Easier to debug layer-specific issues

### Why Collect Silver Configs First

For `silver_tables.source_streams`, we need to know ALL streams that feed a table before inserting.

**Alternative**: Update source_streams array incrementally with array_append.

**Rejected because**:
1. Order-dependent behavior
2. Risk of duplicates
3. More complex SQL

### Why Store Cross-Field Rules with NULL Column

Cross-field rules (like `pm10_gte_pm25`) don't apply to a single column.

**Options**:
1. `silver_column = NULL` for cross-field rules (chosen)
2. Separate `silver_cross_field_rules` table
3. Store in JSONB blob on `silver_tables`

**Choice rationale**:
- NULL column is semantically clear
- Single table simplifies queries
- Can filter: `WHERE silver_column IS NULL` for cross-field rules

---

## Consequences

### Positive

1. **Atomic Sync**: Transaction ensures all-or-nothing
2. **Idempotent**: Safe to re-run; produces same result
3. **Multi-Stream Aware**: Correctly handles merged Silver tables
4. **Lineage Complete**: Every Silver column traced to Bronze source
5. **DQ Visible**: Both column-level and cross-field rules documented

### Negative

1. **Sync Complexity**: More parsing logic than Bronze
2. **UPSERT Overhead**: Slightly slower than TRUNCATE + INSERT
3. **Order Sensitivity**: `silver_tables` must be inserted before `silver_columns` (FK)

### Risks

1. **Malformed YAML**: Invalid config could generate bad SQL
   - **Mitigation**: yq errors will be captured; transaction rolls back
2. **Missing transform detection**: Complex transforms may not be classified
   - **Mitigation**: Default to "direct"; can enhance detection later
3. **Large configs**: Many field_mappings could generate large SQL
   - **Mitigation**: Current scale (4 tables, ~50 columns) is manageable

---

## Testing Plan

### Unit Tests (Shell)

```bash
# Test YAML parsing
test_parse_silver_etl() {
    local config="config/base/streams/air-quality/config.yaml"

    # Assert target_table extraction
    local table=$(yq eval '.silver_etl.target_table' "$config")
    assert_equals "silver.air_quality_observations" "$table"

    # Assert field_mappings count
    local count=$(yq eval '.silver_etl.field_mappings | length' "$config")
    assert_equals "7" "$count"
}
```

### Integration Tests

```bash
# Full sync and verify
test_silver_sync() {
    ./deploy.sh sync

    # Verify tables
    local table_count=$(docker exec pi5-timescaledb psql -U postgres -d ndp -t -c \
        "SELECT COUNT(*) FROM data_dictionary.silver_tables")
    assert_equals "4" "$table_count"

    # Verify lineage
    local lineage_count=$(docker exec pi5-timescaledb psql -U postgres -d ndp -t -c \
        "SELECT COUNT(*) FROM data_dictionary.silver_lineage")
    assert_greater_than "$lineage_count" "20"
}
```

### Idempotency Test

```bash
# Run sync twice, verify same result
test_idempotency() {
    ./deploy.sh sync
    local hash1=$(docker exec pi5-timescaledb psql -U postgres -d ndp -t -c \
        "SELECT MD5(STRING_AGG(table_name || column_name, '')) FROM data_dictionary.silver_columns ORDER BY table_name, column_name")

    ./deploy.sh sync
    local hash2=$(docker exec pi5-timescaledb psql -U postgres -d ndp -t -c \
        "SELECT MD5(STRING_AGG(table_name || column_name, '')) FROM data_dictionary.silver_columns ORDER BY table_name, column_name")

    assert_equals "$hash1" "$hash2"
}
```

---

## Related Decisions

- **ADR-003 (dp-002)**: Bronze Sync Mechanism - Pattern being extended
- **ADR-009-001**: Silver Dictionary Tables - Target schema
- **ADR-009-002**: Config Schema Extension - Source YAML structure

---

## References

1. [deploy.sh](../../../../deploy/pi/deploy.sh) - Current sync implementation
2. [ADR-003 (dp-002)](../../dp-002/architecture/ADR-003-SYNC-MECHANISM.md) - Bronze sync ADR
3. [PostgreSQL UPSERT](https://www.postgresql.org/docs/current/sql-insert.html#SQL-ON-CONFLICT)

---

**Last Updated**: 2026-01-16
**Next Review**: After sync implementation and integration testing
