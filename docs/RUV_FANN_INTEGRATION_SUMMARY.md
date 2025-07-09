# ruv-FANN and neuro-divergent Integration Analysis

## Architecture Overview

The neural-trader project has access to a powerful neural network stack:

```
┌─────────────────────────────────────────────┐
│           neural-trader (Top Level)          │
│  Current: Placeholder implementations only   │
└─────────────────────────────────────────────┘
                      ↓ Should use
┌─────────────────────────────────────────────┐
│     neuro-divergent (Time Series Models)    │
│  27+ production-ready forecasting models    │
│  (NHITS, TCN, DeepAR, LSTM, GRU, etc.)     │
└─────────────────────────────────────────────┘
                      ↓ Built on
┌─────────────────────────────────────────────┐
│      ruv-FANN (Core Neural Networks)        │
│  Low-level FANN implementation in Rust      │
│  (Network, Layer, Neuron, Training, etc.)   │
└─────────────────────────────────────────────┘
```

## Integration Points

### 1. ruv-FANN Core (vendor/ruv-fann/src/lib.rs)

Provides fundamental neural network building blocks:
- `Network<T>`: Core neural network structure
- `NetworkBuilder<T>`: Fluent API for building networks
- `Layer`, `Neuron`, `Connection`: Network components
- `TrainingAlgorithm`: Various training methods (Backprop, RProp, QuickProp)
- `CascadeTrainer`: Dynamic topology optimization
- `ActivationFunction`: ReLU, Sigmoid, Tanh, etc.

### 2. neuro-divergent Models (vendor/ruv-fann/neuro-divergent/)

Builds sophisticated time-series models on top of ruv-FANN:

#### Basic Models
- **MLP**: Multi-layer perceptron using `NetworkBuilder`
- **DLinear**: Decomposition-based linear model
- **NLinear**: Normalized linear forecasting

#### Advanced Models
- **NHITS**: Hierarchical interpolation with multi-rate sampling
- **NBEATS**: Basis expansion with interpretable components
- **NBEATSx**: Extended with exogenous variables

#### Recurrent Models (Using ruv-FANN networks with state)
- **RNN**: Basic recurrent architecture
- **LSTM**: Long Short-Term Memory
- **GRU**: Gated Recurrent Units

#### Specialized Models
- **TCN**: Temporal convolutions (simulated with MLPs)
- **DeepAR**: Probabilistic autoregressive
- **TFT**: Temporal Fusion Transformer

### 3. Key Integration Classes

```rust
// neuro-divergent uses ruv-FANN through adapters
pub trait NetworkAdapter<T: Float> {
    fn prepare_input(&self, ts_input: &TimeSeriesInput<T>) -> Result<Vec<T>>;
    fn process_output(&self, network_output: Vec<T>) -> Result<ForecastOutput<T>>;
    fn create_network(&self, config: &dyn ModelConfig<T>) -> Result<Network<T>>;
}

// Each model implements BaseModel trait
pub trait BaseModel<T: Float> {
    fn fit(&mut self, data: &TimeSeriesDataset<T>) -> Result<TrainingMetrics<T>>;
    fn predict(&self, input: &TimeSeriesInput<T>) -> Result<ForecastOutput<T>>;
}
```

## Current Issues in neural-trader

1. **Not Using Real Models**: The `src/neural/mod.rs` contains only placeholder implementations
2. **Missing Dependencies**: Need to add neuro-divergent to Cargo.toml
3. **No Data Pipeline**: Need adapters between neural-trader and neuro-divergent formats
4. **No Model Persistence**: Models are recreated on each prediction

## Recommended Integration Path

### Step 1: Add Dependencies
```toml
[dependencies]
ruv-fann = { path = "vendor/ruv-fann" }
neuro-divergent-models = { path = "vendor/ruv-fann/neuro-divergent/neuro-divergent-models" }
neuro-divergent-core = { path = "vendor/ruv-fann/neuro-divergent/neuro-divergent-core" }
```

### Step 2: Create Adapter Layer
```rust
// src/neural/adapter.rs
use neuro_divergent_models::{NeuralForecast, BaseModel};
use crate::data::TimeSeriesData;

pub struct NeuroAdapter {
    neural_forecast: NeuralForecast<f64>,
}

impl NeuroAdapter {
    pub fn new(models: Vec<Box<dyn BaseModel<f64>>>) -> Self {
        let nf = NeuralForecast::new()
            .with_models(models)
            .build()
            .expect("Failed to build NeuralForecast");
        Self { neural_forecast: nf }
    }
}
```

### Step 3: Replace Placeholder Models
```rust
// src/neural/models.rs
use neuro_divergent_models::{
    models::{NHITS, TCN, LSTM},
    specialized::DeepAR,
    basic::MLP,
};

pub fn create_nhits(horizon: usize) -> Box<dyn BaseModel<f64>> {
    let config = NHITSConfig::default()
        .with_horizon(horizon)
        .with_sampling_rates(vec![1, 2, 4]);
    Box::new(NHITS::new(config).expect("Failed to create NHITS"))
}
```

## Benefits of Full Integration

1. **Real Neural Networks**: Replace fake predictions with actual neural computations
2. **27+ Model Choices**: Access to state-of-the-art forecasting models
3. **Probabilistic Forecasting**: Get uncertainty estimates with DeepAR
4. **Optimized Performance**: Leverage ruv-FANN's SIMD optimizations
5. **Production Ready**: Battle-tested implementations

## Performance Considerations

- ruv-FANN includes WebGPU support for acceleration
- SIMD optimizations for CPU inference
- Parallel training capabilities
- Memory-efficient implementations

## Conclusion

The neural-trader project has all the components needed for real neural forecasting but isn't using them. The migration from placeholders to neuro-divergent models will provide immediate value with minimal integration effort.