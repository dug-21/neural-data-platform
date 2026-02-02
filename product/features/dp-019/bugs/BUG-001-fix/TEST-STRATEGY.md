# BUG-001-fix: Unified Validation Types - Test Strategy

**Document Type**: SPARC Test Strategy (Phase R)
**Bug Reference**: BUG-001 Validation Drift Risk
**Feature**: dp-019 Config Validation Pipeline Extension
**Version**: 1.0
**Date**: 2026-02-02
**Status**: Proposed

---

## 1. Executive Summary

This document defines the comprehensive test strategy for the `ndp-types` crate and its integration into the NDP validation ecosystem. The strategy follows **London School TDD** principles, emphasizing behavior verification, mock-driven design, and contract testing.

### Test Goals

1. **Ensure serialization fidelity** - Rust types serialize/deserialize correctly
2. **Verify schema generation** - Generated JSON Schema matches Rust enum variants exactly
3. **Validate schema drift detection** - CLI detects when committed schema differs from generated
4. **Confirm integration round-trips** - Same config validates identically everywhere
5. **Maintain backward compatibility** - Existing configs continue to work

### Test Pyramid

```
                    /\
                   /  \
                  / E2E \           TC-600 series (3 tests)
                 /-------\          Full pipeline validation
                / Integr. \         TC-500 series (8 tests)
               /-----------\        Component integration
              /    Unit     \       TC-100 to TC-400 (35 tests)
             /---------------\      Individual functions/types
```

---

## 2. Unit Tests for ndp-types

### 2.1 Enum Serialization/Deserialization Tests

**Location**: `crates/ndp-types/src/source_type.rs` (and similar for each type)

#### TC-101: SourceType Serialization to JSON

| Field | Value |
|-------|-------|
| **Test ID** | TC-101 |
| **Description** | Verify SourceType enum variants serialize to snake_case JSON strings |
| **Type** | Unit |
| **Priority** | Critical |
| **London TDD Focus** | State verification |

```rust
#[test]
fn test_source_type_serializes_to_snake_case() {
    // Arrange
    let source_types = vec![
        (SourceType::Mqtt, "\"mqtt\""),
        (SourceType::HttpPoll, "\"http_poll\""),
        (SourceType::Webhook, "\"webhook\""),
        (SourceType::FileWatch, "\"file_watch\""),
        (SourceType::Csv, "\"csv\""),
    ];

    // Act & Assert
    for (variant, expected_json) in source_types {
        let serialized = serde_json::to_string(&variant).unwrap();
        assert_eq!(serialized, expected_json, "Failed for {:?}", variant);
    }
}
```

**Expected Outcome**: All variants serialize to lowercase snake_case strings.

---

#### TC-102: SourceType Deserialization from JSON

| Field | Value |
|-------|-------|
| **Test ID** | TC-102 |
| **Description** | Verify JSON strings deserialize to correct SourceType variants |
| **Type** | Unit |
| **Priority** | Critical |

```rust
#[test]
fn test_source_type_deserializes_from_snake_case() {
    // Arrange
    let test_cases = vec![
        ("\"mqtt\"", SourceType::Mqtt),
        ("\"http_poll\"", SourceType::HttpPoll),
        ("\"webhook\"", SourceType::Webhook),
        ("\"file_watch\"", SourceType::FileWatch),
        ("\"csv\"", SourceType::Csv),
    ];

    // Act & Assert
    for (json, expected_variant) in test_cases {
        let deserialized: SourceType = serde_json::from_str(json).unwrap();
        assert_eq!(deserialized, expected_variant);
    }
}
```

**Expected Outcome**: JSON strings map to correct enum variants.

---

#### TC-103: SourceType Invalid Value Deserialization Fails

| Field | Value |
|-------|-------|
| **Test ID** | TC-103 |
| **Description** | Verify invalid source type strings fail deserialization with clear error |
| **Type** | Unit - Error Path |
| **Priority** | High |

```rust
#[test]
fn test_source_type_rejects_invalid_values() {
    // Arrange
    let invalid_values = vec!["\"ftp\"", "\"grpc\"", "\"unknown\"", "\"MQTT\""];  // Case matters

    // Act & Assert
    for invalid in invalid_values {
        let result: Result<SourceType, _> = serde_json::from_str(invalid);
        assert!(result.is_err(), "Should reject: {}", invalid);
    }
}
```

**Expected Outcome**: Unknown variants produce serde deserialization errors.

---

#### TC-104: FieldType Serialization Round-Trip

| Field | Value |
|-------|-------|
| **Test ID** | TC-104 |
| **Description** | Verify FieldType enum round-trips through JSON correctly |
| **Type** | Unit |
| **Priority** | Critical |

```rust
#[test]
fn test_field_type_round_trip() {
    // Arrange
    let field_types = vec![
        FieldType::Float,
        FieldType::Int,
        FieldType::String,
        FieldType::Bool,
        FieldType::Json,
    ];

    // Act & Assert
    for original in field_types {
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: FieldType = serde_json::from_str(&json).unwrap();
        assert_eq!(original, deserialized);
    }
}
```

---

#### TC-105: DqRuleType Serialization (Tagged Enum)

| Field | Value |
|-------|-------|
| **Test ID** | TC-105 |
| **Description** | Verify DqRule tagged enum serializes with correct discriminator |
| **Type** | Unit |
| **Priority** | High |

```rust
#[test]
fn test_dq_rule_tagged_serialization() {
    // Arrange
    let rule = DqRule::RangeCheck {
        field: "temperature".to_string(),
        min: Some(0.0),
        max: Some(100.0),
        action: DqAction::Flag,
    };

    // Act
    let json = serde_json::to_string(&rule).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

    // Assert
    assert_eq!(parsed["rule"], "range_check");
    assert_eq!(parsed["field"], "temperature");
    assert_eq!(parsed["min"], 0.0);
    assert_eq!(parsed["max"], 100.0);
    assert_eq!(parsed["action"], "flag");
}
```

---

#### TC-106: DqAction Serialization

| Field | Value |
|-------|-------|
| **Test ID** | TC-106 |
| **Description** | Verify all DqAction variants serialize correctly |
| **Type** | Unit |
| **Priority** | High |

```rust
#[test]
fn test_dq_action_serialization() {
    let test_cases = vec![
        (DqAction::Flag, "\"flag\""),
        (DqAction::Reject, "\"reject\""),
        (DqAction::Clamp, "\"clamp\""),
        (DqAction::Drop, "\"drop\""),
        (DqAction::Warn, "\"warn\""),
    ];

    for (action, expected) in test_cases {
        let json = serde_json::to_string(&action).unwrap();
        assert_eq!(json, expected);
    }
}
```

---

#### TC-107: TimestampTransform Serialization

| Field | Value |
|-------|-------|
| **Test ID** | TC-107 |
| **Description** | Verify TimestampTransform variants serialize correctly |
| **Type** | Unit |
| **Priority** | Medium |

```rust
#[test]
fn test_timestamp_transform_serialization() {
    let transforms = vec![
        (TimestampTransform::MicrosecondsToTimestamp, "\"microseconds_to_timestamp\""),
        (TimestampTransform::Iso8601, "\"iso8601\""),
        (TimestampTransform::UnixSeconds, "\"unix_seconds\""),
        (TimestampTransform::NwsDuration, "\"nws_duration\""),
    ];

    for (transform, expected) in transforms {
        assert_eq!(serde_json::to_string(&transform).unwrap(), expected);
    }
}
```

---

### 2.2 NdpValidate Trait Implementation Tests

**Location**: `crates/ndp-types/src/validate.rs`

#### TC-201: ValidationError Construction

| Field | Value |
|-------|-------|
| **Test ID** | TC-201 |
| **Description** | Verify ValidationError can be constructed with all fields |
| **Type** | Unit |
| **Priority** | High |

```rust
#[test]
fn test_validation_error_construction() {
    // Arrange & Act
    let error = ValidationError {
        layer: ValidationLayer::Semantic,
        code: ErrorCode::InvalidSourceType,
        path: "$.sources[0].type".to_string(),
        message: "Invalid source type 'ftp'".to_string(),
        severity: Severity::Error,
        suggestion: Some("Did you mean 'http_poll'?".to_string()),
    };

    // Assert
    assert_eq!(error.layer, ValidationLayer::Semantic);
    assert_eq!(error.path, "$.sources[0].type");
    assert!(error.suggestion.is_some());
}
```

---

#### TC-202: ValidationError Serialization

| Field | Value |
|-------|-------|
| **Test ID** | TC-202 |
| **Description** | Verify ValidationError serializes to expected JSON format |
| **Type** | Unit |
| **Priority** | High |

```rust
#[test]
fn test_validation_error_json_format() {
    let error = ValidationError::semantic_error(
        ErrorCode::InvalidDqRule,
        "$.silver_etl.dq_rules[0]",
        "range_check min must be less than max",
    );

    let json = serde_json::to_value(&error).unwrap();

    assert_eq!(json["layer"], "semantic");
    assert_eq!(json["code"], "INVALID_DQ_RULE");
    assert_eq!(json["severity"], "error");
}
```

---

#### TC-203: NdpValidate for RangeCheck - Min Greater Than Max

| Field | Value |
|-------|-------|
| **Test ID** | TC-203 |
| **Description** | Verify NdpValidate catches min > max in range_check |
| **Type** | Unit |
| **Priority** | High |
| **London TDD Focus** | Behavior verification |

```rust
#[test]
fn test_range_check_validates_min_less_than_max() {
    // Arrange
    let rule = DqRule::RangeCheck {
        field: "temperature".to_string(),
        min: Some(100.0),  // min > max
        max: Some(0.0),
        action: DqAction::Flag,
    };

    // Act
    let errors = rule.validate();

    // Assert
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, ErrorCode::InvalidDqRule);
    assert!(errors[0].message.contains("must be less than max"));
}
```

---

#### TC-204: NdpValidate for RangeCheck - Missing Both Bounds

| Field | Value |
|-------|-------|
| **Test ID** | TC-204 |
| **Description** | Verify range_check requires at least min or max |
| **Type** | Unit |
| **Priority** | High |

```rust
#[test]
fn test_range_check_requires_bounds() {
    let rule = DqRule::RangeCheck {
        field: "value".to_string(),
        min: None,
        max: None,
        action: DqAction::Flag,
    };

    let errors = rule.validate();

    assert_eq!(errors.len(), 1);
    assert!(errors[0].message.contains("at least one of 'min' or 'max'"));
}
```

---

#### TC-205: NdpValidate Valid Config Returns Empty

| Field | Value |
|-------|-------|
| **Test ID** | TC-205 |
| **Description** | Verify valid configuration returns no errors |
| **Type** | Unit |
| **Priority** | Critical |

```rust
#[test]
fn test_valid_range_check_passes_validation() {
    let rule = DqRule::RangeCheck {
        field: "pm25".to_string(),
        min: Some(0.0),
        max: Some(500.0),
        action: DqAction::Flag,
    };

    let errors = rule.validate();

    assert!(errors.is_empty(), "Valid config should have no errors");
}
```

---

### 2.3 Strum-based Iteration and Conversion Tests

#### TC-301: SourceType::all_names Returns All Variants

| Field | Value |
|-------|-------|
| **Test ID** | TC-301 |
| **Description** | Verify SourceType::all_names() returns all variants as strings |
| **Type** | Unit |
| **Priority** | High |

```rust
#[test]
fn test_source_type_all_names_complete() {
    let names = SourceType::all_names();

    assert_eq!(names.len(), 5);
    assert!(names.contains(&"mqtt"));
    assert!(names.contains(&"http_poll"));
    assert!(names.contains(&"webhook"));
    assert!(names.contains(&"file_watch"));
    assert!(names.contains(&"csv"));
}
```

---

#### TC-302: SourceType EnumString Parsing

| Field | Value |
|-------|-------|
| **Test ID** | TC-302 |
| **Description** | Verify SourceType can be parsed from string using strum |
| **Type** | Unit |
| **Priority** | High |

```rust
#[test]
fn test_source_type_from_string() {
    use std::str::FromStr;

    assert_eq!(SourceType::from_str("mqtt").unwrap(), SourceType::Mqtt);
    assert_eq!(SourceType::from_str("http_poll").unwrap(), SourceType::HttpPoll);
    assert!(SourceType::from_str("invalid").is_err());
}
```

---

#### TC-303: SourceType AsRef<str> Conversion

| Field | Value |
|-------|-------|
| **Test ID** | TC-303 |
| **Description** | Verify SourceType variants convert to static str references |
| **Type** | Unit |
| **Priority** | Medium |

```rust
#[test]
fn test_source_type_as_ref_str() {
    assert_eq!(SourceType::Mqtt.as_ref(), "mqtt");
    assert_eq!(SourceType::HttpPoll.as_ref(), "http_poll");
    assert_eq!(SourceType::Webhook.as_ref(), "webhook");
    assert_eq!(SourceType::FileWatch.as_ref(), "file_watch");
    assert_eq!(SourceType::Csv.as_ref(), "csv");
}
```

---

## 3. Schema Generation Tests

**Location**: `tools/ndp-validate/src/schema_gen.rs` (or dedicated test file)

### 3.1 Generate Schema CLI Tests

#### TC-401: Generate Schema Produces Valid JSON

| Field | Value |
|-------|-------|
| **Test ID** | TC-401 |
| **Description** | Verify `--generate-schema` produces valid JSON Schema |
| **Type** | Integration |
| **Priority** | Critical |

```rust
#[test]
fn test_generate_schema_produces_valid_json() {
    // Arrange
    use std::process::Command;

    // Act
    let output = Command::new("cargo")
        .args(["run", "-p", "ndp-validate", "--", "--generate-schema"])
        .output()
        .expect("Failed to execute");

    // Assert
    assert!(output.status.success());

    let schema: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("Output should be valid JSON");

    assert!(schema.get("$schema").is_some());
    assert!(schema.get("definitions").is_some() || schema.get("$defs").is_some());
}
```

**Expected Outcome**: Valid JSON Schema document to stdout.

---

#### TC-402: Schema Includes All SourceType Variants

| Field | Value |
|-------|-------|
| **Test ID** | TC-402 |
| **Description** | Verify generated schema has enum array matching Rust SourceType |
| **Type** | Integration |
| **Priority** | Critical |

```rust
#[test]
fn test_schema_includes_all_source_types() {
    // Arrange
    let schema = generate_schema();  // Helper function

    // Act
    let source_type_enum = schema
        .pointer("/definitions/SourceType/enum")
        .or_else(|| schema.pointer("/$defs/SourceType/enum"));

    // Assert
    let enum_values: Vec<&str> = source_type_enum
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();

    assert_eq!(enum_values.len(), 5);
    assert!(enum_values.contains(&"mqtt"));
    assert!(enum_values.contains(&"http_poll"));
    assert!(enum_values.contains(&"webhook"));
    assert!(enum_values.contains(&"file_watch"));
    assert!(enum_values.contains(&"csv"));
}
```

---

#### TC-403: Schema Includes Doc Comment Descriptions

| Field | Value |
|-------|-------|
| **Test ID** | TC-403 |
| **Description** | Verify schema includes descriptions from Rust doc comments |
| **Type** | Integration |
| **Priority** | High |

```rust
#[test]
fn test_schema_includes_descriptions() {
    let schema = generate_schema();

    let source_type_def = schema
        .pointer("/definitions/SourceType")
        .or_else(|| schema.pointer("/$defs/SourceType"))
        .expect("SourceType definition should exist");

    let description = source_type_def.get("description")
        .and_then(|d| d.as_str());

    assert!(description.is_some(), "Should have description from doc comment");
    assert!(description.unwrap().contains("source types"),
        "Description should mention source types");
}
```

---

#### TC-404: Schema Output to File

| Field | Value |
|-------|-------|
| **Test ID** | TC-404 |
| **Description** | Verify `--generate-schema --output <path>` writes to file |
| **Type** | Integration |
| **Priority** | High |

```rust
#[test]
fn test_generate_schema_to_file() {
    use tempfile::NamedTempFile;
    use std::process::Command;

    // Arrange
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap();

    // Act
    let status = Command::new("cargo")
        .args(["run", "-p", "ndp-validate", "--",
               "--generate-schema", "--output", path])
        .status()
        .expect("Failed to execute");

    // Assert
    assert!(status.success());
    let contents = std::fs::read_to_string(path).unwrap();
    let schema: serde_json::Value = serde_json::from_str(&contents).unwrap();
    assert!(schema.get("$schema").is_some());
}
```

---

#### TC-405: Discriminated Union Output for Tagged Enums

| Field | Value |
|-------|-------|
| **Test ID** | TC-405 |
| **Description** | Verify DqRule generates discriminated union schema |
| **Type** | Integration |
| **Priority** | Medium |

```rust
#[test]
fn test_dq_rule_discriminated_union_schema() {
    let schema = generate_schema();

    let dq_rule_def = schema
        .pointer("/definitions/DqRule")
        .or_else(|| schema.pointer("/$defs/DqRule"));

    // schemars generates oneOf for tagged enums
    let one_of = dq_rule_def
        .and_then(|d| d.get("oneOf"))
        .expect("DqRule should have oneOf for tagged enum");

    assert!(one_of.as_array().unwrap().len() >= 11,
        "Should have all 11 DQ rule variants");
}
```

---

## 4. Schema Verification Tests

**Location**: `tools/ndp-validate/src/schema_verify.rs` (or integration test)

### 4.1 Verify Schema CLI Tests

#### TC-501: Verify Schema Returns 0 When Matching

| Field | Value |
|-------|-------|
| **Test ID** | TC-501 |
| **Description** | Verify `--verify-schema` returns exit 0 when schema matches generated |
| **Type** | Integration |
| **Priority** | Critical |

```rust
#[test]
fn test_verify_schema_returns_zero_when_matching() {
    use std::process::Command;
    use tempfile::NamedTempFile;

    // Arrange: Generate fresh schema
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path().to_str().unwrap();

    Command::new("cargo")
        .args(["run", "-p", "ndp-validate", "--",
               "--generate-schema", "--output", path])
        .status()
        .unwrap();

    // Act: Verify against itself
    let status = Command::new("cargo")
        .args(["run", "-p", "ndp-validate", "--",
               "--verify-schema", path])
        .status()
        .expect("Failed to execute");

    // Assert
    assert!(status.success(), "Exit code should be 0 for matching schema");
}
```

**Expected Outcome**: Exit code 0, no output (or "Schema verified" message).

---

#### TC-502: Verify Schema Returns 1 When Drift Detected

| Field | Value |
|-------|-------|
| **Test ID** | TC-502 |
| **Description** | Verify `--verify-schema` returns exit 1 when schema differs |
| **Type** | Integration |
| **Priority** | Critical |

```rust
#[test]
fn test_verify_schema_returns_one_when_drift() {
    use std::process::Command;
    use tempfile::NamedTempFile;
    use std::io::Write;

    // Arrange: Create outdated schema (missing 'csv' source type)
    let temp_file = NamedTempFile::new().unwrap();
    let outdated_schema = r#"{
        "$schema": "http://json-schema.org/draft-07/schema#",
        "definitions": {
            "SourceType": {
                "type": "string",
                "enum": ["mqtt", "http_poll", "webhook", "file_watch"]
            }
        }
    }"#;
    std::fs::write(temp_file.path(), outdated_schema).unwrap();

    // Act
    let status = Command::new("cargo")
        .args(["run", "-p", "ndp-validate", "--",
               "--verify-schema", temp_file.path().to_str().unwrap()])
        .status()
        .expect("Failed to execute");

    // Assert
    assert!(!status.success(), "Exit code should be 1 for drift");
    assert_eq!(status.code(), Some(1));
}
```

---

#### TC-503: Verify Schema Shows Diff Output

| Field | Value |
|-------|-------|
| **Test ID** | TC-503 |
| **Description** | Verify drift detection shows what changed |
| **Type** | Integration |
| **Priority** | High |

```rust
#[test]
fn test_verify_schema_shows_diff() {
    use std::process::Command;
    use tempfile::NamedTempFile;

    // Arrange: Create schema missing 'csv'
    let temp_file = NamedTempFile::new().unwrap();
    let outdated = /* schema missing csv */;
    std::fs::write(temp_file.path(), outdated).unwrap();

    // Act
    let output = Command::new("cargo")
        .args(["run", "-p", "ndp-validate", "--",
               "--verify-schema", temp_file.path().to_str().unwrap()])
        .output()
        .expect("Failed to execute");

    // Assert
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("csv") || stderr.contains("drift"),
        "Should show what's different: {}", stderr);
}
```

---

## 5. Integration Tests

**Location**: `crates/ndp-types/tests/integration/` or `tools/ndp-validate/tests/`

### 5.1 Round-Trip Tests

#### TC-504: Rust Type to Schema to Validation Round-Trip

| Field | Value |
|-------|-------|
| **Test ID** | TC-504 |
| **Description** | Verify Rust type serializes and validates against generated schema |
| **Type** | Integration |
| **Priority** | Critical |

```rust
#[test]
fn test_rust_to_schema_to_validation_round_trip() {
    use jsonschema::JSONSchema;

    // Arrange: Create valid Rust config
    let config = StreamConfig {
        stream_id: "test-stream".to_string(),
        sources: vec![SourceConfig {
            source_type: SourceType::HttpPoll,
            endpoints: vec![Endpoint::new("https://api.example.com")],
            poll_interval_secs: 300,
        }],
        // ...
    };

    // Act: Serialize to JSON
    let json = serde_json::to_value(&config).unwrap();

    // Generate schema from same types
    let schema_json = schema_for!(StreamConfig);
    let compiled = JSONSchema::compile(&schema_json).unwrap();

    // Assert: JSON validates against generated schema
    let result = compiled.validate(&json);
    assert!(result.is_ok(), "Rust-generated JSON should validate against schema");
}
```

---

#### TC-505: Same Config Validates Identically via CLI, MCP, and Runtime

| Field | Value |
|-------|-------|
| **Test ID** | TC-505 |
| **Description** | Verify identical validation results across all entry points |
| **Type** | Integration |
| **Priority** | Critical |
| **London TDD Focus** | Contract verification |

```rust
#[test]
fn test_same_validation_across_entry_points() {
    // Arrange: Config with known validation error
    let config_json = r#"{
        "stream_id": "test",
        "sources": [{
            "type": "mqtt"
            // Missing broker_url - should fail
        }]
    }"#;

    // Act: Validate via CLI
    let cli_errors = validate_via_cli(config_json);

    // Act: Validate via library (same as MCP would use)
    let lib_errors = validate_via_library(config_json);

    // Act: Attempt runtime deserialization
    let runtime_result: Result<StreamConfig, _> = serde_json::from_str(config_json);

    // Assert: All detect the same issue
    assert!(!cli_errors.is_empty());
    assert!(!lib_errors.is_empty());
    assert_eq!(cli_errors[0].code, lib_errors[0].code);
}
```

---

#### TC-506: Adding New Enum Variant Propagates Everywhere

| Field | Value |
|-------|-------|
| **Test ID** | TC-506 |
| **Description** | Verify new variant in ndp-types is recognized by all consumers |
| **Type** | Integration (Manual verification) |
| **Priority** | Critical |
| **Note** | This is a design-time verification test |

**Test Procedure**:

1. Add new variant `Grpc` to `SourceType` in `ndp-types/src/source_type.rs`
2. Run `cargo build --workspace`
3. Verify:
   - `core` compiles without changes (imports from ndp-types)
   - `ndp-validate` compiles without changes (imports from ndp-types)
   - Generated schema includes "grpc" in enum
   - Validation accepts configs with `"type": "grpc"`

**Expected Outcome**: Single-file change propagates to all consumers.

---

### 5.2 Validator Integration Tests

#### TC-507: Validator Uses Enum Methods Instead of Constants

| Field | Value |
|-------|-------|
| **Test ID** | TC-507 |
| **Description** | Verify validator error messages come from enum, not hardcoded |
| **Type** | Integration |
| **Priority** | High |

```rust
#[test]
fn test_validator_error_uses_enum_methods() {
    // Arrange: Invalid source type
    let config_json = r#"{
        "stream_id": "test",
        "sources": [{ "type": "invalid_type" }]
    }"#;

    // Act
    let errors = validate_config(config_json);

    // Assert: Error message lists all valid types dynamically
    let error_msg = &errors[0].message;
    for variant in SourceType::all_names() {
        assert!(error_msg.contains(variant),
            "Error should mention '{}' from enum", variant);
    }
}
```

---

#### TC-508: No Hardcoded Constants Remain

| Field | Value |
|-------|-------|
| **Test ID** | TC-508 |
| **Description** | Verify validator source has no hardcoded type constants |
| **Type** | Static Analysis |
| **Priority** | High |

```bash
# Run as part of CI
grep -r "SUPPORTED_SOURCE_TYPES" tools/ndp-validate/src/
# Should return no results after migration
```

**Expected Outcome**: No matches for deprecated constant patterns.

---

## 6. Regression Tests

**Location**: `tools/ndp-validate/tests/regression/`

### 6.1 Backward Compatibility Tests

#### TC-601: Existing Configs Validate Successfully

| Field | Value |
|-------|-------|
| **Test ID** | TC-601 |
| **Description** | Verify all existing stream configs pass validation |
| **Type** | Regression |
| **Priority** | Critical |

```rust
#[test]
fn test_existing_configs_still_valid() {
    // Arrange: Load all existing configs
    let config_paths = glob::glob("config/base/streams/*/config.json")
        .expect("Failed to read glob pattern");

    // Act & Assert: Each config should validate
    for path in config_paths {
        let path = path.unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let errors = validate_config(&content);

        assert!(
            errors.iter().filter(|e| e.severity == Severity::Error).count() == 0,
            "Config {:?} should have no errors: {:?}",
            path,
            errors
        );
    }
}
```

---

#### TC-602: Error Messages Unchanged for Known Cases

| Field | Value |
|-------|-------|
| **Test ID** | TC-602 |
| **Description** | Verify error message format matches existing behavior |
| **Type** | Regression |
| **Priority** | Medium |

```rust
#[test]
fn test_error_message_format_unchanged() {
    // Arrange: Config with known error
    let config = r#"{"stream_id": "test", "sources": [{"type": "ftp"}]}"#;

    // Act
    let errors = validate_config(config);

    // Assert: Format matches expected pattern
    assert!(errors[0].message.starts_with("Source type 'ftp' is not supported"));
    assert!(errors[0].message.contains("Must be one of:"));
}
```

---

#### TC-603: Re-Export Backward Compatibility

| Field | Value |
|-------|-------|
| **Test ID** | TC-603 |
| **Description** | Verify old import paths still work via re-exports |
| **Type** | Regression |
| **Priority** | High |

```rust
#[test]
fn test_core_reexports_work() {
    // These imports should compile and work identically
    use neural_core::SourceType as CoreSourceType;
    use ndp_types::SourceType as NdpSourceType;

    let core_type = CoreSourceType::Mqtt;
    let ndp_type = NdpSourceType::Mqtt;

    assert_eq!(
        serde_json::to_string(&core_type).unwrap(),
        serde_json::to_string(&ndp_type).unwrap()
    );
}
```

---

## 7. CI Test Matrix

### 7.1 Tests Run on Every PR

| Test Category | Test IDs | Estimated Time |
|---------------|----------|----------------|
| Unit - Serialization | TC-101 to TC-107 | ~5s |
| Unit - Validation | TC-201 to TC-205 | ~3s |
| Unit - Strum | TC-301 to TC-303 | ~2s |
| Integration - Schema | TC-401 to TC-405 | ~15s |
| Integration - Verify | TC-501 to TC-503 | ~10s |
| Regression | TC-601 to TC-603 | ~20s |

**Total PR Time**: ~55 seconds

### 7.2 Tests Run on Release

| Test Category | Test IDs | Notes |
|---------------|----------|-------|
| All PR Tests | All above | Full regression |
| Round-Trip | TC-504 to TC-508 | Extended integration |
| Cross-Platform | All | Run on ARM64 (Pi) |
| Performance | (Not listed) | Schema gen < 5s |

### 7.3 CI Configuration

```yaml
# .github/workflows/validation-types.yml
name: Validation Types CI

on:
  pull_request:
    paths:
      - 'crates/ndp-types/**'
      - 'tools/ndp-validate/**'
      - 'schemas/**'
  push:
    branches: [main]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Run Unit Tests
        run: cargo test -p ndp-types

      - name: Run Validator Tests
        run: cargo test -p ndp-validate

      - name: Verify Schema Not Drifted
        run: |
          cargo run -p ndp-validate -- --generate-schema > /tmp/generated.json
          cargo run -p ndp-validate -- --verify-schema schemas/stream-config.v1.2.schema.json

      - name: Run Regression Tests
        run: cargo test -p ndp-validate --test regression

  cross-compile:
    runs-on: ubuntu-latest
    if: github.event_name == 'push' && github.ref == 'refs/heads/main'
    steps:
      - uses: actions/checkout@v4
      - name: Install ARM64 Target
        run: rustup target add aarch64-unknown-linux-gnu
      - name: Cross Compile ndp-types
        run: cargo build -p ndp-types --target aarch64-unknown-linux-gnu
```

---

## 8. Test Implementation Guidelines

### 8.1 London School TDD Approach

**For each new type in ndp-types**:

1. **Write serialization test FIRST** (TC-1xx pattern)
2. **Write validation test FIRST** (TC-2xx pattern)
3. **Implement type with derives**
4. **Verify tests pass**

### 8.2 Test File Organization

```
crates/ndp-types/
  src/
    lib.rs
    source_type.rs       # Type + unit tests in same file
    field_type.rs
    dq_rule.rs
    validate.rs
  tests/
    integration/
      schema_round_trip.rs    # TC-504 to TC-506
      validation_parity.rs    # TC-507 to TC-508

tools/ndp-validate/
  src/
    schema_gen.rs        # Schema generation
    schema_verify.rs     # Schema verification
  tests/
    regression/
      existing_configs.rs     # TC-601 to TC-603
```

### 8.3 Test Naming Convention

```rust
// Pattern: test_<component>_<scenario>_<expected_outcome>
#[test]
fn test_source_type_mqtt_serializes_to_snake_case() { }

#[test]
fn test_range_check_min_greater_than_max_fails_validation() { }

#[test]
fn test_verify_schema_drift_detected_returns_exit_one() { }
```

### 8.4 Arrange-Act-Assert Structure

All tests follow AAA pattern with explicit comments:

```rust
#[test]
fn test_example() {
    // Arrange: Set up preconditions
    let input = ...;

    // Act: Perform the operation
    let result = operation(input);

    // Assert: Verify outcomes
    assert_eq!(result, expected);
}
```

---

## 9. Test Dependencies

### 9.1 Cargo.toml Additions

```toml
# crates/ndp-types/Cargo.toml
[dev-dependencies]
serde_json = "1.0"
tempfile = "3.0"

# tools/ndp-validate/Cargo.toml
[dev-dependencies]
tempfile = "3.0"
glob = "0.3"
assert_cmd = "2.0"    # For CLI integration tests
predicates = "3.0"    # For CLI output assertions
```

### 9.2 Test Fixtures

**Location**: `tests/fixtures/`

```
tests/fixtures/
  valid_configs/
    air-quality.json
    outdoor-weather.json
  invalid_configs/
    missing_stream_id.json
    invalid_source_type.json
  schemas/
    outdated_v1.1.json      # For drift detection tests
```

---

## 10. Test Success Criteria

### 10.1 Coverage Targets

| Component | Line Coverage | Branch Coverage |
|-----------|---------------|-----------------|
| ndp-types enums | 90%+ | 85%+ |
| NdpValidate implementations | 85%+ | 80%+ |
| Schema generation | 80%+ | 75%+ |
| Schema verification | 80%+ | 75%+ |

### 10.2 Quality Gates

- All TC-xxx tests PASS
- Zero regression failures on existing configs
- Schema verification passes in CI
- Cross-compile to ARM64 succeeds

---

## 11. References

- **SPECIFICATION**: `/workspaces/neural-data-platform/product/features/dp-019/bugs/BUG-001-fix/SPECIFICATION.md`
- **ADR-019-002**: `/workspaces/neural-data-platform/product/features/dp-019/bugs/BUG-001-fix/ADR-019-002-unified-validation-types.md`
- **Existing Test Patterns**: `/workspaces/neural-data-platform/tools/ndp-validate/src/semantic/dq_rules.rs`
- **London TDD Reference**: `/workspaces/neural-data-platform/docs/testing/AIR-005-TEST-DESIGN.md`

---

*Test Strategy created: 2026-02-02*
*SPARC Phase: Refinement (R)*
*Bug Reference: BUG-001 Validation Drift Risk*
*Next: Implementation following this test strategy*
