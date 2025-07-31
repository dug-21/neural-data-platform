# Technical Debt Cleanup Phase 1 - Architecture

## System Architecture Overview

### Current Architecture (Problematic)

```
┌─────────────────────┐     ┌──────────────────────┐
│   Market Data       │────▶│  EnhancedNeuralAdapter│
└─────────────────────┘     └───────┬──────────────┘
                                    │
                            ┌───────┴───────┐
                            ▼               ▼
                    ┌──────────────┐ ┌────────────────┐
                    │ FannPredictor│ │NeuroDivergent  │ ❌ Bypass
                    └──────┬───────┘ │   Adapter      │
                           │         └────────────────┘
                           ▼
                    ┌──────────────┐
                    │  ruv-fann    │
                    └──────────────┘

❌ Problems:
- Multiple paths to predictions
- Mock adapters bypass ruv-fann
- No centralized routing enforcement
```

### Target Architecture (Fixed)

```
┌─────────────────────┐     ┌──────────────────────┐
│   Market Data       │────▶│  EnhancedNeuralAdapter│
└─────────────────────┘     └───────┬──────────────┘
                                    │
                                    ▼ (ONLY PATH)
                            ┌──────────────┐
                            │ FannPredictor│ ✅ Central Router
                            └──────┬───────┘
                                   │
                                   ▼
                            ┌──────────────┐
                            │  ruv-fann    │
                            └──────────────┘

✅ Benefits:
- Single path enforcement
- No bypass possible
- Compile-time guarantees
```

## Component Architecture

### 1. FannPredictor - Central Neural Router

```rust
// src/neural/fann_predictor.rs
pub struct FannPredictor {
    // Core FANN networks
    networks: HashMap<String, Arc<Network>>,
    
    // Model registry
    model_registry: ModelRegistry,
    
    // Performance monitoring
    metrics: Arc<Mutex<ModelMetrics>>,
    
    // Training state
    training_state: Arc<RwLock<TrainingState>>,
    
    // Configuration
    config: FannConfig,
}

impl FannPredictor {
    /// Central execution point - ALL models go through here
    pub async fn execute_model(
        &self,
        model_type: ModelType,
        data: &[TimeSeriesData],
        config: ModelConfig,
    ) -> Result<Vec<PredictionResult>> {
        // Centralized routing logic
        let network = self.get_or_create_network(model_type, &config)?;
        let predictions = self.execute_fann_network(network, data)?;
        self.emit_performance_metrics(&predictions)?;
        Ok(predictions)
    }
    
    /// Private - prevents external network access
    fn get_or_create_network(
        &self,
        model_type: ModelType,
        config: &ModelConfig,
    ) -> Result<Arc<Network>> {
        // Network creation/caching logic
    }
}
```

### 2. DAA Coordinator - Autonomous Orchestration

```rust
// src/integration/daa_coordinator.rs
pub struct DaaCoordinator {
    // REQUIRED - No longer Optional
    autonomous_training: Arc<AutonomousTrainingEngine>,
    training_scheduler: Arc<DAATrainingScheduler>,
    
    // Market awareness
    market_hours: Arc<MarketHours>,
    market_analyzer: Arc<MarketAnalyzer>,
    
    // Performance bridge
    performance_bridge: Arc<PerformanceTrainingBridge>,
    
    // Decision engine
    decision_engine: DecisionEngine,
    
    // Event channels
    training_tx: mpsc::Sender<TrainingEvent>,
    performance_rx: mpsc::Receiver<PerformanceEvent>,
}

/// Autonomous decision types
pub enum AutonomousAction {
    InitiateTraining {
        priority: TrainingPriority,
        reason: String,
        constraints: MarketConstraints,
    },
    ContinueTrading {
        confidence: f64,
        next_evaluation: Duration,
    },
    EmergencyTraining {
        severity: EmergencySeverity,
        immediate: bool,
    },
    MarketPause {
        duration: Duration,
        reason: String,
    },
}
```

### 3. Performance Training Bridge

```rust
// src/integration/performance_training_bridge.rs
pub struct PerformanceTrainingBridge {
    // Data converters
    metric_converter: MetricConverter,
    snapshot_builder: SnapshotBuilder,
    
    // Channels
    performance_rx: mpsc::Receiver<PerformanceStats>,
    training_tx: mpsc::Sender<TrainingDecision>,
    
    // State
    performance_history: RingBuffer<PerformanceSnapshot>,
    training_thresholds: TrainingThresholds,
    
    // Market awareness
    market_hours: Arc<MarketHours>,
}

impl PerformanceTrainingBridge {
    /// Converts incompatible performance metrics to training format
    pub fn convert_metrics(&self, stats: PerformanceStats) -> PerformanceSnapshot {
        PerformanceSnapshot {
            accuracy: stats.success_rate,
            confidence: self.calculate_confidence(&stats),
            price_error: self.estimate_price_error(&stats),
            sharpe_ratio: self.calculate_sharpe(),
            max_drawdown: self.calculate_drawdown(),
            volatility: self.calculate_volatility(),
            model_agreement: self.calculate_agreement(&stats),
            consecutive_failures: stats.consecutive_failures,
            trading_volume: stats.volume_24h,
            profit_loss: stats.pnl,
        }
    }
}
```

## Data Flow Architecture

### 1. Prediction Flow

```
Market Event
    │
    ▼
TimeSeriesConverter
    │
    ▼
EnhancedNeuralAdapter
    │
    ▼ (ONLY PATH)
FannPredictor.execute_model()
    │
    ├─▶ Network Selection
    ├─▶ FANN Execution
    ├─▶ Result Formatting
    └─▶ Performance Emission
         │
         ▼
    PerformanceChannel
         │
         ▼
    DaaCoordinator
```

### 2. Training Flow

```
PerformanceChannel
    │
    ▼
PerformanceTrainingBridge
    │
    ├─▶ Metric Conversion
    ├─▶ Threshold Evaluation
    └─▶ Market Timing Check
         │
         ▼
    DaaCoordinator
         │
         ├─▶ Decision Engine
         ├─▶ Market Analysis
         └─▶ Training Submission
              │
              ▼
         TrainingScheduler
              │
              ▼
         AutonomousTrainingEngine
              │
              ▼
         FannPredictor.update_model()
```

## Module Structure

### Before (Multiple Entry Points)
```
src/
├── adapters/
│   ├── mod.rs (exports both adapters) ❌
│   ├── enhanced_neural_adapter.rs
│   └── neuro_divergent.rs (MOCK) ❌
├── neural/
│   ├── mod.rs
│   ├── fann_predictor.rs
│   └── mlp_adapter.rs
└── integration/
    └── daa_coordinator.rs
```

### After (Single Entry Point)
```
src/
├── adapters/
│   ├── mod.rs (ONLY exports enhanced) ✅
│   └── enhanced_neural_adapter.rs
├── neural/
│   ├── mod.rs (ONLY exports fann_predictor) ✅
│   ├── fann_predictor.rs (CENTRAL ROUTER) ✅
│   ├── performance_channel.rs (NEW) ✅
│   └── performance_events.rs (NEW) ✅
└── integration/
    ├── daa_coordinator.rs (UPDATED) ✅
    └── performance_training_bridge.rs (NEW) ✅
```

## Interface Design

### 1. Neural Prediction Interface

```rust
// Single public interface for ALL predictions
pub trait NeuralPredictor: Send + Sync {
    async fn predict(
        &self,
        data: &[TimeSeriesData],
        horizon: usize,
        features: &[String],
    ) -> Result<Vec<PredictionResult>>;
    
    async fn update_model(
        &self,
        model_name: &str,
        weights: ModelWeights,
    ) -> Result<()>;
}

// ONLY FannPredictor implements this
impl NeuralPredictor for FannPredictor {
    // Implementation
}
```

### 2. DAA Orchestration Interface

```rust
pub trait AutonomousOrchestrator: Send + Sync {
    async fn orchestrate_operations(&self) -> Result<AutonomousAction>;
    
    async fn evaluate_training_need(
        &self,
        snapshot: PerformanceSnapshot,
    ) -> Result<TrainingDecision>;
    
    async fn submit_training_job(
        &self,
        job: DAATrainingJob,
    ) -> Result<JobId>;
}
```

### 3. Performance Bridge Interface

```rust
pub trait PerformanceBridge: Send + Sync {
    fn convert_metrics(&self, stats: PerformanceStats) -> PerformanceSnapshot;
    
    async fn evaluate_performance(&self) -> Result<PerformanceEvaluation>;
    
    fn should_trigger_training(
        &self,
        snapshot: &PerformanceSnapshot,
        window: TrainingWindow,
    ) -> bool;
}
```

## Event Architecture

### 1. Performance Events

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PerformanceEvent {
    PredictionCompleted {
        model: String,
        accuracy: f64,
        latency_ms: u64,
        timestamp: DateTime<Utc>,
    },
    ModelDegraded {
        model: String,
        current_accuracy: f64,
        baseline_accuracy: f64,
        degradation_percent: f64,
    },
    ThresholdBreached {
        metric: String,
        current_value: f64,
        threshold: f64,
        severity: Severity,
    },
}
```

### 2. Training Events

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TrainingEvent {
    TrainingRequested {
        job_id: Uuid,
        priority: TrainingPriority,
        reason: String,
        market_window: TrainingWindow,
    },
    TrainingStarted {
        job_id: Uuid,
        models: Vec<String>,
        estimated_duration: Duration,
    },
    TrainingCompleted {
        job_id: Uuid,
        improved_models: Vec<String>,
        performance_gain: f64,
    },
}
```

## Error Handling Architecture

### 1. Layered Error Types

```rust
#[derive(Debug, thiserror::Error)]
pub enum NeuralError {
    #[error("Model routing error: {0}")]
    RoutingError(String),
    
    #[error("FANN execution error: {0}")]
    FannError(#[from] FannError),
    
    #[error("Training error: {0}")]
    TrainingError(String),
    
    #[error("Performance bridge error: {0}")]
    BridgeError(String),
}

#[derive(Debug, thiserror::Error)]
pub enum DaaError {
    #[error("Orchestration error: {0}")]
    OrchestrationError(String),
    
    #[error("Market timing error: {0}")]
    MarketTimingError(String),
    
    #[error("Decision engine error: {0}")]
    DecisionError(String),
}
```

### 2. Fallback Strategy

```rust
pub struct FallbackManager {
    fallback_models: Vec<Box<dyn FallbackPredictor>>,
    cache: PredictionCache,
    error_threshold: ErrorThreshold,
}

impl FallbackManager {
    pub async fn handle_prediction_failure(
        &self,
        error: NeuralError,
        context: PredictionContext,
    ) -> Result<Vec<PredictionResult>> {
        match error {
            NeuralError::FannError(_) => {
                // Try simpler FANN model
                self.try_simple_model(context).await
            }
            NeuralError::RoutingError(_) => {
                // Use cached predictions
                self.get_cached_predictions(context).await
            }
            _ => {
                // Use statistical baseline
                self.statistical_fallback(context).await
            }
        }
    }
}
```

## Security Architecture

### 1. Access Control

```rust
// Compile-time access control
mod neural {
    mod fann_predictor {
        pub struct FannPredictor { /* ... */ }
        
        // Public API
        pub async fn predict() -> Result<_> { /* ... */ }
        
        // Private internals
        fn create_network() -> Network { /* ... */ }
    }
    
    // ONLY export what's needed
    pub use fann_predictor::FannPredictor;
    // DO NOT export adapters
}
```

### 2. Resource Limits

```rust
pub struct ResourceLimits {
    max_memory_mb: usize,
    max_cpu_percent: f32,
    max_concurrent_models: usize,
    training_timeout: Duration,
}

impl DaaCoordinator {
    fn check_resource_limits(&self) -> Result<()> {
        let current = self.get_resource_usage()?;
        if current.exceeds(&self.limits) {
            return Err(DaaError::ResourceExceeded);
        }
        Ok(())
    }
}
```

## Monitoring Architecture

### 1. Metrics Collection

```rust
pub struct MetricsCollector {
    // Prometheus metrics
    prediction_counter: Counter,
    prediction_latency: Histogram,
    model_accuracy: Gauge,
    training_duration: Histogram,
    
    // Custom metrics
    routing_decisions: Counter,
    fallback_usage: Counter,
    performance_events: Counter,
}
```

### 2. Observability

```rust
#[instrument(skip(self, data))]
pub async fn execute_model(
    &self,
    model_type: ModelType,
    data: &[TimeSeriesData],
) -> Result<Vec<PredictionResult>> {
    let span = info_span!("neural_prediction", 
        model = %model_type,
        data_points = data.len()
    );
    
    async move {
        // Execution with tracing
    }
    .instrument(span)
    .await
}
```

## Deployment Architecture

### 1. Configuration

```yaml
# config/neural.yaml
neural:
  routing:
    enforce_central: true
    allow_direct_adapter: false
    
  models:
    cache_size: 100
    max_concurrent: 10
    
  training:
    auto_orchestration: true
    market_aware: true
    min_accuracy: 0.6
    
  performance:
    collection_interval: 60s
    evaluation_window: 5m
    trigger_threshold: 0.1
```

### 2. Migration Strategy

```rust
// Gradual migration with feature flags
pub struct MigrationConfig {
    enforce_routing: bool,
    block_mock_adapters: bool,
    enable_daa_orchestration: bool,
    enable_performance_bridge: bool,
}

impl MigrationConfig {
    pub fn phase_1() -> Self {
        Self {
            enforce_routing: false,
            block_mock_adapters: true,
            enable_daa_orchestration: false,
            enable_performance_bridge: false,
        }
    }
    
    pub fn phase_2() -> Self {
        Self {
            enforce_routing: true,
            block_mock_adapters: true,
            enable_daa_orchestration: true,
            enable_performance_bridge: true,
        }
    }
}
```

## Testing Architecture

### 1. Unit Test Structure

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_routing_enforcement() {
        let predictor = FannPredictor::new_test();
        
        // Should route through FANN
        let result = predictor.execute_model(
            ModelType::DeepAR,
            &test_data(),
            config()
        ).await;
        
        assert!(result.is_ok());
        assert!(predictor.metrics.lock().unwrap()
            .routing_decisions["fann"] > 0);
    }
}
```

### 2. Integration Test Structure

```rust
#[tokio::test]
async fn test_complete_flow() {
    // Setup
    let system = TestSystem::new().await;
    
    // Market event → Prediction
    let event = MarketEvent::new_test();
    system.process_event(event).await;
    
    // Verify routing
    assert_eq!(system.routing_paths(), vec!["fann_predictor"]);
    
    // Verify performance emission
    assert!(system.performance_events() > 0);
    
    // Verify DAA orchestration
    assert!(system.daa_decisions() > 0);
}
```

## Performance Considerations

### 1. Caching Strategy

```rust
pub struct ModelCache {
    networks: Arc<DashMap<CacheKey, Arc<Network>>>,
    max_size: usize,
    eviction_policy: EvictionPolicy,
}

impl ModelCache {
    pub fn get_or_create<F>(
        &self,
        key: CacheKey,
        create_fn: F,
    ) -> Arc<Network>
    where
        F: FnOnce() -> Network,
    {
        self.networks
            .entry(key)
            .or_insert_with(|| Arc::new(create_fn()))
            .clone()
    }
}
```

### 2. Parallelization

```rust
pub async fn parallel_predictions(
    &self,
    requests: Vec<PredictionRequest>,
) -> Vec<Result<PredictionResult>> {
    let tasks: Vec<_> = requests
        .into_iter()
        .map(|req| {
            let predictor = self.clone();
            tokio::spawn(async move {
                predictor.execute_model(
                    req.model_type,
                    &req.data,
                    req.config
                ).await
            })
        })
        .collect();
    
    futures::future::join_all(tasks)
        .await
        .into_iter()
        .map(|r| r.unwrap_or_else(|e| Err(e.into())))
        .collect()
}
```

## Summary

This architecture ensures:
1. **All neural predictions route through ruv-fann**
2. **DAA orchestrates training decisions autonomously**
3. **Performance metrics connect to training decisions**
4. **Mock adapters are completely removed**
5. **System is maintainable, testable, and observable**

The key architectural changes:
- Centralized routing through FannPredictor
- Required (not Optional) DAA components
- New PerformanceTrainingBridge component
- Event-driven performance feedback
- Compile-time routing enforcement