# Phase 2: Central Routing Architecture Design (Updated Target State)

## Overview
This document defines the simplified central routing architecture where EnhancedNeuralAdapter serves as the primary implementation, providing all production features while maintaining a single path to ruv-FANN neural networks.

## Architecture Goals
1. **Single Implementation**: EnhancedNeuralAdapter handles all neural predictions
2. **Production Features**: Health monitoring, fallbacks, performance tracking built-in
3. **Performance Visibility**: Every prediction emits performance events
4. **Training Integration**: Direct notification channel to training systems
5. **All Models Supported**: MLP, LSTM, GRU, Transformer, etc. all route to ruv-FANN

## Core Architecture Components

### 1. Simplified Public API
```rust
impl NeuralPredictor {
    enhanced_adapter: Arc<EnhancedNeuralAdapter>, // Single implementation
}

impl NeuralPredictor {
    /// Create predictor with enhanced features
    pub fn new(config: NeuralConfig) -> Result<Self> {
        let enhanced_adapter = Arc::new(
            EnhancedNeuralAdapter::new(config)?
        );
        Ok(Self { enhanced_adapter })
    }
    
    /// Single prediction entry point
    pub async fn predict(
        &self,
        data: &[TimeSeriesData],
        horizon: usize,
        features: Option<HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<PredictionResult>> {
        self.enhanced_adapter
            .predict_enhanced(data, horizon, features)
            .await
    }
}
```

### 2. Enhanced Neural Adapter Features
```rust
impl EnhancedNeuralAdapter {
    /// Production-ready prediction with all features
    pub async fn predict_enhanced(
        &self,
        data: &[TimeSeriesData],
        horizon: usize,
        features: Option<HashMap<String, serde_json::Value>>,
    ) -> Result<Vec<PredictionResult>> {
        // 1. Health check
        self.health_monitor.check_system_health().await?;
        
        // 2. Circuit breaker check
        if self.circuit_breaker.is_open() {
            return self.fallback_manager.execute_fallback(data).await;
        }
        
        // 3. Performance tracking start
        let start_time = Instant::now();
        
        // 4. Execute prediction through FANN
        let result = self.fann_predictor
            .predict(data, horizon, features)
            .await?;
        
        // 5. Emit performance event
        self.emit_performance_event(&result, start_time.elapsed()).await?;
        
        // 6. Notify training system if needed
        self.notify_training_system(&result).await?;
        
        Ok(result)
    }
}
```

### 3. FANN Routing (Internal)
```rust
impl EnhancedNeuralAdapter {
    /// All models route through FANN
    async fn predict_with_fann_model(
        &self,
        model_type: &str,
        data: &[TimeSeriesData],
        horizon: usize,
    ) -> Result<Vec<PredictionResult>> {
        // ALL model types use ruv-FANN networks
        match model_type {
            "MLP" => self.fann_predictor.predict(data, horizon, None).await,
            "LSTM" => self.fann_predictor.predict(data, horizon, None).await,
            "GRU" => self.fann_predictor.predict(data, horizon, None).await,
            "Transformer" => self.fann_predictor.predict(data, horizon, None).await,
            "DeepAR" => self.fann_predictor.predict(data, horizon, None).await,
            "TCN" => self.fann_predictor.predict(data, horizon, None).await,
            _ => self.fann_predictor.predict(data, horizon, None).await,
        }
        // Note: Model-specific behavior is simulated within FANN networks
    }
}
```

### 4. Performance Channel Integration
```rust
impl EnhancedNeuralAdapter {
    /// Emit performance events with training notifications
    async fn emit_performance_event(
        &self,
        results: &[PredictionResult],
        duration: Duration,
    ) -> Result<()> {
        let event = PerformanceEvent {
            timestamp: Utc::now(),
            source: PerformanceSource::EnhancedAdapter,
            event_type: PerformanceEventType::PredictionCompleted {
                model: self.current_model.clone(),
                accuracy: self.calculate_accuracy(results),
                confidence: self.calculate_confidence(results),
                latency_ms: duration.as_millis() as u64,
            },
            metrics: self.build_performance_metrics(results, duration),
        };
        
        // Emit to performance channel
        self.performance_channel.emit(event.clone()).await?;
        
        // Check if training notification needed
        if self.should_notify_training(&event) {
            self.training_notification_channel.send(
                TrainingNotification {
                    trigger: TrainingTrigger::LowAccuracy,
                    metrics: event.metrics,
                    timestamp: Utc::now(),
                }
            ).await?;
        }
        
        Ok(())
    }
}
```

### 5. Training Notification System
```rust
impl EnhancedNeuralAdapter {
    /// Determine if training system should be notified
    fn should_notify_training(&self, event: &PerformanceEvent) -> bool {
        match &event.event_type {
            PerformanceEventType::PredictionCompleted { accuracy, confidence, .. } => {
                // Notify if accuracy drops below threshold
                *accuracy < self.config.accuracy_threshold ||
                // Or confidence is too low
                *confidence < self.config.confidence_threshold ||
                // Or too many recent errors
                self.recent_error_rate() > self.config.error_threshold
            }
            _ => false,
        }
    }
    
    /// Send notification to training system
    async fn notify_training_system(&self, result: &[PredictionResult]) -> Result<()> {
        if let Some(ref channel) = self.training_notification_channel {
            let notification = self.build_training_notification(result);
            channel.send(notification).await?;
        }
        Ok(())
    }
}
```

### 6. Health Monitoring & Fallbacks
```rust
impl EnhancedNeuralAdapter {
    /// Built-in health monitoring
    async fn monitor_health(&self) -> HealthStatus {
        HealthStatus {
            fann_predictor: self.fann_predictor.health_check().await,
            circuit_breaker: self.circuit_breaker.status(),
            recent_errors: self.error_tracker.recent_count(),
            performance_metrics: self.performance_aggregator.snapshot(),
        }
    }
    
    /// Fallback strategy when primary prediction fails
    async fn execute_fallback(&self, data: &[TimeSeriesData]) -> Result<Vec<PredictionResult>> {
        // Try different strategies in order
        for strategy in &self.fallback_strategies {
            match strategy.execute(data).await {
                Ok(result) => {
                    self.emit_fallback_event(strategy.name()).await;
                    return Ok(result);
                }
                Err(_) => continue,
            }
        }
        
        // Ultimate fallback: simple moving average
        self.simple_moving_average_fallback(data).await
    }
}
```

## Module Structure (Simplified)

### 1. Module Exports (src/neural/mod.rs)
```rust
// Public exports - single predictor interface
pub use self::neural_predictor::NeuralPredictor;
pub use self::traits::{NeuralPredictorTrait, PredictionResult};
pub use self::performance_channel::{PerformanceChannel, PerformanceEvent};

// Enhanced adapter is internal - not exported
// Access only through NeuralPredictor
```

### 2. Internal Structure
```
neural/
├── mod.rs                    # Public API
├── neural_predictor.rs       # Thin wrapper
├── enhanced_adapter/         # Main implementation (internal)
│   ├── mod.rs
│   ├── health_monitor.rs
│   ├── circuit_breaker.rs
│   ├── fallback_manager.rs
│   └── performance_tracker.rs
└── fann_predictor.rs         # FANN integration (internal)
```

## Data Flow (Simplified)

```
Client Request
    ↓
NeuralPredictor (public API)
    ↓
EnhancedNeuralAdapter (all features)
    ├→ Health Check
    ├→ Circuit Breaker
    ├→ Performance Start
    ↓
FannPredictor (internal)
    ↓
ruv-FANN Network Execution
    ↓
Performance Event Emission
    ├→ Performance Channel
    └→ Training Notification (if needed)
    ↓
Return Results
```

## Benefits of This Architecture

### 1. Simplicity
- Single implementation path
- No confusing feature flags
- Clear data flow

### 2. Production Features
- Health monitoring built-in
- Automatic fallbacks
- Performance tracking
- Training integration

### 3. Flexibility
- All models supported through FANN
- Easy to add new features
- Centralized configuration

### 4. Performance
- Minimal routing overhead
- Efficient caching
- Async throughout

## Migration Steps

### Phase 1: Update NeuralPredictor
1. Change from FannPredictor to EnhancedNeuralAdapter
2. Remove redundant routing logic
3. Update tests

### Phase 2: Remove Redundancy
1. Remove duplicate MLP handling
2. Consolidate model routing
3. Clean up unused code

### Phase 3: Enhance Features
1. Improve training notifications
2. Add more sophisticated fallbacks
3. Enhanced performance analytics

## Success Criteria

1. ✅ All predictions flow through EnhancedNeuralAdapter
2. ✅ All models (including MLP) supported
3. ✅ Performance events for every prediction
4. ✅ Training notifications working
5. ✅ Health monitoring active
6. ✅ Fallback strategies functional
7. ✅ No duplicate code paths
8. ✅ All tests passing

## Architecture Decision Records

### ADR-001: Enhanced as Primary
- **Decision**: Use EnhancedNeuralAdapter as the single implementation
- **Rationale**: Already has all production features, proven in use
- **Consequences**: Need to ensure all models supported

### ADR-002: Remove Feature Flags
- **Decision**: Remove use_real_models flag
- **Rationale**: All paths lead to ruv-FANN anyway
- **Consequences**: Simpler configuration

### ADR-003: Training Integration
- **Decision**: Direct notification channel from Enhanced to training
- **Rationale**: Immediate feedback for model performance
- **Consequences**: Tighter coupling but better responsiveness