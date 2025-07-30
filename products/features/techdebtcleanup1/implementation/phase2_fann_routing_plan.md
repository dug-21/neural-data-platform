# Phase 2: FANN Central Routing Implementation Plan

## FannSpecialist Analysis - Waiting for TDD Tests

### Current State Analysis

1. **Model Types Identified**:
   - MLP (Multi-Layer Perceptron)
   - LSTM (Long Short-Term Memory)
   - GRU (Gated Recurrent Unit)
   - DeepAR (Probabilistic forecasting)
   - TCN (Temporal Convolutional Networks)
   - NHITS (Neural Hierarchical Interpolation)
   - Transformer (Attention mechanism)

2. **Current Structure**:
   - `FannPredictor` implements `NeuralPredictorTrait`
   - Methods: `predict_with_model`, `train_model`, `ensure_model`
   - Network creation happens in `ensure_model`
   - No `execute_model` method exists yet

3. **Required Changes per REFINEMENT.md Step 2.1**:
   - Create `execute_model()` as the ONLY prediction path
   - Make network creation methods private
   - Implement proper FANN network management
   - Add performance event emission
   - Create ModelType, ModelConfig, and ModelKey types

### Implementation Requirements

```rust
// New types needed
pub enum ModelType {
    MLP, LSTM, GRU, DeepAR, TCN, NHITS, Transformer
}

pub struct ModelConfig {
    pub horizon: usize,
    pub input_size: usize,
    pub hidden_layers: Vec<usize>,
    pub activation: ActivationFunction,
    // ... other config fields
}

pub struct ModelKey {
    model_type: ModelType,
    config_hash: u64,
}

// Central routing method
pub async fn execute_model(
    &self,
    model_type: ModelType,
    data: &[TimeSeriesData],
    config: ModelConfig,
) -> Result<Vec<PredictionResult>>
```

### Waiting for TDD Tests

Before implementing, I need tests from the TesterAgent that will:
1. Test execute_model routing for each model type
2. Verify network creation is private
3. Test performance event emission
4. Verify error handling
5. Test caching behavior

### Next Steps

1. **TesterAgent** will provide Phase 2 TDD tests
2. **FannSpecialist** will implement based on failing tests
3. **CentralRouter** will verify integration
4. **PerformanceAgent** will validate metrics

## Coordination Status

- Analysis complete: ✅
- Waiting for tests: ⏳
- Implementation ready to start: 🔄