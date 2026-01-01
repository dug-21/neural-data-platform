# Config-Store Integration Tests

This directory contains comprehensive integration tests for the config-store integration with data-ingestion components, following the SPARC Refinement Plan Phase 2: TDD Migration Strategy.

## Overview

The test suite validates all critical scenarios for migrating from environment variables to the config-store system:

- **Configuration Loading**: Loading configurations from config-store with caching and validation
- **Fallback Mechanisms**: Graceful fallback to environment variables when config-store is unavailable
- **Hot Reloading**: Real-time configuration updates without service restart
- **Provider Configuration**: Dynamic provider configuration management (Polygon, Alpaca, etc.)
- **Rate Limiting**: Dynamic rate limit configuration and enforcement
- **Database & Redis**: Database and Redis connection configuration management
- **Complete Migration**: End-to-end migration process with rollback capabilities

## Test Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                   Integration Test Environment               │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐        │
│  │   PostgreSQL │  │    Redis    │  │  Prometheus │        │
│  │  (TimescaleDB)  │   (Cache)   │  │ (Monitoring)│        │
│  └─────────────┘  └─────────────┘  └─────────────┘        │
│           │              │              │                  │
│           └──────────────┼──────────────┘                  │
│                          │                                 │
│  ┌─────────────────────────────────────────────────────┐   │
│  │            Mock Config-Store Service                │   │
│  │  - REST API for configuration management           │   │
│  │  - Redis backend for fast access                   │   │
│  │  - PostgreSQL for persistent storage               │   │
│  │  - Encryption support for sensitive data           │   │
│  │  - Real-time notifications via WebSocket           │   │
│  └─────────────────────────────────────────────────────┘   │
│                          │                                 │
│  ┌─────────────────────────────────────────────────────┐   │
│  │         Data Ingestion Test Service                 │   │
│  │  - Config-store integration                        │   │
│  │  - Fallback to environment variables               │   │
│  │  - Hot-reload configuration support                │   │
│  │  - Provider management (Polygon, Alpaca)           │   │
│  │  - Rate limiting and performance monitoring        │   │
│  └─────────────────────────────────────────────────────┘   │
│                          │                                 │
│  ┌─────────────────────────────────────────────────────┐   │
│  │         Integration Test Runner                     │   │
│  │  - Comprehensive test suite execution              │   │
│  │  - Multiple test scenarios                         │   │
│  │  - Coverage reporting                              │   │
│  │  - Performance and load testing                    │   │
│  └─────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

## Quick Start

### Prerequisites

- Docker and Docker Compose
- Python 3.11+ (for local development)
- Git

### Running All Tests

```bash
# Run the complete test suite
docker-compose -f docker-compose.test.yml --profile full up --build

# Or use the test runner script
python tests/integration/run_tests.py --scenario full_suite --verbose
```

### Running Specific Test Scenarios

```bash
# Test configuration loading only
python tests/integration/run_tests.py --scenario basic_integration

# Test fallback mechanisms
python tests/integration/run_tests.py --scenario fallback_mechanism

# Test hot reloading
python tests/integration/run_tests.py --scenario hot_reloading

# Test provider configuration
python tests/integration/run_tests.py --scenario provider_configuration

# Test rate limiting
python tests/integration/run_tests.py --scenario rate_limiting

# Test database and Redis configuration
python tests/integration/run_tests.py --scenario database_redis

# Test complete migration process
python tests/integration/run_tests.py --scenario migration_process
```

### Running with Different Profiles

```bash
# Basic integration tests (minimal services)
docker-compose -f docker-compose.test.yml --profile integration up

# Fallback testing (config-store unavailable)
docker-compose -f docker-compose.test.yml --profile fallback up

# Full test suite with monitoring
docker-compose -f docker-compose.test.yml --profile full up

# Performance testing
docker-compose -f docker-compose.test.yml --profile performance up
```

## Test Scenarios

### 1. Configuration Loading Tests (`TestConfigurationLoading`)

- **test_load_configuration_from_config_store**: Validates loading configuration values from config-store
- **test_configuration_caching**: Tests configuration caching behavior and performance
- **test_configuration_validation**: Tests configuration validation during loading

**Coverage**: Configuration retrieval, caching mechanisms, validation logic

### 2. Fallback Mechanism Tests (`TestFallbackMechanism`)

- **test_fallback_to_env_vars_on_config_store_unavailable**: Tests fallback when config-store is down
- **test_graceful_degradation_during_migration**: Tests hybrid configuration during migration period

**Coverage**: Environment variable fallback, graceful degradation, error handling

### 3. Hot Reloading Tests (`TestHotReloading`)

- **test_configuration_hot_reload**: Tests real-time configuration updates
- **test_configuration_change_propagation**: Tests configuration change propagation to services

**Coverage**: Real-time updates, event propagation, service coordination

### 4. Provider Configuration Tests (`TestProviderConfiguration`)

- **test_polygon_provider_configuration**: Tests Polygon provider configuration loading
- **test_alpaca_provider_configuration**: Tests Alpaca provider configuration loading
- **test_dynamic_provider_switching**: Tests dynamic provider switching based on configuration

**Coverage**: Provider management, API configuration, dynamic switching

### 5. Rate Limiting Configuration Tests (`TestRateLimitConfiguration`)

- **test_rate_limit_configuration_loading**: Tests loading rate limit configurations
- **test_dynamic_rate_limit_updates**: Tests dynamic rate limit updates without restart

**Coverage**: Rate limiting, performance configuration, dynamic updates

### 6. Database & Redis Configuration Tests (`TestDatabaseRedisConfiguration`)

- **test_database_configuration_loading**: Tests database configuration loading
- **test_redis_configuration_loading**: Tests Redis configuration loading
- **test_database_connection_health_monitoring**: Tests connection health monitoring

**Coverage**: Database connections, Redis configuration, health monitoring

### 7. Complete Migration Process Tests (`TestCompleteMigrationProcess`)

- **test_complete_migration_workflow**: Tests the complete migration from env vars to config-store
- **test_migration_rollback_capability**: Tests rollback capability during migration
- **test_migration_validation_gates**: Tests validation gates during migration

**Coverage**: Migration process, rollback mechanisms, validation workflows

## Configuration

### Environment Variables

The test environment supports various configuration options:

```bash
# Database Configuration
POSTGRES_HOST=postgres-test
POSTGRES_PORT=5432
POSTGRES_DB=neural_trader_test
POSTGRES_USER=postgres
POSTGRES_PASSWORD=test_password_123

# Redis Configuration
REDIS_HOST=redis-test
REDIS_PORT=6379
REDIS_PASSWORD=test_redis_pass
REDIS_DB=15

# Config Store Configuration
CONFIG_STORE_URL=http://config-store-mock:8080
CONFIG_STORE_ENABLED=true
CONFIG_STORE_PREFIX=neural_trader_test
CONFIG_STORE_FALLBACK_TO_ENV=true

# Test Configuration
TEST_TIMEOUT=300
COVERAGE_MIN_PERCENTAGE=85
PYTEST_ARGS="-v --tb=short"
```

### Test Data Setup

The test environment automatically initializes with:

- **Configuration Data**: Test configurations for all providers and components
- **Market Data**: Sample market data for integration testing
- **Provider Metrics**: Test metrics for rate limiting and performance validation
- **Audit Logs**: Configuration change audit trails

## Monitoring and Observability

### Prometheus Metrics

The test environment includes Prometheus monitoring with metrics for:

- Configuration access patterns
- Cache hit/miss rates
- Rate limiting effectiveness
- Database and Redis performance
- Service health and availability

Access Prometheus at: `http://localhost:9091`

### Grafana Dashboards

Pre-configured dashboards for:

- Config-Store Performance
- Data Ingestion Metrics  
- Test Execution Monitoring
- Resource Usage

Access Grafana at: `http://localhost:3001` (admin/test_admin_pass)

### Structured Logging

All services use structured logging with:

- JSON formatted logs
- Correlation IDs for request tracking
- Performance metrics
- Security audit events

## Results and Reporting

### Test Results

Test results are saved to:
- `test-results/` - JUnit XML and HTML reports
- `coverage/` - Coverage reports (HTML and XML)
- `reports/` - Custom test reports

### Coverage Requirements

- **Minimum Coverage**: 85%
- **Coverage Types**: Line, branch, and function coverage
- **Exclusions**: Test files, generated code, external dependencies

### Performance Benchmarks

The test suite includes performance benchmarks for:

- Configuration loading times
- Cache performance
- Database query performance
- Memory usage patterns

## Troubleshooting

### Common Issues

1. **Services not starting**: Check Docker resources and port conflicts
2. **Database connection errors**: Verify PostgreSQL is healthy with `docker-compose logs postgres-test`
3. **Redis connection errors**: Check Redis password and connectivity
4. **Test timeouts**: Increase `TEST_TIMEOUT` environment variable
5. **Coverage failures**: Review excluded files in `.coveragerc`

### Debug Mode

Run tests in debug mode:

```bash
# Enable debug logging
export LOG_LEVEL=DEBUG

# Run single test with detailed output  
python tests/integration/run_tests.py --scenario basic_integration --verbose --no-cleanup

# Check service health
docker-compose -f docker-compose.test.yml exec integration-test-runner python /app/health_check.py
```

### Log Analysis

```bash
# View all service logs
docker-compose -f docker-compose.test.yml logs -f

# View specific service logs
docker-compose -f docker-compose.test.yml logs config-store-mock
docker-compose -f docker-compose.test.yml logs data-ingestion-test

# View test runner logs
docker-compose -f docker-compose.test.yml logs integration-test-runner
```

## Development

### Adding New Tests

1. Add test methods to appropriate test class in `test_data_ingestion_config.py`
2. Update test scenarios in `run_tests.py`
3. Add any required test data to `sql/init.sql`
4. Update this README with new test documentation

### Extending Mock Services

The mock config-store service can be extended by:

1. Adding new API endpoints in `mocks/app.py`
2. Implementing additional storage backends
3. Adding new notification mechanisms
4. Enhancing security features

### Performance Testing

For performance and load testing:

```bash
# Run performance test profile
docker-compose -f docker-compose.test.yml --profile performance up

# Run with high load configuration
RATE_LIMIT_REQUESTS_PER_MINUTE=1000 MAX_CONCURRENT_REQUESTS=50 \
docker-compose -f docker-compose.test.yml --profile performance up
```

## References

- [SPARC Refinement Plan - Phase 2: TDD Migration Strategy](/workspaces/neural-trader/product/features/v2Planning/phase2/4-SPARC-Refinement-TDD.md)
- [Config-Store Architecture Documentation](/workspaces/neural-trader/config-store/README.md)
- [Data Ingestion Configuration](/workspaces/neural-trader/data_ingestion/config/README.md)
- [Docker Test Environment](/workspaces/neural-trader/docker/test/README.md)