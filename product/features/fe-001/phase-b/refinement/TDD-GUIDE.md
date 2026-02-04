# Phase B: First Stream (air-quality) - TDD Guide (London School)

> **Phase:** B (First Stream - Reference Implementation)
> **Testing Approach:** London School TDD (Outside-In, Mock-Driven)
> **Parent Document:** [TEST-PLAN.md](./TEST-PLAN.md)
> **Created:** 2026-02-04

---

## Overview

This guide provides step-by-step London TDD instructions for implementing Phase B features. Phase B applies the architecture foundation to the `air-quality` stream as the reference implementation.

**Key Principle**: Air-quality is the exemplar. Every pattern established here becomes the template for subsequent streams.

---

## Feature v11-001: Stream Type Classification

### TDD Cycle 1: StreamType Enum

#### Red Phase: Write Failing Test

```rust
// Location: core/src/types/stream.rs (test module)

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_stream_type_deserializes_observation() {
        // Arrange
        let json = json!("observation");

        // Act
        let result: Result<StreamType, _> = serde_json::from_value(json);

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), StreamType::Observation);
    }
}
```

**Expected Failure**: `StreamType` enum does not exist.

#### Green Phase: Minimal Implementation

```rust
// core/src/types/stream.rs

use serde::{Deserialize, Serialize};

/// Classification of data stream types for correlation analysis
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamType {
    /// Continuous numeric readings (PM2.5, temperature, etc.)
    Observation,
    /// Binary/discrete state changes (door open/close)
    StateEvent,
    /// Future predictions from external source (NWS forecast)
    Forecast,
    /// Slowly changing reference data
    Dimension,
}
```

#### Refactor Phase

- Add doc comments for each variant
- Consider adding `Default` implementation

---

### TDD Cycle 2: StreamType in StreamConfig

#### Red Phase

```rust
#[test]
fn test_stream_config_parses_stream_type() {
    // Arrange
    let config_json = json!({
        "stream_id": "air-quality",
        "stream_type": "observation",
        "description": "Air quality measurements",
        "version": "1.0.0",
        "enabled": true,
        "retention_days": 365,
        "compression_after_days": 7,
        "partitioning_strategy": "daily",
        "fields": [{"name": "pm25", "type": "float", "nullable": false}],
        "sources": [{"type": "mqtt", "enabled": true}]
    });

    // Act
    let config: StreamConfig = serde_json::from_value(config_json).unwrap();

    // Assert
    assert_eq!(config.stream_type, Some(StreamType::Observation));
}
```

#### Green Phase

```rust
// Add to StreamConfig struct
pub struct StreamConfig {
    pub stream_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stream_type: Option<StreamType>,  // Optional for backward compatibility
    pub description: String,
    // ... other existing fields
}
```

---

### TDD Cycle 3: Correlation Role Mapping

#### Red Phase

```rust
#[test]
fn test_observation_type_is_effect_role() {
    assert_eq!(StreamType::Observation.correlation_role(), "effect");
}

#[test]
fn test_state_event_type_is_cause_role() {
    assert_eq!(StreamType::StateEvent.correlation_role(), "cause");
}

#[test]
fn test_forecast_type_is_context_role() {
    assert_eq!(StreamType::Forecast.correlation_role(), "context");
}
```

#### Green Phase

```rust
impl StreamType {
    /// Map stream type to correlation role for V1.2 pattern detection
    pub fn correlation_role(&self) -> &'static str {
        match self {
            StreamType::Observation => "effect",
            StreamType::StateEvent => "cause",
            StreamType::Forecast => "context",
            StreamType::Dimension => "metadata",
        }
    }
}
```

---

## Feature v11-002: Classification Propagation

### TDD Cycle 1: Generate Classification SQL

#### Red Phase

```rust
// Location: tools/ndp-gold-ddl/src/generators/classification.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generates_classification_insert() {
        // Arrange
        let stream_id = "air-quality";
        let stream_type = StreamType::Observation;

        // Act
        let sql = generate_classification_sql(stream_id, stream_type);

        // Assert
        assert!(sql.contains("INSERT INTO data_dictionary.stream_classification"));
        assert!(sql.contains("'air-quality'"));
        assert!(sql.contains("'observation'"));
    }
}
```

#### Green Phase

```rust
pub fn generate_classification_sql(stream_id: &str, stream_type: StreamType) -> String {
    let role = stream_type.correlation_role();

    format!(
        "INSERT INTO data_dictionary.stream_classification \
         (stream_id, stream_type, correlation_role, updated_at) \
         VALUES ('{stream_id}', '{stream_type:?}', '{role}', NOW()) \
         ON CONFLICT (stream_id) DO UPDATE SET \
         stream_type = EXCLUDED.stream_type, \
         correlation_role = EXCLUDED.correlation_role, \
         updated_at = NOW();",
        stream_id = stream_id,
        stream_type = stream_type.to_string().to_lowercase(),
        role = role
    )
}
```

---

### TDD Cycle 2: Sync Classification with Mock DB

#### Red Phase

```rust
#[tokio::test]
async fn test_sync_classification_executes_sql() {
    // Arrange
    let loader = MockConfigLoader::new()
        .with_stream(create_typed_stream("air-quality", StreamType::Observation));
    let db = MockTimescaleDb::new();

    // Act
    sync_stream_classification(&loader, &db, "air-quality").await.unwrap();

    // Assert
    assert!(db.sql_contains("INSERT INTO data_dictionary.stream_classification"));
    assert!(db.sql_contains("'observation'"));
}
```

#### Green Phase

```rust
pub async fn sync_stream_classification(
    loader: &dyn ConfigLoader,
    db: &dyn TimescaleConnection,
    stream_id: &str
) -> Result<(), SyncError> {
    let config = loader.load_stream_config(stream_id).await?;

    if let Some(stream_type) = config.stream_type {
        let sql = generate_classification_sql(stream_id, stream_type);
        db.execute(&sql).await?;
    }

    Ok(())
}
```

---

## Feature v11-003: Per-Stream Continuous Aggregates

### TDD Cycle 1: Basic View Generation for air-quality

#### Red Phase

```rust
#[test]
fn test_generates_air_quality_hourly_view() {
    // Arrange
    let config = create_air_quality_gold_config();

    // Act
    let sql = generate_continuous_aggregate(&config).unwrap();

    // Assert
    assert!(sql.contains("CREATE MATERIALIZED VIEW gold.air_quality_hourly"));
    assert!(sql.contains("WITH (timescaledb.continuous)"));
}
```

#### Green Phase

Already implemented in Phase A. This test validates it works for air-quality specifically.

---

### TDD Cycle 2: PM2.5 Specific Metrics

#### Red Phase

```rust
#[test]
fn test_pm25_generates_five_metrics() {
    // Arrange: air-quality config specifies pm25: mean, std, min, max, p95
    let config = create_air_quality_gold_config();

    // Act
    let sql = generate_continuous_aggregate(&config).unwrap();

    // Assert: All 5 metrics present
    assert!(sql.contains("AVG(pm25) AS pm25_mean"));
    assert!(sql.contains("STDDEV(pm25) AS pm25_std"));
    assert!(sql.contains("MIN(pm25) AS pm25_min"));
    assert!(sql.contains("MAX(pm25) AS pm25_max"));
    assert!(sql.contains("pm25_p95")); // Percentile format may vary
}
```

#### Green Phase

The generic `metric_to_sql` function from Phase A handles this.

#### Refactor Phase

```rust
// Create helper for air-quality specific config
fn create_air_quality_gold_config() -> GoldEtlConfig {
    GoldEtlConfig {
        enabled: true,
        stream_id: "air-quality".to_string(),
        aggregates: AggregatesConfig {
            granularities: vec!["1 hour".to_string(), "1 day".to_string()],
            default_metrics: Some(vec!["mean".to_string(), "count".to_string()]),
            fields: hashmap! {
                "pm25".to_string() => FieldConfig {
                    metrics: vec!["mean", "std", "min", "max", "p95"]
                        .into_iter().map(String::from).collect()
                },
                "pm10".to_string() => FieldConfig {
                    metrics: vec!["mean", "min", "max"]
                        .into_iter().map(String::from).collect()
                },
                "co2".to_string() => FieldConfig {
                    metrics: vec!["mean", "std", "min", "max"]
                        .into_iter().map(String::from).collect()
                },
                "temperature_c".to_string() => FieldConfig {
                    metrics: vec!["mean", "min", "max"]
                        .into_iter().map(String::from).collect()
                },
                "humidity_pct".to_string() => FieldConfig {
                    metrics: vec!["mean", "min", "max"]
                        .into_iter().map(String::from).collect()
                },
            },
        },
        features: None,
        refresh_policy: None,
    }
}
```

---

### TDD Cycle 3: Daily Aggregate

#### Red Phase

```rust
#[test]
fn test_generates_daily_aggregate_when_configured() {
    // Arrange: Config with both hourly and daily
    let config = GoldEtlConfig {
        aggregates: AggregatesConfig {
            granularities: vec!["1 hour".to_string(), "1 day".to_string()],
            ..Default::default()
        },
        ..create_air_quality_gold_config()
    };

    // Act
    let sqls = generate_all_continuous_aggregates(&config).unwrap();

    // Assert: Both views generated
    assert!(sqls.iter().any(|s| s.contains("gold.air_quality_hourly")));
    assert!(sqls.iter().any(|s| s.contains("gold.air_quality_daily")));
    assert!(sqls.iter().any(|s| s.contains("time_bucket('1 day'")));
}
```

#### Green Phase

```rust
pub fn generate_all_continuous_aggregates(config: &GoldEtlConfig) -> Result<Vec<String>, GeneratorError> {
    let mut sqls = Vec::new();

    for granularity in &config.aggregates.granularities {
        let view_suffix = granularity_to_suffix(granularity);
        let sql = generate_continuous_aggregate_for_granularity(config, granularity, &view_suffix)?;
        sqls.push(sql);
    }

    Ok(sqls)
}

fn granularity_to_suffix(granularity: &str) -> &str {
    match granularity {
        "1 hour" => "hourly",
        "1 day" => "daily",
        "15 minutes" => "15min",
        "4 hours" => "4h",
        g if g.ends_with("hours") => "multi_hour",
        g if g.ends_with("days") => "multi_day",
        _ => "custom",
    }
}
```

---

## Feature v11-004: Aggregate Refresh Policy

### TDD Cycle 1: Generate Refresh Policy SQL

#### Red Phase

```rust
#[test]
fn test_generates_refresh_policy() {
    // Arrange
    let view_name = "gold.air_quality_hourly";
    let policy = RefreshPolicyConfig {
        schedule_interval: "15 minutes".to_string(),
        start_offset: "4 hours".to_string(),
        end_offset: "15 minutes".to_string(),
    };

    // Act
    let sql = generate_refresh_policy_sql(view_name, &policy);

    // Assert
    assert!(sql.contains("add_continuous_aggregate_policy"));
    assert!(sql.contains("'gold.air_quality_hourly'"));
    assert!(sql.contains("schedule_interval => INTERVAL '15 minutes'"));
    assert!(sql.contains("start_offset => INTERVAL '4 hours'"));
    assert!(sql.contains("end_offset => INTERVAL '15 minutes'"));
}
```

#### Green Phase

```rust
pub fn generate_refresh_policy_sql(view_name: &str, policy: &RefreshPolicyConfig) -> String {
    format!(
        "SELECT add_continuous_aggregate_policy('{view_name}',\n    \
         start_offset => INTERVAL '{start_offset}',\n    \
         end_offset => INTERVAL '{end_offset}',\n    \
         schedule_interval => INTERVAL '{schedule_interval}'\n);",
        view_name = view_name,
        start_offset = policy.start_offset,
        end_offset = policy.end_offset,
        schedule_interval = policy.schedule_interval,
    )
}
```

---

### TDD Cycle 2: Policy with Config Defaults

#### Red Phase

```rust
#[test]
fn test_default_hourly_policy() {
    // Arrange: No explicit policy
    let config = GoldEtlConfig {
        refresh_policy: None,
        ..create_test_gold_config("air-quality")
    };

    // Act
    let policy = get_refresh_policy_for_granularity(&config, "1 hour");

    // Assert: Uses defaults
    assert_eq!(policy.schedule_interval, "15 minutes");
    assert_eq!(policy.start_offset, "4 hours");
    assert_eq!(policy.end_offset, "15 minutes");
}

#[test]
fn test_default_daily_policy() {
    let config = GoldEtlConfig {
        refresh_policy: None,
        ..create_test_gold_config("air-quality")
    };

    let policy = get_refresh_policy_for_granularity(&config, "1 day");

    // Daily has different defaults
    assert_eq!(policy.schedule_interval, "1 hour");
    assert_eq!(policy.start_offset, "3 days");
}
```

#### Green Phase

```rust
pub fn get_refresh_policy_for_granularity(
    config: &GoldEtlConfig,
    granularity: &str
) -> RefreshPolicyConfig {
    // Use explicit policy if provided
    if let Some(ref policy) = config.refresh_policy {
        return policy.clone();
    }

    // Otherwise use defaults based on granularity
    match granularity {
        "1 hour" => RefreshPolicyConfig {
            schedule_interval: "15 minutes".to_string(),
            start_offset: "4 hours".to_string(),
            end_offset: "15 minutes".to_string(),
        },
        "1 day" => RefreshPolicyConfig {
            schedule_interval: "1 hour".to_string(),
            start_offset: "3 days".to_string(),
            end_offset: "1 hour".to_string(),
        },
        _ => RefreshPolicyConfig {
            schedule_interval: "30 minutes".to_string(),
            start_offset: "4 hours".to_string(),
            end_offset: "30 minutes".to_string(),
        },
    }
}
```

---

### TDD Cycle 3: Idempotent Policy Creation

#### Red Phase

```rust
#[test]
fn test_policy_is_idempotent() {
    // Arrange: Policy already exists
    let sql = generate_idempotent_refresh_policy_sql("gold.air_quality_hourly", &default_policy());

    // Assert: Uses IF NOT EXISTS pattern or handles conflict
    assert!(
        sql.contains("IF NOT EXISTS") ||
        sql.contains("DO NOTHING") ||
        sql.contains("timescaledb_information.continuous_aggregate_stats"),
        "Policy should be idempotent"
    );
}
```

#### Green Phase

```rust
pub fn generate_idempotent_refresh_policy_sql(view_name: &str, policy: &RefreshPolicyConfig) -> String {
    // TimescaleDB doesn't have IF NOT EXISTS for policies
    // We check if policy exists first
    format!(
        "DO $$\n\
         BEGIN\n    \
         IF NOT EXISTS (\n        \
         SELECT 1 FROM timescaledb_information.jobs\n        \
         WHERE hypertable_name = '{view_name}'\n    \
         ) THEN\n        \
         PERFORM add_continuous_aggregate_policy('{view_name}',\n            \
         start_offset => INTERVAL '{start_offset}',\n            \
         end_offset => INTERVAL '{end_offset}',\n            \
         schedule_interval => INTERVAL '{schedule_interval}'\n        \
         );\n    \
         END IF;\n\
         END $$;",
        view_name = view_name.split('.').last().unwrap_or(view_name),
        start_offset = policy.start_offset,
        end_offset = policy.end_offset,
        schedule_interval = policy.schedule_interval,
    )
}
```

---

## Integration Pattern: Full Pipeline Test

### TDD Cycle: End-to-End Mock Pipeline

#### Red Phase

```rust
#[tokio::test]
async fn test_full_air_quality_pipeline() {
    // Arrange
    let loader = MockConfigLoader::new()
        .with_stream(create_typed_stream("air-quality", StreamType::Observation))
        .with_gold_config("air-quality", create_air_quality_gold_config());

    let db = MockTimescaleDb::new();

    // Act: Run full pipeline
    let result = deploy_gold_for_stream(&loader, &db, "air-quality").await;

    // Assert: Pipeline succeeded
    assert!(result.is_ok());

    // Assert: Classification synced
    assert!(db.sql_contains("stream_classification"));

    // Assert: Both aggregates created
    assert!(db.continuous_aggregate_exists("gold", "air_quality_hourly").await.unwrap());
    assert!(db.continuous_aggregate_exists("gold", "air_quality_daily").await.unwrap());

    // Assert: Refresh policies created
    assert!(db.sql_contains("add_continuous_aggregate_policy"));
}
```

#### Green Phase

```rust
pub async fn deploy_gold_for_stream(
    loader: &dyn ConfigLoader,
    db: &dyn TimescaleConnection,
    stream_id: &str
) -> Result<(), DeployError> {
    // 1. Sync classification
    sync_stream_classification(loader, db, stream_id).await?;

    // 2. Load Gold config
    let gold_config = loader.load_gold_etl_config(stream_id).await?;

    // 3. Generate and execute continuous aggregates
    let aggregate_sqls = generate_all_continuous_aggregates(&gold_config)?;
    for sql in aggregate_sqls {
        db.execute(&sql).await?;
    }

    // 4. Generate and execute refresh policies
    for granularity in &gold_config.aggregates.granularities {
        let view_suffix = granularity_to_suffix(granularity);
        let view_name = format!("gold.{}_{}", stream_id.replace("-", "_"), view_suffix);
        let policy = get_refresh_policy_for_granularity(&gold_config, granularity);
        let policy_sql = generate_idempotent_refresh_policy_sql(&view_name, &policy);
        db.execute(&policy_sql).await?;
    }

    Ok(())
}
```

---

## Test Helpers for Phase B

```rust
// tests/fixtures/phase_b.rs

/// Create air-quality stream with observation type
pub fn create_typed_stream(stream_id: &str, stream_type: StreamType) -> StreamConfig {
    let mut config = create_test_stream_config(stream_id);
    config.stream_type = Some(stream_type);
    config
}

/// Create standard air-quality gold config
pub fn create_air_quality_gold_config() -> GoldEtlConfig {
    GoldEtlConfig {
        enabled: true,
        stream_id: "air-quality".to_string(),
        description: Some("Hourly and daily aggregates for air quality".to_string()),
        aggregates: AggregatesConfig {
            granularities: vec!["1 hour".to_string(), "1 day".to_string()],
            default_metrics: Some(vec!["mean".to_string(), "count".to_string()]),
            fields: hashmap! {
                "pm25".to_string() => FieldConfig {
                    metrics: vec!["mean", "std", "min", "max", "p95"]
                        .into_iter().map(String::from).collect()
                },
                "co2".to_string() => FieldConfig {
                    metrics: vec!["mean", "std", "min", "max"]
                        .into_iter().map(String::from).collect()
                },
                "temperature_c".to_string() => FieldConfig {
                    metrics: vec!["mean", "min", "max"]
                        .into_iter().map(String::from).collect()
                },
            },
        },
        features: None,
        refresh_policy: Some(RefreshPolicyConfig {
            schedule_interval: "15 minutes".to_string(),
            start_offset: "4 hours".to_string(),
            end_offset: "15 minutes".to_string(),
        }),
    }
}
```

---

## References

- [TEST-PLAN.md](./TEST-PLAN.md) - Phase B test cases
- [Phase A TDD-GUIDE.md](../phase-a/refinement/TDD-GUIDE.md) - Foundation patterns
- [MOCK-DEFINITIONS.md](../phase-a/refinement/MOCK-DEFINITIONS.md) - Mock implementations

---

*Phase B TDD Guide created: 2026-02-04*
