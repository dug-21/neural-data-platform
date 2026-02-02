# ruv-FANN Deep Assessment for NDP Gold Layer

> **Research Date:** 2026-02-02
> **Context:** Supplementary research for Gold layer neural capabilities
> **Focus:** Should ruv-FANN be the primary neural engine for NDP?

---

## Executive Summary

ruv-FANN is a **Rust-native neural network ecosystem** with 27+ forecasting models, EWC++ online learning, MCP integration, and WASM deployment support. NDP already has a partial integration (`core/src/forecast/fann_adapter.rs`) using a mock model.

**Recommendation:** Complete the ruv-FANN integration as **Phase 2 neural layer**, but maintain augurs as the **Phase 1 production baseline** due to ruv-FANN's limited production validation.

---

## 1. ruv-FANN Ecosystem Components

### 1.1 Full Stack Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    ruv-FANN Ecosystem                        │
├─────────────────────────────────────────────────────────────┤
│  ruv-swarm-mcp    │ MCP server for Claude Code integration   │
│  ruv-swarm-agents │ Cognitive patterns (researcher, analyst) │
│  ruv-swarm-ml     │ 27+ neural forecasting models            │
│  ruvector-sona    │ Online learning with EWC++               │
│  ruv-fann         │ Core neural network engine               │
│  ruv-swarm-core   │ Orchestration topologies                 │
└─────────────────────────────────────────────────────────────┘
```

### 1.2 Model Inventory (27+ Models)

| Category | Models | NDP Applicability |
|----------|--------|-------------------|
| **Basic** | MLP, DLinear, NLinear | High - lightweight baseline |
| **Recurrent** | RNN, LSTM, GRU | High - time-series standard |
| **Advanced** | N-BEATS, NBEATSx, NHITS, TiDE | Very High - interpretable forecasting |
| **Transformer** | TFT, Informer, AutoFormer, PatchTST | Medium - may be too heavy for Pi |
| **TCN** | TCN, BiTCN | High - efficient convolutions |
| **Specialized** | DeepAR, TimesNet, StemGNN | Medium - specific use cases |

### 1.3 Key Differentiators

| Feature | ruv-FANN | augurs | Relevance to NDP |
|---------|----------|--------|------------------|
| Model variety | 27+ neural models | ETS, MSTL, Prophet, DBSCAN | ruv-FANN for complex patterns |
| Online learning | EWC++ (ruvector-sona) | Not built-in | Critical for drift adaptation |
| MCP integration | Native (ruv-swarm-mcp) | None | Enables agentic workflows |
| WASM support | Yes | Yes | Both edge-compatible |
| Production validation | Limited | Grafana Cloud | augurs more proven |
| Rust-native | Yes | Yes | Both suitable |

---

## 2. Current NDP Integration

### 2.1 Existing Code (`core/src/forecast/`)

```
core/src/forecast/
├── mod.rs           # Module exports
├── fann_adapter.rs  # FannForecaster with MockModel (stub)
├── features.rs      # Feature engineering (lag, rolling, temporal)
└── scaler.rs        # StandardScaler for normalization
```

### 2.2 FannForecaster Status

**What's Implemented:**
- `Forecast` trait with `train()`, `predict()`, `metrics()`
- Feature engineering: lag (1h, 3h, 24h), rolling mean/std
- Model selection: NHITS vs NBEATSx based on trend strength
- Confidence interval calculation
- Comprehensive test suite

**What's Missing (Phase 3 marked):**
- Actual ruv-fann model loading from safetensors
- Real inference instead of mock predictions
- Online learning integration (EWC++)
- MCP tool exposure

### 2.3 Gap Analysis

| Component | Status | Effort to Complete |
|-----------|--------|-------------------|
| Model loading | Stub | Medium - need safetensors integration |
| Inference | Mock | Medium - wire up actual ruv-fann |
| Feature engineering | Complete | - |
| Online learning | Not started | High - integrate ruvector-sona |
| MCP tools | Not started | Medium - use rmcp with existing functions |

---

## 3. ruv-FANN vs augurs Comparison

### 3.1 Technical Comparison

| Aspect | ruv-FANN | augurs | Winner |
|--------|----------|--------|--------|
| **Forecasting accuracy** | Claims 2-4x faster, unverified | Proven in Grafana Cloud | augurs (validated) |
| **Model sophistication** | 27+ neural models | ETS, MSTL, Prophet, DBSCAN | ruv-FANN |
| **Edge deployment** | WASM, CPU-native | WASM, Python bindings | Tie |
| **Online learning** | EWC++ built-in | Not included | ruv-FANN |
| **Community** | Small | Grafana-backed | augurs |
| **Documentation** | Variable quality | Good | augurs |
| **Production evidence** | Limited | Grafana Cloud scale | augurs |

### 3.2 When to Use Each

**Use augurs when:**
- Production stability is critical
- Simple forecasting (ETS, MSTL) is sufficient
- Monitoring/alerting is primary use case
- Need proven, well-documented solution

**Use ruv-FANN when:**
- Complex patterns require neural networks
- Online learning with EWC++ is needed
- MCP integration for agentic workflows
- Research/experimentation phase

### 3.3 Recommended Hybrid Approach

```
Phase 1 (Production): augurs for forecasting
                      ├── ETS for short-term
                      ├── MSTL for seasonal
                      └── DBSCAN for anomaly detection

Phase 2 (Enhancement): ruv-FANN for advanced capabilities
                       ├── NHITS/NBEATSx for complex patterns
                       ├── EWC++ for online learning
                       └── MCP tools for agentic integration
```

---

## 4. Integration Roadmap

### 4.1 Phase 1: Complete Mock Replacement (Weeks 1-2)

**Goal:** Replace `MockModel` with actual ruv-fann inference

```rust
// Current (mock)
let model = MockModel::new(self.input_window * 13, self.forecast_horizon);

// Target (real)
use ruv_fann::Model;
let model = Model::load_safetensors(&self.model_path)?;
```

**Tasks:**
1. Add `ruv-fann` crate dependency
2. Load pre-trained NHITS/NBEATSx models from safetensors
3. Replace mock predictions with real inference
4. Benchmark latency on Pi 5

### 4.2 Phase 2: Online Learning (Weeks 3-4)

**Goal:** Integrate ruvector-sona for EWC++ continual learning

```rust
use ruvector_sona::{SonaLearner, EwcConfig};

impl FannForecaster {
    async fn incremental_update(&mut self, new_data: &[TimeSeriesPoint]) {
        let ewc = EwcConfig {
            lambda: 2000.0,  // Memory protection strength
            enabled: true,
        };

        self.sona_learner.update(new_data, &ewc).await?;
    }
}
```

**Tasks:**
1. Add `ruvector-sona` crate
2. Implement incremental training with EWC++
3. Wire up ADWIN drift detection to trigger updates
4. Test catastrophic forgetting prevention

### 4.3 Phase 3: MCP Integration (Weeks 5-6)

**Goal:** Expose ruv-FANN capabilities via MCP tools

```rust
use rmcp::prelude::*;

#[tool]
/// Forecast air quality using NHITS neural network
async fn forecast_neural(
    hours_ahead: u32,
    model_type: String,  // "nhits" | "nbeats" | "tcn"
) -> Result<NeuralForecast, Error> {
    let forecaster = GLOBAL_FANN.read().await;
    forecaster.predict("air-quality", "pm25", hours_ahead as usize).await
}

#[tool]
/// Trigger online learning with recent data
async fn update_model_online(
    hours_of_data: u32,
) -> Result<UpdateResult, Error> {
    let mut forecaster = GLOBAL_FANN.write().await;
    forecaster.incremental_update(recent_data).await
}
```

### 4.4 Phase 4: Model Ensemble (Weeks 7-8)

**Goal:** Combine augurs + ruv-FANN for best of both

```rust
struct EnsembleForecaster {
    augurs: AugursForecaster,      // ETS, MSTL (production baseline)
    fann: FannForecaster,          // NHITS, NBEATSx (neural enhancement)
    weights: HashMap<String, f64>, // Learned combination weights
}

impl EnsembleForecaster {
    async fn predict(&self, horizon: usize) -> Vec<ForecastedPoint> {
        let augurs_pred = self.augurs.predict(horizon).await?;
        let fann_pred = self.fann.predict(horizon).await?;

        // Weighted combination (weights learned from validation)
        combine_forecasts(augurs_pred, fann_pred, &self.weights)
    }
}
```

---

## 5. Risk Assessment

### 5.1 Technical Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| ruv-FANN models too slow on Pi | Medium | High | INT8 quantization, simpler models |
| EWC++ integration complexity | Medium | Medium | Start without EWC++, add later |
| Safetensors loading issues | Low | Medium | Pre-test model format compatibility |
| MCP server conflicts | Low | Low | Use separate MCP tool namespace |

### 5.2 Production Readiness Concerns

**From existing research (`03-rust-ml-frameworks.md`):**

> "Prototype/Research: Excellent candidate for experimentation"
> "Production: Requires thorough evaluation, benchmarking, and risk assessment"
> "Hybrid Approach: Consider using specific components"

**Concerns:**
- Limited production evidence (claims unverified)
- Small community (slower bug fixes)
- Documentation quality varies
- Dependency stability unknown

**Mitigation:**
- Keep augurs as production fallback
- Extensive benchmarking before deployment
- Shadow testing (run both, compare)
- Gradual rollout with monitoring

---

## 6. Benchmarking Plan

### 6.1 Metrics to Measure

| Metric | Target | Measurement Method |
|--------|--------|-------------------|
| Inference latency | <50ms on Pi 5 | `std::time::Instant` |
| Memory footprint | <200MB | `/proc/[pid]/status` |
| Forecast MAE | Better than augurs | Validation dataset |
| Online learning time | <5s per update | Timer around `incremental_update()` |
| EWC++ forgetting | <10% accuracy drop | Historical data test |

### 6.2 Benchmark Protocol

```bash
# 1. Build release binary
cargo build --release --features fann

# 2. Run on Pi 5 with monitoring
./scripts/benchmark_fann.sh \
  --model nhits \
  --horizon 360 \
  --iterations 100 \
  --output results/fann_benchmark.json

# 3. Compare with augurs baseline
./scripts/compare_forecasters.sh \
  --forecasters augurs,fann \
  --dataset validation_2025.parquet
```

---

## 7. Recommendation

### 7.1 Decision Matrix

| Criterion | Weight | augurs | ruv-FANN | Winner |
|-----------|--------|--------|----------|--------|
| Production readiness | 30% | 9 | 5 | augurs |
| Model sophistication | 20% | 5 | 9 | ruv-FANN |
| Online learning | 20% | 3 | 9 | ruv-FANN |
| Edge performance | 15% | 8 | 7 | augurs |
| MCP integration | 10% | 3 | 9 | ruv-FANN |
| Community/support | 5% | 8 | 4 | augurs |
| **Weighted Total** | 100% | **6.4** | **7.1** | **ruv-FANN** |

### 7.2 Final Recommendation

**Complete ruv-FANN integration as Phase 2 neural capability:**

1. **Phase 1 (Now):** Use augurs for production forecasting
2. **Phase 2 (Q2 2026):** Complete ruv-FANN integration
3. **Phase 3 (Q3 2026):** Ensemble (augurs + ruv-FANN)
4. **Phase 4 (Q4 2026):** Online learning with EWC++

**Why both:**
- augurs provides production stability
- ruv-FANN provides neural sophistication + online learning
- Ensemble captures best of both
- Risk is mitigated by fallback to augurs

### 7.3 Immediate Actions

1. **Add to fe-002 scope:** Complete ruv-FANN integration
2. **Benchmark:** Run comparative benchmarks on Pi 5
3. **Model training:** Train NHITS/NBEATSx on NDP historical data
4. **MCP tools:** Design tool interface for neural forecasting

---

## Sources

- [GitHub - ruvnet/ruv-FANN](https://github.com/ruvnet/ruv-FANN)
- [ruv-FANN on Lib.rs](https://lib.rs/crates/ruv-fann)
- [ruv-swarm-ml on Lib.rs](https://lib.rs/crates/ruv-swarm-ml)
- [GitHub - grafana/augurs](https://github.com/grafana/augurs)
- [FOSDEM 2025 - Augurs](https://archive.fosdem.org/2025/schedule/event/fosdem-2025-4668-augurs-a-time-series-toolkit-for-rust/)
- [ruvector-sona (EWC++)](https://github.com/ruvnet/ruvector)
- NDP existing research: `product/research/03-rust-ml-frameworks.md`
- NDP existing code: `core/src/forecast/fann_adapter.rs`
