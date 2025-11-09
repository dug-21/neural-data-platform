# Testing Strategy for Neural Data Platform

## Executive Summary

This document defines the testing strategy for the Neural Data Platform, emphasizing **module-specific testing** for rapid development cycles, regression testing to detect drift, synthetic data generation, and an "alert and continue" approach for non-critical failures.

## Testing Philosophy

- **Module-First**: Test individual modules for fast feedback (3 min vs 16 min)
- **Start Simple**: Focus on regression testing initially
- **Alert Don't Block**: Detect issues but continue pipeline
- **Synthetic Data**: Generate realistic test data
- **Progressive Enhancement**: Add complexity over time

## Testing Modes

### Module Testing (Default for Development)
- **Scope**: Single service and its direct dependencies
- **Duration**: 2-3 minutes
- **Use Case**: Active development, quick validation
- **Coverage**: Unit + limited integration

### Platform Testing (Pre-commit/Release)
- **Scope**: All services and full integration
- **Duration**: 15-20 minutes
- **Use Case**: Pre-commit validation, release testing
- **Coverage**: Unit + integration + regression

## Testing Pyramid

```
         ╱ E2E Tests ╲          (5%)
        ╱─────────────╲
       ╱ Integration   ╲        (20%)
      ╱───────────────-─╲
     ╱  Service Tests    ╲      (25%)
    ╱─────────────────────╲
   ╱    Unit Tests         ╲    (50%)
  ╱───────────────────────-─╲
```

## Module-Specific Testing

### Module Test Suites

Each module has its own focused test suite with minimal dependencies:

| Module | Unit Tests | Integration Tests | Dependencies |
|--------|------------|-------------------|--------------|
| config-store | Config loading, gRPC API | Seed from Git, health checks | Redis, TimescaleDB |
| data-ingestion | Data fetching, validation | Redis pub, Timescale write | Redis, TimescaleDB, config-store |
| data-staging | JSON validation, proto transform | Stream consumption, EventBus publish | Redis, config-store |
| neural-ml-ops | Feature engineering, model ops | EventBus subscribe, TimescaleDB query | All infrastructure |
| neural-trading | Trading logic, risk checks | EventBus consume, order execution | Redis, config-store |

### Module Testing Workflow

```bash
# 1. Developer makes change
vim neural-trading/src/risk/manager.rs

# 2. Quick unit test (20 sec)
cargo test -p neural-trading risk::manager

# 3. Module integration (2 min)
make module-integration MODULE=neural-trading

# 4. Full module pipeline if needed (3 min)
make pipeline MODULE=neural-trading

# 5. Platform test before commit (16 min)
make platform-pipeline
```

### Module Test Isolation

```yaml
# Module test container with minimal services
version: '3.8'
services:
  # Only required dependencies
  redis:
    profiles: ["neural-trading", "all"]
  
  config-store:
    profiles: ["neural-trading", "all"]
    
  neural-trading:
    depends_on:
      - redis
      - config-store
    environment:
      TEST_MODE: "module"
```

## Test Categories

### 1. Unit Tests (Non-Container)
**Purpose**: Validate individual components in isolation

```rust
// Example: Testing Neural Trading predictor
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_inference_cache_hit() {
        let cache = InferenceCache::new(100);
        let result = cache.get("AAPL", &features).await;
        assert!(result.is_some());
    }
}
```

**Coverage Targets**:
- neural-core: 80%
- config-store: 75%
- data-staging: 75%
- neural-ml-ops: 70%
- neural-trading: 70%

### 2. Service Tests (Container Optional)
**Purpose**: Test service boundaries and APIs

```rust
// Test config-store gRPC API
#[tokio::test]
async fn test_config_store_service() {
    let client = ConfigStoreClient::connect("http://localhost:50051").await?;
    let request = GetServiceConfigRequest {
        service_name: "neural-trading".into(),
        environment: "test".into(),
    };
    let response = client.get_service_config(request).await?;
    assert!(response.is_ok());
}
```

### 3. Integration Tests (Full Stack)
**Purpose**: Validate service interactions

```python
# Test data pipeline flow
def test_data_pipeline_integration():
    # Publish to data-ingestion
    publisher.publish("market:AAPL", market_data)
    
    # Verify data-staging receives and transforms
    event = wait_for_eventbus_message("trading:market-data:AAPL")
    assert event.proto_format == True
    assert event.quality_score > 0.8
```

### 4. End-to-End Tests
**Purpose**: Validate complete workflows

```python
def test_trading_workflow():
    # Generate market data
    generator.create_market_event("AAPL", price=150.0)
    
    # Wait for ML features
    features = wait_for_event("ml:features:AAPL")
    
    # Verify trading decision
    signal = wait_for_event("trading:signal:AAPL")
    assert signal.action in ["BUY", "SELL", "HOLD"]
```

## Regression Testing Strategy

### Drift Detection Tests

#### 1. Schema Regression
```python
class SchemaRegressionTest:
    def test_eventbus_message_schema(self):
        """Detect proto message schema changes"""
        current = load_proto_schema("EventEnvelope")
        baseline = load_baseline_schema("EventEnvelope")
        
        differences = compare_schemas(current, baseline)
        if differences:
            alert_drift("Schema drift detected", differences)
            # Continue pipeline - don't fail
```

#### 2. Performance Regression
```python
class PerformanceRegressionTest:
    def test_inference_latency(self):
        """Detect performance degradation"""
        latency = measure_inference_latency()
        baseline = load_baseline_metric("inference_latency")
        
        if latency > baseline * 1.2:  # 20% degradation
            alert_drift(f"Performance regression: {latency}ms vs {baseline}ms")
```

#### 3. Configuration Drift
```python
class ConfigurationDriftTest:
    def test_config_consistency(self):
        """Detect config drift across services"""
        configs = {}
        for service in SERVICES:
            configs[service] = config_store.get_config(service)
        
        inconsistencies = validate_cross_service_config(configs)
        if inconsistencies:
            alert_drift("Configuration inconsistency", inconsistencies)
```

#### 4. Data Quality Regression
```python
class DataQualityRegressionTest:
    def test_data_quality_scores(self):
        """Monitor data quality trends"""
        scores = run_quality_pipeline(synthetic_data)
        baseline = load_baseline_quality_scores()
        
        for metric, score in scores.items():
            if score < baseline[metric] * 0.9:
                alert_drift(f"Data quality degradation: {metric}")
```

## Synthetic Data Generation

### Market Data Generator
```python
class MarketDataGenerator:
    def __init__(self):
        self.base_prices = {"AAPL": 150, "GOOGL": 2800, "TSLA": 250}
        
    def generate_ohlcv(self, symbol, periods=100):
        """Generate realistic OHLCV data"""
        data = []
        price = self.base_prices[symbol]
        
        for i in range(periods):
            # Random walk with drift
            change = np.random.normal(0, price * 0.02)
            price += change
            
            data.append({
                "timestamp": time.time() - (periods - i) * 60,
                "open": price * 0.99,
                "high": price * 1.01,
                "low": price * 0.98,
                "close": price,
                "volume": np.random.randint(1000000, 10000000)
            })
        return data
```

### Event Generator
```python
class EventGenerator:
    def generate_proto_event(self, event_type, payload):
        """Generate proto-formatted events"""
        event = EventEnvelope()
        event.event_id = str(uuid.uuid4())
        event.event_type = event_type
        event.timestamp = int(time.time() * 1000)
        event.payload.Pack(payload)
        return event
```

### Configuration Generator
```python
class ConfigGenerator:
    def generate_service_config(self, service, env="test"):
        """Generate test configurations"""
        base_config = load_template(f"{service}.yaml")
        
        # Randomize within bounds
        config = deepcopy(base_config)
        config['batch_size'] = random.randint(10, 100)
        config['timeout_ms'] = random.randint(100, 5000)
        
        return config
```

## Alert and Continue Strategy

### Alert Mechanism
```python
class DriftAlerter:
    def alert_drift(self, category, details, severity="warning"):
        """Alert on drift but don't fail pipeline"""
        
        # Log to file
        with open("drift-report.json", "a") as f:
            json.dump({
                "timestamp": time.time(),
                "category": category,
                "severity": severity,
                "details": details
            }, f)
        
        # Send notification (future)
        if severity == "critical":
            notify_slack(f"Critical drift: {category}")
        
        # Don't raise exception - continue pipeline
        print(f"⚠️  Drift detected: {category}")
```

### Severity Levels
- **Info**: Minor deviations, log only
- **Warning**: Notable changes, alert team
- **Critical**: Major issues, immediate attention
- **Fatal**: Pipeline must stop (rare)

## Test Data Management

### Data Fixtures
```
tests/fixtures/
├── market_data/
│   ├── bull_market.json
│   ├── bear_market.json
│   └── volatile_market.json
├── events/
│   ├── valid_events.proto
│   └── invalid_events.proto
└── configs/
    ├── minimal.yaml
    ├── standard.yaml
    └── stress.yaml
```

### Data Scenarios
1. **Happy Path**: Normal market conditions
2. **Edge Cases**: Extreme values, missing data
3. **Error Cases**: Invalid formats, timeouts
4. **Stress Cases**: High volume, rapid changes

## Test Environments

### Local Testing
```bash
# Unit tests without Docker
cargo test --workspace

# Integration with Docker
docker-compose -f docker-compose.test.yml up --abort-on-container-exit
```

### CI Testing
```bash
# Full pipeline with test config
CONFIG_ENV=test make pipeline
```

## Performance Testing

### Load Testing
```python
def test_eventbus_throughput():
    """Test EventBus can handle target load"""
    target_rate = 10000  # events/sec
    duration = 60  # seconds
    
    actual_rate = load_test_eventbus(target_rate, duration)
    assert actual_rate > target_rate * 0.95  # 95% achievement
```

### Stress Testing
```python
def test_system_under_stress():
    """Test graceful degradation"""
    # Gradually increase load
    for rate in [1000, 5000, 10000, 50000]:
        metrics = stress_test(rate)
        
        # Should degrade gracefully, not crash
        assert metrics['error_rate'] < 0.05  # 5% errors acceptable
        if metrics['latency_p99'] > 1000:  # 1 second
            alert_drift("System degrading at high load")
            break
```

## Test Automation

### Makefile Targets
```makefile
test-unit:
	cargo test --workspace --lib --bins

test-service:
	cargo test --workspace --test '*service*'

test-integration:
	docker-compose -f docker-compose.test.yml up --abort-on-container-exit test-runner

test-regression:
	python scripts/run_regression_tests.py --alert-only

test-all: test-unit test-service test-integration test-regression
```

### Continuous Testing
```yaml
# GitHub Actions (future)
on:
  schedule:
    - cron: '0 */6 * * *'  # Every 6 hours
jobs:
  regression:
    runs-on: ubuntu-latest
    steps:
      - name: Run regression tests
        run: make test-regression
```

## Test Reporting

### Coverage Reports
```bash
# Generate coverage
cargo tarpaulin --out Html --output-dir coverage/

# Python coverage
pytest --cov=. --cov-report=html:coverage/python/
```

### Drift Reports
```json
{
  "timestamp": "2024-01-15T10:00:00Z",
  "summary": {
    "total_tests": 150,
    "passed": 145,
    "drifted": 5,
    "failed": 0
  },
  "drifts": [
    {
      "category": "performance",
      "test": "inference_latency",
      "baseline": 45,
      "current": 52,
      "severity": "warning"
    }
  ]
}
```

## Best Practices

1. **Test Independence**: Tests should not depend on each other
2. **Deterministic**: Use fixed seeds for random data
3. **Fast Feedback**: Fail fast on critical issues
4. **Clean State**: Reset between test runs
5. **Meaningful Names**: Describe what is being tested
6. **Documentation**: Comment complex test logic

## Future Enhancements

### Phase 1 (Current)
- Basic regression tests
- Simple synthetic data
- Alert-only drift detection

### Phase 2
- Property-based testing
- Chaos engineering
- Advanced load testing

### Phase 3
- ML model validation
- A/B test framework
- Production traffic replay

## Next Steps

1. Implement synthetic data generators
2. Create regression test baselines
3. Set up drift alerting
4. Write initial test suites
5. Configure test environments