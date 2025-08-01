# Phase 1 Test Strategy: Vendor Model Foundation

## Testing Philosophy

Phase 1 testing follows Test-Driven Development (TDD) principles with comprehensive coverage of vendor model integration, DAA preservation, and sector-based architecture. All critical paths must be validated before production deployment.

## 1. Test Pyramid Architecture

```
                 ┌─────────────────────┐
                 │   E2E Tests (5%)    │ ← Full system validation
                 └─────────────────────┘
              ┌─────────────────────────────┐
              │  Integration Tests (25%)    │ ← Component interaction
              └─────────────────────────────┘
           ┌─────────────────────────────────────┐
           │      Unit Tests (70%)              │ ← Individual components
           └─────────────────────────────────────┘
```

## 2. Unit Testing Strategy (70% of test effort)

### 2.1 VendorPredictor Unit Tests

```rust
#[cfg(test)]
mod vendor_predictor_tests {
    use super::*;
    use vendor::ruv_fann::neuro_divergent_models::test_utils::*;

    #[tokio::test]
    async fn test_predict_with_lstm_model() {
        // Arrange
        let mut predictor = VendorPredictor::new_for_test();
        let lstm_config = ModelConfig::lstm_default();
        predictor.add_model("LSTM_test", lstm_config).await.unwrap();
        
        let market_data = create_test_market_data(24); // 24 price points
        
        // Act
        let result = predictor.predict("AAPL", &market_data).await;
        
        // Assert
        assert!(result.is_ok());
        let prediction = result.unwrap();
        assert!(prediction.value > 0.0);
        assert!(prediction.confidence >= 0.0 && prediction.confidence <= 1.0);
        assert_eq!(prediction.model_id, "LSTM_test");
    }

    #[tokio::test]
    async fn test_sector_based_model_selection() {
        // Arrange
        let mut predictor = VendorPredictor::new_for_test();
        predictor.configure_sector_model("technology", "TFT_tech").await.unwrap();
        predictor.configure_sector_model("financial", "LSTM_fin").await.unwrap();
        
        // Act & Assert - Technology sector
        let tech_models = predictor.get_models_for_symbol("AAPL").await.unwrap();
        assert!(tech_models.contains(&"TFT_tech".to_string()));
        
        // Act & Assert - Financial sector  
        let fin_models = predictor.get_models_for_symbol("JPM").await.unwrap();
        assert!(fin_models.contains(&"LSTM_fin".to_string()));
    }

    #[tokio::test]
    async fn test_lazy_model_activation() {
        // Arrange
        let mut predictor = VendorPredictor::new_for_test();
        let config = ModelConfig {
            architecture: "DeepAR".to_string(),
            data_requirements: DataRequirements {
                required: vec![DataType::Price, DataType::Sentiment],
                optional: vec![],
                min_history: 100,
            },
            ..Default::default()
        };
        predictor.configure_lazy_model("DeepAR_sentiment", config).await.unwrap();
        
        // Act - Model should not be active initially (no sentiment data)
        let active_models = predictor.get_active_models().await.unwrap();
        assert!(!active_models.contains(&"DeepAR_sentiment".to_string()));
        
        // Simulate sentiment data arrival
        predictor.notify_data_availability("AAPL", DataType::Sentiment).await.unwrap();
        
        // Assert - Model should now be active
        let active_models = predictor.get_active_models().await.unwrap();
        assert!(active_models.contains(&"DeepAR_sentiment".to_string()));
    }
}
```

### 2.2 ModelFactory Unit Tests

```rust
#[cfg(test)]
mod model_factory_tests {
    use super::*;

    #[test]
    fn test_create_lstm_model() {
        // Arrange
        let config = ModelConfig {
            architecture: "LSTM".to_string(),
            input_size: 24,
            hidden_size: 64,
            num_layers: Some(2),
            dropout: Some(0.1),
            ..Default::default()
        };
        
        // Act
        let model = ModelFactory::create_model("LSTM", config);
        
        // Assert
        assert!(model.is_ok());
        let model = model.unwrap();
        // Test model can process TimeSeriesData
        let test_data = TimeSeriesData::new(vec![1.0, 2.0, 3.0]);
        assert!(model.predict(&test_data).is_ok());
    }

    #[test]
    fn test_model_capability_detection() {
        // Act
        let lstm_caps = ModelFactory::get_model_capabilities("LSTM");
        let tft_caps = ModelFactory::get_model_capabilities("TFT");
        
        // Assert
        assert!(lstm_caps.requires_sequential_data);
        assert!(lstm_caps.supports_exogenous);
        assert!(!lstm_caps.supports_static);
        
        assert!(tft_caps.requires_sequential_data);
        assert!(tft_caps.supports_exogenous);
        assert!(tft_caps.supports_static);
    }

    #[test]
    fn test_unsupported_model_error() {
        // Arrange
        let config = ModelConfig::default();
        
        // Act
        let result = ModelFactory::create_model("UNSUPPORTED_MODEL", config);
        
        // Assert
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unsupported model"));
    }
}
```

### 2.3 SectorMapper Unit Tests

```rust
#[cfg(test)]
mod sector_mapper_tests {
    use super::*;

    #[test]
    fn test_symbol_to_sector_mapping() {
        // Arrange
        let mapper = SectorMapper::from_config("test_config/sectors.toml").unwrap();
        
        // Act & Assert
        let aapl_sector = mapper.get_sector("AAPL").unwrap();
        assert_eq!(aapl_sector.sector_id, SectorId::Technology);
        assert_eq!(aapl_sector.sub_sector, Some("Consumer Electronics".to_string()));
        
        let jpm_sector = mapper.get_sector("JPM").unwrap();
        assert_eq!(jpm_sector.sector_id, SectorId::FinancialServices);
        assert_eq!(jpm_sector.sub_sector, Some("Banking".to_string()));
    }

    #[test]
    fn test_sector_symbols_retrieval() {
        // Arrange
        let mapper = SectorMapper::from_config("test_config/sectors.toml").unwrap();
        
        // Act
        let tech_symbols = mapper.get_symbols_in_sector(&SectorId::Technology);
        
        // Assert
        assert!(tech_symbols.contains(&"AAPL".to_string()));
        assert!(tech_symbols.contains(&"MSFT".to_string()));
        assert!(tech_symbols.len() >= 2);
    }

    #[tokio::test]
    async fn test_sector_aggregation() {
        // Arrange
        let mapper = SectorMapper::from_config("test_config/sectors.toml").unwrap();
        let mut market_data = HashMap::new();
        market_data.insert("AAPL".to_string(), create_test_market_data_with_change(5.0));
        market_data.insert("MSFT".to_string(), create_test_market_data_with_change(3.0));
        
        // Act
        let sector_features = mapper.get_sector_features(&SectorId::Technology, &market_data).await.unwrap();
        
        // Assert
        assert!(sector_features.weighted_price_change > 0.0);
        assert!(sector_features.weighted_volume > 0.0);
        assert!(sector_features.breadth_ratio > 0.0);
    }
}
```

### 2.4 DataConverter Unit Tests

```rust
#[cfg(test)]
mod data_converter_tests {
    use super::*;

    #[test]
    fn test_market_data_to_vendor_format() {
        // Arrange
        let converter = DataConverter::new(DataConverterConfig::default());
        let market_data = MarketData {
            prices: vec![100.0, 101.0, 102.0, 103.0],
            volume: vec![1000.0, 1100.0, 900.0, 1200.0],
            volatility: vec![0.1, 0.12, 0.11, 0.13],
            market_cap: 2e12,
            sector_id: 1,
            volatility_regime: 0,
        };
        
        // Act
        let ts_data = converter.to_vendor_format(&market_data, None).unwrap();
        
        // Assert
        assert_eq!(ts_data.values().len(), 4);
        assert_eq!(ts_data.values()[0], 100.0);
        assert_eq!(ts_data.exogenous().unwrap().len(), 2); // volume + volatility
        assert_eq!(ts_data.static_features().unwrap().len(), 3); // market_cap + sector_id + volatility_regime
    }

    #[test]
    fn test_vendor_format_to_prediction_result() {
        // Arrange
        let converter = DataConverter::new(DataConverterConfig::default());
        let mut metadata = HashMap::new();
        metadata.insert("confidence".to_string(), "0.85".to_string());
        
        let forecast = ForecastResult {
            forecasts: vec![105.5],
            metadata,
        };
        
        // Act
        let result = converter.from_vendor_format(forecast, "LSTM_test").unwrap();
        
        // Assert
        assert_eq!(result.value, 105.5);
        assert_eq!(result.confidence, 0.85);
        assert_eq!(result.model_id, "LSTM_test");
    }
}
```

## 3. Integration Testing Strategy (25% of test effort)

### 3.1 DAA Integration Tests

```rust
#[cfg(test)]
mod daa_integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_daa_receives_performance_data() {
        // Arrange
        let mut system = create_test_system_with_daa().await;
        let performance_tracker = system.get_performance_tracker();
        let daa_engine = system.get_daa_engine();
        
        // Act - Record a prediction with poor performance
        performance_tracker.record_prediction(
            "AAPL",
            "LSTM_test", 
            &PredictionResult { value: 100.0, confidence: 0.8, ..Default::default() },
            Some(110.0), // Actual was much higher
            &MarketContext::default()
        ).await.unwrap();
        
        // Assert - DAA should receive performance update
        let received_updates = daa_engine.get_received_performance_updates().await;
        assert_eq!(received_updates.len(), 1);
        assert_eq!(received_updates[0].symbol, "AAPL");
        assert_eq!(received_updates[0].model_id, "LSTM_test");
    }

    #[tokio::test]
    async fn test_daa_autonomous_training_decision() {
        // Arrange
        let mut system = create_test_system_with_daa().await;
        let daa_engine = system.get_daa_engine();
        
        // Create poor performance data that should trigger training
        let poor_performance = DAAPerformanceInput {
            prediction_accuracy: 0.4, // Below threshold
            consecutive_failures: 6,   // Above threshold
            sharpe_ratio: 0.2,        // Poor performance
            ..Default::default()
        };
        
        // Act
        let decision = daa_engine.make_autonomous_training_decision(
            "LSTM_test", 
            "AAPL", 
            poor_performance
        ).await.unwrap();
        
        // Assert
        assert!(matches!(decision.action, TrainingAction::ExecuteTraining { .. }));
        assert!(decision.urgency > 0.8); // Should be high urgency
        assert!(decision.reasoning.contains("accuracy"));
    }

    #[tokio::test]
    async fn test_daa_voting_weights_preserved() {
        // Arrange
        let mut system = create_test_system_with_daa().await;
        let daa_coordinator = system.get_daa_coordinator();
        
        // Act
        let decision = daa_coordinator.make_autonomous_decision(&MarketContext::default()).await.unwrap();
        
        // Assert - Verify 60/40 neural/strategy split
        assert!((decision.neural_weight - 0.6).abs() < 0.01);
        assert!((decision.strategy_weight - 0.4).abs() < 0.01);
        assert!(decision.neural_votes.len() > 0);
        assert!(decision.strategy_votes.len() > 0);
    }
}
```

### 3.2 Enhanced Neural Adapter Integration Tests

```rust
#[cfg(test)]
mod neural_adapter_integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_neural_request_response_flow() {
        // Arrange
        let system = create_test_system().await;
        let adapter = system.get_neural_adapter();
        
        let request = NeuralRequest {
            symbol: "AAPL".to_string(),
            market_data: create_test_market_data(24),
            timestamp: Utc::now(),
        };
        
        // Act
        let response = adapter.get_neural_signals(request).await.unwrap();
        
        // Assert
        assert!(response.predictions.len() > 0);
        assert!(response.confidence >= 0.0 && response.confidence <= 1.0);
        assert!(response.model_agreement >= 0.0);
        assert!(response.metadata.contains_key("models_used"));
    }

    #[tokio::test]
    async fn test_ensemble_prediction_aggregation() {
        // Arrange
        let system = create_test_system_with_multiple_models().await;
        let adapter = system.get_neural_adapter();
        
        // Ensure multiple models are active
        system.activate_models(vec!["LSTM_test", "GRU_test", "TCN_test"]).await;
        
        let request = NeuralRequest {
            symbol: "AAPL".to_string(),
            market_data: create_test_market_data(24),
            timestamp: Utc::now(),
        };
        
        // Act
        let response = adapter.get_neural_signals(request).await.unwrap();
        
        // Assert
        assert!(response.predictions.len() == 1); // Ensemble prediction  
        assert!(response.metadata.get("models_used").unwrap().parse::<i32>().unwrap() >= 3);
        assert!(response.model_agreement > 0.0); // Should have some agreement metric
    }
}
```

### 3.3 Configuration Integration Tests

```rust
#[cfg(test)]
mod configuration_integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_model_activation_from_config() {
        // Arrange
        let config_content = r#"
        [models.test_lstm]
        architecture = "LSTM"
        input_size = 24
        hidden_size = 32
        
        [models.test_lstm.data_requirements]
        required = ["price"]
        optional = ["volume"]
        "#;
        
        write_test_config("models.toml", config_content);
        let system = create_system_from_config("test_config/").await;
        
        // Act
        system.load_configuration().await.unwrap();
        
        // Assert
        let active_models = system.get_active_models().await.unwrap();
        assert!(active_models.contains(&"test_lstm".to_string()));
    }

    #[tokio::test] 
    async fn test_hot_configuration_reload() {
        // Arrange
        let system = create_test_system().await;
        let initial_models = system.get_active_models().await.unwrap();
        
        // Modify configuration to add a new model
        let new_config = create_config_with_additional_model();
        update_test_config("models.toml", &new_config);
        
        // Act
        system.reload_configuration().await.unwrap();
        
        // Assert
        let new_models = system.get_active_models().await.unwrap();
        assert!(new_models.len() > initial_models.len());
    }
}
```

## 4. End-to-End Testing Strategy (5% of test effort)

### 4.1 Full System Workflow Tests

```rust
#[cfg(test)]
mod e2e_tests {
    use super::*;

    #[tokio::test]
    async fn test_complete_prediction_workflow() {
        // Arrange - Full system setup
        let system = create_production_like_system().await;
        system.start_all_services().await.unwrap();
        
        // Market data simulation
        let market_data_stream = create_market_data_stream("AAPL");
        
        // Act - Send market data through complete pipeline
        for data_point in market_data_stream {
            system.ingest_market_data(data_point).await.unwrap();
        }
        
        tokio::time::sleep(Duration::from_millis(200)).await; // Allow processing
        
        // Assert - Verify complete workflow
        let neural_signals = system.get_latest_neural_signals("AAPL").await.unwrap();
        let daa_decisions = system.get_latest_daa_decisions().await.unwrap();
        let performance_metrics = system.get_performance_metrics("AAPL").await.unwrap();
        
        assert!(neural_signals.is_some());
        assert!(daa_decisions.len() > 0);
        assert!(performance_metrics.prediction_count > 0);
    }

    #[tokio::test]
    async fn test_multi_symbol_concurrent_processing() {
        // Arrange
        let system = create_production_like_system().await;
        let symbols = vec!["AAPL", "MSFT", "GOOGL", "TSLA", "NVDA"];
        
        // Act - Concurrent processing
        let tasks = symbols.iter().map(|symbol| {
            let system_clone = system.clone();
            let symbol_clone = symbol.to_string();
            tokio::spawn(async move {
                let market_data = create_test_market_data(24);
                system_clone.process_symbol_data(&symbol_clone, &market_data).await
            })
        });
        
        let results = futures::future::join_all(tasks).await;
        
        // Assert
        assert!(results.iter().all(|r| r.is_ok()));
        
        // Verify all symbols processed
        for symbol in symbols {
            let signals = system.get_latest_neural_signals(symbol).await.unwrap();
            assert!(signals.is_some());
        }
    }

    #[tokio::test]
    async fn test_system_resilience_during_failures() {
        // Arrange
        let system = create_production_like_system().await;
        
        // Act - Simulate various failure conditions
        
        // 1. Model failure
        system.simulate_model_failure("LSTM_test").await;
        let result1 = system.get_neural_signals("AAPL").await;
        assert!(result1.is_ok()); // Should fallback to other models
        
        // 2. Data corruption
        let corrupted_data = create_corrupted_market_data();
        let result2 = system.process_market_data("AAPL", &corrupted_data).await;
        assert!(result2.is_err()); // Should handle gracefully
        
        // 3. Memory pressure
        system.simulate_memory_pressure().await;
        let result3 = system.get_neural_signals("AAPL").await;
        assert!(result3.is_ok()); // Should continue operating
        
        // 4. Configuration error
        system.load_invalid_configuration().await;
        let result4 = system.get_neural_signals("AAPL").await;
        assert!(result4.is_ok()); // Should use previous valid config
    }
}
```

## 5. Performance Testing Strategy

### 5.1 Load Testing

```rust
#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn test_prediction_latency_under_load() {
        // Arrange
        let system = create_production_like_system().await;
        let symbols = (0..15).map(|i| format!("TEST{:02}", i)).collect::<Vec<_>>();
        
        // Act - Measure latency under load
        let start = Instant::now();
        let mut latencies = Vec::new();
        
        for _ in 0..100 {
            let symbol = &symbols[rand::random::<usize>() % symbols.len()];
            let pred_start = Instant::now();
            let _ = system.get_neural_signals(symbol).await.unwrap();
            latencies.push(pred_start.elapsed());
        }
        
        // Assert
        let avg_latency = latencies.iter().sum::<Duration>() / latencies.len() as u32;
        let p99_latency = latencies.iter().max().unwrap();
        
        assert!(avg_latency < Duration::from_millis(100), "Average latency: {:?}", avg_latency);
        assert!(p99_latency < &Duration::from_millis(150), "P99 latency: {:?}", p99_latency);
    }

    #[tokio::test]
    async fn test_memory_usage_scaling() {
        // Arrange
        let system = create_production_like_system().await;
        let initial_memory = get_memory_usage();
        
        // Act - Add symbols progressively
        for i in 1..=20 {
            let symbol = format!("SCALE{:02}", i);
            system.add_symbol_support(&symbol).await.unwrap();
            
            let memory_usage = get_memory_usage();
            let memory_per_symbol = (memory_usage - initial_memory) as f64 / i as f64;
            
            // Assert - Memory usage should scale reasonably
            assert!(memory_per_symbol < 200.0 * 1024.0 * 1024.0, // <200MB per symbol
                "Memory per symbol: {:.2}MB", memory_per_symbol / 1024.0 / 1024.0);
        }
    }

    #[tokio::test]
    async fn test_throughput_capacity() {
        // Arrange
        let system = create_production_like_system().await;
        let symbols = vec!["AAPL", "MSFT", "GOOGL"];
        
        // Act - High frequency predictions
        let start = Instant::now();
        let mut prediction_count = 0;
        
        while start.elapsed() < Duration::from_secs(60) {
            for symbol in &symbols {
                let _ = system.get_neural_signals(symbol).await.unwrap();
                prediction_count += 1;
            }
        }
        
        let predictions_per_minute = prediction_count;
        
        // Assert - Should handle >600 predictions per minute
        assert!(predictions_per_minute >= 600, 
            "Throughput: {} predictions/minute", predictions_per_minute);
    }
}
```

## 6. Test Data Management

### 6.1 Test Data Generation

```rust
pub mod test_utils {
    use super::*;

    pub fn create_test_market_data(points: usize) -> MarketData {
        let base_price = 100.0;
        let prices = (0..points)
            .map(|i| base_price + (i as f64 * 0.1) + (rand::random::<f64>() - 0.5))
            .collect();
        
        let volume = (0..points)
            .map(|_| 1000.0 + rand::random::<f64>() * 500.0)
            .collect();
            
        let volatility = (0..points)
            .map(|_| 0.1 + rand::random::<f64>() * 0.05)
            .collect();
        
        MarketData {
            prices,
            volume,
            volatility,
            market_cap: 2e12,
            sector_id: 1,
            volatility_regime: 0,
        }
    }

    pub fn create_sector_test_data() -> HashMap<String, SectorInfo> {
        let mut sectors = HashMap::new();
        
        sectors.insert("AAPL".to_string(), SectorInfo {
            sector_id: SectorId::Technology,
            sub_sector: Some("Consumer Electronics".to_string()),
            market_cap_tier: MarketCapTier::LargeCap,
            weight_in_sector: 0.22,
            correlation_group: Some("FAANG".to_string()),
        });
        
        sectors.insert("MSFT".to_string(), SectorInfo {
            sector_id: SectorId::Technology,
            sub_sector: Some("Software".to_string()),
            market_cap_tier: MarketCapTier::LargeCap,
            weight_in_sector: 0.21,
            correlation_group: None,
        });
        
        sectors
    }
}
```

## 7. Test Execution Strategy

### 7.1 Continuous Integration Pipeline

```yaml
# .github/workflows/phase1-tests.yml
name: Phase 1 Test Suite

on: [push, pull_request]

jobs:
  unit-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Run unit tests
        run: cargo test --lib -- --test-threads=1
      - name: Generate coverage report
        run: cargo tarpaulin --out xml
        
  integration-tests:
    runs-on: ubuntu-latest
    needs: unit-tests
    steps:
      - uses: actions/checkout@v3
      - name: Setup test environment
        run: docker-compose -f test/docker-compose.yml up -d
      - name: Run integration tests
        run: cargo test --test integration -- --test-threads=1
        
  performance-tests:
    runs-on: ubuntu-latest
    needs: integration-tests
    steps:
      - uses: actions/checkout@v3
      - name: Run performance benchmarks
        run: cargo bench
      - name: Validate performance targets
        run: ./scripts/validate-performance.sh
```

### 7.2 Test Categories and Execution

**Development Testing (Fast Feedback)**
```bash
# Unit tests (< 30 seconds)
cargo test --lib

# Integration tests (< 2 minutes)  
cargo test --test integration

# Smoke tests (< 30 seconds)
cargo test --test smoke
```

**Pre-commit Testing (Comprehensive)**
```bash
# Full test suite (< 10 minutes)
./scripts/run-all-tests.sh

# Performance validation (< 5 minutes)  
cargo bench --bench prediction_performance

# Memory leak detection (< 3 minutes)
valgrind --tool=memcheck ./target/release/neural-trader --test-mode
```

**Pre-deployment Testing (Production-like)**
```bash
# End-to-end tests (< 30 minutes)
./scripts/e2e-test-suite.sh

# Load testing (< 60 minutes)
./scripts/load-test.sh --duration=60m --symbols=15

# Chaos testing (< 45 minutes)  
./scripts/chaos-test.sh --scenarios=all
```

## 8. Test Success Criteria

### 8.1 Coverage Targets
- **Unit Tests**: 90%+ code coverage
- **Integration Tests**: 100% critical path coverage
- **E2E Tests**: 100% user journey coverage
- **Performance Tests**: All SLA targets validated

### 8.2 Quality Gates
- All tests must pass before merge to main
- Performance regression tests must validate <5% degradation
- Memory leak tests must show zero leaks over 24 hours
- Security tests must show zero critical vulnerabilities

### 8.3 Acceptance Criteria
- **Functional**: All vendor models operational, DAA preserved
- **Performance**: Latency <100ms, memory <2GB, throughput >600/min  
- **Reliability**: 99.5% uptime, graceful failure handling
- **Integration**: Zero breaking changes to dependent systems

This comprehensive test strategy ensures Phase 1 vendor model foundation is thoroughly validated and ready for production deployment.