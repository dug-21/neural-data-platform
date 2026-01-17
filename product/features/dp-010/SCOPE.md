# DP-010: Extend MCP to Silver Layer & Data Dictionary

**Feature ID**: dp-010
**Title**: NDP MCP Server - Silver Layer & Data Dictionary Access
**Status**: Scope Definition
**Created**: 2026-01-16
**Depends On**: dp-005 (Bronze MCP), dp-006 (Silver Layer), dp-009 (Silver Data Dictionary ✅), dp-011 (ETL Statistics ✅)

---

## Executive Summary

Extend the existing `ndp-bronze` MCP server to become a comprehensive **NDP MCP server** that provides agents with unified access to:
- Silver layer data (tables, schemas, sampling)
- Data dictionary (Bronze + Silver metadata, lineage)
- Platform fundamentals for troubleshooting/validation

**Purpose**: Enable data analytics agents to explore the platform, understand data definitions, and perform troubleshooting during platform development - all via config-driven, queryable interfaces.

---

## Current State

### Existing MCP Tools (dp-005)

| Tool | Scope | Description |
|------|-------|-------------|
| `list_streams` | Bronze | List Bronze streams with metadata |
| `describe_schema` | Bronze | Get raw_payload structure, field mappings, entity_schemas |
| `validate_config` | Bronze | Compare etcd config vs Parquet |
| `sample_data` | Bronze | Sample rows from Bronze Parquet |

### Gap

| Need | Current Support |
|------|-----------------|
| Silver table discovery | ❌ None |
| Silver schema inspection | ❌ None |
| Silver data sampling | ❌ None |
| Data dictionary queries | ❌ None |
| Bronze→Silver lineage | ❌ None |
| DQ rule inspection | ❌ None |

---

## Objectives

1. **Silver layer visibility** - Agents can discover and inspect Silver tables
2. **Data dictionary access** - Unified queries across Bronze + Silver definitions
3. **Data sampling** - Sample Silver data for validation/troubleshooting
4. **Lineage queries** - Trace data from Bronze to Silver
5. **Platform diagnostics** - DQ rules, ETL status, config validation

---

## Scope

### In Scope

#### 1. New Silver Layer Tools

| Tool | Description | Input | Output |
|------|-------------|-------|--------|
| `list_silver_tables` | List all Silver hypertables | none | Table names, row counts, chunk info |
| `describe_silver_table` | Get Silver table schema | `table_name` | Columns, types, units, descriptions |
| `sample_silver_data` | Sample rows from Silver | `table_name`, `n`, `filters?` | JSON rows |
| `silver_stats` | Table statistics | `table_name` | Row count, time range, null counts |

#### 2. Data Dictionary Tools

| Tool | Description | Input | Output |
|------|-------------|-------|--------|
| `query_dictionary` | Search data dictionary | `query`, `layer?` | Matching entries (columns, tables) |
| `describe_column` | Full column details | `table`, `column` | Type, unit, source, DQ rules |
| `trace_lineage` | Bronze→Silver mapping | `silver_column` | Source stream, source_path, transforms |
| `list_dq_rules` | DQ rules for table/column | `table?`, `column?` | Rule definitions with params |

#### 3. Platform Diagnostics Tools

| Tool | Description | Input | Output |
|------|-------------|-------|--------|
| `etl_status` | Silver ETL run status | `stream_id?` | Last run, rows processed, errors |
| `validate_silver` | Config vs Silver schema | `table_name` | Mismatches, missing columns |
| `data_freshness` | Latest data timestamps | `layer?` | Per-stream/table freshness |

#### 4. Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                     NDP MCP Server (Extended)                        │
├─────────────────────────────────────────────────────────────────────┤
│  Bronze Tools (existing)       │  Silver Tools (new)                │
│  ├── list_streams              │  ├── list_silver_tables           │
│  ├── describe_schema           │  ├── describe_silver_table        │
│  ├── validate_config           │  ├── sample_silver_data           │
│  └── sample_data               │  └── silver_stats                 │
├─────────────────────────────────────────────────────────────────────┤
│  Dictionary Tools (new)        │  Diagnostics Tools (new)          │
│  ├── query_dictionary          │  ├── etl_status                   │
│  ├── describe_column           │  ├── validate_silver              │
│  ├── trace_lineage             │  └── data_freshness               │
│  └── list_dq_rules             │                                    │
├─────────────────────────────────────────────────────────────────────┤
│  Storage Adapters                                                    │
│  ├── BronzeStorage (existing)  → /data/raw/{stream}/               │
│  ├── SilverStorage (new)       → TimescaleDB silver.*              │
│  └── DictionaryStore (new)     → PostgreSQL data_dictionary.*      │
└─────────────────────────────────────────────────────────────────────┘
```

#### 5. Example Tool Outputs

**`list_silver_tables`**:
```json
{
  "success": true,
  "tables": [
    {
      "table_name": "air_quality_observations",
      "schema": "silver",
      "description": "Indoor AQ from AirGradient sensors",
      "is_hypertable": true,
      "chunk_interval": "1 day",
      "row_count": 142857,
      "time_range": {
        "min": "2026-01-01T00:00:00Z",
        "max": "2026-01-16T14:00:00Z"
      }
    }
  ]
}
```

**`describe_silver_table`**:
```json
{
  "success": true,
  "table_name": "air_quality_observations",
  "columns": [
    {"name": "observation_time", "type": "TIMESTAMPTZ", "nullable": false, "description": "Measurement timestamp"},
    {"name": "pm25", "type": "DOUBLE PRECISION", "unit": "ug/m3", "nullable": true, "description": "PM2.5 concentration"},
    {"name": "dq_flags", "type": "TEXT[]", "nullable": true, "description": "Data quality flags"}
  ],
  "primary_key": ["observation_time", "ndp_id"],
  "source_streams": ["air-quality"]
}
```

**`trace_lineage`**:
```json
{
  "success": true,
  "silver_table": "air_quality_observations",
  "silver_column": "pm25",
  "lineage": {
    "source_stream": "air-quality",
    "source_path": "raw_payload.pm02",
    "transformation": "direct",
    "dq_rules": [
      {"rule": "range_check", "min": 0, "max": 1000, "action": "flag"}
    ]
  }
}
```

**`query_dictionary`** (search: "temperature"):
```json
{
  "success": true,
  "query": "temperature",
  "results": [
    {"layer": "silver", "table": "air_quality_observations", "column": "temperature_c", "unit": "Celsius"},
    {"layer": "silver", "table": "weather_observations", "column": "temperature_c", "unit": "Celsius"},
    {"layer": "bronze", "stream": "outdoor-weather", "path": "raw_payload.main.temp", "unit": "Kelvin"}
  ]
}
```

#### 6. Storage Adapters

**SilverStorage** (new trait):
```rust
pub trait SilverStorage: Send + Sync {
    async fn list_tables(&self) -> Result<Vec<SilverTableInfo>>;
    async fn describe_table(&self, name: &str) -> Result<SilverTableSchema>;
    async fn sample(&self, name: &str, limit: usize, filters: Option<Filters>) -> Result<Vec<Row>>;
    async fn stats(&self, name: &str) -> Result<TableStats>;
}
```

**DictionaryStore** (new trait):
```rust
pub trait DictionaryStore: Send + Sync {
    async fn search(&self, query: &str, layer: Option<&str>) -> Result<Vec<DictEntry>>;
    async fn column_details(&self, table: &str, column: &str) -> Result<ColumnDetails>;
    async fn lineage(&self, silver_table: &str, silver_column: &str) -> Result<Lineage>;
    async fn dq_rules(&self, table: Option<&str>, column: Option<&str>) -> Result<Vec<DqRule>>;
}
```

#### 7. Configuration

```bash
# Existing
NDP_MCP_LISTEN=0.0.0.0:9100
NDP_ETCD_ENDPOINTS=http://localhost:2379
NDP_RAW_PATH=/data/raw

# New for Silver/Dictionary
NDP_TIMESCALE_URL=postgresql://user:pass@localhost:5432/ndp
NDP_DICTIONARY_SCHEMA=data_dictionary
NDP_SILVER_SCHEMA=silver
```

---

### Out of Scope

| Item | Reason | Target |
|------|--------|--------|
| Write operations | Read-only by design | N/A |
| Arbitrary SQL execution | Security concern | Future with safeguards |
| Gold layer tools | Not implemented yet | dp-011+ |
| Real-time streaming | Batch sampling sufficient | Future |
| Authentication | Design now, implement later | Future |

---

## Success Criteria

| Criterion | Validation |
|-----------|------------|
| `list_silver_tables` works | Returns 4 Silver tables with metadata |
| `describe_silver_table` works | Returns columns with types, units |
| `sample_silver_data` works | Returns N rows as JSON |
| `query_dictionary` works | Searches across Bronze + Silver |
| `trace_lineage` works | Shows Bronze→Silver path |
| Tools appear in Claude Code | `tools/list` includes all new tools |
| Memory footprint | < 75MB (up from 50MB baseline) |

---

## Use Cases

### 1. Data Analytics Agent Workflow

```
Agent: "What Silver data is available?"
→ list_silver_tables → Shows 4 tables with row counts

Agent: "What columns are in weather_observations?"
→ describe_silver_table("weather_observations") → Full schema

Agent: "Show me recent weather data"
→ sample_silver_data("weather_observations", n=5) → JSON rows
```

### 2. Troubleshooting/Validation

```
Agent: "Why is pm25 NULL for some rows?"
→ trace_lineage("air_quality_observations", "pm25") → Shows source + DQ rules
→ list_dq_rules("air_quality_observations", "pm25") → range_check flags values > 1000

Agent: "Is Silver in sync with config?"
→ validate_silver("weather_observations") → Reports any mismatches
```

### 3. Platform Discovery

```
Agent: "What columns have temperature data?"
→ query_dictionary("temperature") → All matching Bronze + Silver columns

Agent: "What's the data freshness?"
→ data_freshness() → Latest timestamps per stream/table
```

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| dp-005 Bronze MCP | ✅ Complete | Base server to extend |
| dp-006 Silver Layer | ✅ Complete | 4 hypertables operational |
| dp-009 Silver Dictionary | ✅ Complete | `data_dictionary.silver_tables/columns/lineage/dq_rules` operational |
| dp-011 ETL Statistics | ✅ Complete | `silver.etl_runs` table operational |
| TimescaleDB | ✅ Deployed | Connection required |
| tokio-postgres | ✅ Available | Async PostgreSQL client |

---

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Query performance on large tables | Medium | Low | Add LIMIT, use indexes |
| Dictionary not populated | ~~Medium~~ **Resolved** | ~~Medium~~ | dp-009 complete - tables populated via sync |
| Connection pool exhaustion | Low | Medium | Configure pool limits |
| TimescaleDB API changes | Low | Medium | Use stable `timescaledb_information` views |

---

## Deliverables

| Deliverable | Description |
|-------------|-------------|
| `silver_storage.rs` | SilverStorage trait + TimescaleDB impl |
| `dictionary_store.rs` | DictionaryStore trait + PostgreSQL impl |
| Silver tools (`list_silver_tables`, etc.) | 4 new tools |
| Dictionary tools (`query_dictionary`, etc.) | 4 new tools |
| Diagnostics tools (`etl_status`, etc.) | 3 new tools |
| Updated CLAUDE.md | Document new tools for agents |

---

## Implementation Phases

### Phase 1: Silver Access (MVP)
- `list_silver_tables`
- `describe_silver_table`
- `sample_silver_data`

### Phase 2: Data Dictionary
- `query_dictionary`
- `describe_column`
- `trace_lineage`
- `list_dq_rules`

### Phase 3: Diagnostics
- `etl_status`
- `validate_silver`
- `data_freshness`

---

## References

- [dp-005 SCOPE](../dp-005/SCOPE.md) - Bronze MCP Server
- [dp-006 SCOPE](../dp-006/SCOPE.md) - Silver Layer
- [dp-009 SCOPE](../dp-009/SCOPE.md) - Silver Data Dictionary
- [MCP Specification](https://modelcontextprotocol.io/specification/2025-11-25)

---

*Scope defined: 2026-01-16*
