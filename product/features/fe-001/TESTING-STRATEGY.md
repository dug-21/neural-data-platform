# FE-001: Gold Layer Testing Strategy (London TDD)

> **Feature ID:** fe-001
> **Version:** 1.0
> **Created:** 2026-02-04
> **Testing Approach:** London School TDD (Outside-In, Mock-Driven)

---

## Executive Summary

This document defines the London TDD testing strategy for FE-001 Gold Layer Foundation. The strategy emphasizes **outside-in development**, **mocking collaborators**, and **testing behavior over implementation**. Tests are organized in a pyramid structure with integration tests at the top driving the development of unit tests below.

---

## 1. London TDD Overview

### Core Principles Applied to FE-001

| Principle | Application to Gold Layer |
|-----------|---------------------------|
| **Outside-In Development** | Start with integration tests using deploy.sh, work inward to ndp-gold-ddl unit tests |
| **Mock Collaborators** | Mock ConfigLoader, etcd client, TimescaleDB connection for unit tests |
| **Test Behavior** | Verify SQL generation behavior, not internal data structures |
| **Interface Contracts** | Define contracts between components before implementation |
| **Isolate Units** | Each generator module tested independently with mocked dependencies |

### Development Flow

```
1. ACCEPTANCE TEST (integration)
   └── Write failing integration test using deploy.sh
       └── Identifies what ndp-gold-ddl must produce

2. OUTSIDE-IN UNIT TESTS
   ├── Test CLI interface behavior
   ├── Test config loading (with MockConfigLoader)
   └── Test SQL generation output

3. IMPLEMENTATION
   └── Write minimal code to pass tests

4. REFACTOR
   └── Improve code while keeping tests green
```

---

## 2. Test Pyramid Structure

```
                    ┌─────────────────────────────────────┐
                    │       INTEGRATION TESTS             │
                    │   (deploy.sh integration mode)      │
                    │   - Full pipeline: config → DDL     │
                    │   - TimescaleDB schema creation     │
                    │   - ~5 tests, slow (~30s each)      │
                    └─────────────────────────────────────┘
                                     ▲
                    ┌─────────────────────────────────────┐
                    │      COMPONENT TESTS                │
                    │   (ndp-gold-ddl with mock deps)     │
                    │   - CLI argument handling           │
                    │   - Config loading + validation     │
                    │   - ~20 tests, medium (~1s each)    │
                    └─────────────────────────────────────┘
                                     ▲
    ┌─────────────────────────────────────────────────────────────────┐
    │                       UNIT TESTS                                 │
    │              (generators/*.rs, validation/*.rs)                  │
    │   - Continuous aggregate SQL generation                         │
    │   - Aligned view SQL generation                                 │
    │   - Feature SQL generation                                      │
    │   - Expression validation                                       │
    │   - ~50+ tests, fast (<100ms each)                              │
    └─────────────────────────────────────────────────────────────────┘
```

### Test Counts by Phase

| Phase | Unit Tests | Component Tests | Integration Tests |
|-------|------------|-----------------|-------------------|
| **A: Foundation** | 25-30 | 8-10 | 3-5 |
| **B: First Stream** | 15-20 | 5-8 | 2-3 |
| **C: Cross-Stream** | 20-25 | 8-10 | 3-5 |
| **D: Validation** | 10-15 | 3-5 | 2-3 |
| **E: Events** | 15-20 | 5-8 | 2-3 |
| **Total** | ~80-110 | ~30-40 | ~12-18 |

---

## 3. Mocking Strategy

### What to Mock (Unit Tests)

| Dependency | Mock Approach | Rationale |
|------------|---------------|-----------|
| **ConfigLoader** | `MockConfigLoader` from `core/src/config/mock_loader.rs` | Already exists, proven pattern |
| **etcd client** | `MockEtcdClient` (new) | Isolate from infrastructure |
| **TimescaleDB** | `MockDbConnection` (new) | Isolate from database |
| **File system** | In-memory config (already supported) | Test without file I/O |

### What to Test Real (Integration Tests)

| Dependency | Why Test Real |
|------------|---------------|
| **TimescaleDB** | Verify SQL syntax is valid; catch dialect issues |
| **deploy.sh orchestration** | Verify integration with existing patterns |
| **JSON Schema validation** | Verify schema files work correctly |
| **etcd config sync** | Verify end-to-end config flow |

### Mock Implementation Pattern

Following the established pattern from `mock_loader.rs`:

```rust
// tools/ndp-gold-ddl/src/mocks/db_connection.rs
pub struct MockDbConnection {
    should_fail: RwLock<Option<DbError>>,
    executed_sql: RwLock<Vec<String>>,
}

impl MockDbConnection {
    pub fn new() -> Self { ... }

    pub fn with_error(self, error: DbError) -> Self { ... }

    pub fn get_executed_sql(&self) -> Vec<String> {
        self.executed_sql.read().unwrap().clone()
    }
}

#[async_trait]
impl DbConnection for MockDbConnection {
    async fn execute(&self, sql: &str) -> Result<(), DbError> {
        if let Some(ref err) = *self.should_fail.read().unwrap() {
            return Err(err.clone());
        }
        self.executed_sql.write().unwrap().push(sql.to_string());
        Ok(())
    }
}
```

---

## 4. Interface Contracts

### Contract 1: ConfigLoader -> ndp-gold-ddl

```rust
// INPUT: StreamConfig with gold_etl section
pub struct StreamConfig {
    pub stream_id: String,
    pub gold_etl: Option<GoldEtlConfig>,
    // ... other fields
}

pub struct GoldEtlConfig {
    pub enabled: bool,
    pub aggregates: AggregateConfig,
    pub features: FeatureConfig,
    pub transitions: Option<TransitionConfig>,
}

// CONTRACT:
// - If gold_etl is None, return early with no SQL
// - If gold_etl.enabled is false, return early with no SQL
// - If gold_etl.aggregates.fields is empty, return error
// - Validate field references exist in stream.fields
```

### Contract 2: ndp-gold-ddl -> deploy.sh

```rust
// OUTPUT: Valid SQL strings to stdout

// CONTRACT:
// - SQL is valid TimescaleDB syntax
// - SQL is idempotent (can re-run safely)
// - Exit code 0 on success, non-zero on failure
// - Errors to stderr, SQL to stdout
// - Supports --action sync|recreate modes
```

### Contract 3: Generators -> Validators

```rust
// CONTRACT:
// - All field references must be validated before SQL generation
// - Invalid metrics return ValidationError, not panics
// - All generated column names follow naming convention
```

---

## 5. Integration Test Approach

### Using deploy.sh in Integration Mode

Integration tests use `DEPLOY_ENV=integration` to test against local Docker infrastructure.

```bash
# Start integration infrastructure
docker compose -f docker-compose.integration.yml up -d

# Run integration tests
DEPLOY_ENV=integration cargo test -p ndp-gold-ddl --test integration -- --ignored
```

### Integration Test Structure

```
tests/integration/
├── mod.rs
├── continuous_aggregate_test.rs
├── aligned_view_test.rs
└── deploy_idempotency_test.rs
```

### Integration Test Pattern

```rust
// tests/integration/continuous_aggregate_test.rs

use std::process::Command;

/// Test that generated DDL creates valid TimescaleDB objects
#[tokio::test]
#[ignore] // Requires Docker infrastructure
async fn test_continuous_aggregate_creates_successfully() {
    // Arrange: Ensure clean state
    cleanup_gold_schema().await;

    // Act: Run ndp-gold-ddl through deploy.sh pattern
    let output = Command::new("./deploy/pi/deploy.sh")
        .env("DEPLOY_ENV", "integration")
        .arg("apply")
        .arg("--dry-run")
        .output()
        .expect("deploy.sh should run");

    // Extract generated SQL from dry-run output
    let sql = extract_gold_ddl_from_output(&output.stdout);

    // Execute SQL directly to verify validity
    let result = execute_sql_on_timescale(&sql).await;

    // Assert: SQL executed without errors
    assert!(result.is_ok(), "Generated SQL should be valid: {:?}", result);

    // Assert: Objects exist
    let exists = check_materialized_view_exists("gold.air_quality_hourly").await;
    assert!(exists, "Continuous aggregate should exist after DDL");
}

/// Test that deploy.sh handles gold-table declarations
#[tokio::test]
#[ignore]
async fn test_deploy_handles_gold_table_declaration() {
    // Arrange: Create manifest with gold-table declaration
    let manifest = json!({
        "version": "test",
        "declarations": [
            { "type": "gold-table", "stream_id": "air-quality", "action": "sync" }
        ]
    });
    write_test_manifest(&manifest);

    // Act: Run deploy apply
    let result = run_deploy_apply().await;

    // Assert: Should complete successfully
    assert!(result.success, "deploy apply should succeed");

    // Assert: Gold objects created
    assert!(check_gold_schema_exists().await);
}
```

---

## 6. Acceptance Test Templates

### Phase A: Architecture Foundation

```rust
// Acceptance: v11-A01 Gold ETL JSON Schema
#[test]
fn acceptance_gold_etl_schema_validates_correctly() {
    // Given: A valid gold_etl config
    let config = load_test_config("fixtures/valid_gold_etl.json");

    // When: Validated against schema
    let result = validate_against_schema(&config, "gold-etl.schema.json");

    // Then: Validation passes
    assert!(result.is_ok());
}

#[test]
fn acceptance_gold_etl_schema_rejects_invalid() {
    // Given: Invalid gold_etl config (unknown metric)
    let config = json!({
        "gold_etl": {
            "enabled": true,
            "aggregates": {
                "fields": {
                    "pm25": { "metrics": ["invalid_metric"] }
                }
            }
        }
    });

    // When: Validated against schema
    let result = validate_against_schema(&config, "gold-etl.schema.json");

    // Then: Validation fails with helpful message
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("invalid_metric"));
}

// Acceptance: v11-A02 Gold DDL Tool
#[test]
fn acceptance_ddl_tool_generates_sql() {
    // Given: Valid stream config with gold_etl
    let config = load_test_config("fixtures/air_quality_with_gold.json");

    // When: Run ndp-gold-ddl generate
    let sql = generate_gold_ddl(&config);

    // Then: SQL contains expected structures
    assert!(sql.contains("CREATE MATERIALIZED VIEW"));
    assert!(sql.contains("gold.air_quality_hourly"));
    assert!(sql.contains("WITH (timescaledb.continuous)"));
}
```

### Phase B: First Stream

```rust
// Acceptance: v11-003 Per-Stream Continuous Aggregates
#[tokio::test]
#[ignore]
async fn acceptance_continuous_aggregate_works_for_air_quality() {
    // Given: Integration environment with Silver data
    setup_test_silver_data("air-quality").await;

    // When: Deploy Gold for air-quality
    deploy_gold_for_stream("air-quality").await;

    // Then: Aggregate refreshes and contains data
    wait_for_aggregate_refresh().await;
    let count = query_aggregate_count("gold.air_quality_hourly").await;
    assert!(count > 0, "Aggregate should have data after refresh");
}

// Acceptance: v11-008 Basic Feature Computation
#[tokio::test]
#[ignore]
async fn acceptance_lag_features_computed() {
    // Given: Gold aggregate exists with data
    setup_gold_aggregate("air-quality").await;

    // When: Query for lag features
    let row = query_gold_features("air-quality", "2024-01-15 12:00:00").await;

    // Then: Lag features present
    assert!(row.contains_key("pm25_lag_1h"));
    assert!(row.contains_key("pm25_lag_24h"));
}
```

---

## 7. Test Data Strategy

### Fixture Organization

```
tools/ndp-gold-ddl/tests/fixtures/
├── configs/
│   ├── valid/
│   │   ├── air_quality_basic.json         # Minimal valid config
│   │   ├── air_quality_full.json          # All features enabled
│   │   ├── outdoor_weather.json           # Different stream type
│   │   └── home_assistant_state.json      # state_event type
│   └── invalid/
│       ├── missing_fields.json            # Required field missing
│       ├── unknown_metric.json            # Invalid metric type
│       ├── wrong_field_reference.json     # References non-existent field
│       └── empty_aggregates.json          # No aggregates defined
├── expected_sql/
│   ├── air_quality_hourly.sql             # Expected output for basic
│   ├── air_quality_daily.sql              # Expected daily aggregate
│   └── aligned_view.sql                   # Expected aligned view
└── schemas/
    └── gold-etl.schema.json               # Schema for testing
```

### Test Data Helpers

```rust
// tests/helpers/fixtures.rs

pub fn load_test_config(name: &str) -> StreamConfig {
    let path = format!("tests/fixtures/configs/valid/{}.json", name);
    let content = std::fs::read_to_string(&path)
        .expect(&format!("Failed to read fixture: {}", path));
    serde_json::from_str(&content)
        .expect(&format!("Failed to parse fixture: {}", path))
}

pub fn load_expected_sql(name: &str) -> String {
    let path = format!("tests/fixtures/expected_sql/{}.sql", name);
    std::fs::read_to_string(&path)
        .expect(&format!("Failed to read expected SQL: {}", path))
}

pub fn create_mock_config_loader() -> MockConfigLoader {
    MockConfigLoader::new()
        .with_stream(load_test_config("air_quality_basic"))
        .with_stream(load_test_config("outdoor_weather"))
}

// For integration tests: sample Silver data
pub async fn setup_test_silver_data(stream_id: &str) {
    let sql = match stream_id {
        "air-quality" => include_str!("fixtures/silver_data/air_quality.sql"),
        "outdoor-weather" => include_str!("fixtures/silver_data/outdoor_weather.sql"),
        _ => panic!("Unknown stream for test data: {}", stream_id),
    };
    execute_sql_on_timescale(sql).await.unwrap();
}
```

### SQL Comparison Pattern

```rust
// tests/helpers/sql_compare.rs

pub fn assert_sql_equivalent(actual: &str, expected: &str) {
    let actual_normalized = normalize_sql(actual);
    let expected_normalized = normalize_sql(expected);

    if actual_normalized != expected_normalized {
        // Show diff for debugging
        let diff = pretty_diff(&expected_normalized, &actual_normalized);
        panic!("SQL mismatch:\n{}", diff);
    }
}

fn normalize_sql(sql: &str) -> String {
    sql.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty() && !l.starts_with("--"))
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
```

---

## 8. Test Running Guide

### Unit Tests (Fast, No Infrastructure)

```bash
# Run all unit tests
cargo test -p ndp-gold-ddl --lib

# Run specific generator tests
cargo test -p ndp-gold-ddl generators::continuous_aggregate

# Run with output
cargo test -p ndp-gold-ddl -- --nocapture
```

### Component Tests (Fast, No Infrastructure)

```bash
# Run CLI tests
cargo test -p ndp-gold-ddl --test cli_tests

# Run validation tests
cargo test -p ndp-gold-ddl --test validation_tests
```

### Integration Tests (Slow, Requires Docker)

```bash
# Start infrastructure
docker compose -f docker-compose.integration.yml up -d

# Wait for TimescaleDB
./scripts/wait-for-timescale.sh

# Run integration tests
DEPLOY_ENV=integration cargo test -p ndp-gold-ddl --test integration -- --ignored

# Cleanup
docker compose -f docker-compose.integration.yml down
```

### Full Test Suite

```bash
# CI/CD pattern
./scripts/run-gold-tests.sh
```

---

## 9. Test Categories and Markers

### Test Naming Convention

```rust
// Pattern: test_{component}_{scenario}_{expected_result}

// Unit tests
#[test]
fn test_continuous_aggregate_generator_valid_config_generates_sql() { ... }

#[test]
fn test_continuous_aggregate_generator_empty_fields_returns_error() { ... }

// Integration tests
#[tokio::test]
#[ignore]
async fn test_deploy_air_quality_gold_creates_aggregate() { ... }
```

### Test Markers

| Marker | Usage | Command |
|--------|-------|---------|
| `#[test]` | Unit tests, no async | `cargo test --lib` |
| `#[tokio::test]` | Async tests, no infra | `cargo test --lib` |
| `#[ignore]` | Integration, needs infra | `cargo test -- --ignored` |
| `#[should_panic]` | Expected panic | Normal `cargo test` |

---

## 10. Coverage Strategy

### Target Coverage by Component

| Component | Target | Priority | Rationale |
|-----------|--------|----------|-----------|
| **generators/continuous_aggregate.rs** | 90% | Critical | Core SQL generation |
| **generators/aligned_view.rs** | 85% | Critical | Cross-stream joins |
| **generators/features.rs** | 80% | High | Feature computations |
| **validation/expressions.rs** | 90% | High | Prevents invalid SQL |
| **validation/config.rs** | 85% | High | Config validation |
| **cli/main.rs** | 70% | Medium | CLI argument handling |
| **generators/events.rs** | 80% | Medium | Event extraction |

### Measuring Coverage

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Run with coverage report
cargo tarpaulin -p ndp-gold-ddl --out Html --output-dir coverage/

# View report
open coverage/tarpaulin-report.html
```

---

## 11. Mocking External Dependencies

### MockConfigLoader (Already Exists)

Location: `core/src/config/mock_loader.rs`

```rust
use platform_core::config::{MockConfigLoader, ConfigLoader};

#[tokio::test]
async fn test_loads_gold_config_from_mock() {
    let loader = MockConfigLoader::new()
        .with_stream(create_gold_enabled_config("air-quality"));

    let config = loader.load_stream_config("air-quality").await.unwrap();
    assert!(config.gold_etl.is_some());
}
```

### MockTimescaleDb (New)

```rust
// tools/ndp-gold-ddl/src/mocks/timescale.rs

pub struct MockTimescaleDb {
    views: RwLock<HashSet<String>>,
    should_fail: RwLock<Option<TimescaleError>>,
}

impl MockTimescaleDb {
    pub fn new() -> Self { ... }

    pub fn with_existing_view(self, name: &str) -> Self {
        self.views.write().unwrap().insert(name.to_string());
        self
    }

    pub fn view_exists(&self, name: &str) -> bool {
        self.views.read().unwrap().contains(name)
    }
}

#[async_trait]
impl TimescaleConnection for MockTimescaleDb {
    async fn execute(&self, sql: &str) -> Result<(), TimescaleError> {
        if let Some(ref err) = *self.should_fail.read().unwrap() {
            return Err(err.clone());
        }
        // Parse SQL to detect CREATE statements and update views
        if sql.contains("CREATE MATERIALIZED VIEW") {
            let view_name = extract_view_name(sql);
            self.views.write().unwrap().insert(view_name);
        }
        Ok(())
    }

    async fn view_exists(&self, schema: &str, name: &str) -> Result<bool, TimescaleError> {
        let full_name = format!("{}.{}", schema, name);
        Ok(self.views.read().unwrap().contains(&full_name))
    }
}
```

---

## 12. Test Checklist (Per Feature)

Before marking any feature complete, verify:

### Unit Test Checklist

- [ ] Happy path test exists
- [ ] Error case tests exist for each failure mode
- [ ] Edge cases covered (empty arrays, missing optional fields)
- [ ] Test names follow `test_{component}_{scenario}_{expected}` pattern
- [ ] Tests use Arrange-Act-Assert structure
- [ ] Mocks configured for all external dependencies
- [ ] No flaky tests (deterministic)

### Integration Test Checklist

- [ ] Integration test exists with `#[ignore]` marker
- [ ] Test uses `DEPLOY_ENV=integration` pattern
- [ ] Cleanup step included (reset state after test)
- [ ] Infrastructure requirements documented
- [ ] Idempotency verified (test can run twice)

### Documentation Checklist

- [ ] Public functions have doc comments
- [ ] Error types documented
- [ ] Usage examples in module docs
- [ ] Test assumptions documented

---

## 13. CI/CD Integration

### GitHub Actions Workflow Pattern

```yaml
# .github/workflows/gold-layer-tests.yml
name: Gold Layer Tests

on:
  pull_request:
    paths:
      - 'tools/ndp-gold-ddl/**'
      - 'config/schemas/gold-*.json'
      - 'config/base/streams/**/gold_etl*'

jobs:
  unit-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Run unit tests
        run: cargo test -p ndp-gold-ddl --lib

  integration-tests:
    runs-on: ubuntu-latest
    services:
      timescaledb:
        image: timescale/timescaledb:latest-pg15
        env:
          POSTGRES_PASSWORD: test
        ports:
          - 5432:5432
      etcd:
        image: bitnami/etcd:latest
        env:
          ALLOW_NONE_AUTHENTICATION: yes
        ports:
          - 2379:2379
    steps:
      - uses: actions/checkout@v4
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Wait for TimescaleDB
        run: |
          until pg_isready -h localhost -p 5432; do sleep 1; done
      - name: Run integration tests
        env:
          DEPLOY_ENV: integration
          TEST_POSTGRES_URL: postgres://postgres:test@localhost:5432/postgres
        run: cargo test -p ndp-gold-ddl --test integration -- --ignored
```

---

## 14. Related Documents

- [SCOPE.md](./SCOPE.md) - Feature scope and requirements
- [architecture/DECISIONS.md](./architecture/DECISIONS.md) - Architecture decisions including ADR-FE001-001
- [Phase A Test Plan](./phase-a/refinement/TEST-PLAN.md) - Detailed Phase A test plan
- [docs/testing/AIR-005-TEST-DESIGN.md](/workspaces/neural-data-platform/docs/testing/AIR-005-TEST-DESIGN.md) - Reference London TDD approach
- [core/src/config/mock_loader.rs](/workspaces/neural-data-platform/core/src/config/mock_loader.rs) - Mock pattern reference

---

## 15. London TDD Principles Checklist

| Principle | FE-001 Application | Status |
|-----------|-------------------|--------|
| Outside-in development | Start with deploy.sh integration, work to generators | Defined |
| Mock collaborators | MockConfigLoader, MockTimescaleDb | Patterns defined |
| Test behavior not implementation | Test SQL output, not internal state | Defined |
| Define contracts through tests | ConfigLoader, DDL Tool, Generator contracts | Defined |
| Isolate units through mocking | Each generator tested independently | Defined |
| Error path coverage | Tests for all failure modes | Required |
| Arrange-Act-Assert structure | All tests follow AAA | Required |
| Deterministic tests | No flaky tests allowed | Required |

---

## Appendix A: Sample Test File Structure

```
tools/ndp-gold-ddl/
├── Cargo.toml
├── src/
│   ├── main.rs
│   ├── lib.rs
│   ├── cli/
│   │   └── mod.rs
│   ├── generators/
│   │   ├── mod.rs
│   │   ├── continuous_aggregate.rs
│   │   ├── aligned_view.rs
│   │   ├── features.rs
│   │   └── events.rs
│   ├── validation/
│   │   ├── mod.rs
│   │   ├── config.rs
│   │   └── expressions.rs
│   └── mocks/                  # Test mocks
│       ├── mod.rs
│       └── timescale.rs
└── tests/
    ├── fixtures/
    │   ├── configs/
    │   │   ├── valid/
    │   │   └── invalid/
    │   ├── expected_sql/
    │   └── silver_data/
    ├── helpers/
    │   ├── mod.rs
    │   ├── fixtures.rs
    │   └── sql_compare.rs
    ├── cli_tests.rs            # Component tests
    ├── validation_tests.rs     # Component tests
    └── integration/            # Integration tests
        ├── mod.rs
        ├── continuous_aggregate_test.rs
        ├── aligned_view_test.rs
        └── deploy_idempotency_test.rs
```
