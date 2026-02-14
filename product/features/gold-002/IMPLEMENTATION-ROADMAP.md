# gold-002: V1.2 Implementation Roadmap

> **Architecture:** `product/features/gold-002/ARCHITECTURE.md`
> **Parent roadmap:** `product/features/gold-001/FEATURE-ROADMAPv1.2.md`
> **Created:** 2026-02-13
> **Status:** Draft for Review

---

## Phasing Strategy

V1.2 is split into **5 implementation phases**, each delivering discrete, testable functionality. Phases are designed so that:

1. Each phase produces a deployable artifact or measurable capability
2. Earlier phases de-risk later ones
3. The system is useful after Phase 2 (not waiting until Phase 5)
4. Phase 2 is the first "value delivery" — predictions from day 30+

### Phase Summary

| Phase | Name | Delivers | Prerequisite |
|-------|------|----------|-------------|
| **0** | Go/No-Go Gate | Compilation proof on aarch64 | None |
| **1** | Foundation | Crates, pgvector schema, Embedder trait, MetricEmbedder, storage | Phase 0 |
| **2** | Similarity Intelligence | HNSW search, predictions, outcome tracking, daemon | Phase 1 |
| **3** | Statistical Validation | Granger causality, candidate registry, evidence accumulation | Phase 2 |
| **4** | Event Intelligence | EventEmbedder, CompositeEmbedder, NWS text dogfooding | Phase 2 |
| **5** | Polish & Dashboards | Grafana panels, anomaly detection, CLI commands, production hardening | Phase 3 + 4 |

Phases 3 and 4 can overlap — they are independent tracks with a shared dependency on Phase 2.

---

## Phase 0: Go/No-Go Gate

**Goal:** Prove ruvector-core compiles and runs on aarch64 (Pi 5).

**Duration:** 1 day

### Deliverables

| ID | Task | Exit Criterion |
|----|------|---------------|
| P0-01 | Create minimal Rust project with `ruvector-core = "2.0.1"` + `ruvector-graph = "0.1"` | Compiles with `cargo build --release` |
| P0-02 | Run on Pi 5 (native) or cross-compile for aarch64 | Binary executes without crash |
| P0-03 | Smoke test ruvector-core: insert 100 vectors, search, verify results | Correct K-NN results returned |
| P0-04 | Smoke test ruvector-graph: add nodes + edges, traverse neighbors | Correct traversal results |
| P0-05 | Measure: memory usage, search latency, build time | Documented in go/no-go report |

### Decision Gate

| Component | Outcome | Action |
|-----------|---------|--------|
| ruvector-core | Compiles and works | Primary HNSW backend |
| ruvector-core | SimSIMD fails, scalar fallback works | Proceed with `default-features = false, features = ["storage", "hnsw", "parallel"]` |
| ruvector-core | Fails entirely | pgvector-only mode (slower search, functionally equivalent) |
| ruvector-graph | Compiles and works | Primary graph backend |
| ruvector-graph | Fails | SQL adjacency tables (`gold.graph_nodes`, `gold.graph_edges`) |

### Artifact

`product/features/gold-002/reports/phase0-go-no-go.md` — compilation results, timing, memory, decision.

---

## Phase 1: Foundation

**Goal:** Establish the crate structure, database schema, Embedder trait, and MetricEmbedder. No runtime intelligence yet — this phase builds the infrastructure that all subsequent phases depend on.

**Duration:** 2-3 weeks

### Deliverables

| ID | Task | Description | Exit Criterion |
|----|------|-------------|---------------|
| **P1-01** | Create `crates/ndp-intelligence` crate | Cargo.toml, lib.rs, module structure, workspace member | Compiles, `cargo test` passes |
| **P1-02** | Create `apps/ndp-intelligence-app` crate | Cargo.toml, main.rs stub with clap CLI, workspace member | Compiles, `--help` works |
| **P1-03** | Embedder trait + GoldRow types | `ndp-lib::gold::embeddings::mod.rs` — trait, GoldRow, Embedding | Unit tests pass |
| **P1-04** | MetricEmbedder implementation | Z-score normalization, temporal encoding, NULL handling | Unit tests with known inputs/outputs |
| **P1-05** | RunningStats for z-score | Exponential decay mean/std tracker | Statistical accuracy tests |
| **P1-06** | EmbeddingConfig types | Config structs for embedding, deserialization from domain JSON | Deserialize tests with sample config |
| **P1-07** | DomainConfig `intelligence` extension | Add optional `intelligence: Option<IntelligenceConfig>` to DomainConfig | Existing tests still pass, new field deserializes |
| **P1-08** | pgvector schema DDL generator | `PgVectorSchemaGenerator` in ndp-lib — generates tables for metric_embeddings, predictions, graph (SQL fallback), reasoning_bank (V1.3 prep, empty) | DDL generation tests |
| **P1-09** | pgvector extension in TimescaleDB Docker | Add `postgresql-15-pgvector` to Docker image, init script | Extension loads: `SELECT * FROM pg_extension WHERE extname = 'vector'` |
| **P1-10** | StorageBackend trait + PostgresStorage | Trait definition + pgvector INSERT/SELECT for embeddings and predictions | Integration test against TimescaleDB |
| **P1-11** | GraphStore trait + backend | `ndp-intelligence::graph` — trait + ruvector-graph or SQL adjacency backend (per Phase 0 outcome) | Node/edge CRUD + traversal tests |
| **P1-12** | EmbeddingWriter populator | `ndp-lib::gold::populator::embedding_writer` — writes Embedding to PostgresStorage | Integration test: write + read round-trip |
| **P1-13** | ndp-cli `gold intelligence` subcommand | `ndp gold intelligence schema` — generates pgvector + graph DDL | CLI outputs valid SQL |

### Architecture Notes

- P1-03 through P1-05 are pure Rust with no database dependencies — highly testable
- P1-07 must NOT break existing DomainConfig deserialization (intelligence field is `Option`)
- P1-08 follows the exact same pattern as existing `ContinuousAggregateGenerator`
- P1-10 requires the integration environment (TimescaleDB running)
- P1-11 builds the graph capability — the generic infrastructure. Domain-specific node/edge types are shaped in Phase 4 when event data exists

### Config Change

`config/domains/indoor-air-quality/domain.json` gets a new `intelligence` block (see ARCHITECTURE.md section 6). Existing fields unchanged.

### Release

Phase 1 does NOT get a Pi deployment. It's library-only. Tests run in the integration environment.

---

## Phase 2: Similarity Intelligence

**Goal:** End-to-end intelligence cycle running as a daemon. After a warmup period (~1 week of data), the system generates predictions based on K-NN similarity search. This is the first phase that delivers user-visible value.

**Duration:** 3-4 weeks

### Deliverables

| ID | Task | Description | Exit Criterion |
|----|------|-------------|---------------|
| **P2-01** | SimilarityEngine trait | `ndp-intelligence::similarity::mod.rs` — trait definition | Compiles |
| **P2-02** | HnswEngine (ruvector-core) | Wrapper around ruvector-core VectorDB | Insert + search integration test |
| **P2-03** | PgVectorEngine (fallback) | pgvector SQL-based K-NN search | Insert + search integration test |
| **P2-04** | HNSW index rebuild from pgvector | On startup, load all embeddings from pgvector into HNSW | Rebuild test: pgvector → HNSW → search matches |
| **P2-05** | PredictionEngine | K-NN results → outcome lookup → prediction generation | Unit test with mock neighbors |
| **P2-06** | Confidence scoring | Prediction confidence = supporting_neighbors / total_neighbors | Unit test |
| **P2-07** | Prediction storage | Write predictions to `gold.predictions` | Integration test |
| **P2-08** | Outcome tracker | Evaluate predictions after horizon elapsed, update correctness | Integration test with seeded data |
| **P2-09** | IntelligenceService orchestrator | Coordinates: observe → embed → store → search → predict → evaluate | Integration test: full cycle |
| **P2-10** | PG NOTIFY listener | `LISTEN gold_refresh` with reconnection logic | Integration test |
| **P2-11** | Timer fallback | If no NOTIFY within 20 min, poll on timer | Unit test for timer logic |
| **P2-12** | Daemon mode | `ndp-intelligence-app daemon` — continuous loop with NOTIFY/timer | Runs for 10 min without crash |
| **P2-13** | One-shot mode | `ndp-intelligence-app one-shot` — single cycle then exit | Runs, produces predictions, exits 0 |
| **P2-14** | Backfill mode | `ndp-intelligence-app backfill --since <date>` — process historical data | Processes N hours, generates embeddings |
| **P2-15** | Warmup logic | Skip predictions during warmup (first 168 hours), only collect stats | Embeddings stored but no predictions during warmup |
| **P2-16** | Docker container | `docker/intelligence/Dockerfile` + docker-compose service | Container builds and starts |
| **P2-17** | deploy.sh integration | Add ndp-intelligence to deployment script | Deploys alongside existing services |

### Architecture Notes

- P2-02/P2-03 implement the same SimilarityEngine trait — runtime selection based on Phase 0 outcome
- P2-04 is critical for crash recovery: HNSW is ephemeral, pgvector is durable
- P2-09 is the integration point where all pieces come together
- P2-15 means the system is safe to deploy immediately — it won't make bad predictions while warming up

### Acceptance Criteria

| Criterion | Target |
|-----------|--------|
| Embeddings generated for all aligned view hours | `SELECT count(*) FROM gold.metric_embeddings` matches aligned view count |
| Predictions generated after warmup | At least 1 prediction per hour after 168-hour warmup |
| Search latency | <1ms p99 via HNSW; <10ms p99 via pgvector |
| Full cycle latency | <500ms total (observe + embed + store + search + predict + evaluate) |
| Memory usage (daemon) | <100 MB actual, within 256 MB limit |
| Prediction accuracy baseline | Logged (no target yet — baseline establishment) |

### Release

**v1.2.0** — First intelligence release. Deploy to Pi. Begins warmup period.

---

## Phase 3: Statistical Validation (Granger)

**Goal:** Add Granger causality analysis to statistically validate relationships discovered by K-NN similarity search. This provides a "second opinion" — K-NN finds correlations, Granger tests whether they're causal.

**Duration:** 2-3 weeks. Can overlap with Phase 4.

### Deliverables

| ID | Task | Description | Exit Criterion |
|----|------|-------------|---------------|
| **P3-01** | Granger causality F-test | Pure Rust implementation using ndarray | Unit test with known synthetic causal series |
| **P3-02** | Lag optimizer | Test multiple lag values, find optimal | Unit test: finds correct lag for synthetic data |
| **P3-03** | Similarity-guided candidate selection | Use K-NN prediction patterns to identify top field pairs | Unit test with mock prediction data |
| **P3-04** | GrangerScanner | Orchestrates: candidates → test → validate → store as graph edges | Integration test |
| **P3-05** | Graph causal storage | Store Granger results as graph edges (edge_type = 'causes') via GraphStore | Integration test |
| **P3-06** | Evidence accumulator | Increment evidence_count on re-validation, status transitions | Unit test for state machine |
| **P3-07** | Candidate ranker | Rank by strength x relevance to objectives | Unit test |
| **P3-08** | Granger cycle integration | Add Granger scan to intelligence cycle (runs less frequently — daily) | Full cycle with Granger enabled |
| **P3-09** | `ndp intelligence candidates` CLI | List validated causal candidates | CLI outputs table |

### Architecture Notes

- Granger runs daily (not every 15 min) — it needs at least a week of data per test
- P3-03 is the key insight: instead of O(n^2) field pairs, test only pairs that K-NN already found correlated
- P3-01 is a self-contained statistical function — straightforward to unit test
- P3-06 implements a simple state machine: candidate → confirmed → stable → degraded

### Acceptance Criteria

| Criterion | Target |
|-----------|--------|
| Granger detects known relationships | Test with synthetic data where causality is designed-in |
| At least 3 significant relationships found | After 30+ days of data |
| False positive rate | <20% (verified by domain knowledge) |
| Computation time | <30s for full candidate scan |

### Release

**v1.2.1** — Granger validation. Deploy to Pi as update.

---

## Phase 4: Event Intelligence (Text Dogfooding)

**Goal:** Prove the text embedding pipeline works using NWS forecast text as the dogfood event stream. This validates EventEmbedder, template caching, CompositeEmbedder, and prepares the architecture for V1.3's sysops domain. This is also where the graph domain model is shaped — when both metric and event data exist, define the specific node types and edge types for cross-type linking.

**Duration:** 3-4 weeks. Can overlap with Phase 3.

### Deliverables

| ID | Task | Description | Exit Criterion |
|----|------|-------------|---------------|
| **P4-01** | EventEmbedder trait implementation | MiniLM ONNX inference → Vec<f32> [384D] | Unit test with sample text |
| **P4-02** | MiniLM ONNX model integration | `ort` crate + HuggingFace tokenizer, on-demand loading | Model loads, produces 384D vector |
| **P4-03** | Template cache | In-memory template → embedding cache with similarity threshold | Cache hit/miss unit tests |
| **P4-04** | NWS forecast event stream config | Stream config for `nws-forecast-hourly` `detailedForecast` text field | Config loads, validates |
| **P4-05** | Event embeddings table DDL | `gold.event_embeddings` hypertable with vector(384) column | DDL generates, table creates |
| **P4-06** | Event embedding storage | Store per-event embeddings with template_hash, severity | Integration test |
| **P4-07** | CompositeEmbedder | Combines MetricEmbedder output + EventEmbedder centroid (PCA-reduced) | Unit test: correct dimensions |
| **P4-08** | PCA reduction | Reduce 384D event centroid to ~16D for composite | Unit test with known data |
| **P4-09** | Composite search | K-NN on composite embeddings (metrics + text context) | Integration test: returns results |
| **P4-10** | Forecast-aware predictions | Predictions using composite embeddings distinguish stagnation vs front | Qualitative evaluation |
| **P4-11** | Quantization recall validation | Compare ruvector-core PQ8 search results against exact pgvector search | PQ8 recall >95% of f32 baseline |
| **P4-12** | Tiered retention implementation | Hot/warm/cold lifecycle job for event embeddings | Job runs: hot events age correctly |
| **P4-13** | Container memory update | Increase limit to 512 MB for MiniLM | Container runs with MiniLM loaded |
| **P4-14** | Graph domain model | Define node types (metric_state, event, prediction) and edge types (causes, correlates_with, precedes) for cross-type linking. Informed by real data from Phases 2-3 | Model documented, graph populated with cross-type edges |
| **P4-15** | Forecast validation report | Compare forecast text predictions with actual sensor outcomes | Report generated |

### Architecture Notes

- P4-01 through P4-03 are the core text pipeline — test independently before integration
- P4-02 adds ~200 MB to memory when MiniLM is loaded; it's loaded on demand and can be unloaded
- P4-07 requires both MetricEmbedder and EventEmbedder to produce vectors for the same time bucket
- P4-08 uses a simple PCA implementation (ndarray + SVD) — not a machine learning framework
- P4-11 validates ruvector-core's built-in PQ quantization (we do NOT implement custom PQ) — proves recall at V1.3 sysops scale
- P4-12 is exercised at air quality scale (unnecessary but validates the machinery)

### Acceptance Criteria

| Criterion | Target |
|-----------|--------|
| EventEmbedder produces vectors | NWS forecast texts embedded, stored in pgvector |
| Template cache hit rate | >90% after warmup (NWS forecasts are repetitive) |
| CompositeEmbedder works | Composite search returns results |
| Composite vs metric-only | Composite predictions >= metric-only (A/B comparison) |
| PQ8 recall | >95% vs f32 baseline |
| Tiered retention | Hot → warm → cold lifecycle runs without error |
| MiniLM startup time | <30s on Pi |
| Container memory | <400 MB with MiniLM loaded |

### Release

**v1.2.2** — Event intelligence. Deploy to Pi as update.

---

## Phase 5: Polish & Dashboards

**Goal:** Grafana visualization, anomaly detection, CLI completeness, and production hardening. This phase turns the intelligence layer from "it works" into "it's useful daily."

**Duration:** 2-3 weeks

### Deliverables

| ID | Task | Description | Exit Criterion |
|----|------|-------------|---------------|
| **P5-01** | Anomaly detector | Distance-based anomaly flagging in embedding space | Unit test with outlier injection |
| **P5-02** | Anomaly integration | Flag anomalous hours in predictions + embeddings | Anomalies appear in database |
| **P5-03** | Intelligence Overview dashboard | Grafana: prediction accuracy, confidence, index stats | Dashboard renders with data |
| **P5-04** | Causal Relationships dashboard | Grafana: validated relationships table, evidence timeline | Dashboard renders with data |
| **P5-05** | Anomaly Timeline panel | Grafana: annotation overlay on existing dashboards | Anomalies visible on air quality dashboard |
| **P5-06** | `ndp intelligence status` CLI | Current state: last run, prediction accuracy, index size | CLI outputs formatted status |
| **P5-07** | `ndp intelligence search` CLI | Interactive K-NN search for debugging | CLI returns neighbor list |
| **P5-08** | `ndp intelligence run` CLI | Alias for one-shot mode | CLI runs single cycle |
| **P5-09** | Config-driven stream addition test | Add outdoor-air-quality to intelligence config, verify predictions | New stream embedded via config only |
| **P5-10** | Health endpoint | HTTP endpoint for docker healthcheck | `curl localhost:8081/health` returns 200 |
| **P5-11** | Metric export | Prometheus-compatible metrics (cycle count, latency, accuracy) | Metrics scraped by Grafana |

### Acceptance Criteria

| Criterion | Target |
|-----------|--------|
| Anomaly detection operational | Flags >80% of threshold crossings |
| Dashboards render | All panels show data, no errors |
| Config-driven addition | New stream via config change only, no code changes |
| Health check works | Docker `healthcheck` passes |
| 30-day prediction accuracy | >60% for 1-hour horizon (metric-only) |

### Release

**v1.2.3** — Final V1.2 release. Full intelligence foundation deployed.

---

## Cross-Phase Dependencies

```
Phase 0 ─────> Phase 1 ─────> Phase 2 ──┬──> Phase 3 ──┐
  (1 day)      (2-3 wk)      (3-4 wk)   │   (2-3 wk)   ├──> Phase 5
                                          │               │   (2-3 wk)
                                          └──> Phase 4 ──┘
                                              (3-4 wk)
```

- **Phase 0 → 1:** Compilation gate determines HNSW backend choice AND graph backend choice
- **Phase 1 → 2:** Foundation types and storage must exist before runtime
- **Phase 2 → 3:** Granger needs prediction data to guide candidate selection
- **Phase 2 → 4:** Event pipeline extends the embedding infrastructure from Phase 2
- **Phase 3 + 4 → 5:** Dashboards visualize data from both Granger and event intelligence

### Overlap Opportunity

Phases 3 and 4 can be worked in parallel since they share Phase 2 as a common dependency but have no direct dependencies on each other. Phase 3 adds Granger validation to metric predictions. Phase 4 adds a new embedding type (text). Both extend Phase 2 independently.

---

## Version Mapping

| Phase | Version | Semver Rationale |
|-------|---------|-----------------|
| Phase 0 | (no release) | Go/no-go report only |
| Phase 1 | (no release) | Library-only, no Pi deployment |
| Phase 2 | v1.2.0 | First intelligence capability — MINOR bump |
| Phase 3 | v1.2.1 | Adds Granger — PATCH (additive feature) |
| Phase 4 | v1.2.2 | Adds event intelligence — PATCH (additive feature) |
| Phase 5 | v1.2.3 | Dashboards + polish — PATCH |

---

## ruvector Overlap Decisions

Based on analysis of ruvector ecosystem capabilities vs our custom architecture:

| Component | Decision | Rationale |
|-----------|----------|-----------|
| **Quantization (PQ8, scalar, binary)** | Delegate to ruvector-core | Built-in, ARM NEON accelerated, saves ~1 week of custom implementation |
| **ONNX embeddings** | Keep custom via `ort` crate | ruvector-onnx-embeddings is ARM64-problematic; `ort` with MiniLM is proven on ARM |
| **Graph capability** | ruvector-graph (preferred) or SQL adjacency (fallback) | Built in Phase 1 as generic infrastructure. Domain model shaped in Phase 4 when event data exists |
| **ReasoningBank schema** | Create empty table in V1.2 | Avoids V1.3 schema migration when SONA is integrated |
| **SONA (LoRA + EWC++)** | Use ruvector's integrated SONA in V1.3 | Eliminates need for separate ruv-fann dependency entirely |
| **ruv-fann** | NOT needed | V1.2 uses statistical methods (Granger) + non-parametric (K-NN). V1.3 uses ruvector SONA instead |

---

## Risk Register

| Risk | Phase | Likelihood | Impact | Mitigation |
|------|-------|-----------|--------|-----------|
| ruvector-core won't compile on aarch64 | 0 | Medium | Medium | pgvector-only fallback, functionally equivalent |
| ruvector-graph won't compile on aarch64 | 0 | Medium | Low | SQL adjacency tables provide same GraphStore interface |
| ruvector API changes between versions | 1-2 | Medium | Medium | Pin exact version, wrap behind SimilarityEngine trait |
| K-NN predictions too noisy (low accuracy) | 2 | Medium | Medium | Minimum similarity threshold, warmup period, confidence scoring |
| MiniLM ONNX doesn't run on aarch64 | 4 | Low | Medium | `ort` crate supports ARM; fallback to SimHash for approximate text embedding |
| Granger finds no significant relationships | 3 | Low | Low | Expected with limited data; evidence accumulator handles long-term convergence |
| pgvector index too slow for interactive search | 2 | Low | Low | HNSW in-process handles hot path; pgvector is durable backup |
| Memory pressure from MiniLM + HNSW | 4 | Low | Medium | On-demand loading; unload MiniLM between cycles |
| NWS forecast text too repetitive for meaningful clustering | 4 | Medium | Low | Template dedup handles this; proves pipeline even if text diversity is low |

---

## Estimation Summary

| Phase | Duration | Cumulative |
|-------|----------|-----------|
| Phase 0 | 1 day | 1 day |
| Phase 1 | 2-3 weeks | 3-4 weeks |
| Phase 2 | 3-4 weeks | 6-8 weeks |
| Phase 3 | 2-3 weeks | (overlaps Phase 4) |
| Phase 4 | 3-4 weeks | (overlaps Phase 3) |
| Phase 5 | 2-3 weeks | 13-18 weeks total |

With Phase 3/4 overlap: **~13-15 weeks** for full V1.2.

First predictions (Phase 2 deployed + warmup): **~8-9 weeks** from start.

---

## SPARC Planning Approach

Each phase will get its own SPARC planning cycle before implementation begins:

1. **SCOPE.md** — Phase scope, constraints, acceptance criteria (human-authored)
2. **IMPLEMENTATION-BRIEF.md** — Planning swarm output: decomposed tasks, dependencies, test strategy
3. Implementation via agent swarm, following the brief
4. `/validate` before release
5. `/align` to check against vision criteria
6. Release per RELEASE-POLICY.md

We start with Phase 0 immediately (it's a 1-day spike), then SPARC-plan Phase 1.

---

*Implementation roadmap for gold-002 V1.2 Intelligence Foundation. Each phase delivers discrete, testable functionality. Review this document alongside ARCHITECTURE.md for the full picture.*
