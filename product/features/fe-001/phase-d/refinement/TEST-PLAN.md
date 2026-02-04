# Phase D: Validation + Dashboard - Test Plan

> **Phase:** D (Validation + Dashboard)
> **Target:** Week 5
> **Testing Approach:** London TDD + Fast-Follower Validation
> **Parent Document:** [TESTING-STRATEGY.md](../../TESTING-STRATEGY.md)

---

## Overview

Phase D is the **critical validation phase** that proves the V1.1 architecture works as designed. The centerpiece is the **Fast-Follower Test**: adding `outdoor-air-quality` to the Gold layer using **config changes only** - zero Rust code changes.

**Critical Success Metric**: Fast-follower test completes in < 1 hour with zero code changes.

---

## Phase D Scope

| ID | Feature | Testing Priority |
|----|---------|------------------|
| **v11-V01** | Fast-Follower Stream Test | **Critical** |
| **v11-V02** | New Feature Type Test | Medium |
| **v11-008** | Basic Feature Computation | Medium |
| **v11-009** | Lag Feature Computation | Medium |
| **v11-010** | Gold Layer Data Dictionary | Medium |
| **v11-011** | Correlation-Ready Dashboard | High |

---

## 1. Fast-Follower Test Strategy

### 1.1 Pre-Conditions

Before running the fast-follower test:

- [ ] Phase A-C complete and verified
- [ ] `gold.air_quality_hourly` operational
- [ ] `gold.indoor_air_quality_aligned` operational with 3 streams
- [ ] `outdoor-air-quality` Silver table exists with data
- [ ] `outdoor-air-quality` NOT in Gold layer (deliberately excluded from Phase C)

### 1.2 Test Constraints

| Constraint | Requirement | Verification |
|------------|-------------|--------------|
| Time | < 1 hour total | Timed checkpoints |
| Code Changes | Zero `.rs` files modified | `git diff --name-only` |
| Config Changes | Only YAML/JSON files | `git diff --name-only` |
| Deployment | Standard `deploy.sh apply` | No manual SQL |

### 1.3 Test Categories

```
FAST-FOLLOWER TESTS
├── Pre-Flight Tests (verify clean state)
├── Timed Procedure Tests (config-only additions)
├── Verification Tests (Gold layer operational)
└── Post-Test Analysis (document learnings)
```

---

## 2. v11-V01: Fast-Follower Stream Test

### 2.1 Pre-Flight Tests

```rust
// Location: tools/ndp-gold-ddl/tests/fast_follower/pre_flight.rs

/// PRE-FLIGHT: Verify outdoor-air-quality NOT in Gold layer
#[tokio::test]
#[ignore]
async fn pre_flight_outdoor_air_quality_not_in_gold() {
    // Assert: No Gold aggregate exists yet
    assert!(
        !check_continuous_aggregate_exists("gold", "outdoor_air_quality_hourly").await,
        "outdoor-air-quality should NOT be in Gold layer before test"
    );
}

/// PRE-FLIGHT: Verify outdoor-air-quality Silver table exists
#[tokio::test]
#[ignore]
async fn pre_flight_outdoor_air_quality_silver_exists() {
    // Assert: Silver table has data
    let count = query_table_count("silver.outdoor_air_quality_observations").await;
    assert!(count > 0, "Silver table should have data for fast-follower test");
}

/// PRE-FLIGHT: Verify aligned view does NOT include outdoor-air-quality
#[tokio::test]
#[ignore]
async fn pre_flight_aligned_view_excludes_outdoor_air() {
    let columns = get_view_columns("gold.indoor_air_quality_aligned").await;

    // Should NOT have outdoor_air columns yet
    assert!(
        !columns.iter().any(|c| c.contains("outdoor_air")),
        "Aligned view should not include outdoor-air-quality yet"
    );
}

/// PRE-FLIGHT: Verify no uncommitted code changes
#[test]
fn pre_flight_clean_git_state() {
    let output = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .expect("git command failed");

    let status = String::from_utf8_lossy(&output.stdout);
    assert!(
        status.is_empty(),
        "Git working directory should be clean before fast-follower test: {}",
        status
    );
}
```

### 2.2 Config Change Tests

```rust
/// CONFIG: gold_etl section validates for outdoor-air-quality
#[test]
fn test_outdoor_air_quality_gold_config_valid() {
    // Given: The gold_etl config we will add
    let config = json!({
        "gold_etl": {
            "enabled": true,
            "aggregates": {
                "granularities": ["1 hour"],
                "fields": {
                    "pm25": { "metrics": ["mean", "std", "min", "max"] },
                    "pm10": { "metrics": ["mean", "min", "max"] },
                    "aqi": { "metrics": ["mean", "max"] }
                }
            }
        }
    });

    // When: Validate against schema
    let result = validate_against_schema(&config, "gold-etl.schema.json");

    // Then: Valid
    assert!(result.is_ok(), "Config should be valid: {:?}", result);
}

/// CONFIG: Domain config validates with 4th stream
#[test]
fn test_domain_config_with_outdoor_air_valid() {
    // Given: Domain config extended with outdoor-air-quality
    let config = load_domain_config("config/domains/indoor-air-quality/domain.yaml");

    // Simulate adding 4th stream
    let mut extended = config.clone();
    extended.streams.push(StreamRef {
        stream_id: "outdoor-air-quality".to_string(),
        alias: "oaq".to_string(),
        role: StreamRole::Context,
    });

    // When: Validate
    let result = validate_domain_config(&extended);

    // Then: Valid
    assert!(result.is_ok());
}
```

### 2.3 DDL Generation Tests

```rust
/// DDL: outdoor-air-quality generates valid SQL
#[test]
fn test_outdoor_air_quality_ddl_generation() {
    // Given: Mock config loader with outdoor-air-quality gold_etl
    let loader = MockConfigLoader::new()
        .with_stream(create_stream_config("outdoor-air-quality"))
        .with_gold_config("outdoor-air-quality", create_outdoor_air_gold_config());

    // When: Generate DDL
    let gold_config = loader.load_gold_etl_config("outdoor-air-quality").unwrap();
    let sql = generate_continuous_aggregate(&gold_config).unwrap();

    // Then: Valid SQL structure
    assert!(sql.contains("CREATE MATERIALIZED VIEW gold.outdoor_air_quality_hourly"));
    assert!(sql.contains("WITH (timescaledb.continuous)"));
    assert!(sql.contains("pm25_mean") || sql.contains("AVG(pm25)"));
}

/// DDL: Aligned view regenerates with 4th stream
#[test]
fn test_aligned_view_regenerates_with_4_streams() {
    // Given: Domain config with 4 streams
    let domain = create_four_stream_domain();

    // When: Generate aligned view
    let sql = generate_aligned_view(&domain).unwrap();

    // Then: All 4 streams in JOIN
    assert!(sql.contains("gold.outdoor_air_quality_hourly") ||
            sql.contains("oaq."));

    // Should have 3 JOINs for 4 streams
    let join_count = sql.matches("FULL OUTER JOIN").count();
    assert_eq!(join_count, 3, "Should have 3 JOINs for 4 streams");
}
```

### 2.4 Verification Tests

```rust
/// VERIFY: Continuous aggregate created
#[tokio::test]
#[ignore]
async fn verify_outdoor_air_quality_aggregate_created() {
    // After fast-follower procedure
    assert!(
        check_continuous_aggregate_exists("gold", "outdoor_air_quality_hourly").await,
        "Continuous aggregate should exist after fast-follower"
    );
}

/// VERIFY: Refresh policy created
#[tokio::test]
#[ignore]
async fn verify_outdoor_air_quality_refresh_policy() {
    let policies = get_refresh_policies("gold.outdoor_air_quality_hourly").await;
    assert!(!policies.is_empty(), "Refresh policy should exist");
}

/// VERIFY: Aligned view includes new stream
#[tokio::test]
#[ignore]
async fn verify_aligned_view_includes_outdoor_air() {
    let columns = get_view_columns("gold.indoor_air_quality_aligned").await;

    // Should now have outdoor_air columns
    assert!(
        columns.iter().any(|c| c.contains("oaq_") || c.contains("outdoor_air")),
        "Aligned view should include outdoor-air-quality columns"
    );
}

/// VERIFY: Data flows through pipeline
#[tokio::test]
#[ignore]
async fn verify_data_flows_through() {
    // Wait for refresh
    tokio::time::sleep(std::time::Duration::from_secs(60)).await;

    // Query aggregate
    let rows = query_aggregate("gold.outdoor_air_quality_hourly", 1).await;
    assert!(!rows.is_empty(), "Should have data after refresh");

    // Query aligned view
    let aligned_rows = query_aligned_view("indoor_air_quality_aligned", 1).await;
    assert!(!aligned_rows.is_empty(), "Aligned view should have data");
}

/// VERIFY: Zero Rust code changes
#[test]
fn verify_no_rust_changes() {
    let output = std::process::Command::new("git")
        .args(["diff", "--name-only", "HEAD"])
        .output()
        .expect("git command failed");

    let changed_files = String::from_utf8_lossy(&output.stdout);
    let rust_changes: Vec<&str> = changed_files
        .lines()
        .filter(|f| f.ends_with(".rs"))
        .collect();

    assert!(
        rust_changes.is_empty(),
        "Fast-follower test FAILED: Rust files were modified: {:?}",
        rust_changes
    );
}
```

---

## 3. v11-008/v11-009: Feature Computation Tests

### 3.1 Lag Feature Tests

```rust
/// Unit: Lag feature SQL generation
#[test]
fn test_lag_1h_feature_generation() {
    let config = FeatureConfig {
        lag: Some(LagConfig {
            enabled: true,
            lags_hours: vec![1, 6, 24],
            fields: vec!["pm25".to_string(), "co2".to_string()],
        }),
        ..Default::default()
    };

    let sql = generate_lag_features(&config, "air_quality_hourly");

    // 1-hour lag
    assert!(sql.contains("pm25_lag_1h") || sql.contains("pm25_mean_lag_1h"));
    assert!(sql.contains("LAG(") && sql.contains(", 1)"));

    // 6-hour lag
    assert!(sql.contains("pm25_lag_6h") || sql.contains(", 6)"));

    // 24-hour lag
    assert!(sql.contains("pm25_lag_24h") || sql.contains(", 24)"));
}

/// Unit: Lag handles edge cases (no prior data)
#[test]
fn test_lag_null_at_start() {
    let sql = generate_lag_feature("pm25_mean", 1);

    // LAG returns NULL at start - should NOT use COALESCE
    assert!(sql.contains("LAG(pm25_mean, 1)"));
    // NULL is valid for lag features (no prior data)
}

/// Component: Lag features in aligned view
#[test]
fn test_lag_features_in_aligned_view() {
    let domain = create_domain_with_features(FeatureConfig {
        lag: Some(LagConfig {
            enabled: true,
            lags_hours: vec![1, 24],
            fields: vec!["pm25".to_string()],
        }),
        ..Default::default()
    });

    let sql = generate_aligned_view(&domain).unwrap();

    assert!(sql.contains("pm25_lag_1h") || sql.contains("pm25_mean_lag_1h"));
    assert!(sql.contains("pm25_lag_24h") || sql.contains("pm25_mean_lag_24h"));
}
```

### 3.2 Rolling Feature Tests

```rust
/// Unit: Rolling mean generation
#[test]
fn test_rolling_mean_4h() {
    let config = FeatureConfig {
        rolling: Some(RollingConfig {
            enabled: true,
            windows: vec!["4 hours".to_string()],
            stats: vec!["mean".to_string()],
            fields: vec!["pm25".to_string()],
        }),
        ..Default::default()
    };

    let sql = generate_rolling_features(&config, "air_quality_hourly");

    assert!(sql.contains("pm25_rolling_4h_mean") || sql.contains("pm25_mean_4h"));
    assert!(sql.contains("AVG(") && sql.contains("OVER ("));
    assert!(sql.contains("ROWS BETWEEN") || sql.contains("RANGE BETWEEN"));
}

/// Unit: Rolling std generation
#[test]
fn test_rolling_std_24h() {
    let config = FeatureConfig {
        rolling: Some(RollingConfig {
            enabled: true,
            windows: vec!["24 hours".to_string()],
            stats: vec!["std".to_string()],
            fields: vec!["pm25".to_string()],
        }),
        ..Default::default()
    };

    let sql = generate_rolling_features(&config, "air_quality_hourly");

    assert!(sql.contains("STDDEV(") || sql.contains("STDDEV_SAMP("));
    assert!(sql.contains("24") || sql.contains("23 PRECEDING"));
}
```

---

## 4. v11-010: Gold Layer Data Dictionary Tests

### 4.1 Component Tests

```rust
/// Component: Gold tables registered in dictionary
#[tokio::test]
async fn test_gold_tables_in_dictionary() {
    // Arrange
    let db = MockTimescaleDb::new();

    // Act
    sync_gold_table_to_dictionary(&db, "gold.air_quality_hourly", "air-quality").await.unwrap();

    // Assert
    assert!(db.sql_contains("INSERT INTO data_dictionary.gold_tables"));
    assert!(db.sql_contains("'gold.air_quality_hourly'"));
}

/// Component: Gold columns registered
#[tokio::test]
async fn test_gold_columns_in_dictionary() {
    let db = MockTimescaleDb::new();

    sync_gold_columns_to_dictionary(&db, "gold.air_quality_hourly", &[
        ("pm25_mean", "aggregate"),
        ("pm25_std", "aggregate"),
        ("pm25_lag_1h", "lag_feature"),
    ]).await.unwrap();

    // Assert: All columns registered
    let sqls = db.get_executed_sql();
    assert!(sqls.iter().any(|s| s.contains("pm25_mean")));
    assert!(sqls.iter().any(|s| s.contains("'aggregate'")));
    assert!(sqls.iter().any(|s| s.contains("'lag_feature'")));
}

/// Component: MCP query returns Gold metadata
#[tokio::test]
#[ignore]
async fn test_mcp_gold_metadata_query() {
    let result = mcp_query_gold_tables().await;

    assert!(result.tables.iter().any(|t| t.name == "gold.air_quality_hourly"));

    let table = result.tables.iter()
        .find(|t| t.name == "gold.air_quality_hourly")
        .unwrap();

    assert!(table.columns.iter().any(|c| c.name == "pm25_mean"));
}
```

---

## 5. v11-011: Correlation-Ready Dashboard Tests

### 5.1 Dashboard Load Tests

```rust
/// Dashboard: Panel definitions valid JSON
#[test]
fn test_dashboard_panels_valid_json() {
    let dashboard = load_dashboard_json("deploy/grafana/dashboards/gold-correlation.json");

    // Parse succeeds
    assert!(dashboard.is_ok());

    // Has expected panels
    let dashboard = dashboard.unwrap();
    assert!(dashboard.panels.len() >= 5, "Should have at least 5 panels");
}

/// Dashboard: Queries reference Gold views
#[test]
fn test_dashboard_queries_reference_gold() {
    let dashboard = load_dashboard_json("deploy/grafana/dashboards/gold-correlation.json").unwrap();

    // At least one panel queries aligned view
    let aligned_query = dashboard.panels.iter()
        .any(|p| p.targets.iter().any(|t| t.raw_sql.contains("indoor_air_quality_aligned")));

    assert!(aligned_query, "Dashboard should query aligned view");
}

/// Dashboard: Objective thresholds shown
#[test]
fn test_dashboard_shows_objectives() {
    let dashboard = load_dashboard_json("deploy/grafana/dashboards/gold-correlation.json").unwrap();

    // Has threshold lines or annotations
    let has_thresholds = dashboard.panels.iter()
        .any(|p| p.field_config.defaults.thresholds.is_some() ||
                 p.field_config.defaults.custom.threshold_labels.is_some());

    assert!(has_thresholds, "Dashboard should show objective thresholds");
}
```

### 5.2 Performance Tests

```rust
/// Dashboard: Load time < 2 seconds
#[tokio::test]
#[ignore]
async fn test_dashboard_load_time() {
    let start = std::time::Instant::now();

    // Simulate dashboard load (all queries)
    let queries = get_dashboard_queries("gold-correlation").await;
    for query in queries {
        execute_query(&query).await.unwrap();
    }

    let duration = start.elapsed();

    assert!(
        duration.as_secs() < 2,
        "Dashboard load took {}s, expected < 2s", duration.as_secs()
    );
}

/// Dashboard: Individual panel queries < 500ms
#[tokio::test]
#[ignore]
async fn test_panel_query_performance() {
    let slow_panels: Vec<String> = Vec::new();

    for (panel_name, query) in get_panel_queries("gold-correlation").await {
        let start = std::time::Instant::now();
        execute_query(&query).await.unwrap();
        let duration = start.elapsed();

        if duration.as_millis() > 500 {
            slow_panels.push(format!("{}: {}ms", panel_name, duration.as_millis()));
        }
    }

    assert!(
        slow_panels.is_empty(),
        "Slow panels (>500ms): {:?}", slow_panels
    );
}
```

---

## 6. Integration Tests

```rust
/// INTEGRATION: Complete fast-follower procedure
#[tokio::test]
#[ignore]
async fn integration_fast_follower_procedure() {
    // Pre-flight verification
    assert!(!check_continuous_aggregate_exists("gold", "outdoor_air_quality_hourly").await);

    // Execute fast-follower (config changes + deploy)
    // This would be done manually during the timed test

    // Verification after manual execution
    // See VALIDATION-PROCEDURE.md for timed procedure
}

/// INTEGRATION: Data dictionary complete after Phase D
#[tokio::test]
#[ignore]
async fn integration_data_dictionary_complete() {
    // All Gold tables documented
    let gold_tables = query_data_dictionary_gold_tables().await;

    let expected_tables = [
        "gold.air_quality_hourly",
        "gold.air_quality_daily",
        "gold.outdoor_weather_hourly",
        "gold.state_events_hourly",
        "gold.indoor_air_quality_aligned",
        "gold.outdoor_air_quality_hourly", // After fast-follower
    ];

    for table in expected_tables {
        assert!(
            gold_tables.iter().any(|t| t.name == table),
            "Missing data dictionary entry for: {}", table
        );
    }
}
```

---

## 7. Test Execution Commands

```bash
# Run pre-flight tests
cargo test -p ndp-gold-ddl --test fast_follower -- pre_flight --ignored

# Run config validation tests
cargo test -p ndp-gold-ddl --test fast_follower -- config

# Run verification tests (after manual fast-follower procedure)
cargo test -p ndp-gold-ddl --test fast_follower -- verify --ignored

# Run feature computation tests
cargo test -p ndp-gold-ddl --lib feature
cargo test -p ndp-gold-ddl --lib lag

# Run all Phase D tests
./scripts/test-phase-d.sh
```

---

## 8. Test Metrics (Phase D Target)

| Category | Target | Priority |
|----------|--------|----------|
| Pre-Flight Tests | 4 | Critical |
| Config Validation Tests | 5-8 | High |
| DDL Generation Tests | 5-8 | High |
| Verification Tests | 6-8 | Critical |
| Feature Tests | 10-15 | Medium |
| Dashboard Tests | 5-8 | High |
| Fast-Follower Time | < 60 min | **Critical** |
| Code Changes | 0 `.rs` files | **Critical** |

---

## 9. Exit Criteria

Phase D testing complete when:

- [ ] Pre-flight tests pass (clean state verified)
- [ ] Fast-follower procedure completed in < 1 hour
- [ ] Zero Rust code changes verified
- [ ] Verification tests pass (Gold layer operational)
- [ ] Feature computation tests pass
- [ ] Data dictionary tests pass
- [ ] Dashboard tests pass
- [ ] FAST-FOLLOWER-REPORT.md documented
- [ ] Architecture validated for V1.2 handoff

---

## References

- [PHASE-D-OVERVIEW.md](../specification/PHASE-D-OVERVIEW.md) - Phase D specification
- [VALIDATION-PROCEDURE.md](./VALIDATION-PROCEDURE.md) - Detailed timed procedure
- [TESTING-STRATEGY.md](../../TESTING-STRATEGY.md) - Overall testing strategy

---

*Phase D Test Plan created: 2026-02-04*
