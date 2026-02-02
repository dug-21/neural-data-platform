# Unsupervised Learning for Edge Devices: Neural Data Platform Research

**Research Date:** 2026-02-02
**Platform Target:** Raspberry Pi 4 (4GB RAM, ARM Cortex-A72, 1.5GHz quad-core)
**Data Domain:** Time-series environmental data (air quality, weather sensors)
**Platform Stack:** Rust-based, potential ONNX Runtime integration

---

## Executive Summary

This research evaluates unsupervised learning techniques suitable for deployment on resource-constrained edge devices, specifically the Raspberry Pi 4 running the Neural Data Platform (NDP). The analysis covers anomaly detection, clustering, dimensionality reduction, and self-supervised learning approaches, with a focus on practical implementations that balance detection accuracy with computational efficiency.

### Key Recommendations

| Technique Category | Recommended Approach | Resource Impact | Use Case |
|-------------------|---------------------|-----------------|----------|
| **Anomaly Detection** | Statistical (Z-score/IQR) + Isolation Forest | Very Low / Low | Real-time sensor validation, DQ |
| **Clustering** | Mini-batch K-means + DBSCAN | Low | Pattern discovery, event grouping |
| **Dimensionality Reduction** | PCA (runtime) | Very Low | Feature compression, visualization |
| **Self-Supervised** | Contrastive learning (offline) | Medium | Representation learning for downstream tasks |
| **Online Learning** | ADWIN drift detection | Very Low | Concept drift monitoring |

### Priority Implementation Order

1. **Tier 1 (Immediate):** Statistical anomaly detection (Z-score, IQR, MAD)
2. **Tier 2 (Short-term):** Isolation Forest for multivariate anomaly detection
3. **Tier 3 (Medium-term):** DBSCAN clustering for pattern discovery
4. **Tier 4 (Long-term):** Quantized autoencoder for complex anomaly patterns

---

## 1. Anomaly Detection Techniques

### 1.1 Statistical Methods (Tier 1 - Immediate Priority)

Statistical methods are the most computationally efficient and should be the first line of defense for sensor data validation.

#### Z-Score Detection

**Description:** Measures how many standard deviations a data point is from the mean.

**Formula:**
```
z = (x - μ) / σ
```

**Implementation (Rust):**
```rust
/// Z-score based anomaly detection
pub struct ZScoreDetector {
    mean: f64,
    std_dev: f64,
    threshold: f64, // Typically 2.0-3.0
}

impl ZScoreDetector {
    pub fn new(threshold: f64) -> Self {
        Self { mean: 0.0, std_dev: 1.0, threshold }
    }

    pub fn fit(&mut self, data: &[f64]) {
        let n = data.len() as f64;
        self.mean = data.iter().sum::<f64>() / n;
        let variance = data.iter()
            .map(|x| (x - self.mean).powi(2))
            .sum::<f64>() / n;
        self.std_dev = variance.sqrt();
    }

    pub fn is_anomaly(&self, value: f64) -> bool {
        let z = (value - self.mean) / self.std_dev;
        z.abs() > self.threshold
    }
}
```

**Resource Requirements:**
- Memory: ~100 bytes per detector
- CPU: O(1) per prediction, O(n) for fitting
- Latency: <1ms

**Pros:**
- Extremely lightweight
- No external dependencies
- Easy to implement and interpret

**Cons:**
- Assumes normal distribution
- Sensitive to outliers in training data
- Single-variate only

**Best For:** PM2.5, CO2, temperature real-time validation

---

#### IQR (Interquartile Range) Detection

**Description:** Uses quartiles to identify outliers, more robust than Z-score for non-normal distributions.

**Formula:**
```
Lower bound = Q1 - 1.5 * IQR
Upper bound = Q3 + 1.5 * IQR
```

**Implementation (Rust):**
```rust
/// IQR-based anomaly detection (robust to outliers)
pub struct IqrDetector {
    q1: f64,
    q3: f64,
    iqr: f64,
    multiplier: f64, // Typically 1.5
}

impl IqrDetector {
    pub fn fit(&mut self, data: &mut [f64]) {
        data.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let n = data.len();
        self.q1 = data[n / 4];
        self.q3 = data[3 * n / 4];
        self.iqr = self.q3 - self.q1;
    }

    pub fn is_anomaly(&self, value: f64) -> bool {
        let lower = self.q1 - self.multiplier * self.iqr;
        let upper = self.q3 + self.multiplier * self.iqr;
        value < lower || value > upper
    }
}
```

**Resource Requirements:**
- Memory: ~100 bytes per detector
- CPU: O(n log n) for fitting (sorting), O(1) for prediction
- Latency: <1ms

**Pros:**
- Robust to outliers and non-normal distributions
- Uses median-based reference (more stable)
- Simple interpretation

**Cons:**
- Requires sorted data for fitting
- Single-variate only

**Best For:** Humidity, pressure, skewed sensor readings

---

#### Median Absolute Deviation (MAD)

**Description:** Robust measure using median instead of mean.

**Formula:**
```
MAD = median(|xi - median(x)|)
Modified Z-score = 0.6745 * (x - median) / MAD
```

**Resource Requirements:**
- Memory: ~200 bytes per detector
- CPU: O(n log n) for fitting
- Latency: <1ms

**Best For:** Data with heavy-tailed distributions

---

#### Moving Window Statistical Detection

**Description:** Applies statistical methods over sliding windows for streaming data.

**Implementation Pattern:**
```rust
/// Moving Z-score for streaming time series
pub struct MovingZScore {
    window: VecDeque<f64>,
    window_size: usize,
    threshold: f64,
}

impl MovingZScore {
    pub fn add_and_check(&mut self, value: f64) -> bool {
        self.window.push_back(value);
        if self.window.len() > self.window_size {
            self.window.pop_front();
        }

        if self.window.len() < self.window_size / 2 {
            return false; // Not enough data
        }

        let mean: f64 = self.window.iter().sum::<f64>() / self.window.len() as f64;
        let variance: f64 = self.window.iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / self.window.len() as f64;
        let std_dev = variance.sqrt();

        let z = (value - mean) / std_dev.max(0.0001);
        z.abs() > self.threshold
    }
}
```

**Resource Requirements (1-hour window @ 1-minute intervals):**
- Memory: ~500 bytes per sensor
- CPU: O(window_size) per update
- Latency: <5ms

---

### 1.2 Isolation Forest (Tier 2 - Short-term Priority)

**Description:** Tree-based anomaly detection that isolates anomalies by randomly partitioning the feature space. Anomalies require fewer partitions to isolate.

**Key Characteristics:**
- Unsupervised (no labels required)
- Handles multivariate data
- Linear time complexity O(n)
- Memory efficient

**Research Findings:**
- Isolation Forest consistently outperformed One-Class SVM for IoT anomaly detection with higher detection accuracy, superior precision and recall, and significantly better F1-score ([Nature Scientific Reports, 2025](https://www.nature.com/articles/s41598-025-20445-4))
- Achieves <50ms latency with <160KB memory footprint on microcontrollers ([ArXiv, 2024](https://arxiv.org/html/2512.19383))
- Works well without labeled data

**Rust Implementation (linfa):**
```rust
use linfa::prelude::*;
use linfa_trees::IsolationForest;

fn train_isolation_forest(data: &Array2<f64>) -> Result<IsolationForest> {
    let dataset = Dataset::new(data.clone(), Array1::zeros(data.nrows()));

    let model = IsolationForest::params()
        .n_trees(100)           // Number of trees (100-256 typical)
        .max_samples(256)       // Subsample size
        .contamination(0.05)    // Expected anomaly ratio
        .fit(&dataset)?;

    Ok(model)
}

fn predict_anomalies(model: &IsolationForest, data: &Array2<f64>) -> Vec<bool> {
    let scores = model.predict(data);
    scores.iter().map(|&s| s < 0.0).collect() // Negative scores = anomaly
}
```

**Resource Requirements (Pi 4):**
- Memory: 5-50 MB (depends on n_trees, subsample size)
- Training: 1-5 seconds for 10K samples
- Inference: 10-50ms per sample
- Model size: 1-10 MB (serialized)

**Optimization for Edge:**
```rust
// Edge-optimized parameters
IsolationForest::params()
    .n_trees(50)          // Reduced from 100 (slight accuracy trade-off)
    .max_samples(128)     // Reduced subsample (faster training)
    .max_depth(8)         // Limit tree depth
```

**Pros:**
- Multivariate anomaly detection
- No distribution assumptions
- Handles mixed feature types
- Rust-native implementation available (linfa, smartcore)

**Cons:**
- Higher memory than statistical methods
- Training required
- Less interpretable than statistical methods

**Best For:** Multi-sensor anomaly detection (PM2.5 + CO2 + temperature combined)

---

### 1.3 One-Class SVM (Tier 2 - Alternative)

**Description:** Learns a boundary around normal data points using support vectors.

**Research Findings:**
- OCSVM achieves superior precision in some IoT security contexts compared to Isolation Forest ([Nature, 2025](https://www.nature.com/articles/s41598-025-20445-4))
- Requires more computational resources than Isolation Forest
- Better for smaller datasets with clear boundaries

**Rust Implementation (smartcore):**
```rust
use smartcore::svm::one_class_svm::OneClassSVM;
use smartcore::linalg::basic::matrix::DenseMatrix;

fn train_ocsvm(data: &DenseMatrix<f64>) -> OneClassSVM<f64, DenseMatrix<f64>> {
    OneClassSVM::fit(
        data,
        SmartCoreKernelParameters::default()
            .with_kernel(Kernel::RBF(0.5))
            .with_nu(0.05) // Expected anomaly ratio
    ).unwrap()
}
```

**Resource Requirements (Pi 4):**
- Memory: 10-100 MB (depends on support vectors)
- Training: 10-60 seconds for 10K samples
- Inference: 5-20ms per sample

**Pros:**
- Good for non-linear decision boundaries
- Well-understood theoretical properties

**Cons:**
- Slower training than Isolation Forest
- Kernel selection can be tricky
- Scales poorly with dataset size O(n^2 - n^3)

**Best For:** Smaller datasets with clear normal/anomaly separation

---

### 1.4 Autoencoders (Tier 4 - Long-term)

**Description:** Neural network that learns to compress and reconstruct normal data. High reconstruction error indicates anomaly.

**Research Findings:**
- LSTM-AE achieves up to 93.6% detection accuracy for IoT sensor data ([MDPI Sensors](https://www.mdpi.com/1424-8220/21/14/4946))
- Quantized autoencoders can run on ARM Cortex-M4 with 256KB SRAM ([ResearchGate](https://www.researchgate.net/publication/364182863_An_Auto-Encoder_Based_TinyML_Approach_for_Real-Time_Anomaly_Detection))
- OutlierNets achieve 2.7KB model size with 686 parameters
- Quantization reduces inference time by 76% and power consumption by 35%

**Architecture Options:**

**Option A: Dense Autoencoder (Simple)**
```
Input (6 features) -> Dense(16) -> Dense(4) -> Dense(16) -> Output (6 features)
```

**Option B: LSTM Autoencoder (Temporal)**
```
Input (sequence) -> LSTM(32) -> RepeatVector -> LSTM(32) -> TimeDistributed(Dense)
```

**Quantized Implementation Approach:**
```rust
// Using ONNX Runtime for quantized inference
use ort::{Environment, Session, Value};

async fn load_quantized_autoencoder() -> Session {
    let env = Environment::builder().build()?;
    Session::builder()?
        .with_optimization_level(GraphOptimizationLevel::All)?
        .with_model_from_file("autoencoder_int8.onnx")?
        .build()
}

fn detect_anomaly(session: &Session, input: &[f64], threshold: f64) -> bool {
    let input_tensor = Value::from_array(input.to_vec())?;
    let outputs = session.run(vec![input_tensor])?;
    let reconstruction: Vec<f64> = outputs[0].try_extract()?;

    // Compute reconstruction error (MSE)
    let mse: f64 = input.iter()
        .zip(reconstruction.iter())
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f64>() / input.len() as f64;

    mse > threshold
}
```

**Resource Requirements (Pi 4, quantized INT8):**
- Model size: 10-50 KB (quantized)
- Memory: 50-200 MB (with ONNX Runtime)
- Inference: 5-50ms per sample
- Training: Offline (not on Pi)

**Optimization Techniques:**
1. **Quantization:** FP32 -> INT8 (4x size reduction, 2-4x speedup)
2. **Pruning:** Remove near-zero weights (50-90% sparsity)
3. **Knowledge distillation:** Train smaller model to mimic larger one

**Pros:**
- Captures complex non-linear patterns
- Handles high-dimensional data
- Can learn temporal dependencies (LSTM variant)

**Cons:**
- Requires offline training
- Needs ONNX Runtime or similar
- Black-box (less interpretable)
- Higher resource requirements

**Best For:** Complex temporal anomaly patterns, multi-sensor fusion

---

### 1.5 Anomaly Detection Comparison Matrix

| Method | Memory | Latency | Accuracy | Multivariate | Online | Interpretable |
|--------|--------|---------|----------|--------------|--------|---------------|
| Z-score | <1 KB | <1ms | Medium | No | Yes | High |
| IQR | <1 KB | <1ms | Medium | No | Yes | High |
| MAD | <1 KB | <1ms | Medium | No | Yes | High |
| Moving Z-score | 1-5 KB | <5ms | Medium-High | No | Yes | High |
| Isolation Forest | 5-50 MB | 10-50ms | High | Yes | No | Low |
| One-Class SVM | 10-100 MB | 5-20ms | High | Yes | No | Low |
| Autoencoder (Q) | 50-200 MB | 5-50ms | Very High | Yes | No | Very Low |

---

## 2. Clustering Techniques

### 2.1 K-Means and Mini-batch K-Means

**Description:** Partitions data into k clusters by minimizing within-cluster variance.

**Mini-batch Variant:** Processes random subsets for faster convergence, ideal for streaming.

**Rust Implementation (linfa):**
```rust
use linfa::prelude::*;
use linfa_clustering::KMeans;

fn cluster_sensor_patterns(data: &Array2<f64>, k: usize) -> KMeans {
    KMeans::params(k)
        .max_n_iterations(100)
        .tolerance(1e-4)
        .fit(&Dataset::new(data.clone(), Array1::zeros(data.nrows())))
        .expect("KMeans fitting failed")
}

// Mini-batch for streaming
fn incremental_cluster(
    model: &mut KMeans,
    batch: &Array2<f64>,
) {
    model.partial_fit(batch);
}
```

**Resource Requirements (Pi 4):**
- Memory: 1-10 MB (depends on k and dimensions)
- Training: 100ms - 2s for 10K samples
- Inference: <1ms per sample

**Use Cases:**
- Grouping similar pollution events
- Identifying hourly/daily patterns
- Segmenting sensor behavior modes

**Pros:**
- Fast and memory efficient
- Mini-batch supports incremental updates
- Well-understood algorithm

**Cons:**
- Requires specifying k
- Assumes spherical clusters
- Sensitive to initialization

---

### 2.2 DBSCAN

**Description:** Density-based clustering that finds arbitrarily shaped clusters and identifies noise/outliers.

**Research Findings:**
- DBSCAN is well-suited for IoT time-series with varying densities
- Integrated into augurs (Grafana's Rust time-series toolkit) for production monitoring
- Standard O(N^2) complexity, but can be optimized with spatial indexing

**Rust Implementation (linfa):**
```rust
use linfa::prelude::*;
use linfa_clustering::Dbscan;

fn cluster_events(data: &Array2<f64>) -> Vec<Option<usize>> {
    let model = Dbscan::params(2)      // min_samples
        .tolerance(0.5)                 // eps (neighborhood radius)
        .transform(&Dataset::new(data.clone(), ()))
        .expect("DBSCAN failed");

    // Returns cluster labels (None = noise/outlier)
    model.targets().to_vec()
}
```

**Resource Requirements (Pi 4):**
- Memory: O(n^2) distance matrix, or O(n) with approximation
- Training: 1-10 seconds for 10K samples
- No separate inference (labels assigned during fit)

**Parameters:**
- `eps`: Maximum distance for neighborhood
- `min_samples`: Minimum points to form a cluster

**Pros:**
- Finds arbitrary cluster shapes
- Automatic outlier detection
- No need to specify cluster count

**Cons:**
- Sensitive to eps parameter
- O(n^2) complexity (without indexing)
- Not incremental

**Best For:** Pollution event clustering, identifying anomalous periods

---

### 2.3 HDBSCAN (Offline Analysis)

**Description:** Hierarchical DBSCAN that finds clusters of varying densities without requiring eps parameter.

**Limitations for Edge:**
- Requires full dataset access (not streaming)
- Higher memory requirements
- Better suited for offline analysis

**Recommendation:** Use for offline exploration, not real-time edge inference.

---

### 2.4 Online/Incremental Clustering (DenStream)

**Description:** Stream-oriented density-based clustering for continuous data.

**Research Findings:**
- DenStream is positioned as the most promising online alternative to HDBSCAN ([ArXiv, 2025](https://arxiv.org/html/2601.20680))
- Addresses concept drift by gradually forgetting old data
- Supports arbitrary cluster shapes

**Conceptual Implementation:**
```rust
/// Online density-based clustering
pub struct DenStream {
    potential_micro_clusters: Vec<MicroCluster>,
    outlier_micro_clusters: Vec<MicroCluster>,
    lambda: f64,  // Decay factor
    beta: f64,    // Outlier threshold
    mu: f64,      // Weight threshold
}

impl DenStream {
    pub fn process_point(&mut self, point: &[f64], timestamp: u64) {
        // 1. Try to merge with existing potential micro-cluster
        // 2. If not possible, try outlier micro-clusters
        // 3. Create new outlier micro-cluster
        // 4. Periodically check if outliers become potential clusters
        // 5. Decay old micro-clusters based on timestamp
    }

    pub fn get_macro_clusters(&self) -> Vec<Vec<usize>> {
        // Run DBSCAN on micro-cluster centers
        dbscan_on_centroids(&self.potential_micro_clusters)
    }
}
```

**Resource Requirements:**
- Memory: O(micro-clusters), typically 1-10 MB
- Update: O(micro-clusters) per point
- Macro-clustering: Periodic, O(mc^2)

**Best For:** Continuous sensor streams, evolving patterns

---

### 2.5 Time-Series Specific Clustering

#### k-Shape

**Description:** K-means variant using shape-based distance (cross-correlation) for time series.

**Use Case:** Finding similar daily/weekly air quality patterns.

#### DTW-Based Clustering

**Description:** Uses Dynamic Time Warping distance for time-warped similarity.

**Research Findings:**
- DTW clustering can be 10x slower than Euclidean
- Pruning strategies can achieve order-of-magnitude speedup ([ResearchGate](https://www.researchgate.net/publication/311411265_A_General_Framework_for_Density_Based_Time_Series_Clustering_Exploiting_a_Novel_Admissible_Pruning_Strategy))

**Recommendation:** Use for offline pattern analysis, not real-time.

---

### 2.6 Clustering Comparison Matrix

| Method | Memory | Speed | Streaming | Arbitrary Shapes | Outlier Detection |
|--------|--------|-------|-----------|------------------|-------------------|
| K-Means | Low | Fast | Mini-batch | No | No |
| DBSCAN | Medium | Medium | No | Yes | Yes |
| HDBSCAN | High | Slow | No | Yes | Yes |
| DenStream | Medium | Fast | Yes | Yes | Yes |
| k-Shape | Medium | Medium | No | No | No |

---

## 3. Dimensionality Reduction

### 3.1 PCA (Principal Component Analysis)

**Description:** Linear projection that preserves maximum variance.

**Research Findings:**
- PCA projection takes microseconds (matrix multiply)
- Ideal for real-time edge inference
- Reduces memory by 3-6x while maintaining 90%+ reconstruction

**Rust Implementation (linfa):**
```rust
use linfa::prelude::*;
use linfa_reduction::Pca;

fn fit_pca(data: &Array2<f64>, n_components: usize) -> Pca<f64> {
    Pca::params(n_components)
        .fit(&Dataset::new(data.clone(), ()))
        .expect("PCA fitting failed")
}

fn transform(pca: &Pca<f64>, data: &Array2<f64>) -> Array2<f64> {
    pca.transform(data)
}

// Inverse transform for reconstruction error anomaly detection
fn reconstruction_error(pca: &Pca<f64>, sample: &Array1<f64>) -> f64 {
    let reduced = pca.transform(&sample.view().insert_axis(Axis(0)));
    let reconstructed = pca.inverse_transform(&reduced);

    sample.iter()
        .zip(reconstructed.iter())
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f64>()
        .sqrt()
}
```

**Resource Requirements (Pi 4):**
- Memory: ~1 MB for typical sensor data
- Training: 100ms for 10K samples
- Inference: <1ms per sample

**Use Cases:**
1. Feature compression before ML models
2. Anomaly detection via reconstruction error
3. Visualization of high-dimensional sensor data

**Pros:**
- Very fast inference
- Well-understood mathematics
- Rust-native implementation

**Cons:**
- Linear only
- Sensitive to feature scaling
- May lose non-linear structure

---

### 3.2 UMAP (Offline Only)

**Description:** Non-linear manifold learning that preserves local and global structure.

**Research Findings:**
- UMAP has no native inference (requires retraining or learned approximation)
- Better for offline analysis and visualization
- Rust implementation available: `annembed` crate

**Recommendation:**
- Use UMAP for offline data exploration
- Use PCA for real-time edge inference

---

### 3.3 Autoencoder-Based Compression

**Description:** Neural network learns non-linear compression to bottleneck layer.

**Architecture:**
```
Input (10 features) -> Dense(8) -> Dense(3) -> Dense(8) -> Output (10 features)
                                    ^
                              Compressed representation
```

**Use Case:** Compress high-dimensional sensor readings for transmission or storage.

**Resource Requirements:**
- Model: 10-100 KB (quantized)
- Inference: 5-20ms

---

## 4. Self-Supervised Learning

### 4.1 Contrastive Learning for Time Series

**Description:** Learns representations by distinguishing similar vs. dissimilar time series segments.

**Research Findings:**
- Self-supervised learning reduces dependence on labeled data ([IEEE TPAMI, 2024](https://ieeexplore.ieee.org/document/10496248/))
- CARLA framework uses anomaly injection for contrastive learning ([ScienceDirect](https://www.sciencedirect.com/science/article/pii/S0031320324006253))
- TS2Vec produces fixed-length embeddings for clustering

**Key Techniques:**

#### Data Augmentation for Time Series
- **Jittering:** Add small noise
- **Scaling:** Multiply by random factor
- **Permutation:** Shuffle segments
- **Crop-and-resize:** Random subsequence scaling
- **Random smoothing:** Apply moving average

#### Contrastive Loss
```rust
// Simplified NT-Xent loss concept
fn contrastive_loss(
    anchor: &[f64],
    positive: &[f64],  // Augmented version of anchor
    negatives: &[&[f64]], // Other samples
    temperature: f64,
) -> f64 {
    let sim_pos = cosine_similarity(anchor, positive) / temperature;
    let sim_negs: Vec<f64> = negatives.iter()
        .map(|n| cosine_similarity(anchor, n) / temperature)
        .collect();

    -sim_pos + log_sum_exp(&sim_negs)
}
```

**Edge Deployment Strategy:**
1. Train encoder model offline (desktop/cloud)
2. Export encoder to ONNX (quantized)
3. Deploy encoder on Pi for feature extraction
4. Use extracted features for clustering/anomaly detection

**Resource Requirements (Pi 4, inference only):**
- Model: 50-500 KB (quantized encoder)
- Inference: 10-50ms per window
- Training: Offline only

---

### 4.2 Masked Autoencoding

**Description:** Learns by reconstructing masked portions of input sequences.

**Approach:**
1. Mask random portions of time series (e.g., 15%)
2. Train model to reconstruct masked values
3. Use encoder for downstream tasks

**Advantage:** Learns temporal dependencies naturally.

**Limitation:** Requires significant compute for training.

---

### 4.3 Self-Supervised Implementation Roadmap

**Phase 1 (Offline):**
- Train TS2Vec or similar encoder on historical data
- Validate learned representations on held-out set

**Phase 2 (Export):**
- Convert encoder to ONNX
- Apply INT8 quantization
- Optimize for ARM inference

**Phase 3 (Edge Deployment):**
- Load quantized encoder on Pi
- Extract embeddings in real-time
- Apply clustering/anomaly detection on embeddings

---

## 5. Lightweight Implementations for Raspberry Pi

### 5.1 Native Rust Libraries

| Library | Algorithms | Production Ready | Notes |
|---------|------------|------------------|-------|
| **linfa** | PCA, K-Means, DBSCAN, Gaussian Mixture | Yes | scikit-learn equivalent |
| **smartcore** | K-Means, DBSCAN, SVM, Random Forest | Yes | Comprehensive ML library |
| **augurs** | ETS, MSTL, DBSCAN (outlier), Prophet | Partial | Grafana's time-series toolkit |
| **ndarray-stats** | Statistical methods | Yes | Extends ndarray |

**Linfa Example (Cargo.toml):**
```toml
[dependencies]
linfa = "0.7"
linfa-clustering = "0.7"
linfa-reduction = "0.7"
ndarray = "0.15"
```

**SmartCore Example:**
```toml
[dependencies]
smartcore = "0.4"
```

---

### 5.2 ONNX Runtime for ARM64

**Description:** Cross-platform ML inference engine supporting quantized models.

**Research Findings:**
- ONNX Runtime provides cross-platform support for ARM64 ([ONNX Runtime Docs](https://onnxruntime.ai/docs/))
- Static quantization recommended for Pi (dynamic adds overhead) ([Medium](https://medium.com/@connect.hashblock/real-time-ai-on-the-edge-deploying-a-quantized-llm-on-raspberry-pi-with-onnx-ea7fba9d0826))
- Production deployments running on Raspberry Pi CM4 at Hoomanely ([Tech Hoomanely](https://tech.hoomanely.com/onnx-runtime-the-engine-behind-fast-and-flexible-ai-inference/))

**Setup:**
```toml
[dependencies]
ort = { version = "2.0", features = ["load-dynamic"] }
```

**Inference Example:**
```rust
use ort::{Environment, Session, SessionBuilder, Value};

fn load_model(model_path: &str) -> Session {
    let env = Environment::builder()
        .with_name("ndp_inference")
        .build()
        .unwrap()
        .into_arc();

    SessionBuilder::new(&env)
        .unwrap()
        .with_optimization_level(GraphOptimizationLevel::All)
        .with_model_from_file(model_path)
        .unwrap()
}

fn run_inference(session: &Session, input: Vec<f32>) -> Vec<f32> {
    let input_tensor = Value::from_array(ndarray::arr1(&input).into_dyn())
        .unwrap();

    let outputs = session.run(vec![input_tensor]).unwrap();
    outputs[0].try_extract::<f32>().unwrap().view().to_vec()
}
```

**Resource Requirements:**
- ONNX Runtime library: ~50 MB
- Quantized models: 10 KB - 10 MB
- Inference: 5-100ms depending on model

---

### 5.3 Model Optimization Techniques

#### Quantization

**INT8 Static Quantization:**
```python
# Python (offline conversion)
import onnxruntime.quantization as quantization

quantization.quantize_static(
    model_input='model_fp32.onnx',
    model_output='model_int8.onnx',
    calibration_data_reader=CalibrationDataReader(data),
    quant_format=quantization.QuantFormat.QDQ,
    weight_type=quantization.QuantType.QInt8,
    activation_type=quantization.QuantType.QUInt8,
)
```

**Benefits:**
- 4x model size reduction
- 2-4x inference speedup
- Minimal accuracy loss (typically <1%)

#### Pruning

**Structured Pruning:**
- Remove entire channels/filters
- Achieves 50-90% sparsity
- Direct speedup without special hardware

#### Knowledge Distillation

**Teacher-Student Training:**
1. Train large "teacher" model offline
2. Train small "student" to mimic teacher
3. Deploy student on edge

---

### 5.4 Memory Budget Guidelines

**Raspberry Pi 4 (4GB RAM) Budget:**

| Component | Allocation |
|-----------|------------|
| OS + Services | 500 MB |
| TimescaleDB | 1.0 GB |
| Rust Application | 200 MB |
| **ML Models** | **500 MB** |
| Data Buffers | 500 MB |
| Headroom | 1.3 GB |

**Per-Model Guidelines:**
- Statistical detectors: <1 KB each
- Isolation Forest: <50 MB
- Quantized autoencoder: <10 MB
- ONNX Runtime overhead: ~50 MB

---

## 6. Recommendations by Use Case

### 6.1 Real-Time Sensor Validation (Data Quality)

**Recommended Stack:**
1. Moving Z-score/IQR per sensor (Tier 1)
2. Range checks (min/max bounds)
3. Rate-of-change limits

**Implementation:**
```rust
pub struct SensorValidator {
    zscore: MovingZScore,
    min_value: f64,
    max_value: f64,
    max_rate: f64, // per minute
    last_value: Option<(f64, Instant)>,
}

impl SensorValidator {
    pub fn validate(&mut self, value: f64) -> ValidationResult {
        // 1. Range check
        if value < self.min_value || value > self.max_value {
            return ValidationResult::OutOfRange;
        }

        // 2. Rate-of-change check
        if let Some((last, time)) = self.last_value {
            let dt = time.elapsed().as_secs_f64() / 60.0;
            let rate = (value - last).abs() / dt;
            if rate > self.max_rate {
                return ValidationResult::RateExceeded;
            }
        }

        // 3. Statistical anomaly check
        if self.zscore.add_and_check(value) {
            return ValidationResult::StatisticalAnomaly;
        }

        self.last_value = Some((value, Instant::now()));
        ValidationResult::Valid
    }
}
```

**Resource Impact:** Negligible (<1MB total, <1ms per check)

---

### 6.2 Multi-Sensor Anomaly Detection

**Recommended Stack:**
1. Isolation Forest for multivariate detection
2. Retrain weekly on recent data
3. Fallback to per-sensor Z-score

**Implementation:**
```rust
pub struct MultiSensorAnomalyDetector {
    isolation_forest: Option<IsolationForest>,
    per_sensor_detectors: Vec<MovingZScore>,
    last_training: Instant,
    training_interval: Duration,
}

impl MultiSensorAnomalyDetector {
    pub async fn detect(&mut self, readings: &SensorReadings) -> AnomalyResult {
        // Primary: Isolation Forest (if trained)
        if let Some(ref model) = self.isolation_forest {
            let features = readings.to_feature_vector();
            if model.is_anomaly(&features) {
                return AnomalyResult::Anomaly {
                    method: "isolation_forest",
                    confidence: model.anomaly_score(&features),
                };
            }
        }

        // Fallback: Per-sensor statistical
        for (i, detector) in self.per_sensor_detectors.iter_mut().enumerate() {
            if detector.is_anomaly(readings.values[i]) {
                return AnomalyResult::Anomaly {
                    method: "statistical",
                    sensor_index: i,
                };
            }
        }

        // Check if retraining needed
        if self.last_training.elapsed() > self.training_interval {
            self.schedule_retraining();
        }

        AnomalyResult::Normal
    }
}
```

**Resource Impact:** 50-100 MB, 20-50ms per detection

---

### 6.3 Pattern Discovery (Offline/Periodic)

**Recommended Stack:**
1. Extract daily/hourly feature vectors
2. Apply DBSCAN for pattern clustering
3. Store cluster centroids as "typical patterns"
4. Compare new data to known patterns

**Workflow:**
```rust
pub async fn discover_patterns(data: &[HourlyFeatures]) -> Vec<Pattern> {
    // 1. Normalize features
    let normalized = normalize(data);

    // 2. Run DBSCAN
    let labels = dbscan(&normalized, eps: 0.3, min_samples: 5);

    // 3. Extract cluster centroids
    let patterns: Vec<Pattern> = unique_labels(&labels)
        .filter(|l| *l >= 0) // Exclude noise
        .map(|label| {
            let members: Vec<_> = data.iter()
                .zip(labels.iter())
                .filter(|(_, l)| **l == label)
                .map(|(d, _)| d)
                .collect();

            Pattern {
                centroid: compute_centroid(&members),
                count: members.len(),
                label: format!("pattern_{}", label),
            }
        })
        .collect();

    patterns
}
```

**Scheduling:** Run nightly (2-3 AM) or after significant data accumulation

---

### 6.4 Concept Drift Detection

**Recommended Stack:**
1. ADWIN algorithm for drift detection
2. Trigger model retraining on drift
3. EWC++ for incremental updates (prevents forgetting)

**Implementation:**
```rust
use std::collections::VecDeque;

pub struct AdwinDriftDetector {
    window: VecDeque<f64>,
    delta: f64, // Confidence parameter
    min_window: usize,
}

impl AdwinDriftDetector {
    pub fn add(&mut self, value: f64) -> bool {
        self.window.push_back(value);

        if self.window.len() < self.min_window * 2 {
            return false;
        }

        // Check all possible split points
        for split in self.min_window..(self.window.len() - self.min_window) {
            let (left, right) = self.window.as_slices();
            let (w0, w1) = if split < left.len() {
                (&left[..split], &left[split..])
            } else {
                (left, &right[..(split - left.len())])
            };

            let mean0 = mean(w0);
            let mean1 = mean(w1);
            let n0 = w0.len() as f64;
            let n1 = w1.len() as f64;

            // Hoeffding bound
            let eps = hoeffding_bound(n0, n1, self.delta);

            if (mean0 - mean1).abs() > eps {
                // Drift detected! Drop old data
                for _ in 0..split {
                    self.window.pop_front();
                }
                return true;
            }
        }

        false
    }
}

fn hoeffding_bound(n0: f64, n1: f64, delta: f64) -> f64 {
    let m = 1.0 / (1.0 / n0 + 1.0 / n1);
    (2.0 / m * (4.0 / delta).ln()).sqrt()
}
```

**Integration:**
```rust
// Monitor prediction error for drift
let mut drift_detector = AdwinDriftDetector::new(delta: 0.001);

loop {
    let prediction = model.predict(&current_features);
    let actual = await_actual_value().await;
    let error = (prediction - actual).abs();

    if drift_detector.add(error) {
        log::warn!("Concept drift detected! Scheduling retraining...");
        schedule_retraining(&model, &recent_data).await;
    }

    tokio::time::sleep(Duration::from_secs(60)).await;
}
```

---

## 7. Novel Approaches for Constrained Environments

### 7.1 Hybrid Statistical-ML Pipeline

**Concept:** Use fast statistical methods as first filter, expensive ML only when needed.

```
Sensor Reading
      |
      v
[Statistical Check] --pass--> [Store Normally]
      |
      | flagged
      v
[Isolation Forest] --normal--> [Store with warning]
      |
      | anomaly confirmed
      v
[Alert + Detailed Analysis]
```

**Benefits:**
- 95%+ of readings processed with <1ms latency
- ML only invoked for suspicious cases
- Significant CPU/power savings

---

### 7.2 Federated Learning for Multi-Device Deployments

**Concept:** Train shared model across multiple Pis without centralizing data.

**Workflow:**
1. Each Pi trains local model on its data
2. Periodically share model weights (not data)
3. Aggregate weights on coordinator
4. Distribute updated global model

**Benefits:**
- Privacy-preserving
- Leverages diverse data sources
- No central data collection

**Implementation Consideration:** Requires coordination infrastructure.

---

### 7.3 Temporal Compression for Efficient Storage

**Concept:** Store compressed representations instead of raw data.

```rust
pub struct TemporalCompressor {
    pca: Pca<f64>,
    window_size: usize,
}

impl TemporalCompressor {
    pub fn compress(&self, window: &[SensorReading]) -> CompressedWindow {
        // Extract features from window
        let features = extract_temporal_features(window);

        // PCA compression
        let compressed = self.pca.transform(&features);

        CompressedWindow {
            start_time: window.first().unwrap().timestamp,
            end_time: window.last().unwrap().timestamp,
            compressed_features: compressed.to_vec(),
            summary_stats: compute_summary(window),
        }
    }
}
```

**Storage Savings:** 10-100x compression ratio

---

### 7.4 Adaptive Model Complexity

**Concept:** Dynamically adjust model complexity based on available resources.

```rust
pub enum ModelTier {
    Statistical,    // Z-score, IQR
    LightML,        // Small Isolation Forest
    FullML,         // Full model ensemble
}

pub fn select_model_tier(system_state: &SystemState) -> ModelTier {
    if system_state.cpu_load > 0.8 {
        ModelTier::Statistical
    } else if system_state.available_memory < 100_000_000 {
        ModelTier::LightML
    } else {
        ModelTier::FullML
    }
}
```

---

## 8. Implementation Roadmap

### Phase 1: Statistical Foundation (Week 1-2)

**Deliverables:**
- [ ] Implement MovingZScore, MovingIQR, MovingMAD
- [ ] Integrate with existing Bronze->Silver pipeline
- [ ] Add per-sensor DQ flags
- [ ] TimescaleDB storage for DQ results

**Resource Requirements:**
- Development: 16-24 hours
- Runtime: <1MB, <1ms per reading

---

### Phase 2: Isolation Forest Integration (Week 3-4)

**Deliverables:**
- [ ] Integrate linfa Isolation Forest
- [ ] Training pipeline (nightly cron)
- [ ] Model persistence (safetensors)
- [ ] Multi-sensor anomaly API

**Resource Requirements:**
- Development: 24-32 hours
- Runtime: 50MB, 20-50ms per detection
- Storage: 5-10MB per model

---

### Phase 3: Clustering for Pattern Discovery (Week 5-6)

**Deliverables:**
- [ ] DBSCAN integration for event clustering
- [ ] Pattern extraction and storage
- [ ] Pattern matching for new data
- [ ] Grafana visualization

**Resource Requirements:**
- Development: 24-32 hours
- Runtime: 50-100MB for analysis
- Storage: 1-5MB for patterns

---

### Phase 4: Online Learning & Drift Detection (Week 7-8)

**Deliverables:**
- [ ] ADWIN drift detector implementation
- [ ] Automated retraining trigger
- [ ] Model versioning and rollback
- [ ] Performance monitoring

**Resource Requirements:**
- Development: 32-40 hours
- Runtime: <10MB for drift detection

---

### Phase 5: Neural Models (Future)

**Deliverables:**
- [ ] Quantized autoencoder (ONNX)
- [ ] Self-supervised encoder
- [ ] Feature extraction pipeline
- [ ] Advanced temporal anomaly detection

**Resource Requirements:**
- Development: 40+ hours
- Runtime: 100-200MB
- Requires: ONNX Runtime integration

---

## 9. References

### Research Papers & Articles

- [Analysis of Machine Learning Algorithms for Anomaly Detection on Edge Devices](https://www.mdpi.com/1424-8220/21/14/4946) - MDPI Sensors, 2021
- [Edge AI for Real-Time Anomaly Detection in Smart Homes](https://www.mdpi.com/1999-5903/17/4/179) - MDPI Future Internet, 2025
- [Real-Time Machine Learning for Embedded Anomaly Detection](https://arxiv.org/html/2512.19383) - ArXiv, 2024
- [Robust IoT security using isolation forest and one class SVM algorithms](https://www.nature.com/articles/s41598-025-20445-4) - Nature Scientific Reports, 2025
- [Self-Supervised Learning for Time Series Analysis](https://ieeexplore.ieee.org/document/10496248/) - IEEE TPAMI, 2024
- [CARLA: Self-supervised contrastive representation learning for time series anomaly detection](https://www.sciencedirect.com/science/article/pii/S0031320324006253) - Pattern Recognition, 2024
- [Comprehensive review of dimensionality reduction algorithms](https://pmc.ncbi.nlm.nih.gov/articles/PMC12453773/) - PMC, 2025
- [Online Density-Based Clustering for Real-Time Narrative Evolution Monitoring](https://arxiv.org/html/2601.20680) - ArXiv, 2025
- [Time Series Clustering Using DBSCAN](https://arxiv.org/abs/2403.14798) - ArXiv, 2024
- [Detecting Time Series Anomalies: Moving Z-Score vs. Moving IQR](https://medium.com/@kis.andras.nandor/detecting-time-series-anomalies-moving-z-score-vs-moving-iqr-70754d853105) - Medium, 2025
- [Lightweight Signal Processing and Edge AI for Real-Time Anomaly Detection](https://www.mdpi.com/1424-8220/25/21/6629) - MDPI Sensors, 2025
- [An Auto-Encoder Based TinyML Approach for Real-Time Anomaly Detection](https://www.researchgate.net/publication/364182863_An_Auto-Encoder_Based_TinyML_Approach_for_Real-Time_Anomaly_Detection) - ResearchGate, 2022
- [Tiny Machine Learning and On-Device Inference: A Survey](https://pmc.ncbi.nlm.nih.gov/articles/PMC12115890/) - PMC, 2025

### Rust Libraries

- [linfa](https://github.com/rust-ml/linfa) - Rust machine learning framework (K-Means, DBSCAN, PCA)
- [smartcore](https://smartcorelib.org/) - Comprehensive ML library (SVM, Random Forest, DBSCAN)
- [augurs](https://github.com/grafana/augurs) - Grafana's time-series toolkit (ETS, MSTL, DBSCAN)
- [annembed](https://lib.rs/crates/annembed) - Rust UMAP implementation
- [ndarray-stats](https://crates.io/crates/ndarray-stats) - Statistical extensions for ndarray

### ONNX & Edge Inference

- [ONNX Runtime Raspberry Pi Tutorial](https://onnxruntime.ai/docs/tutorials/iot-edge/rasp-pi-cv.html)
- [ONNX Runtime: The Engine Behind Fast and Flexible AI Inference](https://tech.hoomanely.com/onnx-runtime-the-engine-behind-fast-and-flexible-ai-inference/)
- [Real-Time AI on the Edge: Deploying a Quantized LLM on Raspberry Pi with ONNX](https://medium.com/@connect.hashblock/real-time-ai-on-the-edge-deploying-a-quantized-llm-on-raspberry-pi-with-onnx-ea7fba9d0826)
- [AI Inference Acceleration with Python ONNX Runtime](https://johal.in/ai-inference-acceleration-with-python-onnx-runtime-deploying-models-on-edge-devices-2025/)

### Related NDP Research

- [Rust ML Frameworks Research](/workspaces/neural-data-platform/product/research/03-rust-ml-frameworks.md)
- [ML Feature Engineering Research](/workspaces/neural-data-platform/product/research/Silver/ml-feature-engineering.md)
- [Technology Selection Guide](/workspaces/neural-data-platform/product/research/07-technology-selection.md)

---

**Document Version:** 1.0
**Author:** Research Agent (Claude)
**Status:** Complete
**Last Updated:** 2026-02-02
