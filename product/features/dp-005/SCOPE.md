# dp-005: Bronze MCP Server

## Summary

Build a Rust-based MCP (Model Context Protocol) server that exposes Bronze layer data and configuration validation tools to development agents. The server runs on the Pi (edge) but is designed for cloud portability.

## Problem Statement

NDP is a **config-driven** data platform. Configuration defines the expected data schema, sources, and behavior. The actual data flowing through Bronze should conform to this configuration.

Development agents (Claude Code on Mac) need to explore Bronze layer data structure and validate that incoming data matches configuration. Currently, there's no standardized way for agents to:

1. Discover what streams exist in Bronze
2. Understand the exact schema of Parquet files
3. Validate that config (in etcd) matches actual data structure
4. Sample data for exploration and ETL development

## Goals

1. **Enable agent data exploration** - Agents can ask "What's the structure of indoor-air?" and get accurate answers
2. **Validate config↔data alignment** - Detect mismatches between etcd config and Bronze reality
3. **Validate config pipeline** - By reading from etcd (not source YAML), we validate the full sync pipeline: `source YAML → config-client → etcd → MCP`
4. **Cloud-portable design** - Same server works on Pi today, cloud tomorrow
5. **Minimal footprint** - Rust for CPU/memory efficiency on edge
6. **Standards-compliant** - Follow MCP specification for broad tooling compatibility

## Non-Goals (MVP)

- Query execution (SELECT statements) - Phase 2
- Silver layer access - Phase 2
- Non-deterministic agent spawning on Pi - Phase 2+
- Authentication/authorization - Design now, implement when needed
- Write operations - Read-only by design
- **Type validation** - Bronze stores raw JSON; type parsing is complex, defer to Phase 2
- **Value constraint validation** - Defer to Phase 2

---

## Data Landscape

### Stream Configuration (etcd)

Config files live in `config/base/streams/{stream_id}/config.yaml` and are synced to etcd via `scripts/sync-config-to-etcd.sh`. The YAML is flattened with `/` separators.

**etcd key pattern**: `/streams/{stream_id}/{path}`

Example keys for `air-quality`:
```
/streams/air-quality/stream_id          → "air-quality"
/streams/air-quality/enabled            → true
/streams/air-quality/fields/pm25/type   → "float"
/streams/air-quality/fields/pm25/unit   → "µg/m³"
/streams/air-quality/entity_schemas/0/schema_name → "airgradient"
```

**Key config sections**:
| Section | Purpose | Used By |
|---------|---------|---------|
| `stream_id`, `description`, `version` | Stream metadata | `list_streams` |
| `sources[].parser.field_mappings` | Source→target field transforms | `describe_schema(source)` |
| `entity_schemas` | Target schema (data dictionary) | `describe_schema(target)` |
| `sources` | Data source configuration | `list_streams` |
| `enabled` | Whether stream is active | `list_streams` |

**Decision: Multiple sources of truth, each authoritative for its domain.**

| Domain | Source of Truth | Config Location |
|--------|-----------------|-----------------|
| **Bronze structure** | Parquet file | Introspected from `/data/raw/` |
| **Field mappings** | Parser config | `sources[].parser.field_mappings` |
| **Silver/Target schema** | Entity schemas | `entity_schemas[].attributes` |

**Rationale**:
- Bronze = actual data (introspection ensures accuracy)
- Mappings = how to transform (parser config)
- Silver = what we want (data dictionary alignment)

The `fields` section exists in some configs but has inconsistent formats (dict vs array). **Skip it** - use `entity_schemas` for target schema.

**Note**: Config schema may evolve. MCP server should be resilient to missing/extra keys.

### Bronze File Organization

Hive-style partitioning with single file per partition:

```
/data/raw/{stream_id}/
└── year=YYYY/
    └── month=MM/
        └── day=DD/
            └── data.parquet
```

Example: `/data/raw/outdoor-weather/year=2026/month=01/day=03/data.parquet`

**Key characteristics**:
- One `data.parquet` file per day partition
- Daily partitions by default
- File grows throughout the day, finalized at day boundary

### What Bronze Contains

Bronze stores a **standard envelope** with the raw payload inside. Schema defined in `core/src/types/raw_data_point.rs`:

| Column | Type | Description |
|--------|------|-------------|
| `timestamp` | INT64 (ms) | Ingestion timestamp |
| `source_id` | STRING | Source identifier (e.g., "outdoor-weather-Http") |
| `ndp_id` | STRING? | Platform-assigned stable identifier |
| `context` | JSON | Config-derived metadata snapshot |
| `raw_payload` | JSON | **Exact payload from source, untransformed** |
| `day`, `month`, `year` | INT | Partition columns (added by storage layer) |

**Key insight**: The domain fields (temperature, humidity, pm25) are **inside** `raw_payload` as JSON, not as separate Parquet columns. Field extraction happens in Silver layer.

### Schema Discovery Strategy

**The MCP server uses Parquet introspection** - reads schema directly from the data file:

```rust
// Dynamic schema discovery - no hardcoded expectations
let reader = ParquetReader::new(file);
let schema = reader.schema();  // Always matches reality
```

This ensures:
- No config/code synchronization issues
- Automatic adaptation if Bronze schema evolves
- Single source of truth = the actual data

### Validation Implications

Since domain fields are inside `raw_payload` JSON, `validate_config` has two modes:

1. **Envelope validation**: Verify Bronze envelope structure exists (timestamp, source_id, raw_payload)
2. **Payload validation**: Parse `raw_payload` JSON and compare keys against `entity_schemas.attributes`

MVP focuses on **payload validation** - comparing what's inside `raw_payload` against config expectations.

---

## MVP Tool Set

| Tool | Purpose | Input | Output |
|------|---------|-------|--------|
| `list_streams` | Enumerate available Bronze streams | none | Stream IDs, file counts, date ranges |
| `describe_schema` | Get schema info for a stream | `stream_id: string`, `mode: all\|source\|target` | Schema details based on mode |
| `validate_config` | Compare etcd config vs Bronze schema | `stream_id: string` | Pretty JSON diff: config fields vs Parquet fields |
| `sample_data` | Get N recent rows from a stream | `stream_id: string`, `n: int` | JSON array of rows |

### Example Tool Outputs

**`list_streams`** response:
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
        "file_size_bytes": 7310,
        "file_modified": "2026-01-03T14:54:00Z"
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

**Notes**:
- `enabled` flag from etcd config
- `storage` from filesystem scan (null if no data exists)
- File is always `data.parquet` (one per day partition)
- No actual data rows in response, just metadata

**`describe_schema`** - modes and responses:

**Mode: `source`** - What comes in and how does it transform?
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
    {"source_path": "main.feels_like", "target_field": "feels_like", "unit": "celsius"},
    {"source_path": "main.pressure", "target_field": "pressure", "unit": "hpa"},
    {"source_path": "main.humidity", "target_field": "humidity", "unit": "percent"},
    {"source_path": "wind.speed", "target_field": "wind_speed", "unit": "m/s"},
    {"source_path": "wind.deg", "target_field": "wind_deg", "unit": "degrees"},
    {"source_path": "wind.gust", "target_field": "wind_gust", "unit": "m/s"},
    {"source_path": "clouds.all", "target_field": "clouds", "unit": "percent"},
    {"source_path": "visibility", "target_field": "visibility", "unit": "meters"}
  ],
  "unmapped_source_fields": ["base", "cod", "coord", "dt", "id", "name", "sys", "timezone", "weather"],
  "file_analyzed": "/data/raw/outdoor-weather/year=2026/month=01/day=03/data.parquet"
}
```

**Mode: `target`** - What's the target schema?
```json
{
  "success": true,
  "stream_id": "outdoor-weather",
  "mode": "target",
  "entity_schema": "nws-weather",
  "attributes": [
    {"name": "temperature", "type": "float", "unit": "celsius", "nullable": false},
    {"name": "feels_like", "type": "float", "unit": "celsius", "nullable": true},
    {"name": "pressure", "type": "float", "unit": "hpa", "nullable": true},
    {"name": "humidity", "type": "float", "unit": "percent", "nullable": true},
    {"name": "wind_speed", "type": "float", "unit": "m/s", "nullable": true},
    {"name": "wind_deg", "type": "float", "unit": "degrees", "nullable": true},
    {"name": "wind_gust", "type": "float", "unit": "m/s", "nullable": true},
    {"name": "clouds", "type": "float", "unit": "percent", "nullable": true},
    {"name": "visibility", "type": "float", "unit": "meters", "nullable": true}
  ]
}
```

**Mode: `all`** (default) - Complete ETL picture
```json
{
  "success": true,
  "stream_id": "outdoor-weather",
  "mode": "all",
  "source": {
    "raw_payload_structure": { /* keys and nested structure */ },
    "field_mappings": [ /* source_path → target_field */ ]
  },
  "target": {
    "entity_schema": "nws-weather",
    "attributes": [ /* name, type, unit, nullable */ ]
  },
  "gap_analysis": {
    "unmapped_source_fields": ["base", "cod", "coord", "dt", "id", "name", "sys", "timezone", "weather"],
    "target_fields_without_mapping": ["rain_1h", "snow_1h"]
  }
}
```

**`validate_config`** response (pretty JSON diff):
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
    "notes": "Config uses flattened field names; raw_payload preserves source structure (main.temp, wind.speed). Mapping happens in Silver layer."
  }
}
```

**Notes**:
- `config_fields` extracted from `entity_schemas[].attributes[].name` in etcd
- `raw_payload_fields` from parsing `raw_payload` JSON in Parquet
- Mismatch is expected: config defines **target** schema, raw_payload has **source** structure
- This validation reveals where parsing/mapping is needed for Silver ETL

**`sample_data`** response:
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
      "context": {"location": {"coordinates": [29.95838, -81.30878], "path": "beachhouse", "type": "outdoor"}},
      "raw_payload": {"main": {"temp": 19.72, "humidity": 76}, "wind": {"speed": 5.66, "deg": 220}}
    },
    {
      "timestamp": 1767452039777563,
      "source_id": "outdoor-weather-Http",
      "ndp_id": "weather-owm-002",
      "context": {"location": {"coordinates": [29.95838, -81.30878], "path": "beachhouse", "type": "outdoor"}},
      "raw_payload": {"main": {"temp": 18.87, "humidity": 77}, "wind": {"speed": 0.45, "deg": 271}}
    }
  ],
  "source_file": "/data/raw/outdoor-weather/year=2026/month=01/day=03/data.parquet"
}
```

**Notes**:
- Returns full Bronze envelope structure
- `raw_payload` truncated for readability (actual contains full source JSON)
- Useful for agents to understand actual data structure for ETL development

### Tool Definitions (MCP Format)

```json
{
  "name": "list_streams",
  "description": "List all available Bronze layer streams with metadata",
  "inputSchema": {
    "type": "object",
    "properties": {},
    "required": []
  }
}
```

```json
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
}
```

```json
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
}
```

```json
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
        "default": 10
      }
    },
    "required": ["stream_id"]
  }
}
```

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                          MAC (Development)                               │
│                                                                          │
│   Claude Code ──► MCP Client ──► HTTP ──► Pi MCP Server                 │
│                                                                          │
│   .claude/mcp.json:                                                      │
│   { "ndp-bronze": { "type": "http", "url": "http://pi:9100/mcp" } }    │
└─────────────────────────────────────────────────────────────────────────┘
                                     │
                                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                          PI (Production) - ndp-mcp-server               │
│                                                                          │
│   ┌─────────────────────────────────────────────────────────────────┐  │
│   │  HTTP Layer (axum)                                               │  │
│   │  POST /mcp ──► JSON-RPC Router                                  │  │
│   │  GET /health ──► Health check                                   │  │
│   └─────────────────────────────────────────────────────────────────┘  │
│                                     │                                    │
│   ┌─────────────────────────────────────────────────────────────────┐  │
│   │  MCP Protocol Handler                                            │  │
│   │  tools/list ──► Tool definitions                                │  │
│   │  tools/call ──► Route to implementation                         │  │
│   └─────────────────────────────────────────────────────────────────┘  │
│                                     │                                    │
│   ┌─────────────────────────────────────────────────────────────────┐  │
│   │  Tool Implementations                                            │  │
│   │  list_streams ────► BronzeStorage.list()                        │  │
│   │  describe_schema ──► BronzeStorage.schema()                     │  │
│   │  validate_config ──► ConfigStore.get() + BronzeStorage.schema() │  │
│   │  sample_data ─────► BronzeStorage.sample()                      │  │
│   └─────────────────────────────────────────────────────────────────┘  │
│                          │                    │                          │
│   ┌──────────────────────┴─┐  ┌──────────────┴──────────────────────┐  │
│   │  ConfigStore (etcd)    │  │  BronzeStorage (trait)              │  │
│   │  - Read stream configs │  │  - LocalParquetStorage (today)      │  │
│   │  - Validate sync state │  │  - S3ParquetStorage (tomorrow)      │  │
│   └────────────────────────┘  └─────────────────────────────────────┘  │
│                                                                          │
│   Data: /data/raw/{stream_id}/{year}/{month}/{day}/*.parquet            │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Client Integration

### MCP Client Configuration

Add to `.claude/mcp.json` on development machine:

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

Environment variable `NDP_PI_HOST` should resolve to Pi hostname/IP.

### CLAUDE.md Integration

Add to project CLAUDE.md for agent guidance:

```markdown
## NDP Data Exploration (MCP)

The `ndp-bronze` MCP server provides tools for exploring Bronze layer data and validating configuration.

### Available Tools

| Tool | When to Use |
|------|-------------|
| `list_streams` | Discover available data streams and their status |
| `describe_schema(stream, mode)` | Understand data structure for ETL development |
| `validate_config(stream)` | Check if config matches actual data |
| `sample_data(stream, n)` | See actual records for debugging/exploration |

### describe_schema Modes

| Mode | Use When |
|------|----------|
| `source` | Building ETL - need to see raw data + mappings |
| `target` | Defining Silver schema - need entity_schemas |
| `all` | Complete picture - gap analysis for missing mappings |

### Example Workflows

**"What data do we have?"**
→ `list_streams` → shows all streams with enabled status and latest data

**"Help me build ETL for outdoor-weather"**
→ `describe_schema("outdoor-weather", mode="source")` → raw structure + existing mappings
→ `describe_schema("outdoor-weather", mode="target")` → what Silver expects
→ Identify gaps, write transformation code

**"Why is temperature missing in Silver?"**
→ `describe_schema("outdoor-weather", mode="all")` → gap_analysis shows unmapped fields
→ `sample_data("outdoor-weather", 5)` → verify raw data has the field
→ Check if mapping exists in parser config

**"Is config synced correctly?"**
→ `validate_config("outdoor-weather")` → compare etcd config vs actual Parquet
```

### Tool Discovery

Claude Code automatically discovers tools via MCP `tools/list` protocol. The tool definitions include:
- Name and description
- Input schema (JSON Schema format)
- Required vs optional parameters

Rich descriptions enable Claude to understand when each tool is appropriate without explicit prompting.

---

## Cloud Portability Requirements

| Aspect | Pi (Today) | Cloud (Tomorrow) |
|--------|------------|------------------|
| **Transport** | HTTP (plain) | HTTPS (TLS) |
| **Auth** | Disabled | Bearer token / OAuth |
| **Config** | etcd (local) | etcd (managed) or env vars |
| **Storage** | Local filesystem | S3/GCS via object_store crate |
| **Endpoint** | `http://pi:9100` | `https://ndp-api.example.com` |

**Design Principle**: All environment-specific values come from configuration (env vars), not code.

---

## Response Format

Following MCP specification with consistent structure:

**Success:**
```json
{
  "content": [{
    "type": "text",
    "text": "{\"success\": true, \"data\": {...}}"
  }]
}
```

**Error:**
```json
{
  "content": [{
    "type": "text",
    "text": "{\"success\": false, \"error\": \"Stream not found: invalid-stream\"}"
  }],
  "isError": true
}
```

---

## Configuration

### Environment Variables

```bash
# Server
NDP_MCP_LISTEN=0.0.0.0:9100
NDP_MCP_LOG_LEVEL=info

# etcd
NDP_ETCD_ENDPOINTS=http://localhost:2379
NDP_ETCD_PREFIX=/config/streams

# Storage (Bronze/Raw layer)
NDP_RAW_PATH=/data/raw
# Future: NDP_RAW_PATH=s3://bucket/raw

# Auth (disabled for MVP)
NDP_AUTH_ENABLED=false
# Future: NDP_AUTH_ISSUER=https://auth.example.com
```

---

## Dependencies (Rust)

```toml
[dependencies]
# HTTP
axum = "0.7"
tokio = { version = "1", features = ["full"] }
tower-http = { version = "0.5", features = ["cors", "trace"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# etcd
etcd-client = "0.14"

# Parquet
parquet = "53"
arrow = "53"

# Observability
tracing = "0.1"
tracing-subscriber = "0.3"

# Future: Cloud storage
# object_store = { version = "0.11", features = ["aws"], optional = true }
```

---

## Project Structure

```
/core/ndp-mcp-server/
├── Cargo.toml
├── src/
│   ├── main.rs                 # Entry, config loading, server start
│   ├── server.rs               # Axum routes, middleware
│   ├── mcp/
│   │   ├── mod.rs
│   │   ├── protocol.rs         # JSON-RPC types, MCP messages
│   │   ├── handler.rs          # Request routing (tools/list, tools/call)
│   │   └── tools/
│   │       ├── mod.rs          # Tool registry
│   │       ├── list_streams.rs
│   │       ├── describe_schema.rs
│   │       ├── validate_config.rs
│   │       └── sample_data.rs
│   ├── storage/
│   │   ├── mod.rs
│   │   ├── traits.rs           # BronzeStorage trait
│   │   └── local.rs            # LocalParquetStorage
│   └── config/
│       ├── mod.rs
│       └── etcd.rs             # etcd client, config types
└── tests/
    ├── integration/
    │   ├── mcp_protocol_test.rs
    │   └── tool_tests.rs
    └── fixtures/
        └── sample.parquet
```

---

## Acceptance Criteria

### Must Have (MVP)

- [ ] MCP server starts and responds to `tools/list`
- [ ] `list_streams` returns all Bronze streams with file counts
- [ ] `describe_schema` returns accurate Parquet schema for any stream
- [ ] `validate_config` compares etcd config vs Parquet schema, reports differences
- [ ] `sample_data` returns N rows as JSON
- [ ] Health endpoint (`GET /health`) returns server status
- [ ] Server runs on Pi with <50MB memory overhead
- [ ] Claude Code can connect and use tools via mcp.json config

### Should Have

- [ ] Structured logging with tracing
- [ ] Graceful shutdown
- [ ] Config validation on startup
- [ ] Error messages include actionable context

### Could Have (Post-MVP)

- [ ] Prometheus metrics endpoint
- [ ] SSE transport mode (in addition to HTTP POST)
- [ ] Tool for raw SQL queries
- [ ] Authentication layer

---

## Open Questions

### Resolved

1. ✅ **etcd schema for stream config** - Flattened YAML at `/streams/{stream_id}/*`. See "Data Landscape" section.

2. ✅ **Parquet file organization** - Hive-style: `/data/raw/{stream_id}/year=YYYY/month=MM/day=DD/*.parquet`

3. ✅ **Field mapping** - Use `entity_schemas` as source of truth. MVP compares field names only (pretty JSON), no types.

4. ✅ **Date range for samples** - Most recent N rows is adequate for MVP.

5. ✅ **MCP transport** - HTTP POST. SSE can be added later if needed.

6. ✅ **etcd unavailability** - Fail fast. Config validation should not use stale data.

7. ✅ **Config schema variations** - Use `entity_schemas` (consistent structure) as source of truth, not `fields` (inconsistent formats). Forces alignment with data dictionary.

8. ✅ **Stream discovery** - Hybrid: etcd is source of truth for stream metadata + enabled flag. Filesystem provides latest partition date and file size metadata. No data in `list_streams` response.

### Still Open

None - scope is complete pending review.

---

## Reference Materials

### MCP Specification
- https://modelcontextprotocol.io/specification/2025-11-25

### Reference Implementation (Node.js)
- https://gist.github.com/ruvnet/ea1ec6678b1552c3ff3ae92dc1001d23
- Patterns adopted:
  - Tool definition with `inputSchema` (JSON Schema)
  - Consistent response format with `success` flag
  - Health endpoint for monitoring
  - UUID-based request tracking (if needed)

### Research Context
- `/workspaces/neural-data-platform/research/agenticdataplatform/` - Prior research on agentic capabilities

---

## Related Features

- **dp-001** - TimescaleDB Silver layer (future: MCP could expose Silver too)
- **air-*** - Bronze layer data sources (what this MCP server exposes)

---

*Initial scope draft - iterating*
