# Lightweight Causal Discovery Algorithms for Edge Devices

**Research Date:** 2026-02-02
**Research Focus:** Causal discovery algorithms feasible for Raspberry Pi and edge deployment
**Target Platform:** Raspberry Pi 4/5 (4-16GB RAM, ARM Cortex-A72/A76)
**Variables:** 10-50 sensors/indicators
**Status:** Research Complete

---

## Executive Summary

This research evaluates causal discovery algorithms for deployment on resource-constrained edge devices like Raspberry Pi. The goal is to automatically discover **causal relationships** (not just correlations) from sensor data - answering questions like "Does opening a window cause indoor AQI change?" rather than just "Are they correlated?"

### Key Findings

| Algorithm | Time Complexity | Memory | Pi Feasibility | Best For |
|-----------|-----------------|--------|----------------|----------|
| **Granger Causality** | O(n*p*d^2) | Low | **Excellent** | Quick screening, time-series |
| **PC Algorithm** | O(d^k) worst, O(d^2) sparse | Medium | **Good** | Sparse graphs, <30 vars |
| **DirectLiNGAM** | O(d^3*n) | Medium | **Good** | Non-Gaussian data, <20 vars |
| **NOTEARS** | O(d^3) per iter | Medium-High | **Marginal** | Continuous, <20 vars |
| **FCI** | O(d^k) worst | High | **Poor** | Only if latent confounders critical |
| **GES/fGES** | O(d^2) to O(d^4) | Medium | **Marginal** | Score-based, moderate vars |

### Recommended Approach for NDP

**Tiered Strategy:**
1. **Tier 1 (Always Run):** Granger Causality screening - fast, O(1) memory per pair
2. **Tier 2 (Triggered):** PC Algorithm with pruning - when Granger finds candidates
3. **Tier 3 (Offline/Cloud):** NOTEARS or LiNGAM for validation and refinement

---

## 1. Algorithm Deep Dive

### 1.1 Granger Causality

**Description:** Tests whether past values of X help predict future values of Y beyond what Y's own past predicts. Statistical test, not true causality, but computationally cheap.

**Mathematical Foundation:**
```
H0: X does not Granger-cause Y
H1: X Granger-causes Y

Test: Compare VAR models:
  Restricted:  Y_t = a0 + sum(a_i * Y_{t-i}) + e_t
  Unrestricted: Y_t = a0 + sum(a_i * Y_{t-i}) + sum(b_j * X_{t-j}) + e_t

F-statistic tests if coefficients b_j are jointly zero
```

**Complexity Analysis:**
| Aspect | Complexity | Notes |
|--------|------------|-------|
| Time (single pair) | O(n * p) | n=samples, p=max lag |
| Time (all pairs) | O(d^2 * n * p) | d=variables |
| Memory | O(n * p) | Store lagged values |
| Parallelizable | Yes | Each pair independent |

**Pi 5 Feasibility:** **Excellent**
- 50 variables, 1000 samples, 20 lags: ~1-5 seconds
- Memory: <10MB
- Can run incrementally with new data

**Limitations:**
- Assumes linear relationships
- Requires stationarity (differencing may be needed)
- "Correlation != Causation" - Granger measures predictive causality only
- Sensitive to lag selection

**When Sufficient:**
- Screening phase to identify candidate relationships
- Time-series data with clear temporal ordering
- Linear or approximately linear mechanisms
- Quick hypothesis testing

**Rust Implementation Approach:**
```rust
use nalgebra::{DMatrix, DVector};
use statrs::distribution::{FisherSnedecor, ContinuousCDF};

pub struct GrangerTest {
    max_lag: usize,
    alpha: f64,
}

impl GrangerTest {
    /// Test if x Granger-causes y
    pub fn test(&self, x: &[f64], y: &[f64]) -> GrangerResult {
        let n = y.len() - self.max_lag;

        // Build lagged matrices
        let (y_lags, x_lags) = self.build_lag_matrices(x, y);

        // Restricted model: Y ~ Y_lags
        let rss_restricted = self.fit_ols(&y_lags, &y[self.max_lag..]);

        // Unrestricted model: Y ~ Y_lags + X_lags
        let combined = self.hstack(&y_lags, &x_lags);
        let rss_unrestricted = self.fit_ols(&combined, &y[self.max_lag..]);

        // F-test
        let df1 = self.max_lag as f64;
        let df2 = (n - 2 * self.max_lag - 1) as f64;
        let f_stat = ((rss_restricted - rss_unrestricted) / df1) /
                     (rss_unrestricted / df2);

        let f_dist = FisherSnedecor::new(df1, df2).unwrap();
        let p_value = 1.0 - f_dist.cdf(f_stat);

        GrangerResult {
            f_statistic: f_stat,
            p_value,
            significant: p_value < self.alpha,
            optimal_lag: self.find_optimal_lag(x, y),
        }
    }
}
```

**Source:** [Granger Causality Review - PMC](https://pmc.ncbi.nlm.nih.gov/articles/PMC10571505/)

---

### 1.2 PC Algorithm (Constraint-Based)

**Description:** The Peter-Clark algorithm is the foundational constraint-based method. It starts with a complete undirected graph and iteratively removes edges based on conditional independence tests.

**Algorithm Steps:**
1. Start with complete undirected graph
2. For each pair (X, Y), test X _||_ Y | {} (unconditional independence)
3. Remove edge if independent
4. For remaining edges, test X _||_ Y | {Z} for each neighbor Z
5. Continue increasing conditioning set size
6. Orient edges using v-structures and orientation rules

**Complexity Analysis:**
| Aspect | Complexity | Notes |
|--------|------------|-------|
| Time (worst case) | O(d^k) | k=max degree, d=variables |
| Time (sparse graphs) | O(d^2) | Most practical cases |
| Memory | O(d^2) | Adjacency matrix |
| CI Tests | O(d^(max_degree)) | Exponential in max neighborhood size |

**Pi 5 Feasibility:** **Good for sparse graphs**
- 30 variables, sparse (degree < 5): ~10-60 seconds
- 50 variables, sparse: ~1-5 minutes
- Dense graphs: Not feasible

**Optimization Strategies for Edge:**

1. **Max Conditioning Set Limit:**
```rust
// Limit max conditioning set size to reduce exponential blowup
const MAX_COND_SIZE: usize = 3;  // Trades accuracy for speed
```

2. **Parallel CI Tests:**
```rust
// Each CI test is independent - parallelize across cores
use rayon::prelude::*;

let results: Vec<_> = pairs.par_iter()
    .map(|(i, j)| ci_test(data, *i, *j, &cond_set))
    .collect();
```

3. **Order-Independent PC (PC-Stable):**
```rust
// Decide all edge removals before applying any
// Prevents order-dependent results
```

4. **Caching P-Values:**
```rust
// Cache CI test results - same test may be needed multiple times
let cache: HashMap<(usize, usize, Vec<usize>), f64> = HashMap::new();
```

**Memory Optimization:**
```rust
// Use sparse adjacency representation for large graphs
use petgraph::graphmap::UnGraphMap;

struct SparseDAG {
    graph: UnGraphMap<u32, ()>,
    separation_sets: HashMap<(u32, u32), Vec<u32>>,
}
```

**Sources:**
- [PC Algorithm - causal-learn](https://causal-learn.readthedocs.io/en/latest/search_methods_index/Constraint-based%20causal%20discovery%20methods/PC.html)
- [Fast PC Algorithm](https://arxiv.org/pdf/1502.02454)
- [GPU Optimization for PC Algorithm](https://journalwjarr.com/sites/default/files/fulltext_pdf/WJARR-2025-1113.pdf)

---

### 1.3 FCI Algorithm (Handles Latent Confounders)

**Description:** Fast Causal Inference extends PC to handle latent (unobserved) confounders. Outputs a Partial Ancestral Graph (PAG) with additional edge markings for potential hidden variables.

**Key Difference from PC:**
- PC assumes no hidden confounders (causal sufficiency)
- FCI can detect and represent latent confounders via bi-directed edges
- Uses Y-structures: W1 -> X <- W2, X -> Y to rule out confounding

**Complexity Analysis:**
| Aspect | Complexity | Notes |
|--------|------------|-------|
| Time | O(d^k) worst | Higher constant than PC |
| Memory | O(d^2) | PAG representation |
| Additional overhead | 2-5x PC | Extra orientation rules |

**Pi 5 Feasibility:** **Poor for general use**
- 20 variables: ~1-10 minutes
- 30+ variables: Often impractical
- Only use if latent confounders are critical concern

**Lighter Alternative: RFCI**
RFCI (Really Fast Causal Inference) trades some accuracy for speed:
- Parameter k controls accuracy/speed tradeoff
- k=1: RFCI (fast, less accurate)
- k=|X|-2: Full FCI (slow, most accurate)

```rust
// RFCI configuration for edge deployment
pub struct RFCIConfig {
    k: usize,  // 1-3 for edge, higher for cloud
    max_path_length: usize,  // Limit ancestral search
    alpha: f64,  // CI test significance level
}
```

**Recommendation:** Only use FCI/RFCI if you strongly suspect hidden confounders AND have:
- Few variables (<20)
- Tolerance for longer compute times
- Hybrid setup (run on cloud, deploy results to edge)

**Source:** [FCI Documentation - causal-learn](https://causal-learn.readthedocs.io/en/latest/search_methods_index/Constraint-based%20causal%20discovery%20methods/FCI.html)

---

### 1.4 NOTEARS (Continuous Optimization)

**Description:** Transforms the combinatorial DAG learning problem into continuous optimization using an acyclicity constraint based on matrix exponential.

**Mathematical Formulation:**
```
minimize    ||X - XW||^2 + lambda * ||W||_1
subject to  trace(exp(W o W)) - d = 0  (acyclicity)

Where:
- X is n x d data matrix
- W is d x d adjacency matrix
- W o W is element-wise square
- exp() is matrix exponential
```

**Complexity Analysis:**
| Aspect | Complexity | Notes |
|--------|------------|-------|
| Time per iteration | O(d^3) | Matrix exponential |
| Total iterations | 50-500 typical | L-BFGS optimization |
| Memory | O(d^2) | Store W matrix |
| Total time | O(d^3 * iterations) | Can be slow for d>30 |

**Pi 5 Feasibility:** **Marginal**
- 10 variables: ~5-30 seconds
- 20 variables: ~30 seconds - 5 minutes
- 30+ variables: Often impractical without GPU

**Advantages:**
- Clean continuous formulation
- Handles continuous data well
- Can be extended to non-linear (NOTEARS-MLP)

**Disadvantages:**
- Local optima issues
- Scale-invariance problems with dimensional quantities
- Slower than PC for sparse graphs

**Edge Optimization:**
```rust
// Reduce iterations for edge deployment
pub struct NOTEARSConfig {
    max_iter: usize,        // 50-100 for edge (vs 500 for full)
    tolerance: f64,         // 1e-4 (vs 1e-6 for full)
    lambda1: f64,           // L1 regularization (increase for sparser graphs)
    use_warm_start: bool,   // Reuse previous solution
}

// Incremental NOTEARS - update with new data batch
impl IncrementalNOTEARS {
    pub fn update(&mut self, new_data: &DMatrix<f64>) -> DMatrix<f64> {
        // Start from previous solution
        let warm_start = self.current_w.clone();

        // Run fewer iterations since we're close to optimum
        self.config.max_iter = 20;

        self.optimize(new_data, Some(warm_start))
    }
}
```

**Source:** [NOTEARS GitHub](https://github.com/xunzheng/notears)

---

### 1.5 LiNGAM (Linear Non-Gaussian)

**Description:** Exploits non-Gaussianity of error terms to uniquely identify causal direction. Uses Independent Component Analysis (ICA) to decompose the mixing.

**Assumption:** If X causes Y: Y = aX + e, where e is non-Gaussian and independent of X.

**Key Insight:** Under non-Gaussianity, X -> Y and Y -> X produce different statistical signatures.

**Variants:**
| Variant | Complexity | Best For |
|---------|------------|----------|
| ICA-LiNGAM | O(d^3 * n) | Original, needs many samples |
| DirectLiNGAM | O(d^3 * n) | More stable, iterative |
| VAR-LiNGAM | O(d^3 * n * p) | Time-series with lags |

**Complexity Analysis:**
| Aspect | Complexity | Notes |
|--------|------------|-------|
| Time | O(d^3 * n) | ICA + ordering |
| Memory | O(d * n) | Data + correlation matrices |
| Samples needed | 500-5000 | Depends on non-Gaussianity strength |

**Pi 5 Feasibility:** **Good for small d**
- 10 variables, 1000 samples: ~5-30 seconds
- 20 variables, 2000 samples: ~30 seconds - 3 minutes
- 30+ variables: Borderline

**Edge Advantages:**
- Non-iterative (no convergence issues)
- Identifies unique causal direction
- Works well with sensor data (often non-Gaussian)

**Rust Implementation Strategy:**
```rust
use nalgebra::DMatrix;

pub struct DirectLiNGAM {
    max_iterations: usize,
    threshold: f64,
}

impl DirectLiNGAM {
    pub fn fit(&self, data: &DMatrix<f64>) -> CausalOrder {
        let n = data.nrows();
        let d = data.ncols();

        let mut order: Vec<usize> = Vec::with_capacity(d);
        let mut remaining: Vec<usize> = (0..d).collect();
        let mut residuals = data.clone();

        while !remaining.is_empty() {
            // Find the most exogenous variable (root)
            let root = self.find_root(&residuals, &remaining);
            order.push(root);
            remaining.retain(|&x| x != root);

            // Regress out root from remaining variables
            residuals = self.regress_out(&residuals, root, &remaining);
        }

        CausalOrder { order, adjacency: self.build_adjacency(data, &order) }
    }

    fn find_root(&self, data: &DMatrix<f64>, candidates: &[usize]) -> usize {
        // Variable with highest non-Gaussianity is most likely root
        candidates.iter()
            .max_by(|&&a, &&b| {
                let ng_a = self.non_gaussianity(&data.column(a));
                let ng_b = self.non_gaussianity(&data.column(b));
                ng_a.partial_cmp(&ng_b).unwrap()
            })
            .copied()
            .unwrap()
    }
}
```

**Source:** [LiNGAM Python Package](https://github.com/cdt15/lingam)

---

### 1.6 GES (Greedy Equivalence Search)

**Description:** Score-based algorithm that greedily adds/removes edges to maximize a scoring function (typically BIC). Works in forward (add edges) and backward (remove edges) phases.

**Algorithm Phases:**
1. **Forward:** Greedily add edges that improve score
2. **Backward:** Greedily remove edges that improve score
3. **Turning:** Flip edge orientations to improve score

**Complexity Analysis:**
| Aspect | Complexity | Notes |
|--------|------------|-------|
| Time (worst case) | O(d^4) | Score evaluation per edge |
| Time (sparse) | O(d^2 * k^2) | k=max parents |
| Memory | O(d^2) | Graph + score cache |
| Parallelizable | Partial | Score computations independent |

**Pi 5 Feasibility:** **Marginal**
- 20 variables: ~30 seconds - 5 minutes
- 30 variables: ~5-30 minutes
- 50 variables: Often impractical

**Faster Variant: fGES (Fast GES)**
- Parallelizes score computations
- Reorganizes caching strategy
- Can handle ~1 million variables for sparse graphs

```rust
// fGES configuration for edge
pub struct FGESConfig {
    num_threads: usize,    // Use all Pi cores
    score_cache_mb: usize, // 100-500MB for caching
    max_parents: usize,    // Limit parent set size (3-5)
    penalty_discount: f64, // BIC penalty (1.0 for BIC, 2.0 for stricter)
}
```

**Source:** [XGES Paper](https://arxiv.org/html/2502.19551v1)

---

## 2. Complexity Comparison for Pi 5

### 2.1 Benchmark Estimates (Raspberry Pi 5, 4GB RAM)

| Algorithm | 10 vars | 20 vars | 30 vars | 50 vars |
|-----------|---------|---------|---------|---------|
| **Granger** | <1s | <5s | <15s | <60s |
| **PC (sparse)** | 2-10s | 10-60s | 1-5min | 5-30min |
| **PC (dense)** | 10-60s | 5-30min | hours | infeasible |
| **DirectLiNGAM** | 5-30s | 30s-3min | 3-15min | 15-60min |
| **NOTEARS** | 5-30s | 30s-5min | 5-30min | 30min-2hr |
| **FCI** | 30s-5min | 5-30min | 30min-2hr | infeasible |
| **GES/fGES** | 10-60s | 1-10min | 10-60min | 1-4hr |

### 2.2 Memory Requirements

| Algorithm | 10 vars | 20 vars | 30 vars | 50 vars |
|-----------|---------|---------|---------|---------|
| **Granger** | <5MB | <10MB | <20MB | <50MB |
| **PC** | 10MB | 50MB | 150MB | 500MB |
| **DirectLiNGAM** | 10MB | 40MB | 100MB | 300MB |
| **NOTEARS** | 5MB | 20MB | 50MB | 150MB |
| **FCI** | 20MB | 100MB | 300MB | 1GB+ |
| **GES** | 50MB | 200MB | 500MB | 1GB+ |

### 2.3 Data Requirements

| Algorithm | Minimum Samples | Recommended Samples |
|-----------|-----------------|---------------------|
| **Granger** | 50 + 10*lags | 200-1000 |
| **PC** | 5*d^2 | 10*d^2 (500-5000) |
| **DirectLiNGAM** | 100*d | 500-5000 |
| **NOTEARS** | 10*d | 100-1000 |
| **FCI** | 5*d^2 | 10*d^2 (500-5000) |
| **GES** | 20*d | 200-2000 |

**Source:** [Sample Complexity of Causal Discovery](https://arxiv.org/abs/2102.03274)

---

## 3. Lightweight Variants and Approximations

### 3.1 Incremental/Online Causal Discovery

For streaming edge data, traditional batch algorithms are impractical. Recent research has developed incremental approaches.

**CORAL Framework (Real-time Root Cause Analysis):**
- Trigger point detection using multivariate singular spectrum analysis
- Incremental causal graph updates from streaming data
- Experience replay to prevent catastrophic forgetting

**INCADET Framework (Cyberattack Detection):**
- Detects system state transitions via edge-weight divergence
- Incrementally refines causal structures with experience replay
- Uses GCNs for graph classification

**Implementation Pattern:**
```rust
pub struct IncrementalCausalDiscovery {
    current_graph: CausalGraph,
    window_buffer: RingBuffer<DataPoint>,
    trigger_detector: ADWIN,  // From drift detection
    experience_replay: ReplayBuffer,
}

impl IncrementalCausalDiscovery {
    pub async fn process_stream(&mut self, point: DataPoint) -> Option<GraphUpdate> {
        self.window_buffer.push(point);

        // Check for distribution shift (trigger point)
        if self.trigger_detector.detect_drift(&self.window_buffer) {
            // Incremental graph update
            let update = self.incremental_update().await;
            self.experience_replay.store(&update);
            return Some(update);
        }

        None
    }

    async fn incremental_update(&mut self) -> GraphUpdate {
        // Only test edges likely to have changed
        let candidate_edges = self.identify_changed_edges();

        for (i, j) in candidate_edges {
            let ci_result = self.ci_test(i, j);
            self.current_graph.update_edge(i, j, ci_result);
        }

        // Reinforce stable edges, decay uncertain ones
        self.edge_reinforcement();

        self.current_graph.get_update()
    }
}
```

**Source:** [Incremental Causal Graph Learning](https://arxiv.org/abs/2507.14387)

---

### 3.2 Approximate Algorithms

**DAS (Discovery At Scale):**
- Reduces pruning complexity by factor proportional to graph size
- Over 10x faster than standard SCORE algorithm
- Maintains competitive accuracy

**Approximate Kernel Methods (Linear Complexity):**
- Uses Nystrom approximation for kernel matrices
- Reduces O(n^3) to O(n) time and space
- Enables large-sample causal discovery

```rust
// Nystrom approximation for kernel-based CI tests
pub struct NystromKernel {
    num_landmarks: usize,  // sqrt(n) typical
    kernel_bandwidth: f64,
}

impl NystromKernel {
    pub fn approximate_gram_matrix(&self, data: &DMatrix<f64>) -> DMatrix<f64> {
        let n = data.nrows();
        let m = self.num_landmarks;

        // Sample landmark points
        let landmarks = self.sample_landmarks(data, m);

        // Compute K_nm (n x m) and K_mm (m x m)
        let k_nm = self.compute_kernel_block(data, &landmarks);
        let k_mm = self.compute_kernel_block(&landmarks, &landmarks);

        // Nystrom approximation: K ≈ K_nm * K_mm^(-1) * K_nm^T
        let k_mm_inv = k_mm.pseudo_inverse(1e-6).unwrap();
        &k_nm * &k_mm_inv * k_nm.transpose()
    }
}
```

**Source:** [Fast Causal Discovery with Linear Complexity](https://arxiv.org/abs/2412.17717)

---

### 3.3 Pruning and Search Space Reduction

**Strategies to reduce complexity:**

1. **Variable Pre-selection:**
```rust
// Only include variables with sufficient variance and correlation
fn preselect_variables(data: &DMatrix<f64>, threshold: f64) -> Vec<usize> {
    let variances = compute_variances(data);
    let correlations = compute_pairwise_correlations(data);

    (0..data.ncols())
        .filter(|&i| variances[i] > threshold)
        .filter(|&i| correlations.row(i).iter().any(|&c| c.abs() > 0.1))
        .collect()
}
```

2. **Granger Pre-screening:**
```rust
// Use Granger causality to identify candidate edges
fn granger_prescreen(data: &DMatrix<f64>, alpha: f64) -> Vec<(usize, usize)> {
    let granger = GrangerTest::new(20, alpha);
    let mut candidates = Vec::new();

    for i in 0..data.ncols() {
        for j in 0..data.ncols() {
            if i != j && granger.test(&data.column(i), &data.column(j)).significant {
                candidates.push((i, j));
            }
        }
    }

    candidates
}
```

3. **Local Causal Discovery:**
```rust
// Focus on neighborhood of target variable
fn local_causal_discovery(data: &DMatrix<f64>, target: usize, max_depth: usize) -> LocalGraph {
    let mut neighbors = vec![target];
    let mut graph = LocalGraph::new(target);

    for depth in 0..max_depth {
        let new_neighbors = pc_expand_neighborhood(data, &neighbors);
        neighbors.extend(new_neighbors);
        graph.update(&neighbors);
    }

    graph
}
```

4. **Constraint Set Limiting:**
```rust
// Limit maximum conditioning set size
const MAX_COND_SIZE: usize = 3;

fn limited_pc(data: &DMatrix<f64>) -> CausalGraph {
    let mut graph = complete_graph(data.ncols());

    for cond_size in 0..=MAX_COND_SIZE {
        for (i, j) in graph.edges() {
            let neighbors = graph.neighbors(i).filter(|&k| k != j);
            for cond_set in neighbors.combinations(cond_size) {
                if ci_test(data, i, j, &cond_set) {
                    graph.remove_edge(i, j);
                    break;
                }
            }
        }
    }

    graph
}
```

---

## 4. Granger Causality vs Structural Causal Models

### 4.1 Comparison

| Aspect | Granger Causality | Structural Causal Models |
|--------|-------------------|--------------------------|
| **Definition** | Predictive causality | True causal mechanisms |
| **Assumption** | Temporal precedence | Causal sufficiency, DAG |
| **Data Type** | Time-series only | Cross-sectional or time-series |
| **Complexity** | O(n * p * d^2) | O(d^k) to O(d^3) |
| **Interpretability** | "X predicts Y" | "X causes Y" |
| **Handles cycles** | VAR models can | No (DAG assumption) |
| **Handles confounders** | No (unless VAR) | FCI can detect |
| **Sample requirements** | Low (100s) | Higher (1000s) |

### 4.2 When to Use Each

**Use Granger Causality when:**
- Primary interest is forecasting, not mechanism understanding
- Data is clearly time-series with temporal ordering
- Computational resources are limited
- Quick screening phase before deeper analysis
- Variables are ~linearly related
- Sample size is limited

**Use Structural Causal Models when:**
- Need to understand true causal mechanisms
- Want to reason about interventions ("What if we do X?")
- Have cross-sectional data or no clear temporal ordering
- Can afford computational cost
- Need to detect hidden confounders (use FCI)
- Have sufficient samples for CI tests

### 4.3 Hybrid Approach for NDP

```
┌─────────────────────────────────────────────────────────────────┐
│                    HYBRID CAUSAL DISCOVERY                       │
│                                                                  │
│  Stage 1: GRANGER SCREENING (Always run, on-device)             │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  For each variable pair (X, Y):                         │    │
│  │    1. Test X -> Y (Granger)                             │    │
│  │    2. Test Y -> X (Granger)                             │    │
│  │    3. Store significant relationships                    │    │
│  │  Output: Candidate edge set                              │    │
│  └─────────────────────────────────────────────────────────┘    │
│                         │                                        │
│                         ▼                                        │
│  Stage 2: PC REFINEMENT (Triggered, on-device)                  │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  Starting from Granger candidates:                       │    │
│  │    1. Run PC on candidate subgraph                       │    │
│  │    2. Test conditional independencies                    │    │
│  │    3. Orient edges using v-structures                    │    │
│  │  Output: Partial DAG                                     │    │
│  └─────────────────────────────────────────────────────────┘    │
│                         │                                        │
│                         ▼                                        │
│  Stage 3: SCM VALIDATION (Periodic, cloud/batch)                │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  Using accumulated data:                                 │    │
│  │    1. Run NOTEARS or LiNGAM for validation              │    │
│  │    2. Check for hidden confounders (optional FCI)        │    │
│  │    3. Update edge weights and confidence                 │    │
│  │  Output: Validated causal graph                          │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

**Source:** [Granger Causality Review](https://pmc.ncbi.nlm.nih.gov/articles/PMC10571505/)

---

## 5. Implementation Options

### 5.1 Rust Implementations

| Crate | Description | Status | Pi Compatible |
|-------|-------------|--------|---------------|
| **[causal-hub](https://crates.io/crates/causal-hub)** | Causal discovery algorithms | Active | Yes |
| **[deep_causality](https://crates.io/crates/deep_causality)** | Hyper-geometric causality | Active (LF sandbox) | Yes |
| **[causalkit](https://github.com/rakutentech/causalkit)** | Treatment effect estimation | Active | Yes |

**causal-hub Example:**
```rust
use causal_hub::prelude::*;

// PC algorithm
let pc = PC::new(alpha);
let graph = pc.fit(&data)?;

// Available algorithms vary - check documentation
```

**deep_causality Example:**
```rust
use deep_causality::prelude::*;

// Build causal model
let model = CausalModel::new()
    .add_causal_relation(cause_a, effect_b, weight)
    .add_causal_relation(cause_b, effect_c, weight);

// Reason over model
let result = model.reason(observation)?;
```

### 5.2 Python Libraries (via PyO3 FFI)

| Library | Algorithms | Notes |
|---------|------------|-------|
| **[causal-learn](https://causal-learn.readthedocs.io/)** | PC, FCI, GES, LiNGAM, NOTEARS | Most comprehensive |
| **[lingam](https://github.com/cdt15/lingam)** | All LiNGAM variants | Official implementation |
| **[dowhy](https://github.com/py-why/dowhy)** | Causal inference | Microsoft, includes discovery |
| **[tigramite](https://github.com/jakobrunge/tigramite)** | Time-series causal discovery | PCMCI algorithm |

**Python FFI Approach:**
```rust
use pyo3::prelude::*;

pub fn run_pc_python(data: &DMatrix<f64>) -> PyResult<CausalGraph> {
    Python::with_gil(|py| {
        let causal_learn = py.import("causallearn")?;
        let pc = causal_learn.getattr("search")?.getattr("PC")?;

        let np_data = numpy_from_nalgebra(py, data)?;
        let result = pc.call1((np_data,))?;

        parse_causal_graph(result)
    })
}
```

### 5.3 WASM-Compatible Options

For browser or WASM-edge deployment:

| Option | Status | Notes |
|--------|--------|-------|
| **deep_causality** | WASM compatible | Full no_std support |
| **Custom Granger** | Easy to port | Pure math operations |
| **PC (simplified)** | Possible | Need WASM-compatible CI tests |

```rust
// WASM-compatible Granger causality
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn granger_test_wasm(x: &[f64], y: &[f64], max_lag: usize) -> f64 {
    let granger = GrangerTest::new(max_lag, 0.05);
    granger.test(x, y).p_value
}
```

---

## 6. Practical Deployment Guidance

### 6.1 Recommended Configuration for Pi 5

```yaml
# causal_discovery.yaml
causal_discovery:
  enabled: true

  # Tier 1: Always-on Granger screening
  granger:
    enabled: true
    max_lag: 24  # Hours for hourly data
    alpha: 0.05
    run_interval_hours: 6
    min_samples: 168  # 1 week of hourly data

  # Tier 2: Triggered PC refinement
  pc:
    enabled: true
    trigger: "granger_candidate_count > 5"
    max_conditioning_set: 3
    alpha: 0.01
    max_variables: 30  # Subset if more
    timeout_seconds: 300

  # Tier 3: Periodic validation (optional, resource-intensive)
  validation:
    enabled: false  # Enable for cloud/batch
    algorithm: "notears"  # or "lingam"
    schedule: "0 3 * * 0"  # Weekly at 3am Sunday

  # Resource limits
  resources:
    max_memory_mb: 500
    max_cpu_percent: 50
    max_runtime_seconds: 600

  # Results storage
  storage:
    graph_path: /data/causal/current_graph.json
    history_path: /data/causal/graph_history.parquet
    max_history_days: 90
```

### 6.2 NDP Integration Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    NDP + CAUSAL DISCOVERY                        │
│                                                                  │
│  SILVER LAYER (TimescaleDB)                                     │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  Air Quality  │  Weather  │  System Metrics  │  etc.   │    │
│  └───────┬───────────┬──────────────┬───────────────────────┘    │
│          │           │              │                            │
│          └───────────┴──────────────┘                            │
│                      │                                           │
│                      ▼                                           │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │              CAUSAL DISCOVERY ENGINE                     │    │
│  │                                                          │    │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │    │
│  │  │   Granger    │  │     PC       │  │  Validation  │  │    │
│  │  │  Screener    │──│   Refiner    │──│   (Cloud)    │  │    │
│  │  │  (6 hourly)  │  │ (triggered)  │  │  (weekly)    │  │    │
│  │  └──────────────┘  └──────────────┘  └──────────────┘  │    │
│  │                          │                              │    │
│  │                          ▼                              │    │
│  │  ┌─────────────────────────────────────────────────┐   │    │
│  │  │            CAUSAL GRAPH STORE                    │   │    │
│  │  │  - Current graph (JSON)                          │   │    │
│  │  │  - Edge confidence scores                        │   │    │
│  │  │  - Historical changes (Parquet)                  │   │    │
│  │  └─────────────────────────────────────────────────┘   │    │
│  │                                                          │    │
│  └─────────────────────────────────────────────────────────┘    │
│                      │                                           │
│                      ▼                                           │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                   APPLICATIONS                           │    │
│  │                                                          │    │
│  │  - Root Cause Analysis: "Why did PM2.5 spike?"          │    │
│  │  - Intervention Reasoning: "If I open window, what?"     │    │
│  │  - Anomaly Explanation: "This is unusual because..."     │    │
│  │  - Alert Prioritization: Based on causal centrality      │    │
│  │                                                          │    │
│  └─────────────────────────────────────────────────────────┘    │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 6.3 Handling Non-Stationarity

Sensor data often exhibits non-stationarity (trends, seasonal patterns). This violates assumptions of many causal discovery algorithms.

**Solutions:**

1. **Differencing:**
```rust
// First-order differencing for trend removal
fn difference(series: &[f64]) -> Vec<f64> {
    series.windows(2)
        .map(|w| w[1] - w[0])
        .collect()
}
```

2. **Seasonal Adjustment:**
```rust
// Subtract seasonal component (e.g., from MSTL decomposition)
fn deseasonalize(series: &[f64], seasonal: &[f64]) -> Vec<f64> {
    series.iter().zip(seasonal.iter())
        .map(|(s, season)| s - season)
        .collect()
}
```

3. **Windowed Analysis:**
```rust
// Run causal discovery on rolling windows
fn windowed_causal_discovery(data: &DMatrix<f64>, window_size: usize, step: usize) {
    for start in (0..data.nrows() - window_size).step_by(step) {
        let window = data.rows(start, window_size);
        let graph = run_causal_discovery(&window);
        store_temporal_graph(start + window_size, graph);
    }
}
```

4. **PCMCI for Time-Series:**
PCMCI (Peter-Clark Momentary Conditional Independence) is specifically designed for time-series causal discovery:
```python
# Using tigramite library
from tigramite import PCMCI
pcmci = PCMCI(dataframe, cond_ind_test, verbosity=0)
results = pcmci.run_pcmci(tau_max=10, pc_alpha=0.05)
```

### 6.4 Handling Noisy Sensor Data

**Strategies:**

1. **Robust CI Tests:**
```rust
// Use permutation-based tests instead of parametric
fn permutation_ci_test(x: &[f64], y: &[f64], z: &[f64], n_perms: usize) -> f64 {
    let observed_stat = compute_ci_statistic(x, y, z);
    let perm_stats: Vec<f64> = (0..n_perms)
        .map(|_| {
            let x_perm = permute(x);
            compute_ci_statistic(&x_perm, y, z)
        })
        .collect();

    // P-value = proportion of permutation stats >= observed
    perm_stats.iter().filter(|&&s| s >= observed_stat).count() as f64 / n_perms as f64
}
```

2. **Median Regression:**
```rust
// Use LAD (Least Absolute Deviations) instead of OLS
fn robust_regression(x: &DMatrix<f64>, y: &DVector<f64>) -> DVector<f64> {
    // Iteratively reweighted least squares for LAD
    irls_lad(x, y, max_iter: 100)
}
```

3. **Bootstrap Confidence:**
```rust
// Bootstrap to estimate edge confidence
fn bootstrap_edge_confidence(data: &DMatrix<f64>, edge: (usize, usize), n_bootstrap: usize) -> f64 {
    let detected: usize = (0..n_bootstrap)
        .map(|_| {
            let sample = bootstrap_sample(data);
            let graph = run_causal_discovery(&sample);
            if graph.has_edge(edge.0, edge.1) { 1 } else { 0 }
        })
        .sum();

    detected as f64 / n_bootstrap as f64
}
```

### 6.5 Incremental Updates

For continuous operation, avoid full graph re-computation:

```rust
pub struct IncrementalCausalGraph {
    graph: CausalGraph,
    edge_confidence: HashMap<(usize, usize), f64>,
    last_full_discovery: DateTime<Utc>,
    data_buffer: RingBuffer<DataRow>,
}

impl IncrementalCausalGraph {
    /// Update with new data point
    pub fn update(&mut self, row: DataRow) {
        self.data_buffer.push(row);

        // Decay confidence on all edges slightly
        for (_, conf) in self.edge_confidence.iter_mut() {
            *conf *= 0.999;  // Slow decay
        }

        // Quick Granger check on recent data
        if self.data_buffer.len() >= 100 {
            let new_candidates = self.quick_granger_scan();
            for (i, j) in new_candidates {
                // Reinforce or add edges
                *self.edge_confidence.entry((i, j)).or_insert(0.0) += 0.1;
            }
        }

        // Periodic full refresh
        if self.should_full_refresh() {
            self.full_discovery();
        }

        // Prune low-confidence edges
        self.prune_weak_edges(threshold: 0.3);
    }
}
```

---

## 7. Example Use Cases for NDP

### 7.1 Air Quality Causal Analysis

**Variables:**
- PM2.5 indoor, PM2.5 outdoor
- Temperature (indoor/outdoor)
- Humidity (indoor/outdoor)
- CO2, VOC levels
- Window/door state
- HVAC state
- Occupancy

**Expected Discoveries:**
- PM2.5_outdoor -> PM2.5_indoor (with lag)
- Window_open -> (PM2.5_indoor, Temperature_indoor)
- Cooking_activity -> (PM2.5_indoor, VOC)
- HVAC_running -> (Temperature_indoor, Humidity_indoor)

**Implementation:**
```rust
let air_quality_vars = vec![
    "pm25_indoor", "pm25_outdoor", "temp_indoor", "temp_outdoor",
    "humidity_indoor", "humidity_outdoor", "co2", "voc",
    "window_state", "hvac_state", "occupancy"
];

let causal_engine = CausalDiscovery::new(CausalConfig {
    variables: air_quality_vars,
    granger_max_lag: 4,  // 4 hours for hourly data
    pc_max_cond: 2,
    ..Default::default()
});

// Run discovery on last 7 days of data
let graph = causal_engine.discover(&silver_data).await?;

// Query causal relationships
let pm25_causes = graph.get_causes("pm25_indoor");
// -> ["pm25_outdoor", "window_state", "cooking_activity"]

let pm25_effects = graph.get_effects("pm25_indoor");
// -> ["health_risk_score"]
```

### 7.2 Financial Regime Detection

**Variables:**
- SPY returns, VIX
- Yield curve slope (10Y-2Y)
- Credit spreads (HY-IG)
- Dollar index
- Copper/Gold ratio
- Leading economic indicators

**Expected Discoveries:**
- Yield_curve_inversion -> Recession_indicator (long lag)
- VIX_spike -> SPY_volatility
- Credit_spread_widening -> Equity_selloff

### 7.3 IoT Sensor Network

**Variables:**
- Temperature sensors (multiple locations)
- Motion sensors
- Energy consumption
- HVAC commands

**Expected Discoveries:**
- Thermostat_command -> HVAC_state -> Temperature_change
- Motion_detected -> Lights_on -> Energy_spike
- Outdoor_temp -> Indoor_temp (with building thermal lag)

---

## 8. Conclusion and Recommendations

### 8.1 Summary

| Scenario | Recommended Algorithm | Pi Feasibility |
|----------|----------------------|----------------|
| **Quick screening** | Granger Causality | Excellent |
| **Sparse graphs, <30 vars** | PC Algorithm | Good |
| **Non-Gaussian data, <20 vars** | DirectLiNGAM | Good |
| **Continuous data, <20 vars** | NOTEARS | Marginal |
| **Hidden confounders** | RFCI (cloud) or avoid | Poor |
| **Streaming data** | Incremental Granger + PC | Good |
| **Very high-dim (50+)** | Local causal discovery | Depends |

### 8.2 Recommended Implementation Roadmap

**Phase 1: Foundation (Weeks 1-2)**
- Implement Granger causality in Rust
- Integrate with Silver layer queries
- Basic causal graph storage (JSON)

**Phase 2: Refinement (Weeks 3-4)**
- Add PC algorithm (leverage causal-hub crate)
- Implement Granger -> PC pipeline
- Add bootstrap confidence estimation

**Phase 3: Advanced (Weeks 5-8)**
- Incremental update mechanism
- Integration with ADWIN drift detection
- Causal explanation API for alerts

**Phase 4: Validation (Weeks 9-12)**
- Optional cloud validation with NOTEARS/LiNGAM
- Historical causal graph tracking
- Grafana visualization dashboard

### 8.3 Key Takeaways

1. **Start with Granger:** Fast, interpretable, sufficient for many use cases
2. **Use PC for refinement:** When Granger finds candidates, PC adds rigor
3. **Limit scope:** Local causal discovery around target variables beats global
4. **Embrace incrementality:** Streaming data needs incremental updates
5. **Accept approximations:** Perfect causal discovery is infeasible; good-enough is valuable
6. **Validate carefully:** Causal claims need domain knowledge validation

---

## 9. References

### Causal Discovery Algorithms
- [PC Algorithm - causal-learn](https://causal-learn.readthedocs.io/en/latest/search_methods_index/Constraint-based%20causal%20discovery%20methods/PC.html)
- [Fast PC Algorithm for High Dimensional Data](https://arxiv.org/pdf/1502.02454)
- [GPU Optimization for PC Algorithm](https://journalwjarr.com/sites/default/files/fulltext_pdf/WJARR-2025-1113.pdf)
- [FCI Algorithm - causal-learn](https://causal-learn.readthedocs.io/en/latest/search_methods_index/Constraint-based%20causal%20discovery%20methods/FCI.html)
- [NOTEARS GitHub](https://github.com/xunzheng/notears)
- [LiNGAM Python Package](https://github.com/cdt15/lingam)
- [XGES - Extremely Greedy Equivalence Search](https://arxiv.org/html/2502.19551v1)

### Incremental and Online Methods
- [Incremental Causal Graph Learning - INCADET](https://arxiv.org/abs/2507.14387)
- [CORAL Framework for Root Cause Analysis](https://arxiv.org/pdf/2305.10638)
- [Local Causal Discovery for Streaming Features](https://onlinelibrary.wiley.com/doi/10.1111/exsy.70170)

### Approximate and Scalable Methods
- [Fast Causal Discovery with Linear Complexity](https://arxiv.org/abs/2412.17717)
- [DAS - Discovery At Scale](https://proceedings.mlr.press/v213/montagna23b/montagna23b.pdf)

### Granger Causality
- [Granger Causality Review - PMC](https://pmc.ncbi.nlm.nih.gov/articles/PMC10571505/)
- [Granger Causality vs DBN](https://pmc.ncbi.nlm.nih.gov/articles/PMC2691740/)

### Sample Complexity
- [Sample Complexity of Causal Discovery](https://arxiv.org/abs/2102.03274)

### Rust Libraries
- [causal-hub - Rust](https://crates.io/crates/causal-hub)
- [deep_causality - Rust](https://crates.io/crates/deep_causality)

### Edge AI Context
- [Edge AI Deployment Framework](https://www.mdpi.com/2079-9292/14/24/4877)
- [TinyML 2026](https://research.aimultiple.com/tinyml/)

### NDP Project Context
- [Correlation Discovery Techniques](/workspaces/neural-data-platform/product/research/gold/financial-intelligence/correlation-discovery/TECHNIQUES.md)
- [Self-Learning Adaptive Systems](/workspaces/neural-data-platform/product/research/gold/self-learning/ADAPTIVE-SYSTEMS.md)
- [Edge ML Deployment Strategies](/workspaces/neural-data-platform/product/research/gold/edge-ml/DEPLOYMENT-STRATEGIES.md)

---

**Document Version:** 1.0
**Author:** Research Agent
**Status:** Complete
**Last Updated:** 2026-02-02
