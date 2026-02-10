# RuVector Learning Acceleration Analysis: V1.1 to V2.0

**Research Date:** 2026-02-10
**Revised:** 2026-02-10 (actual Pi 5 specs: 16GB RAM, 1TB NVMe SSD)
**Author:** Research Agent
**Version:** 1.1
**Status:** Complete

---

## Executive Summary

This document analyzes whether ruvector's self-learning stack (SONA, ReasoningBank, GNN, Q-Learning, Attention Mechanisms) can accelerate NDP's journey from V1.1 (Gold Layer Foundation) through V1.2 (Pattern Detection), V1.3 (Prediction & Actions), to V2.0 (Multi-Stream Intelligence).

**Bottom line:** The hybrid approach -- traditional statistical methods as foundation, ruvector's learning layer on top -- is the recommended path. It preserves the deterministic, auditable foundation NDP needs while adding adaptive intelligence that improves over time. Estimated acceleration: **30-40% time reduction** for V1.2+V1.3, primarily from collapsing the correlation-to-prediction pipeline and eliminating manual model selection.

**~~Critical constraint:~~** ~~Raspberry Pi 5 with <2GB total memory budget.~~ **REVISED:** Actual Pi 5 has **16GB RAM with ~15.3GB available**. The memory constraint is no longer critical — the full ruvector stack (HNSW + GNN + SONA + ReasoningBank + Q-Learning) fits in ~3GB, leaving 12GB+ headroom. The hybrid approach is still recommended for complexity management, not resource constraints.

> **REVISION NOTE (16GB + NVMe):** Key impacts on this document's analysis:
> - **Section 1 (SONA):** Memory savings from LoRA are still valuable for architectural elegance, but the "models exhaust RAM quickly" concern is eliminated. Multiple full models would also fit.
> - **Section 3 (GNN):** The "defer until 30+ streams" recommendation is softened. At 16GB, GNN's ~320MB is trivial. Starting the causal graph early provides months of passive learning before V1.3.
> - **Section 6 (Hybrid Architecture):** Memory budget table should reference ~76MB new allocation out of 15,300MB available (0.5%), not a constrained 512MB budget.
> - **"What NOT to Do" #3:** "Do not run ruvector as a separate service on Pi" is **REVISED** — full container is viable at ~1GB. See `00-SYNTHESIS.md`.
> - The 30-40% acceleration estimate and phased integration recommendation remain unchanged.

---

## 1. SONA vs V1.3's Planned EWC++

### What V1.3 Plans

V1.3 (per `product/features/gold-001/FEATURE-ROADMAP.md`) uses EWC++ to prevent catastrophic forgetting when the model tournament selects winners across different stream relationships. The planned flow:

1. Train TCN, ARIMA, Prophet on each validated causal relationship
2. Tournament selection picks the best model per relationship
3. EWC++ constrains weight updates when learning new relationships so old relationships are not degraded

The existing `core/src/forecast/fann_adapter.rs` implements a `FannForecaster` with `ModelType::NHITS` and `ModelType::NBEATSx` variants, but these are currently mock implementations returning fixed values. The `Forecast` trait in `core/src/traits.rs` defines `train`, `predict`, and `evaluate` -- a clean interface that any model backend can implement.

### What SONA Adds

SONA = LoRA + EWC++. The LoRA component adds low-rank adaptation layers on top of a base model, enabling:

- **Parameter-efficient fine-tuning**: Instead of updating all model weights (expensive on Pi), LoRA updates a small rank-decomposed matrix. For a model with weight matrix W (d x k), LoRA decomposes the update as delta_W = B * A where B is (d x r) and A is (r x k), with rank r << min(d,k). This reduces trainable parameters from d*k to r*(d+k).
- **Task-specific adaptation**: Each stream relationship can have its own LoRA adapter (a few KB each) while sharing the base model weights.
- **Rapid switching**: Swap LoRA adapters to switch between prediction tasks without reloading the full model.

### Comparison

| Dimension | EWC++ Alone (V1.3 Plan) | SONA (LoRA + EWC++) |
|-----------|------------------------|---------------------|
| **Memory cost** | Single model per relationship, O(n) models for n relationships | One base model + n small LoRA adapters, O(1) base + O(n * r) adapters |
| **Training speed** | Full model retrain per new relationship | Fine-tune adapter only (10-100x fewer parameters) |
| **Forgetting protection** | EWC++ constrains weights globally | LoRA isolates per-relationship changes; EWC++ protects shared base |
| **Pi feasibility** | Multiple full models exhaust RAM quickly | One base model (~50MB) + adapters (~2KB each) fits in ~60MB total |
| **Model switching** | Load entirely new model weights | Swap adapter matrix (microseconds) |
| **Accuracy** | Full fine-tuning, theoretical optimum | 95-99% of full fine-tuning accuracy per LoRA literature |

### Synergy Assessment

SONA does NOT replace the model tournament. It sits within each tournament contestant:

```
Tournament Structure (V1.3 Plan):
  TCN-Lite   → Train → Evaluate → Score
  ARIMA      → Train → Evaluate → Score
  Prophet    → Train → Evaluate → Score
  Winner selected per relationship

With SONA Integration:
  TCN-Lite + LoRA adapter  → Fine-tune adapter → Evaluate → Score
  ARIMA (statistical, no LoRA needed)
  Prophet (statistical, no LoRA needed)
  Winner selected per relationship
  EWC++ protects base TCN weights when adding new adapters
```

SONA is most valuable for the neural models (TCN-Lite, MLP) where full retraining is expensive. Statistical models (ARIMA, Prophet) do not benefit from LoRA because they lack neural weight matrices.

### Cold Start Considerations

- EWC++ requires at least one fully-trained model before it can constrain future updates. Cold start cost: one full training cycle.
- LoRA requires a pre-trained base model. The base model must be trained on general time-series data (potentially from the first 30 days of data accumulation per the roadmap's statistical significance threshold).
- Combined cold start: Train base TCN on initial data (~1-2 hours on Pi), then all subsequent relationships use LoRA adaptation (~minutes each).

### Recommendation

**Adopt SONA for neural models in the tournament.** The memory savings alone justify it -- one 50MB base model + tiny adapters vs. multiple 50MB models. This directly addresses the roadmap's risk: "Resource exhaustion on Pi" (listed as Low likelihood, High impact). With SONA, the tournament can evaluate more model variants without exceeding the 512MB app memory budget.

**Implementation note:** The existing `FannForecaster` in `core/src/forecast/fann_adapter.rs` already has the right interface (`Forecast` trait). SONA would be a new `SonaForecaster` implementing the same trait, with an internal LoRA adapter registry keyed by (source_stream, target_stream, lag) tuple.

---

## 2. ReasoningBank vs Granger Causality

### What V1.2 Plans

V1.2's Pattern Detection Engine (per the roadmap) uses Granger causality scanning:

1. Take aligned hourly data from `gold.aligned_hourly` (built by V1.1)
2. For every pair of streams (state_event x observation), run Granger causality test
3. Rank correlations by strength and relevance to objectives
4. Promote candidates exceeding threshold to `gold.candidate_registry`

The Granger test asks: "Does including past values of X improve the prediction of Y beyond using past Y alone?" This is purely statistical -- it requires no prior knowledge, no training, just data.

### What ReasoningBank Offers

ReasoningBank captures decision trajectories:

```
Context:   CO2 at 780 ppm, rising trend, window closed, outdoor temp 15C
Decision:  Suggest "open window"
Outcome:   CO2 dropped to 620 ppm in 25 minutes
Judgment:  Success (reward 0.9) -- achieved objective, faster than expected
```

Over time, this builds a corpus of experiential knowledge that can be queried: "When we saw this pattern before, what worked?"

### Head-to-Head Comparison

| Dimension | Granger Causality (V1.2 Plan) | ReasoningBank |
|-----------|------------------------------|---------------|
| **What it discovers** | Statistical lead-lag relationships between stream pairs | What decisions led to good/bad outcomes |
| **Data requirement** | 30+ days of aligned hourly data (per roadmap) | Needs operational decisions + outcomes (V1.3+ data) |
| **Cold start** | Works from day 1 with enough historical data | Zero useful knowledge until system starts making/suggesting actions |
| **Computational cost** | O(n^2 * T) for n streams and T time points per Granger test | O(1) per decision recording; O(log n) per lookup |
| **Pi resource impact** | Nightly batch job, bounded by SQL query on TimescaleDB | Lightweight key-value + vector store, continuous |
| **False positives** | Granger detects correlation, not causation (known limitation) | Encodes actual cause-effect from real interventions |
| **Explainability** | "X Granger-causes Y at lag L with p-value P" | "Last time we saw this, action A led to outcome B" |
| **Generalization** | Discovers ALL pairwise relationships (some useless) | Only captures relationships where actions were taken |
| **Surprising discoveries** | Yes -- finds unexpected correlations | No -- only knows about patterns it has experienced |

### Why ReasoningBank Cannot Replace Granger

The fundamental issue is temporal ordering in the development lifecycle:

```
Timeline:
V1.1 → V1.2 (Granger) → V1.3 (Actions) → ReasoningBank becomes useful
                                              ↑
                                   ReasoningBank needs actions to exist
                                   before it can record trajectories
```

ReasoningBank requires the system to be making decisions (V1.3's action framework) before it can record decision trajectories. But V1.2 must discover WHICH relationships to act on BEFORE V1.3 can create actions. This is a chicken-and-egg problem.

Granger causality resolves the cold start: it needs only data, not decisions. It runs on the `gold.aligned_hourly` view that V1.1 builds, discovering candidate relationships that V1.3 then validates causally and acts upon.

### Augmentation Strategy (Recommended)

Use Granger as the discovery engine (V1.2). Use ReasoningBank as the learning memory (V1.3+):

```
V1.2: Granger discovers candidates
  "window_open Granger-causes co2_drop at 17-minute lag (p < 0.01)"

V1.3: System acts on candidates, records to ReasoningBank
  Context: CO2=820, window=closed, outdoor_pm25=8
  Decision: open_window
  Outcome: CO2 dropped to 640 in 22 minutes
  Reward: 0.95

V1.3+: ReasoningBank augments Granger
  - Granger says "window→CO2 correlation weakening" (seasonal)
  - ReasoningBank knows "window opening in winter has lower effect size"
  - System adjusts prediction confidence by season automatically

V2.0: New domain added via config
  - Granger scans new stream pairs (cold start, no ReasoningBank data)
  - Once actions begin, ReasoningBank captures domain-specific reasoning
  - Transfer learning: similar patterns from air-quality domain inform new domain
```

### Cold Start Mitigation

For V2.0's new domains, the cold start for ReasoningBank can be partially mitigated:

1. **Transfer from existing domains**: If weather→air_quality patterns exist in ReasoningBank, and a new energy domain also uses weather data, the system can transfer relevant trajectories.
2. **Seeding from Granger**: When Granger discovers a candidate in a new domain, seed ReasoningBank with a synthetic trajectory recording the statistical finding. Reward = correlation strength. This gives the ReasoningBank something to work with before real decisions are made.
3. **Expert seeding**: Domain experts can manually input known cause-effect relationships as initial trajectories.

### Recommendation

**Keep Granger for V1.2 discovery. Add ReasoningBank starting in V1.3 as the experiential learning layer.** The two are complementary, not competing. Granger handles "what MIGHT cause what" (statistical). ReasoningBank handles "what DID cause what in practice" (experiential). Together, they form a feedback loop where Granger discovers, actions validate, and ReasoningBank remembers.

---

## 3. GNN for Correlation Discovery

### Current Plan: Brute-Force Pair Scanning

V1.2 plans to scan ALL stream pairs via Granger causality. For n streams:
- Number of pairs: n * (n-1) / 2
- Current streams: 5 (air-quality, outdoor-weather, outdoor-air-quality, home-assistant-state, nws-forecast)
- Pairs to scan: 10
- With V2.0 financial streams: potentially 15+ streams = 105+ pairs

At hourly granularity over 30 days (720 data points per stream), each Granger test is computationally cheap. Even 105 pairs at O(T) per test is trivial for TimescaleDB.

### GNN Alternative: Learned Graph Structure

Model streams as graph nodes. Edges represent correlations. GNN learns which connections matter and reinforces paths that lead to successful predictions.

```
Initial Graph (from Granger):
  air-quality ──0.73──→ outdoor-weather
  air-quality ──0.45──→ state-events
  outdoor-weather ──0.12──→ state-events  (weak, likely noise)

After GNN Learning:
  air-quality ──0.89──→ outdoor-weather  (reinforced: confirmed causal)
  air-quality ──0.67──→ state-events     (reinforced: confirmed causal)
  outdoor-weather ──0.02──→ state-events (suppressed: spurious)
```

### Cost-Benefit Analysis

| Dimension | Brute-Force Granger | GNN Graph Learning |
|-----------|--------------------|--------------------|
| **Computational cost at 5 streams** | 10 pairs, ~seconds on Pi | Overkill -- GNN setup cost exceeds benefit |
| **Computational cost at 50 streams** | 1,225 pairs, ~minutes on Pi | Potentially faster via learned pruning |
| **Computational cost at 200 streams** | 19,900 pairs, ~hours on Pi | Significant speedup from graph traversal |
| **Memory on Pi** | Only SQL queries, minimal | GNN model + graph structure: 50-200MB |
| **Discovery quality** | Tests everything equally | Focuses on likely-productive paths |
| **Initialization** | Needs only data | Needs initial graph (from where?) |
| **Adaptability** | Re-run from scratch each time | Incrementally learns, retains knowledge |

### The Scale Question

The critical question is: at what stream count does GNN become worthwhile?

NDP's V2.0 vision adds financial streams (FRED, Alpaca, Finnhub). Realistically:
- V1.x: 5-10 streams (Granger handles this trivially)
- V2.0: 10-20 streams (Granger still manageable: 190 pairs max)
- V2.0+ (theoretical): 50+ streams (GNN starts to shine)

On a Raspberry Pi 5, the limiting factor is not Granger's compute cost but rather the statistical significance of pair tests. With hourly data, you need 720+ observations (30 days). Adding streams does not change the per-test cost, only the number of tests.

### GNN Initialization Strategy

If implemented, the GNN should be initialized from Granger results, not randomly:

1. **Phase 1 (V1.2)**: Run Granger on all pairs. Store results as initial graph edges.
2. **Phase 2 (V1.3)**: When predictions succeed/fail, update edge weights in the GNN.
3. **Phase 3 (V2.0)**: When new streams arrive, GNN predicts which existing nodes they are likely to connect with, reducing Granger's search space.

This eliminates random initialization (which would produce garbage paths on Pi-scale data).

### RuVector GNN Module Applicability

Per the prior ruvector analysis (`product/research/13-ruvector-centralized-service-analysis.md`), ruvector's GNN module supports:
- Relationship modeling with explicit edges
- Multi-hop queries
- Community detection
- Path finding
- Differentiable search (learn optimal parameters via gradient descent)

For NDP, the "causal relationship modeling" use case from that analysis directly applies:

```
ruvector.gnn.add_edge(
    "event-cooking-started",
    "event-pm25-spike",
    "causes",
    0.89  // Causal confidence from Granger + PC validation
)
```

However, ruvector's GNN module requires ~320MB for 500K edges (per the performance benchmarks). NDP's stream graph will have at most hundreds of edges (not 500K). At this scale, a simple adjacency matrix in TimescaleDB is sufficient.

### Recommendation

**Defer GNN until stream count exceeds 30.** For V1.2 through V2.0, brute-force Granger is:
- Cheaper to implement (SQL queries on existing TimescaleDB)
- Cheaper to run (no separate GNN service)
- Easier to debug (deterministic, reproducible)
- More explainable (p-values, correlation coefficients)

If NDP reaches 50+ streams in V2.0+, revisit GNN as a learned pruning layer on top of Granger. Store the correlation graph in ruvector's GNN module at that point, using Granger results as initialization weights.

---

## 4. Q-Learning for Graduated Autonomy

### V1.3's Planned Autonomy Progression

V1.3 plans three autonomy levels per action type:

```
Level 1: ALERT    -- Notify user of prediction ("CO2 will exceed 800 in 45 min")
Level 2: SUGGEST  -- Recommend action ("Open window now?")
Level 3: AUTO     -- Execute action automatically within safety limits
```

The roadmap's v13-009 (Autonomy Controller) manages per-action automation level. The current plan is manual promotion: user explicitly upgrades an action from ALERT to SUGGEST after seeing enough correct predictions.

### Q-Learning Alternative

Q-Learning can learn the optimal autonomy level per action type:

```
State:  (sensor_context, time_of_day, recent_events, prediction_confidence)
Action: (alert, suggest, auto_execute, do_nothing)
Reward: objective_achievement - user_override_penalty
```

The Q-value Q(s, a) represents the expected cumulative reward of taking autonomy action a in state s. Over time, the system learns when to suggest vs. auto-execute.

### Convergence Analysis on Pi

Q-Learning convergence depends on state-action space size and exploration rate.

**State space estimation:**
- Sensor bins: 5 metrics x 10 bins = 50 dimensions (but most combinations are rare)
- Time bins: 24 hours x 7 days = 168
- Confidence bins: 5 levels (very_low, low, medium, high, very_high)
- Effective state space: ~500-2000 distinct states (after bucketing)

**Action space:** 4 options (alert, suggest, auto, nothing)

**Interactions needed for convergence:**
- With tabular Q-Learning: ~10x state-action pairs = 10 * 2000 * 4 = 80,000 interactions
- At 1 decision per hour (sensor reads every 5-10 minutes, but meaningful decision points are hourly): ~80,000 hours = ~9 years
- At 1 decision per 15 minutes: ~80,000 * 0.25 hours = ~2.3 years

This is clearly too slow for tabular Q-Learning.

**With function approximation (neural Q-network):**
- Small MLP (2 hidden layers, 64 units): ~8K parameters
- Convergence typically requires 10-50K samples
- At 1 sample per 15 minutes: 10,000 * 0.25 hours = 2,500 hours = ~104 days

**With experience replay and prioritized sampling:**
- Can reduce to ~5,000 meaningful samples
- ~52 days to reasonable convergence

### Pi Resource Impact

A Q-Learning agent with function approximation requires:
- Model memory: ~64KB (8K float32 parameters)
- Experience replay buffer: ~5MB (10K transitions at 500 bytes each)
- Training compute: Negligible (single forward/backward pass per step)
- Total: ~6MB -- fits easily within Pi memory constraints

### Comparison with Manual Progression

| Dimension | Manual Promotion (V1.3 Plan) | Q-Learning |
|-----------|------------------------------|------------|
| **User burden** | Must manually review and promote each action | Automatic promotion based on track record |
| **Safety** | User has full control | Safety limits enforced but decisions are autonomous |
| **Time to full autonomy** | Depends on user engagement | ~100 days to reasonable policy |
| **Adaptability** | None -- fixed levels | Adapts to changing conditions (seasonal, etc.) |
| **Explainability** | Clear: "user set this to auto" | Moderate: Q-values show why, but harder to audit |
| **Risk of over-automation** | Low (user controls) | Moderate (must bound exploration) |

### Hybrid Approach

The safest design combines both:

1. **Manual ceiling**: User sets maximum autonomy level per action type (e.g., "window actions can go up to SUGGEST, but never AUTO")
2. **Q-Learning floor**: System learns when to use each level UP TO the manual ceiling
3. **Override tracking**: Every time user overrides a Q-Learning decision, negative reward is applied

This preserves user control while allowing the system to learn within bounds.

### Recommendation

**Implement Q-Learning as an advisory layer in V1.3, constrained by manual ceilings.** The memory cost is trivial (~6MB). The convergence time (~100 days) is acceptable because the manual fallback works immediately. Over the first 3-4 months, the Q-Learning agent becomes increasingly useful, reducing user override rate (the roadmap's target: <20% for mature system).

**Implementation note:** The Q-Learning agent should implement the existing `Forecast` trait pattern -- a `QLearningAutonomyController` that takes (state, prediction_confidence) and returns the recommended autonomy level. This integrates with v13-009 by providing a `recommended_level` that the Autonomy Controller can accept or cap.

---

## 5. Time-to-Intelligence Comparison

### Traditional Path (Current Plan)

```
V1.1: Gold Layer Foundation (current work)
  - Config-driven aggregates, features, events, objectives
  - Elapsed: In progress

V1.2: Pattern Detection Engine
  - Granger causality scanner
  - Transition detection, response measurement, lag detection
  - Candidate promotion pipeline
  - Estimated: 4-6 weeks (5 phases per roadmap)

V1.3: Prediction & Actions
  - Causal validation (PC algorithm)
  - Model zoo (TCN, ARIMA, Prophet)
  - Tournament selection
  - Action framework
  - Outcome tracking
  - EWC++ for continuous learning
  - Graduated autonomy
  - Estimated: 8-12 weeks (10 features)

V2.0: Multi-Stream Intelligence
  - Financial stream sources
  - Full correlation scanner
  - Stream-agnostic learning
  - Estimated: 6-8 weeks

Total V1.2 through V2.0: ~18-26 weeks
```

### Hybrid Path (Traditional + RuVector Learning Layer)

```
V1.1: Gold Layer Foundation (unchanged)
  - No ruvector integration needed yet

V1.2: Pattern Detection Engine (slightly modified)
  - Granger causality scanner (unchanged)
  - ADD: Store correlation graph in simple adjacency structure
  - ADD: Seed ReasoningBank schema (empty, ready for V1.3)
  - Time impact: +1 week for schema prep, but parallelizable
  - Net: Same 4-6 weeks

V1.3: Prediction & Actions (significantly accelerated)
  - Causal validation (unchanged)
  - Model zoo: Replace separate models with SONA base + LoRA adapters
    - Saves: 2-3 weeks model management complexity
  - Tournament: Simplified (swap LoRA adapters vs. load full models)
    - Saves: 1 week tournament infrastructure
  - Action framework (unchanged)
  - Outcome tracking + ReasoningBank (combined)
    - Each outcome feeds both the objective tracker AND ReasoningBank
    - Net cost: Same (ReasoningBank is a thin layer on outcome storage)
  - Q-Learning autonomy controller (replaces manual-only)
    - Additional cost: 1 week implementation
    - Saves: Long-term user burden reduction
  - EWC++ built into SONA (no separate implementation)
    - Saves: 1-2 weeks
  - Estimated: 5-8 weeks (down from 8-12)

V2.0: Multi-Stream Intelligence (accelerated)
  - Financial stream sources (unchanged)
  - Correlation scanner: Granger + ReasoningBank transfer learning
    - New domains get bootstrapped from similar existing domain reasoning
    - Saves: 1-2 weeks per new domain onboarding
  - Stream-agnostic learning: SONA base model is already stream-agnostic
    - LoRA adapter per stream, base model shared
    - Saves: 1-2 weeks model generalization work
  - Estimated: 4-6 weeks (down from 6-8)

Total V1.2 through V2.0: ~13-20 weeks
```

### Acceleration Breakdown

| Phase | Traditional | Hybrid | Savings | Source of Savings |
|-------|-----------|--------|---------|-------------------|
| V1.2 | 4-6 weeks | 4-6 weeks | 0 | None -- Granger is the right tool |
| V1.3 | 8-12 weeks | 5-8 weeks | 3-4 weeks | SONA reduces model management; EWC++ comes free; Q-Learning replaces manual autonomy |
| V2.0 | 6-8 weeks | 4-6 weeks | 2 weeks | ReasoningBank transfer learning; SONA adapter reuse |
| **Total** | **18-26 weeks** | **13-20 weeks** | **5-6 weeks** | **~30% acceleration** |

### Risk: Complexity Tax

The hybrid path introduces new concepts (LoRA, ReasoningBank, Q-Learning) that the team must understand and maintain. This creates:

- **Learning curve**: 1-2 weeks of upfront research and prototyping
- **Debugging complexity**: When predictions fail, is it the base model, the adapter, or the Q-Learning policy?
- **Dependency risk**: If ruvector's SONA implementation has bugs, it affects the entire prediction pipeline

Mitigation: The traditional statistical methods (Granger, ARIMA) remain as the foundation. SONA/ReasoningBank/Q-Learning are additive layers. If any fail, the system degrades to the traditional path rather than failing completely.

### Minimum Viable RuVector Integration

If the team wants to test ruvector's value with minimum investment:

1. **Week 1**: Implement `SonaForecaster` with a single LoRA adapter for CO2 prediction
2. **Week 2**: Compare SONA accuracy vs. standalone ARIMA on 30 days of data
3. **Decision point**: If SONA matches or exceeds ARIMA with lower memory usage, proceed with full integration. If not, discard and use traditional path.

This costs 2 weeks and produces a concrete comparison, not theoretical analysis.

---

## 6. Hybrid Architecture Design

### Layered Intelligence Architecture

```
LAYER 4: ADAPTIVE INTELLIGENCE (V1.3+)
  ┌──────────────────────────────────────────────────────┐
  │  Q-Learning Autonomy Controller                       │
  │  - Learns when to alert / suggest / auto-execute     │
  │  - Constrained by user-set maximum autonomy levels   │
  │  - ~6MB memory, 64KB model                           │
  └──────────────────────────────────────────────────────┘
                          │ recommends autonomy level
                          ▼
LAYER 3: EXPERIENTIAL LEARNING (V1.3+)
  ┌──────────────────────────────────────────────────────┐
  │  ReasoningBank                                        │
  │  - Records: context → decision → outcome → reward    │
  │  - Queryable: "what worked when we saw pattern X?"   │
  │  - Feeds back to Layer 2 prediction confidence       │
  │  - ~10MB storage (grows ~1KB per decision)           │
  └──────────────────────────────────────────────────────┘
                          │ experiential confidence adjustment
                          ▼
LAYER 2: PREDICTION (V1.3)
  ┌──────────────────────────────────────────────────────┐
  │  SONA Model Tournament                                │
  │  - Base model: TCN-Lite (~50MB shared)               │
  │  - Per-relationship LoRA adapters (~2KB each)        │
  │  - EWC++ protects base weights during adaptation     │
  │  - ARIMA / Prophet as statistical fallbacks          │
  │  - Tournament selects best per relationship          │
  └──────────────────────────────────────────────────────┘
                          │ predictions + confidence
                          ▼
LAYER 1: DISCOVERY (V1.2)
  ┌──────────────────────────────────────────────────────┐
  │  Granger Causality + Correlation Scanner              │
  │  - Runs on gold.aligned_hourly (V1.1 output)        │
  │  - All stream pairs scanned nightly                  │
  │  - Candidates promoted to registry                   │
  │  - Pure SQL on TimescaleDB, no ML dependencies       │
  └──────────────────────────────────────────────────────┘
                          │ candidate relationships
                          ▼
LAYER 0: DATA FOUNDATION (V1.1, current)
  ┌──────────────────────────────────────────────────────┐
  │  Gold Layer: Aggregates, Features, Events, Objectives│
  │  - Config-driven continuous aggregates               │
  │  - Cross-stream aligned hourly view                  │
  │  - Unified events (state transitions + thresholds)   │
  │  - Objectives with declared targets                  │
  └──────────────────────────────────────────────────────┘
```

### Memory Budget on Pi

| Component | Memory | Layer |
|-----------|--------|-------|
| TimescaleDB (existing) | 256MB | 0-1 |
| SONA base model | 50MB | 2 |
| LoRA adapters (50 relationships) | 100KB | 2 |
| ARIMA/Prophet statistical models | 10MB | 2 |
| ReasoningBank storage | 10MB | 3 |
| Q-Learning agent | 6MB | 4 |
| **Total new allocation** | **~76MB** | |

This fits within the existing air-quality-app's 512MB limit with 436MB remaining for the application itself (which currently uses ~200MB).

### Integration Points with Existing Code

The hybrid architecture integrates with NDP's existing trait system:

```
core/src/traits.rs:
  Forecast trait      → SonaForecaster implements this
                     → ARIMAForecaster implements this
                     → ProphetForecaster implements this

core/src/forecast/:
  FannForecaster     → Replaced by SonaForecaster (same interface)
  features.rs        → Reused by SONA for feature engineering
  scaler.rs          → Reused by SONA for normalization

New modules:
  core/src/learning/reasoning_bank.rs   → ReasoningBank implementation
  core/src/learning/q_autonomy.rs       → Q-Learning autonomy controller
  core/src/forecast/sona_adapter.rs     → SONA (LoRA + EWC++) forecaster
```

### Data Flow Through Layers

```
1. Bronze ingests raw sensor data (existing)
2. Silver ETL produces typed observations (existing)
3. Gold layer computes hourly aggregates + features (V1.1)
4. Granger scanner runs nightly on gold.aligned_hourly (V1.2)
   → Produces candidate relationships in gold.candidate_registry
5. SONA tournament trains/evaluates on each candidate (V1.3)
   → Loads base model once, creates LoRA adapter per candidate
   → Selects winner per relationship
6. Prediction service forecasts target metrics (V1.3)
   → Uses winning model + adapter for each relationship
7. Action framework scores possible actions (V1.3)
   → Ranks by predicted objective impact
8. ReasoningBank records (context, action, outcome) (V1.3)
   → Feeds back to prediction confidence (step 6)
9. Q-Learning controller recommends autonomy level (V1.3)
   → Constrained by user-set ceiling
   → Learns from override/acceptance patterns
10. When new domain added (V2.0):
    → Granger scans new pairs (Layer 1)
    → ReasoningBank transfers relevant trajectories (Layer 3)
    → SONA reuses base model, creates new adapters (Layer 2)
```

---

## 7. Attention Mechanisms Assessment

### RuVector's 40 Attention Mechanisms

RuVector includes neighborhood-aware attention for graph data and hyperbolic attention for hierarchical structures. For NDP's time-series domain:

**Relevant mechanisms:**
- **Temporal attention**: Weight recent observations more than distant ones in prediction
- **Cross-stream attention**: Learn which streams to attend to for a given prediction target
- **Hierarchical attention**: Model hourly → daily → weekly patterns

**Irrelevant mechanisms (for NDP):**
- Graph topology attention (NDP's stream graph is too small)
- Hyperbolic attention (time-series data is not inherently hierarchical in a hyperbolic sense)

### Assessment

At NDP's current scale (5-20 streams, hourly data), the attention mechanisms add complexity without proportionate benefit. Simple lag features and rolling statistics (already in `core/src/forecast/features.rs`) capture temporal patterns adequately.

Attention becomes valuable when:
- Stream count exceeds 20 (cross-stream attention helps focus)
- Prediction horizons extend beyond 24 hours (temporal attention helps long-range dependencies)
- Hierarchical patterns dominate (seasonal attention for yearly patterns)

### Recommendation

**Defer attention mechanisms to V2.0+.** The existing feature engineering in `features.rs` (lag_feature, rolling_mean, rolling_std) combined with SONA's LoRA adapters provides sufficient adaptive capacity for V1.2-V1.3.

---

## Acceleration Verdict

### Quantified Estimates

| Metric | Traditional Path | Hybrid Path | Delta |
|--------|-----------------|-------------|-------|
| V1.2 through V2.0 elapsed time | 18-26 weeks | 13-20 weeks | **30-40% faster** |
| Models in memory simultaneously | n full models (50MB each) | 1 base + n adapters (50MB + n*2KB) | **90%+ memory reduction** for neural models |
| Time to first prediction (V1.3) | After full model training (~weeks) | After LoRA fine-tuning (~hours) | **Days faster** per new relationship |
| Time to autonomous operation | Manual user promotion (unbounded) | Q-Learning convergence (~100 days) | **Deterministic timeline** |
| New domain onboarding (V2.0) | Full pipeline from scratch | Transfer + adapt | **1-2 weeks faster** per domain |

### Risk-Adjusted Recommendation

**Phase the integration to manage risk:**

1. **V1.2 (next)**: No ruvector integration. Ship Granger scanner as planned. Zero risk.
2. **V1.3 Phase 1**: Add SONA for the TCN model only. Keep ARIMA/Prophet as-is. Low risk (SONA is additive, not replacing).
3. **V1.3 Phase 2**: Add ReasoningBank as a logging layer on top of outcome tracking. Zero risk (read-only learning, does not affect decisions).
4. **V1.3 Phase 3**: Add Q-Learning autonomy controller with manual ceiling. Low risk (user always has override).
5. **V2.0**: Enable ReasoningBank transfer learning for new domains. Moderate risk (transfer may be noisy), mitigated by Granger fallback.

Each phase can be shipped and validated independently. If any phase underperforms, it can be disabled without affecting lower layers.

### What NOT to Do

1. **Do not attempt to replace Granger with GNN.** The stream count does not justify it.
2. **Do not implement all 40 attention mechanisms.** They solve problems NDP does not have yet.
3. **Do not run ruvector as a separate service on Pi.** The 5.75GB memory profile is incompatible. Instead, use ruvector's algorithms as library code within the existing Rust application.
4. **Do not skip the traditional statistical foundation.** Granger, ARIMA, and correlation scanning are battle-tested, interpretable, and work from day 1. The learning layer accelerates but does not replace them.

### Final Assessment

The hybrid approach offers a concrete 30-40% acceleration in the V1.2-V2.0 timeline, primarily from SONA's memory efficiency (enabling more model variants in tournament), ReasoningBank's experiential learning (reducing manual tuning), and Q-Learning's automated autonomy progression (reducing user burden). The traditional statistical foundation remains essential and should not be shortcut.

The minimum viable test (2 weeks, SONA vs. ARIMA on CO2 prediction) provides a concrete decision point before committing to the full hybrid architecture.

---

## Key Files Referenced

- `/workspaces/neural-data-platform/product/features/gold-001/FEATURE-ROADMAP.md` -- V1.1-V2.0 roadmap with feature definitions
- `/workspaces/neural-data-platform/core/src/forecast/fann_adapter.rs` -- Existing (mock) FannForecaster implementation
- `/workspaces/neural-data-platform/core/src/forecast/features.rs` -- Feature engineering (lag, rolling, temporal)
- `/workspaces/neural-data-platform/core/src/forecast/scaler.rs` -- StandardScaler for normalization
- `/workspaces/neural-data-platform/core/src/traits.rs` -- Forecast, Source, Store, RawSource traits
- `/workspaces/neural-data-platform/crates/ndp-lib/src/gold/generators/events.rs` -- Events hypertable DDL generator
- `/workspaces/neural-data-platform/docs/architecture/PLATFORM_ARCHITECTURE_OVERVIEW.md` -- System architecture
- `/workspaces/neural-data-platform/docs/architecture/CONSOLIDATED_ARCHITECTURE_DECISIONS.md` -- ADR summary
- `/workspaces/neural-data-platform/product/research/13-ruvector-centralized-service-analysis.md` -- Prior ruvector analysis
