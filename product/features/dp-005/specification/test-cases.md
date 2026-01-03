# dp-005: Bronze MCP Server - Test Cases

## Overview

This document specifies detailed test cases for all 4 MCP tools in the Bronze MCP Server. Each test case follows the Arrange-Act-Assert pattern with clear inputs, expected outputs, and verification criteria.

---

## Test Case Naming Convention

```
TC-{tool_prefix}-{number}: {brief_description}
```

| Tool | Prefix |
|------|--------|
| list_streams | LS |
| describe_schema | DS |
| validate_config | VC |
| sample_data | SD |
| Integration/Protocol | INT |

---

## 1. list_streams Tests

### TC-LS-001: Returns all configured streams

**Scope**: Unit
**Priority**: P0 (Critical)

**Preconditions**:
- etcd contains 3 stream configurations: `air-quality`, `outdoor-weather`, `nws-forecast-hourly`
- Bronze directory contains data for `air-quality` and `outdoor-weather`

**Input**:
```json
{}
```

**Expected Output**:
```json
{
  "success": true,
  "streams": [
    {
      "stream_id": "air-quality",
      "description": "AirGradient sensor readings from MQTT",
      "enabled": true,
      "version": "1.0.0",
      "sources": ["mqtt"],
      "storage": {
        "latest_partition": "year=2026/month=01/day=03",
        "file_size_bytes": 7310,
        "file_modified": "2026-01-03T14:54:00Z"
      }
    },
    {
      "stream_id": "outdoor-weather",
      "description": "Outdoor weather data from OpenWeatherMap",
      "enabled": true,
      "version": "1.0.0",
      "sources": ["http_poll"],
      "storage": { ... }
    },
    {
      "stream_id": "nws-forecast-hourly",
      "description": "NWS hourly forecast data",
      "enabled": false,
      "version": "1.0.0",
      "sources": ["http_poll"],
      "storage": null
    }
  ]
}
```

**Verification**:
- [ ] Returns exactly 3 streams
- [ ] Each stream has required fields: `stream_id`, `description`, `enabled`, `version`, `sources`
- [ ] `storage` is `null` for streams without data files
- [ ] `storage.latest_partition` reflects most recent date
- [ ] Disabled stream (`nws-forecast-hourly`) shows `enabled: false`

**Mock Setup**:
```rust
mock_config.expect_list_stream_ids()
    .returning(|| Ok(vec!["air-quality", "outdoor-weather", "nws-forecast-hourly"]));

mock_config.expect_get_stream_config()
    .with(eq("air-quality"))
    .returning(|_| Ok(StreamConfig { enabled: true, ... }));
// ... similar for other streams

mock_storage.expect_list_streams()
    .returning(|| Ok(vec![
        StreamInfo { stream_id: "air-quality", ... },
        StreamInfo { stream_id: "outdoor-weather", ... },
    ]));
```

---

### TC-LS-002: Handles empty Bronze directory

**Scope**: Unit
**Priority**: P1 (High)

**Preconditions**:
- etcd contains stream configurations
- Bronze directory `/data/raw/` is empty (no stream subdirectories)

**Input**:
```json
{}
```

**Expected Output**:
```json
{
  "success": true,
  "streams": [
    {
      "stream_id": "air-quality",
      "enabled": true,
      "storage": null
    }
  ]
}
```

**Verification**:
- [ ] Returns streams from config even without data
- [ ] `storage` is `null` for all streams
- [ ] No error thrown

---

### TC-LS-003: Handles etcd unavailable (fail fast)

**Scope**: Unit
**Priority**: P0 (Critical)

**Preconditions**:
- etcd is unreachable or returns connection error

**Input**:
```json
{}
```

**Expected Output**:
```json
{
  "success": false,
  "error": "Configuration unavailable: etcd connection failed"
}
```

**Verification**:
- [ ] Returns error immediately (no retry)
- [ ] Error message indicates etcd issue
- [ ] Does not fall back to stale data
- [ ] Response time < 5 seconds (timeout)

**Mock Setup**:
```rust
mock_config.expect_list_stream_ids()
    .returning(|| Err(ConfigError::ConnectionFailed("connection refused".to_string())));
```

---

### TC-LS-004: Storage metadata accurate

**Scope**: Integration
**Priority**: P1 (High)

**Preconditions**:
- Real Parquet file at `/data/raw/air-quality/year=2026/month=01/day=03/data.parquet`
- File size: 7310 bytes
- File modified: 2026-01-03T14:54:00Z

**Input**:
```json
{}
```

**Expected Output**:
```json
{
  "success": true,
  "streams": [{
    "stream_id": "air-quality",
    "storage": {
      "latest_partition": "year=2026/month=01/day=03",
      "file_size_bytes": 7310,
      "file_modified": "2026-01-03T14:54:00Z"
    }
  }]
}
```

**Verification**:
- [ ] `file_size_bytes` matches actual file size
- [ ] `file_modified` matches file system mtime
- [ ] `latest_partition` reflects most recent date directory

---

## 2. describe_schema Tests

### TC-DS-010: Mode=source returns raw_payload structure + mappings

**Scope**: Unit
**Priority**: P0 (Critical)

**Preconditions**:
- Parquet file exists with `raw_payload` column
- etcd config has `sources[].parser.field_mappings`

**Input**:
```json
{
  "stream_id": "outdoor-weather",
  "mode": "source"
}
```

**Expected Output**:
```json
{
  "success": true,
  "stream_id": "outdoor-weather",
  "mode": "source",
  "raw_payload_structure": {
    "keys": ["base", "clouds", "cod", "coord", "dt", "id", "main", "name", "sys", "timezone", "visibility", "weather", "wind"],
    "nested": {
      "main": ["feels_like", "grnd_level", "humidity", "pressure", "sea_level", "temp", "temp_max", "temp_min"],
      "wind": ["deg", "gust", "speed"],
      "coord": ["lat", "lon"]
    }
  },
  "parser_type": "json_path",
  "field_mappings": [
    {"source_path": "main.temp", "target_field": "temperature", "unit": "celsius"},
    {"source_path": "main.humidity", "target_field": "humidity", "unit": "percent"}
  ],
  "unmapped_source_fields": ["base", "cod", "coord", "dt", "id", "name", "sys", "timezone", "weather"],
  "file_analyzed": "/data/raw/outdoor-weather/year=2026/month=01/day=03/data.parquet"
}
```

**Verification**:
- [ ] `raw_payload_structure.keys` lists all top-level JSON keys
- [ ] `raw_payload_structure.nested` shows nested object keys
- [ ] `field_mappings` from config are included
- [ ] `unmapped_source_fields` identifies fields without mappings
- [ ] `file_analyzed` shows actual file path used

---

### TC-DS-011: Mode=target returns entity_schemas

**Scope**: Unit
**Priority**: P0 (Critical)

**Preconditions**:
- etcd config has `entity_schemas` array

**Input**:
```json
{
  "stream_id": "outdoor-weather",
  "mode": "target"
}
```

**Expected Output**:
```json
{
  "success": true,
  "stream_id": "outdoor-weather",
  "mode": "target",
  "entity_schema": "nws-weather",
  "attributes": [
    {"name": "temperature", "type": "float", "unit": "celsius", "nullable": false},
    {"name": "humidity", "type": "float", "unit": "percent", "nullable": true},
    {"name": "wind_speed", "type": "float", "unit": "m/s", "nullable": true}
  ]
}
```

**Verification**:
- [ ] `entity_schema` matches config `entity_schemas[0].schema_name`
- [ ] `attributes` includes all fields from config
- [ ] Each attribute has `name`, `type`, `unit`, `nullable`

---

### TC-DS-012: Mode=all includes gap_analysis

**Scope**: Unit
**Priority**: P0 (Critical)

**Preconditions**:
- Parquet file with `raw_payload` containing fields not mapped
- Config with target fields not in source

**Input**:
```json
{
  "stream_id": "outdoor-weather",
  "mode": "all"
}
```

**Expected Output**:
```json
{
  "success": true,
  "stream_id": "outdoor-weather",
  "mode": "all",
  "source": {
    "raw_payload_structure": { ... },
    "field_mappings": [ ... ]
  },
  "target": {
    "entity_schema": "nws-weather",
    "attributes": [ ... ]
  },
  "gap_analysis": {
    "unmapped_source_fields": ["base", "cod", "coord"],
    "target_fields_without_mapping": ["rain_1h", "snow_1h"]
  }
}
```

**Verification**:
- [ ] `source` section matches TC-DS-010 output
- [ ] `target` section matches TC-DS-011 output
- [ ] `gap_analysis.unmapped_source_fields` lists raw_payload keys without mapping
- [ ] `gap_analysis.target_fields_without_mapping` lists entity attributes not in mappings

---

### TC-DS-013: Handles stream without data file

**Scope**: Unit
**Priority**: P1 (High)

**Preconditions**:
- Stream exists in etcd config
- No Parquet files in `/data/raw/{stream_id}/`

**Input**:
```json
{
  "stream_id": "nws-forecast-hourly",
  "mode": "source"
}
```

**Expected Output**:
```json
{
  "success": true,
  "stream_id": "nws-forecast-hourly",
  "mode": "source",
  "raw_payload_structure": null,
  "parser_type": "column_oriented",
  "field_mappings": [ ... ],
  "unmapped_source_fields": null,
  "file_analyzed": null,
  "note": "No Bronze data available for this stream. Schema derived from config only."
}
```

**Verification**:
- [ ] Does not error
- [ ] `raw_payload_structure` is `null`
- [ ] `field_mappings` still populated from config
- [ ] `note` explains missing data

---

### TC-DS-014: Mode defaults to 'all'

**Scope**: Unit
**Priority**: P2 (Medium)

**Input**:
```json
{
  "stream_id": "air-quality"
}
```

**Verification**:
- [ ] Returns same structure as `mode=all`
- [ ] Includes `source`, `target`, and `gap_analysis`

---

### TC-DS-015: Invalid stream_id returns error

**Scope**: Unit
**Priority**: P1 (High)

**Input**:
```json
{
  "stream_id": "nonexistent-stream",
  "mode": "all"
}
```

**Expected Output**:
```json
{
  "success": false,
  "error": "Stream not found: nonexistent-stream"
}
```

---

## 3. validate_config Tests

### TC-VC-020: Detects matching fields

**Scope**: Unit
**Priority**: P0 (Critical)

**Preconditions**:
- Config has fields: `["temperature", "humidity", "pressure"]`
- raw_payload has keys: `["temperature", "humidity", "pressure", "extra"]`

**Input**:
```json
{
  "stream_id": "simple-weather"
}
```

**Expected Output**:
```json
{
  "success": true,
  "stream_id": "simple-weather",
  "entity_schema": "simple-weather",
  "validation": {
    "status": "partial_match",
    "config_fields": ["temperature", "humidity", "pressure"],
    "raw_payload_fields": ["temperature", "humidity", "pressure", "extra"],
    "analysis": {
      "in_config_not_in_payload": [],
      "in_payload_not_in_config": ["extra"],
      "matching": ["temperature", "humidity", "pressure"]
    }
  }
}
```

**Verification**:
- [ ] `matching` lists all common fields
- [ ] `in_config_not_in_payload` is empty
- [ ] `in_payload_not_in_config` lists extra source field
- [ ] `status` is `partial_match` (not `mismatch` or `match`)

---

### TC-VC-021: Reports fields in config but not in payload

**Scope**: Unit
**Priority**: P0 (Critical)

**Preconditions**:
- Config defines: `["temperature", "humidity", "rain_1h"]`
- raw_payload contains: `["temperature", "humidity"]`

**Input**:
```json
{
  "stream_id": "weather-missing"
}
```

**Expected Output**:
```json
{
  "success": true,
  "validation": {
    "status": "mismatch",
    "analysis": {
      "in_config_not_in_payload": ["rain_1h"],
      "in_payload_not_in_config": [],
      "matching": ["temperature", "humidity"]
    },
    "notes": "Field 'rain_1h' defined in config but not present in raw_payload. Verify field_mappings or source data."
  }
}
```

**Verification**:
- [ ] `in_config_not_in_payload` correctly identifies missing source field
- [ ] `notes` provides actionable guidance

---

### TC-VC-022: Reports fields in payload but not in config

**Scope**: Unit
**Priority**: P0 (Critical)

**Preconditions**:
- Config defines: `["temperature"]`
- raw_payload contains: `["temperature", "humidity", "pressure"]`

**Input**:
```json
{
  "stream_id": "weather-extra"
}
```

**Expected Output**:
```json
{
  "success": true,
  "validation": {
    "status": "partial_match",
    "analysis": {
      "in_config_not_in_payload": [],
      "in_payload_not_in_config": ["humidity", "pressure"],
      "matching": ["temperature"]
    },
    "notes": "Fields in raw_payload not defined in entity_schemas. These are available for future ETL development."
  }
}
```

**Verification**:
- [ ] Extra fields identified but not treated as errors
- [ ] `status` is `partial_match`, not `mismatch`

---

### TC-VC-023: Handles nested raw_payload structure

**Scope**: Unit
**Priority**: P0 (Critical)

**Preconditions**:
- raw_payload has nested structure: `{"main": {"temp": 20}, "wind": {"speed": 5}}`
- Config field_mappings: `[{"source_path": "main.temp", "target_field": "temperature"}]`
- entity_schemas attributes: `[{"name": "temperature"}]`

**Input**:
```json
{
  "stream_id": "nested-weather"
}
```

**Expected Output**:
```json
{
  "success": true,
  "validation": {
    "status": "mapped",
    "config_fields": ["temperature"],
    "raw_payload_fields": ["main", "wind"],
    "raw_payload_nested": {
      "main": ["temp"],
      "wind": ["speed"]
    },
    "analysis": {
      "mapped_correctly": ["temperature -> main.temp"],
      "unmapped_nested_fields": ["main.humidity", "wind.speed", "wind.deg"],
      "matching": ["temperature"]
    },
    "notes": "Config uses flattened field names; raw_payload preserves source structure. Mapping verified via field_mappings."
  }
}
```

**Verification**:
- [ ] Nested structure is flattened for comparison
- [ ] Mappings are traced through `source_path`
- [ ] `mapped_correctly` shows the path resolution
- [ ] `unmapped_nested_fields` lists nested fields without mappings

---

### TC-VC-024: Handles stream with no entity_schemas

**Scope**: Unit
**Priority**: P1 (High)

**Preconditions**:
- Stream config exists but has no `entity_schemas` defined

**Input**:
```json
{
  "stream_id": "legacy-stream"
}
```

**Expected Output**:
```json
{
  "success": true,
  "validation": {
    "status": "no_target_schema",
    "raw_payload_fields": ["temp", "humidity"],
    "notes": "No entity_schemas defined for this stream. Add entity_schemas to config for Silver layer mapping."
  }
}
```

---

## 4. sample_data Tests

### TC-SD-030: Returns N most recent rows

**Scope**: Unit
**Priority**: P0 (Critical)

**Preconditions**:
- Parquet file has 100 rows, sorted by timestamp descending

**Input**:
```json
{
  "stream_id": "air-quality",
  "n": 5
}
```

**Expected Output**:
```json
{
  "success": true,
  "stream_id": "air-quality",
  "row_count": 5,
  "rows": [
    {
      "timestamp": 1767452639760716,
      "source_id": "air-quality-Mqtt",
      "ndp_id": "airgradient-office-001",
      "context": {"location": {"path": "office", "type": "indoor"}},
      "raw_payload": {"pm02": 12, "rco2": 800, "atmp": 22.5}
    },
    // ... 4 more rows
  ],
  "source_file": "/data/raw/air-quality/year=2026/month=01/day=03/data.parquet"
}
```

**Verification**:
- [ ] Returns exactly `n` rows
- [ ] Rows ordered by timestamp descending (most recent first)
- [ ] Each row has Bronze envelope structure: `timestamp`, `source_id`, `ndp_id`, `context`, `raw_payload`
- [ ] `source_file` indicates file used

---

### TC-SD-031: Handles n > available rows

**Scope**: Unit
**Priority**: P1 (High)

**Preconditions**:
- Parquet file has only 3 rows

**Input**:
```json
{
  "stream_id": "sparse-stream",
  "n": 100
}
```

**Expected Output**:
```json
{
  "success": true,
  "stream_id": "sparse-stream",
  "row_count": 3,
  "rows": [ ... ],  // 3 rows
  "source_file": "/data/raw/sparse-stream/year=2026/month=01/day=03/data.parquet",
  "note": "Requested 100 rows but only 3 available"
}
```

**Verification**:
- [ ] Returns all available rows (3)
- [ ] `row_count` reflects actual count
- [ ] `note` explains the difference

---

### TC-SD-032: Returns proper Bronze envelope structure

**Scope**: Unit
**Priority**: P0 (Critical)

**Preconditions**:
- Parquet file with complete Bronze envelope

**Input**:
```json
{
  "stream_id": "outdoor-weather",
  "n": 1
}
```

**Expected Output**:
```json
{
  "success": true,
  "rows": [{
    "timestamp": 1767452639760716,
    "source_id": "outdoor-weather-Http",
    "ndp_id": "weather-owm-002",
    "context": {
      "location": {
        "coordinates": [29.95838, -81.30878],
        "path": "beachhouse",
        "type": "outdoor"
      }
    },
    "raw_payload": {
      "main": {"temp": 19.72, "humidity": 76, "pressure": 1015},
      "wind": {"speed": 5.66, "deg": 220, "gust": 8.2},
      "clouds": {"all": 75},
      "visibility": 10000
    }
  }]
}
```

**Verification**:
- [ ] `timestamp` is INT64 (microseconds)
- [ ] `source_id` follows `{stream_id}-{SourceType}` pattern
- [ ] `ndp_id` is stable identifier from config
- [ ] `context` is JSON object with location metadata
- [ ] `raw_payload` preserves exact source JSON structure

---

### TC-SD-033: Handles stream with no data

**Scope**: Unit
**Priority**: P1 (High)

**Input**:
```json
{
  "stream_id": "empty-stream",
  "n": 10
}
```

**Expected Output**:
```json
{
  "success": true,
  "stream_id": "empty-stream",
  "row_count": 0,
  "rows": [],
  "source_file": null,
  "note": "No data available for this stream"
}
```

---

### TC-SD-034: Default n value is 10

**Scope**: Unit
**Priority**: P2 (Medium)

**Input**:
```json
{
  "stream_id": "air-quality"
}
```

**Verification**:
- [ ] Returns exactly 10 rows (or all if < 10 available)

---

### TC-SD-035: Maximum n is 100

**Scope**: Unit
**Priority**: P1 (High)

**Input**:
```json
{
  "stream_id": "air-quality",
  "n": 1000
}
```

**Expected Output**:
```json
{
  "success": true,
  "row_count": 100,
  "note": "Requested 1000 rows but maximum is 100. Returning 100."
}
```

---

## 5. Integration Tests - MCP Protocol

### TC-INT-001: tools/list returns all 4 tools

**Scope**: Integration
**Priority**: P0 (Critical)

**Request**:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/list"
}
```

**Expected Response**:
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "tools": [
      {
        "name": "list_streams",
        "description": "List all available Bronze layer streams with metadata",
        "inputSchema": { "type": "object", "properties": {}, "required": [] }
      },
      {
        "name": "describe_schema",
        "description": "Get schema information for a stream...",
        "inputSchema": { "type": "object", "properties": { ... }, "required": ["stream_id"] }
      },
      {
        "name": "validate_config",
        "description": "Compare stream configuration in etcd against actual Bronze Parquet schema...",
        "inputSchema": { ... }
      },
      {
        "name": "sample_data",
        "description": "Retrieve sample rows from a Bronze stream...",
        "inputSchema": { ... }
      }
    ]
  }
}
```

**Verification**:
- [ ] Response is valid JSON-RPC 2.0
- [ ] `result.tools` is array of 4 tools
- [ ] Each tool has `name`, `description`, `inputSchema`
- [ ] `inputSchema` is valid JSON Schema

---

### TC-INT-002: tools/call invokes correct tool

**Scope**: Integration
**Priority**: P0 (Critical)

**Request**:
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "list_streams",
    "arguments": {}
  }
}
```

**Expected Response**:
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "content": [{
      "type": "text",
      "text": "{\"success\": true, \"streams\": [...]}"
    }]
  }
}
```

**Verification**:
- [ ] Response wraps tool output in MCP `content` format
- [ ] `content[0].type` is `"text"`
- [ ] `content[0].text` contains valid JSON

---

### TC-INT-003: tools/call with invalid tool name

**Scope**: Integration
**Priority**: P1 (High)

**Request**:
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "invalid_tool",
    "arguments": {}
  }
}
```

**Expected Response**:
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "error": {
    "code": -32602,
    "message": "Unknown tool: invalid_tool"
  }
}
```

---

### TC-INT-004: Health endpoint returns server status

**Scope**: Integration
**Priority**: P1 (High)

**Request**: `GET /health`

**Expected Response**:
```json
{
  "status": "healthy",
  "version": "0.1.0",
  "checks": {
    "etcd": "connected",
    "bronze_storage": "accessible"
  }
}
```

---

### TC-INT-005: Invalid JSON-RPC request handling

**Scope**: Integration
**Priority**: P1 (High)

**Request**:
```json
{
  "invalid": "request"
}
```

**Expected Response**:
```json
{
  "jsonrpc": "2.0",
  "id": null,
  "error": {
    "code": -32600,
    "message": "Invalid Request"
  }
}
```

---

## Test Case Summary

| Tool | Test Cases | Priority Distribution |
|------|------------|----------------------|
| list_streams | TC-LS-001 to TC-LS-004 | 2 P0, 2 P1 |
| describe_schema | TC-DS-010 to TC-DS-015 | 3 P0, 2 P1, 1 P2 |
| validate_config | TC-VC-020 to TC-VC-024 | 4 P0, 1 P1 |
| sample_data | TC-SD-030 to TC-SD-035 | 2 P0, 3 P1, 1 P2 |
| Integration | TC-INT-001 to TC-INT-005 | 2 P0, 3 P1 |
| **Total** | **24** | **13 P0, 9 P1, 2 P2** |

---

## Related Documents

- `test-plan.md` - Overall testing strategy
- `test-fixtures.md` - Test data specifications
- `/product/features/dp-005/SCOPE.md` - Feature requirements
