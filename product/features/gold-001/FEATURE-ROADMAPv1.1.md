# Gold Layer Feature Roadmap v1.1: Integrated Intelligence

> **Supersedes:** FEATURE-ROADMAP.md (v1.0)
> **Created:** 2026-02-10
> **Method:** Working Backwards from V2.0, augmented with ruvector intelligence layer research
> **Status:** Draft for Review
> **Research Basis:** `product/research/ruvector/` (6 documents, 5-agent swarm)

---

## What Changed and Why

The original roadmap (v1.0) defined a four-version journey:

```
V1.1 Gold Foundation → V1.2 Pattern Detection → V1.3 Prediction & Actions → V2.0 Multi-Stream Intelligence
```

V1.1 is now **complete** (config-driven Gold layer, continuous aggregates, alignment views, feature registry, events infrastructure). The question is: what does V1.2 actually look like?

The v1.0 roadmap defined V1.2 as a pure **statistical** engine: Granger causality testing all O(n^2) stream pairs, building a candidate registry, ranking by strength. This works but it's slow to develop, compute-heavy on the Pi, and requires 30+ days of data before any insight.

Research into [ruvector](https://github.com/ruvnet/ruvector) revealed a complementary **similarity** path: embed the current sensor state as a numerical vector, search for similar past states via HNSW, look up what happened next. This provides prediction from day 30 with zero model training, and accuracy improves automatically as data accumulates.

**These paths are not alternatives. They reinforce each other.**

| Path | Provides | Limitation |
|------|----------|------------|
| **Similarity** (ruvector) | Instant predictions by analogy, anomaly detection, self-improving | Correlation is not causation |
| **Statistical** (Granger) | Causal validation, confidence intervals, hypothesis testing | Slow, compute-heavy, requires large datasets |
| **Combined** | Predictions with causal backing, fewer false positives | More infrastructure to build |

The similarity path acts as a **candidate generator** for the statistical path. Instead of testing all stream pairs exhaustively, embed current state, find similar past states, identify which stream changes co-occurred, then validate only those relationships with Granger. Orders of magnitude fewer tests.

This document redefines V1.2-V2.0 with both paths integrated.

---

## Updated Vision

### The Capability Chain (Revised)

```
V1.0          V1.1           V1.2                V1.3              V2.0
-----         ----           ----                ----              ----
Ingest   ->   Prepare    ->  Detect + Remember -> Predict/Act  ->  New Domain
Data          for Detection  via Similarity      with Learning     via Config
                             + Statistics         + Adaptation

Bronze->Silver Gold Layer    Dual Intelligence   SONA + Causal     Multi-Stream
Pipeline       Foundation    Engine              Validation         Intelligence
                             (NEW: ruvector)     (REVISED: SONA)
```

### The Key Insight

V1.2 is no longer just "scan for correlations." It's "build a memory of situations and what happened next." The platform doesn't just analyze — it **remembers**. Every hour of sensor data becomes an episode in a growing library of experience. When conditions resemble a past episode, the system recalls what followed.

This is fundamentally different from the v1.0 approach where intelligence was a post-hoc statistical analysis. Now intelligence is an **accumulating asset** — each hour of data makes the system smarter.

---

## What Actually Calls Ruvector

This section defines the concrete execution flow on the Pi. No abstractions — what code runs, what data moves, when.

### The Runtime Architecture

Intelligence runs as a **separate process** from ingestion. The workload profiles are fundamentally different:

| | Ingestion (air-quality-app) | Intelligence (ndp-intelligence-app) |
|---|---|---|
| Trigger | Per-event (MQTT/HTTP, ~1/min/sensor) | Batch (every 15 min, reads Gold) |
| Bound | I/O (network, disk writes) | Compute (HNSW, Granger, SONA) |
| Failure mode | Must not drop data | Can miss a cycle, catch up next |
| Memory profile | Stable (~120MB) | Spiky (HNSW rebuild, model ops) |
| Restart tolerance | Low (WAL recovery, gap risk) | High (rebuilds from pgvector) |
| Deploy frequency | Rare (ingestion is stable) | Higher (model tuning, features) |

**Key principle:** Deploying a new intelligence version should never require restarting ingestion. On the Pi, any ingestion restart risks data gaps during WAL replay.

The intelligence service reads from **Gold** (TimescaleDB), not from the EventBus. There is no shared-memory argument for co-location. TimescaleDB is the integration point.

```
Sources (MQTT/HTTP) ──> air-quality-app ──> Bronze (Parquet)
                                        ──> Silver (TimescaleDB)
                                                    │
                                          Gold CAs refresh (15 min)
                                                    │
                                                    ▼
                              ┌─────────────────────────────────────────────┐
                              │          TimescaleDB (PostgreSQL 15)         │
                              │                                              │
                              │  timescaledb extension (existing)            │
                              │  pgvector extension (NEW — durable vectors) │
                              │                                              │
                              │  gold.aligned_hourly  ← intelligence reads  │
                              │  gold.sensor_embeddings (pgvector)           │
                              │  gold.predictions (results)                  │
                              │  gold.learning_episodes (SONA)              │
                              └──────────────────┬──────────────────────────┘
                                                 │
                              ┌──────────────────┴──────────────────────────┐
                              │   ndp-intelligence-app (separate binary)     │
                              │   Docker container, own memory limit         │
                              │                                              │
                              │   IntelligenceService                        │
                              │     ruvector-core  (HNSW index, in-process) │
                              │     ruvector-sona  (learning, V1.3+)        │
                              │     ruvector-attention (causal, V1.3+)      │
                              │                                              │
                              │   Runs on timer (every 15 min)              │
                              │   OR triggered by PG NOTIFY after CA refresh│
                              │   OR one-shot via CLI: ndp intelligence run │
                              └─────────────────────────────────────────────┘
```

**Three execution modes for the intelligence binary:**

| Mode | Use Case | Trigger |
|------|----------|---------|
| **Daemon** | Production on Pi | `ndp-intelligence-app --daemon` — runs forever on 15-min timer |
| **Notify** | Production (reactive) | `ndp-intelligence-app --listen` — wakes on PG NOTIFY from CA refresh |
| **One-shot** | CLI / testing / backfill | `ndp intelligence run` or `ndp intelligence backfill --from 2026-01-01` |

### The Intelligence Cycle (Every 15 Minutes)

```
┌─ 1. OBSERVE ──────────────────────────────────────────────────────────────┐
│                                                                            │
│  Gold continuous aggregates refresh (existing, every 15 min).             │
│  New hourly bucket row appears in gold.aligned_hourly with:               │
│    indoor_pm25_mean, indoor_co2_mean, indoor_temp_mean, ...              │
│    outdoor_temp_mean, outdoor_humidity_mean, wind_speed_mean, ...        │
│    state_transition_count, last_window_state, ...                        │
│    co2_trend_4h, pm25_rolling_std_4h, ...                                │
│                                                                            │
│  ndp-intelligence-app reads latest aligned row via tokio-postgres.        │
└────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─ 2. EMBED ────────────────────────────────────────────────────────────────┐
│                                                                            │
│  Build numerical feature vector from aligned row (~28-32 dimensions):     │
│                                                                            │
│  Temporal [3D]:  hour_sin, hour_cos, is_weekend                          │
│  Indoor   [5D]:  co2, pm25, temp, humidity, voc (z-score normalized)     │
│  Outdoor  [4D]:  temp, humidity, wind_speed, pressure                    │
│  AQI      [2D]:  outdoor_pm25, outdoor_aqi                               │
│  State    [3D]:  window_transitions, door_transitions, window_state      │
│  Derived  [6D]:  co2_trend_4h, pm25_trend_4h, co2_std_4h,              │
│                   pm25_std_4h, co2_diff_1h, pm25_diff_1h                 │
│  Forecast [3D]:  forecast_temp, forecast_precip, forecast_wind           │
│                                                                            │
│  Normalize via rolling z-score (StandardScaler from features.rs).         │
│  Result: Vec<f32> of length ~28-32.                                      │
│                                                                            │
│  NOTE: No LLM, no text embeddings, no embedding model. Pure numerical.   │
└────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─ 3. REMEMBER ─────────────────────────────────────────────────────────────┐
│                                                                            │
│  Store embedding durably in TimescaleDB via pgvector:                     │
│                                                                            │
│    INSERT INTO gold.sensor_embeddings (bucket, embedding, metadata)       │
│    VALUES ($1, $2::vector(32), $3);                                       │
│                                                                            │
│  Also insert into ruvector-core in-process HNSW index:                    │
│                                                                            │
│    let entry = VectorEntry {                                              │
│        id: Some(bucket_id),                                               │
│        vector: embedding.iter().map(|x| *x as f32).collect(),            │
│        metadata: Some(metadata_map),                                      │
│    };                                                                      │
│    db.insert(entry)?;                                                      │
│                                                                            │
│  Two stores: pgvector for durability + SQL JOINs, ruvector for speed.    │
└────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─ 4. SEARCH ───────────────────────────────────────────────────────────────┐
│                                                                            │
│  K-NN search for similar past states via ruvector-core:                   │
│                                                                            │
│    let results = db.search(&SearchQuery {                                 │
│        vector: current_embedding,                                         │
│        k: 20,                                                              │
│        filter: None,                                                       │
│        include_vectors: false,                                             │
│    })?;                                                                    │
│                                                                            │
│  Returns: 20 most similar past hourly states with distance scores.        │
│  Latency: <100 microseconds (HNSW with NEON SIMD on Pi).                │
└────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─ 5. PREDICT ──────────────────────────────────────────────────────────────┐
│                                                                            │
│  For each of the K neighbors, look up what happened NEXT:                 │
│                                                                            │
│    SELECT                                                                  │
│      next.indoor_co2_mean,                                                │
│      next.indoor_pm25_mean,                                               │
│      CASE WHEN next.indoor_co2_mean > 800 THEN TRUE ELSE FALSE END       │
│        AS co2_threshold_breached                                           │
│    FROM gold.aligned_hourly next                                           │
│    WHERE next.bucket = neighbor_bucket + INTERVAL '1 hour';              │
│                                                                            │
│  Aggregate: "In 17 of 20 similar past situations, CO2 exceeded 800       │
│  within 1 hour. Median time to breach: 35 minutes."                       │
│                                                                            │
│  Compare against objectives config:                                        │
│    target: co2 < 800 → prediction says likely breach → flag.             │
│                                                                            │
│  Store prediction:                                                         │
│    INSERT INTO gold.predictions (bucket, metric, prediction, ...)        │
└────────────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─ 6. LEARN (Phase 2+) ────────────────────────────────────────────────────┐
│                                                                            │
│  After the prediction window closes (e.g., 1 hour later):                │
│                                                                            │
│    Was the prediction correct?                                             │
│      actual_co2 = query gold.aligned_hourly at bucket + 1h               │
│      correct = (actual_co2 > 800) == predicted_breach                    │
│      quality_score = f(distance_to_threshold, prediction_confidence)     │
│                                                                            │
│  Record SONA trajectory:                                                   │
│    engine.begin_trajectory(current_embedding)                              │
│    engine.add_step(neighbor_embedding, metadata, signal_strength)         │
│    engine.end_trajectory(builder, quality_score)                           │
│                                                                            │
│  Over time, SONA's micro-LoRA adapts the embedding space so that         │
│  "situations that lead to similar outcomes" cluster together more          │
│  tightly, improving K-NN recall.                                          │
└────────────────────────────────────────────────────────────────────────────┘
```

### What Triggers Each Step

| Step | Trigger | Frequency | Latency Budget |
|------|---------|-----------|---------------|
| OBSERVE | Gold CA refresh → PG NOTIFY (or timer fallback) | Every 15 min | N/A (TimescaleDB internal) |
| EMBED | ndp-intelligence-app wakes, reads Gold | Every 15 min | <10ms |
| REMEMBER | Immediately after EMBED | Every 15 min | <5ms (pgvector INSERT + ruvector insert) |
| SEARCH | Immediately after REMEMBER | Every 15 min | <1ms (ruvector-core HNSW) |
| PREDICT | Immediately after SEARCH | Every 15 min | <50ms (20 SQL lookups, batched) |
| LEARN | 1 hour after PREDICT | Every 15 min (delayed) | <10ms (SONA trajectory) |

**Total intelligence cycle: <75ms every 15 minutes.** This is trivial for the Pi's CPU budget.

**PG NOTIFY pattern** (preferred over timer — reacts to actual data availability):

```sql
-- Added to Gold CA refresh policy callback (or a TimescaleDB job)
CREATE OR REPLACE FUNCTION gold.notify_intelligence_refresh()
RETURNS void AS $$
BEGIN
    PERFORM pg_notify('gold_ca_refreshed', json_build_object(
        'bucket', (SELECT max(bucket) FROM gold.indoor_air_quality_aligned_hourly),
        'refreshed_at', NOW()
    )::text);
END;
$$ LANGUAGE plpgsql;
```

The intelligence binary listens with `LISTEN gold_ca_refreshed` and wakes only when new data is ready — no polling, no wasted cycles.

---

## Updated Version Dependency Chain

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    V2.0: MULTI-STREAM INTELLIGENCE                          │
│                                                                              │
│  "New domain via config → predictions within 30 days (no code)"            │
│                                                                              │
│  REQUIRES FROM V1.3:                                                        │
│  • Stream-agnostic SONA adapters (one base model, per-stream LoRA)        │
│  • Causal validation confirming similarity-discovered relationships         │
│  • Action framework with graduated autonomy                                │
│  • Cross-domain embedding transfer                                         │
│  ENHANCED BY RUVECTOR:                                                      │
│  + Embeddings transfer across domains (weather affects air AND energy)     │
│  + ReasoningBank carries prediction patterns to new domains                │
│  + EWC++ preserves old domain knowledge when learning new domain           │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                V1.3: ADAPTIVE PREDICTION & ACTIONS                          │
│                                                                              │
│  "System predicts + learns from outcomes >80% accuracy"                    │
│                                                                              │
│  REQUIRES FROM V1.2:                                                        │
│  • Validated similarity-based predictions (K-NN operational)               │
│  • Candidate relationships flagged with metadata                           │
│  • Historical evidence from both similarity + statistical paths            │
│  CHANGED BY RUVECTOR:                                                       │
│  × Model tournament (5 separate models, 500MB+)                           │
│  + SONA: 1 base model + micro-LoRA per relationship (~50MB total)         │
│  + EWC++ built-in (no separate implementation)                             │
│  + ReasoningBank for prediction pattern storage                            │
│  + Causal attention (DiffusionAttention from ruvector-attention)           │
│  + Q-Learning advisory for graduated autonomy (~6MB)                       │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│              V1.2: DUAL INTELLIGENCE ENGINE                                 │
│                                                                              │
│  "System remembers situations + identifies candidate relationships"        │
│                                                                              │
│  REQUIRES FROM V1.1:                                                        │
│  • Classified streams (state vs continuous vs forecast) .............. ✅  │
│  • Time-aligned data across all streams ............................ ✅   │
│  • Consistent feature granularity (hourly buckets) ................. ✅   │
│  • Objectives declaring which outcomes matter ...................... ✅   │
│  • State transition events ......................................... ✅  │
│  • Feature registry (lag, rolling, trend) .......................... ✅   │
│  NEW INFRASTRUCTURE:                                                        │
│  • pgvector extension in TimescaleDB (durable embedding storage)          │
│  • ruvector-core as Cargo dependency (in-process HNSW search)             │
│  • Numerical embedding pipeline (Gold aligned row → Vec<f32>)             │
│  • K-NN predictive search + outcome lookup                                 │
│  RETAINED FROM V1.0 ROADMAP:                                               │
│  • Granger causality scanner (validates similarity-discovered candidates) │
│  • Candidate registry with metadata (lag, strength, direction)            │
│  • Transition event materializer (already in Gold DDL)                     │
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                    V1.1: GOLD LAYER FOUNDATION ✅ COMPLETE                  │
│                                                                              │
│  Config-driven Gold DDL generation, continuous aggregates,                  │
│  aligned views, feature registry, events infrastructure,                    │
│  stream classification, objectives schema.                                  │
│                                                                              │
│  See FEATURE-ROADMAP.md for original V1.1 feature details.                │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## V1.2: Dual Intelligence Engine

### Vision Statement (Revised)

> The system builds a growing library of sensor situations. Every hour, it asks: "have I seen conditions like this before? What happened next?" When it finds meaningful patterns, it flags them. Separately, statistical testing validates which patterns have genuine causal backing. Intelligence accumulates automatically — no model training, no human configuration beyond the initial objectives.

### Two Parallel Tracks

#### Track A: Similarity Intelligence (ruvector)

Provides immediate value from day 30. Works by analogy — "find similar past situations, predict by what happened then."

| ID | Feature | Description | Depends On |
|----|---------|-------------|------------|
| **v12-S01** | pgvector Extension Install | Add pgvector to TimescaleDB container on Pi | V1.1 Docker setup |
| **v12-S02** | Sensor Embedding Pipeline | Gold aligned row → normalized Vec<f32> → pgvector + ruvector-core | V1.1 aligned view |
| **v12-S03** | HNSW Index Bootstrap | Load historical embeddings from pgvector into ruvector-core on startup | v12-S01, v12-S02 |
| **v12-S04** | K-NN Predictive Search | Search K nearest past states, look up outcomes | v12-S03 |
| **v12-S05** | Prediction Generation | Aggregate neighbor outcomes into predictions against objectives | v12-S04, V1.1 objectives |
| **v12-S06** | Prediction Storage | Store predictions in `gold.predictions` with confidence | v12-S05 |
| **v12-S07** | Outcome Tracking | After prediction window, record actual vs predicted | v12-S06 |
| **v12-S08** | Embedding-Distance Anomaly Detection | Flag hours where embedding distance from all clusters exceeds threshold | v12-S03 |
| **v12-S09** | Sensor Fingerprinting Dashboard | Grafana panel showing embedding clusters, predictions, anomalies | v12-S06, v12-S08 |

#### Track B: Statistical Validation (Granger, retained from v1.0)

Provides causal rigor. Guided by Track A's discoveries rather than exhaustive search.

| ID | Feature | Description | Depends On |
|----|---------|-------------|------------|
| **v12-G01** | Similarity-Guided Candidate Generation | Use K-NN results to identify which stream pairs to test | v12-S04 |
| **v12-G02** | Granger Causality Scanner | Pairwise Granger test on candidate pairs (not all pairs) | v12-G01, V1.1 aligned view |
| **v12-G03** | Lag Optimizer | Find optimal lag (0-60 min) for each validated relationship | v12-G02 |
| **v12-G04** | Candidate Registry | Store validated correlations with metadata | v12-G02, v12-G03 |
| **v12-G05** | Candidate Ranker | Rank by strength x relevance to objectives | v12-G04, V1.1 objectives |
| **v12-G06** | Evidence Accumulator | Track relationship stability over time | v12-G04 |
| **v12-G07** | Pattern Candidates Dashboard | Grafana panels showing validated causal relationships | v12-G04, v12-G05 |

#### How the Tracks Reinforce Each Other

```
Track A (Similarity):                    Track B (Statistical):

  Current state → K-NN search
       │
       ▼
  "These 20 past hours looked            Exhaustive search would test
   like now. In 15 of them,              ALL stream pairs = O(n^2).
   CO2 spiked after window               That's expensive and slow.
   was closed."
       │
       ▼
  CANDIDATE: window_state →              Instead, Track A says:
  co2 relationship observed              "Test window_state → co2.
  in similar situations.                  And outdoor_temp → indoor_pm25.
       │                                  I've seen these co-occur."
       │                                          │
       └──────────────> v12-G01 ─────────────────┘
                        │
                        ▼
                   Granger validates ONLY
                   the top candidates.
                   Confirms: window_state
                   Granger-causes co2
                   with 17-min lag. p<0.01.
```

**Result:** 5-10 targeted Granger tests instead of hundreds. Faster, cheaper, fewer false positives.

### V1.2 Infrastructure Features

| ID | Feature | Description | Notes |
|----|---------|-------------|-------|
| **v12-I01** | ndp-intelligence crate (library) | Cargo workspace member housing all intelligence logic | See architecture below |
| **v12-I02** | ndp-intelligence-app (binary) | Standalone daemon/CLI binary, own Docker container | Separate from air-quality-app |
| **v12-I03** | IntelligenceService | Core service orchestrating embed → search → predict → learn | Library entry point |
| **v12-I04** | Embedding Config Schema | JSON Schema for embedding pipeline configuration in stream config | Extends gold_etl section |
| **v12-I05** | Intelligence CLI | `ndp intelligence status`, `ndp intelligence search --like-now`, `ndp intelligence run` | Extends ndp-cli |
| **v12-I06** | PG NOTIFY Integration | Gold CA refresh triggers intelligence wake-up via LISTEN/NOTIFY | Zero-polling architecture |
| **v12-I07** | Docker Service | `ndp-intelligence` service in docker-compose with own memory limit | Independent deployment |

### Crate + Binary Architecture

```
crates/ndp-intelligence/             # LIBRARY — all intelligence logic
  Cargo.toml                          # ruvector-core, tokio-postgres, ndp-types
  src/
    lib.rs                            # Public API: IntelligenceService, EmbeddingPipeline
    config.rs                         # IntelligenceConfig (parsed from stream/domain config)
    embeddings.rs                     # Gold aligned row → Vec<f32> (z-score normalization)
    similarity.rs                     # ruvector-core VectorDB wrapper, K-NN, anomaly detection
    predictions.rs                    # Outcome lookup, prediction aggregation, confidence
    granger.rs                        # Granger causality test (pure Rust, ndarray)
    candidates.rs                     # Candidate registry, ranking, evidence accumulation
    storage.rs                        # pgvector read/write, prediction table read/write
    traits.rs                         # IntelligenceProvider trait (testability)

apps/ndp-intelligence-app/           # BINARY — standalone service
  Cargo.toml                          # ndp-intelligence, tokio, clap, tracing
  src/
    main.rs                           # CLI args, daemon loop, PG LISTEN, one-shot mode
```

**Docker Compose addition** (deploy/pi/docker-compose.yml):

```yaml
ndp-intelligence:
  build:
    context: ../..
    dockerfile: deploy/pi/Dockerfile.intelligence
  depends_on:
    - timescaledb
  environment:
    - DATABASE_URL=postgresql://ndp:${POSTGRES_PASSWORD}@timescaledb:5432/ndp
    - INTELLIGENCE_MODE=notify        # daemon | notify | oneshot
    - RUST_LOG=ndp_intelligence=info
  deploy:
    resources:
      limits:
        memory: 256M                  # Independent limit, doesn't affect ingestion
  restart: unless-stopped
```

**Key design decisions:**

1. **Separate process, not embedded in air-quality-app.** Workload profiles are fundamentally different: ingestion is I/O-bound and crash-sensitive; intelligence is compute-bound and restart-tolerant. Updating intelligence should never risk ingestion downtime. On the Pi, ingestion restart = potential data gap.

2. **Library + binary split.** `ndp-intelligence` is a library crate. The binary (`ndp-intelligence-app`) is thin — CLI args, daemon loop, PG LISTEN. This means the library can also be called from `ndp-cli` for one-shot operations like backfill or ad-hoc queries, without duplicating logic.

3. **TimescaleDB is the integration point.** Intelligence reads from Gold aligned views and writes to `gold.predictions`, `gold.sensor_embeddings`, `gold.causal_candidates`. No shared memory, no EventBus coupling. The two binaries are connected only through the database.

4. **ruvector-core compiles into ndp-intelligence-app**, not air-quality-app. Its memory footprint (HNSW index, SONA state) lives in the intelligence container's 256MB budget, isolated from ingestion.

5. **pgvector is the durable store; ruvector-core is the compute engine.** On startup, load historical embeddings from pgvector into ruvector-core HNSW. On each cycle, write to both. If the process restarts, pgvector has everything; ruvector-core rebuilds from pgvector.

6. **Granger is pure Rust.** No Python, no R, no external dependency. The Granger test is a series of OLS regressions — implementable in ~200 lines with `ndarray` (already in ruvector-core's dep tree).

7. **The library crate does NOT depend on core.** It depends on `ndp-types` for shared types and `tokio-postgres` for DB access. No coupling to ingestion code.

### Embedding Config (Added to Stream Config)

```json
{
  "stream_id": "air-quality",
  "gold_etl": {
    "...existing aggregates/features/transitions..."
  },
  "intelligence": {
    "enabled": true,
    "embedding": {
      "dimensions": 32,
      "fields": ["co2_mean", "pm25_mean", "temperature_c_mean", "humidity_pct_mean"],
      "derived": ["co2_trend_4h", "pm25_trend_4h", "co2_std_4h", "pm25_std_4h"],
      "temporal": ["hour_sin", "hour_cos", "is_weekend"],
      "normalization": "z_score",
      "normalization_window": "30 days"
    },
    "search": {
      "k": 20,
      "min_similarity": 0.7,
      "prediction_horizons": ["1 hour", "4 hours"],
      "hnsw": {
        "m": 16,
        "ef_construction": 100,
        "ef_search": 50
      }
    }
  }
}
```

The **domain alignment config** (which joins streams for Gold aligned view) would also get an `intelligence` section specifying which aligned view feeds the embedding pipeline:

```json
{
  "domain_id": "indoor_air_quality",
  "alignment": { "...existing..." },
  "intelligence": {
    "enabled": true,
    "source_view": "gold.indoor_air_quality_aligned_hourly",
    "embedding_streams": ["air-quality", "outdoor-weather", "home-assistant-state"],
    "context_streams": ["nws-forecast-hourly"]
  }
}
```

### V1.2 Acceptance Criteria

| Criterion | Target | Measurement |
|-----------|--------|-------------|
| Embedding pipeline operational | 100% of aligned hours embedded | `SELECT count(*) FROM gold.sensor_embeddings` |
| K-NN search latency | <1ms p99 | In-process timing |
| Prediction accuracy (30-day data) | >60% for 1-hour horizon | Outcome tracking table |
| Anomaly detection | Flags >80% of threshold crossings in advance | Compare with gold.events_unified |
| Granger validation | >3 statistically significant relationships | Candidate registry |
| False positive rate | <30% of similarity candidates rejected by Granger | Candidate registry rejection count |
| Config-only addition | New stream to intelligence pipeline via JSON only | Add outdoor-air-quality, no code |
| Pi resource budget | <200MB additional RAM, <5% CPU sustained | `docker stats` |
| Startup time | <30s to rebuild HNSW from pgvector | Measured on Pi |

---

## V1.3: Adaptive Prediction & Actions (Revised)

### Vision Statement (Revised)

> The system doesn't just predict — it learns. When it says "CO2 will exceed 800 in 30 minutes" and is correct, the prediction pathway strengthens. When wrong, it adapts. Over months, the system's predictions become specific to THIS house, THIS climate, THESE habits. SONA provides this adaptation with a single 50MB base model instead of a 500MB model zoo.

### What Changed from v1.0

| v1.0 Approach | v1.1 Approach | Why |
|---------------|---------------|-----|
| Train 5 separate models (TCN, ARIMA, Prophet, MLP, NHITS) | ONE SONA base model + per-relationship micro-LoRA adapters (~2KB each) | 90% memory reduction. 50MB vs 500MB+ |
| Model tournament: run all 5, compare holdout performance | Adapter selection: switch LoRA in microseconds, test on recent data | Same concept, dramatically cheaper |
| Implement EWC++ manually from research papers | EWC++ included in ruvector-sona crate | 3-4 weeks saved |
| Static action rules | Q-Learning advisory layer with user ceiling | Graduated autonomy with learning |
| No explainability | ReasoningBank stores decision trajectories | "Why did you predict this?" has an answer |

### V1.3 Features (Revised)

| ID | Feature | Description | Depends On |
|----|---------|-------------|------------|
| **v13-001** | SONA Integration | Initialize SonaEngine with NDP's embedding dimensions | v12-S02 embedding pipeline |
| **v13-002** | Trajectory Recording | Record prediction episodes as SONA trajectories | v12-S07 outcome tracking |
| **v13-003** | Micro-LoRA Adaptation | Per-relationship micro-LoRA adapters for embedding transformation | v13-001, v13-002 |
| **v13-004** | ReasoningBank Patterns | Cluster successful prediction patterns for retrieval | v13-002 trajectories |
| **v13-005** | Causal Validation Engine | PC algorithm + natural experiment detection on Granger candidates | v12-G04 candidate registry |
| **v13-006** | DiffusionAttention Integration | ruvector-attention for time-series causal structure learning | v13-005 validated relationships |
| **v13-007** | Action Framework | Define actions with preconditions, effects, safety limits | V1.1 objectives |
| **v13-008** | Q-Learning Advisory | Recommend actions based on learned Q-values, constrained by user ceiling | v13-007 actions, v13-004 patterns |
| **v13-009** | Outcome Feedback Loop | Action → outcome pairs feed SONA trajectories + Q-values | v13-007, v13-008 |
| **v13-010** | EWC++ Seasonal Memory | Prevent catastrophic forgetting across seasons | v13-003 (built into SONA) |
| **v13-011** | Prediction + Actions Dashboard | Forecasts, confidence, suggested actions, learning progress | v13-001 through v13-009 |

### V1.3 Go/No-Go: SONA vs ARIMA Benchmark

Before committing to SONA for production, run a **2-week minimum viable test**:

1. Train ARIMA on 30 days of CO2 data (baseline)
2. Initialize SONA with same 30 days, record trajectories for 2 weeks
3. Compare 1-hour prediction accuracy: SONA vs ARIMA vs K-NN (V1.2 baseline)

**If SONA >= K-NN accuracy:** Proceed with SONA as the learning engine.
**If SONA < K-NN:** Keep K-NN as primary predictor, use SONA only for embedding adaptation (still valuable for improving K-NN recall).

### V1.3 Acceptance Criteria

| Criterion | Target |
|-----------|--------|
| Prediction accuracy (1-hour, with SONA) | >80% |
| SONA memory footprint | <60MB (base model + all adapters) |
| Action suggestion acceptance rate | >80% when system has 3+ months of data |
| Seasonal adaptation | System maintains accuracy through season change |
| ReasoningBank pattern retrieval | <5ms for "why did you predict this?" |
| Learning improvement | Measurable accuracy increase month-over-month |

---

## V2.0: Multi-Stream Intelligence (Revised)

### Vision Statement (Revised)

> A user adds energy meter + solar panel streams via JSON config. Within 30 days, the platform discovers that the same weather patterns driving indoor air decisions also predict energy costs and solar output. The system makes energy predictions using embeddings trained on air quality data — without being told the relationship exists. This is cross-domain transfer via shared embedding space, running on a $75 Raspberry Pi.

### What Ruvector Adds to V2.0

| v1.0 V2.0 Capability | v1.1 V2.0 Capability | How |
|----------------------|----------------------|-----|
| New domain via config | Same, plus predictions within 30 days | K-NN works immediately on new streams |
| Stream-agnostic learning | Same, plus transfer from existing domains | SONA adapters from Domain A seed Domain B |
| Cross-stream correlation scanner | Same, plus embedding-pre-filtered candidates | Similarity search identifies cross-domain patterns |
| *Not planned* | Seasonal memory across all domains | EWC++ preserves old domain patterns |
| *Not planned* | Explainable cross-domain predictions | ReasoningBank shows shared prediction patterns |
| *Not planned* | Federated learning (future, multi-Pi) | Binary vector sync across fleet |

### V2.0 Validation Test (Revised)

**Original test:** Add a new domain via config, zero code changes, system materializes Gold infrastructure.

**Enhanced test:** Add a new domain (e.g., "Energy Efficiency") via config. The system:

1. Materializes Gold aggregates + alignment views (existing V1.1 capability)
2. Begins embedding energy stream data alongside existing streams (V1.2 pipeline)
3. Discovers that weather embeddings (trained on air quality context) are relevant to energy predictions (cross-domain transfer)
4. Within 30 days, makes energy cost predictions using K-NN analogy to past weather states
5. Gradually improves via SONA adaptation specific to energy patterns
6. Does NOT forget air quality patterns while learning energy (EWC++)

**Success criteria (enhanced):**

| Metric | Target |
|--------|--------|
| Code changes required | Zero |
| Config-to-infrastructure time | <15 minutes |
| Time to first prediction | <30 days (enough K-NN data) |
| Cross-domain transfer | Weather embeddings improve energy predictions vs cold-start |
| Air quality accuracy retention | No degradation when energy domain is added |

---

## Integration Architecture: pgvector + ruvector-core

### Why Two Vector Engines

| Need | pgvector | ruvector-core | Winner |
|------|----------|---------------|--------|
| Durable storage | ACID, WAL, replication | redb file (no ACID) | pgvector |
| SQL JOINs with Gold data | Native | Impossible | pgvector |
| Grafana queryable | Yes (SQL datasource) | No | pgvector |
| Sub-millisecond search | ~1.5ms (IPC overhead) | <0.1ms (in-process) | ruvector-core |
| Quantization | halfvec only | PQ, scalar, binary, int4 | ruvector-core |
| Adaptive learning | None | SONA, EWC++, trajectories | ruvector-core |
| Attention mechanisms | None | 20+ types | ruvector-core |
| Survives restart | Yes (PostgreSQL) | Rebuilds from pgvector | pgvector |

**They complement each other perfectly. pgvector is the system of record. ruvector-core is the compute engine.**

### Data Flow Between Engines

```
On startup:
  pgvector (all historical embeddings) ──LOAD──> ruvector-core HNSW index

Every 15 minutes:
  Gold aligned view ──READ──> IntelligenceService
    ──EMBED──> Vec<f32>
    ──WRITE──> pgvector INSERT (durable)
    ──WRITE──> ruvector-core insert (fast search)
    ──SEARCH──> ruvector-core K-NN (sub-ms)
    ──LOOKUP──> pgvector/Gold SQL JOIN (outcome data)
    ──PREDICT──> gold.predictions INSERT

On restart:
  pgvector (survived) ──LOAD──> ruvector-core (rebuilt)
  SONA state restored from redb snapshot
```

### pgvector Schema (New Gold Tables)

```sql
-- Enable pgvector in existing TimescaleDB
CREATE EXTENSION IF NOT EXISTS vector;

-- Sensor state embeddings (one per aligned hourly bucket)
CREATE TABLE gold.sensor_embeddings (
    bucket          TIMESTAMPTZ NOT NULL,
    domain_id       TEXT NOT NULL,                    -- e.g., 'indoor_air_quality'
    embedding       vector(32),                       -- numerical feature vector
    metadata        JSONB DEFAULT '{}',               -- source fields, normalization params
    PRIMARY KEY (bucket, domain_id)
);
SELECT create_hypertable('gold.sensor_embeddings', 'bucket');

-- HNSW index for similarity search from SQL (Grafana, ad-hoc)
CREATE INDEX idx_embeddings_hnsw ON gold.sensor_embeddings
    USING hnsw (embedding vector_cosine_ops)
    WITH (m = 16, ef_construction = 64);

-- Predictions generated by intelligence service
CREATE TABLE gold.predictions (
    id              BIGSERIAL,
    bucket          TIMESTAMPTZ NOT NULL,             -- prediction made at this time
    domain_id       TEXT NOT NULL,
    metric          TEXT NOT NULL,                     -- e.g., 'co2'
    horizon         INTERVAL NOT NULL,                -- e.g., '1 hour'
    predicted_value DOUBLE PRECISION,
    predicted_breach BOOLEAN,                          -- will threshold be crossed?
    confidence      DOUBLE PRECISION,                  -- 0-1
    k_neighbors     INTEGER,                           -- how many neighbors used
    k_supporting    INTEGER,                           -- how many support this prediction
    actual_value    DOUBLE PRECISION,                  -- filled in after horizon passes
    actual_breach   BOOLEAN,                           -- filled in after horizon passes
    correct         BOOLEAN,                           -- filled in after horizon passes
    PRIMARY KEY (id, bucket)
);
SELECT create_hypertable('gold.predictions', 'bucket');

-- Validated causal relationships (from Granger)
CREATE TABLE gold.causal_candidates (
    id              SERIAL PRIMARY KEY,
    source_stream   TEXT NOT NULL,
    source_field    TEXT NOT NULL,
    target_stream   TEXT NOT NULL,
    target_field    TEXT NOT NULL,
    lag_minutes     INTEGER,
    correlation     DOUBLE PRECISION,
    granger_p_value DOUBLE PRECISION,
    direction       TEXT,                              -- 'positive' or 'negative'
    evidence_count  INTEGER DEFAULT 0,
    first_detected  TIMESTAMPTZ DEFAULT NOW(),
    last_confirmed  TIMESTAMPTZ DEFAULT NOW(),
    status          TEXT DEFAULT 'candidate'           -- candidate | validated | rejected | decayed
);
```

### Pi Memory Budget (with Intelligence Layer)

Intelligence runs in its own container with an independent memory limit, isolated from ingestion.

| Component | Container | Memory | Cumulative | % of 16GB |
|-----------|-----------|--------|-----------|-----------|
| air-quality-app | ingestion | 123 MB | — | — |
| timescaledb + pgvector | database | 328 MB (+20 for pgvector) | — | — |
| grafana, etcd, mqtt | services | 195 MB | — | — |
| **Existing NDP total** | | **646 MB** | **646 MB** | **4.0%** |
| ndp-intelligence-app (binary) | **intelligence** | +50 MB | 696 MB | 4.3% |
| ruvector-core HNSW (1yr, 32D, f32) | intelligence | +2 MB | 698 MB | 4.3% |
| pgvector HNSW index (1yr, 32D) | database | +5 MB | 703 MB | 4.3% |
| **V1.2 Total** | | **+57 MB** | **703 MB** | **4.3%** |
| ruvector-sona (V1.3) | intelligence | +50 MB | 753 MB | 4.6% |
| SONA adapters (100 relationships) | intelligence | +0.2 MB | 753 MB | 4.6% |
| ReasoningBank | intelligence | +10 MB | 763 MB | 4.7% |
| Q-Learning tables | intelligence | +6 MB | 769 MB | 4.7% |
| **V1.3 Total** | | **+123 MB** | **769 MB** | **4.7%** |

Intelligence container limit: **256 MB** (v12-I07). At V1.3, intelligence uses ~118 MB — well within limit.

**At full V1.3 build-out, NDP uses less than 5% of Pi's RAM.** Headroom is massive. The intelligence container can be stopped/restarted/upgraded without any impact on ingestion.

---

## Implementation Phases

### Phase 0: Go/No-Go Gate (1 day)

Before any integration work, validate that ruvector-core compiles on aarch64:

```bash
cargo init /tmp/rv-arm-test
cd /tmp/rv-arm-test
echo 'ruvector-core = "2.0.1"' >> Cargo.toml
cargo build --target aarch64-unknown-linux-gnu --release
```

**If it compiles:** Proceed with Phase 1A (ruvector-core path).
**If SimSIMD fails:** Retry with `default-features = false, features = ["storage", "hnsw", "parallel"]`.
**If it still fails:** Proceed with Phase 1B (pgvector-only path). Defer ruvector.

### Phase 1A: Similarity Intelligence with ruvector-core (4-5 weeks)

| Week | Features | Exit Criteria |
|------|----------|---------------|
| 1 | v12-I01 (ndp-intelligence library crate), v12-S01 (pgvector install), v12-S02 (embedding pipeline) | Embedding produced from Gold aligned row, stored in pgvector |
| 2 | v12-I02 (ndp-intelligence-app binary), v12-I06 (PG NOTIFY), v12-I07 (Docker service) | Standalone intelligence binary running on Pi, wakes on CA refresh |
| 3 | v12-S03 (HNSW bootstrap), v12-S04 (K-NN search), v12-S05 (prediction gen) | K-NN search returns neighbors, predictions generated |
| 4 | v12-S06 (prediction storage), v12-S07 (outcome tracking), v12-S08 (anomaly detection) | Predictions tracked against actuals; anomalies flagged |
| 5 | v12-S09 (dashboard), v12-I05 (CLI integration) | Dashboard shows predictions + anomalies; CLI operational |

### Phase 1B: pgvector-Only Fallback (3-4 weeks)

If ruvector-core doesn't compile on aarch64:

| Week | Features | Exit Criteria |
|------|----------|---------------|
| 1 | v12-I01 (library crate, no ruvector dep), v12-S01 (pgvector), v12-S02 (embeddings) | Embeddings in pgvector |
| 2 | v12-I02 (binary), K-NN via pgvector SQL (`ORDER BY embedding <=> $1 LIMIT 20`) | Predictions from SQL-only path |
| 3 | v12-I07 (Docker service), outcome tracking, anomaly detection | Standalone binary operational |
| 4 | Dashboard, CLI | Same as Phase 1A but ~1ms slower per search |

Phase 1B delivers 80% of the intelligence value. The pgvector SQL search is ~1.5ms vs ruvector's ~0.1ms — irrelevant for a 15-minute cycle. The main loss is no SONA/EWC++ in V1.3 (would need alternative implementation). The standalone binary architecture is identical — only the vector search backend differs.

### Phase 2: Statistical Validation (2-3 weeks, parallel or after Phase 1)

| Week | Features | Exit Criteria |
|------|----------|---------------|
| 1 | v12-G01 (candidate generation from K-NN), v12-G02 (Granger scanner) | Granger tests run on top candidates |
| 2 | v12-G03 (lag optimizer), v12-G04 (candidate registry), v12-G05 (ranker) | Validated relationships with metadata |
| 3 | v12-G06 (evidence accumulator), v12-G07 (dashboard) | Dashboard shows causal relationships |

### Phase 3: SONA Adaptive Learning (3-4 weeks, V1.3)

| Week | Features | Exit Criteria |
|------|----------|---------------|
| 1 | v13-001 (SONA init), v13-002 (trajectory recording) | SONA trajectories recording from predictions |
| 2 | v13-003 (micro-LoRA), v13-004 (ReasoningBank) | Embedding adaptation operational |
| 3 | v13-005 (causal validation), v13-007 (action framework) | PC algorithm validates candidates; actions defined |
| 4 | v13-008 (Q-Learning), v13-009 (feedback loop), v13-011 (dashboard) | Graduated autonomy operational |

Go/no-go: SONA vs ARIMA benchmark at end of week 2.

### Phase 4: Attention & Causal Structure (2-3 weeks, V1.3+)

| Week | Features | Exit Criteria |
|------|----------|---------------|
| 1 | v13-006 (DiffusionAttention), v13-010 (EWC++ seasonal) | Causal attention identifies feature importance |
| 2 | Integration, seasonal validation | System maintains accuracy through weather change |

---

## Risk Assessment (Updated)

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| ruvector-core doesn't compile on aarch64 | Medium | Medium | Phase 0 go/no-go. Phase 1B pgvector-only fallback delivers 80% value |
| ruvector API changes (3,648 downloads, experimental) | High | Medium | Pin exact version, vendor if needed, abstract behind IntelligenceProvider trait |
| K-NN predictions are too noisy (insufficient similar past states) | Medium | Medium | Require minimum K neighbors with min_similarity threshold. Fall back to "no prediction" |
| Granger finds no significant relationships | Low | Medium | Existing streams (window→CO2) have known relationships. If Granger fails, the relationship truly isn't there |
| SONA doesn't outperform K-NN baseline | Medium | Low | K-NN is already good. SONA is additive. If SONA underperforms, keep K-NN as primary |
| pgvector + ruvector-core sync complexity | Medium | Medium | pgvector is system of record. ruvector-core is rebuilt from pgvector on restart. One-way flow |
| Embedding dimensionality wrong (too few/many features) | Medium | Low | Config-driven dimensions. Start with 32D, adjust based on cluster analysis |
| Pi build time too long (ruvector-core compilation) | Medium | Low | Build on Pi, expect 30-60 min first time. Subsequent builds incremental |

---

## Decision Log

| # | Decision | Rationale |
|---|----------|-----------|
| 1 | Add similarity path alongside statistical path | K-NN provides predictions from day 30 with zero model training. Statistical path alone requires 30+ days before any insight, and can't predict — only identify relationships |
| 2 | pgvector for durable storage, ruvector-core for compute | pgvector is battle-tested, arm64 available, SQL-native. ruvector-core provides sub-ms in-process search, quantization, and SONA (which pgvector cannot). They complement, not compete |
| 3 | Intelligence as standalone binary, not embedded in air-quality-app | Workload profiles differ (I/O-bound ingestion vs compute-bound intelligence). Independent deployment — intelligence updates don't restart ingestion. Fault isolation — intelligence crash doesn't lose data. TimescaleDB is the integration point, not shared memory |
| 4 | SONA replaces model tournament (V1.3) | 90% memory reduction (50MB vs 500MB+). EWC++ included for free. Microsecond adapter switching. Eliminates 3-4 weeks of manual EWC++ implementation |
| 5 | Numerical embeddings, not text/LLM embeddings | NDP data is structured sensor readings. Text embeddings add 200MB+ for no benefit. Numerical vectors are 32D, f32, trivial on Pi |
| 6 | Phase 0 go/no-go gate before any ruvector work | 3,648 downloads, no visible arm64 CI. 30-minute test de-risks entire integration. If it fails, Phase 1B (pgvector-only) still delivers |
| 7 | Retain Granger causality (don't replace with ruvector) | Similarity says "these things happened together." Granger says "this caused that." Both are needed. Statistical rigor is not optional for causal claims |
| 8 | Config-driven embedding pipeline (follows V1.0/V1.1 pattern) | Which fields go into the embedding = JSON config. Add a stream to intelligence = edit config. No code changes. Consistent with platform philosophy |
| 9 | ruvector-core compiles into ndp-intelligence-app, not air-quality-app | ruvector's memory (HNSW, SONA) lives in the intelligence container's budget. Ingestion binary stays lean and stable. Intelligence container has its own 256MB limit |
| 10 | PG NOTIFY preferred over timer polling | Intelligence wakes only when Gold CA has fresh data. No wasted cycles. Degrades gracefully to timer if NOTIFY unavailable |

---

## Appendix A: Crate Dependencies

**Library crate** (`crates/ndp-intelligence/Cargo.toml`):

```toml
[dependencies]
ruvector-core = { version = "2.0.1" }  # HNSW, VectorDB, redb, SimSIMD

# For V1.3 (add when needed):
# ruvector-sona = { version = "0.1" }     # SONA, LoRA, EWC++, ReasoningBank
# ruvector-attention = { version = "0.1", features = ["simd"] }  # Causal attention

# Standard NDP deps
tokio = { version = "1", features = ["full"] }
tokio-postgres = "0.7"
ndp-types = { path = "../ndp-types" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
tracing = "0.1"
ndarray = "0.16"  # For Granger OLS regression
```

**Binary** (`apps/ndp-intelligence-app/Cargo.toml`):

```toml
[dependencies]
ndp-intelligence = { path = "../../crates/ndp-intelligence" }
tokio = { version = "1", features = ["full"] }
clap = { version = "4", features = ["derive"] }
tracing = "0.1"
tracing-subscriber = "0.3"
```

**Dockerfile** (`deploy/pi/Dockerfile.intelligence`):

```dockerfile
FROM rust:1-bookworm AS builder
RUN apt-get update && apt-get install -y build-essential  # SimSIMD needs gcc
WORKDIR /build
COPY . .
RUN cargo build --release --bin ndp-intelligence-app

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y libssl3 ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/ndp-intelligence-app /usr/local/bin/
ENTRYPOINT ["ndp-intelligence-app"]
CMD ["--daemon"]
```

## Appendix B: Embedding Vector Dimensions (Reference)

| Category | Fields | Dimensions |
|----------|--------|-----------|
| Temporal | hour_sin, hour_cos, is_weekend | 3 |
| Indoor Air | co2, pm25, temp, humidity, voc | 5 |
| Outdoor Weather | temp, humidity, wind_speed, pressure | 4 |
| Outdoor AQI | pm25, aqi | 2 |
| State Events | window_transitions, door_transitions, window_state | 3 |
| Derived | co2_trend_4h, pm25_trend_4h, co2_std_4h, pm25_std_4h, co2_diff_1h, pm25_diff_1h | 6 |
| Forecast Context | forecast_temp, forecast_precip, forecast_wind | 3 |
| **Total** | | **~26-32** |

All fields z-score normalized using a 30-day rolling window. Stored as f32 (no quantization needed at NDP scale — 1 year of hourly vectors = 1.2MB at 32D f32).

## Appendix C: Research References

| Document | Content |
|----------|---------|
| `product/research/ruvector/00-SYNTHESIS.md` | 5-agent research synthesis, Pi 5 budget, phased recommendations |
| `product/research/ruvector/01-architecture-fit.md` | Layer mapping, module analysis, integration patterns |
| `product/research/ruvector/02-event-intelligence.md` | Event embedding, pattern detection, trigger intelligence |
| `product/research/ruvector/03-edge-feasibility.md` | Memory budgets, deployment options (revised for 16GB Pi) |
| `product/research/ruvector/04-learning-acceleration.md` | SONA vs EWC++, time-to-intelligence acceleration |
| `product/research/ruvector/05-creative-use-cases.md` | 10 creative applications ranked by value x feasibility |
| `product/research/ruvector/06-pi5-compilation-feasibility.md` | Compilation analysis, arm64 deps, go/no-go test |
| `product/research/gold/ruvector-analysis/` | Earlier ruvector deep dive and ruv-FANN assessment |

---

*Document created using backwards design from V2.0 vision, augmented with ruvector intelligence layer.*
*Each feature exists because the next version requires it.*
*The similarity path accelerates the statistical path. The statistical path validates the similarity path.*
