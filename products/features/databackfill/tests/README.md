# Historical Data Backfill Test Suite

## Overview

This comprehensive test suite ensures the reliability, performance, and accuracy of the Historical Data Backfill System. The tests cover all aspects from unit testing individual components to end-to-end integration testing with real infrastructure.

## Test Structure

```
tests/
├── unit/                    # Unit tests for individual components
│   ├── test_backfill_job.py
│   ├── test_data_validation.py
│   └── test_provider_selection.py
├── integration/             # Integration tests with external services
│   ├── test_s3_integration.py
│   ├── test_timescale_integration.py
│   └── test_provider_integration.py
├── performance/             # Performance and load tests
│   ├── test_load_performance.py
│   └── test_stress.py
├── e2e/                     # End-to-end workflow tests
│   └── test_backfill_workflow.py
├── validation/              # Data validation tests
│   ├── test_data_quality.py
│   └── test_data_completeness.py
├── resilience/              # Failure recovery tests
│   └── test_failure_recovery.py
├── mocks/                   # Mock data generators
│   ├── market_data_generator.py
│   └── provider_mocks.py
├── conftest.py              # Pytest configuration
├── docker-compose.test.yml  # Test environment setup
├── run_tests.sh            # Test runner script
└── README.md               # This file
```

## Quick Start

### Prerequisites

- Python 3.8+
- Docker and Docker Compose
- pytest and required test packages

```bash
pip install -r requirements-test.txt
```

### Running Tests

```bash
# Run all tests (unit + integration)
./run_tests.sh

# Run only unit tests
./run_tests.sh --unit-only

# Run only integration tests
./run_tests.sh --integration-only

# Include performance tests
./run_tests.sh --performance

# Run everything including E2E tests
./run_tests.sh --all
```

## Test Categories

### 1. Unit Tests

Fast, isolated tests for individual components:

- **BackfillJob Tests**: Job lifecycle, progress tracking, serialization
- **Data Validation Tests**: OHLC consistency, duplicate detection, quality scoring
- **Provider Selection Tests**: Optimal provider selection logic

```bash
pytest tests/unit/ -v
```

### 2. Integration Tests

Tests with real or mocked external services:

- **S3 Integration**: Upload/download, partitioning, compression
- **TimescaleDB Integration**: Hypertable operations, batch insertion
- **Provider Integration**: API connectivity, data consistency

```bash
# Start test containers first
docker-compose -f docker-compose.test.yml up -d

# Run integration tests
pytest tests/integration/ -v -m integration

# Stop containers
docker-compose -f docker-compose.test.yml down
```

### 3. Performance Tests

Load and stress testing:

- **High Volume Ingestion**: 1M+ data points
- **Concurrent Jobs**: 10+ parallel backfill jobs
- **Memory Usage**: Memory leak detection
- **Database Throughput**: Sustained write performance

```bash
pytest tests/performance/ -v -m performance
```

### 4. End-to-End Tests

Complete workflow testing:

- **Single Symbol Backfill**: Full backfill process
- **Multi-Symbol Parallel**: Concurrent backfills
- **Incremental Updates**: Gap filling
- **Resume from Checkpoint**: Failure recovery

```bash
pytest tests/e2e/ -v -m slow
```

## Mock Data Generation

The test suite includes realistic market data generators:

```python
from tests.mocks.market_data_generator import MarketDataGenerator

# Generate OHLCV data
data = MarketDataGenerator.generate_ohlcv_data(
    symbol="AAPL",
    start_date=datetime(2023, 1, 1),
    end_date=datetime(2023, 12, 31),
    interval="1hour",
    realistic=True
)

# Generate tick data
ticks = MarketDataGenerator.generate_tick_data(
    symbol="AAPL",
    date=datetime(2023, 1, 1),
    tick_count=50000
)

# Inject anomalies for testing
anomalous_data = MarketDataGenerator.inject_anomalies(
    data,
    gap_probability=0.1,
    duplicate_probability=0.05
)
```

## Test Environment

### Local Test Infrastructure

The `docker-compose.test.yml` provides:

- **TimescaleDB**: Time-series database on port 5433
- **LocalStack**: S3-compatible storage on port 4566
- **Redis**: Optional caching on port 6380

### Environment Variables

Test environment variables are automatically set in `conftest.py`:

```python
POLYGON_API_KEY=test_polygon_key
ALPACA_API_KEY=test_alpaca_key
AWS_ACCESS_KEY_ID=test
AWS_SECRET_ACCESS_KEY=test
S3_BUCKET=test-trading-backfill
DATABASE_URL=postgresql://test_user:test_pass@localhost:5433/trading_test
```

## Coverage Requirements

- **Unit Tests**: ≥ 90% coverage
- **Integration Tests**: ≥ 80% coverage
- **Overall**: ≥ 85% coverage

Check coverage:

```bash
# Generate coverage report
pytest --cov=data_ingestion.providers.historical_backfill --cov-report=html

# View report
open htmlcov/index.html
```

## Performance Benchmarks

Expected performance targets:

- **Ingestion Rate**: ≥ 10,000 points/second
- **Memory Usage**: < 2GB for 1M points
- **Error Recovery**: < 30 seconds
- **Data Quality Score**: ≥ 95%
- **P95 Latency**: < 50ms for database writes

## CI/CD Integration

Add to your CI pipeline:

```yaml
# .github/workflows/test.yml
name: Backfill Tests
on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    
    services:
      postgres:
        image: timescale/timescaledb:latest-pg14
        env:
          POSTGRES_PASSWORD: test_pass
        options: >-
          --health-cmd pg_isready
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Set up Python
        uses: actions/setup-python@v4
        with:
          python-version: '3.9'
      
      - name: Install dependencies
        run: |
          pip install -r requirements.txt
          pip install -r requirements-test.txt
      
      - name: Run tests
        run: ./run_tests.sh
      
      - name: Upload coverage
        uses: codecov/codecov-action@v3
```

## Debugging Tests

### Verbose Output

```bash
# Show detailed output
pytest tests/unit/test_backfill_job.py -vv

# Show print statements
pytest tests/unit/test_backfill_job.py -s

# Run specific test
pytest tests/unit/test_backfill_job.py::TestBackfillJob::test_progress_calculation -v
```

### Interactive Debugging

```python
# Add breakpoint in test
import pdb; pdb.set_trace()

# Or use pytest's built-in
pytest tests/unit/test_backfill_job.py --pdb
```

### Test Logs

```bash
# Enable debug logging
LOG_LEVEL=DEBUG pytest tests/integration/ -v

# Capture logs in report
pytest tests/integration/ --log-cli-level=DEBUG
```

## Contributing

When adding new tests:

1. Follow the existing structure and naming conventions
2. Add appropriate pytest markers (@pytest.mark.performance, etc.)
3. Include docstrings describing what is being tested
4. Ensure tests are deterministic and repeatable
5. Mock external dependencies appropriately
6. Update this README if adding new test categories

## Troubleshooting

### Common Issues

1. **Container startup failures**
   ```bash
   docker-compose -f docker-compose.test.yml logs
   docker-compose -f docker-compose.test.yml down -v
   ```

2. **Database connection errors**
   - Ensure TimescaleDB is healthy: `docker ps`
   - Check port conflicts: `lsof -i :5433`

3. **S3/LocalStack issues**
   - Verify LocalStack is running: `curl http://localhost:4566/_localstack/health`
   - Check AWS credentials in environment

4. **Memory issues in performance tests**
   - Increase Docker memory limits
   - Run performance tests individually
   - Use `--forked` mode: `pytest -n auto tests/performance/`

## Test Maintenance

Regular maintenance tasks:

1. **Update mock data** to reflect current market conditions
2. **Review performance benchmarks** quarterly
3. **Update provider mocks** when APIs change
4. **Prune old test artifacts** from test-reports/
5. **Validate test coverage** remains above thresholds

## Contact

For questions or issues with the test suite, please contact the Data Infrastructure team.