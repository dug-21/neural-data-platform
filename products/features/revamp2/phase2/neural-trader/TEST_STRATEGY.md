# Neural-Trader Multi-Channel Testing Strategy - Phase 2

## Testing Overview

Comprehensive testing strategy for the multi-channel Redis subscription system, ensuring fair processing, performance, and integration reliability.

## Testing Architecture

### Test Categories
1. **Unit Tests**: Individual component testing
2. **Integration Tests**: Multi-component interaction testing  
3. **Performance Tests**: Load, stress, and benchmark testing
4. **Fairness Tests**: Fair processing algorithm validation
5. **Resilience Tests**: Error handling and recovery testing
6. **End-to-End Tests**: Full system workflow testing

## Unit Testing Strategy

### 1. Multi-Channel Subscription Manager Tests
**Location**: `tests/unit/multi_channel_subscription_test.rs`

```rust
#[cfg(test)]
mod multi_channel_tests {
    use super::*;
    use mockall::predicate::*;
    
    #[tokio::test]
    async fn test_subscribe_to_multiple_symbols() -> Result<()> {
        let mut manager = create_test_manager().await?;
        let symbols = vec!["AAPL", "NVDA", "MSFT"];
        
        let result = manager.subscribe_to_symbols(symbols.clone()).await;
        assert!(result.is_ok());
        
        // Verify subscriptions created
        let subscriptions = manager.get_active_subscriptions().await;
        assert_eq!(subscriptions.len(), 3);
        assert!(subscriptions.contains_key("AAPL"));
        assert!(subscriptions.contains_key("NVDA"));
        assert!(subscriptions.contains_key("MSFT"));
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_dynamic_subscription_management() -> Result<()> {
        let mut manager = create_test_manager().await?;
        
        // Add subscription
        manager.add_symbol_subscription("TSLA").await?;
        assert!(manager.is_subscribed("TSLA").await);
        
        // Remove subscription
        manager.remove_symbol_subscription("TSLA").await?;
        assert!(!manager.is_subscribed("TSLA").await);
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_subscription_recovery_on_failure() -> Result<()> {
        let manager = create_test_manager_with_redis_failures().await?;
        
        // Simulate Redis connection failure
        simulate_redis_failure().await;
        
        // Wait for reconnection
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Verify automatic recovery
        assert!(manager.check_subscription_health("AAPL").await.is_ok());
        
        Ok(())
    }
}
```

### 2. Fair Processing Scheduler Tests
**Location**: `tests/unit/fair_processing_scheduler_test.rs`

```rust
#[cfg(test)]
mod fair_processing_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_fair_processing_enforcement() -> Result<()> {
        let mut scheduler = FairProcessingScheduler::new(
            Duration::from_secs(60), // 1-minute window
            0.20, // 20% max per symbol
        );
        
        // Simulate NVDA consuming too much processing time
        for _ in 0..1000 {
            scheduler.record_processing_time("NVDA", Duration::from_millis(10));
        }
        
        // Should throttle NVDA
        assert!(!scheduler.should_process("NVDA"));
        
        // Other symbols should still process
        assert!(scheduler.should_process("AAPL"));
        assert!(scheduler.should_process("MSFT"));
        
        Ok(())
    }
    
    #[tokio::test]  
    async fn test_throttle_recovery() -> Result<()> {
        let mut scheduler = FairProcessingScheduler::new(
            Duration::from_millis(100), // Short window for testing
            0.20,
        );
        
        // Trigger throttling
        for _ in 0..100 {
            scheduler.record_processing_time("NVDA", Duration::from_millis(5));
        }
        assert!(!scheduler.should_process("NVDA"));
        
        // Wait for window reset
        tokio::time::sleep(Duration::from_millis(150)).await;
        
        // Should recover
        assert!(scheduler.should_process("NVDA"));
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_processing_time_tracking() -> Result<()> {
        let mut scheduler = FairProcessingScheduler::new(
            Duration::from_secs(60),
            0.20,
        );
        
        // Record processing times
        scheduler.record_processing_time("AAPL", Duration::from_millis(100));
        scheduler.record_processing_time("NVDA", Duration::from_millis(150));
        scheduler.record_processing_time("MSFT", Duration::from_millis(75));
        
        let stats = scheduler.get_processing_stats().await;
        assert_eq!(stats.get("AAPL").unwrap().total_processing_time.as_millis(), 100);
        assert_eq!(stats.get("NVDA").unwrap().total_processing_time.as_millis(), 150);
        assert_eq!(stats.get("MSFT").unwrap().total_processing_time.as_millis(), 75);
        
        Ok(())
    }
}
```

### 3. Worker Pool Tests  
**Location**: `tests/unit/worker_pool_test.rs`

```rust
#[cfg(test)]
mod worker_pool_tests {
    use super::*;
    
    #[tokio::test]
    async fn test_worker_assignment_consistency() -> Result<()> {
        let worker_pool = WorkerPool::new(4).await?;
        
        // Same symbol should always go to same worker
        let worker_id_1 = worker_pool.get_worker_for_symbol("AAPL").await;
        let worker_id_2 = worker_pool.get_worker_for_symbol("AAPL").await;
        let worker_id_3 = worker_pool.get_worker_for_symbol("AAPL").await;
        
        assert_eq!(worker_id_1, worker_id_2);
        assert_eq!(worker_id_2, worker_id_3);
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_load_balancing() -> Result<()> {
        let mut worker_pool = WorkerPool::new(4).await?;
        
        // Simulate high load on one worker
        worker_pool.simulate_load("AAPL", 1000).await;
        
        // Trigger rebalancing
        worker_pool.rebalance_if_needed().await?;
        
        // Verify load distribution improved
        let load_distribution = worker_pool.get_load_distribution().await;
        let max_load = load_distribution.iter().max().unwrap();
        let min_load = load_distribution.iter().min().unwrap();
        
        assert!(max_load - min_load < 0.3); // Less than 30% difference
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_worker_failure_recovery() -> Result<()> {
        let mut worker_pool = WorkerPool::new(4).await?;
        
        // Simulate worker failure
        worker_pool.simulate_worker_failure(2).await;
        
        // Submit work that would go to failed worker
        let work_item = create_test_work_item("SYMBOL_ASSIGNED_TO_WORKER_2");
        let result = worker_pool.submit_work(work_item).await;
        
        // Should still succeed (reassigned to healthy worker)
        assert!(result.is_ok());
        
        Ok(())
    }
}
```

## Integration Testing Strategy

### 1. Redis Integration Tests
**Location**: `tests/integration/redis_multi_channel_test.rs`

```rust
#[cfg(test)]
mod redis_integration_tests {
    use super::*;
    use testcontainers::*;
    
    #[tokio::test]
    async fn test_multi_channel_subscription_flow() -> Result<()> {
        let docker = clients::Cli::default();
        let redis_container = docker.run(images::redis::Redis::default());
        let redis_port = redis_container.get_host_port_ipv4(6379);
        
        let redis_config = RedisConfig {
            host: "localhost".to_string(),
            port: redis_port,
            ..Default::default()
        };
        
        let mut redis_adapter = RedisAdapter::new(redis_config);
        redis_adapter.connect().await?;
        
        // Test multi-channel subscription
        let channels = vec!["market:AAPL", "market:NVDA", "market:MSFT"];
        let streams = redis_adapter.subscribe_multiple_channels(channels).await?;
        
        assert_eq!(streams.len(), 3);
        
        // Publish test data
        let test_data = create_test_market_data("AAPL");
        redis_adapter.publish_market_data("market:AAPL", &test_data).await?;
        
        // Verify reception
        let mut aapl_stream = streams.get("market:AAPL").unwrap();
        if let Some(Ok(received_data)) = aapl_stream.next().await {
            assert_eq!(received_data.symbol, "AAPL");
        }
        
        Ok(())
    }
    
    #[tokio::test]
    async fn test_connection_pool_efficiency() -> Result<()> {
        // Test that connection pooling works efficiently for multiple subscriptions
        // Measure connection count vs subscription count
        Ok(())
    }
    
    #[tokio::test]
    async fn test_reconnection_resilience() -> Result<()> {
        // Test automatic reconnection when Redis goes down and comes back up
        Ok(())
    }
}
```

### 2. End-to-End Integration Tests
**Location**: `tests/integration/e2e_multi_channel_test.rs`

```rust
#[tokio::test]
async fn test_complete_multi_channel_flow() -> Result<()> {
    // Setup complete system
    let system = setup_test_system().await?;
    
    // Start multi-channel subscriptions
    system.start_multi_channel_subscriptions(vec!["AAPL", "NVDA", "MSFT"]).await?;
    
    // Publish market data for all symbols
    for symbol in &["AAPL", "NVDA", "MSFT"] {
        let market_data = create_test_market_data(symbol);
        system.publish_market_data(&format!("market:{}", symbol), &market_data).await?;
    }
    
    // Wait for processing
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Verify events reached DAA coordinator
    let processed_events = system.get_processed_events().await;
    assert!(processed_events.contains_key("AAPL"));
    assert!(processed_events.contains_key("NVDA"));
    assert!(processed_events.contains_key("MSFT"));
    
    // Verify fair processing
    let processing_stats = system.get_fair_processing_stats().await;
    for (symbol, stats) in processing_stats {
        assert!(stats.processing_percentage <= 0.20); // 20% limit
    }
    
    Ok(())
}
```

## Performance Testing Strategy

### 1. Load Testing
**Location**: `tests/performance/multi_channel_load_test.rs`

```rust
#[tokio::test]
async fn test_high_throughput_processing() -> Result<()> {
    let system = setup_performance_test_system().await?;
    
    // Setup 50 symbols
    let symbols: Vec<String> = (0..50)
        .map(|i| format!("SYMBOL_{:03}", i))
        .collect();
    
    system.start_multi_channel_subscriptions(symbols.clone()).await?;
    
    // Generate high-frequency data
    let start_time = Instant::now();
    let target_events = 10_000;
    
    for _ in 0..target_events {
        for symbol in &symbols {
            let market_data = create_test_market_data(symbol);
            system.publish_market_data(&format!("market:{}", symbol), &market_data).await?;
        }
    }
    
    // Wait for processing to complete
    system.wait_for_processing_completion().await?;
    let processing_time = start_time.elapsed();
    
    // Verify performance requirements
    let events_per_second = (target_events * symbols.len()) as f64 / processing_time.as_secs_f64();
    assert!(events_per_second >= 10_000.0, "Throughput too low: {}", events_per_second);
    
    // Verify latency requirements
    let avg_latency = system.get_average_processing_latency().await;
    assert!(avg_latency.as_millis() <= 200, "Latency too high: {}ms", avg_latency.as_millis());
    
    Ok(())
}

#[tokio::test] 
async fn test_memory_usage_compliance() -> Result<()> {
    let system = setup_performance_test_system().await?;
    let initial_memory = get_memory_usage();
    
    // Subscribe to 100 symbols
    let symbols: Vec<String> = (0..100).map(|i| format!("SYM{}", i)).collect();
    system.start_multi_channel_subscriptions(symbols).await?;
    
    // Run for extended period
    for _ in 0..1000 {
        // Generate moderate load
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    
    let final_memory = get_memory_usage();
    let memory_growth = final_memory - initial_memory;
    
    // Should not exceed 500MB
    assert!(memory_growth <= 500 * 1024 * 1024, "Memory usage too high: {} bytes", memory_growth);
    
    Ok(())
}
```

### 2. Fairness Testing
**Location**: `tests/performance/fairness_validation_test.rs`

```rust
#[tokio::test]
async fn test_fair_processing_under_load() -> Result<()> {
    let system = setup_fairness_test_system().await?;
    
    // Setup symbols with different volumes
    system.start_multi_channel_subscriptions(vec!["NVDA", "AAPL", "SMALL_CAP"]).await?;
    
    // Generate heavily skewed load (NVDA gets 10x more messages)
    for _ in 0..1000 {
        // NVDA: 10 messages
        for _ in 0..10 {
            system.publish_market_data("market:NVDA", &create_test_market_data("NVDA")).await?;
        }
        
        // AAPL: 1 message  
        system.publish_market_data("market:AAPL", &create_test_market_data("AAPL")).await?;
        
        // SMALL_CAP: 1 message
        system.publish_market_data("market:SMALL_CAP", &create_test_market_data("SMALL_CAP")).await?;
    }
    
    // Wait for processing  
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    // Verify fairness
    let processing_stats = system.get_fair_processing_stats().await;
    
    // No symbol should exceed 20% of processing time
    for (symbol, stats) in processing_stats {
        assert!(
            stats.processing_percentage <= 0.20,
            "Symbol {} exceeded fair processing limit: {:.2}%",
            symbol,
            stats.processing_percentage * 100.0
        );
    }
    
    Ok(())
}
```

## Testing Infrastructure

### Mock Redis Server
```rust
pub struct MockRedisServer {
    channels: Arc<RwLock<HashMap<String, Vec<String>>>>,
    subscribers: Arc<RwLock<HashMap<String, Vec<mpsc::Sender<String>>>>>,
    failure_simulation: Arc<AtomicBool>,
}

impl MockRedisServer {
    pub async fn publish(&self, channel: &str, message: &str) -> Result<()> {
        if self.failure_simulation.load(Ordering::Relaxed) {
            return Err(anyhow::anyhow!("Simulated Redis failure"));
        }
        
        // Implement pub/sub simulation
        Ok(())
    }
    
    pub async fn simulate_failure(&self) {
        self.failure_simulation.store(true, Ordering::Relaxed);
    }
    
    pub async fn recover_from_failure(&self) {
        self.failure_simulation.store(false, Ordering::Relaxed);
    }
}
```

### Test Data Generators
```rust
pub fn create_test_market_data(symbol: &str) -> MarketData {
    MarketData {
        symbol: symbol.to_string(),
        timestamp: chrono::Utc::now().timestamp(),
        open: 100.0 + rand::random::<f64>() * 10.0,
        high: 105.0 + rand::random::<f64>() * 10.0,
        low: 95.0 + rand::random::<f64>() * 10.0,
        close: 100.0 + rand::random::<f64>() * 10.0,
        volume: 1_000_000.0 + rand::random::<f64>() * 500_000.0,
    }
}

pub fn create_high_volume_test_scenario() -> Vec<(String, u32)> {
    vec![
        ("NVDA".to_string(), 1000),   // High volume
        ("AAPL".to_string(), 800),    // High volume
        ("MSFT".to_string(), 600),    // Medium volume
        ("SMALL_CAP".to_string(), 10), // Low volume
    ]
}
```

## Continuous Integration

### Cargo Test Configuration
```toml
# Cargo.toml test configuration
[dev-dependencies]
tokio-test = "0.4"
mockall = "0.11"
testcontainers = "0.14"
criterion = "0.4"

[[test]]
name = "integration"
path = "tests/integration/mod.rs"

[[bench]]
name = "multi_channel_benchmarks"
harness = false
```

### Test Execution Pipeline
```bash
# Unit tests
cargo test --lib

# Integration tests (requires Redis)
cargo test --test integration

# Performance tests
cargo test --release --test performance

# Benchmarks
cargo bench

# Coverage report
cargo tarpaulin --out Html --output-dir coverage
```

### Performance Benchmarks
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_multi_channel_processing(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    c.bench_function("multi_channel_1000_events", |b| {
        b.iter(|| {
            rt.block_on(async {
                // Benchmark multi-channel processing
                black_box(process_1000_events().await);
            });
        });
    });
}

criterion_group!(benches, benchmark_multi_channel_processing);
criterion_main!(benches);
```

## Test Success Criteria

### Unit Test Coverage
- **Target**: >95% code coverage
- **Critical Paths**: 100% coverage for fair processing algorithms
- **Error Paths**: All error conditions tested

### Integration Test Goals  
- **Multi-Channel**: All subscription scenarios covered
- **Redis Integration**: Connection management and resilience
- **End-to-End**: Complete workflow validation

### Performance Test Targets
- **Throughput**: >10,000 events/second sustained
- **Latency**: <200ms average processing time  
- **Memory**: <500MB total usage for 100 symbols
- **Fairness**: No symbol >20% processing time over 1-minute windows

### Continuous Monitoring
- **Test Execution**: All tests pass on every commit
- **Performance Regression**: Benchmark results tracked
- **Memory Leaks**: Valgrind integration for leak detection
- **Fair Processing**: Continuous fairness validation

This comprehensive testing strategy ensures the multi-channel system meets all requirements for performance, fairness, and reliability while maintaining integration compatibility with existing systems.