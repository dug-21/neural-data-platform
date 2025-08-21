# Phase 1: TDD Test Plan

## Test-Driven Development Strategy

### Core Principle
Every feature starts with a failing test. No production code without a test first.

## 1. Test Hierarchy

```
Level 1: Unit Tests (Isolated components)
├── ConfigStore Trait Tests
├── RedisConfigStore Tests  
├── InMemoryConfigStore Tests
├── ServiceConfig Tests
└── Validation Tests

Level 2: Integration Tests (Component interactions)
├── Redis Backend Integration
├── Service Integration Pattern
├── Migration from ENV
└── Cache Invalidation

Level 3: System Tests (End-to-end flows)
├── Complete Configuration Lifecycle
├── Multi-Service Configuration Sharing
└── Performance Benchmarks

Level 4: Contract Tests (Interface compliance)
├── ConfigStore Trait Compliance
├── Error Handling Contracts
└── Serialization Contracts
```

## 2. Test Implementation Order (TDD Flow)

### Phase 1A: ConfigStore Trait (Day 1)
```rust
// TEST FIRST - Write these before any implementation

#[cfg(test)]
mod config_store_trait_tests {
    use super::*;
    
    #[test]
    fn test_config_store_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Box<dyn ConfigStore>>();
    }
    
    #[async_trait::test]
    async fn test_get_existing_key() {
        let store = create_test_store();
        store.set("/test/key", json!({"value": 42})).await.unwrap();
        
        let result = store.get("/test/key").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap()["value"], 42);
    }
    
    #[async_trait::test]
    async fn test_get_nonexistent_key() {
        let store = create_test_store();
        
        let result = store.get("/nonexistent").await;
        assert!(matches!(result, Err(ConfigError::NotFound(_))));
    }
    
    #[async_trait::test]
    async fn test_set_validates_path() {
        let store = create_test_store();
        
        let result = store.set("invalid-path", json!({})).await;
        assert!(matches!(result, Err(ConfigError::InvalidPath(_))));
    }
}
```

### Phase 1B: InMemoryConfigStore (Day 2)
```rust
// TEST FIRST - In-memory implementation for testing

#[cfg(test)]
mod in_memory_store_tests {
    use super::*;
    
    #[test]
    fn test_new_store_is_empty() {
        let store = InMemoryConfigStore::new();
        assert_eq!(store.size(), 0);
    }
    
    #[async_trait::test]
    async fn test_concurrent_access_safe() {
        let store = Arc::new(InMemoryConfigStore::new());
        let mut handles = vec![];
        
        for i in 0..100 {
            let store_clone = store.clone();
            handles.push(tokio::spawn(async move {
                let path = format!("/test/{}", i);
                store_clone.set(&path, json!({"id": i})).await.unwrap();
            }));
        }
        
        for handle in handles {
            handle.await.unwrap();
        }
        
        assert_eq!(store.size(), 100);
    }
    
    #[test]
    fn test_snapshot_and_restore() {
        let store1 = InMemoryConfigStore::new();
        store1.set("/test", json!({"value": 1})).await.unwrap();
        
        let snapshot = store1.snapshot();
        let store2 = InMemoryConfigStore::from_snapshot(snapshot);
        
        assert_eq!(store1.get("/test").await, store2.get("/test").await);
    }
}
```

### Phase 1C: RedisConfigStore (Day 3-4)
```rust
// TEST FIRST - Redis implementation with testcontainers

#[cfg(test)]
mod redis_store_tests {
    use testcontainers::{clients, images::redis::Redis};
    
    #[async_trait::test]
    async fn test_redis_connection() {
        let docker = clients::Cli::default();
        let redis_container = docker.run(Redis::default());
        let port = redis_container.get_host_port(6379);
        
        let store = RedisConfigStore::new(&format!("redis://localhost:{}", port))
            .await
            .expect("Should connect to Redis");
            
        assert!(store.ping().await.is_ok());
    }
    
    #[async_trait::test]
    async fn test_redis_persistence() {
        let docker = clients::Cli::default();
        let redis_container = docker.run(Redis::default());
        let url = format!("redis://localhost:{}", redis_container.get_host_port(6379));
        
        // First connection - write data
        {
            let store = RedisConfigStore::new(&url).await.unwrap();
            store.set("/persistent", json!({"value": "survives"})).await.unwrap();
        }
        
        // Second connection - read data
        {
            let store = RedisConfigStore::new(&url).await.unwrap();
            let value = store.get("/persistent").await.unwrap();
            assert_eq!(value["value"], "survives");
        }
    }
    
    #[async_trait::test]
    async fn test_redis_transaction_rollback() {
        let store = create_redis_test_store().await;
        
        let result = store.transaction(|tx| async {
            tx.set("/tx/1", json!({"step": 1})).await?;
            tx.set("/tx/2", json!({"step": 2})).await?;
            Err(ConfigError::Custom("Simulated failure"))
        }).await;
        
        assert!(result.is_err());
        assert!(store.get("/tx/1").await.is_err());
        assert!(store.get("/tx/2").await.is_err());
    }
}
```

### Phase 1D: ServiceConfig Pattern (Day 5)
```rust
// TEST FIRST - Service integration pattern

#[cfg(test)]
mod service_config_tests {
    use super::*;
    
    #[derive(Debug, Deserialize, Validate)]
    struct TradingHoursConfig {
        #[validate(regex = "^\\d{2}:\\d{2}$")]
        market_open: String,
        #[validate(regex = "^\\d{2}:\\d{2}$")]
        market_close: String,
    }
    
    #[async_trait::test]
    async fn test_service_config_caching() {
        let store = Arc::new(InMemoryConfigStore::new());
        store.set("/trading/hours", json!({
            "market_open": "09:30",
            "market_close": "16:00"
        })).await.unwrap();
        
        let config = ServiceConfig::<TradingHoursConfig>::new(
            store.clone(),
            "/trading/hours",
            Duration::from_secs(60)
        );
        
        // First load - hits store
        let value1 = config.load().await.unwrap();
        
        // Modify store
        store.set("/trading/hours", json!({
            "market_open": "09:00",
            "market_close": "17:00"
        })).await.unwrap();
        
        // Second load - should use cache
        let value2 = config.load().await.unwrap();
        
        assert_eq!(value1.market_open, value2.market_open);
        assert_eq!(value1.market_open, "09:30"); // Cached value
    }
    
    #[async_trait::test]
    async fn test_service_config_validation() {
        let store = Arc::new(InMemoryConfigStore::new());
        store.set("/invalid", json!({
            "market_open": "invalid-time",
            "market_close": "16:00"
        })).await.unwrap();
        
        let config = ServiceConfig::<TradingHoursConfig>::new(
            store,
            "/invalid",
            Duration::from_secs(60)
        );
        
        let result = config.load().await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConfigError::ValidationFailed(_)));
    }
}
```

## 3. Test Data Fixtures

### 3.1 Standard Test Configuration Tree
```yaml
/system/global/trading_hours:
  us_equity:
    market_open: "09:30"
    market_close: "16:00"
    timezone: "America/New_York"

/domain/trading/risk_limits:
  max_position_pct: 0.05
  max_daily_loss_pct: 0.02
  stop_loss_pct: 0.05

/services/data_ingestion/config:
  inherits: ["/system/global/trading_hours"]
  overrides:
    buffer_minutes: 5
```

### 3.2 Test Helper Functions
```rust
pub fn create_test_store() -> Box<dyn ConfigStore> {
    Box::new(InMemoryConfigStore::new())
}

pub async fn create_redis_test_store() -> RedisConfigStore {
    // Use testcontainers for isolated Redis
    let docker = clients::Cli::default();
    let container = docker.run(Redis::default());
    RedisConfigStore::new(&format!("redis://localhost:{}", 
        container.get_host_port(6379))).await.unwrap()
}

pub fn load_test_fixtures(store: &dyn ConfigStore) {
    // Load standard test data
}
```

## 4. Performance Test Suite

```rust
#[cfg(test)]
mod performance_tests {
    use criterion::{black_box, criterion_group, criterion_main, Criterion};
    
    fn benchmark_get_operation(c: &mut Criterion) {
        let store = create_test_store();
        // Preload 1000 keys
        for i in 0..1000 {
            store.set(&format!("/perf/key{}", i), json!({"id": i})).await.unwrap();
        }
        
        c.bench_function("config_get", |b| {
            b.iter(|| {
                store.get(black_box("/perf/key500")).await
            })
        });
    }
    
    fn benchmark_cache_hit_rate(c: &mut Criterion) {
        // Measure cache effectiveness
    }
    
    criterion_group!(benches, benchmark_get_operation, benchmark_cache_hit_rate);
    criterion_main!(benches);
}
```

## 5. Test Coverage Requirements

### Minimum Coverage Targets
- **Unit Tests**: 100% of public API
- **Integration Tests**: 100% of critical paths
- **Error Paths**: 100% of error conditions
- **Edge Cases**: All boundary conditions

### Coverage Report Generation
```bash
# Generate coverage report
cargo tarpaulin --out Html --output-dir coverage/

# Fail if coverage below threshold
cargo tarpaulin --fail-under 90
```

## 6. Continuous Testing

### Pre-commit Hooks
```bash
#!/bin/bash
# .git/hooks/pre-commit

# Run tests before commit
cargo test --quiet
if [ $? -ne 0 ]; then
    echo "Tests failed. Commit aborted."
    exit 1
fi

# Check coverage
cargo tarpaulin --print-summary --fail-under 90
```

### CI Pipeline
```yaml
# .github/workflows/test.yml
name: Test Suite

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    services:
      redis:
        image: redis:7-alpine
        options: >-
          --health-cmd "redis-cli ping"
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
        ports:
          - 6379:6379
    
    steps:
    - uses: actions/checkout@v2
    - uses: actions-rs/toolchain@v1
    - name: Run tests
      run: cargo test --all-features
    - name: Run integration tests
      run: cargo test --test '*' -- --test-threads=1
    - name: Generate coverage
      run: cargo tarpaulin --out Xml
    - name: Upload coverage
      uses: codecov/codecov-action@v1
```

## 7. Test Documentation

Each test should include:
1. **Purpose**: What is being tested
2. **Setup**: Required preconditions
3. **Execution**: Test steps
4. **Verification**: Expected outcomes
5. **Cleanup**: Resource cleanup

Example:
```rust
/// Tests that configuration inheritance correctly merges parent and child values.
/// 
/// Setup: Parent config with base values, child with overrides
/// Execution: Load child config with inheritance enabled
/// Verification: Child has both parent values and overrides
/// Cleanup: Automatic via test framework
#[test]
async fn test_config_inheritance() {
    // Test implementation
}
```

---

*TDD Test Plan Version*: 1.0
*Created*: 2025-01-20
*Methodology*: Red-Green-Refactor
*Coverage Target*: >90%