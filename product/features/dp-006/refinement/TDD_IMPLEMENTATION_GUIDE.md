# DP-006: TDD Implementation Guide

**Document**: TDD_IMPLEMENTATION_GUIDE.md
**Version**: 1.0
**Date**: 2026-01-10
**Author**: NDP Tester
**Status**: Active
**Feature**: DP-006 (Silver Layer Implementation)

---

## Executive Summary

This guide defines the Test-Driven Development (TDD) approach for implementing the Silver ETL binary. Following the Red-Green-Refactor cycle, each component is developed test-first to ensure correctness, maintainability, and alignment with the config-driven architecture.

### TDD Principles for DP-006

| Principle | Application |
|-----------|-------------|
| **Test First** | Write failing test before implementation |
| **Small Steps** | One test at a time, minimal code to pass |
| **Refactor Often** | Clean up after each green phase |
| **Config-Driven** | All tests validate config-driven behavior |
| **DQ Transparency** | Tests verify flag generation correctness |

---

## 1. Development Phases Overview

```
Phase 1: Config Types (core/src/config/silver_etl.rs)
   └── Foundation: Parse and validate YAML configs

Phase 2: SQL Generator (apps/silver-etl/src/sql_gen.rs)
   └── Core: Generate DuckDB SQL from config

Phase 3: DQ Evaluator (apps/silver-etl/src/dq.rs)
   └── Quality: Generate DQ check expressions

Phase 4: ETL Runner (apps/silver-etl/src/etl.rs)
   └── Execution: DuckDB + TimescaleDB integration

Phase 5: Integration (apps/silver-etl/src/main.rs)
   └── Orchestration: Full pipeline tests
```

---

## 2. Module Structure

```
apps/silver-etl/
├── Cargo.toml
├── src/
│   ├── main.rs           # CLI entry point + orchestration
│   ├── config.rs         # Config loading from etcd/files
│   ├── sql_gen.rs        # SQL generation from config
│   ├── dq.rs             # DQ rule evaluation + flag generation
│   ├── etl.rs            # ETL execution engine
│   ├── metrics.rs        # Prometheus metrics
│   └── lib.rs            # Library exports for testing
└── tests/
    ├── config_tests.rs   # Config parsing tests
    ├── sql_gen_tests.rs  # SQL generation tests
    ├── dq_tests.rs       # DQ rule tests
    ├── etl_tests.rs      # ETL runner tests
    ├── integration_tests.rs  # Full pipeline tests
    └── fixtures/
        ├── air_quality_config.yaml
        ├── weather_config.yaml
        ├── sample_bronze_data.parquet
        └── expected_silver_data.sql

core/src/config/
├── mod.rs                # Module exports
└── silver_etl.rs         # Silver ETL config types (NEW)
```

---

## 3. Phase 1: Config Types

### 3.1 Module Location

`core/src/config/silver_etl.rs` - Config types shared between core and apps.

### 3.2 Development Order

```
Test 1: Parse minimal valid silver_etl config
Test 2: Parse complete silver_etl config with all fields
Test 3: Parse field_mappings with transforms
Test 4: Parse DQ rules (range_check)
Test 5: Parse DQ rules (all types)
Test 6: Validate rejects invalid config (missing target_table)
Test 7: Validate rejects invalid config (bad field type)
Test 8: Default values applied correctly
```

### 3.3 Test-First Examples

```rust
// core/src/config/silver_etl.rs

#[cfg(test)]
mod tests {
    use super::*;
    use serde_yaml;

    // ============================================================
    // Test 1: Parse minimal valid config (RED -> GREEN)
    // ============================================================
    #[test]
    fn test_parse_minimal_silver_etl_config() {
        let yaml = r#"
enabled: true
target_table: silver.air_quality_observations
timestamp:
  source_field: timestamp
  target_field: observation_time
  transform: microseconds_to_timestamp
field_mappings: []
"#;

        let config: SilverEtlConfig = serde_yaml::from_str(yaml)
            .expect("Should parse minimal config");

        assert!(config.enabled);
        assert_eq!(config.target_table, "silver.air_quality_observations");
        assert_eq!(config.timestamp.source_field, "timestamp");
        assert_eq!(config.timestamp.target_field, "observation_time");
        assert!(matches!(
            config.timestamp.transform,
            TimestampTransform::MicrosecondsToTimestamp
        ));
    }

    // ============================================================
    // Test 2: Parse complete config with all optional fields
    // ============================================================
    #[test]
    fn test_parse_complete_silver_etl_config() {
        let yaml = r#"
enabled: true
target_table: silver.air_quality_observations
target_schema: air_quality_observations_v1
timestamp:
  source_field: timestamp
  target_field: observation_time
  transform: microseconds_to_timestamp
identity_fields:
  - source: ndp_id
    target: ndp_id
  - source: context.location.path
    target: location_path
field_mappings:
  - source_path: raw_payload.pm02
    target_column: pm25
    type: double_precision
    nullable: false
dq_output:
  enabled: true
  target_column: dq_flags
  include_rules: true
  include_values: false
deduplication:
  enabled: true
  key_columns: [observation_time, ndp_id]
  strategy: upsert
incremental:
  enabled: true
  watermark_column: observation_time
  lag_interval: 5 minutes
"#;

        let config: SilverEtlConfig = serde_yaml::from_str(yaml)
            .expect("Should parse complete config");

        assert!(config.enabled);
        assert_eq!(config.target_schema, Some("air_quality_observations_v1".to_string()));
        assert_eq!(config.identity_fields.len(), 2);
        assert_eq!(config.field_mappings.len(), 1);
        assert!(config.dq_output.enabled);
        assert!(config.deduplication.enabled);
        assert!(config.incremental.enabled);
        assert_eq!(config.incremental.lag_interval, "5 minutes");
    }

    // ============================================================
    // Test 3: Parse field mapping with unit conversion transform
    // ============================================================
    #[test]
    fn test_parse_field_mapping_with_transform() {
        let yaml = r#"
source_path: raw_payload.main.temp
target_column: temperature_c
type: double_precision
nullable: true
transform:
  type: unit_conversion
  from: kelvin
  to: celsius
  formula:
    type: linear
    scale: 1.0
    offset: -273.15
"#;

        let mapping: SilverFieldMapping = serde_yaml::from_str(yaml)
            .expect("Should parse field mapping with transform");

        assert_eq!(mapping.source_path, "raw_payload.main.temp");
        assert_eq!(mapping.target_column, "temperature_c");
        assert_eq!(mapping.column_type, "double_precision");
        assert!(mapping.nullable);

        match mapping.transform {
            Some(TransformConfig::UnitConversion { from, to, formula }) => {
                assert_eq!(from, "kelvin");
                assert_eq!(to, "celsius");
                match formula {
                    ConversionFormula::Linear { scale, offset } => {
                        assert!((scale - 1.0).abs() < f64::EPSILON);
                        assert!((offset - (-273.15)).abs() < f64::EPSILON);
                    }
                    _ => panic!("Expected Linear formula"),
                }
            }
            _ => panic!("Expected UnitConversion transform"),
        }
    }

    // ============================================================
    // Test 4: Parse DQ rule - range_check with flag action
    // ============================================================
    #[test]
    fn test_parse_dq_rule_range_check() {
        let yaml = r#"
rule: range_check
field: pm25
min: 0.0
max: 1000.0
action: flag
"#;

        let rule: DqRule = serde_yaml::from_str(yaml)
            .expect("Should parse range_check rule");

        match rule {
            DqRule::RangeCheck { field, min, max, action, .. } => {
                assert_eq!(field, "pm25");
                assert_eq!(min, Some(0.0));
                assert_eq!(max, Some(1000.0));
                assert!(matches!(action, DqAction::Flag));
            }
            _ => panic!("Expected RangeCheck rule"),
        }
    }

    // ============================================================
    // Test 5: Parse DQ rule - range_check with clamp action
    // ============================================================
    #[test]
    fn test_parse_dq_rule_range_check_clamp() {
        let yaml = r#"
rule: range_check
field: humidity_pct
min: 0.0
max: 100.0
action: clamp
clamp_to_bounds: true
"#;

        let rule: DqRule = serde_yaml::from_str(yaml)
            .expect("Should parse range_check with clamp");

        match rule {
            DqRule::RangeCheck { field, min, max, action, clamp_to_bounds } => {
                assert_eq!(field, "humidity_pct");
                assert_eq!(min, Some(0.0));
                assert_eq!(max, Some(100.0));
                assert!(matches!(action, DqAction::Clamp));
                assert!(clamp_to_bounds);
            }
            _ => panic!("Expected RangeCheck rule"),
        }
    }

    // ============================================================
    // Test 6: Parse DQ rule - null_check with reject action
    // ============================================================
    #[test]
    fn test_parse_dq_rule_null_check() {
        let yaml = r#"
rule: null_check
field: observation_time
action: reject
"#;

        let rule: DqRule = serde_yaml::from_str(yaml)
            .expect("Should parse null_check rule");

        match rule {
            DqRule::NullCheck { field, action } => {
                assert_eq!(field, "observation_time");
                assert!(matches!(action, DqAction::Reject));
            }
            _ => panic!("Expected NullCheck rule"),
        }
    }

    // ============================================================
    // Test 7: Parse DQ rule - cross_field_check
    // ============================================================
    #[test]
    fn test_parse_dq_rule_cross_field_check() {
        let yaml = r#"
rule: cross_field_check
name: pm10_gte_pm25
expression: "pm10 IS NULL OR pm25 IS NULL OR pm10 >= pm25"
message: pm10_less_than_pm25
action: flag
"#;

        let rule: DqRule = serde_yaml::from_str(yaml)
            .expect("Should parse cross_field_check rule");

        match rule {
            DqRule::CrossFieldCheck { name, expression, message, action } => {
                assert_eq!(name, "pm10_gte_pm25");
                assert!(expression.contains("pm10 >= pm25"));
                assert_eq!(message, Some("pm10_less_than_pm25".to_string()));
                assert!(matches!(action, DqAction::Flag));
            }
            _ => panic!("Expected CrossFieldCheck rule"),
        }
    }

    // ============================================================
    // Test 8: Validation rejects config without target_table
    // ============================================================
    #[test]
    fn test_validate_rejects_missing_target_table() {
        let yaml = r#"
enabled: true
timestamp:
  source_field: timestamp
  target_field: observation_time
  transform: microseconds_to_timestamp
field_mappings: []
"#;

        let result: Result<SilverEtlConfig, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err(), "Should fail without target_table");
    }

    // ============================================================
    // Test 9: Validation rejects invalid column type
    // ============================================================
    #[test]
    fn test_validate_rejects_invalid_column_type() {
        let config = SilverEtlConfig {
            enabled: true,
            target_table: "silver.test".to_string(),
            target_schema: None,
            timestamp: TimestampMapping {
                source_field: "timestamp".to_string(),
                target_field: "observation_time".to_string(),
                transform: TimestampTransform::MicrosecondsToTimestamp,
            },
            identity_fields: vec![],
            field_mappings: vec![
                SilverFieldMapping {
                    source_path: "raw_payload.pm02".to_string(),
                    target_column: "pm25".to_string(),
                    column_type: "invalid_type".to_string(),  // Invalid!
                    nullable: true,
                    transform: None,
                    dq_rules: vec![],
                },
            ],
            dq_rules: vec![],
            dq_output: DqOutputConfig::default(),
            deduplication: DeduplicationConfig::default(),
            incremental: IncrementalConfig::default(),
        };

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid_type"));
    }

    // ============================================================
    // Test 10: Default values applied when not specified
    // ============================================================
    #[test]
    fn test_default_values_applied() {
        let yaml = r#"
enabled: true
target_table: silver.test
timestamp:
  source_field: timestamp
  target_field: observation_time
  transform: microseconds_to_timestamp
field_mappings: []
"#;

        let config: SilverEtlConfig = serde_yaml::from_str(yaml)
            .expect("Should parse with defaults");

        // DQ output defaults
        assert!(!config.dq_output.enabled);  // Default false
        assert_eq!(config.dq_output.target_column, "dq_flags");

        // Deduplication defaults
        assert!(!config.deduplication.enabled);
        assert!(matches!(config.deduplication.strategy, DeduplicationStrategy::Upsert));

        // Incremental defaults
        assert!(!config.incremental.enabled);
    }
}
```

### 3.4 Implementation Skeleton

After tests pass, the implementation should look like:

```rust
// core/src/config/silver_etl.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Silver ETL configuration errors
#[derive(Debug, Error)]
pub enum SilverConfigError {
    #[error("Invalid column type '{column_type}' for field '{field}'")]
    InvalidColumnType { field: String, column_type: String },

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Invalid DQ rule: {0}")]
    InvalidDqRule(String),
}

/// Silver ETL configuration for a stream
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SilverEtlConfig {
    pub enabled: bool,
    pub target_table: String,
    #[serde(default)]
    pub target_schema: Option<String>,
    pub timestamp: TimestampMapping,
    #[serde(default)]
    pub identity_fields: Vec<IdentityField>,
    #[serde(default)]
    pub field_mappings: Vec<SilverFieldMapping>,
    #[serde(default)]
    pub dq_rules: Vec<DqRule>,
    #[serde(default)]
    pub dq_output: DqOutputConfig,
    #[serde(default)]
    pub deduplication: DeduplicationConfig,
    #[serde(default)]
    pub incremental: IncrementalConfig,
}

impl SilverEtlConfig {
    /// Validate configuration
    pub fn validate(&self) -> Result<(), SilverConfigError> {
        // Validate target_table format
        if !self.target_table.starts_with("silver.") {
            return Err(SilverConfigError::MissingField(
                "target_table must start with 'silver.'".to_string()
            ));
        }

        // Validate field mappings
        for mapping in &self.field_mappings {
            mapping.validate()?;
        }

        Ok(())
    }
}

/// Timestamp mapping configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TimestampMapping {
    pub source_field: String,
    pub target_field: String,
    pub transform: TimestampTransform,
}

/// Timestamp transform types
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TimestampTransform {
    MicrosecondsToTimestamp,
    Iso8601,
    UnixSeconds,
    NwsDuration,
}

/// Identity field passthrough
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IdentityField {
    pub source: String,
    pub target: String,
}

/// Field mapping for Silver ETL
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SilverFieldMapping {
    pub source_path: String,
    pub target_column: String,
    #[serde(rename = "type")]
    pub column_type: String,
    #[serde(default = "default_true")]
    pub nullable: bool,
    #[serde(default)]
    pub transform: Option<TransformConfig>,
    #[serde(default)]
    pub dq_rules: Vec<DqRule>,
}

impl SilverFieldMapping {
    fn validate(&self) -> Result<(), SilverConfigError> {
        const VALID_TYPES: &[&str] = &[
            "double_precision", "real", "integer", "bigint", "smallint",
            "text", "varchar", "boolean", "timestamptz", "jsonb",
        ];

        if !VALID_TYPES.contains(&self.column_type.as_str()) {
            return Err(SilverConfigError::InvalidColumnType {
                field: self.target_column.clone(),
                column_type: self.column_type.clone(),
            });
        }

        Ok(())
    }
}

fn default_true() -> bool { true }

/// Transform configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TransformConfig {
    UnitConversion {
        from: String,
        to: String,
        formula: ConversionFormula,
    },
    Expression {
        expr: String,
    },
    Lookup {
        table: HashMap<String, String>,
    },
    JsonExtract {
        path: String,
    },
    Timestamp {
        format: TimestampTransform,
    },
    Computed {
        depends_on: Vec<String>,
        expr: String,
    },
}

/// Conversion formula types
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConversionFormula {
    Linear { scale: f64, offset: f64 },
    Custom { code: String },
}

/// DQ rule configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "rule", rename_all = "snake_case")]
pub enum DqRule {
    RangeCheck {
        field: String,
        #[serde(default)]
        min: Option<f64>,
        #[serde(default)]
        max: Option<f64>,
        #[serde(default)]
        action: DqAction,
        #[serde(default = "default_true")]
        clamp_to_bounds: bool,
    },
    NullCheck {
        field: String,
        #[serde(default = "default_reject")]
        action: DqAction,
    },
    EnumCheck {
        field: String,
        allowed_values: Vec<String>,
        #[serde(default)]
        case_sensitive: bool,
        #[serde(default)]
        action: DqAction,
    },
    PatternCheck {
        field: String,
        pattern: String,
        #[serde(default)]
        action: DqAction,
    },
    FreshnessCheck {
        field: String,
        #[serde(default)]
        max_age: Option<String>,
        #[serde(default)]
        max_future: Option<String>,
        #[serde(default = "default_ingestion_time")]
        reference: String,
        #[serde(default)]
        action: DqAction,
    },
    RateOfChange {
        field: String,
        max_change_per_minute: f64,
        partition_by: Vec<String>,
        #[serde(default)]
        action: DqAction,
    },
    CrossFieldCheck {
        name: String,
        expression: String,
        #[serde(default)]
        message: Option<String>,
        #[serde(default)]
        action: DqAction,
    },
}

/// DQ action types
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DqAction {
    #[default]
    Flag,
    Reject,
    Clamp,
    Drop,
    Warn,
}

fn default_reject() -> DqAction { DqAction::Reject }
fn default_ingestion_time() -> String { "ingestion_time".to_string() }

/// DQ output configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DqOutputConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_dq_flags")]
    pub target_column: String,
    #[serde(default = "default_true")]
    pub include_rules: bool,
    #[serde(default)]
    pub include_values: bool,
}

impl Default for DqOutputConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            target_column: "dq_flags".to_string(),
            include_rules: true,
            include_values: false,
        }
    }
}

fn default_dq_flags() -> String { "dq_flags".to_string() }

/// Deduplication configuration
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct DeduplicationConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub key_columns: Vec<String>,
    #[serde(default)]
    pub strategy: DeduplicationStrategy,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DeduplicationStrategy {
    #[default]
    Upsert,
    Skip,
    Replace,
}

/// Incremental load configuration
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct IncrementalConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub watermark_column: String,
    #[serde(default)]
    pub lag_interval: String,
}
```

---

## 4. Phase 2: SQL Generator

### 4.1 Module Location

`apps/silver-etl/src/sql_gen.rs` - Generates DuckDB SQL from config.

### 4.2 Development Order

```
Test 1: Generate SELECT for simple field mapping (no transform)
Test 2: Generate SELECT for field with unit conversion
Test 3: Generate SELECT for json_extract transform
Test 4: Generate timestamp transform expression
Test 5: Generate identity field expressions
Test 6: Generate complete SELECT clause with multiple fields
Test 7: Generate FROM clause with parquet glob pattern
Test 8: Generate WHERE clause for incremental watermark
Test 9: Generate INSERT statement for TimescaleDB
Test 10: Generate ON CONFLICT clause for upsert
Test 11: Generate complete ETL SQL statement
```

### 4.3 Test-First Examples

```rust
// apps/silver-etl/src/sql_gen.rs

#[cfg(test)]
mod tests {
    use super::*;
    use neural_core::config::silver_etl::*;

    // ============================================================
    // Test 1: Generate SELECT for simple field (no transform)
    // ============================================================
    #[test]
    fn test_generate_select_simple_field() {
        let mapping = SilverFieldMapping {
            source_path: "raw_payload.pm02".to_string(),
            target_column: "pm25".to_string(),
            column_type: "double_precision".to_string(),
            nullable: true,
            transform: None,
            dq_rules: vec![],
        };

        let gen = SqlGenerator::new();
        let sql = gen.generate_select_expr(&mapping);

        assert_eq!(
            sql,
            "CAST(json_extract(raw_payload, '$.pm02') AS DOUBLE) AS pm25"
        );
    }

    // ============================================================
    // Test 2: Generate SELECT with unit conversion transform
    // ============================================================
    #[test]
    fn test_generate_select_with_unit_conversion() {
        let mapping = SilverFieldMapping {
            source_path: "raw_payload.main.temp".to_string(),
            target_column: "temperature_c".to_string(),
            column_type: "double_precision".to_string(),
            nullable: true,
            transform: Some(TransformConfig::UnitConversion {
                from: "kelvin".to_string(),
                to: "celsius".to_string(),
                formula: ConversionFormula::Linear {
                    scale: 1.0,
                    offset: -273.15,
                },
            }),
            dq_rules: vec![],
        };

        let gen = SqlGenerator::new();
        let sql = gen.generate_select_expr(&mapping);

        assert_eq!(
            sql,
            "(CAST(json_extract(raw_payload, '$.main.temp') AS DOUBLE) * 1.0 + -273.15) AS temperature_c"
        );
    }

    // ============================================================
    // Test 3: Generate SELECT with json_extract transform
    // ============================================================
    #[test]
    fn test_generate_select_with_json_extract() {
        let mapping = SilverFieldMapping {
            source_path: "raw_payload".to_string(),
            target_column: "aqi".to_string(),
            column_type: "integer".to_string(),
            nullable: true,
            transform: Some(TransformConfig::JsonExtract {
                path: "$.list[0].main.aqi".to_string(),
            }),
            dq_rules: vec![],
        };

        let gen = SqlGenerator::new();
        let sql = gen.generate_select_expr(&mapping);

        assert_eq!(
            sql,
            "CAST(json_extract(raw_payload, '$.list[0].main.aqi') AS INTEGER) AS aqi"
        );
    }

    // ============================================================
    // Test 4: Generate timestamp transform expression
    // ============================================================
    #[test]
    fn test_generate_timestamp_microseconds() {
        let ts_mapping = TimestampMapping {
            source_field: "timestamp".to_string(),
            target_field: "observation_time".to_string(),
            transform: TimestampTransform::MicrosecondsToTimestamp,
        };

        let gen = SqlGenerator::new();
        let sql = gen.generate_timestamp_expr(&ts_mapping);

        assert_eq!(
            sql,
            "to_timestamp(timestamp / 1000000) AS observation_time"
        );
    }

    #[test]
    fn test_generate_timestamp_iso8601() {
        let ts_mapping = TimestampMapping {
            source_field: "raw_payload.properties.timestamp".to_string(),
            target_field: "observation_time".to_string(),
            transform: TimestampTransform::Iso8601,
        };

        let gen = SqlGenerator::new();
        let sql = gen.generate_timestamp_expr(&ts_mapping);

        assert_eq!(
            sql,
            "CAST(json_extract_string(raw_payload, '$.properties.timestamp') AS TIMESTAMPTZ) AS observation_time"
        );
    }

    // ============================================================
    // Test 5: Generate identity field expressions
    // ============================================================
    #[test]
    fn test_generate_identity_field_simple() {
        let identity = IdentityField {
            source: "ndp_id".to_string(),
            target: "ndp_id".to_string(),
        };

        let gen = SqlGenerator::new();
        let sql = gen.generate_identity_expr(&identity);

        assert_eq!(sql, "ndp_id AS ndp_id");
    }

    #[test]
    fn test_generate_identity_field_json_path() {
        let identity = IdentityField {
            source: "context.location.path".to_string(),
            target: "location_path".to_string(),
        };

        let gen = SqlGenerator::new();
        let sql = gen.generate_identity_expr(&identity);

        assert_eq!(
            sql,
            "json_extract_string(context, '$.location.path') AS location_path"
        );
    }

    // ============================================================
    // Test 6: Generate complete SELECT clause
    // ============================================================
    #[test]
    fn test_generate_select_clause_complete() {
        let config = SilverEtlConfig {
            enabled: true,
            target_table: "silver.air_quality".to_string(),
            target_schema: None,
            timestamp: TimestampMapping {
                source_field: "timestamp".to_string(),
                target_field: "observation_time".to_string(),
                transform: TimestampTransform::MicrosecondsToTimestamp,
            },
            identity_fields: vec![
                IdentityField {
                    source: "ndp_id".to_string(),
                    target: "ndp_id".to_string(),
                },
            ],
            field_mappings: vec![
                SilverFieldMapping {
                    source_path: "raw_payload.pm02".to_string(),
                    target_column: "pm25".to_string(),
                    column_type: "double_precision".to_string(),
                    nullable: true,
                    transform: None,
                    dq_rules: vec![],
                },
            ],
            dq_rules: vec![],
            dq_output: DqOutputConfig::default(),
            deduplication: DeduplicationConfig::default(),
            incremental: IncrementalConfig::default(),
        };

        let gen = SqlGenerator::new();
        let sql = gen.generate_select_clause(&config);

        assert!(sql.contains("to_timestamp(timestamp / 1000000) AS observation_time"));
        assert!(sql.contains("ndp_id AS ndp_id"));
        assert!(sql.contains("json_extract(raw_payload, '$.pm02')"));
    }

    // ============================================================
    // Test 7: Generate FROM clause with parquet glob
    // ============================================================
    #[test]
    fn test_generate_from_clause() {
        let gen = SqlGenerator::new();
        let sql = gen.generate_from_clause("air-quality", "/data/raw");

        assert_eq!(
            sql,
            "FROM read_parquet('/data/raw/air-quality/**/*.parquet')"
        );
    }

    // ============================================================
    // Test 8: Generate WHERE clause for incremental
    // ============================================================
    #[test]
    fn test_generate_where_clause_incremental() {
        let incremental = IncrementalConfig {
            enabled: true,
            watermark_column: "observation_time".to_string(),
            lag_interval: "5 minutes".to_string(),
        };

        let gen = SqlGenerator::new();
        let sql = gen.generate_where_clause(&incremental, "silver.air_quality");

        assert!(sql.contains("to_timestamp(timestamp / 1000000) >"));
        assert!(sql.contains("SELECT COALESCE(MAX(observation_time)"));
        assert!(sql.contains("FROM pg.silver.air_quality"));
        assert!(sql.contains("INTERVAL '5 minutes'"));
    }

    // ============================================================
    // Test 9: Generate INSERT with ON CONFLICT (upsert)
    // ============================================================
    #[test]
    fn test_generate_upsert_clause() {
        let dedup = DeduplicationConfig {
            enabled: true,
            key_columns: vec!["observation_time".to_string(), "ndp_id".to_string()],
            strategy: DeduplicationStrategy::Upsert,
        };
        let columns = vec!["observation_time", "ndp_id", "pm25", "dq_flags"];

        let gen = SqlGenerator::new();
        let sql = gen.generate_upsert_clause(&dedup, &columns);

        assert!(sql.contains("ON CONFLICT (observation_time, ndp_id)"));
        assert!(sql.contains("DO UPDATE SET"));
        assert!(sql.contains("pm25 = EXCLUDED.pm25"));
        assert!(sql.contains("dq_flags = EXCLUDED.dq_flags"));
    }

    // ============================================================
    // Test 10: Generate complete ETL SQL
    // ============================================================
    #[test]
    fn test_generate_complete_etl_sql() {
        let config = create_test_config();

        let gen = SqlGenerator::new();
        let sql = gen.generate_etl_sql(&config, "air-quality", "/data/raw")
            .expect("Should generate ETL SQL");

        // Verify structure
        assert!(sql.contains("INSERT INTO pg.silver.air_quality"));
        assert!(sql.contains("SELECT"));
        assert!(sql.contains("FROM read_parquet"));
        assert!(sql.contains("WHERE"));
        assert!(sql.contains("ON CONFLICT"));
    }

    fn create_test_config() -> SilverEtlConfig {
        SilverEtlConfig {
            enabled: true,
            target_table: "silver.air_quality".to_string(),
            target_schema: None,
            timestamp: TimestampMapping {
                source_field: "timestamp".to_string(),
                target_field: "observation_time".to_string(),
                transform: TimestampTransform::MicrosecondsToTimestamp,
            },
            identity_fields: vec![
                IdentityField {
                    source: "ndp_id".to_string(),
                    target: "ndp_id".to_string(),
                },
            ],
            field_mappings: vec![
                SilverFieldMapping {
                    source_path: "raw_payload.pm02".to_string(),
                    target_column: "pm25".to_string(),
                    column_type: "double_precision".to_string(),
                    nullable: true,
                    transform: None,
                    dq_rules: vec![],
                },
            ],
            dq_rules: vec![],
            dq_output: DqOutputConfig { enabled: true, ..Default::default() },
            deduplication: DeduplicationConfig {
                enabled: true,
                key_columns: vec!["observation_time".to_string(), "ndp_id".to_string()],
                strategy: DeduplicationStrategy::Upsert,
            },
            incremental: IncrementalConfig {
                enabled: true,
                watermark_column: "observation_time".to_string(),
                lag_interval: "5 minutes".to_string(),
            },
        }
    }
}
```

---

## 5. Phase 3: DQ Evaluator

### 5.1 Module Location

`apps/silver-etl/src/dq.rs` - DQ rule SQL generation.

### 5.2 Development Order

```
Test 1: range_check generates correct CASE expression (flag)
Test 2: range_check generates correct CASE expression (clamp)
Test 3: range_check generates correct CASE expression (reject)
Test 4: null_check generates correct CASE expression
Test 5: enum_check generates correct IN expression
Test 6: pattern_check generates correct regex expression
Test 7: cross_field_check generates correct expression
Test 8: Multiple rules generate ARRAY_REMOVE construct
Test 9: dq_flags array construction is correct
Test 10: Clamp action generates LEAST/GREATEST expression
```

### 5.3 Test-First Examples

```rust
// apps/silver-etl/src/dq.rs

#[cfg(test)]
mod tests {
    use super::*;
    use neural_core::config::silver_etl::*;

    // ============================================================
    // Test 1: range_check with flag action
    // ============================================================
    #[test]
    fn test_range_check_flag_sql() {
        let rule = DqRule::RangeCheck {
            field: "pm25".to_string(),
            min: Some(0.0),
            max: Some(1000.0),
            action: DqAction::Flag,
            clamp_to_bounds: false,
        };

        let gen = DqSqlGenerator::new();
        let sql = gen.generate_check_sql(&rule);

        assert_eq!(
            sql,
            r#"CASE
  WHEN pm25 < 0.0 OR pm25 > 1000.0
  THEN 'range_check:pm25:out_of_bounds'
  ELSE NULL
END"#
        );
    }

    // ============================================================
    // Test 2: range_check with clamp action (value expression)
    // ============================================================
    #[test]
    fn test_range_check_clamp_value_sql() {
        let rule = DqRule::RangeCheck {
            field: "humidity_pct".to_string(),
            min: Some(0.0),
            max: Some(100.0),
            action: DqAction::Clamp,
            clamp_to_bounds: true,
        };

        let gen = DqSqlGenerator::new();
        let sql = gen.generate_value_expr(&rule, "humidity_pct_raw");

        assert_eq!(
            sql,
            "LEAST(GREATEST(humidity_pct_raw, 0.0), 100.0) AS humidity_pct"
        );
    }

    #[test]
    fn test_range_check_clamp_flag_sql() {
        let rule = DqRule::RangeCheck {
            field: "humidity_pct".to_string(),
            min: Some(0.0),
            max: Some(100.0),
            action: DqAction::Clamp,
            clamp_to_bounds: true,
        };

        let gen = DqSqlGenerator::new();
        let sql = gen.generate_check_sql(&rule);

        assert_eq!(
            sql,
            r#"CASE
  WHEN humidity_pct < 0.0 OR humidity_pct > 100.0
  THEN 'range_check:humidity_pct:clamped'
  ELSE NULL
END"#
        );
    }

    // ============================================================
    // Test 3: range_check with reject action (NULL value)
    // ============================================================
    #[test]
    fn test_range_check_reject_value_sql() {
        let rule = DqRule::RangeCheck {
            field: "temperature_c".to_string(),
            min: Some(-60.0),
            max: Some(60.0),
            action: DqAction::Reject,
            clamp_to_bounds: false,
        };

        let gen = DqSqlGenerator::new();
        let sql = gen.generate_value_expr(&rule, "temperature_c_raw");

        assert_eq!(
            sql,
            r#"CASE
  WHEN temperature_c_raw < -60.0 OR temperature_c_raw > 60.0
  THEN NULL
  ELSE temperature_c_raw
END AS temperature_c"#
        );
    }

    // ============================================================
    // Test 4: null_check with reject action
    // ============================================================
    #[test]
    fn test_null_check_sql() {
        let rule = DqRule::NullCheck {
            field: "observation_time".to_string(),
            action: DqAction::Reject,
        };

        let gen = DqSqlGenerator::new();
        let sql = gen.generate_check_sql(&rule);

        assert_eq!(
            sql,
            r#"CASE
  WHEN observation_time IS NULL
  THEN 'null_check:observation_time:missing'
  ELSE NULL
END"#
        );
    }

    // ============================================================
    // Test 5: enum_check generates IN expression
    // ============================================================
    #[test]
    fn test_enum_check_sql() {
        let rule = DqRule::EnumCheck {
            field: "wind_direction".to_string(),
            allowed_values: vec![
                "N".to_string(), "NE".to_string(), "E".to_string(),
                "SE".to_string(), "S".to_string(), "SW".to_string(),
                "W".to_string(), "NW".to_string(),
            ],
            case_sensitive: false,
            action: DqAction::Flag,
        };

        let gen = DqSqlGenerator::new();
        let sql = gen.generate_check_sql(&rule);

        assert!(sql.contains("UPPER(wind_direction) NOT IN"));
        assert!(sql.contains("'N','NE','E','SE','S','SW','W','NW'"));
        assert!(sql.contains("enum_check:wind_direction:invalid_value"));
    }

    // ============================================================
    // Test 6: pattern_check generates regex
    // ============================================================
    #[test]
    fn test_pattern_check_sql() {
        let rule = DqRule::PatternCheck {
            field: "device_serial".to_string(),
            pattern: r"^[A-Z0-9]{8,12}$".to_string(),
            action: DqAction::Flag,
        };

        let gen = DqSqlGenerator::new();
        let sql = gen.generate_check_sql(&rule);

        assert!(sql.contains("device_serial !~"));
        assert!(sql.contains("'^[A-Z0-9]{8,12}$'"));
        assert!(sql.contains("pattern_check:device_serial:pattern_mismatch"));
    }

    // ============================================================
    // Test 7: cross_field_check expression
    // ============================================================
    #[test]
    fn test_cross_field_check_sql() {
        let rule = DqRule::CrossFieldCheck {
            name: "pm10_gte_pm25".to_string(),
            expression: "pm10 IS NULL OR pm25 IS NULL OR pm10 >= pm25".to_string(),
            message: Some("pm10_less_than_pm25".to_string()),
            action: DqAction::Flag,
        };

        let gen = DqSqlGenerator::new();
        let sql = gen.generate_check_sql(&rule);

        assert_eq!(
            sql,
            r#"CASE
  WHEN NOT (pm10 IS NULL OR pm25 IS NULL OR pm10 >= pm25)
  THEN 'cross_field_check:pm10_less_than_pm25'
  ELSE NULL
END"#
        );
    }

    // ============================================================
    // Test 8: Multiple rules generate ARRAY_REMOVE
    // ============================================================
    #[test]
    fn test_multiple_rules_array_construct() {
        let rules = vec![
            DqRule::RangeCheck {
                field: "pm25".to_string(),
                min: Some(0.0),
                max: Some(1000.0),
                action: DqAction::Flag,
                clamp_to_bounds: false,
            },
            DqRule::NullCheck {
                field: "observation_time".to_string(),
                action: DqAction::Reject,
            },
        ];

        let gen = DqSqlGenerator::new();
        let sql = gen.generate_dq_flags_expr(&rules);

        assert!(sql.contains("ARRAY_REMOVE(ARRAY["));
        assert!(sql.contains("range_check:pm25:out_of_bounds"));
        assert!(sql.contains("null_check:observation_time:missing"));
        assert!(sql.contains("], NULL) AS dq_flags"));
    }

    // ============================================================
    // Test 9: Empty rules produce empty array
    // ============================================================
    #[test]
    fn test_empty_rules_array() {
        let rules: Vec<DqRule> = vec![];

        let gen = DqSqlGenerator::new();
        let sql = gen.generate_dq_flags_expr(&rules);

        assert_eq!(sql, "ARRAY[]::TEXT[] AS dq_flags");
    }

    // ============================================================
    // Test 10: Freshness check SQL
    // ============================================================
    #[test]
    fn test_freshness_check_sql() {
        let rule = DqRule::FreshnessCheck {
            field: "observation_time".to_string(),
            max_age: Some("2 hours".to_string()),
            max_future: Some("10 minutes".to_string()),
            reference: "ingestion_time".to_string(),
            action: DqAction::Flag,
        };

        let gen = DqSqlGenerator::new();
        let sql = gen.generate_check_sql(&rule);

        assert!(sql.contains("observation_time < ingestion_time - INTERVAL '2 hours'"));
        assert!(sql.contains("freshness_check:observation_time:stale"));
        assert!(sql.contains("observation_time > ingestion_time + INTERVAL '10 minutes'"));
        assert!(sql.contains("freshness_check:observation_time:future"));
    }
}
```

---

## 6. Phase 4: ETL Runner

### 6.1 Module Location

`apps/silver-etl/src/etl.rs` - ETL execution with DuckDB.

### 6.2 Development Order

```
Test 1: DuckDB connection initializes successfully
Test 2: PostgreSQL extension loads
Test 3: Parquet file glob resolves correctly
Test 4: Watermark query returns max timestamp
Test 5: ETL executes with mock/fixture data
Test 6: ETL handles empty result set gracefully
Test 7: ETL updates watermark after successful run
Test 8: ETL collects metrics (rows processed, duration)
Test 9: Error handling for missing Parquet files
Test 10: Error handling for PostgreSQL connection failure
```

### 6.3 Test-First Examples

```rust
// apps/silver-etl/src/etl.rs

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ============================================================
    // Test 1: DuckDB connection initializes
    // ============================================================
    #[test]
    fn test_duckdb_connection_initializes() {
        let runner = EtlRunner::new_in_memory()
            .expect("Should create in-memory DuckDB connection");

        assert!(runner.is_connected());
    }

    // ============================================================
    // Test 2: PostgreSQL extension loads (requires integration)
    // ============================================================
    #[test]
    #[ignore] // Requires DuckDB with postgres extension
    fn test_postgres_extension_loads() {
        let runner = EtlRunner::new_in_memory()
            .expect("Should create connection");

        let result = runner.load_postgres_extension();
        assert!(result.is_ok(), "Should load postgres extension");
    }

    // ============================================================
    // Test 3: Parquet glob resolves files
    // ============================================================
    #[test]
    fn test_parquet_glob_resolves_files() {
        let temp_dir = TempDir::new().unwrap();
        let stream_dir = temp_dir.path().join("air-quality/year=2026/month=01/day=10");
        std::fs::create_dir_all(&stream_dir).unwrap();

        // Create a minimal parquet file
        let parquet_path = stream_dir.join("data.parquet");
        create_test_parquet(&parquet_path);

        let runner = EtlRunner::new_in_memory().unwrap();
        let files = runner.resolve_parquet_files(
            "air-quality",
            temp_dir.path().to_str().unwrap()
        ).unwrap();

        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("data.parquet"));
    }

    // ============================================================
    // Test 4: Watermark query returns correct value
    // ============================================================
    #[test]
    #[ignore] // Requires PostgreSQL
    fn test_watermark_query_returns_max_timestamp() {
        let runner = EtlRunner::with_postgres("postgresql://test@localhost/ndp")
            .expect("Should connect to PostgreSQL");

        // Setup: Insert test data with known timestamp
        runner.execute_sql(r#"
            INSERT INTO silver.test_watermark (observation_time, value)
            VALUES ('2026-01-10 12:00:00+00', 1.0),
                   ('2026-01-10 13:00:00+00', 2.0)
        "#).unwrap();

        let watermark = runner.get_watermark("silver.test_watermark", "observation_time")
            .unwrap();

        assert_eq!(
            watermark.to_rfc3339(),
            "2026-01-10T13:00:00+00:00"
        );
    }

    // ============================================================
    // Test 5: ETL executes with fixture data
    // ============================================================
    #[test]
    #[ignore] // Integration test
    fn test_etl_executes_with_fixture_data() {
        let temp_dir = setup_test_bronze_data();
        let config = create_test_silver_config();

        let runner = EtlRunner::with_test_db().unwrap();
        let result = runner.run_etl(
            &config,
            "air-quality",
            temp_dir.path().to_str().unwrap(),
        );

        assert!(result.is_ok());
        let stats = result.unwrap();
        assert!(stats.rows_processed > 0);
    }

    // ============================================================
    // Test 6: ETL handles empty result set
    // ============================================================
    #[test]
    fn test_etl_handles_empty_data() {
        let temp_dir = TempDir::new().unwrap();
        // No parquet files created

        let config = create_test_silver_config();
        let runner = EtlRunner::new_in_memory().unwrap();

        let result = runner.run_etl(
            &config,
            "air-quality",
            temp_dir.path().to_str().unwrap(),
        );

        assert!(result.is_ok());
        let stats = result.unwrap();
        assert_eq!(stats.rows_processed, 0);
    }

    // ============================================================
    // Test 7: ETL collects metrics
    // ============================================================
    #[test]
    fn test_etl_collects_metrics() {
        let stats = EtlStats {
            stream_id: "air-quality".to_string(),
            rows_processed: 100,
            rows_with_dq_flags: 5,
            rows_rejected: 2,
            duration_ms: 1500,
            watermark_before: None,
            watermark_after: Some(Utc::now()),
        };

        assert_eq!(stats.rows_processed, 100);
        assert_eq!(stats.rows_with_dq_flags, 5);
        assert!(stats.duration_ms > 0);
    }

    // ============================================================
    // Test 8: Error handling - missing parquet files
    // ============================================================
    #[test]
    fn test_error_missing_parquet_files() {
        let runner = EtlRunner::new_in_memory().unwrap();

        let result = runner.resolve_parquet_files(
            "nonexistent-stream",
            "/nonexistent/path"
        );

        // Should return empty vec, not error
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    // Helper functions
    fn create_test_parquet(path: &std::path::Path) {
        use polars::prelude::*;

        let df = df! {
            "timestamp" => &[1704886800000000_i64],
            "ndp_id" => &["test-sensor"],
            "source_id" => &["mqtt://test"],
            "context" => &[r#"{"location":{"path":"test"}}"#],
            "raw_payload" => &[r#"{"pm02":25.5}"#]
        }.unwrap();

        let file = std::fs::File::create(path).unwrap();
        ParquetWriter::new(file).finish(&mut df.clone()).unwrap();
    }

    fn setup_test_bronze_data() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let stream_dir = temp_dir.path().join("air-quality/year=2026/month=01/day=10");
        std::fs::create_dir_all(&stream_dir).unwrap();
        create_test_parquet(&stream_dir.join("data.parquet"));
        temp_dir
    }

    fn create_test_silver_config() -> SilverEtlConfig {
        // Same as Phase 2 test config
        SilverEtlConfig {
            enabled: true,
            target_table: "silver.air_quality".to_string(),
            target_schema: None,
            timestamp: TimestampMapping {
                source_field: "timestamp".to_string(),
                target_field: "observation_time".to_string(),
                transform: TimestampTransform::MicrosecondsToTimestamp,
            },
            identity_fields: vec![],
            field_mappings: vec![
                SilverFieldMapping {
                    source_path: "raw_payload.pm02".to_string(),
                    target_column: "pm25".to_string(),
                    column_type: "double_precision".to_string(),
                    nullable: true,
                    transform: None,
                    dq_rules: vec![],
                },
            ],
            dq_rules: vec![],
            dq_output: DqOutputConfig::default(),
            deduplication: DeduplicationConfig::default(),
            incremental: IncrementalConfig::default(),
        }
    }
}
```

---

## 7. Phase 5: Integration Tests

### 7.1 Module Location

`apps/silver-etl/tests/integration_tests.rs` - Full pipeline tests.

### 7.2 Development Order

```
Test 1: Full pipeline with single stream
Test 2: Full pipeline with DQ violations (flags populated)
Test 3: Incremental load processes only new data
Test 4: Upsert updates existing rows
Test 5: Config reload triggers schema refresh
Test 6: Multi-stream ETL run
Test 7: Error recovery and retry
Test 8: Dry-run mode generates SQL without executing
```

### 7.3 Test-First Examples

```rust
// apps/silver-etl/tests/integration_tests.rs

use silver_etl::{EtlRunner, SilverEtlConfig};
use std::fs;

// ============================================================
// Integration Test 1: Full pipeline with fixture data
// ============================================================
#[tokio::test]
#[ignore] // Requires full infrastructure
async fn test_full_pipeline_air_quality() {
    // Setup
    let temp_bronze = setup_bronze_fixtures("air-quality");
    let config = load_config("tests/fixtures/air_quality_config.yaml");

    // Execute
    let runner = EtlRunner::from_env().await
        .expect("Should create runner from environment");

    let stats = runner.run_etl(&config, "air-quality", &temp_bronze)
        .await
        .expect("ETL should complete");

    // Verify
    assert!(stats.rows_processed > 0, "Should process rows");
    assert_eq!(stats.rows_rejected, 0, "No rows should be rejected");

    // Verify data in Silver
    let silver_count = query_silver_count("silver.air_quality_observations").await;
    assert_eq!(silver_count, stats.rows_processed);
}

// ============================================================
// Integration Test 2: DQ violations flagged correctly
// ============================================================
#[tokio::test]
#[ignore]
async fn test_dq_violations_flagged() {
    // Setup with out-of-range data
    let temp_bronze = setup_bronze_with_violations();
    let config = load_config("tests/fixtures/air_quality_config.yaml");

    let runner = EtlRunner::from_env().await.unwrap();
    let stats = runner.run_etl(&config, "air-quality", &temp_bronze)
        .await
        .unwrap();

    // Verify DQ flags
    assert!(stats.rows_with_dq_flags > 0, "Should have flagged rows");

    // Query Silver for specific flag
    let flagged_rows = query_flagged_rows(
        "silver.air_quality_observations",
        "range_check:pm25:out_of_bounds"
    ).await;

    assert!(!flagged_rows.is_empty());
}

// ============================================================
// Integration Test 3: Incremental load
// ============================================================
#[tokio::test]
#[ignore]
async fn test_incremental_load() {
    let temp_bronze = setup_bronze_fixtures("air-quality");
    let config = load_config("tests/fixtures/air_quality_config.yaml");
    let runner = EtlRunner::from_env().await.unwrap();

    // First run - full load
    let stats1 = runner.run_etl(&config, "air-quality", &temp_bronze)
        .await
        .unwrap();

    // Add more data to Bronze
    add_more_bronze_data(&temp_bronze);

    // Second run - incremental
    let stats2 = runner.run_etl(&config, "air-quality", &temp_bronze)
        .await
        .unwrap();

    // Verify only new data processed
    assert!(stats2.rows_processed < stats1.rows_processed);
    assert!(stats2.watermark_after > stats1.watermark_after);
}

// ============================================================
// Integration Test 4: Dry-run mode
// ============================================================
#[tokio::test]
async fn test_dry_run_generates_sql() {
    let config = load_config("tests/fixtures/air_quality_config.yaml");
    let runner = EtlRunner::new_in_memory().unwrap();

    let sql = runner.dry_run(&config, "air-quality", "/data/raw")
        .expect("Should generate SQL");

    // Verify SQL structure
    assert!(sql.contains("INSERT INTO pg.silver.air_quality"));
    assert!(sql.contains("SELECT"));
    assert!(sql.contains("FROM read_parquet"));
    assert!(sql.contains("dq_flags"));

    // Print for manual inspection
    println!("Generated SQL:\n{}", sql);
}

// Helper functions
fn setup_bronze_fixtures(stream_id: &str) -> String {
    let temp_dir = tempfile::tempdir().unwrap();
    let fixtures_path = format!("tests/fixtures/bronze/{}", stream_id);

    // Copy fixture files
    let src = std::path::Path::new(&fixtures_path);
    let dst = temp_dir.path().join(stream_id);
    fs::create_dir_all(&dst).unwrap();

    copy_dir_all(src, &dst).unwrap();
    temp_dir.into_path().to_string_lossy().to_string()
}

fn setup_bronze_with_violations() -> String {
    // Create Bronze data with known DQ violations
    let temp_dir = tempfile::tempdir().unwrap();
    let stream_dir = temp_dir.path().join("air-quality/year=2026/month=01/day=10");
    fs::create_dir_all(&stream_dir).unwrap();

    // Create parquet with out-of-range pm25 value (2000, max is 1000)
    create_violation_parquet(&stream_dir.join("data.parquet"));

    temp_dir.into_path().to_string_lossy().to_string()
}

fn load_config(path: &str) -> SilverEtlConfig {
    let contents = fs::read_to_string(path)
        .expect("Should read config file");
    serde_yaml::from_str(&contents)
        .expect("Should parse config")
}
```

---

## 8. Cargo.toml Dependencies

```toml
# apps/silver-etl/Cargo.toml

[package]
name = "silver-etl"
version = "0.1.0"
edition = "2021"
description = "Silver layer ETL for Neural Data Platform"
authors = ["NDP Team"]

[dependencies]
# Project dependencies
neural-core = { path = "../../core" }
config-client = { path = "../../config-client" }

# DuckDB for Parquet reading and PostgreSQL writing
duckdb = { version = "1.1", features = ["bundled", "parquet", "json"] }

# Async runtime
tokio = { version = "1", features = ["rt-multi-thread", "macros", "time"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_yaml = "0.9"

# CLI
clap = { version = "4", features = ["derive"] }

# Logging and tracing
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Error handling
anyhow = "1.0"
thiserror = "1.0"

# Time handling
chrono = { version = "0.4", features = ["serde"] }

# Metrics
prometheus = "0.13"

[dev-dependencies]
# Testing
tempfile = "3"
polars = { version = "0.45", features = ["parquet"] }
assert_matches = "1.5"
tokio-test = "0.4"

[[bin]]
name = "silver-etl"
path = "src/main.rs"

[lib]
name = "silver_etl"
path = "src/lib.rs"
```

---

## 9. Test Fixture Requirements

### 9.1 Fixture Directory Structure

```
apps/silver-etl/tests/fixtures/
├── air_quality_config.yaml      # Full config for air-quality stream
├── weather_config.yaml          # Full config for weather stream
├── invalid_config.yaml          # Config with validation errors
├── bronze/
│   ├── air-quality/
│   │   └── year=2026/month=01/day=10/
│   │       └── data.parquet     # Sample Bronze data
│   └── outdoor-weather/
│       └── year=2026/month=01/day=10/
│           └── data.parquet
└── expected/
    ├── air_quality_etl.sql      # Expected generated SQL
    └── weather_etl.sql
```

### 9.2 Sample Fixture: air_quality_config.yaml

```yaml
# apps/silver-etl/tests/fixtures/air_quality_config.yaml

enabled: true
target_table: silver.air_quality_observations
target_schema: air_quality_observations_v1

timestamp:
  source_field: timestamp
  target_field: observation_time
  transform: microseconds_to_timestamp

identity_fields:
  - source: ndp_id
    target: ndp_id
  - source: context.location.path
    target: location_path

field_mappings:
  - source_path: raw_payload.pm02
    target_column: pm25
    type: double_precision
    nullable: false
    dq_rules:
      - rule: range_check
        min: 0.0
        max: 1000.0
        action: flag

  - source_path: raw_payload.rco2
    target_column: co2
    type: smallint
    nullable: true
    dq_rules:
      - rule: range_check
        min: 380
        max: 10000
        action: flag

  - source_path: raw_payload.atmp
    target_column: temperature_c
    type: double_precision
    nullable: true

  - source_path: raw_payload.rhum
    target_column: humidity_pct
    type: double_precision
    nullable: true
    dq_rules:
      - rule: range_check
        min: 0.0
        max: 100.0
        action: clamp

dq_rules:
  - rule: null_check
    field: observation_time
    action: reject

  - rule: cross_field_check
    name: pm10_gte_pm25
    expression: "pm10 IS NULL OR pm25 IS NULL OR pm10 >= pm25"
    message: pm10_less_than_pm25
    action: flag

dq_output:
  enabled: true
  target_column: dq_flags
  include_rules: true
  include_values: false

deduplication:
  enabled: true
  key_columns: [observation_time, ndp_id]
  strategy: upsert

incremental:
  enabled: true
  watermark_column: observation_time
  lag_interval: 5 minutes
```

---

## 10. Running Tests

### 10.1 Unit Tests (Fast, No Infrastructure)

```bash
# Run all unit tests
cargo test -p silver-etl

# Run specific phase tests
cargo test -p silver-etl config
cargo test -p silver-etl sql_gen
cargo test -p silver-etl dq

# With output
cargo test -p silver-etl -- --nocapture
```

### 10.2 Integration Tests (Requires Infrastructure)

```bash
# Start test infrastructure
docker compose -f deploy/pi/docker-compose.test.yml up -d

# Run integration tests
cargo test -p silver-etl -- --ignored

# Run specific integration test
cargo test -p silver-etl test_full_pipeline -- --ignored

# Cleanup
docker compose -f deploy/pi/docker-compose.test.yml down
```

### 10.3 Coverage

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin -p silver-etl --out Html

# Open report
open tarpaulin-report.html
```

---

## 11. TDD Checklist

### Per-Phase Checklist

- [ ] **Phase 1: Config Types**
  - [ ] All serde tests passing
  - [ ] Validation tests for invalid configs
  - [ ] Default value tests
  - [ ] Round-trip serialization tests

- [ ] **Phase 2: SQL Generator**
  - [ ] Simple field mapping SQL correct
  - [ ] Transform expressions correct
  - [ ] Timestamp handling correct
  - [ ] Complete ETL SQL structure valid

- [ ] **Phase 3: DQ Evaluator**
  - [ ] All rule types generate correct SQL
  - [ ] All actions (flag/reject/clamp) handled
  - [ ] Array construction for dq_flags
  - [ ] Cross-field checks work

- [ ] **Phase 4: ETL Runner**
  - [ ] DuckDB connection works
  - [ ] Parquet glob resolves files
  - [ ] Watermark query correct
  - [ ] Metrics collection works

- [ ] **Phase 5: Integration**
  - [ ] Full pipeline completes
  - [ ] DQ flags populated correctly
  - [ ] Incremental load works
  - [ ] Dry-run generates valid SQL

### Quality Gates

| Metric | Target | Measured By |
|--------|--------|-------------|
| Unit test coverage | > 80% | cargo tarpaulin |
| Integration tests | All passing | cargo test --ignored |
| No clippy warnings | 0 warnings | cargo clippy |
| Format check | Passes | cargo fmt --check |
| Doc coverage | All public APIs | cargo doc |

---

## 12. Key Principles Summary

1. **Test First**: Every function starts with a failing test
2. **Small Steps**: One assertion per test initially
3. **Config-Driven**: All behavior controlled by YAML
4. **DQ Transparency**: Verify flag strings exactly
5. **Isolation**: Unit tests mock external dependencies
6. **Integration**: Full tests validate end-to-end
7. **Documentation**: Tests serve as living documentation

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-10 | NDP Tester | Initial TDD guide |

---

## References

1. `product/features/dp-006/SCOPE.md` - Feature scope
2. `product/features/dp-006/specification/SPECIFICATION.md` - Requirements
3. `product/features/dp-006/architecture/ADR-006-002-binary-architecture.md` - Binary design
4. `product/features/dp-006/architecture/DQ-FRAMEWORK-DESIGN.md` - DQ rules
5. `docs/architecture/CONFIG_DRIVEN_SILVER_ETL_DESIGN.md` - Config schema
6. `core/src/parsers/config.rs` - Existing parser config patterns
