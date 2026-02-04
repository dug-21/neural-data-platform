# Phase C: Cross-Stream + Alignment - TDD Guide (London School)

> **Phase:** C (Cross-Stream + Alignment)
> **Testing Approach:** London TDD (Outside-In, Mock-Driven)
> **Parent Document:** [TEST-PLAN.md](./TEST-PLAN.md)
> **Created:** 2026-02-04

---

## Overview

This guide provides step-by-step London TDD instructions for Phase C features. Phase C introduces the most complex SQL generation: cross-stream JOINs with type-specific NULL handling.

**Key Challenge**: Testing JOIN logic without a real database. The solution is rigorous SQL string testing combined with MockTimescaleDb for execution verification.

---

## Feature v11-005: Cross-Stream Aligned View

### TDD Cycle 1: Basic Two-Stream JOIN

#### Red Phase: Write Failing Test

```rust
// Location: tools/ndp-gold-ddl/src/generators/aligned_view.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generates_full_outer_join_for_two_streams() {
        // Arrange
        let domain = DomainConfig {
            id: "test-domain".to_string(),
            streams: vec![
                StreamRef {
                    stream_id: "air-quality".to_string(),
                    alias: "aq".to_string(),
                    role: StreamRole::Primary,
                },
                StreamRef {
                    stream_id: "outdoor-weather".to_string(),
                    alias: "ow".to_string(),
                    role: StreamRole::Context,
                },
            ],
            alignment: AlignmentConfig {
                view_name: "test_aligned".to_string(),
                granularity: "1 hour".to_string(),
                join_strategy: JoinStrategy::FullOuter,
                null_handling: NullHandling::Preserve,
            },
            objectives: vec![],
        };

        // Act
        let sql = generate_aligned_view(&domain).unwrap();

        // Assert
        assert!(sql.contains("FULL OUTER JOIN"));
        assert!(sql.contains("gold.air_quality_hourly"));
        assert!(sql.contains("gold.outdoor_weather_hourly"));
    }
}
```

**Expected Failure**: `generate_aligned_view` function does not exist.

#### Green Phase: Minimal Implementation

```rust
pub fn generate_aligned_view(config: &DomainConfig) -> Result<String, GeneratorError> {
    let view_name = format!("gold.{}", config.alignment.view_name);

    // First stream is FROM
    let first = config.streams.iter()
        .find(|s| s.role == StreamRole::Primary)
        .or_else(|| config.streams.first())
        .ok_or(GeneratorError::NoStreams)?;

    let first_table = format!(
        "gold.{}_hourly {}",
        first.stream_id.replace("-", "_"),
        first.alias
    );

    // Remaining streams are JOINs
    let joins: Vec<String> = config.streams.iter()
        .filter(|s| s.stream_id != first.stream_id)
        .map(|s| {
            let table = format!("gold.{}_hourly", s.stream_id.replace("-", "_"));
            format!(
                "FULL OUTER JOIN {} {} ON {}.bucket = {}.bucket",
                table, s.alias, first.alias, s.alias
            )
        })
        .collect();

    Ok(format!(
        "CREATE MATERIALIZED VIEW {view_name} AS\n\
         SELECT\n    bucket,\n    -- columns\n\
         FROM {first_table}\n\
         {};",
        joins.join("\n")
    ))
}
```

#### Refactor Phase

- Extract JOIN generation to separate function
- Add support for different join strategies

---

### TDD Cycle 2: COALESCE Bucket

#### Red Phase

```rust
#[test]
fn test_bucket_coalesces_from_all_streams() {
    // Arrange
    let domain = create_three_stream_domain();

    // Act
    let sql = generate_aligned_view(&domain).unwrap();

    // Assert: Bucket should COALESCE from all stream aliases
    assert!(sql.contains("COALESCE(aq.bucket, ow.bucket, se.bucket)") ||
            sql.contains("COALESCE("));

    // All aliases should appear in COALESCE
    for stream in &domain.streams {
        assert!(sql.contains(&format!("{}.bucket", stream.alias)),
                "Missing bucket for alias: {}", stream.alias);
    }
}
```

#### Green Phase

```rust
fn generate_bucket_coalesce(streams: &[StreamRef]) -> String {
    let buckets: Vec<String> = streams.iter()
        .map(|s| format!("{}.bucket", s.alias))
        .collect();

    format!("COALESCE({}) AS bucket", buckets.join(", "))
}
```

---

### TDD Cycle 3: NULL Handling by Stream Type (ADR-FE001-004)

#### Red Phase

```rust
#[test]
fn test_observation_preserves_null() {
    // Arrange
    let stream = StreamRef {
        stream_id: "air-quality".to_string(),
        alias: "aq".to_string(),
        role: StreamRole::Primary,
    };
    let column = "pm25_mean";
    let stream_type = StreamType::Observation;

    // Act
    let sql = generate_column_select(&stream, column, stream_type, NullHandling::ByStreamType);

    // Assert: Should NOT use COALESCE or LAG for observation
    assert!(sql.contains("aq.pm25_mean"));
    assert!(!sql.contains("COALESCE(aq.pm25_mean, LAG"));
}

#[test]
fn test_state_event_carries_forward() {
    // Arrange
    let stream = StreamRef {
        stream_id: "home-assistant-state".to_string(),
        alias: "se".to_string(),
        role: StreamRole::Actuator,
    };
    let column = "window_state";
    let stream_type = StreamType::StateEvent;

    // Act
    let sql = generate_column_select(&stream, column, stream_type, NullHandling::ByStreamType);

    // Assert: Should use LOCF pattern
    assert!(sql.contains("COALESCE") || sql.contains("LAG"));
    assert!(sql.contains("IGNORE NULLS") || sql.contains("OVER ("));
}
```

#### Green Phase

```rust
fn generate_column_select(
    stream: &StreamRef,
    column: &str,
    stream_type: StreamType,
    null_handling: NullHandling
) -> String {
    let full_column = format!("{}.{}", stream.alias, column);
    let alias = format!("{}_{}", stream.alias, column);

    match get_effective_null_handling(stream_type, null_handling) {
        NullHandling::Preserve => {
            format!("{} AS {}", full_column, alias)
        }
        NullHandling::CarryForward => {
            // LOCF (Last Observation Carried Forward)
            format!(
                "COALESCE({col}, LAG({col}) IGNORE NULLS OVER (ORDER BY bucket)) AS {alias}",
                col = full_column,
                alias = alias
            )
        }
        NullHandling::Interpolate => {
            // Linear interpolation - more complex
            format!(
                "COALESCE({col}, ({prev} + {next}) / 2.0) AS {alias}",
                col = full_column,
                prev = format!("LAG({}) IGNORE NULLS OVER (ORDER BY bucket)", full_column),
                next = format!("LEAD({}) IGNORE NULLS OVER (ORDER BY bucket)", full_column),
                alias = alias
            )
        }
        NullHandling::ByStreamType => unreachable!("Should be resolved")
    }
}

fn get_effective_null_handling(stream_type: StreamType, configured: NullHandling) -> NullHandling {
    if configured != NullHandling::ByStreamType {
        return configured;
    }

    match stream_type {
        StreamType::Observation => NullHandling::Preserve,
        StreamType::StateEvent => NullHandling::CarryForward,
        StreamType::Forecast => NullHandling::Preserve,
        StreamType::Dimension => NullHandling::CarryForward,
    }
}
```

---

### TDD Cycle 4: Column Aliasing by Stream Role

#### Red Phase

```rust
#[test]
fn test_primary_stream_uses_descriptive_alias() {
    // Arrange
    let domain = DomainConfig {
        streams: vec![
            StreamRef {
                stream_id: "air-quality".to_string(),
                alias: "indoor".to_string(),  // Descriptive alias
                role: StreamRole::Primary,
            },
        ],
        ..Default::default()
    };

    // Act
    let sql = generate_aligned_view(&domain).unwrap();

    // Assert: Uses alias, not stream_id
    assert!(sql.contains("indoor_pm25") || sql.contains("AS indoor_"));
    assert!(!sql.contains("air_quality_pm25"));
}

#[test]
fn test_columns_prefixed_with_stream_alias() {
    let domain = create_three_stream_domain();

    let sql = generate_aligned_view(&domain).unwrap();

    // Each stream's columns prefixed with alias
    assert!(sql.contains("aq.") || sql.contains("AS aq_"));
    assert!(sql.contains("ow.") || sql.contains("AS ow_"));
    assert!(sql.contains("se.") || sql.contains("AS se_"));
}
```

#### Green Phase

```rust
fn generate_select_columns(config: &DomainConfig, loader: &dyn ConfigLoader) -> Result<Vec<String>, GeneratorError> {
    let mut columns = Vec::new();

    // Bucket column
    columns.push(generate_bucket_coalesce(&config.streams));

    // Columns from each stream
    for stream in &config.streams {
        let gold_config = loader.load_gold_etl_config(&stream.stream_id)?;
        let stream_type = loader.load_stream_config(&stream.stream_id)?.stream_type
            .unwrap_or(StreamType::Observation);

        for (field, field_config) in &gold_config.aggregates.fields {
            for metric in &field_config.metrics {
                let column = format!("{}_{}", field, metric);
                let select = generate_column_select(
                    stream,
                    &column,
                    stream_type,
                    config.alignment.null_handling
                );
                columns.push(select);
            }
        }
    }

    Ok(columns)
}
```

---

## Feature v11-006: State Transition Materializer

### TDD Cycle 1: Basic Transition Detection

#### Red Phase

```rust
#[test]
fn test_generates_transition_view() {
    // Arrange
    let config = TransitionConfig {
        stream_id: "home-assistant-state".to_string(),
        state_field: "state".to_string(),
        entity_field: "ndp_id".to_string(),
        track_duration: true,
    };

    // Act
    let sql = generate_state_transitions(&config).unwrap();

    // Assert
    assert!(sql.contains("CREATE") && sql.contains("VIEW"));
    assert!(sql.contains("LAG(state)"));
    assert!(sql.contains("from_state"));
    assert!(sql.contains("to_state"));
}
```

#### Green Phase

```rust
pub fn generate_state_transitions(config: &TransitionConfig) -> Result<String, GeneratorError> {
    let view_name = format!("gold.{}_transitions", config.stream_id.replace("-", "_"));
    let source_table = format!("silver.{}", config.stream_id.replace("-", "_"));

    let window_clause = format!(
        "PARTITION BY {} ORDER BY event_time",
        config.entity_field
    );

    Ok(format!(
        "CREATE OR REPLACE VIEW {view_name} AS\n\
         SELECT\n    \
         event_time AS transition_time,\n    \
         {entity} AS entity_id,\n    \
         LAG({state}) OVER ({window}) AS from_state,\n    \
         {state} AS to_state\n\
         FROM {source};",
        view_name = view_name,
        entity = config.entity_field,
        state = config.state_field,
        window = window_clause,
        source = source_table,
    ))
}
```

---

### TDD Cycle 2: is_actual_transition Column

#### Red Phase

```rust
#[test]
fn test_is_actual_transition_filters_noise() {
    let config = create_transition_config("home-assistant-state");

    let sql = generate_state_transitions(&config).unwrap();

    // Should have boolean column for actual transitions
    assert!(sql.contains("is_actual_transition"));

    // Logic: LAG(state) IS DISTINCT FROM state (or IS NULL for first)
    assert!(sql.contains("IS DISTINCT FROM") || sql.contains("IS NOT NULL"));
}

#[test]
fn test_first_event_is_transition() {
    let config = create_transition_config("home-assistant-state");

    let sql = generate_state_transitions(&config).unwrap();

    // First event (LAG is NULL) should be marked as transition
    assert!(sql.contains("IS NULL") && sql.contains("LAG"));
}
```

#### Green Phase

```rust
fn generate_is_actual_transition(state_field: &str, window_ref: &str) -> String {
    format!(
        "CASE\n        \
         WHEN LAG({state}) OVER {window} IS DISTINCT FROM {state} THEN TRUE\n        \
         WHEN LAG({state}) OVER {window} IS NULL THEN TRUE\n        \
         ELSE FALSE\n    \
         END AS is_actual_transition",
        state = state_field,
        window = window_ref,
    )
}
```

---

### TDD Cycle 3: Duration in Previous State

#### Red Phase

```rust
#[test]
fn test_duration_calculated() {
    let config = TransitionConfig {
        track_duration: true,
        ..create_transition_config("home-assistant-state")
    };

    let sql = generate_state_transitions(&config).unwrap();

    assert!(sql.contains("duration_in_previous"));
    assert!(sql.contains("event_time -") || sql.contains("EXTRACT"));
    assert!(sql.contains("LAG(event_time)"));
}

#[test]
fn test_duration_skipped_when_disabled() {
    let config = TransitionConfig {
        track_duration: false,
        ..create_transition_config("home-assistant-state")
    };

    let sql = generate_state_transitions(&config).unwrap();

    assert!(!sql.contains("duration_in_previous"));
}
```

#### Green Phase

```rust
pub fn generate_state_transitions(config: &TransitionConfig) -> Result<String, GeneratorError> {
    let view_name = format!("gold.{}_transitions", config.stream_id.replace("-", "_"));
    let source_table = format!("silver.{}", config.stream_id.replace("-", "_"));
    let window_clause = format!("PARTITION BY {} ORDER BY event_time", config.entity_field);
    let window_ref = format!("({})", window_clause);

    let mut columns = vec![
        "event_time AS transition_time".to_string(),
        format!("{} AS entity_id", config.entity_field),
        format!("LAG({}) OVER {} AS from_state", config.state_field, window_ref),
        format!("{} AS to_state", config.state_field),
        generate_is_actual_transition(&config.state_field, &window_ref),
    ];

    if config.track_duration {
        columns.push(format!(
            "event_time - LAG(event_time) OVER {} AS duration_in_previous_state",
            window_ref
        ));
    }

    Ok(format!(
        "CREATE OR REPLACE VIEW {view_name} AS\n\
         SELECT\n    {columns}\n\
         FROM {source}\n\
         WINDOW w AS ({window});",
        view_name = view_name,
        columns = columns.join(",\n    "),
        source = source_table,
        window = window_clause,
    ))
}
```

---

## Feature v11-007: Objectives Storage

### TDD Cycle 1: Parse Objectives from Config

#### Red Phase

```rust
#[test]
fn test_parse_objective_from_yaml() {
    let yaml = r#"
        id: healthy_co2
        target:
          stream: air-quality
          metric: co2
          condition: "<"
          threshold: 800
          unit: ppm
        priority: high
    "#;

    let objective: Objective = serde_yaml::from_str(yaml).unwrap();

    assert_eq!(objective.id, "healthy_co2");
    assert_eq!(objective.target.threshold, 800.0);
    assert_eq!(objective.priority, Some("high".to_string()));
}
```

#### Green Phase

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Objective {
    pub id: String,
    pub target: ObjectiveTarget,
    pub priority: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectiveTarget {
    pub stream: String,
    pub metric: String,
    pub condition: String,
    pub threshold: f64,
    pub unit: Option<String>,
}
```

---

### TDD Cycle 2: Generate Insert SQL

#### Red Phase

```rust
#[test]
fn test_generates_objective_insert() {
    let objective = Objective {
        id: "healthy_co2".to_string(),
        target: ObjectiveTarget {
            stream: "air-quality".to_string(),
            metric: "co2".to_string(),
            condition: "<".to_string(),
            threshold: 800.0,
            unit: Some("ppm".to_string()),
        },
        priority: Some("high".to_string()),
    };

    let sql = generate_objective_insert_sql(&objective, "indoor-air-quality");

    assert!(sql.contains("INSERT INTO data_dictionary.objectives"));
    assert!(sql.contains("'healthy_co2'"));
    assert!(sql.contains("'indoor-air-quality'"));
    assert!(sql.contains("800"));
    assert!(sql.contains("'<'"));
}
```

#### Green Phase

```rust
pub fn generate_objective_insert_sql(objective: &Objective, domain_id: &str) -> String {
    let priority = objective.priority.as_deref().unwrap_or("medium");
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
```

---

### TDD Cycle 3: Sync All Objectives

#### Red Phase

```rust
#[tokio::test]
async fn test_sync_all_objectives() {
    // Arrange
    let db = MockTimescaleDb::new();
    let domain = DomainConfig {
        id: "indoor-air-quality".to_string(),
        objectives: vec![
            create_objective("healthy_co2", "co2", "<", 800.0),
            create_objective("healthy_pm25", "pm25", "<", 12.0),
        ],
        ..Default::default()
    };

    // Act
    sync_objectives(&db, &domain).await.unwrap();

    // Assert: Both objectives synced
    let sqls = db.get_executed_sql();
    assert!(sqls.iter().any(|s| s.contains("healthy_co2")));
    assert!(sqls.iter().any(|s| s.contains("healthy_pm25")));
}
```

#### Green Phase

```rust
pub async fn sync_objectives(
    db: &dyn TimescaleConnection,
    domain: &DomainConfig
) -> Result<(), SyncError> {
    for objective in &domain.objectives {
        let sql = generate_objective_insert_sql(objective, &domain.id);
        db.execute(&sql).await?;
    }

    Ok(())
}
```

---

## Test Helpers for Phase C

```rust
// tests/fixtures/phase_c.rs

pub fn create_three_stream_domain() -> DomainConfig {
    DomainConfig {
        id: "indoor-air-quality".to_string(),
        description: Some("Indoor air quality domain".to_string()),
        streams: vec![
            StreamRef {
                stream_id: "air-quality".to_string(),
                alias: "aq".to_string(),
                role: StreamRole::Primary,
            },
            StreamRef {
                stream_id: "outdoor-weather".to_string(),
                alias: "ow".to_string(),
                role: StreamRole::Context,
            },
            StreamRef {
                stream_id: "home-assistant-state".to_string(),
                alias: "se".to_string(),
                role: StreamRole::Actuator,
            },
        ],
        alignment: AlignmentConfig {
            view_name: "indoor_air_quality_aligned".to_string(),
            granularity: "1 hour".to_string(),
            join_strategy: JoinStrategy::FullOuter,
            null_handling: NullHandling::ByStreamType,
        },
        objectives: vec![
            create_objective("healthy_co2", "co2", "<", 800.0),
            create_objective("healthy_pm25", "pm25", "<", 12.0),
        ],
    }
}

pub fn create_transition_config(stream_id: &str) -> TransitionConfig {
    TransitionConfig {
        stream_id: stream_id.to_string(),
        state_field: "state".to_string(),
        entity_field: "ndp_id".to_string(),
        track_duration: true,
    }
}

pub fn create_objective(id: &str, metric: &str, condition: &str, threshold: f64) -> Objective {
    Objective {
        id: id.to_string(),
        target: ObjectiveTarget {
            stream: "air-quality".to_string(),
            metric: metric.to_string(),
            condition: condition.to_string(),
            threshold,
            unit: None,
        },
        priority: Some("high".to_string()),
    }
}
```

---

## References

- [TEST-PLAN.md](./TEST-PLAN.md) - Phase C test cases
- [ADR-FE001-004](../../architecture/ADR-FE001-004-null-handling.md) - NULL handling
- [Phase B TDD-GUIDE.md](../../phase-b/refinement/TDD-GUIDE.md) - Reference patterns

---

*Phase C TDD Guide created: 2026-02-04*
