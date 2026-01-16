# Silver Data Dictionary Sync Algorithm

**Feature**: dp-009 (Config-Driven Silver Layer Data Dictionary)
**Document Type**: Pseudocode
**Date**: 2026-01-16
**Author**: NDP Rust Developer

---

## 1. Algorithm Overview

The Silver Data Dictionary sync extends the existing `sync_to_data_dictionary()` function
to populate four new tables from YAML configuration:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        sync_to_data_dictionary()                             │
│                                                                             │
│  PHASE 1: Bronze Sync (existing)        PHASE 2: Silver Sync (NEW)          │
│  ─────────────────────────────          ────────────────────────            │
│  - streams                              - silver_tables                     │
│  - fields                               - silver_columns                    │
│  - sources                              - silver_lineage                    │
│  - entity_schemas                       - silver_dq_rules                   │
│  - entity_schema_attributes                                                 │
│                                                                             │
│  Strategy: TRUNCATE + INSERT            Strategy: UPSERT (ON CONFLICT)      │
│  (single stream = single row)           (multiple streams -> one table)     │
└─────────────────────────────────────────────────────────────────────────────┘
```

### High-Level Steps

```
1. WAIT for TimescaleDB to be ready
2. BEGIN transaction
3. RECORD sync start in sync_status table
4. SYNC Bronze metadata (existing logic - TRUNCATE + INSERT)
5. COLLECT Silver configs from all streams (first pass - gather unique tables)
6. DEDUPLICATE Silver tables (multiple streams may feed same table)
7. GENERATE Silver tables SQL (UPSERT)
8. GENERATE Silver columns SQL (UPSERT per stream)
9. GENERATE Silver lineage SQL (UPSERT per stream)
10. GENERATE Silver DQ rules SQL (UPSERT per stream)
11. UPDATE sync_status with counts
12. COMMIT transaction
13. REPORT summary
```

---

## 2. Function Signatures

### Main Entry Point

```bash
sync_to_data_dictionary()
    # Orchestrates full sync (Bronze + Silver)
    # Returns: 0 on success, 1 on failure
```

### Bronze Sync Functions (Existing)

```bash
sync_bronze_metadata(config_dir, sql_file)
    # Generates Bronze layer SQL: streams, fields, sources, entity_schemas
    # Uses: TRUNCATE + INSERT strategy
```

### Silver Sync Functions (New)

```bash
sync_silver_metadata(config_dir, sql_file)
    # Orchestrates Silver layer sync
    # Calls: collect_silver_configs, generate_silver_*_sql functions
    # Uses: UPSERT strategy

collect_silver_configs(config_dir)
    # First pass: scan all configs, build associative array of unique Silver tables
    # Populates: SILVER_TABLES[table_name] = "stream1 stream2 ..."
    # Populates: SILVER_CONFIG_FILES[table_name] = "path1 path2 ..."

generate_silver_tables_sql(sql_file)
    # Generates UPSERT for data_dictionary.silver_tables
    # Uses: SILVER_TABLES associative array for source_streams aggregation

generate_silver_columns_sql(config_file, sql_file)
    # Generates UPSERT for data_dictionary.silver_columns
    # Parses: field_mappings[].target_column, type, unit, description, nullable

generate_silver_lineage_sql(config_file, sql_file)
    # Generates UPSERT for data_dictionary.silver_lineage
    # Parses: field_mappings[].source_path, target_column, transform

generate_silver_dq_rules_sql(config_file, sql_file)
    # Generates UPSERT for data_dictionary.silver_dq_rules
    # Parses: field_mappings[].dq_rules[] (column-level)
    # Parses: dq_rules[] (cross-field, table-level)

generate_standard_columns_sql(target_table, sql_file)
    # Generates UPSERT for standard columns (observation_time, ndp_id, etc.)
```

### Utility Functions

```bash
yaml_get(file, key, default)
    # Extracts scalar value from YAML
    # Handles both Python yq and Go yq variants
    # Returns: extracted value or default

yaml_array_len(file, key)
    # Returns: array length (0 if not present)

yaml_array_get(file, path, default)
    # Extracts value from YAML array element
    # Path format: ".array[$i].field"

sql_escape(string)
    # Escapes single quotes for SQL: ' -> ''
    # Returns: escaped string

sql_null(value)
    # Converts empty/null to SQL NULL keyword
    # Returns: NULL or 'value'

to_postgres_type(config_type)
    # Maps config types to PostgreSQL types
    # double_precision -> DOUBLE PRECISION
    # smallint -> SMALLINT
    # text -> TEXT

detect_transformation(config_file, mapping_index)
    # Detects transformation type from field_mapping
    # Returns: "direct", "unit_conversion", or specific transform name
```

---

## 3. YAML Parsing Logic

### Detecting Silver-Enabled Streams

```bash
# Pseudocode: Check if stream has Silver ETL enabled
function is_silver_enabled(config_file):
    enabled = yaml_get(config_file, "silver_etl.enabled", "false")
    return enabled == "true"
```

### Extracting Silver ETL Configuration

```bash
# Pseudocode: Extract all silver_etl fields from a config
function extract_silver_config(config_file):
    silver_config = {
        stream_id:      yaml_get(config_file, "stream_id", ""),
        target_table:   yaml_get(config_file, "silver_etl.target_table", ""),
        description:    yaml_get(config_file, "silver_etl.description", ""),
        grain:          yaml_get(config_file, "silver_etl.grain", ""),
        timestamp_field: yaml_get(config_file, "silver_etl.timestamp.target_field", "observation_time"),
        mapping_count:  yaml_array_len(config_file, "silver_etl.field_mappings"),
        dq_rules_count: yaml_array_len(config_file, "silver_etl.dq_rules")
    }
    return silver_config
```

### Extracting Field Mappings

```bash
# Pseudocode: Extract field_mapping at index i
function extract_field_mapping(config_file, i):
    base_path = ".silver_etl.field_mappings[$i]"
    mapping = {
        source_path:   yaml_array_get(config_file, base_path + ".source_path", ""),
        target_column: yaml_array_get(config_file, base_path + ".target_column", ""),
        type:          yaml_array_get(config_file, base_path + ".type", "text"),
        unit:          yaml_array_get(config_file, base_path + ".unit", null),
        description:   yaml_array_get(config_file, base_path + ".description", ""),
        nullable:      yaml_array_get(config_file, base_path + ".nullable", "true"),
        dq_rules_count: yaml_array_len(config_file, "silver_etl.field_mappings[$i].dq_rules")
    }
    return mapping
```

### Extracting DQ Rules

```bash
# Pseudocode: Extract column-level DQ rule at mapping index i, rule index j
function extract_column_dq_rule(config_file, i, j):
    base_path = ".silver_etl.field_mappings[$i].dq_rules[$j]"
    rule = {
        rule_name: yaml_array_get(config_file, base_path + ".rule", ""),
        action:    yaml_array_get(config_file, base_path + ".action", "flag"),
        # Capture remaining params as JSON (exclude rule, action keys)
        params:    yaml_extract_json(config_file, base_path, exclude=["rule", "action"])
    }
    return rule

# Pseudocode: Extract table-level (cross-field) DQ rule at index k
function extract_table_dq_rule(config_file, k):
    base_path = ".silver_etl.dq_rules[$k]"
    rule = {
        rule_name: yaml_array_get(config_file, base_path + ".rule", ""),
        rule_id:   yaml_array_get(config_file, base_path + ".name", ""),  # Optional identifier
        action:    yaml_array_get(config_file, base_path + ".action", "flag"),
        params:    yaml_extract_json(config_file, base_path, exclude=["rule", "action", "name"])
    }
    return rule
```

### yq Command Examples (Go yq - mikefarah/yq)

```bash
# Extract scalar value
yq eval '.silver_etl.target_table // ""' config.yaml

# Extract array length
yq eval '.silver_etl.field_mappings | length' config.yaml

# Extract array element field
yq eval '.silver_etl.field_mappings[0].target_column // ""' config.yaml

# Extract object as JSON (for rule_params), excluding certain keys
yq eval '.silver_etl.field_mappings[0].dq_rules[0] | del(.rule) | del(.action)' config.yaml -o=json

# Check if key exists
yq eval '.silver_etl.field_mappings[0].transform != null' config.yaml
```

---

## 4. SQL Generation

### 4.1 Silver Tables SQL

```bash
# Pseudocode: Generate INSERT/UPSERT for silver_tables
function generate_silver_tables_sql():
    output = "-- Silver Tables\n"

    # Iterate unique Silver tables from collection phase
    for target_table in SILVER_TABLES.keys():
        streams_list = SILVER_TABLES[target_table]  # "outdoor-weather nws-observations"
        first_config = get_first_config_for_table(target_table)

        # Extract metadata from first stream's config
        description = yaml_get(first_config, "silver_etl.description", "") | sql_escape
        grain = yaml_get(first_config, "silver_etl.grain", "") | sql_escape
        hypertable_col = yaml_get(first_config, "silver_etl.timestamp.target_field", "observation_time")
        schema_name = extract_schema(target_table)  # "silver" from "silver.weather_observations"

        # Build PostgreSQL array literal from streams
        stream_array = build_pg_array(streams_list)  # ARRAY['outdoor-weather','nws-observations']

        output += """
INSERT INTO data_dictionary.silver_tables
    (table_name, schema_name, description, grain, source_streams, hypertable_column)
VALUES
    ('${target_table}', '${schema_name}', '${description}', '${grain}',
     ${stream_array}, '${hypertable_col}')
ON CONFLICT (table_name) DO UPDATE SET
    description = EXCLUDED.description,
    grain = EXCLUDED.grain,
    source_streams = EXCLUDED.source_streams,
    hypertable_column = EXCLUDED.hypertable_column,
    updated_at = NOW();
"""
    return output
```

### 4.2 Silver Columns SQL

```bash
# Pseudocode: Generate INSERT/UPSERT for silver_columns from one config
function generate_silver_columns_sql(config_file):
    target_table = yaml_get(config_file, "silver_etl.target_table", "")
    mapping_count = yaml_array_len(config_file, "silver_etl.field_mappings")
    output = "-- Silver Columns for ${target_table}\n"

    for i in 0..(mapping_count - 1):
        mapping = extract_field_mapping(config_file, i)

        # Convert config type to PostgreSQL type
        pg_type = to_postgres_type(mapping.type)

        # Handle NULL for unit
        unit_sql = mapping.unit == null ? "NULL" : "'${mapping.unit}'"

        output += """
INSERT INTO data_dictionary.silver_columns
    (table_name, column_name, data_type, unit, description, nullable, sort_order)
VALUES
    ('${target_table}', '${mapping.target_column}', '${pg_type}',
     ${unit_sql}, '${mapping.description | sql_escape}', ${mapping.nullable}, ${i})
ON CONFLICT (table_name, column_name) DO UPDATE SET
    data_type = EXCLUDED.data_type,
    unit = EXCLUDED.unit,
    description = EXCLUDED.description,
    nullable = EXCLUDED.nullable,
    sort_order = EXCLUDED.sort_order;
"""

    # Add standard columns (observation_time, ndp_id, ingestion_time, dq_flags)
    output += generate_standard_columns_sql(target_table)

    return output
```

### 4.3 Silver Lineage SQL

```bash
# Pseudocode: Generate INSERT/UPSERT for silver_lineage from one config
function generate_silver_lineage_sql(config_file):
    stream_id = yaml_get(config_file, "stream_id", "")
    target_table = yaml_get(config_file, "silver_etl.target_table", "")
    mapping_count = yaml_array_len(config_file, "silver_etl.field_mappings")
    output = "-- Silver Lineage: ${stream_id} -> ${target_table}\n"

    for i in 0..(mapping_count - 1):
        mapping = extract_field_mapping(config_file, i)

        # Detect transformation type
        transform = detect_transformation(config_file, i)

        output += """
INSERT INTO data_dictionary.silver_lineage
    (silver_table, silver_column, source_stream, source_path, transformation)
VALUES
    ('${target_table}', '${mapping.target_column}', '${stream_id}',
     '${mapping.source_path}', '${transform}')
ON CONFLICT (silver_table, silver_column, source_stream) DO UPDATE SET
    source_path = EXCLUDED.source_path,
    transformation = EXCLUDED.transformation;
"""
    return output
```

### 4.4 Silver DQ Rules SQL

```bash
# Pseudocode: Generate INSERT/UPSERT for silver_dq_rules from one config
function generate_silver_dq_rules_sql(config_file):
    target_table = yaml_get(config_file, "silver_etl.target_table", "")
    mapping_count = yaml_array_len(config_file, "silver_etl.field_mappings")
    output = "-- Silver DQ Rules for ${target_table}\n"

    # =========================================
    # COLUMN-LEVEL DQ RULES (from field_mappings)
    # =========================================
    for i in 0..(mapping_count - 1):
        mapping = extract_field_mapping(config_file, i)
        rule_count = mapping.dq_rules_count

        for j in 0..(rule_count - 1):
            rule = extract_column_dq_rule(config_file, i, j)

            # Convert params to JSON string
            params_json = rule.params | to_json | sql_escape

            output += """
INSERT INTO data_dictionary.silver_dq_rules
    (silver_table, silver_column, rule_name, rule_params, action)
VALUES
    ('${target_table}', '${mapping.target_column}', '${rule.rule_name}',
     '${params_json}'::jsonb, '${rule.action}')
ON CONFLICT (silver_table, silver_column, rule_name) DO UPDATE SET
    rule_params = EXCLUDED.rule_params,
    action = EXCLUDED.action;
"""

    # =========================================
    # TABLE-LEVEL DQ RULES (cross-field rules)
    # =========================================
    table_rule_count = yaml_array_len(config_file, "silver_etl.dq_rules")

    for k in 0..(table_rule_count - 1):
        rule = extract_table_dq_rule(config_file, k)

        # Create composite rule name for cross-field rules
        # Format: rule_type:identifier (e.g., "cross_field_check:pm10_gte_pm25")
        rule_identifier = rule.rule_id != "" ? rule.rule_id : "rule_${k}"
        composite_name = "${rule.rule_name}:${rule_identifier}"

        params_json = rule.params | to_json | sql_escape

        # Note: silver_column is NULL for cross-field rules
        output += """
INSERT INTO data_dictionary.silver_dq_rules
    (silver_table, silver_column, rule_name, rule_params, action)
VALUES
    ('${target_table}', NULL, '${composite_name}',
     '${params_json}'::jsonb, '${rule.action}')
ON CONFLICT (silver_table, silver_column, rule_name) DO UPDATE SET
    rule_params = EXCLUDED.rule_params,
    action = EXCLUDED.action;
"""
    return output
```

---

## 5. Multi-Stream Handling

### Problem

Multiple streams can feed the same Silver table:

```yaml
# outdoor-weather/config.yaml
silver_etl:
  target_table: silver.weather_observations
  field_mappings:
    - source_path: raw_payload.main.temp
      target_column: temperature_c
      ...

# nws-observations/config.yaml
silver_etl:
  target_table: silver.weather_observations
  field_mappings:
    - source_path: raw_payload.properties.temperature.value
      target_column: temperature_c
      ...
```

Both contribute to `silver.weather_observations` but with different source paths.

### Solution: Two-Pass Collection

```bash
# Pseudocode: collect_silver_configs
function collect_silver_configs(config_dir):
    # Associative arrays (bash 4.0+)
    declare -A SILVER_TABLES      # table_name -> space-separated stream_ids
    declare -A SILVER_CONFIG_FILES # table_name -> space-separated config paths

    # FIRST PASS: Collect all Silver configs
    for config_file in glob("${config_dir}/*/config.yaml"):
        if not is_silver_enabled(config_file):
            continue

        stream_id = yaml_get(config_file, "stream_id", "")
        target_table = yaml_get(config_file, "silver_etl.target_table", "")

        # Accumulate streams per table (space-separated)
        SILVER_TABLES[target_table] += "${stream_id} "
        SILVER_CONFIG_FILES[target_table] += "${config_file} "

    # After collection, SILVER_TABLES looks like:
    # SILVER_TABLES["silver.weather_observations"] = "outdoor-weather nws-observations "
    # SILVER_TABLES["silver.air_quality_observations"] = "air-quality "

    # Export for use by generation functions
    export SILVER_TABLES
    export SILVER_CONFIG_FILES
```

### Building source_streams Array

```bash
# Pseudocode: Convert stream list to PostgreSQL array literal
function build_pg_array(space_separated_list):
    # Input: "outdoor-weather nws-observations "
    # Output: ARRAY['outdoor-weather','nws-observations']

    # Trim trailing space, split, sort, unique
    streams = space_separated_list | trim | split(" ") | sort | unique

    # Build array literal
    array_items = streams.map(s => "'${s}'").join(",")
    return "ARRAY[${array_items}]"
```

### Column Deduplication Strategy

When multiple streams define the same target_column, we rely on UPSERT behavior:

1. First stream's column definition is INSERTed
2. Second stream's definition triggers ON CONFLICT DO UPDATE
3. Last-writer-wins, but configs should be consistent

```sql
-- If outdoor-weather and nws-observations both define temperature_c:
-- First insert wins, subsequent upserts update if different
INSERT INTO data_dictionary.silver_columns
    (table_name, column_name, data_type, unit, description, nullable, sort_order)
VALUES ('silver.weather_observations', 'temperature_c', 'DOUBLE PRECISION',
        'celsius', 'Ambient temperature', true, 0)
ON CONFLICT (table_name, column_name) DO UPDATE SET
    data_type = EXCLUDED.data_type,
    unit = EXCLUDED.unit,
    ...
```

### Lineage Preserves All Sources

Unlike columns, lineage preserves all source mappings:

```sql
-- Lineage from outdoor-weather
INSERT INTO data_dictionary.silver_lineage
    (silver_table, silver_column, source_stream, source_path, transformation)
VALUES ('silver.weather_observations', 'temperature_c', 'outdoor-weather',
        'raw_payload.main.temp', 'direct')
ON CONFLICT (silver_table, silver_column, source_stream) DO UPDATE ...

-- Lineage from nws-observations (different source_stream = different row)
INSERT INTO data_dictionary.silver_lineage
    (silver_table, silver_column, source_stream, source_path, transformation)
VALUES ('silver.weather_observations', 'temperature_c', 'nws-observations',
        'raw_payload.properties.temperature.value', 'direct')
ON CONFLICT (silver_table, silver_column, source_stream) DO UPDATE ...
```

Unique constraint `(silver_table, silver_column, source_stream)` ensures both mappings are preserved.

---

## 6. Idempotency Strategy

### Bronze Layer: TRUNCATE + INSERT

```sql
-- Bronze uses full refresh (existing behavior)
DELETE FROM data_dictionary.entity_schema_attributes;
DELETE FROM data_dictionary.entity_schemas;
DELETE FROM data_dictionary.sources;
DELETE FROM data_dictionary.fields;
DELETE FROM data_dictionary.streams;

INSERT INTO data_dictionary.streams (...) VALUES (...);
-- ... more inserts
```

**Why TRUNCATE works for Bronze:**
- One stream = one row (no multi-source aggregation)
- Clean slate is simpler to reason about
- No accumulation across configs

### Silver Layer: UPSERT (INSERT ... ON CONFLICT DO UPDATE)

```sql
-- Silver uses UPSERT (NO truncate)
INSERT INTO data_dictionary.silver_tables (...) VALUES (...)
ON CONFLICT (table_name) DO UPDATE SET
    description = EXCLUDED.description,
    grain = EXCLUDED.grain,
    source_streams = EXCLUDED.source_streams,
    ...

INSERT INTO data_dictionary.silver_columns (...) VALUES (...)
ON CONFLICT (table_name, column_name) DO UPDATE SET
    data_type = EXCLUDED.data_type,
    ...

INSERT INTO data_dictionary.silver_lineage (...) VALUES (...)
ON CONFLICT (silver_table, silver_column, source_stream) DO UPDATE SET
    source_path = EXCLUDED.source_path,
    ...

INSERT INTO data_dictionary.silver_dq_rules (...) VALUES (...)
ON CONFLICT (silver_table, silver_column, rule_name) DO UPDATE SET
    rule_params = EXCLUDED.rule_params,
    ...
```

**Why UPSERT works for Silver:**
- Multiple streams contribute to same table
- Processing stream A must not erase stream B's contributions
- Re-running sync produces identical result (idempotent)
- Updated fields overwrite stale values

### Unique Constraints Required

```sql
-- silver_tables: natural PK
PRIMARY KEY (table_name)

-- silver_columns: one definition per column per table
UNIQUE (table_name, column_name)

-- silver_lineage: one mapping per column per source stream
UNIQUE (silver_table, silver_column, source_stream)

-- silver_dq_rules: one rule definition per column per rule type
-- Note: silver_column can be NULL for cross-field rules
UNIQUE (silver_table, COALESCE(silver_column, ''), rule_name)
-- Or use partial unique index for NULL handling
```

### Handling Deleted Configs

UPSERT alone does not remove orphaned rows (e.g., if a stream is disabled or removed).

**Options:**

1. **Periodic cleanup job** (recommended for now):
   ```sql
   -- Run after sync to remove orphaned lineage
   DELETE FROM data_dictionary.silver_lineage
   WHERE source_stream NOT IN (SELECT stream_id FROM data_dictionary.streams WHERE enabled = true);
   ```

2. **Mark-and-sweep during sync** (future enhancement):
   - Mark all existing Silver rows as "pending_delete"
   - UPSERT clears the flag for touched rows
   - Delete rows still flagged after sync

3. **Version stamping**:
   - Add `sync_version` column
   - Set current version during sync
   - Delete rows with old version after sync

For MVP, periodic cleanup is sufficient.

---

## 7. Error Handling

### Transaction Wrapper

```bash
# Pseudocode: Main sync function with error handling
function sync_to_data_dictionary():
    log "Syncing Data Dictionary to TimescaleDB..."

    # Wait for database readiness
    until dcx timescaledb pg_isready -U postgres -d ndp:
        warn "Waiting for TimescaleDB..."
        sleep 2

    SQL_FILE="/tmp/data_dictionary_sync_$$.sql"
    ERROR_FILE="/tmp/data_dictionary_error_$$.log"

    # Generate SQL
    {
        echo "-- Data Dictionary Sync"
        echo "-- Generated: $(date -Iseconds)"
        echo ""
        echo "BEGIN;"
        echo ""

        # Record sync start
        echo "INSERT INTO data_dictionary.sync_status (sync_type, status) VALUES ('full', 'running');"
        echo ""

        # Bronze sync (existing)
        sync_bronze_metadata "$CONFIG_DIR" "$SQL_FILE"

        # Silver sync (new)
        collect_silver_configs "$CONFIG_DIR"
        sync_silver_metadata "$CONFIG_DIR" "$SQL_FILE"

        # Update sync status
        echo ""
        echo "-- Update sync status"
        echo "UPDATE data_dictionary.sync_status"
        echo "SET completed_at = NOW(),"
        echo "    status = 'success',"
        echo "    streams_synced = (SELECT COUNT(*) FROM data_dictionary.streams),"
        echo "    schemas_synced = (SELECT COUNT(*) FROM data_dictionary.entity_schemas),"
        echo "    silver_tables_synced = (SELECT COUNT(*) FROM data_dictionary.silver_tables),"
        echo "    silver_columns_synced = (SELECT COUNT(*) FROM data_dictionary.silver_columns)"
        echo "WHERE status = 'running' AND completed_at IS NULL;"
        echo ""
        echo "COMMIT;"

    } > "$SQL_FILE"

    # Execute with error capture
    if dcx timescaledb psql -U postgres -d ndp \
            -v ON_ERROR_STOP=1 < "$SQL_FILE" 2> "$ERROR_FILE"; then
        log "Data Dictionary sync successful"
        cleanup_files
        show_summary
        return 0
    else
        handle_sync_failure
        return 1
    fi
```

### Error Handling Function

```bash
function handle_sync_failure():
    error "Data Dictionary sync FAILED"

    # Log error details
    if [ -s "$ERROR_FILE" ]; then
        echo "Error details:"
        cat "$ERROR_FILE"
    fi

    # Transaction already rolled back by ON_ERROR_STOP
    # Update sync_status to record failure (separate transaction)
    dcx timescaledb psql -U postgres -d ndp -c \
        "UPDATE data_dictionary.sync_status
         SET completed_at = NOW(), status = 'failed'
         WHERE status = 'running';" 2>/dev/null || true

    # Preserve SQL file for debugging
    ERROR_SQL="/tmp/data_dictionary_sync_failed_$(date +%Y%m%d_%H%M%S).sql"
    mv "$SQL_FILE" "$ERROR_SQL"
    warn "Failed SQL preserved at: $ERROR_SQL"

    cleanup_files
```

### YAML Parsing Error Handling

```bash
function yaml_get(file, key, default):
    # Attempt extraction with yq
    result=$(yq eval ".$key // \"$default\"" "$file" 2>/dev/null)
    exit_code=$?

    if [ $exit_code -ne 0 ]; then
        warn "YAML parse error for key '$key' in $file"
        echo "$default"
        return 0  # Continue with default, don't fail sync
    fi

    # Handle null/empty
    if [ -z "$result" ] || [ "$result" = "null" ]; then
        echo "$default"
    else
        echo "$result"
    fi
```

### Partial Success Considerations

The entire sync runs in a single transaction:
- **Success**: All Bronze + Silver metadata updated atomically
- **Failure**: Entire transaction rolled back, previous state preserved

This ensures:
1. No partial updates visible to queries
2. Dashboards always see consistent state
3. Re-running sync after fixing issue produces correct result

---

## 8. Complete Algorithm Flow

```
sync_to_data_dictionary()
│
├── Wait for TimescaleDB
│
├── Generate SQL File
│   │
│   ├── BEGIN;
│   │
│   ├── INSERT sync_status (running)
│   │
│   ├── ┌─ BRONZE SYNC (existing) ─────────────────┐
│   │   │ DELETE streams, fields, sources, etc.     │
│   │   │ for each stream config:                   │
│   │   │   INSERT INTO streams (...)               │
│   │   │   INSERT INTO entity_schemas (...)        │
│   │   │   INSERT INTO entity_schema_attributes    │
│   │   └───────────────────────────────────────────┘
│   │
│   ├── ┌─ SILVER SYNC (new) ──────────────────────┐
│   │   │                                           │
│   │   │ PHASE 1: Collect Silver Configs           │
│   │   │   for each stream config:                 │
│   │   │     if silver_etl.enabled:                │
│   │   │       SILVER_TABLES[target] += stream_id  │
│   │   │                                           │
│   │   │ PHASE 2: Generate Silver Tables SQL       │
│   │   │   for each unique target_table:           │
│   │   │     UPSERT INTO silver_tables             │
│   │   │       (table_name, source_streams[], ...) │
│   │   │                                           │
│   │   │ PHASE 3: Generate Per-Stream SQL          │
│   │   │   for each silver-enabled config:         │
│   │   │     │                                     │
│   │   │     ├─ generate_silver_columns_sql        │
│   │   │     │    for each field_mapping:          │
│   │   │     │      UPSERT INTO silver_columns     │
│   │   │     │                                     │
│   │   │     ├─ generate_silver_lineage_sql        │
│   │   │     │    for each field_mapping:          │
│   │   │     │      UPSERT INTO silver_lineage     │
│   │   │     │                                     │
│   │   │     └─ generate_silver_dq_rules_sql       │
│   │   │          for each column dq_rule:         │
│   │   │            UPSERT INTO silver_dq_rules    │
│   │   │          for each table dq_rule:          │
│   │   │            UPSERT INTO silver_dq_rules    │
│   │   │            (silver_column = NULL)         │
│   │   └───────────────────────────────────────────┘
│   │
│   ├── UPDATE sync_status (success, counts)
│   │
│   └── COMMIT;
│
├── Execute SQL
│   │
│   ├── ON SUCCESS:
│   │   ├── Log success
│   │   ├── Show summary (counts from sync_status)
│   │   └── Cleanup temp files
│   │
│   └── ON FAILURE (ON_ERROR_STOP triggers rollback):
│       ├── Log error details
│       ├── Update sync_status to 'failed'
│       ├── Preserve failed SQL for debugging
│       └── Return exit code 1
│
└── Return exit code 0
```

---

## 9. Appendix: Type Mappings

### Config Type to PostgreSQL Type

| Config Type | PostgreSQL Type |
|-------------|-----------------|
| `double_precision` | `DOUBLE PRECISION` |
| `smallint` | `SMALLINT` |
| `integer` | `INTEGER` |
| `bigint` | `BIGINT` |
| `text` | `TEXT` |
| `boolean` | `BOOLEAN` |
| `timestamptz` | `TIMESTAMPTZ` |
| `float` | `DOUBLE PRECISION` |
| `int` | `INTEGER` |

### Transformation Detection Logic

```bash
function detect_transformation(config_file, mapping_index):
    base_path = ".silver_etl.field_mappings[${mapping_index}]"

    # Check if transform key exists
    has_transform = yaml_eval("$base_path.transform != null", config_file)

    if not has_transform:
        return "direct"

    # Extract transform type
    transform_type = yaml_get(config_file, "$base_path.transform.type", "direct")

    # For unit conversions, include from/to for clarity
    if transform_type == "unit_conversion":
        from_unit = yaml_get(config_file, "$base_path.transform.from", "")
        to_unit = yaml_get(config_file, "$base_path.transform.to", "")
        return "unit_conversion:${from_unit}_to_${to_unit}"

    return transform_type
```

### Standard Columns

Every Silver table includes these standard columns:

| Column | Type | Description |
|--------|------|-------------|
| `observation_time` | `TIMESTAMPTZ` | Observation timestamp (PK, hypertable time) |
| `ndp_id` | `TEXT` | NDP source identifier (PK) |
| `ingestion_time` | `TIMESTAMPTZ` | When ETL processed the record |
| `dq_flags` | `TEXT[]` | Data quality violation flags |

---

## 10. References

- [ADR-009-001](../architecture/ADR-009-001-silver-dictionary-tables.md) - Silver Dictionary Tables Schema
- [ADR-009-002](../architecture/ADR-009-002-config-schema-extension.md) - Config Schema Extension
- [ADR-009-003](../architecture/ADR-009-003-sync-mechanism.md) - Sync Mechanism Architecture
- [deploy.sh](../../../../deploy/pi/deploy.sh) - Current sync implementation (lines 146-343)
- [air-quality config](../../../../config/base/streams/air-quality/config.yaml) - Example silver_etl config

---

**Last Updated**: 2026-01-16
