# DP-006: Silver Layer ETL - Test Strategy

**Feature**: dp-006 (Silver Layer Implementation)
**Phase**: Refinement
**Version**: 1.0
**Date**: 2026-01-10
**Author**: NDP Tester
**Status**: Draft

---

## Executive Summary

This document defines the comprehensive test strategy for dp-006, the Silver layer ETL implementation. The strategy covers unit, integration, and end-to-end testing with specific focus on the config-driven ETL approach, DuckDB operations, and memory-constrained Raspberry Pi 5 deployment.

### Key Constraints

| Constraint | Value | Impact on Testing |
|------------|-------|-------------------|
| Memory limit | 300MB peak | Performance tests must validate memory usage |
| ETL latency | < 60 seconds | Batch timing tests for hourly runs |
| Platform | Raspberry Pi 5 (ARM64) | Integration tests must run on ARM64 |
| Dependencies | DuckDB + TimescaleDB | Docker containers for integration tests |

---

## 1. Testing Pyramid

```
                          +-------------------+
                         /                     \
                        /    END-TO-END (5%)    \
                       /   Full ETL Pipeline     \
                      /   Bronze -> Silver        \
                     /   Real data, real infra     \
                    +---------------------------+
                   /                             \
                  /     INTEGRATION (25%)         \
                 /   DuckDB Parquet reads          \
                /    TimescaleDB writes             \
               /     etcd config loading             \
              /      Docker test containers           \
             +-------------------------------------+
            /                                       \
           /           UNIT TESTS (70%)              \
          /   Config parsing, SQL generation          \
         /    DQ rule expressions, transforms          \
        /     Type definitions, validation              \
       /      No external dependencies                   \
      +---------------------------------------------------+
```

### Test Distribution

| Level | Percentage | Count (Est.) | Execution Time | Dependencies |
|-------|------------|--------------|----------------|--------------|
| Unit | 70% | ~100 tests | < 10 seconds | None |
| Integration | 25% | ~35 tests | < 2 minutes | Docker |
| End-to-End | 5% | ~7 tests | < 5 minutes | Full stack |

---

## 2. Unit Test Categories

### 2.1 Config Parsing Tests

**Location**: `apps/silver-etl/src/config_tests.rs` or inline `#[cfg(test)]`

| Test Case | Input | Expected | Mocking Required |
|-----------|-------|----------|------------------|
| Valid YAML parses | Complete silver_etl section | SilverEtlConfig struct | None |
| Missing required field | YAML without target_table | Clear error message | None |
| Invalid transform type | Unknown transform name | Validation error | None |
| Empty field_mappings | Empty array | Warning, allowed | None |
| Nested JSON path | `raw_payload.main.temp` | Parsed correctly | None |
| dq_rules parsing | All 11 rule types | DqRule enum variants | None |
| dq_output defaults | Partial dq_output | Correct defaults | None |

**Test Template**:
```rust
#[cfg(test)]
mod config_tests {
    use super::*;

    #[test]
    fn test_parse_valid_silver_etl_config() {
        let yaml = r#"
            silver_etl:
              enabled: true
              target_table: silver.air_quality_observations
              field_mappings:
                - source_path: raw_payload.pm02
                  target_column: pm25
                  type: double_precision
        "#;

        let config: SilverEtlConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.enabled);
        assert_eq!(config.target_table, "silver.air_quality_observations");
        assert_eq!(config.field_mappings.len(), 1);
    }

    #[test]
    fn test_missing_required_field_returns_error() {
        let yaml = r#"
            silver_etl:
              enabled: true
              # target_table missing
        "#;

        let result: Result<SilverEtlConfig, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("target_table"), "Error should mention missing field");
    }
}
```

**Coverage Target**: 90%

---

### 2.2 SQL Generation Tests

**Location**: `apps/silver-etl/src/sql_gen_tests.rs`

| Test Case | Input | Expected SQL Fragment | Mocking Required |
|-----------|-------|----------------------|------------------|
| Simple field mapping | source_path -> target_column | `SELECT ... AS target_column` | None |
| JSON extraction | `raw_payload.main.temp` | `json_extract(raw_payload, '$.main.temp')` | None |
| Unit conversion | Kelvin to Celsius | `(field - 273.15) AS target` | None |
| Multiple mappings | 5 field_mappings | Valid SELECT with all columns | None |
| Timestamp transform | microseconds_to_timestamp | `to_timestamp(ts / 1000000)` | None |
| Identity fields | ndp_id passthrough | `ndp_id` | None |
| Deduplication | upsert strategy | `ON CONFLICT ... DO UPDATE` | None |
| Incremental WHERE | watermark_column | `WHERE ts > $watermark` | None |

**Test Template**:
```rust
#[cfg(test)]
mod sql_gen_tests {
    use super::*;

    #[test]
    fn test_unit_conversion_generates_correct_sql() {
        let mapping = FieldMapping {
            source_path: "raw_payload.main.temp".into(),
            target_column: "temperature_c".into(),
            field_type: "double_precision".into(),
            transform: Some(Transform::UnitConversion {
                from: "kelvin".into(),
                to: "celsius".into(),
                formula: LinearFormula { scale: 1.0, offset: -273.15 },
            }),
            ..Default::default()
        };

        let sql = generate_field_sql(&mapping);
        assert!(sql.contains("- 273.15"));
        assert!(sql.contains("AS temperature_c"));
    }

    #[test]
    fn test_json_path_extraction() {
        let mapping = FieldMapping {
            source_path: "raw_payload.wind.speed".into(),
            target_column: "wind_speed".into(),
            ..Default::default()
        };

        let sql = generate_field_sql(&mapping);
        // DuckDB JSON syntax
        assert!(sql.contains("raw_payload->>'$.wind.speed'")
            || sql.contains("json_extract_string"));
    }
}
```

**Coverage Target**: 90%

---

### 2.3 DQ Rule Tests

**Location**: `apps/silver-etl/src/dq_tests.rs`

| Rule Type | Test Cases | Expected SQL/Behavior |
|-----------|------------|----------------------|
| range_check | value < min, value > max, value in range | CASE WHEN with bounds |
| null_check | NULL value, non-NULL value | CASE WHEN IS NULL |
| enum_check | valid value, invalid value, case sensitivity | IN clause |
| pattern_check | matching regex, non-matching | `~` operator |
| freshness_check | stale, future, valid | INTERVAL comparison |
| rate_of_change | exceeded, within bounds | LAG window function |
| cross_field_check | valid relationship, invalid | Boolean expression |
| conditional_check | condition true + rule fails | Nested CASE |
| completeness_check | below threshold, above | COUNT aggregate |
| cardinality_check | count out of range, in range | COUNT DISTINCT |

**Test Template**:
```rust
#[cfg(test)]
mod dq_rule_tests {
    use super::*;

    #[test]
    fn test_range_check_generates_correct_sql() {
        let rule = DqRule::RangeCheck {
            field: "pm25".into(),
            min: Some(0.0),
            max: Some(1000.0),
            action: DqAction::Flag,
            clamp_to_bounds: false,
        };

        let sql = generate_dq_check_sql(&rule);
        assert!(sql.contains("pm25 < 0.0 OR pm25 > 1000.0"));
        assert!(sql.contains("'range_check:pm25:out_of_bounds'"));
    }

    #[test]
    fn test_clamp_action_generates_least_greatest() {
        let rule = DqRule::RangeCheck {
            field: "humidity_pct".into(),
            min: Some(0.0),
            max: Some(100.0),
            action: DqAction::Clamp,
            clamp_to_bounds: true,
        };

        let sql = generate_dq_clamp_sql(&rule);
        assert!(sql.contains("LEAST(GREATEST(humidity_pct, 0.0), 100.0)"));
    }

    #[test]
    fn test_rate_of_change_requires_window_function() {
        let rule = DqRule::RateOfChange {
            field: "temperature_c".into(),
            max_change_per_minute: 2.0,
            partition_by: vec!["ndp_id".into()],
            action: DqAction::Flag,
        };

        let sql = generate_dq_check_sql(&rule);
        assert!(sql.contains("LAG(temperature_c)"));
        assert!(sql.contains("PARTITION BY ndp_id"));
        assert!(sql.contains("ORDER BY"));
    }
}
```

**DQ Action Tests**:

| Action | Test Case | Assertion |
|--------|-----------|-----------|
| flag | Value fails range_check | Original value kept, flag added |
| reject | Value fails null_check | Value set to NULL, flag added |
| clamp | Value > max | Value clamped to max, flag added |
| drop | Row-level catastrophic | Row excluded from INSERT |

**Coverage Target**: 95% (critical component)

---

### 2.4 Transform Logic Tests

**Location**: `apps/silver-etl/src/transform_tests.rs`

| Transform Type | Test Cases | Mocking Required |
|----------------|------------|------------------|
| unit_conversion | Kelvin->Celsius, m/s->km/h, hPa->Pa | None |
| expression | Custom SQL expression | None |
| lookup | Value mapping table | None |
| json_extract | Nested paths, arrays | None |
| timestamp | All 4 formats | None |
| computed | lead_time_hours calculation | None |

**Test Template**:
```rust
#[cfg(test)]
mod transform_tests {
    use super::*;

    #[test]
    fn test_kelvin_to_celsius_conversion() {
        let transform = Transform::UnitConversion {
            from: "kelvin".into(),
            to: "celsius".into(),
            formula: LinearFormula { scale: 1.0, offset: -273.15 },
        };

        // Apply to test value
        let kelvin = 293.15; // 20 C
        let celsius = apply_transform(kelvin, &transform);
        assert!((celsius - 20.0).abs() < 0.01);
    }

    #[test]
    fn test_wind_speed_ms_to_kmh() {
        let transform = Transform::UnitConversion {
            from: "m_s".into(),
            to: "km_h".into(),
            formula: LinearFormula { scale: 3.6, offset: 0.0 },
        };

        let ms = 10.0;
        let kmh = apply_transform(ms, &transform);
        assert!((kmh - 36.0).abs() < 0.01);
    }
}
```

**Coverage Target**: 85%

---

## 3. Integration Test Categories

### 3.1 DuckDB Parquet Tests

**Location**: `tests/integration/duckdb_parquet.rs`

**Prerequisites**: Test Parquet files in `tests/fixtures/parquet/`

| Test Case | Setup | Assertion | Dependencies |
|-----------|-------|-----------|--------------|
| Read single Parquet file | Create fixture file | Rows returned correctly | DuckDB in-memory |
| Read glob pattern | Multiple files | All rows combined | DuckDB in-memory |
| Read with schema projection | Subset of columns | Only selected columns | DuckDB in-memory |
| Read partitioned data | year=/month=/day= structure | Correct partition filtering | DuckDB in-memory |
| Handle empty file | Empty Parquet | Zero rows, no error | DuckDB in-memory |
| Handle missing file | Non-existent path | Clear error message | DuckDB in-memory |

**Test Template**:
```rust
#[tokio::test]
#[ignore] // Requires fixtures
async fn test_duckdb_reads_bronze_parquet() {
    // Arrange
    let fixture_path = "tests/fixtures/parquet/air-quality/2026/01/10/data.parquet";
    let conn = duckdb::Connection::open_in_memory().unwrap();

    // Act
    let sql = format!(
        "SELECT * FROM read_parquet('{}') LIMIT 10",
        fixture_path
    );
    let mut stmt = conn.prepare(&sql).unwrap();
    let rows: Vec<_> = stmt.query_map([], |row| {
        Ok(row.get::<_, i64>(0).unwrap())
    }).unwrap().collect();

    // Assert
    assert!(!rows.is_empty(), "Should read rows from parquet");
}
```

**Fixture Requirements**:
```
tests/fixtures/parquet/
├── air-quality/
│   └── 2026/01/10/data.parquet      # Valid sensor data
├── outdoor-weather/
│   └── 2026/01/10/data.parquet      # OWM weather data
├── invalid/
│   └── corrupt.parquet              # For error testing
└── empty/
    └── empty.parquet                 # Zero rows
```

---

### 3.2 DuckDB SQL Execution Tests

**Location**: `tests/integration/duckdb_sql.rs`

| Test Case | Generated SQL | Assertion |
|-----------|--------------|-----------|
| Simple SELECT executes | field_mappings SQL | Results match expectations |
| JSON extraction works | JSON path expressions | Values extracted correctly |
| DQ CASE expressions | dq_rules SQL | Flags populated correctly |
| Window functions work | rate_of_change SQL | LAG values computed |
| Type coercion | Mixed types | Correct PostgreSQL types |
| ARRAY aggregation | dq_flags combining | Array contains all flags |

**Test Template**:
```rust
#[tokio::test]
async fn test_generated_sql_executes_correctly() {
    // Arrange
    let conn = duckdb::Connection::open_in_memory().unwrap();

    // Create test table with Bronze-like schema
    conn.execute(
        "CREATE TABLE bronze (
            timestamp BIGINT,
            ndp_id TEXT,
            raw_payload JSON
        )",
        [],
    ).unwrap();

    // Insert test data
    conn.execute(
        "INSERT INTO bronze VALUES (?, ?, ?)",
        [
            &1704931200000000i64,
            &"sensor-1",
            &r#"{"pm02": 25.5, "atmp": 22.0}"#,
        ],
    ).unwrap();

    // Act: Execute generated SQL
    let config = load_test_config();
    let sql = generate_etl_sql(&config);
    let result = conn.query_row(&sql, [], |row| {
        Ok((
            row.get::<_, f64>("pm25")?,
            row.get::<_, f64>("temperature_c")?,
        ))
    });

    // Assert
    let (pm25, temp) = result.unwrap();
    assert!((pm25 - 25.5).abs() < 0.01);
    assert!((temp - 22.0).abs() < 0.01);
}
```

---

### 3.3 PostgreSQL/TimescaleDB Write Tests

**Location**: `tests/integration/timescaledb_writes.rs`

**Prerequisites**: Docker TimescaleDB container

| Test Case | Setup | Assertion | Dependencies |
|-----------|-------|-----------|--------------|
| INSERT via DuckDB postgres ext | Test table | Rows in TimescaleDB | Docker TimescaleDB |
| Upsert (ON CONFLICT) | Duplicate key | Updated row, not duplicate | Docker TimescaleDB |
| Hypertable insert | create_hypertable() table | Data in chunks | Docker TimescaleDB |
| dq_flags TEXT[] column | Array values | Queryable array | Docker TimescaleDB |
| Batch insert performance | 1000 rows | < 5 seconds | Docker TimescaleDB |
| Connection failure handling | Stop container | Clear error, retry | Docker TimescaleDB |

**Docker Setup**:
```yaml
# tests/docker-compose.test.yml
version: '3.8'
services:
  timescaledb-test:
    image: timescale/timescaledb:latest-pg15
    environment:
      POSTGRES_USER: test
      POSTGRES_PASSWORD: test
      POSTGRES_DB: ndp_test
    ports:
      - "5433:5432"
    tmpfs:
      - /var/lib/postgresql/data
```

**Test Template**:
```rust
#[tokio::test]
#[ignore] // Requires Docker
async fn test_duckdb_postgres_extension_writes() {
    // Arrange
    let duckdb_conn = duckdb::Connection::open_in_memory().unwrap();

    // Load postgres extension
    duckdb_conn.execute("INSTALL postgres", []).unwrap();
    duckdb_conn.execute("LOAD postgres", []).unwrap();

    // Attach TimescaleDB
    duckdb_conn.execute(
        "ATTACH 'host=localhost port=5433 dbname=ndp_test user=test password=test' AS pg (TYPE POSTGRES)",
        [],
    ).unwrap();

    // Create test table in TimescaleDB
    duckdb_conn.execute(
        "CREATE TABLE IF NOT EXISTS pg.silver.test_observations (
            observation_time TIMESTAMPTZ NOT NULL,
            ndp_id TEXT NOT NULL,
            pm25 DOUBLE PRECISION,
            dq_flags TEXT[]
        )",
        [],
    ).unwrap();

    // Act
    duckdb_conn.execute(
        "INSERT INTO pg.silver.test_observations VALUES (?, ?, ?, ?)",
        [&"2026-01-10T12:00:00Z", &"sensor-1", &25.5, &"{}"],
    ).unwrap();

    // Assert
    let count: i64 = duckdb_conn.query_row(
        "SELECT COUNT(*) FROM pg.silver.test_observations",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(count, 1);
}
```

---

### 3.4 Config Loading Tests

**Location**: `tests/integration/config_loading.rs`

| Test Case | Setup | Assertion | Dependencies |
|-----------|-------|-----------|--------------|
| Load from etcd | Put config in etcd | Config struct populated | Docker etcd |
| Watch config change | Update config in etcd | Callback triggered | Docker etcd |
| Fallback to YAML | etcd unavailable | Load from file | YAML files |
| Merge base + overlay | Environment-specific | Correct precedence | YAML files |
| Invalid config in etcd | Malformed YAML | Clear error, continue | Docker etcd |

---

## 4. End-to-End Test Categories

**Location**: `tests/e2e/`

### 4.1 Happy Path E2E

| Scenario | Setup | Execution | Validation |
|----------|-------|-----------|------------|
| Full ETL run | Bronze Parquet + TimescaleDB | Run silver-etl binary | Silver tables populated |
| Multi-stream batch | All 7 streams | Hourly batch | All 4 Silver tables have data |
| Incremental run | Previous watermark | Run again | Only new data processed |

### 4.2 Error Recovery E2E

| Scenario | Induced Failure | Expected Behavior |
|----------|-----------------|-------------------|
| TimescaleDB down | Stop container mid-ETL | Retry, eventual success |
| Corrupt Parquet | Invalid file in path | Skip file, log error, continue |
| OOM simulation | Large batch + low limit | Graceful failure, no crash |

### 4.3 Performance E2E

| Scenario | Input Size | Pass Criteria |
|----------|------------|---------------|
| Hourly batch | 1 hour of 7 streams | < 60 seconds |
| Memory limit | Full ETL | < 300MB peak RSS |
| Backfill 24h | 24 hours of data | Completes without OOM |

---

## 5. Mocking Strategy

### 5.1 What to Mock

| Component | Mock Approach | Why |
|-----------|---------------|-----|
| DuckDB | In-memory database | Fast, no file I/O |
| TimescaleDB | Docker container | Need real SQL execution |
| etcd | Docker container OR mock client | Config loading |
| Parquet files | Fixture files | Controlled test data |
| System time | `tokio::time::pause()` | Deterministic freshness checks |

### 5.2 Mock Implementation Pattern

```rust
// Using mockall for trait mocking
use mockall::{automock, predicate::*};

#[automock]
#[async_trait]
pub trait ConfigStore: Send + Sync {
    async fn get_stream_config(&self, stream_id: &str) -> Result<StreamConfig, ConfigError>;
    async fn list_streams(&self) -> Result<Vec<String>, ConfigError>;
    async fn watch_changes(&self) -> Result<ConfigWatcher, ConfigError>;
}

// Test usage
#[tokio::test]
async fn test_etl_with_mock_config() {
    let mut mock_config = MockConfigStore::new();
    mock_config
        .expect_get_stream_config()
        .with(eq("air-quality"))
        .times(1)
        .returning(|_| Ok(test_stream_config()));

    let etl = SilverEtl::new(Box::new(mock_config), ...);
    let result = etl.run_batch().await;
    assert!(result.is_ok());
}
```

### 5.3 DuckDB In-Memory Strategy

```rust
fn create_test_duckdb() -> duckdb::Connection {
    let conn = duckdb::Connection::open_in_memory().unwrap();

    // Create Bronze-like schema
    conn.execute(
        "CREATE TABLE bronze (
            timestamp BIGINT,
            source_id TEXT,
            ndp_id TEXT,
            context JSON,
            raw_payload JSON
        )",
        [],
    ).unwrap();

    conn
}

fn insert_test_data(conn: &duckdb::Connection, data: &[TestRecord]) {
    for record in data {
        conn.execute(
            "INSERT INTO bronze VALUES (?, ?, ?, ?, ?)",
            [
                &record.timestamp,
                &record.source_id,
                &record.ndp_id,
                &record.context,
                &record.raw_payload,
            ],
        ).unwrap();
    }
}
```

---

## 6. Test Data Requirements

### 6.1 Fixture Files

```
tests/fixtures/
├── parquet/
│   ├── air-quality/
│   │   ├── valid/           # Normal sensor readings
│   │   ├── out-of-range/    # Values outside DQ bounds
│   │   ├── nulls/           # NULL field testing
│   │   ├── duplicates/      # Deduplication testing
│   │   └── late-arrivals/   # Watermark testing
│   ├── outdoor-weather/
│   │   ├── valid/
│   │   └── unit-conversion/ # Kelvin, m/s values
│   ├── nws-observations/
│   │   └── valid/
│   ├── nws-forecast-hourly/
│   │   └── valid/
│   └── outdoor-air-quality/
│       └── valid/
├── configs/
│   ├── valid/               # Complete stream configs
│   ├── invalid/             # Malformed YAML
│   └── edge-cases/          # Unusual but valid configs
└── expected/
    ├── sql/                 # Expected generated SQL
    └── results/             # Expected output data
```

### 6.2 Test Data Characteristics

| Category | Characteristics | Test Purpose |
|----------|-----------------|--------------|
| Valid data | All fields present, in range | Happy path |
| Out-of-range | pm25 = 1500, humidity = 110% | DQ range_check |
| NULL values | Missing pm25, missing timestamp | DQ null_check |
| Duplicates | Same (observation_time, ndp_id) | Deduplication |
| Late arrivals | timestamp < watermark - 1 hour | Incremental lag handling |
| Future timestamps | timestamp > now + 1 hour | DQ freshness_check |
| Rate spikes | pm25 jumps 500 in 1 minute | DQ rate_of_change |
| Cross-field invalid | dew_point > temperature | DQ cross_field_check |

### 6.3 Fixture Generation Script

```rust
// tests/fixtures/generate.rs
use parquet::file::writer::SerializedFileWriter;
use parquet::schema::parser::parse_message_type;

fn generate_air_quality_fixtures() {
    let schema = parse_message_type(
        "message bronze {
            required int64 timestamp;
            required binary source_id (UTF8);
            required binary ndp_id (UTF8);
            optional binary context (UTF8);
            required binary raw_payload (UTF8);
        }"
    ).unwrap();

    // Generate valid data
    let valid_records = vec![
        BronzeRecord {
            timestamp: 1704931200000000, // 2026-01-10T12:00:00
            source_id: "air-quality".into(),
            ndp_id: "sensor-office".into(),
            context: None,
            raw_payload: r#"{"pm02": 25.5, "rco2": 800, "atmp": 22.0, "rhum": 45.0}"#.into(),
        },
        // ... more records
    ];

    write_parquet("tests/fixtures/parquet/air-quality/valid/data.parquet", valid_records);

    // Generate out-of-range data
    let out_of_range_records = vec![
        BronzeRecord {
            raw_payload: r#"{"pm02": 1500.0}"#.into(), // > 1000 max
            ..valid_records[0].clone()
        },
    ];

    write_parquet("tests/fixtures/parquet/air-quality/out-of-range/data.parquet", out_of_range_records);
}
```

---

## 7. Performance Test Methodology

### 7.1 Performance Test Suite

| Test | Input | Target | Measurement |
|------|-------|--------|-------------|
| Hourly batch | 1 hour of 7 streams (~10K rows) | < 60 seconds | Wall clock time |
| Memory limit | Full ETL run | < 300MB peak | RSS via `/proc/self/status` |
| Backfill 24h | 24 hours (~240K rows) | No OOM | Completion status |
| Concurrent streams | 7 streams parallel | Linear scaling | Per-stream timing |
| Large batch | 100K rows single stream | < 120 seconds | Wall clock time |

### 7.2 Memory Profiling

```rust
#[cfg(test)]
mod perf_tests {
    use std::fs;

    fn get_memory_usage_kb() -> Option<usize> {
        let status = fs::read_to_string("/proc/self/status").ok()?;
        for line in status.lines() {
            if line.starts_with("VmRSS:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                return parts.get(1)?.parse().ok();
            }
        }
        None
    }

    #[tokio::test]
    #[ignore] // Performance test
    async fn test_memory_usage_under_limit() {
        let initial_mem = get_memory_usage_kb().unwrap();

        // Run full ETL
        let etl = SilverEtl::new(...).await;
        etl.run_batch().await.unwrap();

        let peak_mem = get_memory_usage_kb().unwrap();
        let delta_mb = (peak_mem - initial_mem) / 1024;

        assert!(
            delta_mb < 300,
            "Memory usage exceeded 300MB limit: {} MB",
            delta_mb
        );
    }
}
```

### 7.3 Timing Tests

```rust
#[tokio::test]
#[ignore] // Performance test
async fn test_hourly_batch_under_60_seconds() {
    // Setup: Load 1 hour of test data for all streams
    setup_test_data_1_hour().await;

    let start = std::time::Instant::now();

    // Run ETL
    let etl = SilverEtl::new(...).await;
    etl.run_batch().await.unwrap();

    let elapsed = start.elapsed();

    assert!(
        elapsed.as_secs() < 60,
        "ETL took {} seconds, exceeds 60s limit",
        elapsed.as_secs()
    );
}
```

---

## 8. CI/CD Integration

### 8.1 Test Stages

```yaml
# .github/workflows/silver-etl-tests.yml
name: Silver ETL Tests

on:
  push:
    paths:
      - 'apps/silver-etl/**'
      - 'core/src/config/silver_etl.rs'
      - 'core/src/config/dq_rules.rs'

jobs:
  unit-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Run unit tests
        run: cargo test -p silver-etl --lib

  integration-tests:
    runs-on: ubuntu-latest
    services:
      timescaledb:
        image: timescale/timescaledb:latest-pg15
        env:
          POSTGRES_USER: test
          POSTGRES_PASSWORD: test
          POSTGRES_DB: ndp_test
        ports:
          - 5433:5432
      etcd:
        image: quay.io/coreos/etcd:v3.5.9
        ports:
          - 2379:2379
    steps:
      - uses: actions/checkout@v4
      - name: Run integration tests
        run: cargo test -p silver-etl --test '*' -- --ignored
        env:
          TIMESCALEDB_URL: postgres://test:test@localhost:5433/ndp_test
          ETCD_ENDPOINTS: http://localhost:2379

  performance-tests:
    runs-on: ubuntu-latest
    if: github.ref == 'refs/heads/main'
    steps:
      - uses: actions/checkout@v4
      - name: Run performance tests
        run: cargo test -p silver-etl perf_ -- --ignored --test-threads=1
```

### 8.2 Test Tags

```rust
// Tag tests for selective execution
#[test]
fn test_config_parsing() { ... }  // Always runs

#[test]
#[ignore]  // Requires Docker
fn test_timescaledb_write() { ... }

#[test]
#[ignore]  // Performance test
fn test_memory_usage() { ... }
```

### 8.3 Coverage Requirements

| Component | Minimum Coverage | Enforcement |
|-----------|-----------------|-------------|
| Config parsing | 90% | CI fail |
| SQL generation | 90% | CI fail |
| DQ rules | 95% | CI fail |
| Transform logic | 85% | CI warn |
| Integration | 70% | CI warn |

---

## 9. Test Checklist

### 9.1 Before Marking Tests Complete

- [ ] Unit tests for all config types (serde roundtrip)
- [ ] Unit tests for all SQL generation paths
- [ ] Unit tests for all 11 DQ rule types
- [ ] Unit tests for all 4 DQ actions
- [ ] Unit tests for all transform types
- [ ] Integration tests for DuckDB Parquet reads
- [ ] Integration tests for TimescaleDB writes
- [ ] Integration tests for etcd config loading
- [ ] E2E test for full ETL pipeline
- [ ] Performance test for < 60s hourly batch
- [ ] Performance test for < 300MB memory
- [ ] Test fixtures for all stream types
- [ ] Test fixtures for all error cases
- [ ] CI workflow configured
- [ ] Coverage thresholds set

### 9.2 Test Naming Convention

```rust
// Pattern: test_{component}_{scenario}_{expected}
#[test]
fn test_range_check_value_exceeds_max_returns_flag() { ... }

#[test]
fn test_sql_gen_kelvin_to_celsius_includes_offset() { ... }

#[test]
fn test_etl_missing_parquet_logs_error_continues() { ... }
```

---

## 10. Test Execution Commands

### 10.1 Local Development

```bash
# Run all unit tests (fast)
cargo test -p silver-etl --lib

# Run specific test module
cargo test -p silver-etl config_tests

# Run with output
cargo test -p silver-etl -- --nocapture

# Run single test
cargo test -p silver-etl test_range_check_value_exceeds_max

# Run integration tests (requires Docker)
docker-compose -f tests/docker-compose.test.yml up -d
cargo test -p silver-etl --test '*' -- --ignored

# Run performance tests
cargo test -p silver-etl perf_ -- --ignored --test-threads=1

# Coverage report
cargo tarpaulin -p silver-etl --out Html
```

### 10.2 CI Commands

```bash
# Unit tests (no dependencies)
cargo test -p silver-etl --lib -- --test-threads=4

# Integration tests (Docker running)
cargo test -p silver-etl --test '*' -- --ignored --test-threads=2

# Performance tests (single-threaded for accurate timing)
cargo test -p silver-etl perf_ -- --ignored --test-threads=1
```

---

## 11. Risk Mitigation

### 11.1 Testing Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| DuckDB postgres ext fails on ARM64 | Medium | High | Fallback test with tokio-postgres |
| Test flakiness in CI | Medium | Medium | Retry logic, deterministic data |
| Fixture data drift | Low | Medium | Fixture generation scripts |
| Performance test variance | Medium | Low | Statistical thresholds, multiple runs |

### 11.2 Untested Areas (Documented)

| Area | Reason | Future Plan |
|------|--------|-------------|
| Actual Pi 5 performance | CI runs on x86 | Manual validation on deployment |
| Long-running stability | Time constraints | Soak tests in staging |
| Concurrent ETL instances | Single instance design | Document as limitation |

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-10 | NDP Tester | Initial test strategy |

---

## References

1. `product/features/dp-006/SCOPE.md` - Feature scope
2. `product/features/dp-006/specification/SPECIFICATION.md` - Requirements
3. `product/features/dp-006/architecture/DQ-FRAMEWORK-DESIGN.md` - DQ rules
4. `docs/testing/AIR-005-TEST-DESIGN.md` - Existing test patterns
5. `.claude/skills/mcp-tool-testing-pattern` - MCP testing pattern
