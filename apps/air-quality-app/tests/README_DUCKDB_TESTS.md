# DuckDB Silver Layer Tests - DP-001

## Overview

This directory contains comprehensive tests for the DuckDB Silver Layer implementation using **London School TDD** (outside-in, mock-driven) approach.

## Test Files

### 1. `duckdb_views_test.rs` - Unit Tests

**Purpose**: Test SQL view logic in isolation using in-memory DuckDB.

**Test Categories**:
- **T-DB-003**: View creation and syntax validation
- **T-DB-004**: NULL handling in views
- **T-DB-005**: Range filtering logic (data quality rules)
- **T-DB-006**: Cross-stream JOIN correctness
- Schema validation tests
- Precision rounding tests

**Test Count**: 18 unit tests

**Execution Time**: < 1 second (fast, in-memory)

**London TDD Approach**:
- Tests verify **behavior** (SQL output), not implementation
- Mock Parquet data using in-memory tables
- Focus on **contract verification** (view schema, filter logic)
- Tests drive the SQL view design

### 2. `silver_layer_integration_test.rs` - Integration Tests

**Purpose**: Test end-to-end integration of DuckDB with Parquet files.

**Test Categories**:
- **T-DB-001**: Parquet file discovery and loading
- **T-DB-002**: Schema inference correctness
- **T-DB-007**: Query performance benchmarks
- Data quality integration tests
- Large dataset tests
- Error handling tests

**Test Count**: 20 integration tests

**Execution Time**: Varies (marked with `#[ignore]` for CI flexibility)

**London TDD Approach**:
- Tests verify **interactions** between DuckDB and Parquet
- Use real Parquet files (generated in tests)
- Mock-driven: Generate test data with known characteristics
- Performance benchmarks validate query execution time

## Prerequisites

### DuckDB C Library Installation

These tests require the DuckDB C library to be installed on the system:

#### macOS
```bash
brew install duckdb
```

#### Ubuntu/Debian
```bash
sudo apt-get install libduckdb-dev
```

#### Windows
Download from: https://github.com/duckdb/duckdb/releases

### Cargo Dependencies

Add to `Cargo.toml` dev-dependencies:
```toml
[dev-dependencies]
duckdb = "1.4"
parquet = "57"
tempfile = "3.8"
```

## Running Tests

### Run Unit Tests (Fast)
```bash
# All unit tests (< 1 second)
cargo test --package air-quality-app --test duckdb_views_test

# Specific test
cargo test --package air-quality-app --test duckdb_views_test test_pm25_range_filter

# With output
cargo test --package air-quality-app --test duckdb_views_test -- --nocapture
```

### Run Integration Tests (Slower)
```bash
# All integration tests (may take minutes)
cargo test --package air-quality-app --test silver_layer_integration_test -- --ignored

# Specific integration test
cargo test --package air-quality-app --test silver_layer_integration_test test_parquet_file_loading -- --ignored

# Performance benchmarks only
cargo test --package air-quality-app --test silver_layer_integration_test bench_ -- --ignored
```

### Run All DP-001 Tests
```bash
cargo test --package air-quality-app duckdb
cargo test --package air-quality-app silver_layer --ignored
```

## Test Structure

### London TDD Pattern

All tests follow the **Arrange-Act-Assert** structure:

```rust
#[test]
fn test_pm25_range_filter() {
    // Arrange: Set up test data and environment
    let conn = setup_test_db();
    create_mock_parquet_tables(&conn).unwrap();
    insert_indoor_air_test_data(&conn).unwrap();
    create_silver_views(&conn).unwrap();

    // Act: Execute the behavior under test
    let values: Vec<f64> = conn
        .prepare("SELECT pm25 FROM silver_indoor_air WHERE pm25 IS NOT NULL")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    // Assert: Verify expected behavior
    assert!(
        !values.iter().any(|&v| v < 0.0 || v > 500.0),
        "All pm25 values should be in range [0, 500]"
    );
}
```

### Test Fixtures

**Reusable test helpers** in each file:

- `setup_test_db()` - Create in-memory DuckDB connection
- `create_mock_parquet_tables()` - Mock Parquet schema
- `insert_indoor_air_test_data()` - Insert test data with edge cases
- `create_silver_views()` - Create Silver layer views
- `create_test_parquet_file()` - Generate Parquet files for integration tests

## Test Coverage

### Data Quality Rules Tested

| Field | Min | Max | Precision | Tests |
|-------|-----|-----|-----------|-------|
| pm25 | 0 | 500 | 1 decimal | 5 tests |
| pm10 | 0 | 1000 | 1 decimal | 3 tests |
| co2 | 400 | 5000 | 0 decimals | 3 tests |
| temperature | -10 | 50 | 1 decimal | 4 tests |
| humidity | 0 | 100 | 1 decimal | 4 tests |
| tvoc | 0 | 60000 | 0 decimals | 2 tests |
| nox | 0 | 1000 | 0 decimals | 2 tests |
| aqi | 1 | 5 | 0 decimals | 3 tests |

### Edge Cases Covered

- ✅ Boundary values (min/max)
- ✅ Out-of-range values
- ✅ NULL values
- ✅ Mixed valid/invalid rows
- ✅ Extreme values (NaN, Infinity)
- ✅ Empty datasets
- ✅ Large datasets (100k+ rows)
- ✅ Multiple Parquet files
- ✅ Corrupted Parquet files

## Performance Benchmarks

| Test | Dataset Size | Target Time | Test ID |
|------|--------------|-------------|---------|
| 7-day query | 10,080 rows | < 5s | `bench_7_day_query` |
| 30-day aggregation | 43,200 rows | < 15s | `bench_aggregation_query` |
| Time range filter | 50,000 rows | < 10s | `bench_time_range_filter` |
| Large dataset | 100,000 rows | < 30s | `test_large_dataset_memory_efficiency` |

## Test Patterns Saved

After implementing these tests, save the patterns to NDP knowledge base:

```bash
# Save DuckDB testing pattern
claude-flow memory store "testing:duckdb-london-tdd" \
  "London TDD approach for DuckDB Silver layer: mock Parquet tables, verify SQL behavior, performance benchmarks" \
  --namespace ndp-patterns

# Save integration test pattern
claude-flow memory store "testing:duckdb-parquet-integration" \
  "Integration tests for DuckDB + Parquet: generate test files, verify schema inference, benchmark queries" \
  --namespace ndp-patterns
```

## CI/CD Integration

### GitHub Actions Workflow

```yaml
name: DP-001 Tests

on:
  pull_request:
    paths:
      - 'product/features/dp-001/**'
      - 'config/duckdb/views/*.sql'
      - 'apps/air-quality-app/tests/duckdb_*.rs'

jobs:
  unit-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Install DuckDB
        run: sudo apt-get install -y libduckdb-dev
      - name: Run unit tests
        run: cargo test --package air-quality-app --test duckdb_views_test

  integration-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Install DuckDB
        run: sudo apt-get install -y libduckdb-dev
      - name: Run integration tests
        run: cargo test --package air-quality-app --test silver_layer_integration_test -- --ignored
```

## Troubleshooting

### DuckDB Linking Error

**Error**: `cannot find -lduckdb`

**Solution**:
1. Install DuckDB C library (see Prerequisites)
2. Set `LIBRARY_PATH` if needed:
   ```bash
   export LIBRARY_PATH=/usr/local/lib:$LIBRARY_PATH
   export LD_LIBRARY_PATH=/usr/local/lib:$LD_LIBRARY_PATH
   ```

### Parquet Schema Mismatch

**Error**: `Schema mismatch when loading Parquet files`

**Solution**:
1. Ensure all test Parquet files have consistent schema
2. Use `union_by_name = true` in `read_parquet()`
3. Check `create_test_parquet_file()` schema definition

### Test Timeout

**Error**: Integration test times out

**Solution**:
1. Reduce test dataset size
2. Run with `-- --test-threads=1` for sequential execution
3. Mark as `#[ignore]` for CI

## References

- Test Specification: `/workspaces/neural-data-platform/product/features/dp-001/specification/TEST_SPECIFICATION.md`
- SQL Views: `/workspaces/neural-data-platform/config/duckdb/views/`
- London TDD: https://github.com/testdouble/contributing-tests/wiki/London-school-TDD

## Next Steps

1. **Install DuckDB C library** on development machines
2. **Enable `duckdb-tests` feature** in Cargo.toml:
   ```toml
   [features]
   duckdb-tests = ["dep:duckdb", "dep:parquet"]
   ```
3. **Run tests** to verify Silver layer implementation
4. **Add to CI/CD pipeline** for automated testing
5. **Save test patterns** to NDP knowledge base

## Status

- **Unit Tests**: ✅ Implemented (18 tests)
- **Integration Tests**: ✅ Implemented (20 tests)
- **Performance Benchmarks**: ✅ Implemented (4 benchmarks)
- **CI/CD Integration**: ⏳ Pending (requires DuckDB in CI environment)
- **Documentation**: ✅ Complete

---

**Author**: ndp-tester
**Date**: 2025-12-18
**Feature**: DP-001 - DuckDB Analytics + Grafana
