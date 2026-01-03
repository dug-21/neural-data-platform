# DP-005: Bronze MCP Server - Interface Specification

**Document Type**: SPARC Specification
**Version**: 1.0.0
**Last Updated**: 2026-01-03
**Status**: Draft

---

## Overview

This document defines the API contracts for the Bronze MCP Server, including MCP JSON-RPC formats, tool input schemas, tool output structures, and error codes.

---

## Transport Layer

### HTTP Endpoints

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/mcp` | POST | MCP JSON-RPC protocol messages |
| `/health` | GET | Health check for monitoring |

### Request Headers

```http
POST /mcp HTTP/1.1
Host: {NDP_MCP_LISTEN}
Content-Type: application/json
Accept: application/json
```

### Health Endpoint

**Request:**
```http
GET /health HTTP/1.1
```

**Response (200 OK):**
```json
{
  "status": "ok",
  "version": "0.1.0",
  "uptime_seconds": 3600
}
```

**Response (503 Service Unavailable):**
```json
{
  "status": "degraded",
  "version": "0.1.0",
  "error": "etcd connection failed"
}
```

---

## MCP Protocol Messages

### JSON-RPC Envelope

All MCP messages follow JSON-RPC 2.0 format:

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": "request-uuid",
  "method": "method/name",
  "params": { /* method-specific */ }
}
```

**Response (Success):**
```json
{
  "jsonrpc": "2.0",
  "id": "request-uuid",
  "result": { /* method-specific */ }
}
```

**Response (Error):**
```json
{
  "jsonrpc": "2.0",
  "id": "request-uuid",
  "error": {
    "code": -32600,
    "message": "Invalid request"
  }
}
```

### Method: tools/list

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": "1",
  "method": "tools/list",
  "params": {}
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": "1",
  "result": {
    "tools": [
      {
        "name": "list_streams",
        "description": "List all available Bronze layer streams with metadata",
        "inputSchema": {
          "type": "object",
          "properties": {},
          "required": []
        }
      },
      {
        "name": "describe_schema",
        "description": "Get schema information for a stream. Modes: source (raw_payload structure + field mappings), target (entity_schemas), all (complete ETL picture with gap analysis)",
        "inputSchema": {
          "type": "object",
          "properties": {
            "stream_id": {
              "type": "string",
              "description": "The stream identifier (e.g., 'air-quality', 'outdoor-weather')"
            },
            "mode": {
              "type": "string",
              "enum": ["all", "source", "target"],
              "description": "Schema view mode (default: all)",
              "default": "all"
            }
          },
          "required": ["stream_id"]
        }
      },
      {
        "name": "validate_config",
        "description": "Compare stream configuration in etcd against actual Bronze Parquet schema. Detects mismatches, missing fields, and extra fields.",
        "inputSchema": {
          "type": "object",
          "properties": {
            "stream_id": {
              "type": "string",
              "description": "The stream identifier to validate"
            }
          },
          "required": ["stream_id"]
        }
      },
      {
        "name": "sample_data",
        "description": "Retrieve sample rows from a Bronze stream for exploration",
        "inputSchema": {
          "type": "object",
          "properties": {
            "stream_id": {
              "type": "string",
              "description": "The stream identifier"
            },
            "n": {
              "type": "integer",
              "description": "Number of rows to return (default: 10, max: 100)",
              "default": 10,
              "minimum": 1,
              "maximum": 100
            }
          },
          "required": ["stream_id"]
        }
      }
    ]
  }
}
```

### Method: tools/call

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": "2",
  "method": "tools/call",
  "params": {
    "name": "tool_name",
    "arguments": { /* tool-specific */ }
  }
}
```

**Response (Success):**
```json
{
  "jsonrpc": "2.0",
  "id": "2",
  "result": {
    "content": [
      {
        "type": "text",
        "text": "{\"success\": true, \"data\": {...}}"
      }
    ]
  }
}
```

**Response (Tool Error):**
```json
{
  "jsonrpc": "2.0",
  "id": "2",
  "result": {
    "content": [
      {
        "type": "text",
        "text": "{\"success\": false, \"error\": \"Stream not found: invalid-stream\", \"code\": \"STREAM_NOT_FOUND\"}"
      }
    ],
    "isError": true
  }
}
```

---

## Tool Input Schemas

### list_streams

```json
{
  "type": "object",
  "properties": {},
  "required": [],
  "additionalProperties": false
}
```

**Notes:**
- No input parameters required
- Returns all streams regardless of enabled status

### describe_schema

```json
{
  "type": "object",
  "properties": {
    "stream_id": {
      "type": "string",
      "description": "The stream identifier (e.g., 'air-quality', 'outdoor-weather')",
      "pattern": "^[a-z][a-z0-9-]*$",
      "minLength": 1,
      "maxLength": 64
    },
    "mode": {
      "type": "string",
      "enum": ["all", "source", "target"],
      "description": "Schema view mode",
      "default": "all"
    }
  },
  "required": ["stream_id"],
  "additionalProperties": false
}
```

**Validation Rules:**
- `stream_id`: kebab-case format, lowercase letters, digits, hyphens
- `mode`: Must be one of: all, source, target

### validate_config

```json
{
  "type": "object",
  "properties": {
    "stream_id": {
      "type": "string",
      "description": "The stream identifier to validate",
      "pattern": "^[a-z][a-z0-9-]*$",
      "minLength": 1,
      "maxLength": 64
    }
  },
  "required": ["stream_id"],
  "additionalProperties": false
}
```

### sample_data

```json
{
  "type": "object",
  "properties": {
    "stream_id": {
      "type": "string",
      "description": "The stream identifier",
      "pattern": "^[a-z][a-z0-9-]*$",
      "minLength": 1,
      "maxLength": 64
    },
    "n": {
      "type": "integer",
      "description": "Number of rows to return",
      "default": 10,
      "minimum": 1,
      "maximum": 100
    }
  },
  "required": ["stream_id"],
  "additionalProperties": false
}
```

---

## Tool Output Structures

### list_streams Response

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
      "storage": {
        "latest_partition": "year=2026/month=01/day=03",
        "file_size_bytes": 12480,
        "file_modified": "2026-01-03T15:00:00Z"
      }
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

**Field Definitions:**

| Field | Type | Description |
|-------|------|-------------|
| `stream_id` | string | Unique stream identifier |
| `description` | string | Human-readable description from config |
| `enabled` | boolean | Whether stream is currently active |
| `version` | string | Semantic version from config |
| `sources` | string[] | Source types (mqtt, http_poll, etc.) |
| `storage` | object\|null | Storage metadata or null if no data |
| `storage.latest_partition` | string | Most recent partition path |
| `storage.file_size_bytes` | integer | Size of data.parquet file |
| `storage.file_modified` | string | ISO 8601 modification timestamp |

### describe_schema Response (mode: source)

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
      "coord": ["lat", "lon"],
      "sys": ["country", "id", "sunrise", "sunset", "type"],
      "clouds": ["all"]
    }
  },
  "parser_type": "json_path",
  "field_mappings": [
    {
      "source_path": "main.temp",
      "target_field": "temperature",
      "unit": "celsius"
    },
    {
      "source_path": "main.feels_like",
      "target_field": "feels_like",
      "unit": "celsius"
    },
    {
      "source_path": "main.pressure",
      "target_field": "pressure",
      "unit": "hpa"
    },
    {
      "source_path": "main.humidity",
      "target_field": "humidity",
      "unit": "percent"
    },
    {
      "source_path": "wind.speed",
      "target_field": "wind_speed",
      "unit": "m/s"
    },
    {
      "source_path": "wind.deg",
      "target_field": "wind_deg",
      "unit": "degrees"
    },
    {
      "source_path": "wind.gust",
      "target_field": "wind_gust",
      "unit": "m/s"
    },
    {
      "source_path": "clouds.all",
      "target_field": "clouds",
      "unit": "percent"
    },
    {
      "source_path": "visibility",
      "target_field": "visibility",
      "unit": "meters"
    }
  ],
  "unmapped_source_fields": ["base", "cod", "coord", "dt", "id", "name", "sys", "timezone", "weather"],
  "file_analyzed": "/data/raw/outdoor-weather/year=2026/month=01/day=03/data.parquet"
}
```

### describe_schema Response (mode: target)

```json
{
  "success": true,
  "stream_id": "outdoor-weather",
  "mode": "target",
  "entity_schema": "nws-weather",
  "attributes": [
    {
      "name": "temperature",
      "type": "float",
      "unit": "celsius",
      "nullable": false,
      "description": "Current temperature",
      "range": [-50, 60]
    },
    {
      "name": "feels_like",
      "type": "float",
      "unit": "celsius",
      "nullable": true,
      "description": "Feels-like temperature",
      "range": [-50, 60]
    },
    {
      "name": "pressure",
      "type": "float",
      "unit": "hpa",
      "nullable": true,
      "description": "Atmospheric pressure at sea level",
      "range": [800, 1200]
    },
    {
      "name": "humidity",
      "type": "float",
      "unit": "percent",
      "nullable": true,
      "description": "Relative humidity",
      "range": [0, 100]
    }
  ]
}
```

### describe_schema Response (mode: all)

```json
{
  "success": true,
  "stream_id": "outdoor-weather",
  "mode": "all",
  "source": {
    "raw_payload_structure": {
      "keys": ["base", "clouds", "cod", "coord", "dt", "id", "main", "name", "sys", "timezone", "visibility", "weather", "wind"],
      "nested": {
        "main": ["feels_like", "grnd_level", "humidity", "pressure", "sea_level", "temp", "temp_max", "temp_min"],
        "wind": ["deg", "gust", "speed"]
      }
    },
    "parser_type": "json_path",
    "field_mappings": [
      {"source_path": "main.temp", "target_field": "temperature", "unit": "celsius"}
    ]
  },
  "target": {
    "entity_schema": "nws-weather",
    "attributes": [
      {"name": "temperature", "type": "float", "unit": "celsius", "nullable": false}
    ]
  },
  "gap_analysis": {
    "unmapped_source_fields": ["base", "cod", "coord", "dt", "id", "name", "sys", "timezone", "weather"],
    "target_fields_without_mapping": ["rain_1h", "snow_1h"]
  },
  "file_analyzed": "/data/raw/outdoor-weather/year=2026/month=01/day=03/data.parquet"
}
```

### validate_config Response

```json
{
  "success": true,
  "stream_id": "outdoor-weather",
  "entity_schema": "nws-weather",
  "validation": {
    "status": "mismatch",
    "config_fields": ["temperature", "feels_like", "pressure", "humidity", "wind_speed", "wind_deg", "wind_gust", "clouds", "visibility", "rain_1h", "snow_1h"],
    "raw_payload_fields": ["base", "clouds", "cod", "coord", "dt", "id", "main", "name", "sys", "timezone", "visibility", "weather", "wind"],
    "analysis": {
      "in_config_not_in_payload": ["temperature", "feels_like", "pressure", "humidity", "wind_speed", "wind_deg", "wind_gust", "rain_1h", "snow_1h"],
      "in_payload_not_in_config": ["base", "cod", "coord", "dt", "id", "main", "name", "sys", "timezone", "weather", "wind"],
      "matching": ["clouds", "visibility"]
    },
    "notes": "Config uses flattened field names; raw_payload preserves source structure (main.temp, wind.speed). Mapping happens in Silver layer via parser field_mappings."
  }
}
```

**Validation Status Values:**

| Status | Meaning |
|--------|---------|
| `match` | All config fields found in raw_payload (after applying mappings) |
| `partial` | Some config fields found, some missing |
| `mismatch` | Significant differences between config and payload |

### sample_data Response

```json
{
  "success": true,
  "stream_id": "outdoor-weather",
  "row_count": 3,
  "rows": [
    {
      "timestamp": 1767452639760716,
      "source_id": "outdoor-weather-Http",
      "ndp_id": "weather-owm-002",
      "context": {
        "source_type": {
          "provider": "openweathermap",
          "purpose": "current_weather"
        },
        "location": {
          "coordinates": [29.95838, -81.30878],
          "type": "outdoor",
          "path": "beachhouse"
        }
      },
      "raw_payload": {
        "coord": {"lon": -81.3088, "lat": 29.9584},
        "weather": [{"id": 803, "main": "Clouds", "description": "broken clouds", "icon": "04n"}],
        "base": "stations",
        "main": {
          "temp": 19.72,
          "feels_like": 19.78,
          "temp_min": 18.29,
          "temp_max": 21.15,
          "pressure": 1020,
          "humidity": 76,
          "sea_level": 1020,
          "grnd_level": 1019
        },
        "visibility": 10000,
        "wind": {"speed": 5.66, "deg": 220, "gust": 8.23},
        "clouds": {"all": 75},
        "dt": 1767452400,
        "sys": {"type": 2, "id": 2010624, "country": "US", "sunrise": 1767435600, "sunset": 1767472800},
        "timezone": -18000,
        "id": 4151440,
        "name": "Crescent Beach",
        "cod": 200
      }
    }
  ],
  "source_file": "/data/raw/outdoor-weather/year=2026/month=01/day=03/data.parquet"
}
```

---

## Error Response Structures

### Tool-Level Errors

Returned via normal MCP response with `isError: true`:

```json
{
  "content": [
    {
      "type": "text",
      "text": "{\"success\": false, \"error\": \"Stream not found: invalid-stream\", \"code\": \"STREAM_NOT_FOUND\"}"
    }
  ],
  "isError": true
}
```

### Error Code Reference

| Code | HTTP Equivalent | Description | Example Trigger |
|------|-----------------|-------------|-----------------|
| `STREAM_NOT_FOUND` | 404 | Stream ID does not exist in etcd | `describe_schema("nonexistent")` |
| `ETCD_UNAVAILABLE` | 503 | Cannot connect to etcd cluster | etcd container down |
| `NO_DATA_AVAILABLE` | 404 | Stream exists but no Parquet files | New stream, not yet ingested |
| `INVALID_PARAMETER` | 400 | Parameter validation failed | `sample_data("stream", n=200)` |
| `PARSE_ERROR` | 500 | Failed to parse Parquet/JSON | Corrupted file |
| `INTERNAL_ERROR` | 500 | Unexpected server error | Panic recovery |
| `UNKNOWN_TOOL` | 400 | Tool name not registered | `tools/call("bad_tool")` |

### Error Response Examples

**Stream Not Found:**
```json
{
  "success": false,
  "error": "Stream not found: invalid-stream-id",
  "code": "STREAM_NOT_FOUND",
  "details": {
    "stream_id": "invalid-stream-id",
    "available_streams": ["air-quality", "outdoor-weather", "nws-observations"]
  }
}
```

**Parameter Validation Error:**
```json
{
  "success": false,
  "error": "Parameter 'n' exceeds maximum value of 100",
  "code": "INVALID_PARAMETER",
  "details": {
    "parameter": "n",
    "value": 200,
    "constraint": "maximum: 100"
  }
}
```

**No Data Available:**
```json
{
  "success": false,
  "error": "No Parquet data available for stream: nws-forecast-hourly",
  "code": "NO_DATA_AVAILABLE",
  "details": {
    "stream_id": "nws-forecast-hourly",
    "data_path": "/data/raw/nws-forecast-hourly",
    "suggestion": "Stream may be disabled or not yet ingesting data"
  }
}
```

**etcd Unavailable:**
```json
{
  "success": false,
  "error": "Failed to connect to etcd cluster",
  "code": "ETCD_UNAVAILABLE",
  "details": {
    "endpoints": ["http://localhost:2379"],
    "timeout_ms": 5000
  }
}
```

---

## JSON-RPC Error Codes

Standard JSON-RPC 2.0 error codes for protocol-level errors:

| Code | Message | Meaning |
|------|---------|---------|
| -32700 | Parse error | Invalid JSON |
| -32600 | Invalid Request | Missing required fields |
| -32601 | Method not found | Unknown MCP method |
| -32602 | Invalid params | Malformed params object |
| -32603 | Internal error | Server error |

---

## Client Configuration

### Claude Code MCP Configuration

Add to `.claude/mcp.json`:

```json
{
  "mcpServers": {
    "ndp-bronze": {
      "type": "http",
      "url": "http://${NDP_PI_HOST}:9100/mcp",
      "description": "NDP Bronze layer data exploration and validation"
    }
  }
}
```

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `NDP_PI_HOST` | - | Pi hostname or IP address |

---

## Content Types

### Request Content-Type
```
Content-Type: application/json
```

### Response Content-Type
```
Content-Type: application/json
```

### Character Encoding
All text is UTF-8 encoded.

---

## Rate Limiting (Future)

Not implemented for MVP. Design considerations:
- Per-client rate limiting
- Burst allowance for tool discovery
- 429 Too Many Requests response

---

## References

- [MCP Specification](https://modelcontextprotocol.io/specification/2025-11-25)
- [JSON-RPC 2.0](https://www.jsonrpc.org/specification)
- [JSON Schema](https://json-schema.org/)

---

*This document is part of the SPARC Specification phase for DP-005.*
