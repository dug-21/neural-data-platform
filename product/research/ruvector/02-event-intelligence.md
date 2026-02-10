# RuVector for Event Intelligence in NDP

**Research Date:** 2026-02-10
**Revised:** 2026-02-10 (actual Pi 5 specs: 16GB RAM, 1TB NVMe SSD)
**Researcher:** Research Agent
**Status:** Complete
**Context:** NDP v1.1.21, Gold Layer Foundation (FE-001) complete, V1.2 Pattern Detection Engine planned

> **REVISION NOTE (16GB + NVMe):** This document's recommendations remain valid — event intelligence use cases are not memory-constrained. The revised hardware profile means:
> - All embedding strategies (Approach A, B, C) are viable simultaneously
> - f32 vectors are affordable (no compression needed) — see Section 1.4 and `00-SYNTHESIS.md`
> - GNN causal graph (Section 2.3) can start in Phase 1 rather than being deferred
> - ReasoningBank storage (~20MB) and Q-Learning (~6MB) are trivially accommodated
> - Resource table in Section 6.2 already reflects 16GB (790MB total used of 16GB)

---

## Executive Summary

This document analyzes how ruvector's capabilities -- HNSW vector indexing, GNN self-improving index, ReasoningBank, Q-Learning, hyperbolic embeddings, SONA (LoRA + EWC++), and Cypher graph queries -- could enhance NDP's event intelligence pipeline. The analysis covers five areas: event embedding, pattern detection acceleration, trigger intelligence, learning loops, and a concrete end-to-end scenario.

**Top 3 Most Promising Use Cases:**

1. **Event Sequence Embedding for Similarity Retrieval** (High Value) -- Embed event-context windows as vectors, enabling "find similar past situations" queries that directly accelerate V1.2 pattern detection and V1.3 prediction.

2. **Causal Graph as GNN with Reinforcement** (High Value) -- Store discovered causal relationships as graph edges that strengthen with evidence over time, replacing the static Candidate Registry (v12-008) with a living knowledge graph.

3. **Anomaly Detection via Embedding Distance** (Medium-High Value) -- Detect "unusual hours" by comparing the current sensor fingerprint embedding against learned cluster centroids, providing a lightweight alternative to statistical anomaly detection that adapts via SONA.

---

## 1. Event Embedding: How to Represent Time-Series Events in Vector Space

### 1.1 The Problem

NDP's Gold layer produces structured events (state transitions, threshold crossings) and aligned hourly sensor readings. V1.2 needs to find patterns across these -- "does this combination of sensor readings resemble a situation that previously led to a CO2 spike?" Currently, V1.2 plans brute-force pairwise Granger causality across all streams. There is no mechanism for "this hour looks like that hour."

### 1.2 Embedding Strategy: Context Windows

The most natural embedding unit for NDP is a **context window** -- the sensor fingerprint at a point in time, optionally centered around an event.

```
EVENT CONTEXT WINDOW (t = event time, w = +/- 30 min)
======================================================

Time:    t-30min .......... t (event) .......... t+30min
         |                  |                    |
Sensors: [pm25, co2, temp, humidity, outdoor_temp, wind, ...]
Events:  [window_state, threshold_crossings_count, ...]
Context: [hour_of_day, day_of_week, is_weekend]

                    |
                    v

Embedding: [0.23, -0.45, 0.82, 0.11, -0.33, 0.67, ...]
           dim = 64-128
```

**Three embedding scopes, from simplest to most powerful:**

| Scope | What It Captures | Dimensionality | Compute Cost | When to Use |
|-------|-----------------|----------------|-------------|-------------|
| **Point Snapshot** | Sensor readings at one hour | 32-64 | Trivial | Anomaly detection |
| **Event Context Window** | Readings +/- 30min around event | 64-128 | Low | Similarity retrieval |
| **Sequence Trajectory** | 4-hour sliding window of readings | 128-256 | Medium | Pattern detection |

### 1.3 Embedding Generation Without External LLMs

NDP runs on a Raspberry Pi. External LLM calls for embedding are unacceptable for latency and privacy. Three viable approaches:

**Approach A: Normalized Feature Vector (Immediate, No Training)**

```
Input:  [pm25=45, co2=820, temp=22, humidity=55, outdoor_temp=15, wind=8]
Step 1: Z-normalize per metric using rolling 7-day mean/std
Step 2: Append temporal features: [sin(hour/24), cos(hour/24), is_weekend]
Step 3: Append event context: [window_open=1, crossings_last_hour=2]
Output: 32-dim vector

Pros: Zero training, deterministic, interpretable
Cons: No learned similarity, linear relationships only
```

**Approach B: Lightweight Autoencoder (Short-Term, Trainable on Pi)**

```
Architecture:
  Encoder: Linear(input_dim, 128) -> ReLU -> Linear(128, 64) -> ReLU -> Linear(64, embed_dim)
  Decoder: Mirror of encoder

Training:
  - Reconstruction loss on hourly aligned view rows
  - Train weekly on last 30 days (batch, ~2 min on Pi 5)
  - ~50K parameters, <1MB model size

Pros: Learns non-linear feature interactions
Cons: Requires training infrastructure, may overfit with limited data
```

**Approach C: Temporal Convolutional Embedding (Future, Most Powerful)**

```
Architecture:
  4-hour window of readings -> 1D Conv(kernel=3) -> Pool -> Conv -> Pool -> Linear(embed_dim)

Training:
  - Contrastive loss: embeddings of "same outcome" windows should be similar
  - An "outcome" = what happened to target metric in next 30 min
  - Train on accumulated history (needs 60+ days of data)

Pros: Captures temporal dynamics, best similarity
Cons: More complex training, more parameters (~200K)
```

**Recommendation:** Start with Approach A for immediate value, evolve to Approach B after 60 days of Gold layer data accumulation. Approach C is a V1.3+ enhancement.

### 1.4 Dimensionality Selection

For NDP's data volume (~160K vectors/year for hourly readings):

| Dimension | HNSW Search Time | Memory (160K vectors, PQ8) | Suitability |
|-----------|-----------------|---------------------------|-------------|
| 32 | ~15 us | ~2 MB | Good for point snapshots |
| 64 | ~25 us | ~4 MB | Good for event contexts |
| 128 | ~40 us | ~8 MB | Good for sequence trajectories |
| 256 | ~55 us | ~15 MB | Overkill for NDP's variable count |

**Recommendation:** 64 dimensions for event context windows. This captures the full sensor state (~15 metrics + 5 temporal features + ~10 event features) with room for learned compression. Memory cost is negligible on Pi 5.

### 1.5 Value Rating

**Event Embedding: HIGH VALUE**

This is the foundation for use cases 2-5. Without embeddings, the system can only do explicit statistical tests (Granger causality). With embeddings, it gains an "intuition" layer -- "this situation feels like the one on January 15th when CO2 spiked."

---

## 2. Pattern Detection Acceleration

### 2.1 Current V1.2 Plan: Brute-Force Granger

V1.2's planned approach (from LIGHTWEIGHT-ALGORITHMS.md and FEATURE-ROADMAP.md):

```
For each pair of streams (X, Y) where X has d variables, Y has d variables:
    For each lag in [1, 2, ..., 24] hours:
        Run Granger causality F-test
        If p < 0.05 AND |correlation| > 0.3:
            Store as candidate

Complexity: O(d^2 * max_lag * n)
With 15 variables, 24 lags, 720 samples (30 days):
    15 * 15 * 24 * 720 = ~3.9M operations
    Estimated time on Pi 5: 5-15 seconds
```

This is feasible but has limitations:
- Tests ALL pairs equally -- no prioritization
- Discovers only linear, pairwise relationships
- Cannot find "this context is similar to a known-causal context"
- Re-runs from scratch; no memory of what was tested

### 2.2 How RuVector Could Pre-Filter Stream Pairs

**Concept:** Before running Granger causality, use embedding similarity to identify which stream pairs are worth testing.

```
PRE-FILTERING PIPELINE
======================

Step 1: Embed each stream's hourly trajectory (128-dim, last 4 hours)
        stream_embed["indoor_pm25"]  = embed(last_4h_pm25_readings)
        stream_embed["window_state"] = embed(last_4h_window_transitions)
        ...

Step 2: For each event embedding, find K nearest neighbor embeddings
        from DIFFERENT streams:
        Similar contexts = ruvector.search(event_embed, k=10, filter=different_stream)

Step 3: Stream pairs that frequently appear as nearest neighbors
        are "candidate related" -- test these with Granger FIRST

Step 4: Only if budget remains, test remaining pairs

        +------------------+       +------------------+
        |  Stream Embeds   |       | Candidate Pairs  |
        |  (ruvector HNSW) | ----> | (top 20 by       |
        |  ~15 vectors     |       |  co-occurrence)  |
        +------------------+       +--------+---------+
                                            |
                                            v
                                   +------------------+
                                   | Granger Causality|
                                   | (focused on      |
                                   |  candidates)     |
                                   +------------------+
```

**Savings estimate:** With 15 variables, brute-force tests 210 pairs. If embedding similarity narrows to 30 high-priority pairs, that is a 7x reduction in Granger tests. The HNSW search adds ~600 microseconds total (15 searches at ~40us each). Net savings: significant if Granger is the bottleneck on larger variable sets.

**Caveat:** For NDP's current 15 variables, brute-force is already fast (5-15 seconds). The pre-filtering value increases dramatically with more streams (V2.0 multi-domain, 50+ variables).

**Value Rating: MEDIUM VALUE today, HIGH VALUE for V2.0**

### 2.3 GNN for Strengthening Discovered Correlations Over Time

This is where ruvector's self-improving GNN index becomes powerful.

**Current plan (v12-008 Candidate Registry):** Store discovered correlations as static records:

```
PairwiseCorrelation {
    variable_x: "window_state",
    variable_y: "indoor_co2",
    lag_minutes: 17,
    correlation: -0.73,
    p_value: 0.001,
    confidence: 0.5,          // Static until manually updated
}
```

**With ruvector GNN:** Store relationships as graph edges that evolve:

```
GRAPH STRUCTURE (Cypher-like)
=============================

(window_state:Variable)
    -[:CAUSES {
        strength: 0.73,
        lag_minutes: 17,
        confidence: 0.85,       // Updated by GNN from evidence
        evidence_count: 47,
        first_seen: "2026-01-15",
        last_confirmed: "2026-02-10",
        seasonal_weight: {
            winter: 0.68,
            summer: 0.81         // Stronger in summer (windows open more)
        }
    }]->
(indoor_co2:Variable)

GNN LEARNING CYCLE (weekly):
1. Collect new event-outcome pairs from Gold layer
2. For each confirmed cause-effect observation:
   - Reinforce edge weight (+0.05 to confidence)
3. For each expected-but-not-observed effect:
   - Decay edge weight (-0.02 to confidence)
4. GNN message passing:
   - Propagate confidence through multi-hop paths
   - Discover transitive relationships:
     window_open -> ventilation_increase -> co2_drop
5. Prune edges below confidence threshold (0.2)
```

**Key advantage over static registry:** The GNN discovers transitive and multi-hop relationships that pairwise Granger cannot. If A causes B and B causes C, the GNN infers A indirectly causes C and can validate this against data.

**Value Rating: HIGH VALUE**

### 2.4 Hyperbolic Embeddings for Event Hierarchies

NDP's events have natural hierarchies:

```
HYPERBOLIC EVENT HIERARCHY
==========================

                        Event
                       /     \
              State           Threshold
              Transition      Crossing
             /    \           /       \
         Open    Close     Rising    Falling
         /  \    /  \      /    \    /     \
     Window Door Window Door  CO2  PM25  CO2  PM25
```

In hyperbolic space, this tree structure is preserved with logarithmic distortion -- root-level concepts (Event, State Transition) are far from leaf concepts (Window Open), and siblings (Window Open, Door Open) are close to each other.

**Application:** When searching for "events similar to this CO2 threshold crossing," hyperbolic embeddings naturally return other threshold crossings first, then related events, rather than mixing unrelated event types.

**However:** NDP's event hierarchy is shallow (3-4 levels) and small (2 event types, expanding to 4). The advantage of hyperbolic over Euclidean embeddings is most pronounced for deep hierarchies with 100+ nodes.

**Value Rating: LOW VALUE for current NDP, MEDIUM VALUE if event taxonomy grows significantly**

---

## 3. Trigger Intelligence: Beyond Static Thresholds

### 3.1 Current State: Static Objectives

NDP's objectives are declared in JSON:

```json
{
    "targets": [{
        "metric": "co2",
        "condition": "<",
        "threshold": 800,
        "priority": "high"
    }]
}
```

The threshold 800 ppm is context-blind. It applies identically whether it is 2 PM on a Tuesday (occupied office) or 3 AM on Sunday (empty house).

### 3.2 Context-Aware Thresholds via Embedding Clusters

**Concept:** Instead of a single threshold, define what "normal" looks like for different contexts, then alert when the current state deviates from the expected norm for THIS context.

```
CONTEXT CLUSTERING
==================

Step 1: Embed all hourly readings from last 90 days
        Each embedding captures: sensor values + temporal features

Step 2: Cluster embeddings into context groups (K-Means, k=8-12)
        Cluster 0: "Weekday daytime, occupied"         (mean CO2: 650)
        Cluster 1: "Weekday nighttime, unoccupied"      (mean CO2: 420)
        Cluster 2: "Weekend cooking"                     (mean CO2: 780)
        Cluster 3: "Summer, windows open"                (mean CO2: 510)
        ...

Step 3: For current hour, find nearest cluster centroid
        Current embedding -> Cluster 2 (weekend cooking)

Step 4: Alert if CO2 deviates significantly from cluster norm
        Cluster 2 mean CO2 = 780, std = 45
        Current CO2 = 920 -> z-score = (920-780)/45 = 3.1 -> ALERT
        vs. static threshold: 920 > 800 -> ALERT (same result here)

        But consider: Cluster 0 mean CO2 = 650, std = 35
        Current CO2 = 750 -> z-score = (750-650)/35 = 2.9 -> ALERT
        vs. static threshold: 750 < 800 -> NO ALERT (misses this anomaly)
```

**The insight:** Context-aware thresholds catch TWO classes of problems that static thresholds miss:
1. Values that are "normal" globally but abnormal for this context
2. Values that exceed the static threshold but are actually normal for this context (reducing false alarms)

**Implementation with ruvector:**

```
                     +---------------------+
                     |  Current hour embed |
                     |  (64-dim vector)    |
                     +----------+----------+
                                |
                     +----------v----------+
                     | ruvector HNSW search|
                     | Find nearest        |
                     | cluster centroid    |
                     +----------+----------+
                                |
                     +----------v----------+
                     | Compare current     |
                     | values to cluster   |
                     | statistics          |
                     +----------+----------+
                                |
                 +--------------+--------------+
                 |                             |
         +-------v-------+           +--------v--------+
         | Within norms  |           | Anomaly detected|
         | (no action)   |           | (context-aware  |
         +---------------+           |  alert)         |
                                     +-----------------+
```

**SONA (EWC++) for seasonal adaptation:** As seasons change, cluster centroids drift. SONA's continual learning prevents catastrophic forgetting -- it learns "summer patterns" without forgetting "winter patterns." The cluster model maintains separate seasonal knowledge.

**Value Rating: MEDIUM-HIGH VALUE**

### 3.3 Predictive Triggers via Embedding Similarity

**Concept:** "This pattern of sensor readings historically preceded a CO2 spike in 17 minutes."

This is the most natural application of embedding retrieval for NDP.

```
PREDICTIVE TRIGGER PIPELINE
============================

1. Every 5 minutes, embed current sensor state
   current_embed = embed(last_30min_readings)

2. Search ruvector for K nearest past embeddings
   similar_past = ruvector.search(current_embed, k=20)

3. For each similar past context, look up what happened NEXT
   outcomes = [gold.aligned_hourly(t+1, t+2, t+3) for t in similar_past.times]

4. Compute outcome statistics:
   "In 17 of 20 similar situations, CO2 exceeded 800 within 30 minutes"
   "In 14 of 20, PM2.5 stayed below 12"

5. If outcome probability exceeds threshold:
   ALERT: "CO2 likely to exceed 800 in ~30 minutes (85% confidence based on 17/20 similar episodes)"
```

This is a **non-parametric prediction** -- no model training needed. It works with as few as 30 days of data and improves automatically as more data accumulates.

**Comparison with V1.3's planned model tournament:**

| Aspect | Embedding Retrieval | Model Tournament (V1.3) |
|--------|-------------------|------------------------|
| Training required | None | Significant |
| Data needed | 30+ days | 60+ days per model |
| Interpretability | High ("similar to Jan 15 at 3pm") | Low (model weights) |
| Accuracy | Moderate (limited by K and data) | Higher for smooth relationships |
| Novel situations | Poor (no similar history) | Better (generalization) |
| Complementary? | **Yes** | **Yes** |

**These approaches complement each other.** Embedding retrieval provides immediate "intuition" with zero training. Model tournament provides refined accuracy for well-characterized relationships. Use embeddings as the fast path and models as the validated path.

**Value Rating: HIGH VALUE**

### 3.4 Anomaly Detection via Embedding Distance

**Concept:** Compute the distance from the current sensor embedding to its nearest neighbors. If the distance is unusually large, the current situation is anomalous -- it does not resemble any previously seen context.

```
ANOMALY SCORE COMPUTATION
==========================

1. current_embed = embed(current_sensors)

2. distances = ruvector.search(current_embed, k=10).distances

3. anomaly_score = mean(distances)

4. If anomaly_score > threshold_95th_percentile:
   ALERT: "Current sensor configuration is unusual"
   Context: "Nearest similar hour was 2026-01-23 at 14:00"
           "That hour had: CO2=720, PM25=8, temp=21"
           "Current hour: CO2=720, PM25=8, temp=21, BUT humidity=95"
           "Humidity is the unusual factor"

   +------------------------------------------------------------------+
   |  Normal hours (clustered)        * * *                            |
   |                                * * * * *                          |
   |                               * * * * * *                         |
   |                                * * * * *                          |
   |                                  * * *                            |
   |                                                                    |
   |                                                  X <-- anomaly    |
   |                                               (far from cluster)  |
   +------------------------------------------------------------------+
```

**SONA adaptation:** The anomaly threshold adapts over time. Summer readings that were anomalous in winter become normal as SONA's EWC++ integrates new seasonal data without forgetting old patterns. The 95th percentile distance threshold recomputes monthly.

**Value Rating: MEDIUM-HIGH VALUE**

---

## 4. Learning Loop: Q-Learning + ReasoningBank

### 4.1 Event-Action-Outcome Cycle

NDP's V1.3 plans a "graduated autonomy" system:

```
EVENT -> ACTION SUGGESTED -> OUTCOME MEASURED -> REWARD SIGNAL
```

ruvector's Q-Learning hooks and ReasoningBank can implement this directly.

### 4.2 Q-Learning for Action Selection

```
Q-LEARNING STATE-ACTION MODEL
==============================

State:   Current sensor embedding (64-dim) + active events
Action:  {alert_user, suggest_open_window, suggest_close_window,
          suggest_hvac_on, do_nothing}
Reward:  Did target metric improve in next 30 min?

Q-Table (simplified):
  State (cluster)  | Action              | Q-value | Updates
  -----------------+---------------------+---------+--------
  high_co2_day     | suggest_open_window | 0.82    | 47
  high_co2_day     | suggest_hvac_on     | 0.65    | 23
  high_co2_day     | do_nothing          | -0.31   | 89
  high_co2_night   | suggest_open_window | 0.23    | 12
  high_co2_night   | suggest_hvac_on     | 0.78    | 31
  ...

Q-Update Rule:
  Q(s,a) = Q(s,a) + alpha * (reward + gamma * max_Q(s') - Q(s,a))

Where:
  s  = current state embedding cluster
  a  = action taken
  s' = next state (30 min later)
  reward = improvement toward objective
  alpha  = 0.1 (learning rate)
  gamma  = 0.9 (discount factor)
```

**Key insight:** The state space is defined by embedding clusters, not raw sensor values. This makes Q-learning tractable -- instead of a continuous 15-dimensional state space, we have 8-12 discrete clusters from the embedding space.

### 4.3 ReasoningBank for Decision Trajectories

ruvector's ReasoningBank captures decision trajectories -- the full chain of reasoning that led to an outcome.

```
REASONING BANK ENTRY
=====================

Trajectory ID: traj-2026-02-10-1430
Trigger:       CO2 crossed 800 ppm (rising) at 14:30
Context:       {cluster: "weekday_afternoon", outdoor_temp: 18, wind: 5,
                outdoor_pm25: 8, window_state: "closed"}
Embedding:     [0.23, -0.45, 0.82, ...]

Decision Chain:
  1. Detected: threshold_crossing(co2, rising, 800)
  2. Retrieved: 15 similar past contexts from ruvector
  3. Observed: In 12/15, opening window resolved CO2 within 20 min
  4. Checked: outdoor PM25 = 8 (below constraint threshold of 35)
  5. Action: suggest_open_window (confidence: 0.80)

Outcome (measured at 15:00):
  - User opened window at 14:35
  - CO2 dropped from 820 to 680 by 15:00
  - Reward: +1.0 (objective met, correct suggestion)

Stored for future reference:
  - Similar trigger + similar context -> open_window works
  - Confidence updated: 0.80 -> 0.83
```

**How this compares to V1.3's model tournament:**

| Aspect | ReasoningBank | Model Tournament |
|--------|--------------|-----------------|
| What it stores | Full decision trajectory | Model weights + performance metrics |
| Explainability | "Last time in this situation, X worked" | "Model Y had lowest RMSE" |
| Learning speed | Immediate (one-shot from outcome) | Slow (needs retraining batch) |
| Generalization | Limited to similar contexts | Better for unseen contexts |
| Complementary? | **Yes -- provides the "why"** | **Yes -- provides the "how"** |

**They complement each other perfectly.** The model tournament selects the best prediction model. The ReasoningBank records WHY that prediction was made and whether the recommended action worked. Future agents can query the ReasoningBank: "What worked last time CO2 spiked on a weekday afternoon?"

### 4.4 Value Rating

**Q-Learning for action selection: MEDIUM VALUE** (useful but V1.3 is still far out; design for it now, implement later)

**ReasoningBank for decision trajectories: HIGH VALUE** (can be implemented alongside V1.2 pattern detection to record discovery trajectories)

---

## 5. Concrete End-to-End Scenario

### 5.1 The Window-CO2 Relationship: Full Lifecycle

This walks through how ruvector enhances every stage of discovering, learning, and acting on the window-open-causes-CO2-drop relationship.

```
DAY 1-30: DATA ACCUMULATION
============================

Gold Layer produces:
  - 720 hourly aligned readings (30 days * 24 hours)
  - ~50 state transition events (window open/close)
  - ~30 threshold crossing events (CO2 > 800)

Embedding Pipeline:
  - Each hourly reading embedded as 64-dim vector
  - Each event gets a context window embedding (sensors +/- 30min)
  - Stored in ruvector HNSW index: ~770 vectors, ~200 KB


DAY 30: FIRST PATTERN DETECTION SCAN (V1.2)
=============================================

Step 1: Brute-Force Granger (existing plan)
  - Tests 210 stream pairs at 24 lags each
  - Finds: window_state -> indoor_co2 (p=0.001, lag=17min, r=-0.73)
  - Also finds: outdoor_temp -> indoor_temp (p=0.003, lag=2h, r=0.81)
  - 8 candidates total

Step 2: Embedding-Enhanced Validation (ruvector addition)
  For each window_open event:
    a. Get context window embedding at event time
    b. Search ruvector for 10 nearest non-event hours
    c. Compare: CO2 trajectory after event vs after similar non-event hours

  Result:
    After window_open events:  CO2 drops avg 120 ppm in 17 min
    After similar non-events:  CO2 stays flat (+/- 15 ppm)
    Embedding similarity confirms: the event ITSELF matters,
    not just the general sensor context

Step 3: Store in Causal Graph (ruvector GNN)
  CREATE (window_open:Event)
    -[:CAUSES {
        strength: 0.73,
        lag_minutes: 17,
        confidence: 0.60,    // Initial, will grow with evidence
        evidence_count: 23,  // Events observed so far
        method: "granger + embedding_validation"
    }]->
  (co2_drop:Outcome)


DAY 30-60: CONTINUOUS LEARNING
===============================

Step 4: Each new window_open event triggers
  a. Embed context window
  b. Predict CO2 trajectory from similar past contexts (K-NN retrieval)
  c. Record prediction: "CO2 will drop ~120 ppm in 17 min"
  d. 17 min later, measure actual outcome
  e. Update:
     - GNN edge confidence: 0.60 -> 0.65 -> 0.70 -> ...
     - ReasoningBank: new trajectory entry
     - Q-Learning: Q(high_co2_weekday, suggest_open_window) += reward

Step 5: GNN Weekly Training
  - Message passing discovers: outdoor_temp also affects CO2 drop magnitude
  - When outdoor_temp > 20C, window_open causes LARGER CO2 drop
  - New edge discovered through multi-hop reasoning:
    outdoor_temp -> ventilation_effectiveness -> co2_drop_magnitude

Step 6: SONA/EWC++ Adaptation
  - Winter pattern learned: window_open less effective (cold air, shorter openings)
  - Summer pattern learned: window_open very effective
  - EWC++ preserves both seasonal models without forgetting


DAY 60+: PREDICTIVE OPERATION
==============================

Step 7: Real-Time Prediction Pipeline
  14:25 - CO2 reading: 750 ppm, trending up (+30 ppm/hour)
  14:25 - Embed current context
  14:25 - ruvector search: 15 similar past contexts found
  14:25 - In 13/15 similar contexts, CO2 exceeded 800 within 30 min
  14:26 - PREDICTIVE ALERT:
          "CO2 likely to exceed 800 in ~25 minutes (87% confidence)"
          "Based on 13 similar past situations"
          "Suggested action: open window (effectiveness: 82% in similar conditions)"

Step 8: User Acts
  14:28 - User opens window

Step 9: Outcome Measurement
  14:45 - CO2 peaked at 770, now dropping
  14:55 - CO2 at 680 (below threshold)

Step 10: Learning Update
  - Prediction was correct (CO2 would have exceeded 800)
  - Action was effective (CO2 dropped)
  - ReasoningBank entry stored
  - GNN edge reinforced: confidence now 0.87
  - Q-value updated: Q(rising_co2_afternoon, suggest_open_window) = 0.84


ARCHITECTURE DIAGRAM
=====================

    +-------------------+        +-------------------+
    |  Gold Layer       |        |  ruvector         |
    |  (TimescaleDB)    |        |  (HNSW + GNN)     |
    |                   |        |                   |
    |  aligned_hourly   |------->|  Hourly embeddings|
    |  events_unified   |------->|  Event embeddings |
    |  events_hourly    |        |  Context windows  |
    +--------+----------+        +--------+----------+
             |                            |
             v                            v
    +-------------------+        +-------------------+
    |  V1.2 Scanner     |<------>|  Similarity Search|
    |  (Granger)        |        |  (pre-filter +    |
    |                   |        |   validation)     |
    +--------+----------+        +--------+----------+
             |                            |
             v                            v
    +-------------------+        +-------------------+
    |  Candidate        |        |  Causal Graph     |
    |  Registry         |<------>|  (GNN edges)      |
    |  (static today)   |        |  (dynamic,        |
    |                   |        |   self-improving)  |
    +--------+----------+        +--------+----------+
             |                            |
             v                            v
    +-------------------+        +-------------------+
    |  V1.3 Prediction  |<------>|  ReasoningBank    |
    |  + Action         |        |  + Q-Learning     |
    |                   |        |  + SONA/EWC++     |
    +-------------------+        +-------------------+
```

---

## 6. Integration Architecture

### 6.1 Where RuVector Fits in NDP's Data Flow

```
BRONZE          SILVER           GOLD             INTELLIGENCE
(Parquet)       (TimescaleDB)    (TimescaleDB)    (ruvector)
                                  |
  Ingest -->    Cleanse -->      Aggregate -->    Embed -->
  Raw data      Validated        Hourly CAs       64-dim vectors
                                  |                |
                                  v                v
                              Aligned View      HNSW Index
                                  |                |
                                  v                v
                              Events Unified    Causal Graph (GNN)
                                  |                |
                                  v                v
                              V1.2 Scanner      Similarity Search
                                  |                |
                                  v                v
                              Candidates        ReasoningBank
                                  |                |
                                  v                v
                              V1.3 Prediction   Q-Learning
```

### 6.2 Resource Requirements on Pi 5

| Component | Memory | CPU (avg) | Disk | Notes |
|-----------|--------|-----------|------|-------|
| HNSW index (160K vectors, 64-dim, PQ8) | ~5 MB | <1% | ~5 MB | Sub-millisecond search |
| GNN model (graph with ~100 edges) | ~10 MB | 5% (weekly training) | ~2 MB | Weekly batch training |
| ReasoningBank (1000 trajectories) | ~20 MB | <1% | ~50 MB | Append-only, compress old |
| Q-Table (12 clusters * 5 actions) | <1 MB | <1% | <1 MB | Trivial |
| Embedding computation (per hour) | ~5 MB burst | 2% burst | - | Matrix multiply |
| **Total ruvector addition** | **~40 MB** | **~3% avg** | **~58 MB** | Fits within budget |

Current NDP memory usage: ~750 MB of 16 GB. Adding ruvector: ~790 MB. Headroom: 15.2 GB.

### 6.3 Implementation Phasing

```
Phase 1: Foundation (With V1.2, 2-3 weeks)
------------------------------------------
- Implement normalized feature vector embeddings (Approach A)
- Set up ruvector HNSW index alongside Gold layer
- Embed each hourly aligned view row on refresh
- Embed event context windows on detection
- Basic similarity search API: "find hours similar to this one"
- Use for: anomaly detection (embedding distance)

Phase 2: Pattern Acceleration (V1.2 enhancement, 2 weeks)
----------------------------------------------------------
- Integrate embedding similarity as pre-filter for Granger scanner
- Implement embedding-validated response analysis
- Initialize causal graph with Granger-discovered edges
- Store discovery trajectories in ReasoningBank

Phase 3: Predictive Triggers (V1.3 foundation, 3 weeks)
--------------------------------------------------------
- Implement K-NN outcome prediction from similar contexts
- Add context-aware thresholds via cluster centroids
- Wire Q-Learning for action selection
- Implement SONA/EWC++ for seasonal adaptation

Phase 4: Self-Improving Intelligence (V1.3+, ongoing)
------------------------------------------------------
- Weekly GNN training on accumulated evidence
- Multi-hop causal chain discovery
- ReasoningBank-based explanation generation
- Autoencoder embeddings (Approach B) to replace feature vectors
```

---

## 7. Use Case Value Summary

| Use Case | Value Rating | NDP Version | Effort | Risk |
|----------|-------------|-------------|--------|------|
| Event sequence embedding | **High** | V1.2 | 2 weeks | Low |
| Similarity-based anomaly detection | **Medium-High** | V1.2 | 1 week | Low |
| Granger pre-filtering via similarity | **Medium** (now), **High** (V2.0) | V1.2 | 1 week | Low |
| Causal graph as GNN | **High** | V1.2-V1.3 | 3 weeks | Medium |
| Context-aware thresholds | **Medium-High** | V1.2-V1.3 | 2 weeks | Low |
| Predictive triggers (K-NN) | **High** | V1.3 | 2 weeks | Low |
| ReasoningBank trajectories | **High** | V1.2-V1.3 | 2 weeks | Low |
| Q-Learning action selection | **Medium** | V1.3 | 3 weeks | Medium |
| SONA seasonal adaptation | **Medium-High** | V1.3+ | 3 weeks | Medium |
| Hyperbolic event embeddings | **Low** (now) | V2.0+ | 2 weeks | Low |

---

## 8. Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| ruvector maturity (rvLite v0.1.0) | High | Medium | Use core ruvector crate, not rvLite. Monitor releases. |
| Embeddings too simple for meaningful similarity | Medium | Medium | Start with Approach A; if precision <0.7, upgrade to autoencoder (Approach B). |
| GNN training unstable on small graph | Medium | Low | Start GNN only after 50+ confirmed edges. Use simple edge reinforcement before GNN. |
| SONA catastrophic forgetting despite EWC++ | Low | Medium | Keep seasonal checkpoints. Validate on held-out winter data before deploying summer model. |
| Over-engineering for 15 variables | Medium | Low | Acknowledge: brute-force Granger works fine for 15 vars. ruvector value is in scaling to 50+ vars (V2.0) and in the predictive trigger capability. |

---

## 9. Comparison with Planned V1.2/V1.3 Approaches

| Capability | Planned Approach | With RuVector | Verdict |
|------------|-----------------|---------------|---------|
| Correlation scanning | Granger causality (brute-force) | Granger + embedding pre-filter | **Complement** (ruvector accelerates, does not replace) |
| Candidate storage | Static registry (v12-008) | GNN causal graph (dynamic) | **Upgrade** (ruvector replaces static with living graph) |
| Anomaly detection | Z-score / IQR (V1.2 scope) | Embedding distance | **Complement** (both useful; embedding catches multivariate anomalies) |
| Prediction | Model tournament (V1.3) | K-NN from similar contexts | **Complement** (K-NN for fast intuition, models for accuracy) |
| Action selection | Graduated autonomy (V1.3) | Q-Learning | **Complement** (Q-Learning implements the "learning" in graduated autonomy) |
| Seasonal handling | Not explicitly planned | SONA/EWC++ | **Addition** (fills a gap in current roadmap) |
| Explainability | Not planned until V1.3+ | ReasoningBank | **Addition** (fills a significant gap) |

---

## 10. Conclusions

RuVector's capabilities map well onto NDP's event intelligence needs, particularly for three areas:

1. **Event embedding + similarity retrieval** provides immediate value with zero model training, complementing V1.2's Granger causality approach.

2. **GNN-backed causal graph** evolves NDP's static candidate registry into a living knowledge structure that improves with every confirmed observation.

3. **ReasoningBank** fills a gap in NDP's current roadmap: explainability. When the system suggests "open window," it can point to specific past episodes that inform the recommendation.

The resource footprint (~40 MB RAM, ~3% CPU) fits comfortably within Pi 5's budget. Implementation should be phased to align with V1.2 and V1.3 milestones, starting with normalized feature embeddings (no training required) and evolving toward learned embeddings as data accumulates.

The primary risk is over-engineering for NDP's current variable count (15). The strongest ROI case for ruvector is when the platform scales to V2.0 multi-domain intelligence (50+ variables) where brute-force approaches become costly. For now, the predictive trigger capability (K-NN outcome prediction from similar contexts) is the single highest-value feature, as it provides V1.3-like prediction capability with zero model training.

---

## References

### NDP Architecture
- `/workspaces/neural-data-platform/product/features/fe-001/SCOPE.md` -- V1.1 Gold Layer Foundation scope
- `/workspaces/neural-data-platform/product/features/gold-001/FEATURE-ROADMAP.md` -- V1.1 to V2.0 roadmap
- `/workspaces/neural-data-platform/product/features/fe-001/phase-e/specification/PHASE-E-OVERVIEW.md` -- Unified Event Abstraction
- `/workspaces/neural-data-platform/product/features/fe-001/phase-e/refinement/V12-HANDOFF-CHECKLIST.md` -- V1.2 interface contract
- `/workspaces/neural-data-platform/crates/ndp-lib/src/gold/generators/events.rs` -- Events hypertable DDL generator

### Causal Discovery
- `/workspaces/neural-data-platform/product/research/gold/autonomous-edge/causal-discovery/LIGHTWEIGHT-ALGORITHMS.md` -- Algorithm analysis for Pi
- `/workspaces/neural-data-platform/product/research/gold/autonomous-edge/integration-pattern/UNIFIED-ARCHITECTURE.md` -- Full loop architecture
- `/workspaces/neural-data-platform/product/research/gold/autonomous-edge/recommendations/VISION.md` -- 3-year vision

### RuVector
- `/workspaces/neural-data-platform/research/agenticdataplatform/02-ruvector-analysis.md` -- Initial ruvector analysis
- RuVector repository: https://github.com/ruvnet/ruvector

---

**Document Version:** 1.0
**Author:** Research Agent
**Status:** Complete
**Last Updated:** 2026-02-10
