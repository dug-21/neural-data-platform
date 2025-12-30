# DP-002 Test Strategy

## Overview

This document defines the test strategy for DP-002: Online Data Dictionary & HomeAssistant Stream Preparation. The strategy follows NDP testing patterns established in AIR-005, emphasizing behavior verification, config-driven testing, and London School TDD principles where applicable.

---

## Test Philosophy

### Guiding Principles

1. **Config-Driven Testing**: Verify behavior comes from configuration, not hardcoded logic
2. **Behavior Verification**: Focus on WHAT the system does, not internal implementation
3. **Contract Testing**: Validate interfaces between components (etcd, TimescaleDB, Grafana)
4. **Non-Destructive**: Existing functionality (Bronze layer, dashboards) must remain unaffected
5. **Reproducible**: Tests produce consistent results across environments

### Test Pyramid for DP-002

```
                    ┌─────────────────┐
                    │   E2E Tests     │  (Deployment verification)
                    │   5 tests       │
                    ├─────────────────┤
                    │  Integration    │  (Component interactions)
                    │  20 tests       │
                    ├─────────────────┤
                    │   Unit Tests    │  (Individual functions)
                    │   40 tests      │
                    └─────────────────┘
```

---

## Test Categories

### 1. Unit Tests

**Focus**: Individual functions and modules in isolation

**Target Coverage**: 80%

**Components**:

| Component | Description | Est. Tests |
|-----------|-------------|------------|
| Entity Schema Parser | YAML parsing for entity_schemas | 10 |
| Pattern Matching | Glob/regex for HomeAssistant entities | 8 |
| Data Dictionary Sync | Transform functions (config -> SQL) | 12 |
| Schema Validator | Attribute type/unit validation | 10 |

**Test Patterns**:

```rust
// Entity Schema Parser Tests
#[test]
fn test_parse_entity_schema_valid_yaml() {
    // Arrange
    let yaml = r#"
        schema_name: airgradient
        description: AirGradient indoor sensors
        attributes:
          - name: pm25
            type: f64
            unit: "ug/m3"
    "#;

    // Act
    let result = parse_entity_schema(yaml);

    // Assert
    assert!(result.is_ok());
    let schema = result.unwrap();
    assert_eq!(schema.schema_name, "airgradient");
}

#[test]
fn test_parse_entity_schema_missing_required_field() {
    let yaml = r#"
        description: Missing schema_name
        attributes: []
    "#;
    let result = parse_entity_schema(yaml);
    assert!(matches!(result, Err(ParseError::MissingField("schema_name"))));
}
```

**Mocking Strategy**:
- No external dependencies in unit tests
- Pure function testing with sample data
- Use test fixtures from `core/tests/fixtures/`

---

### 2. Integration Tests

**Focus**: Component interactions and data flow

**Target Coverage**: 70%

**Test Scenarios**:

| Scenario | Components | Est. Tests |
|----------|------------|------------|
| etcd -> TimescaleDB sync | ConfigStore, PostgreSQL client | 6 |
| Schema validation pipeline | Parser, Validator, Dictionary | 4 |
| Grafana query execution | TimescaleDB, DuckDB plugin | 4 |
| Deploy script commands | Shell, Docker, etcd, TimescaleDB | 6 |

**Test Patterns**:

```rust
#[tokio::test]
#[ignore] // Requires TimescaleDB container
async fn test_sync_entity_schema_to_dictionary() {
    // Arrange
    let etcd = setup_test_etcd().await;
    let timescale = setup_test_timescaledb().await;

    // Store schema in etcd
    etcd.put("/streams/air-quality/entity_schemas/0",
             &sample_entity_schema()).await.unwrap();

    // Act
    let syncer = DataDictionarySyncer::new(etcd, timescale);
    syncer.sync_all().await.unwrap();

    // Assert
    let rows = timescale.query(
        "SELECT * FROM data_dictionary WHERE stream_id = 'air-quality'"
    ).await.unwrap();
    assert!(!rows.is_empty());
}
```

**Infrastructure Requirements**:
- Docker containers for TimescaleDB (testcontainers)
- Local etcd instance or mock
- Grafana API access for dashboard tests

**Marking Convention**:
- `#[ignore]` for tests requiring infrastructure
- Run with `cargo test -- --ignored` when infrastructure available

---

### 3. Data Validation Tests

**Focus**: Schema completeness and data consistency

**Target Coverage**: 90% of entity schemas

**Validation Categories**:

| Category | Description | Est. Tests |
|----------|-------------|------------|
| Schema Completeness | All required fields present | 6 streams |
| Type Consistency | Attribute types match Bronze data | 6 streams |
| Unknown Detection | Identify entities without schemas | 3 |
| Pattern Matching | HomeAssistant entity patterns | 5 |

**Test Approach**:

```rust
#[test]
fn test_airgradient_schema_matches_bronze_data() {
    // Load entity schema
    let schema = load_entity_schema("air-quality", "airgradient");

    // Load sample Bronze Parquet
    let parquet = read_bronze_sample("air-quality");

    // Verify all Parquet fields are in schema
    for field in parquet.fields() {
        if is_metric_field(field) {
            assert!(
                schema.has_attribute(field.name()),
                "Field '{}' in Bronze but not in entity_schema", field.name()
            );
        }
    }
}

#[test]
fn test_detect_unknown_homeassistant_entities() {
    let schemas = load_all_entity_schemas("homeassistant");
    let sample_entities = vec![
        "sensor.airgradient_co2",     // Should match
        "sensor.unknown_device",      // Should NOT match
        "binary_sensor.window_open",  // Should NOT match (no schema yet)
    ];

    let unknown = detect_unknown_entities(&sample_entities, &schemas);

    assert!(!unknown.contains(&"sensor.airgradient_co2"));
    assert!(unknown.contains(&"sensor.unknown_device"));
}
```

**Test Data Requirements**:
- Sample Bronze Parquet files for each stream
- Mock HomeAssistant state payloads
- Known good/bad entity examples

---

### 4. Deployment Tests

**Focus**: deploy.sh commands and containerized services

**Target Coverage**: All new deploy.sh commands

**Test Scenarios**:

| Command | Test Type | Est. Tests |
|---------|-----------|------------|
| `sync-dictionary` | Functional | 3 |
| TimescaleDB startup | Health check | 2 |
| Resource constraints | Performance | 2 |
| Rollback procedure | Recovery | 1 |

**Test Approach** (Shell-based):

```bash
#!/bin/bash
# test_sync_dictionary.sh

# Setup
./deploy.sh start
sleep 10

# Test sync-dictionary creates tables
./deploy.sh sync-dictionary
RESULT=$(docker exec timescaledb psql -U postgres -d ndp \
    -c "SELECT COUNT(*) FROM data_dictionary;" -t)

if [ "$RESULT" -gt 0 ]; then
    echo "PASS: sync-dictionary created entries"
else
    echo "FAIL: No entries in data_dictionary"
    exit 1
fi
```

**Pi Resource Validation**:

```bash
# Check TimescaleDB memory usage
./deploy.sh start
sleep 30
MEM=$(docker stats timescaledb --no-stream --format "{{.MemUsage}}" | cut -d'/' -f1)
# Verify < 512MB for Pi deployment
```

---

### 5. Regression Tests

**Focus**: Ensure existing functionality remains unaffected

**Target Coverage**: 100% of critical paths

**Regression Areas**:

| Area | What to Verify | Est. Tests |
|------|----------------|------------|
| Bronze Ingestion | All 6 streams continue ingesting | 6 |
| DuckDB Plugin | Parquet queries work without container | 3 |
| Existing Dashboards | No panel errors after changes | 4 |
| etcd Config Sync | Existing sync command unchanged | 2 |

**Test Approach**:

```rust
#[tokio::test]
#[ignore]
async fn test_bronze_ingestion_unchanged_after_duckdb_removal() {
    // Before: Count current Bronze files
    let before_count = count_parquet_files("/data/bronze/air-quality");

    // Trigger: Wait for new ingestion
    tokio::time::sleep(Duration::from_secs(60)).await;

    // After: Verify new files created
    let after_count = count_parquet_files("/data/bronze/air-quality");
    assert!(after_count > before_count, "Bronze ingestion should continue");
}

#[test]
fn test_grafana_duckdb_plugin_queries_parquet_directly() {
    // DuckDB plugin should NOT require container
    let query = "SELECT * FROM '/data/bronze/air-quality/*.parquet' LIMIT 10";

    // Execute via Grafana datasource API
    let result = grafana_query("duckdb", query);

    assert!(result.is_ok());
    assert!(!result.unwrap().rows.is_empty());
}
```

---

## Coverage Goals

### By Component

| Component | Unit | Integration | Overall Target |
|-----------|------|-------------|----------------|
| Entity Schema Parser | 90% | N/A | 90% |
| Pattern Matching | 85% | N/A | 85% |
| Data Dictionary Sync | 80% | 70% | 80% |
| Deploy Script | N/A | 80% | 80% |
| Grafana Dashboards | N/A | 60% | 60% |

### By Scope Item

| Scope Item | Test Coverage Target |
|------------|---------------------|
| 1. Remove DuckDB container | 90% (regression) |
| 2. Instantiate TimescaleDB | 80% |
| 3. Entity schemas (6 streams) | 95% |
| 4. Online Data Dictionary | 85% |
| 5. HomeAssistant stream config | 80% |
| 6. Deploy script extension | 75% |
| 7. Data Quality dashboard | 70% |
| 8. Documentation updates | 100% (review) |

---

## Critical Path Identification

### Must-Pass Tests (Blocking)

These tests MUST pass before deployment:

1. **TC-1.1**: DuckDB plugin queries work without container
2. **TC-2.1**: TimescaleDB container starts successfully
3. **TC-3.1**: Parser handles valid entity_schemas
4. **TC-4.1**: Query all streams from data dictionary
5. **TC-6.1**: sync-dictionary creates tables
6. **Regression**: All 6 Bronze streams continue ingesting

### Should-Pass Tests (Non-Blocking)

These tests should pass but won't block deployment:

1. Dashboard panel load times
2. Documentation accuracy checks
3. Pi memory optimization tests

---

## Test Data Requirements

### Sample Entity Schemas

Location: `core/tests/fixtures/entity_schemas/`

```yaml
# fixtures/entity_schemas/airgradient.yaml
schema_name: airgradient
description: AirGradient indoor air quality sensors
device_class: air_quality
attributes:
  - name: pm25
    type: f64
    unit: "ug/m3"
    description: PM2.5 particulate matter
  - name: rco2
    type: f64
    unit: ppm
    description: CO2 concentration
  - name: tvoc
    type: f64
    unit: index
    description: Total VOC index
```

### Mock TimescaleDB Data

```sql
-- fixtures/timescaledb/init.sql
CREATE TABLE IF NOT EXISTS data_dictionary (
    id SERIAL PRIMARY KEY,
    stream_id TEXT NOT NULL,
    schema_name TEXT NOT NULL,
    attribute_name TEXT NOT NULL,
    attribute_type TEXT NOT NULL,
    unit TEXT,
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(stream_id, schema_name, attribute_name)
);
```

### Test Bronze Parquet Data

- Use existing Bronze data from `/data/bronze/` on deployment
- For CI, generate synthetic Parquet with Arrow:

```rust
fn generate_test_parquet(stream_id: &str) -> Vec<u8> {
    let schema = Arc::new(Schema::new(vec![
        Field::new("timestamp", DataType::Timestamp(TimeUnit::Microsecond, None), false),
        Field::new("location_id", DataType::Utf8, false),
        Field::new("metric", DataType::Utf8, false),
        Field::new("value", DataType::Float64, false),
    ]));
    // ... generate sample data
}
```

---

## Test Environment

### Local Development

```bash
# Unit tests (no infrastructure)
cargo test --package platform-core

# Integration tests (requires Docker)
docker-compose -f deploy/pi/docker-compose.test.yml up -d
cargo test -- --ignored
```

### CI/CD Pipeline

```yaml
# .github/workflows/test.yml
jobs:
  unit-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo test --package platform-core

  integration-tests:
    runs-on: ubuntu-latest
    services:
      timescaledb:
        image: timescale/timescaledb:latest-pg16
        ports: ["5432:5432"]
      etcd:
        image: bitnami/etcd:3.5
        ports: ["2379:2379"]
    steps:
      - uses: actions/checkout@v4
      - run: cargo test -- --ignored
```

### Pi Deployment Testing

```bash
# On Raspberry Pi
./deploy/pi/deploy.sh update
./deploy/pi/test-deployment.sh  # New script for validation
```

---

## Test Execution Commands

### Run All Unit Tests

```bash
cargo test --package platform-core
cargo test --package air-quality-app
```

### Run Integration Tests

```bash
# Requires infrastructure
cargo test -- --ignored

# Specific component
cargo test data_dictionary -- --ignored
```

### Run Deployment Tests

```bash
cd deploy/pi
./test-deployment.sh
```

### Generate Coverage Report

```bash
cargo tarpaulin --out Html --output-dir target/coverage
```

---

## CI/CD Integration Recommendations

### Pre-Merge Checks

1. All unit tests pass
2. Linting (cargo clippy) passes
3. Format check (cargo fmt --check) passes
4. No security vulnerabilities (cargo audit)

### Post-Merge Checks

1. Integration tests with containerized dependencies
2. Deploy to staging environment
3. Run deployment validation suite
4. Performance benchmark comparison

### Deployment Gates

1. All blocking tests pass
2. TimescaleDB memory usage < 512MB
3. Bronze ingestion rate unchanged
4. No Grafana panel errors

---

## Related Documents

- [TEST_CASES.md](./TEST_CASES.md) - Detailed test case specifications
- [VALIDATION_CHECKLIST.md](./VALIDATION_CHECKLIST.md) - Manual deployment validation
- [AIR-005-TEST-DESIGN.md](/workspaces/neural-data-platform/docs/testing/AIR-005-TEST-DESIGN.md) - Reference patterns

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2025-12-30 | ndp-tester | Initial test strategy |

---

*This document defines the test strategy for DP-002. Detailed test cases follow in TEST_CASES.md.*
