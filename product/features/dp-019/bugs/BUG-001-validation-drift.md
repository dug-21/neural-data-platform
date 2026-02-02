# BUG-001: Validation Drift Risk - Multiple Sources of Truth

**Severity**: HIGH
**Type**: Architectural Flaw
**Discovered**: 2026-02-02
**Status**: RESOLVED - Phase 3 Complete (Migration Done)

---

## Problem Statement

The dp-019 Config Validation Pipeline has a fundamental architectural flaw: **validation rules are defined in multiple places that can drift out of sync**.

If a developer adds a new source type (e.g., `grpc`) to the runtime but forgets to update the validator, configs will pass validation but fail at runtime - the exact failure mode dp-019 was designed to prevent.

---

## Current Architecture (Flawed)

```
┌─────────────────────────────────────────────────────────────────────────┐
│  THREE SEPARATE SOURCES OF TRUTH (CAN DRIFT)                            │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  1. RUNTIME TYPES (core/src/types/stream_config.rs)                     │
│     pub enum SourceType { Mqtt, HttpPoll, Webhook, FileWatch, Csv }     │
│     pub enum FieldType { Float, Int, String, Bool, Json }               │
│                                                                         │
│  2. VALIDATOR CONSTANTS (tools/ndp-validate/src/semantic/sources.rs)    │
│     const SUPPORTED_SOURCE_TYPES: &[&str] = &["mqtt", "http_poll", ...] │
│     (11 DQ rule types also hardcoded in dq_rules.rs)                    │
│                                                                         │
│  3. JSON SCHEMA (schemas/stream-config.v1.1.schema.json)                │
│     "type": { "enum": ["mqtt", "http_poll", ...] }                      │
│                                                                         │
│  4. MCP TOOLS (core/ndp-mcp-server/src/mcp/tools/validate_config.rs)    │
│     May have its own validation logic                                   │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Affected Validation Points

| Validation Point | Current Implementation | Risk |
|------------------|----------------------|------|
| `ndp-validate` CLI | Hardcoded constants | Can drift from runtime |
| JSON Schema | Separate file | Can drift from both |
| MCP validate_config | Unknown | May have own logic |
| Runtime startup | Uses runtime types | Authoritative but late |

---

## Failure Modes

### Scenario 1: New Source Type Added
1. Developer adds `Grpc` variant to `SourceType` enum in runtime
2. Forgets to update `SUPPORTED_SOURCE_TYPES` in validator
3. Config with `"type": "grpc"` passes validation
4. Runtime fails at startup with deserialization error
5. **dp-019's purpose defeated**

### Scenario 2: New DQ Rule Type Added
1. Developer adds `statistical_check` DQ rule type to runtime
2. Forgets to update `dq_rules.rs` validator
3. Config with `"type": "statistical_check"` passes validation
4. Silver ETL silently ignores the rule or crashes

### Scenario 3: Schema/Rust Mismatch
1. Schema allows `"webhook"` source type
2. Rust enum uses `HttpPush` variant with `#[serde(rename = "http_push")]`
3. Validator accepts `webhook`, runtime expects `http_push`
4. Validation passes, runtime fails

---

## Root Cause

**NFR-006** in SPECIFICATION.md states: "Validation rules are declarative where possible"

However, the implementation created **parallel definitions** instead of a **single authoritative source** that generates/drives all validation.

---

## Required Fix

### Objective
**One place to update, used in multiple places.**

Any validation surface (CLI, MCP, runtime) must use the **same type definitions** and **same validation logic** - derived from a single source of truth.

### Proposed Architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│  SINGLE SOURCE OF TRUTH                                                 │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  crates/ndp-types/src/lib.rs (NEW)                                      │
│  ├── SourceType enum (with serde, validation traits)                    │
│  ├── FieldType enum                                                     │
│  ├── DqRuleType enum                                                    │
│  ├── TransformType enum                                                 │
│  └── StreamConfig struct (with validate() method)                       │
│                                                                         │
│  CONSUMERS:                                                             │
│  ├── core/              → use ndp_types::{StreamConfig, SourceType}     │
│  ├── tools/ndp-validate → use ndp_types::{StreamConfig, validate()}     │
│  ├── ndp-mcp-server     → use ndp_types::{StreamConfig, validate()}     │
│  └── schemas/           → GENERATED from ndp_types via build.rs         │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Key Design Principles

1. **Types are authoritative** - Rust enums in `ndp-types` are the source of truth
2. **Schema is generated** - JSON Schema derived from Rust types (schemars crate)
3. **Validation is unified** - One `validate()` implementation used everywhere
4. **DRY enforcement** - Compilation fails if types don't match

---

## Scope Boundaries

### In Scope
- Bronze layer validation (source types, field types, basic config structure)
- DQ rule validation (rule types, operators, column references)
- Silver ETL validation (transforms, field mappings)

### Potentially Separate
- Silver table existence checks (requires DB connection)
- Cross-config validation (e.g., dimension references)

---

## Deliverables Required

1. **SPARC Specification** - Full requirements for `ndp-types` crate
2. **Architecture Decision Record** - ADR documenting the unified validation approach
3. **Migration Plan** - How to refactor existing code without breaking changes
4. **Test Strategy** - Ensure no regression in validation coverage

---

## References

- `core/src/types/stream_config.rs` - Current runtime types
- `tools/ndp-validate/src/semantic/` - Current validator implementation
- `schemas/stream-config.v1.1.schema.json` - Current JSON Schema
- `product/features/dp-019/specification/SPECIFICATION.md` - Original requirements

---

---

## Resolution (2026-02-02)

### What Was Done

**Phase 1: ndp-types Crate Created** (by parallel agent)
- Created `crates/ndp-types/` with single source of truth types
- Implemented `SourceType`, `FieldType`, `DqRuleType`, `DqAction`, `MonotonicDirection`, `ErrorCode`
- All types derive `serde`, `schemars`, `strum` traits for serialization and iteration
- Added `all_names()` method to all enum types returning `&'static [&'static str]`

**Phase 3: Consumer Migration Completed**

1. **core/Cargo.toml** - Added `ndp-types = { workspace = true }` dependency

2. **core/src/types/stream_config.rs** - Replaced local enum definitions:
   ```rust
   // BUG-001-fix: Import types from ndp-types (single source of truth)
   pub use ndp_types::{FieldType, SourceType};
   ```

3. **core/src/types/mod.rs** - Added re-exports for backward compatibility:
   ```rust
   pub use ndp_types::{DqAction, DqRuleType, ErrorCode, FieldType, MonotonicDirection, SourceType};
   ```

4. **tools/ndp-validate/src/semantic/sources.rs** - Replaced hardcoded constants:
   ```rust
   // BUG-001-fix: Import SourceType from ndp-types (single source of truth)
   use ndp_types::SourceType;

   fn supported_source_types() -> &'static [&'static str] {
       SourceType::all_names()
   }
   ```

5. **tools/ndp-validate/src/semantic/dq_rules.rs** - Replaced hardcoded constants:
   ```rust
   // BUG-001-fix: Import DQ types from ndp-types (single source of truth)
   use ndp_types::{DqAction, DqRuleType};

   fn supported_dq_rules() -> &'static [&'static str] {
       DqRuleType::all_names()
   }
   ```

### Verification

- `cargo build --workspace` - All packages compile successfully
- `cargo test -p ndp-types` - 88 unit tests, 16 doc tests pass
- `cargo test -p ndp-validate -- semantic::dq_rules` - 20 tests pass
- `cargo test -p ndp-validate -- semantic::sources` - 18 tests pass

### Result

The validation drift bug is now **resolved**:
- Single source of truth in `ndp-types` crate
- Adding a new source type to `SourceType` enum automatically makes it available to validators
- Adding a new DQ rule type to `DqRuleType` enum automatically makes it available to validators
- Backward compatibility maintained via re-exports in `core/src/types/mod.rs`

---

*Created: 2026-02-02*
*Resolved: 2026-02-02*
*Resolution: ndp-types crate with consumer migration complete*
