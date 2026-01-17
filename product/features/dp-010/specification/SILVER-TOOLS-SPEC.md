# Silver Layer MCP Tools Specification

**Feature**: dp-010 (NDP MCP Server - Silver Layer Extension)
**Version**: 1.0
**Status**: Specification
**Created**: 2026-01-16
**Author**: ndp-timescale-dev

---

## 1. Overview

This specification defines four MCP tools for Silver layer (TimescaleDB) access, enabling agents to discover, inspect, and sample queryable time-series data.

### 1.1 Tool Summary

| Tool | Purpose | Input | Output |
|------|---------|-------|--------|
| `list_silver_tables` | Enumerate Silver hypertables | none | Table metadata with row counts |
| `describe_silver_table` | Get table schema | `table_name` | Column definitions with units |
| `sample_silver_data` | Retrieve sample rows | `table_name`, `n`, `filters?` | JSON rows |
| `silver_stats` | Get table statistics | `table_name` | Counts, ranges, DQ summary |

### 1.2 Dependencies

| Component | Purpose |
|-----------|---------|
| TimescaleDB | Silver layer storage |
| `data_dictionary.silver_tables` | Table metadata (dp-009) |
| `data_dictionary.silver_columns` | Column metadata (dp-009) |
| `data_dictionary.silver_dq_rules` | DQ rule definitions (dp-009) |

---

## 2. Tool: list_silver_tables

### 2.1 Purpose

Enumerate all Silver layer hypertables with metadata, row counts, time ranges, and TimescaleDB-specific chunk information. Analogous to Bronze `list_streams`.

### 2.2 MCP Tool Definition

```json
{
  "name": "list_silver_tables",
  "description": "List all Silver layer hypertables with metadata including row counts, time ranges, and chunk information. Returns table configuration from data dictionary and live statistics from TimescaleDB.",
  "inputSchema": {
    "type": "object",
    "properties": {},
    "required": []
  }
}
```

### 2.3 Response Schema

```json
{
  "type": "object",
  "properties": {
    "success": { "type": "boolean" },
    "tables": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "table_name": { "type": "string", "description": "Short table name (e.g., 'air_quality_observations')" },
          "schema": { "type": "string", "description": "PostgreSQL schema (always 'silver')" },
          "full_name": { "type": "string", "description": "Fully qualified name ('silver.air_quality_observations')" },
          "description": { "type": "string", "description": "Human-readable table purpose" },
          "grain": { "type": "string", "description": "What one row represents" },
          "source_streams": { "type": "array", "items": { "type": "string" }, "description": "Bronze streams feeding this table" },
          "is_hypertable": { "type": "boolean", "description": "Whether converted to TimescaleDB hypertable" },
          "hypertable_column": { "type": "string", "description": "Time dimension column" },
          "chunk_interval": { "type": "string", "description": "TimescaleDB chunk interval (e.g., '1 day')" },
          "row_count": { "type": "integer", "description": "Approximate row count" },
          "time_range": {
            "type": "object",
            "properties": {
              "min": { "type": "string", "format": "date-time" },
              "max": { "type": "string", "format": "date-time" }
            }
          },
          "chunk_count": { "type": "integer", "description": "Number of TimescaleDB chunks" },
          "total_bytes": { "type": "integer", "description": "Total table size in bytes" }
        },
        "required": ["table_name", "schema", "full_name", "is_hypertable"]
      }
    }
  },
  "required": ["success", "tables"]
}
```

### 2.4 Example Request/Response

**Request**:
```json
{
  "method": "tools/call",
  "params": {
    "name": "list_silver_tables",
    "arguments": {}
  }
}
```

**Response**:
```json
{
  "success": true,
  "tables": [
    {
      "table_name": "air_quality_observations",
      "schema": "silver",
      "full_name": "silver.air_quality_observations",
      "description": "Indoor air quality measurements from AirGradient sensors",
      "grain": "One row per sensor reading (~1 minute intervals)",
      "source_streams": ["air-quality"],
      "is_hypertable": true,
      "hypertable_column": "observation_time",
      "chunk_interval": "1 day",
      "row_count": 142857,
      "time_range": {
        "min": "2026-01-01T00:00:00Z",
        "max": "2026-01-16T14:30:00Z"
      },
      "chunk_count": 16,
      "total_bytes": 15728640
    },
    {
      "table_name": "weather_observations",
      "schema": "silver",
      "full_name": "silver.weather_observations",
      "description": "Outdoor weather observations from NWS and OpenWeatherMap",
      "grain": "One row per observation per provider",
      "source_streams": ["outdoor-weather", "nws-observations"],
      "is_hypertable": true,
      "hypertable_column": "observation_time",
      "chunk_interval": "1 day",
      "row_count": 28571,
      "time_range": {
        "min": "2026-01-01T00:00:00Z",
        "max": "2026-01-16T14:25:00Z"
      },
      "chunk_count": 16,
      "total_bytes": 3145728
    },
    {
      "table_name": "weather_forecasts",
      "schema": "silver",
      "full_name": "silver.weather_forecasts",
      "description": "Weather forecasts from NWS gridpoint API",
      "grain": "One row per forecast hour per issue time",
      "source_streams": ["nws-gridpoints-forecast"],
      "is_hypertable": true,
      "hypertable_column": "valid_time",
      "chunk_interval": "1 day",
      "row_count": 571428,
      "time_range": {
        "min": "2026-01-01T00:00:00Z",
        "max": "2026-01-23T00:00:00Z"
      },
      "chunk_count": 23,
      "total_bytes": 62914560
    },
    {
      "table_name": "outdoor_air_quality",
      "schema": "silver",
      "full_name": "silver.outdoor_air_quality",
      "description": "Outdoor air quality from OpenWeatherMap Air Pollution API",
      "grain": "One row per observation (~10 minute intervals)",
      "source_streams": ["outdoor-air-quality"],
      "is_hypertable": true,
      "hypertable_column": "observation_time",
      "chunk_interval": "1 day",
      "row_count": 21428,
      "time_range": {
        "min": "2026-01-01T00:00:00Z",
        "max": "2026-01-16T14:20:00Z"
      },
      "chunk_count": 16,
      "total_bytes": 2097152
    }
  ]
}
```

### 2.5 SQL Query Approach

```sql
-- Query 1: Table metadata from data dictionary (dp-009)
SELECT
    t.table_name,
    'silver' AS schema,
    t.table_name AS full_name,
    t.description,
    t.grain,
    t.source_streams,
    t.hypertable_column,
    t.chunk_interval
FROM data_dictionary.silver_tables t
ORDER BY t.table_name;

-- Query 2: Live statistics from TimescaleDB
-- Note: Uses correct TimescaleDB 2.x views and functions
SELECT
    ht.hypertable_name AS table_name,
    TRUE AS is_hypertable,
    (SELECT reltuples::BIGINT FROM pg_class WHERE oid = format('silver.%I', ht.hypertable_name)::regclass) AS row_count,
    (SELECT COUNT(*) FROM timescaledb_information.chunks ch
     WHERE ch.hypertable_schema = 'silver' AND ch.hypertable_name = ht.hypertable_name) AS chunk_count,
    hypertable_size(format('silver.%I', ht.hypertable_name)::regclass) AS total_bytes
FROM timescaledb_information.hypertables ht
WHERE ht.hypertable_schema = 'silver';

-- Alternatively, for accurate row counts (slower):
SELECT
    'air_quality_observations' AS table_name,
    COUNT(*) AS row_count,
    MIN(observation_time) AS time_min,
    MAX(observation_time) AS time_max
FROM silver.air_quality_observations;
```

### 2.6 Error Cases

| Code | Condition | Response |
|------|-----------|----------|
| `TIMESCALE_UNAVAILABLE` | Cannot connect to TimescaleDB | `{"success": false, "code": "TIMESCALE_UNAVAILABLE", "error": "TimescaleDB unavailable: connection refused"}` |
| `DICTIONARY_UNAVAILABLE` | data_dictionary schema not populated | `{"success": false, "code": "DICTIONARY_UNAVAILABLE", "error": "Data dictionary not available"}` |

### 2.7 Implementation Notes

- **Row count source**: Use `pg_class.reltuples` for fast approximate counts; fall back to `COUNT(*)` only if explicitly requested
- **Time range**: Query MIN/MAX from actual data (may be slow on large tables - consider caching)
- **Chunk info**: Use TimescaleDB information views for hypertable metadata
- **Graceful degradation**: If data_dictionary not populated, derive basic metadata from information_schema

---

## 3. Tool: describe_silver_table

### 3.1 Purpose

Get detailed schema information for a Silver table including column definitions with data types, units, descriptions, and nullability. Analogous to Bronze `describe_schema`.

### 3.2 MCP Tool Definition

```json
{
  "name": "describe_silver_table",
  "description": "Get detailed schema information for a Silver table including column definitions with data types, units, descriptions, and primary key information.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "table_name": {
        "type": "string",
        "description": "The table name (e.g., 'air_quality_observations' or 'silver.air_quality_observations')"
      }
    },
    "required": ["table_name"]
  }
}
```

### 3.3 Response Schema

```json
{
  "type": "object",
  "properties": {
    "success": { "type": "boolean" },
    "table_name": { "type": "string" },
    "full_name": { "type": "string" },
    "description": { "type": "string" },
    "grain": { "type": "string" },
    "source_streams": { "type": "array", "items": { "type": "string" } },
    "primary_key": { "type": "array", "items": { "type": "string" } },
    "hypertable_info": {
      "type": "object",
      "properties": {
        "time_column": { "type": "string" },
        "chunk_interval": { "type": "string" }
      }
    },
    "columns": {
      "type": "array",
      "items": {
        "type": "object",
        "properties": {
          "name": { "type": "string", "description": "Column name" },
          "type": { "type": "string", "description": "PostgreSQL data type" },
          "unit": { "type": "string", "description": "Measurement unit (e.g., 'Celsius', 'ug/m3')" },
          "description": { "type": "string", "description": "Column purpose" },
          "nullable": { "type": "boolean", "description": "Whether NULL is allowed" },
          "is_primary_key": { "type": "boolean", "description": "Part of primary key constraint" },
          "sort_order": { "type": "integer", "description": "Display ordering" }
        },
        "required": ["name", "type", "nullable"]
      }
    }
  },
  "required": ["success", "table_name", "columns"]
}
```

### 3.4 Example Request/Response

**Request**:
```json
{
  "method": "tools/call",
  "params": {
    "name": "describe_silver_table",
    "arguments": {
      "table_name": "air_quality_observations"
    }
  }
}
```

**Response**:
```json
{
  "success": true,
  "table_name": "air_quality_observations",
  "full_name": "silver.air_quality_observations",
  "description": "Indoor air quality measurements from AirGradient sensors",
  "grain": "One row per sensor reading (~1 minute intervals)",
  "source_streams": ["air-quality"],
  "primary_key": ["observation_time", "ndp_id"],
  "hypertable_info": {
    "time_column": "observation_time",
    "chunk_interval": "1 day"
  },
  "columns": [
    {
      "name": "observation_time",
      "type": "TIMESTAMPTZ",
      "unit": null,
      "description": "Timestamp of sensor reading (UTC)",
      "nullable": false,
      "is_primary_key": true,
      "sort_order": 1
    },
    {
      "name": "ingestion_time",
      "type": "TIMESTAMPTZ",
      "unit": null,
      "description": "Timestamp when record was ingested to Silver (UTC)",
      "nullable": false,
      "is_primary_key": false,
      "sort_order": 2
    },
    {
      "name": "ndp_id",
      "type": "TEXT",
      "unit": null,
      "description": "Unique identifier for data source",
      "nullable": false,
      "is_primary_key": true,
      "sort_order": 3
    },
    {
      "name": "pm25",
      "type": "DOUBLE PRECISION",
      "unit": "ug/m3",
      "description": "PM2.5 particulate matter concentration (compensated)",
      "nullable": true,
      "is_primary_key": false,
      "sort_order": 4
    },
    {
      "name": "pm10",
      "type": "DOUBLE PRECISION",
      "unit": "ug/m3",
      "description": "PM10 particulate matter concentration",
      "nullable": true,
      "is_primary_key": false,
      "sort_order": 5
    },
    {
      "name": "co2",
      "type": "SMALLINT",
      "unit": "ppm",
      "description": "Carbon dioxide concentration",
      "nullable": true,
      "is_primary_key": false,
      "sort_order": 6
    },
    {
      "name": "temperature_c",
      "type": "DOUBLE PRECISION",
      "unit": "Celsius",
      "description": "Ambient temperature (compensated)",
      "nullable": true,
      "is_primary_key": false,
      "sort_order": 7
    },
    {
      "name": "humidity_pct",
      "type": "DOUBLE PRECISION",
      "unit": "%",
      "description": "Relative humidity (compensated)",
      "nullable": true,
      "is_primary_key": false,
      "sort_order": 8
    },
    {
      "name": "voc_index",
      "type": "SMALLINT",
      "unit": "index",
      "description": "Total Volatile Organic Compounds index (1-500 scale)",
      "nullable": true,
      "is_primary_key": false,
      "sort_order": 9
    },
    {
      "name": "nox_index",
      "type": "SMALLINT",
      "unit": "index",
      "description": "Nitrogen Oxides index (1-500 scale)",
      "nullable": true,
      "is_primary_key": false,
      "sort_order": 10
    },
    {
      "name": "dq_flags",
      "type": "TEXT[]",
      "unit": null,
      "description": "Data quality flags array",
      "nullable": true,
      "is_primary_key": false,
      "sort_order": 11
    }
  ]
}
```

### 3.5 SQL Query Approach

```sql
-- Query 1: Table metadata from data dictionary
SELECT
    t.table_name,
    t.table_name AS full_name,
    t.description,
    t.grain,
    t.source_streams,
    t.hypertable_column
FROM data_dictionary.silver_tables t
WHERE t.table_name = $1
   OR t.table_name = 'silver.' || $1;

-- Query 2: Column definitions from data dictionary
SELECT
    c.column_name AS name,
    c.data_type AS type,
    c.unit,
    c.description,
    c.nullable,
    c.is_primary_key,
    c.sort_order
FROM data_dictionary.silver_columns c
WHERE c.table_name = $1
   OR c.table_name = 'silver.' || $1
ORDER BY c.sort_order;

-- Fallback Query: information_schema (if dictionary not populated)
SELECT
    column_name AS name,
    UPPER(data_type) AS type,
    NULL AS unit,
    NULL AS description,
    (is_nullable = 'YES') AS nullable,
    FALSE AS is_primary_key,
    ordinal_position AS sort_order
FROM information_schema.columns
WHERE table_schema = 'silver'
  AND table_name = $1
ORDER BY ordinal_position;

-- Query 3: Primary key columns
SELECT a.attname AS column_name
FROM pg_index i
JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = ANY(i.indkey)
WHERE i.indrelid = 'silver.air_quality_observations'::regclass
  AND i.indisprimary;

-- Query 4: Hypertable dimension info
-- Note: Join on both schema AND name to avoid ambiguity
SELECT
    d.column_name AS time_column,
    h.chunk_time_interval::TEXT AS chunk_interval
FROM timescaledb_information.hypertables h
JOIN timescaledb_information.dimensions d
  ON h.hypertable_schema = d.hypertable_schema
  AND h.hypertable_name = d.hypertable_name
WHERE h.hypertable_schema = 'silver'
  AND h.hypertable_name = $1
  AND d.dimension_number = 1;  -- Primary (time) dimension
```

### 3.6 Error Cases

| Code | Condition | Response |
|------|-----------|----------|
| `TABLE_NOT_FOUND` | Table does not exist | `{"success": false, "code": "TABLE_NOT_FOUND", "error": "Table not found: nonexistent"}` |
| `TIMESCALE_UNAVAILABLE` | Cannot connect to TimescaleDB | `{"success": false, "code": "TIMESCALE_UNAVAILABLE", "error": "TimescaleDB unavailable"}` |

### 3.7 Implementation Notes

- **Table name normalization**: Accept both `air_quality_observations` and `silver.air_quality_observations`
- **Fallback strategy**: Use information_schema if data_dictionary not populated (units/descriptions will be NULL)
- **Primary key detection**: Query pg_index for constraint info
- **Column ordering**: Use sort_order from dictionary, or ordinal_position from information_schema

---

## 4. Tool: sample_silver_data

### 4.1 Purpose

Retrieve sample rows from a Silver table for exploration and debugging. Supports optional time range and ndp_id filters. Analogous to Bronze `sample_data`.

### 4.2 MCP Tool Definition

```json
{
  "name": "sample_silver_data",
  "description": "Retrieve sample rows from a Silver table for exploration. Returns most recent rows by default, with optional time range and ndp_id filters.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "table_name": {
        "type": "string",
        "description": "The table name (e.g., 'air_quality_observations')"
      },
      "n": {
        "type": "integer",
        "description": "Number of rows to return (default: 10, max: 100)",
        "default": 10,
        "minimum": 1,
        "maximum": 100
      },
      "filters": {
        "type": "object",
        "description": "Optional filters for the query",
        "properties": {
          "time_start": {
            "type": "string",
            "format": "date-time",
            "description": "Start of time range (inclusive)"
          },
          "time_end": {
            "type": "string",
            "format": "date-time",
            "description": "End of time range (exclusive)"
          },
          "ndp_id": {
            "type": "string",
            "description": "Filter to specific ndp_id"
          }
        }
      }
    },
    "required": ["table_name"]
  }
}
```

### 4.3 Response Schema

```json
{
  "type": "object",
  "properties": {
    "success": { "type": "boolean" },
    "table_name": { "type": "string" },
    "row_count": { "type": "integer" },
    "rows": {
      "type": "array",
      "items": { "type": "object" },
      "description": "Array of row objects with column values"
    },
    "filters_applied": {
      "type": "object",
      "description": "Echo of filters that were applied"
    },
    "note": {
      "type": "string",
      "description": "Optional note about result (e.g., 'Requested 100 but only 50 available')"
    }
  },
  "required": ["success", "table_name", "row_count", "rows"]
}
```

### 4.4 Example Request/Response

**Request (basic)**:
```json
{
  "method": "tools/call",
  "params": {
    "name": "sample_silver_data",
    "arguments": {
      "table_name": "air_quality_observations",
      "n": 5
    }
  }
}
```

**Response**:
```json
{
  "success": true,
  "table_name": "air_quality_observations",
  "row_count": 5,
  "rows": [
    {
      "observation_time": "2026-01-16T14:30:00Z",
      "ingestion_time": "2026-01-16T14:30:05Z",
      "ndp_id": "aq-airgradient-001",
      "pm25": 8.5,
      "pm10": 12.3,
      "co2": 650,
      "temperature_c": 22.1,
      "humidity_pct": 45.2,
      "voc_index": 85,
      "nox_index": 12,
      "dq_flags": null
    },
    {
      "observation_time": "2026-01-16T14:29:00Z",
      "ingestion_time": "2026-01-16T14:29:05Z",
      "ndp_id": "aq-airgradient-001",
      "pm25": 8.7,
      "pm10": 12.5,
      "co2": 655,
      "temperature_c": 22.0,
      "humidity_pct": 45.5,
      "voc_index": 87,
      "nox_index": 13,
      "dq_flags": null
    }
  ],
  "filters_applied": null,
  "note": null
}
```

**Request (with filters)**:
```json
{
  "method": "tools/call",
  "params": {
    "name": "sample_silver_data",
    "arguments": {
      "table_name": "weather_observations",
      "n": 10,
      "filters": {
        "time_start": "2026-01-15T00:00:00Z",
        "time_end": "2026-01-16T00:00:00Z",
        "ndp_id": "weather-owm-002"
      }
    }
  }
}
```

**Response**:
```json
{
  "success": true,
  "table_name": "weather_observations",
  "row_count": 10,
  "rows": [
    {
      "observation_time": "2026-01-15T23:55:00Z",
      "ingestion_time": "2026-01-15T23:55:10Z",
      "ndp_id": "weather-owm-002",
      "source_provider": "owm",
      "temperature_c": 5.2,
      "humidity_pct": 78.0,
      "pressure_pa": 101325,
      "wind_speed_kmh": 12.5,
      "wind_direction_deg": 225,
      "visibility_m": 10000,
      "cloud_cover_pct": 75,
      "dew_point_c": 1.8,
      "weather_description": "Overcast",
      "dq_flags": null
    }
  ],
  "filters_applied": {
    "time_start": "2026-01-15T00:00:00Z",
    "time_end": "2026-01-16T00:00:00Z",
    "ndp_id": "weather-owm-002"
  },
  "note": null
}
```

### 4.5 SQL Query Approach

```sql
-- Base query structure
SELECT *
FROM silver.air_quality_observations
WHERE 1=1
  -- Optional time filter (uses hypertable index)
  AND observation_time >= $2  -- time_start (if provided)
  AND observation_time < $3   -- time_end (if provided)
  -- Optional ndp_id filter
  AND ndp_id = $4             -- ndp_id (if provided)
ORDER BY observation_time DESC
LIMIT $1;  -- n (clamped to max 100)

-- Note: For weather_forecasts table, use valid_time instead of observation_time
SELECT *
FROM silver.weather_forecasts
WHERE valid_time >= $2
  AND valid_time < $3
  AND ndp_id = $4
ORDER BY valid_time DESC, issue_time DESC
LIMIT $1;
```

### 4.6 Time Column Mapping

| Table | Time Column | Order By |
|-------|-------------|----------|
| `air_quality_observations` | `observation_time` | `observation_time DESC` |
| `weather_observations` | `observation_time` | `observation_time DESC` |
| `weather_forecasts` | `valid_time` | `valid_time DESC, issue_time DESC` |
| `outdoor_air_quality` | `observation_time` | `observation_time DESC` |

### 4.7 Error Cases

| Code | Condition | Response |
|------|-----------|----------|
| `TABLE_NOT_FOUND` | Table does not exist | `{"success": false, "code": "TABLE_NOT_FOUND", "error": "Table not found: nonexistent"}` |
| `INVALID_FILTER` | Invalid filter value | `{"success": false, "code": "INVALID_FILTER", "error": "Invalid time format: not-a-date"}` |
| `TIMESCALE_UNAVAILABLE` | Cannot connect to TimescaleDB | `{"success": false, "code": "TIMESCALE_UNAVAILABLE", "error": "TimescaleDB unavailable"}` |

### 4.8 Implementation Notes

- **Default n**: 10 rows
- **Maximum n**: 100 rows (clamp with note if exceeded)
- **Time filtering**: Uses hypertable time index for efficient queries
- **Ordering**: Most recent first (time DESC)
- **JSON serialization**: Use `row_to_json()` or sqlx Row mapping
- **Timestamp handling**: Return ISO8601 format with timezone
- **NULL handling**: Preserve NULLs in JSON output (not empty strings)

---

## 5. Tool: silver_stats

### 5.1 Purpose

Get comprehensive statistics for a Silver table including row counts, time range, distinct ndp_ids, null counts per column, and DQ flag summary.

### 5.2 MCP Tool Definition

```json
{
  "name": "silver_stats",
  "description": "Get comprehensive statistics for a Silver table including row counts, time ranges, distinct identifiers, null counts per column, and data quality flag summary.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "table_name": {
        "type": "string",
        "description": "The table name (e.g., 'air_quality_observations')"
      }
    },
    "required": ["table_name"]
  }
}
```

### 5.3 Response Schema

```json
{
  "type": "object",
  "properties": {
    "success": { "type": "boolean" },
    "table_name": { "type": "string" },
    "row_count": { "type": "integer", "description": "Total row count" },
    "distinct_ndp_ids": { "type": "integer", "description": "Number of unique ndp_id values" },
    "ndp_id_list": { "type": "array", "items": { "type": "string" }, "description": "List of unique ndp_ids" },
    "time_range": {
      "type": "object",
      "properties": {
        "min": { "type": "string", "format": "date-time" },
        "max": { "type": "string", "format": "date-time" },
        "span_days": { "type": "number" }
      }
    },
    "null_counts": {
      "type": "object",
      "description": "Count of NULL values per column",
      "additionalProperties": { "type": "integer" }
    },
    "null_percentages": {
      "type": "object",
      "description": "Percentage of NULL values per column",
      "additionalProperties": { "type": "number" }
    },
    "dq_flag_summary": {
      "type": "object",
      "properties": {
        "rows_with_flags": { "type": "integer", "description": "Rows where dq_flags IS NOT NULL" },
        "rows_with_flags_pct": { "type": "number", "description": "Percentage of rows with flags" },
        "flag_counts": {
          "type": "object",
          "description": "Count of each distinct flag value",
          "additionalProperties": { "type": "integer" }
        }
      }
    },
    "chunk_info": {
      "type": "object",
      "properties": {
        "chunk_count": { "type": "integer" },
        "total_bytes": { "type": "integer" },
        "oldest_chunk": { "type": "string" },
        "newest_chunk": { "type": "string" }
      }
    }
  },
  "required": ["success", "table_name", "row_count"]
}
```

### 5.4 Example Request/Response

**Request**:
```json
{
  "method": "tools/call",
  "params": {
    "name": "silver_stats",
    "arguments": {
      "table_name": "air_quality_observations"
    }
  }
}
```

**Response**:
```json
{
  "success": true,
  "table_name": "air_quality_observations",
  "row_count": 142857,
  "distinct_ndp_ids": 2,
  "ndp_id_list": ["aq-airgradient-001", "aq-airgradient-002"],
  "time_range": {
    "min": "2026-01-01T00:00:00Z",
    "max": "2026-01-16T14:30:00Z",
    "span_days": 15.6
  },
  "null_counts": {
    "observation_time": 0,
    "ingestion_time": 0,
    "ndp_id": 0,
    "pm25": 12,
    "pm10": 45,
    "co2": 0,
    "temperature_c": 5,
    "humidity_pct": 8,
    "voc_index": 1428,
    "nox_index": 1428,
    "dq_flags": 141429
  },
  "null_percentages": {
    "observation_time": 0.0,
    "ingestion_time": 0.0,
    "ndp_id": 0.0,
    "pm25": 0.01,
    "pm10": 0.03,
    "co2": 0.0,
    "temperature_c": 0.0,
    "humidity_pct": 0.01,
    "voc_index": 1.0,
    "nox_index": 1.0,
    "dq_flags": 99.0
  },
  "dq_flag_summary": {
    "rows_with_flags": 1428,
    "rows_with_flags_pct": 1.0,
    "flag_counts": {
      "range_check:pm25:exceeded_max": 12,
      "range_check:humidity_pct:clamped_max": 8,
      "range_check:co2:exceeded_max": 5
    }
  },
  "chunk_info": {
    "chunk_count": 16,
    "total_bytes": 15728640,
    "oldest_chunk": "_hyper_1_1_chunk",
    "newest_chunk": "_hyper_1_16_chunk"
  }
}
```

### 5.5 SQL Query Approach

```sql
-- Query 1: Basic counts and time range
SELECT
    COUNT(*) AS row_count,
    COUNT(DISTINCT ndp_id) AS distinct_ndp_ids,
    ARRAY_AGG(DISTINCT ndp_id) AS ndp_id_list,
    MIN(observation_time) AS time_min,
    MAX(observation_time) AS time_max,
    EXTRACT(EPOCH FROM (MAX(observation_time) - MIN(observation_time))) / 86400.0 AS span_days
FROM silver.air_quality_observations;

-- Query 2: NULL counts per column (dynamic based on column list)
SELECT
    COUNT(*) - COUNT(observation_time) AS observation_time_nulls,
    COUNT(*) - COUNT(ndp_id) AS ndp_id_nulls,
    COUNT(*) - COUNT(pm25) AS pm25_nulls,
    COUNT(*) - COUNT(pm10) AS pm10_nulls,
    COUNT(*) - COUNT(co2) AS co2_nulls,
    COUNT(*) - COUNT(temperature_c) AS temperature_c_nulls,
    COUNT(*) - COUNT(humidity_pct) AS humidity_pct_nulls,
    COUNT(*) - COUNT(voc_index) AS voc_index_nulls,
    COUNT(*) - COUNT(nox_index) AS nox_index_nulls,
    COUNT(*) - COUNT(dq_flags) AS dq_flags_nulls
FROM silver.air_quality_observations;

-- Query 3: DQ flag summary
SELECT
    COUNT(*) FILTER (WHERE dq_flags IS NOT NULL AND array_length(dq_flags, 1) > 0) AS rows_with_flags,
    ROUND(
        100.0 * COUNT(*) FILTER (WHERE dq_flags IS NOT NULL AND array_length(dq_flags, 1) > 0)
        / NULLIF(COUNT(*), 0)::NUMERIC,
        2
    ) AS rows_with_flags_pct
FROM silver.air_quality_observations;

-- Query 4: Flag value counts (unnest array)
SELECT
    flag_value,
    COUNT(*) AS flag_count
FROM silver.air_quality_observations,
     LATERAL unnest(dq_flags) AS flag_value
WHERE dq_flags IS NOT NULL
GROUP BY flag_value
ORDER BY flag_count DESC
LIMIT 20;

-- Query 5: Chunk information
-- Note: Use hypertable_size() function as chunks view lacks total_bytes column
SELECT
    COUNT(*) AS chunk_count,
    hypertable_size(format('silver.%I', $1)::regclass) AS total_bytes,
    MIN(chunk_name) AS oldest_chunk,
    MAX(chunk_name) AS newest_chunk
FROM timescaledb_information.chunks
WHERE hypertable_schema = 'silver'
  AND hypertable_name = $1;
```

### 5.6 Error Cases

| Code | Condition | Response |
|------|-----------|----------|
| `TABLE_NOT_FOUND` | Table does not exist | `{"success": false, "code": "TABLE_NOT_FOUND", "error": "Table not found: nonexistent"}` |
| `TIMESCALE_UNAVAILABLE` | Cannot connect to TimescaleDB | `{"success": false, "code": "TIMESCALE_UNAVAILABLE", "error": "TimescaleDB unavailable"}` |

### 5.7 Implementation Notes

- **Performance**: These queries scan the entire table; consider caching results with TTL
- **Dynamic column list**: Get column names from information_schema, build NULL count query dynamically
- **DQ flag unnest**: Handle empty arrays (check array_length before unnest)
- **Chunk info**: May require superuser or TimescaleDB read permissions

---

## 6. Rust Implementation Patterns

### 6.1 SilverStorage Trait

```rust
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Silver table metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SilverTableInfo {
    pub table_name: String,
    pub schema: String,
    pub full_name: String,
    pub description: Option<String>,
    pub grain: Option<String>,
    pub source_streams: Vec<String>,
    pub is_hypertable: bool,
    pub hypertable_column: Option<String>,
    pub chunk_interval: Option<String>,
    pub row_count: Option<i64>,
    pub time_range: Option<TimeRange>,
    pub chunk_count: Option<i32>,
    pub total_bytes: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub min: DateTime<Utc>,
    pub max: DateTime<Utc>,
}

/// Silver column metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SilverColumnInfo {
    pub name: String,
    #[serde(rename = "type")]
    pub data_type: String,
    pub unit: Option<String>,
    pub description: Option<String>,
    pub nullable: bool,
    pub is_primary_key: bool,
    pub sort_order: i32,
}

/// Sample data filters
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SampleFilters {
    pub time_start: Option<DateTime<Utc>>,
    pub time_end: Option<DateTime<Utc>>,
    pub ndp_id: Option<String>,
}

/// Table statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SilverTableStats {
    pub table_name: String,
    pub row_count: i64,
    pub distinct_ndp_ids: i32,
    pub ndp_id_list: Vec<String>,
    pub time_range: Option<TimeRange>,
    pub null_counts: std::collections::HashMap<String, i64>,
    pub null_percentages: std::collections::HashMap<String, f64>,
    pub dq_flag_summary: DqFlagSummary,
    pub chunk_info: Option<ChunkInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DqFlagSummary {
    pub rows_with_flags: i64,
    pub rows_with_flags_pct: f64,
    pub flag_counts: std::collections::HashMap<String, i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkInfo {
    pub chunk_count: i32,
    pub total_bytes: i64,
    pub oldest_chunk: String,
    pub newest_chunk: String,
}

/// Storage errors
#[derive(Debug, thiserror::Error)]
pub enum SilverStorageError {
    #[error("Table not found: {0}")]
    TableNotFound(String),
    #[error("Database unavailable: {0}")]
    Unavailable(String),
    #[error("Query failed: {0}")]
    QueryFailed(String),
    #[error("Invalid filter: {0}")]
    InvalidFilter(String),
}

/// SilverStorage trait for TimescaleDB access
#[async_trait]
pub trait SilverStorage: Send + Sync {
    /// List all Silver tables with metadata
    async fn list_tables(&self) -> Result<Vec<SilverTableInfo>, SilverStorageError>;

    /// Describe a specific table's schema
    async fn describe_table(&self, table_name: &str) -> Result<SilverTableSchema, SilverStorageError>;

    /// Sample rows from a table
    async fn sample(
        &self,
        table_name: &str,
        limit: usize,
        filters: Option<SampleFilters>,
    ) -> Result<Vec<serde_json::Value>, SilverStorageError>;

    /// Get table statistics
    async fn stats(&self, table_name: &str) -> Result<SilverTableStats, SilverStorageError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SilverTableSchema {
    pub table_name: String,
    pub full_name: String,
    pub description: Option<String>,
    pub grain: Option<String>,
    pub source_streams: Vec<String>,
    pub primary_key: Vec<String>,
    pub hypertable_info: Option<HypertableInfo>,
    pub columns: Vec<SilverColumnInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HypertableInfo {
    pub time_column: String,
    pub chunk_interval: String,
}
```

### 6.2 Tool Module Pattern

```rust
//! list_silver_tables MCP Tool (dp-010)
//!
//! Enumerates all Silver layer hypertables with metadata.

use crate::mcp::tools::{
    create_error_response, create_tool_response, error_codes, AppState,
};
use crate::mcp::{McpRpcError, ToolDefinition};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// Input/Output types...

/// Get the MCP tool definition
pub fn tool_definition() -> ToolDefinition {
    ToolDefinition::no_params(
        "list_silver_tables",
        "List all Silver layer hypertables with metadata including row counts, time ranges, and chunk information.",
    )
}

/// Execute the tool
pub async fn execute(state: &AppState, _args: Value) -> Result<Value, McpRpcError> {
    // 1. Get table list from Silver storage
    let tables = match state.silver.list_tables().await {
        Ok(t) => t,
        Err(SilverStorageError::Unavailable(msg)) => {
            return create_error_response(
                error_codes::TIMESCALE_UNAVAILABLE,
                &format!("TimescaleDB unavailable: {}", msg),
                None,
            );
        }
        Err(e) => return Err(McpRpcError::new(-32603, format!("Silver error: {}", e))),
    };

    // 2. Build response
    let output = ListSilverTablesOutput { tables };
    create_tool_response(output)
}
```

---

## 7. Error Codes

| Code | Constant | HTTP Analog | Usage |
|------|----------|-------------|-------|
| `TABLE_NOT_FOUND` | `error_codes::TABLE_NOT_FOUND` | 404 | Table does not exist in silver schema |
| `TIMESCALE_UNAVAILABLE` | `error_codes::TIMESCALE_UNAVAILABLE` | 503 | Cannot connect to TimescaleDB |
| `DICTIONARY_UNAVAILABLE` | `error_codes::DICTIONARY_UNAVAILABLE` | 503 | data_dictionary schema not populated |
| `INVALID_FILTER` | `error_codes::INVALID_FILTER` | 400 | Filter parameter has invalid format |
| `INTERNAL_ERROR` | `error_codes::INTERNAL_ERROR` | 500 | Unexpected internal error |

---

## 8. Configuration

### 8.1 Environment Variables

```bash
# TimescaleDB connection (required)
NDP_TIMESCALE_URL=postgresql://ndp:password@localhost:5432/ndp

# Schema names (optional, have defaults)
NDP_SILVER_SCHEMA=silver
NDP_DICTIONARY_SCHEMA=data_dictionary

# Connection pool settings (optional)
NDP_TIMESCALE_MAX_CONNECTIONS=5
NDP_TIMESCALE_CONNECT_TIMEOUT_SECS=10
```

### 8.2 AppState Extension

```rust
pub struct AppState {
    // Existing (Bronze)
    pub storage: Arc<dyn BronzeStorage>,
    pub config: Arc<dyn ConfigStore>,

    // New (Silver)
    pub silver: Arc<dyn SilverStorage>,
}
```

---

## 9. Testing Requirements

### 9.1 Unit Tests (Mock-Driven)

| Test ID | Tool | Scenario |
|---------|------|----------|
| TC-LST-001 | list_silver_tables | Returns all 4 Silver tables |
| TC-LST-002 | list_silver_tables | Handles TimescaleDB unavailable |
| TC-LST-003 | list_silver_tables | Includes hypertable metadata |
| TC-DST-001 | describe_silver_table | Returns column definitions with units |
| TC-DST-002 | describe_silver_table | Handles table not found |
| TC-DST-003 | describe_silver_table | Includes primary key info |
| TC-SSD-001 | sample_silver_data | Returns N rows ordered by time DESC |
| TC-SSD-002 | sample_silver_data | Applies time range filter |
| TC-SSD-003 | sample_silver_data | Applies ndp_id filter |
| TC-SSD-004 | sample_silver_data | Clamps n to MAX_N (100) |
| TC-SSD-005 | sample_silver_data | Handles empty table |
| TC-SST-001 | silver_stats | Returns row count and time range |
| TC-SST-002 | silver_stats | Returns null counts per column |
| TC-SST-003 | silver_stats | Returns DQ flag summary |
| TC-SST-004 | silver_stats | Returns chunk info |

### 9.2 Integration Tests

| Test ID | Scenario |
|---------|----------|
| IT-SILVER-001 | list_silver_tables against real TimescaleDB |
| IT-SILVER-002 | describe_silver_table returns correct column types |
| IT-SILVER-003 | sample_silver_data with time filter uses index |
| IT-SILVER-004 | silver_stats handles large table (>1M rows) |

---

## 10. References

- [dp-010 SCOPE.md](/workspaces/neural-data-platform/product/features/dp-010/SCOPE.md) - Feature scope
- [dp-009 ADR-009-001](/workspaces/neural-data-platform/product/features/dp-009/architecture/ADR-009-001-silver-dictionary-tables.md) - Data dictionary tables
- [dp-009 silver-tables-spec](/workspaces/neural-data-platform/product/features/dp-009/specification/silver-tables-spec.md) - Silver column definitions
- [001_silver_schema.sql](/workspaces/neural-data-platform/deploy/timescaledb/migrations/001_silver_schema.sql) - Silver DDL
- [Bronze MCP list_streams.rs](/workspaces/neural-data-platform/core/src/mcp/tools/list_streams.rs) - Pattern reference
- [Bronze MCP sample_data.rs](/workspaces/neural-data-platform/core/src/mcp/tools/sample_data.rs) - Pattern reference

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-16 | ndp-timescale-dev | Initial specification |

---

*Specification complete: 2026-01-16*
