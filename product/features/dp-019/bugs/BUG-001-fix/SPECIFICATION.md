# BUG-001-fix: Unified Validation Types (`ndp-types` Crate) - SPARC Specification

**Document Type**: SPARC Specification (Phase S)
**Bug Reference**: BUG-001 Validation Drift Risk
**Feature**: dp-019 Config Validation Pipeline Extension
**Version**: 1.0
**Date**: 2026-02-02
**Severity**: HIGH
**Status**: Proposed

---

## 1. Executive Summary

This specification defines the requirements for creating an `ndp-types` crate that serves as the **single source of truth** for all NDP configuration types. The crate eliminates validation drift by ensuring that Rust enums, JSON Schema, and all validation surfaces derive from the same type definitions.

### Problem Statement

BUG-001 identified a fundamental architectural flaw in dp-019: validation rules are defined in **multiple places that can drift out of sync**:

1. **Runtime types** (`core/src/types/stream_config.rs`) - Rust enums for `SourceType`, `FieldType`
2. **Validator constants** (`tools/ndp-validate/src/semantic/sources.rs`) - Hardcoded `SUPPORTED_SOURCE_TYPES`
3. **JSON Schema** (`schemas/stream-config.v1.1.schema.json`) - Enum arrays
4. **MCP tools** (`core/ndp-mcp-server/src/mcp/tools/validate_config.rs`) - Independent validation
5. **Silver ETL config** (`core/src/config/silver_etl.rs`) - `DqRule`, `TransformConfig` enums

If a developer adds a new variant (e.g., `Grpc` source type) to runtime but forgets to update the validator, configs pass validation but fail at runtime - defeating dp-019's purpose.

### Solution

**Create a single `ndp-types` crate containing all authoritative type definitions:**

- Types are defined once (Rust enums with serde and schemars derives)
- JSON Schema is **generated** from Rust types (schemars crate)
- Validation is **unified** - one `validate()` trait used everywhere
- Compilation fails if types drift (consumers import from `ndp-types`)

### Key Outcomes

1. **Single source of truth** - All types in one crate
2. **Generated schema** - JSON Schema derived from Rust via schemars
3. **Unified validation trait** - `NdpValidate` trait implemented by all config types
4. **Compile-time safety** - Type mismatches become compiler errors
5. **Backward compatible** - Existing code migrates incrementally

---

## 2. Requirements Analysis

### 2.1 Functional Requirements

#### Type Unification

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| **FR-001** | Create `ndp-types` crate at `crates/ndp-types/` | CRITICAL | Crate compiles, has workspace membership |
| **FR-002** | Move `SourceType` enum to ndp-types | CRITICAL | `core` imports `SourceType` from `ndp_types` |
| **FR-003** | Move `FieldType` enum to ndp-types | CRITICAL | `core` imports `FieldType` from `ndp_types` |
| **FR-004** | Move `TimestampTransform` enum to ndp-types | HIGH | Used by Silver ETL config |
| **FR-005** | Move `DqRuleType` enum to ndp-types | HIGH | Validator uses unified enum, not string constants |
| **FR-006** | Move `DqAction` enum to ndp-types | HIGH | Actions validated against Rust enum |
| **FR-007** | Move `TransformType` enum to ndp-types | MEDIUM | Silver field transforms validated |
| **FR-008** | Move `DeduplicationStrategy` enum to ndp-types | LOW | Strategy validated at schema level |
| **FR-009** | Move `MonotonicDirection` enum to ndp-types | LOW | DQ rule direction validated |

#### Schema Generation

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| **FR-010** | Add schemars derives to all enums | CRITICAL | `#[derive(JsonSchema)]` on every type |
| **FR-011** | `ndp-validate --generate-schema` outputs JSON Schema | CRITICAL | Schema matches Rust enum variants exactly |
| **FR-012** | `ndp-validate --generate-schema --output <path>` writes to file | HIGH | File written, stdout if omitted |
| **FR-013** | `ndp-validate --verify-schema <path>` checks for drift | CRITICAL | Exit 0 if match, exit 1 if drift |
| **FR-014** | Schema output includes enum descriptions | HIGH | `description` field from Rust doc comments |
| **FR-015** | Schema supports discriminated unions | MEDIUM | Tagged enums produce correct JSON Schema |

#### Validation Trait

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| **FR-020** | Define `NdpValidate` trait in ndp-types | CRITICAL | Trait with `validate(&self) -> Vec<ValidationError>` |
| **FR-021** | Implement `NdpValidate` for `StreamConfig` | HIGH | Existing validation logic migrated |
| **FR-022** | Implement `NdpValidate` for `SilverEtlConfig` | HIGH | DQ rules and mappings validated |
| **FR-023** | Implement `NdpValidate` for `DqRule` | HIGH | Rule-specific validation (range bounds, etc.) |
| **FR-024** | `ValidationError` struct in ndp-types | HIGH | Unified error format with path, code, message |

#### Consumer Migration

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| **FR-030** | `core` depends on `ndp-types` | CRITICAL | `neural-core` Cargo.toml has dependency |
| **FR-031** | `ndp-validate` depends on `ndp-types` | CRITICAL | Validator uses same types as runtime |
| **FR-032** | `ndp-mcp-server` depends on `ndp-types` | HIGH | MCP validation uses unified types |
| **FR-033** | Remove hardcoded constants from validator | HIGH | No `SUPPORTED_SOURCE_TYPES` strings |
| **FR-034** | Re-export types from `core` for backward compat | MEDIUM | Existing `use neural_core::SourceType` still works |

### 2.2 Non-Functional Requirements

| ID | Category | Requirement | Measurement |
|----|----------|-------------|-------------|
| **NFR-001** | Performance | Schema generation completes in <5s | Build time benchmark |
| **NFR-002** | Reliability | Zero drift between Rust and schema | CI check: generated schema matches repo |
| **NFR-003** | Maintainability | Adding a new enum variant requires 1 file change | Developer workflow test |
| **NFR-004** | Compatibility | Existing configs deserialize without change | Regression tests pass |
| **NFR-005** | Portability | Crate compiles on ARM64 (Raspberry Pi) | Cross-compile check |

---

## 3. Architecture

### 3.1 Crate Structure

```
crates/
  ndp-types/
    Cargo.toml
    src/
      lib.rs              # Re-exports all types
      source_type.rs      # SourceType enum
      field_type.rs       # FieldType enum
      dq_rule.rs          # DqRuleType, DqAction, rule variants
      transform.rs        # TransformType, TimestampTransform
      validate.rs         # NdpValidate trait, ValidationError
      stream_config.rs    # StreamConfig, SchemaField (optional)
      silver_etl.rs       # SilverEtlConfig, SilverFieldMapping (optional)
    build.rs              # Schema generation (optional)
```

### 3.2 Type Definitions Pattern

All enums follow this pattern for maximum derivation:

```rust
// crates/ndp-types/src/source_type.rs
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use strum::{EnumIter, EnumString, Display, AsRefStr};

/// Data source types supported by NDP.
///
/// Each variant corresponds to a specific data ingestion pattern.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash,
    Serialize, Deserialize, JsonSchema,
    EnumIter, EnumString, Display, AsRefStr
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum SourceType {
    /// MQTT broker subscription
    Mqtt,
    /// HTTP endpoint polling
    HttpPoll,
    /// HTTP webhook receiver
    Webhook,
    /// File system watcher
    FileWatch,
    /// CSV file import
    Csv,
}

impl SourceType {
    /// Returns all supported source types.
    pub fn all() -> impl Iterator<Item = Self> {
        use strum::IntoEnumIterator;
        Self::iter()
    }

    /// Returns source types as string slice (for error messages).
    pub fn all_names() -> Vec<&'static str> {
        use strum::IntoEnumIterator;
        Self::iter().map(|t| t.as_ref()).collect()
    }
}
```

### 3.3 Validation Trait Design

```rust
// crates/ndp-types/src/validate.rs
use std::collections::HashSet;

/// Validation error with JSONPath location and actionable message.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationError {
    /// Validation layer (syntax, schema, semantic)
    pub layer: ValidationLayer,
    /// Error code for programmatic handling
    pub code: ErrorCode,
    /// JSONPath to the error location
    pub path: String,
    /// Human-readable error message
    pub message: String,
    /// Severity (error blocks, warning does not)
    pub severity: Severity,
    /// Optional suggestion for fixing
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationLayer {
    Syntax,
    Schema,
    Semantic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// Trait for NDP configuration validation.
///
/// Implementors provide semantic validation beyond JSON Schema.
pub trait NdpValidate {
    /// Validate this configuration, returning all errors.
    fn validate(&self) -> Vec<ValidationError>;

    /// Validate with additional context (e.g., known columns).
    fn validate_with_context(&self, ctx: &ValidationContext) -> Vec<ValidationError> {
        self.validate()
    }
}

/// Validation context providing cross-reference information.
pub struct ValidationContext {
    /// Known field names (for source_path validation)
    pub field_names: HashSet<String>,
    /// Known Silver column names (for DQ rule validation)
    pub silver_columns: HashSet<String>,
    /// Database connection string (optional, for table checks)
    pub database_url: Option<String>,
}
```

### 3.4 Consumer Pattern

After migration, consumers import from `ndp-types`:

```rust
// In core/src/types/stream_config.rs
use ndp_types::{SourceType, FieldType, NdpValidate, ValidationError};

// Struct uses imported types
pub struct SourceConfig {
    #[serde(rename = "type")]
    pub source_type: SourceType,  // From ndp-types
    // ...
}
```

```rust
// In tools/ndp-validate/src/semantic/sources.rs
use ndp_types::SourceType;

pub fn validate_source_type(source_type: &str) -> Result<SourceType, ValidationError> {
    source_type.parse::<SourceType>()
        .map_err(|_| ValidationError {
            code: ErrorCode::InvalidSourceType,
            message: format!(
                "Source type '{}' is not supported. Must be one of: {}",
                source_type,
                SourceType::all_names().join(", ")
            ),
            // ...
        })
}
```

### 3.5 Schema Generation

JSON Schema is generated from Rust types using schemars:

```rust
// crates/ndp-types/build.rs (or separate CLI tool)
use schemars::schema_for;
use ndp_types::{SourceType, FieldType, DqRuleType, DqAction};

fn main() {
    // Generate partial schemas for each type
    let source_type_schema = schema_for!(SourceType);
    let field_type_schema = schema_for!(FieldType);

    // Or generate full StreamConfig schema
    let stream_config_schema = schema_for!(StreamConfig);

    // Write to schemas/ directory
    std::fs::write(
        "schemas/stream-config.v1.2.schema.json",
        serde_json::to_string_pretty(&stream_config_schema).unwrap()
    ).unwrap();
}
```

---

## 4. Types to Migrate

### 4.1 Core Types (CRITICAL)

| Type | Current Location | New Location | Notes |
|------|------------------|--------------|-------|
| `SourceType` | `core/src/types/stream_config.rs:183-191` | `ndp-types/src/source_type.rs` | 5 variants: Mqtt, HttpPoll, Webhook, FileWatch, Csv |
| `FieldType` | `core/src/types/stream_config.rs:31-38` | `ndp-types/src/field_type.rs` | 5 variants: Float, Int, String, Bool, Json |
| `TimestampTransform` | `core/src/config/silver_etl.rs:191-202` | `ndp-types/src/transform.rs` | 4 variants: MicrosecondsToTimestamp, Iso8601, UnixSeconds, NwsDuration |

### 4.2 DQ Types (HIGH)

| Type | Current Location | New Location | Notes |
|------|------------------|--------------|-------|
| `DqRule` | `core/src/config/silver_etl.rs:354-498` | `ndp-types/src/dq_rule.rs` | 11 variants (tagged enum) |
| `DqAction` | `core/src/config/silver_etl.rs:596-610` | `ndp-types/src/dq_rule.rs` | 5 variants: Flag, Reject, Clamp, Drop, Warn |
| `MonotonicDirection` | `core/src/config/silver_etl.rs:587-593` | `ndp-types/src/dq_rule.rs` | 3 variants |

### 4.3 Transform Types (MEDIUM)

| Type | Current Location | New Location | Notes |
|------|------------------|--------------|-------|
| `TransformConfig` | `core/src/config/silver_etl.rs:290-317` | `ndp-types/src/transform.rs` | 6 variants (tagged enum) |
| `ConversionFormula` | `core/src/config/silver_etl.rs:320-328` | `ndp-types/src/transform.rs` | 2 variants: Linear, Custom |

### 4.4 Strategy Types (LOW)

| Type | Current Location | New Location | Notes |
|------|------------------|--------------|-------|
| `DeduplicationStrategy` | `core/src/config/silver_etl.rs:688-698` | `ndp-types/src/strategy.rs` | 3 variants: Upsert, Skip, Replace |
| `OnError` | `core/src/types/stream_config.rs:244-253` | `ndp-types/src/csv_source.rs` | CSV error handling: Skip, Fail, Log |
| `TimestampFormat` | `core/src/types/stream_config.rs:228-240` | `ndp-types/src/csv_source.rs` | CSV timestamp formats |

### 4.5 Constants to Remove

| Constant | Location | Replacement |
|----------|----------|-------------|
| `SUPPORTED_SOURCE_TYPES` | `tools/ndp-validate/src/semantic/sources.rs:14` | `SourceType::all_names()` |
| `SUPPORTED_DQ_RULES` | `tools/ndp-validate/src/semantic/dq_rules.rs:36-48` | `DqRuleType::all_names()` |
| `SUPPORTED_ACTIONS` | `tools/ndp-validate/src/semantic/dq_rules.rs:51` | `DqAction::all_names()` |
| JSON Schema enums | `schemas/stream-config.v1.1.schema.json` | Generated from Rust |

---

## 5. Migration Strategy

### Phase 1: Create ndp-types Crate (No Breaking Changes)

1. Create `crates/ndp-types/` with all type definitions
2. Types are duplicated (exist in both locations)
3. Add `#[deprecated]` to original locations pointing to ndp-types
4. All tests pass with both locations

### Phase 2: Migrate Consumers (Incremental)

1. Update `core` to depend on ndp-types
2. Replace `use crate::types::SourceType` with `use ndp_types::SourceType`
3. Re-export from core for backward compatibility: `pub use ndp_types::SourceType`
4. Update `ndp-validate` to use ndp-types
5. Update `ndp-mcp-server` to use ndp-types

### Phase 3: Remove Duplicates

1. Remove hardcoded constants from validator
2. Remove deprecated type definitions from core
3. Generate JSON Schema from Rust
4. Update CI to verify schema matches generated output

### Backward Compatibility

During migration, existing code continues to work:

```rust
// These both work during migration:
use neural_core::SourceType;      // Re-exported from ndp-types
use ndp_types::SourceType;        // Direct import (preferred)
```

---

## 6. JSON Schema Generation

### 6.1 Generation Strategy

**Option A: Build-time generation (build.rs)**
- Schema generated during cargo build
- Always in sync with Rust types
- Cons: Requires build to update schema

**Option B: ndp-validate --generate-schema (CHOSEN)**
- Extends existing `ndp-validate` CLI with schema generation
- Single tool for all config validation concerns
- Schema committed to repo
- CI verifies schema matches generated output

### 6.2 CLI Interface

```bash
# Generate schema to stdout
ndp-validate --generate-schema

# Generate schema to file
ndp-validate --generate-schema --output schemas/stream-config.v1.2.schema.json

# Verify committed schema matches generated (for CI)
ndp-validate --verify-schema schemas/stream-config.v1.2.schema.json
```

**New CLI flags:**
| Flag | Description |
|------|-------------|
| `--generate-schema` | Output JSON Schema derived from ndp-types |
| `--output <path>` | Write schema to file (default: stdout) |
| `--verify-schema <path>` | Exit 0 if file matches generated, exit 1 if drift |

### 6.3 Schema Versioning

| Version | Description |
|---------|-------------|
| v1.0 | Original handwritten schema |
| v1.1 | dp-018 enriched fields, entity_schemas deprecated |
| v1.2 | **Generated from ndp-types** - exact enum matching |

### 6.4 CI Verification

```yaml
# .github/workflows/schema-check.yml
- name: Verify Schema Not Drifted
  run: |
    cargo build -p ndp-validate --release
    ./target/release/ndp-validate --verify-schema schemas/stream-config.v1.2.schema.json
    # Exit code 0 = schema matches, 1 = drift detected
```

If drift is detected, developer runs:
```bash
ndp-validate --generate-schema --output schemas/stream-config.v1.2.schema.json
git add schemas/stream-config.v1.2.schema.json
git commit --amend  # Include schema update with type change
```

---

## 7. Acceptance Criteria

### AC-001: Type Unification

```gherkin
Feature: Single Source of Truth

  Scenario: Adding new source type
    Given ndp-types has SourceType enum with Mqtt, HttpPoll, Webhook, FileWatch, Csv
    When developer adds Grpc variant to SourceType enum
    Then core/types/stream_config.rs uses the new variant automatically
    And ndp-validate accepts "grpc" source type automatically
    And MCP validate_config accepts "grpc" automatically
    And generated JSON Schema includes "grpc" in enum
    And no hardcoded strings need updating

  Scenario: Removing deprecated type
    Given ndp-types has FieldType with deprecated "decimal" variant
    When developer removes "decimal" from FieldType
    Then compilation fails in any code still using "decimal"
    And JSON Schema no longer includes "decimal"
```

### AC-002: Schema Generation

```gherkin
Feature: Generated JSON Schema

  Scenario: Schema matches Rust enums
    Given ndp-types/src/source_type.rs has SourceType with 5 variants
    When I run `cargo run -p ndp-schema-gen`
    Then schemas/stream-config.v1.2.schema.json is updated
    And schema $.sources[].type.enum has exactly ["mqtt", "http_poll", "webhook", "file_watch", "csv"]
    And schema enum order matches Rust enum order

  Scenario: Schema includes descriptions
    Given SourceType::Mqtt has doc comment "MQTT broker subscription"
    When I generate schema
    Then schema $.sources[].type has description mentioning "MQTT"
```

### AC-003: Unified Validation

```gherkin
Feature: NdpValidate Trait

  Scenario: StreamConfig validation
    Given a StreamConfig instance with invalid source_path
    When I call config.validate()
    Then I get ValidationError with code INVALID_SOURCE_PATH
    And error.path is "$.silver_etl.field_mappings[0].source_path"

  Scenario: DqRule validation
    Given a DqRule::RangeCheck with min > max
    When I call rule.validate()
    Then I get ValidationError with code INVALID_DQ_RULE
    And error.message contains "min must be less than max"
```

### AC-004: No Hardcoded Constants

```gherkin
Feature: Constant Elimination

  Scenario: Validator uses enum methods
    Given tools/ndp-validate/src/semantic/sources.rs
    Then file does NOT contain "const SUPPORTED_SOURCE_TYPES"
    And file contains "SourceType::all_names()"

  Scenario: Error messages use enum
    Given invalid source type "ftp" in config
    When validation runs
    Then error message contains all valid types from SourceType::all_names()
    And error message is dynamically generated, not hardcoded
```

---

## 8. Dependencies

### 8.1 Crate Dependencies

```toml
# crates/ndp-types/Cargo.toml
[package]
name = "ndp-types"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
schemars = "0.8"
strum = { version = "0.26", features = ["derive"] }
thiserror = "1.0"
```

### 8.2 Workspace Integration

```toml
# Cargo.toml (workspace root)
[workspace]
members = [
    "core",
    "apps/*",
    "tools/*",
    "crates/ndp-types",  # NEW
]

[workspace.dependencies]
ndp-types = { path = "crates/ndp-types" }
```

### 8.3 Consumer Updates

```toml
# core/Cargo.toml
[dependencies]
ndp-types = { workspace = true }

# tools/ndp-validate/Cargo.toml
[dependencies]
ndp-types = { workspace = true }
```

---

## 9. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Breaking changes during migration | Medium | High | Phased migration with re-exports |
| schemars limitations for complex types | Low | Medium | Manual schema patches if needed |
| Build time increase | Low | Low | Schema generation is fast |
| strum dependency conflicts | Low | Low | Use workspace dependency |
| Enum variant ordering affects schema | Low | Low | Alphabetical ordering convention |

---

## 10. Success Metrics

| Metric | Before | After | Measurement |
|--------|--------|-------|-------------|
| Places to update for new type | 4+ files | 1 file | Code review |
| Hardcoded type constants | 3 locations | 0 | grep for "SUPPORTED_" |
| Schema/Rust drift | Possible | Impossible | CI schema check |
| Validation code duplication | High | None | Unified trait |
| Compile-time type safety | Partial | Full | Compiler errors |

---

## 11. Timeline Estimate

| Phase | Duration | Dependencies |
|-------|----------|--------------|
| Phase 1: Create crate | 2-3 days | None |
| Phase 2: Migrate consumers | 3-4 days | Phase 1 |
| Phase 3: Remove duplicates | 1-2 days | Phase 2 |
| Schema generation tool | 1 day | Phase 1 |
| CI integration | 0.5 days | Phase 3 |
| **Total** | **8-10 days** | |

---

## 12. References

- **BUG-001**: `/workspaces/neural-data-platform/product/features/dp-019/bugs/BUG-001-validation-drift.md`
- **dp-019 Specification**: `/workspaces/neural-data-platform/product/features/dp-019/specification/SPECIFICATION.md`
- **ADR-019-001**: `/workspaces/neural-data-platform/product/features/dp-019/architecture/ADR-019-001-two-layer-validation.md`
- **Current runtime types**: `/workspaces/neural-data-platform/core/src/types/stream_config.rs`
- **Current Silver ETL types**: `/workspaces/neural-data-platform/core/src/config/silver_etl.rs`
- **Current validator**: `/workspaces/neural-data-platform/tools/ndp-validate/src/semantic/`
- **Current JSON Schema**: `/workspaces/neural-data-platform/schemas/stream-config.v1.1.schema.json`

---

*Specification created: 2026-02-02*
*SPARC Phase: Specification (S)*
*Bug Reference: BUG-001 Validation Drift Risk*
*Next: ADR-019-002 Unified Validation Types*
