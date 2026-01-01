# Redis Streams EventBus Component Tests

This directory contains comprehensive independent unit tests for the Redis Streams EventBus implementation used in the neural-trader platform.

## Test Structure

### Test Files

1. **`test_stream_channels.py`** - Stream channel configuration and management
   - Channel creation with proper configurations
   - Message schema validation
   - Channel lifecycle management
   - Configuration completeness validation

2. **`test_message_routing.py`** - Message routing and pub/sub patterns
   - Message publishing to different channel types
   - Subscription patterns and consumption
   - Routing statistics and metadata
   - Concurrent publishing and consumption

3. **`test_consumer_groups.py`** - Consumer group management
   - Consumer group creation and destruction
   - Consumer registration and lifecycle
   - Message acknowledgment and pending tracking
   - Health monitoring and scaling

4. **`test_message_ordering.py`** - Message ordering guarantees
   - FIFO ordering validation
   - Timestamp-based ordering
   - Sequence number ordering
   - Partition-based ordering
   - Concurrent ordering preservation

5. **`test_throughput_benchmarks.py`** - Performance and throughput validation
   - Throughput benchmarks (100K msgs/sec target)
   - Latency measurements (P50, P95, P99)
   - Load testing scenarios
   - Performance regression detection

## Channel Types Tested

- **market-data**: High-frequency market data (10K msgs/sec target)
- **predictions**: ML model predictions (1K msgs/sec target)  
- **actions**: Trading actions (500 msgs/sec target)
- **monitoring**: System monitoring (5K msgs/sec target)

## Consumer Groups Tested

- **market-data**: ml-processors, analytics, monitoring
- **predictions**: trading-engine, risk-management, analytics
- **actions**: execution-engine, compliance, audit
- **monitoring**: alerting, metrics, logging

## Test Features

### Standalone Testing
- All tests use mocked Redis implementations
- No external dependencies required
- Tests can run in parallel
- Isolated test data and state

### Performance Validation
- Throughput benchmarks with configurable targets
- Latency percentile measurements
- Memory usage monitoring
- CPU utilization tracking
- Error rate validation

### Comprehensive Coverage
- Stream channel management
- Message routing patterns
- Consumer group operations
- Ordering guarantees
- Performance characteristics
- Error handling and resilience

## Running the Tests

### Prerequisites

Install test dependencies:
```bash
pip install -r requirements.txt
```

### Run All Tests

```bash
# Run all tests
pytest

# Run with coverage
pytest --cov=. --cov-report=html

# Run specific test categories
pytest -m "not slow"  # Skip slow tests
pytest -m benchmark   # Only benchmark tests
pytest -m throughput  # Only throughput tests
```

### Run Individual Test Files

```bash
# Channel configuration tests
pytest test_stream_channels.py -v

# Message routing tests  
pytest test_message_routing.py -v

# Consumer group tests
pytest test_consumer_groups.py -v

# Message ordering tests
pytest test_message_ordering.py -v

# Throughput benchmark tests
pytest test_throughput_benchmarks.py -v
```

### Performance Testing

```bash
# Run performance benchmarks
pytest test_throughput_benchmarks.py::TestThroughputBenchmarks::test_market_data_throughput_target -v

# Run with performance profiling
pytest --benchmark-only --benchmark-sort=mean
```

## Test Configuration

### Mock Redis Configuration

Tests use a high-performance mock Redis implementation that simulates:
- Network latency (configurable)
- Message persistence
- Consumer group operations
- Stream trimming
- Concurrent operations

### Performance Targets

| Channel Type | Throughput Target | Latency P99 | Memory/1K msgs |
|-------------|------------------|-------------|-----------------|
| market-data | 10,000 msgs/sec  | 50ms        | 2.5MB          |
| predictions | 1,000 msgs/sec   | 100ms       | 5.0MB          |
| actions     | 500 msgs/sec     | 200ms       | 3.0MB          |
| monitoring  | 5,000 msgs/sec   | 100ms       | 1.5MB          |

### Test Categories

- **Unit Tests**: Individual component testing
- **Integration Tests**: Cross-component interactions
- **Performance Tests**: Throughput and latency validation
- **Load Tests**: High-volume scenario testing
- **Stress Tests**: Resource limit testing

## Test Data

### Sample Messages

Tests use realistic message structures based on the protocol buffer schemas:

```python
# Market Data Message
{
    'symbol': 'AAPL',
    'price': 150.75,
    'volume': 1000,
    'timestamp': 1634567890000,
    'source': 'binance'
}

# Prediction Message  
{
    'symbol': 'AAPL',
    'prediction': 0.75,
    'confidence': 0.85,
    'model_id': 'lstm_v2',
    'correlation_id': 'uuid-string'
}
```

## Assertion Patterns

### Performance Assertions
```python
# Throughput validation
assert metrics.throughput_msgs_per_sec >= target_throughput * 0.8

# Latency validation  
assert metrics.latency_p99_ms < max_latency_threshold

# Memory validation
assert metrics.memory_usage_mb < memory_limit
```

### Functional Assertions
```python
# Message ordering
assert all(msg_ids[i] < msg_ids[i+1] for i in range(len(msg_ids)-1))

# Consumer group health
assert health_metrics['status'] in ['healthy', 'degraded', 'unhealthy']

# Schema validation
assert channel_config.validate_message_schema(channel, message)
```

## Debugging and Troubleshooting

### Verbose Output
```bash
# Enable debug logging
pytest -v -s --log-cli-level=DEBUG

# Show test durations
pytest --durations=10

# Run specific failing test
pytest test_file.py::test_function_name -vvv
```

### Performance Debugging
```bash
# Profile memory usage
pytest --profile

# Show benchmark statistics
pytest --benchmark-only --benchmark-verbose

# Monitor system resources
pytest --monitor-system-resources
```

## Continuous Integration

### GitHub Actions Integration
```yaml
- name: Run Redis Streams Tests
  run: |
    cd tests/components/redis_streams
    pip install -r requirements.txt
    pytest --cov=. --cov-report=xml --junit-xml=results.xml
```

### Quality Gates
- Test coverage ≥ 90%
- All performance benchmarks pass
- No test failures
- Memory usage within limits
- Error rates below thresholds

## Extending the Tests

### Adding New Channel Types
1. Update channel configurations in test fixtures
2. Add schema validation tests
3. Include in throughput benchmarks
4. Add consumer group configurations

### Adding Performance Tests
1. Define performance targets
2. Create test configurations
3. Implement benchmark functions
4. Add assertion validations

### Adding Integration Scenarios
1. Define multi-component workflows
2. Create realistic test data
3. Implement end-to-end flows
4. Validate system behavior

## Best Practices

### Test Organization
- One test class per major component
- Descriptive test method names
- Clear setup and teardown
- Isolated test data

### Mock Usage
- Realistic behavior simulation
- Configurable latency and errors
- Thread-safe operations
- Resource cleanup

### Performance Testing
- Consistent test environments
- Statistical significance
- Regression detection
- Resource monitoring

### Error Handling
- Exception path testing
- Failure scenario simulation
- Recovery behavior validation
- Graceful degradation testing

## Related Documentation

- [Redis Streams Channel Specification](../../../product/features/v2Planning/mvp/architecture/REDIS_STREAMS_CHANNEL_SPECIFICATION.md)
- [Neural Trader Architecture](../../../docs/architecture/)
- [Testing Strategy](../../../product/features/v2Planning/phase3/testing/strategy/TDD_MASTER_PLAN.md)