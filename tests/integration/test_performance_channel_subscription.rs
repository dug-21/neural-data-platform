//! Test for performance channel subscription in DaaCoordinator

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use chrono::Utc;

use autonomous_platform::{
    config::NeuralConfig,
    integration::daa_coordinator::{DaaCoordinator, DaaConfig},
    neural::{NeuralPredictor, PerformanceChannel, PerformanceEventBuilder, PerformanceEventType, PerformanceSource, EventPriority},
    utils::market_hours::MarketHours,
};

#[tokio::test]
async fn test_daa_coordinator_performance_channel_subscription() -> Result<()> {
    // Create neural predictor
    let neural_config = NeuralConfig {
        memory_gb: 1.0,
        models: vec!["MLP".to_string()],
        prediction_cache_ttl: 300,
        model_load_timeout: 60,
        max_concurrent_predictions: 10,
        enable_model_monitoring: true,
        accuracy_threshold: 0.8,
        use_real_models: false,
        enable_health_checks: true,
        enable_fallback: true,
        enable_circuit_breakers: true,
        enable_graceful_degradation: false,
        enable_performance_monitoring: true,
        enable_adaptive_retry: true,
        enable_model_ensembles: false,
        model_timeout_seconds: 60,
        max_retries: 3,
        error_threshold: 0.05,
        lookback_window: 24,
    };
    
    let neural_predictor = Arc::new(NeuralPredictor::new(neural_config)?);
    let (tx, _rx) = mpsc::channel(100);
    let market_hours = Arc::new(MarketHours::new(None));
    
    // Create DaaCoordinator
    let mut coordinator = DaaCoordinator::new(
        DaaConfig::default(),
        neural_predictor,
        tx,
        market_hours,
    )?;
    
    // Create performance channel
    let (channel, _) = PerformanceChannel::new_with_buffer(100);
    
    // Subscribe coordinator to channel
    coordinator.subscribe_to_performance_channel(channel.clone());
    
    // Emit test event
    let event = PerformanceEventBuilder::new()
        .source(PerformanceSource::NeuralPredictor {
            model_name: "test_model".to_string(),
            predictor_id: "test_pred".to_string(),
        })
        .event_type(PerformanceEventType::PredictionCompleted {
            model: "test_model".to_string(),
            accuracy: 0.75, // Below threshold to trigger evaluation
            confidence: 0.85,
            latency_ms: 100,
            input_features: 10,
            output_dimension: 1,
            timestamp: Utc::now(),
        })
        .priority(EventPriority::High)
        .build()?;
    
    // Emit event
    channel.emit(event).await?;
    
    // Give time for processing
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Check that event was processed (metrics updated)
    let metrics = coordinator.get_metrics().await;
    assert!(metrics.model_accuracy.contains_key("test_model"));
    assert_eq!(metrics.model_accuracy.get("test_model"), Some(&0.75));
    
    Ok(())
}

#[tokio::test]
async fn test_performance_degradation_event() -> Result<()> {
    // Create neural predictor
    let neural_config = NeuralConfig::default();
    let neural_predictor = Arc::new(NeuralPredictor::new(neural_config)?);
    let (tx, _rx) = mpsc::channel(100);
    let market_hours = Arc::new(MarketHours::new(None));
    
    // Create DaaCoordinator
    let mut coordinator = DaaCoordinator::new(
        DaaConfig::default(),
        neural_predictor,
        tx,
        market_hours,
    )?;
    
    // Create performance channel
    let (channel, _) = PerformanceChannel::new_with_buffer(100);
    
    // Subscribe coordinator to channel
    coordinator.subscribe_to_performance_channel(channel.clone());
    
    // Emit performance degradation event
    let event = PerformanceEventBuilder::new()
        .source(PerformanceSource::System {
            service_name: "neural_engine".to_string(),
        })
        .event_type(PerformanceEventType::PerformanceDegradation {
            metric_name: "prediction_accuracy".to_string(),
            current_value: 0.65,
            baseline_value: 0.85,
            degradation_percent: 23.5,
            impact_severity: "high".to_string(),
        })
        .priority(EventPriority::Critical)
        .build()?;
    
    // Emit event
    channel.emit(event).await?;
    
    // Give time for processing
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Test passes if no panic occurs (event was processed)
    Ok(())
}

#[tokio::test]
async fn test_event_latency() -> Result<()> {
    // Create performance channel
    let (channel, _) = PerformanceChannel::new_with_buffer(1000);
    
    // Measure emission latency
    let start = std::time::Instant::now();
    
    for i in 0..100 {
        let event = PerformanceEventBuilder::new()
            .source(PerformanceSource::NeuralPredictor {
                model_name: format!("model_{}", i),
                predictor_id: format!("pred_{}", i),
            })
            .event_type(PerformanceEventType::PredictionCompleted {
                model: format!("model_{}", i),
                accuracy: 0.95,
                confidence: 0.9,
                latency_ms: 50 + i,
                input_features: 10,
                output_dimension: 1,
                timestamp: Utc::now(),
            })
            .build()?;
        
        channel.emit_fast(event);
    }
    
    let elapsed = start.elapsed();
    let avg_latency_ms = elapsed.as_millis() as f64 / 100.0;
    
    println!("Average event emission latency: {:.2}ms", avg_latency_ms);
    
    // Verify <1ms latency requirement
    assert!(avg_latency_ms < 1.0, "Event emission latency {} ms exceeds 1ms requirement", avg_latency_ms);
    
    Ok(())
}