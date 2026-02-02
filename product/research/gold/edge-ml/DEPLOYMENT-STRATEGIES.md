# Edge ML Deployment Strategies for Raspberry Pi

**Research Date**: 2026-02-02
**Platform**: Raspberry Pi 5 (16GB RAM, ARM Cortex-A76, VideoCore VII GPU)
**Context**: Neural Data Platform - Time-series prediction for environmental monitoring
**Author**: Research Agent (Claude)

---

## Executive Summary

This research evaluates edge ML deployment strategies for running machine learning inference on resource-constrained devices, specifically the Raspberry Pi 5. The analysis covers framework selection, optimization techniques, inference patterns, and integration approaches for the Neural Data Platform's time-series forecasting requirements.

### Key Findings

| Area | Recommendation | Rationale |
|------|----------------|-----------|
| **Primary Framework** | ONNX Runtime + Tract | Best ARM64 performance, 3.2x speedup over baselines |
| **Rust-Native ML** | Burn (deep learning) + augurs (time-series) | Production-ready, no_std support, WASM-compatible |
| **Quantization** | INT8 with FP16 fallback | 4x memory reduction, <2% accuracy loss |
| **Inference Pattern** | Inline processing hook (pre-batching) | 1-10ms latency vs 5s current |
| **ruv-FANN Assessment** | Promising for prototyping, needs validation | Pure Rust, comprehensive features, unverified claims |
| **Model Updates** | Hierarchical federated learning | On-device training with cloud aggregation |

### Resource Budget Summary (Pi 5 16GB)

| Configuration | Memory | CPU | Available Headroom |
|---------------|--------|-----|-------------------|
| Current NDP | 750MB | 10-20% | 15GB+ RAM, 80%+ CPU |
| + Small LLM (1-2B) | 3.5GB | 30-40% | 12GB RAM |
| + Full ML Stack | 5.5GB | 50-60% | 10GB RAM |
| + Inference Only (ruv-FANN) | 1.5GB | 20-30% | 14GB RAM |

---

## 1. Edge ML Framework Comparison Matrix

### 1.1 Framework Overview

| Framework | Type | Language | ARM64 Support | Maturity | Use Case |
|-----------|------|----------|---------------|----------|----------|
| **TensorFlow Lite** | Inference | C++/Python | Excellent | Production | General-purpose, mobile |
| **ONNX Runtime** | Inference | C++/Python/Rust | Excellent | Production | Cross-platform, model portability |
| **Tract** | Inference | Rust | Good | Production | ONNX/TF models in Rust |
| **Burn** | Training+Inference | Rust | Good | Maturing | Custom architectures |
| **Candle** | Inference | Rust | Partial (see note) | Maturing | HuggingFace models, LLMs |
| **augurs** | Time-series | Rust | Good | Early | Forecasting, anomaly detection |
| **ruv-FANN** | Training+Inference | Rust | Native | Experimental | Neural networks, time-series |
| **ncnn** | Inference | C++ | Excellent | Production | Mobile, embedded |

### 1.2 ARM64 Performance Benchmarks

| Framework | Model | Pi 5 Performance | Notes |
|-----------|-------|------------------|-------|
| TensorFlow Lite | MobileNetV2 | 25-30 FPS | 5x faster than Pi 4 |
| ONNX Runtime | YOLOv8n | 8-12 FPS | With ARM NEON |
| ncnn + Vulkan | YOLOv8n | 12 FPS | GPU acceleration |
| Tract | ONNX model | ~15ms inference | Rust-native |
| Burn (NdArray) | Custom MLP | ~5ms inference | Pure Rust |

### 1.3 Detailed Framework Analysis

#### TensorFlow Lite (LiteRT)

**Strengths**:
- Most widely used edge ML framework
- Extensive model zoo
- 10x performance gain with INT8 quantization
- Official ARM64 builds

**Weaknesses**:
- Python-centric (C++ API available but complex)
- Large dependency footprint
- Not Rust-native

**Performance on Pi 5**:
```
Model           | FP32      | INT8 Quantized
MobileNetV2     | 120ms     | 30ms
EfficientNetB0  | 180ms     | 45ms
YOLOv5n         | 200ms     | 60ms
```

**Installation**:
```bash
# ARM64 precompiled wheel
pip install tensorflow-lite-runtime
# Or build from source for latest optimizations
bazel build --config=elinux_aarch64 -c opt //tensorflow/lite/c:libtensorflowlite_c.so
```

**Source**: [Benchmarking TensorFlow Lite on Raspberry Pi 5](https://www.hackster.io/news/benchmarking-tensorflow-and-tensorflow-lite-on-raspberry-pi-5-b9156d58a6a2)

#### ONNX Runtime

**Strengths**:
- 3.2x speedup over PyTorch/TensorFlow baselines
- 2.5x faster than PyTorch Mobile (12ms vs 30ms)
- 50% memory reduction with INT8
- Extensible Execution Providers (ARM NN, XNNPACK)
- Model portability (export from any framework)

**Weaknesses**:
- Native Rust support via tract/ort
- Setup complexity for optimal performance

**ARM NEON Optimization**:
The ARM NN execution provider enables NEON-optimized kernels for:
- Convolutions
- Matrix multiplications
- Activation functions

**Rust Integration** (via `ort` crate):
```rust
use ort::{Environment, Session};

let environment = Environment::builder()
    .with_name("air-quality-inference")
    .build()?;

let session = Session::builder()
    .with_optimization_level(GraphOptimizationLevel::Level3)?
    .with_model_from_file("model.onnx")?;

let outputs = session.run(inputs)?;
```

**Source**: [ONNX Runtime Edge Inference 2025](https://johal.in/ai-inference-acceleration-with-python-onnx-runtime-deploying-models-on-edge-devices-2025/)

#### Tract (Rust ONNX/TF Runtime)

**Strengths**:
- Pure Rust ONNX/TensorFlow inference
- No external dependencies
- Excellent for embedded/no_std
- Streaming tensor support

**Weaknesses**:
- Limited operator coverage vs ONNX Runtime
- Smaller community

**NDP Integration Example**:
```rust
use tract_onnx::prelude::*;

// Load optimized model
let model = tract_onnx::onnx()
    .model_for_path("pm25_forecast.onnx")?
    .with_input_fact(0, InferenceFact::dt_shape(f32::datum_type(), tvec!(1, 14)))?
    .into_optimized()?
    .into_runnable()?;

// Inference
let input = tract_ndarray::arr2(&[[
    pm25_mean_4h, pm25_std_4h, temp_current, humidity_current,
    co2_current, pm25_lag_1h, pm25_lag_6h, pm25_lag_24h,
    temp_outdoor, wind_speed, hour_of_day, day_of_week,
    pm25_indoor_outdoor_diff, pm25_trend_4h
]]);

let result = model.run(tvec!(input.into()))?;
let prediction: f32 = result[0].to_array_view::<f32>()?[[0, 0]];
```

**Source**: [Tract on Lib.rs](https://lib.rs/crates/tract-onnx)

#### Burn (Deep Learning Framework)

**Strengths**:
- Written entirely in Rust
- no_std support (bare metal compatible)
- Multiple backends (NdArray, WGPU, Candle)
- WASM deployment for browser
- Type-safe tensor operations
- ONNX model import

**Weaknesses**:
- Younger ecosystem than PyTorch/TensorFlow
- Less pretrained model availability

**Backend Options**:
| Backend | Platform | Use Case |
|---------|----------|----------|
| NdArray | CPU (no_std) | Embedded, Pi |
| WGPU | GPU (WebGPU) | Browser, GPU acceleration |
| Candle | CPU/GPU | HuggingFace models |
| Tch | PyTorch | Training with PyTorch interop |

**NDP Time-Series Model Example**:
```rust
use burn::prelude::*;
use burn::nn::{Linear, LinearConfig, Lstm, LstmConfig};

#[derive(Module, Debug)]
pub struct AirQualityForecaster<B: Backend> {
    lstm: Lstm<B>,
    fc: Linear<B>,
}

impl<B: Backend> AirQualityForecaster<B> {
    pub fn forward(&self, x: Tensor<B, 3>) -> Tensor<B, 2> {
        let (_, hidden) = self.lstm.forward(x, None);
        self.fc.forward(hidden)
    }
}
```

**Source**: [Burn Framework for Rust ML](https://calmops.com/programming/rust/burn-framework-rust-ml/)

#### Candle (HuggingFace)

**Strengths**:
- Minimalist, small binary size
- Fast startup times
- Serverless inference focus
- HuggingFace model ecosystem
- Quantization support (2025)

**Weaknesses**:
- ARM64 linker issues with MKL
- LLM-focused (less time-series support)

**Note**: ARM64 builds may require workarounds for BLAS linkage:
```
Undefined symbols for architecture arm64: '_dgemm_', '_sgemm_'
```
Use OpenBLAS or disable MKL for ARM deployments.

**Source**: [Candle GitHub](https://github.com/huggingface/candle)

#### augurs (Grafana Time-Series)

**Strengths**:
- Purpose-built for monitoring
- ETS, MSTL, Prophet models
- DBSCAN anomaly detection
- Rust core with Python/JS bindings
- WASM support

**Weaknesses**:
- Early stage development
- Not official Grafana project (slower maintenance)

**NDP Integration**:
```rust
use augurs::mstl::{MSTLModel, TrendModel};
use augurs::ets::{ETSModel, AutoETS};

// Multi-seasonal decomposition
let model = MSTLModel::new(
    vec![24, 168],  // Daily (24h) and weekly (168h) seasonality
    TrendModel::Linear,
)?;
let decomposition = model.fit(&pm25_data)?;

// Exponential smoothing forecast
let ets = AutoETS::new()?;
let forecast = ets.fit_predict(&pm25_data, 24)?;  // 24-hour horizon
```

**Source**: [augurs GitHub](https://github.com/grafana/augurs)

---

## 2. Model Optimization Technique Catalog

### 2.1 Quantization Methods

| Method | Precision | Memory Reduction | Accuracy Loss | Hardware Support |
|--------|-----------|------------------|---------------|------------------|
| **FP32** (baseline) | 32-bit float | 1x | 0% | Universal |
| **FP16** | 16-bit float | 2x | 0-0.36% | GPU, modern CPU |
| **BF16** | 16-bit bfloat | 2x | ~0.5% | Ampere+, TPU |
| **INT8** | 8-bit integer | 4x | 0.14-1.52% | NEON, most accelerators |
| **INT4** | 4-bit integer | 8x | >10% (some tasks) | Limited |
| **Mixed** | Variable | 2-4x | <1% | Framework-dependent |

### 2.2 Quantization Strategies

#### Post-Training Quantization (PTQ)

**When to use**: Quick deployment, no retraining budget

**Process**:
1. Train model normally (FP32)
2. Calibrate with representative dataset
3. Convert weights and activations to lower precision

**TensorFlow Lite Example**:
```python
import tensorflow as tf

converter = tf.lite.TFLiteConverter.from_saved_model(saved_model_dir)
converter.optimizations = [tf.lite.Optimize.DEFAULT]
converter.representative_dataset = representative_dataset_gen
converter.target_spec.supported_ops = [tf.lite.OpsSet.TFLITE_BUILTINS_INT8]
converter.inference_input_type = tf.int8
converter.inference_output_type = tf.int8

quantized_model = converter.convert()
```

#### Quantization-Aware Training (QAT)

**When to use**: PTQ accuracy loss unacceptable

**Process**:
1. Insert fake quantization nodes during training
2. Model learns to be robust to quantization noise
3. Convert to integer model post-training

**Accuracy Recovery**: QAT typically recovers 50-100% of PTQ accuracy loss

### 2.3 Pruning Techniques

| Technique | Description | Speedup | Accuracy Impact |
|-----------|-------------|---------|-----------------|
| **Magnitude Pruning** | Remove smallest weights | 2-4x | Low if <70% sparse |
| **Structured Pruning** | Remove entire channels | 1.5-3x | Medium |
| **Dynamic Pruning** | Prune at runtime | 1.2-2x | Variable |

**Rust Example (manual pruning)**:
```rust
fn prune_weights(weights: &mut Array2<f32>, sparsity: f32) {
    let threshold = percentile(&weights.view(), sparsity * 100.0);
    weights.mapv_inplace(|w| if w.abs() < threshold { 0.0 } else { w });
}
```

### 2.4 Knowledge Distillation

**Concept**: Train small "student" model to mimic large "teacher" model

**Process**:
1. Train large teacher model (high accuracy)
2. Generate soft labels from teacher
3. Train small student on soft labels
4. Deploy student to edge

**Benefits for NDP**:
- Train complex LSTM/Transformer teacher in cloud
- Distill to small MLP for Pi deployment
- 10-50x size reduction with <5% accuracy loss

### 2.5 Neural Architecture Search (NAS) for Edge

**Edge-Optimized Architectures**:
- MobileNetV3 (Google)
- EfficientNet-Lite (Google)
- MCUNet (MIT)
- TinyNAS (Microsoft)

**NAS Constraints for Pi 5**:
```yaml
constraints:
  max_memory_mb: 512
  max_latency_ms: 50
  min_accuracy: 0.95
  target_ops: INT8
```

---

## 3. Inference Patterns for Raspberry Pi

### 3.1 Inline Processing Hook (Recommended)

**Latency**: 1-10ms (vs 5s with batch storage)

**Architecture**:
```
MQTT Message Arrives
        |
        v (microseconds)
   [JSON parse]
        |
        v (1-10ms)
   [ML Inference Hook] <-- INSERT ML HERE
        |
        v
   [Cache + Storage]
        |
        v (5 seconds)
   [Parquet Write]
```

**Implementation** (from NDP research):
```rust
// core/src/traits.rs
#[async_trait]
pub trait Processor: Send + Sync {
    async fn process(&self, point: &RawDataPoint) -> Result<ProcessorOutput, CoreError>;
    fn name(&self) -> &str;
}

// In mqtt/mod.rs process_events()
for processor in &processors {
    match processor.process(&raw_point).await {
        Ok(result) => {
            if let Some(predictions) = result.predictions {
                // Emit to predictions channel
            }
        }
        Err(e) => warn!("Processor {} failed: {}", processor.name(), e),
    }
}
```

### 3.2 Batch Inference

**When to use**: Non-real-time analytics, historical reprocessing

**Architecture**:
```
TimescaleDB Features --> Batch Export (Parquet) --> ML Training
                    \
                     --> Redis Cache --> Inference Engine
```

**Example**:
```rust
async fn batch_inference(features: &[FeatureVector]) -> Vec<Prediction> {
    // Process in batches for efficiency
    features
        .chunks(32)  // Batch size
        .flat_map(|batch| model.predict_batch(batch))
        .collect()
}
```

### 3.3 Streaming Inference Pipeline

**When to use**: Continuous predictions, alert systems

**Architecture**:
```
Event Bus (broadcast) --> ML Subscriber --> Predictions Channel --> Alert System
                     |
                     --> Bronze Subscriber (parallel)
                     |
                     --> Silver Subscriber (parallel)
```

**Example**:
```rust
async fn ml_subscriber(mut rx: broadcast::Receiver<RawDataPoint>) {
    let model = load_model("/models/pm25_v1.onnx")?;
    let mut feature_buffer = RingBuffer::new(24);  // 24-hour context

    loop {
        let point = rx.recv().await?;
        feature_buffer.push(extract_features(&point));

        if feature_buffer.is_ready() {
            let prediction = model.predict(&feature_buffer.as_slice())?;
            predictions_tx.send(prediction).await?;
        }
    }
}
```

### 3.4 Model Caching and Warm-up

**Strategies**:
1. **Memory-mapped loading**: Load model weights via mmap, avoid copy
2. **Warm-up runs**: Execute dummy inference at startup
3. **Model pool**: Keep multiple model versions loaded
4. **Lazy loading**: Load on first request, keep warm

**Rust Example**:
```rust
use memmap2::Mmap;
use std::sync::Arc;

struct ModelCache {
    models: RwLock<HashMap<String, Arc<LoadedModel>>>,
}

impl ModelCache {
    async fn get_or_load(&self, path: &str) -> Arc<LoadedModel> {
        {
            let cache = self.models.read().await;
            if let Some(model) = cache.get(path) {
                return model.clone();
            }
        }

        // Memory-mapped loading
        let file = File::open(path)?;
        let mmap = unsafe { Mmap::map(&file)? };
        let model = LoadedModel::from_bytes(&mmap)?;

        // Warm-up
        let dummy_input = Tensor::zeros(&[1, 14]);
        let _ = model.run(&dummy_input);

        let model = Arc::new(model);
        self.models.write().await.insert(path.to_string(), model.clone());
        model
    }
}
```

---

## 4. Raspberry Pi 5 Specific Recommendations

### 4.1 Hardware Capabilities

| Component | Specification | ML Relevance |
|-----------|---------------|--------------|
| CPU | Cortex-A76 x4 @ 2.4GHz | 2-3x faster than Pi 4 |
| RAM | 16GB LPDDR4X | Ample for multiple models |
| GPU | VideoCore VII @ 800MHz | Limited ML support |
| Storage | NVMe via PCIe 2.0 | Fast model loading |
| Network | 1Gbps Ethernet | Cloud model updates |

### 4.2 ARM NEON Optimizations

NEON is ARM's SIMD extension, enabling parallel processing of multiple data elements.

**Enable NEON in Rust**:
```toml
# .cargo/config.toml
[build]
rustflags = ["-C", "target-feature=+neon"]

[target.aarch64-unknown-linux-gnu]
rustflags = ["-C", "target-cpu=cortex-a76"]
```

**NEON-Optimized Operations**:
- Matrix multiplication (GEMM)
- Convolutions
- Activation functions (ReLU, Sigmoid)
- Pooling operations

**Framework Support**:
| Framework | NEON Status |
|-----------|-------------|
| TensorFlow Lite | Auto-enabled |
| ONNX Runtime + ARM NN | Opt-in EP |
| Tract | Limited |
| Burn | Backend-dependent |

### 4.3 VideoCore GPU Utilization

**Current State**: VideoCore VII has limited ML support

**Options**:
1. **Vulkan** (via ncnn/MNN): 12 FPS YOLOv8n achievable
2. **OpenGL ES**: Limited compute shader support
3. **Proprietary SDK**: Not available for ML

**Recommendation**: Focus on CPU (NEON) optimization unless using ncnn/Vulkan

**Source**: [Vulkan-based Inference on Pi](https://medium.com/analytics-vidhya/towards-gpu-accelerated-image-classification-on-low-end-hardware-ec592e125ad9)

### 4.4 Memory-Mapped Model Loading

**Benefits**:
- No copy-on-load
- Shared between processes
- Kernel manages paging

**Implementation**:
```rust
use memmap2::MmapOptions;

fn load_model_mmap(path: &str) -> Result<Model> {
    let file = File::open(path)?;
    let mmap = unsafe {
        MmapOptions::new()
            .populate()  // Pre-fault pages
            .map(&file)?
    };

    Model::from_bytes(&mmap)
}
```

### 4.5 Power Consumption Considerations

| Configuration | Idle Power | Inference Power | Notes |
|---------------|------------|-----------------|-------|
| CPU only | 3W | 8-12W | Recommended for NDP |
| + eGPU | 10-12W | 30-50W | For larger models |
| + Coral TPU | 4W | 10-12W | 10x speedup option |

**Power Optimization**:
1. Use INT8 quantization (lower compute = lower power)
2. Batch inferences when possible
3. Sleep between inference cycles
4. Consider Coral USB accelerator for intensive workloads

---

## 5. ruv-FANN Integration Assessment

### 5.1 Overview

[ruv-FANN](https://lib.rs/crates/ruv-fann) is a pure Rust rewrite of the Fast Artificial Neural Network (FANN) library, designed for time-series forecasting.

### 5.2 Architecture

```
ruv-FANN Ecosystem
|
+-- ruv-fann (core neural network library)
|   - Zero unsafe code
|   - FANN API compatible
|   - RPROP, Quickprop algorithms
|
+-- ruv-swarm-ml (27+ forecasting models)
|   - LSTM, GRU, Transformer
|   - NBEATS, NHITS, TiDE
|   - TFT, PatchTST
|
+-- ruvector-sona (online learning)
|   - EWC++ (catastrophic forgetting prevention)
|   - Sub-millisecond learning (<0.8ms)
|   - ReasoningBank pattern storage
|
+-- ruv-swarm-mcp (MCP server)
    - 13+ orchestration tools
    - Claude Code integration
```

### 5.3 Suitability for Time-Series Prediction

**Strengths**:
- 27+ forecasting models (most comprehensive Rust offering)
- Native time-series focus (unlike general-purpose frameworks)
- Online learning with EWC++ (critical for drift adaptation)
- MCP integration (aligns with NDP Claude tooling strategy)
- WASM deployment support

**Concerns**:
- Maturity unclear (limited production usage evidence)
- Performance claims unverified (84.8% SWE-Bench, 2.8-4.4x speed)
- Documentation varies in quality
- Small community compared to established frameworks

### 5.4 Integration Pattern for NDP

**Recommended Approach**: Hybrid evaluation

```
Phase 1: Prototype with ruv-FANN
|
+-- Train MLP/LSTM on historical PM2.5 data
+-- Benchmark against augurs baseline
+-- Validate EWC++ for drift adaptation
|
Phase 2: Production Decision
|
+-- If ruv-FANN outperforms: Deploy as primary
+-- If augurs outperforms: Use augurs + custom ADWIN
+-- Hybrid: augurs for ETS/MSTL, ruv-FANN for deep learning
```

**Example Integration**:
```rust
use ruv_fann::Fann;

// Create network: 14 inputs -> 32 hidden -> 16 hidden -> 1 output
let mut model = Fann::new(&[14, 32, 16, 1])?;
model.set_training_algorithm(TrainAlgorithm::RPROP);

// Train
model.train_on_file("features.data", max_epochs: 1000, target_mse: 0.001)?;

// Save (safetensors format)
model.save("/models/pm25_v1.safetensors")?;

// Inference
let input = vec![pm25_mean_4h, pm25_std_4h, temp, humidity, ...];
let output = model.run(&input)?;
let predicted_pm25 = output[0];
```

### 5.5 Comparison: ruv-FANN vs Alternatives

| Aspect | ruv-FANN | augurs | Burn | ONNX Runtime |
|--------|----------|--------|------|--------------|
| Time-series focus | Primary | Primary | General | General |
| Model variety | 27+ | 4-5 | Custom | Any ONNX |
| Online learning | EWC++ | None | Custom | Custom |
| Rust-native | Yes | Yes | Yes | Via tract/ort |
| Maturity | Experimental | Early | Maturing | Production |
| Pi 5 optimized | Unknown | Unknown | NEON via ndarray | ARM NN EP |

### 5.6 Assessment Summary

**Recommendation**: **Evaluate ruv-FANN for research/prototyping; validate before production**

**Action Items**:
1. Benchmark ruv-FANN LSTM vs augurs MSTL on NDP historical data
2. Test EWC++ online learning with simulated concept drift
3. Measure inference latency on Pi 5 with INT8 quantization
4. Verify MCP server integration with Claude Code

---

## 6. Hybrid Deployment Architecture

### 6.1 Edge Inference + Cloud Training

**Architecture**:
```
┌─────────────────────────────────────────────────────────────────┐
│                         RASPBERRY PI 5                           │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                    air-quality-app                         │   │
│  │                                                            │   │
│  │  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐   │   │
│  │  │   Sources   │───▶│  Event Bus  │───▶│ ML Processor│   │   │
│  │  │ (MQTT, HTTP)│    │ (broadcast) │    │ (inference) │   │   │
│  │  └─────────────┘    └──────┬──────┘    └──────┬──────┘   │   │
│  │                            │                   │          │   │
│  │                   ┌────────┴────────┐         │          │   │
│  │                   │                 │         │          │   │
│  │                   ▼                 ▼         ▼          │   │
│  │            ┌──────────┐      ┌──────────┐  ┌──────────┐  │   │
│  │            │  Bronze  │      │  Silver  │  │Predictions│  │   │
│  │            │ (Parquet)│      │(Timescale)│  │ (Alerts) │  │   │
│  │            └──────────┘      └──────────┘  └──────────┘  │   │
│  │                                                           │   │
│  │  ┌─────────────────────────────────────────────────────┐  │   │
│  │  │              MODEL CACHE (mmap)                      │  │   │
│  │  │  pm25_v3.onnx | temp_v2.onnx | co2_v1.safetensors  │  │   │
│  │  └─────────────────────────────────────────────────────┘  │   │
│  └──────────────────────────────────────────────────────────┘   │
│                              │                                   │
│                              │ MQTT/HTTPS                        │
│                              ▼                                   │
└─────────────────────────────────────────────────────────────────┘
                               │
                               │ Model Updates, Training Data
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                          CLOUD/SERVER                            │
│                                                                  │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐      │
│  │   Training   │    │    Model     │    │   Federated  │      │
│  │   Pipeline   │    │   Registry   │    │  Aggregator  │      │
│  │  (PyTorch)   │    │ (MLflow/S3)  │    │   (FedAvg)   │      │
│  └──────────────┘    └──────────────┘    └──────────────┘      │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 6.2 Federated Learning for NDP

**Why Federated Learning?**:
- Multiple Pi deployments (different homes/locations)
- Privacy-preserving (raw data stays local)
- Collective model improvement
- Personalization per location

**Architecture**:
```
┌────────────┐  ┌────────────┐  ┌────────────┐
│   Pi #1    │  │   Pi #2    │  │   Pi #3    │
│  (Home A)  │  │  (Home B)  │  │  (Home C)  │
└─────┬──────┘  └─────┬──────┘  └─────┬──────┘
      │               │               │
      │  Local Model  │  Local Model  │  Local Model
      │  Gradients    │  Gradients    │  Gradients
      │               │               │
      └───────────────┼───────────────┘
                      │
                      ▼
              ┌──────────────┐
              │   Federated  │
              │  Aggregator  │
              │   (FedAvg)   │
              └──────┬───────┘
                     │
                     │ Global Model Update
                     │
              ┌──────▼───────┐
              │    Model     │
              │   Registry   │
              └──────────────┘
```

**Implementation Considerations**:
1. **Gradient compression**: Reduce upload bandwidth
2. **Asynchronous updates**: Handle offline devices
3. **Differential privacy**: Add noise for privacy guarantees
4. **Personalization**: Keep local fine-tuning layer

**Source**: [Federated Learning at the Edge](https://www.intechopen.com/online-first/1230198)

### 6.3 Model Update Mechanisms

| Mechanism | Latency | Bandwidth | Complexity |
|-----------|---------|-----------|------------|
| **Full model push** | Minutes | High (MBs) | Low |
| **Delta updates** | Seconds | Low (KBs) | Medium |
| **On-device training** | Real-time | None | High |
| **Federated learning** | Hours | Medium | High |

**Recommended for NDP**: Delta updates with periodic federated aggregation

**Delta Update Flow**:
```
1. Cloud trains new model version
2. Compute delta: new_weights - old_weights
3. Quantize delta to INT8
4. Push compressed delta to Pi
5. Pi applies: current_weights += delta
6. Validate accuracy on local holdout
7. If accuracy drops: rollback to previous version
```

---

## 7. Resource Budget Recommendations

### 7.1 Memory Allocation (16GB Total)

| Component | Allocation | Notes |
|-----------|------------|-------|
| System + Docker | 2GB | Baseline overhead |
| NDP Services | 1GB | Mosquitto, etcd, Grafana, TimescaleDB |
| air-quality-app | 500MB | Rust binary, event bus |
| Bronze/Silver buffers | 500MB | In-memory caching |
| **ML Models** | **2GB** | 2-3 models loaded |
| **Inference scratch** | **1GB** | Tensor allocations |
| **Available** | **9GB** | Headroom for data ops |

### 7.2 CPU Allocation (4 cores)

| Component | Cores | Notes |
|-----------|-------|-------|
| System + Docker | 0.5 | Background tasks |
| NDP Services | 0.5 | Mosquitto, etcd, etc |
| air-quality-app | 1.0 | Event processing |
| **ML Inference** | **1.5** | Primary ML workload |
| **Available** | **0.5** | Burst headroom |

### 7.3 Recommended Model Sizes

| Model Type | Max Size | Inference Time | Notes |
|------------|----------|----------------|-------|
| MLP (time-series) | 10MB | <5ms | Simple forecasting |
| LSTM | 50MB | <20ms | Sequence modeling |
| Transformer-tiny | 100MB | <50ms | Complex patterns |
| **Total loaded** | **200MB** | - | Keep 2-3 models |

### 7.4 Monitoring Thresholds

```yaml
# alerts.yaml
resources:
  memory:
    warning: 80%  # 12.8GB used
    critical: 90% # 14.4GB used
  cpu:
    warning: 70%  # Sustained 70%
    critical: 85% # Sustained 85%
  inference_latency:
    warning: 50ms
    critical: 100ms
  model_load_time:
    warning: 5s
    critical: 10s
```

---

## 8. Implementation Roadmap

### Phase 1: Baseline ML Integration (Weeks 1-2)

**Objective**: Add ML inference hook to air-quality-app

**Tasks**:
1. Add `Processor` trait to core library
2. Implement inline processing in MQTT source
3. Create simple MLP model with tract
4. Benchmark inference latency

**Deliverables**:
- [ ] `core/src/traits.rs` - Processor trait
- [ ] `core/src/sources/mqtt/mod.rs` - Processor hook
- [ ] `apps/air-quality-app/src/processors/ml.rs` - ML processor
- [ ] `/models/pm25_baseline.onnx` - Trained model

### Phase 2: Optimized Inference (Weeks 3-4)

**Objective**: Achieve <10ms inference with INT8

**Tasks**:
1. Quantize model to INT8
2. Enable NEON optimizations
3. Implement memory-mapped loading
4. Add model warm-up on startup

**Deliverables**:
- [ ] Quantized model (`pm25_int8.onnx`)
- [ ] NEON build configuration
- [ ] Model cache with mmap
- [ ] Benchmark report

### Phase 3: Advanced Models (Weeks 5-8)

**Objective**: Evaluate ruv-FANN and augurs

**Tasks**:
1. Benchmark augurs MSTL vs ruv-FANN LSTM
2. Test EWC++ online learning
3. Implement ADWIN drift detection
4. Select production framework

**Deliverables**:
- [ ] Framework comparison report
- [ ] Drift detection implementation
- [ ] Online learning prototype
- [ ] Final framework recommendation

### Phase 4: Production Deployment (Weeks 9-12)

**Objective**: Full edge ML stack deployment

**Tasks**:
1. Implement model versioning
2. Add model update mechanism
3. Configure monitoring and alerts
4. Document operational procedures

**Deliverables**:
- [ ] Model registry on Pi
- [ ] Update mechanism (delta or full)
- [ ] Grafana ML dashboard
- [ ] Operations runbook

---

## 9. References

### Framework Documentation
- [TensorFlow Lite for ARM](https://ai.google.dev/edge/litert/conversion/tensorflow/quantization/post_training_quantization)
- [ONNX Runtime Execution Providers](https://onnxruntime.ai/docs/execution-providers/)
- [Burn Framework](https://lib.rs/crates/burn)
- [Candle GitHub](https://github.com/huggingface/candle)
- [augurs Time-Series Toolkit](https://github.com/grafana/augurs)
- [ruv-FANN](https://lib.rs/crates/ruv-fann)

### Raspberry Pi ML
- [Benchmarking TensorFlow Lite on Pi 5](https://www.hackster.io/news/benchmarking-tensorflow-and-tensorflow-lite-on-raspberry-pi-5-b9156d58a6a2)
- [Deep Learning with Raspberry Pi](https://qengineering.eu/deep-learning-with-raspberry-pi-and-alternatives.html)
- [Running AI Models on Pi 5](https://www.tech-reader.blog/2025/02/running-ai-models-on-raspberry-pi-5-8gb.html)

### Optimization Techniques
- [AI Model Quantization Guide 2025](https://local-ai-zone.github.io/guides/what-is-ai-quantization-q4-k-m-q8-gguf-guide-2025.html)
- [INT8 vs FP16 Tradeoffs](https://eureka.patsnap.com/article/what-is-quantization-int8-vs-fp16-tradeoffs-for-edge-ai-deployment)
- [Microsoft Low-bit Quantization](https://www.microsoft.com/en-us/research/blog/advances-to-low-bit-quantization-enable-llms-on-edge-devices/)

### Federated Learning
- [Federated Learning at the Edge](https://www.intechopen.com/online-first/1230198)
- [On-Device Learning](https://mlsysbook.ai/contents/core/ondevice_learning/ondevice_learning.html)
- [Adaptive Federated Learning for IoT](https://www.nature.com/articles/s41598-024-78239-z)

### Rust ML Ecosystem
- [Rust ML Framework Comparison 2025](https://markaicode.com/rust-machine-learning-framework-comparison-2025/)
- [Building Sentence Transformers in Rust](https://dev.to/mayu2008/building-sentence-transformers-in-rust-a-practical-guide-with-burn-onnx-runtime-and-candle-281k)
- [Rust for AI and ML 2025](https://andrewodendaal.com/rust-ai-machine-learning/)

---

**Document Version**: 1.0
**Status**: Complete
**Next Review**: After Phase 2 completion
