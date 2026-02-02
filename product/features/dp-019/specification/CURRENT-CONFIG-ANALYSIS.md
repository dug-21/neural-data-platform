# Current Config Analysis for dp-019

## Overview

This document analyzes the current configuration handling in NDP to inform the Config Validation Pipeline (dp-019). It covers config structures, loading paths, error handling patterns, and recommended crates for validation.

---

## 1. Current Config Structures

### 1.1 StreamConfig (Primary Configuration)

**Location**: `core/src/types/stream_config.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StreamConfig {
    // Identification
    pub stream_id: String,              // Unique identifier (kebab-case, 3-64 chars)
    pub description: String,            // Human-readable description
    pub version: String,                // Semver (default: "1.0.0")
    pub enabled: bool,                  // Whether stream is active (default: true)

    // Retention
    pub retention_days: u32,            // Days to retain (0 = infinite)
    pub compression_after_days: u32,    // Days before compression
    pub partitioning_strategy: String,  // "daily" | "hourly" | "monthly"

    // Schema
    pub fields: Vec<SchemaField>,       // REQUIRED: At least one field
    pub sources: Vec<SourceConfig>,     // REQUIRED: At least one source

    // Optional
    pub storage: Option<StorageConfig>,
    pub silver_etl: Option<SilverEtlConfig>,      // DP-018 unified config
    pub entity_schemas: Option<Vec<EntitySchema>>, // v1.0 format (deprecated)
}
```

### 1.2 SchemaField

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SchemaField {
    pub name: String,                           // snake_case, 1-64 chars
    #[serde(rename = "type")]
    pub field_type: FieldType,                  // float | int | string | bool | json
    pub unit: Option<String>,                   // Physical unit
    pub description: Option<String>,            // Human-readable
    pub range: Option<Vec<f64>>,                // [min, max] (informational)
    pub display_precision: Option<u32>,         // Decimal places
    pub nullable: bool,                         // Default: true
    pub default: Option<serde_json::Value>,     // Default value
}
```

### 1.3 FieldType Enum

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

**Note**: No `timestamp` type defined at Bronze level. Timestamps are handled separately.

### 1.4 SourceConfig

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourceConfig {
    #[serde(rename = "type")]
    pub source_type: SourceType,        // mqtt | http_poll | webhook | file_watch | csv
    pub enabled: bool,                  // Default: true
    pub ndp_id: Option<String>,         // AIR-009: Stable source identifier
    pub context: Option<serde_json::Value>,  // AIR-009: Mutable context
    #[serde(flatten)]
    pub params: HashMap<String, serde_json::Value>,  // Source-specific params
}
```

### 1.5 SourceType Enum

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    Mqtt,
    HttpPoll,
    Webhook,
    FileWatch,
    Csv,  // dp-013
}
```

### 1.6 StorageConfig

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StorageConfig {
    pub batch_size: usize,          // Default: 100
    pub batch_timeout_secs: u64,    // Default: 5
    pub buffer_capacity: usize,     // Default: 1000
}
```

---

## 2. Silver ETL Configuration

**Location**: `core/src/config/silver_etl.rs`

### 2.1 SilverEtlConfig

```rust
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct SilverEtlConfig {
    pub enabled: bool,
    pub target_table: String,           // Must start with "silver."
    pub target_schema: Option<String>,  // Schema versioning

    // Timestamps
    pub timestamp: TimestampMapping,
    pub valid_timestamp: Option<ValidTimestampMapping>,  // For forecasts

    // Transformations
    pub pre_transform: Option<PreTransformConfig>,
    pub identity_fields: Vec<IdentityField>,
    pub field_mappings: Vec<SilverFieldMapping>,

    // Data Quality
    pub dq_rules: Vec<DqRule>,
    pub dq_output: DqOutputConfig,

    // Processing
    pub deduplication: DeduplicationConfig,
    pub incremental: IncrementalConfig,
}
```

### 2.2 SilverFieldMapping

```rust
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct SilverFieldMapping {
    pub source_path: String,        // e.g., "raw_payload.pm02"
    pub target_column: String,      // Silver column name
    #[serde(rename = "type")]
    pub column_type: String,        // PostgreSQL type
    pub nullable: bool,             // Default: true
    pub transform: Option<TransformConfig>,
    pub dq_rules: Vec<DqRule>,
}
```

### 2.3 Valid PostgreSQL Column Types

From validation in `SilverFieldMapping::validate()`:

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

### 2.4 TimestampTransform Enum

```rust
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TimestampTransform {
    MicrosecondsToTimestamp,  // Convert microseconds since epoch
    Iso8601,                   // Parse ISO 8601 string
    UnixSeconds,               // Convert Unix seconds
    NwsDuration,               // Parse NWS duration format
}
```

### 2.5 DQ Rule Types (11 types)

| Category | Rule Type | Description |
|----------|-----------|-------------|
| **Value-Level** | `range_check` | Validates numeric range |
| | `null_check` | Validates required fields |
| | `enum_check` | Validates allowed values |
| | `pattern_check` | Validates regex pattern |
| **Temporal** | `freshness_check` | Validates timestamp recency |
| | `monotonic_check` | Validates monotonic progression |
| | `rate_of_change` | Validates change rate |
| **Cross-Field** | `cross_field_check` | Validates field relationships |
| | `conditional_check` | Conditional validation |
| **Batch-Level** | `completeness_check` | Batch completeness |
| | `cardinality_check` | Distinct value count |

### 2.6 DQ Action Types

```rust
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum DqAction {
    #[default]
    Flag,    // Keep value, add to dq_flags
    Reject,  // Set to NULL, add to dq_flags
    Clamp,   // Clamp to bounds
    Drop,    // Drop entire row
    Warn,    // Log warning (batch-level)
}
```

---

## 3. Config Loading Path

### 3.1 Production: etcd via ConfigClient

**Location**: `apps/air-quality-app/src/config_etcd.rs`

```rust
// Load from etcd with environment variable overrides
pub async fn load_from_etcd() -> Result<EtcdAppConfig, Box<dyn std::error::Error>> {
    let etcd_endpoint = std::env::var("ETCD_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:2379".to_string());

    let client = ConfigClient::with_prefix(&[&etcd_endpoint], "/air-quality").await?;
    // ... load individual config sections
}
```

### 3.2 Testing: MockConfigLoader

**Location**: `core/src/config/mock_loader.rs`

```rust
#[async_trait]
pub trait ConfigLoader: Send + Sync {
    async fn load_stream_config(&self, stream_id: &str) -> Result<StreamConfig, ConfigLoaderError>;
    async fn load_silver_etl_config(&self, stream_id: &str) -> Result<SilverEtlConfig, ConfigLoaderError>;
    async fn list_streams(&self) -> Result<Vec<String>, ConfigLoaderError>;
    async fn stream_exists(&self, stream_id: &str) -> Result<bool, ConfigLoaderError>;
    fn source_name(&self) -> &'static str;
}
```

### 3.3 Config Sources

| Source | Format | Used By | Notes |
|--------|--------|---------|-------|
| etcd | JSON | Production | Synced from YAML files |
| YAML files | YAML | GitOps | `config/base/streams/*/config.yaml` |
| MockConfigLoader | In-memory | Tests | No serde involved |

---

## 4. Existing Validation

### 4.1 StreamConfig Validation

**Location**: `core/src/types/stream_config.rs::StreamConfig::validate()`

```rust
pub fn validate(&self) -> Result<(), StreamConfigError> {
    // 1. Validate stream_id format (kebab-case, 3-64 chars)
    if !is_valid_stream_id(&self.stream_id) {
        return Err(StreamConfigError::InvalidStreamId(self.stream_id.clone()));
    }

    // 2. Must have at least one field
    if self.fields.is_empty() {
        return Err(StreamConfigError::NoFields);
    }

    // 3. Must have at least one source
    if self.sources.is_empty() {
        return Err(StreamConfigError::NoSources);
    }

    // 4. Validate each field
    for field in &self.fields {
        field.validate()?;
    }

    Ok(())
}
```

### 4.2 SchemaField Validation

```rust
pub fn validate(&self) -> Result<(), StreamConfigError> {
    // 1. Field name format (snake_case)
    if !is_valid_field_name(&self.name) {
        return Err(StreamConfigError::InvalidFieldName(self.name.clone()));
    }

    // 2. Type-specific validation
    match self.field_type {
        FieldType::String | FieldType::Bool | FieldType::Json => {
            // Cannot have range or precision
        }
        FieldType::Int => {
            // Cannot have precision
        }
        FieldType::Float => {
            // Can have both range and precision
        }
    }

    // 3. Range validation (if present)
    if let Some(ref range) = self.range {
        if range.len() != 2 || range[0] >= range[1] {
            return Err(StreamConfigError::InvalidRange { ... });
        }
    }

    Ok(())
}
```

### 4.3 SilverEtlConfig Validation

**Location**: `core/src/config/silver_etl.rs::SilverEtlConfig::validate()`

```rust
pub fn validate(&self) -> Result<(), SilverConfigError> {
    // 1. target_table must start with "silver."
    if !self.target_table.starts_with("silver.") {
        return Err(SilverConfigError::InvalidTargetTable(...));
    }

    // 2. Validate field mappings (column types)
    for mapping in &self.field_mappings {
        mapping.validate()?;  // Checks VALID_TYPES
    }

    // 3. Validate DQ rules
    for rule in &self.dq_rules {
        rule.validate()?;
    }

    Ok(())
}
```

### 4.4 DQ Rule Validation

```rust
pub fn validate(&self) -> Result<(), SilverConfigError> {
    match self {
        DqRule::RangeCheck { min, max, field, .. } => {
            // Must have at least min or max
            // Min must be less than max
        }
        DqRule::CompletenessCheck { min_completeness, .. } => {
            // Must be between 0.0 and 1.0
        }
        DqRule::CardinalityCheck { expected_range, .. } => {
            // expected_range[0] must be <= expected_range[1]
        }
        _ => {}  // Other rules pass
    }
    Ok(())
}
```

---

## 5. Error Types

### 5.1 StreamConfigError

**Location**: `core/src/types/stream_config.rs`

```rust
#[derive(Debug, Error, PartialEq)]
pub enum StreamConfigError {
    #[error("Invalid stream ID: {0}")]
    InvalidStreamId(String),

    #[error("Invalid field name: {0}")]
    InvalidFieldName(String),

    #[error("Stream must have at least one field")]
    NoFields,

    #[error("Stream must have at least one source")]
    NoSources,

    #[error("Invalid field type for {field}: {reason}")]
    InvalidFieldType { field: String, reason: String },

    #[error("Invalid range for field {field}: {reason}")]
    InvalidRange { field: String, reason: String },
}
```

### 5.2 SilverConfigError

**Location**: `core/src/config/silver_etl.rs`

```rust
#[derive(Debug, Error, PartialEq)]
pub enum SilverConfigError {
    #[error("Invalid column type '{column_type}' for field '{field}'")]
    InvalidColumnType { field: String, column_type: String },

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Invalid DQ rule: {0}")]
    InvalidDqRule(String),

    #[error("Invalid target table: {0}")]
    InvalidTargetTable(String),

    #[error("Validation error: {0}")]
    ValidationError(String),
}
```

### 5.3 ConfigLoaderError

**Location**: `core/src/config/mock_loader.rs`

```rust
#[derive(Debug, Error, Clone)]
pub enum ConfigLoaderError {
    #[error("Stream not found: {0}")]
    StreamNotFound(String),

    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),
}
```

### 5.4 CoreError

**Location**: `core/src/error.rs`

```rust
#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Source error: {0}")]
    Source(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Parser error: {0}")]
    Parser(String),

    // ... other variants
}
```

---

## 6. Validation Gaps (What dp-019 Will Add)

### 6.1 Schema-Level Gaps

| Gap | Current State | dp-019 Solution |
|-----|---------------|-----------------|
| No JSON syntax validation | serde panics on malformed JSON | Layer 1: JSON syntax check with line numbers |
| No schema validation | Accepts any structure | Layer 1: JSON Schema with `jsonschema` crate |
| Unknown fields accepted | `serde(flatten)` captures extras | Layer 1: `additionalProperties: false` |
| YAML not validated | Deserialized directly | Layer 1: Validate YAML before etcd sync |

### 6.2 Semantic-Level Gaps

| Gap | Current State | dp-019 Solution |
|-----|---------------|-----------------|
| No source_path reference check | Silent field skipping | Layer 2: Cross-reference `source_path` against `fields` |
| No table existence check | INSERT fails at runtime | Layer 2: Check TimescaleDB for target table |
| Unknown DQ operators | Invalid SQL at runtime | Layer 2: Validate DQ expressions |
| No source config validation | Missing broker_url accepted | Layer 2: Validate required params per source type |

### 6.3 Missing Validations by Config Area

**StreamConfig**:
- No validation that `fields` dict keys match `field.name` (YAML dict format)
- No validation of `partitioning_strategy` values

**SilverEtlConfig**:
- No validation that `source_path` references exist in Bronze schema
- No validation that `target_table` exists in TimescaleDB
- No validation of DQ rule SQL expressions

**SourceConfig**:
- No validation of required params for each source type
- No validation of `broker_url`, `topic_pattern` format for MQTT
- No validation of HTTP endpoint format

---

## 7. Recommended Crates for dp-019

### 7.1 JSON Schema Validation: `jsonschema`

**Purpose**: Layer 1 schema validation

```rust
use jsonschema::{JSONSchema, Draft};
use serde_json::json;

// Compile schema once
let schema = json!({
    "type": "object",
    "required": ["stream_id", "fields", "sources"],
    "properties": {
        "stream_id": { "type": "string", "pattern": "^[a-z][a-z0-9-]{2,63}$" },
        "fields": { "type": "array", "minItems": 1 }
    },
    "additionalProperties": false
});
let compiled = JSONSchema::options()
    .with_draft(Draft::Draft7)
    .compile(&schema)?;

// Validate instance
let instance = json!({ "stream_id": "air-quality", ... });
let result = compiled.validate(&instance);
if let Err(errors) = result {
    for error in errors {
        println!("Path: {}, Message: {}", error.instance_path, error);
    }
}
```

**Crate Features**:
- Draft 4, 6, 7, 2019-09, 2020-12 support
- Detailed error paths
- Custom format validators
- No additional dependencies for core validation

**Cargo.toml**:
```toml
jsonschema = "0.17"  # or latest
```

### 7.2 Schema Generation: `schemars`

**Purpose**: Generate JSON Schema from Rust structs

```rust
use schemars::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};

#[derive(JsonSchema, Serialize, Deserialize)]
#[schemars(deny_unknown_fields)]  // additionalProperties: false
pub struct StreamConfig {
    /// Unique stream identifier (kebab-case)
    #[schemars(regex(pattern = r"^[a-z][a-z0-9-]{2,63}$"))]
    pub stream_id: String,

    #[schemars(length(min = 1))]
    pub fields: Vec<SchemaField>,

    // ...
}

// Generate schema
let schema = schema_for!(StreamConfig);
let json = serde_json::to_string_pretty(&schema)?;
```

**Cargo.toml**:
```toml
schemars = "0.8"
```

**Trade-off**: Consider whether to:
1. **Generate from Rust structs** (schemars) - Single source of truth, but limits schema expressiveness
2. **Hand-write schemas** - More control, but must keep in sync with Rust structs

**Recommendation**: Use schemars for initial generation, then hand-tune for semantic rules.

### 7.3 Error Types: `thiserror`

**Purpose**: Already used in NDP for error types

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Schema validation failed at {path}: {message}")]
    SchemaError { path: String, message: String },

    #[error("Semantic validation failed: {0}")]
    SemanticError(String),

    #[error("Source path '{source_path}' not found in fields")]
    InvalidSourcePath { source_path: String },
}
```

### 7.4 Alternative: `valico`

**Purpose**: Alternative JSON Schema validator with builder pattern

```rust
use valico::json_schema;

let mut scope = json_schema::Scope::new();
let schema = scope.compile_and_return(schema_json, true)?;

let state = schema.validate(&instance);
if !state.is_valid() {
    for error in state.errors {
        println!("Path: {:?}, Code: {}", error.get_path(), error.get_code());
    }
}
```

**Trade-off**: Older, less maintained than `jsonschema`, but has builder API.

---

## 8. Type Mapping Reference

### 8.1 Bronze FieldType to PostgreSQL

| FieldType (Bronze) | PostgreSQL Type | Notes |
|--------------------|-----------------|-------|
| `float` | `DOUBLE PRECISION` | 64-bit float |
| `int` | `BIGINT` | 64-bit integer |
| `string` | `TEXT` | Variable length |
| `bool` | `BOOLEAN` | True/false |
| `json` | `JSONB` | Binary JSON |
| (timestamp) | `TIMESTAMPTZ` | With timezone |

### 8.2 Silver column_type to PostgreSQL

| Silver column_type | PostgreSQL Type | Notes |
|--------------------|-----------------|-------|
| `double_precision` | `DOUBLE PRECISION` | Recommended for floats |
| `real` | `REAL` | 32-bit float |
| `integer` | `INTEGER` | 32-bit integer |
| `bigint` | `BIGINT` | 64-bit integer |
| `smallint` | `SMALLINT` | 16-bit integer |
| `text` | `TEXT` | Variable length |
| `varchar` | `VARCHAR` | Variable length (consider TEXT instead) |
| `boolean` | `BOOLEAN` | True/false |
| `timestamptz` | `TIMESTAMPTZ` | With timezone |
| `jsonb` | `JSONB` | Binary JSON |
| `text[]` | `TEXT[]` | Text array |

---

## 9. Summary for dp-019 Implementation

### 9.1 What Exists

1. **Rust structs** with serde derive for config representation
2. **Validation methods** on StreamConfig, SchemaField, SilverEtlConfig
3. **Error types** using thiserror
4. **ConfigLoader trait** abstraction for loading
5. **Comprehensive test coverage** of existing validation

### 9.2 What dp-019 Must Add

1. **Layer 1: Schema Validation**
   - JSON syntax validation (serde_json error handling)
   - JSON Schema validation (jsonschema crate)
   - Unknown field detection (additionalProperties: false)

2. **Layer 2: Semantic Validation**
   - source_path reference checking against fields
   - target_table existence check (TimescaleDB query)
   - Source-type-specific required fields
   - DQ expression syntax validation

3. **Integration**
   - Validator CLI tool
   - deploy.sh integration
   - Structured error output (JSON)

### 9.3 Recommended Architecture

```
tools/ndp-validate/
├── src/
│   ├── main.rs              # CLI entry point
│   ├── schema/
│   │   ├── mod.rs           # Layer 1: JSON Schema validation
│   │   ├── stream.json      # StreamConfig schema
│   │   └── silver.json      # SilverEtlConfig schema
│   ├── semantic/
│   │   ├── mod.rs           # Layer 2: Semantic validation
│   │   ├── source_paths.rs  # Cross-reference validation
│   │   └── table_check.rs   # TimescaleDB table existence
│   └── output.rs            # Structured error formatting
└── Cargo.toml
```

---

*Analysis completed: 2026-02-02*
*Author: ndp-rust-dev agent*
*Feature: dp-019 Config Validation Pipeline*
