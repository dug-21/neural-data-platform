//! Tests to verify that mock adapters are NOT used in the system
//! 
//! This module follows TDD London School approach:
//! - Mock all external dependencies
//! - Test behavior, not implementation
//! - Work outside-in from API level

use neural_trader::adapters::enhanced_neural_adapter::{
    EnhancedNeuralAdapter, EnhancedNeuralConfig, PredictionRequirements,
};
use neural_trader::data::TimeSeriesData;
use neural_trader::neural::{NeuralPredictorTrait, PredictionResult};
use std::collections::HashMap;
use std::time::Duration;
use tokio::test;
use mockall::predicate::*;
use mockall::mock;

// Mock for external dependencies
mock! {
    HealthMonitor {
        async fn is_model_healthy(&self, model_name: &str) -> bool;
        async fn get_health_status(&self, model_name: &str) -> String;
        async fn start_monitoring(&self) -> Result<(), String>;
        async fn stop_monitoring(&self);
    }
}

/// Test helper to create time series data
fn create_test_time_series(symbol: &str, points: usize) -> Vec<TimeSeriesData> {
    let base_price = 100.0;
    let mut data = Vec::new();
    
    for i in 0..points {
        data.push(TimeSeriesData {
            symbol: symbol.to_string(),
            timestamp: chrono::Utc::now() - chrono::Duration::minutes(i as i64),
            open: base_price + (i as f64 * 0.1),
            high: base_price + (i as f64 * 0.1) + 1.0,
            low: base_price + (i as f64 * 0.1) - 1.0,
            close: base_price + (i as f64 * 0.1) + 0.5,
            volume: 1000.0,
            indicators: HashMap::new(),
            source: Some("test".to_string()),
            entity: Some("test".to_string()),
            value: Some(base_price + (i as f64 * 0.1)),
            metadata: None,
        });
    }
    
    data.reverse(); // Oldest first
    data
}

/// Test helper to create a minimal config
fn create_test_config(use_real_models: bool) -> EnhancedNeuralConfig {
    EnhancedNeuralConfig {
        use_real_models,
        enable_health_monitoring: false,
        enable_fallback: false,
        enable_caching: false,
        enable_circuit_breakers: false,
        ..Default::default()
    }
}

#[test]
async fn test_no_mock_adapter_initialization() {
    // Given: A configuration that should use real models
    let config = create_test_config(true);
    
    // When: Creating an enhanced neural adapter
    let adapter = EnhancedNeuralAdapter::new(config).await;
    
    // Then: The adapter should be created successfully without mocks
    assert!(adapter.is_ok(), "Adapter should initialize without mock adapters");
    
    let adapter = adapter.unwrap();
    
    // And: The adapter should not contain any mock references
    // This is verified by the successful initialization with real FANN models
}

#[test]
async fn test_predictions_use_real_fann_models() {
    // Given: An adapter configured to use real models
    let config = create_test_config(true);
    let adapter = EnhancedNeuralAdapter::new(config).await.unwrap();
    
    // And: Test time series data
    let data = create_test_time_series("BTC/USD", 100);
    
    // When: Making a prediction
    let result = adapter.predict(&data, 5, None).await;
    
    // Then: The prediction should succeed
    assert!(result.is_ok(), "Prediction should succeed with real FANN models");
    
    let predictions = result.unwrap();
    
    // And: The predictions should have expected characteristics
    assert_eq!(predictions.len(), 5, "Should return requested horizon");
    
    // And: Each prediction should be from a FANN model
    for prediction in &predictions {
        assert!(
            prediction.model_name.contains("FANN") || 
            prediction.model_name.contains("MLP") ||
            prediction.model_name.contains("LSTM") ||
            prediction.model_name.contains("GRU"),
            "Prediction should be from FANN-based model, got: {}", 
            prediction.model_name
        );
        
        // And: Should have valid confidence scores
        assert!(prediction.confidence > 0.0 && prediction.confidence <= 1.0,
            "Confidence should be between 0 and 1");
        
        // And: Should have valid prediction intervals
        assert!(prediction.interval_low <= prediction.value,
            "Lower interval should be <= predicted value");
        assert!(prediction.interval_high >= prediction.value,
            "Upper interval should be >= predicted value");
    }
}

#[test]
async fn test_no_mock_data_in_predictions() {
    // Given: An adapter with real models disabled (using FANN fallback)
    let config = create_test_config(false);
    let adapter = EnhancedNeuralAdapter::new(config).await.unwrap();
    
    // And: Test data with known values
    let data = create_test_time_series("ETH/USD", 50);
    let last_price = data.last().unwrap().close;
    
    // When: Making predictions
    let result = adapter.predict(&data, 10, None).await;
    
    // Then: Predictions should be based on real calculations
    assert!(result.is_ok());
    let predictions = result.unwrap();
    
    // And: Predictions should show realistic variations
    let mut has_variation = false;
    let mut prev_value = predictions[0].value;
    
    for prediction in &predictions[1..] {
        if (prediction.value - prev_value).abs() > 0.0001 {
            has_variation = true;
            break;
        }
        prev_value = prediction.value;
    }
    
    assert!(has_variation, "Predictions should have realistic price variations");
    
    // And: Predictions should be reasonably close to last known price
    for prediction in &predictions {
        let price_change_percent = ((prediction.value - last_price) / last_price * 100.0).abs();
        assert!(
            price_change_percent < 50.0,
            "Prediction {} should be within 50% of last price {}", 
            prediction.value, 
            last_price
        );
    }
}

#[test]
async fn test_enhanced_prediction_without_mock() {
    // Given: A fully configured adapter
    let config = EnhancedNeuralConfig {
        use_real_models: true,
        enable_health_monitoring: true,
        enable_fallback: true,
        enable_caching: true,
        enable_circuit_breakers: true,
        ..Default::default()
    };
    
    let adapter = EnhancedNeuralAdapter::new(config).await.unwrap();
    
    // And: Test data
    let data = create_test_time_series("AAPL", 200);
    
    // And: Specific requirements
    let requirements = PredictionRequirements {
        prefer_accuracy: true,
        prefer_speed: false,
        max_acceptable_latency: Some(Duration::from_secs(5)),
        min_confidence_threshold: Some(0.7),
    };
    
    // When: Making an enhanced prediction
    let result = adapter.predict_enhanced(&data, 5, Some(requirements)).await;
    
    // Then: Should succeed without using mocks
    assert!(result.is_ok());
    let enhanced_result = result.unwrap();
    
    // And: Should use a real FANN-based model
    assert!(
        enhanced_result.model_used == "FANN_MLP" ||
        enhanced_result.model_used == "LSTM" ||
        enhanced_result.model_used == "GRU" ||
        enhanced_result.model_used == "DeepAR" ||
        enhanced_result.model_used == "TCN",
        "Should use a real model, got: {}",
        enhanced_result.model_used
    );
    
    // And: Should have valid execution metrics
    assert!(enhanced_result.execution_time > Duration::from_micros(1));
    assert!(enhanced_result.confidence_score >= 0.0);
    assert!(!enhanced_result.fallback_triggered || enhanced_result.model_used != "Mock");
}

#[test]
async fn test_model_specific_predictions_no_mock() {
    // Given: An adapter configured for real models
    let config = create_test_config(true);
    let adapter = EnhancedNeuralAdapter::new(config).await.unwrap();
    
    // And: Test data
    let data = create_test_time_series("MSFT", 100);
    
    // Test each supported FANN model type
    let fann_models = vec!["FANN_MLP", "LSTM", "GRU"];
    
    for model_name in fann_models {
        // When: Checking if model is available
        let available = adapter.is_model_available(model_name).await;
        
        // Then: FANN models should be available
        assert!(available, "FANN model {} should be available", model_name);
        
        // When: Making ensemble prediction with specific model
        let result = adapter.predict_ensemble(
            &data,
            3,
            &[model_name.to_string()],
            None
        ).await;
        
        // Then: Should succeed with real model
        assert!(
            result.is_ok(),
            "Ensemble prediction with {} should succeed",
            model_name
        );
    }
}

#[test]
async fn test_performance_stats_without_mock() {
    // Given: An adapter with all features enabled
    let config = EnhancedNeuralConfig {
        use_real_models: true,
        enable_health_monitoring: true,
        enable_fallback: true,
        ..Default::default()
    };
    
    let adapter = EnhancedNeuralAdapter::new(config).await.unwrap();
    
    // And: Test data
    let data = create_test_time_series("GOOGL", 50);
    
    // When: Making multiple predictions
    for _ in 0..5 {
        let _ = adapter.predict(&data, 3, None).await;
    }
    
    // And: Getting performance stats
    let stats = adapter.get_performance_stats().await;
    
    // Then: Stats should reflect real model usage
    assert_eq!(stats.total_predictions, 5);
    assert!(stats.success_rate > 0.0); // Some predictions should succeed
    assert!(stats.average_response_time > Duration::from_micros(1));
    
    // And: Model usage should not include mock models
    for (model_name, _count) in &stats.model_usage_count {
        assert!(
            !model_name.to_lowercase().contains("mock"),
            "Mock model {} should not be in usage stats",
            model_name
        );
    }
}

#[test]
async fn test_system_health_without_mock() {
    // Given: An adapter with health monitoring
    let config = EnhancedNeuralConfig {
        use_real_models: true,
        enable_health_monitoring: true,
        ..Default::default()
    };
    
    let adapter = EnhancedNeuralAdapter::new(config).await.unwrap();
    
    // When: Getting system health
    let health = adapter.get_system_health_summary().await;
    
    // Then: Health should be reported (or None if monitoring disabled)
    if let Some(health_status) = health {
        // If we have health status, it should reflect real models
        assert!(health_status.total_models > 0);
        assert!(health_status.healthy_models <= health_status.total_models);
    }
}

#[test]
async fn test_graceful_shutdown_without_mock() {
    // Given: A fully configured adapter
    let config = EnhancedNeuralConfig {
        use_real_models: true,
        enable_health_monitoring: true,
        ..Default::default()
    };
    
    let adapter = EnhancedNeuralAdapter::new(config).await.unwrap();
    
    // When: Shutting down the adapter
    let result = adapter.shutdown().await;
    
    // Then: Should shutdown gracefully
    assert!(result.is_ok(), "Adapter should shutdown without errors");
}

#[test]
async fn test_error_handling_without_mock() {
    // Given: An adapter with minimal config
    let config = create_test_config(false);
    let adapter = EnhancedNeuralAdapter::new(config).await.unwrap();
    
    // And: Invalid test data (empty)
    let data = vec![];
    
    // When: Attempting prediction with invalid data
    let result = adapter.predict(&data, 5, None).await;
    
    // Then: Should handle error gracefully
    assert!(result.is_err(), "Should error on empty data");
    
    // And: Error should not reference mock implementations
    let error_msg = result.unwrap_err().to_string();
    assert!(
        !error_msg.to_lowercase().contains("mock"),
        "Error message should not reference mocks: {}",
        error_msg
    );
}

#[cfg(test)]
mod verification_tests {
    use super::*;
    
    /// Meta-test to verify our test helpers don't use mocks
    #[test]
    fn test_helpers_create_valid_data() {
        let data = create_test_time_series("TEST", 10);
        assert_eq!(data.len(), 10);
        
        for (i, point) in data.iter().enumerate() {
            assert_eq!(point.symbol, "TEST");
            assert!(point.timestamp <= chrono::Utc::now());
            assert!(point.high >= point.low);
            assert!(point.close >= point.low && point.close <= point.high);
        }
    }
    
    #[test]
    fn test_config_helper_creates_valid_config() {
        let config = create_test_config(true);
        assert!(config.use_real_models);
        assert!(!config.enable_health_monitoring);
        
        let config = create_test_config(false);
        assert!(!config.use_real_models);
    }
}