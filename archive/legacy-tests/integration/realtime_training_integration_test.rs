//! Real-Time Training Integration Tests
//!
//! Tests the integration between real-time training extensions and existing
//! VendorPredictor and AutonomousTrainingEngine systems.

use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::test;

// Import system components
use autonomous_platform::daa::{
    AutonomousTrainingEngine, 
    TrainingTriggerConfig,
    CoordinationConfig,
    TrainingSystemFactory,
};
use autonomous_platform::neural::{
    VendorPredictor, 
    PredictionResult,
    realtime_training::{
        RealtimeTrainingExtension, 
        RealtimeTrainingConfig, 
        ModelFeedback, 
        FeedbackType,
        VendorPredictorRealtimeExt,
    },
};
use autonomous_platform::data::{TimeSeriesData, sector_mapper::SectorMapper};
use autonomous_platform::monitoring::model_performance_tracker::ModelPerformanceTracker;
use autonomous_platform::config::NeuralConfig;

#[tokio::test]
async fn test_realtime_training_extension_creation() -> Result<()> {
    // Create mock components
    let neural_config = NeuralConfig::default();
    let sector_mapper = Arc::new(SectorMapper::new(Default::default())?);
    let performance_tracker = Arc::new(ModelPerformanceTracker::new());
    
    // Create VendorPredictor
    let vendor_predictor = Arc::new(RwLock::new(VendorPredictor::new(
        &neural_config,
        sector_mapper,
        performance_tracker,
    )?));
    
    // Create training components
    let training_config = TrainingTriggerConfig::default();
    let autonomous_engine = Arc::new(RwLock::new(
        AutonomousTrainingEngine::new(training_config.clone())?
    ));
    
    // Create real-time training extension
    let realtime_config = RealtimeTrainingConfig::default();
    let realtime_extension = RealtimeTrainingExtension::new(
        vendor_predictor,
        autonomous_engine,
        realtime_config,
        training_config,
    );
    
    // Verify extension was created successfully
    let metrics = realtime_extension.get_metrics().await;
    assert_eq!(metrics.update_count, 0);
    assert_eq!(metrics.accuracy_improvements, 0);
    assert_eq!(metrics.accuracy_degradations, 0);
    
    Ok(())
}

#[tokio::test]
async fn test_feedback_processing_pipeline() -> Result<()> {
    // Create integrated system using factory
    let neural_config = NeuralConfig::default();
    let sector_mapper = Arc::new(SectorMapper::new(Default::default())?);
    let performance_tracker = Arc::new(ModelPerformanceTracker::new());
    
    let vendor_predictor = Arc::new(RwLock::new(VendorPredictor::new(
        &neural_config,
        sector_mapper,
        performance_tracker,
    )?));
    
    let (scheduler, realtime_extension) = 
        TrainingSystemFactory::create_integrated_system(vendor_predictor).await?;
    
    // Start the processing
    realtime_extension.start_processing().await?;
    
    // Create test feedback
    let feedback = ModelFeedback {
        symbol: "AAPL".to_string(),
        model_id: "test_model".to_string(),
        accuracy: 0.7, // Below threshold to trigger update
        prediction_error: 0.05,
        confidence: 0.8,
        timestamp: Utc::now(),
        feedback_type: FeedbackType::Performance,
        actual_value: Some(102.0),
        predicted_value: 100.0,
    };
    
    // Send feedback
    realtime_extension.send_feedback(feedback).await?;
    
    // Allow some processing time
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Check metrics were updated
    let metrics = realtime_extension.get_metrics().await;
    assert!(metrics.update_count > 0 || metrics.accuracy_degradations > 0);
    
    Ok(())
}

#[tokio::test]
async fn test_parameter_update_latency() -> Result<()> {
    // Create test feedback that should trigger immediate update
    let feedback = ModelFeedback {
        symbol: "AAPL".to_string(),
        model_id: "test_model".to_string(),
        accuracy: 0.5, // Critical threshold
        prediction_error: 0.15,
        confidence: 0.6,
        timestamp: Utc::now(),
        feedback_type: FeedbackType::Emergency,
        actual_value: Some(105.0),
        predicted_value: 100.0,
    };
    
    let start_time = std::time::Instant::now();
    
    // Create parameter update (simulates real processing)
    let config = RealtimeTrainingConfig::default();
    let update = RealtimeTrainingExtension::create_parameter_update(
        &feedback,
        autonomous_platform::neural::realtime_training::UpdateUrgency::Critical,
        &config,
    )?;
    
    let latency = start_time.elapsed().as_millis();
    
    // Verify latency is under target
    assert!(latency < 50, "Parameter update creation took {}ms (target: <50ms)", latency);
    
    // Verify update parameters
    assert_eq!(update.model_id, "test_model");
    assert!(update.learning_rate >= config.min_learning_rate);
    assert!(update.learning_rate <= config.max_learning_rate);
    assert!(!update.parameters.is_empty());
    
    Ok(())
}

#[tokio::test]
async fn test_safety_bounds_integration() -> Result<()> {
    // Create system with conservative safety bounds
    let mut training_config = TrainingTriggerConfig::default();
    training_config.accuracy_threshold = 0.9; // High threshold
    training_config.error_rate_threshold = 0.05; // Low error tolerance
    
    let realtime_config = RealtimeTrainingConfig {
        min_learning_rate: 0.0001,
        max_learning_rate: 0.005, // Conservative max
        emergency_accuracy_threshold: 0.7,
        ..Default::default()
    };
    
    // Test feedback that should respect safety bounds
    let feedback = ModelFeedback {
        symbol: "AAPL".to_string(),
        model_id: "test_model".to_string(),
        accuracy: 0.85, // Above emergency but below accuracy threshold
        prediction_error: 0.03,
        confidence: 0.8,
        timestamp: Utc::now(),
        feedback_type: FeedbackType::Performance,
        actual_value: Some(101.5),
        predicted_value: 100.0,
    };
    
    // Create update and verify it respects bounds
    let update = RealtimeTrainingExtension::create_parameter_update(
        &feedback,
        autonomous_platform::neural::realtime_training::UpdateUrgency::Medium,
        &realtime_config,
    )?;
    
    // Verify learning rate is within conservative bounds
    assert!(update.learning_rate >= realtime_config.min_learning_rate);
    assert!(update.learning_rate <= realtime_config.max_learning_rate);
    
    // Test safety check function
    let safety_result = RealtimeTrainingExtension::apply_safety_checks(&update, &training_config);
    assert!(safety_result.is_ok(), "Safety checks should pass for valid update");
    
    Ok(())
}

#[tokio::test]
async fn test_batch_training_coordination() -> Result<()> {
    // Create system with coordination enabled
    let coordination_config = CoordinationConfig {
        allow_concurrent_updates: false, // Strict coordination
        max_realtime_updates_before_batch: 5, // Low threshold for testing
        emergency_batch_threshold: 0.8,
        ..Default::default()
    };
    
    let neural_config = NeuralConfig::default();
    let sector_mapper = Arc::new(SectorMapper::new(Default::default())?);
    let performance_tracker = Arc::new(ModelPerformanceTracker::new());
    
    let vendor_predictor = Arc::new(RwLock::new(VendorPredictor::new(
        &neural_config,
        sector_mapper,
        performance_tracker,
    )?));
    
    let (scheduler, realtime_extension) = TrainingSystemFactory::create_custom_system(
        vendor_predictor,
        TrainingTriggerConfig::default(),
        RealtimeTrainingConfig::default(),
        coordination_config,
    ).await?;
    
    // Start coordination
    scheduler.start_coordination().await?;
    
    // Verify initial state
    let status = scheduler.get_training_status().await;
    assert_eq!(status.get("batch_training_active").unwrap(), &serde_json::json!(false));
    
    // Check that real-time updates are allowed initially
    assert!(scheduler.are_realtime_updates_allowed().await);
    
    Ok(())
}

#[tokio::test]
async fn test_prediction_outcome_feedback_loop() -> Result<()> {
    // Create integrated system
    let neural_config = NeuralConfig::default();
    let sector_mapper = Arc::new(SectorMapper::new(Default::default())?);
    let performance_tracker = Arc::new(ModelPerformanceTracker::new());
    
    let vendor_predictor = Arc::new(RwLock::new(VendorPredictor::new(
        &neural_config,
        sector_mapper,
        performance_tracker,
    )?));
    
    let (scheduler, _realtime_extension) = 
        TrainingSystemFactory::create_integrated_system(vendor_predictor).await?;
    
    // Create test prediction
    let prediction = PredictionResult {
        value: 150.0,
        confidence: 0.85,
        model_name: "test_model".to_string(),
        interval_low: 145.0,
        interval_high: 155.0,
        timestamp: Utc::now(),
        metadata: None,
    };
    
    // Simulate trading outcome
    let actual_outcome = 152.0; // Close to prediction (good accuracy)
    
    // Process the outcome
    scheduler.process_trading_outcome("AAPL", &prediction, Some(actual_outcome)).await?;
    
    // Allow processing time
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    
    // Verify status was updated
    let status = scheduler.get_training_status().await;
    let performance_summary = status.get("performance_summary").unwrap();
    
    // Should have performance data
    assert!(performance_summary.get("history_length").unwrap().as_u64().unwrap() > 0);
    
    Ok(())
}

#[tokio::test]
async fn test_vendor_predictor_realtime_extensions() -> Result<()> {
    // Create VendorPredictor
    let neural_config = NeuralConfig::default();
    let sector_mapper = Arc::new(SectorMapper::new(Default::default())?);
    let performance_tracker = Arc::new(ModelPerformanceTracker::new());
    
    let mut vendor_predictor = VendorPredictor::new(
        &neural_config,
        sector_mapper,
        performance_tracker,
    )?;
    
    // Test real-time parameter update
    let feedback = ModelFeedback {
        symbol: "AAPL".to_string(),
        model_id: "test_model".to_string(),
        accuracy: 0.75, // Below threshold
        prediction_error: 0.08,
        confidence: 0.7,
        timestamp: Utc::now(),
        feedback_type: FeedbackType::Performance,
        actual_value: Some(103.0),
        predicted_value: 100.0,
    };
    
    let start_time = std::time::Instant::now();
    
    // Apply real-time update
    vendor_predictor.update_parameters_realtime(&feedback).await?;
    
    let latency = start_time.elapsed().as_millis();
    assert!(latency < 100, "Real-time update took {}ms (should be <100ms)", latency);
    
    // Test confidence adjustment
    let mut prediction = PredictionResult {
        value: 100.0,
        confidence: 0.8,
        model_name: "test_model".to_string(),
        interval_low: 95.0,
        interval_high: 105.0,
        timestamp: Utc::now(),
        metadata: None,
    };
    
    let performance_snapshot = autonomous_platform::daa::PerformanceSnapshot {
        timestamp: Utc::now(),
        accuracy: 0.95, // High accuracy should boost confidence
        latency_ms: 50,
        error_rate: 0.02,
        recent_predictions: 100,
        confidence: 0.9,
        price_error: 0.02,
        sharpe_ratio: 1.5,
        max_drawdown: 0.03,
        volatility: 0.02,
        model_agreement: 0.95,
        consecutive_failures: 0,
        trading_volume: vec![2000000.0],
        profit_loss: 150.0,
    };
    
    let original_confidence = prediction.confidence;
    vendor_predictor.adjust_prediction_confidence(&mut prediction, &performance_snapshot).await?;
    
    // Confidence should be boosted for high performance
    assert!(prediction.confidence >= original_confidence, 
            "Confidence should be boosted for high performance");
    
    Ok(())
}

#[tokio::test]
async fn test_update_statistics_tracking() -> Result<()> {
    // Create system components
    let neural_config = NeuralConfig::default();
    let sector_mapper = Arc::new(SectorMapper::new(Default::default())?);
    let performance_tracker = Arc::new(ModelPerformanceTracker::new());
    
    let vendor_predictor = Arc::new(RwLock::new(VendorPredictor::new(
        &neural_config,
        sector_mapper,
        performance_tracker,
    )?));
    
    let (_scheduler, realtime_extension) = 
        TrainingSystemFactory::create_integrated_system(vendor_predictor).await?;
    
    // Start processing
    realtime_extension.start_processing().await?;
    
    // Send multiple feedback samples
    for i in 0..3 {
        let feedback = ModelFeedback {
            symbol: format!("TEST{}", i),
            model_id: "test_model".to_string(),
            accuracy: 0.8 + (i as f64 * 0.05), // Varying accuracy
            prediction_error: 0.05 - (i as f64 * 0.01),
            confidence: 0.8,
            timestamp: Utc::now(),
            feedback_type: FeedbackType::Routine,
            actual_value: Some(100.0 + i as f64),
            predicted_value: 100.0,
        };
        
        realtime_extension.send_feedback(feedback).await?;
    }
    
    // Allow processing time
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    
    // Get update statistics
    let stats = realtime_extension.get_update_statistics().await;
    
    // Verify statistics are tracked
    assert!(stats.contains_key("total_updates"));
    assert!(stats.contains_key("avg_latency_ms"));
    assert!(stats.contains_key("success_rate"));
    assert!(stats.contains_key("latency_efficiency"));
    
    // Check that some metrics have been recorded
    let total_updates = stats.get("total_updates").unwrap().as_u64().unwrap_or(0);
    assert!(total_updates > 0, "Should have recorded some updates");
    
    Ok(())
}

#[test]
fn test_feedback_type_classification() {
    // Test feedback creation with different accuracy levels
    let base_prediction = PredictionResult {
        value: 100.0,
        confidence: 0.8,
        model_name: "test_model".to_string(),
        interval_low: 95.0,
        interval_high: 105.0,
        timestamp: Utc::now(),
        metadata: None,
    };
    
    // Test emergency feedback (low accuracy)
    let emergency_feedback = RealtimeTrainingExtension::create_feedback(
        "AAPL",
        &base_prediction,
        Some(110.0), // Large error
    ).unwrap();
    
    match emergency_feedback.feedback_type {
        FeedbackType::Emergency => {}, // Expected
        _ => panic!("Should be emergency feedback for large error"),
    }
    
    // Test routine feedback (good accuracy)
    let routine_feedback = RealtimeTrainingExtension::create_feedback(
        "AAPL",
        &base_prediction,
        Some(101.0), // Small error
    ).unwrap();
    
    match routine_feedback.feedback_type {
        FeedbackType::Routine => {}, // Expected for good accuracy
        FeedbackType::Performance => {}, // Also acceptable
        _ => panic!("Should be routine or performance feedback for small error"),
    }
}