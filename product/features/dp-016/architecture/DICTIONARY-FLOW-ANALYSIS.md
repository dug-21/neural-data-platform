# Data Dictionary Flow Analysis

**Feature**: dp-016
**Author**: NDP Analytics Engineer
**Date**: 2026-02-01

## Executive Summary

This document analyzes how `entity_schemas` defined in YAML stream configurations flow to the `data_dictionary` schema in TimescaleDB, and how MCP tools consume that metadata.

---

## 1. Sync Mechanism: How entity_schemas Get to data_dictionary Tables

### 1.1 The Sync Command

The sync is triggered via `deploy.sh sync-dictionary`:

**File**: `deploy/pi/deploy.sh:34`
```bash
#   sync-dictionary - Sync entity schemas to TimescaleDB data dictionary
```

**File**: `deploy/pi/deploy.sh:1383-1384`
```bash
sync-dictionary)
    sync_to_data_dictionary
```

### 1.2 The sync_to_data_dictionary Function

**File**: `deploy/pi/deploy.sh:347-805`

This is a ~450 line Bash function that:

1. **Waits for TimescaleDB** (lines 351-354):
```bash
until dcx timescaledb pg_isready -U postgres -d ndp >/dev/null 2>&1; do
    warn "Waiting for TimescaleDB to be ready..."
    sleep 2
done
```

2. **Iterates through stream config directories** (lines 379-440):
```bash
for config_dir in "$CONFIG_DIR"/*/; do
    if [ -f "$config_dir/config.yaml" ]; then
        local stream_id=$(basename "$config_dir")
        local config_file="$config_dir/config.yaml"
```

3. **Extracts entity_schemas using YAML helpers** (lines 399-437):
```bash
# Process entity_schemas if present
local es_count=$(yaml_array_len "$config_file" "entity_schemas")
if [ "$es_count" -gt 0 ]; then
    for i in $(seq 0 $((es_count - 1))); do
        local schema_name=$(yaml_array_get "$config_file" ".entity_schemas[$i].schema_name" "")
        local schema_desc=$(yaml_array_get "$config_file" ".entity_schemas[$i].description" "")
        local device_class=$(yaml_array_get "$config_file" ".entity_schemas[$i].device_class" "null")
```

4. **Generates INSERT statements for Bronze metadata** (lines 412-434):
```bash
echo "INSERT INTO data_dictionary.entity_schemas (stream_id, schema_name, description, device_class)"
echo "VALUES ('$stream_id', '$schema_name', '$schema_desc', $device_class);"

# Process attributes
local attr_count=$(yaml_array_len "$config_file" "entity_schemas[$i].attributes")
for j in $(seq 0 $((attr_count - 1))); do
    echo "INSERT INTO data_dictionary.entity_schema_attributes ..."
```

5. **Processes silver_etl field_mappings** (lines 559-692):
```bash
# Process field_mappings for columns, lineage, and column-level DQ rules
local fm_count=$(yaml_array_len "$config_file" "silver_etl.field_mappings")

for i in $(seq 0 $((fm_count - 1))); do
    # UPSERT silver_columns
    echo "INSERT INTO data_dictionary.silver_columns ..."

    # UPSERT silver_lineage
    echo "INSERT INTO data_dictionary.silver_lineage ..."

    # Process column-level DQ rules
    echo "INSERT INTO data_dictionary.silver_dq_rules ..."
```

### 1.3 Two-Pass Algorithm for Silver Tables

The sync uses a two-pass approach because multiple Bronze streams can feed the same Silver table:

**Pass 1** (lines 473-506): Collect all Silver-enabled streams and group by target table
```bash
declare -A SILVER_TABLES
declare -A SILVER_DESCRIPTIONS
# ...
for config_dir in "$CONFIG_DIR"/*/; do
    local target_table=$(yaml_get "$config_file" "silver_etl.target_table" "")
    SILVER_TABLES[$target_table]="${SILVER_TABLES[$target_table]} $stream_id"
```

**Pass 2** (lines 509-555): Generate UPSERT SQL with array of source streams
```bash
INSERT INTO data_dictionary.silver_tables (table_name, schema_name, description, grain, source_streams, hypertable_column)
VALUES ('$target_table', '$schema_name', $desc_sql, $grain_sql, $pg_array, '$timestamp_col')
ON CONFLICT (table_name) DO UPDATE SET ...
```

---

## 2. Storage Location: data_dictionary Schema in TimescaleDB

### 2.1 Schema Creation

**File**: `deploy/pi/init-scripts/01-create-data-dictionary.sql:5`
```sql
CREATE SCHEMA IF NOT EXISTS data_dictionary;
```

### 2.2 Bronze Layer Tables (DP-002)

**File**: `deploy/pi/init-scripts/01-create-data-dictionary.sql:8-93`

| Table | Purpose | Key Columns |
|-------|---------|-------------|
| `data_dictionary.streams` | Stream metadata | `stream_id`, `description`, `version`, `retention_days` |
| `data_dictionary.fields` | Bronze field definitions | `stream_id`, `field_name`, `field_type`, `unit` |
| `data_dictionary.sources` | Source configurations | `stream_id`, `source_id`, `source_type`, `config` (JSONB) |
| `data_dictionary.entity_schemas` | Entity schemas | `stream_id`, `schema_name`, `device_class` |
| `data_dictionary.entity_schema_attributes` | Schema attributes | `schema_id`, `attribute_name`, `attribute_type`, `unit` |
| `data_dictionary.sync_status` | Sync audit trail | `sync_type`, `status`, `streams_synced` |

### 2.3 Silver Layer Tables (DP-009)

**File**: `deploy/pi/init-scripts/003_silver_data_dictionary.sql:19-171`

| Table | Purpose | Key Columns |
|-------|---------|-------------|
| `data_dictionary.silver_tables` | Silver table metadata | `table_name`, `source_streams[]`, `hypertable_column`, `grain` |
| `data_dictionary.silver_columns` | Column definitions | `table_name`, `column_name`, `data_type`, `unit`, `nullable` |
| `data_dictionary.silver_lineage` | Bronze-to-Silver mapping | `silver_table`, `silver_column`, `source_stream`, `source_path`, `transformation` |
| `data_dictionary.silver_dq_rules` | DQ rule definitions | `silver_table`, `silver_column`, `rule_name`, `rule_params` (JSONB), `action` |

### 2.4 Unified Views

**File**: `deploy/pi/init-scripts/003_silver_data_dictionary.sql:238-394`

| View | Purpose |
|------|---------|
| `v_complete_dictionary` | UNION of Bronze `fields` and Silver `silver_columns` |
| `v_silver_table_overview` | Silver tables with column/lineage/DQ counts |
| `v_lineage` | Full Bronze-to-Silver lineage with metadata |
| `v_dq_rules_summary` | DQ rules with column context |
| `v_column_search` | Searchable view across both layers |

---

## 3. MCP Usage: How MCP Tools Query the Data Dictionary

### 3.1 TimescaleDictionaryStore Adapter

**File**: `core/ndp-mcp-server/src/storage/timescale_dictionary.rs`

The `TimescaleDictionaryStore` implements the `DictionaryStore` trait and queries the data_dictionary schema.

**Key methods**:

#### 3.1.1 search() - Query v_complete_dictionary

**File**: `core/ndp-mcp-server/src/storage/timescale_dictionary.rs:158-217`
```rust
async fn search(&self, query: &str, layer: Option<String>) -> McpResult<Vec<DictionaryEntry>> {
    let rows = conn
        .query(
            r#"
            SELECT layer, entity, column_name, data_type, unit, description
            FROM data_dictionary.v_complete_dictionary
            WHERE ($1 = 'all' OR layer = $1)
              AND (column_name ILIKE '%' || $2 || '%'
                   OR description ILIKE '%' || $2 || '%')
            ORDER BY layer, entity, column_name
            LIMIT 50
            "#,
            &[&layer_filter, &query],
        )
        .await?;
```

#### 3.1.2 describe_column() - Query silver_columns with lineage

**File**: `core/ndp-mcp-server/src/storage/timescale_dictionary.rs:533-653`
```rust
async fn describe_silver_column(&self, conn: &PgConnection<'_>, table_name: &str, column_name: &str) {
    let row = conn.query_opt(
        r#"
        SELECT sc.data_type, sc.unit, sc.description, sc.nullable,
               sl.source_stream, sl.source_path, sl.transformation
        FROM data_dictionary.silver_columns sc
        LEFT JOIN data_dictionary.silver_lineage sl
            ON sc.table_name = sl.silver_table
           AND sc.column_name = sl.silver_column
        WHERE sc.table_name = $1 AND sc.column_name = $2
        "#,
        &[&table_name, &column_name],
    ).await?;
```

#### 3.1.3 trace_lineage() - Query silver_lineage + silver_dq_rules

**File**: `core/ndp-mcp-server/src/storage/timescale_dictionary.rs:262-404`
```rust
async fn trace_lineage(&self, silver_table: &str, silver_column: &str) {
    // Get lineage sources
    let lineage_rows = conn.query(
        r#"
        SELECT l.source_stream, l.source_path, l.transformation,
               f.field_type AS bronze_type, f.unit AS bronze_unit
        FROM data_dictionary.silver_lineage l
        LEFT JOIN data_dictionary.fields f
            ON l.source_stream = f.stream_id
        WHERE l.silver_table = $1 AND l.silver_column = $2
        "#,
    ).await?;

    // Get DQ rules
    let dq_rows = conn.query(
        r#"
        SELECT rule_name, rule_params, action, ...
        FROM data_dictionary.silver_dq_rules
        WHERE silver_table = $1
          AND (silver_column = $2 OR silver_column IS NULL)
        "#,
    ).await?;
```

#### 3.1.4 list_dq_rules() - Query silver_dq_rules

**File**: `core/ndp-mcp-server/src/storage/timescale_dictionary.rs:411-530`
```rust
async fn list_dq_rules(&self, table: Option<String>, column: Option<String>) {
    let rows = conn.query(
        r#"
        SELECT silver_table, silver_column, rule_name, rule_params, action,
               CASE WHEN silver_column IS NULL THEN 'cross-field' ELSE 'column' END AS scope
        FROM data_dictionary.silver_dq_rules
        WHERE silver_table = $1 OR silver_table = $2
        ORDER BY ...
        "#,
    ).await?;
```

### 3.2 MCP Tool Implementations

**File**: `core/ndp-mcp-server/src/mcp/tools/query_dictionary.rs:114-172`

The `query_dictionary` tool calls `dictionary.search()`:
```rust
pub async fn execute<D>(dictionary: &D, args: serde_json::Value) -> McpResult<McpToolResult>
where D: DictionaryStore + ?Sized,
{
    let entries = dictionary.search(&query, layer_filter).await?;
    // Build response with results
}
```

---

## 4. Relationship to Silver: field_mappings vs entity_schemas

### 4.1 Two Distinct Metadata Sources

The YAML config files contain **two** metadata structures:

| Section | Purpose | Used By |
|---------|---------|---------|
| `entity_schemas` | Bronze layer documentation (DP-002) | Data catalog, documentation |
| `silver_etl.field_mappings` | Silver ETL configuration (DP-006) | ETL process, data dictionary |

### 4.2 entity_schemas (Bronze Documentation)

**File**: `config/base/streams/air-quality/config.yaml:102-148`
```yaml
entity_schemas:
  - schema_name: airgradient
    description: AirGradient indoor air quality sensors
    device_class: air_quality
    attributes:
      - name: pm25
        type: float
        unit: ug/m3
        description: Particulate Matter 2.5 micrometers
```

**Populates tables**:
- `data_dictionary.entity_schemas` (schema metadata)
- `data_dictionary.entity_schema_attributes` (attribute metadata)

### 4.3 silver_etl.field_mappings (ETL Configuration)

**File**: `config/base/streams/air-quality/config.yaml:169-256`
```yaml
silver_etl:
  enabled: true
  target_table: silver.air_quality_observations
  field_mappings:
    - source_path: raw_payload.pm02Compensated
      target_column: pm25
      type: double_precision
      unit: "ug/m3"
      description: "PM2.5 concentration (humidity-compensated)"
      dq_rules:
        - rule: range_check
          min: 0.0
          max: 1000.0
```

**Populates tables**:
- `data_dictionary.silver_tables` (table metadata)
- `data_dictionary.silver_columns` (column definitions)
- `data_dictionary.silver_lineage` (Bronze-to-Silver mapping)
- `data_dictionary.silver_dq_rules` (DQ rule definitions)

### 4.4 Key Differences

| Aspect | entity_schemas | field_mappings |
|--------|----------------|----------------|
| Layer | Bronze | Silver |
| Purpose | Documentation | ETL execution |
| Field names | Raw sensor names (`pm25`) | Target column names (`pm25`) |
| Source path | Not specified | `raw_payload.pm02Compensated` |
| Transformation | Not specified | `direct`, `unit_conversion`, etc. |
| DQ rules | Not specified | Inline with mappings |
| Lineage | N/A | Bronze path -> Silver column |

### 4.5 Why Both Exist

1. **entity_schemas**: Documents what sensors produce (raw Bronze data)
2. **field_mappings**: Documents how Bronze transforms to Silver

The `field_mappings` is the **authoritative source** for:
- Silver column definitions
- Data lineage (Bronze -> Silver)
- DQ rules applied during ETL
- Unit conversions and transformations

---

## 5. Data Flow Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        YAML CONFIGURATION FILES                              │
│                   config/base/streams/*/config.yaml                          │
└──────────────────────────────────┬──────────────────────────────────────────┘
                                   │
         ┌─────────────────────────┴─────────────────────────┐
         │                                                    │
         ▼                                                    ▼
┌─────────────────────┐                          ┌─────────────────────────┐
│   entity_schemas    │                          │   silver_etl section    │
│   (Bronze docs)     │                          │   (ETL config)          │
└─────────┬───────────┘                          └───────────┬─────────────┘
          │                                                  │
          │                                                  │
          ▼                                                  ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                     deploy.sh sync_to_data_dictionary()                      │
│                        deploy/pi/deploy.sh:347-805                           │
│                                                                              │
│   1. Parse YAML with yaml_get/yaml_array_get helpers                        │
│   2. Generate SQL INSERT statements                                          │
│   3. Execute against TimescaleDB                                             │
└──────────────────────────────────┬──────────────────────────────────────────┘
                                   │
                                   ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                     TimescaleDB: data_dictionary schema                      │
├─────────────────────────────────────────────────────────────────────────────┤
│  BRONZE TABLES (DP-002)              │  SILVER TABLES (DP-009)              │
│  ───────────────────────             │  ────────────────────────            │
│  • streams                           │  • silver_tables                     │
│  • fields                            │  • silver_columns                    │
│  • sources                           │  • silver_lineage                    │
│  • entity_schemas                    │  • silver_dq_rules                   │
│  • entity_schema_attributes          │                                      │
│  • sync_status                       │                                      │
├─────────────────────────────────────────────────────────────────────────────┤
│  UNIFIED VIEWS                                                               │
│  ─────────────                                                               │
│  • v_complete_dictionary (Bronze UNION Silver columns)                       │
│  • v_lineage (Bronze -> Silver with metadata)                               │
│  • v_dq_rules_summary (DQ rules with context)                               │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │
                                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                     MCP Server: TimescaleDictionaryStore                     │
│              core/ndp-mcp-server/src/storage/timescale_dictionary.rs         │
├─────────────────────────────────────────────────────────────────────────────┤
│  Trait: DictionaryStore                                                      │
│  ─────────────────────                                                       │
│  • search() -> v_complete_dictionary                                         │
│  • describe_column() -> silver_columns + silver_lineage                      │
│  • trace_lineage() -> silver_lineage + fields + silver_dq_rules             │
│  • list_dq_rules() -> silver_dq_rules                                       │
└──────────────────────────────────────┬──────────────────────────────────────┘
                                       │
                                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                           MCP TOOLS                                          │
│                core/ndp-mcp-server/src/mcp/tools/                            │
├─────────────────────────────────────────────────────────────────────────────┤
│  • query_dictionary   - Search columns by name/description                   │
│  • describe_column    - Get detailed column metadata                         │
│  • trace_lineage      - Follow Bronze -> Silver path                        │
│  • list_dq_rules      - List DQ rules for table/column                      │
│  • describe_silver_table - Get Silver table metadata                        │
│  • list_silver_tables - List all Silver tables                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 6. Code Evidence Summary

| Question | Answer | Evidence |
|----------|--------|----------|
| **Sync Mechanism** | Bash script generates SQL from YAML | `deploy/pi/deploy.sh:347-805` |
| **Storage Location** | TimescaleDB `data_dictionary` schema | `deploy/pi/init-scripts/01-create-data-dictionary.sql:5` |
| **MCP Usage** | `TimescaleDictionaryStore` queries views/tables | `core/ndp-mcp-server/src/storage/timescale_dictionary.rs:148-530` |
| **Relationship** | `entity_schemas` = Bronze docs; `field_mappings` = Silver ETL | Config files: `config/base/streams/*/config.yaml` |

---

## 7. Recommendations for Analytics Work

1. **Query Silver metadata first**: Use `data_dictionary.silver_columns` for column definitions and units

2. **Use lineage for transformations**: Check `data_dictionary.silver_lineage` to understand Bronze -> Silver transforms

3. **Consult DQ rules**: Check `data_dictionary.silver_dq_rules` to understand data quality constraints

4. **Use unified views**: `v_complete_dictionary` provides a single view across both layers

5. **Sync after config changes**: Always run `./deploy.sh sync-dictionary` after modifying YAML configs
