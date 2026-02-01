# dp-019: Config Validation Pipeline

## Parent Initiative

This feature implements **Phase 2** of [dp-016: Configuration Architecture Review](../dp-016/IMPLEMENTATION-ROADMAP.md).

---

## Problem Statement

Invalid configurations are accepted until runtime failure:

1. **No schema validation** - Malformed JSON/YAML accepted at sync time
2. **No semantic validation** - Invalid field types, missing references accepted
3. **Silent failures** - Bad `source_path` references cause Silver ETL to silently skip fields
4. **No table existence check** - Missing Silver tables discovered at INSERT time
5. **Unknown NDP-supported values** - No documented list of valid types, device_classes, transforms

These issues were documented in dp-016's pain points (P-002, P-003, P-004, P-005, P-006).

---

## Goals

1. **Two-layer validation**: Schema (structure) + Semantic (application rules)
2. **Research NDP-supported values** - Document what the platform actually accepts
3. **Research DDL generation requirements** - Type mapping, index strategy for dp-020
4. **Fail fast** - Catch errors at deploy time, not runtime

---

## Scope

### In Scope

**Research Tasks**

| ID | Task | Description | Output |
|----|------|-------------|--------|
| 2.0 | Research NDP-supported values | Audit codebase for valid types, device_classes, transforms, DQ operators | `docs/config/SUPPORTED-VALUES.md` |
| 2.0a | Research DDL generation | Type mapping (JSON → PostgreSQL), index strategy, hypertable config | `docs/config/DDL-GENERATION.md` |

**Layer 1: Schema Validation**

| ID | Task | Description | Acceptance Criteria |
|----|------|-------------|---------------------|
| 2.1 | Create Validator component | Rust binary with two-layer validation | Returns structured errors |
| 2.2 | JSON syntax validation | Catch malformed JSON early | Clear error messages with line numbers |
| 2.3 | JSON Schema validation | Validate structure, types, required fields | Uses `jsonschema` crate |
| 2.4 | Unknown field detection | Fail on unexpected fields | `additionalProperties: false` |

**Layer 2: Semantic Validation**

| ID | Task | Description | Acceptance Criteria |
|----|------|-------------|---------------------|
| 2.5 | Valid `type` values | Check field types against NDP-supported types | Rejects unsupported types |
| 2.6 | Valid `device_class` values | Check against NDP-recognized device classes | Warns or errors on unknown |
| 2.7 | Cross-reference validation | Validate `source_path` references exist in `fields` | Catches P-005 |
| 2.8 | Silver table existence check | Verify target table exists in TimescaleDB | Catches P-006 |
| 2.9 | DQ rule syntax validation | Validate DQ expressions against supported operators | Catches invalid rules |
| 2.10 | Source config validation | Validate MQTT/HTTP configs have required fields | Catches misconfigured sources |

**Integration**

| ID | Task | Description | Acceptance Criteria |
|----|------|-------------|---------------------|
| 2.11 | Integrate into deploy.sh | Validation gates deployment | Bad config = deploy failure |
| 2.12 | Runtime startup validation | Defensive check at app startup | Defense in depth |
| 2.13 | Decide: Schema vs Code | Determine which semantic rules can be JSON Schema enums | Balance declarative vs programmatic |

### Out of Scope

- DDL generation implementation (dp-020)
- Manifest-driven deployment (dp-020)
- Hot-reload (dp-021)

---

## Technical Context

### Two Validation Layers

| Layer | What | How | Example |
|-------|------|-----|---------|
| **Schema** | Structure, types, required fields | JSON Schema (declarative) | "fields must be an array" |
| **Semantic** | Application rules, valid values | Rust code (programmatic) | "type 'decimal' not supported" |

### Research: NDP-Supported Values (2.0)

| Field | Question | Where to Look |
|-------|----------|---------------|
| `fields[].type` | What types does Bronze/Silver support? | `core/src/models/`, Parquet writers |
| `fields[].device_class` | Constrained or freeform? | entity_schemas usage, Grafana |
| `sources[].type` | What source types exist? | `core/src/sources/`, SourceManager |
| `silver_etl.field_mappings[].transform` | What transforms supported? | Silver ETL code |
| `dq_rules[].expression` | What DQ operators valid? | DQ evaluation code |
| `storage.format` | What storage formats? | BronzeSubscriber |

### Research: DDL Generation (2.0a)

| Topic | Question | Output |
|-------|----------|--------|
| Type Mapping | JSON type → PostgreSQL type? | Type mapping table |
| Index Strategy | Auto-create (timestamp, ndp_id)? Additional from DQ? | Index generation rules |
| Hypertable Config | chunk_time_interval? Compression? | Default settings |
| Permissions | What roles need access? | Permission templates |

**Proposed Type Mapping** (to validate):

| Config Type | PostgreSQL Type |
|-------------|-----------------|
| `string` | `TEXT` |
| `float` | `DOUBLE PRECISION` |
| `integer` | `BIGINT` |
| `boolean` | `BOOLEAN` |
| `timestamp` | `TIMESTAMPTZ` |
| `json` | `JSONB` |

### Validator CLI

```bash
# Validate single config (both layers)
ndp-validate config/base/streams/air-quality/config.json

# Schema validation only (fast, no DB)
ndp-validate --schema-only config.json

# Full validation with database checks
ndp-validate --check-tables --check-source-paths config.json

# Validate all configs
ndp-validate --all
```

### Error Output Format

```json
{
  "valid": false,
  "errors": [
    {
      "layer": "schema",
      "path": "$.fields[0].type",
      "message": "must be one of: float, integer, string, boolean, timestamp",
      "severity": "error"
    },
    {
      "layer": "semantic",
      "path": "$.silver_etl.field_mappings[2].source_path",
      "message": "source_path 'raw_payload.typo_field' not found in fields",
      "severity": "error"
    }
  ]
}
```

---

## Deliverables

| Deliverable | Location | Description |
|-------------|----------|-------------|
| NDP-supported values doc | `docs/config/SUPPORTED-VALUES.md` | Research output |
| DDL generation doc | `docs/config/DDL-GENERATION.md` | Type mapping, index strategy |
| Validator binary | `tools/ndp-validate/` | Two-layer validation tool |
| Updated JSON Schemas | `schemas/` | With enums where applicable |
| deploy.sh integration | `deploy/pi/deploy.sh` | Validation gates deployment |

---

## Success Criteria

1. **Research complete** - SUPPORTED-VALUES.md and DDL-GENERATION.md produced
2. **Schema validation catches** - Malformed JSON, missing required fields, unknown fields
3. **Semantic validation catches** - Invalid types, bad source_path refs, missing tables
4. **Deploy blocked** on validation failure
5. **Structured error output** with path and actionable message

### Verification Commands

```bash
# Validate all configs
ndp-validate --all

# Test with intentionally bad config
ndp-validate tests/fixtures/invalid-config.json
# Expected: validation failure with clear error

# Deploy should fail on bad config
DEPLOY_ENV=integration ./deploy.sh sync
# Expected: blocked if validation fails
```

---

## Dependencies

| Dependency | Status | Notes |
|------------|--------|-------|
| dp-018 | **REQUIRED** | JSON configs and ConfigLoader trait |
| dp-017 | **REQUIRED** | Integration environment for testing |

---

## References

- [dp-016 IMPLEMENTATION-ROADMAP.md](../dp-016/IMPLEMENTATION-ROADMAP.md) - Phase 2 details
- [dp-016 VALIDATION-RESEARCH.md](../dp-016/specification/VALIDATION-RESEARCH.md) - Current validation gaps
- [dp-016 PAIN-POINTS.md](../dp-016/specification/PAIN-POINTS.md) - P-002 through P-006

---

*Scope created: 2026-02-01*
*Parent: dp-016 Configuration Architecture Review*
