# Neural Trader Test Environment

This directory contains the complete test environment setup for Neural Trader, providing isolated testing of all system components with mock data and services.

## Overview

The test environment mirrors the production setup but with:
- **Isolated networks**: Completely separate from production
- **Mock data providers**: No external API dependencies
- **Faster cycles**: Reduced intervals for quicker testing
- **Test fixtures**: Predefined datasets for consistent testing
- **Comprehensive logging**: Debug-level logging for troubleshooting

## Architecture

```
Test Environment Components:
├── timescaledb-test     (Port 5434) - Test database with fixtures
├── redis-test          (Port 6380) - Test cache
├── neural-trader-test  (Port 8081) - Main application in test mode
├── data-ingestion-test (Port 8003) - Data ingestion with mock providers
├── prometheus-test     (Port 9095) - Metrics collection
├── grafana-test        (Port 3001) - Monitoring dashboards
├── test-data-generator             - Generates realistic test data
└── mock-api-server     (Port 8004) - Mocks external APIs (Alpaca, Finnhub, etc.)
```

## Quick Start

### 1. Build Test Images

```bash
cd /workspaces/neural-trader/docker/test
./build-test.sh
```

### 2. Start Test Environment

```bash
# Start all services
docker-compose -f docker-compose.test.yml up -d

# Check status
docker-compose -f docker-compose.test.yml ps

# View logs
docker-compose -f docker-compose.test.yml logs -f neural-trader-test
```

### 3. Generate Test Data

```bash
# Generate comprehensive test data
docker-compose -f docker-compose.test.yml up test-data-generator

# Check generated data
docker-compose -f docker-compose.test.yml exec timescaledb-test psql -U test_user -d neural_trader_test -c "SELECT COUNT(*) FROM market_data;"
```

### 4. Access Services

- **Neural Trader API**: http://localhost:8081
- **Data Ingestion API**: http://localhost:8003  
- **Grafana Dashboards**: http://localhost:3001 (admin/test_admin_123)
- **Prometheus Metrics**: http://localhost:9095
- **Mock API Server**: http://localhost:8004

## Test Configuration

### Environment Variables

Key test environment variables:
- `TESTING_MODE=true` - Enables test mode
- `MOCK_DATA_ENABLED=true` - Uses mock data providers
- `LOG_LEVEL=DEBUG` - Verbose logging
- `UPDATE_INTERVAL=5` - Fast updates (5 seconds)

### Test Database

- **Database**: `neural_trader_test`
- **User**: `test_user`
- **Password**: `test_password_123`
- **Host**: `localhost:5434`

### Mock Data Providers

The environment includes mock implementations for:
- **Alpaca**: Stock market data and trading
- **Finnhub**: Market quotes and sentiment
- **Alpha Vantage**: Historical and intraday data
- **Polygon**: Aggregated market data
- **News APIs**: Sentiment and news data
- **Reddit**: Social sentiment data

## Testing Workflows

### 1. Data Ingestion Testing

```bash
# Test data ingestion from mock providers
curl http://localhost:8003/health
curl http://localhost:8003/providers/status
curl http://localhost:8003/data/AAPL/latest
```

### 2. Neural Network Testing

```bash
# Test prediction endpoints
curl http://localhost:8081/health
curl http://localhost:8081/predictions/AAPL
curl http://localhost:8081/models/status
```

### 3. Feature Engineering Testing

```bash
# Test feature calculation
curl http://localhost:8081/features/AAPL/latest
curl http://localhost:8081/features/calculate
```

### 4. Integration Testing

```bash
# Run comprehensive integration tests
docker-compose -f docker-compose.test.yml exec neural-trader-test /usr/local/bin/test-runner.sh

# Run specific test suites
docker-compose -f docker-compose.test.yml exec data-ingestion-test python -m pytest tests/
```

## Test Data Management

### Generated Test Data

The test environment generates:
- **30 days** of historical market data
- **Technical indicators** and features
- **Mock predictions** from all neural models
- **Sentiment data** from various sources
- **Performance metrics** and logs

### Data Fixtures

Test fixtures are stored in:
- `./fixtures/market-data/` - Sample market data files
- `./fixtures/api-responses/` - Mock API response templates
- `./fixtures/test-datasets/` - Generated comprehensive datasets

### Data Reset

```bash
# Reset test database
docker-compose -f docker-compose.test.yml down -v
docker-compose -f docker-compose.test.yml up -d
```

## Monitoring and Metrics

### Prometheus Metrics

Test-specific metrics available at http://localhost:9095:
- `test_execution_duration_seconds` - Test runtime metrics
- `mock_api_requests_total` - Mock API usage
- `data_generation_records_total` - Generated test data volume

### Grafana Dashboards

Pre-configured test dashboards:
- **Test Overview**: Overall system health in test mode
- **Data Ingestion Test**: Mock provider performance
- **Neural Model Test**: Model accuracy and latency
- **Database Test**: TimescaleDB performance with test data

## Performance Testing

### Load Testing

```bash
# Generate high-frequency test data
docker-compose -f docker-compose.test.yml run --rm test-data-generator python generate_load_test_data.py

# Monitor performance
docker-compose -f docker-compose.test.yml logs -f prometheus-test
```

### Memory Testing

```bash
# Monitor memory usage during tests
docker stats --format "table {{.Container}}\t{{.CPUPerc}}\t{{.MemUsage}}\t{{.NetIO}}\t{{.BlockIO}}"
```

## Troubleshooting

### Common Issues

1. **Port Conflicts**: Test ports (5434, 6380, 8081, 8003, 3001, 9095) must be available
2. **Memory**: Test environment requires ~4GB RAM
3. **Database Connection**: Wait for TimescaleDB to be fully ready before starting other services

### Debug Commands

```bash
# Check service health
docker-compose -f docker-compose.test.yml exec neural-trader-test neural-trader health --test-mode

# Check database connectivity  
docker-compose -f docker-compose.test.yml exec timescaledb-test psql -U test_user -d neural_trader_test -c "SELECT version();"

# Check mock API server
curl http://localhost:8004/test/status

# View detailed logs
docker-compose -f docker-compose.test.yml logs --tail=100 -f [service-name]
```

### Performance Optimization

For faster test execution:
- Reduce `HISTORICAL_DAYS` in test data generation
- Increase `UPDATE_INTERVAL` for slower updates
- Use fewer symbols in `TEST_SYMBOLS`
- Disable detailed logging by setting `LOG_LEVEL=INFO`

## Clean Up

```bash
# Stop and remove all test containers and volumes
docker-compose -f docker-compose.test.yml down -v

# Remove test images
docker rmi $(docker images "neural-trader/*:test" -q)

# Clean up test fixtures
rm -rf ./fixtures/generated/*
```

## Integration with CI/CD

This test environment is designed for:
- **Automated testing** in CI/CD pipelines
- **Integration tests** before deployment
- **Performance regression** testing
- **Feature validation** testing

Example GitHub Actions usage:
```yaml
- name: Run Neural Trader Tests
  run: |
    cd docker/test
    ./build-test.sh
    docker-compose -f docker-compose.test.yml up -d
    docker-compose -f docker-compose.test.yml exec neural-trader-test /usr/local/bin/test-runner.sh
```

## Contributing

When adding new features to Neural Trader:
1. Add corresponding test configurations to this environment
2. Update mock providers with new API endpoints
3. Add new test fixtures for new data types
4. Update integration tests to cover new functionality

## Support

For issues with the test environment:
- Check service logs: `docker-compose -f docker-compose.test.yml logs [service]`
- Verify port availability: `netstat -tulpn | grep [port]`
- Check disk space: `df -h`
- Monitor resource usage: `docker stats`