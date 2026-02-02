# ADR-019-002: Unified Validation Types (`ndp-types` Crate)

**Status**: Proposed
**Date**: 2026-02-02
**Decision Makers**: NDP Architecture Team
**Bug Reference**: BUG-001 Validation Drift Risk
**Feature**: dp-019 Config Validation Pipeline Extension
**Parent ADRs**: ADR-019-001 (Two-Layer Validation)

---

## Context

### The Problem

ADR-019-001 established a two-layer validation architecture for NDP configurations. During implementation, BUG-001 identified a fundamental flaw: **validation rules are defined in multiple places that can drift out of sync**.

Current state analysis reveals **four independent sources of truth**:

| Source | Location | Examples |
|--------|----------|----------|
| **Runtime Rust enums** | `core/src/types/stream_config.rs` | `SourceType { Mqtt, HttpPoll, Webhook, FileWatch, Csv }` |
| **Validator constants** | `tools/ndp-validate/src/semantic/sources.rs` | `const SUPPORTED_SOURCE_TYPES: &[&str] = &["mqtt", "http_poll", ...]` |
| **JSON Schema enums** | `schemas/stream-config.v1.1.schema.json` | `"enum": ["mqtt", "http_poll", "http_push", "file_watch"]` |
| **Silver ETL types** | `core/src/config/silver_etl.rs` | `DqRule` enum with 11 variants, `DqAction` enum |

**Observed discrepancies (as of 2026-02-02):**

1. Runtime has `Csv` source type; JSON Schema does not include it
2. JSON Schema has `"http_push"` but runtime has `Webhook` (naming mismatch)
3. Validator has 11 `SUPPORTED_DQ_RULES` strings; runtime has 11 `DqRule` variants (match but could drift)
4. MCP `validate_config.rs` has independent validation logic

### Failure Scenario

1. Developer adds `Grpc` variant to `SourceType` enum in `core/src/types/stream_config.rs`
2. Developer forgets to update:
   - `SUPPORTED_SOURCE_TYPES` in validator (still 5 types)
   - JSON Schema enum (still has old types)
   - MCP validation logic
3. User creates config with `"type": "grpc"`
4. Config passes `ndp-validate` (semantic layer uses old constants)
5. Config syncs to etcd
6. Runtime fails to deserialize: `unknown variant 'grpc'`
7. **dp-019's purpose defeated** - validation was supposed to prevent this

### Why This Happened

ADR-019-001's two-layer design correctly separated schema (structural) from semantic (application) validation. However, it did not address **type authority**: who owns the canonical list of valid enum values?

The implementation created parallel definitions instead of a single authoritative source that all validation surfaces consume.

---

## Decision

**Create a new `ndp-types` crate that serves as the single source of truth for all NDP configuration types, with generated JSON Schema and a unified validation trait.**

### Core Principles

1. **Types are authoritative** - Rust enums in `ndp-types` are the source of truth
2. **Schema is generated** - JSON Schema derived from Rust types via schemars crate
3. **Validation is unified** - One `NdpValidate` trait implementation used everywhere
4. **DRY enforcement** - Adding a variant to one file automatically propagates everywhere
5. **Compilation fails if types drift** - Consumers import from `ndp-types`, not local definitions

### Architecture

```
                                                ndp-types crate
                                    +-------------------------------+
                                    |                               |
                                    |  SourceType enum              |
                                    |  FieldType enum               |
                                    |  DqRule enum                  |
                                    |  DqAction enum                |
                                    |  TransformType enum           |
                                    |  NdpValidate trait            |
                                    |  ValidationError struct       |
                                    |                               |
                                    +-------------------------------+
                                         /      |      |      \
                                        /       |      |       \
                                       v        v      v        v
                          +----------+   +----------+   +-----------+   +---------------+
                          |   core   |   |ndp-validate| |ndp-mcp-svr|   |schema-gen CLI |
                          +----------+   +----------+   +-----------+   +---------------+
                               |              |              |                  |
                               v              v              v                  v
                          StreamConfig   Semantic      MCP Tools         JSON Schema
                          deserialization validation   validation        v1.2 (generated)
```

### Type Definition Pattern

All enums use maximum derivation for flexibility:

```rust
// crates/ndp-types/src/source_type.rs
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use strum::{EnumIter, EnumString, Display, AsRefStr};

/// Data source types supported by NDP.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash,
    Serialize, Deserialize,  // serde for JSON (de)serialization
    JsonSchema,               // schemars for JSON Schema generation
    EnumIter,                 // strum for iteration over variants
    EnumString,               // strum for string parsing
    Display,                  // strum for Display impl
    AsRefStr,                 // strum for &'static str conversion
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum SourceType {
    /// MQTT broker subscription
    Mqtt,
    /// HTTP endpoint polling
    HttpPoll,
    /// HTTP webhook receiver (push)
    Webhook,
    /// File system watcher
    FileWatch,
    /// CSV file import
    Csv,
}

impl SourceType {
    /// Returns all supported source types (for error messages).
    pub fn all_names() -> Vec<&'static str> {
        use strum::IntoEnumIterator;
        Self::iter().map(|t| t.as_ref()).collect()
    }
}
```

### Validation Trait

```rust
// crates/ndp-types/src/validate.rs

/// Unified validation trait for NDP configurations.
pub trait NdpValidate {
    /// Validate this configuration, returning all errors.
    fn validate(&self) -> Vec<ValidationError>;

    /// Validate with context (field names, columns, etc.)
    fn validate_with_context(&self, ctx: &ValidationContext) -> Vec<ValidationError> {
        self.validate()  // Default: ignore context
    }
}

/// Validation context for cross-reference checks.
pub struct ValidationContext {
    pub field_names: HashSet<String>,
    pub silver_columns: HashSet<String>,
    pub database_url: Option<String>,
}
```

### Consumer Pattern After Migration

```rust
// tools/ndp-validate/src/semantic/sources.rs
use ndp_types::SourceType;

pub fn validate_source_type(source_type: &str) -> Result<(), ValidationError> {
    // Parse using strum's EnumString derive
    source_type.parse::<SourceType>()
        .map_err(|_| ValidationError {
            code: ErrorCode::InvalidSourceType,
            path: "$.sources[].type".into(),
            message: format!(
                "Source type '{}' is not supported. Must be one of: {}",
                source_type,
                SourceType::all_names().join(", ")  // Dynamic from enum!
            ),
            ..Default::default()
        })?;
    Ok(())
}
```

### Schema Generation

JSON Schema is generated from Rust types:

```rust
// tools/ndp-schema-gen/src/main.rs
use schemars::schema_for;
use ndp_types::StreamConfig;

fn main() {
    let schema = schema_for!(StreamConfig);
    let json = serde_json::to_string_pretty(&schema).unwrap();
    std::fs::write("schemas/stream-config.v1.2.schema.json", json).unwrap();
}
```

CI verifies schema matches generated output:

```yaml
- name: Verify Schema
  run: |
    cargo run -p ndp-schema-gen
    git diff --exit-code schemas/
```

---

## Consequences

### Positive

1. **Single source of truth** - One place to add/remove enum variants
2. **Compile-time safety** - Type mismatches become compiler errors
3. **No hardcoded constants** - `SourceType::all_names()` replaces `SUPPORTED_SOURCE_TYPES`
4. **Generated schema** - JSON Schema always matches Rust (CI verified)
5. **Unified validation** - Same logic in CLI, MCP, and runtime
6. **Better error messages** - Dynamic type lists from enum
7. **IDE support** - Type information available everywhere
8. **Documentation in code** - Doc comments become schema descriptions

### Negative

1. **New crate to maintain** - `ndp-types` adds workspace complexity
2. **Migration effort** - Existing code must update imports
3. **Build dependency** - Core, validator, and MCP all depend on ndp-types
4. **schemars limitations** - Complex types may need manual schema patches
5. **Potential circular deps** - Must carefully structure ndp-types to avoid cycles

### Neutral

1. **Re-exports for compatibility** - `core` re-exports types during transition
2. **Schema versioning** - v1.2 is generated, v1.1 remains for reference
3. **strum dependency** - Already used elsewhere in codebase

---

## Alternatives Considered

### Alternative 1: Generate Rust from JSON Schema

Define types in JSON Schema, generate Rust code.

**Rejected because:**
- Rust is more expressive than JSON Schema (methods, traits)
- Generated code is harder to maintain and extend
- IDE support for generated code is poor
- Validation logic would still be separate from types

### Alternative 2: Runtime Type Registry

Build a runtime registry that both schema and validator query.

**Rejected because:**
- Still two definitions (registry + schema)
- No compile-time safety
- Registry could have bugs
- More complex than static enums

### Alternative 3: Shared Constants File

Create a `constants.rs` with all type arrays, import everywhere.

**Rejected because:**
- String arrays, not typed enums
- No compile-time validation
- Schema still separate
- Easy to forget to update constants

### Alternative 4: Macro-based Generation

Use Rust macros to generate types, constants, and schema simultaneously.

**Considered but deferred:**
- Macros are complex to debug
- schemars already handles schema generation
- Can revisit if schemars proves insufficient

### Alternative 5: Keep Multiple Sources, Add CI Check

Keep current structure, add CI to verify all sources match.

**Rejected because:**
- Doesn't prevent drift, only detects it
- CI check is brittle (regex matching)
- Still requires manual sync
- Doesn't provide unified validation

---

## Implementation Plan

### Phase 1: Create ndp-types Crate (No Breaking Changes)

1. Create `crates/ndp-types/` with workspace membership
2. Define all types with full derives
3. Add `NdpValidate` trait
4. Types exist in both locations (no removal yet)

**Exit criteria:** `cargo build --workspace` succeeds

### Phase 2: Migrate Consumers (Incremental)

1. Add `ndp-types` dependency to `core`
2. Replace imports: `use crate::types::SourceType` -> `use ndp_types::SourceType`
3. Add re-exports in `core` for backward compatibility
4. Migrate `ndp-validate` to use `ndp_types`
5. Migrate `ndp-mcp-server` to use `ndp_types`

**Exit criteria:** All tests pass, no hardcoded type constants

### Phase 3: Generate Schema and Cleanup

1. Create `ndp-schema-gen` tool
2. Generate `schemas/stream-config.v1.2.schema.json`
3. Add CI verification
4. Remove deprecated type definitions from `core`
5. Update documentation

**Exit criteria:** CI verifies schema matches generated output

### Migration Compatibility

During migration, both paths work:

```rust
// Both valid during Phase 2:
use neural_core::SourceType;      // Re-exported from ndp-types
use ndp_types::SourceType;        // Direct import (preferred)
```

After Phase 3, only `ndp_types` is canonical.

---

## Validation Impact

### Before (ADR-019-001 Implementation)

```
JSON Schema (v1.1)          Validator Constants         Runtime Enums
      |                           |                          |
      v                           v                          v
"enum": [...]              SUPPORTED_*: &[&str]        enum SourceType
      |                           |                          |
      +------ CAN DRIFT ----------+------ CAN DRIFT ---------+
```

### After (This ADR)

```
                     ndp-types crate
                           |
          +----------------+----------------+
          |                |                |
          v                v                v
    JSON Schema      Validator Logic   Runtime Enums
    (generated)      (uses enum)       (from ndp-types)
          |                |                |
          +------ CANNOT DRIFT -------------+
```

---

## Types Covered

### Critical (Must Migrate)

| Type | Current Location | Variants |
|------|------------------|----------|
| `SourceType` | `core/src/types/stream_config.rs` | Mqtt, HttpPoll, Webhook, FileWatch, Csv |
| `FieldType` | `core/src/types/stream_config.rs` | Float, Int, String, Bool, Json |

### High Priority

| Type | Current Location | Variants |
|------|------------------|----------|
| `DqRule` | `core/src/config/silver_etl.rs` | 11 rule types (RangeCheck, NullCheck, etc.) |
| `DqAction` | `core/src/config/silver_etl.rs` | Flag, Reject, Clamp, Drop, Warn |
| `TimestampTransform` | `core/src/config/silver_etl.rs` | MicrosecondsToTimestamp, Iso8601, UnixSeconds, NwsDuration |

### Medium Priority

| Type | Current Location | Variants |
|------|------------------|----------|
| `TransformConfig` | `core/src/config/silver_etl.rs` | UnitConversion, Expression, Lookup, JsonExtract, Timestamp, Computed |
| `MonotonicDirection` | `core/src/config/silver_etl.rs` | Increasing, Decreasing, StrictIncreasing |
| `DeduplicationStrategy` | `core/src/config/silver_etl.rs` | Upsert, Skip, Replace |

### Low Priority (Future)

| Type | Current Location | Notes |
|------|------------------|-------|
| `OnError` | `core/src/types/stream_config.rs` | CSV source error handling |
| `TimestampFormat` | `core/src/types/stream_config.rs` | CSV timestamp parsing |
| `ValidationStatus` | `ndp-mcp-server` | Could unify with ValidationError |

---

## Success Criteria

1. **Adding a new SourceType variant requires changing exactly 1 file** (ndp-types)
2. **No grep results for `SUPPORTED_SOURCE_TYPES`, `SUPPORTED_DQ_RULES`** etc.
3. **JSON Schema is 100% generated** (verified by CI)
4. **All validation surfaces use `ndp_types::` imports**
5. **Existing configs deserialize without changes**

---

## Related Decisions

- **ADR-019-001**: Two-Layer Validation (parent - establishes validation architecture)
- **ADR-018-001**: JSON Pass-Through (JSON as configuration standard)
- **ADR-016-001**: JSON Source of Truth (JSON over YAML)

---

## References

- BUG-001: `/workspaces/neural-data-platform/product/features/dp-019/bugs/BUG-001-validation-drift.md`
- SPECIFICATION: `/workspaces/neural-data-platform/product/features/dp-019/bugs/BUG-001-fix/SPECIFICATION.md`
- schemars crate: https://docs.rs/schemars
- strum crate: https://docs.rs/strum

---

*Architecture decision created: 2026-02-02*
*Bug Reference: BUG-001 Validation Drift Risk*
*Feature: dp-019 Config Validation Pipeline Extension*
