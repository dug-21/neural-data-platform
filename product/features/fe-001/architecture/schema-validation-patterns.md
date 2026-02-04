# Schema Validation Patterns Analysis for Gold Layer

> **Created:** 2026-02-03
> **Author:** NDP Architect Agent
> **Purpose:** Analysis of existing schema validation architecture for Gold layer planning (fe-001)
> **Related ADR:** ADR-019-001 (Two-Layer Validation)

---

## Executive Summary

This document analyzes the existing NDP schema validation architecture to inform Gold layer schema design for fe-001. The platform uses a **two-layer validation pattern** established by dp-019:

1. **Layer 1 (JSON Schema)**: Declarative structural validation
2. **Layer 2 (Rust Code)**: Semantic validation with cross-references

**Key Finding**: The existing validation infrastructure is well-designed for extension. Gold layer schemas should follow the established patterns with minimal architectural changes.

---

## Current Schema Validation Architecture

### Existing JSON Schemas

Location: `/workspaces/neural-data-platform/schemas/`

| Schema File | Purpose | Version | Status |
|-------------|---------|---------|--------|
| `stream-config.v1.1.schema.json` | Stream configuration (enriched fields) | 1.1 | Current |
| `stream-config.v2.schema.json` | Stream configuration (v2, requires description) | 2.0 | Draft |
| `manifest.schema.json` | Deployment manifest declarations | 1.0 | Current |

### Schema Structure Patterns

All existing schemas follow consistent patterns:

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://neural-data-platform.local/schemas/{name}.schema.json",
  "title": "{Human Title}",
  "description": "{Description}",
  "type": "object",
  "required": [...],
  "additionalProperties": false,
  "properties": { ... },
  "$defs": { ... }
}
```

**Key Conventions:**
- Draft 2020-12 JSON Schema standard
- `$defs` for reusable definitions (not `definitions`)
- `additionalProperties: false` to catch typos
- Required fields explicitly listed
- Pattern validation for identifiers (kebab-case, snake_case)
- Enum validation for fixed value sets

### Validation Code Architecture

Location: `/workspaces/neural-data-platform/tools/ndp-validate/`

```
tools/ndp-validate/
├── src/
│   ├── main.rs           # CLI entry point
│   ├── lib.rs            # Library exports
│   ├── schema.rs         # Layer 1: JSON Schema validation
│   ├── schema_gen.rs     # Schema generation from Rust types
│   ├── error.rs          # Error types and codes
│   ├── cli.rs            # CLI argument handling
│   └── semantic/         # Layer 2: Semantic validation
│       ├── mod.rs        # SemanticValidator coordinator
│       ├── sources.rs    # Source config validation
│       ├── source_path.rs # Cross-reference validation
│       ├── table_exists.rs # Database table checks
│       └── dq_rules.rs   # DQ rule syntax validation
└── Cargo.toml
```

### Two-Layer Validation Flow

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

## Validation Components Deep Dive

### SchemaValidator (Layer 1)

**File:** `/workspaces/neural-data-platform/tools/ndp-validate/src/schema.rs`

**Key Methods:**
```rust
impl SchemaValidator {
    /// Create validator from schema file
    pub fn from_file(path: &Path) -> Result<Self, SchemaValidatorError>;

    /// Create validator using embedded default schema
    pub fn default_schema() -> Result<Self, SchemaValidatorError>;

    /// Validate JSON syntax (returns parsed Value or error)
    pub fn validate_json_syntax(content: &str) -> Result<Value, ValidationError>;

    /// Validate against schema
    pub fn validate_schema(&self, instance: &Value) -> Vec<ValidationError>;

    /// Combined syntax + schema validation
    pub fn validate(&self, content: &str) -> Vec<ValidationError>;
}
```

**Error Mapping:**
```rust
let code = match error.kind {
    jsonschema::error::ValidationErrorKind::Required { .. } => ErrorCode::MissingRequired,
    jsonschema::error::ValidationErrorKind::Type { .. } => ErrorCode::InvalidType,
    jsonschema::error::ValidationErrorKind::Enum { .. } => ErrorCode::EnumViolation,
    jsonschema::error::ValidationErrorKind::Pattern { .. } => ErrorCode::PatternMismatch,
    jsonschema::error::ValidationErrorKind::AdditionalProperties { .. } => ErrorCode::UnknownField,
    ...
};
```

**Typo Suggestions:**
The validator includes smart suggestions for common typos:
```rust
let corrections = [
    ("silver_elt", "silver_etl"),
    ("field_mapings", "field_mappings"),
    ("temperture", "temperature"),
    ...
];
```

### SemanticValidator (Layer 2)

**File:** `/workspaces/neural-data-platform/tools/ndp-validate/src/semantic/mod.rs`

**Validation Rules:**
| Rule | Function | Purpose |
|------|----------|---------|
| FR-020 | `validate_sources()` | Source type + required fields per type |
| FR-022 | `validate_source_paths()` | `source_path` references valid field |
| FR-023 | `validate_table_exists()` | Silver target table exists (optional) |
| DQ | `validate_dq_rules()` | DQ rule syntax + column references |

**Cross-Reference Pattern:**
```rust
// Extract field names from config
let field_names: HashSet<String> = config
    .get("fields")
    .and_then(|v| v.as_array())
    .map(|fields| {
        fields.iter()
            .filter_map(|f| f.get("name").and_then(|n| n.as_str()))
            .map(|s| s.to_string())
            .collect()
    })
    .unwrap_or_default();

// Validate source_path references
errors.extend(validate_source_paths(&field_names, &field_mappings));
```

### Error Types

**File:** `/workspaces/neural-data-platform/tools/ndp-validate/src/error.rs`

**Validation Layers:**
```rust
pub enum ValidationLayer {
    Syntax,    // JSON syntax errors
    Schema,    // JSON Schema validation
    Semantic,  // Application-level rules
}
```

**Error Codes (partial list):**
```rust
pub enum ErrorCode {
    // Layer 1: Syntax
    SyntaxError,

    // Layer 1: Schema
    MissingRequired,
    InvalidType,
    UnknownField,
    PatternMismatch,
    EnumViolation,
    ArrayBounds,

    // Layer 2: Semantic - Types
    InvalidFieldType,
    InvalidSourceType,
    InvalidRange,

    // Layer 2: Semantic - Cross-Reference
    InvalidSourcePath,
    DuplicateName,

    // Layer 2: Semantic - External
    TableNotFound,
    ColumnNotFound,

    // Layer 2: Semantic - DQ
    InvalidDqRuleType,
    InvalidDqRule,
    InvalidDqSyntax,
    ...
}
```

**Structured Error Output:**
```rust
pub struct ValidationError {
    pub layer: ValidationLayer,
    pub code: ErrorCode,
    pub path: String,           // JSONPath (e.g., "$.fields[0].type")
    pub message: String,
    pub severity: Severity,
    pub suggestion: Option<String>,
    pub context: Option<serde_json::Value>,
}
```

### Schema Generation

**File:** `/workspaces/neural-data-platform/tools/ndp-validate/src/schema_gen.rs`

The platform generates JSON schemas from Rust types using `schemars`:

```rust
use schemars::{schema_for, JsonSchema};

/// Generate JSON Schema from ndp-types
pub fn generate_schema() -> SchemaGenResult<String> {
    let root_schema = schema_for!(NdpTypesSchema);
    // ... add metadata, return JSON
}

/// Verify committed schema matches generated
pub fn verify_schema(path: &Path) -> SchemaGenResult<bool> {
    // ... compare existing vs generated
}
```

**Why This Matters for Gold:** Schema generation ensures enum values stay in sync between Rust types and JSON Schema.

---

## Validation Pipeline Integration Points

### When Validation Runs

| Trigger | Mode | DB Required | Exit Code |
|---------|------|-------------|-----------|
| `deploy.sh` | Full (schema + semantic) | Yes (for table checks) | Non-zero blocks deploy |
| Pre-commit | Schema-only | No | Non-zero blocks commit |
| App startup | Full | Yes | Fails loudly |
| MCP tools | Schema-only | No | Returns errors to caller |

### CLI Interface

```bash
# Validate single config (full)
ndp-validate config/base/streams/air-quality/config.json

# Schema-only mode (fast, no DB)
ndp-validate --schema-only config.json

# Full validation with table checks
ndp-validate --all --check-tables

# Generate schema from Rust types
ndp-validate --generate-schema --output schemas/ndp-types.schema.json

# Verify committed schema matches generated
ndp-validate --verify-schema schemas/stream-config.v1.1.schema.json
```

### Adding New Schemas to Pipeline

1. **Create schema file:** `schemas/{name}.schema.json`
2. **Add to validation:**
   - Option A: Use `SchemaValidator::from_file(path)` directly
   - Option B: Embed in code via `default_{name}_schema()` function
3. **Register for CI:** Add to `deploy.sh` validation calls
4. **Document:** Update data dictionary

---

## Recommendations for Gold Layer Schemas

### Proposed New Schemas

Based on fe-001 SCOPE.md and the Gold Layer Roadmap:

| Schema | Purpose | Priority |
|--------|---------|----------|
| `gold-etl.schema.json` | Gold ETL config section in stream configs | Critical |
| `alignment.schema.json` | Cross-stream alignment configuration | Critical |
| `objectives.schema.json` | Objectives and targets declaration | High |

### gold-etl.schema.json Design

**Location:** `schemas/gold-etl.schema.json`

**Recommended Structure:**
```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://neural-data-platform.local/schemas/gold-etl.schema.json",
  "title": "Gold ETL Configuration",
  "description": "Configuration for Silver-to-Gold transformation",
  "type": "object",
  "properties": {
    "gold_etl": {
      "type": "object",
      "properties": {
        "enabled": { "type": "boolean", "default": true },
        "aggregates": { "$ref": "#/$defs/aggregates" },
        "features": { "$ref": "#/$defs/features" },
        "transitions": { "$ref": "#/$defs/transitions" }
      }
    }
  },
  "$defs": {
    "aggregates": {
      "type": "object",
      "properties": {
        "granularities": {
          "type": "array",
          "items": {
            "type": "string",
            "pattern": "^\\d+ (hour|day|minute)s?$"
          }
        },
        "fields": {
          "type": "object",
          "additionalProperties": {
            "$ref": "#/$defs/field_aggregates"
          }
        }
      }
    },
    "field_aggregates": {
      "type": "object",
      "properties": {
        "metrics": {
          "type": "array",
          "items": {
            "enum": ["mean", "std", "min", "max", "count", "p95", "p99", "sum"]
          }
        }
      }
    },
    "features": {
      "type": "object",
      "properties": {
        "lag": { "$ref": "#/$defs/lag_feature" },
        "rolling": { "$ref": "#/$defs/rolling_feature" },
        "trend": { "$ref": "#/$defs/trend_feature" }
      }
    },
    "lag_feature": {
      "type": "object",
      "properties": {
        "enabled": { "type": "boolean", "default": true },
        "lags_hours": {
          "type": "array",
          "items": { "type": "integer", "minimum": 1 }
        },
        "fields": {
          "type": "array",
          "items": { "type": "string" }
        }
      }
    },
    "rolling_feature": {
      "type": "object",
      "properties": {
        "enabled": { "type": "boolean", "default": true },
        "windows": {
          "type": "array",
          "items": {
            "type": "string",
            "pattern": "^\\d+ (hour|day|minute)s?$"
          }
        },
        "stats": {
          "type": "array",
          "items": { "enum": ["mean", "std", "min", "max"] }
        },
        "fields": {
          "type": "array",
          "items": { "type": "string" }
        }
      }
    },
    "trend_feature": {
      "type": "object",
      "properties": {
        "enabled": { "type": "boolean", "default": true },
        "window": {
          "type": "string",
          "pattern": "^\\d+ (hour|day|minute)s?$"
        },
        "fields": {
          "type": "array",
          "items": { "type": "string" }
        }
      }
    },
    "transitions": {
      "type": "object",
      "description": "State transition extraction for state_event streams",
      "properties": {
        "enabled": { "type": "boolean", "default": true },
        "state_field": {
          "type": "string",
          "description": "Field containing state value"
        },
        "entity_field": {
          "type": "string",
          "description": "Field identifying entity (e.g., ndp_id)"
        },
        "track_duration": {
          "type": "boolean",
          "default": true,
          "description": "Track duration in previous state"
        },
        "include_in_alignment": {
          "type": "boolean",
          "default": true,
          "description": "Include transition counts in aligned view"
        }
      }
    }
  }
}
```

### alignment.schema.json Design

**Location:** `schemas/alignment.schema.json`

**Recommended Structure:**
```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://neural-data-platform.local/schemas/alignment.schema.json",
  "title": "Cross-Stream Alignment Configuration",
  "description": "Configuration for aligning multiple streams in Gold layer",
  "type": "object",
  "required": ["view_name", "granularity", "streams"],
  "properties": {
    "enabled": { "type": "boolean", "default": true },
    "view_name": {
      "type": "string",
      "pattern": "^[a-z][a-z0-9_]*$",
      "description": "Name of the aligned view (e.g., 'aligned_hourly')"
    },
    "granularity": {
      "type": "string",
      "pattern": "^\\d+ (hour|day|minute)s?$",
      "description": "Time bucket granularity"
    },
    "streams": {
      "type": "array",
      "minItems": 2,
      "items": {
        "$ref": "#/$defs/stream_reference"
      }
    },
    "join_strategy": {
      "type": "string",
      "enum": ["full_outer", "inner", "left"],
      "default": "full_outer"
    },
    "null_handling": {
      "type": "string",
      "enum": ["preserve", "interpolate", "forward_fill"],
      "default": "preserve"
    }
  },
  "$defs": {
    "stream_reference": {
      "type": "object",
      "required": ["stream_id"],
      "properties": {
        "stream_id": {
          "type": "string",
          "pattern": "^[a-z][a-z0-9-]*$"
        },
        "alias": {
          "type": "string",
          "pattern": "^[a-z][a-z0-9_]*$",
          "description": "Column prefix in aligned view"
        },
        "include_fields": {
          "type": "array",
          "items": { "type": "string" },
          "description": "Specific fields to include (default: all)"
        },
        "exclude_fields": {
          "type": "array",
          "items": { "type": "string" },
          "description": "Fields to exclude"
        }
      }
    }
  }
}
```

### objectives.schema.json Design

**Location:** `schemas/objectives.schema.json`

**Recommended Structure:**
```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://neural-data-platform.local/schemas/objectives.schema.json",
  "title": "Objectives Configuration",
  "description": "Declarative objectives with targets for pattern filtering",
  "type": "object",
  "required": ["objectives"],
  "properties": {
    "objectives": {
      "type": "array",
      "items": { "$ref": "#/$defs/objective" }
    }
  },
  "$defs": {
    "objective": {
      "type": "object",
      "required": ["id", "targets"],
      "properties": {
        "id": {
          "type": "string",
          "pattern": "^[a-z][a-z0-9_]*$"
        },
        "description": { "type": "string" },
        "targets": {
          "type": "array",
          "minItems": 1,
          "items": { "$ref": "#/$defs/target" }
        },
        "constraints": {
          "type": "array",
          "items": { "$ref": "#/$defs/constraint" }
        }
      }
    },
    "target": {
      "type": "object",
      "required": ["stream", "metric", "condition", "threshold"],
      "properties": {
        "stream": {
          "type": "string",
          "pattern": "^[a-z][a-z0-9-]*$"
        },
        "metric": { "type": "string" },
        "condition": {
          "enum": ["<", ">", "<=", ">=", "==", "between"]
        },
        "threshold": {
          "oneOf": [
            { "type": "number" },
            {
              "type": "array",
              "items": { "type": "number" },
              "minItems": 2,
              "maxItems": 2
            }
          ]
        },
        "unit": { "type": "string" },
        "priority": {
          "enum": ["high", "medium", "low"],
          "default": "medium"
        }
      }
    },
    "constraint": {
      "type": "object",
      "properties": {
        "description": { "type": "string" },
        "stream": { "type": "string" },
        "metric": { "type": "string" },
        "condition": { "type": "string" },
        "threshold": { "type": "number" }
      }
    }
  }
}
```

---

## Integration with Existing Validation Pipeline

### Option 1: Extend Existing Schema (Recommended)

Add `gold_etl` to `stream-config.v2.schema.json`:

```json
{
  "properties": {
    ...existing...,
    "gold_etl": { "$ref": "gold-etl.schema.json" }
  }
}
```

**Advantages:**
- Single validation call for stream configs
- Consistent error reporting
- Existing CLI works unchanged

**Implementation:**
1. Create `gold-etl.schema.json` as standalone
2. Reference via `$ref` in stream config schema
3. Semantic validator extended for Gold-specific rules

### Option 2: Standalone Schemas

Keep Gold schemas separate, validate independently.

**Advantages:**
- Independent versioning
- Cleaner separation

**Disadvantages:**
- Multiple validation calls
- Must ensure consistency manually

### Recommended Semantic Validations for Gold

New semantic validation rules needed:

| Rule | Purpose | Implementation |
|------|---------|----------------|
| `gold_field_exists` | `gold_etl.aggregates.fields.*` references valid field in `fields[]` | HashSet lookup |
| `stream_type_matches` | `transitions` only valid for `stream_type: "state_event"` | Conditional check |
| `alignment_stream_exists` | All `alignment.streams[].stream_id` exist in registry | etcd lookup |
| `objective_stream_exists` | All `objectives[].targets[].stream` exist in registry | etcd lookup |
| `aggregate_view_exists` | Gold aggregate view exists in TimescaleDB | SQL query (optional) |

---

## Error Codes for Gold Validation

Recommended additions to `ErrorCode` enum:

```rust
// Layer 2: Semantic - Gold ETL (400-419)
InvalidGoldField,           // Field in gold_etl not in fields[]
InvalidStreamType,          // transitions on non-state_event stream
InvalidGranularity,         // Unparseable granularity string
InvalidFeatureConfig,       // Feature misconfigured

// Layer 2: Semantic - Alignment (420-439)
UnknownAlignmentStream,     // Stream in alignment not in registry
DuplicateAlias,             // Same alias used twice
InvalidJoinStrategy,        // Unknown join strategy

// Layer 2: Semantic - Objectives (440-459)
UnknownObjectiveStream,     // Stream in objective not in registry
UnknownObjectiveMetric,     // Metric not in stream's fields[]
InvalidThresholdFormat,     // Threshold doesn't match condition
```

---

## Typo Suggestions for Gold Schemas

Add to `suggest_field_correction()`:

```rust
let gold_corrections = [
    ("gold_elt", "gold_etl"),
    ("agregates", "aggregates"),
    ("granulairty", "granularity"),
    ("graularities", "granularities"),
    ("features", "features"),  // common misspelling
    ("trasitions", "transitions"),
    ("alignement", "alignment"),
    ("objetives", "objectives"),
    ("threshhold", "threshold"),
    ("taget", "target"),
];
```

---

## Implementation Roadmap

### Phase 1: Schema Definition (Week 1)

1. Create `schemas/gold-etl.schema.json`
2. Create `schemas/alignment.schema.json`
3. Create `schemas/objectives.schema.json`
4. Add `$ref` to stream-config schema
5. Test with example configs

### Phase 2: Semantic Validation (Week 2)

1. Add new `ErrorCode` variants
2. Implement `validate_gold_etl()` in `semantic/` module
3. Implement `validate_alignment()` in `semantic/` module
4. Implement `validate_objectives()` in `semantic/` module
5. Add typo suggestions

### Phase 3: Integration (Week 3)

1. Update `SemanticValidator::validate()` to call new rules
2. Update CLI help text
3. Add schema verification to CI
4. Document new schemas in data dictionary

---

## References

### Related ADRs
- [ADR-019-001: Two-Layer Config Validation](../dp-019/architecture/ADR-019-001-two-layer-validation.md)
- [ADR-016-001: JSON Config Source of Truth](../dp-016/architecture/ADR-016-001-config-source-of-truth.md)
- [ADR-018-001: Config Loader Design](../dp-018/architecture/ADR-018-001-config-loader-design.md)

### Existing Patterns (AgentDB)
- Pattern ID 1: `architecture:two-layer-validation` (90% success rate)
- Pattern ID 12: `pseudocode:config-validation-pipeline` (95% success rate)
- Pattern ID 14: `implementation:london-tdd-schema-validation` (95% success rate)

### Code Locations
- Schema files: `/workspaces/neural-data-platform/schemas/`
- Validator: `/workspaces/neural-data-platform/tools/ndp-validate/src/`
- Error types: `/workspaces/neural-data-platform/tools/ndp-validate/src/error.rs`

---

## Summary

The NDP schema validation infrastructure is well-architected for extension:

1. **Two-layer pattern proven**: Schema + Semantic validation works well
2. **Error reporting mature**: Structured errors with JSONPath, suggestions, severity
3. **Extensible**: Add new schemas via `$ref`, new semantic rules via modules
4. **CI integrated**: Schema verification prevents drift

**Recommendation**: Follow existing patterns exactly. Create standalone schema files, reference from stream config, add semantic validation modules. No architectural changes needed.

---

*Analysis completed: 2026-02-03*
*Feature: fe-001 (Gold Layer Foundation)*
