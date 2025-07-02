# Performance Benchmarking Suite

## Overview

This comprehensive benchmarking suite validates all performance targets specified in the Week 3 development plan for the Neural Trading Platform. It measures actual latency, throughput, and memory usage across all critical system components.

## Performance Targets

The benchmarks validate these specific performance requirements:

- **Data Storage Latency**: < 50ms (TimescaleDB operations)
- **Cache Operation Latency**: < 5ms (Redis operations)  
- **Neural Prediction Latency**: < 100ms (FANN model predictions)
- **Agent Decision Latency**: < 100ms (DAA agent processing)

## Benchmark Categories

### 1. Data Storage Benchmarks (`benchmark_data_storage`)

Tests TimescaleDB performance with realistic trading data:

- **Single Insert**: Individual time series data point insertion
- **Batch Insert**: Bulk data insertion (100, 1K, 10K records)
- **Time Range Query**: Historical data retrieval
- **Prediction Storage**: Neural prediction result storage
- **Statistics Aggregation**: Time-bucketed analytics queries

### 2. Cache Operation Benchmarks (`benchmark_cache_operations`)

Tests Redis caching performance:

- **SET Operations**: Data caching with TTL
- **GET Operations**: Cache retrieval performance
- **Prediction Cache**: Specialized prediction result caching
- **TTL Operations**: Time-to-live management
- **Multiple Operations**: Batch cache operations

### 3. Neural Prediction Benchmarks (`benchmark_neural_predictions`)

Tests FANN model prediction performance:

- **Single Prediction**: Individual forecast generation
- **Model-Specific Tests**: Performance per model type (N-HiTS, DeepAR, TCN, MLP)
- **Batch Predictions**: Concurrent prediction processing
- **Model Selection**: Optimal model choice algorithms

### 4. Agent Decision Benchmarks (`benchmark_agent_decisions`)

Tests DAA-FANN integration performance:

- **Single Decision**: Individual agent decision processing
- **Prediction Requests**: Agent-to-FANN communication
- **Enhanced Decisions**: Coordinated decision-making
- **Multi-Agent Coordination**: Consensus algorithms
- **Streaming Processing**: Real-time decision flows

### 5. Throughput Benchmarks (`benchmark_throughput`)

Tests system capacity limits:

- **Events per Second**: Market event processing capacity
- **Predictions per Second**: Neural prediction throughput
- **Concurrent Requests**: Parallel request handling
- **System Scalability**: Load-based performance testing

### 6. Memory Usage Benchmarks (`benchmark_memory_usage`)

Tests memory efficiency and leak detection:

- **Base Memory Footprint**: System initialization memory usage
- **Memory Under Load**: Memory growth patterns
- **Memory Cleanup**: Garbage collection efficiency
- **Leak Detection**: Long-running memory stability

### 7. Latency Analysis (`benchmark_latency_analysis`)

Statistical analysis of response times:

- **Percentile Analysis**: P50, P90, P95, P99, P99.9 latencies
- **Distribution Analysis**: Latency patterns and outliers
- **Regression Detection**: Performance degradation monitoring
- **Bottleneck Identification**: System constraint analysis

## Prerequisites

### Required Services

Ensure these services are running before executing benchmarks:

```bash
# TimescaleDB (for data storage benchmarks)
docker run -d --name timescaledb \
  -p 5432:5432 \
  -e POSTGRES_PASSWORD=password \
  -e POSTGRES_DB=neural_trader_test \
  timescale/timescaledb:latest-pg14

# Redis (for cache benchmarks)
docker run -d --name redis \
  -p 6379:6379 \
  redis:latest
```

### Environment Variables

```bash
export DATABASE_URL="postgresql://postgres:password@localhost:5432/neural_trader_test"
export REDIS_URL="redis://127.0.0.1:6379"
```

## Running Benchmarks

### Quick Benchmark Execution

```bash
# Run all benchmarks
cargo bench --bench performance_benchmarks

# Run specific benchmark group
cargo bench --bench performance_benchmarks -- "data_storage"
cargo bench --bench performance_benchmarks -- "cache_operations"
cargo bench --bench performance_benchmarks -- "neural_predictions"
cargo bench --bench performance_benchmarks -- "agent_decisions"
```

### Comprehensive Analysis with Memory Storage

```bash
# Run benchmarks and store results in Memory system
./scripts/store_benchmark_results.sh
```

This script will:
1. Execute all benchmark suites
2. Process and analyze results
3. Store results in Memory system with key: `swarm-auto-centralized-1751484080479/performance-benchmarks/results`
4. Generate performance summary report
5. Validate all Week 3 targets

### Advanced Benchmark Options

```bash
# Detailed HTML reports (saved to target/criterion/)
cargo bench --bench performance_benchmarks -- --output-format html

# Specific sample size for more accurate measurements
CRITERION_SAMPLE_SIZE=100 cargo bench --bench performance_benchmarks

# Benchmark with profiling
cargo bench --bench performance_benchmarks --features criterion/html_reports
```

## Interpreting Results

### Expected Performance Ranges

Based on Week 3 targets, expect these performance ranges:

#### Data Storage (TimescaleDB)
- Single Insert: 10-30ms
- Batch Insert: 20-45ms  
- Time Range Query: 15-35ms
- Target: < 50ms ✅

#### Cache Operations (Redis)
- SET Operations: 1-3ms
- GET Operations: 0.5-2ms
- Prediction Cache: 2-4ms
- Target: < 5ms ✅

#### Neural Predictions (FANN)
- Single Prediction: 70-90ms
- Batch Predictions: 80-95ms
- Model Selection: 40-50ms
- Target: < 100ms ✅

#### Agent Decisions (DAA)
- Single Decision: 80-95ms
- Multi-Agent: 85-100ms
- Streaming: 70-85ms
- Target: < 100ms ✅

### Performance Regression Detection

Monitor these key indicators for performance degradation:

1. **P95 Latency Increase**: > 20% increase in 95th percentile
2. **Throughput Decline**: > 15% decrease in operations per second
3. **Memory Growth**: Consistent memory usage increase over time
4. **Error Rate Spike**: > 1% error rate in any component

### Benchmark Result Structure

Results are stored in Memory with this structure:

```json
{
  "timestamp": "ISO-8601",
  "benchmark_suite": "neural_trading_platform_performance",
  "results": {
    "data_storage": { "operations": [...] },
    "cache_operations": { "operations": [...] },
    "neural_predictions": { "operations": [...] },
    "agent_decisions": { "operations": [...] },
    "throughput": { "metrics": {...} },
    "memory_usage": { "metrics": {...} },
    "latency_analysis": { "percentiles": {...} }
  },
  "performance_targets_summary": {
    "data_storage_target_met": true,
    "cache_operation_target_met": true,
    "neural_prediction_target_met": true,
    "agent_decision_target_met": true,
    "overall_performance": "EXCELLENT"
  }
}
```

## Troubleshooting

### Common Issues

#### Database Connection Failures
```bash
# Check TimescaleDB is running
docker ps | grep timescaledb

# Test connection
psql postgresql://postgres:password@localhost:5432/neural_trader_test -c "SELECT 1;"
```

#### Redis Connection Failures
```bash
# Check Redis is running
docker ps | grep redis

# Test connection  
redis-cli ping
```

#### Compilation Errors
```bash
# Update dependencies
cargo update

# Clean and rebuild
cargo clean && cargo build --release
```

#### Performance Target Misses

If benchmarks fail to meet targets:

1. **System Resources**: Ensure adequate CPU/memory
2. **Background Processes**: Stop unnecessary applications
3. **Database Tuning**: Optimize PostgreSQL configuration
4. **Network Latency**: Use local services only for benchmarks

### Benchmark Accuracy

For most accurate results:

1. Run on dedicated hardware
2. Close unnecessary applications
3. Use release builds: `cargo bench --release`
4. Run multiple iterations for statistical significance
5. Monitor system resources during execution

## Integration with CI/CD

### Automated Performance Testing

```yaml
# .github/workflows/performance.yml
name: Performance Benchmarks
on: [push, pull_request]

jobs:
  benchmark:
    runs-on: ubuntu-latest
    services:
      postgres:
        image: timescale/timescaledb:latest-pg14
        env:
          POSTGRES_PASSWORD: password
          POSTGRES_DB: neural_trader_test
        ports:
          - 5432:5432
      redis:
        image: redis:latest
        ports:
          - 6379:6379
    
    steps:
      - uses: actions/checkout@v3
      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Run Benchmarks
        run: ./scripts/store_benchmark_results.sh
      - name: Archive Results
        uses: actions/upload-artifact@v3
        with:
          name: benchmark-results
          path: memory/data/performance_benchmarks.json
```

### Performance Regression Alerts

Set up monitoring to alert on performance regressions:

```bash
# Performance threshold checks
if [ "$P95_LATENCY" -gt 100 ]; then
  echo "Performance regression detected!"
  exit 1
fi
```

## Memory Storage Integration

Benchmark results are automatically stored in the Memory system using the key:

```
swarm-auto-centralized-1751484080479/performance-benchmarks/results
```

This enables:
- Historical performance tracking
- Regression analysis over time
- Performance trend visualization
- Automated alerting on degradation

The stored data includes all latency measurements, throughput metrics, memory usage patterns, and target validation results for comprehensive performance analysis.

## Contributing

When adding new benchmarks:

1. Follow existing naming conventions
2. Include comprehensive documentation
3. Validate against realistic data sizes
4. Ensure proper cleanup after tests
5. Update this documentation

For performance optimization suggestions or benchmark improvements, please submit issues with detailed performance analysis and suggested improvements.