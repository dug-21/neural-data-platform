# RuVector Architecture Fit Analysis for NDP

**Research Date:** 2026-02-10
**Revised:** 2026-02-10 (actual Pi 5 specs: 16GB RAM, 1TB NVMe SSD)
**Author:** Research Agent
**Version:** 1.1
**Status:** Complete
**Prior Art:** `product/research/13-ruvector-centralized-service-analysis.md` (agent/memory focus)

> **REVISION NOTE (16GB + NVMe):** This document was originally written assuming a 4GB Pi 5 with ~2.2GB headroom. Actual production hardware is **16GB RAM + 1TB NVMe SSD** with ~15.3GB available. Key impacts:
> - **Pattern C (Full ruvector sidecar)** verdict changes from "Poor Fit" to **VIABLE** (~1GB, well within budget)
> - **GNN module** verdict changes from "Defer to cloud/NAS" to **VIABLE on Pi** (~320MB)
> - **SONA (LoRA + EWC++)** verdict changes from "Marginal" to **VIABLE** (~200MB)
> - **PQ compression** changes from "Must use" to "Optional" — f32 is fine for years at NDP's data volume
> - **Embedding generation (all-MiniLM-L6-v2)** changes from "Too large" to "Available if wanted" (~200MB)
> - The recommended integration path can be more aggressive — full container from Phase 1 is feasible
> - See `00-SYNTHESIS.md` for revised deployment verdicts and memory budget

---

## Executive Summary

This document analyzes how ruvector (a self-learning distributed vector database) fits into the Neural Data Platform's Bronze/Silver/Gold data lake architecture running on a Raspberry Pi 5. The analysis maps ruvector capabilities to NDP layers, evaluates integration patterns, and assesses resource feasibility.

**Bottom Line:** ruvector's most valuable integration point is as the engine behind NDP's v1.2 Discovery and v1.3 Intelligence roadmap milestones. All deployment options — PG extension, rvLite, and full ruvector container — are viable on the 16GB Pi 5. The choice is driven by architectural preference and complexity management, not resource constraints.

---

## Table of Contents

1. [NDP Architecture Summary](#1-ndp-architecture-summary)
2. [Where Does RuVector Fit in Bronze/Silver/Gold?](#2-where-does-ruvector-fit-in-bronzesilvergold)
3. [Integration Patterns](#3-integration-patterns)
4. [Which RuVector Module Is Most Relevant?](#4-which-ruvector-module-is-most-relevant)
5. [Data Flow: Time-Series to Vector Space](#5-data-flow-time-series-to-vector-space)
6. [Compatibility Concerns](#6-compatibility-concerns)
7. [Assessment Summary](#7-assessment-summary)

---

## 1. NDP Architecture Summary

### Current State (v1.1.21)

```
Sources (MQTT, HTTP Poll)
    |
    v
[Bronze] Parquet + WAL          -- Raw JSON, schema-on-read
    |
    v (Silver ETL daemon)
[Silver] TimescaleDB hypertables -- Typed columns, DQ rules, continuous aggregates
    |
    v (ndp-gold-ddl)
[Gold]   Materialized views      -- Aligned cross-stream, features, events
    |
    v (PLANNED: v1.2-v1.3)
[Intelligence] ???               -- Correlation discovery, causal validation, prediction
```

### Resource Budget (Pi 5, 4GB allocated to Docker)

| Service | Memory Limit | Current |
|---------|-------------|---------|
| mosquitto | 128MB | 50MB |
| etcd | 256MB | 100MB |
| air-quality-app | 512MB | 200MB |
| timescaledb | 256MB | ~200MB |
| ndp-mcp-server | 96MB | ~50MB |
| grafana | 256MB | ~150MB |
| silver-etl-daemon | 256MB | ~200MB |
| **Total allocated** | **~1.8GB** | **~950MB** |
| **Available headroom** | **~2.2GB** | |

### Roadmap Context

The NDP roadmap (product/vision/ROADMAP-TO-V2.md) defines:
- **v1.1**: Gold layer -- feature computation, stream classification (in progress)
- **v1.2**: Discovery -- automatic correlation detection, transition tracking
- **v1.3**: Intelligence -- causal validation, predictions, model selection, actions
- **v2.0**: Cross-domain intelligence -- multi-domain correlation discovery

RuVector's value proposition maps directly to v1.2 and v1.3.

---

## 2. Where Does RuVector Fit in Bronze/Silver/Gold?

### Layer-by-Layer Analysis

#### Bronze Layer: No Fit

**Assessment: Poor Fit**

Bronze stores raw JSON payloads via append-only Parquet writes. It is a write-optimized, schema-on-read archive. ruvector adds nothing here -- Bronze does not need similarity search, indexing, or embeddings. Data arrives as structured sensor readings, not unstructured text.

#### Silver Layer: Limited Fit (DQ Anomaly Detection)

**Assessment: Moderate Fit**

Silver handles typed columns, range validation, and deduplication via TimescaleDB hypertables. ruvector could potentially enhance Silver through anomaly detection -- embedding time windows and flagging observations that are semantically dissimilar to recent history. However, the existing DQ rules (range checks, null handling, clamping) are sufficient for current needs and are config-driven. Adding vector-based anomaly detection adds complexity without clear benefit at this stage.

Potential future use: Embed Silver records as feature vectors for novelty detection. Flag readings that fall within range but represent unusual combinations (e.g., high CO2 + low humidity + nighttime = anomalous).

#### Gold Layer: Strong Fit (Feature Store + Correlation Index)

**Assessment: Strong Fit**

Gold is where ruvector becomes highly relevant. The Gold layer already computes:
- Continuous aggregates (time-bucketed features)
- Cross-stream aligned views (indoor vs outdoor)
- State transitions and events
- Classification labels

RuVector capabilities mapped to Gold needs:

| ruvector Capability | Gold Layer Use Case | Value |
|--------------------|--------------------|-------|
| HNSW vector index | Feature similarity search -- find similar time windows | High |
| GNN graph module | Causal relationship graph between streams/events | High |
| Hyperbolic embeddings | Hierarchical sensor-event relationships | Medium |
| PQ compression (PQ4/PQ8) | Fit large feature stores in Pi memory | High |
| Q-Learning hooks | Learn which correlations lead to successful predictions | High |
| ReasoningBank | Store and replay successful prediction trajectories | Medium |

#### New "Intelligence" Layer: Best Fit

**Assessment: Strong Fit**

Rather than embedding ruvector into an existing layer, the strongest pattern is a new layer above Gold:

```
[Gold]   Materialized features, aligned streams, events, transitions
    |
    v
[Intelligence]  ruvector-powered
    - Correlation index (embed Gold features, find similar patterns)
    - Causal graph (GNN edges between events with time lags)
    - Prediction store (embed state snapshots, nearest-neighbor forecasting)
    - Action scoring (Q-learning from outcome tracking)
```

This maps directly to the v1.2 Discovery and v1.3 Intelligence milestones:

| Roadmap Milestone | ruvector Component | How |
|-------------------|--------------------|-----|
| v1.2 Correlation discovery | HNSW + embedding | Embed Gold features, similarity search finds co-occurring patterns |
| v1.2 Transition tracking | GNN module | Graph edges between state transitions with temporal weights |
| v1.3 Causal validation | GNN + Q-Learning | Score causal hypotheses via interventional analysis |
| v1.3 Model selection | ReasoningBank | Store model performance trajectories, replay best |
| v1.3 Action scoring | Q-Learning hooks | RL-based action selection from outcome history |
| v2.0 Cross-domain | Hyperbolic embeddings | Hierarchical domain representations in Poincare space |

---

## 3. Integration Patterns

### Pattern A: rvLite Embedded Library (Recommended)

```
[Gold views in TimescaleDB]
        |
        | SQL query
        v
[Intelligence Service]  (Rust binary, new NDP crate)
   |-- rvLite (2MB, embedded in process)
   |-- Reads Gold features via tokio-postgres
   |-- Writes correlation index to rvLite
   |-- Exposes results via NDP MCP tools
        |
        v
[Grafana / MCP / Actions]
```

**Pros:**
- 2MB binary, minimal memory footprint
- No additional Docker container needed
- Rust-to-Rust, no serialization overhead
- Runs in-process with the intelligence service
- Works offline (no network dependency)

**Cons:**
- Subset of full ruvector capabilities
- No GNN module (rvLite is vector-only)
- No built-in embedding generation (need external or custom)

**Resource Cost:** ~50-100MB additional memory for index + service

**Assessment: Strong Fit** for v1.2 correlation discovery

### Pattern B: PostgreSQL Extension Inside TimescaleDB

```
[Silver/Gold tables in TimescaleDB]
        |
        | SQL with vector functions
        v
[TimescaleDB + ruvector pgvector extension]
   |-- 77+ SQL functions
   |-- Vector columns alongside time-series columns
   |-- pgvector-compatible API
        |
        v
[Grafana / MCP / SQL clients]
```

**Pros:**
- Zero additional containers -- lives inside existing TimescaleDB
- SQL-native interface (SELECT * FROM gold.features ORDER BY embedding <-> query_vector)
- Leverages existing connection pools (bb8 in silver-etl)
- pgvector compatibility means ecosystem tooling works
- Co-located with Gold features -- no data movement

**Cons:**
- TimescaleDB is already at 256MB limit on Pi
- Extension adds memory pressure to PostgreSQL
- Extension installation requires custom Docker image (FROM timescale/timescaledb + install extension)
- Limited to SQL interface -- no GNN, no RL hooks

**Resource Cost:** ~50-100MB additional memory within TimescaleDB container (would need to increase limit to 384-512MB)

**Assessment: Strong Fit** if TimescaleDB memory can be increased

### Pattern C: Full ruvector Server Sidecar

```
[Gold views in TimescaleDB]
        |
        v
[ruvector server]  (separate Docker container)
   |-- HTTP :8080 / gRPC :50051
   |-- HNSW index
   |-- GNN module
   |-- Semantic router
   |-- Embedding generation
        |
        v
[NDP services connect via HTTP/gRPC]
```

**Pros:**
- Full feature set (GNN, RL, embeddings, semantic routing)
- Independent scaling and lifecycle
- Already documented in research/13-ruvector-centralized-service-analysis.md

**Cons:**
- Memory hungry: benchmarks show 2.3GB for HNSW index alone (1M vectors)
- Embedding model (all-minilm-l6-v2) adds ~500MB
- Exceeds Pi resource budget
- Additional container complexity
- Network latency between services

**Resource Cost:** 512MB minimum (tiny index), 2-4GB realistic

**Assessment: Poor Fit** for Pi deployment. Viable only if running on a more powerful host.

### Pattern D: Hybrid (rvLite + Periodic Cloud Sync)

```
[Pi: rvLite embedded]
   |-- Local correlation index
   |-- Recent 30 days of feature vectors
   |-- PQ4 compressed (32x reduction)
        |
        | Periodic sync (daily)
        v
[Cloud/NAS: Full ruvector server]
   |-- Complete history
   |-- GNN module for deep causal analysis
   |-- Model training with full dataset
```

**Pros:**
- Best of both worlds -- local speed, full features remotely
- Pi stays within budget
- PQ4 compression makes 100K+ vectors fit in ~50MB

**Cons:**
- Requires additional infrastructure
- Sync complexity
- Offline-only mode degrades to rvLite capabilities

**Assessment: Moderate Fit** -- adds operational complexity but enables full ruvector features

---

## 4. Which RuVector Module Is Most Relevant?

### Decision Matrix for Pi Deployment

| Module | Memory | Pi Viable? | NDP Value | Verdict |
|--------|--------|------------|-----------|---------|
| **rvLite** (2MB edge DB) | 50-100MB | Yes | High -- vector index for features | **Use this** |
| **PostgreSQL extension** | +50-100MB in TimescaleDB | Yes (with limit increase) | High -- SQL-native, co-located | **Strong alternative** |
| **Full server** | 512MB-4GB | No | Highest features but too heavy | Skip for Pi |
| **HNSW index** (standalone) | Proportional to vectors | Yes if compressed | Core of correlation search | Part of rvLite |
| **GNN module** | ~320MB (500K edges) | Marginal | High for causal graphs | Defer to cloud/NAS |
| **SONA (LoRA + EWC++)** | ~200MB+ | Marginal | High for continuous learning | Defer to v1.3 |
| **ReasoningBank** | ~50MB | Yes | Medium for trajectory replay | Include in v1.3 |
| **Q-Learning hooks** | ~10MB | Yes | High for action scoring | Include in v1.3 |
| **Embedding generation** | ~500MB (model) | No (too large) | High but use pre-computed | Pre-compute in ETL |
| **40 attention variants** | Variable | No (most too heavy) | Research only | Skip |
| **Raft consensus** | N/A | N/A | N/A (single node) | Not needed |
| **Cypher graph queries** | ~50MB | Yes | Medium for relationship queries | Nice-to-have |
| **PQ compression** | Saves memory | Yes (essential) | Critical for Pi | **Must use** |

### Recommendation

**Phase 1 (v1.2):** rvLite embedded in a new `ndp-intelligence` crate, with PQ4/PQ8 compression. Pre-compute embeddings in the Silver ETL or Gold DDL pipeline. Store feature vectors in rvLite for similarity search.

**Phase 2 (v1.3):** Add Q-Learning hooks and ReasoningBank for action scoring. Consider PostgreSQL extension if SQL-native access proves more ergonomic.

**Phase 3 (v2.0):** If multi-domain requires GNN/SONA, deploy full ruvector on a companion device or NAS.

---

## 5. Data Flow: Time-Series to Vector Space

### What Gets Embedded?

Not raw sensor readings. Not individual data points. The correct unit of embedding for time-series correlation discovery is the **feature window** -- a fixed-duration summary of multi-stream state.

### Embedding Pipeline

```
1. Gold continuous aggregates produce 10-minute feature windows:

   [time_bucket] [indoor_temp] [indoor_co2] [indoor_pm25] [outdoor_temp] [wind_speed] [window_state]
   10:00         22.1          680          8.2            15.3           12.4         0 (closed)
   10:10         22.3          720          8.5            15.1           11.8         0
   10:20         21.8          650          7.1            15.0           13.2         1 (opened)
   10:30         21.2          580          6.8            14.8           14.1         1

2. Feature extraction produces fixed-dimension vectors per window:

   window_10:00 = [22.1, 680, 8.2, 15.3, 12.4, 0, delta_co2=+40, delta_temp=+0.2, trend_pm25=rising, ...]
   window_10:10 = [22.3, 720, 8.5, 15.1, 11.8, 0, delta_co2=+40, delta_temp=+0.2, trend_pm25=rising, ...]
   window_10:20 = [21.8, 650, 7.1, 15.0, 13.2, 1, delta_co2=-70, delta_temp=-0.5, trend_pm25=falling, ...]

3. Vectors stored in rvLite with metadata:

   rvlite.insert(
     id: "2026-02-10T10:20",
     vector: [22.1, 680, 8.2, ...],  // normalized feature vector
     metadata: {
       timestamp: "2026-02-10T10:20:00Z",
       events: ["window_opened"],
       labels: { co2_direction: "falling", temp_direction: "falling" }
     }
   )

4. Correlation discovery via similarity search:

   Query: "Find windows similar to current state (high CO2, rising trend)"
   Result: 47 similar windows. In 38 of them, window was opened within 30 min.
           In 35 of those 38, CO2 dropped below 600 within 20 min.

   Discovered correlation: window_open -> CO2_drop (lag: 20min, confidence: 0.92)
```

### Feature Vector Dimensions

Based on NDP's current streams (3 active: air-quality, outdoor-weather, outdoor-air-quality):

| Component | Dimensions | Source |
|-----------|-----------|--------|
| Indoor air values | 8 | pm25, co2, tvoc, nox, temp, humidity, pm10, pm25_compensated |
| Indoor derivatives | 8 | delta and trend for each value |
| Outdoor weather | 6 | temp, humidity, pressure, wind_speed, wind_deg, clouds |
| Outdoor weather derivatives | 6 | delta and trend |
| Outdoor AQI | 5 | aqi, pm25, pm10, co, no2 |
| Temporal features | 4 | hour_of_day, day_of_week, is_weekend, season |
| State features | 2 | window_state (future), hvac_state (future) |
| **Total** | **~39** | |

With PQ8 compression: 39 dimensions x 1 byte = 39 bytes per vector.
30 days of 10-minute windows = 4,320 vectors = ~170KB.
1 year = ~52,560 vectors = ~2MB.

This fits trivially in rvLite on Pi.

### Embedding Strategy

For NDP's numerical time-series data, traditional NLP-style embeddings (all-minilm-l6-v2, etc.) are **not appropriate**. The data is already numerical. The correct approach:

1. **Direct numerical vectors** -- normalize each feature to [0, 1] range using min-max from Silver schema ranges (already defined in config YAML)
2. **No LLM embedding model needed** -- saves 500MB of model weight
3. **Cosine similarity** on normalized feature vectors finds similar system states
4. **Optional: PCA/autoencoder** to reduce dimensionality if feature count grows beyond ~100

This is a critical distinction from the prior ruvector research (research/13) which assumed text embeddings. For NDP, embeddings are numerical feature vectors, not semantic text representations.

---

## 6. Compatibility Concerns

### Rust-to-Rust Integration

**Status: Strong Compatibility**

NDP is entirely Rust. ruvector is Rust. Key compatibility points:

| Aspect | NDP | ruvector | Compatible? |
|--------|-----|----------|-------------|
| Language | Rust | Rust | Yes |
| Async runtime | tokio 1.40 | tokio (same) | Yes |
| Serialization | serde 1.0, serde_json | serde (same) | Yes |
| Error handling | thiserror 1.0 | thiserror | Yes |
| Arrow/Parquet | arrow 57, parquet 57 | N/A (vector storage) | N/A |
| PostgreSQL | tokio-postgres 0.7 | pgvector extension | Compatible |

rvLite as a library dependency would add to the `core` or a new `intelligence` crate's Cargo.toml. No FFI, no C bindings, pure Rust.

### Docker Resource Constraints

**Current allocation: ~1.8GB of ~4GB budget**

| Integration Pattern | Additional Memory | Feasible? |
|--------------------|-------------------|-----------|
| rvLite in existing service | +50MB | Yes -- plenty of headroom |
| rvLite in new container | +100MB (process overhead) | Yes |
| PG extension in TimescaleDB | +100MB (increase limit to 384MB) | Yes, tight |
| Full ruvector server | +512MB minimum | Marginal -- would need to steal from other services |
| Full ruvector + GNN + embeddings | +2GB+ | No |

### TimescaleDB Coexistence

**No conflict.** ruvector does not replace TimescaleDB -- it complements it:

- TimescaleDB stores time-indexed observations (Silver) and features (Gold)
- ruvector stores feature vectors indexed by similarity (Intelligence)
- TimescaleDB answers "what happened at time T?"
- ruvector answers "when did something similar to this happen before?"

If using the PostgreSQL extension approach, ruvector lives inside TimescaleDB as an extension alongside `timescaledb` extension. Both are PostgreSQL extensions and can coexist in the same database.

### MCP Integration

NDP already has an MCP server (`core/ndp-mcp-server`) exposing 15 tools for Bronze, Silver, and Dictionary access. Adding ruvector-powered Intelligence tools follows the same pattern:

```
Existing MCP tools:
  - list_streams, describe_schema, sample_data, validate_config (Bronze)
  - list_silver_tables, describe_silver_table, sample_silver_data (Silver)
  - query_dictionary, describe_column, trace_lineage (Dictionary)

New Intelligence MCP tools (ruvector-powered):
  - find_similar_windows(feature_vector, top_k)
  - list_correlations(stream_id, min_confidence)
  - predict_state(current_features, horizon_minutes)
  - explain_correlation(correlation_id)  -- GNN path query
  - score_action(action_type, current_state)  -- Q-learning
```

---

## 7. Assessment Summary

### Per Integration Pattern

| Pattern | Layer Fit | Pi Feasibility | Implementation Effort | Assessment |
|---------|-----------|----------------|----------------------|------------|
| rvLite embedded in new crate | Intelligence (above Gold) | **Strong** -- 50MB | 2-3 weeks | **Strong Fit** |
| PG extension in TimescaleDB | Gold/Intelligence | **Strong** -- +100MB | 1-2 weeks | **Strong Fit** |
| Full ruvector sidecar | Intelligence | **Poor** -- 2GB+ | 1 week (deploy) | **Poor Fit** on Pi |
| Hybrid rvLite + remote server | Intelligence | **Strong** locally | 3-4 weeks | **Moderate Fit** |
| Replace parts of pipeline | N/A | N/A | N/A | **Not Recommended** |
| Query accelerator for TimescaleDB | Silver/Gold | Moderate | 2 weeks | **Moderate Fit** |

### Per RuVector Capability

| Capability | NDP Relevance | Pi Viable? | Roadmap Alignment | Assessment |
|-----------|---------------|------------|-------------------|------------|
| HNSW indexing (61us p50) | Core of correlation search | Yes (rvLite) | v1.2 | **Strong Fit** |
| GNN self-improving index | Causal relationship graphs | No (too heavy) | v1.3 (defer) | **Needs Investigation** |
| SONA (LoRA + EWC++) | Continuous model improvement | No (too heavy) | v1.3 (defer) | **Needs Investigation** |
| ReasoningBank | Trajectory learning for predictions | Yes (~50MB) | v1.3 | **Moderate Fit** |
| Q-Learning hooks | Action scoring from outcomes | Yes (~10MB) | v1.3 | **Strong Fit** |
| Hyperbolic embeddings | Hierarchical domain modeling | Yes (math only) | v2.0 | **Moderate Fit** |
| rvLite (2MB) | Edge vector DB | **Yes** | v1.2 | **Strong Fit** |
| PostgreSQL extension | SQL-native vector search | **Yes** | v1.2 | **Strong Fit** |
| Multi-tier compression | Fit vectors in Pi memory | **Essential** | v1.2 | **Strong Fit** |
| Local LLM runtime | Text understanding | No (500MB+ model) | Not needed | **Poor Fit** |
| 40 attention variants | Research only | No | Not applicable | **Poor Fit** |
| Raft consensus | Distributed coordination | No (single node) | Not applicable | **Poor Fit** |
| Cypher graph queries | Relationship navigation | Yes (~50MB) | v1.3 | **Moderate Fit** |
| MCP Server | AI tool calling | Yes (NDP has MCP) | v1.2 | **Strong Fit** |

### Recommended Integration Path

```
v1.2 (Discovery):
  - Add rvLite as dependency to new `crates/ndp-intelligence` crate
  - Pre-compute normalized feature vectors in Gold ETL pipeline
  - Store in rvLite with PQ8 compression
  - Implement find_similar_windows() for correlation discovery
  - Expose via NDP MCP server as new Intelligence tools
  - Memory cost: ~50-100MB additional

v1.3 (Intelligence):
  - Add Q-Learning hooks for action scoring
  - Add ReasoningBank for trajectory storage
  - Evaluate PostgreSQL extension for SQL-native access
  - Consider GNN if memory budget allows (unlikely on Pi)
  - Memory cost: ~100-150MB additional

v2.0 (Cross-Domain):
  - Evaluate hyperbolic embeddings for domain hierarchy
  - Consider companion device for full ruvector if GNN needed
  - Cross-domain correlation via shared rvLite vector space
```

### Key Insight

RuVector's primary value to NDP is NOT as a replacement for any existing component. It is the **missing engine** for the v1.2 Discovery and v1.3 Intelligence milestones. The existing data pipeline (Bronze/Silver/Gold) produces the features; ruvector indexes them for similarity-based discovery. This is a complementary, additive integration -- not a substitution.

The critical constraint is memory. On Pi, only rvLite (embedded) or the PostgreSQL extension are viable. The full ruvector server with GNN, SONA, and embedding generation exceeds the Pi's budget by a factor of 3-5x.

---

## Appendix A: File References

| File | Relevance |
|------|-----------|
| `/workspaces/neural-data-platform/core/src/traits.rs` | Core traits (Source, Store, Forecast) that Intelligence layer would extend |
| `/workspaces/neural-data-platform/core/src/forecast/fann_adapter.rs` | Existing ML forecaster (mock) -- ruvector would complement, not replace |
| `/workspaces/neural-data-platform/core/src/forecast/features.rs` | Feature engineering -- vectors to embed in rvLite |
| `/workspaces/neural-data-platform/core/src/silver/outputs/timescale.rs` | TimescaleDB output -- pattern for Intelligence output |
| `/workspaces/neural-data-platform/core/src/event_bus/mod.rs` | EventBus -- could feed Intelligence layer in real-time |
| `/workspaces/neural-data-platform/core/src/processors/threshold.rs` | ThresholdProcessor -- Intelligence would generate smarter thresholds |
| `/workspaces/neural-data-platform/deploy/pi/docker-compose.yml` | Resource constraints and container topology |
| `/workspaces/neural-data-platform/docs/architecture/CONFIG_DRIVEN_SILVER_ETL_DESIGN.md` | Config-driven ETL pattern to replicate for Intelligence |
| `/workspaces/neural-data-platform/product/vision/ROADMAP-TO-V2.md` | v1.2/v1.3 milestones that ruvector enables |
| `/workspaces/neural-data-platform/product/vision/EDGE-INTELLIGENCE-PLATFORM.md` | Vision: discovery, validation, prediction, action |
| `/workspaces/neural-data-platform/product/research/13-ruvector-centralized-service-analysis.md` | Prior ruvector research (agent/memory focus) |

## Appendix B: Memory Budget Projection

### v1.2 with rvLite

| Service | Current | After rvLite | Delta |
|---------|---------|-------------|-------|
| mosquitto | 128MB | 128MB | 0 |
| etcd | 256MB | 256MB | 0 |
| air-quality-app | 512MB | 512MB | 0 |
| timescaledb | 256MB | 256MB | 0 |
| ndp-mcp-server | 96MB | 96MB | 0 |
| grafana | 256MB | 256MB | 0 |
| silver-etl-daemon | 256MB | 256MB | 0 |
| **ndp-intelligence** | **0** | **128MB** | **+128MB** |
| **Total** | **1,760MB** | **1,888MB** | **+128MB** |

Headroom remaining: ~2.1GB on 4GB budget. Feasible.

### v1.3 with Q-Learning + ReasoningBank

| Service | After v1.2 | After v1.3 | Delta |
|---------|-----------|-----------|-------|
| ndp-intelligence | 128MB | 256MB | +128MB |
| **Total** | **1,888MB** | **2,016MB** | **+128MB** |

Headroom remaining: ~2.0GB. Still feasible.

---

*This analysis is based on architecture review of the NDP codebase at v1.1.21, ruvector capabilities as documented, and the NDP product roadmap. Actual memory usage should be validated with a proof of concept.*
