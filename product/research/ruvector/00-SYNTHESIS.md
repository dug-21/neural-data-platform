# RuVector Intelligence Layer: Research Synthesis

**Date:** 2026-02-10
**Revised:** 2026-02-10 (updated with actual Pi 5 specs: 16GB RAM, 1TB NVMe SSD)
**Method:** 5-agent research swarm (architecture, events, edge feasibility, learning, creative)
**Verdict:** Strong candidate for integration. Full ruvector stack viable on Pi. Phased path identified.

---

## The One-Sentence Summary

RuVector can add a **full-featured** self-improving intelligence layer to NDP — including GNN causal graphs, SONA model adaptation, and ReasoningBank — that delivers V1.3-like prediction capability (K-NN from similar past contexts) with **zero model training**, runs comfortably on the Pi 5's **16GB RAM + NVMe SSD**, and accelerates the V1.2-V2.0 timeline by an estimated **30-40%**.

---

## Actual Hardware Profile

> **Revision note:** The initial research assumed a 4GB Pi 5 with microSD. Actual production hardware is significantly more capable. All recommendations revised accordingly.

### Measured Resource Usage (Production Pi 5)

```
Pi 5: 16GB RAM, 1TB NVMe SSD, Quad-core ARM Cortex-A76 @ 2.4GHz
```

| Service | Memory Used | Memory Limit | CPU |
|---------|-----------|-------------|-----|
| air-quality-app | 123 MB | 512 MB | 0.09% |
| ndp-mcp-server | 19 MB | 96 MB | 3.54% |
| timescaledb | 308 MB | *uncapped* | 0.01% |
| grafana | 98 MB | *uncapped* | 0.22% |
| etcd | 89 MB | *uncapped* | 0.30% |
| mqtt-broker | 8 MB | *uncapped* | 0.03% |
| **Total used** | **~646 MB** | | **~4.2%** |
| **Available** | **~15.3 GB** | | **~96%** |

**Storage:** 1TB NVMe SSD — ~100K random IOPS, mmap-viable for HNSW indices.

### What This Changes

The 4GB constraint was the primary reason most ruvector capabilities were deferred. At 16GB + NVMe, **every ruvector module is viable from day one.** The phasing becomes about *complexity management*, not resource constraints.

| Capability | 4GB Verdict | 16GB + NVMe Verdict | Change |
|-----------|------------|-------------------|--------|
| PG extension in TimescaleDB | Recommended | Still good starting point | — |
| rvLite embedded | Viable | Viable | — |
| **Full ruvector container** | NOT VIABLE | **VIABLE (~1GB)** | Unlocked |
| **GNN module** | Defer to companion | **VIABLE (~320MB)** | Unlocked |
| **SONA full stack** | Too heavy | **VIABLE (~200MB)** | Unlocked |
| **ReasoningBank** | Marginal | **Comfortable (~50MB)** | Upgraded |
| f32 vectors (no compression) | Need PQ8 | **f32 fine for years** | Simplified |
| mmap HNSW from disk | Requires NVMe | **NVMe confirmed** | Confirmed |
| all-MiniLM-L6-v2 (text embed) | Unnecessary | **Available if wanted (~200MB)** | Optional |
| GGUF local LLM (TinyLlama 1.1B) | NOT VIABLE | **Marginal (~700MB)** | Possible |

---

## Consensus Findings Across All 5 Research Threads

### 1. There IS clear value (not a solution looking for a problem)

Every research thread independently identified the same core insight: **NDP's Gold layer produces ML-ready features, but has no mechanism for "this situation resembles a past situation."** RuVector fills that gap.

The highest-value capability is **K-NN predictive triggers**: embed current sensor state as a vector, search for similar past states, look up what happened next. "In 17 of 20 similar past situations, CO2 exceeded 800 within 30 minutes." This provides prediction without model training.

### 2. All deployment options are viable on 16GB Pi 5

With 15.3GB available, the deployment choice is driven by architectural preference, not constraints:

| Option | Memory | Verdict |
|--------|--------|---------|
| **PG extension in TimescaleDB** | +50-80MB | **Simplest — co-located with Gold data** |
| **rvLite embedded in Rust binary** | +50-100MB | **Lightest — best for hot-path queries** |
| **Full ruvector container** | +512MB-1GB | **Most capable — full GNN, SONA, graph queries** |
| **Hybrid (rvLite + full container)** | +600MB-1.1GB | **Best of both — hot-path local + full features** |

**Recommended:** Start with PG extension (simplest, zero new containers). Evaluate standalone container if GNN or SONA proves valuable in Phase 2.

### 3. Embed numerical feature vectors, not text

All threads agree: NDP's data is structured sensor readings. No LLM embedding model needed. Build 28-39 dimension vectors directly from Gold layer aggregates.

**With 16GB, compression is optional.** f32 vectors (full precision, zero recall loss) are affordable:

| Duration | Vectors (hourly) | f32 Size | PQ8 Size |
|----------|-----------------|---------|---------|
| 1 month | 720 | 100 KB | 17 KB |
| 1 year | 8,760 | 1.2 MB | 200 KB |
| 5 years | 43,800 | 6 MB | 1 MB |
| 10 years + all raw (1M) | 1,000,000 | 169 MB | 24 MB |

Even at 1M vectors with f32 (no compression), the index is 169MB — ~1% of available RAM. **Skip compression complexity until it's needed (likely never for NDP's data volume).**

### 4. Complements, does not replace, the planned roadmap

| Planned Approach | RuVector Addition | Relationship |
|-----------------|-------------------|-------------|
| Granger causality (V1.2) | Embedding pre-filter + validation | **Complement** |
| Static candidate registry | GNN causal graph (living, self-pruning) | **Upgrade** |
| Model tournament (V1.3) | SONA (one base model + LoRA adapters) | **90% memory reduction** |
| Manual EWC++ implementation | SONA includes EWC++ for free | **Simplification** |
| Manual graduated autonomy | Q-Learning advisory layer | **Complement** |
| Not planned: explainability | ReasoningBank decision trajectories | **Gap fill** |
| Not planned: seasonal adaptation | SONA/EWC++ seasonal memory | **Gap fill** |

### 5. Sensor Fingerprinting is the foundation for everything

The creative use cases thread identified that **all 10 ideas share sensor fingerprinting as a prerequisite**. The HNSW index + embedding pipeline is a one-time infrastructure investment that enables: anomaly detection, experience replay, predictive triggers, self-monitoring, dream mode, and eventually federated learning.

### 6. GNN causal graph can start early (16GB revision)

With memory unconstrained, the **living causal knowledge graph** can begin accumulating evidence from Phase 1 rather than being deferred to Phase 3. Starting the graph early means it has months of reinforcement data by the time V1.3 needs validated causal relationships. This is a learning-rate acceleration: the graph gets smarter passively while other features are being built.

---

## Top 5 Candidates (Ranked by Value x Feasibility)

### #1: K-NN Predictive Triggers (Event Intelligence + Learning)

**What:** Embed hourly sensor state, search for similar past states, predict outcomes from historical neighbors.

**Why it's #1:** Delivers V1.3-like prediction with zero model training. Works from day 30 (with enough hourly data). Accuracy improves automatically as data accumulates. Interpretable: "based on 17 similar past situations."

**Resource cost:** ~6MB index (first year, f32), ~40us per search.
**Implementation:** 2-3 weeks alongside V1.2.
**Risk:** Low (additive, does not modify existing pipeline).

### #2: Sensor Fingerprinting (Creative)

**What:** Embed full sensor state as single vector. Auto-discover situations (morning, cooking, sleeping). Build a library of "known states."

**Why it's #2:** Foundation for #1, #3, #4, #5, and nearly every future intelligence capability. Simple to implement (z-score normalization). Validates that situations naturally cluster in vector space.

**Resource cost:** ~160MB/year storage at f32 (no compression needed).
**Implementation:** 1-2 weeks.
**Risk:** Very low.

### #3: SONA for Model Tournament (Learning Acceleration)

**What:** Replace separate full models per relationship with one shared base model + tiny LoRA adapters (2KB each). EWC++ comes built-in.

**Why it's #3:** 90%+ memory reduction for neural models (though less critical at 16GB). Microsecond model switching. Eliminates separate EWC++ implementation effort. Estimated 3-4 week acceleration on V1.3 timeline.

**Resource cost:** 50MB base model + negligible adapters (trivial at 16GB).
**Implementation:** V1.3 phase, 2-week integration.
**Risk:** Medium (depends on ruvector SONA maturity for time-series).
**Go/no-go test:** 2-week minimum viable test — SONA vs ARIMA on CO2 prediction.

### #4: Embedding-Distance Anomaly Detection (Creative + Events)

**What:** "Normal" = what the system has seen before. "Anomaly" = embedding far from all clusters. Self-calibrating via SONA/EWC++ across seasons.

**Why it's #4:** Simpler than Isolation Forest. Self-calibrating. Catches multivariate anomalies that single-metric z-scores miss (e.g., CO2 normal but humidity+CO2+temp combination is unusual). Reuses fingerprinting infrastructure.

**Resource cost:** Near-zero incremental (uses fingerprint index).
**Implementation:** 1 week after fingerprinting exists.
**Risk:** Very low.

### #5: Living Causal Knowledge Graph (Creative — promoted from deferred)

> **16GB revision:** Previously ranked lower due to GNN memory concerns. Now viable from Phase 1.

**What:** Store every discovered correlation as a graph edge in ruvector's GNN. Edges strengthen with evidence, decay without it. Self-pruning. Queryable via Cypher: "What causes PM2.5 spikes in this house?"

**Why it's #5 (promoted):** At 16GB, the GNN module (~320MB) fits easily. Starting the graph early means months of passive learning before V1.3 needs validated causal relationships. This is the knowledge artifact that makes NDP truly intelligent over time.

**Resource cost:** ~320MB for GNN module + graph.
**Implementation:** Seed with Granger results in V1.2, continuous reinforcement thereafter.
**Risk:** Medium (GNN training stability on small graphs needs validation).

**Previously #5 (Experience Replay) moves to #6** — still high value, implementation unchanged.

---

## What NOT to Do

1. ~~**Do not run full ruvector server on Pi.**~~ **REVISED: Full ruvector container is now viable.** Allocate 1GB, plenty of headroom.
2. **Do not use LLM-based text embeddings for sensor data.** NDP data is numerical. Text embeddings are available if needed for future NL query features, but not for core intelligence.
3. ~~**Do not replace Granger with GNN.**~~ **REVISED: GNN can start alongside Granger from Phase 1.** Don't replace Granger (still needed for cold-start discovery), but do build the causal graph concurrently.
4. **Do not implement all 40 attention mechanisms.** Most solve problems NDP doesn't have. Temporal and cross-stream attention are worth exploring in Phase 3.
5. **Do not skip the statistical foundation.** Granger/ARIMA are battle-tested and work from day 1. RuVector accelerates but does not replace them.
6. **Do not over-compress vectors.** At 16GB, f32 (full precision) is fine for years. Skip PQ8/PQ4 complexity.

---

## Recommended Integration Timeline (Revised for 16GB)

```
Phase 1: Foundation (with V1.2 work, 2-3 weeks)
  - Deploy ruvector PG extension in TimescaleDB (simplest start)
  - Build numerical feature vector pipeline from Gold aggregates (f32, no compression)
  - Sensor fingerprinting: embed hourly aligned view rows
  - Embedding-distance anomaly detection
  - K-NN similarity search: "find hours like this one"
  - Platform self-monitoring (reuse fingerprint infra)
  - Seed causal knowledge graph with initial stream relationships

  Decision point: If PG extension proves limiting, deploy full ruvector container

Phase 2: Intelligence (with V1.3 work, 3-4 weeks)
  - K-NN predictive triggers: predict outcomes from similar contexts
  - SONA model tournament (go/no-go after 2-week test)
  - ReasoningBank episode recording
  - Q-Learning autonomy advisory (6MB, constrained by user ceiling)
  - GNN weekly training on causal graph (accumulated evidence from Phase 1)
  - Experience Replay: record situation-action-outcome episodes

Phase 3: Adaptation (V1.3+, ongoing)
  - SONA/EWC++ seasonal adaptation
  - Dream mode offline consolidation
  - Temporal attention for multi-scale pattern learning
  - Natural language → sensor query (if MCP + embedding alignment mature)

Phase 4: Scale (V2.0+)
  - Cross-domain transfer via hyperbolic embeddings
  - Federated learning across Pi fleet (binary vector sync)
  - Full ruvector container if not already deployed
```

---

## Pi Memory Budget (Revised for 16GB)

| Phase | New Memory | Cumulative Used | Available (16GB) | % Used |
|-------|-----------|----------------|-----------------|--------|
| Current (v1.1.21) | — | 646 MB | 15,354 MB | 4.0% |
| Phase 1 (PG ext + f32 index) | +100 MB | 746 MB | 15,254 MB | 4.6% |
| Phase 1 alt (full container) | +1,000 MB | 1,646 MB | 14,354 MB | 10.1% |
| Phase 2 (SONA + GNN + Q-Learn + RB) | +650 MB | 2,296 MB | 13,704 MB | 14.1% |
| Phase 3 (Dream + Attention + NL) | +300 MB | 2,596 MB | 13,404 MB | 16.0% |
| Phase 4 (Full stack, all features) | +500 MB | 3,096 MB | 12,904 MB | 19.1% |

**At full build-out, NDP + ruvector uses ~19% of available RAM.** The 16GB Pi has more than 5x the headroom needed for the complete intelligence stack.

### NVMe SSD Impact

| Use Case | microSD (old assumption) | NVMe SSD (actual) |
|----------|------------------------|-------------------|
| HNSW mmap query latency | 10-100ms (unusable) | 1-5ms (excellent) |
| Random IOPS for graph traversal | ~4,000 (slow) | ~100,000 (fast) |
| Large index support | Must fit in RAM | Can exceed RAM via mmap |
| Bronze Parquet I/O | Bottleneck | Non-issue |
| Dream mode disk operations | Slow batch | Fast batch |

The NVMe SSD means HNSW indices can grow without concern — mmap provides near-RAM performance for the access patterns vector search requires.

---

## Decision: Proceed or Not?

**Recommendation: Proceed with Phase 1. Resource constraints are a non-issue.**

The minimum viable integration (PG extension + numerical embeddings + K-NN search) costs:
- 2-3 weeks of implementation
- ~100MB of Pi memory (0.6% of available)
- Zero changes to existing pipeline

The maximum integration (full ruvector container + all modules) costs:
- ~3GB of Pi memory (19% of available)
- Still leaves 13GB headroom

**The decision is purely about complexity management and development bandwidth, not hardware limitations.** Start simple, validate the thesis (do sensor fingerprints cluster meaningfully?), then expand.

If Phase 1 shows that sensor fingerprints produce meaningful clusters and K-NN retrieval finds genuinely similar past states, the entire ruvector integration thesis is validated. The 16GB Pi can support the full intelligence stack whenever the software is ready for it.

---

## Research Artifacts

| File | Content | Revision Status |
|------|---------|----------------|
| `00-SYNTHESIS.md` | This document — unified findings and recommendations | **Revised for 16GB + NVMe** |
| `01-architecture-fit.md` | Layer mapping, integration patterns, module analysis | Revision note added |
| `02-event-intelligence.md` | Event embedding, pattern detection, trigger intelligence | Revision note added |
| `03-edge-feasibility.md` | Memory budgets, ARM compilation, deployment options | **Fully revised for 16GB + NVMe** |
| `04-learning-acceleration.md` | SONA vs EWC++, ReasoningBank vs Granger, time-to-intelligence | Revision note added |
| `05-creative-use-cases.md` | 10 creative applications ranked by value x feasibility | Revision note added |

---

*Research conducted by 5-agent swarm. Synthesis by coordinator.*
*Revised with actual Pi 5 hardware specs (16GB RAM, 1TB NVMe SSD).*
*All findings are theoretical analysis — implementation requires proof-of-concept validation.*
