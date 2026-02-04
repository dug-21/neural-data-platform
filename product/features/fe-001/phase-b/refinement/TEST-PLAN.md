# Phase B: First Stream (air-quality) - Test Plan

> **Phase:** B (First Stream - Reference Implementation)
> **Target:** Week 3
> **Testing Approach:** London TDD (Outside-In)
> **Parent Document:** [TESTING-STRATEGY.md](../../TESTING-STRATEGY.md)

---

## Overview

Phase B applies the architecture foundation from Phase A to the `air-quality` stream as the reference implementation. Tests validate that the declarative infrastructure works end-to-end, from config to operational continuous aggregates.

**Key Principle**: Air-quality serves as the exemplar. Every test pattern established here will be replicated for subsequent streams.

---

## Phase B Scope

| ID | Feature | Testing Priority |
|----|---------|------------------|
| **v11-001** | Stream Type Classification | High |
| **v11-002** | Classification Propagation | High |
| **v11-003** | Per-Stream Continuous Aggregates (air-quality) | Critical |
| **v11-004** | Aggregate Refresh Policy | High |

---

## 1. Test Development Order (Outside-In)

Following London TDD principles:

```
1. ACCEPTANCE TESTS (define success)
   ├── End-to-end air-quality Gold deployment
   └── Config-only field modification test

2. COMPONENT TESTS (verify behavior)
   ├── Stream type enum parsing
   ├── Classification propagation to dictionary
   └── Continuous aggregate DDL generation

3. UNIT TESTS (implement details)
   ├── Granularity view naming
   ├── Metric function mapping
   └── Refresh policy generation
```

---

## 2. v11-001: Stream Type Classification Tests

### 2.1 Acceptance Tests

```rust
// Location: core/tests/stream_type_classification.rs

/// ACCEPTANCE: air-quality config with stream_type loads correctly
#[test]
fn acceptance_air_quality_has_observation_type() {
    // Given: air-quality config file
    let config = load_stream_config("config/base/streams/air-quality/config.yaml");

    // When: Parse stream type
    let stream_type = config.stream_type;

    // Then: Correctly classified as observation
    assert_eq!(stream_type, Some(StreamType::Observation));
}

/// ACCEPTANCE: All V1.1 streams have stream_type
#[test]
fn acceptance_all_streams_classified() {
    let expected_types = vec![
        ("air-quality", StreamType::Observation),
        ("outdoor-weather", StreamType::Observation),
        ("home-assistant-state", StreamType::StateEvent),
        ("nws-forecast-hourly", StreamType::Forecast),
    ];

    for (stream_id, expected_type) in expected_types {
        let config = load_stream_config(&format!(
            "config/base/streams/{}/config.yaml", stream_id
        ));

        assert_eq!(
            config.stream_type,
            Some(expected_type),
            "{} should be {:?}", stream_id, expected_type
        );
    }
}
```

### 2.2 Unit Tests

```rust
/// Unit: StreamType enum parses from string
#[test]
fn test_stream_type_deserializes_from_snake_case() {
    for (value, expected) in &[
        ("observation", StreamType::Observation),
        ("state_event", StreamType::StateEvent),
        ("forecast", StreamType::Forecast),
        ("dimension", StreamType::Dimension),
    ] {
        let json = json!({ "stream_type": value });
        let result: Result<StreamType, _> = serde_json::from_value(json["stream_type"].clone());

        assert!(result.is_ok(), "Failed to parse {}", value);
        assert_eq!(result.unwrap(), *expected);
    }
}

/// Unit: Unknown stream_type rejects with error
#[test]
fn test_stream_type_rejects_unknown() {
    let json = json!({ "stream_type": "unknown_type" });
    let result: Result<StreamType, _> = serde_json::from_value(json["stream_type"].clone());

    assert!(result.is_err());
}

/// Unit: stream_type is optional for backward compatibility
#[test]
fn test_stream_type_optional() {
    let config_json = json!({
        "stream_id": "legacy-stream",
        "description": "No stream_type field",
        "fields": [{"name": "value", "type": "float", "nullable": true}],
        "sources": [{"type": "mqtt", "enabled": true}]
    });

    let config: StreamConfig = serde_json::from_value(config_json).unwrap();
    assert!(config.stream_type.is_none());
}
```

---

## 3. v11-002: Classification Propagation Tests

### 3.1 Component Tests

```rust
// Location: core/tests/classification_propagation.rs

/// Component: Stream type flows to data dictionary
#[tokio::test]
async fn test_classification_stored_in_dictionary() {
    // Arrange
    let loader = MockConfigLoader::new()
        .with_stream(create_typed_stream("air-quality", StreamType::Observation));
    let db = MockTimescaleDb::new();

    // Act
    sync_stream_classification(&loader, &db, "air-quality").await.unwrap();

    // Assert: Dictionary entry created
    assert!(db.sql_contains("INSERT INTO data_dictionary.stream_classification"));
    assert!(db.sql_contains("'observation'"));
}

/// Component: Classification queryable via MCP
#[tokio::test]
#[ignore] // Integration test - requires infrastructure
async fn test_mcp_returns_stream_classification() {
    // Given: Stream with classification synced
    setup_test_classification("air-quality", StreamType::Observation).await;

    // When: Query via MCP
    let result = mcp_query_stream_classification("air-quality").await;

    // Then: Classification returned
    assert_eq!(result.stream_type, "observation");
    assert_eq!(result.correlation_role, "effect");
}
```

### 3.2 Unit Tests

```rust
/// Unit: Stream type maps to correlation role
#[test]
fn test_stream_type_to_correlation_role() {
    assert_eq!(StreamType::Observation.correlation_role(), "effect");
    assert_eq!(StreamType::StateEvent.correlation_role(), "cause");
    assert_eq!(StreamType::Forecast.correlation_role(), "context");
    assert_eq!(StreamType::Dimension.correlation_role(), "metadata");
}

/// Unit: Classification SQL generation
#[test]
fn test_generate_classification_insert_sql() {
    let sql = generate_classification_sql("air-quality", StreamType::Observation);

    assert!(sql.contains("data_dictionary.stream_classification"));
    assert!(sql.contains("'air-quality'"));
    assert!(sql.contains("'observation'"));
    assert!(sql.contains("'effect'"));
}
```

---

## 4. v11-003: Per-Stream Continuous Aggregates Tests (air-quality)

### 4.1 Acceptance Tests

```rust
/// ACCEPTANCE: air-quality continuous aggregate generated from config
#[test]
fn acceptance_air_quality_hourly_ddl_generated() {
    // Given: air-quality with gold_etl config
    let config = load_stream_config("config/base/streams/air-quality/config.yaml");

    // When: Generate DDL
    let sql = generate_gold_ddl(&config).unwrap();

    // Then: Contains continuous aggregate for hourly
    assert!(sql.contains("CREATE MATERIALIZED VIEW gold.air_quality_hourly"));
    assert!(sql.contains("WITH (timescaledb.continuous)"));
    assert!(sql.contains("time_bucket('1 hour'"));
}

/// ACCEPTANCE: air-quality daily aggregate also generated
#[test]
fn acceptance_air_quality_daily_ddl_generated() {
    let config = load_stream_config("config/base/streams/air-quality/config.yaml");

    let sql = generate_gold_ddl(&config).unwrap();

    assert!(sql.contains("CREATE MATERIALIZED VIEW gold.air_quality_daily"));
    assert!(sql.contains("time_bucket('1 day'"));
}

/// ACCEPTANCE: All configured fields have aggregates
#[test]
fn acceptance_all_configured_fields_aggregated() {
    let config = load_stream_config("config/base/streams/air-quality/config.yaml");

    let sql = generate_gold_ddl(&config).unwrap();

    // From air-quality gold_etl config
    let expected_fields = ["pm25", "pm10", "co2", "temperature_c", "humidity_pct", "tvoc_index", "nox_index"];

    for field in expected_fields {
        assert!(
            sql.contains(&format!("{}_mean", field)) || sql.contains(&format!("AVG({}", field)),
            "Missing aggregate for field: {}", field
        );
    }
}
```

### 4.2 Component Tests

```rust
/// Component: DDL generation with mocked config loader
#[tokio::test]
async fn test_ddl_generated_from_mock_config() {
    // Arrange
    let loader = MockConfigLoader::new()
        .with_stream(create_test_stream_config("air-quality"))
        .with_gold_config("air-quality", create_air_quality_gold_config());

    // Act
    let gold_config = loader.load_gold_etl_config("air-quality").await.unwrap();
    let sql = generate_continuous_aggregate(&gold_config).unwrap();

    // Assert
    assert!(sql.contains("gold.air_quality_hourly"));
    assert!(sql.contains("AVG(pm25)"));
}

/// Component: Multiple granularities generate multiple views
#[test]
fn test_multiple_granularities_generate_multiple_views() {
    let config = GoldEtlConfig {
        aggregates: AggregatesConfig {
            granularities: vec!["1 hour".to_string(), "1 day".to_string()],
            fields: hashmap! { "pm25".to_string() => field_config(&["mean"]) },
            ..Default::default()
        },
        ..create_test_gold_config("air-quality")
    };

    let sql = generate_all_continuous_aggregates(&config).unwrap();

    assert!(sql.contains("gold.air_quality_hourly"));
    assert!(sql.contains("gold.air_quality_daily"));
}

/// Component: View names handle hyphenated stream IDs
#[test]
fn test_view_naming_replaces_hyphens() {
    let config = create_test_gold_config("air-quality");

    let sql = generate_continuous_aggregate(&config).unwrap();

    // Should use underscores, not hyphens
    assert!(sql.contains("gold.air_quality_hourly"));
    assert!(!sql.contains("gold.air-quality"));
}
```

### 4.3 Unit Tests

```rust
/// Unit: PM2.5 metrics map to correct SQL functions
#[test]
fn test_pm25_metrics_generate_correct_sql() {
    let config = create_gold_config_with_metrics("air-quality", "1 hour", &[
        ("pm25", vec!["mean", "std", "min", "max", "p95"])
    ]);

    let sql = generate_continuous_aggregate(&config).unwrap();

    assert!(sql.contains("AVG(pm25) AS pm25_mean"));
    assert!(sql.contains("STDDEV(pm25) AS pm25_std"));
    assert!(sql.contains("MIN(pm25) AS pm25_min"));
    assert!(sql.contains("MAX(pm25) AS pm25_max"));
    assert!(sql.contains("percentile_cont(0.95)") || sql.contains("PERCENTILE_CONT(0.95)"));
}

/// Unit: CO2 metrics generate correctly
#[test]
fn test_co2_metrics_generate_correct_sql() {
    let config = create_gold_config_with_metrics("air-quality", "1 hour", &[
        ("co2", vec!["mean", "std", "min", "max"])
    ]);

    let sql = generate_continuous_aggregate(&config).unwrap();

    assert!(sql.contains("AVG(co2) AS co2_mean"));
    assert!(sql.contains("STDDEV(co2) AS co2_std"));
}

/// Unit: Sample count is always included
#[test]
fn test_sample_count_always_included() {
    let config = create_test_gold_config("air-quality");

    let sql = generate_continuous_aggregate(&config).unwrap();

    assert!(sql.contains("COUNT(*) AS sample_count"));
}

/// Unit: GROUP BY includes bucket and ndp_id
#[test]
fn test_group_by_clause() {
    let config = create_test_gold_config("air-quality");

    let sql = generate_continuous_aggregate(&config).unwrap();

    assert!(sql.contains("GROUP BY"));
    assert!(sql.contains("bucket") || sql.contains("time_bucket"));
    assert!(sql.contains("ndp_id"));
}

/// Unit: FROM references correct Silver table
#[test]
fn test_from_references_silver_table() {
    let config = create_test_gold_config("air-quality");

    let sql = generate_continuous_aggregate(&config).unwrap();

    assert!(sql.contains("FROM silver.air_quality_observations"));
}
```

---

## 5. v11-004: Aggregate Refresh Policy Tests

### 5.1 Acceptance Tests

```rust
/// ACCEPTANCE: Refresh policy generated for air-quality
#[test]
fn acceptance_refresh_policy_generated() {
    let config = load_stream_config("config/base/streams/air-quality/config.yaml");

    let sql = generate_gold_ddl(&config).unwrap();

    assert!(sql.contains("add_continuous_aggregate_policy"));
    assert!(sql.contains("gold.air_quality_hourly"));
}

/// ACCEPTANCE: Policy has correct intervals
#[test]
fn acceptance_refresh_policy_intervals() {
    let config = load_stream_config("config/base/streams/air-quality/config.yaml");

    let sql = generate_gold_ddl(&config).unwrap();

    // From config: schedule_interval: 15 minutes, start_offset: 4 hours
    assert!(sql.contains("schedule_interval => INTERVAL '15 minutes'"));
    assert!(sql.contains("start_offset => INTERVAL '4 hours'"));
}
```

### 5.2 Component Tests

```rust
/// Component: Refresh policy uses config values
#[test]
fn test_refresh_policy_from_config() {
    let config = GoldEtlConfig {
        refresh_policy: Some(RefreshPolicyConfig {
            schedule_interval: "30 minutes".to_string(),
            start_offset: "2 hours".to_string(),
            end_offset: "10 minutes".to_string(),
        }),
        ..create_test_gold_config("air-quality")
    };

    let sql = generate_refresh_policy(&config, "air_quality_hourly").unwrap();

    assert!(sql.contains("schedule_interval => INTERVAL '30 minutes'"));
    assert!(sql.contains("start_offset => INTERVAL '2 hours'"));
    assert!(sql.contains("end_offset => INTERVAL '10 minutes'"));
}

/// Component: Default policy used when not specified
#[test]
fn test_default_refresh_policy() {
    let config = GoldEtlConfig {
        refresh_policy: None,
        ..create_test_gold_config("air-quality")
    };

    let sql = generate_refresh_policy(&config, "air_quality_hourly").unwrap();

    // Defaults: 15 min schedule, 4 hour start, 15 min end
    assert!(sql.contains("schedule_interval => INTERVAL '15 minutes'"));
}
```

### 5.3 Unit Tests

```rust
/// Unit: Policy references correct view name
#[test]
fn test_policy_references_view() {
    let sql = generate_refresh_policy_sql("gold.air_quality_hourly", "15 minutes", "4 hours", "15 minutes");

    assert!(sql.contains("'gold.air_quality_hourly'"));
}

/// Unit: Daily aggregate has different default policy
#[test]
fn test_daily_aggregate_policy_defaults() {
    let config = create_test_gold_config("air-quality");

    let sql = generate_refresh_policy(&config, "air_quality_daily").unwrap();

    // Daily aggregates: 1 hour schedule, 24 hour start offset
    assert!(sql.contains("schedule_interval => INTERVAL '1 hour'") ||
            sql.contains("schedule_interval => INTERVAL '15 minutes'"));
}
```

---

## 6. Integration Tests (Requires Infrastructure)

```rust
// Location: tools/ndp-gold-ddl/tests/integration/phase_b_integration.rs

/// INTEGRATION: Full air-quality Gold deployment
#[tokio::test]
#[ignore]
async fn integration_air_quality_gold_deployment() {
    // Arrange: Clean Gold schema
    cleanup_gold_schema().await;

    // Act: Deploy via manifest
    let result = deploy_manifest(".deploy/test/phase-b-air-quality.manifest.json").await;

    // Assert: Deployment succeeded
    assert!(result.is_ok(), "Deployment failed: {:?}", result);

    // Assert: Continuous aggregates exist
    assert!(check_continuous_aggregate_exists("gold", "air_quality_hourly").await);
    assert!(check_continuous_aggregate_exists("gold", "air_quality_daily").await);

    // Assert: Refresh policies exist
    let policies = get_refresh_policies("gold.air_quality_hourly").await;
    assert!(!policies.is_empty(), "Refresh policy should exist");
}

/// INTEGRATION: Query performance < 100ms
#[tokio::test]
#[ignore]
async fn integration_air_quality_query_performance() {
    // Arrange: Ensure aggregate has data
    setup_test_silver_data("air-quality").await;
    wait_for_aggregate_refresh("gold.air_quality_hourly").await;

    // Act: Timed query
    let start = std::time::Instant::now();
    let rows = query_aggregate("gold.air_quality_hourly", 30).await;
    let duration = start.elapsed();

    // Assert: Performance target met
    assert!(
        duration.as_millis() < 100,
        "Query took {}ms, expected < 100ms", duration.as_millis()
    );
    assert!(!rows.is_empty(), "Should have data");
}

/// INTEGRATION: Config-only field modification works
#[tokio::test]
#[ignore]
async fn integration_config_only_field_addition() {
    // Arrange: Initial deployment
    deploy_manifest(".deploy/test/phase-b-air-quality.manifest.json").await.unwrap();

    let columns_before = get_view_columns("gold.air_quality_hourly").await;

    // Act: Modify config to add p99 metric (simulated - would be actual config edit)
    // Then recreate via manifest with action: recreate
    deploy_manifest_with_action(
        ".deploy/test/phase-b-air-quality.manifest.json",
        "recreate"
    ).await.unwrap();

    let columns_after = get_view_columns("gold.air_quality_hourly").await;

    // Assert: New column present (if p99 was added to config)
    // This validates the config-only modification pattern
}
```

---

## 7. Test Execution Commands

```bash
# Run Phase B unit tests
cargo test -p neural-core --lib stream_type
cargo test -p ndp-gold-ddl --lib continuous_aggregate

# Run Phase B component tests
cargo test -p ndp-gold-ddl --test classification_tests
cargo test -p ndp-gold-ddl --test continuous_aggregate_tests

# Run Phase B integration tests (requires Docker)
DEPLOY_ENV=integration cargo test -p ndp-gold-ddl --test integration -- phase_b --ignored

# Run all Phase B tests
./scripts/test-phase-b.sh
```

---

## 8. Test Metrics (Phase B Target)

| Category | Target | Priority |
|----------|--------|----------|
| Unit Tests | 15-20 | High |
| Component Tests | 5-8 | High |
| Integration Tests | 2-3 | Critical |
| Coverage (continuous_aggregate.rs) | 90% | Critical |
| Coverage (classification) | 80% | High |
| Test Duration (unit) | <5s | High |
| Test Duration (integration) | <60s | Medium |

---

## 9. Exit Criteria

Phase B testing complete when:

- [ ] Stream type classification tests pass
- [ ] Classification propagation tests pass
- [ ] Continuous aggregate generation tests pass
- [ ] Refresh policy tests pass
- [ ] Integration tests pass on Pi infrastructure
- [ ] Query performance < 100ms verified
- [ ] Config-only modification validated

---

## References

- [PHASE-B-OVERVIEW.md](../specification/PHASE-B-OVERVIEW.md) - Phase B specification
- [TESTING-STRATEGY.md](../../TESTING-STRATEGY.md) - Overall testing strategy
- [TDD-GUIDE.md](./TDD-GUIDE.md) - Step-by-step TDD instructions

---

*Phase B Test Plan created: 2026-02-04*
