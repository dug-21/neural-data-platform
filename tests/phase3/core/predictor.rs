//! NeuralPredictor Integration Tests for Phase 3
//!
//! Tests focusing on async initialization and current API compatibility

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing_test::traced_test;

use neural_trader::config::NeuralConfig;
use neural_trader::data::TimeSeriesData;
use neural_trader::neural::{NeuralPredictor, NeuralPredictorTrait, PredictionResult};

use crate::phase3::utilities::*;

#[traced_test]
#[tokio::test]
async fn test_neural_predictor_async_initialization() -> Result<()> {
    let config = Phase3TestConfig::default();
    let memory_tracker = MemoryTracker::new(config.memory_budget_mb);
    
    // Test async initialization with timeout
    let predictor = with_timeout(
        create_test_neural_predictor(None),
        config.max_test_duration_secs
    ).await?;
    
    // Verify predictor is ready for use
    assert!(Arc::strong_count(&predictor) >= 1);
    
    // Check memory budget compliance
    assert!(memory_tracker.check_budget_compliance().await?);
    
    Ok(())
}

#[traced_test]
#[tokio::test]
async fn test_neural_predictor_phase3_api_compatibility() -> Result<()> {
    let predictor = create_test_neural_predictor(None).await?;
    let timestamp = chrono::Utc::now();
    let data = create_test_time_series_data("AAPL", timestamp);
    
    // Test prediction with current Phase 3 TimeSeriesData structure
    let result = predictor.predict(&data).await?;
    
    // Verify prediction result structure
    assert!(result.confidence >= 0.0 && result.confidence <= 1.0);
    assert!(!result.values.is_empty());
    assert!(result.timestamp.is_some());
    
    Ok(())
}

#[traced_test]
#[tokio::test]
async fn test_ensemble_performance_reset() -> Result<()> {
    let predictor = create_test_neural_predictor(None).await?;
    
    // Test ensemble performance reset (required by DAA coordinator)
    predictor.reset_ensemble_performance().await?;
    
    // Verify predictor still functional after reset
    let timestamp = chrono::Utc::now();
    let data = create_test_time_series_data("AAPL", timestamp);
    let _result = predictor.predict(&data).await?;
    
    Ok(())
}

#[traced_test]
#[tokio::test]
async fn test_predictor_training_integration() -> Result<()> {
    let predictor = create_test_neural_predictor(None).await?;
    let timestamp = chrono::Utc::now();
    let data = create_test_time_series_data("AAPL", timestamp);
    
    // Test training with new data (required by autonomous training)
    predictor.update_model(&data, 102.5).await?;
    
    // Verify predictor can still make predictions after training
    let result = predictor.predict(&data).await?;
    assert!(!result.values.is_empty());
    
    Ok(())
}

#[traced_test]
#[tokio::test]
async fn test_predictor_memory_efficiency() -> Result<()> {
    let memory_tracker = MemoryTracker::new(256); // 256MB budget
    
    // Create multiple predictors to test memory usage
    let mut predictors = Vec::new();
    for i in 0..10 {
        let config = NeuralConfig {
            model_path: format!("test_model_{}", i),
            ..Default::default()
        };
        let predictor = create_test_neural_predictor(Some(config)).await?;
        predictors.push(predictor);
        
        // Check memory after each predictor creation
        assert!(memory_tracker.check_budget_compliance().await?);
    }
    
    // Test predictions with all predictors
    let timestamp = chrono::Utc::now();
    let data = create_test_time_series_data("AAPL", timestamp);
    
    for predictor in &predictors {
        let _result = predictor.predict(&data).await?;
    }
    
    // Final memory check
    assert!(memory_tracker.check_budget_compliance().await?);
    let usage = memory_tracker.get_memory_usage_mb().await;
    println!("Final memory usage: {}MB", usage);
    
    Ok(())
}

#[traced_test]
#[tokio::test]
async fn test_predictor_concurrent_operations() -> Result<()> {
    let predictor = Arc::new(create_test_neural_predictor(None).await?);
    let timestamp = chrono::Utc::now();
    
    // Test concurrent predictions
    let mut handles = Vec::new();
    for i in 0..5 {
        let predictor_clone = Arc::clone(&predictor);
        let data = create_test_time_series_data(&format!("SYMBOL{}", i), timestamp);
        
        let handle = tokio::spawn(async move {
            predictor_clone.predict(&data).await
        });
        handles.push(handle);
    }
    
    // Wait for all predictions to complete
    for handle in handles {
        let result = handle.await??;
        assert!(!result.values.is_empty());
    }
    
    Ok(())
}

#[traced_test]
#[tokio::test]
async fn test_predictor_error_handling() -> Result<()> {
    let predictor = create_test_neural_predictor(None).await?;
    
    // Test with malformed data
    let mut data = create_test_time_series_data("INVALID", chrono::Utc::now());
    data.values.clear(); // Empty values should be handled gracefully
    
    // Predictor should handle invalid data without crashing
    let result = predictor.predict(&data).await;
    
    // Either succeeds with default values or returns appropriate error
    match result {
        Ok(pred) => {
            // If it succeeds, should have sensible defaults
            assert!(pred.confidence >= 0.0 && pred.confidence <= 1.0);
        },
        Err(_) => {
            // Error is acceptable for invalid data
            println!("Predictor correctly rejected invalid data");
        }
    }
    
    Ok(())
}

#[traced_test]
#[tokio::test]
async fn test_predictor_performance_benchmarks() -> Result<()> {
    let predictor = create_test_neural_predictor(None).await?;
    let timestamp = chrono::Utc::now();
    let data = create_test_time_series_data("AAPL", timestamp);
    
    // Benchmark prediction latency
    let start = std::time::Instant::now();
    for _ in 0..100 {
        let _result = predictor.predict(&data).await?;
    }
    let duration = start.elapsed();
    
    // Predictions should complete within reasonable time
    let avg_latency_ms = duration.as_millis() / 100;
    println!("Average prediction latency: {}ms", avg_latency_ms);
    
    // Should be under 100ms per prediction for good performance
    assert!(avg_latency_ms < 100, "Prediction latency too high: {}ms", avg_latency_ms);
    
    Ok(())
}