# Neural Pattern Recognition for Time-Series: Edge-Optimized Architectures

**Research Date**: 2026-02-02
**Platform**: Raspberry Pi 5 (ARM64, 16GB RAM)
**Framework**: ruv-FANN (Rust-native neural networks)
**Domain**: Environmental sensor data (air quality, weather)

---

## Executive Summary

This research evaluates neural network architectures for time-series forecasting on edge devices, specifically targeting air quality prediction and weather data processing on Raspberry Pi. The analysis covers architecture selection, lightweight optimization techniques, self-supervised pre-training strategies, and online learning approaches for concept drift adaptation.

### Key Recommendations

| Component | Recommendation | Rationale |
|-----------|---------------|-----------|
| **Primary Architecture** | TCN (Temporal Convolutional Network) | Parallelizable, faster training, better long-term memory than LSTM |
| **Interpretable Model** | N-BEATS / N-HiTS | Fully-connected, no recurrence, trend/seasonality decomposition |
| **Foundation Model** | Chronos-Bolt (tiny) | 250x faster than original, 20x memory efficient, zero-shot capable |
| **Optimization** | INT8 quantization + pruning | 75% memory reduction, 4x speed improvement |
| **Online Learning** | EWC++ with ADWIN drift detection | Prevents catastrophic forgetting, automatic concept drift adaptation |
| **Edge Runtime** | ONNX Runtime + TensorFlow Lite | Cross-platform, ARM-optimized, INT8 inference |

---

## 1. Time-Series Neural Architectures

### 1.1 Architecture Comparison Matrix

| Architecture | Memory | Latency | Long-term Deps | Parallelizable | Edge Feasibility |
|--------------|--------|---------|----------------|----------------|------------------|
| **LSTM** | Medium | Medium | Limited (50-200 steps) | No | Good |
| **GRU** | Low | Low | Limited (50-200 steps) | No | Excellent |
| **TCN** | Low | Very Low | Excellent (1000+ steps) | Yes | Excellent |
| **Transformer** | High | High | Excellent | Yes | Poor (Pi 5) |
| **N-BEATS** | Medium | Low | Good (horizon-dependent) | Yes | Good |
| **N-HiTS** | Medium | Low | Excellent (hierarchical) | Yes | Good |
| **LNN (Liquid)** | Very Low | Very Low | Adaptive | Yes | Excellent |
| **SNN (Spiking)** | Very Low | Very Low | Event-driven | Yes | Excellent (w/NPU) |

### 1.2 Temporal Convolutional Networks (TCN)

**Architecture Overview**:
TCNs use dilated causal convolutions to achieve exponentially large receptive fields while maintaining computational efficiency.

```
Input: [x_1, x_2, ..., x_T]
                |
    +-----------+-----------+
    |  Dilated Causal Conv  |  (dilation=1)
    +-----------+-----------+
                |
    +-----------+-----------+
    |  Dilated Causal Conv  |  (dilation=2)
    +-----------+-----------+
                |
    +-----------+-----------+
    |  Dilated Causal Conv  |  (dilation=4)
    +-----------+-----------+
                |
Output: [y_1, y_2, ..., y_T]
```

**Key Advantages for Edge Deployment**:

1. **Parallelizable**: Unlike RNNs, convolutions can be computed in parallel
2. **Faster Training**: No sequential dependencies during backpropagation
3. **Larger Receptive Field**: Dilated convolutions capture long-range dependencies
4. **Causal Constraints**: Ensures predictions only depend on past data
5. **Simpler Architecture**: Fewer hyperparameters than LSTM/Transformer

**Performance Comparison** (from research):
- TCN achieves 33.1% higher accuracy than single LSTM for nitrogen concentration forecasting
- Training is 2-3x faster than equivalent LSTM models
- Memory footprint is 40-60% smaller due to absence of hidden states

**Reference**: [Unit8 TCN Forecasting Guide](https://unit8.com/resources/temporal-convolutional-networks-and-forecasting/)

**Rust Implementation Pattern** (for ruv-FANN):
```rust
// TCN block structure for ruv-FANN integration
struct TcnBlock {
    dilation: usize,
    kernel_size: usize,
    filters: usize,
    residual: bool,
}

struct TcnConfig {
    input_dim: usize,
    output_dim: usize,
    num_blocks: usize,      // 4-8 for air quality
    kernel_size: usize,     // 3-7 typical
    num_filters: usize,     // 32-64 for Pi 5
    dilations: Vec<usize>,  // [1, 2, 4, 8, 16, 32]
    dropout: f64,           // 0.1-0.3
}

// Receptive field = 1 + (kernel_size - 1) * sum(dilations)
// For kernel=3, dilations=[1,2,4,8,16,32]: RF = 1 + 2*63 = 127 time steps
```

### 1.3 N-BEATS (Neural Basis Expansion Analysis)

**Architecture Overview**:
N-BEATS uses stacks of fully-connected blocks with residual connections, providing interpretable trend and seasonality decomposition.

```
Input Lookback Window
        |
   +----+----+
   |  Block  | --> Backcast (reconstructs input)
   +----+----+
        |
   +----+----+
   |  Block  | --> Backcast
   +----+----+
        |
   ... (multiple blocks)
        |
   Sum of Forecasts --> Output
```

**Key Properties**:

1. **Interpretability**: Separate trend and seasonality stacks
2. **No Recurrence**: Pure feedforward architecture
3. **Residual Learning**: Each block learns residual patterns
4. **Ensemble-Ready**: Multiple stacks can be combined

**Performance** (from M4 competition):
- 11% improvement over statistical benchmarks
- State-of-the-art on M3, M4, TOURISM datasets
- MAE of 0.172 for environmental applications

**Configuration for Air Quality**:
```yaml
n_beats_config:
  stack_types: ["trend", "seasonality", "generic"]
  num_blocks_per_stack: 3
  num_layers: 4
  layer_width: 256  # Reduce to 128 for Pi 5
  expansion_coefficient_dim: 5
  lookback_length: 168  # 7 days hourly
  forecast_length: 24   # 24 hours ahead
```

**Reference**: [N-BEATS Paper](https://arxiv.org/abs/1905.10437)

### 1.4 N-HiTS (Neural Hierarchical Interpolation)

**Architecture Overview**:
N-HiTS extends N-BEATS with hierarchical interpolation for multi-scale temporal patterns.

**Key Improvements over N-BEATS**:
- Hierarchical pooling for multi-resolution features
- Better handling of long forecast horizons
- More memory-efficient for long sequences

**Use Case**: Air quality with multiple seasonalities (hourly, daily, weekly)

### 1.5 Liquid Neural Networks (LNN)

**Architecture Overview**:
Liquid Neural Networks use Liquid Time-Constant (LTC) neurons that continuously adapt their dynamics.

**Key Properties**:
- Adaptive time constants respond to input dynamics
- Compact representation (10-100x fewer neurons)
- Handles irregular time series naturally
- Ultra-low latency (<1ms inference)

**Edge Suitability**:
- Extremely lightweight (1,000-10,000 parameters typical)
- CPU-only inference feasible
- Well-suited for Raspberry Pi

**Reference**: [Liquid Neural Networks for Edge AI](https://ajithp.com/2025/05/04/liquid-neural-networks-edge-ai/)

### 1.6 Hybrid Architectures

**TCN-LSTM Hybrid**:
Combines TCN's efficient feature extraction with LSTM's sequential modeling.

```
Input --> TCN Encoder --> LSTM Decoder --> Output
```

**Performance**: 33.1% accuracy improvement over single models for water quality forecasting.

**Attention-Enhanced Models**:
- TCAN (TCN + Attention): Superior PM2.5 prediction
- Captures long-term dependencies more effectively
- Reference: [Spatio-temporal Attention Causal CNN](https://www.frontiersin.org/journals/environmental-science/articles/10.3389/fenvs.2024.1408370/full)

---

## 2. Lightweight Neural Networks for Edge

### 2.1 Model Compression Techniques

#### INT8 Quantization

**Benefits**:
- 75% memory reduction (FP32 -> INT8)
- 4x speed improvement on ARM NEON
- Minimal accuracy loss (<1% typically)

**Implementation**:
```python
# TensorFlow Lite quantization
converter = tf.lite.TFLiteConverter.from_saved_model(model_path)
converter.optimizations = [tf.lite.Optimize.DEFAULT]
converter.target_spec.supported_types = [tf.int8]
converter.inference_input_type = tf.int8
converter.inference_output_type = tf.int8
quantized_model = converter.convert()
```

**Raspberry Pi Performance**:
- Inference time reduction: 539ms -> 21ms (first image)
- Average inference: 114ms -> 3.7ms
- Reference: [Efficient CNNs on Raspberry Pi](https://www.researchsquare.com/article/rs-4345141/v1)

#### Pruning

**Magnitude-Based Pruning**:
1. Train full model
2. Rank weights by magnitude
3. Remove lowest X% of weights
4. Fine-tune remaining weights
5. Repeat until target sparsity

**Structured Pruning** (better for edge):
- Remove entire filters/channels
- Maintains dense tensor operations
- Better hardware utilization

**Combined Optimization Pipeline**:
```
Full Model (FP32, 100% weights)
        |
    Pruning (50-70% sparsity)
        |
    Quantization (INT8)
        |
    TensorFlow Lite Conversion
        |
Optimized Model (4-8x smaller, 3-10x faster)
```

### 2.2 TinyML Approaches

**Resource Constraints**:
| Platform | RAM | Flash | Typical Model Size |
|----------|-----|-------|-------------------|
| ESP32 | 520KB | 4MB | 10-50KB |
| STM32F4 | 192KB | 1MB | 5-30KB |
| Cortex-M55 + Ethos-U55 | 1MB | 2MB | 50-200KB |
| **Raspberry Pi 5** | 16GB | SD | 1-100MB |

**Air Quality TinyML Results**:
- Model sizes as small as 3KB-5KB
- 83% size reduction with quantization
- MSE of 0.03, R-squared of 0.95 for ozone prediction
- Reference: [TinyML for Air Quality](https://www.researchgate.net/publication/373942606_TinyML_Models_for_a_Low-cost_Air_Quality_Monitoring_Device)

### 2.3 Neural Network Accelerators for Pi 5

**Raspberry Pi AI Kit (Hailo-8L)**:
- 13 TOPS neural inference
- M.2 2242 form factor
- Ideal for real-time inference

**Google Coral USB Accelerator**:
- Edge TPU for INT8 inference
- USB plug-and-play
- 4 TOPS performance

**Performance Comparison**:
| Accelerator | TOPS | Power | Latency (MobileNet) |
|-------------|------|-------|---------------------|
| Pi 5 CPU | ~0.1 | 15W | ~100ms |
| Coral USB | 4 | 2W | ~10ms |
| Hailo-8L | 13 | 2.5W | ~5ms |

### 2.4 Spiking Neural Networks (SNN)

**Energy Efficiency**:
- 90% accuracy at 0.16W vs 9.75W for conventional
- Event-driven computation (only active on input changes)
- Ideal for "always-on" sensor monitoring

**Time Series Applicability**:
- Natural temporal encoding
- Low SWaP (Size, Weight, Power)
- Requires specialized hardware (neuromorphic chips)

**Performance**:
- Surrogate gradient SNNs: within 1-2% of ANN accuracy
- Latency as low as 10ms
- Energy as low as 5mJ per inference

**Reference**: [SNN for Edge Intelligence](https://dl.acm.org/doi/abs/10.1109/TWC.2024.3374549)

---

## 3. Foundation Models for Time-Series

### 3.1 Chronos (Amazon)

**Overview**:
Pre-trained transformer model treating time series as a "language" - tokenizes data points and uses T5 architecture for forecasting.

**Model Variants**:
| Model | Parameters | Memory | Speed |
|-------|------------|--------|-------|
| Chronos-T5-Tiny | 8M | ~50MB | Fast |
| Chronos-T5-Mini | 20M | ~100MB | Medium |
| Chronos-T5-Small | 46M | ~200MB | Slow |
| Chronos-T5-Base | 200M | ~800MB | Very Slow |
| **Chronos-Bolt** | 20M | ~100MB | **250x faster** |

**Chronos-Bolt Advantages**:
- 5% lower error than original Chronos
- 250x faster inference
- 20x more memory efficient
- Patch-based architecture (chunks time series)
- Direct multi-step forecasting

**Zero-Shot Performance**:
- Outperforms benchmark models with <60% training data
- Superior at high-complexity stations
- Cross-station transferability demonstrated

**Reference**: [Chronos Forecasting](https://github.com/amazon-science/chronos-forecasting)

### 3.2 TimeGPT (Nixtla)

**Training Data**:
- 100+ billion data points
- Finance, healthcare, weather, IoT sensors, energy, etc.

**Edge Deployment Considerations**:
- Requires API access (cloud-based)
- Not suitable for offline Pi deployment
- Use for training data generation or validation

### 3.3 Chronos-2 (October 2025)

**Latest Capabilities**:
- 120M parameters (encoder-only)
- Univariate + multivariate + covariates in single architecture
- Best performance on GIFT-Eval, Chronos Benchmark II
- Zero-shot support

**Reference**: [Chronos-2 HuggingFace](https://huggingface.co/amazon/chronos-2)

### 3.4 Foundation Model Strategy for NDP

**Recommended Approach**:

1. **Pre-training**: Use Chronos-Bolt (tiny) for zero-shot baseline
2. **Fine-tuning**: Fine-tune on local air quality data
3. **Distillation**: Distill to smaller TCN/N-BEATS for production
4. **Edge Deployment**: Run distilled model on Pi 5

```
Chronos-Bolt (pretrained)
        |
    Fine-tune on NDP Bronze data
        |
    Knowledge Distillation
        |
    TCN-Lite (student model)
        |
    INT8 Quantization
        |
    ruv-FANN Deployment
```

---

## 4. Self-Supervised Pre-training Strategies

### 4.1 Pre-training on Unlabeled Sensor Data

**Contrastive Learning for Time Series**:
```
Augmentation 1 --> Encoder --> Representation 1
     |                              |
Original Data                  Contrastive Loss
     |                              |
Augmentation 2 --> Encoder --> Representation 2
```

**Augmentations for Environmental Data**:
1. Jittering (add small noise)
2. Scaling (amplitude variation)
3. Time warping (temporal distortion)
4. Cropping (subsequence extraction)
5. Masking (random time points removed)

### 4.2 Masked Autoencoder Pre-training

**Process**:
1. Mask 15-50% of time series values
2. Encoder processes visible patches
3. Decoder reconstructs masked values
4. Pre-trained encoder used for downstream tasks

**Benefits for Sensor Data**:
- Learns robust representations from unlabeled data
- Handles missing values naturally
- Captures temporal patterns without supervision

### 4.3 Transfer Learning Workflow

```
Phase 1: Pre-train on Public Data
    - Open-Meteo historical weather (free, extensive)
    - EPA air quality archives
    - Chronos pre-training corpus

Phase 2: Fine-tune on NDP Data
    - Bronze layer Parquet files
    - 90+ days of local sensor data
    - Task-specific heads (PM2.5, CO2, etc.)

Phase 3: Continual Learning
    - Online updates with EWC++
    - Drift detection with ADWIN
    - Periodic full retraining
```

---

## 5. Online Learning Approaches

### 5.1 Concept Drift in Environmental Data

**Types of Drift**:
1. **Sudden Drift**: New pollution source, equipment change
2. **Gradual Drift**: Seasonal transitions, sensor degradation
3. **Incremental Drift**: Slow climate changes
4. **Recurring Drift**: Seasonal patterns, holidays

**Detection with ADWIN**:
```rust
struct AdwinDriftDetector {
    window: VecDeque<f64>,
    threshold: f64,  // Typically 0.002 for 95% confidence
}

impl AdwinDriftDetector {
    fn add_element(&mut self, error: f64) -> bool {
        self.window.push_back(error);

        // Binary search for optimal cut point
        for cut_point in 0..self.window.len() {
            let (left, right) = self.split_at(cut_point);
            let diff = (mean(left) - mean(right)).abs();
            let threshold = self.compute_threshold(left.len(), right.len());

            if diff > threshold {
                self.window.drain(0..cut_point);
                return true;  // Drift detected
            }
        }
        false
    }
}
```

**Reference**: [ADWIN Documentation](https://riverml.xyz/dev/api/drift/ADWIN/)

### 5.2 Elastic Weight Consolidation (EWC++)

**Purpose**: Prevent catastrophic forgetting during incremental updates.

**Mechanism**:
1. Compute Fisher Information Matrix (importance of each weight)
2. Penalize changes to important weights during new learning
3. Allow flexibility for unimportant weights

**Loss Function**:
```
L_total = L_task + (lambda/2) * sum(F_i * (theta_i - theta_i*)^2)

Where:
- L_task: Current task loss
- F_i: Fisher information for weight i
- theta_i: Current weight value
- theta_i*: Weight value after previous task
- lambda: EWC strength (typically 2000-5000)
```

**Performance**:
- Reduces catastrophic forgetting from 12.62% to 6.85% (45.7% reduction)
- Online variants (EWC++) optimized for streaming data

**Reference**: [EWC for Continual Learning](https://www.nature.com/articles/s41467-025-64601-w)

### 5.3 Continual Learning Framework for NDP

**Three Strategy Categories**:

| Strategy | Approach | Memory Cost | Compute Cost |
|----------|----------|-------------|--------------|
| **Rehearsal** | Store/replay past samples | High | Medium |
| **Regularization** | EWC, SI, LwF | Low | Low |
| **Isolation** | Separate params per task | Medium | Low |

**Recommended: Regularization-Based (EWC++)**

```rust
struct ContinualLearner {
    model: FannNetwork,
    fisher_info: Vec<f64>,
    optimal_weights: Vec<f64>,
    ewc_lambda: f64,
    drift_detector: AdwinDriftDetector,
}

impl ContinualLearner {
    fn update(&mut self, new_data: &[TrainingSample]) -> Result<()> {
        // Check for concept drift
        let predictions = self.model.predict(&new_data);
        let errors: Vec<f64> = compute_errors(&predictions, new_data);

        for error in errors {
            if self.drift_detector.add_element(error) {
                info!("Concept drift detected, triggering update");
                self.incremental_train(new_data)?;
                break;
            }
        }

        Ok(())
    }

    fn incremental_train(&mut self, data: &[TrainingSample]) -> Result<()> {
        // Train with EWC regularization
        let ewc_loss = |weights: &[f64]| {
            let task_loss = self.compute_task_loss(weights, data);
            let ewc_term = self.compute_ewc_penalty(weights);
            task_loss + self.ewc_lambda * ewc_term
        };

        self.model.train_with_loss(ewc_loss)?;

        // Update Fisher information for future updates
        self.update_fisher_information(data);
        self.optimal_weights = self.model.get_weights();

        Ok(())
    }
}
```

### 5.4 MESU (Metaplasticity from Synaptic Uncertainty)

**Latest Approach (2025)**:
- Bayesian learning rule scaling by uncertainty
- Principled combination of learning and forgetting
- No explicit task boundaries needed

**Performance**: Outperforms EWC, SI on 200 sequential tasks.

**Reference**: [Bayesian Continual Learning](https://www.nature.com/articles/s41467-025-64601-w)

---

## 6. Recommended NDP Neural Architecture

### 6.1 Primary Architecture: TCN-Lite

**Configuration for Air Quality Forecasting**:

```yaml
tcn_lite_config:
  # Input/Output
  input_features: 14
    # - pm25_mean_4h, pm25_std_4h, pm25_trend_4h
    # - pm25_lag_1h, pm25_lag_6h, pm25_lag_24h
    # - temp_current, humidity_current, co2_current
    # - temp_outdoor, wind_speed
    # - hour_of_day, day_of_week, week_of_year
  output_dim: 1  # PM2.5 forecast
  forecast_horizon: 24  # hours

  # Architecture
  num_blocks: 4
  kernel_size: 3
  num_filters: 32  # Reduced from 64 for Pi 5
  dilations: [1, 2, 4, 8]
  dropout: 0.2

  # Receptive field: 1 + 2*(1+2+4+8) = 31 time steps

  # Optimization
  quantization: int8
  pruning_sparsity: 0.5

  # Estimated size: ~150KB (INT8, pruned)
  # Estimated latency: ~10ms on Pi 5
```

### 6.2 Interpretable Backup: N-BEATS-Lite

```yaml
nbeats_lite_config:
  # Architecture
  stack_types: ["trend", "seasonality"]
  num_blocks_per_stack: 2
  num_layers: 3
  layer_width: 128  # Reduced from 256

  # Time series
  lookback_length: 168  # 7 days hourly
  forecast_length: 24

  # Optimization
  quantization: int8

  # Estimated size: ~500KB (INT8)
  # Estimated latency: ~20ms on Pi 5
```

### 6.3 Foundation Model Integration

```yaml
chronos_integration:
  # Pre-training
  base_model: chronos-bolt-tiny
  fine_tune_epochs: 50

  # Distillation
  teacher: chronos-bolt-fine-tuned
  student: tcn-lite
  distillation_temperature: 3.0

  # Deployment
  runtime: onnx-runtime
  precision: int8
```

### 6.4 Online Learning Configuration

```yaml
online_learning:
  # Drift Detection
  drift_detector: adwin
  adwin_threshold: 0.002

  # Continual Learning
  method: ewc_plus_plus
  ewc_lambda: 2000

  # Update Policy
  min_samples_for_update: 168  # 1 week of hourly data
  max_update_frequency: "weekly"
  validation_threshold: 0.95  # Only swap if MAE < 0.95 * current

  # Shadow Model
  enable_shadow_training: true
  shadow_model_path: "/models/shadow/"
```

### 6.5 Deployment Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Raspberry Pi 5                            │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────────┐ │
│  │  TimescaleDB │───>│ Feature     │───>│ Redis Cache    │ │
│  │  (Silver)   │    │ Engineering │    │ (5min TTL)     │ │
│  └─────────────┘    └─────────────┘    └───────┬─────────┘ │
│                                                  │          │
│  ┌─────────────────────────────────────────────┬┘          │
│  │                                             │            │
│  v                                             v            │
│  ┌─────────────┐                    ┌─────────────────────┐ │
│  │ Active Model│                    │ Shadow Model        │ │
│  │ (TCN-Lite)  │                    │ (Training)          │ │
│  │ INT8, ONNX  │                    │                     │ │
│  └──────┬──────┘                    └──────────┬──────────┘ │
│         │                                      │            │
│         v                                      v            │
│  ┌─────────────┐                    ┌─────────────────────┐ │
│  │ Prediction  │                    │ ADWIN Drift         │ │
│  │ (< 10ms)    │                    │ Detector            │ │
│  └──────┬──────┘                    └──────────┬──────────┘ │
│         │                                      │            │
│         v                                      v            │
│  ┌─────────────┐                    ┌─────────────────────┐ │
│  │ Prediction  │                    │ EWC++ Incremental   │ │
│  │ Log (Timesc)│                    │ Training            │ │
│  └─────────────┘                    └─────────────────────┘ │
│                                                              │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │                    Model Hot-Swap                        │ │
│  │  (Validation gate: only swap if error < threshold)       │ │
│  └─────────────────────────────────────────────────────────┘ │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

---

## 7. Implementation Roadmap

### Phase 1: Baseline (Weeks 1-4)

**Objective**: Establish baseline with statistical models + simple neural networks

| Task | Tool | Output |
|------|------|--------|
| Statistical baseline | augurs (ETS, MSTL) | MAE baseline |
| Simple MLP | ruv-FANN | Neural baseline |
| Feature engineering | TimescaleDB | Feature views |
| ONNX export | TensorFlow/PyTorch | Portable models |

### Phase 2: TCN Development (Weeks 5-8)

**Objective**: Implement and optimize TCN architecture

| Task | Tool | Output |
|------|------|--------|
| TCN implementation | ruv-FANN / Burn | TCN model |
| Hyperparameter search | Optuna | Optimal config |
| INT8 quantization | TFLite / ONNX | Quantized model |
| Pruning | PyTorch / TF | Sparse model |
| Benchmark on Pi 5 | ONNX Runtime | Latency/memory |

### Phase 3: Foundation Model Integration (Weeks 9-12)

**Objective**: Leverage pre-trained models for improved accuracy

| Task | Tool | Output |
|------|------|--------|
| Chronos-Bolt evaluation | HuggingFace | Zero-shot baseline |
| Fine-tuning | PyTorch | Fine-tuned model |
| Knowledge distillation | PyTorch | TCN-Lite student |
| Transfer learning | Custom | Domain-adapted |

### Phase 4: Online Learning (Weeks 13-16)

**Objective**: Enable continuous adaptation

| Task | Tool | Output |
|------|------|--------|
| ADWIN implementation | Rust (custom) | Drift detector |
| EWC++ integration | ruv-FANN | Continual learner |
| Shadow model training | Background task | Hot-swap capability |
| Monitoring dashboard | Grafana | Drift/error tracking |

---

## 8. Risk Assessment

| Risk | Impact | Mitigation |
|------|--------|------------|
| TCN insufficient for long horizons | Medium | Fall back to N-HiTS or Transformer |
| Chronos too large for Pi 5 | Medium | Use distilled student model |
| EWC++ insufficient for drift | Medium | Add rehearsal buffer (100 samples) |
| INT8 quantization accuracy loss | Low | Use INT8 + FP16 hybrid |
| Inference latency > 100ms | Medium | Add Hailo-8L accelerator |

---

## 9. References

### Time-Series Architectures
- [Temporal Convolutional Networks and Forecasting](https://unit8.com/resources/temporal-convolutional-networks-and-forecasting/)
- [N-BEATS: Neural Basis Expansion Analysis](https://arxiv.org/abs/1905.10437)
- [Liquid Neural Networks for Edge AI](https://ajithp.com/2025/05/04/liquid-neural-networks-edge-ai/)
- [TCN-LSTM Hybrid for PM2.5](https://www.sciencedirect.com/science/article/abs/pii/S1309104223000570)
- [Spatio-temporal Attention Causal CNN](https://www.frontiersin.org/journals/environmental-science/articles/10.3389/fenvs.2024.1408370/full)

### Foundation Models
- [Amazon Chronos Forecasting](https://github.com/amazon-science/chronos-forecasting)
- [Chronos-2 HuggingFace](https://huggingface.co/amazon/chronos-2)
- [Time Series Foundation Models Overview](https://towardsdatascience.com/chronos-the-rise-of-foundation-models-for-time-series-forecasting-aaeba62d9da3/)

### Edge Deployment
- [Efficient CNNs on Raspberry Pi](https://www.researchsquare.com/article/rs-4345141/v1)
- [TinyML for Air Quality Monitoring](https://www.researchgate.net/publication/373942606_TinyML_Models_for_a_Low-cost_Air_Quality_Monitoring_Device)
- [SNN for Wireless Edge Intelligence](https://dl.acm.org/doi/abs/10.1109/TWC.2024.3374549)

### Online Learning
- [Bayesian Continual Learning (MESU)](https://www.nature.com/articles/s41467-025-64601-w)
- [ADWIN Drift Detection](https://riverml.xyz/dev/api/drift/ADWIN/)
- [Three Types of Incremental Learning](https://www.nature.com/articles/s42256-022-00568-3)
- [Continual Learning Survey](https://ietresearch.onlinelibrary.wiley.com/doi/full/10.1049/cvi2.70013)

### ruv-FANN Ecosystem
- [ruv-FANN GitHub](https://github.com/ruvnet/ruv-FANN)
- [ruv-swarm-ml on Lib.rs](https://lib.rs/crates/ruv-swarm-ml)
- [ruvector-sona EWC++](https://github.com/ruvnet/ruvector)

---

**Document Version**: 1.0
**Last Updated**: 2026-02-02
**Author**: Research Agent (Claude)
**Status**: Complete
