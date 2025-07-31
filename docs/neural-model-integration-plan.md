# Neural Model Integration Plan

## Executive Summary

This document outlines the integration strategy for incorporating the vendor neural models (ruv-fann neuro-divergent) into the neural-trader system. The plan focuses on creating an adapter pattern that maintains compatibility while enabling advanced features.

## Current Architecture Analysis

### Existing System (FannPredictor)
- **Location**: `/src/neural/fann_predictor.rs`
- **Key Components**:
  - Uses ruv-fann directly for neural networks
  - Implements custom model configurations for different architectures
  - Manages ensemble predictions with dynamic weighting
  - Includes recurrent state management for LSTM/GRU
  - Has market regime detection and adaptive model selection

### Vendor Models (neuro-divergent)
- **Location**: `/vendor/ruv-fann/neuro-divergent/`
- **Key Features**:
  - Comprehensive model registry system
  - 27+ pre-built neural forecasting models
  - Plugin architecture for extensibility
  - Built-in model discovery and factory pattern
  - Advanced models: NBEATS, Transformer, DeepAR, etc.

## Integration Strategy

### 1. Adapter Pattern Implementation

Create a new module `neural_adapter` that bridges between FannPredictor and vendor models:

```rust
// src/neural/adapters/mod.rs
pub mod neuro_divergent_adapter;
pub mod model_registry;
pub mod config_mapper;
```

### 2. Model Registry Integration

```rust
// src/neural/adapters/model_registry.rs
use neuro_divergent_registry::{ModelFactory, ModelRegistry, ModelCategory};

pub struct VendorModelRegistry {
    registry: Arc<RwLock<ModelRegistry>>,
    factory: ModelFactory,
    config_cache: HashMap<String, ModelConfig>,
}

impl VendorModelRegistry {
    pub fn new() -> Result<Self> {
        neuro_divergent_registry::initialize_registry()?;
        Ok(Self {
            registry: global_registry(),
            factory: ModelFactory::new(),
            config_cache: HashMap::new(),
        })
    }
    
    pub fn create_model(&self, name: &str) -> Result<Box<dyn BaseModel<f32>>> {
        self.factory.create(name)
    }
}
```

### 3. Configuration Mapping

Map existing FannModelConfig to vendor ModelConfig:

```rust
// src/neural/adapters/config_mapper.rs
impl ConfigMapper {
    pub fn map_to_vendor(fann_config: &FannModelConfig) -> ModelConfig {
        let mut config = ModelConfig::new(
            fann_config.model_name,
            self.map_category(&fann_config.model_type)
        );
        
        config
            .set_parameter("learning_rate", json!(fann_config.learning_rate))
            .set_parameter("epochs", json!(fann_config.max_epochs))
            .set_parameter("hidden_layers", json!(fann_config.hidden_layers))
            .set_dimensions(Some(fann_config.input_size), Some(fann_config.output_size));
            
        config
    }
}
```

### 4. Main Adapter Implementation

```rust
// src/neural/adapters/neuro_divergent_adapter.rs
pub struct NeuroDivergentAdapter {
    vendor_registry: VendorModelRegistry,
    models: HashMap<String, Arc<Mutex<Box<dyn BaseModel<f32>>>>>,
    performance_tracker: PerformanceTracker,
}

impl NeuroDivergentAdapter {
    pub async fn create_model(&self, model_type: &str) -> Result<()> {
        let vendor_model = match model_type {
            "NHITS" => self.vendor_registry.create_model("NHITS")?,
            "DeepAR" => self.vendor_registry.create_model("DeepAR")?,
            "Transformer" => self.vendor_registry.create_model("TFT")?,
            "LSTM" => self.vendor_registry.create_model("LSTM")?,
            _ => return Err(anyhow!("Unsupported model type")),
        };
        
        self.models.insert(model_type.to_string(), Arc::new(Mutex::new(vendor_model)));
        Ok(())
    }
    
    pub async fn predict(&self, model_name: &str, input: &[f32]) -> Result<Vec<f32>> {
        let model = self.models.get(model_name)
            .ok_or_else(|| anyhow!("Model not found"))?;
            
        let model_lock = model.lock().await;
        Ok(model_lock.forward(input)?)
    }
}
```

## Refactoring Plan for FannPredictor

### Phase 1: Add Adapter Support
1. Add vendor model detection in `ensure_model()`
2. Create adapter initialization in constructor
3. Add routing logic for vendor vs custom models

### Phase 2: Modify Prediction Pipeline
```rust
impl FannPredictor {
    async fn predict_with_model(&self, model_name: &str, data: &[TimeSeriesData], horizon: usize) -> Result<Vec<PredictionResult>> {
        // Check if vendor model
        if self.is_vendor_model(model_name) {
            return self.vendor_adapter.predict_timeseries(model_name, data, horizon).await;
        }
        
        // Existing FANN logic
        self.predict_with_fann_model(model_name, data, horizon).await
    }
}
```

### Phase 3: Update Training Logic
```rust
async fn train_model(&self, model_name: &str, data: &[TimeSeriesData]) -> Result<()> {
    if self.is_vendor_model(model_name) {
        // Use vendor training API
        let training_data = self.prepare_vendor_training_data(data)?;
        return self.vendor_adapter.train(model_name, training_data).await;
    }
    
    // Existing training logic
    self.train_fann_model(model_name, data).await
}
```

## Model Loading Mechanisms

### 1. Lazy Loading Strategy
```rust
pub struct ModelLoader {
    loaded_models: Arc<RwLock<HashMap<String, ModelHandle>>>,
    loading_queue: Arc<Mutex<VecDeque<String>>>,
    max_memory_gb: f64,
}

impl ModelLoader {
    pub async fn ensure_loaded(&self, model_name: &str) -> Result<()> {
        if !self.is_loaded(model_name).await {
            self.load_model(model_name).await?;
        }
        Ok(())
    }
}
```

### 2. Memory Management
```rust
pub struct MemoryManager {
    current_usage: AtomicU64,
    max_usage: u64,
    eviction_policy: EvictionPolicy,
}

impl MemoryManager {
    pub fn can_load(&self, estimated_size: u64) -> bool {
        self.current_usage.load(Ordering::Relaxed) + estimated_size <= self.max_usage
    }
    
    pub async fn evict_if_needed(&self, required_size: u64) -> Result<()> {
        // Implement LRU eviction
    }
}
```

## Configuration Updates

### 1. Extended Neural Config
```toml
[neural]
memory_gb = 2.0
models = ["MLP", "NHITS", "TCN", "DeepAR", "LSTM", "GRU", "Transformer"]
vendor_models = ["NHITS", "DeepAR", "TFT", "NBEATS", "TSMixer"]
custom_models = ["MLP", "TCN", "LSTM", "GRU", "Transformer"]
model_registry_path = "./models/registry"
enable_plugins = true
plugin_dirs = ["./models/plugins"]
```

### 2. Model Mapping Configuration
```yaml
model_mappings:
  NHITS:
    vendor_name: "NHITS"
    category: "Advanced"
    default_params:
      n_blocks: 3
      mlp_units: [512, 512]
      n_pool_kernel_size: [2, 2, 1]
  
  DeepAR:
    vendor_name: "DeepAR"
    category: "Specialized"
    default_params:
      hidden_size: 40
      rnn_layers: 2
      dropout_rate: 0.1
```

## Testing Strategy

### 1. Unit Tests
- Test adapter creation and initialization
- Test configuration mapping
- Test model loading and caching
- Test prediction routing

### 2. Integration Tests
```rust
#[tokio::test]
async fn test_vendor_model_integration() {
    let predictor = FannPredictor::new(config)?;
    
    // Test vendor model prediction
    let vendor_predictions = predictor.predict_with_model("NHITS", &data, 10).await?;
    assert_eq!(vendor_predictions.len(), 10);
    
    // Test ensemble with mixed models
    let ensemble = predictor.predict_ensemble(
        &data, 
        10, 
        &["MLP", "NHITS", "DeepAR"], // Mix of custom and vendor
        None
    ).await?;
}
```

### 3. Performance Tests
- Benchmark vendor vs custom model performance
- Memory usage comparison
- Prediction latency tests
- Ensemble coordination overhead

## Rollout Plan

### Phase 1: Foundation (Week 1-2)
1. Implement adapter pattern structure
2. Create model registry integration
3. Add configuration mapping
4. Basic unit tests

### Phase 2: Integration (Week 3-4)
1. Modify FannPredictor to support adapters
2. Update prediction pipeline
3. Implement model loading mechanisms
4. Integration testing

### Phase 3: Advanced Features (Week 5-6)
1. Add plugin support
2. Implement memory management
3. Enhanced ensemble coordination
4. Performance optimization

### Phase 4: Production Ready (Week 7-8)
1. Comprehensive testing
2. Performance benchmarking
3. Documentation updates
4. Gradual rollout with feature flags

## Risk Mitigation

### 1. API Compatibility
- **Risk**: Vendor API changes breaking integration
- **Mitigation**: Version pinning, adapter abstraction layer

### 2. Performance Overhead
- **Risk**: Adapter layer adding latency
- **Mitigation**: Caching, lazy loading, optimized routing

### 3. Memory Management
- **Risk**: Multiple models exceeding memory limits
- **Mitigation**: Dynamic loading/unloading, memory quotas

### 4. Model Quality
- **Risk**: Vendor models performing differently than expected
- **Mitigation**: A/B testing, gradual rollout, performance monitoring

## Monitoring and Observability

### 1. Metrics to Track
- Model load times
- Prediction latencies by model type
- Memory usage per model
- Cache hit rates
- Adapter overhead

### 2. Logging Strategy
```rust
info!("Loading vendor model: {} via adapter", model_name);
debug!("Model memory footprint: {} MB", memory_usage);
warn!("Vendor model prediction slower than threshold: {} ms", latency);
```

## Conclusion

This integration plan provides a structured approach to incorporating vendor neural models while maintaining system stability and performance. The adapter pattern ensures loose coupling and allows for gradual migration and testing.

## Next Steps

1. Review and approve integration plan
2. Set up development branch for implementation
3. Begin Phase 1 implementation
4. Schedule regular progress reviews