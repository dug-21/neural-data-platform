# BUG-001: Validation Drift Risk - Codebase Analysis

**Analysis Date**: 2026-02-02
**Analyst**: NDP Rust Developer Agent
**Status**: Complete

---

## Executive Summary

This analysis identifies **every location** where validation-relevant types are defined or used across the NDP codebase. The goal is to support the fix for BUG-001 (Validation Drift Risk) by providing a complete inventory of type definitions that must be unified into a single source of truth.

### Key Findings

1. **THREE primary sources of truth** exist (as documented in BUG-001)
2. **14 distinct enum/constant definitions** were found that can drift
3. **Multiple discrepancies already exist** between sources
4. The JSON Schema has the **most comprehensive** DQ rule coverage
5. Runtime types in `core/` are **more restrictive** than validator constants

---

## 1. Source Type Definitions

### 1.1 Runtime Type (AUTHORITATIVE)

**File**: `/workspaces/neural-data-platform/core/src/types/stream_config.rs`
**Lines**: 183-191

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    Mqtt,
    HttpPoll,
    Webhook,
    FileWatch,
    Csv, // dp-013: CSV file source
}
```

**Serialized Values**: `mqtt`, `http_poll`, `webhook`, `file_watch`, `csv`

### 1.2 Validator Constant

**File**: `/workspaces/neural-data-platform/tools/ndp-validate/src/semantic/sources.rs`
**Line**: 14

```rust
const SUPPORTED_SOURCE_TYPES: &[&str] = &["mqtt", "http_poll", "webhook", "file_watch", "csv"];
```

**Status**: MATCHES runtime (5 types)

### 1.3 JSON Schema

**File**: `/workspaces/neural-data-platform/schemas/stream-config.v1.1.schema.json`
**Lines**: 141-144

```json
"type": {
  "type": "string",
  "enum": ["mqtt", "http_poll", "http_push", "file_watch"],
  "description": "Source type"
}
```

**DISCREPANCY FOUND**:
- Schema has `http_push` but runtime has `Webhook`
- Schema is **missing** `csv` source type
- Schema is **missing** `webhook` (uses `http_push` instead)

### 1.4 Import Locations (Consumers)

| File | Usage |
|------|-------|
| `core/src/lib.rs` | Re-exports `SourceType` |
| `core/src/types/mod.rs` | Module declaration |
| `apps/air-quality-app/src/coordinator/source_manager.rs` | Pattern matching on `SourceType` |
| `apps/air-quality-app/src/coordinator/router.rs` | Source routing |
| `config-client/src/stream/registry.rs` | Configuration parsing |

---

## 2. Field Type Definitions

### 2.1 Runtime Type (AUTHORITATIVE)

**File**: `/workspaces/neural-data-platform/core/src/types/stream_config.rs`
**Lines**: 31-39

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    Float,
    Int,
    String,
    Bool,
    Json,
}
```

**Serialized Values**: `float`, `int`, `string`, `bool`, `json`

### 2.2 JSON Schema

**File**: `/workspaces/neural-data-platform/schemas/stream-config.v1.1.schema.json`
**Lines**: 97-100

```json
"type": {
  "type": "string",
  "enum": ["float", "int", "string", "bool", "json"],
  "description": "Field data type"
}
```

**Status**: MATCHES runtime

### 2.3 Dimension Config Field Type

**File**: `/workspaces/neural-data-platform/core/src/types/dimension_config.rs`

Uses the same `FieldType` enum imported from `stream_config.rs`.

### 2.4 PostgreSQL Column Types (Silver Layer)

**File**: `/workspaces/neural-data-platform/core/src/config/silver_etl.rs`
**Lines**: 251-263

```rust
const VALID_TYPES: &[&str] = &[
    "double_precision",
    "real",
    "integer",
    "bigint",
    "smallint",
    "text",
    "varchar",
    "boolean",
    "timestamptz",
    "jsonb",
    "text[]",
];
```

### 2.5 JSON Schema Silver Field Types

**File**: `/workspaces/neural-data-platform/schemas/stream-config.v1.1.schema.json`
**Lines**: 622-624

```json
"type": {
  "type": "string",
  "enum": ["double_precision", "real", "smallint", "integer", "bigint", "text", "boolean", "jsonb", "timestamptz"]
}
```

**DISCREPANCY FOUND**:
- Runtime has `varchar` and `text[]` - Schema is missing these
- Schema has `smallint` - Runtime has `smallint` (matches)

---

## 3. DQ Rule Type Definitions

### 3.1 Validator Constants (Used by `ndp-validate`)

**File**: `/workspaces/neural-data-platform/tools/ndp-validate/src/semantic/dq_rules.rs`
**Lines**: 36-48

```rust
pub const SUPPORTED_DQ_RULES: &[&str] = &[
    "range_check",
    "null_check",
    "enum_check",
    "pattern_check",
    "freshness_check",
    "monotonic_check",
    "rate_of_change",
    "cross_field_check",
    "conditional_check",
    "completeness_check",
    "cardinality_check",
];
```

**Count**: 11 rule types

### 3.2 Runtime DqRule Enum (Silver ETL)

**File**: `/workspaces/neural-data-platform/core/src/config/silver_etl.rs`
**Lines**: 356-498

```rust
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "rule", rename_all = "snake_case")]
pub enum DqRule {
    RangeCheck { ... },
    NullCheck { ... },
    EnumCheck { ... },
    PatternCheck { ... },
    FreshnessCheck { ... },
    MonotonicCheck { ... },
    RateOfChange { ... },
    CrossFieldCheck { ... },
    ConditionalCheck { ... },
    CompletenessCheck { ... },
    CardinalityCheck { ... },
}
```

**Count**: 11 rule types
**Status**: MATCHES validator constants

### 3.3 JSON Schema Field-Level DQ Rules

**File**: `/workspaces/neural-data-platform/schemas/stream-config.v1.1.schema.json`
**Lines**: 718-720

```json
"rule": {
  "type": "string",
  "enum": ["range_check", "not_null", "enum_check", "regex_check", "length_check"]
}
```

**DISCREPANCY FOUND**:
- Schema uses `not_null` vs runtime `null_check`
- Schema uses `regex_check` vs runtime `pattern_check`
- Schema has `length_check` - NOT in runtime
- Schema missing: `freshness_check`, `monotonic_check`, `rate_of_change`, `conditional_check`, `completeness_check`, `cardinality_check`

### 3.4 JSON Schema Batch-Level DQ Rules

**File**: `/workspaces/neural-data-platform/schemas/stream-config.v1.1.schema.json`
**Lines**: 764-766

```json
"rule": {
  "type": "string",
  "enum": ["cross_field_check", "freshness_check", "rate_of_change", "completeness_check", "uniqueness_check", "referential_check"]
}
```

**DISCREPANCY FOUND**:
- Schema has `uniqueness_check` and `referential_check` - NOT in runtime
- Schema missing: `monotonic_check`, `conditional_check`, `cardinality_check`

---

## 4. DQ Action Definitions

### 4.1 Validator Constants

**File**: `/workspaces/neural-data-platform/tools/ndp-validate/src/semantic/dq_rules.rs`
**Line**: 51

```rust
pub const SUPPORTED_ACTIONS: &[&str] = &["flag", "reject", "clamp", "drop", "warn"];
```

### 4.2 Runtime DqAction Enum

**File**: `/workspaces/neural-data-platform/core/src/config/silver_etl.rs`
**Lines**: 596-610

```rust
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DqAction {
    #[default]
    Flag,
    Reject,
    Clamp,
    Drop,
    Warn,
}
```

**Status**: MATCHES validator constants

### 4.3 JSON Schema Field-Level Actions

**File**: `/workspaces/neural-data-platform/schemas/stream-config.v1.1.schema.json`
**Lines**: 747-750

```json
"action": {
  "type": "string",
  "enum": ["flag", "reject", "clamp", "nullify", "warn"],
  "default": "flag"
}
```

**DISCREPANCY FOUND**:
- Schema has `nullify` - runtime has `Drop` (different semantics)
- Schema missing: `drop`

### 4.4 JSON Schema Batch-Level Actions

**File**: `/workspaces/neural-data-platform/schemas/stream-config.v1.1.schema.json`
**Lines**: 818-821

```json
"action": {
  "type": "string",
  "enum": ["flag", "reject", "warn", "abort"],
  "default": "flag"
}
```

**DISCREPANCY FOUND**:
- Schema has `abort` - NOT in runtime
- Schema missing: `clamp`, `drop`

---

## 5. Transform Type Definitions

### 5.1 Runtime TransformConfig Enum

**File**: `/workspaces/neural-data-platform/core/src/config/silver_etl.rs`
**Lines**: 292-317

```rust
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TransformConfig {
    UnitConversion { from, to, formula },
    Expression { expression },
    Lookup { table },
    JsonExtract { path },
    Timestamp { format },
    Computed { depends_on, expression },
}
```

**Serialized Values**: `unit_conversion`, `expression`, `lookup`, `json_extract`, `timestamp`, `computed`

### 5.2 JSON Schema Transform Types

**File**: `/workspaces/neural-data-platform/schemas/stream-config.v1.1.schema.json`
**Lines**: 656-659

```json
"type": {
  "type": "string",
  "enum": ["unit_conversion", "scale", "offset", "expression", "lookup", "coalesce"]
}
```

**DISCREPANCY FOUND**:
- Schema has `scale`, `offset`, `coalesce` - NOT in runtime
- Schema missing: `json_extract`, `timestamp`, `computed`

---

## 6. Timestamp Transform Definitions

### 6.1 Runtime TimestampTransform Enum

**File**: `/workspaces/neural-data-platform/core/src/config/silver_etl.rs`
**Lines**: 191-202

```rust
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TimestampTransform {
    MicrosecondsToTimestamp,
    Iso8601,
    UnixSeconds,
    NwsDuration,
}
```

### 6.2 JSON Schema Timestamp Transforms

**File**: `/workspaces/neural-data-platform/schemas/stream-config.v1.1.schema.json`
**Lines**: 553-556

```json
"transform": {
  "type": "string",
  "enum": ["microseconds_to_timestamp", "milliseconds_to_timestamp", "seconds_to_timestamp", "iso8601_to_timestamp", "none"],
  "default": "none"
}
```

**DISCREPANCY FOUND**:
- Schema has `milliseconds_to_timestamp`, `seconds_to_timestamp`, `none` - NOT in runtime
- Schema uses `iso8601_to_timestamp` vs runtime `iso8601`
- Runtime has `nws_duration` - NOT in schema
- Runtime has `unix_seconds` vs schema `seconds_to_timestamp`

---

## 7. Other Enum Definitions

### 7.1 TimestampFormat (CSV Source)

**File**: `/workspaces/neural-data-platform/core/src/types/stream_config.rs`
**Lines**: 228-240

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TimestampFormat {
    #[default]
    Iso8601,
    EpochSeconds,
    EpochMillis,
    Custom(String),
}
```

### 7.2 OnError (CSV Source)

**File**: `/workspaces/neural-data-platform/core/src/types/stream_config.rs`
**Lines**: 243-253

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OnError {
    #[default]
    Skip,
    Fail,
    Log,
}
```

### 7.3 DeduplicationStrategy

**File**: `/workspaces/neural-data-platform/core/src/config/silver_etl.rs`
**Lines**: 688-698

```rust
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DeduplicationStrategy {
    #[default]
    Upsert,
    Skip,
    Replace,
}
```

**JSON Schema** (lines 899-902):
```json
"strategy": {
  "type": "string",
  "enum": ["upsert", "skip", "last_wins", "first_wins"],
  "default": "upsert"
}
```

**DISCREPANCY FOUND**:
- Schema has `last_wins`, `first_wins` - NOT in runtime
- Runtime has `Replace` - NOT in schema

### 7.4 MonotonicDirection

**File**: `/workspaces/neural-data-platform/core/src/config/silver_etl.rs`
**Lines**: 587-593

```rust
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MonotonicDirection {
    Increasing,
    Decreasing,
    StrictIncreasing,
}
```

### 7.5 Partitioning Strategy

**File**: `/workspaces/neural-data-platform/schemas/stream-config.v1.1.schema.json`
**Lines**: 48-52

```json
"partitioning_strategy": {
  "type": "string",
  "enum": ["daily", "hourly", "monthly"],
  "default": "daily"
}
```

**Runtime** (`stream_config.rs` line 441):
```rust
pub partitioning_strategy: String,  // Just a String, no enum!
```

**DISCREPANCY FOUND**: Runtime has no validation; schema constrains to 3 values.

---

## 8. Validation Error Codes

**File**: `/workspaces/neural-data-platform/tools/ndp-validate/src/error.rs`
**Lines**: 36-86

```rust
pub enum ErrorCode {
    // Layer 1: Syntax
    SyntaxError,
    // Layer 1: Schema
    MissingRequired, InvalidType, UnknownField, PatternMismatch, EnumViolation, ArrayBounds,
    // Layer 2: Semantic - Types (300-319)
    InvalidFieldType, InvalidSourceType, InvalidRange, InvalidPrecision,
    // Layer 2: Semantic - Cross-Reference (320-339)
    InvalidSourcePath, DuplicateName, ConstraintViolation,
    // Layer 2: Semantic - External (340-359)
    TableNotFound, ColumnNotFound, TypeMismatch, TableCheckFailed, ColumnCheckFailed, InvalidTableFormat,
    // Layer 2: Semantic - Source Config (360-379)
    MissingSourceConfig, InvalidSourceConfig,
    // Layer 2: Semantic - DQ Rules (380-399)
    InvalidDqRuleType, InvalidDqRule, InvalidDqAction, InvalidDqColumn, InvalidDqSyntax, InvalidRegex, InvalidInterval, InvalidTransform,
    // Warnings (900-999)
    UnknownDeviceClass,
}
```

---

## 9. Complete Discrepancy Summary

| Type Category | Location 1 | Location 2 | Discrepancy |
|---------------|-----------|-----------|-------------|
| Source Types | Runtime (5) | Schema (4) | Schema missing `csv`, `webhook`; has `http_push` |
| Field Types (Bronze) | Runtime (5) | Schema (5) | **MATCH** |
| Field Types (Silver) | Runtime (11) | Schema (9) | Schema missing `varchar`, `text[]` |
| DQ Rule Types | Runtime (11) | Validator (11) | **MATCH** |
| DQ Rule Types | Runtime (11) | Schema Field (5) | Schema uses different names, missing 6 types |
| DQ Rule Types | Runtime (11) | Schema Batch (6) | Schema has 2 extras, missing 5 |
| DQ Actions | Runtime (5) | Validator (5) | **MATCH** |
| DQ Actions | Runtime (5) | Schema Field (5) | `drop` vs `nullify` |
| DQ Actions | Runtime (5) | Schema Batch (4) | Schema has `abort`, missing `clamp`, `drop` |
| Transform Types | Runtime (6) | Schema (6) | 3 different each way |
| Timestamp Transforms | Runtime (4) | Schema (5) | Naming differences, missing types |
| Deduplication Strategy | Runtime (3) | Schema (4) | Different strategies |
| Partitioning Strategy | Runtime (String) | Schema (3) | Runtime has no validation |

---

## 10. Dependency Graph

```
┌─────────────────────────────────────────────────────────────────────────┐
│  CURRENT ARCHITECTURE (MULTIPLE SOURCES OF TRUTH)                       │
└─────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────┐
│  RUNTIME TYPES (core/)                                                   │
│  ├── stream_config.rs                                                    │
│  │   ├── SourceType enum                                                 │
│  │   ├── FieldType enum                                                  │
│  │   ├── TimestampFormat enum                                            │
│  │   └── OnError enum                                                    │
│  └── silver_etl.rs                                                       │
│      ├── DqRule enum                                                     │
│      ├── DqAction enum                                                   │
│      ├── TransformConfig enum                                            │
│      ├── TimestampTransform enum                                         │
│      ├── DeduplicationStrategy enum                                      │
│      └── MonotonicDirection enum                                         │
└───────────────────────────────┬─────────────────────────────────────────┘
                                │ IMPORTS
                                ▼
┌─────────────────────────────────────────────────────────────────────────┐
│  CONSUMERS                                                               │
│  ├── core/src/lib.rs (re-exports)                                        │
│  ├── apps/air-quality-app/ (uses SourceType)                             │
│  ├── apps/silver-etl/ (uses DqRule, TransformConfig)                     │
│  ├── config-client/ (deserializes StreamConfig)                          │
│  └── core/ndp-mcp-server/ (field mapping comparison)                     │
└─────────────────────────────────────────────────────────────────────────┘

                    NO CONNECTION (CAN DRIFT!)
                              ↓
┌─────────────────────────────────────────────────────────────────────────┐
│  VALIDATOR CONSTANTS (tools/ndp-validate/)                               │
│  ├── sources.rs                                                          │
│  │   └── SUPPORTED_SOURCE_TYPES: &[&str]                                 │
│  └── dq_rules.rs                                                         │
│      ├── SUPPORTED_DQ_RULES: &[&str]                                     │
│      └── SUPPORTED_ACTIONS: &[&str]                                      │
└─────────────────────────────────────────────────────────────────────────┘

                    NO CONNECTION (CAN DRIFT!)
                              ↓
┌─────────────────────────────────────────────────────────────────────────┐
│  JSON SCHEMA (schemas/)                                                  │
│  └── stream-config.v1.1.schema.json                                      │
│      ├── source.type enum                                                │
│      ├── field.type enum                                                 │
│      ├── field_dq_rule.rule enum                                         │
│      ├── dq_rule.rule enum                                               │
│      ├── field_dq_rule.action enum                                       │
│      ├── dq_rule.action enum                                             │
│      ├── field_transform.type enum                                       │
│      ├── timestamp_mapping.transform enum                                │
│      ├── deduplication.strategy enum                                     │
│      └── partitioning_strategy enum                                      │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 11. Recommended Refactoring Order

Based on the analysis, the following refactoring order minimizes risk and maximizes impact:

### Phase 1: High Impact, Low Risk
1. **SourceType** - 3 locations, clear discrepancy with schema
2. **FieldType** - Already matches, just needs consolidation
3. **DqAction** - Already matches, just needs consolidation

### Phase 2: High Impact, Medium Complexity
4. **DqRule** - 11 types, runtime and validator match but schema differs
5. **TransformConfig** - 6 types, significant differences

### Phase 3: Medium Impact
6. **TimestampTransform** - 4-5 types, naming differences
7. **DeduplicationStrategy** - 3-4 types
8. **MonotonicDirection** - Already only in runtime

### Phase 4: Low Priority
9. **TimestampFormat** - CSV-specific
10. **OnError** - CSV-specific
11. **PartitioningStrategy** - Runtime needs enum, not String

---

## 12. Files Requiring Modification

### Must Change (Type Definition Sources)
1. `core/src/types/stream_config.rs` - Move to `ndp-types`
2. `core/src/config/silver_etl.rs` - Move DQ/Transform types to `ndp-types`
3. `tools/ndp-validate/src/semantic/sources.rs` - Import from `ndp-types`
4. `tools/ndp-validate/src/semantic/dq_rules.rs` - Import from `ndp-types`
5. `schemas/stream-config.v1.1.schema.json` - GENERATE from `ndp-types`

### Must Update (Consumers)
6. `core/src/lib.rs` - Re-export from `ndp-types`
7. `core/src/types/mod.rs` - Import from `ndp-types`
8. `apps/air-quality-app/src/coordinator/source_manager.rs`
9. `apps/air-quality-app/src/coordinator/router.rs`
10. `apps/silver-etl/src/etl.rs`
11. `apps/silver-etl/src/dq.rs`
12. `config-client/src/stream/registry.rs`

---

## 13. Test Coverage Gaps

Based on the analysis, the following tests should be added after unification:

1. **Round-trip tests**: Serialize Rust enum -> JSON -> deserialize, verify match
2. **Schema generation tests**: Verify generated schema matches runtime types
3. **Exhaustive enum tests**: Ensure all enum variants are tested
4. **Cross-boundary tests**: Validate config with `ndp-validate`, run through runtime

---

*Analysis complete. This document provides the foundation for designing the `ndp-types` crate that unifies all validation-relevant types.*
