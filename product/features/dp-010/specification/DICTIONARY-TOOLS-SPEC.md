# DP-010: Data Dictionary MCP Tools Specification

**Document**: DICTIONARY-TOOLS-SPEC.md
**Feature ID**: dp-010
**Author**: ndp-dq-engineer
**Created**: 2026-01-16
**Status**: Draft

---

## Overview

This specification defines four MCP tools that enable agents to discover, explore, and trace data lineage across the NDP data dictionary. These tools provide unified access to metadata spanning both Bronze and Silver layers, enabling agents to:

1. **Search** for columns/fields across the entire platform
2. **Inspect** detailed column metadata including DQ rules
3. **Trace** data lineage from Silver back to Bronze sources
4. **List** DQ rules applied during ETL transformations

---

## Tool Summary

| Tool | Purpose | Primary Table(s) |
|------|---------|------------------|
| `query_dictionary` | Search for columns/fields by name | `v_complete_dictionary` |
| `describe_column` | Get full column details | `silver_columns`, `fields`, `silver_lineage`, `silver_dq_rules` |
| `trace_lineage` | Trace Silver column to Bronze source | `silver_lineage` |
| `list_dq_rules` | List DQ rules for table/column | `silver_dq_rules`, config `dq_rules[]` |

---

## Database Schema Context

### Existing Bronze Tables (dp-002)

```sql
data_dictionary.streams        -- Stream metadata (stream_id, enabled, retention)
data_dictionary.fields         -- Bronze Parquet columns (field_name, field_type, unit)
data_dictionary.entity_schemas -- Logical entity definitions (device_class)
data_dictionary.entity_schema_attributes -- Attribute definitions with ranges
```

### New Silver Tables (dp-009)

```sql
data_dictionary.silver_tables   -- Silver table metadata (grain, source_streams[])
data_dictionary.silver_columns  -- Column definitions (data_type, unit, description)
data_dictionary.silver_lineage  -- Bronze->Silver field mappings (transformation)
data_dictionary.silver_dq_rules -- DQ rules per column (rule_params JSONB, action)
```

### Unified Views (dp-009)

```sql
data_dictionary.v_complete_dictionary  -- UNION of Bronze fields + Silver columns
data_dictionary.v_lineage              -- Joined lineage with column details
```

---

## Tool 1: query_dictionary

### Purpose

Search the data dictionary for columns/fields matching a query string. Enables agents to discover available data columns across Bronze and Silver layers.

### MCP Input Schema

```json
{
  "type": "object",
  "properties": {
    "query": {
      "type": "string",
      "description": "Search term to match against column names and descriptions (case-insensitive substring match)"
    },
    "layer": {
      "type": "string",
      "enum": ["bronze", "silver", "all"],
      "default": "all",
      "description": "Filter results by data layer"
    }
  },
  "required": ["query"],
  "additionalProperties": false
}
```

### Response Schema

```json
{
  "type": "object",
  "properties": {
    "success": {"type": "boolean"},
    "query": {"type": "string"},
    "layer": {"type": "string"},
    "result_count": {"type": "integer"},
    "results": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "layer": {"type": "string", "enum": ["bronze", "silver"]},
          "entity": {"type": "string", "description": "stream_id (Bronze) or table_name (Silver)"},
          "column_name": {"type": "string"},
          "data_type": {"type": "string"},
          "unit": {"type": "string", "nullable": true},
          "description": {"type": "string", "nullable": true}
        }
      }
    }
  }
}
```

### SQL Query Approach

```sql
-- Uses v_complete_dictionary unified view
SELECT
    layer,
    entity,
    column_name,
    data_type,
    unit,
    description
FROM data_dictionary.v_complete_dictionary
WHERE
    ($1 = 'all' OR layer = $1)
    AND (
        column_name ILIKE '%' || $2 || '%'
        OR description ILIKE '%' || $2 || '%'
    )
ORDER BY layer, entity, column_name
LIMIT 50;
```

**Query Parameters**:
- `$1`: layer filter ('bronze', 'silver', or 'all')
- `$2`: search query string

### Example Request/Response

**Request** (search for "temperature"):
```json
{
  "query": "temperature",
  "layer": "all"
}
```

**Response**:
```json
{
  "success": true,
  "query": "temperature",
  "layer": "all",
  "result_count": 4,
  "results": [
    {
      "layer": "silver",
      "entity": "air_quality_observations",
      "column_name": "temperature_c",
      "data_type": "DOUBLE PRECISION",
      "unit": "Celsius",
      "description": "Ambient temperature (sensor-compensated)"
    },
    {
      "layer": "silver",
      "entity": "weather_observations",
      "column_name": "temperature_c",
      "data_type": "DOUBLE PRECISION",
      "unit": "Celsius",
      "description": "Ambient air temperature"
    },
    {
      "layer": "silver",
      "entity": "weather_forecasts",
      "column_name": "temperature_c",
      "data_type": "DOUBLE PRECISION",
      "unit": "Celsius",
      "description": "Forecast temperature"
    },
    {
      "layer": "bronze",
      "entity": "air-quality",
      "column_name": "temperature",
      "data_type": "float",
      "unit": "celsius",
      "description": "Ambient temperature"
    }
  ]
}
```

### Edge Cases

| Scenario | Behavior |
|----------|----------|
| No matches | Return `{"success": true, "result_count": 0, "results": []}` |
| Empty query | Return error: "query cannot be empty" |
| Invalid layer | Return error: "layer must be one of: bronze, silver, all" |
| > 50 results | Truncate to 50, add `"truncated": true` to response |

---

## Tool 2: describe_column

### Purpose

Get comprehensive details for a specific column, combining information from multiple data dictionary tables including source lineage and DQ rules.

### MCP Input Schema

```json
{
  "type": "object",
  "properties": {
    "table_or_stream": {
      "type": "string",
      "description": "Silver table name (e.g., 'air_quality_observations') or Bronze stream_id (e.g., 'air-quality')"
    },
    "column_name": {
      "type": "string",
      "description": "Column or field name to describe"
    }
  },
  "required": ["table_or_stream", "column_name"],
  "additionalProperties": false
}
```

### Response Schema

```json
{
  "type": "object",
  "properties": {
    "success": {"type": "boolean"},
    "layer": {"type": "string", "enum": ["bronze", "silver"]},
    "table_or_stream": {"type": "string"},
    "column_name": {"type": "string"},
    "data_type": {"type": "string"},
    "unit": {"type": "string", "nullable": true},
    "description": {"type": "string", "nullable": true},
    "nullable": {"type": "boolean"},
    "source": {
      "type": "object",
      "nullable": true,
      "description": "Lineage info (Silver columns only)",
      "properties": {
        "stream": {"type": "string"},
        "path": {"type": "string"},
        "transformation": {"type": "string"}
      }
    },
    "dq_rules": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "rule_name": {"type": "string"},
          "params": {"type": "object"},
          "action": {"type": "string"}
        }
      }
    },
    "validation_range": {
      "type": "object",
      "nullable": true,
      "properties": {
        "min": {"type": "number"},
        "max": {"type": "number"}
      }
    }
  }
}
```

### SQL Query Approach

The tool first determines if the input is a Silver table or Bronze stream, then queries the appropriate tables.

**Step 1: Detect Layer**
```sql
-- Check if it's a Silver table
SELECT EXISTS(
    SELECT 1 FROM data_dictionary.silver_tables WHERE table_name = $1
) AS is_silver;
```

**Step 2a: Silver Column Query**
```sql
SELECT
    'silver' AS layer,
    sc.table_name,
    sc.column_name,
    sc.data_type,
    sc.unit,
    sc.description,
    sc.nullable,
    sl.source_stream,
    sl.source_path,
    sl.transformation,
    COALESCE(
        (SELECT json_agg(json_build_object(
            'rule_name', rule_name,
            'params', rule_params,
            'action', action
        ))
        FROM data_dictionary.silver_dq_rules dr
        WHERE dr.silver_table = sc.table_name
          AND dr.silver_column = sc.column_name),
        '[]'::json
    ) AS dq_rules
FROM data_dictionary.silver_columns sc
LEFT JOIN data_dictionary.silver_lineage sl
    ON sc.table_name = sl.silver_table
   AND sc.column_name = sl.silver_column
WHERE sc.table_name = $1
  AND sc.column_name = $2;
```

**Step 2b: Bronze Field Query**
```sql
SELECT
    'bronze' AS layer,
    f.stream_id,
    f.field_name,
    f.field_type AS data_type,
    f.unit,
    f.description,
    f.nullable,
    f.validation_min,
    f.validation_max
FROM data_dictionary.fields f
WHERE f.stream_id = $1
  AND f.field_name = $2;
```

### Example Request/Response

**Request** (Silver column):
```json
{
  "table_or_stream": "air_quality_observations",
  "column_name": "pm25"
}
```

**Response**:
```json
{
  "success": true,
  "layer": "silver",
  "table_or_stream": "air_quality_observations",
  "column_name": "pm25",
  "data_type": "DOUBLE PRECISION",
  "unit": "ug/m3",
  "description": "PM2.5 particulate matter concentration (humidity-compensated)",
  "nullable": false,
  "source": {
    "stream": "air-quality",
    "path": "raw_payload.pm02Compensated",
    "transformation": "direct"
  },
  "dq_rules": [
    {
      "rule_name": "range_check",
      "params": {"min": 0.0, "max": 1000.0},
      "action": "flag"
    }
  ],
  "validation_range": {
    "min": 0.0,
    "max": 1000.0
  }
}
```

**Request** (Bronze field):
```json
{
  "table_or_stream": "air-quality",
  "column_name": "temperature"
}
```

**Response**:
```json
{
  "success": true,
  "layer": "bronze",
  "table_or_stream": "air-quality",
  "column_name": "temperature",
  "data_type": "float",
  "unit": "celsius",
  "description": "Ambient temperature",
  "nullable": true,
  "source": null,
  "dq_rules": [],
  "validation_range": {
    "min": -40.0,
    "max": 85.0
  }
}
```

### Edge Cases

| Scenario | Behavior |
|----------|----------|
| Table/stream not found | Return error with code "NOT_FOUND" |
| Column not found | Return error: "column '{name}' not found in {table}" |
| No DQ rules | Return `"dq_rules": []` |
| No lineage (Bronze) | Return `"source": null` |

---

## Tool 3: trace_lineage

### Purpose

Trace a Silver column back to its Bronze source(s), showing the complete data lineage path including transformations and DQ rules applied during ETL.

### MCP Input Schema

```json
{
  "type": "object",
  "properties": {
    "silver_table": {
      "type": "string",
      "description": "Silver table name (e.g., 'weather_observations')"
    },
    "silver_column": {
      "type": "string",
      "description": "Silver column name to trace (e.g., 'temperature_c')"
    }
  },
  "required": ["silver_table", "silver_column"],
  "additionalProperties": false
}
```

### Response Schema

```json
{
  "type": "object",
  "properties": {
    "success": {"type": "boolean"},
    "silver_table": {"type": "string"},
    "silver_column": {"type": "string"},
    "silver_type": {"type": "string"},
    "silver_unit": {"type": "string", "nullable": true},
    "lineage": {
      "type": "array",
      "description": "Array for multi-source columns (e.g., merged weather streams)",
      "items": {
        "type": "object",
        "properties": {
          "source_stream": {"type": "string"},
          "source_path": {"type": "string"},
          "transformation": {"type": "string", "nullable": true},
          "bronze_type": {"type": "string", "nullable": true},
          "bronze_unit": {"type": "string", "nullable": true}
        }
      }
    },
    "dq_rules": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "rule_name": {"type": "string"},
          "params": {"type": "object"},
          "action": {"type": "string"},
          "scope": {"type": "string", "enum": ["column", "cross-field"]}
        }
      }
    }
  }
}
```

### SQL Query Approach

```sql
-- Query silver_lineage joined with silver_columns and dq_rules
SELECT
    l.source_stream,
    l.source_path,
    l.transformation,
    sc.data_type AS silver_type,
    sc.unit AS silver_unit,
    -- Get DQ rules (both column-level and table-level)
    (
        SELECT json_agg(json_build_object(
            'rule_name', rule_name,
            'params', rule_params,
            'action', action,
            'scope', CASE WHEN silver_column IS NULL THEN 'cross-field' ELSE 'column' END
        ))
        FROM data_dictionary.silver_dq_rules dr
        WHERE dr.silver_table = $1
          AND (dr.silver_column = $2 OR dr.silver_column IS NULL)
    ) AS dq_rules
FROM data_dictionary.silver_lineage l
JOIN data_dictionary.silver_columns sc
    ON l.silver_table = sc.table_name
   AND l.silver_column = sc.column_name
WHERE l.silver_table = $1
  AND l.silver_column = $2
ORDER BY l.source_stream;
```

**Query Parameters**:
- `$1`: silver_table
- `$2`: silver_column

### Example Request/Response

**Request** (single-source column):
```json
{
  "silver_table": "air_quality_observations",
  "silver_column": "pm25"
}
```

**Response**:
```json
{
  "success": true,
  "silver_table": "air_quality_observations",
  "silver_column": "pm25",
  "silver_type": "DOUBLE PRECISION",
  "silver_unit": "ug/m3",
  "lineage": [
    {
      "source_stream": "air-quality",
      "source_path": "raw_payload.pm02Compensated",
      "transformation": "direct",
      "bronze_type": "float",
      "bronze_unit": "ug/m3"
    }
  ],
  "dq_rules": [
    {
      "rule_name": "range_check",
      "params": {"min": 0.0, "max": 1000.0},
      "action": "flag",
      "scope": "column"
    },
    {
      "rule_name": "cross_field_check",
      "params": {
        "expression": "pm10 IS NULL OR pm25 IS NULL OR pm10 >= pm25",
        "message": "pm10_less_than_pm25"
      },
      "action": "flag",
      "scope": "cross-field"
    }
  ]
}
```

**Request** (multi-source column - weather_observations):
```json
{
  "silver_table": "weather_observations",
  "silver_column": "temperature_c"
}
```

**Response**:
```json
{
  "success": true,
  "silver_table": "weather_observations",
  "silver_column": "temperature_c",
  "silver_type": "DOUBLE PRECISION",
  "silver_unit": "Celsius",
  "lineage": [
    {
      "source_stream": "outdoor-weather",
      "source_path": "raw_payload.main.temp",
      "transformation": "direct",
      "bronze_type": "float",
      "bronze_unit": "celsius"
    },
    {
      "source_stream": "nws-observations",
      "source_path": "raw_payload.temperature.value",
      "transformation": "direct",
      "bronze_type": "float",
      "bronze_unit": "celsius"
    }
  ],
  "dq_rules": [
    {
      "rule_name": "range_check",
      "params": {"min": -60.0, "max": 60.0},
      "action": "flag",
      "scope": "column"
    },
    {
      "rule_name": "rate_of_change",
      "params": {
        "field": "temperature_c",
        "max_change_per_minute": 2.0,
        "partition_by": ["ndp_id"]
      },
      "action": "flag",
      "scope": "cross-field"
    }
  ]
}
```

### Edge Cases

| Scenario | Behavior |
|----------|----------|
| Silver table not found | Return error: "Silver table '{name}' not found" |
| Column not found | Return error: "Column '{name}' not found in table '{table}'" |
| No lineage recorded | Return `"lineage": []` with warning |
| Multi-source column | Return array with all source mappings |

---

## Tool 4: list_dq_rules

### Purpose

List DQ rules applied to Silver tables/columns. Can filter by table and/or column, or list all rules.

### MCP Input Schema

```json
{
  "type": "object",
  "properties": {
    "table": {
      "type": "string",
      "description": "Filter by Silver table name (optional)",
      "nullable": true
    },
    "column": {
      "type": "string",
      "description": "Filter by column name (optional, requires table)",
      "nullable": true
    }
  },
  "required": [],
  "additionalProperties": false
}
```

### Response Schema

```json
{
  "type": "object",
  "properties": {
    "success": {"type": "boolean"},
    "filters": {
      "type": "object",
      "properties": {
        "table": {"type": "string", "nullable": true},
        "column": {"type": "string", "nullable": true}
      }
    },
    "rule_count": {"type": "integer"},
    "rules": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "silver_table": {"type": "string"},
          "silver_column": {"type": "string", "nullable": true, "description": "NULL for cross-field rules"},
          "rule_name": {"type": "string"},
          "rule_params": {"type": "object"},
          "action": {"type": "string", "enum": ["flag", "reject", "clamp", "warn"]},
          "scope": {"type": "string", "enum": ["column", "cross-field"]}
        }
      }
    }
  }
}
```

### SQL Query Approach

```sql
SELECT
    silver_table,
    silver_column,
    rule_name,
    rule_params,
    action,
    CASE WHEN silver_column IS NULL THEN 'cross-field' ELSE 'column' END AS scope
FROM data_dictionary.silver_dq_rules
WHERE
    ($1 IS NULL OR silver_table = $1)
    AND ($2 IS NULL OR silver_column = $2 OR (silver_column IS NULL AND $2 IS NULL))
ORDER BY silver_table,
         CASE WHEN silver_column IS NULL THEN 1 ELSE 0 END,
         silver_column,
         rule_name
LIMIT 100;
```

**Query Parameters**:
- `$1`: table filter (NULL for all)
- `$2`: column filter (NULL for all)

### Example Request/Response

**Request** (all rules for a table):
```json
{
  "table": "air_quality_observations"
}
```

**Response**:
```json
{
  "success": true,
  "filters": {
    "table": "air_quality_observations",
    "column": null
  },
  "rule_count": 9,
  "rules": [
    {
      "silver_table": "air_quality_observations",
      "silver_column": "pm25",
      "rule_name": "range_check",
      "rule_params": {"min": 0.0, "max": 1000.0},
      "action": "flag",
      "scope": "column"
    },
    {
      "silver_table": "air_quality_observations",
      "silver_column": "pm10",
      "rule_name": "range_check",
      "rule_params": {"min": 0.0, "max": 2000.0},
      "action": "flag",
      "scope": "column"
    },
    {
      "silver_table": "air_quality_observations",
      "silver_column": "co2",
      "rule_name": "range_check",
      "rule_params": {"min": 380, "max": 10000},
      "action": "flag",
      "scope": "column"
    },
    {
      "silver_table": "air_quality_observations",
      "silver_column": "humidity_pct",
      "rule_name": "range_check",
      "rule_params": {"min": 0.0, "max": 100.0, "clamp_to_bounds": true},
      "action": "clamp",
      "scope": "column"
    },
    {
      "silver_table": "air_quality_observations",
      "silver_column": null,
      "rule_name": "cross_field_check",
      "rule_params": {
        "name": "pm10_gte_pm25",
        "expression": "pm10 IS NULL OR pm25 IS NULL OR pm10 >= pm25",
        "message": "pm10_less_than_pm25"
      },
      "action": "flag",
      "scope": "cross-field"
    },
    {
      "silver_table": "air_quality_observations",
      "silver_column": null,
      "rule_name": "freshness_check",
      "rule_params": {
        "field": "observation_time",
        "max_age": "2 hours",
        "max_future": "5 minutes",
        "reference": "ingestion_time"
      },
      "action": "flag",
      "scope": "cross-field"
    }
  ]
}
```

**Request** (specific column):
```json
{
  "table": "weather_observations",
  "column": "temperature_c"
}
```

**Response**:
```json
{
  "success": true,
  "filters": {
    "table": "weather_observations",
    "column": "temperature_c"
  },
  "rule_count": 1,
  "rules": [
    {
      "silver_table": "weather_observations",
      "silver_column": "temperature_c",
      "rule_name": "range_check",
      "rule_params": {"min": -60.0, "max": 60.0},
      "action": "flag",
      "scope": "column"
    }
  ]
}
```

**Request** (all rules across all tables):
```json
{}
```

**Response** (truncated):
```json
{
  "success": true,
  "filters": {
    "table": null,
    "column": null
  },
  "rule_count": 28,
  "rules": [
    {"silver_table": "air_quality_observations", "silver_column": "pm25", "rule_name": "range_check", ...},
    {"silver_table": "air_quality_observations", "silver_column": "pm10", "rule_name": "range_check", ...},
    ...
  ]
}
```

### Edge Cases

| Scenario | Behavior |
|----------|----------|
| No filters | Return all DQ rules (max 100) |
| Table not found | Return `{"success": true, "rule_count": 0, "rules": []}` |
| Column without table | Return error: "column filter requires table filter" |
| Cross-field rules | Include with `silver_column: null` |

---

## Implementation Notes

### Layer Detection Logic

Both `describe_column` and Bronze/Silver disambiguation use this pattern:

```rust
enum DataLayer {
    Bronze,
    Silver,
}

async fn detect_layer(store: &impl DictionaryStore, name: &str) -> McpResult<DataLayer> {
    // Silver tables use snake_case (e.g., "air_quality_observations")
    // Bronze streams use kebab-case (e.g., "air-quality")

    // Check Silver first (more common for analytics queries)
    if store.silver_table_exists(name).await? {
        return Ok(DataLayer::Silver);
    }

    // Check Bronze
    if store.bronze_stream_exists(name).await? {
        return Ok(DataLayer::Bronze);
    }

    Err(McpError::NotFound(format!(
        "'{name}' not found as Silver table or Bronze stream"
    )))
}
```

### Search Optimization

For `query_dictionary`, use PostgreSQL full-text search if performance becomes an issue:

```sql
-- Optional: Add tsvector column to v_complete_dictionary
ALTER TABLE data_dictionary.silver_columns
    ADD COLUMN search_vector tsvector
    GENERATED ALWAYS AS (
        to_tsvector('english', coalesce(column_name, '') || ' ' || coalesce(description, ''))
    ) STORED;

CREATE INDEX idx_silver_columns_search ON data_dictionary.silver_columns USING GIN(search_vector);
```

### DQ Rule Aggregation

Cross-field vs column-level rules are distinguished by `silver_column`:
- `silver_column IS NOT NULL` -> Column-level rule (applies to specific column)
- `silver_column IS NULL` -> Cross-field rule (applies to whole row/batch)

### Connection Pooling

All dictionary queries share the TimescaleDB connection pool:

```rust
// Reuse existing pool from SilverStorage
pub struct DictionaryStore {
    pool: Pool<Postgres>,
    dictionary_schema: String,  // default: "data_dictionary"
}
```

---

## Testing Strategy

### Unit Tests

| Test Case | Tool | Validates |
|-----------|------|-----------|
| `test_query_dictionary_matches` | `query_dictionary` | Returns matching results |
| `test_query_dictionary_empty` | `query_dictionary` | Handles no matches |
| `test_query_dictionary_layer_filter` | `query_dictionary` | Layer filtering works |
| `test_describe_silver_column` | `describe_column` | Silver column details |
| `test_describe_bronze_field` | `describe_column` | Bronze field details |
| `test_describe_column_not_found` | `describe_column` | Error handling |
| `test_trace_lineage_single` | `trace_lineage` | Single-source column |
| `test_trace_lineage_multi` | `trace_lineage` | Multi-source column |
| `test_list_dq_rules_all` | `list_dq_rules` | All rules returned |
| `test_list_dq_rules_filtered` | `list_dq_rules` | Table/column filter |
| `test_list_dq_rules_cross_field` | `list_dq_rules` | Cross-field rules included |

### Integration Tests

```rust
#[tokio::test]
async fn test_dictionary_tools_integration() {
    let pool = setup_test_db().await;
    let store = DictionaryStore::new(pool);

    // 1. Query dictionary
    let results = store.search("temperature", Some("silver")).await?;
    assert!(!results.is_empty());

    // 2. Describe a found column
    let first = &results[0];
    let details = store.column_details(&first.entity, &first.column_name).await?;
    assert!(details.dq_rules.len() > 0);

    // 3. Trace lineage
    let lineage = store.lineage(&first.entity, &first.column_name).await?;
    assert!(!lineage.sources.is_empty());

    // 4. List DQ rules
    let rules = store.dq_rules(Some(&first.entity), None).await?;
    assert!(rules.iter().any(|r| r.rule_name == "range_check"));
}
```

---

## MCP Tool Registration

These tools are registered in `handler.rs`:

```rust
fn get_dictionary_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "query_dictionary".to_string(),
            description: "Search data dictionary for columns matching a query across Bronze and Silver layers".to_string(),
            input_schema: ToolInputSchema::with_properties(
                json!({
                    "query": {
                        "type": "string",
                        "description": "Search term to match against column names and descriptions"
                    },
                    "layer": {
                        "type": "string",
                        "enum": ["bronze", "silver", "all"],
                        "default": "all",
                        "description": "Filter results by data layer"
                    }
                }),
                vec!["query".to_string()],
            ),
        },
        ToolDefinition {
            name: "describe_column".to_string(),
            description: "Get comprehensive details for a column including type, unit, source lineage, and DQ rules".to_string(),
            input_schema: ToolInputSchema::with_properties(
                json!({
                    "table_or_stream": {
                        "type": "string",
                        "description": "Silver table name or Bronze stream_id"
                    },
                    "column_name": {
                        "type": "string",
                        "description": "Column or field name to describe"
                    }
                }),
                vec!["table_or_stream".to_string(), "column_name".to_string()],
            ),
        },
        ToolDefinition {
            name: "trace_lineage".to_string(),
            description: "Trace a Silver column back to its Bronze source(s) with transformation and DQ rules".to_string(),
            input_schema: ToolInputSchema::with_properties(
                json!({
                    "silver_table": {
                        "type": "string",
                        "description": "Silver table name"
                    },
                    "silver_column": {
                        "type": "string",
                        "description": "Silver column name to trace"
                    }
                }),
                vec!["silver_table".to_string(), "silver_column".to_string()],
            ),
        },
        ToolDefinition {
            name: "list_dq_rules".to_string(),
            description: "List DQ rules applied during Silver ETL, optionally filtered by table and column".to_string(),
            input_schema: ToolInputSchema::with_properties(
                json!({
                    "table": {
                        "type": "string",
                        "description": "Filter by Silver table name (optional)"
                    },
                    "column": {
                        "type": "string",
                        "description": "Filter by column name (optional, requires table)"
                    }
                }),
                vec![],  // Both optional
            ),
        },
    ]
}
```

---

## References

- [dp-002 SCOPE](../../dp-002/SCOPE.md) - Bronze Data Dictionary
- [dp-005 SCOPE](../../dp-005/SCOPE.md) - Bronze MCP Server
- [dp-009 SCOPE](../../dp-009/SCOPE.md) - Silver Data Dictionary
- [dp-010 SCOPE](../SCOPE.md) - Full MCP Extension Scope
- [01-create-data-dictionary.sql](/workspaces/neural-data-platform/deploy/pi/init-scripts/01-create-data-dictionary.sql) - Bronze schema
- [04-LAYERED-DQ-STRATEGY.md](/workspaces/neural-data-platform/product/research/analyticplatforminfrastructure/04-LAYERED-DQ-STRATEGY.md) - DQ framework

---

*Specification created: 2026-01-16*
*Author: ndp-dq-engineer*
