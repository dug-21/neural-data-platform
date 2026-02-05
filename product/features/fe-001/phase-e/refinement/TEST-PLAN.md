# Phase E: Unified Event Abstraction - Test Plan

> **Phase:** E (Unified Event Abstraction)
> **Target:** Week 6
> **Testing Approach:** London TDD (Outside-In)
> **Parent Document:** [TESTING-STRATEGY.md](../../TESTING-STRATEGY.md)

---

## Overview

Phase E completes V1.1 by implementing the Unified Event Abstraction. Tests validate threshold crossing detection, unified event view generation, and V1.2 handoff requirements.

**Key Deliverable**: `gold.events_unified` - the interface contract for V1.2 Pattern Detection Engine.

---

## Phase E Scope

| ID | Feature | Testing Priority |
|----|---------|------------------|
| **v11-012** | Threshold Crossing Generator | Critical |
| **v11-013** | Unified Events View | Critical |
| **v11-014** | Gold Layer Dashboard | High |
| **v11-V02** | New Feature Type Test | Medium |

---

## 1. Test Development Order (Outside-In)

```
1. ACCEPTANCE TESTS (define V1.2 interface)
   ├── Unified events view schema contract
   ├── Threshold crossing detection
   └── V1.2 query patterns work

2. COMPONENT TESTS (verify behavior)
   ├── Threshold crossing SQL generation
   ├── Event type UNION
   └── Hourly aggregation

3. UNIT TESTS (implement details)
   ├── Rising/falling direction detection
   ├── Condition type mapping
   └── JSONB details structure
```

---

## 2. v11-012: Threshold Crossing Generator Tests

### 2.1 Acceptance Tests

```rust
/// ACCEPTANCE: Threshold crossings generated from objectives
#[test]
fn acceptance_threshold_crossings_from_objectives() {
    // Given: Objectives config with healthy_co2 threshold
    let domain = load_domain_config("config/domains/indoor-air-quality/domain.yaml");
    let objective = domain.objectives.iter()
        .find(|o| o.id == "healthy_co2")
        .expect("healthy_co2 objective should exist");

    // When: Generate threshold crossing view
    let sql = generate_threshold_crossings(&domain).unwrap();

    // Then: References objective threshold
    assert!(sql.contains("800") || sql.contains(&objective.target.threshold.to_string()));
    assert!(sql.contains("co2"));
    assert!(sql.contains("'healthy_co2'") || sql.contains(&objective.id));
}

/// ACCEPTANCE: Rising and falling crossings detected
#[test]
fn acceptance_rising_falling_detected() {
    let domain = create_test_domain_with_objective();

    let sql = generate_threshold_crossings(&domain).unwrap();

    // Should have direction logic
    assert!(sql.contains("'rising'") && sql.contains("'falling'"));
    assert!(sql.contains("CASE") || sql.contains("WHEN"));
    assert!(sql.contains(">=") && sql.contains("<"));
}

/// ACCEPTANCE: All condition types supported
#[test]
fn acceptance_all_condition_types() {
    for condition in &["<", ">", "<=", ">="] {
        let domain = DomainConfig {
            objectives: vec![Objective {
                id: format!("test_{}", condition.replace("<", "lt").replace(">", "gt")),
                target: ObjectiveTarget {
                    condition: condition.to_string(),
                    threshold: 100.0,
                    ..Default::default()
                },
                ..Default::default()
            }],
            ..create_test_domain()
        };

        let result = generate_threshold_crossings(&domain);
        assert!(result.is_ok(), "Condition {} should be supported", condition);
    }
}
```

### 2.2 Component Tests

```rust
/// Component: Crossing detection uses LAG window function
#[test]
fn test_crossing_uses_lag_for_previous_value() {
    let domain = create_test_domain_with_objective();

    let sql = generate_threshold_crossings(&domain).unwrap();

    assert!(sql.contains("LAG("));
    assert!(sql.contains("OVER ("));
    assert!(sql.contains("ORDER BY"));
}

/// Component: Previous value included for analysis
#[test]
fn test_previous_value_included() {
    let domain = create_test_domain_with_objective();

    let sql = generate_threshold_crossings(&domain).unwrap();

    assert!(sql.contains("prev_value") || sql.contains("previous_value"));
}

/// Component: Only actual crossings emitted (not every reading)
#[test]
fn test_only_crossings_emitted() {
    let domain = create_test_domain_with_objective();

    let sql = generate_threshold_crossings(&domain).unwrap();

    // Should filter to only crossing events
    assert!(sql.contains("WHERE") || sql.contains("HAVING"));
    assert!(sql.contains("crossing") || sql.contains("direction"));
}

/// Component: Crossing indexed on event_time
#[test]
fn test_crossing_indexing() {
    let domain = create_test_domain_with_objective();

    let full_sql = generate_threshold_crossings_with_indexes(&domain).unwrap();

    // Should create index for V1.2 queries
    assert!(full_sql.contains("CREATE INDEX") || full_sql.contains("INDEX"));
    assert!(full_sql.contains("event_time") || full_sql.contains("observation_time"));
}
```

### 2.3 Unit Tests

```rust
/// Unit: Rising detection logic for < condition
#[test]
fn test_rising_detection_less_than() {
    // Objective: co2 < 800
    // Rising crossing: prev < 800, current >= 800
    let sql = generate_crossing_direction_case("<", 800.0, "metric_value", "prev_value");

    assert!(sql.contains("'rising'"));
    // Rising: was meeting threshold (< 800), now violating (>= 800)
    assert!(sql.contains("prev_value < 800") || sql.contains("prev < 800"));
    assert!(sql.contains("metric_value >= 800") || sql.contains("current >= 800"));
}

/// Unit: Falling detection logic for < condition
#[test]
fn test_falling_detection_less_than() {
    // Objective: co2 < 800
    // Falling crossing: prev >= 800, current < 800
    let sql = generate_crossing_direction_case("<", 800.0, "metric_value", "prev_value");

    assert!(sql.contains("'falling'"));
    // Falling: was violating (>= 800), now meeting (< 800)
    assert!(sql.contains("prev_value >= 800"));
    assert!(sql.contains("metric_value < 800"));
}

/// Unit: Direction logic for > condition
#[test]
fn test_direction_for_greater_than() {
    // Objective: temp > 20 (want temp above 20)
    // Rising: was violating (<=20), now meeting (>20)
    let sql = generate_crossing_direction_case(">", 20.0, "value", "prev");

    assert!(sql.contains("'rising'"));
    assert!(sql.contains("'falling'"));
}

/// Unit: Objective ID in crossing event
#[test]
fn test_objective_id_included() {
    let objective = create_objective("healthy_co2", "co2", "<", 800.0);

    let sql = generate_crossing_select(&objective);

    assert!(sql.contains("'healthy_co2'") || sql.contains("objective_id"));
}
```

---

## 3. v11-013: Unified Events View Tests

### 3.1 Acceptance Tests

```rust
/// ACCEPTANCE: Unified view combines state transitions + threshold crossings
#[test]
fn acceptance_unified_view_combines_event_types() {
    let domain = create_test_domain_with_transitions_and_crossings();

    let sql = generate_unified_events_view(&domain).unwrap();

    // Should UNION two event sources
    assert!(sql.contains("UNION ALL") || sql.contains("UNION"));
    assert!(sql.contains("state_transition") || sql.contains("transitions"));
    assert!(sql.contains("threshold_crossing") || sql.contains("crossings"));
}

/// ACCEPTANCE: Consistent event schema
#[test]
fn acceptance_consistent_event_schema() {
    let domain = create_test_domain_with_transitions_and_crossings();

    let sql = generate_unified_events_view(&domain).unwrap();

    // Both sides of UNION should have same columns
    assert!(sql.contains("event_time"));
    assert!(sql.contains("stream_id"));
    assert!(sql.contains("entity_id"));
    assert!(sql.contains("event_type"));
    assert!(sql.contains("details"));
}

/// ACCEPTANCE: Details is JSONB with type-specific content
#[test]
fn acceptance_details_jsonb() {
    let domain = create_test_domain_with_transitions_and_crossings();

    let sql = generate_unified_events_view(&domain).unwrap();

    // State transition details
    assert!(sql.contains("from_state") || sql.contains("'from_state'"));
    assert!(sql.contains("to_state") || sql.contains("'to_state'"));

    // Threshold crossing details
    assert!(sql.contains("threshold") || sql.contains("'threshold'"));
    assert!(sql.contains("direction") || sql.contains("'direction'"));

    // JSONB builder
    assert!(sql.contains("jsonb_build_object") || sql.contains("json_build_object"));
}
```

### 3.2 V1.2 Contract Tests

```rust
/// V1.2 CONTRACT: Query events by time range
#[test]
fn test_v12_query_pattern_time_range() {
    // V1.2 will query:
    // SELECT * FROM gold.events_unified
    // WHERE event_time BETWEEN :start AND :end

    // View must support this pattern
    let sql = generate_unified_events_view(&create_test_domain()).unwrap();

    // Must have event_time column accessible
    assert!(sql.contains("event_time") || sql.contains("AS event_time"));
}

/// V1.2 CONTRACT: Query events by type
#[test]
fn test_v12_query_pattern_by_type() {
    // V1.2 will query:
    // SELECT * FROM gold.events_unified
    // WHERE event_type = 'threshold_crossing'

    let sql = generate_unified_events_view(&create_test_domain()).unwrap();

    // event_type must be string column
    assert!(sql.contains("event_type") || sql.contains("AS event_type"));
    assert!(sql.contains("'state_transition'") || sql.contains("'threshold_crossing'"));
}

/// V1.2 CONTRACT: Query by objective_id
#[test]
fn test_v12_query_pattern_by_objective() {
    // V1.2 will query:
    // SELECT * FROM gold.events_unified
    // WHERE event_type = 'threshold_crossing'
    //   AND details->>'objective_id' = 'healthy_co2'

    let sql = generate_unified_events_view(&create_test_domain()).unwrap();

    // details must be JSONB with objective_id for crossings
    assert!(sql.contains("objective_id"));
}

/// V1.2 CONTRACT: Schema matches specification
#[test]
fn test_v12_schema_contract() {
    let domain = create_test_domain_with_transitions_and_crossings();

    let columns = extract_select_columns(&generate_unified_events_view(&domain).unwrap());

    // Required columns per contract
    let required = ["event_time", "stream_id", "entity_id", "event_type", "details"];

    for col in required {
        assert!(
            columns.iter().any(|c| c.contains(col)),
            "Missing required column: {}", col
        );
    }
}
```

### 3.3 Component Tests

```rust
/// Component: Hourly event aggregate generated
#[test]
fn test_hourly_events_aggregate() {
    let domain = create_test_domain_with_transitions_and_crossings();

    let sql = generate_events_hourly_aggregate(&domain).unwrap();

    // Should be continuous aggregate
    assert!(sql.contains("CREATE MATERIALIZED VIEW gold.events_hourly"));
    assert!(sql.contains("timescaledb.continuous") || sql.contains("WITH ("));

    // Should count events by type
    assert!(sql.contains("COUNT(*)") || sql.contains("SUM("));
    assert!(sql.contains("GROUP BY"));
    assert!(sql.contains("time_bucket"));
}

/// Component: Event counts per type
#[test]
fn test_event_counts_by_type() {
    let domain = create_test_domain_with_transitions_and_crossings();

    let sql = generate_events_hourly_aggregate(&domain).unwrap();

    // Should have counts by event type
    assert!(sql.contains("state_transition_count") ||
            sql.contains("COUNT(*) FILTER (WHERE event_type = 'state_transition')"));
    assert!(sql.contains("threshold_crossing_count") ||
            sql.contains("COUNT(*) FILTER (WHERE event_type = 'threshold_crossing')"));
}

/// Component: Events hourly joins with aligned view
#[test]
fn test_events_join_aligned_view() {
    // V1.2 will join events with aligned view on bucket
    // Ensure bucket column exists and is compatible

    let events_sql = generate_events_hourly_aggregate(&create_test_domain()).unwrap();

    assert!(sql.contains("bucket") || sql.contains("time_bucket"));
}
```

### 3.4 Unit Tests

```rust
/// Unit: State transition JSONB details
#[test]
fn test_state_transition_details_jsonb() {
    let sql = generate_state_transition_details();

    assert!(sql.contains("jsonb_build_object("));
    assert!(sql.contains("'from_state'"));
    assert!(sql.contains("'to_state'"));
    assert!(sql.contains("'duration_in_previous_ms'"));
}

/// Unit: Threshold crossing JSONB details
#[test]
fn test_threshold_crossing_details_jsonb() {
    let objective = create_objective("healthy_co2", "co2", "<", 800.0);

    let sql = generate_crossing_details(&objective);

    assert!(sql.contains("jsonb_build_object("));
    assert!(sql.contains("'metric'"));
    assert!(sql.contains("'threshold'"));
    assert!(sql.contains("'direction'"));
    assert!(sql.contains("'value'"));
    assert!(sql.contains("'objective_id'"));
    assert!(sql.contains("'condition'"));
}

/// Unit: Event type cast to text
#[test]
fn test_event_type_is_text() {
    let sql = generate_unified_events_view(&create_test_domain()).unwrap();

    // Event type should be text (not enum for flexibility)
    assert!(sql.contains("'state_transition'::text") ||
            sql.contains("AS event_type"));
}
```

---

## 4. Integration Tests

```rust
/// INTEGRATION: Full Phase E deployment
#[tokio::test]
#[ignore]
async fn integration_phase_e_deployment() {
    // Arrange: Phase D complete
    assert!(check_continuous_aggregate_exists("gold", "air_quality_hourly").await);

    // Act: Deploy Phase E
    deploy_manifest(".deploy/test/phase-e-events.manifest.json").await.unwrap();

    // Assert: Threshold crossings view exists
    assert!(check_view_exists("gold", "threshold_crossings").await);

    // Assert: Unified events view exists
    assert!(check_view_exists("gold", "events_unified").await);

    // Assert: Events hourly aggregate exists
    assert!(check_continuous_aggregate_exists("gold", "events_hourly").await);
}

/// INTEGRATION: V1.2 query patterns work
#[tokio::test]
#[ignore]
async fn integration_v12_queries_work() {
    // Setup test data
    setup_threshold_crossing_test_data().await;

    // Pattern 1: Time range query
    let events = query_sql(
        "SELECT * FROM gold.events_unified WHERE event_time >= NOW() - INTERVAL '24 hours'"
    ).await;
    assert!(!events.is_empty());

    // Pattern 2: Query by type
    let crossings = query_sql(
        "SELECT * FROM gold.events_unified WHERE event_type = 'threshold_crossing'"
    ).await;
    // May be empty if no crossings occurred

    // Pattern 3: Join with aligned view
    let joined = query_sql(
        "SELECT a.bucket, e.event_type, e.details
         FROM gold.indoor_air_quality_aligned a
         LEFT JOIN gold.events_hourly e ON a.bucket = e.bucket
         WHERE a.bucket >= NOW() - INTERVAL '24 hours'"
    ).await;
    assert!(!joined.is_empty());
}

/// INTEGRATION: Performance requirements met
#[tokio::test]
#[ignore]
async fn integration_events_query_performance() {
    let start = std::time::Instant::now();

    // 30-day query
    let _events = query_sql(
        "SELECT * FROM gold.events_unified WHERE event_time >= NOW() - INTERVAL '30 days'"
    ).await;

    let duration = start.elapsed();
    assert!(
        duration.as_millis() < 100,
        "Events query took {}ms, expected < 100ms", duration.as_millis()
    );
}
```

---

## 5. v11-014: Gold Layer Dashboard Tests

> **Note**: Dashboard JSON will be created during implementation. These tests define validation requirements.

### 5.1 Dashboard JSON Validation Tests

```rust
/// VALIDATION: Dashboard JSON is valid Grafana format
#[test]
fn test_dashboard_json_valid_format() {
    let dashboard_path = "config/dashboards/gold-layer-dashboard.json";
    let json: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dashboard_path).unwrap()
    ).expect("Dashboard JSON should be valid");

    // Required Grafana fields
    assert!(json.get("title").is_some(), "Dashboard must have title");
    assert!(json.get("panels").is_some(), "Dashboard must have panels");
    assert!(json.get("templating").is_some(), "Dashboard must have templating");
    assert!(json.get("time").is_some(), "Dashboard must have time range");
}

/// VALIDATION: Dashboard references Gold layer data sources
#[test]
fn test_dashboard_uses_gold_layer_sources() {
    let dashboard = load_dashboard_json("gold-layer-dashboard.json");
    let panels = dashboard["panels"].as_array().unwrap();

    for panel in panels {
        if let Some(targets) = panel.get("targets") {
            for target in targets.as_array().unwrap_or(&vec![]) {
                if let Some(raw_sql) = target.get("rawSql") {
                    let sql = raw_sql.as_str().unwrap_or("");
                    // Should query Gold schema
                    assert!(
                        sql.contains("gold.") || sql.is_empty(),
                        "Panel queries should use gold schema: {}", sql
                    );
                }
            }
        }
    }
}

/// VALIDATION: Dashboard includes all required panels
#[test]
fn test_dashboard_required_panels() {
    let dashboard = load_dashboard_json("gold-layer-dashboard.json");
    let panel_titles: Vec<&str> = dashboard["panels"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|p| p["title"].as_str())
        .collect();

    // AC-E-09: Dashboard displays all Gold continuous aggregates
    let required_aggregates = [
        "Hourly Air Quality",     // gold.air_quality_hourly
        "Daily Air Quality",      // gold.air_quality_daily
        "Window Transitions",     // gold.window_transitions_hourly
    ];

    for required in required_aggregates {
        assert!(
            panel_titles.iter().any(|t| t.contains(required) || t.to_lowercase().contains(&required.to_lowercase())),
            "Missing required panel for: {}", required
        );
    }

    // AC-E-10: Dashboard displays aligned view
    assert!(
        panel_titles.iter().any(|t| t.contains("Aligned") || t.contains("Correlation")),
        "Missing aligned view panel"
    );

    // AC-E-11: Dashboard displays unified events
    assert!(
        panel_titles.iter().any(|t| t.contains("Events") || t.contains("Crossings")),
        "Missing unified events panel"
    );
}

/// VALIDATION: Dashboard has objective threshold annotations
#[test]
fn test_dashboard_threshold_annotations() {
    let dashboard = load_dashboard_json("gold-layer-dashboard.json");
    let panels = dashboard["panels"].as_array().unwrap();

    // AC-E-12: Objective thresholds visible as annotations/lines
    let has_thresholds = panels.iter().any(|panel| {
        // Check for threshold lines in field config
        if let Some(field_config) = panel.get("fieldConfig") {
            if let Some(defaults) = field_config.get("defaults") {
                if let Some(thresholds) = defaults.get("thresholds") {
                    return thresholds.get("steps").map_or(false, |s| !s.as_array().unwrap_or(&vec![]).is_empty());
                }
            }
        }
        // Check for annotations
        panel.get("options")
            .and_then(|o| o.get("annotations"))
            .is_some()
    });

    assert!(has_thresholds, "Dashboard should have threshold annotations for objectives");
}
```

### 5.2 Dashboard Variables Tests

```rust
/// Variables: Time range selector works
#[test]
fn test_dashboard_time_range_variable() {
    let dashboard = load_dashboard_json("gold-layer-dashboard.json");
    let templating = dashboard["templating"]["list"].as_array().unwrap_or(&vec![]);

    // Should have time bucket variable or use Grafana time range
    let time_config = dashboard.get("time").expect("Time config required");
    assert!(time_config.get("from").is_some(), "Time range 'from' required");
    assert!(time_config.get("to").is_some(), "Time range 'to' required");
}

/// Variables: Entity selector populates from data
#[test]
fn test_dashboard_entity_variable() {
    let dashboard = load_dashboard_json("gold-layer-dashboard.json");
    let templating = dashboard["templating"]["list"].as_array().unwrap_or(&vec![]);

    // Look for entity/sensor/ndp_id variable
    let entity_var = templating.iter().find(|v| {
        let name = v["name"].as_str().unwrap_or("");
        name.contains("entity") || name.contains("sensor") || name.contains("ndp_id")
    });

    if let Some(var) = entity_var {
        // Should query from data source
        assert!(
            var.get("query").is_some() || var.get("definition").is_some(),
            "Entity variable should be data-driven"
        );
    }
}

/// Variables: Event type filter available
#[test]
fn test_dashboard_event_type_variable() {
    let dashboard = load_dashboard_json("gold-layer-dashboard.json");
    let templating = dashboard["templating"]["list"].as_array().unwrap_or(&vec![]);

    // Look for event_type variable
    let event_var = templating.iter().find(|v| {
        let name = v["name"].as_str().unwrap_or("");
        name.contains("event") || name.contains("type")
    });

    // Event type filter is recommended but not required
    // Just log if missing
    if event_var.is_none() {
        println!("INFO: No event_type variable found - consider adding for filtering");
    }
}
```

### 5.3 Dashboard Performance Tests

```rust
/// PERFORMANCE: Dashboard loads within 3 seconds
#[tokio::test]
#[ignore]
async fn test_dashboard_load_time() {
    let start = std::time::Instant::now();

    // Simulate dashboard load (all queries in parallel)
    let queries = get_dashboard_queries("gold-layer-dashboard.json");
    let mut handles = vec![];

    for query in queries {
        handles.push(tokio::spawn(async move {
            execute_query(&query).await
        }));
    }

    futures::future::join_all(handles).await;
    let duration = start.elapsed();

    // AC: Dashboard loads < 3s
    assert!(
        duration.as_secs() < 3,
        "Dashboard load took {}s, expected < 3s", duration.as_secs_f32()
    );
}

/// PERFORMANCE: 30-day queries complete within 5 seconds
#[tokio::test]
#[ignore]
async fn test_dashboard_30_day_performance() {
    // Most expensive query: 30-day unified events
    let start = std::time::Instant::now();

    let _result = execute_query(
        "SELECT * FROM gold.events
         WHERE event_time >= NOW() - INTERVAL '30 days'
         ORDER BY event_time DESC
         LIMIT 1000"
    ).await;

    let duration = start.elapsed();

    // 30-day queries should complete < 5s
    assert!(
        duration.as_secs() < 5,
        "30-day query took {}s, expected < 5s", duration.as_secs_f32()
    );
}

/// PERFORMANCE: Aligned view query is responsive
#[tokio::test]
#[ignore]
async fn test_aligned_view_query_performance() {
    let start = std::time::Instant::now();

    let _result = execute_query(
        "SELECT * FROM gold.indoor_air_quality_aligned
         WHERE bucket >= NOW() - INTERVAL '7 days'
         ORDER BY bucket DESC"
    ).await;

    let duration = start.elapsed();

    assert!(
        duration.as_millis() < 500,
        "Aligned view query took {}ms, expected < 500ms", duration.as_millis()
    );
}
```

### 5.4 Dashboard Integration Tests

```rust
/// INTEGRATION: Dashboard deploys to Grafana
#[tokio::test]
#[ignore]
async fn integration_dashboard_deploy() {
    // Deploy dashboard via API
    let result = deploy_dashboard("gold-layer-dashboard.json").await;
    assert!(result.is_ok(), "Dashboard deployment failed: {:?}", result.err());

    // Verify dashboard exists
    let dashboards = list_grafana_dashboards().await.unwrap();
    assert!(
        dashboards.iter().any(|d| d.title.contains("Gold")),
        "Gold Layer Dashboard not found in Grafana"
    );
}

/// INTEGRATION: Dashboard queries return data
#[tokio::test]
#[ignore]
async fn integration_dashboard_has_data() {
    // Setup: Ensure test data exists
    setup_gold_layer_test_data().await;

    // Each panel should return data (not empty)
    let queries = get_dashboard_queries("gold-layer-dashboard.json");

    for (panel_name, query) in queries {
        let result = execute_query(&query).await;
        // Allow empty results for event queries (may not have crossings)
        if !panel_name.contains("Event") && !panel_name.contains("Crossing") {
            assert!(
                !result.is_empty(),
                "Panel '{}' returned no data", panel_name
            );
        }
    }
}

/// INTEGRATION: Dashboard uses correct data source
#[tokio::test]
#[ignore]
async fn integration_dashboard_data_source() {
    let dashboard = load_dashboard_json("gold-layer-dashboard.json");

    // All panels should use TimescaleDB data source
    let panels = dashboard["panels"].as_array().unwrap();
    for panel in panels {
        if let Some(datasource) = panel.get("datasource") {
            let ds_type = datasource.get("type").and_then(|t| t.as_str()).unwrap_or("");
            assert!(
                ds_type == "postgres" || ds_type == "grafana-postgresql-datasource" || ds_type.is_empty(),
                "Panel should use PostgreSQL datasource, got: {}", ds_type
            );
        }
    }
}
```

### 5.5 Acceptance Criteria Mapping

| AC ID | Criterion | Test |
|-------|-----------|------|
| AC-E-09 | Dashboard displays all Gold CAs | `test_dashboard_required_panels` |
| AC-E-10 | Dashboard displays aligned view | `test_dashboard_required_panels` |
| AC-E-11 | Dashboard displays unified events | `test_dashboard_required_panels` |
| AC-E-12 | Objective thresholds as annotations | `test_dashboard_threshold_annotations` |
| AC-E-13 | Dashboard loads < 3s | `test_dashboard_load_time` |
| AC-E-14 | 30-day queries < 5s | `test_dashboard_30_day_performance` |

---

## 6. Test Execution Commands

```bash
# Run Phase E unit tests
cargo test -p ndp-gold-ddl --lib threshold_crossing
cargo test -p ndp-gold-ddl --lib unified_events
cargo test -p ndp-gold-ddl --lib events

# Run V1.2 contract tests
cargo test -p ndp-gold-ddl --test v12_contract

# Run dashboard validation tests
cargo test -p ndp-gold-ddl --lib dashboard_validation
cargo test -p ndp-gold-ddl --test dashboard -- --ignored

# Run Phase E integration tests
DEPLOY_ENV=integration cargo test -p ndp-gold-ddl --test integration -- phase_e --ignored

# Run dashboard performance tests (requires running Grafana)
GRAFANA_URL=http://localhost:3000 cargo test -p ndp-gold-ddl --test dashboard_perf -- --ignored

# Run all Phase E tests
./scripts/test-phase-e.sh
```

---

## 7. Test Metrics (Phase E Target)

| Category | Target | Priority |
|----------|--------|----------|
| Threshold Crossing Tests | 12-15 | Critical |
| Unified Events Tests | 10-12 | Critical |
| V1.2 Contract Tests | 5-8 | Critical |
| Dashboard Validation Tests | 8-10 | High |
| Dashboard Performance Tests | 3-4 | High |
| Integration Tests | 5-8 | Critical |
| Coverage (events.rs) | 85% | High |
| Events Query Performance | < 100ms | Critical |
| Dashboard Load Time | < 3s | High |

---

## 8. Exit Criteria

Phase E testing complete when:

- [ ] Threshold crossing generation tests pass
- [ ] Unified events view tests pass
- [ ] V1.2 contract tests pass
- [ ] All condition types supported
- [ ] JSONB details structure validated
- [ ] Events hourly aggregate tests pass
- [ ] Dashboard JSON validation tests pass
- [ ] Dashboard displays all Gold CAs (AC-E-09)
- [ ] Dashboard displays aligned view (AC-E-10)
- [ ] Dashboard displays unified events (AC-E-11)
- [ ] Dashboard threshold annotations present (AC-E-12)
- [ ] Dashboard loads < 3s (AC-E-13)
- [ ] 30-day queries < 5s (AC-E-14)
- [ ] Integration tests pass
- [ ] Performance requirements met
- [ ] V1.2 handoff checklist complete

---

## References

- [PHASE-E-OVERVIEW.md](../specification/PHASE-E-OVERVIEW.md) - Phase E specification
- [V12-HANDOFF-CHECKLIST.md](./V12-HANDOFF-CHECKLIST.md) - V1.2 handoff checklist
- [TESTING-STRATEGY.md](../../TESTING-STRATEGY.md) - Overall testing strategy

---

*Phase E Test Plan created: 2026-02-04*
*Updated: 2026-02-05 - Added v11-014 Dashboard tests (Section 5)*
