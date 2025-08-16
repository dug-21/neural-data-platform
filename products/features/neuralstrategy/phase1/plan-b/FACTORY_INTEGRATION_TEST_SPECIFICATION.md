# Factory Pattern Integration Test Specification

## Overview

This document specifies comprehensive integration tests for validating the single factory pattern implementation that eliminates the triple factory anti-pattern.

## Test Strategy

### Core Principles
1. **Test Real Integration**: Validate actual component connections
2. **Performance Validation**: Ensure latency and throughput targets
3. **Honesty Verification**: Confirm transparent model capabilities
4. **Production Readiness**: Validate system stability under load

## Test Categories

### 1. Factory Creation Tests

#### Test: Single Factory Creates All Models
```rust
#[tokio::test]
async fn test_single_factory_creates_all_models() {
    let factory = ModelAdapterFactory::new(false);
    let config = UnifiedModelConfig::default();
    
    let model_types = vec![
        ModelType::MLP,
        ModelType::LSTM,
        ModelType::NHITS,
        ModelType::TCN,
        ModelType::DeepAR,
    ];
    
    for model_type in model_types {
        let result = factory.create_adapter(model_type.clone(), config.clone()).await;
        
        assert!(result.is_ok(), "Failed to create {:?}: {:?}", model_type, result.err());
        
        let adapter = result.unwrap();
        let info = adapter.get_info();
        
        assert_eq!(info.model_type, model_type);
        assert!(info.input_size > 0);
        assert!(info.output_size > 0);
    }
}
```

#### Test: No Mock Implementations
```rust
#[tokio::test]
async fn test_no_mock_implementations() {
    let factory = ModelAdapterFactory::new(false);
    
    // Scan factory code for mock patterns
    let factory_code = std::fs::read_to_string("src/neural/adapters/factory.rs").unwrap();
    
    assert!(!factory_code.contains("MockAdapter"));
    assert!(!factory_code.contains("unimplemented!()"));
    assert!(!factory_code.contains("todo!()"));
    assert!(!factory_code.contains("panic!(\"Not implemented\")"));
    
    // Test runtime behavior
    for model_type in [MLP, LSTM, NHITS, TCN, DeepAR] {
        let adapter = factory.create_adapter(model_type, config).await.unwrap();
        
        // Should not panic on basic operations
        let test_input = vec![0.1; 24];
        let predict_result = std::panic::catch_unwind(|| {
            adapter.predict(&test_input)
        });
        
        assert!(predict_result.is_ok(), "Model {:?} panicked on predict", model_type);
    }
}
```

### 2. Performance Tests

#### Test: Model Creation Latency
```rust
#[tokio::test]
async fn test_model_creation_latency() {
    let factory = ModelAdapterFactory::new(false);
    let config = UnifiedModelConfig::default();
    
    let mut latencies = Vec::new();
    
    // Warm up
    for _ in 0..10 {
        let _ = factory.create_adapter(ModelType::MLP, config.clone()).await;
    }
    
    // Measure 1000 iterations
    for _ in 0..1000 {
        let start = Instant::now();
        let _ = factory.create_adapter(ModelType::MLP, config.clone()).await.unwrap();
        latencies.push(start.elapsed());
    }
    
    // Calculate statistics
    latencies.sort();
    let avg_latency = latencies.iter().sum::<Duration>() / latencies.len() as u32;
    let p95_latency = latencies[950];
    let p99_latency = latencies[990];
    
    // Performance targets
    assert!(avg_latency < Duration::from_micros(100), "Avg latency {}μs > 100μs", avg_latency.as_micros());
    assert!(p95_latency < Duration::from_micros(200), "P95 latency {}μs > 200μs", p95_latency.as_micros());
    assert!(p99_latency < Duration::from_micros(300), "P99 latency {}μs > 300μs", p99_latency.as_micros());
}
```

#### Test: Prediction Performance
```rust
#[tokio::test]
async fn test_prediction_performance() {
    let factory = ModelAdapterFactory::new(false);
    let config = UnifiedModelConfig::new(ModelType::MLP, 24, 1);
    let mut adapter = factory.create_adapter(ModelType::MLP, config).await.unwrap();
    
    // Train model first
    let training_data = generate_training_data(100);
    adapter.train(&training_data.inputs, &training_data.targets).await.unwrap();
    
    // Test prediction latency
    let test_input = vec![0.5; 24];
    let mut latencies = Vec::new();
    
    for _ in 0..1000 {
        let start = Instant::now();
        let _ = adapter.predict(&test_input).await.unwrap();
        latencies.push(start.elapsed());
    }
    
    let avg_latency = latencies.iter().sum::<Duration>() / latencies.len() as u32;
    
    assert!(avg_latency < Duration::from_micros(50), "Prediction latency {}μs > 50μs", avg_latency.as_micros());
}
```

#### Test: Memory Efficiency
```rust
#[tokio::test]
async fn test_memory_efficiency() {
    let factory = ModelAdapterFactory::new(false);
    let initial_memory = get_memory_usage();
    
    // Create 100 models
    let mut adapters = Vec::new();
    for i in 0..100 {
        let config = UnifiedModelConfig::new(ModelType::MLP, 24, 1);
        let adapter = factory.create_adapter(ModelType::MLP, config).await.unwrap();
        adapters.push(adapter);
    }
    
    let peak_memory = get_memory_usage();
    let memory_per_model = (peak_memory - initial_memory) / 100;
    
    // Target: <1KB per model
    assert!(memory_per_model < 1024, "Memory per model {}B > 1KB", memory_per_model);
    
    // Test for memory leaks
    drop(adapters);
    force_gc();
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    let final_memory = get_memory_usage();
    let memory_leaked = final_memory.saturating_sub(initial_memory);
    
    assert!(memory_leaked < 1024 * 1024, "Memory leak detected: {}B", memory_leaked);
}
```

### 3. Integration Tests

#### Test: Ensemble Management Integration
```rust
#[tokio::test]
async fn test_ensemble_management_integration() {
    let factory = ModelAdapterFactory::new(false);
    let ensemble_manager = EnsembleManager::new();
    
    // Create ensemble of 5 models
    let model_types = vec![MLP, LSTM, NHITS, TCN, DeepAR];
    
    for (i, model_type) in model_types.iter().enumerate() {
        let config = UnifiedModelConfig::new(model_type.clone(), 24, 1);
        let adapter = factory.create_adapter(model_type.clone(), config).await.unwrap();
        
        let registered = ensemble_manager.register_model(&format!("model_{}", i), adapter).await;
        assert!(registered.is_ok());
    }
    
    // Test ensemble prediction
    let test_input = vec![0.5; 24];
    let ensemble_result = ensemble_manager.predict_ensemble(&test_input).await;
    
    assert!(ensemble_result.is_ok());
    let prediction = ensemble_result.unwrap();
    
    assert_eq!(prediction.contributing_models.len(), 5);
    assert!(prediction.confidence > 0.0 && prediction.confidence <= 1.0);
}
```

#### Test: Health Monitoring Integration
```rust
#[tokio::test]
async fn test_health_monitoring_integration() {
    let factory = ModelAdapterFactory::new(false);
    let health_monitor = HealthMonitor::new();
    
    // Create models and register with health monitor
    for model_type in [MLP, LSTM, NHITS] {
        let config = UnifiedModelConfig::new(model_type.clone(), 24, 1);
        let adapter = factory.create_adapter(model_type.clone(), config).await.unwrap();
        
        health_monitor.register_component(
            &format!("neural_model_{}", model_type),
            ComponentType::NeuralModel(adapter),
        ).await;
    }
    
    // Perform health check
    let health_status = health_monitor.check_all_components().await;
    
    assert_eq!(health_status.neural_models.len(), 3);
    for model_health in &health_status.neural_models {
        assert!(model_health.is_healthy || model_health.is_degraded);
        assert!(model_health.latency < Duration::from_millis(100));
    }
}
```

### 4. Honesty & Transparency Tests

#### Test: Approximation Warnings
```rust
#[tokio::test]
async fn test_approximation_warnings() {
    let factory = ModelAdapterFactory::new(false);
    let config = UnifiedModelConfig::default();
    
    // Capture logs during creation
    let test_logger = TestLogger::new();
    
    // Create LSTM approximation
    let _ = factory.create_adapter(ModelType::LSTM, config.clone()).await.unwrap();
    
    let logs = test_logger.get_logs();
    assert!(logs.iter().any(|log| log.contains("FANN approximation")));
    assert!(logs.iter().any(|log| log.contains("NOT true LSTM")));
    
    // Create TCN approximation
    let _ = factory.create_adapter(ModelType::TCN, config).await.unwrap();
    
    let logs = test_logger.get_logs();
    assert!(logs.iter().any(|log| log.contains("NOT true temporal convolutions")));
}
```

#### Test: Model Info Transparency
```rust
#[tokio::test]
async fn test_model_info_transparency() {
    let factory = ModelAdapterFactory::new(false);
    
    for model_type in [MLP, LSTM, TCN, NHITS, DeepAR] {
        let config = UnifiedModelConfig::new(model_type.clone(), 24, 1);
        let adapter = factory.create_adapter(model_type.clone(), config).await.unwrap();
        
        let info = adapter.get_info();
        
        // All models should have complete info
        assert!(!info.architecture_description.is_empty());
        assert!(!info.performance_characteristics.is_empty());
        
        // Approximations should have limitations listed
        if matches!(model_type, LSTM | TCN | NHITS | DeepAR) {
            assert!(!info.limitations.is_empty());
            assert!(info.is_approximation);
        }
    }
}
```

### 5. Concurrent Operation Tests

#### Test: Concurrent Model Creation
```rust
#[tokio::test]
async fn test_concurrent_model_creation() {
    let factory = Arc::new(ModelAdapterFactory::new(false));
    let mut tasks = Vec::new();
    
    // Spawn 50 concurrent creation tasks
    for i in 0..50 {
        let factory_clone = factory.clone();
        let task = tokio::spawn(async move {
            let model_type = match i % 5 {
                0 => ModelType::MLP,
                1 => ModelType::LSTM,
                2 => ModelType::NHITS,
                3 => ModelType::TCN,
                _ => ModelType::DeepAR,
            };
            
            let config = UnifiedModelConfig::new(model_type.clone(), 24, 1);
            factory_clone.create_adapter(model_type, config).await
        });
        
        tasks.push(task);
    }
    
    // All should succeed
    let results = futures::future::join_all(tasks).await;
    let mut success_count = 0;
    
    for result in results {
        if let Ok(Ok(_)) = result {
            success_count += 1;
        }
    }
    
    assert_eq!(success_count, 50, "Only {}/50 concurrent creations succeeded", success_count);
}
```

### 6. End-to-End Workflow Tests

#### Test: Complete Prediction Workflow
```rust
#[tokio::test]
async fn test_complete_prediction_workflow() {
    // Initialize system
    let factory = ModelAdapterFactory::new(false);
    let predictor = NeuralPredictor::with_factory(factory);
    
    // Configure models
    let models = vec![
        ("mlp_model", ModelType::MLP),
        ("lstm_approx", ModelType::LSTM),
        ("tcn_approx", ModelType::TCN),
    ];
    
    for (name, model_type) in models {
        predictor.add_model(name, model_type).await.unwrap();
    }
    
    // Train models
    let training_data = generate_market_data(1000);
    predictor.train_all_models(&training_data).await.unwrap();
    
    // Make predictions
    let test_data = generate_test_market_data();
    let predictions = predictor.predict_ensemble(&test_data).await.unwrap();
    
    // Validate results
    assert_eq!(predictions.model_count, 3);
    assert!(predictions.confidence > 0.0);
    assert!(predictions.value.is_finite());
    
    // Verify integration with trading system
    let trading_decision = make_trading_decision(&predictions);
    assert!(trading_decision.uses_neural_predictions);
}
```

## Performance Benchmarks

### Benchmark Suite
```rust
#[bench]
fn bench_model_creation(b: &mut Bencher) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let factory = ModelAdapterFactory::new(false);
    let config = UnifiedModelConfig::default();
    
    b.iter(|| {
        rt.block_on(async {
            factory.create_adapter(ModelType::MLP, config.clone()).await.unwrap()
        })
    });
}

#[bench]
fn bench_prediction_throughput(b: &mut Bencher) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let factory = ModelAdapterFactory::new(false);
    let config = UnifiedModelConfig::new(ModelType::MLP, 24, 1);
    let adapter = rt.block_on(factory.create_adapter(ModelType::MLP, config)).unwrap();
    
    let test_input = vec![0.5; 24];
    
    b.iter(|| {
        rt.block_on(adapter.predict(&test_input))
    });
}
```

## Test Execution Plan

### Phase 1: Unit Tests (2 hours)
- Factory creation tests
- Configuration tests
- Adapter interface tests

### Phase 2: Integration Tests (3 hours)
- Ensemble management
- Health monitoring
- Production workflow

### Phase 3: Performance Tests (2 hours)
- Latency benchmarks
- Throughput testing
- Memory profiling

### Phase 4: Stress Tests (1 hour)
- Concurrent operations
- Resource limits
- Failure scenarios

## Success Criteria

### All Tests Must:
1. **Pass Consistently**: 100% pass rate over 10 runs
2. **Meet Performance Targets**: Latency <100ms, throughput >100/sec
3. **Show No Memory Leaks**: Stable memory over 1000 iterations
4. **Demonstrate Honesty**: Clear warnings for approximations
5. **Integrate Properly**: Work with existing systems

### Coverage Requirements
- **Unit Test Coverage**: >95%
- **Integration Coverage**: >90%
- **Performance Coverage**: All critical paths
- **Error Path Coverage**: >80%

## Continuous Integration

### GitHub Actions Workflow
```yaml
name: Factory Integration Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    
    steps:
    - uses: actions/checkout@v3
    
    - name: Run Factory Tests
      run: |
        cargo test --test factory_integration_tests
        cargo test --test factory_performance_tests
        
    - name: Run Benchmarks
      run: cargo bench --bench factory_benchmarks
      
    - name: Check Test Coverage
      run: cargo tarpaulin --out Xml
      
    - name: Upload Coverage
      uses: codecov/codecov-action@v3
```

---

*Test Specification Version*: 1.0  
*Created*: 2025-08-01  
*Total Test Scenarios*: 25+  
*Estimated Execution Time*: 8 hours