# Phase A: Architecture Foundation - TDD Guide (London School)

> **Phase:** A (Architecture Foundation)
> **Testing Approach:** London School TDD (Outside-In, Mock-Driven)
> **Parent Documents:** [TEST-PLAN.md](./TEST-PLAN.md), [TESTING-STRATEGY.md](../../TESTING-STRATEGY.md)
> **Created:** 2026-02-04

---

## Overview

This guide provides step-by-step London TDD instructions for implementing Phase A features. Each feature follows the Red-Green-Refactor cycle with mock collaborators to isolate units under test.

**Key Principle**: Test behavior through interfaces, not implementation details. All external dependencies (ConfigLoader, TimescaleDB, etcd) are mocked.

---

## Feature v11-A01: Gold ETL JSON Schema

### TDD Cycle 1: Basic Schema Validation

#### Red Phase: Write Failing Test

```rust
// Location: tools/ndp-validate/tests/gold_etl_schema_acceptance.rs

use serde_json::json;
use ndp_validate::schema::validate_against_schema;

/// ACCEPTANCE: Minimal valid gold_etl config passes schema validation
#[test]
fn test_minimal_gold_etl_config_validates() {
    // Arrange: Minimal valid gold_etl config
    let config = json!({
        "gold_etl": {
            "enabled": true,
            "aggregates": {
                "granularities": ["1 hour"],
                "fields": {
                    "pm25": { "metrics": ["mean"] }
                }
            }
        }
    });

    // Act: Validate against schema
    let result = validate_against_schema(&config, "gold-etl.schema.json");

    // Assert: Validation passes
    assert!(result.is_ok(), "Minimal valid config should pass: {:?}", result);
}
```

**Expected Failure**: Schema file does not exist or schema validation function not implemented.

#### Green Phase: Minimal Implementation

1. Create schema file at `config/schemas/gold-etl.schema.json`:

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "$id": "gold-etl.schema.json",
  "type": "object",
  "properties": {
    "gold_etl": {
      "type": "object",
      "required": ["enabled", "aggregates"],
      "properties": {
        "enabled": { "type": "boolean" },
        "aggregates": { "$ref": "#/definitions/aggregates" }
      }
    }
  },
  "definitions": {
    "aggregates": {
      "type": "object",
      "required": ["granularities", "fields"],
      "properties": {
        "granularities": {
          "type": "array",
          "items": { "type": "string" }
        },
        "fields": {
          "type": "object"
        }
      }
    }
  }
}
```

2. Implement `validate_against_schema` function in `tools/ndp-validate/src/schema.rs`.

#### Refactor Phase

- Extract schema path resolution to config
- Add better error messages
- Consider caching loaded schemas

---

### TDD Cycle 2: Granularity Pattern Validation

#### Red Phase

```rust
#[test]
fn test_granularity_pattern_rejects_invalid_format() {
    // Arrange: Invalid granularity format
    let config = json!({
        "gold_etl": {
            "enabled": true,
            "aggregates": {
                "granularities": ["hourly"],  // Invalid - should be "1 hour"
                "fields": { "pm25": { "metrics": ["mean"] } }
            }
        }
    });

    // Act
    let result = validate_against_schema(&config, "gold-etl.schema.json");

    // Assert: Validation fails
    assert!(result.is_err(), "'hourly' should be rejected");
}

#[test]
fn test_granularity_pattern_accepts_valid_formats() {
    // Test each valid format
    for granularity in &["1 hour", "1 day", "15 minutes", "4 hours", "7 days"] {
        let config = json!({
            "gold_etl": {
                "enabled": true,
                "aggregates": {
                    "granularities": [granularity],
                    "fields": { "pm25": { "metrics": ["mean"] } }
                }
            }
        });

        let result = validate_against_schema(&config, "gold-etl.schema.json");
        assert!(result.is_ok(), "'{}' should be valid", granularity);
    }
}
```

#### Green Phase

Update schema to include pattern validation:

```json
"granularities": {
  "type": "array",
  "items": {
    "type": "string",
    "pattern": "^\\d+\\s+(hour|day|minute|week)s?$"
  },
  "minItems": 1
}
```

#### Refactor Phase

- Extract pattern regex to a constant
- Add test for edge cases (singular vs plural)

---

### TDD Cycle 3: Metrics Enum Validation

#### Red Phase

```rust
#[test]
fn test_metrics_enum_rejects_unknown() {
    let config = json!({
        "gold_etl": {
            "enabled": true,
            "aggregates": {
                "granularities": ["1 hour"],
                "fields": {
                    "pm25": { "metrics": ["average"] }  // Invalid - should be "mean"
                }
            }
        }
    });

    let result = validate_against_schema(&config, "gold-etl.schema.json");
    assert!(result.is_err());

    let error_msg = result.unwrap_err().to_string();
    assert!(
        error_msg.contains("average") || error_msg.contains("enum"),
        "Error should mention invalid value"
    );
}

#[test]
fn test_metrics_enum_accepts_all_valid() {
    let valid_metrics = ["mean", "std", "min", "max", "count", "p95", "p99"];

    for metric in valid_metrics {
        let config = json!({
            "gold_etl": {
                "enabled": true,
                "aggregates": {
                    "granularities": ["1 hour"],
                    "fields": { "pm25": { "metrics": [metric] } }
                }
            }
        });

        let result = validate_against_schema(&config, "gold-etl.schema.json");
        assert!(result.is_ok(), "'{}' should be valid metric", metric);
    }
}
```

#### Green Phase

Update schema with metrics enum:

```json
"fields": {
  "type": "object",
  "additionalProperties": {
    "type": "object",
    "required": ["metrics"],
    "properties": {
      "metrics": {
        "type": "array",
        "items": {
          "enum": ["mean", "std", "min", "max", "count", "p95", "p99"]
        },
        "minItems": 1
      }
    }
  },
  "minProperties": 1
}
```

---

## Feature v11-A02: Gold DDL Tool (ndp-gold-ddl)

### TDD Cycle 1: Continuous Aggregate SQL Generation

#### Red Phase

```rust
// Location: tools/ndp-gold-ddl/src/generators/continuous_aggregate.rs

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mocks::{MockConfigLoader, MockTimescaleDb};

    #[test]
    fn test_generates_create_materialized_view() {
        // Arrange
        let config = create_test_gold_config("air-quality", "1 hour", &[
            ("pm25", vec!["mean", "std"])
        ]);

        // Act
        let sql = generate_continuous_aggregate(&config).unwrap();

        // Assert
        assert!(sql.contains("CREATE MATERIALIZED VIEW"));
        assert!(sql.contains("gold.air_quality_hourly"));
        assert!(sql.contains("WITH (timescaledb.continuous)"));
    }

    #[test]
    fn test_generates_time_bucket() {
        // Arrange
        let config = create_test_gold_config("air-quality", "1 hour", &[
            ("pm25", vec!["mean"])
        ]);

        // Act
        let sql = generate_continuous_aggregate(&config).unwrap();

        // Assert
        assert!(sql.contains("time_bucket('1 hour'"));
    }

    // Helper function
    fn create_test_gold_config(
        stream_id: &str,
        granularity: &str,
        fields: &[(&str, Vec<&str>)]
    ) -> GoldEtlConfig {
        // Build test config
        GoldEtlConfig {
            enabled: true,
            stream_id: stream_id.to_string(),
            aggregates: AggregatesConfig {
                granularities: vec![granularity.to_string()],
                fields: fields.iter().map(|(name, metrics)| {
                    (name.to_string(), FieldConfig {
                        metrics: metrics.iter().map(|m| m.to_string()).collect()
                    })
                }).collect(),
                ..Default::default()
            },
            ..Default::default()
        }
    }
}
```

#### Green Phase

```rust
// tools/ndp-gold-ddl/src/generators/continuous_aggregate.rs

pub fn generate_continuous_aggregate(config: &GoldEtlConfig) -> Result<String, GeneratorError> {
    let stream_id = &config.stream_id;
    let view_name = format!("gold.{}_hourly", stream_id.replace("-", "_"));

    let granularity = config.aggregates.granularities
        .first()
        .ok_or(GeneratorError::EmptyGranularities)?;

    let columns = generate_aggregate_columns(&config.aggregates)?;
    let source_table = format!("silver.{}_observations", stream_id.replace("-", "_"));

    Ok(format!(
        "CREATE MATERIALIZED VIEW {view_name}\n\
         WITH (timescaledb.continuous) AS\n\
         SELECT\n    \
         time_bucket('{granularity}', observation_time) AS bucket,\n    \
         ndp_id,\n    \
         {columns}\n\
         FROM {source_table}\n\
         GROUP BY bucket, ndp_id;"
    ))
}
```

#### Refactor Phase

- Extract view naming to separate function
- Add template for SQL generation
- Support multiple granularities

---

### TDD Cycle 2: Metric Function Mapping

#### Red Phase

```rust
#[test]
fn test_mean_generates_avg_function() {
    let config = create_test_gold_config("air-quality", "1 hour", &[
        ("pm25", vec!["mean"])
    ]);

    let sql = generate_continuous_aggregate(&config).unwrap();

    assert!(sql.contains("AVG(pm25) AS pm25_mean"));
}

#[test]
fn test_std_generates_stddev_function() {
    let config = create_test_gold_config("air-quality", "1 hour", &[
        ("pm25", vec!["std"])
    ]);

    let sql = generate_continuous_aggregate(&config).unwrap();

    assert!(sql.contains("STDDEV(pm25) AS pm25_std"));
}

#[test]
fn test_p95_generates_percentile_function() {
    let config = create_test_gold_config("air-quality", "1 hour", &[
        ("pm25", vec!["p95"])
    ]);

    let sql = generate_continuous_aggregate(&config).unwrap();

    assert!(sql.contains("percentile_cont(0.95)") ||
            sql.contains("PERCENTILE_CONT(0.95)"));
    assert!(sql.contains("pm25_p95"));
}
```

#### Green Phase

```rust
fn metric_to_sql(field: &str, metric: &str) -> Result<String, GeneratorError> {
    let alias = format!("{}_{}", field, metric);

    let sql = match metric {
        "mean" => format!("AVG({}) AS {}", field, alias),
        "std" => format!("STDDEV({}) AS {}", field, alias),
        "min" => format!("MIN({}) AS {}", field, alias),
        "max" => format!("MAX({}) AS {}", field, alias),
        "count" => format!("COUNT({}) AS {}", field, alias),
        "p95" => format!(
            "PERCENTILE_CONT(0.95) WITHIN GROUP (ORDER BY {}) AS {}",
            field, alias
        ),
        "p99" => format!(
            "PERCENTILE_CONT(0.99) WITHIN GROUP (ORDER BY {}) AS {}",
            field, alias
        ),
        _ => return Err(GeneratorError::UnknownMetric(metric.to_string())),
    };

    Ok(sql)
}
```

---

### TDD Cycle 3: Field Reference Validation

#### Red Phase

```rust
#[test]
fn test_rejects_field_not_in_stream() {
    // Arrange: Mock loader returns stream with only pm25 field
    let loader = MockConfigLoader::new()
        .with_stream_fields("air-quality", vec!["pm25", "co2"]);

    let config = GoldEtlConfig {
        stream_id: "air-quality".to_string(),
        aggregates: AggregatesConfig {
            fields: hashmap! {
                "nonexistent_field".to_string() => FieldConfig {
                    metrics: vec!["mean".to_string()]
                }
            },
            ..Default::default()
        },
        ..Default::default()
    };

    // Act
    let result = validate_gold_config(&config, &loader);

    // Assert
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("nonexistent_field"));
}
```

#### Green Phase

```rust
pub async fn validate_gold_config(
    config: &GoldEtlConfig,
    loader: &dyn ConfigLoader
) -> Result<(), ValidationError> {
    // Load stream config to get available fields
    let stream_config = loader.load_stream_config(&config.stream_id).await?;
    let stream_fields: HashSet<_> = stream_config.fields
        .iter()
        .map(|f| f.name.clone())
        .collect();

    // Validate all gold_etl fields exist in stream
    for field_name in config.aggregates.fields.keys() {
        if !stream_fields.contains(field_name) {
            return Err(ValidationError::FieldNotFound {
                field: field_name.clone(),
                stream: config.stream_id.clone(),
                available: stream_fields.into_iter().collect(),
            });
        }
    }

    Ok(())
}
```

---

## Feature v11-A03: Alignment JSON Schema

### TDD Cycle 1: Basic Alignment Schema

#### Red Phase

```rust
#[test]
fn test_alignment_schema_validates_basic_structure() {
    let config = json!({
        "alignment": {
            "view_name": "indoor_air_quality_aligned",
            "granularity": "1 hour",
            "join_strategy": "full_outer"
        }
    });

    let result = validate_against_schema(&config, "alignment.schema.json");
    assert!(result.is_ok());
}

#[test]
fn test_alignment_schema_requires_view_name() {
    let config = json!({
        "alignment": {
            "granularity": "1 hour",
            "join_strategy": "full_outer"
        }
    });

    let result = validate_against_schema(&config, "alignment.schema.json");
    assert!(result.is_err());
}
```

#### Green Phase

Create `config/schemas/alignment.schema.json`:

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "$id": "alignment.schema.json",
  "type": "object",
  "properties": {
    "alignment": {
      "type": "object",
      "required": ["view_name", "granularity", "join_strategy"],
      "properties": {
        "view_name": {
          "type": "string",
          "pattern": "^[a-z][a-z0-9_]*$"
        },
        "granularity": {
          "type": "string",
          "pattern": "^\\d+\\s+(hour|day|minute)s?$"
        },
        "join_strategy": {
          "enum": ["full_outer", "left", "inner"]
        },
        "null_handling": {
          "enum": ["preserve", "carry_forward", "interpolate"]
        }
      }
    }
  }
}
```

---

### TDD Cycle 2: Join Strategy Validation

#### Red Phase

```rust
#[test]
fn test_alignment_schema_validates_join_strategy_enum() {
    for strategy in &["full_outer", "left", "inner"] {
        let config = json!({
            "alignment": {
                "view_name": "test_aligned",
                "granularity": "1 hour",
                "join_strategy": strategy
            }
        });

        assert!(
            validate_against_schema(&config, "alignment.schema.json").is_ok(),
            "{} should be valid join strategy", strategy
        );
    }
}

#[test]
fn test_alignment_schema_rejects_invalid_join_strategy() {
    let config = json!({
        "alignment": {
            "view_name": "test_aligned",
            "granularity": "1 hour",
            "join_strategy": "outer"  // Invalid - should be "full_outer"
        }
    });

    let result = validate_against_schema(&config, "alignment.schema.json");
    assert!(result.is_err());
}
```

---

## Feature v11-A04: Alignment Interpreter

### TDD Cycle 1: Basic JOIN Generation

#### Red Phase

```rust
#[test]
fn test_generates_full_outer_join_sql() {
    // Arrange
    let domain_config = DomainConfig {
        id: "indoor-air-quality".to_string(),
        streams: vec![
            StreamRef { stream_id: "air-quality".to_string(), alias: "aq".to_string() },
            StreamRef { stream_id: "outdoor-weather".to_string(), alias: "ow".to_string() },
        ],
        alignment: AlignmentConfig {
            view_name: "indoor_air_quality_aligned".to_string(),
            granularity: "1 hour".to_string(),
            join_strategy: JoinStrategy::FullOuter,
            null_handling: NullHandling::Preserve,
        },
        ..Default::default()
    };

    // Act
    let sql = generate_aligned_view(&domain_config).unwrap();

    // Assert
    assert!(sql.contains("FULL OUTER JOIN"));
    assert!(sql.contains("gold.indoor_air_quality_aligned"));
}

#[test]
fn test_aligned_view_joins_on_bucket() {
    let domain_config = create_two_stream_domain();

    let sql = generate_aligned_view(&domain_config).unwrap();

    // Should join on bucket column
    assert!(sql.contains("ON aq.bucket = ow.bucket"));
}
```

#### Green Phase

```rust
pub fn generate_aligned_view(config: &DomainConfig) -> Result<String, GeneratorError> {
    let view_name = format!("gold.{}", config.alignment.view_name);
    let granularity = &config.alignment.granularity;

    // Build FROM clause with JOINs
    let mut joins = Vec::new();
    let first_stream = config.streams.first()
        .ok_or(GeneratorError::NoStreams)?;

    let first_table = format!(
        "gold.{}_hourly {}",
        first_stream.stream_id.replace("-", "_"),
        first_stream.alias
    );

    for (i, stream) in config.streams.iter().skip(1).enumerate() {
        let table = format!("gold.{}_hourly", stream.stream_id.replace("-", "_"));
        let alias = &stream.alias;
        let prev_alias = &config.streams[i].alias;

        let join = match config.alignment.join_strategy {
            JoinStrategy::FullOuter => format!(
                "FULL OUTER JOIN {} {} ON {}.bucket = {}.bucket",
                table, alias, prev_alias, alias
            ),
            JoinStrategy::Left => format!(
                "LEFT JOIN {} {} ON {}.bucket = {}.bucket",
                table, alias, prev_alias, alias
            ),
            JoinStrategy::Inner => format!(
                "INNER JOIN {} {} ON {}.bucket = {}.bucket",
                table, alias, prev_alias, alias
            ),
        };
        joins.push(join);
    }

    Ok(format!(
        "CREATE MATERIALIZED VIEW {view_name}\n\
         WITH (timescaledb.continuous) AS\n\
         SELECT\n    \
         time_bucket('{granularity}', COALESCE({coalesce_buckets})) AS bucket,\n    \
         -- columns here\n\
         FROM {first_table}\n\
         {joins};",
        view_name = view_name,
        granularity = granularity,
        coalesce_buckets = generate_coalesce_buckets(config),
        first_table = first_table,
        joins = joins.join("\n"),
    ))
}
```

---

## Feature v11-A05: Objectives JSON Schema

### TDD Cycle 1: Basic Objectives Schema

#### Red Phase

```rust
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

        assert!(
            validate_against_schema(&config, "objectives.schema.json").is_ok(),
            "{} should be valid condition", condition
        );
    }
}
```

#### Green Phase

Create `config/schemas/objectives.schema.json`:

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "$id": "objectives.schema.json",
  "type": "object",
  "properties": {
    "objectives": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["id", "target"],
        "properties": {
          "id": { "type": "string", "pattern": "^[a-z][a-z0-9_]*$" },
          "target": {
            "type": "object",
            "required": ["stream", "metric", "condition", "threshold"],
            "properties": {
              "stream": { "type": "string" },
              "metric": { "type": "string" },
              "condition": { "enum": ["<", ">", "<=", ">=", "==", "!="] },
              "threshold": { "type": "number" },
              "unit": { "type": "string" }
            }
          },
          "priority": { "enum": ["low", "medium", "high", "critical"] }
        }
      }
    }
  }
}
```

---

## Feature v11-001: Stream Type Classification

### TDD Cycle 1: StreamType Enum Parsing

#### Red Phase

```rust
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
    for (value, expected) in &[
        ("observation", StreamType::Observation),
        ("state_event", StreamType::StateEvent),
        ("forecast", StreamType::Forecast),
        ("dimension", StreamType::Dimension),
    ] {
        let json = json!({ "stream_type": value });
        let stream_type: StreamType = serde_json::from_value(json["stream_type"].clone()).unwrap();
        assert_eq!(stream_type, *expected);
    }
}
```

#### Green Phase

```rust
// core/src/types/stream.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamType {
    Observation,
    StateEvent,
    Forecast,
    Dimension,
}

// Add to StreamConfig
pub struct StreamConfig {
    pub stream_id: String,
    pub stream_type: Option<StreamType>,  // Optional for backward compatibility
    // ... other fields
}
```

---

## General TDD Patterns for Phase A

### Mock Usage Pattern

```rust
// 1. Create mock with expected data
let loader = MockConfigLoader::new()
    .with_stream(create_test_stream_config("air-quality"))
    .with_silver_config("air-quality", create_test_silver_config());

// 2. Configure mock to simulate errors if needed
let failing_loader = MockConfigLoader::new()
    .with_error(ConfigLoaderError::ConnectionError("etcd unreachable".into()));

// 3. Use mock in test
let result = function_under_test(&loader).await;
```

### Test Naming Convention

```
test_{component}_{scenario}_{expected_result}

Examples:
- test_continuous_aggregate_generator_valid_config_generates_sql
- test_continuous_aggregate_generator_empty_fields_returns_error
- test_schema_validation_invalid_metric_returns_error
```

### Arrange-Act-Assert Pattern

```rust
#[test]
fn test_example() {
    // Arrange: Set up test data and mocks
    let config = create_test_config();
    let loader = MockConfigLoader::new().with_stream(config);

    // Act: Call the function under test
    let result = function_under_test(&loader);

    // Assert: Verify the result
    assert!(result.is_ok());
    assert_eq!(result.unwrap().field, expected_value);
}
```

---

## References

- [TEST-PLAN.md](./TEST-PLAN.md) - Detailed Phase A test cases
- [TESTING-STRATEGY.md](../../TESTING-STRATEGY.md) - Overall testing strategy
- [MOCK-DEFINITIONS.md](./MOCK-DEFINITIONS.md) - Mock implementation details
- [mock_loader.rs](/workspaces/neural-data-platform/core/src/config/mock_loader.rs) - Existing MockConfigLoader

---

*TDD Guide created: 2026-02-04*
