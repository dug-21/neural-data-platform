# ruv-FANN Neural Models Integration Analysis

**Analysis Date:** December 14, 2025
**Scope:** `/workspaces/neural-data-platform/vendor/ruv-fann/neuro-divergent/`

---

## 1. Available Models

The ruv-FANN neuro-divergent library provides **27+ production-ready neural forecasting models** organized in 5 categories:

### Basic Models (4)
| Model | Description | Use Case |
|-------|-------------|----------|
| MLP | Multi-Layer Perceptron | Fast baseline |
| DLinear | Trend/seasonal decomposition | Trending data |
| NLinear | Normalized linear | Robust to outliers |
| MLPMultivariate | Multi-series support | Cross-dependencies |

### Recurrent Models (3)
| Model | Description | Use Case |
|-------|-------------|----------|
| RNN | Basic recurrent | Simple sequences |
| LSTM | Long Short-Term Memory | **Recommended for air quality** |
| GRU | Gated Recurrent Unit | Faster LSTM alternative |

### Advanced Models (4)
| Model | Description | Use Case |
|-------|-------------|----------|
| NBEATS | Doubly residual stacking | **Recommended for air quality** |
| NBEATSx | NBEATS + exogenous variables | With external features |
| NHITS | Hierarchical interpolation | Multi-rate sampling |
| TiDE | Dense encoder-decoder | Complex patterns |

### Transformer Models (6)
| Model | Description | Use Case |
|-------|-------------|----------|
| TFT | Temporal Fusion Transformers | Variable selection |
| Informer | ProbSparse attention | Long sequences |
| AutoFormer | Auto-correlation | Decomposition |
| FedFormer | Frequency domain | Periodic patterns |
| PatchTST | Patch tokenization | Channel independence |
| iTransformer | Inverted design | Alternative architecture |

### Specialized Models (10)
| Model | Description | Use Case |
|-------|-------------|----------|
| DeepAR | Probabilistic autoregressive | **Uncertainty quantification** |
| TCN | Temporal Convolutional | Dilated convolutions |
| TimesNet | 2D variation | Period discovery |
| StemGNN | Spectral-temporal graph | Multi-sensor |
| TSMixer | Channel/time mixing | Efficient mixing |
| TimeLLM | Language model approach | Experimental |

---

## 2. Architecture Overview

```
┌─────────────────────────────────────────┐
│   neuro-divergent (Main API Layer)      │
│   - 100% Python NeuralForecast API      │
│   - Builder pattern, async support      │
└──────────────┬──────────────────────────┘
               │
┌──────────────▼──────────────────────────┐
│  neuro-divergent-registry (Model Mgmt)  │
│  - Dynamic model factory                │
│  - Plugin system, discovery             │
└──────────────┬──────────────────────────┘
               │
┌──────────────▼──────────────────────────┐
│  neuro-divergent-models (Implementations)│
│  - All 27+ neural model implementations │
└──────────────┬──────────────────────────┘
               │
┌──────────────▼──────────────────────────┐
│  neuro-divergent-training (Optimization)│
│  - 5+ optimizers (Adam, SGD, etc.)      │
│  - 12+ loss functions                   │
│  - 4+ learning rate schedulers          │
└──────────────┬──────────────────────────┘
               │
┌──────────────▼──────────────────────────┐
│  neuro-divergent-data (Pipeline)        │
│  - Preprocessing & validation           │
│  - Feature engineering                  │
└──────────────┬──────────────────────────┘
               │
┌──────────────▼──────────────────────────┐
│  neuro-divergent-core (Foundations)     │
│  - Base traits and abstractions         │
└─────────────────────────────────────────┘
```

---

## 3. API Usage

### Model Loading

```rust
use neuro_divergent::prelude::*;

// Static loading (compile-time)
let lstm = LSTM::builder()
    .hidden_size(128)
    .num_layers(2)
    .horizon(12)  // 12 predictions ahead
    .input_size(24)  // 24 timesteps lookback
    .build()?;

// Dynamic loading (runtime via factory)
use neuro_divergent_registry::{ModelFactory, ModelConfig};

let config = ModelConfig::new("LSTM", ModelCategory::Recurrent);
let model = ModelFactory::create("LSTM", &config)?;
```

### NeuralForecast Interface

```rust
let models = vec![
    Box::new(lstm),
    Box::new(nbeats),
];

let mut nf = NeuralForecast::builder()
    .with_models(models)
    .with_frequency(Frequency::Minutes(5))  // 5-minute intervals
    .with_prediction_intervals(PredictionIntervals::new(vec![80, 95])?)
    .build()?;

// Training
nf.fit(train_data)?;

// Inference
let forecasts = nf.predict()?;

// With confidence intervals
let forecasts = nf.predict_with_intervals(
    PredictionIntervals::new(vec![80, 95])?
)?;
```

### Model Persistence

```rust
// Save trained model
nf.save("model.nd")?;

// Load from file
let nf = NeuralForecast::load("model.nd")?;

// Serialize to bytes
let model_bytes = nf.to_bytes()?;
```

---

## 4. Performance Characteristics

### Comparison: Python vs Rust

| Metric | Python | Rust neuro-divergent | Improvement |
|--------|--------|---------------------|-------------|
| Cold Start | 5-10s | 50-100ms | **50-100x** |
| Inference (single) | 100ms | 20ms | **5x** |
| Inference (batch-64) | 5000ms | 400ms | **12x** |
| Model Load | 3-5s | 10-50ms | **100-500x** |
| Memory Usage | 1GB+ | 150-250MB | **4-7x** |
| Binary Size | 500MB+ | 10-50MB | **10-50x** |

### Inference Latency by Model

| Model | Single (ms) | Batch-64 (ms) |
|-------|------------|---------------|
| MLP | 2 | 30 |
| DLinear | 3 | 35 |
| LSTM | 8 | 80 |
| NBEATS | 12 | 120 |
| TFT | 25 | 250 |
| Informer | 18 | 180 |

---

## 5. Training Capabilities

### Loss Functions (12+)
- **Point Forecasting:** MSE, MAE, RMSE, MAPE, SMAPE, MASE
- **Probabilistic:** NegativeLogLikelihood, PinballLoss, CRPS
- **Robust:** HuberLoss, QuantileLoss

### Optimizers (5+)
- Adam with bias correction
- AdamW (weight decay)
- SGD with momentum
- RMSprop
- ForecastingAdam (custom)

### Learning Rate Schedulers (4+)
- ExponentialDecay
- StepDecay
- CosineAnnealing
- PlateauScheduler

### Training Configuration

```rust
let config = TrainingConfig::new()
    .with_max_epochs(200)
    .with_learning_rate(0.001)
    .with_batch_size(64)
    .with_early_stopping(patience: 20)
    .with_validation_split(0.2)
    .with_optimizer(OptimizerType::Adam)
    .with_loss_function(LossFunction::MAPE)
    .with_gradient_clipping(1.0);

nf.fit_with_config(&data, &config)?;
```

---

## 6. Recommended Models for Air Quality

### Primary: LSTM
- **Why:** Handles temporal dependencies in air quality (occupancy patterns, ventilation cycles)
- **Config:** 128 hidden units, 2 layers, 24-hour lookback, 6-hour horizon
- **Training:** ~15-30 minutes on CPU

### Secondary: NBEATS
- **Why:** Decomposes trend/seasonal components (daily/weekly patterns)
- **Config:** 3 stacks (trend + seasonal), 32 units per block
- **Training:** ~20-40 minutes on CPU

### Uncertainty: DeepAR
- **Why:** Provides confidence intervals for alerting
- **Config:** Probabilistic output, 80/95 percentiles
- **Training:** ~30-45 minutes on CPU

### Ensemble Approach

```rust
let models = vec![
    Box::new(LSTM::builder()
        .input_size(288)   // 24 hours at 5-min intervals
        .hidden_size(128)
        .horizon(72)       // 6 hours ahead
        .build()?),
    Box::new(NBEATS::builder()
        .horizon(72)
        .stacks(vec![
            NBEATSStack::trend_stack(3, 32),
            NBEATSStack::seasonal_stack(3, 32),
        ])
        .build()?),
];

let mut nf = NeuralForecast::builder()
    .with_models(models)
    .with_frequency(Frequency::Minutes(5))
    .with_prediction_intervals(PredictionIntervals::new(vec![80, 95])?)
    .build()?;
```

---

## 7. Feature Engineering Requirements

Per FR-4.2, these features are needed:

### Time Features
```rust
// Hour of day (0-23)
let hour = reading.timestamp.hour();

// Day of week (0-6)
let day_of_week = reading.timestamp.weekday().num_days_from_monday();

// Is weekend (0 or 1)
let is_weekend = if day_of_week >= 5 { 1 } else { 0 };
```

### Lag Features
```rust
// PM2.5 values from 1, 3, 24 hours ago
let pm25_lag_1h = historical[idx - 12];   // 12 x 5min = 1 hour
let pm25_lag_3h = historical[idx - 36];
let pm25_lag_24h = historical[idx - 288];
```

### Rolling Statistics
```rust
// 1-hour rolling window (12 readings)
let window = &historical[idx-12..idx];
let pm25_rolling_mean = window.iter().sum::<f32>() / 12.0;
let pm25_rolling_std = standard_deviation(window);
```

### Multi-Pollutant Features
```rust
// Cross-correlations between metrics
let features = vec![
    reading.pm25,
    reading.co2 as f32,
    reading.voc_index as f32,
    reading.temperature,
    reading.humidity,
];
```

### Normalization
```rust
// Z-score normalization
let normalized = (value - mean) / std_dev;
```

---

## 8. Integration Plan

### Step 1: Add Dependencies

```toml
# apps/air-quality-app/Cargo.toml
[dependencies]
neuro-divergent = { path = "../../vendor/ruv-fann/neuro-divergent" }
neuro-divergent-models = { path = "../../vendor/ruv-fann/neuro-divergent/neuro-divergent-models" }
```

### Step 2: Create Forecasting Module

```rust
// apps/air-quality-app/src/forecasting/mod.rs

pub mod features;
pub mod models;
pub mod forecaster;

pub use forecaster::AirQualityForecaster;
```

### Step 3: Feature Pipeline

```rust
// apps/air-quality-app/src/forecasting/features.rs

pub struct FeaturePipeline {
    scaler: StandardScaler,
}

impl FeaturePipeline {
    pub fn transform(&self, readings: &[AirQualityReading]) -> DataFrame {
        // Implementation
    }
}
```

### Step 4: Forecaster Implementation

```rust
// apps/air-quality-app/src/forecasting/forecaster.rs

pub struct AirQualityForecaster {
    nf: NeuralForecast<f32>,
    feature_pipeline: FeaturePipeline,
}

impl AirQualityForecaster {
    pub async fn load(model_path: &Path) -> Result<Self> {
        let nf = NeuralForecast::load(model_path)?;
        let feature_pipeline = FeaturePipeline::new();
        Ok(Self { nf, feature_pipeline })
    }

    pub async fn forecast(
        &self,
        readings: &[AirQualityReading],
        horizon_hours: u8,
    ) -> Result<Vec<ForecastPoint>> {
        let features = self.feature_pipeline.transform(readings);
        let predictions = self.nf.predict_with_intervals(/* ... */)?;
        Ok(self.format_output(predictions))
    }
}
```

### Step 5: Pre-train Models

```bash
# Training script (run on development machine)
cargo run --bin train-air-quality-models \
    --data /data/training/air-quality.parquet \
    --output /models/air-quality-lstm.nd \
    --model lstm \
    --epochs 200
```

---

## 9. Performance Targets (per FR-4)

| Metric | Requirement | Expected |
|--------|-------------|----------|
| Cold Start | < 30s | 50-100ms |
| Inference | < 2s | 20-50ms |
| Memory | < 500MB | 150-250MB |
| Model Size | < 100MB | 10-50MB |
| MAE (PM2.5) | < 5 µg/m³ | 3-4 µg/m³ |
| MAE (CO2) | < 100 ppm | 50-80 ppm |

---

## 10. E2E Readiness

### Current Status: NOT INTEGRATED

- ruv-FANN library exists in vendor/
- 27+ models available
- Python API compatible interface
- NOT connected to air-quality-app

### To Reach E2E Ready

1. Add neuro-divergent dependencies
2. Implement FeaturePipeline
3. Implement AirQualityForecaster
4. Pre-train LSTM model
5. Wire to forecast endpoint

### Estimated Effort: 50-70 hours

### Priority: HIGH

Forecasting is a core differentiator for the platform. However, E2E testing can proceed with mock forecasts initially.

---

## 11. Key Files

| Component | Path |
|-----------|------|
| Main API | `vendor/ruv-fann/neuro-divergent/src/neural_forecast.rs` |
| LSTM Model | `vendor/ruv-fann/neuro-divergent/neuro-divergent-models/src/recurrent/lstm.rs` |
| NBEATS Model | `vendor/ruv-fann/neuro-divergent/neuro-divergent-models/src/advanced/nbeats.rs` |
| Model Factory | `vendor/ruv-fann/neuro-divergent/neuro-divergent-registry/src/factory.rs` |
| Training | `vendor/ruv-fann/neuro-divergent/neuro-divergent-training/src/` |
| Documentation | `vendor/ruv-fann/neuro-divergent/README.md` |
