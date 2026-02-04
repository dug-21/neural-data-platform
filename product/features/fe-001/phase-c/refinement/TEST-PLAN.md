# Phase C: Cross-Stream + Alignment - Test Plan

> **Phase:** C (Cross-Stream + Alignment)
> **Target:** Week 4
> **Testing Approach:** London TDD (Outside-In)
> **Parent Document:** [TESTING-STRATEGY.md](../../TESTING-STRATEGY.md)

---

## Overview

Phase C extends Gold layer to three streams and introduces the cross-stream aligned view. Tests validate JOIN complexity, NULL handling by stream type, and state transition extraction.

**Key Challenge**: Testing complex SQL JOINs and ensuring correct NULL handling across different stream types.

---

## Phase C Scope

| ID | Feature | Testing Priority |
|----|---------|------------------|
| **v11-005** | Cross-Stream Aligned View | Critical |
| **v11-006** | State Transition Materializer | High |
| **v11-007** | Objectives Storage | Medium |
| **v11-003** | Per-Stream Continuous Aggregates (outdoor-weather, state-events) | Critical |

---

## 1. Test Development Order (Outside-In)

```
1. ACCEPTANCE TESTS (define success)
   ├── Aligned view with 3 streams
   ├── State transitions extracted correctly
   └── Objectives stored and queryable

2. COMPONENT TESTS (verify behavior)
   ├── FULL OUTER JOIN generation
   ├── NULL handling by stream type
   ├── State transition detection logic
   └── Objectives CRUD operations

3. UNIT TESTS (implement details)
   ├── COALESCE bucket generation
   ├── LOCF (carry forward) SQL
   ├── is_actual_transition logic
   └── Objective condition parsing
```

---

## 2. v11-003: Additional Stream Aggregates Tests

### 2.1 outdoor-weather Tests

```rust
/// ACCEPTANCE: outdoor-weather continuous aggregate generated
#[test]
fn acceptance_outdoor_weather_hourly_generated() {
    let config = load_stream_config("config/base/streams/outdoor-weather/config.yaml");

    let sql = generate_gold_ddl(&config).unwrap();

    assert!(sql.contains("CREATE MATERIALIZED VIEW gold.outdoor_weather_hourly"));
    assert!(sql.contains("temperature_c_mean") || sql.contains("AVG(temperature_c)"));
    assert!(sql.contains("humidity_pct_mean") || sql.contains("AVG(humidity_pct)"));
}

/// Unit: outdoor-weather view references correct Silver table
#[test]
fn test_outdoor_weather_references_silver() {
    let config = create_gold_config_for_stream("outdoor-weather");

    let sql = generate_continuous_aggregate(&config).unwrap();

    assert!(sql.contains("FROM silver.weather_observations") ||
            sql.contains("FROM silver.outdoor_weather"));
}
```

### 2.2 home-assistant-state Tests

```rust
/// ACCEPTANCE: state-events aggregate handles state_event type
#[test]
fn acceptance_state_events_hourly_generated() {
    let config = load_stream_config("config/base/streams/home-assistant-state/config.yaml");

    let sql = generate_gold_ddl(&config).unwrap();

    assert!(sql.contains("CREATE MATERIALIZED VIEW gold.state_events_hourly") ||
            sql.contains("CREATE MATERIALIZED VIEW gold.home_assistant_state_hourly"));
}

/// Unit: state_event streams aggregate state changes
#[test]
fn test_state_event_aggregates_changes() {
    let config = create_state_event_gold_config("home-assistant-state");

    let sql = generate_continuous_aggregate(&config).unwrap();

    // State events should count transitions, not just average
    assert!(sql.contains("COUNT(*)") || sql.contains("SUM(CASE"));
}
```

---

## 3. v11-005: Cross-Stream Aligned View Tests

### 3.1 Acceptance Tests

```rust
/// ACCEPTANCE: Aligned view joins all 3 streams
#[test]
fn acceptance_aligned_view_joins_three_streams() {
    let domain_config = load_domain_config("config/domains/indoor-air-quality/domain.yaml");

    let sql = generate_aligned_view(&domain_config).unwrap();

    // Three streams should be joined
    assert!(sql.contains("gold.air_quality_hourly"));
    assert!(sql.contains("gold.outdoor_weather_hourly"));
    assert!(sql.contains("gold.state_events_hourly") || sql.contains("gold.home_assistant_state_hourly"));
}

/// ACCEPTANCE: Aligned view uses FULL OUTER JOIN
#[test]
fn acceptance_aligned_view_uses_full_outer_join() {
    let domain_config = load_domain_config("config/domains/indoor-air-quality/domain.yaml");

    let sql = generate_aligned_view(&domain_config).unwrap();

    // Should use FULL OUTER JOIN for preserving all rows
    assert!(sql.contains("FULL OUTER JOIN"));
}

/// ACCEPTANCE: Aligned view creates correct output columns
#[test]
fn acceptance_aligned_view_has_expected_columns() {
    let domain_config = load_domain_config("config/domains/indoor-air-quality/domain.yaml");

    let sql = generate_aligned_view(&domain_config).unwrap();

    // Should have aliased columns from each stream
    assert!(sql.contains("indoor_pm25") || sql.contains("AS indoor_pm25"));
    assert!(sql.contains("outdoor_temp") || sql.contains("AS outdoor_temp"));
    assert!(sql.contains("window_opens") || sql.contains("transition_count"));
}
```

### 3.2 Component Tests

```rust
/// Component: JOIN generation with MockConfigLoader
#[tokio::test]
async fn test_aligned_view_with_mock_streams() {
    // Arrange
    let loader = MockConfigLoader::new()
        .with_stream(create_typed_stream("air-quality", StreamType::Observation))
        .with_stream(create_typed_stream("outdoor-weather", StreamType::Observation))
        .with_stream(create_typed_stream("home-assistant-state", StreamType::StateEvent));

    let domain_config = create_test_domain_config("indoor-air-quality");

    // Act
    let sql = generate_aligned_view(&domain_config).unwrap();

    // Assert: All streams joined
    let join_count = sql.matches("FULL OUTER JOIN").count();
    assert_eq!(join_count, 2, "Should have 2 JOINs for 3 streams");
}

/// Component: COALESCE bucket handles all streams
#[test]
fn test_coalesce_bucket_from_all_streams() {
    let domain_config = create_test_domain_config("indoor-air-quality");

    let sql = generate_aligned_view(&domain_config).unwrap();

    // Bucket should COALESCE from all streams
    assert!(sql.contains("COALESCE("));
    // Should reference bucket from each alias
    let aliases = ["aq.bucket", "ow.bucket", "se.bucket"];
    for alias in &aliases {
        assert!(sql.contains(alias) || sql.contains(&alias.replace(".", "_")),
                "Missing COALESCE for {}", alias);
    }
}

/// Component: Column aliasing follows convention
#[test]
fn test_column_aliasing_convention() {
    let domain_config = DomainConfig {
        streams: vec![
            StreamRef { stream_id: "air-quality".to_string(), alias: "indoor".to_string(), role: StreamRole::Primary },
            StreamRef { stream_id: "outdoor-weather".to_string(), alias: "outdoor".to_string(), role: StreamRole::Context },
        ],
        ..create_test_domain_config("test")
    };

    let sql = generate_aligned_view(&domain_config).unwrap();

    // Columns should be prefixed with alias
    assert!(sql.contains("AS indoor_") || sql.contains("indoor."));
    assert!(sql.contains("AS outdoor_") || sql.contains("outdoor."));
}
```

### 3.3 NULL Handling Tests (ADR-FE001-004)

```rust
/// Unit: Observation streams preserve NULL
#[test]
fn test_observation_null_handling_preserve() {
    let config = DomainConfig {
        streams: vec![
            StreamRef {
                stream_id: "air-quality".to_string(),
                alias: "aq".to_string(),
                role: StreamRole::Primary,
            },
        ],
        alignment: AlignmentConfig {
            null_handling: NullHandling::ByStreamType,
            ..Default::default()
        },
        ..Default::default()
    };

    let sql = generate_column_for_stream(&config.streams[0], "pm25_mean", StreamType::Observation);

    // Observation should NOT use COALESCE/LOCF
    assert!(!sql.contains("COALESCE") || sql.contains("COALESCE(aq.bucket"));
    assert!(!sql.contains("LAG(") || sql.contains("LAG(aq.bucket"));
}

/// Unit: state_event streams use LOCF (carry forward)
#[test]
fn test_state_event_null_handling_locf() {
    let config = DomainConfig {
        streams: vec![
            StreamRef {
                stream_id: "home-assistant-state".to_string(),
                alias: "se".to_string(),
                role: StreamRole::Actuator,
            },
        ],
        alignment: AlignmentConfig {
            null_handling: NullHandling::ByStreamType,
            ..Default::default()
        },
        ..Default::default()
    };

    let sql = generate_column_for_stream(&config.streams[0], "window_state", StreamType::StateEvent);

    // State event should use LOCF pattern
    assert!(sql.contains("COALESCE") || sql.contains("LAG"));
    assert!(sql.contains("OVER") || sql.contains("IGNORE NULLS"));
}

/// Unit: NULL handling follows ADR-FE001-004
#[test]
fn test_null_handling_by_stream_type_enum() {
    // observation -> preserve
    assert_eq!(
        get_null_handling_for_type(StreamType::Observation),
        NullHandling::Preserve
    );

    // state_event -> carry_forward
    assert_eq!(
        get_null_handling_for_type(StreamType::StateEvent),
        NullHandling::CarryForward
    );

    // forecast -> preserve (use actual forecast, don't carry forward old)
    assert_eq!(
        get_null_handling_for_type(StreamType::Forecast),
        NullHandling::Preserve
    );
}
```

### 3.4 JOIN Order Tests

```rust
/// Unit: Primary stream is first in FROM clause
#[test]
fn test_primary_stream_first_in_from() {
    let domain_config = DomainConfig {
        streams: vec![
            StreamRef { stream_id: "outdoor-weather".to_string(), alias: "ow".to_string(), role: StreamRole::Context },
            StreamRef { stream_id: "air-quality".to_string(), alias: "aq".to_string(), role: StreamRole::Primary },
        ],
        ..create_test_domain_config("test")
    };

    let sql = generate_aligned_view(&domain_config).unwrap();

    // Primary stream should be in FROM, not in JOIN
    let from_pos = sql.find("FROM gold.air_quality");
    let join_pos = sql.find("JOIN gold.air_quality");

    assert!(from_pos.is_some(), "Primary stream should be in FROM");
    assert!(join_pos.is_none() || from_pos.unwrap() < join_pos.unwrap(),
            "Primary stream should be first");
}
```

---

## 4. v11-006: State Transition Materializer Tests

### 4.1 Acceptance Tests

```rust
/// ACCEPTANCE: State transitions view generated from config
#[test]
fn acceptance_state_transitions_view_generated() {
    let config = load_stream_config("config/base/streams/home-assistant-state/config.yaml");

    let sql = generate_state_transitions(&config).unwrap();

    assert!(sql.contains("CREATE VIEW") || sql.contains("CREATE OR REPLACE VIEW"));
    assert!(sql.contains("gold.") && sql.contains("_transitions"));
}

/// ACCEPTANCE: Transition detects state change
#[test]
fn acceptance_transition_detects_change() {
    let sql = generate_state_transitions_sql("home-assistant-state", "state", "ndp_id");

    assert!(sql.contains("LAG(state)"));
    assert!(sql.contains("IS DISTINCT FROM") || sql.contains("!="));
    assert!(sql.contains("from_state"));
    assert!(sql.contains("to_state"));
}
```

### 4.2 Component Tests

```rust
/// Component: is_actual_transition filters noise
#[test]
fn test_is_actual_transition_column() {
    let sql = generate_state_transitions_sql("test-stream", "state", "entity_id");

    // Should have column that identifies real transitions
    assert!(sql.contains("is_actual_transition"));
    assert!(sql.contains("CASE WHEN") || sql.contains("DISTINCT FROM"));
}

/// Component: Duration calculation included
#[test]
fn test_duration_in_previous_state() {
    let sql = generate_state_transitions_sql("test-stream", "state", "entity_id");

    assert!(sql.contains("duration") || sql.contains("EXTRACT") || sql.contains("interval"));
    assert!(sql.contains("LAG(") && sql.contains("event_time"));
}

/// Component: Partitioned by entity
#[test]
fn test_transitions_partitioned_by_entity() {
    let sql = generate_state_transitions_sql("test-stream", "state", "ndp_id");

    assert!(sql.contains("PARTITION BY ndp_id"));
    assert!(sql.contains("ORDER BY") && (sql.contains("event_time") || sql.contains("observation_time")));
}
```

### 4.3 Unit Tests

```rust
/// Unit: WINDOW clause generated correctly
#[test]
fn test_window_clause_generation() {
    let sql = generate_window_clause("ndp_id", "event_time");

    assert_eq!(
        sql,
        "PARTITION BY ndp_id ORDER BY event_time"
    );
}

/// Unit: Transition from NULL handled (first event)
#[test]
fn test_first_event_is_transition() {
    let sql = generate_state_transitions_sql("test", "state", "entity");

    // First event (where LAG is NULL) should be marked as transition
    assert!(sql.contains("LAG(state) OVER") && sql.contains("IS NULL"));
}

/// Unit: State field is configurable
#[test]
fn test_configurable_state_field() {
    let sql_state = generate_state_transitions_sql("test", "state", "entity");
    let sql_status = generate_state_transitions_sql("test", "status", "entity");

    assert!(sql_state.contains("LAG(state)"));
    assert!(sql_status.contains("LAG(status)"));
}
```

---

## 5. v11-007: Objectives Storage Tests

### 5.1 Acceptance Tests

```rust
/// ACCEPTANCE: Objectives loaded from domain config
#[test]
fn acceptance_objectives_loaded_from_config() {
    let domain = load_domain_config("config/domains/indoor-air-quality/domain.yaml");

    assert!(!domain.objectives.is_empty());

    let healthy_co2 = domain.objectives.iter()
        .find(|o| o.id == "healthy_co2");
    assert!(healthy_co2.is_some());

    let obj = healthy_co2.unwrap();
    assert_eq!(obj.target.stream, "air-quality");
    assert_eq!(obj.target.metric, "co2");
    assert_eq!(obj.target.condition, "<");
    assert_eq!(obj.target.threshold, 800.0);
}

/// ACCEPTANCE: Objectives synced to data dictionary
#[tokio::test]
async fn acceptance_objectives_synced_to_dictionary() {
    // Arrange
    let db = MockTimescaleDb::new();
    let domain = load_domain_config("config/domains/indoor-air-quality/domain.yaml");

    // Act
    sync_objectives(&db, &domain).await.unwrap();

    // Assert
    assert!(db.sql_contains("INSERT INTO data_dictionary.objectives"));
    assert!(db.sql_contains("'healthy_co2'"));
}
```

### 5.2 Component Tests

```rust
/// Component: Objective SQL generation
#[test]
fn test_objective_insert_sql() {
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

    assert!(sql.contains("'healthy_co2'"));
    assert!(sql.contains("'air-quality'"));
    assert!(sql.contains("'co2'"));
    assert!(sql.contains("'<'"));
    assert!(sql.contains("800"));
}

/// Component: All condition types supported
#[test]
fn test_all_condition_types() {
    for condition in &["<", ">", "<=", ">=", "==", "!="] {
        let objective = Objective {
            target: ObjectiveTarget {
                condition: condition.to_string(),
                threshold: 100.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let sql = generate_objective_insert_sql(&objective, "test");
        assert!(sql.contains(condition), "Condition {} not in SQL", condition);
    }
}
```

### 5.3 Unit Tests

```rust
/// Unit: Objective validation rejects invalid condition
#[test]
fn test_invalid_condition_rejected() {
    let objective = Objective {
        target: ObjectiveTarget {
            condition: "LIKE".to_string(), // Invalid
            ..Default::default()
        },
        ..Default::default()
    };

    let result = validate_objective(&objective);
    assert!(result.is_err());
}

/// Unit: Priority enum parsing
#[test]
fn test_priority_parsing() {
    for (value, expected) in &[
        ("low", Priority::Low),
        ("medium", Priority::Medium),
        ("high", Priority::High),
        ("critical", Priority::Critical),
    ] {
        let json = json!({ "priority": value });
        let priority: Priority = serde_json::from_value(json["priority"].clone()).unwrap();
        assert_eq!(priority, *expected);
    }
}
```

---

## 6. Integration Tests

```rust
/// INTEGRATION: Full Phase C deployment
#[tokio::test]
#[ignore]
async fn integration_phase_c_full_deployment() {
    // Arrange: Clean state
    cleanup_gold_schema().await;
    setup_silver_tables(&["air-quality", "outdoor-weather", "home-assistant-state"]).await;

    // Act: Deploy Phase C
    deploy_manifest(".deploy/test/phase-c-alignment.manifest.json").await.unwrap();

    // Assert: All continuous aggregates exist
    assert!(check_continuous_aggregate_exists("gold", "air_quality_hourly").await);
    assert!(check_continuous_aggregate_exists("gold", "outdoor_weather_hourly").await);
    assert!(check_continuous_aggregate_exists("gold", "state_events_hourly").await);

    // Assert: Aligned view exists
    assert!(check_view_exists("gold", "indoor_air_quality_aligned").await);

    // Assert: Objectives stored
    let objectives = query_objectives("indoor-air-quality").await;
    assert!(!objectives.is_empty());
}

/// INTEGRATION: Aligned view query performance
#[tokio::test]
#[ignore]
async fn integration_aligned_view_performance() {
    // Arrange: Ensure data exists
    setup_test_data_30_days().await;

    // Act: Timed query
    let start = std::time::Instant::now();
    let rows = query_aligned_view("indoor_air_quality_aligned", 30).await;
    let duration = start.elapsed();

    // Assert: < 100ms
    assert!(
        duration.as_millis() < 100,
        "Aligned view query took {}ms, expected < 100ms", duration.as_millis()
    );
}

/// INTEGRATION: State transitions extraction
#[tokio::test]
#[ignore]
async fn integration_state_transitions_work() {
    // Arrange
    setup_state_event_test_data().await;

    // Act
    let transitions = query_state_transitions("home-assistant-state").await;

    // Assert: Transitions detected
    assert!(!transitions.is_empty());

    // Assert: is_actual_transition filtering works
    let actual_transitions = transitions.iter()
        .filter(|t| t.is_actual_transition)
        .count();
    assert!(actual_transitions > 0);
    assert!(actual_transitions <= transitions.len());
}
```

---

## 7. Test Execution Commands

```bash
# Run Phase C unit tests
cargo test -p ndp-gold-ddl --lib aligned_view
cargo test -p ndp-gold-ddl --lib state_transitions
cargo test -p ndp-gold-ddl --lib objectives

# Run Phase C component tests
cargo test -p ndp-gold-ddl --test alignment_tests
cargo test -p ndp-gold-ddl --test transition_tests

# Run Phase C integration tests (requires Docker)
DEPLOY_ENV=integration cargo test -p ndp-gold-ddl --test integration -- phase_c --ignored
```

---

## 8. Test Metrics (Phase C Target)

| Category | Target | Priority |
|----------|--------|----------|
| Unit Tests | 20-25 | High |
| Component Tests | 8-10 | High |
| Integration Tests | 3-5 | Critical |
| Coverage (aligned_view.rs) | 85% | Critical |
| Coverage (state_transitions.rs) | 80% | High |
| Test Duration (unit) | <5s | High |

---

## 9. Exit Criteria

Phase C testing complete when:

- [ ] All 3 stream aggregates generated and tested
- [ ] Aligned view JOIN tests pass
- [ ] NULL handling tests verify ADR-FE001-004
- [ ] State transition tests pass
- [ ] Objectives storage tests pass
- [ ] Integration tests pass
- [ ] Aligned view query < 100ms verified
- [ ] `outdoor-air-quality` NOT in Gold (reserved for Phase D)

---

## References

- [PHASE-C-OVERVIEW.md](../specification/PHASE-C-OVERVIEW.md) - Phase C specification
- [ADR-FE001-004](../../architecture/ADR-FE001-004-null-handling.md) - NULL handling
- [TESTING-STRATEGY.md](../../TESTING-STRATEGY.md) - Overall testing strategy

---

*Phase C Test Plan created: 2026-02-04*
