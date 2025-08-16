# Real-Time ML Training Enhancement - Usage Examples

## Overview

This document provides practical examples of how to use the real-time ML training enhancements that extend the existing VendorPredictor and AutonomousTrainingEngine systems.

## Basic Setup

### 1. Creating an Integrated Training System

```rust
use neural_trader::daa::{TrainingSystemFactory, CoordinationConfig};
use neural_trader::neural::{VendorPredictor, realtime_training::RealtimeTrainingConfig};
use neural_trader::config::NeuralConfig;
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create VendorPredictor (existing system)
    let neural_config = NeuralConfig::default();
    let sector_mapper = Arc::new(SectorMapper::new(Default::default())?);
    let performance_tracker = Arc::new(ModelPerformanceTracker::new());
    
    let vendor_predictor = Arc::new(RwLock::new(VendorPredictor::new(
        &neural_config,
        sector_mapper,
        performance_tracker,
    )?));
    
    // Create integrated system with real-time capabilities
    let (scheduler, realtime_extension) = 
        TrainingSystemFactory::create_integrated_system(vendor_predictor).await?;
    
    // Start the coordination system
    scheduler.start_coordination().await?;
    
    println!("✅ Real-time training system started!");
    Ok(())
}
```

### 2. Custom Configuration

```rust
use neural_trader::daa::{TrainingTriggerConfig, CoordinationConfig};
use neural_trader::neural::realtime_training::RealtimeTrainingConfig;

async fn create_custom_system() -> Result<(), Box<dyn std::error::Error>> {
    // Configure batch training thresholds (existing system)
    let training_config = TrainingTriggerConfig {
        accuracy_threshold: 0.85,      // Higher accuracy requirement
        error_rate_threshold: 0.05,    // Lower error tolerance  
        min_predictions_for_evaluation: 50,
    };
    
    // Configure real-time training
    let realtime_config = RealtimeTrainingConfig {
        enable_realtime_updates: true,
        max_update_frequency_per_sec: 5,    // Conservative update rate
        min_learning_rate: 0.0001,
        max_learning_rate: 0.005,           // Conservative learning rate
        emergency_accuracy_threshold: 0.6,  // Emergency intervention threshold
        latency_target_ms: 50,              // <50ms requirement
        safety_check_enabled: true,
        coordination_required: true,
    };
    
    // Configure coordination between batch and real-time training
    let coordination_config = CoordinationConfig {
        allow_concurrent_updates: false,    // Conservative: no concurrent updates
        batch_training_cooldown_minutes: 30,
        max_realtime_updates_before_batch: 100,
        emergency_batch_threshold: 0.5,
    };
    
    // Create system with custom configurations
    let vendor_predictor = create_vendor_predictor().await?;
    let (scheduler, realtime_extension) = TrainingSystemFactory::create_custom_system(
        vendor_predictor,
        training_config,
        realtime_config,
        coordination_config,
    ).await?;
    
    Ok(())
}
```

## Real-Time Feedback Processing

### 3. Processing Trading Outcomes

```rust
use neural_trader::neural::{PredictionResult, realtime_training::ModelFeedback};
use chrono::Utc;

async fn process_trading_outcomes(
    scheduler: &DAATrainingScheduler,
) -> Result<(), Box<dyn std::error::Error>> {
    // Simulate a trading scenario
    let prediction = PredictionResult {
        value: 150.0,
        confidence: 0.85,
        model_name: "vendor_model_lstm".to_string(),
        interval_low: 145.0,
        interval_high: 155.0,
        timestamp: Utc::now(),
        metadata: None,
    };
    
    // Execute trade based on prediction
    // ... trading logic ...
    
    // Market outcome occurs
    let actual_price = 152.0;
    
    // Process the outcome to improve model
    scheduler.process_trading_outcome("AAPL", &prediction, Some(actual_price)).await?;
    
    println!("📊 Processed trading outcome - model will be updated in real-time");
    Ok(())
}
```

### 4. Manual Feedback Injection

```rust
use neural_trader::neural::realtime_training::{ModelFeedback, FeedbackType};

async fn inject_custom_feedback(
    realtime_extension: &RealtimeTrainingExtension,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create custom feedback based on external analysis
    let feedback = ModelFeedback {
        symbol: "TSLA".to_string(),
        model_id: "vendor_model_transformer".to_string(),
        accuracy: 0.72,  // Below 0.8 threshold - will trigger update
        prediction_error: 0.08,
        confidence: 0.75,
        timestamp: Utc::now(),
        feedback_type: FeedbackType::Performance,
        actual_value: Some(245.0),
        predicted_value: 240.0,
    };
    
    // Send feedback for processing
    realtime_extension.send_feedback(feedback).await?;
    
    println!("🔄 Custom feedback sent for real-time processing");
    Ok(())
}
```

## Monitoring and Metrics

### 5. Real-Time Performance Monitoring

```rust
async fn monitor_training_performance(
    scheduler: &DAATrainingScheduler,
    realtime_extension: &RealtimeTrainingExtension,
) -> Result<(), Box<dyn std::error::Error>> {
    // Get comprehensive training status
    let status = scheduler.get_training_status().await;
    println!("🎯 Training Status: {:#?}", status);
    
    // Get real-time specific metrics
    let metrics = realtime_extension.get_metrics().await;
    println!("📊 Real-time Metrics:");
    println!("  - Total Updates: {}", metrics.update_count);
    println!("  - Avg Latency: {:.2}ms", metrics.avg_update_latency_ms);
    println!("  - Accuracy Improvements: {}", metrics.accuracy_improvements);
    println!("  - Accuracy Degradations: {}", metrics.accuracy_degradations);
    
    // Get detailed statistics
    let stats = realtime_extension.get_update_statistics().await;
    let success_rate = stats.get("success_rate").unwrap().as_f64().unwrap_or(0.0);
    let latency_efficiency = stats.get("latency_efficiency").unwrap().as_f64().unwrap_or(0.0);
    
    println!("📈 Performance Metrics:");
    println!("  - Success Rate: {:.1}%", success_rate * 100.0);
    println!("  - Latency Efficiency: {:.1}%", latency_efficiency * 100.0);
    
    Ok(())
}
```

### 6. Setting Up Monitoring Dashboard

```rust
use tokio::time::{interval, Duration};

async fn start_monitoring_dashboard(
    scheduler: Arc<DAATrainingScheduler>,
    realtime_extension: Arc<RealtimeTrainingExtension>,
) {
    let mut monitor_interval = interval(Duration::from_secs(30)); // Monitor every 30 seconds
    
    tokio::spawn(async move {
        loop {
            monitor_interval.tick().await;
            
            // Check system health
            let status = scheduler.get_training_status().await;
            let metrics = realtime_extension.get_metrics().await;
            
            // Alert on performance issues
            if metrics.avg_update_latency_ms > 50.0 {
                println!("⚠️  Alert: Update latency exceeded 50ms target: {:.2}ms", 
                        metrics.avg_update_latency_ms);
            }
            
            if metrics.safety_violations > 0 {
                println!("🚨 Alert: {} safety violations detected", metrics.safety_violations);
            }
            
            // Check if batch training should be triggered
            let batch_active = status.get("batch_training_active").unwrap().as_bool().unwrap_or(false);
            if !batch_active && metrics.accuracy_degradations > metrics.accuracy_improvements {
                println!("💡 Suggestion: Consider triggering batch retraining");
            }
            
            // Success metrics
            if metrics.update_count > 0 {
                let success_rate = metrics.accuracy_improvements as f64 / 
                    (metrics.accuracy_improvements + metrics.accuracy_degradations) as f64;
                println!("📊 Current success rate: {:.1}%", success_rate * 100.0);
            }
        }
    });
}
```

## Advanced Usage

### 7. VendorPredictor Real-Time Extensions

```rust
use neural_trader::neural::realtime_training::VendorPredictorRealtimeExt;

async fn advanced_prediction_with_realtime_adjustments(
    vendor_predictor: &mut VendorPredictor,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create test data
    let time_series_data = create_test_data("AAPL").await?;
    
    // Make prediction using existing system
    let mut predictions = vendor_predictor.predict(
        &[time_series_data], 
        24, // 24-hour horizon
        None
    ).await?;
    
    // Simulate recent performance data
    let recent_performance = PerformanceSnapshot {
        timestamp: Utc::now(),
        accuracy: 0.95, // High recent accuracy
        latency_ms: 45,
        error_rate: 0.02,
        recent_predictions: 100,
        confidence: 0.9,
        price_error: 0.01,
        sharpe_ratio: 1.8,
        max_drawdown: 0.02,
        volatility: 0.015,
        model_agreement: 0.95,
        consecutive_failures: 0,
        trading_volume: 2500000.0,
        profit_loss: 320.0,
    };
    
    // Adjust prediction confidence based on recent performance
    for prediction in &mut predictions {
        vendor_predictor.adjust_prediction_confidence(prediction, &recent_performance).await?;
    }
    
    println!("🎯 Predictions adjusted based on real-time performance");
    println!("   Original confidence: 0.85, Adjusted: {:.3}", predictions[0].confidence);
    
    Ok(())
}
```

### 8. Emergency Training Triggers

```rust
async fn handle_emergency_situations(
    scheduler: &DAATrainingScheduler,
    realtime_extension: &RealtimeTrainingExtension,
) -> Result<(), Box<dyn std::error::Error>> {
    // Simulate critical performance degradation
    let emergency_feedback = ModelFeedback {
        symbol: "SPY".to_string(),
        model_id: "market_index_model".to_string(),
        accuracy: 0.45, // Critical accuracy level
        prediction_error: 0.25,
        confidence: 0.3,
        timestamp: Utc::now(),
        feedback_type: FeedbackType::Emergency,
        actual_value: Some(420.0),
        predicted_value: 400.0,
    };
    
    // Send emergency feedback
    realtime_extension.send_feedback(emergency_feedback).await?;
    
    println!("🚨 Emergency feedback sent - system will trigger immediate updates");
    
    // Allow emergency processing
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Check if emergency measures were taken
    let metrics = realtime_extension.get_metrics().await;
    if metrics.update_count > 0 {
        println!("✅ Emergency update processed in {:.2}ms average", 
                metrics.avg_update_latency_ms);
    }
    
    // Manually trigger batch retraining if needed
    if metrics.accuracy_degradations > 5 {
        println!("🔄 Triggering emergency batch retraining");
        scheduler.force_batch_training().await?;
    }
    
    Ok(())
}
```

### 9. Coordination with Existing Systems

```rust
async fn coordinate_with_existing_systems(
    scheduler: &DAATrainingScheduler,
) -> Result<(), Box<dyn std::error::Error>> {
    // Check if real-time updates are currently allowed
    let updates_allowed = scheduler.are_realtime_updates_allowed().await;
    
    if updates_allowed {
        println!("✅ Real-time updates are currently allowed");
        
        // Process routine market data
        process_market_data(scheduler).await?;
        
    } else {
        println!("⏸️  Real-time updates paused - batch training active");
        
        // Wait for batch training to complete
        wait_for_batch_completion(scheduler).await?;
    }
    
    Ok(())
}

async fn wait_for_batch_completion(
    scheduler: &DAATrainingScheduler,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut check_interval = interval(Duration::from_secs(5));
    
    loop {
        check_interval.tick().await;
        
        let status = scheduler.get_training_status().await;
        let batch_active = status.get("batch_training_active").unwrap().as_bool().unwrap_or(false);
        
        if !batch_active {
            println!("✅ Batch training completed - real-time updates resumed");
            break;
        }
        
        println!("⏳ Waiting for batch training completion...");
    }
    
    Ok(())
}
```

## Performance Optimization

### 10. Latency Optimization

```rust
async fn optimize_for_low_latency() -> Result<(), Box<dyn std::error::Error>> {
    // Configure for minimal latency
    let realtime_config = RealtimeTrainingConfig {
        enable_realtime_updates: true,
        max_update_frequency_per_sec: 20,    // Higher frequency
        min_learning_rate: 0.0005,          // Slightly higher minimum
        max_learning_rate: 0.008,           // Slightly higher maximum
        emergency_accuracy_threshold: 0.65, // Less aggressive threshold
        latency_target_ms: 25,              // Aggressive 25ms target
        safety_check_enabled: true,
        coordination_required: false,        // Skip coordination for speed
    };
    
    // Configure for minimal coordination overhead
    let coordination_config = CoordinationConfig {
        allow_concurrent_updates: true,      // Allow concurrent updates
        batch_training_cooldown_minutes: 60, // Longer cooldown
        max_realtime_updates_before_batch: 200, // More updates before batch
        emergency_batch_threshold: 0.4,     // Lower emergency threshold
    };
    
    println!("⚡ Configured for low-latency operation");
    println!("   Target latency: 25ms");
    println!("   Max update frequency: 20/sec");
    println!("   Coordination: Minimal");
    
    Ok(())
}
```

## Error Handling and Recovery

### 11. Robust Error Handling

```rust
async fn robust_error_handling(
    scheduler: &DAATrainingScheduler,
    realtime_extension: &RealtimeTrainingExtension,
) -> Result<(), Box<dyn std::error::Error>> {
    // Handle potential errors gracefully
    match realtime_extension.process_queued_updates().await {
        Ok(_) => {
            println!("✅ Queued updates processed successfully");
        }
        Err(e) => {
            println!("⚠️  Error processing updates: {}", e);
            
            // Reset metrics and try again
            scheduler.reset_realtime_metrics().await?;
            println!("🔄 Reset metrics and retrying...");
            
            // Retry with conservative settings
            match realtime_extension.process_queued_updates().await {
                Ok(_) => println!("✅ Recovery successful"),
                Err(e) => {
                    println!("🚨 Recovery failed: {}", e);
                    // Escalate to batch training
                    scheduler.force_batch_training().await?;
                }
            }
        }
    }
    
    Ok(())
}
```

## Integration Checklist

### 12. Complete Integration Example

```rust
#[tokio::main]
async fn complete_integration_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Starting complete real-time training integration");
    
    // 1. Create system components
    let vendor_predictor = create_vendor_predictor().await?;
    let (scheduler, realtime_extension) = 
        TrainingSystemFactory::create_integrated_system(vendor_predictor).await?;
    
    // 2. Start coordination
    scheduler.start_coordination().await?;
    
    // 3. Set up monitoring
    let scheduler_clone = scheduler.clone();
    let extension_clone = realtime_extension.clone();
    start_monitoring_dashboard(scheduler_clone, extension_clone).await;
    
    // 4. Process some trading outcomes
    for i in 0..5 {
        let prediction = create_test_prediction(i).await?;
        let actual_outcome = simulate_market_outcome(&prediction).await?;
        
        scheduler.process_trading_outcome(
            &format!("TEST{}", i), 
            &prediction, 
            Some(actual_outcome)
        ).await?;
        
        // Allow processing time
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    
    // 5. Check final performance
    let final_metrics = realtime_extension.get_metrics().await;
    println!("📊 Final Performance:");
    println!("   Updates: {}", final_metrics.update_count);
    println!("   Avg Latency: {:.2}ms", final_metrics.avg_update_latency_ms);
    println!("   Success Rate: {:.1}%", 
        if final_metrics.update_count > 0 {
            final_metrics.accuracy_improvements as f64 / final_metrics.update_count as f64 * 100.0
        } else { 0.0 }
    );
    
    println!("✅ Real-time training integration complete!");
    Ok(())
}
```

## Key Benefits

### Performance Improvements
- **<50ms parameter updates**: Real-time model adjustments
- **Preserved batch training**: Existing retraining capabilities maintained
- **Safety bounds**: Existing thresholds used as safety mechanisms
- **Coordination**: Seamless integration with existing DAA system

### Integration Advantages
- **Zero breaking changes**: All existing APIs preserved
- **Gradual adoption**: Can be enabled/disabled via configuration
- **Comprehensive monitoring**: Detailed metrics and performance tracking
- **Error recovery**: Robust fallback to batch training when needed

### Use Cases
1. **High-frequency trading**: Sub-50ms model updates for rapid market changes
2. **Risk management**: Real-time accuracy monitoring and adjustment
3. **Performance optimization**: Continuous model improvement without downtime
4. **Emergency response**: Automatic model correction during market volatility

This real-time training enhancement extends the existing neural trading system with minimal risk and maximum benefit, preserving all current functionality while adding powerful new capabilities.