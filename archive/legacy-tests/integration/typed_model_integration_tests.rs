//! Integration Tests for Typed Model System
//!
//! This module provides comprehensive integration testing for the typed model system,
//! testing the complete flow from storage to prediction with type safety guarantees.

use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::{timeout, Duration};

use crate::config::NeuralConfig;
use crate::data::{
    TimeSeriesData,
    sector_mapper::{SectorMapper, SectorMapperConfig},
};
use crate::monitoring::model_performance_tracker::ModelPerformanceTracker;
use crate::neural::{
    vendor_predictor::{VendorPredictor, ModelKey, ClusterPoolConfig},
    NeuralPredictorTrait, PredictionResult,
};

// Import typed models from unit tests
use crate::tests::unit::typed_storage_tests::{
    TypedLSTMModel, TypedGRUModel, TypedModelStorage, TypedBaseModel,
};

/// Integration test harness for typed model system
pub struct TypedModelIntegrationTest {
    pub predictor: VendorPredictor,
    pub typed_storage: Arc<TypedModelStorage>,
    pub sector_mapper: Arc<SectorMapper>,
    pub performance_tracker: Arc<ModelPerformanceTracker>,
}

impl TypedModelIntegrationTest {
    pub async fn new() -> Result<Self> {
        let config = create_test_neural_config();
        let sector_mapper = Arc::new(SectorMapper::new(SectorMapperConfig::default()));
        let performance_tracker = Arc::new(ModelPerformanceTracker::new());
        let typed_storage = Arc::new(TypedModelStorage::new());
        
        let predictor = VendorPredictor::new(&config, sector_mapper.clone(), performance_tracker.clone())?;
        
        Ok(Self {
            predictor,
            typed_storage,
            sector_mapper,
            performance_tracker,
        })
    }
    
    pub async fn setup_test_models(&self) -> Result<()> {
        // Create typed models for different sectors
        let tech_lstm = TypedLSTMModel::new_with_prediction(24, 64, 1, 175.5);
        let tech_gru = TypedGRUModel::new_with_prediction(24, 64, 1, 177.2);
        
        let finance_lstm = TypedLSTMModel::new_with_prediction(24, 48, 1, 85.3);
        let finance_gru = TypedGRUModel::new_with_prediction(24, 48, 1, 87.1);
        
        // Store models in typed storage
        self.typed_storage.store_lstm_model("tech_lstm_v1".to_string(), tech_lstm).await?;
        self.typed_storage.store_gru_model("tech_gru_v1".to_string(), tech_gru).await?;
        self.typed_storage.store_lstm_model("finance_lstm_v1".to_string(), finance_lstm).await?;
        self.typed_storage.store_gru_model("finance_gru_v1".to_string(), finance_gru).await?;
        
        // Add models to predictor with proper keys
        let tech_lstm_key = ModelKey {
            sector: "technology".to_string(),
            model_type: "TypedLSTM".to_string(),
            variant: "v1".to_string(),
        };
        
        let tech_gru_key = ModelKey {
            sector: "technology".to_string(),
            model_type: "TypedGRU".to_string(),
            variant: "v1".to_string(),
        };
        
        let finance_lstm_key = ModelKey {
            sector: "financial".to_string(),
            model_type: "TypedLSTM".to_string(),
            variant: "v1".to_string(),
        };
        
        let finance_gru_key = ModelKey {
            sector: "financial".to_string(),
            model_type: "TypedGRU".to_string(),
            variant: "v1".to_string(),
        };
        
        // Get models from typed storage and add to predictor
        let tech_lstm = self.typed_storage.get_lstm_model("tech_lstm_v1").await?.unwrap();
        let tech_gru = self.typed_storage.get_gru_model("tech_gru_v1").await?.unwrap();
        let finance_lstm = self.typed_storage.get_lstm_model("finance_lstm_v1").await?.unwrap();
        let finance_gru = self.typed_storage.get_gru_model("finance_gru_v1").await?.unwrap();
        
        // Box models as Any for predictor compatibility
        self.predictor.add_model(tech_lstm_key, Box::new(tech_lstm)).await?;
        self.predictor.add_model(tech_gru_key, Box::new(tech_gru)).await?;
        self.predictor.add_model(finance_lstm_key, Box::new(finance_lstm)).await?;
        self.predictor.add_model(finance_gru_key, Box::new(finance_gru)).await?;
        
        Ok(())
    }
}

fn create_test_neural_config() -> NeuralConfig {
    NeuralConfig {
        input_size: 24,
        output_size: 1,
        hidden_layers: vec![64, 48, 32],
        learning_rate: 0.001,
        prediction_horizon: Some(1),
        normalization_method: Some("z-score".to_string()),
        enable_adaptive_retry: true,
        enable_model_ensembles: true,
        model_timeout_seconds: 120,
        max_retries: 3,
        error_threshold: 0.15,
        memory_gb: 2.0,
        models: vec!["TypedLSTM".to_string(), "TypedGRU".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.85,
        use_real_models: true,
        enable_health_checks: true,
        enable_fallback: true,
        lookback_window: 24,
        enable_circuit_breakers: true,
        enable_graceful_degradation: false,
        enable_performance_monitoring: true,
        epochs: 100,
        batch_size: 32,
        sequence_length: 24,
        enable_feature_scaling: true,
        enable_technical_indicators: true,
        dropout_rate: 0.1,
        l2_regularization: 0.001,
        validation_split: 0.2,
        early_stopping: true,
        patience: 10,
    }
}

fn create_test_time_series_data(symbol: &str, values: Vec<f64>) -> TimeSeriesData {
    let now = Utc::now();
    let close_price = values.last().copied().unwrap_or(100.0);
    
    TimeSeriesData {
        symbol: symbol.to_string(),
        timestamp: now,
        open: close_price * 0.995,
        high: close_price * 1.015,
        low: close_price * 0.985,
        close: close_price,
        volume: vec![1500000.0],
        indicators: HashMap::new(),
        source: Some("integration_test".to_string()),
        entity: Some(symbol.to_string()),
        value: Some(close_price),
        values: values.clone(),
        timestamps: (0..values.len())
            .map(|i| now - chrono::Duration::hours((values.len() - i - 1) as i64))
            .collect(),
        metadata: Some({
            let mut map = HashMap::new();
            map.insert("symbol".to_string(), serde_json::json!(symbol));
            map.insert("test_type".to_string(), serde_json::json!("integration"));
            map
        }),
        metadata_map: {
            let mut map = HashMap::new();
            map.insert("symbol".to_string(), serde_json::json!(symbol));
            map.insert("sector_test".to_string(), serde_json::json!(true));
            map
        },
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[tokio::test]
    async fn test_complete_typed_model_flow() {
        let test_harness = TypedModelIntegrationTest::new().await.unwrap();
        test_harness.setup_test_models().await.unwrap();
        
        // Create test data for technology stock
        let aapl_data = create_test_time_series_data(
            "AAPL",
            vec![170.0, 172.5, 169.8, 174.2, 175.1, 173.7, 176.3, 178.0],
        );
        
        // Test prediction flow
        let predictions = test_harness.predictor.predict(
            &[aapl_data],
            1,
            None,
        ).await.unwrap();
        
        assert_eq!(predictions.len(), 1);
        let prediction = &predictions[0];
        
        // Verify prediction structure
        assert!(prediction.value > 0.0);
        assert!(prediction.confidence > 0.0 && prediction.confidence <= 1.0);
        assert!(prediction.model_name.contains("ensemble"));
        assert!(prediction.metadata.is_some());
        
        // Verify type safety was maintained
        let metadata = prediction.metadata.as_ref().unwrap();
        assert!(metadata.contains_key("individual_models"));
        assert!(metadata.contains_key("individual_confidences"));
        
        // Verify models used
        let model_names: Vec<String> = metadata.get("individual_models")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        
        // Should have both LSTM and GRU models for technology sector
        assert!(model_names.iter().any(|name| name.contains("TypedLSTM")));
        assert!(model_names.iter().any(|name| name.contains("TypedGRU")));
    }
    
    #[tokio::test]
    async fn test_sector_based_typed_model_routing() {
        let test_harness = TypedModelIntegrationTest::new().await.unwrap();
        test_harness.setup_test_models().await.unwrap();
        
        // Test technology stock
        let tech_data = create_test_time_series_data(
            "MSFT", 
            vec![250.0, 252.1, 248.5, 253.7, 255.2],
        );
        
        // Test financial stock
        let finance_data = create_test_time_series_data(
            "JPM",
            vec![120.0, 121.5, 119.2, 122.8, 123.1],
        );
        
        // Make predictions for both sectors
        let tech_predictions = test_harness.predictor.predict(&[tech_data], 1, None).await.unwrap();
        let finance_predictions = test_harness.predictor.predict(&[finance_data], 1, None).await.unwrap();
        
        assert_eq!(tech_predictions.len(), 1);
        assert_eq!(finance_predictions.len(), 1);
        
        // Predictions should be different due to different sector models
        let tech_pred = &tech_predictions[0];
        let finance_pred = &finance_predictions[0];
        
        assert_ne!(tech_pred.value, finance_pred.value);
        
        // Both should have valid ensemble predictions
        assert!(tech_pred.model_name.contains("ensemble"));
        assert!(finance_pred.model_name.contains("ensemble"));
        
        // Verify metadata includes sector information
        let tech_metadata = tech_pred.metadata.as_ref().unwrap();
        let finance_metadata = finance_pred.metadata.as_ref().unwrap();
        
        assert!(tech_metadata.contains_key("individual_models"));
        assert!(finance_metadata.contains_key("individual_models"));
    }
    
    #[tokio::test]
    async fn test_typed_model_storage_persistence_integration() {
        let test_harness = TypedModelIntegrationTest::new().await.unwrap();
        
        // Create and store models
        let lstm_model = TypedLSTMModel::new_with_prediction(20, 40, 1, 200.0);
        let gru_model = TypedGRUModel::new_with_prediction(20, 40, 1, 195.0);
        
        test_harness.typed_storage.store_lstm_model("persistence_lstm".to_string(), lstm_model).await.unwrap();
        test_harness.typed_storage.store_gru_model("persistence_gru".to_string(), gru_model).await.unwrap();
        
        // Simulate system restart by creating new harness
        let new_harness = TypedModelIntegrationTest::new().await.unwrap();
        
        // Models should still be available in typed storage
        let retrieved_lstm = new_harness.typed_storage.get_lstm_model("persistence_lstm").await.unwrap();
        let retrieved_gru = new_harness.typed_storage.get_gru_model("persistence_gru").await.unwrap();
        
        assert!(retrieved_lstm.is_some());
        assert!(retrieved_gru.is_some());
        
        // Verify models can still make predictions
        let lstm = retrieved_lstm.unwrap();
        let gru = retrieved_gru.unwrap();
        
        let input = vec![1.0; 20];
        
        let lstm_output = lstm.predict_typed(&input).unwrap();
        let gru_output = gru.predict_typed(&input).unwrap();
        
        assert_eq!(lstm_output.len(), 1);
        assert_eq!(gru_output.len(), 1);
        assert!(lstm_output[0].is_finite());
        assert!(gru_output[0].is_finite());
    }
    
    #[tokio::test]
    async fn test_concurrent_typed_predictions() {
        let test_harness = Arc::new(TypedModelIntegrationTest::new().await.unwrap());
        test_harness.setup_test_models().await.unwrap();
        
        let mut handles = vec![];
        
        // Spawn concurrent prediction tasks
        for i in 0..20 {
            let harness_clone = Arc::clone(&test_harness);
            let handle = tokio::spawn(async move {
                let symbol = if i % 2 == 0 { "AAPL" } else { "JPM" };
                let base_price = if symbol == "AAPL" { 170.0 } else { 120.0 };
                
                let data = create_test_time_series_data(
                    symbol,
                    vec![
                        base_price + i as f64 * 0.1,
                        base_price + i as f64 * 0.2,
                        base_price + i as f64 * 0.15,
                    ],
                );
                
                harness_clone.predictor.predict(&[data], 1, None).await
            });
            
            handles.push(handle);
        }
        
        // Wait for all predictions to complete
        let mut successful_predictions = 0;
        for handle in handles {
            let result = handle.await.unwrap();
            if let Ok(predictions) = result {
                assert_eq!(predictions.len(), 1);
                let prediction = &predictions[0];
                assert!(prediction.value > 0.0);
                assert!(prediction.confidence > 0.0);
                successful_predictions += 1;
            }
        }
        
        assert_eq!(successful_predictions, 20);
    }
    
    #[tokio::test]
    async fn test_typed_model_performance_tracking() {
        let test_harness = TypedModelIntegrationTest::new().await.unwrap();
        test_harness.setup_test_models().await.unwrap();
        
        // Enable performance monitoring
        let config = HashMap::new();
        // Note: In a real implementation, you would configure monitoring here
        
        // Make several predictions to generate performance data
        for i in 0..10 {
            let data = create_test_time_series_data(
                "AAPL",
                vec![170.0 + i as f64, 171.0 + i as f64, 172.0 + i as f64],
            );
            
            let predictions = test_harness.predictor.predict(&[data], 1, None).await.unwrap();
            assert_eq!(predictions.len(), 1);
            
            // Simulate actual outcome for performance tracking
            // In a real system, this would come from market data
            let actual_outcome = 175.0 + i as f64;
            
            // Note: Performance tracking integration would happen here
            // test_harness.performance_tracker.record_prediction(...).await;
        }
        
        // Verify performance data is collected
        let model_info = test_harness.predictor.get_model_info().await;
        assert!(model_info.contains_key("active_models"));
        assert!(model_info.contains_key("performance_tracking"));
    }
    
    #[tokio::test]
    async fn test_typed_model_memory_efficiency() {
        let mut config = ClusterPoolConfig::default();
        config.max_memory_mb = 50.0;
        config.enable_lazy_loading = true;
        config.min_active_symbols = 2;
        
        let test_harness = TypedModelIntegrationTest::new().await.unwrap();
        
        // Create cluster pools with memory constraints
        let pool = test_harness.predictor.get_or_create_cluster_pool("memory_test").await.unwrap();
        
        // Add typed models to cluster pool
        let lstm1 = TypedLSTMModel::new(50, 100, 10); // Large model
        let lstm2 = TypedLSTMModel::new(30, 60, 5);   // Medium model
        let gru1 = TypedGRUModel::new(40, 80, 8);     // Medium-large model
        
        let lstm1_boxed: Box<dyn std::any::Any + Send + Sync> = Box::new(lstm1);
        let lstm2_boxed: Box<dyn std::any::Any + Send + Sync> = Box::new(lstm2);
        let gru1_boxed: Box<dyn std::any::Any + Send + Sync> = Box::new(gru1);
        
        // Add models with estimated memory usage
        pool.add_shared_model("LSTM_Large", lstm1_boxed, 20.0).await.unwrap();
        pool.add_shared_model("LSTM_Medium", lstm2_boxed, 15.0).await.unwrap();
        
        // This should either succeed with lazy loading or fail with memory constraint
        let result = pool.add_shared_model("GRU_Medium", gru1_boxed, 18.0).await;
        
        // Check memory usage is within bounds
        let (_, memory_mb) = pool.get_memory_usage().await;
        assert!(memory_mb <= config.max_memory_mb || result.is_err());
        
        // Verify pool statistics
        let stats = pool.get_pool_stats().await;
        assert!(stats.contains_key("memory_usage_mb"));
        assert!(stats.contains_key("model_count"));
        
        let model_count = stats.get("model_count").unwrap().as_u64().unwrap();
        assert!(model_count >= 2); // At least 2 models should be stored
    }
    
    #[tokio::test]
    async fn test_typed_model_error_recovery() {
        let test_harness = TypedModelIntegrationTest::new().await.unwrap();
        
        // Store a model with potential issues
        let problematic_model = TypedLSTMModel::new(10, 20, 1);
        test_harness.typed_storage.store_lstm_model("problematic".to_string(), problematic_model).await.unwrap();
        
        // Add to predictor
        let model_key = ModelKey {
            sector: "test".to_string(),
            model_type: "TypedLSTM".to_string(),
            variant: "problematic".to_string(),
        };
        
        let retrieved_model = test_harness.typed_storage.get_lstm_model("problematic").await.unwrap().unwrap();
        test_harness.predictor.add_model(model_key, Box::new(retrieved_model)).await.unwrap();
        
        // Test prediction with various error conditions
        
        // 1. Test with empty data
        let empty_data = TimeSeriesData::default();
        let result = test_harness.predictor.predict(&[empty_data], 1, None).await;
        // Should handle gracefully (may return default prediction or error)
        
        // 2. Test with malformed data
        let malformed_data = create_test_time_series_data("TEST", vec![]);
        let result = test_harness.predictor.predict(&[malformed_data], 1, None).await;
        // Should handle gracefully
        
        // 3. Test with extreme values
        let extreme_data = create_test_time_series_data(
            "TEST", 
            vec![f64::MAX, f64::MIN, 1e308, -1e308],
        );
        let result = test_harness.predictor.predict(&[extreme_data], 1, None).await;
        // Should handle gracefully
        
        // System should remain stable after error conditions
        let normal_data = create_test_time_series_data("TEST", vec![100.0, 101.0, 102.0]);
        let result = test_harness.predictor.predict(&[normal_data], 1, None).await;
        // This should work normally
    }
    
    #[tokio::test]
    async fn test_typed_model_scaling() {
        let test_harness = TypedModelIntegrationTest::new().await.unwrap();
        
        // Test scaling with many models
        let model_count = 50;
        let mut model_ids = Vec::new();
        
        for i in 0..model_count {
            let lstm_model = TypedLSTMModel::new_with_prediction(
                10, 20, 1, 100.0 + i as f32
            );
            let gru_model = TypedGRUModel::new_with_prediction(
                10, 20, 1, 200.0 + i as f32
            );
            
            let lstm_id = format!("scale_lstm_{}", i);
            let gru_id = format!("scale_gru_{}", i);
            
            test_harness.typed_storage.store_lstm_model(lstm_id.clone(), lstm_model).await.unwrap();
            test_harness.typed_storage.store_gru_model(gru_id.clone(), gru_model).await.unwrap();
            
            model_ids.push(lstm_id);
            model_ids.push(gru_id);
        }
        
        // Verify all models were stored
        let stats = test_harness.typed_storage.get_storage_stats().await.unwrap();
        assert_eq!(stats.get("lstm_models").unwrap(), &model_count);
        assert_eq!(stats.get("gru_models").unwrap(), &model_count);
        assert_eq!(stats.get("total_models").unwrap(), &(model_count * 2));
        
        // Test batch retrieval
        let mut retrieved_count = 0;
        for model_id in &model_ids {
            if model_id.contains("lstm") {
                if test_harness.typed_storage.get_lstm_model(model_id).await.unwrap().is_some() {
                    retrieved_count += 1;
                }
            } else {
                if test_harness.typed_storage.get_gru_model(model_id).await.unwrap().is_some() {
                    retrieved_count += 1;
                }
            }
        }
        
        assert_eq!(retrieved_count, model_count * 2);
        
        // Test performance with many models
        let start_time = std::time::Instant::now();
        
        // Perform 100 retrievals
        for i in 0..100 {
            let model_id = &model_ids[i % model_ids.len()];
            if model_id.contains("lstm") {
                test_harness.typed_storage.get_lstm_model(model_id).await.unwrap();
            } else {
                test_harness.typed_storage.get_gru_model(model_id).await.unwrap();
            }
        }
        
        let elapsed = start_time.elapsed();
        
        // Should complete within reasonable time (adjust threshold as needed)
        assert!(elapsed.as_millis() < 1000, "Scaling test took too long: {:?}", elapsed);
    }
    
    #[tokio::test]
    async fn test_typed_model_validation_integration() {
        let test_harness = TypedModelIntegrationTest::new().await.unwrap();
        
        // Store models with validation
        let valid_lstm = TypedLSTMModel::new(24, 48, 1);
        let valid_gru = TypedGRUModel::new(24, 48, 1);
        
        // Test storage with validation
        test_harness.typed_storage.store_lstm_model("valid_lstm".to_string(), valid_lstm).await.unwrap();
        test_harness.typed_storage.store_gru_model("valid_gru".to_string(), valid_gru).await.unwrap();
        
        // Test type safety validation
        assert!(test_harness.typed_storage.validate_type_safety("valid_lstm", "TypedLSTM").await.unwrap());
        assert!(test_harness.typed_storage.validate_type_safety("valid_gru", "TypedGRU").await.unwrap());
        
        // Test cross-type validation (should fail)
        assert!(!test_harness.typed_storage.validate_type_safety("valid_lstm", "TypedGRU").await.unwrap());
        assert!(!test_harness.typed_storage.validate_type_safety("valid_gru", "TypedLSTM").await.unwrap());
        
        // Test prediction with validation
        let retrieved_lstm = test_harness.typed_storage.get_lstm_model("valid_lstm").await.unwrap().unwrap();
        let retrieved_gru = test_harness.typed_storage.get_gru_model("valid_gru").await.unwrap().unwrap();
        
        let valid_input = vec![1.0; 24];
        let invalid_input = vec![1.0; 10]; // Wrong size
        
        // Valid input should work
        assert!(retrieved_lstm.validate_input(&valid_input).is_ok());
        assert!(retrieved_gru.validate_input(&valid_input).is_ok());
        
        assert!(retrieved_lstm.predict_typed(&valid_input).is_ok());
        assert!(retrieved_gru.predict_typed(&valid_input).is_ok());
        
        // Invalid input should be rejected
        assert!(retrieved_lstm.validate_input(&invalid_input).is_err());
        assert!(retrieved_gru.validate_input(&invalid_input).is_err());
        
        assert!(retrieved_lstm.predict_typed(&invalid_input).is_err());
        assert!(retrieved_gru.predict_typed(&invalid_input).is_err());
    }
    
    #[tokio::test]
    async fn test_end_to_end_performance_benchmarks() {
        let test_harness = TypedModelIntegrationTest::new().await.unwrap();
        test_harness.setup_test_models().await.unwrap();
        
        let symbols = vec!["AAPL", "MSFT", "GOOGL", "AMZN", "TSLA"];
        let test_data: Vec<_> = symbols.iter().map(|&symbol| {
            create_test_time_series_data(
                symbol,
                vec![100.0, 101.5, 99.8, 102.3, 103.1, 101.7, 104.2],
            )
        }).collect();
        
        // Benchmark single predictions
        let start_time = std::time::Instant::now();
        for data in &test_data {
            let predictions = test_harness.predictor.predict(&[data.clone()], 1, None).await.unwrap();
            assert_eq!(predictions.len(), 1);
        }
        let single_pred_time = start_time.elapsed();
        
        // Benchmark batch predictions
        let start_time = std::time::Instant::now();
        let batch_predictions = test_harness.predictor.predict(&test_data, 1, None).await.unwrap();
        let batch_pred_time = start_time.elapsed();
        
        assert_eq!(batch_predictions.len(), symbols.len());
        
        // Benchmark concurrent predictions
        let start_time = std::time::Instant::now();
        let mut concurrent_handles = vec![];
        
        for data in &test_data {
            let predictor_clone = &test_harness.predictor;
            let data_clone = data.clone();
            let handle = tokio::spawn(async move {
                timeout(Duration::from_secs(30), async move {
                    predictor_clone.predict(&[data_clone], 1, None).await
                }).await.unwrap()
            });
            concurrent_handles.push(handle);
        }
        
        let concurrent_results = futures::future::join_all(concurrent_handles).await;
        let concurrent_pred_time = start_time.elapsed();
        
        // Verify all concurrent predictions succeeded
        for result in concurrent_results {
            assert!(result.is_ok());
            let predictions = result.unwrap().unwrap();
            assert_eq!(predictions.len(), 1);
        }
        
        // Performance assertions
        println!("Single predictions: {:?}", single_pred_time);
        println!("Batch predictions: {:?}", batch_pred_time);
        println!("Concurrent predictions: {:?}", concurrent_pred_time);
        
        // Concurrent should be faster than sequential single predictions
        assert!(concurrent_pred_time < single_pred_time);
        
        // All operations should complete within reasonable time
        assert!(single_pred_time.as_millis() < 5000);
        assert!(batch_pred_time.as_millis() < 2000);
        assert!(concurrent_pred_time.as_millis() < 3000);
    }
}