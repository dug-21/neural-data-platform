# Gold Layer Feature Roadmap v1.3: Self-Learning Edge Intelligence

> **Supersedes:** FEATURE-ROADMAPv1.2.md
> **Created:** 2026-02-18
> **Method:** Working backwards from online learning target, grounded in current codebase state
> **Status:** Draft for Review
> **Prior versions:** `FEATURE-ROADMAP.md` (v1.0), `FEATURE-ROADMAPv1.1.md` (v1.1), `FEATURE-ROADMAPv1.2.md` (v1.2)

---

## What Changed From v1.2 and Why

v1.2 framed intelligence as **K-NN similarity search** — embed Gold data as vectors, find nearest past states, predict by looking up what happened next. That's a lookup table with a distance metric. It doesn't learn. It retrieves.

Three realizations emerged:

### 1. Retrieval Is Not Learning

K-NN has a ceiling. It can only find situations it has seen before, at the resolution it was embedded. It cannot generalize, interpolate, or discover latent structure. A system that embeds and searches is an index, not an intelligence engine.

The platform's goal is **self-learning, continuously improving prediction** — a model that gets better with every observation, adapts to distribution drift, and discovers relationships the operator didn't specify. That requires gradient-based learning, not nearest-neighbor retrieval.

**K-NN remains valuable** as a bootstrapping mechanism (useful while the neural network has no training data) and a validation baseline (compare NN predictions against neighbor-based predictions). But it is not the end state.

### 2. Feature Engineering Was Hiding in the Intelligence Binary

The deployed `ndp-intelligence-app` (v1.2.14) performs:
- Z-score normalization of Gold aligned view rows
- Embedding vector construction
- pgvector storage
- K-NN search
- Prediction generation

Steps 1-3 are **feature engineering** — transforming Gold data into a shape useful for the next consumer. They belong in the Gold layer, not the intelligence layer. The v1.2 roadmap documented this distinction (Section 3: "Feature Engineering ≠ Intelligence") but fe-004 shipped it all in one binary.

The current `ndp-lib::gold` is a **deploy-time DDL generation library** — it creates tables, continuous aggregates, and refresh jobs. It does not run at runtime. Runtime feature engineering (normalization, encoding, feature vector assembly) needs its own library home, separate from both DDL generation and intelligence.

**Fix:** Two library crates with clear boundaries. One binary orchestrates both.

### 3. The Platform Needs a Tiered Intelligence Architecture

A single prediction model can't handle everything. Distribution drift, novel situations, and edge cases require escalation. The architecture needs three tiers:

| Tier | What | Runs | Cost |
|------|------|------|------|
| **Online NN** | Lightweight per-domain MLP, predicts every cycle | Continuously | ~microseconds |
| **SONA Attention** | Meta-learner watching the predictor's error patterns | Continuously | ~milliseconds |
| **External LLM via MCP** | Reasoning engine for novel situations | On anomaly escalation | ~seconds, $ per call |

The NN predicts cheaply and continuously. SONA watches prediction quality as a signal — it learns which error patterns indicate genuine anomalies vs. noise. When SONA detects something the base model can't handle, it escalates to an LLM that can reason about the situation, investigate via MCP tools, and push adjustments back.

The closed loop — predict, observe, learn, escalate when stuck — is the core innovation. Most edge ML systems are deploy-and-forget. This is deploy-and-evolve.

### Summary of Changes

| Aspect | v1.2 | v1.3 |
|--------|------|------|
| Prediction engine | K-NN similarity search | Online neural network |
| Learning | None (retrieval only) | Continuous, every prediction cycle |
| K-NN role | Primary intelligence | Bootstrap + validation baseline |
| SONA role | Future (V1.3) | Core meta-learner (this version) |
| Feature engineering | In intelligence binary | Separate library crate |
| Normalization | Batch z-score | Online EWMA (adapts to drift) |
| Anomaly response | Flag + dashboard | Escalate to LLM via MCP |
| Text embeddings | MiniLM for K-NN search | MiniLM as NN feature input |
| Granger causality | Full pairwise analysis | Lightweight warmup-period feature mask |
| Architecture boundary | Blurred | DDL generation / runtime features / intelligence |

---

## Updated Vision

### The Capability Chain (Revised)

```
V1.0          V1.1           V1.2                    V1.3                V2.0
-----         ----           ----                    ----                ----
Ingest   ->   Prepare    ->  Feature             ->  Self-Learning   ->  Multi-Domain
Data          for Detection  Engineering             Intelligence        via Config
                             Pipeline                Engine

Bronze->Silver Gold Layer    Normalization +         Online NN +         New Domain =
Pipeline       Foundation    Encoding +              SONA Attention +    New Config File
                             Vector Assembly +       MCP Escalation      Same Engine
                             K-NN Baseline
```

### The Key Insight (Updated)

V1.2 built the data pipeline and proved K-NN similarity works. V1.3 replaces the lookup table with an actual learner. The prediction loop becomes:

```
observe features(t) -> predict target(t+h) -> wait ->
observe actual(t+h) -> compute loss -> update weights -> repeat
```

Every prediction becomes a training sample once the actual value arrives. No offline retraining. No model deployment ceremony. The system improves with every observation.

### Three Tiers of Intelligence

```
┌─────────────────────────────────────────────────────────────────────┐
│                    TIER 3: EXTERNAL LLM (on demand)                 │
│                                                                     │
│  Called ONLY when SONA escalates. Uses MCP to investigate:         │
│  - Query external APIs (weather, news, sensor status)              │
│  - Reason about novel situations                                   │
│  - Return adjustment: micro-LoRA update OR config change           │
│                                                                     │
│  Cost: ~seconds, $ per call. Frequency: rare (anomalies only)     │
└──────────────────────────────────┬──────────────────────────────────┘
                                   │ adjustment vector
                                   ▼
┌─────────────────────────────────────────────────────────────────────┐
│                 TIER 2: SONA ATTENTION LAYER (continuous)           │
│                                                                     │
│  Watches the NN's prediction errors as a signal.                   │
│  ReasoningBank stores known error patterns as embeddings.          │
│  Attention mechanism discriminates noise from real shifts.          │
│                                                                     │
│  Low similarity to all known patterns = novel situation = escalate │
│  High similarity to known recovery pattern = apply micro-LoRA     │
│  EWC++ protects prior knowledge during adaptation                  │
│                                                                     │
│  Cost: ~milliseconds. Runs every prediction cycle.                 │
└──────────────────────────────────┬──────────────────────────────────┘
                                   │ anomaly signals
                                   ▼
┌─────────────────────────────────────────────────────────────────────┐
│               TIER 1: ONLINE NEURAL NETWORK (continuous)           │
│                                                                     │
│  Per-domain MLP. Takes normalized feature vector, predicts next.  │
│  Updates weights every cycle via online gradient descent.           │
│  Cheap, fast, always running.                                      │
│                                                                     │
│  Cost: ~microseconds. Runs every Gold refresh (15 min).            │
└──────────────────────────────────┬──────────────────────────────────┘
                                   │ reads
                                   ▼
┌─────────────────────────────────────────────────────────────────────┐
│                 FEATURE ENGINEERING LAYER (continuous)              │
│                                                                     │
│  EWMA normalization (adapts to drift)                              │
│  Binary state temporal encoding                                     │
│  Text categorical/embedding encoding                               │
│  Feature vector assembly (flat Vec<f32> per timestep)              │
│  Writes prepared features to Gold tables                           │
│                                                                     │
│  Cost: ~milliseconds. Runs every Gold CA refresh.                  │
└─────────────────────────────────────────────────────────────────────┘
                                   ▲ reads
                                   │
┌─────────────────────────────────────────────────────────────────────┐
│                    GOLD LAYER (existing, unchanged)                 │
│                                                                     │
│  Continuous aggregates, aligned views, text views, feature tables  │
│  DDL generated at deploy time by ndp-lib::gold                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Architecture: Library and Binary Layout

### The Separation

Three concerns, two library crates, one binary:

```
crates/ndp-lib/src/gold/               # DEPLOY-TIME DDL GENERATION (existing)
  generators/
    continuous_aggregate.rs             # CREATE MATERIALIZED VIEW (CAs)
    aligned_view.rs                     # Cross-stream JOIN views
    text_view.rs                        # Text field views (dp-023)
    features_table.rs                   # NEW — DDL for normalized feature tables
    normalization_state.rs              # NEW — DDL for running stats persistence
  registry/
    lag.rs, rolling.rs, trend.rs        # Feature generators (existing)

crates/ndp-features/                    # RUNTIME FEATURE ENGINEERING (new crate)
  Cargo.toml
  src/
    lib.rs                              # Public API
    config.rs                           # FeatureConfig (per-domain)
    normalization/
      mod.rs                            # Normalizer trait
      ewma.rs                           # EWMA online z-score
      minmax.rs                         # MinMax (for bounded features)
      passthrough.rs                    # No normalization (binary features)
    encoding/
      mod.rs                            # Encoder trait
      numeric.rs                        # Numeric field encoding
      binary.rs                         # Binary state + temporal derivation
      text.rs                           # Text categorical/embedding encoding
    assembly.rs                         # Feature vector assembly (concat all encodings)
    state.rs                            # Persistent normalization state (read/write Gold tables)
    embeddings/                         # MOVED from ndp-intelligence
      mod.rs                            # Embedder trait
      metric.rs                         # MetricEmbedder (z-score normalize)
      event.rs                          # EventEmbedder (MiniLM + template cache)
      composite.rs                      # CompositeEmbedder (combines both)

crates/ndp-intelligence/                # INTELLIGENCE: LEARNING ENGINE (rebuilt)
  Cargo.toml
  src/
    lib.rs                              # Public API
    config.rs                           # IntelligenceConfig
    network/
      mod.rs                            # Network trait
      mlp.rs                            # Online MLP (nalgebra or burn)
      weights.rs                        # Weight persistence + checkpointing
      ewc.rs                            # Elastic Weight Consolidation
    prediction/
      mod.rs                            # Prediction loop orchestration
      error_tracker.rs                  # Rolling error statistics
      confidence.rs                     # Prediction confidence estimation
    sona/
      mod.rs                            # SONA integration orchestration
      reasoning_bank.rs                 # Pattern storage (trajectory embeddings)
      attention.rs                      # Anomaly discrimination
      micro_lora.rs                     # Fast adaptation without forgetting
    escalation/
      mod.rs                            # Escalation decision logic
      context.rs                        # Context package assembly
      mcp_bridge.rs                     # MCP call to external LLM
      adjustment.rs                     # Apply returned adjustments
    baseline/
      knn.rs                            # K-NN search (MOVED from current intelligence)
      granger.rs                        # Lightweight Granger for warmup
    storage.rs                          # Predictions, error history, SONA state

apps/ndp-intelligence-app/              # BINARY (orchestrates both crates)
  Cargo.toml
  src/
    main.rs                             # PG NOTIFY listener, cycle orchestration
                                        # Calls ndp-features for preparation,
                                        # then ndp-intelligence for learning
```

### What Moves, What's New, What Stays

| Component | Currently | Destination | Status |
|-----------|-----------|-------------|--------|
| DDL generators (CAs, views, text) | `ndp-lib::gold` | Stays | No change |
| Z-score normalization | `ndp-intelligence` | `ndp-features::normalization` | **Move** |
| Embedding construction | `ndp-intelligence` | `ndp-features::embeddings` | **Move** |
| pgvector write | `ndp-intelligence` | `ndp-features` | **Move** |
| K-NN search | `ndp-intelligence` | `ndp-intelligence::baseline` | Stays (demoted) |
| Prediction from neighbors | `ndp-intelligence` | `ndp-intelligence::baseline` | Stays (demoted) |
| EWMA online normalization | — | `ndp-features::normalization::ewma` | **New** |
| Binary state encoding | — | `ndp-features::encoding::binary` | **New** |
| Text encoding | — | `ndp-features::encoding::text` | **New** |
| Feature vector assembly | — | `ndp-features::assembly` | **New** |
| Normalization state persistence | — | `ndp-features::state` | **New** |
| Online MLP | — | `ndp-intelligence::network::mlp` | **New** |
| EWC regularization | — | `ndp-intelligence::network::ewc` | **New** |
| Error tracking | — | `ndp-intelligence::prediction::error_tracker` | **New** |
| SONA integration | — | `ndp-intelligence::sona` | **New** |
| MCP escalation bridge | — | `ndp-intelligence::escalation` | **New** |
| PG NOTIFY + cycle orchestration | `ndp-intelligence-app` | Stays | Refactored |

---

## The Online Learning Core

### EWMA Normalization (the bridge)

The Gold layer produces raw aggregates: `AVG(pm25) = 12.3`, `STDDEV(co2) = 45.7`. These are not NN-ready. Batch normalization requires seeing all data upfront. Online learning requires **online normalization** that adapts with each observation.

EWMA z-score maintains a running estimate of mean and variance per feature:

```
u(t) = a * x(t) + (1-a) * u(t-1)          # running mean
s2(t) = a * (x(t) - u(t))^2 + (1-a) * s2(t-1)  # running variance
z(t) = (x(t) - u(t)) / sqrt(s2(t))        # normalized value
```

Where `a` (alpha, decay factor) controls how quickly the normalizer adapts:
- `a = 0.01` — slow adaptation, stable features (hourly metrics)
- `a = 0.05` — moderate, default for most streams
- `a = 0.1` — fast adaptation, volatile features

**Normalization state must persist.** Running mean and variance per feature per domain are stored in a Gold table and survive restarts. Without this, the system re-normalizes from scratch after every restart, producing garbage predictions until warmup completes.

```sql
CREATE TABLE gold.normalization_state (
    domain_id       TEXT NOT NULL,
    feature_name    TEXT NOT NULL,
    running_mean    DOUBLE PRECISION NOT NULL,
    running_var     DOUBLE PRECISION NOT NULL,
    observation_count BIGINT NOT NULL,
    last_updated    TIMESTAMPTZ NOT NULL,
    alpha           DOUBLE PRECISION NOT NULL,
    PRIMARY KEY (domain_id, feature_name)
);
```

### Feature Vector Assembly

At each Gold refresh tick, the feature assembler:

1. Reads the latest aligned view row (numeric fields)
2. Reads binary state features (current state, time-since-transition, event frequency)
3. Reads text encodings (categorical features or reduced embeddings)
4. Applies per-field EWMA normalization
5. Concatenates into a flat `Vec<f32>` of fixed length per domain
6. Optionally includes lag steps (configurable window depth)

Output: one feature vector per timestep per domain. This is the universal NN input regardless of what data domain is configured.

```rust
/// Assembled feature vector for one timestep
pub struct FeatureVector {
    pub domain_id: String,
    pub bucket: DateTime<Utc>,
    pub features: Vec<f32>,          // flat normalized vector
    pub feature_names: Vec<String>,  // for interpretability
    pub lag_depth: usize,            // how many past steps included
}
```

### Online MLP

The simplest possible neural network that can learn online:

```
Input (feature_dim * (1 + lag_depth))
  -> Dense(hidden_1) -> ReLU
  -> Dense(hidden_2) -> ReLU
  -> Dense(prediction_targets)
```

Two hidden layers, configurable width. For air quality with ~32 features and 6 lag steps: input = 224, hidden = 64, output = number of prediction targets.

The learning loop:

```
every Gold refresh:
  1. features = feature_assembler.assemble(latest_gold_row)
  2. prediction = network.forward(features)
  3. store prediction with timestamp
  4. actual = look up actual values for previous prediction
  5. if actual available:
       loss = mse(previous_prediction, actual)
       gradients = backprop(loss)
       apply EWC regularization to gradients
       network.update_weights(gradients, learning_rate)
       error_tracker.record(loss)
```

**Weight persistence:** Network weights checkpoint to a Gold table periodically and on shutdown. On startup, load latest checkpoint. If no checkpoint exists, initialize randomly and enter warmup period.

```sql
CREATE TABLE gold.network_state (
    domain_id       TEXT NOT NULL,
    checkpoint_id   BIGSERIAL,
    bucket          TIMESTAMPTZ NOT NULL,
    weights_blob    BYTEA NOT NULL,           -- serialized network weights
    optimizer_state BYTEA,                     -- Adam/SGD state
    ewc_fisher      BYTEA,                     -- Fisher information matrix
    training_steps  BIGINT NOT NULL,
    validation_loss DOUBLE PRECISION,
    PRIMARY KEY (domain_id, checkpoint_id, bucket)
);
SELECT create_hypertable('gold.network_state', 'bucket');
```

### Elastic Weight Consolidation (EWC)

The single most important mechanism for continuous learning. Without it, the network forgets winter patterns by summer.

EWC adds a regularization term to the loss:

```
total_loss = prediction_loss + lambda * SUM(F_i * (w_i - w_i*)^2)
```

Where `F_i` is the Fisher information for weight `i` (how important was this weight for past predictions?) and `w_i*` are the weights at the last consolidation point.

**Consolidation happens periodically** (configurable, e.g., weekly). The system snapshots the current Fisher information matrix alongside the weights. This tells future gradient updates: "these weights matter for what I've already learned — don't change them too much."

### K-NN Baseline (Demoted)

The existing K-NN similarity search continues running in parallel as a validation baseline:

- During warmup (NN has < N training samples): K-NN predictions are primary
- After warmup: NN predictions are primary, K-NN provides comparison
- Divergence between NN and K-NN predictions is itself a useful signal for SONA

---

## SONA Integration

### Where SONA Fits

SONA is not the predictor. It's the **meta-learner** — it watches the predictor and handles what the predictor can't.

```
NN Predictor output (predictions + errors)
        |
        v
SONA Observation
  - Current error pattern (vector of recent errors)
  - Feature context (what inputs drove this prediction)
  - NN internal state (activation patterns, confidence)
        |
        v
ReasoningBank Comparison
  - "Have I seen this error pattern before?"
  - High similarity to known pattern -> apply stored micro-LoRA
  - Low similarity to all patterns -> NOVEL SITUATION
        |
        +--> Known recovery pattern: apply micro-LoRA (fast, local)
        |
        +--> Novel situation: escalate to Tier 3 (LLM investigation)
```

### SONA Components Mapped to Architecture

| SONA Component | Role in This Architecture |
|---------------|--------------------------|
| **ReasoningBank** | Stores embeddings of known error-pattern/recovery-action pairs |
| **Trajectory recording** | Each prediction cycle = trajectory (input, prediction, error, outcome) |
| **Micro-LoRA** | Fast weight adjustment when SONA recognizes a familiar shift (e.g., "seasonal transition pattern #3 — adjust temperature sensitivity") |
| **EWC++** | Already integrated in the NN layer — SONA's adjustments are protected the same way |
| **Attention mechanism** | Discriminates "noisy Thursday" from "distribution shifted" by learning which error-pattern dimensions matter |

### Anomaly Detection as Escalation Trigger

The prediction error itself is the anomaly signal. Not a separate system — the NN's mistakes are the data SONA analyzes.

```rust
pub struct PredictionCycle {
    pub bucket: DateTime<Utc>,
    pub features: Vec<f32>,
    pub prediction: Vec<f32>,
    pub actual: Option<Vec<f32>>,
    pub error: Option<Vec<f32>>,
    pub nn_confidence: f32,
    pub knn_prediction: Option<Vec<f32>>,    // baseline comparison
    pub knn_divergence: Option<f32>,          // NN vs K-NN disagreement
}
```

SONA embeds the error vector and compares against the ReasoningBank. The decision:

| Similarity to known patterns | Action |
|------------------------------|--------|
| High similarity, known recovery | Apply stored micro-LoRA, no escalation |
| High similarity, known noise | Ignore, no action |
| Low similarity, high error magnitude | **Escalate to LLM** |
| Low similarity, low error magnitude | Log as novel, add to observation buffer |

---

## MCP Escalation (Tier 3)

### When It Triggers

SONA escalates when it detects a situation it can't handle locally:
- Error pattern has low similarity to all known patterns in ReasoningBank
- Error magnitude exceeds configurable threshold
- NN and K-NN predictions diverge significantly (both are confused)

### Context Package

The escalation sends a structured context to the external LLM:

```rust
pub struct EscalationContext {
    pub domain_id: String,
    pub bucket: DateTime<Utc>,
    pub recent_features: Vec<FeatureVector>,   // last N timesteps
    pub recent_errors: Vec<f32>,               // error trajectory
    pub nn_prediction: Vec<f32>,
    pub knn_prediction: Option<Vec<f32>>,
    pub sona_similarity_scores: Vec<f32>,      // how novel is this?
    pub feature_drift_report: HashMap<String, f32>, // which features shifted
    pub domain_config: DomainConfig,           // what this domain monitors
}
```

### LLM Investigation via MCP

The LLM receives the context and has MCP tools available to investigate:

| MCP Tool | Purpose | Example |
|----------|---------|---------|
| `query_external_api` | Check external data sources | NWS alerts, AirNow advisories |
| `query_gold_history` | Look up past similar situations | "Last time features looked like this..." |
| `query_predictions` | Review recent prediction accuracy | "How has the model been doing?" |
| `check_system_health` | Rule out data quality issues | "Is the sensor reporting correctly?" |

### Adjustment Response

The LLM returns one of:

| Response | Effect |
|----------|--------|
| `false_alarm` | SONA records as noise pattern in ReasoningBank |
| `micro_lora_adjustment(vector)` | SONA applies targeted weight adjustment |
| `config_update(changes)` | Modify domain config (new feature weights, lag depth) |
| `learning_rate_adjust(factor)` | Temporarily speed up or slow down learning |
| `checkpoint_rollback(id)` | Revert to a previous weight checkpoint |
| `observation_note(text)` | Log context for future reference (no model change) |

The adjustment propagates back through SONA into the NN. EWC++ protects existing knowledge during the adjustment.

---

## Domain Configuration

### Per-Domain Config (Updated)

Each domain is a configuration file. Same engine, different parameters.

```json
{
  "domain_id": "indoor_air_quality",
  "feature_engineering": {
    "numeric_streams": [
      {
        "stream_id": "purpleair",
        "fields": ["pm25_avg", "co2_avg", "temperature_avg", "humidity_avg", "voc_avg"],
        "normalization": { "method": "ewma", "alpha": 0.02 }
      },
      {
        "stream_id": "nws-observations",
        "fields": ["temperature_c_avg", "humidity_avg", "wind_speed_avg", "pressure_avg"],
        "normalization": { "method": "ewma", "alpha": 0.01 }
      }
    ],
    "binary_streams": [
      {
        "stream_id": "home-assistant-state",
        "fields": ["window_state", "door_state"],
        "derive_temporal": true,
        "temporal_features": ["time_since_transition", "event_frequency_1h"]
      }
    ],
    "text_config": {
      "stream_id": "nws-forecast-hourly",
      "encoding": "categorical",
      "categories": {
        "weather_condition": ["clear", "cloudy", "rain", "snow", "fog", "storm"],
        "air_quality_signal": ["stagnation", "inversion", "smoke", "front", "wind"]
      },
      "embedding_fallback": {
        "enabled": true,
        "model": "all-MiniLM-L6-v2",
        "pca_dims": 16
      }
    },
    "lag_depth": 6,
    "feature_mask": null
  },
  "intelligence": {
    "network": {
      "hidden_layers": [64, 64],
      "learning_rate": 0.001,
      "ewc_lambda": 0.4,
      "consolidation_interval": "7 days",
      "warmup_steps": 168
    },
    "prediction_targets": ["pm25_avg", "co2_avg"],
    "prediction_horizons": ["1 hour", "4 hours"],
    "sona": {
      "enabled": true,
      "reasoning_bank_capacity": 1000,
      "anomaly_threshold": 0.3,
      "micro_lora_rank": 4
    },
    "escalation": {
      "enabled": true,
      "mcp_endpoint": "http://localhost:8080/mcp",
      "min_error_magnitude": 2.0,
      "max_escalations_per_day": 10
    },
    "baseline": {
      "knn_enabled": true,
      "knn_k": 20,
      "knn_min_similarity": 0.7
    }
  }
}
```

### Adding a New Domain

Adding a second domain (e.g., sysops/observability) requires:

1. New stream configs (Docker metrics, system metrics, Docker logs)
2. New Silver hypertable schemas
3. New Gold CA and aligned view DDL
4. New domain intelligence config (JSON file above)
5. **Zero changes to ndp-features or ndp-intelligence crates**

The engine reads the config, creates the appropriate normalizers, assembles feature vectors of the right shape, initializes a network with matching input/output dimensions, and starts the learning loop.

---

## Updated Version Dependency Chain

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                V2.0: MULTI-DOMAIN INTELLIGENCE                              │
│                                                                              │
│  "Add a new domain via config file. Same engine discovers patterns,        │
│   learns relationships, and predicts — with zero code changes."            │
│                                                                              │
│  REQUIRES FROM V1.3:                                                        │
│  * Online NN proven on air quality (Tier 1 operational)                    │
│  * SONA meta-learning proven (Tier 2 operational)                          │
│  * MCP escalation proven (Tier 3 operational)                              │
│  * EWC++ prevents forgetting across seasonal cycles                        │
│  NEW IN V2.0:                                                               │
│  + Second domain (sysops) added via config only                            │
│  + Cross-domain signals (weather affects operations)                       │
│  + MCP query interface for external agents/UI                              │
│  + Per-domain SONA instances with shared escalation infrastructure         │
└─────────────────────────────────────────────────────────────────────────────┘
                                      |
                                      v
┌─────────────────────────────────────────────────────────────────────────────┐
│         V1.3: SELF-LEARNING EDGE INTELLIGENCE                               │
│                                                                              │
│  "System predicts, learns from every observation, detects anomalies,       │
│   and escalates what it can't handle locally."                              │
│                                                                              │
│  REQUIRES FROM V1.2:                                                        │
│  * Gold layer pipeline (CAs, aligned views, text views) ...... COMPLETE    │
│  * K-NN similarity baseline ................................ COMPLETE       │
│  * pgvector embedding storage ............................... COMPLETE      │
│  * Three data buckets (numeric, binary, text) flowing ....... COMPLETE     │
│  * Time-aligned Gold data ................................... COMPLETE      │
│  NEW IN V1.3:                                                               │
│  + ndp-features crate (online normalization, encoding, assembly)           │
│  + ndp-intelligence rebuilt (online MLP, EWC, error tracking)              │
│  + SONA integration (ReasoningBank, attention, micro-LoRA)                 │
│  + MCP escalation bridge (context packaging, LLM investigation)            │
│  + Normalization state persistence                                          │
│  + Network weight checkpointing                                             │
│  + K-NN demoted to validation baseline                                      │
│  + Lightweight Granger for warmup-period feature masking                   │
└─────────────────────────────────────────────────────────────────────────────┘
                                      |
                                      v
┌─────────────────────────────────────────────────────────────────────────────┐
│                    V1.2: GOLD LAYER + K-NN BASELINE .... MOSTLY COMPLETE    │
│                                                                              │
│  Config-driven Gold DDL, continuous aggregates, aligned views,              │
│  text field pipeline (dp-023), K-NN similarity intelligence (fe-004),      │
│  pgvector embeddings, predictions.                                          │
│                                                                              │
│  Remaining V1.2 work:                                                       │
│  * dp-023 commit + release (text field pipeline)                            │
│  * fe-005 text embeddings (MiniLM, feeds into V1.3 feature layer)          │
└─────────────────────────────────────────────────────────────────────────────┘
                                      |
                                      v
┌─────────────────────────────────────────────────────────────────────────────┐
│                    V1.1: GOLD LAYER FOUNDATION ........... COMPLETE          │
│                                                                              │
│  Config-driven Gold DDL generation, continuous aggregates,                  │
│  aligned views, feature registry, events infrastructure.                    │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## V1.3 Feature Breakdown

### Track A: Feature Engineering Infrastructure

Foundation that every subsequent track depends on.

| ID | Feature | Description | Depends On |
|----|---------|-------------|------------|
| **v13-F01** | ndp-features crate scaffold | New workspace member, public API, config types | — |
| **v13-F02** | EWMA Normalizer | Online z-score with configurable alpha, warmup detection | v13-F01 |
| **v13-F03** | Normalization state DDL + persistence | Gold table for running stats, read/write on startup/shutdown | v13-F02 |
| **v13-F04** | Numeric encoder | Reads aligned view row, applies per-field normalization | v13-F02 |
| **v13-F05** | Binary state encoder | Raw state + derived temporal features (time-since-transition, frequency) | v13-F02 |
| **v13-F06** | Text encoder | Categorical mapping from domain config + optional MiniLM embedding reduction | v13-F01, fe-005 |
| **v13-F07** | Feature vector assembler | Concatenates all encodings into flat Vec<f32> with lag window | v13-F04, F05, F06 |
| **v13-F08** | Feature mask (Granger warmup) | Lightweight cross-correlation during initial observation period | v13-F07 |
| **v13-F09** | Move embeddings from intelligence | MetricEmbedder, EventEmbedder, CompositeEmbedder relocate to ndp-features | v13-F01 |

### Track B: Online Learning Core

The prediction engine.

| ID | Feature | Description | Depends On |
|----|---------|-------------|------------|
| **v13-L01** | Online MLP implementation | Forward pass, backprop, weight update in pure Rust | v13-F07 |
| **v13-L02** | Prediction loop | Observe-predict-wait-learn cycle wired to PG NOTIFY | v13-L01 |
| **v13-L03** | Weight persistence | Checkpoint weights to Gold table, restore on startup | v13-L01 |
| **v13-L04** | EWC implementation | Fisher information computation, regularized loss | v13-L01 |
| **v13-L05** | EWC consolidation schedule | Periodic Fisher snapshot (configurable interval) | v13-L04 |
| **v13-L06** | Error tracker | Rolling error statistics, distribution shape tracking | v13-L02 |
| **v13-L07** | Prediction confidence | Confidence estimation from error history + NN output variance | v13-L06 |
| **v13-L08** | K-NN baseline integration | Existing K-NN runs in parallel, predictions compared | v13-L02 |
| **v13-L09** | Warmup mode | K-NN primary during warmup, NN takes over after N training steps | v13-L08 |

### Track C: SONA Meta-Learning

The attention layer that watches the predictor.

| ID | Feature | Description | Depends On |
|----|---------|-------------|------------|
| **v13-S01** | SONA integration scaffold | ruvector-sona wired into ndp-intelligence | v13-L06 |
| **v13-S02** | Trajectory recording | Each prediction cycle → SONA trajectory | v13-S01, L02 |
| **v13-S03** | ReasoningBank population | Error patterns stored as embeddings after trajectory end | v13-S02 |
| **v13-S04** | Attention-based anomaly detection | Discriminate noise from genuine distribution shift | v13-S03 |
| **v13-S05** | Micro-LoRA fast adaptation | Apply targeted weight adjustment for recognized patterns | v13-S04 |
| **v13-S06** | EWC++ protection during adaptation | SONA adjustments respect EWC constraints | v13-S05, L04 |

### Track D: MCP Escalation

The external reasoning tier.

| ID | Feature | Description | Depends On |
|----|---------|-------------|------------|
| **v13-M01** | Escalation decision logic | When SONA similarity < threshold AND error > threshold | v13-S04 |
| **v13-M02** | Context package assembly | Bundle recent features, errors, SONA scores, drift report | v13-M01 |
| **v13-M03** | MCP bridge | Send context to configured LLM endpoint, receive response | v13-M02 |
| **v13-M04** | Adjustment application | Parse LLM response, apply micro-LoRA / config change / rollback | v13-M03 |
| **v13-M05** | Escalation logging | Record all escalations, LLM responses, and applied adjustments | v13-M03 |
| **v13-M06** | Rate limiting | Configurable max escalations per time window | v13-M01 |

### Track E: Infrastructure + Observability

| ID | Feature | Description | Depends On |
|----|---------|-------------|------------|
| **v13-I01** | Feature tables DDL generator | DDL for normalization_state, feature_vectors, network_state | — |
| **v13-I02** | Binary reorganization | intelligence-app calls ndp-features then ndp-intelligence | v13-F09, L02 |
| **v13-I03** | Intelligence CLI update | `ndp intelligence status/train/predict/escalation-log` | v13-L02 |
| **v13-I04** | Grafana dashboards | Prediction accuracy over time, error distribution, SONA events, escalations | v13-L06, S04 |
| **v13-I05** | Docker service update | Container limit 512MB, weight checkpoint volume | v13-I02 |

---

## Implementation Phases

### Phase 1: Feature Engineering Foundation (3-4 weeks)

Build the bridge between Gold layer and any neural component.

| Week | Features | Exit Criteria |
|------|----------|---------------|
| 1 | v13-F01, F02, F03, I01 (crate scaffold, EWMA, state persistence, DDL) | EWMA normalizer produces z-scored values, state survives restart |
| 2 | v13-F04, F05 (numeric + binary encoders) | Numeric and binary features normalized and encoded |
| 3 | v13-F06, F07 (text encoder, vector assembler) | Full feature vector assembled from all three buckets |
| 4 | v13-F08, F09, I02 (feature mask, move embeddings, binary reorg) | Feature engineering separated from intelligence, lightweight Granger produces mask |

### Phase 2: Online Learning Core (3-4 weeks)

Build the simplest NN that learns online, end-to-end.

| Week | Features | Exit Criteria |
|------|----------|---------------|
| 1 | v13-L01, L02 (MLP implementation, prediction loop) | NN predicts and updates weights every cycle |
| 2 | v13-L03, L04, L05 (weight persistence, EWC, consolidation) | Weights survive restart, EWC prevents forgetting |
| 3 | v13-L06, L07 (error tracker, confidence) | Error statistics tracked, confidence scores on predictions |
| 4 | v13-L08, L09, I03 (K-NN baseline, warmup mode, CLI) | K-NN and NN run in parallel, warmup-to-primary transition works |

### Phase 3: SONA + Escalation (4-5 weeks)

Layer the meta-learner and external reasoning.

| Week | Features | Exit Criteria |
|------|----------|---------------|
| 1 | v13-S01, S02 (SONA scaffold, trajectory recording) | Each prediction cycle records a SONA trajectory |
| 2 | v13-S03, S04 (ReasoningBank, anomaly detection) | SONA discriminates noise from genuine shifts |
| 3 | v13-S05, S06 (micro-LoRA, EWC++ protection) | SONA applies fast adaptation without forgetting |
| 4 | v13-M01, M02, M03 (escalation logic, context, MCP bridge) | Anomalies escalate to external LLM, responses received |
| 5 | v13-M04, M05, M06, I04, I05 (adjustments, logging, dashboards, Docker) | Full three-tier loop operational, observable in Grafana |

### Phase 4: Validation + Second Domain (V2.0 prep, 3-4 weeks)

Prove the system works and generalizes.

| Week | Features | Exit Criteria |
|------|----------|---------------|
| 1-2 | Air quality validation | NN predictions improve over 2-week window, SONA detects injected anomalies |
| 3-4 | Sysops domain config (V2.0 preview) | Second domain via config only, zero code changes to ndp-features or ndp-intelligence |

---

## Acceptance Criteria

| Criterion | Target | Measurement |
|-----------|--------|-------------|
| EWMA normalization operational | All configured features normalized online | Feature vector assembly produces correct output |
| Feature vector persistence | Normalization state survives restart | Kill process, restart, verify running stats preserved |
| NN prediction loop running | Predictions generated every Gold refresh cycle | `SELECT count(*) FROM gold.predictions WHERE method = 'nn'` |
| NN improves over time | Prediction error decreases over 2-week window | Rolling error stats trend downward |
| EWC prevents forgetting | Inject seasonal shift, verify old patterns retained | Prediction accuracy on historical patterns after shift |
| Weight checkpointing | NN resumes from checkpoint after restart | Kill, restart, verify first prediction uses learned weights |
| K-NN baseline comparison | NN >= K-NN accuracy after warmup | A/B comparison logged per cycle |
| SONA anomaly detection | Detects injected distribution shifts | Inject shift, verify SONA flags within 2 cycles |
| SONA micro-LoRA | Recognized pattern triggers fast adaptation | Inject known shift type, verify adaptation < 1 cycle |
| MCP escalation fires | Novel anomaly triggers LLM investigation | Inject unknown shift, verify escalation context sent |
| LLM adjustment applied | Adjustment propagates back to NN | Verify weight change after LLM response |
| Domain config only | New domain requires zero code changes | Add sysops config, verify full pipeline starts |
| Pi resource budget | Intelligence container < 512MB | `docker stats` under full load |
| Startup time | < 60s including weight restoration | Measured on Pi |

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Online MLP diverges on bad data | Medium | High | Gradient clipping, learning rate bounds, validation check before accepting weight update |
| EWC lambda too conservative (underfitting) | Medium | Medium | Configurable per-domain, tune during warmup |
| EWC lambda too aggressive (forgetting) | Medium | High | Conservative default (0.4), monitor old-pattern accuracy |
| SONA ReasoningBank grows unbounded | Low | Medium | Capacity limit with LRU eviction of low-utility patterns |
| MCP escalation too frequent (cost) | Medium | Medium | Rate limiting (v13-M06), escalation threshold tuning |
| MCP escalation too rare (misses real anomalies) | Low | High | K-NN divergence as secondary trigger, error magnitude floor |
| SONA micro-LoRA corrupts weights | Low | High | EWC++ constrains adjustment, checkpoint before applying |
| Normalization warmup period too long | Medium | Low | Configurable warmup_steps, seed from historical Gold data |
| Feature vector dimension mismatch across restarts | Low | High | Feature names stored alongside vectors, validate on load |
| nalgebra/burn too slow on ARM for online learning | Low | Low | Network is tiny (2 layers, 64 neurons), inference is microseconds |
| Second domain breaks assumptions | Medium | Medium | This is exactly why V2.0 does it — discover gaps early |
| Concept drift vs. noise distinction too hard | High | Medium | SONA attention is specifically designed for this; escalation is the fallback |

---

## Pi Memory Budget (Updated)

| Component | Container | Memory | Cumulative | % of 16GB |
|-----------|-----------|--------|-----------|-----------|
| air-quality-app | ingestion | 123 MB | — | — |
| timescaledb + pgvector | database | 328 MB | — | — |
| grafana, etcd, mqtt | services | 195 MB | — | — |
| **Existing NDP** | | **646 MB** | **646 MB** | **4.0%** |
| ndp-features (normalization, encoding) | intelligence | +15 MB | 661 MB | 4.1% |
| MiniLM ONNX (text encoding, on demand) | intelligence | +200 MB | 861 MB | 5.3% |
| Online MLP (weights, optimizer state) | intelligence | +5 MB | 866 MB | 5.3% |
| EWC Fisher matrix | intelligence | +2 MB | 868 MB | 5.3% |
| ruvector-core HNSW (K-NN baseline) | intelligence | +2 MB | 870 MB | 5.3% |
| **V1.3 without SONA** | | **+224 MB** | **870 MB** | **5.3%** |
| ruvector-sona (ReasoningBank) | intelligence | +50 MB | 920 MB | 5.6% |
| SONA micro-LoRA matrices | intelligence | +1 MB | 921 MB | 5.6% |
| **V1.3 with SONA** | | **+275 MB** | **921 MB** | **5.6%** |

Intelligence container limit: **512MB**. At V1.3 with SONA, intelligence uses ~275 MB. Comfortable headroom. MiniLM remains the largest single cost and is only loaded when text streams are configured.

---

## Decision Log

| # | Decision | Rationale |
|---|----------|-----------|
| 1-10 | *(Unchanged from v1.2)* | See FEATURE-ROADMAPv1.2.md |
| 11-19 | *(Unchanged from v1.2)* | See FEATURE-ROADMAPv1.2.md |
| 20 | **Online learning replaces K-NN as primary intelligence** | K-NN retrieves, it doesn't learn. The platform's goal is continuous self-improvement, not similarity lookup. K-NN remains as bootstrap + baseline. |
| 21 | **EWMA normalization, not batch z-score** | Online learning requires normalization that adapts without seeing all data. EWMA running stats with configurable decay factor handle distribution drift naturally. |
| 22 | **Feature engineering gets its own crate (ndp-features)** | Deploy-time DDL generation (ndp-lib::gold) and runtime feature engineering (normalization, encoding, assembly) are different concerns with different execution models. Clean library boundary, single binary. |
| 23 | **One binary, two library crates** | Feature engineering and intelligence are tightly temporally coupled (features → learning, every cycle). Separate processes add coordination complexity for no benefit on a single Pi. Library boundary enables future split if needed. |
| 24 | **EWC from day one** | Catastrophic forgetting is the #1 risk in continuous learning. Adding EWC later means retraining from scratch. Adding it early means the model naturally preserves seasonal knowledge. |
| 25 | **Tiered architecture: NN → SONA → LLM** | Each tier handles different failure modes. NN handles routine prediction. SONA handles recognized anomalies. LLM handles novel situations. Cost scales with rarity. |
| 26 | **Prediction error as primary anomaly signal** | The NN's mistakes are inherently the best indicator of "something changed." No separate anomaly detection system needed — the learning loop IS the anomaly detector. |
| 27 | **MCP for external investigation** | When local intelligence is stuck, leverage external reasoning. MCP already exists as a protocol. The Pi becomes a sensor that can ask for help. |
| 28 | **Normalization state persists in Gold** | Running mean/variance per feature must survive restarts. Gold table is the natural home — same durability guarantees as other Gold data. |
| 29 | **Lightweight Granger replaces full pairwise analysis** | The NN learns feature importance through backprop. Full Granger is redundant. A warmup-period cross-correlation produces a feature mask, then the NN takes over. |
| 30 | **Start with nalgebra MLP, not SONA, for Tier 1** | Prove the prediction loop works end-to-end before adding sophistication. SONA layers on top of a working predictor. |

---

## Appendix A: Crate Dependencies (Updated)

**Feature engineering crate** (`crates/ndp-features/Cargo.toml`):

```toml
[dependencies]
# ONNX Runtime for text encoding (MiniLM)
ort = { version = "2", features = ["download-binaries"] }
tokenizers = "0.21"

# Standard NDP deps
tokio = { version = "1", features = ["full"] }
tokio-postgres = "0.7"
ndp-types = { path = "../ndp-types" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
tracing = "0.1"
```

**Intelligence crate** (`crates/ndp-intelligence/Cargo.toml`):

```toml
[dependencies]
ndp-features = { path = "../ndp-features" }
ruvector-core = { version = "2.0.1" }
# ruvector-sona = { version = "0.1" }  # Phase 3

# Linear algebra for MLP
nalgebra = "0.33"

# Standard NDP deps
tokio = { version = "1", features = ["full"] }
tokio-postgres = "0.7"
ndp-types = { path = "../ndp-types" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
tracing = "0.1"
ndarray = "0.16"
```

**Binary** (`apps/ndp-intelligence-app/Cargo.toml`):

```toml
[dependencies]
ndp-features = { path = "../../crates/ndp-features" }
ndp-intelligence = { path = "../../crates/ndp-intelligence" }
tokio = { version = "1", features = ["full"] }
clap = { version = "4", features = ["derive"] }
tracing = "0.1"
tracing-subscriber = "0.3"
```

## Appendix B: Feature Dimensions Reference (Updated)

### Air Quality Feature Vector (~32 raw features + lag)

| Category | Fields | Raw Dims |
|----------|--------|----------|
| Temporal | hour_sin, hour_cos, is_weekend | 3 |
| Indoor Air | co2, pm25, temp, humidity, voc | 5 |
| Outdoor Weather | temp, humidity, wind_speed, pressure | 4 |
| Outdoor AQI | pm25, aqi | 2 |
| Binary State (raw) | window_state, door_state | 2 |
| Binary State (derived) | window_time_since, door_time_since, window_freq_1h, door_freq_1h | 4 |
| Statistical | co2_stddev, pm25_stddev, co2_trend_4h, pm25_trend_4h | 4 |
| Text (categorical) | weather_condition (one-hot 6), air_quality_signal (multi-hot 5) | 11 |
| **Total raw** | | **~35** |
| **With 6 lag steps** | | **~245** |

### Network Shape (Air Quality)

```
Input:  245 (35 features x 7 timesteps [current + 6 lag])
Hidden: 64 -> ReLU -> 64 -> ReLU
Output: 4 (pm25_1h, co2_1h, pm25_4h, co2_4h)
```

Total parameters: ~20,000. Weight checkpoint: ~80KB. Trivial on Pi.

---

*This roadmap replaces K-NN retrieval with continuous online learning as the core intelligence mechanism.*
*The system doesn't look up the past — it learns from it, continuously, on every observation.*
*When local intelligence fails, it asks for help. When help arrives, it incorporates the lesson.*
*Deploy and evolve, not deploy and forget.*
