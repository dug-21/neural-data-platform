# ADR-019-001: Two-Layer Config Validation

**Status**: Proposed
**Date**: 2026-02-02
**Decision Makers**: NDP Architecture Team
**Feature**: dp-019 Config Validation Pipeline
**Parent ADRs**: ADR-016-001 (JSON Source of Truth), ADR-018-001 (JSON Pass-Through)

---

## Context

### The Problem

dp-016's research (VALIDATION-RESEARCH.md) identified a **two-tier validation gap** in the Neural Data Platform. While structural validation (format, types, ranges) exists via serde, semantic validation is largely absent, causing silent failures that are difficult to diagnose.

The specific pain points from dp-016 that drive this decision:

| Pain Point | Current State | Impact |
|------------|---------------|--------|
| **P-002: Unknown fields silent** | `#[serde(flatten)]` captures typos into `extra` HashMap | `silver_elt` (typo) silently ignored, operator has no indication |
| **P-003: No schema validation** | serde parsing only | Malformed JSON accepted if it can be deserialized |
| **P-005: No source_path validation** | `source_path` not checked against `fields` | Silver ETL silently produces NULLs for typos |
| **P-006: No table existence check** | Discovered at INSERT time | Runtime failure, potential data loss |
| **P-007: Unknown fields not reported** | No logging for `extra` HashMap | Typos like `silver_elt` completely invisible |

### Architectural Foundation

dp-018 established **JSON as the platform configuration standard** with **pass-through architecture**:

```
JSON file (source of truth)
    |
    v
etcd (stores JSON blob as-is)
    |
    v
StreamConfig (deserialize from etcd)
```

dp-019 adds the validation gates that prevent invalid configurations from entering etcd.

---

## Decision

**Implement two-layer validation with deploy-time gating and runtime defense-in-depth.**

### Layer 1: JSON Schema Validation (Declarative)

JSON Schema validation handles **structural correctness**:

| Validation | Schema Feature | Example |
|------------|----------------|---------|
| Required fields | `required: [...]` | `stream_id` must exist |
| Type checking | `type: "string"` | `enabled` must be boolean |
| Format validation | `pattern: "regex"` | `stream_id` must be kebab-case |
| Unknown field rejection | `additionalProperties: false` | Typos rejected at schema level |
| Enum constraints | `enum: [...]` | `fields[].type` must be valid NDP type |
| Range validation | `minimum`, `maximum` | `retention_days >= 0` |
| Array constraints | `minItems`, `maxItems` | At least one field required |

**Why JSON Schema for structural validation:**
- Declarative (no code to write per field)
- IDE integration (autocomplete, inline errors)
- Pre-deploy validation without Rust compilation
- Industry standard with mature tooling
- MCP-native (MCP speaks JSON, can validate directly)

### Layer 2: Semantic Validation (Rust Code)

Semantic validation handles **application rules** that JSON Schema cannot express:

| Validation | Why Not Schema | Implementation |
|------------|----------------|----------------|
| `source_path` references `fields` | Cross-field reference | Rust HashSet lookup |
| Silver table exists in TimescaleDB | External system check | SQL query |
| DQ rule expression is valid SQL | Domain-specific language | sqlparser crate |
| Transform formula coefficients match type | Complex logic | Rust match |
| Endpoint URL is reachable | Network check (optional) | reqwest probe |
| MQTT broker is reachable | Network check (optional) | MQTT probe |

### Validation Flow

```
                      Deploy Time                          Runtime
                +------------------------+           +------------------+
                |                        |           |                  |
JSON Config ---+-> Layer 1: Schema      |           | Defensive Check  |
                |   (jsonschema crate)   |           | (same Validator) |
                |   - Structure          |           |                  |
                |   - Types              |           +------------------+
                |   - Enums              |                    ^
                |   - Unknown fields     |                    |
                +------------------------+                    |
                         | pass                               |
                         v                                    |
                +------------------------+                    |
                |                        |                    |
                +-> Layer 2: Semantic    +--------------------+
                |   (Rust code)          |
                |   - Cross-references   |
                |   - Table existence    |
                |   - DQ rule parsing    |
                +------------------------+
                         | pass
                         v
                +------------------------+
                | Sync to etcd           |
                +------------------------+
```

---

## Consequences

### Positive

1. **Fail fast** - Invalid configs caught at deploy time, not runtime. P-005 (source_path) and P-006 (table existence) are caught before data flows
2. **Clear errors** - JSONPath location + actionable message tells operator exactly what to fix
3. **Defense in depth** - Deploy-time validation primary, runtime validation secondary
4. **MCP integration** - `ndp-validate` can be used by MCP tools to validate before save
5. **IDE support** - JSON Schema enables autocomplete and inline validation in editors
6. **Layered approach** - Schema handles structure (bulk of validations), code handles only semantics
7. **P-002/P-007 resolved** - Unknown fields rejected by `additionalProperties: false` in schema
8. **Atomic deployment** - Bad config blocks entire sync, preventing partial state

### Negative

1. **Build complexity** - New Rust binary (`ndp-validate`) to maintain and cross-compile for ARM64
2. **DB dependency for full validation** - Table existence check requires TimescaleDB connection; mitigated by `--schema-only` mode
3. **Schema maintenance** - Enums must stay in sync with Rust code; mitigated by generating schema from code in future
4. **Validation time** - Full validation adds ~500ms to deploy (acceptable for deploy workflow)

### Neutral

1. **CI integration** - Schema-only validation can run in pre-commit (no DB needed)
2. **Emergency bypass** - `--skip-validation` flag available for edge cases

---

## Alternatives Considered

### Alternative 1: Schema-Only Validation

Encode all rules in JSON Schema using complex conditionals.

**Rejected because:**
- JSON Schema conditionals are verbose and hard to maintain
- Cannot check external state (table existence)
- Cross-reference validation (`source_path` -> `fields`) is extremely complex in schema
- Would require JSON Schema extensions or custom keywords

### Alternative 2: Code-Only Validation

Do all validation in Rust, no JSON Schema.

**Rejected because:**
- Loses IDE integration (no autocomplete for config authors)
- Every field requires hand-written validation code
- Duplicates type/required checks that serde already does
- No declarative documentation of constraints
- MCP tools would need to call Rust binary for every validation

### Alternative 3: Runtime-Only Validation

Validate only when app loads config from etcd.

**Rejected because:**
- Invalid config already in etcd (pollutes runtime cache)
- Fails at worst possible time (production startup)
- No pre-deploy safety net
- P-002 (unknown fields) already in etcd by the time validation runs
- Operator discovers problems in production logs, not deploy output

### Alternative 4: Database-Level Constraints

Rely on PostgreSQL constraints to catch invalid data.

**Rejected because:**
- Too late in pipeline (data already attempted)
- Config errors should be caught before any data flows
- Database constraints don't apply to config (which is in etcd/JSON)

---

## Implementation

### Validator Component

A new Rust binary (`ndp-validate`) that can be used:
1. From `deploy.sh` (gates deployment)
2. From app startup (defensive check)
3. From MCP tools (validate before save)
4. From CI/CD (pre-commit validation)

```rust
// tools/ndp-validate/src/lib.rs
pub struct Validator {
    schema: jsonschema::JSONSchema,
    db_pool: Option<PgPool>,  // For table existence checks
}

impl Validator {
    /// Create validator with schema only (no DB checks)
    pub fn schema_only(schema_path: &Path) -> Result<Self, ValidatorError>;

    /// Create validator with full semantic checks
    pub fn with_database(schema_path: &Path, db_url: &str) -> Result<Self, ValidatorError>;

    /// Validate a config file
    pub async fn validate(&self, config_path: &Path) -> ValidationResult;
}
```

### Integration Points

1. **deploy.sh** - Calls `ndp-validate --all` before sync; non-zero exit blocks deployment
2. **app startup** - Runs semantic validation on loaded config; fails loudly if invalid
3. **MCP tools** - Can call validator before saving config changes

### CLI Interface

```bash
# Validate single config
ndp-validate config/base/streams/air-quality/config.json

# Validate all configs (deploy-time)
ndp-validate --all --format human

# Schema-only mode (fast, no DB)
ndp-validate --schema-only config.json

# Full validation with table checks
ndp-validate --all --check-tables
```

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Validation passed (may have warnings) |
| 1 | Validation failed (has errors) |
| 2 | System error (file not found, DB connection failed) |

---

## Validation Responsibilities Summary

### What JSON Schema Validates (Layer 1)

- Required fields (`stream_id`, `fields`, `sources`)
- Type checking (string, number, boolean, array, object)
- Pattern matching (stream_id kebab-case, field names snake_case)
- Enum constraints (`fields[].type` in `[float, integer, string, boolean, timestamp, json]`)
- Unknown field rejection (`additionalProperties: false`)
- Range constraints (`retention_days >= 0`)

### What Rust Code Validates (Layer 2)

- `source_path` references existing field in `fields[]`
- `target_table` exists in TimescaleDB (optional mode)
- DQ rule expressions are valid SQL syntax
- Transform functions are supported
- Source configs have required fields for their type (MQTT needs `broker_url`, HTTP needs `endpoints`)
- `retention_days >= compression_after_days` (cross-field logic)

---

## Related Decisions

- **ADR-016-001**: JSON as source of truth (enables JSON Schema validation)
- **ADR-016-002**: Declarative deploy (validation gates deployment)
- **ADR-018-001**: Pass-through architecture (validation before sync to etcd)

---

## References

- `/workspaces/neural-data-platform/product/features/dp-016/specification/VALIDATION-RESEARCH.md` - Current gaps
- `/workspaces/neural-data-platform/product/features/dp-016/specification/PAIN-POINTS.md` - P-002, P-005, P-006, P-007
- `/workspaces/neural-data-platform/product/features/dp-016/architecture/ADR-016-001-config-source-of-truth.md` - JSON standard
- `/workspaces/neural-data-platform/product/features/dp-018/architecture/ADR-018-001-config-loader-design.md` - Pass-through
- `/workspaces/neural-data-platform/product/features/dp-019/architecture/VALIDATION-ARCHITECTURE.md` - Full architecture
- `/workspaces/neural-data-platform/product/features/dp-019/specification/SPECIFICATION.md` - Requirements
- `/workspaces/neural-data-platform/schemas/stream-config.v1.1.schema.json` - Current schema

---

*Architecture decision created: 2026-02-02*
*Feature: dp-019 Config Validation Pipeline*
