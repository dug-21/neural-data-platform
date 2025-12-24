# AIR-007 Test Plan: NWS Gridpoints Weather Data Expansion

## Overview

This document outlines the comprehensive test strategy for AIR-007, following the Neural Data Platform's London School TDD approach with outside-in, mock-driven development.

## Test Strategy

### London School TDD Principles

1. **Outside-In Development**: Start testing from parser level, then integration with sources
2. **Mock-Driven**: Define contracts through mock expectations before implementation
3. **Behavior Verification**: Focus on HOW components collaborate
4. **Interaction Testing**: Verify message passing and coordination patterns
5. **Contract Definition**: Establish clear interfaces through expectations

### Test Categories

| Category | Focus | Tools |
|----------|-------|-------|
| **Unit Tests** | Parser logic, data extraction | cargo test |
| **Integration Tests** | Stream configuration, pipeline flow | cargo test --test |
| **Acceptance Tests** | Requirements validation | Manual + automated |
| **Manual Tests** | Real API verification | curl, browser |

## 1. Unit Tests

### 1.1 ColumnOrientedParser Tests

**Location**: `/workspaces/neural-data-platform/core/src/parsers/column_oriented.rs`

**Test Module**: `#[cfg(test)] mod tests`

#### Test Cases

##### Basic Parsing Tests

```rust
#[test]
fn test_parse_nws_gridpoints_temperature_single_column() {
    // ARRANGE: Create parser with temperature column config
    let parser = create_test_parser(vec![
        ColumnConfig {
            path: "properties.temperature".to_string(),
            metric_name: "temperature_c".to_string(),
            unit: Some("celsius".to_string()),
        }
    ]);

    let payload = fixture_nws_gridpoints_temperature();

    // ACT: Parse the payload
    let result = parser.parse(&payload, Utc::now());

    // ASSERT: Verify output structure
    assert!(result.is_ok());
    let points = result.unwrap();
    assert!(points.len() >= 2); // At least 2 time periods
    assert_eq!(points[0].value, 16.1);
    assert_eq!(points[0].tags.get("unit"), Some(&"celsius".to_string()));
}

#[test]
fn test_parse_iso8601_duration_pt1h() {
    // Test parsing PT1H (1 hour duration)
    let result = parse_iso8601_duration("2025-12-23T02:00:00+00:00/PT1H");
    assert!(result.is_ok());
    let (start, duration_hours) = result.unwrap();
    assert_eq!(duration_hours, 1);
}

#[test]
fn test_parse_iso8601_duration_pt3h() {
    // Test parsing PT3H (3 hour duration)
    let result = parse_iso8601_duration("2025-12-23T02:00:00+00:00/PT3H");
    assert!(result.is_ok());
    let (start, duration_hours) = result.unwrap();
    assert_eq!(duration_hours, 3);
}

#[test]
fn test_parse_iso8601_duration_pt6h() {
    // Test parsing PT6H (6 hour duration)
    let result = parse_iso8601_duration("2025-12-23T02:00:00+00:00/PT6H");
    assert!(result.is_ok());
    let (start, duration_hours) = result.unwrap();
    assert_eq!(duration_hours, 6);
}
```

##### Multi-Column Parsing Tests

```rust
#[test]
fn test_parse_multiple_columns_in_single_payload() {
    // ARRANGE: Parser with temperature, skyCover, visibility
    let columns = vec![
        ColumnConfig {
            path: "properties.temperature".to_string(),
            metric_name: "temperature_c".to_string(),
            unit: Some("celsius".to_string()),
        },
        ColumnConfig {
            path: "properties.skyCover".to_string(),
            metric_name: "cloud_cover_pct".to_string(),
            unit: Some("percent".to_string()),
        },
        ColumnConfig {
            path: "properties.visibility".to_string(),
            metric_name: "visibility_m".to_string(),
            unit: Some("meters".to_string()),
        }
    ];

    let parser = create_test_parser(columns);
    let payload = fixture_nws_gridpoints_full();

    // ACT
    let result = parser.parse(&payload, Utc::now());

    // ASSERT: Should have points for all columns
    assert!(result.is_ok());
    let points = result.unwrap();

    let temp_points: Vec<_> = points.iter()
        .filter(|p| p.tags.get("metric") == Some(&"temperature_c".to_string()))
        .collect();
    let cloud_points: Vec<_> = points.iter()
        .filter(|p| p.tags.get("metric") == Some(&"cloud_cover_pct".to_string()))
        .collect();

    assert!(!temp_points.is_empty());
    assert!(!cloud_points.is_empty());
}
```

##### Error Handling Tests

```rust
#[test]
fn test_parse_missing_column_graceful_skip() {
    // ARRANGE: Parser expecting a column that doesn't exist
    let parser = create_test_parser(vec![
        ColumnConfig {
            path: "properties.nonexistent".to_string(),
            metric_name: "missing_field".to_string(),
            unit: None,
        }
    ]);

    let payload = fixture_nws_gridpoints_temperature();

    // ACT
    let result = parser.parse(&payload, Utc::now());

    // ASSERT: Should succeed with empty results (graceful skip)
    assert!(result.is_ok());
    let points = result.unwrap();
    assert_eq!(points.len(), 0); // No points for missing column
}

#[test]
fn test_parse_invalid_value_graceful_skip() {
    // ARRANGE: Parser with payload containing null/invalid values
    let parser = create_test_parser(vec![
        ColumnConfig {
            path: "properties.temperature".to_string(),
            metric_name: "temperature_c".to_string(),
            unit: Some("celsius".to_string()),
        }
    ]);

    let payload = json!({
        "properties": {
            "temperature": {
                "values": [
                    {"validTime": "2025-12-23T02:00:00+00:00/PT1H", "value": null},
                    {"validTime": "2025-12-23T03:00:00+00:00/PT1H", "value": 15.0}
                ]
            }
        }
    });

    // ACT
    let result = parser.parse(&payload, Utc::now());

    // ASSERT: Should skip null, parse valid value
    assert!(result.is_ok());
    let points = result.unwrap();
    assert_eq!(points.len(), 1); // Only the valid value
    assert_eq!(points[0].value, 15.0);
}

#[test]
fn test_parse_malformed_json_returns_error() {
    // Test parser gracefully handles malformed JSON
    let parser = create_test_parser(vec![]);
    let payload = json!("not an object");

    let result = parser.parse(&payload, Utc::now());
    assert!(result.is_err());
}
```

##### Edge Case Tests

```rust
#[test]
fn test_parse_empty_values_array() {
    let parser = create_test_parser(vec![
        ColumnConfig {
            path: "properties.temperature".to_string(),
            metric_name: "temperature_c".to_string(),
            unit: Some("celsius".to_string()),
        }
    ]);

    let payload = json!({
        "properties": {
            "temperature": {
                "values": []
            }
        }
    });

    let result = parser.parse(&payload, Utc::now());
    assert!(result.is_ok());
    assert_eq!(result.unwrap().len(), 0);
}

#[test]
fn test_parse_timezone_handling() {
    // Verify timestamps are correctly parsed with timezone info
    // NWS uses +00:00 (UTC)
}
```

### 1.2 Test Fixtures

**Location**: `/workspaces/neural-data-platform/core/src/parsers/fixtures/`

Create test fixture files based on actual NWS API responses:

#### fixtures/nws_gridpoints_sample.json

```json
{
  "properties": {
    "updateTime": "2025-12-23T08:38:36+00:00",
    "validTimes": "2025-12-23T02:00:00+00:00/P7DT23H",
    "temperature": {
      "uom": "wmoUnit:degC",
      "values": [
        {"validTime": "2025-12-23T02:00:00+00:00/PT3H", "value": 16.1},
        {"validTime": "2025-12-23T05:00:00+00:00/PT1H", "value": 15.6}
      ]
    },
    "skyCover": {
      "uom": "wmoUnit:percent",
      "values": [
        {"validTime": "2025-12-23T02:00:00+00:00/PT1H", "value": 5},
        {"validTime": "2025-12-23T03:00:00+00:00/PT2H", "value": 10}
      ]
    }
  }
}
```

#### fixtures/nws_observations_sample.json

```json
{
  "properties": {
    "timestamp": "2025-12-23T12:53:00+00:00",
    "textDescription": "Clear",
    "temperature": {"unitCode": "wmoUnit:degC", "value": 19},
    "dewpoint": {"unitCode": "wmoUnit:degC", "value": 13},
    "windDirection": {"unitCode": "wmoUnit:degree_(angle)", "value": 70},
    "windSpeed": {"unitCode": "wmoUnit:km_h-1", "value": 5.544}
  }
}
```

## 2. Integration Tests

### 2.1 Stream Configuration Tests

**Location**: `/workspaces/neural-data-platform/tests/integration/nws_streams_test.rs`

```rust
#[tokio::test]
async fn test_nws_gridpoints_stream_config_loads() {
    // ARRANGE
    let config_path = "config/base/streams/nws-gridpoints-forecast/config.yaml";

    // ACT
    let config = load_stream_config_from_file(config_path).await;

    // ASSERT
    assert!(config.is_ok());
    let stream_config = config.unwrap();
    assert_eq!(stream_config.stream_id, "nws-gridpoints-forecast");
    assert_eq!(stream_config.parser.parser_type, ParserType::ColumnOriented);
    assert!(stream_config.source.poll_interval_seconds >= 3600); // At least 1 hour
}

#[tokio::test]
async fn test_nws_observations_stream_config_loads() {
    // ARRANGE
    let config_path = "config/base/streams/nws-station-observations/config.yaml";

    // ACT
    let config = load_stream_config_from_file(config_path).await;

    // ASSERT
    assert!(config.is_ok());
    let stream_config = config.unwrap();
    assert_eq!(stream_config.stream_id, "nws-station-observations");
    assert_eq!(stream_config.parser.parser_type, ParserType::FlatJson);
    assert!(stream_config.source.poll_interval_seconds >= 900); // At least 15 min
}

#[tokio::test]
async fn test_nws_gridpoints_has_all_required_columns() {
    let config = load_stream_config_from_file(
        "config/base/streams/nws-gridpoints-forecast/config.yaml"
    ).await.unwrap();

    // Verify parser config has expected columns
    if let Some(array_config) = config.parser.array_config {
        let columns = array_config.columns;

        // Check for key weather fields
        let has_temperature = columns.iter()
            .any(|c| c.path.contains("temperature"));
        let has_sky_cover = columns.iter()
            .any(|c| c.path.contains("skyCover"));

        assert!(has_temperature, "Missing temperature column");
        assert!(has_sky_cover, "Missing skyCover column");
    } else {
        panic!("Expected array_config for ColumnOriented parser");
    }
}
```

### 2.2 End-to-End Pipeline Tests

```rust
#[tokio::test]
#[ignore] // Requires infrastructure
async fn test_nws_gridpoints_full_pipeline() {
    // ARRANGE: Create components
    let parser = create_parser_from_config(load_parser_config("nws-gridpoints-forecast"));
    let source = create_http_source_with_parser(
        "https://api.weather.gov/gridpoints/JAX/79,49",
        parser
    );
    let storage = create_test_parquet_storage().await;

    // ACT: Fetch and store
    let points = source.fetch().await.unwrap();
    for point in &points {
        storage.write(vec![point.clone()]).await.unwrap();
    }

    // ASSERT: Verify data stored
    let query = QueryFilter::latest(10);
    let stored = storage.query(query).await.unwrap();

    assert!(!stored.is_empty());
    assert!(points.len() >= 40); // Should have many fields
}

#[tokio::test]
#[ignore] // Requires infrastructure
async fn test_nws_observations_full_pipeline() {
    // Similar to gridpoints test but for observations stream
}
```

### 2.3 Parser Factory Integration

```rust
#[tokio::test]
async fn test_column_oriented_parser_factory_creates_parser() {
    // Test that factory correctly creates ColumnOriented parser
    let parser_config = ParserConfig {
        parser_type: ParserType::ColumnOriented,
        location_id_field: "station_id".to_string(),
        default_location_id: Some("KJAX".to_string()),
        skip_fields: vec![],
        field_mappings: None,
        default_tags: HashMap::new(),
        array_config: Some(ArrayParserConfig {
            columns: vec![
                ColumnConfig {
                    path: "properties.temperature".to_string(),
                    metric_name: "temperature_c".to_string(),
                    unit: Some("celsius".to_string()),
                }
            ]
        }),
    };

    let parser = create_parser_from_config(parser_config);
    assert!(parser.is_ok());
    assert_eq!(parser.unwrap().name(), "column_oriented");
}
```

## 3. Acceptance Tests

### 3.1 Functional Requirements Validation

| Req ID | Requirement | Test | Pass Criteria |
|--------|-------------|------|---------------|
| FR-1 | NWS Gridpoints stream | `test_nws_gridpoints_full_pipeline` | 40+ fields captured from API |
| FR-2 | NWS Observations stream | `test_nws_observations_full_pipeline` | 15+ fields captured from API |
| FR-3 | Variable time intervals | `test_parse_iso8601_duration_*` | PT1H, PT3H, PT6H parsed correctly |
| FR-4 | Grafana dashboard | Manual test | Dashboard loads, queries succeed |
| FR-5 | ColumnOriented parser | `test_parse_multiple_columns_*` | Multiple columns extracted |

### 3.2 Non-Functional Requirements Validation

| Req ID | Requirement | Test | Pass Criteria |
|--------|-------------|------|---------------|
| NFR-1 | Data freshness | Manual timestamp check | Within 5 min of poll_interval |
| NFR-2 | API rate limiting | Manual monitoring | Stay under 1 req/sec limit |
| NFR-3 | Error resilience | `test_parse_*_graceful_skip` | Parser handles errors gracefully |
| NFR-4 | No regressions | `cargo test` | All existing tests pass |
| NFR-5 | User-Agent compliance | Manual API call | Proper User-Agent header sent |

## 4. Manual Testing Checklist

### 4.1 API Verification

- [ ] Verify NWS gridpoints API returns expected format
  ```bash
  curl -H "User-Agent: (neural-data-platform/1.0, contact@example.com)" \
    "https://api.weather.gov/gridpoints/JAX/79,49" | jq '.properties.temperature.values[:2]'
  ```

- [ ] Verify NWS observations API returns expected format
  ```bash
  curl -H "User-Agent: (neural-data-platform/1.0, contact@example.com)" \
    "https://api.weather.gov/stations/KJAX/observations/latest" | jq '.properties'
  ```

- [ ] Verify parser handles actual API responses (not just fixtures)

### 4.2 Dashboard Verification

- [ ] Grafana dashboard displays temperature data correctly
- [ ] Grafana dashboard displays precipitation data correctly
- [ ] Grafana dashboard displays wind data correctly
- [ ] Dashboard queries execute without errors
- [ ] Dashboard auto-refreshes at configured interval

### 4.3 Regression Testing

- [ ] Existing `nws-forecast-hourly` stream still works
- [ ] All existing unit tests pass
- [ ] All existing integration tests pass
- [ ] No performance regressions in data ingestion

## 5. Test Execution Commands

### 5.1 Unit Tests

```bash
# Run all parser tests
cargo test --package platform-core parser

# Run specific parser tests
cargo test --package platform-core column_oriented::tests

# Run with output
cargo test --package platform-core parser -- --nocapture

# Run single test
cargo test --package platform-core test_parse_nws_gridpoints_temperature -- --exact
```

### 5.2 Integration Tests

```bash
# Run all integration tests
cargo test --test '*' -- --ignored

# Run NWS stream tests specifically
cargo test --test nws_streams_test

# Run with verbose output
cargo test --test nws_streams_test -- --nocapture --test-threads=1
```

### 5.3 Coverage Analysis

```bash
# Generate coverage report (requires cargo-tarpaulin)
cargo tarpaulin --out Html --output-dir coverage

# View coverage report
open coverage/index.html
```

### 5.4 Manual API Testing

```bash
# Test gridpoints endpoint
curl -H "User-Agent: (ndp/1.0, contact@example.com)" \
  "https://api.weather.gov/gridpoints/JAX/79,49" | \
  jq '.properties | keys'

# Test observations endpoint
curl -H "User-Agent: (ndp/1.0, contact@example.com)" \
  "https://api.weather.gov/stations/KJAX/observations/latest" | \
  jq '.properties | keys'

# Count fields in gridpoints response
curl -s -H "User-Agent: (ndp/1.0, contact@example.com)" \
  "https://api.weather.gov/gridpoints/JAX/79,49" | \
  jq '.properties | keys | length'
```

## 6. Test Patterns and Best Practices

### 6.1 Arrange-Act-Assert Pattern

All tests follow the AAA pattern:

```rust
#[test]
fn test_example() {
    // ARRANGE: Set up test data and dependencies
    let parser = create_test_parser();
    let payload = fixture_data();

    // ACT: Execute the operation under test
    let result = parser.parse(&payload, Utc::now());

    // ASSERT: Verify the outcome
    assert!(result.is_ok());
}
```

### 6.2 Test Naming Convention

```
test_<component>_<scenario>_<expected_outcome>

Examples:
- test_parse_nws_gridpoints_temperature_returns_points
- test_parse_missing_column_graceful_skip
- test_nws_gridpoints_stream_config_loads
```

### 6.3 Test Isolation

- Each test creates fresh instances
- No shared mutable state between tests
- Tests can run in parallel safely

### 6.4 Mock Usage

```rust
// For external dependencies (HTTP, storage)
#[tokio::test]
async fn test_with_mock_http() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/gridpoints/JAX/79,49"))
        .respond_with(ResponseTemplate::new(200)
            .set_body_json(fixture_nws_gridpoints()))
        .mount(&mock_server)
        .await;

    // Test with mock
}
```

## 7. Coverage Targets

### 7.1 Target Coverage by Component

| Component | Target | Priority |
|-----------|--------|----------|
| ColumnOrientedParser | 90% | High |
| Parser factory | 85% | High |
| Stream configurations | 80% | High |
| Integration pipeline | 70% | Medium |

### 7.2 Critical Paths

Must have 100% coverage:
- ISO8601 duration parsing
- Column extraction logic
- Error handling for malformed data
- Null/missing value handling

## 8. Test Dependencies

### 8.1 Development Dependencies

```toml
[dev-dependencies]
tokio = { version = "1", features = ["test-util", "macros", "rt-multi-thread"] }
serde_json = "1.0"
wiremock = "0.6"  # For HTTP mocking
tempfile = "3.0"  # For temporary test files
```

### 8.2 External Dependencies

- **NWS API**: Internet connection for manual tests
- **Parquet storage**: For integration tests (can be mocked)
- **Grafana**: For dashboard verification (manual)

## 9. Continuous Integration

### 9.1 CI Pipeline Checks

```bash
# Pre-commit checks
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all

# Integration tests (optional in CI)
cargo test --test '*' -- --ignored
```

### 9.2 Quality Gates

- All unit tests must pass
- Code coverage ≥ 85%
- No clippy warnings
- Code formatted with rustfmt

## 10. Test Documentation

### 10.1 Test Fixture Documentation

Each fixture file should have a header comment:

```rust
//! NWS Gridpoints Sample Data
//!
//! Source: https://api.weather.gov/gridpoints/JAX/79,49
//! Date: 2025-12-23
//! Contains: temperature, skyCover, visibility
```

### 10.2 Test Case Documentation

Complex tests should include rationale:

```rust
/// Tests that the parser correctly handles ISO8601 durations with
/// variable interval lengths (PT1H, PT3H, PT6H) as used by NWS API.
///
/// Background: NWS uses different time intervals for different forecasts.
/// Short-term forecasts use PT1H (hourly), longer-range use PT3H or PT6H.
#[test]
fn test_parse_iso8601_variable_durations() {
    // ...
}
```

## 11. After Testing

### 11.1 Pattern Feedback

Use the `reflexion` skill to record whether testing patterns worked:

```javascript
mcp__agentdb__reflexion_store({
  session_id: "ndp-patterns",
  task: "Used London School TDD for AIR-007 parser testing",
  input: "Pattern: Outside-in TDD with fixtures",
  output: "Completed parser tests with 90% coverage",
  reward: 1.0,
  success: true,
  critique: "Fixture-based testing worked well for NWS data"
})
```

### 11.2 Pattern Storage

If new reusable testing patterns emerge, use `save-pattern`:

```javascript
mcp__agentdb__agentdb_pattern_store({
  taskType: "testing",
  approach: "ColumnOriented parser testing with ISO8601 fixtures",
  successRate: 1.0,
  tags: ["testing", "parser", "weather-data"]
})
```

## References

- **Testing Documentation**: `/workspaces/neural-data-platform/docs/testing/AIR-005-TEST-DESIGN.md`
- **Parser Integration Tests**: `/workspaces/neural-data-platform/core/tests/parser_integration_test.rs`
- **Integration Test README**: `/workspaces/neural-data-platform/tests/integration/README.md`
- **NWS API Documentation**: https://www.weather.gov/documentation/services-web-api
- **ISO8601 Duration Format**: https://en.wikipedia.org/wiki/ISO_8601#Durations

---

**Status**: Draft Test Plan
**Last Updated**: 2025-12-24
**Approved By**: Pending review
