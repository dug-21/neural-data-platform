# Config-Store Comprehensive Test Plan

## Test Philosophy

**Test-Driven Development (TDD)**: Every feature begins with a failing test. No production code without a test first.

## Test Coverage Requirements

- **Business Logic**: 100% coverage required
- **Error Paths**: All error conditions must be tested
- **Edge Cases**: Boundary conditions and limits
- **Concurrency**: Race conditions and thread safety
- **Performance**: Latency and throughput benchmarks

## 1. Unit Test Suite

### 1.1 ConfigStore Trait Tests

```rust
// config-store/tests/unit/config_store_trait_tests.rs

#[cfg(test)]
mod config_store_trait_tests {
    use super::*;
    use async_trait::async_trait;
    
    /// Verify trait is Send + Sync for async usage
    #[test]
    fn test_trait_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Box<dyn ConfigStore>>();
    }
    
    /// Test path validation
    #[tokio::test]
    async fn test_path_validation() {
        let store = create_test_store();
        
        // Valid paths
        assert!(store.get("/valid/path").await.is_ok_or_not_found());
        assert!(store.get("/system/global/config").await.is_ok_or_not_found());
        
        // Invalid paths
        assert_matches!(
            store.get("missing-slash").await,
            Err(ConfigError::InvalidPath(_))
        );
        assert_matches!(
            store.get("/path//double-slash").await,
            Err(ConfigError::InvalidPath(_))
        );
        assert_matches!(
            store.get("/path/with spaces").await,
            Err(ConfigError::InvalidPath(_))
        );
    }
    
    /// Test CRUD operations
    #[tokio::test]
    async fn test_crud_operations() {
        let store = create_test_store();
        let path = "/test/crud";
        let value = json!({"key": "value", "number": 42});
        
        // Create
        assert!(store.set(path, value.clone()).await.is_ok());
        
        // Read
        let retrieved = store.get(path).await.unwrap();
        assert_eq!(retrieved, value);
        
        // Update
        let updated = json!({"key": "updated", "number": 100});
        assert!(store.set(path, updated.clone()).await.is_ok());
        let retrieved = store.get(path).await.unwrap();
        assert_eq!(retrieved, updated);
        
        // Delete
        assert!(store.delete(path).await.is_ok());
        assert_matches!(
            store.get(path).await,
            Err(ConfigError::NotFound(_))
        );
    }
    
    /// Test hierarchical paths
    #[tokio::test]
    async fn test_hierarchical_paths() {
        let store = create_test_store();
        
        // Set nested configurations
        store.set("/app/database/host", json!("localhost")).await.unwrap();
        store.set("/app/database/port", json!(5432)).await.unwrap();
        store.set("/app/cache/ttl", json!(60)).await.unwrap();
        
        // Get tree
        let tree = store.get_tree("/app").await.unwrap();
        assert_eq!(tree.keys().count(), 3);
        assert!(tree.contains_key("/app/database/host"));
        assert!(tree.contains_key("/app/database/port"));
        assert!(tree.contains_key("/app/cache/ttl"));
        
        // Get subtree
        let subtree = store.get_tree("/app/database").await.unwrap();
        assert_eq!(subtree.keys().count(), 2);
    }
    
    /// Test version tracking
    #[tokio::test]
    async fn test_versioning() {
        let store = create_test_store();
        let path = "/test/versioned";
        
        // Create versions
        for i in 1..=5 {
            store.set(path, json!({"version": i})).await.unwrap();
        }
        
        // Get specific version
        let v3 = store.get_version(path, 3).await.unwrap();
        assert_eq!(v3["version"], 3);
        
        // Get history
        let history = store.get_history(path).await.unwrap();
        assert_eq!(history.len(), 5);
        assert_eq!(history[0].version, 1);
        assert_eq!(history[4].version, 5);
    }
    
    /// Test atomic transactions
    #[tokio::test]
    async fn test_transactions() {
        let store = create_test_store();
        
        // Successful transaction
        let result = store.transaction(|tx| async {
            tx.set("/tx/1", json!(1)).await?;
            tx.set("/tx/2", json!(2)).await?;
            tx.set("/tx/3", json!(3)).await?;
            Ok(())
        }).await;
        
        assert!(result.is_ok());
        assert_eq!(store.get("/tx/1").await.unwrap(), json!(1));
        assert_eq!(store.get("/tx/2").await.unwrap(), json!(2));
        assert_eq!(store.get("/tx/3").await.unwrap(), json!(3));
        
        // Failed transaction (rollback)
        let result = store.transaction(|tx| async {
            tx.set("/tx/4", json!(4)).await?;
            tx.set("/tx/5", json!(5)).await?;
            Err(ConfigError::Custom("Simulated failure".to_string()))
        }).await;
        
        assert!(result.is_err());
        assert!(store.get("/tx/4").await.is_err());
        assert!(store.get("/tx/5").await.is_err());
    }
}
```

### 1.2 InMemoryConfigStore Tests

```rust
// config-store/tests/unit/in_memory_store_tests.rs

#[cfg(test)]
mod in_memory_store_tests {
    use super::*;
    use std::sync::Arc;
    
    #[test]
    fn test_new_store_is_empty() {
        let store = InMemoryConfigStore::new();
        assert_eq!(store.size(), 0);
        assert!(store.is_empty());
    }
    
    #[tokio::test]
    async fn test_isolation_between_instances() {
        let store1 = InMemoryConfigStore::new();
        let store2 = InMemoryConfigStore::new();
        
        store1.set("/shared", json!("store1")).await.unwrap();
        store2.set("/shared", json!("store2")).await.unwrap();
        
        assert_eq!(store1.get("/shared").await.unwrap(), json!("store1"));
        assert_eq!(store2.get("/shared").await.unwrap(), json!("store2"));
    }
    
    #[tokio::test]
    async fn test_concurrent_access_safety() {
        let store = Arc::new(InMemoryConfigStore::new());
        let mut handles = vec![];
        
        // Spawn 100 concurrent writers
        for i in 0..100 {
            let store_clone = store.clone();
            handles.push(tokio::spawn(async move {
                let path = format!("/concurrent/{}", i);
                store_clone.set(&path, json!({"id": i})).await.unwrap();
            }));
        }
        
        // Spawn 100 concurrent readers
        for i in 0..100 {
            let store_clone = store.clone();
            handles.push(tokio::spawn(async move {
                let path = format!("/concurrent/{}", i % 50);
                let _ = store_clone.get(&path).await;
            }));
        }
        
        // Wait for all operations
        for handle in handles {
            handle.await.unwrap();
        }
        
        // Verify all writes succeeded
        assert_eq!(store.size(), 100);
    }
    
    #[test]
    fn test_snapshot_and_restore() {
        let store1 = InMemoryConfigStore::new();
        
        // Populate store
        store1.set("/config/a", json!(1)).await.unwrap();
        store1.set("/config/b", json!(2)).await.unwrap();
        store1.set("/config/c", json!(3)).await.unwrap();
        
        // Create snapshot
        let snapshot = store1.snapshot();
        
        // Restore to new store
        let store2 = InMemoryConfigStore::from_snapshot(snapshot);
        
        // Verify identical state
        assert_eq!(store2.size(), 3);
        assert_eq!(store2.get("/config/a").await.unwrap(), json!(1));
        assert_eq!(store2.get("/config/b").await.unwrap(), json!(2));
        assert_eq!(store2.get("/config/c").await.unwrap(), json!(3));
    }
    
    #[tokio::test]
    async fn test_memory_limits() {
        let store = InMemoryConfigStore::with_limits(InMemoryLimits {
            max_entries: 10,
            max_memory_mb: 1,
        });
        
        // Fill to limit
        for i in 0..10 {
            let path = format!("/limited/{}", i);
            assert!(store.set(&path, json!(i)).await.is_ok());
        }
        
        // Exceed limit
        assert_matches!(
            store.set("/limited/11", json!(11)).await,
            Err(ConfigError::StorageLimitExceeded(_))
        );
    }
}
```

### 1.3 RedisConfigStore Tests

```rust
// config-store/tests/unit/redis_store_tests.rs

#[cfg(test)]
mod redis_store_tests {
    use super::*;
    use mockall::predicate::*;
    use mockall::mock;
    
    mock! {
        RedisConnection {
            fn get(&mut self, key: &str) -> Result<Option<String>, redis::RedisError>;
            fn set(&mut self, key: &str, value: &str) -> Result<(), redis::RedisError>;
            fn del(&mut self, key: &str) -> Result<(), redis::RedisError>;
        }
    }
    
    #[tokio::test]
    async fn test_cache_hit_performance() {
        let store = create_redis_store_with_cache();
        let path = "/cached/value";
        
        // First call - cache miss
        let start = Instant::now();
        store.set(path, json!("test")).await.unwrap();
        let value = store.get(path).await.unwrap();
        let first_duration = start.elapsed();
        
        // Second call - cache hit
        let start = Instant::now();
        let cached_value = store.get(path).await.unwrap();
        let second_duration = start.elapsed();
        
        assert_eq!(value, cached_value);
        assert!(second_duration < first_duration / 10); // Cache should be 10x faster
    }
    
    #[tokio::test]
    async fn test_cache_invalidation_on_update() {
        let store = create_redis_store_with_cache();
        let path = "/cache/test";
        
        // Set and cache
        store.set(path, json!("initial")).await.unwrap();
        let _ = store.get(path).await.unwrap();
        
        // Update should invalidate cache
        store.set(path, json!("updated")).await.unwrap();
        
        // Next get should return updated value
        let value = store.get(path).await.unwrap();
        assert_eq!(value, json!("updated"));
    }
    
    #[tokio::test]
    async fn test_connection_pool_limits() {
        let config = RedisStoreConfig {
            pool_size: 5,
            min_idle: 2,
            ..Default::default()
        };
        
        let store = RedisConfigStore::new("redis://localhost", config).await.unwrap();
        
        // Spawn more concurrent operations than pool size
        let mut handles = vec![];
        for i in 0..20 {
            let store_clone = store.clone();
            handles.push(tokio::spawn(async move {
                let path = format!("/pool/{}", i);
                store_clone.set(&path, json!(i)).await
            }));
        }
        
        // All should complete successfully despite pool limit
        for handle in handles {
            assert!(handle.await.unwrap().is_ok());
        }
    }
    
    #[tokio::test]
    async fn test_version_history_limit() {
        let store = create_redis_store();
        let path = "/versioned/config";
        
        // Create more versions than limit (10)
        for i in 1..=15 {
            store.set(path, json!({"version": i})).await.unwrap();
        }
        
        // History should only contain last 10 versions
        let history = store.get_history(path).await.unwrap();
        assert_eq!(history.len(), 10);
        assert_eq!(history[0].version, 6);  // Oldest
        assert_eq!(history[9].version, 15); // Newest
    }
}
```

### 1.4 Schema Validation Tests

```rust
// config-store/tests/unit/schema_validation_tests.rs

#[cfg(test)]
mod schema_validation_tests {
    use super::*;
    
    #[test]
    fn test_json_schema_validation() {
        let mut validator = SchemaValidator::new();
        
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "age": {"type": "number", "minimum": 0, "maximum": 150},
                "email": {"type": "string", "format": "email"}
            },
            "required": ["name", "age"]
        });
        
        validator.register_schema("/person/*", schema).unwrap();
        
        // Valid data
        let valid = json!({
            "name": "John Doe",
            "age": 30,
            "email": "john@example.com"
        });
        assert!(validator.validate("/person/1", &valid).is_ok());
        
        // Invalid: missing required field
        let invalid_missing = json!({
            "name": "Jane Doe"
        });
        assert!(validator.validate("/person/2", &invalid_missing).is_err());
        
        // Invalid: wrong type
        let invalid_type = json!({
            "name": "Bob",
            "age": "thirty"
        });
        assert!(validator.validate("/person/3", &invalid_type).is_err());
        
        // Invalid: out of range
        let invalid_range = json!({
            "name": "Alice",
            "age": 200
        });
        assert!(validator.validate("/person/4", &invalid_range).is_err());
    }
    
    #[test]
    fn test_custom_validator() {
        let mut validator = SchemaValidator::new();
        
        // Register custom trading hours validator
        validator.register_custom_validator(
            "/trading_hours/*",
            Box::new(TradingHoursValidator)
        );
        
        // Valid trading hours
        let valid = json!({
            "market_open": "09:30",
            "market_close": "16:00"
        });
        assert!(validator.validate("/trading_hours/nyse", &valid).is_ok());
        
        // Invalid format
        let invalid_format = json!({
            "market_open": "9:30AM",
            "market_close": "4PM"
        });
        assert!(validator.validate("/trading_hours/nasdaq", &invalid_format).is_err());
        
        // Invalid logic (close before open)
        let invalid_logic = json!({
            "market_open": "16:00",
            "market_close": "09:30"
        });
        assert!(validator.validate("/trading_hours/test", &invalid_logic).is_err());
    }
}
```

## 2. Integration Test Suite

### 2.1 Redis Integration Tests

```rust
// config-store/tests/integration/redis_integration_tests.rs

#[cfg(test)]
mod redis_integration_tests {
    use testcontainers::{clients, images::redis::Redis, Container};
    
    async fn setup_redis() -> (Container<'_, Redis>, String) {
        let docker = clients::Cli::default();
        let container = docker.run(Redis::default());
        let port = container.get_host_port(6379);
        let url = format!("redis://localhost:{}", port);
        (container, url)
    }
    
    #[tokio::test]
    async fn test_redis_connection_and_ping() {
        let (_container, url) = setup_redis().await;
        
        let store = RedisConfigStore::new(&url, Default::default())
            .await
            .expect("Should connect to Redis");
            
        assert!(store.ping().await.is_ok());
    }
    
    #[tokio::test]
    async fn test_redis_persistence_across_connections() {
        let (_container, url) = setup_redis().await;
        
        // First connection - write data
        {
            let store = RedisConfigStore::new(&url, Default::default()).await.unwrap();
            store.set("/persistent/data", json!({
                "value": "should persist",
                "number": 42
            })).await.unwrap();
        }
        
        // Second connection - read data
        {
            let store = RedisConfigStore::new(&url, Default::default()).await.unwrap();
            let value = store.get("/persistent/data").await.unwrap();
            assert_eq!(value["value"], "should persist");
            assert_eq!(value["number"], 42);
        }
    }
    
    #[tokio::test]
    async fn test_redis_atomic_transactions() {
        let (_container, url) = setup_redis().await;
        let store = RedisConfigStore::new(&url, Default::default()).await.unwrap();
        
        // Successful transaction
        let result = store.transaction(|tx| async {
            tx.set("/tx/account/1", json!({"balance": 1000})).await?;
            tx.set("/tx/account/2", json!({"balance": 500})).await?;
            tx.set("/tx/total", json!(1500)).await?;
            Ok(())
        }).await;
        
        assert!(result.is_ok());
        assert_eq!(store.get("/tx/total").await.unwrap(), json!(1500));
        
        // Failed transaction - should rollback all
        let result = store.transaction(|tx| async {
            tx.set("/tx/account/1", json!({"balance": 800})).await?;
            tx.set("/tx/account/2", json!({"balance": 700})).await?;
            // Simulate error
            Err(ConfigError::Custom("Insufficient funds".to_string()))
        }).await;
        
        assert!(result.is_err());
        // Values should remain unchanged
        assert_eq!(store.get("/tx/account/1").await.unwrap()["balance"], 1000);
        assert_eq!(store.get("/tx/account/2").await.unwrap()["balance"], 500);
    }
    
    #[tokio::test]
    async fn test_redis_pub_sub_notifications() {
        let (_container, url) = setup_redis().await;
        let store = RedisConfigStore::new(&url, Default::default()).await.unwrap();
        
        // Subscribe to changes
        let mut subscriber = store.subscribe("/notifications/*").await.unwrap();
        
        // Make changes
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            store.set("/notifications/test", json!("changed")).await.unwrap();
        });
        
        // Receive notification
        let notification = tokio::time::timeout(
            Duration::from_secs(1),
            subscriber.recv()
        ).await.unwrap().unwrap();
        
        assert_eq!(notification.path, "/notifications/test");
        assert_eq!(notification.new_value, Some(json!("changed")));
    }
}
```

### 2.2 gRPC Service Integration Tests

```rust
// config-store/tests/integration/grpc_integration_tests.rs

#[cfg(test)]
mod grpc_integration_tests {
    use tonic::transport::Channel;
    
    async fn start_test_server() -> String {
        let store = InMemoryConfigStore::new();
        let addr = "127.0.0.1:0".parse().unwrap();
        
        let server = ConfigStoreServiceImpl::new(store);
        let svc = ConfigStoreServiceServer::new(server);
        
        let listener = TcpListener::bind(addr).await.unwrap();
        let addr = listener.local_addr().unwrap();
        
        tokio::spawn(async move {
            Server::builder()
                .add_service(svc)
                .serve_with_incoming(TcpListenerStream::new(listener))
                .await
                .unwrap();
        });
        
        format!("http://{}", addr)
    }
    
    #[tokio::test]
    async fn test_grpc_get_config() {
        let addr = start_test_server().await;
        let mut client = ConfigStoreServiceClient::connect(addr).await.unwrap();
        
        // Set a value first
        let set_request = SetConfigRequest {
            namespace_path: "/test".to_string(),
            key: "grpc_key".to_string(),
            value: Some(ConfigValue {
                r#type: ValueType::String as i32,
                string_value: "test_value".to_string(),
                ..Default::default()
            }),
            change_reason: "Test".to_string(),
            ..Default::default()
        };
        
        client.set_config(set_request).await.unwrap();
        
        // Get the value
        let get_request = GetConfigRequest {
            namespace_path: "/test".to_string(),
            key: "grpc_key".to_string(),
            include_metadata: true,
            ..Default::default()
        };
        
        let response = client.get_config(get_request).await.unwrap();
        let response = response.into_inner();
        
        assert!(response.success);
        assert_eq!(response.key, "grpc_key");
        assert!(response.value.is_some());
        assert!(response.metadata.is_some());
    }
    
    #[tokio::test]
    async fn test_grpc_watch_config() {
        let addr = start_test_server().await;
        let mut client = ConfigStoreServiceClient::connect(addr.clone()).await.unwrap();
        
        // Start watching
        let watch_request = WatchConfigRequest {
            namespace_path: "/watch".to_string(),
            keys: vec!["key1".to_string(), "key2".to_string()],
            include_initial_values: false,
        };
        
        let mut stream = client.watch_config(watch_request).await.unwrap().into_inner();
        
        // Make changes
        let mut update_client = ConfigStoreServiceClient::connect(addr).await.unwrap();
        
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            
            let set_request = SetConfigRequest {
                namespace_path: "/watch".to_string(),
                key: "key1".to_string(),
                value: Some(ConfigValue {
                    r#type: ValueType::String as i32,
                    string_value: "updated".to_string(),
                    ..Default::default()
                }),
                change_reason: "Test update".to_string(),
                ..Default::default()
            };
            
            update_client.set_config(set_request).await.unwrap();
        });
        
        // Receive change event
        let event = tokio::time::timeout(
            Duration::from_secs(1),
            stream.message()
        ).await.unwrap().unwrap().unwrap();
        
        assert_eq!(event.key, "key1");
        assert_eq!(event.change_type, ChangeType::Updated as i32);
        assert!(event.new_value.is_some());
    }
}
```

## 3. Performance Test Suite

### 3.1 Benchmark Tests

```rust
// config-store/benches/performance_benchmarks.rs

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

fn benchmark_read_operations(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let store = runtime.block_on(create_populated_store(1000));
    
    let mut group = c.benchmark_group("read_operations");
    
    // Single read latency
    group.bench_function("single_read", |b| {
        b.to_async(&runtime).iter(|| async {
            store.get(black_box("/bench/item_500")).await.unwrap()
        });
    });
    
    // Cached read latency
    group.bench_function("cached_read", |b| {
        let path = "/bench/cached";
        runtime.block_on(store.get(path)).unwrap(); // Prime cache
        
        b.to_async(&runtime).iter(|| async {
            store.get(black_box(path)).await.unwrap()
        });
    });
    
    // Concurrent reads
    for concurrency in [10, 50, 100, 500].iter() {
        group.bench_with_input(
            BenchmarkId::new("concurrent_reads", concurrency),
            concurrency,
            |b, &concurrency| {
                b.to_async(&runtime).iter(|| async {
                    let mut handles = vec![];
                    for i in 0..concurrency {
                        let store = store.clone();
                        handles.push(tokio::spawn(async move {
                            let path = format!("/bench/item_{}", i % 1000);
                            store.get(&path).await.unwrap()
                        }));
                    }
                    for handle in handles {
                        handle.await.unwrap();
                    }
                });
            },
        );
    }
    
    group.finish();
}

fn benchmark_write_operations(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    
    let mut group = c.benchmark_group("write_operations");
    
    // Single write latency
    group.bench_function("single_write", |b| {
        let store = runtime.block_on(create_test_store());
        let mut counter = 0;
        
        b.to_async(&runtime).iter(|| {
            counter += 1;
            let path = format!("/bench/write_{}", counter);
            async move {
                store.set(black_box(&path), json!({"value": counter})).await.unwrap()
            }
        });
    });
    
    // Write with validation
    group.bench_function("validated_write", |b| {
        let store = runtime.block_on(create_store_with_validation());
        let mut counter = 0;
        
        b.to_async(&runtime).iter(|| {
            counter += 1;
            let path = "/validated/config";
            let value = json!({
                "name": format!("test_{}", counter),
                "age": counter % 100
            });
            async move {
                store.set(black_box(path), value).await.unwrap()
            }
        });
    });
    
    group.finish();
}

fn benchmark_cache_effectiveness(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let store = runtime.block_on(create_redis_store_with_cache());
    
    c.bench_function("cache_hit_rate", |b| {
        runtime.block_on(async {
            // Populate cache
            for i in 0..100 {
                let path = format!("/cache/item_{}", i);
                store.set(&path, json!(i)).await.unwrap();
                store.get(&path).await.unwrap(); // Prime cache
            }
        });
        
        b.to_async(&runtime).iter(|| async {
            // 90% cache hits (reading same 100 items)
            // 10% cache misses (reading items 100-110)
            let item = rand::random::<usize>() % 110;
            let path = format!("/cache/item_{}", item);
            let _ = store.get(black_box(&path)).await;
        });
        
        let metrics = store.get_metrics();
        let hit_rate = metrics.cache_hits as f64 / 
                      (metrics.cache_hits + metrics.cache_misses) as f64;
        
        assert!(hit_rate > 0.85, "Cache hit rate {} below 85%", hit_rate);
    });
}

criterion_group!(
    benches,
    benchmark_read_operations,
    benchmark_write_operations,
    benchmark_cache_effectiveness
);
criterion_main!(benches);
```

### 3.2 Load Tests

```rust
// config-store/tests/load/load_tests.rs

#[cfg(test)]
mod load_tests {
    use std::time::Instant;
    
    #[tokio::test]
    async fn test_10k_reads_per_second() {
        let store = create_production_store().await;
        
        // Populate test data
        for i in 0..1000 {
            let path = format!("/load/item_{}", i);
            store.set(&path, json!({"id": i})).await.unwrap();
        }
        
        let start = Instant::now();
        let mut handles = vec![];
        
        // Launch 10,000 reads
        for i in 0..10_000 {
            let store = store.clone();
            handles.push(tokio::spawn(async move {
                let path = format!("/load/item_{}", i % 1000);
                store.get(&path).await
            }));
        }
        
        // Wait for completion
        let mut successes = 0;
        let mut failures = 0;
        
        for handle in handles {
            match handle.await.unwrap() {
                Ok(_) => successes += 1,
                Err(_) => failures += 1,
            }
        }
        
        let duration = start.elapsed();
        let rate = 10_000.0 / duration.as_secs_f64();
        
        println!("Load test results:");
        println!("  Duration: {:?}", duration);
        println!("  Successes: {}", successes);
        println!("  Failures: {}", failures);
        println!("  Rate: {:.2} reads/second", rate);
        
        assert!(successes >= 9_900, "Success rate below 99%");
        assert!(rate >= 10_000.0, "Rate {} below 10,000 reads/second", rate);
    }
    
    #[tokio::test]
    async fn test_1k_writes_per_second() {
        let store = create_production_store().await;
        
        let start = Instant::now();
        let mut handles = vec![];
        
        // Launch 1,000 writes
        for i in 0..1_000 {
            let store = store.clone();
            handles.push(tokio::spawn(async move {
                let path = format!("/load/write_{}", i);
                store.set(&path, json!({"id": i, "timestamp": Instant::now()})).await
            }));
        }
        
        // Wait for completion
        for handle in handles {
            handle.await.unwrap().unwrap();
        }
        
        let duration = start.elapsed();
        let rate = 1_000.0 / duration.as_secs_f64();
        
        println!("Write load test:");
        println!("  Duration: {:?}", duration);
        println!("  Rate: {:.2} writes/second", rate);
        
        assert!(rate >= 1_000.0, "Rate {} below 1,000 writes/second", rate);
    }
    
    #[tokio::test]
    async fn test_latency_percentiles() {
        let store = create_production_store().await;
        let mut read_latencies = vec![];
        let mut write_latencies = vec![];
        
        // Measure 1000 reads
        for i in 0..1000 {
            let path = format!("/latency/read_{}", i % 100);
            let start = Instant::now();
            let _ = store.get(&path).await;
            read_latencies.push(start.elapsed().as_millis());
        }
        
        // Measure 100 writes
        for i in 0..100 {
            let path = format!("/latency/write_{}", i);
            let start = Instant::now();
            store.set(&path, json!(i)).await.unwrap();
            write_latencies.push(start.elapsed().as_millis());
        }
        
        // Calculate percentiles
        read_latencies.sort();
        write_latencies.sort();
        
        let read_p50 = read_latencies[500];
        let read_p95 = read_latencies[950];
        let read_p99 = read_latencies[990];
        
        let write_p50 = write_latencies[50];
        let write_p95 = write_latencies[95];
        let write_p99 = write_latencies[99];
        
        println!("Latency percentiles:");
        println!("  Read  P50: {}ms, P95: {}ms, P99: {}ms", read_p50, read_p95, read_p99);
        println!("  Write P50: {}ms, P95: {}ms, P99: {}ms", write_p50, write_p95, write_p99);
        
        assert!(read_p95 < 10, "Read P95 latency {}ms exceeds 10ms", read_p95);
        assert!(write_p95 < 50, "Write P95 latency {}ms exceeds 50ms", write_p95);
    }
}
```

## 4. End-to-End Test Suite

### 4.1 Complete Lifecycle Tests

```rust
// config-store/tests/e2e/lifecycle_tests.rs

#[tokio::test]
async fn test_complete_configuration_lifecycle() {
    // Setup
    let docker = clients::Cli::default();
    let redis = docker.run(Redis::default());
    let redis_url = format!("redis://localhost:{}", redis.get_host_port(6379));
    
    // Start gRPC server
    let store = RedisConfigStore::new(&redis_url, Default::default()).await.unwrap();
    let server_addr = start_grpc_server(store).await;
    
    // Connect client
    let mut client = ConfigStoreServiceClient::connect(server_addr).await.unwrap();
    
    // 1. Create configuration
    let create_response = client.set_config(SetConfigRequest {
        namespace_path: "/app".to_string(),
        key: "database".to_string(),
        value: Some(ConfigValue {
            r#type: ValueType::Json as i32,
            json_value: Some(prost_types::Struct {
                fields: hashmap! {
                    "host".to_string() => prost_types::Value {
                        kind: Some(prost_types::value::Kind::StringValue("localhost".to_string()))
                    },
                    "port".to_string() => prost_types::Value {
                        kind: Some(prost_types::value::Kind::NumberValue(5432.0))
                    }
                }
            }),
            ..Default::default()
        }),
        change_reason: "Initial setup".to_string(),
        ..Default::default()
    }).await.unwrap();
    
    assert!(create_response.into_inner().success);
    
    // 2. Read configuration
    let read_response = client.get_config(GetConfigRequest {
        namespace_path: "/app".to_string(),
        key: "database".to_string(),
        include_metadata: true,
        ..Default::default()
    }).await.unwrap();
    
    let read = read_response.into_inner();
    assert!(read.success);
    assert!(read.value.is_some());
    assert!(read.metadata.is_some());
    
    // 3. Update configuration
    let update_response = client.set_config(SetConfigRequest {
        namespace_path: "/app".to_string(),
        key: "database".to_string(),
        value: Some(ConfigValue {
            r#type: ValueType::Json as i32,
            json_value: Some(prost_types::Struct {
                fields: hashmap! {
                    "host".to_string() => prost_types::Value {
                        kind: Some(prost_types::value::Kind::StringValue("production.db".to_string()))
                    },
                    "port".to_string() => prost_types::Value {
                        kind: Some(prost_types::value::Kind::NumberValue(5432.0))
                    },
                    "ssl".to_string() => prost_types::Value {
                        kind: Some(prost_types::value::Kind::BoolValue(true))
                    }
                }
            }),
            ..Default::default()
        }),
        change_reason: "Production deployment".to_string(),
        expected_version: "1".to_string(),
        ..Default::default()
    }).await.unwrap();
    
    assert!(update_response.into_inner().success);
    
    // 4. Verify versioning
    let history_response = client.get_audit_trail(GetAuditTrailRequest {
        namespace_path: "/app".to_string(),
        key: "database".to_string(),
        limit: 10,
        ..Default::default()
    }).await.unwrap();
    
    let audit = history_response.into_inner();
    assert!(audit.success);
    assert_eq!(audit.entries.len(), 2);
    
    // 5. Bulk operations
    let bulk_response = client.get_bulk_config(GetBulkConfigRequest {
        namespace_path: "/app".to_string(),
        keys: vec!["database".to_string(), "cache".to_string()],
        include_metadata: false,
        ..Default::default()
    }).await.unwrap();
    
    let bulk = bulk_response.into_inner();
    assert!(bulk.success);
    assert!(bulk.values.contains_key("database"));
    
    // 6. Watch for changes
    let mut watch_stream = client.watch_config(WatchConfigRequest {
        namespace_path: "/app".to_string(),
        keys: vec!["database".to_string()],
        include_initial_values: true,
    }).await.unwrap().into_inner();
    
    // Should receive initial value
    let initial = watch_stream.message().await.unwrap().unwrap();
    assert_eq!(initial.change_type, ChangeType::Created as i32);
    
    // 7. Delete configuration (in another task)
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Note: Implement delete in gRPC service
        // client.delete_config(...)
    });
    
    // Should receive delete event
    let delete_event = tokio::time::timeout(
        Duration::from_secs(1),
        watch_stream.message()
    ).await.unwrap().unwrap().unwrap();
    
    assert_eq!(delete_event.change_type, ChangeType::Deleted as i32);
}
```

## 5. Contract & Compliance Tests

### 5.1 Interface Compliance Tests

```rust
// config-store/tests/contract/trait_compliance_tests.rs

#[cfg(test)]
mod trait_compliance_tests {
    use super::*;
    
    /// Test that all ConfigStore implementations comply with the trait contract
    macro_rules! test_config_store_compliance {
        ($store:expr) => {
            // Test all required methods exist and have correct signatures
            let store = $store;
            
            // CRUD operations
            assert_async_fn_exists!(store.get(&str) -> Result<ConfigValue, ConfigError>);
            assert_async_fn_exists!(store.set(&str, ConfigValue) -> Result<(), ConfigError>);
            assert_async_fn_exists!(store.delete(&str) -> Result<(), ConfigError>);
            
            // Bulk operations
            assert_async_fn_exists!(store.get_tree(&str) -> Result<ConfigTree, ConfigError>);
            assert_async_fn_exists!(store.list_keys(&str) -> Result<Vec<String>, ConfigError>);
            
            // Versioning
            assert_async_fn_exists!(store.get_version(&str, u32) -> Result<ConfigValue, ConfigError>);
            assert_async_fn_exists!(store.get_history(&str) -> Result<Vec<ConfigVersion>, ConfigError>);
            
            // Transactions
            assert_async_fn_exists!(store.transaction(closure) -> Result<(), ConfigError>);
        };
    }
    
    #[test]
    fn test_in_memory_store_compliance() {
        test_config_store_compliance!(InMemoryConfigStore::new());
    }
    
    #[test]
    fn test_redis_store_compliance() {
        let store = RedisConfigStore::new("redis://localhost", Default::default())
            .await
            .unwrap();
        test_config_store_compliance!(store);
    }
    
    #[test]
    fn test_file_store_compliance() {
        let store = FileConfigStore::new("/tmp/config", Default::default())
            .unwrap();
        test_config_store_compliance!(store);
    }
}
```

## 6. Test Utilities & Helpers

### 6.1 Test Fixtures

```rust
// config-store/tests/common/fixtures.rs

pub fn create_test_store() -> Box<dyn ConfigStore> {
    Box::new(InMemoryConfigStore::new())
}

pub async fn create_populated_store(count: usize) -> Box<dyn ConfigStore> {
    let store = create_test_store();
    
    for i in 0..count {
        let path = format!("/test/item_{}", i);
        let value = json!({
            "id": i,
            "name": format!("Item {}", i),
            "active": i % 2 == 0
        });
        store.set(&path, value).await.unwrap();
    }
    
    store
}

pub async fn create_redis_test_store() -> RedisConfigStore {
    let docker = clients::Cli::default();
    let container = docker.run(Redis::default());
    let url = format!("redis://localhost:{}", container.get_host_port(6379));
    
    RedisConfigStore::new(&url, Default::default())
        .await
        .expect("Failed to create Redis store")
}

pub fn assert_error_type<T>(result: Result<T, ConfigError>, expected: ConfigErrorType) {
    match result {
        Err(e) if e.error_type() == expected => (),
        Err(e) => panic!("Expected error type {:?}, got {:?}", expected, e.error_type()),
        Ok(_) => panic!("Expected error type {:?}, got Ok", expected),
    }
}
```

## 7. Test Execution & CI/CD

### 7.1 Test Commands

```bash
# Run all tests
cargo test --all-features

# Run unit tests only
cargo test --lib

# Run integration tests
cargo test --test '*integration*'

# Run with coverage
cargo tarpaulin --out Html --output-dir coverage

# Run benchmarks
cargo bench

# Run specific test suite
cargo test --test redis_integration_tests

# Run with test output
cargo test -- --nocapture

# Run tests in parallel
cargo test -- --test-threads=8
```

### 7.2 CI/CD Pipeline

```yaml
# .github/workflows/test.yml
name: Test Suite

on:
  push:
    branches: [main, develop]
  pull_request:

jobs:
  test:
    runs-on: ubuntu-latest
    
    services:
      redis:
        image: redis:7-alpine
        ports:
          - 6379:6379
        options: >-
          --health-cmd "redis-cli ping"
          --health-interval 10s
          --health-timeout 5s
          --health-retries 5
    
    steps:
      - uses: actions/checkout@v3
      
      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          override: true
          components: rustfmt, clippy
      
      - name: Cache dependencies
        uses: actions/cache@v3
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
      
      - name: Run tests
        run: cargo test --all-features
        env:
          REDIS_URL: redis://localhost:6379
      
      - name: Run benchmarks
        run: cargo bench --no-run
      
      - name: Generate coverage
        run: cargo tarpaulin --out Xml
      
      - name: Upload coverage
        uses: codecov/codecov-action@v3
        with:
          files: ./cobertura.xml
```

## 8. Test Metrics & Quality Gates

### Quality Requirements

1. **Coverage Metrics**
   - Line Coverage: ≥ 90%
   - Branch Coverage: ≥ 85%
   - Function Coverage: 100%

2. **Performance Metrics**
   - Read Latency P95: < 10ms
   - Write Latency P95: < 50ms
   - Throughput: ≥ 10,000 reads/sec
   - Cache Hit Rate: > 90%

3. **Reliability Metrics**
   - Test Pass Rate: 100%
   - Flaky Test Rate: < 1%
   - Memory Leak Detection: 0 leaks

4. **Code Quality**
   - Clippy Warnings: 0
   - Format Check: Pass
   - Security Audit: Pass

## Conclusion

This comprehensive test plan ensures the config-store module meets all quality requirements with:
- 100% business logic coverage
- Performance validation
- Integration verification
- Contract compliance
- End-to-end validation

Every test is designed to catch issues early and ensure production readiness.