# RUV-FANN Neuro-Divergent Integration Plan

## Executive Summary

This document outlines the integration plan for replacing the current fake FANN configurations with actual neural model implementations from the vendor/ruv-fann/neuro-divergent library. The current system simulates advanced models like NHITS, TCN, and DeepAR using basic FANN networks, but the vendor library provides real implementations of these models.

## Current State Analysis

### Fake Implementation Issues

1. **FannPredictor.rs** (lines 156-233):
   - Creates model configs that pretend to be NHITS, TCN, DeepAR, etc.
   - Actually just uses basic FANN networks with different layer sizes
   - No real implementation of model-specific algorithms

2. **Simulated Features**:
   - Attention mechanism simulation (lines 491-584)
   - Recurrent state management (lines 375-489)
   - Basic FANN networks masquerading as advanced models

### Available Real Implementations

The vendor library provides actual implementations:

1. **neuro-divergent-models/src/**:
   - `specialized/deepar.rs` - Real DeepAR with probabilistic forecasting
   - `specialized/tcn.rs` - Real TCN with dilated convolutions
   - `advanced/nhits.rs` - Real NHITS with hierarchical interpolation
   - `recurrent/rnn.rs` - Base for LSTM/GRU (type aliases currently)
   - `transformer/` - Transformer models (not fully implemented)

2. **Registry and Factory**:
   - `neuro-divergent-registry/` - Model discovery and management
   - Factory pattern for dynamic model creation
   - Plugin system for extensibility

## Integration Architecture

### Phase 1: Adapter Pattern Implementation

Create adapters to bridge FannPredictor interface with neuro-divergent models:

```rust
// src/adapters/neural/neuro_divergent_adapter.rs

use neuro_divergent_models::{
    specialized::{DeepAR, TCN, BaseModel},
    advanced::nhits::NHITS,
    data::TimeSeriesData as NDTimeSeriesData,
};

pub struct NeuroDivergentAdapter {
    models: HashMap<String, Box<dyn BaseModel<f32>>>,
    registry: ModelRegistry,
    factory: ModelFactory,
}

impl NeuroDivergentAdapter {
    pub async fn create_model(&mut self, name: &str) -> Result<()> {
        match name {
            "DeepAR" => {
                let config = DeepARConfig::default();
                let model = DeepAR::new(config)?;
                self.models.insert(name.to_string(), Box::new(model));
            }
            "TCN" => {
                let config = TCNConfig::default();
                let model = TCN::new(config)?;
                self.models.insert(name.to_string(), Box::new(model));
            }
            "NHITS" => {
                let config = NHITSConfig::default();
                let model = NHITS::new(config)?;
                self.models.insert(name.to_string(), Box::new(model));
            }
            _ => return Err(anyhow!("Unknown model: {}", name)),
        }
        Ok(())
    }
}
```

### Phase 2: Data Format Translation

Create converters between our TimeSeriesData and vendor formats:

```rust
// src/adapters/neural/data_converter.rs

impl From<&crate::data::TimeSeriesData> for NDTimeSeriesData<f32> {
    fn from(data: &crate::data::TimeSeriesData) -> Self {
        NDTimeSeriesData::new(
            data.symbol.clone(),
            vec![data.timestamp],
            vec![data.close as f32],
        )
        .with_static_features(vec![
            data.volume as f32,
            data.indicators.get("rsi").copied().unwrap_or(50.0) as f32,
        ])
    }
}
```

### Phase 3: Replace FannPredictor Implementation

Update FannPredictor to use real models:

```rust
// src/neural/fann_predictor.rs

pub struct FannPredictor {
    config: NeuralConfig,
    // Keep FANN networks for backward compatibility
    fann_networks: Arc<RwLock<HashMap<String, Network<f32>>>>,
    // Add real model implementations
    neuro_divergent: Arc<RwLock<NeuroDivergentAdapter>>,
    use_real_models: bool,
}

impl FannPredictor {
    async fn predict_with_model(
        &self,
        model_name: &str,
        data: &[TimeSeriesData],
        horizon: usize,
    ) -> Result<Vec<PredictionResult>> {
        if self.use_real_models && is_supported_model(model_name) {
            // Use real implementation
            let adapter = self.neuro_divergent.read().await;
            adapter.predict(model_name, data, horizon).await
        } else {
            // Fall back to FANN simulation
            self.predict_with_fann(model_name, data, horizon).await
        }
    }
}
```

### Phase 4: Feature Parity Implementation

Implement missing features in vendor models:

1. **Ensemble Management**:
   - Keep existing EnsembleManager
   - Use it to coordinate real models
   - Leverage diversity metrics for better predictions

2. **Online Learning**:
   - Implement incremental training for vendor models
   - Use vendor's training APIs where available
   - Fall back to batch retraining if needed

3. **Performance Tracking**:
   - Keep existing ModelPerformance tracking
   - Feed metrics back to ensemble weighting
   - Use for adaptive model selection

## Implementation Steps

### Step 1: Create Adapter Infrastructure (Week 1)
- [ ] Create `src/adapters/neural/` module
- [ ] Implement `NeuroDivergentAdapter` trait
- [ ] Create data format converters
- [ ] Add error handling and logging

### Step 2: Integrate Vendor Models (Week 2)
- [ ] Add vendor dependencies to Cargo.toml
- [ ] Implement model creation for DeepAR, TCN, NHITS
- [ ] Create configuration mappings
- [ ] Add unit tests for each model

### Step 3: Update FannPredictor (Week 3)
- [ ] Add `use_real_models` flag
- [ ] Implement model selection logic
- [ ] Update prediction methods
- [ ] Maintain backward compatibility

### Step 4: Testing and Validation (Week 4)
- [ ] Compare predictions: FANN vs real models
- [ ] Performance benchmarking
- [ ] Memory usage analysis
- [ ] Integration testing

### Step 5: Migration and Deployment (Week 5)
- [ ] Feature flag for gradual rollout
- [ ] A/B testing framework
- [ ] Performance monitoring
- [ ] Rollback procedures

## Technical Considerations

### 1. Type System Compatibility
- Vendor uses `Float` trait, we use `f64`
- Need type conversion layer
- Consider performance implications

### 2. Async/Sync Bridge
- Vendor models are synchronous
- Our system is async
- Use `tokio::task::spawn_blocking` for CPU-intensive operations

### 3. Memory Management
- Vendor models may have different memory patterns
- Monitor heap usage during integration
- Consider model pooling for efficiency

### 4. Error Handling
- Map vendor `ModelError` to our error types
- Preserve error context
- Add telemetry for debugging

## Risk Mitigation

### 1. Performance Regression
- **Risk**: Real models might be slower than FANN
- **Mitigation**: Benchmark before deployment, use caching

### 2. Accuracy Changes
- **Risk**: Different predictions might break downstream systems
- **Mitigation**: Gradual rollout, A/B testing

### 3. Memory Bloat
- **Risk**: Multiple model implementations increase memory
- **Mitigation**: Lazy loading, model lifecycle management

### 4. API Breaking Changes
- **Risk**: Vendor library updates might break integration
- **Mitigation**: Pin versions, abstract behind interfaces

## Success Metrics

1. **Prediction Accuracy**: >10% improvement over FANN baseline
2. **Latency**: <100ms for single predictions
3. **Memory Usage**: <2x current usage
4. **Model Diversity**: Ensemble predictions 20% better than single models
5. **Stability**: <0.1% prediction failures

## Timeline

- **Week 1-2**: Infrastructure and basic integration
- **Week 3-4**: Testing and optimization
- **Week 5**: Deployment preparation
- **Week 6+**: Monitoring and iteration

## Conclusion

This integration will provide real neural network implementations instead of simulated ones, improving prediction accuracy and model diversity. The phased approach ensures backward compatibility while enabling gradual migration to better models.