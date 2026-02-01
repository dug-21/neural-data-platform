# Dimension and Data Dictionary Systems Research

**Feature**: dp-016
**Author**: ndp-analytics-engineer
**Date**: 2026-02-01
**Status**: Research Complete

---

## Executive Summary

This document details how entity metadata (dimensions) and column documentation (data dictionary) are managed in NDP. The platform has two distinct but complementary systems:

1. **Dimension Tables** - Reference data in Silver layer (e.g., `entity_context`) loaded from CSV files
2. **Data Dictionary** - Column-level metadata in `data_dictionary` schema, used by MCP tools

Both systems support the goal of enriching timeseries observations with context and providing discoverability for data consumers.

---

## 1. Dimension Tables System

### 1.1 File Locations

| Artifact Type | Path | Purpose |
|---------------|------|---------|
| **CSV Source Data** | `/workspaces/neural-data-platform/data/dimensions/entity_context.csv` | Raw dimension data |
| **YAML Config** | `/workspaces/neural-data-platform/config/base/dimensions/entity_context.yaml` | Schema definition (single source of truth) |
| **SQL DDL (advanced)** | `/workspaces/neural-data-platform/deploy/pi/sql/dimensions/entity_context.sql` | Full DDL with triggers, comments |
| **SQL Init** | `/workspaces/neural-data-platform/deploy/pi/sql/dimensions/init.sql` | Master initialization script |
| **SQL Sync Functions** | `/workspaces/neural-data-platform/deploy/pi/sql/dimensions/sync_functions.sql` | Helper functions for loading |
| **Docker Init Script** | `/workspaces/neural-data-platform/deploy/pi/init-scripts/04-dimension-tables.sql` | Auto-runs on container start |
| **Rust Module** | `/workspaces/neural-data-platform/core/src/dimensions/mod.rs` | Dimension loader module |
| **Rust Loader** | `/workspaces/neural-data-platform/core/src/dimensions/loader.rs` | CSV parsing and loading logic |
| **Rust DDL Generator** | `/workspaces/neural-data-platform/core/src/dimensions/ddl.rs` | Generates SQL from config |

### 1.2 Current Dimensions

Only one dimension exists:

| Dimension ID | Target Table | Columns |
|--------------|--------------|---------|
| `entity_context` | `silver.entity_context` | ndp_id, category, friendly_name, location_path, correlates_with, orientation |

### 1.3 Entity Context CSV Format

```csv
ndp_id,category,friendly_name,location_path,correlates_with,orientation
temp_living,temperature,Living Room Temperature,home/living_room,{humidity_living},
humidity_living,humidity,Living Room Humidity,home/living_room,{temp_living},
door_back,door,Back Door,home/living_room,{aq_airgradient_1},south
```

**Key columns:**
- `ndp_id` - Primary key, matches fact table identity column
- `category` - Entity classification (temperature, humidity, door, window, etc.)
- `friendly_name` - Human-readable display name for dashboards
- `location_path` - Hierarchical path (e.g., `home/living_room`)
- `correlates_with` - Array of related ndp_ids for cross-correlation
- `orientation` - Compass direction for physical entities

### 1.4 Loading Process (Step-by-Step)

#### Method 1: Docker Init Scripts (Automatic)

1. TimescaleDB container starts
2. Docker entrypoint runs scripts in `/docker-entrypoint-initdb.d/` (numeric order)
3. `04-dimension-tables.sql` executes (copied from `deploy/pi/sql/dimensions/init.sql`)
4. Creates `silver.entity_context` table with indexes and constraints
5. Creates sync functions in `silver` schema:
   - `silver.truncate_and_load_dimension()`
   - `silver.start_dimension_sync()`
   - `silver.complete_dimension_sync()`
6. Creates `silver.dimension_sync_log` for audit trail

#### Method 2: Rust Application (Programmatic)

```rust
use platform_core::dimensions::{CsvDimensionLoader, DimensionLoader, DdlGenerator};
use platform_core::types::dimension_config::DimensionConfig;

// 1. Load YAML config (single source of truth)
let config: DimensionConfig = serde_yaml::from_str(yaml)?;

// 2. Generate DDL from config
let create_table = DdlGenerator::generate_create_table(&config);
let indexes = DdlGenerator::generate_indexes(&config);

// 3. Create loader
let loader = CsvDimensionLoader::new(config);

// 4. Validate (dry run)
let stats = loader.dry_run().await?;

// 5. Load to TimescaleDB (requires 'timescale' feature)
#[cfg(feature = "timescale")]
loader.load(&pool).await?;
```

### 1.5 YAML Configuration Schema

The YAML config (`config/base/dimensions/entity_context.yaml`) is the **single source of truth** for:
- Target table and schema
- Field definitions (name, type, nullable, validation)
- Primary key and indexes
- Load strategy (truncate_and_load or upsert)
- Array field transformations

```yaml
dimension_id: entity_context
target:
  table: entity_context
  schema: silver

source:
  type: csv
  path: data/dimensions/entity_context.csv
  delimiter: ","

schema:
  primary_key:
    - ndp_id
  fields:
    - name: ndp_id
      type: text
      nullable: false
    - name: category
      type: text
      nullable: false
    # ... more fields

load:
  strategy: truncate_and_load
  batch_size: 1000
```

### 1.6 Load Strategies

| Strategy | Behavior | Use Case |
|----------|----------|----------|
| `truncate_and_load` | DELETE all, then INSERT | Full refresh, small datasets |
| `upsert` | INSERT ON CONFLICT DO UPDATE | Incremental updates |

---

## 2. Data Dictionary System

### 2.1 File Locations

| Artifact Type | Path | Purpose |
|---------------|------|---------|
| **Bronze Schema** | `/workspaces/neural-data-platform/deploy/pi/init-scripts/01-create-data-dictionary.sql` | Bronze layer metadata tables |
| **Silver Schema** | `/workspaces/neural-data-platform/deploy/pi/init-scripts/003_silver_data_dictionary.sql` | Silver layer metadata tables |
| **Seed Data** | `/workspaces/neural-data-platform/deploy/timescaledb/init/004_seed_silver_data_dictionary.sql` | Populate Silver dictionary |
| **MCP Tools Spec** | `/workspaces/neural-data-platform/product/features/dp-010/specification/DICTIONARY-TOOLS-SPEC.md` | Tool specifications |
| **Rust MCP Impl** | `/workspaces/neural-data-platform/core/ndp-mcp-server/src/storage/timescale_dictionary.rs` | Dictionary store adapter |
| **Tool: query_dictionary** | `/workspaces/neural-data-platform/core/ndp-mcp-server/src/mcp/tools/query_dictionary.rs` | Search implementation |

### 2.2 Database Schema

#### Bronze Tables (`data_dictionary` schema)

```sql
data_dictionary.streams        -- Stream metadata (stream_id, enabled, retention)
data_dictionary.fields         -- Bronze Parquet columns (field_name, field_type, unit)
data_dictionary.sources        -- Source configuration (MQTT, HTTP, etc.)
data_dictionary.entity_schemas -- Logical entity definitions
data_dictionary.entity_schema_attributes -- Attribute definitions
data_dictionary.sync_status    -- Sync audit log
```

#### Silver Tables (`data_dictionary` schema)

```sql
data_dictionary.silver_tables   -- Table metadata (grain, source_streams[])
data_dictionary.silver_columns  -- Column definitions (data_type, unit, description)
data_dictionary.silver_lineage  -- Bronze->Silver field mappings
data_dictionary.silver_dq_rules -- DQ rules per column
```

#### Unified Views

```sql
data_dictionary.v_complete_dictionary  -- UNION of Bronze fields + Silver columns
data_dictionary.v_silver_table_overview -- Table summary with counts
data_dictionary.v_lineage              -- Joined lineage with column details
data_dictionary.v_dq_rules_summary     -- DQ rules with column context
data_dictionary.v_column_search        -- Searchable view across layers
```

### 2.3 How Streams Register Columns

Silver columns are registered via SQL seed scripts:

```sql
-- Example from 004_seed_silver_data_dictionary.sql
INSERT INTO data_dictionary.silver_columns (
    table_name, column_name, data_type, unit, description, nullable, is_primary_key, sort_order
) VALUES
    ('silver.air_quality_observations', 'pm25', 'DOUBLE PRECISION', 'ug/m3',
     'PM2.5 particulate matter concentration', false, false, 10)
ON CONFLICT (table_name, column_name) DO UPDATE SET
    data_type = EXCLUDED.data_type,
    unit = EXCLUDED.unit,
    description = EXCLUDED.description;
```

### 2.4 Lineage Tracking

Bronze-to-Silver mappings are stored in `silver_lineage`:

```sql
INSERT INTO data_dictionary.silver_lineage (
    silver_table, silver_column, source_stream, source_path, transformation
) VALUES
    ('silver.air_quality_observations', 'pm25', 'air-quality',
     'raw_payload.pm02Compensated', 'direct');
```

### 2.5 MCP Tools

Four MCP tools provide access to the data dictionary:

| Tool | Purpose | Primary Table(s) |
|------|---------|------------------|
| `query_dictionary` | Search for columns/fields by name | `v_complete_dictionary` |
| `describe_column` | Get full column details | `silver_columns`, `fields`, `silver_lineage` |
| `trace_lineage` | Trace Silver column to Bronze source | `silver_lineage` |
| `list_dq_rules` | List DQ rules for table/column | `silver_dq_rules` |

---

## 3. Relationship Between Systems

### 3.1 Dimensions vs Data Dictionary

| Aspect | Dimension Tables | Data Dictionary |
|--------|------------------|-----------------|
| **Purpose** | Enrich observations with context | Document columns for discoverability |
| **Location** | `silver` schema | `data_dictionary` schema |
| **Update Frequency** | Infrequent (manual) | With new streams/tables |
| **Source** | CSV files | SQL seed scripts |
| **Primary Consumer** | SQL JOINs in queries | MCP tools, documentation |
| **Example** | `silver.entity_context` | `data_dictionary.silver_columns` |

### 3.2 Join Pattern

Dimensions JOIN with fact tables on `ndp_id`:

```sql
SELECT
    o.observation_time,
    o.pm25,
    c.friendly_name,
    c.location_path
FROM silver.air_quality_observations o
LEFT JOIN silver.entity_context c USING (ndp_id);
```

### 3.3 No Foreign Key Enforcement

There is **no foreign key** between:
- `silver.entity_context.ndp_id` and fact table `ndp_id` columns
- Dimension tables and data dictionary tables

The relationship is **naming convention only** - both use `ndp_id` as the identity column.

### 3.4 What Happens If Dimension Entry Missing?

- LEFT JOIN returns NULL for dimension columns
- No error is raised
- DQ flags could detect missing context (not currently implemented)

---

## 4. Adding a New Stream's Entities to Dimensions

### Step 1: Add to CSV

Edit `/workspaces/neural-data-platform/data/dimensions/entity_context.csv`:

```csv
new_sensor_id,temperature,New Sensor Name,home/office,{},east
```

### Step 2: Reload Dimension

Option A - Via SQL:
```sql
-- Uses truncate_and_load strategy
SELECT silver.start_dimension_sync('entity_context', 'entity_context', 'truncate_and_load', 'entity_context.csv');
TRUNCATE silver.entity_context;
\copy silver.entity_context(ndp_id, category, friendly_name, location_path, correlates_with, orientation) FROM 'data/dimensions/entity_context.csv' CSV HEADER;
SELECT silver.complete_dimension_sync(<sync_id>, 'success', <row_count>, 0, 0);
```

Option B - Via Rust:
```rust
let loader = CsvDimensionLoader::new(config);
loader.load(&pool).await?;
```

### Step 3: Add to Data Dictionary (Optional)

If documenting in data dictionary, add to seed SQL or call MCP tools.

---

## 5. Pain Points in Current Approach

### 5.1 Manual Synchronization

- **Issue**: CSV and database can drift; no automatic sync
- **Impact**: Dashboards may show stale friendly names
- **Mitigation**: Document sync procedure; consider scheduled job

### 5.2 No Validation of ndp_id References

- **Issue**: Can add dimension entry for ndp_id that doesn't exist in fact tables
- **Impact**: Orphan dimension rows; no referential integrity
- **Mitigation**: Add CHECK constraint or validation query post-load

### 5.3 Dual Systems for Metadata

- **Issue**: Dimensions (Silver) vs Data Dictionary (separate schema) causes confusion
- **Impact**: Unclear which system to use for what purpose
- **Mitigation**: Document clearly (this document); consider consolidation

### 5.4 Array Field Handling

- **Issue**: `correlates_with` is TEXT[] in DB but CSV stores as `{val1,val2}` string
- **Impact**: Requires special parsing logic in loader
- **Mitigation**: YAML config defines `csv_to_array` transform

### 5.5 No Gold Layer View for Enriched Data Dictionary

- **Issue**: Data dictionary has no view of dimension metadata
- **Impact**: MCP tools cannot discover `entity_context` columns
- **Mitigation**: Add `silver.entity_context` to `data_dictionary.silver_tables`

### 5.6 Seed Scripts Require Manual Updates

- **Issue**: Adding new Silver table requires editing SQL seed script
- **Impact**: Easy to forget data dictionary updates
- **Mitigation**: Consider config-driven seed generation

---

## 6. Recommendations

### Short-Term

1. **Add entity_context to data dictionary seed script** - Register dimension table as a Silver table for MCP discoverability
2. **Document sync procedure** - Create runbook for dimension updates
3. **Add validation query** - Post-load check for orphan dimension entries

### Medium-Term

4. **Automate dimension sync** - Schedule or trigger on CSV change
5. **Unify metadata systems** - Consider using data dictionary to store dimension schema
6. **Add DQ rule for missing context** - Flag observations without entity_context match

### Long-Term

7. **Config-driven data dictionary** - Generate seed SQL from YAML configs
8. **Foreign key or constraint** - Enforce referential integrity for ndp_id

---

## 7. File Reference Summary

### Dimension System Files

```
config/base/dimensions/entity_context.yaml     # YAML config (single source of truth)
data/dimensions/entity_context.csv             # CSV source data
core/src/dimensions/mod.rs                     # Rust module
core/src/dimensions/loader.rs                  # CSV loader
core/src/dimensions/ddl.rs                     # DDL generator
core/src/dimensions/error.rs                   # Error types
deploy/pi/sql/dimensions/init.sql              # Master SQL init
deploy/pi/sql/dimensions/entity_context.sql    # Full DDL with triggers
deploy/pi/sql/dimensions/sync_functions.sql    # Sync helper functions
deploy/pi/init-scripts/04-dimension-tables.sql # Docker init script
```

### Data Dictionary Files

```
deploy/pi/init-scripts/01-create-data-dictionary.sql    # Bronze schema
deploy/pi/init-scripts/003_silver_data_dictionary.sql   # Silver schema
deploy/timescaledb/init/004_seed_silver_data_dictionary.sql  # Seed data
core/ndp-mcp-server/src/storage/timescale_dictionary.rs # Storage adapter
core/ndp-mcp-server/src/mcp/tools/query_dictionary.rs   # MCP tool
core/ndp-mcp-server/src/mcp/tools/describe_column.rs    # MCP tool
core/ndp-mcp-server/src/mcp/tools/trace_lineage.rs      # MCP tool
```

---

## References

- [dp-002 SCOPE](../../dp-002/SCOPE.md) - Bronze Data Dictionary
- [dp-009 SCOPE](../../dp-009/SCOPE.md) - Silver Data Dictionary
- [dp-010 SCOPE](../../dp-010/SCOPE.md) - MCP Extension
- [dp-013 SCOPE](../../dp-013/SCOPE.md) - CSV Source Type & Dimension Tables
- [DICTIONARY-TOOLS-SPEC](../../dp-010/specification/DICTIONARY-TOOLS-SPEC.md) - MCP Tools Specification

---

*Research completed: 2026-02-01*
*Author: ndp-analytics-engineer*
