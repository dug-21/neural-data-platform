# fe-004 IMPLEMENTATION-BRIEF: Similarity Intelligence (V1.2 Phase 2)

> **Version target**: v1.2.0 (first intelligence release, deploys to Pi)
> **GitHub Issue**: https://github.com/dug-21/neural-data-platform/issues/18
> **Date**: 2026-02-15

## SPARC Planning Artifacts

| Artifact | Path |
|----------|------|
| Scope | `product/features/fe-004/SCOPE.md` |
| Specification | `product/features/fe-004/specification/SPECIFICATION.md` |
| Task Decomposition | `product/features/fe-004/specification/TASK-DECOMPOSITION.md` |
| Architecture (ADRs) | `product/features/fe-004/architecture/ARCHITECTURE.md` |
| Pseudocode | `product/features/fe-004/pseudocode/PSEUDOCODE.md` |
| Alignment Report | `product/features/fe-004/ALIGNMENT-REPORT.md` |
| Acceptance Map | `product/features/fe-004/ACCEPTANCE-MAP.md` |
| Launch Prompt | `product/features/fe-004/LAUNCH-PROMPT.md` |
| Parent Roadmap | `product/features/gold-001/FEATURE-ROADMAPv1.2.md` |

---

## 1. Goal

Deliver the first end-to-end intelligence cycle as a running daemon on the Raspberry Pi. Implement SimilarityEngine backends (HNSW via ruvector-core + pgvector SQL fallback), PredictionEngine (K-NN neighbor outcome lookup with confidence scoring), IntelligenceService orchestrator (observe-embed-store-search-predict-evaluate), PG NOTIFY/timer wake mechanism, daemon/one-shot/backfill modes, Docker container, and deploy.sh integration. After a 168-hour warmup period, the system generates hourly predictions answering: "Given conditions like now, what happened next in the past?" This is Phase 2 (metric similarity only) -- the first phase delivering user-visible intelligence value from the fe-003 library foundation.

---

## 2. Resolved Decisions

| Decision | Resolution | Source | Pattern ID |
|----------|-----------|--------|-----------|
| SimilarityEngine sync vs async | Sync trait; PgVectorEngine uses block_on internally | ADR-008 | 24 |
| Dual-write pattern | Writes go to both pgvector (durable via StorageBackend) and HNSW (fast via SimilarityEngine); reads prefer HNSW | ADR-001 | 17 |
| PgVectorEngine insert | No-op; embeddings already written via StorageBackend::store_embedding | ADR-001, ADR-014 | 17, 44 |
| Wake mechanism | Timer (20 min) is primary; PG NOTIFY is optimization (CAs cannot have triggers) | ADR-004 | -- |
| Outcome lookup | SQL query per neighbor per horizon (memory-efficient) | ADR-002 | 18 |
| Warmup gate | observation_count >= 168 (not time-based; works for backfill) | ADR-003 | 19 |
| Observation count persistence | Query gold.metric_embeddings count on startup; replay Gold rows for running stats | ADR-013 | 43 |
| Backfill predictions | Backfill mode never generates predictions (embed-only) | Pseudocode sec 11 | -- |
| Container memory | 256 MB limit (metric-only, ~34 MB estimated actual) | ADR-006 | 22 |
| Connection pooling | deadpool-postgres, pool size 2, separate NOTIFY connection | ADR-009 | 39 |
| Gold row query | Dynamic SQL from config field list; view name from domain_id | ADR-010 | 40 |
| Error propagation | Fatal vs recoverable; startup fatal, cycle recoverable | ADR-011 | 41 |
| Config layers | Domain config from etcd via config-client (IntelligenceConfig + objectives) + Runtime config (AppConfig from env vars) | ADR-012 | 42 |
| DualEngine write path | DualSimilarityEngine wraps HNSW only; StorageBackend handles pgvector | ADR-014 | 44 |
| Objective metrics | Config-driven objectives with field/threshold/direction | ADR-015 | 45 |
| ndarray | NOT added in Phase 2 (deferred to Phase 3 for Granger) | fe-003 decision | -- |

---

## 3. Files to Create

| Path | Description |
|------|-------------|
| `crates/ndp-intelligence/src/similarity/hnsw.rs` | HnswEngine: ruvector-core wrapper behind `#[cfg(feature = "ruvector")]` |
| `crates/ndp-intelligence/src/similarity/pgvector.rs` | PgVectorEngine: SQL K-NN search fallback using block_on |
| `crates/ndp-intelligence/src/similarity/dual.rs` | DualSimilarityEngine: HNSW insert + rebuild from StorageBackend |
| `crates/ndp-intelligence/src/predictions/mod.rs` | PredictionEngine + confidence scoring + ObjectiveMetric types |
| `crates/ndp-intelligence/src/predictions/outcome.rs` | OutcomeTracker: evaluate elapsed predictions against actual values |
| `crates/ndp-intelligence/src/service.rs` | IntelligenceService orchestrator + CycleSummary |
| `crates/ndp-intelligence/src/notify.rs` | PG NOTIFY listener with exponential backoff reconnection |
| `docker/intelligence/Dockerfile` | Multi-stage build (rust:1.82 builder, debian:bookworm-slim runtime) |

## 4. Files to Modify

| Path | Change |
|------|--------|
| `crates/ndp-intelligence/src/lib.rs` | Add `pub mod predictions; pub mod service; pub mod notify;` |
| `crates/ndp-intelligence/src/similarity/mod.rs` | Add `pub mod hnsw; pub mod pgvector; pub mod dual;` + factory function |
| `crates/ndp-intelligence/src/error.rs` | Add Database, Config, Shutdown variants to IntelligenceError |
| `crates/ndp-intelligence/Cargo.toml` | Add deps: deadpool-postgres, tokio (time, signal features) |
| `apps/ndp-intelligence-app/src/main.rs` | Replace stubs with real daemon/one-shot/backfill/status implementations |
| `apps/ndp-intelligence-app/Cargo.toml` | Add deps: config-client, signal handling |
| `apps/ndp-intelligence-app/src/config.rs` | AppConfig: runtime config from env vars (etcd_endpoints, db, pool, warmup) |
| `docker-compose.yml` | Add ndp-intelligence service definition |
| `docker-compose.integration.yml` | Add ndp-intelligence for integration testing |
| `deploy/pi/deploy.sh` | Add intelligence service deployment + seed domain config into etcd |
| `config/domains/indoor-air-quality/domain.json` | Add intelligence config block with objectives (already exists, extend) |

---

## 5. Data Structures

### PredictionEngine types (`crates/ndp-intelligence/src/predictions/mod.rs`)

```rust
pub struct PredictionEngine {
    db_pool: Arc<Pool>,
    horizons: Vec<chrono::Duration>,
    min_confidence: f64,
    objective_metrics: Vec<ObjectiveMetric>,
}

pub struct ObjectiveMetric {
    pub field: String,
    pub threshold: f64,
    pub direction: ThresholdDirection,
    pub label: String,
}

pub enum ThresholdDirection {
    Above,
    Below,
}

pub struct EvaluationSummary {
    pub evaluated: usize,
    pub correct: usize,
    pub incorrect: usize,
}
```

### IntelligenceService types (`crates/ndp-intelligence/src/service.rs`)

```rust
pub struct IntelligenceService {
    similarity: Box<dyn SimilarityEngine>,
    storage: Arc<dyn StorageBackend>,
    embedder: MetricEmbedder,
    prediction_engine: PredictionEngine,
    outcome_tracker: OutcomeTracker,
    db_pool: Arc<Pool>,
    domain_id: String,
    search_config: SearchConfig,
    last_processed: Option<DateTime<Utc>>,
    observation_count: usize,
    warmup_threshold: usize,
    backfill_mode: bool,
}

#[derive(Debug, Default)]
pub struct CycleSummary {
    pub rows_observed: usize,
    pub embeddings_generated: usize,
    pub neighbors_found: usize,
    pub predictions_made: usize,
    pub outcomes_evaluated: usize,
    pub correct: usize,
    pub incorrect: usize,
    pub duration: Duration,
}
```

### SimilarityEngine implementations

```rust
// hnsw.rs — #[cfg(feature = "ruvector")]
pub struct HnswEngine {
    db: ruvector_core::VectorDB,
    dimensions: usize,
    count: usize,
}

// pgvector.rs
pub struct PgVectorEngine {
    pool: Arc<Pool>,
    dimensions: usize,
    domain_id: String,
}

// dual.rs — #[cfg(feature = "ruvector")]
pub struct DualSimilarityEngine {
    hnsw: HnswEngine,
}
```

### Runtime config (`apps/ndp-intelligence-app/src/config.rs`)

```rust
pub struct AppConfig {
    pub database_url: String,
    pub domain_id: String,
    pub etcd_endpoints: Vec<String>, // default: ["http://etcd:2379"]
    pub poll_interval_secs: u64,     // default: 1200
    pub pool_size: usize,            // default: 2
    pub warmup_threshold: usize,     // default: 168
}
```

Domain config (IntelligenceConfig + objectives) is loaded from etcd via `config-client` at startup. The library crate accepts parsed structs — config source is the app binary's concern.

---

## 6. Key Function Signatures

```rust
// similarity/mod.rs — factory
pub async fn create_similarity_engine(
    config: &IntelligenceConfig,
    storage: Arc<dyn StorageBackend>,
    pool: Arc<Pool>,
    dimensions: usize,
    domain_id: &str,
) -> Result<Box<dyn SimilarityEngine>>;

// similarity/hnsw.rs
impl HnswEngine {
    pub fn new(dimensions: usize) -> Result<Self>;
    pub async fn rebuild_from_storage(
        &mut self, storage: &dyn StorageBackend, domain_id: &str,
    ) -> Result<usize>;
}

// predictions/mod.rs
impl PredictionEngine {
    pub fn new(db_pool: Arc<Pool>, config: &IntelligenceConfig) -> Self;
    pub async fn generate_predictions(
        &self, current_bucket: DateTime<Utc>, domain_id: &str, neighbors: &[SearchResult],
    ) -> Result<Vec<Prediction>>;
}

// predictions/outcome.rs
impl OutcomeTracker {
    pub fn new(db_pool: Arc<Pool>, storage: Arc<dyn StorageBackend>) -> Self;
    pub async fn evaluate_pending(&self, domain_id: &str) -> Result<EvaluationSummary>;
}

// service.rs
impl IntelligenceService {
    pub async fn new(
        app_config: &AppConfig, intel_config: &IntelligenceConfig,
        pool: Arc<Pool>, storage: Arc<dyn StorageBackend>,
    ) -> Result<Self>;
    pub async fn run_cycle(&mut self) -> Result<CycleSummary>;
    pub fn is_warmed_up(&self) -> bool;
    pub fn set_backfill_mode(&mut self, backfill: bool);
}

// notify.rs
impl NotifyListener {
    pub fn new(connection_string: &str, channel: &str) -> Self;
    pub async fn listen(&self) -> Result<mpsc::Receiver<String>>;
}

// apps/ndp-intelligence-app/src/config.rs
impl AppConfig {
    pub fn from_env() -> Result<Self>;  // reads env vars; domain config loaded from etcd separately
}
```

---

## 7. Implementation Waves

### Wave 1: SimilarityEngine Implementations
- P2-02: HnswEngine (ruvector-core wrapper, feature-gated)
- P2-03: PgVectorEngine (SQL K-NN with block_on)
- P2-04: HNSW rebuild from StorageBackend
- P2-01: Factory function + DualSimilarityEngine
- Cargo.toml: add deadpool-postgres

### Wave 2: Prediction Pipeline
- P2-05/P2-06: PredictionEngine + confidence scoring
- P2-07: Prediction storage (wiring to existing StorageBackend)
- P2-08: OutcomeTracker
- P2-15: Warmup logic (observation_count gate)

### Wave 3: Orchestration
- P2-09: IntelligenceService (full cycle: observe-embed-store-search-predict-evaluate)
- P2-10: PG NOTIFY listener with reconnection
- P2-11: Timer fallback (tokio::time::interval)
- Runtime config: AppConfig from env vars

### Wave 4: App Modes
- P2-12: Daemon mode (NOTIFY + timer + shutdown signal)
- P2-13: One-shot mode (single cycle, exit 0)
- P2-14: Backfill mode (historical embed-only, batch 100)

### Wave 5: Deployment
- P2-16: Docker container (Dockerfile + compose service)
- P2-17: deploy.sh integration
- Domain config: intelligence + objectives block
- Integration tests against TimescaleDB

---

## 8. Test Expectations

### Unit Tests (no DB required)

| Component | Tests |
|-----------|-------|
| HnswEngine | insert, search, count, dimension mismatch, empty index returns empty vec |
| PgVectorEngine | insert is no-op (verify no side effects) |
| DualSimilarityEngine | insert delegates to HNSW, search delegates to HNSW |
| PredictionEngine | generate with mock neighbors, confidence = k_supporting/k_total, min 3 neighbors gate |
| OutcomeTracker | evaluate with seeded predictions + mock storage |
| IntelligenceService | warmup gate (observation_count < 168 skips predict), backfill mode skips predict |
| CycleSummary | default values, Display impl |
| AppConfig | from_env with defaults, missing required vars return error |
| Helper functions | parse_bucket_from_id, format_pgvector, parse_horizon, sql_row_to_gold_row |

### Integration Tests (TimescaleDB + pgvector, `#[ignore]`)

| Test | Assert |
|------|--------|
| PgVectorEngine search | Insert via StorageBackend, search returns ordered results |
| HNSW rebuild | Insert via pgvector, rebuild HNSW, search matches |
| Full cycle | Seed Gold data, run cycle, verify embeddings + predictions stored |
| Outcome evaluation | Seed predictions + aligned view, verify correct/incorrect |
| PG NOTIFY | Send notification, verify listener receives |
| One-shot mode | Run single cycle, verify exit 0 |
| Backfill | Process 10 historical hours, verify embedding count |

---

## 9. Constraints

- ARM64 compilation required (ruvector-core feature-gated, pgvector fallback always available)
- 256 MB container memory limit
- No ndarray (deferred to Phase 3 for Granger)
- Must NOT break existing ingestion pipeline
- Must NOT require schema migration beyond PgVectorSchemaGenerator output
- Intelligence config must be `Option<IntelligenceConfig>` in DomainConfig (existing configs unaffected)
- No `todo!()`, `unimplemented!()`, or placeholder functions
- All Rust code must compile for both x86_64 and aarch64
- Existing 908+ tests must continue passing

---

## 10. Dependencies

```toml
# crates/ndp-intelligence/Cargo.toml — additional deps
deadpool-postgres = "0.12"
tokio = { version = "1", features = ["full", "signal", "time"] }
```

```toml
# apps/ndp-intelligence-app/Cargo.toml — additional deps
config-client = { path = "../../config-client" }
tokio = { version = "1", features = ["full", "signal"] }
```

---

## 11. NOT in Scope

- Event intelligence / text embeddings (Phase 4, fe-006)
- Granger causality (Phase 3, fe-005)
- Anomaly detection implementation (Phase 5; config accepted but not executed)
- Grafana dashboards (Phase 5)
- SONA learning (V1.3)
- MCP query interface (V1.3)
- ndarray dependency (Phase 3)
- Health endpoint / Prometheus metrics (Phase 5)

---

## 12. Alignment Status

From `product/features/fe-004/ALIGNMENT-REPORT.md`:

| Principle | Status |
|-----------|--------|
| Edge-Only | PASS |
| Config-Driven | PASS |
| Domain-Portable | PASS |
| Resource-Constrained | PASS |
| Integration-First | PASS |
| Privacy by Architecture | PASS |
| Self-Learning | PASS |

**Overall**: 7 PASS, 0 WARN, 0 VARIANCE, 0 FAIL. No variances requiring user approval.
