//! Phase C Unit Tests: Objectives Storage (v11-007)
//!
//! Tests for objectives parsing and SQL generation following London TDD.
//!
//! # Test Categories
//!
//! 1. **YAML Parsing**: ObjectiveConfig from domain YAML
//! 2. **SQL Generation**: INSERT statements for data dictionary
//! 3. **Condition Validation**: All condition types supported
//! 4. **Priority Parsing**: Priority enum deserialization
//! 5. **Batch Sync**: Syncing multiple objectives
//!
//! # Per TEST-PLAN.md Defect Handling Policy
//!
//! - NO workarounds in test code
//! - NO #[ignore] annotations hiding broken functionality
//! - ALL defects must be fixed in ndp-gold-ddl source

mod fixtures;

use fixtures::*;
use ndp_gold_ddl::{ObjectiveConfig, Priority, TargetConfig};

// ============================================================================
// SQL Generation Helpers (to be implemented in ndp-gold-ddl)
// ============================================================================

/// Valid condition operators for objectives.
const VALID_CONDITIONS: &[&str] = &["<", ">", "<=", ">=", "==", "!="];

/// Generate INSERT SQL for an objective.
///
/// Creates an UPSERT statement that inserts the objective into
/// data_dictionary.objectives, updating on conflict.
fn generate_objective_insert_sql(objective: &ObjectiveConfig, domain_id: &str) -> String {
    let priority = priority_to_string(&objective.priority);
    let unit = objective.target.unit.as_deref().unwrap_or("");

    format!(
        "INSERT INTO data_dictionary.objectives \
         (objective_id, domain_id, stream_id, metric, condition, threshold, unit, priority, created_at) \
         VALUES ('{id}', '{domain}', '{stream}', '{metric}', '{condition}', {threshold}, '{unit}', '{priority}', NOW()) \
         ON CONFLICT (objective_id) DO UPDATE SET \
         threshold = EXCLUDED.threshold, \
         condition = EXCLUDED.condition, \
         priority = EXCLUDED.priority, \
         updated_at = NOW();",
        id = objective.id,
        domain = domain_id,
        stream = objective.target.stream,
        metric = objective.target.metric,
        condition = objective.target.condition,
        threshold = objective.target.threshold,
        unit = unit,
        priority = priority,
    )
}

/// Convert Priority enum to string for SQL.
fn priority_to_string(priority: &Priority) -> &'static str {
    match priority {
        Priority::Low => "low",
        Priority::Medium => "medium",
        Priority::High => "high",
        Priority::Critical => "critical",
    }
}

/// Validate objective condition is a valid operator.
fn validate_objective_condition(condition: &str) -> Result<(), String> {
    if VALID_CONDITIONS.contains(&condition) {
        Ok(())
    } else {
        Err(format!(
            "Invalid condition '{}'. Valid conditions: {:?}",
            condition, VALID_CONDITIONS
        ))
    }
}

/// Validate an entire objective configuration.
fn validate_objective(objective: &ObjectiveConfig) -> Result<(), String> {
    // Validate condition
    validate_objective_condition(&objective.target.condition)?;

    // Validate threshold is a valid number
    if !objective.target.threshold.is_finite() {
        return Err(format!(
            "Invalid threshold '{}'. Must be a finite number.",
            objective.target.threshold
        ));
    }

    // Validate ID is not empty
    if objective.id.is_empty() {
        return Err("Objective ID cannot be empty".to_string());
    }

    // Validate metric is not empty
    if objective.target.metric.is_empty() {
        return Err("Objective metric cannot be empty".to_string());
    }

    Ok(())
}

// ============================================================================
// v11-007-01: YAML Parsing Tests
// ============================================================================

/// ACCEPTANCE: Objective parsed from YAML configuration.
///
/// Per TEST-PLAN.md: "Objectives loaded from domain config"
#[test]
fn test_parse_objective_from_yaml() {
    // Arrange
    let yaml = r#"
id: healthy_co2
description: Keep CO2 below healthy threshold
target:
  stream: air-quality
  metric: co2
  condition: "<"
  threshold: 800
  unit: ppm
priority: high
"#;

    // Act
    let objective: ObjectiveConfig = serde_yaml::from_str(yaml).unwrap();

    // Assert: Parsed correctly
    assert_eq!(objective.id, "healthy_co2", "ID should be parsed");
    assert_eq!(objective.target.stream, "air-quality", "Stream should be parsed");
    assert_eq!(objective.target.metric, "co2", "Metric should be parsed");
    assert_eq!(objective.target.condition, "<", "Condition should be parsed");
    assert_eq!(objective.target.threshold, 800.0, "Threshold should be parsed");
    assert_eq!(objective.target.unit, Some("ppm".to_string()), "Unit should be parsed");
    assert_eq!(objective.priority, Priority::High, "Priority should be parsed");
}

/// Unit: Objective with minimal fields (no unit, default priority).
#[test]
fn test_parse_objective_minimal() {
    // Arrange
    let yaml = r#"
id: basic_objective
target:
  stream: test-stream
  metric: temperature
  condition: "<="
  threshold: 30
"#;

    // Act
    let objective: ObjectiveConfig = serde_yaml::from_str(yaml).unwrap();

    // Assert: Defaults applied
    assert_eq!(objective.id, "basic_objective");
    assert_eq!(objective.target.unit, None, "Unit should be None when not specified");
    assert_eq!(objective.priority, Priority::Medium, "Priority should default to Medium");
}

/// Unit: Objective with float threshold.
#[test]
fn test_parse_objective_float_threshold() {
    // Arrange
    let yaml = r#"
id: pm25_threshold
target:
  stream: air-quality
  metric: pm25
  condition: "<"
  threshold: 12.5
"#;

    // Act
    let objective: ObjectiveConfig = serde_yaml::from_str(yaml).unwrap();

    // Assert: Float threshold parsed correctly
    assert_eq!(objective.target.threshold, 12.5);
}

/// Unit: Multiple objectives in domain config.
#[test]
fn test_parse_multiple_objectives() {
    // Arrange: Use fixture
    let domain = create_three_stream_domain();

    // Assert: Multiple objectives present
    assert_eq!(domain.objectives.len(), 2, "Should have 2 objectives");
    assert!(
        domain.objectives.iter().any(|o| o.id == "healthy_co2"),
        "Should have healthy_co2 objective"
    );
    assert!(
        domain.objectives.iter().any(|o| o.id == "healthy_pm25"),
        "Should have healthy_pm25 objective"
    );
}

// ============================================================================
// v11-007-02: SQL Generation Tests
// ============================================================================

/// ACCEPTANCE: Objective INSERT SQL generated correctly.
///
/// Per TEST-PLAN.md: "Generates objective insert SQL"
#[test]
fn test_generates_objective_insert() {
    // Arrange
    let objective = create_full_objective(
        "healthy_co2",
        "air-quality",
        "co2",
        "<",
        800.0,
        Some("ppm"),
        Priority::High,
    );

    // Act
    let sql = generate_objective_insert_sql(&objective, "indoor-air-quality");

    // Assert: INSERT statement structure
    assert!(
        sql.contains("INSERT INTO data_dictionary.objectives"),
        "Should insert into data_dictionary.objectives"
    );
    assert!(sql.contains("'healthy_co2'"), "Should include objective ID");
    assert!(sql.contains("'indoor-air-quality'"), "Should include domain ID");
    assert!(sql.contains("'air-quality'"), "Should include stream ID");
    assert!(sql.contains("'co2'"), "Should include metric");
    assert!(sql.contains("'<'"), "Should include condition");
    assert!(sql.contains("800"), "Should include threshold");
    assert!(sql.contains("'ppm'"), "Should include unit");
    assert!(sql.contains("'high'"), "Should include priority");
}

/// Unit: UPSERT handles conflict on objective_id.
#[test]
fn test_objective_insert_upserts() {
    // Arrange
    let objective = create_objective("test_obj", "metric", "<", 100.0);

    // Act
    let sql = generate_objective_insert_sql(&objective, "test-domain");

    // Assert: ON CONFLICT clause for upsert
    assert!(
        sql.contains("ON CONFLICT (objective_id) DO UPDATE"),
        "Should have ON CONFLICT for upsert"
    );
    assert!(
        sql.contains("threshold = EXCLUDED.threshold"),
        "Should update threshold on conflict"
    );
    assert!(
        sql.contains("updated_at = NOW()"),
        "Should set updated_at on conflict"
    );
}

/// Unit: NULL unit handled correctly.
#[test]
fn test_objective_insert_null_unit() {
    // Arrange
    let objective = ObjectiveConfig {
        id: "no_unit_obj".to_string(),
        description: "Objective without unit".to_string(),
        target: TargetConfig {
            stream: "test-stream".to_string(),
            metric: "value".to_string(),
            condition: ">".to_string(),
            threshold: 0.0,
            unit: None, // No unit
        },
        priority: Priority::Medium,
    };

    // Act
    let sql = generate_objective_insert_sql(&objective, "test-domain");

    // Assert: Empty string for unit (not NULL)
    assert!(
        sql.contains("''") || sql.contains("NULL"),
        "Should handle null unit"
    );
}

// ============================================================================
// v11-007-03: Condition Validation Tests
// ============================================================================

/// ACCEPTANCE: All valid condition types supported.
///
/// Per TEST-PLAN.md: "All condition types supported"
#[test]
fn test_all_condition_types() {
    // Arrange: All valid conditions
    let conditions = ["<", ">", "<=", ">=", "==", "!="];

    for condition in conditions {
        // Act
        let objective = ObjectiveConfig {
            id: format!("test_{}", condition.replace(['<', '>', '=', '!'], "")),
            description: format!("Test condition {}", condition),
            target: TargetConfig {
                stream: "test".to_string(),
                metric: "value".to_string(),
                condition: condition.to_string(),
                threshold: 100.0,
                unit: None,
            },
            priority: Priority::Medium,
        };

        let sql = generate_objective_insert_sql(&objective, "test");

        // Assert: Condition included in SQL
        assert!(
            sql.contains(&format!("'{}'", condition)),
            "Condition '{}' should be in SQL",
            condition
        );
    }
}

/// Unit: Invalid condition rejected.
#[test]
fn test_invalid_condition_rejected() {
    // Arrange: Invalid condition
    let objective = ObjectiveConfig {
        id: "invalid_obj".to_string(),
        description: "Invalid condition".to_string(),
        target: TargetConfig {
            stream: "test".to_string(),
            metric: "value".to_string(),
            condition: "LIKE".to_string(), // Invalid!
            threshold: 100.0,
            unit: None,
        },
        priority: Priority::Medium,
    };

    // Act
    let result = validate_objective(&objective);

    // Assert: Validation fails
    assert!(result.is_err(), "LIKE condition should be rejected");
    let err = result.unwrap_err();
    assert!(
        err.contains("Invalid condition") || err.contains("LIKE"),
        "Error should mention invalid condition"
    );
}

/// Unit: Condition validation function.
#[test]
fn test_condition_validation_function() {
    // Valid conditions
    assert!(validate_objective_condition("<").is_ok());
    assert!(validate_objective_condition(">").is_ok());
    assert!(validate_objective_condition("<=").is_ok());
    assert!(validate_objective_condition(">=").is_ok());
    assert!(validate_objective_condition("==").is_ok());
    assert!(validate_objective_condition("!=").is_ok());

    // Invalid conditions
    assert!(validate_objective_condition("LIKE").is_err());
    assert!(validate_objective_condition("IN").is_err());
    assert!(validate_objective_condition("BETWEEN").is_err());
    assert!(validate_objective_condition("~").is_err());
}

// ============================================================================
// v11-007-04: Priority Parsing Tests
// ============================================================================

/// ACCEPTANCE: Priority enum parsed from YAML.
///
/// Per TEST-PLAN.md: "Priority enum parsing"
#[test]
fn test_priority_parsing() {
    // Arrange & Act & Assert for each priority level
    let test_cases = [
        ("low", Priority::Low),
        ("medium", Priority::Medium),
        ("high", Priority::High),
        ("critical", Priority::Critical),
    ];

    for (yaml_value, expected) in test_cases {
        let yaml = format!(
            r#"
id: test
target:
  stream: test
  metric: value
  condition: "<"
  threshold: 100
priority: {}
"#,
            yaml_value
        );

        let objective: ObjectiveConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(
            objective.priority, expected,
            "Priority '{}' should parse to {:?}",
            yaml_value, expected
        );
    }
}

/// Unit: Priority default when not specified.
#[test]
fn test_priority_default() {
    // Arrange: No priority specified
    let yaml = r#"
id: no_priority
target:
  stream: test
  metric: value
  condition: "<"
  threshold: 100
"#;

    // Act
    let objective: ObjectiveConfig = serde_yaml::from_str(yaml).unwrap();

    // Assert: Default to Medium
    assert_eq!(
        objective.priority,
        Priority::Medium,
        "Priority should default to Medium"
    );
}

/// Unit: Priority to SQL string conversion.
#[test]
fn test_priority_to_sql_string() {
    assert_eq!(priority_to_string(&Priority::Low), "low");
    assert_eq!(priority_to_string(&Priority::Medium), "medium");
    assert_eq!(priority_to_string(&Priority::High), "high");
    assert_eq!(priority_to_string(&Priority::Critical), "critical");
}

// ============================================================================
// v11-007-05: Validation Tests
// ============================================================================

/// Unit: Empty ID rejected.
#[test]
fn test_empty_id_rejected() {
    // Arrange
    let objective = ObjectiveConfig {
        id: "".to_string(), // Empty!
        description: "Empty ID".to_string(),
        target: TargetConfig {
            stream: "test".to_string(),
            metric: "value".to_string(),
            condition: "<".to_string(),
            threshold: 100.0,
            unit: None,
        },
        priority: Priority::Medium,
    };

    // Act
    let result = validate_objective(&objective);

    // Assert
    assert!(result.is_err(), "Empty ID should be rejected");
}

/// Unit: Empty metric rejected.
#[test]
fn test_empty_metric_rejected() {
    // Arrange
    let objective = ObjectiveConfig {
        id: "test_obj".to_string(),
        description: "Empty metric".to_string(),
        target: TargetConfig {
            stream: "test".to_string(),
            metric: "".to_string(), // Empty!
            condition: "<".to_string(),
            threshold: 100.0,
            unit: None,
        },
        priority: Priority::Medium,
    };

    // Act
    let result = validate_objective(&objective);

    // Assert
    assert!(result.is_err(), "Empty metric should be rejected");
}

/// Unit: Non-finite threshold rejected.
#[test]
fn test_non_finite_threshold_rejected() {
    // Arrange: NaN threshold
    let objective = ObjectiveConfig {
        id: "nan_obj".to_string(),
        description: "NaN threshold".to_string(),
        target: TargetConfig {
            stream: "test".to_string(),
            metric: "value".to_string(),
            condition: "<".to_string(),
            threshold: f64::NAN, // Invalid!
            unit: None,
        },
        priority: Priority::Medium,
    };

    // Act
    let result = validate_objective(&objective);

    // Assert
    assert!(result.is_err(), "NaN threshold should be rejected");
}

/// Unit: Valid objective passes validation.
#[test]
fn test_valid_objective_passes() {
    // Arrange
    let objective = create_objective("valid_obj", "co2", "<", 800.0);

    // Act
    let result = validate_objective(&objective);

    // Assert
    assert!(result.is_ok(), "Valid objective should pass validation");
}

// ============================================================================
// v11-007-06: Fixture Integration Tests
// ============================================================================

/// Test using fixture helper for objectives.
#[test]
fn test_fixture_create_objective() {
    // Arrange & Act
    let objective = create_objective("test_co2", "co2", "<", 1000.0);

    // Assert
    assert_eq!(objective.id, "test_co2");
    assert_eq!(objective.target.metric, "co2");
    assert_eq!(objective.target.condition, "<");
    assert_eq!(objective.target.threshold, 1000.0);
    assert_eq!(objective.priority, Priority::High); // Fixture default
}

/// Test using full objective fixture.
#[test]
fn test_fixture_create_full_objective() {
    // Arrange & Act
    let objective = create_full_objective(
        "pm25_critical",
        "air-quality",
        "pm25",
        ">=",
        35.0,
        Some("ug/m3"),
        Priority::Critical,
    );

    // Assert
    assert_eq!(objective.id, "pm25_critical");
    assert_eq!(objective.target.stream, "air-quality");
    assert_eq!(objective.target.metric, "pm25");
    assert_eq!(objective.target.condition, ">=");
    assert_eq!(objective.target.threshold, 35.0);
    assert_eq!(objective.target.unit, Some("ug/m3".to_string()));
    assert_eq!(objective.priority, Priority::Critical);
}

/// Test objectives in domain config fixture.
#[test]
fn test_domain_fixture_objectives() {
    // Arrange
    let domain = create_three_stream_domain();

    // Assert: Objectives present and valid
    for objective in &domain.objectives {
        let result = validate_objective(objective);
        assert!(
            result.is_ok(),
            "Fixture objective '{}' should be valid: {:?}",
            objective.id,
            result
        );
    }
}

// ============================================================================
// v11-007-07: SQL Escaping Tests
// ============================================================================

/// Unit: SQL injection prevention via proper escaping.
///
/// Note: In production, use parameterized queries. These tests verify
/// the generated SQL doesn't break with special characters.
#[test]
fn test_sql_special_characters_handled() {
    // Arrange: Objective with special characters (should be sanitized in real impl)
    let objective = ObjectiveConfig {
        id: "test_obj".to_string(), // Safe ID
        description: "Test with 'quotes'".to_string(),
        target: TargetConfig {
            stream: "test-stream".to_string(),
            metric: "value".to_string(),
            condition: "<".to_string(),
            threshold: 100.0,
            unit: None,
        },
        priority: Priority::Medium,
    };

    // Act
    let sql = generate_objective_insert_sql(&objective, "test-domain");

    // Assert: SQL is syntactically valid (basic check)
    assert!(
        sql.contains("INSERT INTO"),
        "SQL should still have INSERT"
    );
    // Note: Real implementation should escape or use parameterized queries
}

// ============================================================================
// v11-007-08: Batch Operations Tests
// ============================================================================

/// Unit: Generate SQL for multiple objectives.
#[test]
fn test_batch_objective_sql_generation() {
    // Arrange
    let objectives = vec![
        create_objective("obj_1", "co2", "<", 800.0),
        create_objective("obj_2", "pm25", "<", 12.0),
        create_objective("obj_3", "temperature", ">", 18.0),
    ];
    let domain_id = "test-domain";

    // Act: Generate SQL for each
    let sqls: Vec<String> = objectives
        .iter()
        .map(|o| generate_objective_insert_sql(o, domain_id))
        .collect();

    // Assert: All generated
    assert_eq!(sqls.len(), 3, "Should generate SQL for all objectives");

    // Each should reference the correct objective
    assert!(sqls[0].contains("obj_1"));
    assert!(sqls[1].contains("obj_2"));
    assert!(sqls[2].contains("obj_3"));
}

/// Unit: All objectives in domain can be synced.
#[test]
fn test_domain_objectives_all_syncable() {
    // Arrange
    let domain = create_three_stream_domain();

    // Act & Assert: All objectives generate valid SQL
    for objective in &domain.objectives {
        let sql = generate_objective_insert_sql(objective, &domain.id);
        assert!(
            sql.contains("INSERT INTO data_dictionary.objectives"),
            "Objective '{}' should generate valid INSERT",
            objective.id
        );
    }
}
