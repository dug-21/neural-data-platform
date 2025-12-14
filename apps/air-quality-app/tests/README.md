# AIR-002 Integration Tests

## Overview

This directory contains comprehensive integration tests for the AIR-002 MQTT to Parquet data pipeline. The tests verify the end-to-end functionality of the ParquetStore component and its integration with the neural_core library.

## Test File Structure

### `/workspaces/neural-data-platform/apps/air-quality-app/tests/integration_test.rs`

Complete integration test suite with 35+ test scenarios covering:

## Test Categories

### 1. Basic Storage Integration Tests (5 tests)
- **test_parquet_write_and_query**: Verifies basic write and query operations
- **test_data_persistence_after_restart**: Ensures data survives store restarts
- **test_storage_health_check**: Validates health check reporting
- **test_multi_location_partitioning**: Tests location-based data partitioning
- **test_batch_write_performance**: Validates batch write efficiency (1000 points)

### 2. WAL (Write-Ahead Log) Tests (2 tests)
- **test_wal_replay_correctness**: Verifies WAL replay after restart
- **test_wal_replay_empty**: Tests empty WAL handling

### 3. Aggregation Query Tests (4 tests)
- **test_aggregation_mean**: Tests mean aggregation
- **test_aggregation_sum**: Tests sum aggregation
- **test_aggregation_max**: Tests maximum value aggregation
- **test_aggregation_min**: Tests minimum value aggregation

### 4. Time Range Filtering Tests (3 tests)
- **test_time_range_exact_boundaries**: Tests precise time range queries
- **test_time_range_no_data**: Handles empty result sets
- **test_time_range_cross_day_boundaries**: Tests multi-day queries with partitioning

### 5. Invalid Input Handling Tests (6 tests)
- **test_invalid_empty_location_id**: Empty location ID handling
- **test_invalid_nan_values**: NaN value handling
- **test_invalid_infinity_values**: Infinity value handling
- **test_invalid_reversed_time_range**: Reversed time range (end < start)
- **test_invalid_empty_batch**: Empty batch write handling

### 6. Concurrent Access Tests (2 tests)
- **test_concurrent_writes_different_locations**: 5 concurrent writes to different locations
- **test_concurrent_reads_same_location**: 10 concurrent reads of same location

### 7. Stress Tests (2 tests)
- **test_air002_batch_size**: AIR-002 specific batch size (100 points) within 5s timeout
- **test_multiple_sequential_batches**: 10 batches × 100 points = 1000 points

### 8. Edge Case Tests (4 tests)
- **test_query_nonexistent_location**: Non-existent location handling
- **test_long_location_id**: 500-character location IDs
- **test_special_characters_in_location_id**: Special chars (/, -, _, .)
- **test_extreme_timestamp_values**: Far future timestamps (100 years)

## AIR-002 Specific Requirements

The tests validate the following AIR-002 pipeline requirements:

- **Batch Size**: 100 points (tested in `test_air002_batch_size`)
- **Timeout**: 5 seconds (tested in `test_air002_batch_size`)
- **Pipeline**: MQTT → Parser → ParquetStore (components tested independently)
- **Data Persistence**: WAL and Parquet file integrity
- **Partitioning**: Location and time-based partitioning

## Running the Tests

### Prerequisites

```bash
# Ensure tempfile dev-dependency is available
cargo build --package air-quality-app

# Optional: Start MQTT broker for future MQTT integration tests
docker compose up mosquitto
```

### Run All Integration Tests

```bash
cargo test -p air-quality-app --test integration_test
```

### Run Specific Test Category

```bash
# Basic storage tests
cargo test -p air-quality-app --test integration_test test_parquet

# WAL tests
cargo test -p air-quality-app --test integration_test test_wal

# Aggregation tests
cargo test -p air-quality-app --test integration_test test_aggregation

# Time range tests
cargo test -p air-quality-app --test integration_test test_time_range

# Invalid input tests
cargo test -p air-quality-app --test integration_test test_invalid

# Concurrent tests
cargo test -p air-quality-app --test integration_test test_concurrent

# Stress tests
cargo test -p air-quality-app --test integration_test test_air002
cargo test -p air-quality-app --test integration_test test_multiple_sequential

# Edge case tests
cargo test -p air-quality-app --test integration_test test_query_nonexistent
cargo test -p air-quality-app --test integration_test test_long_location
cargo test -p air-quality-app --test integration_test test_special_characters
cargo test -p air-quality-app --test integration_test test_extreme_timestamp
```

### Run Single Test

```bash
cargo test -p air-quality-app --test integration_test test_parquet_write_and_query -- --exact
```

### Run with Output

```bash
cargo test -p air-quality-app --test integration_test -- --nocapture
```

## Test Philosophy

These integration tests follow these principles:

1. **Real Dependencies**: Use actual ParquetStore implementation (not mocks)
2. **Temporary Storage**: Each test uses isolated TempDir
3. **Complete Workflows**: Test full data flow from write to query
4. **Performance Validation**: Ensure operations complete within time limits
5. **Error Scenarios**: Test both happy path and error conditions
6. **Concurrent Safety**: Validate thread-safe operations

## Coverage Analysis

Current test coverage:

- ✅ **Write Operations**: Single point, batch, empty batch
- ✅ **Query Operations**: Time range, filters, aggregations
- ✅ **Persistence**: Restart scenarios, WAL replay
- ✅ **Partitioning**: Location-based, time-based
- ✅ **Health Checks**: Storage status reporting
- ✅ **Error Handling**: Invalid inputs, edge cases
- ✅ **Concurrency**: Parallel reads and writes
- ✅ **Performance**: Batch processing within SLA

## Future Test Enhancements

Potential additions for comprehensive coverage:

1. **MQTT Integration Tests** (requires running broker):
   - End-to-end MQTT → ParquetStore flow
   - Connection failure recovery
   - Message parsing validation

2. **Schema Evolution Tests**:
   - Adding new metrics over time
   - Backward compatibility

3. **Large Dataset Tests**:
   - Million-point datasets
   - Multi-GB parquet files

4. **Failure Recovery Tests**:
   - Disk full scenarios
   - Corrupted parquet files
   - Partial WAL entries

5. **Multi-Sensor Scenarios**:
   - 100+ concurrent sensors
   - Mixed metric types

## Troubleshooting

### Test Failures

**Permission Errors**:
```bash
# Ensure temp directory is writable
chmod 755 /tmp
```

**Timeout Errors**:
```bash
# Increase test timeout
cargo test -- --test-threads=1
```

**Resource Exhaustion**:
```bash
# Run tests sequentially
cargo test -- --test-threads=1
```

### Debugging Tests

```bash
# Enable debug logging
RUST_LOG=debug cargo test -p air-quality-app --test integration_test

# Run single test with full output
cargo test -p air-quality-app --test integration_test test_name -- --exact --nocapture
```

## Performance Benchmarks

Expected performance metrics (on typical development machine):

- **Single Write**: < 10ms
- **Batch Write (100 points)**: < 1s (target: < 5s)
- **Batch Write (1000 points)**: < 5s
- **Query (1 day)**: < 100ms
- **Query (1 month)**: < 500ms
- **Aggregation (1 day)**: < 200ms
- **Health Check**: < 10ms

## Dependencies

Test dependencies (from `Cargo.toml`):

```toml
[dev-dependencies]
mockall = "0.13"
axum-test = "14.0"
tokio-test = "0.4"
urlencoding = "2.1"
tempfile = "3.8"
```

Core dependencies used in tests:
- `chrono`: Timestamp handling
- `tokio`: Async runtime
- `neural_core`: ParquetStore and traits
- `tempfile`: Isolated test directories

## Contributing

When adding new tests:

1. Follow the existing categorization structure
2. Use descriptive test names (test_<category>_<scenario>)
3. Add comprehensive documentation comments
4. Verify tests pass in isolation and in parallel
5. Update this README with new test descriptions
6. Ensure performance benchmarks are met

## Related Documentation

- [AIR-002 Feature Specification](/workspaces/neural-data-platform/product/features/air-002/)
- [ParquetStore Implementation](/workspaces/neural-data-platform/core/src/storage/parquet.rs)
- [Store Trait Definition](/workspaces/neural-data-platform/core/src/traits.rs)
- [WAL Implementation](/workspaces/neural-data-platform/core/src/storage/wal.rs)
