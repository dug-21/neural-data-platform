# Backfill Testing Documentation

## Overview

This directory contains comprehensive integration tests for the backfill functionality of the Neural Trader system. The tests cover all aspects of data backfill operations including timezone handling, file format processing, directory traversal, symbol filtering, and end-to-end data flow validation.

## Test Structure

### Test Files

1. **`test_backfill_integration.py`** - Main integration tests covering:
   - Timezone handling and Unix nanosecond conversion
   - File format processing (CSV, CSV.GZ, JSON, Parquet)
   - Directory traversal and recursive search
   - Symbol filtering (single and multiple symbols)
   - Date range filtering with timezone awareness
   - End-to-end data flow to TimescaleDB
   - Performance testing with large datasets
   - Error handling and recovery
   - Checkpoint and resume functionality

2. **`test_backfill_validation.py`** - Data validation and quality scoring tests:
   - OHLC data consistency validation
   - Duplicate data detection
   - Missing data gap analysis
   - Quality score calculation algorithms
   - Statistics generation
   - Performance validation with large datasets

3. **`test_backfill_cli.py`** - CLI interface and command tests:
   - Argument parsing and validation
   - Command execution (file, s3, status, resume, validate, diagnose)
   - Configuration loading and environment variable handling
   - Error handling and reporting
   - Dry run functionality

### Supporting Files

- **`conftest.py`** - Pytest configuration with fixtures and test utilities
- **`pytest.ini`** - Pytest configuration and markers
- **`run_backfill_tests.py`** - Comprehensive test runner with reporting
- **`test_requirements.txt`** - Testing dependencies
- **`README_TESTING.md`** - This documentation file

## Test Categories

### 🕐 Timezone Handling Tests
- Unix nanosecond to datetime conversion
- Timezone-aware date comparisons
- DST transition handling
- Cross-timezone filtering

### 📁 File Format Tests
- Compressed CSV (.csv.gz) processing
- Uncompressed CSV processing
- JSON file processing
- Parquet file processing
- Invalid/corrupted file handling

### 🗂️ Directory Traversal Tests
- Recursive directory search
- Nested year/month structure handling
- Mixed file type processing
- Pattern-based file discovery

### 🏷️ Symbol Filtering Tests
- Single symbol filtering
- Multiple symbol filtering
- Case sensitivity handling
- Performance with large symbol lists
- Non-existent symbol handling

### 📅 Date Range Filtering Tests
- Timezone-aware date filtering
- Cross-timezone date filtering
- DST transition handling
- Boundary condition testing

### 🔄 End-to-End Integration Tests
- Complete file-to-database flow
- Batch processing performance
- Error handling and recovery
- Data transformation and validation

### ⚡ Performance Tests
- Large CSV file processing (1M+ records)
- Memory usage monitoring
- Concurrent file processing
- Throughput benchmarking

### 💾 Checkpoint and Resume Tests
- Checkpoint creation and retrieval
- Resume from checkpoint
- Checkpoint corruption handling
- TTL and cleanup behavior

### ✅ Data Validation Tests
- OHLC consistency validation
- Duplicate detection
- Gap analysis
- Quality scoring algorithms
- Statistics generation

### 🖥️ CLI Integration Tests
- Argument parsing
- Command execution
- Configuration management
- Error handling
- Dry run functionality

## Running Tests

### Prerequisites

Install test dependencies:
```bash
pip install -r test_requirements.txt
```

### Basic Test Execution

Run all tests:
```bash
python run_backfill_tests.py
```

Run specific test file:
```bash
pytest test_backfill_integration.py -v
```

Run tests with specific markers:
```bash
pytest -m "integration and not slow" -v
```

### Advanced Test Options

Run with coverage:
```bash
pytest --cov=utils.file_backfill --cov=cli.backfill --cov=providers.historical_backfill --cov-report=html
```

Run performance tests only:
```bash
pytest -m performance -v
```

Run tests in parallel:
```bash
pytest -n auto
```

Generate detailed JSON report:
```bash
pytest --json-report --json-report-file=test_report.json
```

### Using the Comprehensive Test Runner

The `run_backfill_tests.py` script provides a comprehensive test execution environment:

```bash
# Run all tests with coverage and detailed reporting
python run_backfill_tests.py

# Run without coverage, quiet mode
python run_backfill_tests.py --no-coverage --quiet

# Save detailed report
python run_backfill_tests.py --save-report backfill_results.json

# Check dependencies only
python run_backfill_tests.py --check-deps-only
```

## Test Scenarios

### Timezone Handling Scenarios
- Converting Unix nanoseconds to timezone-aware datetime objects
- Filtering data across different timezone boundaries
- Handling DST transitions correctly
- Edge cases with leap years and year boundaries

### File Format Scenarios
- Processing compressed and uncompressed CSV files
- Handling JSON arrays and objects
- Reading Parquet files with various schemas
- Graceful handling of corrupted files

### Directory Structure Scenarios
- Recursive traversal of nested directories
- Year/month hierarchical structures
- Mixed file types in same directory
- Large directory trees with thousands of files

### Symbol Filtering Scenarios
- Single symbol extraction from multi-symbol files
- Multiple symbol filtering with various combinations
- Case-insensitive symbol matching
- Performance with large symbol lists

### Data Validation Scenarios
- OHLC consistency checks (High >= Low, etc.)
- Duplicate timestamp detection and handling
- Missing data gap identification
- Quality score calculation with various error types

### Performance Scenarios
- Processing 1M+ record files efficiently
- Memory usage monitoring during large operations
- Concurrent processing of multiple files
- Throughput benchmarking across different file sizes

### Error Handling Scenarios
- Network timeouts during data operations
- Database connection failures
- File corruption and recovery
- Partial processing and resume capabilities

## Expected Outcomes

### Quality Metrics
- **Code Coverage**: >80% for all backfill modules
- **Test Success Rate**: >95% for all test suites
- **Performance**: Process 10k+ records per second
- **Memory Efficiency**: <500MB increase for 100k records

### Validation Thresholds
- **Data Quality Score**: >80% for valid data acceptance
- **OHLC Consistency**: 100% for valid market data
- **Timezone Accuracy**: Millisecond precision maintained
- **File Format Support**: CSV, JSON, Parquet compatibility

### Performance Benchmarks
- **Large File Processing**: 1M records in <60 seconds
- **Memory Usage**: Stable memory profile during processing
- **Concurrent Operations**: 3-5x speedup with parallel processing
- **Error Recovery**: <5% data loss during failures

## Troubleshooting

### Common Issues

1. **Import Errors**
   - Ensure all dependencies are installed: `pip install -r test_requirements.txt`
   - Check PYTHONPATH includes data_ingestion directory

2. **Database Connection Errors**
   - Tests use mocked databases by default
   - Check mock configurations in `conftest.py`

3. **Timezone Test Failures**
   - Ensure `pytz` is installed and up-to-date
   - Check system timezone configuration

4. **Performance Test Timeouts**
   - Increase timeout values for slower systems
   - Run performance tests separately: `pytest -m performance`

5. **Memory Issues**
   - Reduce test dataset sizes for resource-constrained environments
   - Run tests individually instead of in batches

### Debugging Tips

- Use `pytest -v -s` for verbose output with print statements
- Add `--pdb` to drop into debugger on failures
- Use `--lf` to run only last failed tests
- Check `htmlcov/index.html` for coverage details

## Contributing

When adding new backfill functionality:

1. **Add corresponding tests** in the appropriate test file
2. **Update test markers** in `pytest.ini` if needed
3. **Maintain >80% code coverage** for new modules
4. **Include performance tests** for data processing features
5. **Document test scenarios** in this README

### Test Development Guidelines

- **One test per scenario**: Each test should validate one specific behavior
- **Descriptive test names**: Test names should explain what is being tested
- **Isolated tests**: Tests should not depend on each other
- **Realistic data**: Use realistic market data patterns in tests
- **Error scenarios**: Include both positive and negative test cases
- **Performance awareness**: Monitor test execution time and resource usage

## Integration with CI/CD

These tests are designed to integrate with continuous integration pipelines:

```yaml
# Example GitHub Actions configuration
- name: Run Backfill Tests
  run: |
    pip install -r data_ingestion/tests/test_requirements.txt
    cd data_ingestion/tests
    python run_backfill_tests.py --no-coverage --save-report ci_report.json

- name: Upload Test Results
  uses: actions/upload-artifact@v3
  with:
    name: test-results
    path: data_ingestion/tests/ci_report.json
```

## Monitoring and Alerting

Consider setting up monitoring for:
- Test execution time trends
- Memory usage patterns during tests
- Coverage percentage changes
- Performance benchmark degradation

This comprehensive test suite ensures the reliability, performance, and correctness of the backfill functionality across all supported scenarios and edge cases.