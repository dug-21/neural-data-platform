# ADR-016-001: Configuration Source of Truth

**Status**: Accepted
**Date**: 2026-02-01
**Decision Makers**: Human + AI Architecture Review
**Feature**: dp-016 Configuration Architecture Review

---

## Context

The Neural Data Platform (NDP) uses configuration files to define data streams, including Bronze ingestion, Silver ETL, and data dictionary metadata. During the air-012 implementation, we discovered that different components load configuration from different sources, leading to silent failures and inconsistent behavior.

**Current State (Broken)**:
| Component | Config Source | Problem |
|-----------|---------------|---------|
| Bronze runtime | etcd (via StreamRegistry) | Works correctly |
| Silver streaming | YAML files directly | Bypasses etcd, causes air-013 |
| Silver batch | etcd first, YAML fallback | Inconsistent with streaming |
| Data dictionary sync | YAML files directly | Bypasses etcd |

**Key Constraints**:
- Platform runs on Raspberry Pi edge devices (NVMe SSD, not microSD)
- Configuration must survive power loss
- Future goal: MCP-enabled administration
- Hot-reload desired for sources (not subscribers)

---

## Decision

**JSON files are the primary source of truth. etcd serves as the runtime cache. JSON is the platform-wide configuration standard.**

All components will read configuration from etcd at runtime. JSON files are synced to etcd natively. Git provides durability and version control.

### JSON as Platform Standard

For a configuration-driven platform where agents author most config and MCP is the target admin interface, JSON provides:

| Benefit | Explanation |
|---------|-------------|
| **Agent reliability** | Strict format, no indentation errors, predictable output |
| **MCP-native** | MCP speaks JSON; no conversion needed |
| **Schema validation** | JSON Schema is mature with excellent tooling |
| **Strictness** | No ambiguity (`true` vs `"yes"` vs `yes`) |
| **etcd-native** | etcd tooling assumes JSON |
| **Tooling ecosystem** | jq, JSONPath, JSON Schema, every language has solid parsing |

### Documentation via Description Fields

JSON has no comments. Instead, use `description` fields within the config structure:

```json
{
  "stream_id": "air-quality",
  "description": "Air quality measurements from AirGradient sensors",
  "fields": [
    {
      "name": "pm25",
      "type": "float",
      "description": "Particulate matter 2.5 micrometers - EPA AQI primary metric",
      "unit": "µg/m³",
      "range": [0.0, 500.0]
    }
  ]
}
```

Benefits of description fields over comments:
- Descriptions are queryable (MCP can read them)
- Descriptions are validated (part of schema)
- Descriptions don't rot (they're part of the config, not alongside it)

### Architecture Flow

```
JSON file (git-versioned, primary source of truth)
    │
    ├── Agent/MCP generates JSON
    ├── Human reviews via tooling (jq, IDE formatters)
    ├── Git provides durability, versioning
    │
    ▼
Declarative Deploy
    │
    ├── Validates (JSON Schema + custom rules)
    ├── Syncs JSON to etcd (native format)
    ├── Extracts data for data_dictionary (parse → SQL INSERT)
    │
    ▼
etcd (runtime cache, stores JSON natively)
    │
    ├── All runtime components read from here
    ├── serde_json parsing (fast, mature)
    │
    ▼
air-quality-app (Bronze + Silver)
```

### MCP Administration Flow

MCP integration is simplified because JSON is MCP's native format:

```
MCP Tool
    │
    ├── Generates/modifies JSON directly
    ├── Validates via JSON Schema
    │
    ▼
Write JSON to disk
    │
    ▼
Trigger deploy (validate → sync → reload)
    │
    ▼
Git commit/push (backup)
```

---

## Consequences

### Positive

1. **Single source of truth** - JSON files are authoritative
2. **Git durability** - Config survives power loss, is versioned, auditable
3. **Consistent runtime access** - All components read from etcd
4. **MCP-native** - No format conversion for MCP tools
5. **Agent reliability** - Strict JSON format eliminates formatting errors
6. **Schema validation** - JSON Schema catches errors before deployment
7. **Fast parsing** - serde_json is faster than serde_yaml
8. **Rich tooling** - jq, JSONPath, IDE support, formatters

### Negative

1. **No comments** - Must use description fields (mitigated by pattern above)
2. **More verbose** - JSON requires quotes, braces (mitigated by formatters)
3. **Migration required** - Existing YAML configs must be converted to JSON
4. **Human reading** - Less scannable than YAML (mitigated by tooling)

### Neutral

1. **Webhook automation** - Can trigger deploy on git push
2. **etcd is local** - No network latency concerns on Pi

---

## Implementation Requirements

### Must Fix (dp-016 scope)

1. **Silver streaming** (`air-quality-app`): Change `load_silver_etl_config()` to read from etcd instead of directly from files
2. **Unified ConfigLoader**: Create shared config loading trait with consistent behavior
3. **Data dictionary sync**: Integrate into declarative deploy flow
4. **Config migration**: Convert existing YAML configs to JSON

### Storage Format

- **Per-stream JSON**: Store entire stream config as JSON in etcd
- **Key pattern**: `/streams/{stream-id}/config`
- **Atomic updates**: One put per stream, no partial updates
- **Native storage**: JSON stored directly (etcd's native format)

### etcd Structure

```
/streams/
  air-quality/
    config    → {"stream_id":"air-quality","description":"...","fields":[...]}
  outdoor-weather/
    config    → {"stream_id":"outdoor-weather",...}
```

### JSON Schema Validation

Define JSON Schema for each config type:
- `stream-config.schema.json` - Stream configuration
- `dimension-config.schema.json` - Dimension tables
- `manifest.schema.json` - Deploy manifest

Schemas enable:
- IDE autocomplete and validation
- Pre-deploy validation
- MCP tool validation
- Agent output verification

### Human Readability Tooling

```bash
# Pretty print
cat config.json | jq .

# View specific section
jq '.silver_etl.field_mappings' config.json

# VS Code: JSON formatter extension
# IDE: Native JSON formatting
```

---

## Alternatives Considered

### Alternative 1: YAML as Configuration Format

Human-friendly format with comments.

**Rejected because**:
- Agents write 90%+ of config; optimize for agent reliability
- YAML indentation errors are common from LLM generation
- MCP speaks JSON; YAML requires conversion
- JSON Schema tooling is more mature
- Comments can be replaced by description fields

### Alternative 2: etcd as Primary Source of Truth

MCP and humans write directly to etcd. Export to JSON for git versioning.

**Rejected because**:
- Power loss could lose config if etcd not backed up
- Git workflow is familiar and proven
- Would require etcd clustering or backup strategy

### Alternative 3: Database as Source of Truth

Store config in TimescaleDB alongside data.

**Rejected because**:
- Adds database dependency for config
- Harder to version control
- Overkill for edge deployment

---

## Platform Standard Declaration

**JSON is the configuration format for the Neural Data Platform.**

| Artifact | Format | Extension |
|----------|--------|-----------|
| Stream configs | JSON | `.json` |
| Dimension configs | JSON | `.json` |
| Deploy manifests | JSON | `.json` |
| etcd storage | JSON | (native) |
| Device state | JSON | `.json` |
| JSON Schemas | JSON | `.schema.json` |

This standard applies to all configuration artifacts. The only exception is external API responses, which are JSON by industry convention anyway.

---

## Related Decisions

- **ADR-016-002**: Declarative Deploy Architecture
- **Q2**: Per-stream isolation (not runtime/schema split)
- **Q3**: JSON storage per stream
- **Q5**: Hot-reload for sources, full reload for subscribers

---

## References

- `product/features/dp-016/architecture/DECISION-QUESTIONS.md` - Full decision log
- `product/features/dp-016/specification/PAIN-POINTS.md` - P-001 (etcd vs YAML split)
- `product/features/air-013/SCOPE.md` - Unified config source (absorbed into dp-016)
