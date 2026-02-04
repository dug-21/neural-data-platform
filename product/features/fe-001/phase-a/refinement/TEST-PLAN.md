# Phase A: Architecture Foundation - Test Plan

> **Phase:** A (Architecture Foundation)
> **Target:** Week 1-2
> **Testing Approach:** London TDD (Outside-In)
> **Parent Document:** [TESTING-STRATEGY.md](../../TESTING-STRATEGY.md)

---

## Phase A Scope

Phase A establishes the extensible architecture for Gold layer. Features under test:

| ID | Feature | Testing Priority |
|----|---------|------------------|
| **v11-A01** | Gold ETL JSON Schema | Critical |
| **v11-A02** | Gold DDL Tool (ndp-gold-ddl) | Critical |
| **v11-A03** | Alignment JSON Schema | High |
| **v11-A05** | Objectives JSON Schema | High |
| **v11-001** | Stream Type Classification | High |

---

## 1. Test Development Order (Outside-In)

Following London TDD principles, tests are written in this order:

```
1. ACCEPTANCE TESTS (define success)
   ├── Schema validation acceptance tests
   └── DDL tool acceptance tests

2. COMPONENT TESTS (verify behavior)
   ├── CLI argument parsing tests
   ├── Config loading with mocks
   └── Schema validation tests

3. UNIT TESTS (implement details)
   ├── Continuous aggregate generator tests
   ├── Expression validator tests
   └── Naming convention tests
```

---

## 2. v11-A01: Gold ETL JSON Schema Tests

### 2.1 Acceptance Tests

```rust
// Location: tools/ndp-validate/tests/gold_etl_schema_acceptance.rs

/// ACCEPTANCE: Valid gold_etl config passes schema validation
#[test]
fn acceptance_valid_gold_etl_config_validates() {
    // Given: Complete valid gold_etl config
    let config = json!({
        "gold_etl": {
            "enabled": true,
            "description": "Air quality Gold layer",
            "aggregates": {
                "granularities": ["1 hour", "1 day"],
                "default_metrics": ["mean", "std", "min", "max", "count"],
                "fields": {
                    "pm25": { "metrics": ["mean", "std", "min", "max", "p95"] },
                    "co2": { "metrics": ["mean", "std", "min", "max"] }
                }
            },
            "features": {
                "lag": {
                    "enabled": true,
                    "lags_hours": [1, 6, 24],
                    "fields": ["pm25", "co2"]
                },
                "rolling": {
                    "enabled": true,
                    "windows": ["4 hours", "24 hours"],
                    "stats": ["mean", "std"],
                    "fields": ["pm25"]
                }
            }
        }
    });

    // When: Validated against gold-etl.schema.json
    let result = validate_against_schema(&config, "gold-etl.schema.json");

    // Then: Validation passes
    assert!(result.is_ok(), "Valid config should pass: {:?}", result);
}

/// ACCEPTANCE: Invalid metrics are rejected with helpful error
#[test]
fn acceptance_invalid_metric_rejected_with_error() {
    // Given: Config with unknown metric type
    let config = json!({
        "gold_etl": {
            "enabled": true,
            "aggregates": {
                "granularities": ["1 hour"],
                "fields": {
                    "pm25": { "metrics": ["average"] }  // Should be "mean"
                }
            }
        }
    });

    // When: Validated
    let result = validate_against_schema(&config, "gold-etl.schema.json");

    // Then: Fails with specific error about invalid metric
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    assert!(error_msg.contains("average") || error_msg.contains("enum"),
            "Error should mention invalid value: {}", error_msg);
}

/// ACCEPTANCE: Invalid granularity format rejected
#[test]
fn acceptance_invalid_granularity_format_rejected() {
    // Given: Config with invalid granularity format
    let config = json!({
        "gold_etl": {
            "enabled": true,
            "aggregates": {
                "granularities": ["hourly"],  // Should be "1 hour"
                "fields": { "pm25": { "metrics": ["mean"] } }
            }
        }
    });

    // When: Validated
    let result = validate_against_schema(&config, "gold-etl.schema.json");

    // Then: Fails with pattern error
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("pattern"));
}
```

### 2.2 Unit Tests

```rust
// Location: tools/ndp-validate/tests/gold_etl_schema_unit.rs

#[test]
fn test_schema_requires_enabled_field() {
    let config = json!({
        "gold_etl": {
            "aggregates": { "granularities": ["1 hour"], "fields": {} }
            // Missing "enabled"
        }
    });

    let result = validate_against_schema(&config, "gold-etl.schema.json");
    // Should pass (enabled has default) or require it - verify expected behavior
}

#[test]
fn test_schema_rejects_empty_fields_object() {
    let config = json!({
        "gold_etl": {
            "enabled": true,
            "aggregates": {
                "granularities": ["1 hour"],
                "fields": {}  // Empty - should this be valid?
            }
        }
    });

    let result = validate_against_schema(&config, "gold-etl.schema.json");
    // Document expected behavior
}

#[test]
fn test_schema_validates_metrics_enum() {
    // Valid metrics
    for metric in &["mean", "std", "min", "max", "count", "p95", "p99"] {
        let config = json!({
            "gold_etl": {
                "enabled": true,
                "aggregates": {
                    "granularities": ["1 hour"],
                    "fields": { "test": { "metrics": [metric] } }
                }
            }
        });
        assert!(validate_against_schema(&config, "gold-etl.schema.json").is_ok(),
                "{} should be valid metric", metric);
    }
}

#[test]
fn test_schema_validates_granularity_pattern() {
    // Valid granularities
    for gran in &["1 hour", "1 day", "15 minutes", "4 hours", "7 days"] {
        let config = json!({
            "gold_etl": {
                "enabled": true,
                "aggregates": {
                    "granularities": [gran],
                    "fields": { "test": { "metrics": ["mean"] } }
                }
            }
        });
        assert!(validate_against_schema(&config, "gold-etl.schema.json").is_ok(),
                "'{}' should be valid granularity", gran);
    }
}

#[test]
fn test_schema_allows_optional_features_section() {
    // Minimal valid config without features
    let config = json!({
        "gold_etl": {
            "enabled": true,
            "aggregates": {
                "granularities": ["1 hour"],
                "fields": { "pm25": { "metrics": ["mean"] } }
            }
            // No features section
        }
    });

    let result = validate_against_schema(&config, "gold-etl.schema.json");
    assert!(result.is_ok(), "Features should be optional");
}

#[test]
fn test_schema_validates_lag_hours_array() {
    let config = json!({
        "gold_etl": {
            "enabled": true,
            "aggregates": {
                "granularities": ["1 hour"],
                "fields": { "pm25": { "metrics": ["mean"] } }
            },
            "features": {
                "lag": {
                    "enabled": true,
                    "lags_hours": [1, 6, 24, 168],  // Up to 1 week
                    "fields": ["pm25"]
                }
            }
        }
    });

    let result = validate_against_schema(&config, "gold-etl.schema.json");
    assert!(result.is_ok(), "Lag hours should accept positive integers");
}
```

---

## 3. v11-A02: Gold DDL Tool (ndp-gold-ddl) Tests

### 3.1 Acceptance Tests

```rust
// Location: tools/ndp-gold-ddl/tests/acceptance/ddl_generation.rs

/// ACCEPTANCE: Tool generates valid CREATE MATERIALIZED VIEW SQL
#[test]
fn acceptance_generates_continuous_aggregate_sql() {
    // Given: Valid stream config with gold_etl
    let config = create_air_quality_gold_config();

    // When: Generate DDL
    let sql = generate_ddl_for_stream(&config);

    // Then: Contains expected SQL structure
    assert!(sql.contains("CREATE MATERIALIZED VIEW"));
    assert!(sql.contains("gold.air_quality_hourly"));
    assert!(sql.contains("WITH (timescaledb.continuous)"));
    assert!(sql.contains("time_bucket('1 hour'"));
    assert!(sql.contains("AVG(pm25) AS pm25_mean"));
    assert!(sql.contains("STDDEV(pm25) AS pm25_std"));
    assert!(sql.contains("GROUP BY"));
}

/// ACCEPTANCE: Tool generates idempotent SQL with sync mode
#[test]
fn acceptance_sync_mode_generates_idempotent_sql() {
    // Given: Valid config
    let config = create_air_quality_gold_config();

    // When: Generate DDL in sync mode
    let sql = generate_ddl_for_stream_with_action(&config, "sync");

    // Then: Contains IF NOT EXISTS or equivalent check
    assert!(
        sql.contains("IF NOT EXISTS") ||
        sql.contains("DO $$") ||
        sql.contains("timescaledb_information.continuous_aggregates"),
        "Sync mode should generate idempotent SQL"
    );
}

/// ACCEPTANCE: Tool generates DROP + CREATE for recreate mode
#[test]
fn acceptance_recreate_mode_drops_then_creates() {
    // Given: Valid config
    let config = create_air_quality_gold_config();

    // When: Generate DDL in recreate mode
    let sql = generate_ddl_for_stream_with_action(&config, "recreate");

    // Then: Contains DROP before CREATE
    let drop_pos = sql.find("DROP MATERIALIZED VIEW");
    let create_pos = sql.find("CREATE MATERIALIZED VIEW");

    assert!(drop_pos.is_some(), "Should contain DROP");
    assert!(create_pos.is_some(), "Should contain CREATE");
    assert!(drop_pos.unwrap() < create_pos.unwrap(), "DROP should come before CREATE");
}

/// ACCEPTANCE: Tool validates field references
#[test]
fn acceptance_rejects_invalid_field_reference() {
    // Given: Config referencing non-existent field
    let config = json!({
        "stream_id": "air-quality",
        "fields": [
            { "name": "pm25", "type": "float" }
        ],
        "gold_etl": {
            "enabled": true,
            "aggregates": {
                "granularities": ["1 hour"],
                "fields": {
                    "nonexistent_field": { "metrics": ["mean"] }
                }
            }
        }
    });

    // When: Attempt to generate DDL
    let result = try_generate_ddl(&config);

    // Then: Returns validation error
    assert!(result.is_err());
    let error = result.unwrap_err();
    assert!(error.to_string().contains("nonexistent_field") ||
            error.to_string().contains("field not found"));
}
```

### 3.2 Component Tests (CLI)

```rust
// Location: tools/ndp-gold-ddl/tests/cli_tests.rs

#[test]
fn test_cli_generate_stream_requires_stream_arg() {
    // When: Run without --stream
    let result = run_cli(&["generate"]);

    // Then: Error about missing argument
    assert!(!result.success);
    assert!(result.stderr.contains("--stream"));
}

#[test]
fn test_cli_generate_stream_validates_mode() {
    // When: Run with invalid mode
    let result = run_cli(&["generate", "--stream", "air-quality", "--mode", "invalid"]);

    // Then: Error about invalid mode
    assert!(!result.success);
    assert!(result.stderr.contains("mode") || result.stderr.contains("invalid"));
}

#[test]
fn test_cli_validate_only_produces_no_sql() {
    // Given: Valid config available
    setup_mock_config("air-quality");

    // When: Run validate subcommand
    let result = run_cli(&["validate", "--stream", "air-quality"]);

    // Then: Success without SQL output
    assert!(result.success);
    assert!(!result.stdout.contains("CREATE"));
    assert!(result.stdout.contains("valid") || result.stdout.is_empty());
}

#[test]
fn test_cli_domain_generate_requires_domain_arg() {
    // When: Run domain without --domain
    let result = run_cli(&["generate", "--domain"]);

    // Then: Error about missing argument
    assert!(!result.success);
}

#[test]
fn test_cli_outputs_sql_to_stdout() {
    // Given: Valid config
    setup_mock_config("air-quality");

    // When: Generate DDL
    let result = run_cli(&["generate", "--stream", "air-quality"]);

    // Then: SQL goes to stdout
    assert!(result.success);
    assert!(result.stdout.contains("CREATE"));
    assert!(result.stderr.is_empty() || result.stderr.contains("[INFO]"));
}

#[test]
fn test_cli_errors_go_to_stderr() {
    // Given: Invalid stream config
    setup_invalid_mock_config("broken-stream");

    // When: Attempt to generate
    let result = run_cli(&["generate", "--stream", "broken-stream"]);

    // Then: Errors go to stderr
    assert!(!result.success);
    assert!(!result.stderr.is_empty());
}
```

### 3.3 Unit Tests (Generators)

```rust
// Location: tools/ndp-gold-ddl/src/generators/continuous_aggregate.rs

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================
    // time_bucket generation
    // =========================================================

    #[test]
    fn test_generates_time_bucket_for_hourly() {
        let config = create_config_with_granularity("1 hour");

        let sql = generate_continuous_aggregate(&config);

        assert!(sql.contains("time_bucket('1 hour'"));
    }

    #[test]
    fn test_generates_time_bucket_for_daily() {
        let config = create_config_with_granularity("1 day");

        let sql = generate_continuous_aggregate(&config);

        assert!(sql.contains("time_bucket('1 day'"));
    }

    // =========================================================
    // Metric function mapping
    // =========================================================

    #[test]
    fn test_mean_generates_avg_function() {
        let config = create_config_with_metric("pm25", "mean");

        let sql = generate_continuous_aggregate(&config);

        assert!(sql.contains("AVG(pm25) AS pm25_mean"));
    }

    #[test]
    fn test_std_generates_stddev_function() {
        let config = create_config_with_metric("pm25", "std");

        let sql = generate_continuous_aggregate(&config);

        assert!(sql.contains("STDDEV(pm25) AS pm25_std"));
    }

    #[test]
    fn test_p95_generates_percentile_function() {
        let config = create_config_with_metric("pm25", "p95");

        let sql = generate_continuous_aggregate(&config);

        assert!(sql.contains("percentile_cont(0.95)") ||
                sql.contains("PERCENTILE_CONT(0.95)"));
        assert!(sql.contains("pm25_p95"));
    }

    #[test]
    fn test_count_generates_count_star() {
        let config = create_config_with_metric("pm25", "count");

        let sql = generate_continuous_aggregate(&config);

        assert!(sql.contains("COUNT(*)") || sql.contains("COUNT(pm25)"));
    }

    // =========================================================
    // Column naming convention
    // =========================================================

    #[test]
    fn test_column_naming_follows_convention() {
        // Convention: {field}_{metric}
        let config = create_config_with_metrics("pm25", &["mean", "std", "min", "max"]);

        let sql = generate_continuous_aggregate(&config);

        assert!(sql.contains("pm25_mean"));
        assert!(sql.contains("pm25_std"));
        assert!(sql.contains("pm25_min"));
        assert!(sql.contains("pm25_max"));
    }

    // =========================================================
    // View naming convention
    // =========================================================

    #[test]
    fn test_view_name_for_hourly() {
        let config = create_config_for_stream("air-quality", "1 hour");

        let sql = generate_continuous_aggregate(&config);

        assert!(sql.contains("gold.air_quality_hourly"));
    }

    #[test]
    fn test_view_name_for_daily() {
        let config = create_config_for_stream("air-quality", "1 day");

        let sql = generate_continuous_aggregate(&config);

        assert!(sql.contains("gold.air_quality_daily"));
    }

    #[test]
    fn test_view_name_handles_hyphenated_stream_id() {
        let config = create_config_for_stream("outdoor-weather", "1 hour");

        let sql = generate_continuous_aggregate(&config);

        assert!(sql.contains("gold.outdoor_weather_hourly"));
    }

    // =========================================================
    // GROUP BY clause
    // =========================================================

    #[test]
    fn test_group_by_includes_bucket_and_ndp_id() {
        let config = create_standard_config();

        let sql = generate_continuous_aggregate(&config);

        assert!(sql.contains("GROUP BY"));
        assert!(sql.contains("bucket") || sql.contains("time_bucket"));
        assert!(sql.contains("ndp_id"));
    }

    // =========================================================
    // Source table reference
    // =========================================================

    #[test]
    fn test_from_references_silver_table() {
        let config = create_config_with_silver_table("air-quality", "silver.air_quality_observations");

        let sql = generate_continuous_aggregate(&config);

        assert!(sql.contains("FROM silver.air_quality_observations"));
    }

    // =========================================================
    // Refresh policy generation
    // =========================================================

    #[test]
    fn test_generates_refresh_policy() {
        let config = create_standard_config();

        let sql = generate_continuous_aggregate(&config);

        assert!(sql.contains("add_continuous_aggregate_policy"));
        assert!(sql.contains("start_offset"));
        assert!(sql.contains("schedule_interval"));
    }

    // =========================================================
    // Multiple fields
    // =========================================================

    #[test]
    fn test_generates_all_configured_fields() {
        let config = create_config_with_multiple_fields(&[
            ("pm25", &["mean", "std"]),
            ("co2", &["mean", "max"]),
            ("temperature_c", &["mean"]),
        ]);

        let sql = generate_continuous_aggregate(&config);

        // All fields present
        assert!(sql.contains("pm25_mean"));
        assert!(sql.contains("pm25_std"));
        assert!(sql.contains("co2_mean"));
        assert!(sql.contains("co2_max"));
        assert!(sql.contains("temperature_c_mean"));
    }

    // =========================================================
    // Error cases
    // =========================================================

    #[test]
    fn test_returns_error_for_empty_fields() {
        let config = create_config_with_empty_fields();

        let result = try_generate_continuous_aggregate(&config);

        assert!(result.is_err());
    }

    #[test]
    fn test_returns_error_for_empty_granularities() {
        let config = create_config_with_empty_granularities();

        let result = try_generate_continuous_aggregate(&config);

        assert!(result.is_err());
    }
}
```

### 3.4 Unit Tests (Validation)

```rust
// Location: tools/ndp-gold-ddl/src/validation/expressions.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_field_exists_in_stream() {
        let stream_fields = vec!["pm25", "co2", "temperature"];
        let gold_fields = vec!["pm25", "co2"];

        let result = validate_field_references(&stream_fields, &gold_fields);

        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_field_missing_returns_error() {
        let stream_fields = vec!["pm25", "co2"];
        let gold_fields = vec!["pm25", "nonexistent"];

        let result = validate_field_references(&stream_fields, &gold_fields);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("nonexistent"));
    }

    #[test]
    fn test_validate_metric_is_known() {
        let known_metrics = &["mean", "std", "min", "max", "count", "p95", "p99"];

        for metric in known_metrics {
            assert!(is_valid_metric(metric), "{} should be valid", metric);
        }
    }

    #[test]
    fn test_validate_unknown_metric_returns_false() {
        assert!(!is_valid_metric("average"));  // Should be "mean"
        assert!(!is_valid_metric("stddev"));   // Should be "std"
        assert!(!is_valid_metric("p50"));      // Not implemented yet
    }

    #[test]
    fn test_validate_granularity_format() {
        let valid = &["1 hour", "1 day", "15 minutes", "4 hours", "7 days"];

        for gran in valid {
            assert!(is_valid_granularity(gran), "'{}' should be valid", gran);
        }
    }

    #[test]
    fn test_validate_invalid_granularity_format() {
        let invalid = &["hourly", "daily", "1hour", "1h", "hour", ""];

        for gran in invalid {
            assert!(!is_valid_granularity(gran), "'{}' should be invalid", gran);
        }
    }
}
```

---

## 4. v11-A03: Alignment JSON Schema Tests

```rust
// Location: tools/ndp-validate/tests/alignment_schema_tests.rs

#[test]
fn test_alignment_schema_validates_basic_structure() {
    let config = json!({
        "alignment": {
            "view_name": "indoor_air_quality_aligned",
            "granularity": "1 hour",
            "join_strategy": "full_outer",
            "null_handling": "preserve"
        }
    });

    let result = validate_against_schema(&config, "alignment.schema.json");
    assert!(result.is_ok());
}

#[test]
fn test_alignment_schema_validates_join_strategy_enum() {
    for strategy in &["full_outer", "left", "inner"] {
        let config = json!({
            "alignment": {
                "view_name": "test",
                "granularity": "1 hour",
                "join_strategy": strategy
            }
        });

        assert!(validate_against_schema(&config, "alignment.schema.json").is_ok(),
                "{} should be valid join strategy", strategy);
    }
}

#[test]
fn test_alignment_schema_rejects_invalid_join_strategy() {
    let config = json!({
        "alignment": {
            "view_name": "test",
            "granularity": "1 hour",
            "join_strategy": "outer"  // Should be "full_outer"
        }
    });

    let result = validate_against_schema(&config, "alignment.schema.json");
    assert!(result.is_err());
}

#[test]
fn test_alignment_schema_validates_null_handling_enum() {
    for handling in &["preserve", "carry_forward", "interpolate"] {
        let config = json!({
            "alignment": {
                "view_name": "test",
                "granularity": "1 hour",
                "null_handling": handling
            }
        });

        assert!(validate_against_schema(&config, "alignment.schema.json").is_ok(),
                "{} should be valid null_handling", handling);
    }
}
```

---

## 5. v11-A05: Objectives JSON Schema Tests

```rust
// Location: tools/ndp-validate/tests/objectives_schema_tests.rs

#[test]
fn test_objectives_schema_validates_basic_structure() {
    let config = json!({
        "objectives": [
            {
                "id": "healthy_co2",
                "target": {
                    "stream": "air-quality",
                    "metric": "co2",
                    "condition": "<",
                    "threshold": 800
                },
                "priority": "high"
            }
        ]
    });

    let result = validate_against_schema(&config, "objectives.schema.json");
    assert!(result.is_ok());
}

#[test]
fn test_objectives_schema_validates_condition_enum() {
    for condition in &["<", ">", "<=", ">=", "==", "!="] {
        let config = json!({
            "objectives": [{
                "id": "test",
                "target": {
                    "stream": "test",
                    "metric": "value",
                    "condition": condition,
                    "threshold": 100
                }
            }]
        });

        assert!(validate_against_schema(&config, "objectives.schema.json").is_ok(),
                "{} should be valid condition", condition);
    }
}

#[test]
fn test_objectives_schema_validates_priority_enum() {
    for priority in &["low", "medium", "high", "critical"] {
        let config = json!({
            "objectives": [{
                "id": "test",
                "target": {
                    "stream": "test",
                    "metric": "value",
                    "condition": "<",
                    "threshold": 100
                },
                "priority": priority
            }]
        });

        assert!(validate_against_schema(&config, "objectives.schema.json").is_ok(),
                "{} should be valid priority", priority);
    }
}

#[test]
fn test_objectives_schema_requires_threshold() {
    let config = json!({
        "objectives": [{
            "id": "test",
            "target": {
                "stream": "test",
                "metric": "value",
                "condition": "<"
                // Missing threshold
            }
        }]
    });

    let result = validate_against_schema(&config, "objectives.schema.json");
    assert!(result.is_err());
}
```

---

## 6. v11-001: Stream Type Classification Tests

```rust
// Location: core/tests/stream_type_classification.rs

#[test]
fn test_stream_config_parses_stream_type() {
    let config_json = json!({
        "stream_id": "air-quality",
        "stream_type": "observation",
        "description": "Air quality measurements",
        "fields": [{"name": "pm25", "type": "float", "nullable": false}],
        "sources": [{"type": "mqtt", "enabled": true}]
    });

    let config: StreamConfig = serde_json::from_value(config_json).unwrap();

    assert_eq!(config.stream_type, Some(StreamType::Observation));
}

#[test]
fn test_stream_type_enum_values() {
    // Verify all expected stream types are recognized
    for (value, expected) in &[
        ("observation", StreamType::Observation),
        ("state_event", StreamType::StateEvent),
        ("forecast", StreamType::Forecast),
        ("dimension", StreamType::Dimension),
    ] {
        let json = json!({ "stream_type": value });
        let stream_type: StreamType = serde_json::from_value(json["stream_type"].clone()).unwrap();
        assert_eq!(stream_type, *expected, "Failed for value: {}", value);
    }
}

#[test]
fn test_stream_type_is_optional_for_backward_compat() {
    // v1.0 configs don't have stream_type
    let config_json = json!({
        "stream_id": "legacy-stream",
        "description": "Legacy stream without type",
        "fields": [{"name": "value", "type": "float", "nullable": true}],
        "sources": [{"type": "mqtt", "enabled": true}]
    });

    let config: StreamConfig = serde_json::from_value(config_json).unwrap();

    assert!(config.stream_type.is_none());
}

#[test]
fn test_stream_type_validation_rejects_unknown() {
    let config_json = json!({
        "stream_id": "test",
        "stream_type": "unknown_type",
        "description": "Test",
        "fields": [{"name": "value", "type": "float", "nullable": true}],
        "sources": [{"type": "mqtt", "enabled": true}]
    });

    let result: Result<StreamConfig, _> = serde_json::from_value(config_json);
    assert!(result.is_err());
}
```

---

## 7. Integration Tests (Phase A)

```rust
// Location: tools/ndp-gold-ddl/tests/integration/phase_a_integration.rs

/// INTEGRATION: Full pipeline generates valid SQL and executes
#[tokio::test]
#[ignore]
async fn integration_gold_ddl_creates_continuous_aggregate() {
    // Arrange: Clean state
    cleanup_gold_schema().await;

    // Arrange: Sync test config to etcd
    sync_test_config("air-quality").await;

    // Act: Generate and execute DDL
    let ddl = Command::new("./target/debug/ndp-gold-ddl")
        .args(["generate", "--stream", "air-quality"])
        .output()
        .expect("ndp-gold-ddl should run");

    assert!(ddl.status.success(), "DDL generation should succeed");

    let sql = String::from_utf8_lossy(&ddl.stdout);
    execute_sql_on_timescale(&sql).await.expect("SQL should execute");

    // Assert: Continuous aggregate exists
    let exists = check_continuous_aggregate_exists("gold", "air_quality_hourly").await;
    assert!(exists, "Continuous aggregate should exist");

    // Assert: Has expected columns
    let columns = get_table_columns("gold.air_quality_hourly").await;
    assert!(columns.contains(&"bucket".to_string()));
    assert!(columns.contains(&"ndp_id".to_string()));
    assert!(columns.contains(&"pm25_mean".to_string()));
}

/// INTEGRATION: Idempotent deployment (sync mode)
#[tokio::test]
#[ignore]
async fn integration_sync_mode_is_idempotent() {
    // Arrange: Clean state and first deploy
    cleanup_gold_schema().await;
    deploy_gold_for_stream("air-quality").await;

    let columns_before = get_table_columns("gold.air_quality_hourly").await;

    // Act: Deploy again (should be idempotent)
    let result = deploy_gold_for_stream("air-quality").await;

    // Assert: No error
    assert!(result.is_ok(), "Second deploy should succeed");

    // Assert: Same structure
    let columns_after = get_table_columns("gold.air_quality_hourly").await;
    assert_eq!(columns_before, columns_after, "Columns should be unchanged");
}

/// INTEGRATION: JSON Schema validation via ndp-validate
#[tokio::test]
#[ignore]
async fn integration_validate_rejects_invalid_gold_config() {
    // Arrange: Invalid config with unknown metric
    let invalid_config = r#"{
        "gold_etl": {
            "enabled": true,
            "aggregates": {
                "granularities": ["1 hour"],
                "fields": { "pm25": { "metrics": ["invalid_metric"] } }
            }
        }
    }"#;

    write_temp_config(invalid_config, "invalid_gold.json");

    // Act: Validate
    let result = Command::new("./target/debug/ndp-validate")
        .args(["--config", "invalid_gold.json", "--schema", "gold-etl"])
        .output()
        .expect("ndp-validate should run");

    // Assert: Validation fails
    assert!(!result.status.success(), "Should reject invalid config");
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("invalid_metric") || stderr.contains("enum"));
}
```

---

## 8. Test Fixtures Required

### Config Fixtures

```
tools/ndp-gold-ddl/tests/fixtures/configs/valid/
├── air_quality_minimal.json      # Minimum valid gold_etl
├── air_quality_full.json         # All features enabled
├── outdoor_weather.json          # Different stream
├── home_assistant_state.json     # state_event type
└── multiple_granularities.json   # Both hourly and daily

tools/ndp-gold-ddl/tests/fixtures/configs/invalid/
├── unknown_metric.json           # Invalid metric type
├── invalid_field_reference.json  # References non-existent field
├── empty_fields.json             # No fields defined
├── invalid_granularity.json      # Bad granularity format
└── missing_required.json         # Missing required fields
```

### Expected SQL Fixtures

```
tools/ndp-gold-ddl/tests/fixtures/expected_sql/
├── air_quality_hourly_sync.sql       # Idempotent CREATE
├── air_quality_hourly_recreate.sql   # DROP + CREATE
├── air_quality_daily_sync.sql        # Daily aggregate
└── weather_hourly_sync.sql           # Weather stream
```

---

## 9. Mocking Requirements

| Dependency | Mock Type | Location |
|------------|-----------|----------|
| ConfigLoader | `MockConfigLoader` | `core/src/config/mock_loader.rs` (exists) |
| TimescaleDB | `MockTimescaleDb` | `tools/ndp-gold-ddl/src/mocks/timescale.rs` (new) |
| etcd | `MockEtcdClient` | `tools/ndp-gold-ddl/src/mocks/etcd.rs` (new) |
| File system | In-memory strings | Use `include_str!` for fixtures |

---

## 10. Test Execution Commands

```bash
# Run all Phase A unit tests
cargo test -p ndp-gold-ddl --lib
cargo test -p ndp-validate --lib

# Run Phase A component tests
cargo test -p ndp-gold-ddl --test cli_tests
cargo test -p ndp-validate --test gold_etl_schema_tests

# Run Phase A integration tests (requires Docker)
DEPLOY_ENV=integration cargo test -p ndp-gold-ddl --test integration -- --ignored

# Run all Phase A tests
./scripts/test-phase-a.sh
```

---

## 11. Exit Criteria (Phase A)

Phase A testing is complete when:

- [ ] All JSON Schema tests pass (v11-A01, v11-A03, v11-A05)
- [ ] All ndp-gold-ddl unit tests pass (v11-A02)
- [ ] All CLI component tests pass
- [ ] All validation unit tests pass
- [ ] Stream type classification tests pass (v11-001)
- [ ] Integration tests pass in CI
- [ ] Test coverage meets targets (90% generators, 85% validation)
- [ ] No flaky tests
- [ ] All tests follow London TDD patterns

---

## 12. Test Metrics (Phase A Target)

| Category | Target | Current |
|----------|--------|---------|
| Unit Tests | 25-30 | 0 |
| Component Tests | 8-10 | 0 |
| Integration Tests | 3-5 | 0 |
| Schema Validation Tests | 15+ | 0 |
| Coverage (generators) | 90% | 0% |
| Coverage (validation) | 85% | 0% |
| Test Duration (unit) | <10s | N/A |
| Test Duration (integration) | <2min | N/A |
