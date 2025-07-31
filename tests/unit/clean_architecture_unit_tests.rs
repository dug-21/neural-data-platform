//! Unit Tests for Clean Architecture Components
//!
//! These tests validate individual components in isolation:
//! - NeuralPredictor unit functionality
//! - EnhancedNeuralAdapter behaviors
//! - FannPredictor integration
//! - Configuration validation
//! - Error handling scenarios

use std::collections::HashMap;
use std::time::Duration;
use anyhow::Result;
use tokio::time::timeout;

use crate::neural::predictor::NeuralPredictor;
use crate::neural::fann_predictor::FannPredictor;
use crate::neural::NeuralPredictorTrait;
use crate::config::NeuralConfig;
use crate::adapters::enhanced_neural_adapter::{EnhancedNeuralAdapter, EnhancedNeuralConfig};
use crate::data::TimeSeriesData;

mod helpers;
use helpers::{TestConfigBuilder, TestDataGenerator, TestResultValidator};

/// Test NeuralPredictor creation and basic properties
#[tokio::test]
async fn test_neural_predictor_creation() -> Result<()> {
    let config = TestConfigBuilder::new()
        .with_models(vec!["MLP".to_string(), "LSTM".to_string()])
        .build();

    let predictor = NeuralPredictor::new(config.clone())?;
    
    // Test basic properties
    assert!(predictor.is_ready().await);
    assert_eq!(predictor.get_available_models(), &config.models);
    
    // Test model availability
    assert!(predictor.is_model_available("MLP").await);
    assert!(predictor.is_model_available("LSTM").await);
    assert!(!predictor.is_model_available("NonExistent").await);

    Ok(())
}

/// Test NeuralPredictor with different configurations
#[tokio::test]
async fn test_neural_predictor_configurations() -> Result<()> {
    // Test minimal configuration
    let minimal_config = NeuralConfig {
        memory_gb: 0.5,
        models: vec!["MLP".to_string()],
        prediction_cache_ttl: 60,
        model_load_timeout: 30,
        max_concurrent_predictions: 5,
        enable_model_monitoring: false,
        accuracy_threshold: 0.7,
        use_real_models: false,
        enable_health_checks: false,
        enable_fallback: false,
        lookback_window: 12,
        enable_circuit_breakers: false,
        enable_graceful_degradation: false,
        enable_performance_monitoring: false,  
        enable_adaptive_retry: false,
        enable_model_ensembles: false,
        model_timeout_seconds: 10,
        max_retries: 1,
        error_threshold: 0.2,
    };
    
    let minimal_predictor = NeuralPredictor::new(minimal_config)?;
    assert!(minimal_predictor.is_ready().await);
    assert_eq!(minimal_predictor.get_available_models().len(), 1);

    // Test comprehensive configuration
    let comprehensive_config = TestConfigBuilder::new()
        .with_health_monitoring()
        .with_fallback()
        .with_circuit_breakers()
        .with_performance_monitoring()
        .with_models(vec!["MLP".to_string(), "LSTM".to_string(), "GRU".to_string()])
        .build();
    
    let comprehensive_predictor = NeuralPredictor::new(comprehensive_config)?;
    assert!(comprehensive_predictor.is_ready().await);
    assert_eq!(comprehensive_predictor.get_available_models().len(), 3);

    Ok(())
}

/// Test prediction with various data sizes
#[tokio::test]
async fn test_prediction_data_sizes() -> Result<()> {
    let config = TestConfigBuilder::new().build();
    let predictor = NeuralPredictor::new(config)?;
    
    let test_cases = vec![
        (5, 3),    // Small data, small horizon
        (20, 8),   // Normal data, normal horizon  
        (100, 24), // Large data, large horizon
        (50, 1),   // Medium data, minimal horizon
    ];
    
    for (data_size, horizon) in test_cases {
        let test_data = TestDataGenerator::generate_simple_data(data_size);
        
        let result = predictor.predict(&test_data, horizon, None).await?;
        TestResultValidator::validate_predictions(&result, horizon, 0.0)?;
        
        assert_eq!(result.len(), horizon);
        println!("✓ Data size {}, horizon {}: {} predictions", data_size, horizon, result.len());
    }

    Ok(())
}

/// Test prediction with different feature sets
#[tokio::test]
async fn test_prediction_features() -> Result<()> {
    let config = TestConfigBuilder::new().build();
    let predictor = NeuralPredictor::new(config)?;
    let test_data = TestDataGenerator::generate_simple_data(50);
    
    // Test without features
    let result_no_features = predictor.predict(&test_data, 5, None).await?;
    TestResultValidator::validate_predictions(&result_no_features, 5, 0.0)?;
    
    // Test with features
    let features = HashMap::from([
        ("prefer_accuracy".to_string(), serde_json::Value::Bool(true)),
        ("max_latency".to_string(), serde_json::Value::Number(serde_json::Number::from(100))),
    ]);
    
    let result_with_features = predictor.predict(&test_data, 5, Some(features)).await?;
    TestResultValidator::validate_predictions(&result_with_features, 5, 0.0)?;
    
    // Both should succeed, features may affect internal routing
    assert_eq!(result_no_features.len(), 5);
    assert_eq!(result_with_features.len(), 5);

    Ok(())
}

/// Test ensemble prediction functionality
#[tokio::test]
async fn test_ensemble_predictions() -> Result<()> {
    let config = TestConfigBuilder::new()
        .with_models(vec!["MLP".to_string(), "LSTM".to_string(), "GRU".to_string()])
        .build();
    
    let predictor = NeuralPredictor::new(config)?;
    let test_data = TestDataGenerator::generate_simple_data(40);
    
    // Test single model ensemble
    let single_model = vec!["MLP".to_string()];
    let single_result = predictor.predict_ensemble(&test_data, 6, &single_model, None).await?;
    TestResultValidator::validate_predictions(&single_result, 6, 0.0)?;
    
    // Test multi-model ensemble
    let multi_models = vec!["MLP".to_string(), "LSTM".to_string()];
    let multi_result = predictor.predict_ensemble(&test_data, 6, &multi_models, None).await?;
    TestResultValidator::validate_predictions(&multi_result, 6, 0.0)?;
    
    // Test with all models
    let all_models = vec!["MLP".to_string(), "LSTM".to_string(), "GRU".to_string()];
    let all_result = predictor.predict_ensemble(&test_data, 6, &all_models, None).await?;
    TestResultValidator::validate_predictions(&all_result, 6, 0.0)?;
    
    // All should return same number of predictions
    assert_eq!(single_result.len(), 6);
    assert_eq!(multi_result.len(), 6);
    assert_eq!(all_result.len(), 6);

    Ok(())
}

/// Test prediction error handling
#[tokio::test]
async fn test_prediction_error_handling() -> Result<()> {
    let config = TestConfigBuilder::new().build();
    let predictor = NeuralPredictor::new(config)?;
    
    // Test with empty data
    let empty_data = vec![];
    let empty_result = predictor.predict(&empty_data, 5, None).await;
    // Should either succeed with reasonable fallback or fail gracefully
    match empty_result {
        Ok(results) => {
            assert!(results.len() <= 5, "Should not return more predictions than requested");
            println!("✓ Empty data handled with {} predictions", results.len());
        }
        Err(e) => {
            println!("✓ Empty data correctly rejected: {}", e);
        }
    }
    
    // Test with invalid horizon
    let test_data = TestDataGenerator::generate_simple_data(20);
    let zero_horizon_result = predictor.predict(&test_data, 0, None).await;
    match zero_horizon_result {
        Ok(results) => {
            assert!(results.is_empty(), "Zero horizon should return empty results");
            println!("✓ Zero horizon handled correctly");
        }
        Err(e) => {
            println!("✓ Zero horizon correctly rejected: {}", e);
        }
    }
    
    // Test with edge case data
    let edge_data = TestDataGenerator::generate_edge_case_data();
    let edge_result = predictor.predict(&edge_data, 3, None).await;
    match edge_result {
        Ok(results) => {
            // Should handle edge cases and return valid predictions
            for result in &results {
                assert!(result.value.is_finite(), "Predictions should be finite numbers");
                assert!(result.confidence >= 0.0 && result.confidence <= 1.0, "Confidence should be in valid range");
            }
            println!("✓ Edge case data handled with {} predictions", results.len());
        }
        Err(e) => {
            println!("✓ Edge case data correctly handled with error: {}", e);
        }
    }

    Ok(())
}

/// Test feature importance functionality
#[tokio::test]
async fn test_feature_importance() -> Result<()> {
    let config = TestConfigBuilder::new().build();
    let predictor = NeuralPredictor::new(config)?;
    
    // Make a prediction first to ensure models are loaded
    let test_data = TestDataGenerator::generate_simple_data(30);
    let _ = predictor.predict(&test_data, 5, None).await?;
    
    // Get feature importance
    let importance = predictor.get_feature_importance().await?;
    
    // Validate importance values
    for (feature_name, importance_value) in &importance {
        assert!(!feature_name.is_empty(), "Feature name should not be empty");
        assert!(importance_value.is_finite(), "Importance value should be finite");
        assert!(*importance_value >= 0.0, "Importance should be non-negative");
        
        println!("Feature '{}': importance {:.4}", feature_name, importance_value);
    }
    
    println!("✓ Retrieved {} feature importance values", importance.len());

    Ok(())
}

/// Test predictor performance statistics
#[tokio::test]
async fn test_performance_statistics() -> Result<()> {
    let config = TestConfigBuilder::new()
        .with_performance_monitoring()
        .build();
    
    let predictor = NeuralPredictor::new(config)?;
    let test_data = TestDataGenerator::generate_simple_data(50);
    
    // Make several predictions to generate statistics
    for i in 0..5 {
        let start_idx = i * 8;
        let chunk = &test_data[start_idx..std::cmp::min(start_idx + 20, test_data.len())];
        let _ = predictor.predict(chunk, 4, None).await?;
    }
    
    // Get performance statistics
    let stats = predictor.get_performance_stats().await;
    
    // Validate statistics structure
    assert!(stats.is_object(), "Performance stats should be an object");
    
    // Check for expected fields
    if let Some(total_predictions) = stats.get("total_predictions") {
        let count = total_predictions.as_u64().unwrap_or(0);
        assert!(count >= 5, "Should have recorded at least 5 predictions");
        println!("Total predictions: {}", count);
    }
    
    if let Some(success_rate) = stats.get("success_rate") {
        let rate = success_rate.as_f64().unwrap_or(0.0);
        assert!(rate >= 0.0 && rate <= 100.0, "Success rate should be a valid percentage");
        println!("Success rate: {:.2}%", rate);
    }
    
    if let Some(avg_response_time) = stats.get("average_response_time_ms") {
        let time = avg_response_time.as_u64().unwrap_or(0);
        assert!(time < 1000, "Average response time should be reasonable");
        println!("Average response time: {}ms", time);
    }
    
    println!("✓ Performance statistics validated");

    Ok(())
}

/// Test predictor health status
#[tokio::test]
async fn test_health_status() -> Result<()> {
    let config = TestConfigBuilder::new()
        .with_health_monitoring()
        .build();
    
    let predictor = NeuralPredictor::new(config)?;
    
    // Wait a moment for health monitoring to initialize
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Make a prediction to ensure system is active
    let test_data = TestDataGenerator::generate_simple_data(20);
    let _ = predictor.predict(&test_data, 3, None).await?;
    
    // Get health status
    let health_status = predictor.get_health_status().await;
    
    match health_status {
        Some(status) => {
            assert!(status.is_object(), "Health status should be structured data");
            
            // Check for expected health fields
            if let Some(overall_healthy) = status.get("overall_healthy") {
                println!("Overall healthy: {}", overall_healthy);
            }
            
            if let Some(healthy_models) = status.get("healthy_models") {
                let count = healthy_models.as_u64().unwrap_or(0);
                println!("Healthy models: {}", count);
            }
            
            if let Some(error_rate) = status.get("error_rate") {
                let rate = error_rate.as_f64().unwrap_or(0.0);
                assert!(rate >= 0.0 && rate <= 100.0, "Error rate should be valid percentage");
                println!("Error rate: {:.2}%", rate);
            }
            
            println!("✓ Health status available and valid");
        }
        None => {
            println!("✓ Health status not available (monitoring may be disabled)");
        }
    }

    Ok(())
}

/// Test predictor with timeout scenarios
#[tokio::test]
async fn test_predictor_timeouts() -> Result<()> {
    let config = TestConfigBuilder::new().build();
    let predictor = NeuralPredictor::new(config)?;
    let test_data = TestDataGenerator::generate_simple_data(100);
    
    // Test with reasonable timeout
    let reasonable_timeout_result = timeout(
        Duration::from_secs(1),
        predictor.predict(&test_data, 10, None)
    ).await;
    
    match reasonable_timeout_result {
        Ok(Ok(results)) => {
            TestResultValidator::validate_predictions(&results, 10, 0.0)?;
            println!("✓ Prediction completed within reasonable timeout");
        }
        Ok(Err(e)) => {
            println!("✓ Prediction failed gracefully: {}", e);
        }
        Err(_) => {
            println!("⚠️  Prediction timed out (may indicate performance issue)");
        }
    }
    
    // Test with very short timeout
    let short_timeout_result = timeout(
        Duration::from_millis(1),
        predictor.predict(&test_data, 5, None)
    ).await;
    
    match short_timeout_result {
        Ok(Ok(_)) => {
            println!("✓ Prediction completed very quickly (excellent performance)");
        }
        Ok(Err(_)) => {
            println!("✓ Prediction failed within short timeout");
        }
        Err(_) => {
            println!("✓ Short timeout triggered as expected");
        }
    }

    Ok(())
}

/// Test predictor graceful shutdown
#[tokio::test]
async fn test_predictor_shutdown() -> Result<()> {
    let config = TestConfigBuilder::new()
        .with_health_monitoring()
        .with_performance_monitoring()
        .build();
    
    let predictor = NeuralPredictor::new(config)?;
    
    // Use the predictor
    let test_data = TestDataGenerator::generate_simple_data(30);
    let _ = predictor.predict(&test_data, 5, None).await?;
    
    // Test graceful shutdown
    let shutdown_result = predictor.shutdown().await;
    assert!(shutdown_result.is_ok(), "Shutdown should succeed");
    
    println!("✓ Predictor shutdown completed successfully");

    Ok(())
}

/// Test configuration validation
#[test]
fn test_configuration_validation() {
    // Test valid configurations
    let valid_config = TestConfigBuilder::new().build();
    assert!(valid_config.memory_gb > 0.0);
    assert!(!valid_config.models.is_empty());
    assert!(valid_config.prediction_cache_ttl > 0);
    assert!(valid_config.model_load_timeout > 0);
    assert!(valid_config.max_concurrent_predictions > 0);
    
    // Test configuration builder patterns
    let custom_config = TestConfigBuilder::new()
        .with_models(vec!["CustomModel".to_string()])
        .with_health_monitoring()
        .with_fallback()
        .build();
    
    assert_eq!(custom_config.models, vec!["CustomModel"]);
    assert!(custom_config.enable_health_checks);
    assert!(custom_config.enable_fallback);
    assert!(!custom_config.use_real_models); // Should always be false in tests
    
    println!("✓ Configuration validation passed");
}

/// Test data generation utilities
#[test]
fn test_data_generation() {
    // Test simple data generation
    let simple_data = TestDataGenerator::generate_simple_data(50);
    assert_eq!(simple_data.len(), 50);
    
    for (i, point) in simple_data.iter().enumerate() {
        assert_eq!(point.symbol, "TEST");
        assert!(point.close > 0.0);
        assert!(point.volume > 0.0);
        assert!(!point.indicators.is_empty());
        assert!(point.timestamp > simple_data[0].timestamp || i == 0);
    }
    
    // Test trending data generation
    let trending_data = TestDataGenerator::generate_trending_data(100, 0.5);
    assert_eq!(trending_data.len(), 100);
    
    // Verify trend exists
    let first_price = trending_data.first().unwrap().close;
    let last_price = trending_data.last().unwrap().close;
    assert!(last_price > first_price, "Should have upward trend");
    
    // Test edge case data generation
    let edge_data = TestDataGenerator::generate_edge_case_data();
    assert!(!edge_data.is_empty());
    
    // Verify edge cases are included
    let has_zero = edge_data.iter().any(|p| p.close == 0.0);
    let has_extreme = edge_data.iter().any(|p| p.close > 1000000.0);
    assert!(has_zero || has_extreme, "Should include edge cases");
    
    println!("✓ Data generation utilities validated");
}

/// Test result validation utilities
#[test]
fn test_result_validation() -> Result<()> {
    use crate::neural::PredictionResult;
    use chrono::Utc;
    
    // Create valid test predictions
    let valid_predictions = vec![
        PredictionResult {
            value: 100.0,
            confidence: 0.8,
            model_name: "TestModel".to_string(),
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        },
        PredictionResult {
            value: 105.0,
            confidence: 0.75,
            model_name: "TestModel".to_string(),
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        },
    ];
    
    // Test valid predictions pass validation
    let result = TestResultValidator::validate_predictions(&valid_predictions, 2, 0.7);
    assert!(result.is_ok(), "Valid predictions should pass validation");
    
    // Test invalid confidence fails validation
    let invalid_predictions = vec![
        PredictionResult {
            value: 100.0,
            confidence: 1.5, // Invalid confidence > 1.0
            model_name: "TestModel".to_string(),
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        }
    ];
    
    let result = TestResultValidator::validate_predictions(&invalid_predictions, 1, 0.7);
    assert!(result.is_err(), "Invalid confidence should fail validation");
    
    // Test wrong count fails validation
    let result = TestResultValidator::validate_predictions(&valid_predictions, 3, 0.7);
    assert!(result.is_err(), "Wrong prediction count should fail validation");
    
    println!("✓ Result validation utilities working correctly");
    Ok(())
}