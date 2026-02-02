# BUG-001-fix: Unified Validation Types - SPARC Pseudocode

**Document Type**: SPARC Pseudocode (Phase P)
**Bug Reference**: BUG-001 Validation Drift Risk
**Feature**: dp-019 Config Validation Pipeline Extension
**Version**: 1.0
**Date**: 2026-02-02
**Status**: Proposed

---

## 1. ndp-types Crate Structure

### 1.1 Module Organization

```
CRATE: ndp-types
VERSION: 0.1.0
EDITION: 2021

MODULES:
    lib.rs              // Public re-exports
    source_type.rs      // SourceType enum
    field_type.rs       // FieldType (Bronze), SilverFieldType
    dq_rule.rs          // DqRule, DqRuleType, DqAction, MonotonicDirection
    transform.rs        // TransformConfig, TimestampTransform, ConversionFormula
    strategy.rs         // DeduplicationStrategy, PartitioningStrategy
    csv_source.rs       // OnError, TimestampFormat (CSV-specific)
    validate.rs         // NdpValidate trait, ValidationError, ValidationContext
    error.rs            // ErrorCode enum, error formatting
```

### 1.2 lib.rs Re-exports

```rust
ALGORITHM: lib_reexports
PURPOSE: Provide single import point for all types

// File: crates/ndp-types/src/lib.rs

// Core types - CRITICAL priority
pub use source_type::SourceType;
pub use field_type::{FieldType, SilverFieldType};

// DQ types - HIGH priority
pub use dq_rule::{DqRule, DqRuleType, DqAction, MonotonicDirection};

// Transform types - MEDIUM priority
pub use transform::{TransformConfig, TimestampTransform, ConversionFormula};

// Strategy types - LOW priority
pub use strategy::{DeduplicationStrategy, PartitioningStrategy};

// CSV-specific types
pub use csv_source::{OnError, TimestampFormat};

// Validation framework
pub use validate::{NdpValidate, ValidationError, ValidationContext, ValidationLayer, Severity};
pub use error::ErrorCode;

// Module declarations
mod source_type;
mod field_type;
mod dq_rule;
mod transform;
mod strategy;
mod csv_source;
mod validate;
mod error;
```

---

## 2. Type Definitions with Derives

### 2.1 Standard Enum Pattern

All enums follow this derivation pattern for maximum flexibility:

```rust
ALGORITHM: standard_enum_pattern
INPUT: enum_name (string), variants (list of variant definitions)
OUTPUT: Rust enum with full derive set

PATTERN:
    use serde::{Deserialize, Serialize};
    use schemars::JsonSchema;
    use strum::{EnumIter, EnumString, Display, AsRefStr, VariantNames};

    /// [Documentation for enum]
    #[derive(
        // Core traits
        Debug, Clone, Copy, PartialEq, Eq, Hash,
        // Serde for JSON serialization/deserialization
        Serialize, Deserialize,
        // Schemars for JSON Schema generation
        JsonSchema,
        // Strum for enum iteration and string conversion
        EnumIter,      // impl Iterator over variants
        EnumString,    // impl FromStr
        Display,       // impl Display
        AsRefStr,      // fn as_ref() -> &'static str
        VariantNames,  // const VARIANTS: &[&str]
    )]
    #[serde(rename_all = "snake_case")]
    #[strum(serialize_all = "snake_case")]
    pub enum {enum_name} {
        /// [Doc comment becomes JSON Schema description]
        Variant1,
        /// [Doc comment]
        Variant2,
        // ... more variants
    }

    impl {enum_name} {
        /// Returns all variant names as strings (for error messages)
        pub fn all_names() -> &'static [&'static str] {
            Self::VARIANTS
        }

        /// Returns iterator over all variants
        pub fn all() -> impl Iterator<Item = Self> {
            use strum::IntoEnumIterator;
            Self::iter()
        }
    }
```

### 2.2 SourceType Definition

```rust
ALGORITHM: source_type_definition
FILE: crates/ndp-types/src/source_type.rs

use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use strum::{EnumIter, EnumString, Display, AsRefStr, VariantNames};

/// Data source types supported by NDP.
///
/// Each variant corresponds to a specific data ingestion pattern
/// for the Bronze layer.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash,
    Serialize, Deserialize, JsonSchema,
    EnumIter, EnumString, Display, AsRefStr, VariantNames,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum SourceType {
    /// MQTT broker subscription for real-time sensor data
    Mqtt,
    /// HTTP endpoint polling for periodic data fetches
    HttpPoll,
    /// HTTP webhook receiver for push-based data delivery
    Webhook,
    /// File system watcher for local file ingestion
    FileWatch,
    /// CSV file import for batch data loading
    Csv,
}

impl SourceType {
    /// Returns all source type names for error messages.
    ///
    /// # Example
    /// ```
    /// let names = SourceType::all_names();
    /// // ["mqtt", "http_poll", "webhook", "file_watch", "csv"]
    /// ```
    pub fn all_names() -> &'static [&'static str] {
        Self::VARIANTS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialization_roundtrip() {
        FOR EACH variant IN SourceType::all() DO
            serialized <- serde_json::to_string(variant)
            deserialized <- serde_json::from_str(serialized)
            ASSERT variant == deserialized
        END FOR
    }

    #[test]
    fn test_all_names_count() {
        ASSERT SourceType::all_names().len() == 5
    }
}
```

### 2.3 FieldType Definitions

```rust
ALGORITHM: field_type_definition
FILE: crates/ndp-types/src/field_type.rs

use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use strum::{EnumIter, EnumString, Display, AsRefStr, VariantNames};

/// Field data types for Bronze layer schema definitions.
///
/// These types represent the logical data types before
/// transformation to Silver layer PostgreSQL types.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash,
    Serialize, Deserialize, JsonSchema,
    EnumIter, EnumString, Display, AsRefStr, VariantNames,
)]
#[serde(rename_all = "lowercase")]  // Note: lowercase for FieldType
#[strum(serialize_all = "lowercase")]
pub enum FieldType {
    /// 64-bit floating point number
    Float,
    /// 64-bit signed integer
    Int,
    /// UTF-8 text string
    String,
    /// Boolean true/false
    Bool,
    /// JSON object or array
    Json,
}

/// PostgreSQL column types for Silver layer tables.
///
/// These types map directly to TimescaleDB/PostgreSQL types.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash,
    Serialize, Deserialize, JsonSchema,
    EnumIter, EnumString, Display, AsRefStr, VariantNames,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum SilverFieldType {
    /// 64-bit floating point (PostgreSQL DOUBLE PRECISION)
    DoublePrecision,
    /// 32-bit floating point (PostgreSQL REAL)
    Real,
    /// 32-bit signed integer (PostgreSQL INTEGER)
    Integer,
    /// 64-bit signed integer (PostgreSQL BIGINT)
    Bigint,
    /// 16-bit signed integer (PostgreSQL SMALLINT)
    Smallint,
    /// Variable-length text (PostgreSQL TEXT)
    Text,
    /// Variable-length text with limit (PostgreSQL VARCHAR)
    Varchar,
    /// Boolean (PostgreSQL BOOLEAN)
    Boolean,
    /// Timestamp with timezone (PostgreSQL TIMESTAMPTZ)
    Timestamptz,
    /// JSON binary (PostgreSQL JSONB)
    Jsonb,
    /// Text array (PostgreSQL TEXT[])
    #[serde(rename = "text[]")]
    #[strum(serialize = "text[]")]
    TextArray,
}

impl FieldType {
    pub fn all_names() -> &'static [&'static str] {
        Self::VARIANTS
    }
}

impl SilverFieldType {
    pub fn all_names() -> &'static [&'static str] {
        Self::VARIANTS
    }
}
```

### 2.4 DQ Rule Type Definitions

```rust
ALGORITHM: dq_rule_definition
FILE: crates/ndp-types/src/dq_rule.rs

use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use strum::{EnumIter, EnumString, Display, AsRefStr, VariantNames};

/// DQ rule type discriminator for semantic validation.
///
/// This enum provides the rule type names without parameters,
/// used for validating rule type strings before full parsing.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash,
    Serialize, Deserialize, JsonSchema,
    EnumIter, EnumString, Display, AsRefStr, VariantNames,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum DqRuleType {
    /// Value must be within min/max bounds
    RangeCheck,
    /// Value must not be null/empty
    NullCheck,
    /// Value must be one of allowed values
    EnumCheck,
    /// Value must match regex pattern
    PatternCheck,
    /// Data must arrive within time threshold
    FreshnessCheck,
    /// Values must be monotonically increasing/decreasing
    MonotonicCheck,
    /// Rate of change must be within bounds
    RateOfChange,
    /// Relationship between fields must hold
    CrossFieldCheck,
    /// Conditional validation based on other field values
    ConditionalCheck,
    /// Required percentage of non-null values
    CompletenessCheck,
    /// Distinct value count within bounds
    CardinalityCheck,
}

/// Data quality actions when a rule fails.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Default,
    Serialize, Deserialize, JsonSchema,
    EnumIter, EnumString, Display, AsRefStr, VariantNames,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum DqAction {
    /// Add DQ flag column but keep record
    #[default]
    Flag,
    /// Reject record entirely
    Reject,
    /// Clamp value to valid range
    Clamp,
    /// Drop the field value (set to null)
    Drop,
    /// Log warning but process normally
    Warn,
}

/// Direction constraint for monotonic checks.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash,
    Serialize, Deserialize, JsonSchema,
    EnumIter, EnumString, Display, AsRefStr, VariantNames,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum MonotonicDirection {
    /// Values must be >= previous value
    Increasing,
    /// Values must be <= previous value
    Decreasing,
    /// Values must be > previous value (no equality)
    StrictIncreasing,
}

/// Full DQ rule definition with parameters (tagged enum).
///
/// Deserialized from JSON with "rule" as the tag field.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(tag = "rule", rename_all = "snake_case")]
pub enum DqRule {
    RangeCheck {
        column: String,
        #[serde(default)]
        min: Option<f64>,
        #[serde(default)]
        max: Option<f64>,
        #[serde(default)]
        inclusive_min: bool,
        #[serde(default)]
        inclusive_max: bool,
        #[serde(default)]
        action: DqAction,
    },
    NullCheck {
        column: String,
        #[serde(default)]
        action: DqAction,
    },
    EnumCheck {
        column: String,
        allowed_values: Vec<serde_json::Value>,
        #[serde(default)]
        action: DqAction,
    },
    PatternCheck {
        column: String,
        pattern: String,
        #[serde(default)]
        action: DqAction,
    },
    FreshnessCheck {
        column: String,
        max_age: String,  // ISO 8601 duration
        #[serde(default)]
        action: DqAction,
    },
    MonotonicCheck {
        column: String,
        direction: MonotonicDirection,
        #[serde(default)]
        action: DqAction,
    },
    RateOfChange {
        column: String,
        #[serde(default)]
        max_increase: Option<f64>,
        #[serde(default)]
        max_decrease: Option<f64>,
        #[serde(default)]
        per_interval: Option<String>,
        #[serde(default)]
        action: DqAction,
    },
    CrossFieldCheck {
        expression: String,
        columns: Vec<String>,
        #[serde(default)]
        action: DqAction,
    },
    ConditionalCheck {
        condition: String,
        rule: Box<DqRule>,
        #[serde(default)]
        action: DqAction,
    },
    CompletenessCheck {
        column: String,
        #[serde(default = "default_threshold")]
        threshold: f64,
        #[serde(default)]
        action: DqAction,
    },
    CardinalityCheck {
        column: String,
        #[serde(default)]
        min_distinct: Option<u64>,
        #[serde(default)]
        max_distinct: Option<u64>,
        #[serde(default)]
        action: DqAction,
    },
}

fn default_threshold() -> f64 {
    1.0  // 100% completeness by default
}

impl DqRuleType {
    pub fn all_names() -> &'static [&'static str] {
        Self::VARIANTS
    }
}

impl DqAction {
    pub fn all_names() -> &'static [&'static str] {
        Self::VARIANTS
    }
}
```

---

## 3. NdpValidate Trait Definition

### 3.1 Core Trait

```rust
ALGORITHM: ndp_validate_trait
FILE: crates/ndp-types/src/validate.rs

use std::collections::HashSet;

/// Validation layer indicating where the error was detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ValidationLayer {
    /// JSON syntax errors (malformed JSON)
    Syntax,
    /// JSON Schema validation errors (wrong types, missing fields)
    Schema,
    /// Semantic validation errors (invalid references, constraint violations)
    Semantic,
}

/// Error severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Severity {
    /// Validation fails; config cannot be used
    Error,
    /// Validation passes with warnings; config can be used
    Warning,
}

/// Unified validation error structure.
///
/// Provides machine-readable code, human-readable message,
/// and JSONPath location for precise error reporting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValidationError {
    /// Validation layer where error was detected
    pub layer: ValidationLayer,
    /// Machine-readable error code for programmatic handling
    pub code: ErrorCode,
    /// JSONPath to the error location (e.g., "$.sources[0].type")
    pub path: String,
    /// Human-readable error message
    pub message: String,
    /// Error severity
    pub severity: Severity,
    /// Optional suggestion for fixing the error
    pub suggestion: Option<String>,
}

/// Context for cross-reference validation.
///
/// Provides information about related configuration elements
/// that semantic validation may need to reference.
#[derive(Debug, Clone, Default)]
pub struct ValidationContext {
    /// Field names defined in schema_fields
    pub field_names: HashSet<String>,
    /// Column names defined in Silver field_mappings
    pub silver_columns: HashSet<String>,
    /// Stream ID being validated
    pub stream_id: Option<String>,
    /// Database connection URL for table/column checks
    pub database_url: Option<String>,
}

/// Trait for NDP configuration validation.
///
/// Implementors provide semantic validation logic that goes beyond
/// JSON Schema structural validation. This includes:
/// - Cross-reference checks (field names exist)
/// - Constraint validation (min < max)
/// - Domain-specific rules (valid regex patterns)
pub trait NdpValidate {
    /// Validate this configuration, returning all errors.
    ///
    /// # Returns
    /// Vector of validation errors (empty if valid)
    fn validate(&self) -> Vec<ValidationError>;

    /// Validate with additional context.
    ///
    /// Default implementation ignores context and calls validate().
    /// Override for validations that need cross-reference information.
    fn validate_with_context(&self, ctx: &ValidationContext) -> Vec<ValidationError> {
        self.validate()
    }

    /// Check if configuration is valid (no errors).
    fn is_valid(&self) -> bool {
        self.validate().iter().all(|e| e.severity != Severity::Error)
    }
}
```

### 3.2 ValidationError Constructor Helpers

```rust
ALGORITHM: validation_error_helpers
PURPOSE: Simplify error creation

impl ValidationError {
    /// Create a semantic error with standard fields.
    pub fn semantic(code: ErrorCode, path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            layer: ValidationLayer::Semantic,
            code,
            path: path.into(),
            message: message.into(),
            severity: Severity::Error,
            suggestion: None,
        }
    }

    /// Create a warning (non-blocking).
    pub fn warning(code: ErrorCode, path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            layer: ValidationLayer::Semantic,
            code,
            path: path.into(),
            message: message.into(),
            severity: Severity::Warning,
            suggestion: None,
        }
    }

    /// Add a suggestion to this error.
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}
```

---

## 4. Schema Generation Algorithm

### 4.1 Generate Schema from Types

```
ALGORITHM: generate_schema
INPUT: none (uses ndp-types crate)
OUTPUT: JSON Schema document (serde_json::Value)

PURPOSE:
    Generate a complete JSON Schema for StreamConfig by introspecting
    Rust types with schemars derives. The schema is a Single Source of Truth
    derived directly from the authoritative Rust type definitions.

STEPS:
    1. Import root type (StreamConfig) from ndp-types
    2. Call schemars::schema_for! macro
    3. Post-process for NDP-specific customizations
    4. Return serializable schema

PSEUDOCODE:
    FUNCTION generate_schema() -> Result<Value, Error>
        // Step 1: Generate raw schema from Rust type
        raw_schema <- schemars::schema_for!(StreamConfig)

        // Step 2: Convert to JSON Value for manipulation
        schema_value <- serde_json::to_value(raw_schema)?

        // Step 3: Add NDP metadata
        schema_value["$schema"] <- "https://json-schema.org/draft/2020-12/schema"
        schema_value["$id"] <- "https://ndp.local/schemas/stream-config.v1.2.json"
        schema_value["title"] <- "NDP Stream Configuration"
        schema_value["description"] <- "Configuration for NDP Bronze and Silver layer streams"

        // Step 4: Add version info
        schema_value["x-ndp-version"] <- "1.2.0"
        schema_value["x-generated-from"] <- "ndp-types crate"
        schema_value["x-generated-at"] <- chrono::Utc::now().to_rfc3339()

        RETURN Ok(schema_value)
    END FUNCTION
```

### 4.2 Combine Multiple Type Schemas

```
ALGORITHM: combine_type_schemas
INPUT: type_list (list of type names to include)
OUTPUT: Combined JSON Schema with definitions

PURPOSE:
    Generate partial schemas for individual types and combine them
    into a schema with shared definitions. Used when generating
    schema for a subset of types or for documentation.

PSEUDOCODE:
    FUNCTION combine_type_schemas(type_list: &[&str]) -> Result<Value, Error>
        definitions <- empty Map

        FOR EACH type_name IN type_list DO
            schema <- MATCH type_name {
                "SourceType" => schema_for!(SourceType),
                "FieldType" => schema_for!(FieldType),
                "DqRuleType" => schema_for!(DqRuleType),
                "DqAction" => schema_for!(DqAction),
                "DqRule" => schema_for!(DqRule),
                "TransformConfig" => schema_for!(TransformConfig),
                _ => RETURN Err("Unknown type")
            }

            // Extract type name from schema
            def_name <- schema.title OR type_name
            definitions.insert(def_name, schema)
        END FOR

        // Create combined schema with $defs
        combined <- {
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$defs": definitions,
        }

        RETURN Ok(combined)
    END FUNCTION
```

### 4.3 Output Format

```
ALGORITHM: schema_output_format
PURPOSE: Define the output structure of generated schema

OUTPUT FORMAT:
    {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://ndp.local/schemas/stream-config.v1.2.json",
        "title": "NDP Stream Configuration",
        "description": "Configuration for NDP Bronze and Silver layer streams",
        "type": "object",

        // NDP-specific metadata (x- prefix for extensions)
        "x-ndp-version": "1.2.0",
        "x-generated-from": "ndp-types crate",
        "x-generated-at": "2026-02-02T12:00:00Z",

        "properties": {
            "stream_id": { "type": "string" },
            "sources": {
                "type": "array",
                "items": {
                    "$ref": "#/$defs/SourceConfig"
                }
            },
            // ... more properties
        },

        "$defs": {
            "SourceType": {
                "type": "string",
                "enum": ["mqtt", "http_poll", "webhook", "file_watch", "csv"],
                "description": "Data source types supported by NDP"
            },
            "FieldType": {
                "type": "string",
                "enum": ["float", "int", "string", "bool", "json"]
            },
            "DqRuleType": {
                "type": "string",
                "enum": [
                    "range_check", "null_check", "enum_check", "pattern_check",
                    "freshness_check", "monotonic_check", "rate_of_change",
                    "cross_field_check", "conditional_check", "completeness_check",
                    "cardinality_check"
                ]
            },
            // ... more definitions
        }
    }
```

---

## 5. Schema Verification Algorithm

### 5.1 Verify Schema Command

```
ALGORITHM: verify_schema
INPUT: path (PathBuf to existing schema file)
OUTPUT: exit code (0 = match, 1 = drift detected)

PURPOSE:
    Compare committed schema file against freshly generated schema.
    Used in CI to detect drift between Rust types and committed schema.

PSEUDOCODE:
    FUNCTION verify_schema(path: PathBuf) -> Result<ExitCode>
        // Step 1: Load existing schema from file
        existing_content <- fs::read_to_string(path)?
        existing_schema <- serde_json::from_str::<Value>(existing_content)?

        // Step 2: Generate fresh schema from Rust types
        generated_schema <- generate_schema()?

        // Step 3: Normalize both schemas for comparison
        existing_normalized <- normalize_schema(existing_schema)
        generated_normalized <- normalize_schema(generated_schema)

        // Step 4: Deep compare
        differences <- compare_schemas(existing_normalized, generated_normalized)

        // Step 5: Report results
        IF differences.is_empty() THEN
            println!("Schema verification PASSED")
            println!("Committed schema matches generated schema")
            RETURN Ok(ExitCode::SUCCESS)  // Exit 0
        ELSE
            eprintln!("Schema verification FAILED")
            eprintln!("Found {} difference(s):", differences.len())
            FOR EACH diff IN differences DO
                eprintln!("  - {}: {}", diff.path, diff.description)
            END FOR
            eprintln!("")
            eprintln!("To fix: run 'ndp-validate --generate-schema --output {}'", path)
            RETURN Ok(ExitCode::FAILURE)  // Exit 1
        END IF
    END FUNCTION
```

### 5.2 Schema Normalization

```
ALGORITHM: normalize_schema
INPUT: schema (Value)
OUTPUT: normalized schema (Value)

PURPOSE:
    Remove non-semantic differences (whitespace, key order, timestamps)
    so that comparison focuses on meaningful schema differences.

PSEUDOCODE:
    FUNCTION normalize_schema(schema: Value) -> Value
        mutable normalized <- schema.clone()

        // Remove volatile metadata
        normalized.remove("x-generated-at")

        // Sort object keys recursively
        normalized <- sort_keys_recursive(normalized)

        // Normalize array orderings where order doesn't matter
        // (Note: enum arrays ARE order-sensitive for some tools)

        RETURN normalized
    END FUNCTION

    FUNCTION sort_keys_recursive(value: Value) -> Value
        MATCH value {
            Value::Object(map) => {
                sorted_map <- BTreeMap::new()
                FOR EACH (key, val) IN map DO
                    sorted_map.insert(key, sort_keys_recursive(val))
                END FOR
                Value::Object(sorted_map)
            },
            Value::Array(arr) => {
                Value::Array(arr.map(sort_keys_recursive).collect())
            },
            other => other
        }
    END FUNCTION
```

### 5.3 Schema Comparison

```
ALGORITHM: compare_schemas
INPUT: existing (Value), generated (Value)
OUTPUT: list of SchemaDifference

STRUCTURE: SchemaDifference
    path: String      // JSONPath to difference
    expected: Value   // Value in generated schema
    actual: Value     // Value in existing schema
    description: String

PSEUDOCODE:
    FUNCTION compare_schemas(existing: Value, generated: Value) -> Vec<SchemaDifference>
        differences <- Vec::new()
        compare_recursive("$", existing, generated, &mut differences)
        RETURN differences
    END FUNCTION

    FUNCTION compare_recursive(path: &str, existing: &Value, generated: &Value, diffs: &mut Vec)
        IF existing == generated THEN
            RETURN  // No difference
        END IF

        MATCH (existing, generated) {
            (Object(e_map), Object(g_map)) => {
                // Check for missing/extra keys
                FOR EACH key IN g_map.keys() DO
                    IF NOT e_map.contains(key) THEN
                        diffs.push(SchemaDifference {
                            path: format!("{}.{}", path, key),
                            expected: g_map[key].clone(),
                            actual: Value::Null,
                            description: format!("Missing key '{}'", key),
                        })
                    END IF
                END FOR

                FOR EACH key IN e_map.keys() DO
                    IF NOT g_map.contains(key) THEN
                        diffs.push(SchemaDifference {
                            path: format!("{}.{}", path, key),
                            expected: Value::Null,
                            actual: e_map[key].clone(),
                            description: format!("Extra key '{}' not in generated schema", key),
                        })
                    END IF
                END FOR

                // Recurse into shared keys
                FOR EACH key IN g_map.keys().filter(|k| e_map.contains(k)) DO
                    compare_recursive(
                        format!("{}.{}", path, key),
                        &e_map[key],
                        &g_map[key],
                        diffs
                    )
                END FOR
            },

            (Array(e_arr), Array(g_arr)) => {
                // For enum arrays, compare as sets
                IF is_enum_array(path) THEN
                    e_set <- e_arr.iter().collect::<HashSet>()
                    g_set <- g_arr.iter().collect::<HashSet>()

                    IF e_set != g_set THEN
                        missing <- g_set.difference(&e_set)
                        extra <- e_set.difference(&g_set)

                        IF NOT missing.is_empty() THEN
                            diffs.push(SchemaDifference {
                                path: path.to_string(),
                                description: format!("Missing enum values: {:?}", missing),
                                ..
                            })
                        END IF

                        IF NOT extra.is_empty() THEN
                            diffs.push(SchemaDifference {
                                path: path.to_string(),
                                description: format!("Extra enum values not in types: {:?}", extra),
                                ..
                            })
                        END IF
                    END IF
                ELSE
                    // Compare element by element
                    FOR i IN 0..max(e_arr.len(), g_arr.len()) DO
                        compare_recursive(
                            format!("{}[{}]", path, i),
                            e_arr.get(i).unwrap_or(&Value::Null),
                            g_arr.get(i).unwrap_or(&Value::Null),
                            diffs
                        )
                    END FOR
                END IF
            },

            _ => {
                // Primitive value mismatch
                diffs.push(SchemaDifference {
                    path: path.to_string(),
                    expected: generated.clone(),
                    actual: existing.clone(),
                    description: format!("Value mismatch: expected {:?}, got {:?}", generated, existing),
                })
            }
        }
    END FUNCTION

    FUNCTION is_enum_array(path: &str) -> bool
        // Paths that contain enum arrays
        path.ends_with(".enum")
    END FUNCTION
```

---

## 6. Validation Unification Algorithm

### 6.1 Unified Config Validation

```
ALGORITHM: validate_config
INPUT: config_json (String or Value)
OUTPUT: ValidationResult with all errors

PURPOSE:
    Two-layer validation as defined in ADR-019-001:
    Layer 1: Schema validation (JSON Schema from ndp-types)
    Layer 2: Semantic validation (NdpValidate trait)

PSEUDOCODE:
    FUNCTION validate_config(config_json: &str) -> ValidationResult
        all_errors <- Vec::new()

        // ============================================
        // LAYER 1: Schema Validation
        // ============================================

        // Step 1a: Syntax check (valid JSON?)
        parsed_value <- MATCH serde_json::from_str(config_json) {
            Ok(value) => value,
            Err(e) => {
                all_errors.push(ValidationError {
                    layer: ValidationLayer::Syntax,
                    code: ErrorCode::SyntaxError,
                    path: format!("line {}, column {}", e.line(), e.column()),
                    message: e.to_string(),
                    severity: Severity::Error,
                    suggestion: None,
                })
                RETURN ValidationResult::new(all_errors)  // Early exit
            }
        }

        // Step 1b: Schema validation (against generated schema)
        schema <- load_or_generate_schema()
        schema_errors <- jsonschema::validate(&schema, &parsed_value)

        FOR EACH schema_error IN schema_errors DO
            all_errors.push(ValidationError {
                layer: ValidationLayer::Schema,
                code: map_schema_error_code(schema_error),
                path: schema_error.instance_path.to_string(),
                message: schema_error.to_string(),
                severity: Severity::Error,
                suggestion: suggest_for_schema_error(schema_error),
            })
        END FOR

        // If schema validation fails badly, may not be able to deserialize
        IF has_blocking_schema_errors(all_errors) THEN
            RETURN ValidationResult::new(all_errors)
        END IF

        // ============================================
        // LAYER 2: Semantic Validation
        // ============================================

        // Step 2a: Deserialize into typed struct
        config <- MATCH serde_json::from_value::<StreamConfig>(parsed_value) {
            Ok(c) => c,
            Err(e) => {
                all_errors.push(ValidationError {
                    layer: ValidationLayer::Schema,
                    code: ErrorCode::InvalidType,
                    path: "$",
                    message: format!("Failed to deserialize: {}", e),
                    severity: Severity::Error,
                    suggestion: None,
                })
                RETURN ValidationResult::new(all_errors)
            }
        }

        // Step 2b: Build validation context
        context <- ValidationContext {
            field_names: config.schema_fields.iter().map(|f| f.name.clone()).collect(),
            silver_columns: config.silver_etl
                .as_ref()
                .map(|s| s.field_mappings.iter().map(|m| m.silver_column.clone()).collect())
                .unwrap_or_default(),
            stream_id: Some(config.stream_id.clone()),
            database_url: env::var("DATABASE_URL").ok(),
        }

        // Step 2c: Call NdpValidate trait method
        semantic_errors <- config.validate_with_context(&context)
        all_errors.extend(semantic_errors)

        RETURN ValidationResult::new(all_errors)
    END FUNCTION
```

### 6.2 NdpValidate Implementation for StreamConfig

```
ALGORITHM: stream_config_validate
FILE: Implementation in consumer crate (core or ndp-validate)

impl NdpValidate for StreamConfig {
    fn validate(&self) -> Vec<ValidationError> {
        errors <- Vec::new()

        // Rule 1: stream_id must not be empty
        IF self.stream_id.is_empty() THEN
            errors.push(ValidationError::semantic(
                ErrorCode::MissingRequired,
                "$.stream_id",
                "stream_id cannot be empty"
            ))
        END IF

        // Rule 2: At least one source required
        IF self.sources.is_empty() THEN
            errors.push(ValidationError::semantic(
                ErrorCode::MissingRequired,
                "$.sources",
                "At least one source configuration is required"
            ))
        END IF

        // Rule 3: Validate each source
        FOR (i, source) IN self.sources.iter().enumerate() DO
            source_errors <- source.validate()
            FOR error IN source_errors DO
                // Prefix path with array index
                error.path <- format!("$.sources[{}]{}", i, error.path.trim_start_matches("$"))
                errors.push(error)
            END FOR
        END FOR

        // Rule 4: Validate schema_fields
        FOR (i, field) IN self.schema_fields.iter().enumerate() DO
            field_errors <- field.validate()
            FOR error IN field_errors DO
                error.path <- format!("$.schema_fields[{}]{}", i, error.path.trim_start_matches("$"))
                errors.push(error)
            END FOR
        END FOR

        // Rule 5: Validate silver_etl if present
        IF let Some(silver_etl) = &self.silver_etl THEN
            silver_errors <- silver_etl.validate()
            FOR error IN silver_errors DO
                error.path <- format!("$.silver_etl{}", error.path.trim_start_matches("$"))
                errors.push(error)
            END FOR
        END IF

        RETURN errors
    }

    fn validate_with_context(&self, ctx: &ValidationContext) -> Vec<ValidationError> {
        errors <- self.validate()

        // Cross-reference validation
        IF let Some(silver_etl) = &self.silver_etl THEN
            // Build context with field names
            enriched_ctx <- ValidationContext {
                field_names: self.schema_fields.iter().map(|f| f.name.clone()).collect(),
                ..ctx.clone()
            }

            // Validate source_path references
            FOR (i, mapping) IN silver_etl.field_mappings.iter().enumerate() DO
                IF let Some(source_path) = &mapping.source_path THEN
                    // Extract field name from path
                    field_name <- extract_field_from_path(source_path)

                    IF NOT enriched_ctx.field_names.contains(&field_name) THEN
                        errors.push(ValidationError::semantic(
                            ErrorCode::InvalidSourcePath,
                            format!("$.silver_etl.field_mappings[{}].source_path", i),
                            format!(
                                "source_path '{}' references undefined field '{}'. Valid fields: {}",
                                source_path,
                                field_name,
                                enriched_ctx.field_names.iter().join(", ")
                            )
                        ).with_suggestion(
                            format!("Define '{}' in schema_fields or use an existing field", field_name)
                        ))
                    END IF
                END IF
            END FOR
        END IF

        RETURN errors
    }
}
```

### 6.3 NdpValidate Implementation for DqRule

```
ALGORITHM: dq_rule_validate
FILE: Implementation in consumer crate

impl NdpValidate for DqRule {
    fn validate(&self) -> Vec<ValidationError> {
        errors <- Vec::new()

        MATCH self {
            DqRule::RangeCheck { column, min, max, .. } => {
                // Rule: column cannot be empty
                IF column.is_empty() THEN
                    errors.push(ValidationError::semantic(
                        ErrorCode::InvalidDqRule,
                        "$.column",
                        "column cannot be empty for range_check"
                    ))
                END IF

                // Rule: at least one of min/max must be set
                IF min.is_none() AND max.is_none() THEN
                    errors.push(ValidationError::semantic(
                        ErrorCode::InvalidDqRule,
                        "$",
                        "range_check must have at least one of min or max"
                    ))
                END IF

                // Rule: min <= max when both present
                IF let (Some(min_val), Some(max_val)) = (min, max) THEN
                    IF min_val > max_val THEN
                        errors.push(ValidationError::semantic(
                            ErrorCode::InvalidRange,
                            "$",
                            format!("min ({}) must be <= max ({})", min_val, max_val)
                        ).with_suggestion("Swap min and max values"))
                    END IF
                END IF
            },

            DqRule::PatternCheck { column, pattern, .. } => {
                IF column.is_empty() THEN
                    errors.push(ValidationError::semantic(
                        ErrorCode::InvalidDqRule,
                        "$.column",
                        "column cannot be empty for pattern_check"
                    ))
                END IF

                // Rule: pattern must be valid regex
                IF let Err(e) = regex::Regex::new(pattern) THEN
                    errors.push(ValidationError::semantic(
                        ErrorCode::InvalidRegex,
                        "$.pattern",
                        format!("Invalid regex pattern '{}': {}", pattern, e)
                    ))
                END IF
            },

            DqRule::FreshnessCheck { column, max_age, .. } => {
                IF column.is_empty() THEN
                    errors.push(ValidationError::semantic(
                        ErrorCode::InvalidDqRule,
                        "$.column",
                        "column cannot be empty for freshness_check"
                    ))
                END IF

                // Rule: max_age must be valid ISO 8601 duration
                IF let Err(e) = parse_iso8601_duration(max_age) THEN
                    errors.push(ValidationError::semantic(
                        ErrorCode::InvalidInterval,
                        "$.max_age",
                        format!("Invalid ISO 8601 duration '{}': {}", max_age, e)
                    ).with_suggestion("Use format like 'PT1H' (1 hour) or 'P1D' (1 day)"))
                END IF
            },

            DqRule::EnumCheck { column, allowed_values, .. } => {
                IF column.is_empty() THEN
                    errors.push(ValidationError::semantic(
                        ErrorCode::InvalidDqRule,
                        "$.column",
                        "column cannot be empty for enum_check"
                    ))
                END IF

                IF allowed_values.is_empty() THEN
                    errors.push(ValidationError::semantic(
                        ErrorCode::InvalidDqRule,
                        "$.allowed_values",
                        "allowed_values cannot be empty for enum_check"
                    ))
                END IF
            },

            // ... other rule variants
            _ => {}
        }

        RETURN errors
    }
}
```

### 6.4 Error Aggregation

```
ALGORITHM: aggregate_errors
INPUT: errors from multiple validation sources
OUTPUT: consolidated ValidationResult

STRUCTURE: ValidationResult
    errors: Vec<ValidationError>
    warnings: Vec<ValidationError>
    is_valid: bool

PSEUDOCODE:
    FUNCTION aggregate_errors(all_errors: Vec<ValidationError>) -> ValidationResult
        errors <- Vec::new()
        warnings <- Vec::new()

        FOR error IN all_errors DO
            MATCH error.severity {
                Severity::Error => errors.push(error),
                Severity::Warning => warnings.push(error),
            }
        END FOR

        // Sort by path for consistent output
        errors.sort_by(|a, b| a.path.cmp(&b.path))
        warnings.sort_by(|a, b| a.path.cmp(&b.path))

        RETURN ValidationResult {
            errors,
            warnings,
            is_valid: errors.is_empty(),
        }
    END FUNCTION

    impl ValidationResult {
        fn to_json(&self) -> Value {
            json!({
                "valid": self.is_valid,
                "error_count": self.errors.len(),
                "warning_count": self.warnings.len(),
                "errors": self.errors,
                "warnings": self.warnings,
            })
        }

        fn to_human_readable(&self) -> String {
            output <- String::new()

            IF self.is_valid THEN
                output.push_str("Validation PASSED")
                IF NOT self.warnings.is_empty() THEN
                    output.push_str(format!(" with {} warning(s)", self.warnings.len()))
                END IF
            ELSE
                output.push_str(format!("Validation FAILED with {} error(s)", self.errors.len()))
            END IF

            output.push_str("\n\n")

            IF NOT self.errors.is_empty() THEN
                output.push_str("ERRORS:\n")
                FOR error IN &self.errors DO
                    output.push_str(format!("  [{}] {}: {}\n",
                        error.code, error.path, error.message))
                    IF let Some(suggestion) = &error.suggestion THEN
                        output.push_str(format!("       Suggestion: {}\n", suggestion))
                    END IF
                END FOR
            END IF

            IF NOT self.warnings.is_empty() THEN
                output.push_str("\nWARNINGS:\n")
                FOR warning IN &self.warnings DO
                    output.push_str(format!("  [{}] {}: {}\n",
                        warning.code, warning.path, warning.message))
                END FOR
            END IF

            RETURN output
        }
    }
```

---

## 7. Migration Algorithm

### 7.1 Type Migration Steps

```
ALGORITHM: migrate_type
INPUT: type_name (String), source_file (Path), target_module (String)
OUTPUT: Migration instructions

PURPOSE:
    Systematic process for moving a type from its current location
    to ndp-types crate while maintaining backward compatibility.

PSEUDOCODE:
    FUNCTION migrate_type(type_name: &str) -> MigrationPlan
        plan <- MigrationPlan::new(type_name)

        // ============================================
        // PHASE 1: Create in ndp-types (Non-Breaking)
        // ============================================

        plan.add_step(Step {
            description: format!("Create {} in ndp-types crate", type_name),
            actions: [
                // 1. Create file in ndp-types
                CreateFile {
                    path: format!("crates/ndp-types/src/{}.rs", type_name.to_snake_case()),
                    content: generate_type_definition(type_name),
                },
                // 2. Add to lib.rs exports
                EditFile {
                    path: "crates/ndp-types/src/lib.rs",
                    add_line: format!("pub use {}::{};", type_name.to_snake_case(), type_name),
                },
                // 3. Add module declaration
                EditFile {
                    path: "crates/ndp-types/src/lib.rs",
                    add_line: format!("mod {};", type_name.to_snake_case()),
                },
            ],
            verification: "cargo build -p ndp-types",
        })

        // ============================================
        // PHASE 2: Update Consumer Dependencies
        // ============================================

        plan.add_step(Step {
            description: "Add ndp-types dependency to consumers",
            actions: [
                EditFile {
                    path: "core/Cargo.toml",
                    add_dependency: "ndp-types = { workspace = true }",
                },
                EditFile {
                    path: "tools/ndp-validate/Cargo.toml",
                    add_dependency: "ndp-types = { workspace = true }",
                },
            ],
            verification: "cargo build --workspace",
        })

        // ============================================
        // PHASE 3: Re-export for Backward Compatibility
        // ============================================

        plan.add_step(Step {
            description: format!("Re-export {} from core for backward compatibility", type_name),
            actions: [
                EditFile {
                    path: "core/src/types/mod.rs",
                    add_line: format!("pub use ndp_types::{};", type_name),
                },
                EditFile {
                    path: find_original_file(type_name),
                    add_deprecation: format!(
                        "#[deprecated(since = \"0.2.0\", note = \"Use ndp_types::{} instead\")]",
                        type_name
                    ),
                },
            ],
            verification: "cargo build --workspace && cargo test --workspace",
        })

        // ============================================
        // PHASE 4: Migrate Consumers
        // ============================================

        consumer_files <- find_files_importing(type_name)

        FOR EACH file IN consumer_files DO
            plan.add_step(Step {
                description: format!("Update imports in {}", file),
                actions: [
                    EditFile {
                        path: file,
                        replace: format!("use crate::types::{};", type_name),
                        with: format!("use ndp_types::{};", type_name),
                    },
                ],
            })
        END FOR

        // ============================================
        // PHASE 5: Remove Original Definition
        // ============================================

        plan.add_step(Step {
            description: format!("Remove original {} definition", type_name),
            actions: [
                EditFile {
                    path: find_original_file(type_name),
                    remove: type_definition_block(type_name),
                },
            ],
            verification: "cargo build --workspace && cargo test --workspace",
        })

        RETURN plan
    END FUNCTION
```

### 7.2 Consumer Update Pattern

```
ALGORITHM: update_consumer_imports
INPUT: file_path (Path)
OUTPUT: Updated file with new imports

PSEUDOCODE:
    FUNCTION update_consumer_imports(file_path: PathBuf)
        content <- fs::read_to_string(file_path)?

        // Map old imports to new imports
        replacements <- [
            // From core/src/types/
            ("use crate::types::SourceType", "use ndp_types::SourceType"),
            ("use crate::types::FieldType", "use ndp_types::FieldType"),
            ("use neural_core::SourceType", "use ndp_types::SourceType"),
            ("use neural_core::FieldType", "use ndp_types::FieldType"),

            // From core/src/config/
            ("use crate::config::silver_etl::DqRule", "use ndp_types::DqRule"),
            ("use crate::config::silver_etl::DqAction", "use ndp_types::DqAction"),

            // Validator hardcoded constants
            ("SUPPORTED_SOURCE_TYPES", "SourceType::all_names()"),
            ("SUPPORTED_DQ_RULES", "DqRuleType::all_names()"),
            ("SUPPORTED_ACTIONS", "DqAction::all_names()"),
        ]

        FOR (old_import, new_import) IN replacements DO
            content <- content.replace(old_import, new_import)
        END FOR

        // Add ndp_types to use statements if not present
        IF content.contains("use ndp_types::") AND NOT content.contains("use ndp_types;") THEN
            // Consolidate multiple ndp_types imports
            content <- consolidate_imports(content, "ndp_types")
        END IF

        fs::write(file_path, content)?
    END FUNCTION
```

### 7.3 Re-export Strategy

```
ALGORITHM: re_export_strategy
PURPOSE: Maintain backward compatibility during migration

FILE: core/src/lib.rs
PATTERN:
    // Re-export from ndp-types for backward compatibility
    // Consumers can use either:
    //   use neural_core::SourceType;  (deprecated but works)
    //   use ndp_types::SourceType;    (preferred)

    #[doc(hidden)]
    #[deprecated(
        since = "0.2.0",
        note = "Import from ndp_types crate instead: use ndp_types::SourceType;"
    )]
    pub use ndp_types::SourceType;

    #[doc(hidden)]
    #[deprecated(
        since = "0.2.0",
        note = "Import from ndp_types crate instead: use ndp_types::FieldType;"
    )]
    pub use ndp_types::FieldType;

FILE: core/src/types/mod.rs
PATTERN:
    // Types module - re-exports from ndp-types
    // Original definitions removed; these are compatibility shims

    pub use ndp_types::{
        SourceType,
        FieldType,
    };

    // StreamConfig still defined here (uses imported types)
    mod stream_config;
    pub use stream_config::StreamConfig;
```

---

## 8. CLI Integration

### 8.1 ndp-validate CLI Extension

```
ALGORITHM: ndp_validate_cli
PURPOSE: Extend ndp-validate with schema generation commands

NEW FLAGS:
    --generate-schema       Generate JSON Schema from ndp-types to stdout
    --output <PATH>         Write generated schema to file (requires --generate-schema)
    --verify-schema <PATH>  Verify file matches generated schema (exit 0/1)

PSEUDOCODE:
    FUNCTION main() -> Result<ExitCode>
        args <- Args::parse()

        IF args.generate_schema THEN
            schema <- generate_schema()?
            json <- serde_json::to_string_pretty(&schema)?

            IF let Some(output_path) = args.output THEN
                fs::write(output_path, json)?
                println!("Schema written to {}", output_path)
            ELSE
                println!("{}", json)
            END IF

            RETURN Ok(ExitCode::SUCCESS)
        END IF

        IF let Some(verify_path) = args.verify_schema THEN
            RETURN verify_schema(verify_path)
        END IF

        // Existing validation logic
        validate_config_files(args.config_files)
    END FUNCTION
```

### 8.2 CI Workflow Integration

```yaml
ALGORITHM: ci_schema_verification
FILE: .github/workflows/schema-check.yml

name: Schema Verification

on:
  push:
    paths:
      - 'crates/ndp-types/**'
      - 'schemas/**'
  pull_request:
    paths:
      - 'crates/ndp-types/**'
      - 'schemas/**'

jobs:
  verify-schema:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-action@stable

      - name: Build ndp-validate
        run: cargo build -p ndp-validate --release

      - name: Verify Schema Not Drifted
        run: |
          ./target/release/ndp-validate --verify-schema schemas/stream-config.v1.2.schema.json
        # Exit code 0 = match, 1 = drift detected

      - name: On Failure - Show Diff
        if: failure()
        run: |
          echo "Schema drift detected! Regenerate with:"
          echo "  cargo run -p ndp-validate -- --generate-schema --output schemas/stream-config.v1.2.schema.json"
          echo ""
          echo "Diff:"
          ./target/release/ndp-validate --generate-schema > /tmp/generated.json
          diff schemas/stream-config.v1.2.schema.json /tmp/generated.json || true
```

---

## 9. Complexity Analysis

### 9.1 Schema Generation

| Operation | Time Complexity | Space Complexity |
|-----------|----------------|------------------|
| Generate schema | O(n) where n = number of fields | O(n) |
| Serialize to JSON | O(n) | O(n) |
| Write to file | O(n) | O(1) |
| **Total** | **O(n)** | **O(n)** |

### 9.2 Schema Verification

| Operation | Time Complexity | Space Complexity |
|-----------|----------------|------------------|
| Load existing schema | O(n) | O(n) |
| Generate fresh schema | O(n) | O(n) |
| Normalize schemas | O(n log n) (sorting) | O(n) |
| Deep compare | O(n) | O(d) where d = max depth |
| **Total** | **O(n log n)** | **O(n)** |

### 9.3 Validation

| Operation | Time Complexity | Space Complexity |
|-----------|----------------|------------------|
| JSON parsing | O(n) | O(n) |
| Schema validation | O(n * s) where s = schema rules | O(n) |
| Deserialization | O(n) | O(n) |
| Semantic validation | O(n * r) where r = rules | O(e) where e = errors |
| **Total** | **O(n * max(s, r))** | **O(n)** |

---

## 10. Test Strategies

### 10.1 Round-Trip Tests

```rust
ALGORITHM: roundtrip_test
PURPOSE: Verify serialization/deserialization preserves values

#[cfg(test)]
mod roundtrip_tests {
    use super::*;
    use strum::IntoEnumIterator;

    #[test]
    fn test_source_type_roundtrip() {
        FOR variant IN SourceType::iter() DO
            // Serialize to JSON
            json <- serde_json::to_string(&variant).unwrap()

            // Deserialize back
            parsed <- serde_json::from_str::<SourceType>(&json).unwrap()

            ASSERT_EQ!(variant, parsed)
        END FOR
    }

    #[test]
    fn test_dq_rule_roundtrip() {
        test_cases <- [
            DqRule::RangeCheck { column: "temp".into(), min: Some(0.0), max: Some(100.0), .. },
            DqRule::NullCheck { column: "id".into(), action: DqAction::Reject },
            DqRule::PatternCheck { column: "email".into(), pattern: r"^.+@.+$".into(), .. },
        ]

        FOR rule IN test_cases DO
            json <- serde_json::to_string(&rule).unwrap()
            parsed <- serde_json::from_str::<DqRule>(&json).unwrap()
            ASSERT_EQ!(rule, parsed)
        END FOR
    }
}
```

### 10.2 Schema Generation Tests

```rust
ALGORITHM: schema_generation_test
PURPOSE: Verify generated schema matches expected structure

#[test]
fn test_source_type_schema_has_all_variants() {
    schema <- schemars::schema_for!(SourceType)
    schema_json <- serde_json::to_value(schema).unwrap()

    // Extract enum values from schema
    enum_values <- schema_json["enum"].as_array().unwrap()

    // Verify all Rust variants are in schema
    FOR variant IN SourceType::iter() DO
        variant_str <- variant.as_ref()
        ASSERT!(
            enum_values.contains(&Value::String(variant_str.to_string())),
            "Schema missing variant: {}", variant_str
        )
    END FOR

    // Verify schema has no extra values
    ASSERT_EQ!(enum_values.len(), SourceType::VARIANTS.len())
}

#[test]
fn test_generated_schema_valid_json_schema() {
    schema <- generate_schema().unwrap()

    // Verify it's a valid JSON Schema
    ASSERT!(schema["$schema"].as_str().is_some())
    ASSERT!(schema["type"].as_str() == Some("object"))
    ASSERT!(schema["properties"].is_object())
}
```

### 10.3 Validation Tests

```rust
ALGORITHM: validation_test
PURPOSE: Test NdpValidate implementations

#[test]
fn test_range_check_min_greater_than_max() {
    rule <- DqRule::RangeCheck {
        column: "temp".into(),
        min: Some(100.0),
        max: Some(0.0),  // Invalid: min > max
        ..Default::default()
    }

    errors <- rule.validate()

    ASSERT_EQ!(errors.len(), 1)
    ASSERT_EQ!(errors[0].code, ErrorCode::InvalidRange)
    ASSERT!(errors[0].message.contains("min"))
    ASSERT!(errors[0].message.contains("max"))
}

#[test]
fn test_pattern_check_invalid_regex() {
    rule <- DqRule::PatternCheck {
        column: "data".into(),
        pattern: "[invalid(regex".into(),  // Invalid regex
        ..Default::default()
    }

    errors <- rule.validate()

    ASSERT_EQ!(errors.len(), 1)
    ASSERT_EQ!(errors[0].code, ErrorCode::InvalidRegex)
}

#[test]
fn test_source_path_validation() {
    config <- StreamConfig {
        schema_fields: vec![
            SchemaField { name: "temperature".into(), .. },
        ],
        silver_etl: Some(SilverEtlConfig {
            field_mappings: vec![
                SilverFieldMapping {
                    source_path: Some("$.payload.humidity".into()),  // Not in schema_fields
                    ..
                },
            ],
            ..
        }),
        ..
    }

    ctx <- ValidationContext::default()
    errors <- config.validate_with_context(&ctx)

    ASSERT!(errors.iter().any(|e| e.code == ErrorCode::InvalidSourcePath))
}
```

---

## 11. References

- **SPECIFICATION.md**: Type definitions, migration phases, acceptance criteria
- **ADR-019-002**: Architecture decision rationale
- **CODEBASE-ANALYSIS.md**: Current type locations and discrepancies
- **schemars documentation**: https://docs.rs/schemars
- **strum documentation**: https://docs.rs/strum
- **JSON Schema 2020-12**: https://json-schema.org/draft/2020-12/schema

---

*Pseudocode created: 2026-02-02*
*SPARC Phase: Pseudocode (P)*
*Bug Reference: BUG-001 Validation Drift Risk*
*Next: Architecture refinement and implementation*
