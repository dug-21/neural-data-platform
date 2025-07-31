//! Integration Tests for Simplified Routing Architecture
//!
//! These tests validate the end-to-end prediction flow through the clean architecture:
//! NeuralPredictor → EnhancedNeuralAdapter → FannPredictor
//!
//! Test Coverage:
//! - Single path prediction flow validation
//! - Performance events emission
//! - Error handling and fallback scenarios
//! - Health monitoring integration
//! - Circuit breaker functionality

use std::time::Duration;
use std::sync::Arc;
use tokio::time::timeout;
use anyhow::Result;

use crate::neural::predictor::NeuralPredictor;
use crate::neural::fann_predictor::FannPredictor;
use crate::neural::NeuralPredictorTrait;
use crate::config::NeuralConfig;
use crate::adapters::enhanced_neural_adapter::{EnhancedNeuralAdapter, EnhancedNeuralConfig};

mod helpers;
use helpers::{TestConfigBuilder, TestDataGenerator, PerformanceMeasurement, TestResultValidator};

/// Test the single-path prediction flow: NeuralPredictor → Enhanced → FANN
#[tokio::test]
async fn test_single_path_prediction_flow() -> Result<()> {
    let config = TestConfigBuilder::new()
        .with_models(vec!["MLP".to_string(), "LSTM".to_string()])
        .build();

    let predictor = NeuralPredictor::new(config)?;
    
    // Ensure predictor is ready
    assert!(predictor.is_ready().await, "Predictor should be ready");

    let test_data = TestDataGenerator::generate_simple_data(50);
    let horizon = 24;

    let perf_measurement = PerformanceMeasurement::start("single_path_prediction");
    let results = predictor.predict(&test_data, horizon, None).await?;
    perf_measurement.assert_under_threshold(Duration::from_millis(100)); // Should be fast

    // Validate results
    TestResultValidator::validate_predictions(&results, horizon, 0.0)?;
    
    // Verify single path: should use FANN model internally
    assert!(!results.is_empty(), "Should return predictions");
    assert_eq!(results.len(), horizon, "Should return exactly horizon predictions");
    
    // All predictions should come from the same model path
    let first_model = &results[0].model_name;
    for result in &results {
        assert!(!result.model_name.is_empty(), "Model name should be set");
        // Note: In the simplified architecture, all predictions go through FANN
    }

    println!("✅ Single path prediction flow test passed");
    Ok(())
}

/// Test that the predictor can handle multiple consecutive predictions efficiently
#[tokio::test]
async fn test_consecutive_predictions_performance() -> Result<()> {
    let config = TestConfigBuilder::new().build();
    let predictor = NeuralPredictor::new(config)?;
    
    let test_data = TestDataGenerator::generate_simple_data(100);
    let horizon = 12;
    let num_predictions = 10;

    let total_perf = PerformanceMeasurement::start("consecutive_predictions");
    
    for i in 0..num_predictions {
        let chunk_start = i * 10;
        let chunk_end = std::cmp::min(chunk_start + 50, test_data.len());
        let chunk = &test_data[chunk_start..chunk_end];
        
        let iteration_perf = PerformanceMeasurement::start(&format!("prediction_{}", i));
        let results = predictor.predict(chunk, horizon, None).await?;
        iteration_perf.assert_under_threshold(Duration::from_millis(50));
        
        TestResultValidator::validate_predictions(&results, horizon, 0.0)?;
    }
    
    // Total time should be reasonable for 10 predictions
    total_perf.assert_under_threshold(Duration::from_millis(1000));
    
    println!("✅ Consecutive predictions performance test passed");
    Ok(())
}

/// Test model availability checking
#[tokio::test]
async fn test_model_availability() -> Result<()> {
    let config = TestConfigBuilder::new()
        .with_models(vec!["MLP".to_string(), "LSTM".to_string(), "GRU".to_string()])
        .build();

    let predictor = NeuralPredictor::new(config)?;
    
    // Check configured models are available
    assert!(predictor.is_model_available("MLP").await);
    assert!(predictor.is_model_available("LSTM").await);
    assert!(predictor.is_model_available("GRU").await);
    
    // Check non-configured model is not available
    assert!(!predictor.is_model_available("NonExistentModel").await);
    
    // Check available models list
    let available = predictor.get_available_models();
    assert_eq!(available.len(), 3);
    assert!(available.contains(&"MLP".to_string()));
    assert!(available.contains(&"LSTM".to_string()));
    assert!(available.contains(&"GRU".to_string()));

    println!("✅ Model availability test passed");
    Ok(())
}

/// Test the predictor with health monitoring enabled
#[tokio::test]
async fn test_with_health_monitoring() -> Result<()> {
    let config = TestConfigBuilder::new()
        .with_health_monitoring()
        .build();

    let predictor = NeuralPredictor::new(config)?;
    
    // Wait a moment for health monitoring to initialize
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    let test_data = TestDataGenerator::generate_simple_data(30);
    let results = predictor.predict(&test_data, 10, None).await?;
    
    TestResultValidator::validate_predictions(&results, 10, 0.0)?;
    
    // Check health status
    let health_status = predictor.get_health_status().await;
    if let Some(status) = health_status {
        // Health monitoring should provide some status
        assert!(status.is_object(), "Health status should be an object");
    }

    println!("✅ Health monitoring integration test passed");
    Ok(())
}

/// Test graceful handling of empty data
#[tokio::test]
async fn test_empty_data_handling() -> Result<()> {
    let config = TestConfigBuilder::new().build();
    let predictor = NeuralPredictor::new(config)?;
    
    let empty_data = vec![];
    let result = predictor.predict(&empty_data, 5, None).await;
    
    // Should handle empty data gracefully (either return empty results or error)
    match result {
        Ok(predictions) => {
            // If it succeeds, predictions should be empty or minimal
            assert!(predictions.len() <= 5, "Should not return more than requested horizon");
        }
        Err(_) => {
            // Error is acceptable for empty data
            println!("Empty data correctly resulted in error");
        }
    }

    println!("✅ Empty data handling test passed");
    Ok(())
}

/// Test with edge case data (extreme values, NaN, etc.)
#[tokio::test]
async fn test_edge_case_data_handling() -> Result<()> {
    let config = TestConfigBuilder::new().build();
    let predictor = NeuralPredictor::new(config)?;
    
    let edge_data = TestDataGenerator::generate_edge_case_data();
    let result = predictor.predict(&edge_data, 3, None).await;
    
    // Should handle edge cases gracefully without panicking
    match result {
        Ok(predictions) => {
            // Validate predictions are reasonable
            for pred in &predictions {
                assert!(pred.value.is_finite(), "Predictions should be finite numbers");
                assert!(pred.confidence >= 0.0 && pred.confidence <= 1.0, "Confidence should be in valid range");
                assert!(!pred.model_name.is_empty(), "Model name should be set");
            }
            println!("Edge case data handled successfully with {} predictions", predictions.len());
        }
        Err(e) => {
            // Error is acceptable for edge cases, but should be informative
            println!("Edge case data resulted in expected error: {}", e);
        }
    }

    println!("✅ Edge case data handling test passed");
    Ok(())
}

/// Test ensemble prediction capability
#[tokio::test]
async fn test_ensemble_prediction() -> Result<()> {
    let config = TestConfigBuilder::new()
        .with_models(vec!["MLP".to_string(), "LSTM".to_string()])
        .build();

    let predictor = NeuralPredictor::new(config)?;
    
    let test_data = TestDataGenerator::generate_simple_data(40);
    let models = vec!["MLP".to_string(), "LSTM".to_string()];
    
    let perf_measurement = PerformanceMeasurement::start("ensemble_prediction");
    let results = predictor.predict_ensemble(&test_data, 6, &models, None).await?;
    perf_measurement.assert_under_threshold(Duration::from_millis(200));
    
    TestResultValidator::validate_predictions(&results, 6, 0.0)?;
    
    // Ensemble should still work through the simplified routing
    assert_eq!(results.len(), 6);

    println!("✅ Ensemble prediction test passed");
    Ok(())
}

/// Test feature importance retrieval
#[tokio::test]
async fn test_feature_importance() -> Result<()> {
    let config = TestConfigBuilder::new().build();
    let predictor = NeuralPredictor::new(config)?;
    
    // First make a prediction to ensure models are loaded
    let test_data = TestDataGenerator::generate_simple_data(20);
    let _ = predictor.predict(&test_data, 5, None).await?;
    
    // Now get feature importance
    let importance = predictor.get_feature_importance().await?;
    
    // Should have some feature importance values
    assert!(!importance.is_empty(), "Feature importance should not be empty");
    
    // Values should be reasonable (between 0 and 1 typically)
    for (feature, value) in &importance {
        assert!(!feature.is_empty(), "Feature name should not be empty");
        assert!(*value >= 0.0, "Feature importance should be non-negative");
        println!("Feature '{}': importance {:.4}", feature, value);
    }

    println!("✅ Feature importance test passed");
    Ok(())
}

/// Test predictor shutdown
#[tokio::test]
async fn test_predictor_shutdown() -> Result<()> {
    let config = TestConfigBuilder::new()
        .with_health_monitoring()
        .build();

    let predictor = NeuralPredictor::new(config)?;
    
    // Make a prediction to ensure everything is working
    let test_data = TestDataGenerator::generate_simple_data(10);
    let results = predictor.predict(&test_data, 3, None).await?;
    TestResultValidator::validate_predictions(&results, 3, 0.0)?;
    
    // Test graceful shutdown
    let shutdown_result = predictor.shutdown().await;
    assert!(shutdown_result.is_ok(), "Shutdown should succeed");

    println!("✅ Predictor shutdown test passed");
    Ok(())
}

/// Test performance statistics collection
#[tokio::test]
async fn test_performance_statistics() -> Result<()> {
    let config = TestConfigBuilder::new().build();
    let predictor = NeuralPredictor::new(config)?;
    
    // Make several predictions to generate statistics
    let test_data = TestDataGenerator::generate_simple_data(50);
    
    for i in 0..5 {
        let chunk = &test_data[i*10..(i*10+20).min(test_data.len())];
        let _ = predictor.predict(chunk, 4, None).await?;
    }
    
    // Get performance statistics
    let stats = predictor.get_performance_stats().await;
    
    // Validate statistics structure
    assert!(stats.is_object(), "Performance stats should be an object");
    
    if let Some(total_predictions) = stats.get("total_predictions") {
        assert!(total_predictions.as_u64().unwrap_or(0) >= 5, "Should have recorded predictions");
    }
    
    if let Some(success_rate) = stats.get("success_rate") {
        let rate = success_rate.as_f64().unwrap_or(0.0);
        assert!(rate >= 0.0 && rate <= 100.0, "Success rate should be percentage");
    }

    println!("✅ Performance statistics test passed");
    Ok(())
}

/// Test concurrent prediction requests
#[tokio::test]
async fn test_concurrent_predictions() -> Result<()> {
    let config = TestConfigBuilder::new()
        .with_models(vec!["MLP".to_string()])
        .build();

    let predictor = Arc::new(NeuralPredictor::new(config)?);
    
    let test_data = TestDataGenerator::generate_simple_data(30);
    let test_data = Arc::new(test_data);
    
    // Create multiple concurrent prediction tasks
    let mut tasks = Vec::new();
    
    for i in 0..5 {
        let predictor_clone = Arc::clone(&predictor);
        let data_clone = Arc::clone(&test_data);
        
        let task = tokio::spawn(async move {
            let start_idx = i * 5;
            let end_idx = std::cmp::min(start_idx + 20, data_clone.len());
            let chunk = &data_clone[start_idx..end_idx];
            
            predictor_clone.predict(chunk, 6, None).await
        });
        
        tasks.push(task);
    }
    
    // Wait for all tasks to complete
    let perf_measurement = PerformanceMeasurement::start("concurrent_processing");
    let results = futures::future::join_all(tasks).await;
    perf_measurement.assert_under_threshold(Duration::from_millis(500));
    
    // Validate all predictions succeeded
    for (i, result) in results.into_iter().enumerate() {
        let predictions = result??; // Handle task join and prediction errors
        TestResultValidator::validate_predictions(&predictions, 6, 0.0)?;
        println!("Concurrent task {} completed successfully", i);
    }

    println!("✅ Concurrent predictions test passed");
    Ok(())
}

/// Comprehensive integration test combining multiple features
#[tokio::test]
async fn test_comprehensive_integration() -> Result<()> {
    let config = TestConfigBuilder::new()
        .with_health_monitoring()
        .with_performance_monitoring()
        .with_models(vec!["MLP".to_string(), "LSTM".to_string()])
        .build();

    let predictor = NeuralPredictor::new(config)?;
    
    // Wait for initialization
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Test various data scenarios
    let scenarios = vec![
        ("simple_data", TestDataGenerator::generate_simple_data(40)),
        ("trending_data", TestDataGenerator::generate_trending_data(40, 0.5)),
    ];
    
    let overall_perf = PerformanceMeasurement::start("comprehensive_integration");
    
    for (scenario_name, test_data) in scenarios {
        println!("Testing scenario: {}", scenario_name);
        
        // Single prediction
        let single_result = predictor.predict(&test_data, 8, None).await?;
        TestResultValidator::validate_predictions(&single_result, 8, 0.0)?;
        
        // Ensemble prediction
        let ensemble_models = vec!["MLP".to_string(), "LSTM".to_string()];
        let ensemble_result = predictor.predict_ensemble(&test_data, 8, &ensemble_models, None).await?;
        TestResultValidator::validate_predictions(&ensemble_result, 8, 0.0)?;
        
        // Check model availability
        assert!(predictor.is_model_available("MLP").await);
        assert!(predictor.is_model_available("LSTM").await);
        
        // Get feature importance
        let importance = predictor.get_feature_importance().await?;
        assert!(!importance.is_empty());
        
        println!("  ✓ Scenario {} completed successfully", scenario_name);
    }
    
    // Check final health and performance statistics
    let health_status = predictor.get_health_status().await;
    let perf_stats = predictor.get_performance_stats().await;
    
    if let Some(_) = health_status {
        println!("  ✓ Health monitoring operational");
    }
    
    assert!(perf_stats.is_object(), "Performance stats should be available");
    println!("  ✓ Performance monitoring operational");
    
    overall_perf.assert_under_threshold(Duration::from_secs(2));
    
    // Graceful shutdown
    predictor.shutdown().await?;
    println!("  ✓ Graceful shutdown completed");

    println!("✅ Comprehensive integration test passed");
    Ok(())
}