# fe-004 Specification: Similarity Intelligence (V1.2 Phase 2)

> **Feature**: fe-004
> **Version target**: v1.2.0
> **Predecessor**: fe-003 (Phase 0+1 library foundation)
> **Date**: 2026-02-15

---

## 1. Overview

fe-004 delivers the first end-to-end intelligence cycle: a running daemon that reads Gold aligned view data, generates metric embeddings, performs K-NN similarity search, produces predictions with confidence scores, tracks outcomes, and evaluates accuracy. After a 168-hour warmup period, the system generates hourly predictions answering: "Given conditions like now, what happened next in the past?"

This is Phase 2 ONLY -- metric similarity using numeric Gold data. No text/event embeddings (Phase 4), no Granger causality (Phase 3), no anomaly detection (Phase 5).

---

## 2. Deliverable Specifications

### P2-01: SimilarityEngine Factory + Wiring

**Description**: Wire the existing `SimilarityEngine` trait (defined in fe-003) to concrete implementations via a factory function.

**Interface**:
```rust
// crates/ndp-intelligence/src/similarity/mod.rs
pub fn create_similarity_engine(
    config: &IntelligenceConfig,
    storage: Arc<dyn StorageBackend>,
    dimensions: usize,
) -> Result<Box<dyn SimilarityEngine>>;
```

**Behavior**:
- If `ruvector` feature is enabled: return `DualSimilarityEngine` (HNSW + pgvector)
- If `ruvector` feature is disabled: return `PgVectorEngine` (SQL-only fallback)
- Factory reads `config.search` for K and min_similarity defaults

**Error Cases**:
- Invalid dimensions (0): return `SimilarityError::Backend`
- Database connection failure (pgvector only): return `SimilarityError::Backend`

---

### P2-02: HnswEngine (ruvector-core wrapper)

**Description**: Wrap ruvector-core `VectorDB` behind the `SimilarityEngine` trait.

**File**: `crates/ndp-intelligence/src/similarity/hnsw.rs`

**Interface**:
```rust
pub struct HnswEngine {
    db: ruvector_core::VectorDB,
    dimensions: usize,
    count: usize,
}

impl HnswEngine {
    pub fn new(dimensions: usize) -> Result<Self>;
    pub async fn rebuild_from_storage(
        &mut self, storage: &dyn StorageBackend, domain_id: &str,
    ) -> Result<usize>;
}

impl SimilarityEngine for HnswEngine { /* insert, search, count */ }
```

**Behavior**:
- `insert`: delegates to `ruvector_core::VectorDB::insert(VectorEntry { id, vector, metadata })`
- `search`: delegates to `ruvector_core::VectorDB::search(SearchQuery { vector, k, min_similarity })`, maps results
- `count`: returns internal counter (incremented on insert)
- `rebuild_from_storage`: loads all embeddings from `StorageBackend::load_embeddings`, inserts into HNSW, returns count

**Constraints**:
- Entire module behind `#[cfg(feature = "ruvector")]`
- Dimension mismatch on insert returns `SimilarityError::DimensionMismatch`
- Search on empty index returns empty `Vec<SearchResult>` (not error)

---

### P2-03: PgVectorEngine (SQL fallback)

**Description**: K-NN search via pgvector SQL queries.

**File**: `crates/ndp-intelligence/src/similarity/pgvector.rs`

**Interface**:
```rust
pub struct PgVectorEngine {
    pool: Arc<Pool>,
    dimensions: usize,
    domain_id: String,
}

impl PgVectorEngine {
    pub fn new(pool: Arc<Pool>, dimensions: usize, domain_id: String) -> Self;
}

impl SimilarityEngine for PgVectorEngine { /* insert, search, count */ }
```

**Behavior**:
- `insert`: No-op. Embeddings are already written via `StorageBackend::store_embedding`. PgVectorEngine only reads.
- `search`: SQL query `SELECT id, bucket, 1 - (embedding <=> $1::vector) AS similarity FROM gold.metric_embeddings WHERE domain_id = $2 ORDER BY embedding <=> $1::vector LIMIT $3`
- `count`: SQL `SELECT count(*) FROM gold.metric_embeddings WHERE domain_id = $1`

**Note**: The `SimilarityEngine` trait is synchronous. PgVectorEngine uses `tokio::runtime::Handle::current().block_on()` for async SQL inside sync trait methods per ADR-008.

---

### P2-04: HNSW Index Rebuild from pgvector

**Description**: On startup, load all stored embeddings from pgvector into the HNSW index.

**Behavior**:
- Called during `IntelligenceService::new()`
- Uses `StorageBackend::load_embeddings(domain_id, None)` to get all embeddings
- Iterates and calls `HnswEngine::insert()` for each
- Logs count at INFO level: "Rebuilt HNSW index with {n} vectors for domain {domain_id}"
- Returns count for startup metrics

**Performance**: O(n * log(n)) for HNSW insertion. Expected <1s for 10K vectors.

---

### P2-05: PredictionEngine

**Description**: Converts K-NN search results into predictions by looking up what happened NEXT for each neighbor.

**File**: `crates/ndp-intelligence/src/predictions/mod.rs`

**Interface**:
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
}

pub enum ThresholdDirection { Above, Below }

impl PredictionEngine {
    pub fn new(db_pool: Arc<Pool>, config: &IntelligenceConfig) -> Self;
    pub async fn generate_predictions(
        &self,
        current_bucket: DateTime<Utc>,
        domain_id: &str,
        neighbors: &[SearchResult],
    ) -> Result<Vec<Prediction>>;
}
```

**Algorithm**:
1. For each horizon (e.g., 1h, 4h):
2. For each neighbor: query `gold.aligned_hourly` at `neighbor.bucket + horizon` to get the actual outcome
3. For each objective metric: count how many neighbors crossed the threshold
4. `confidence = k_supporting / k_total` (neighbors with valid outcomes)
5. If `confidence >= min_confidence`: generate a `Prediction`
6. Skip if fewer than 3 neighbors have valid outcomes for that horizon

**Error Cases**:
- Database query failure: log warning, skip that neighbor, continue
- All neighbors lack outcomes: return empty predictions (no error)

---

### P2-06: Confidence Scoring

**Description**: Prediction confidence = supporting_neighbors / total_neighbors_with_outcomes.

**Behavior**:
- `k_supporting`: neighbors where the outcome matches the prediction direction
- `k_neighbors`: neighbors with valid outcome data (not null)
- `confidence = k_supporting as f64 / k_neighbors as f64`
- Stored alongside prediction in `gold.predictions`
- Minimum confidence threshold is configurable (default: 0.5)

---

### P2-07: Prediction Storage

**Description**: Write predictions to `gold.predictions` table.

**Behavior**: Uses existing `StorageBackend::store_prediction()`. The `Prediction` struct from fe-003 already has all required fields.

No new code needed -- this is wiring PredictionEngine output to existing storage.

---

### P2-08: OutcomeTracker

**Description**: Evaluate predictions after their horizon has elapsed.

**File**: `crates/ndp-intelligence/src/predictions/outcome.rs`

**Interface**:
```rust
pub struct OutcomeTracker {
    db_pool: Arc<Pool>,
    storage: Arc<dyn StorageBackend>,
}

pub struct EvaluationSummary {
    pub evaluated: usize,
    pub correct: usize,
    pub incorrect: usize,
}

impl OutcomeTracker {
    pub fn new(db_pool: Arc<Pool>, storage: Arc<dyn StorageBackend>) -> Self;
    pub async fn evaluate_pending(&self, domain_id: &str) -> Result<EvaluationSummary>;
}
```

**Algorithm**:
1. `StorageBackend::get_pending_outcomes(domain_id)` -- predictions where `actual_value IS NULL`
2. For each prediction where `bucket + horizon < now()`:
3. Query `gold.aligned_hourly` at `bucket + horizon` for the actual metric value
4. Compare actual vs threshold: determine `actual_breach`
5. `correct = (predicted_breach == actual_breach)`
6. Call `StorageBackend::record_outcome(prediction_id, actual)`

---

### P2-09: IntelligenceService Orchestrator

**Description**: Coordinates the full intelligence cycle: observe -> embed -> store -> search -> predict -> evaluate.

**File**: `crates/ndp-intelligence/src/service.rs`

**Interface**:
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

impl IntelligenceService {
    pub async fn new(/* all deps */) -> Result<Self>;
    pub async fn run_cycle(&mut self) -> Result<CycleSummary>;
    pub fn is_warmed_up(&self) -> bool;
    pub fn set_backfill_mode(&mut self, backfill: bool);
}
```

**Cycle Algorithm**:
1. **OBSERVE**: Query Gold aligned view for new rows since `last_processed`
2. **WARMUP**: For each row, call `embedder.observe()` to build running stats
3. **EMBED**: If warmed up, call `embedder.embed()` for each new row
4. **STORE**: Write embeddings via `StorageBackend::store_embedding()`
5. **INDEX**: Insert into `SimilarityEngine` (HNSW + pgvector dual-write)
6. **SEARCH**: If not backfill mode, search for K nearest neighbors of latest embedding
7. **PREDICT**: If warmed up and not backfill, generate predictions
8. **EVALUATE**: Evaluate any pending predictions whose horizons have elapsed
9. Update `last_processed`, increment `observation_count`

**Warmup Gate**: `observation_count >= warmup_threshold (168)`. Observation-count-based, not time-based (works correctly for backfill).

---

### P2-10: PG NOTIFY Listener

**Description**: Listen for `gold_refresh` notifications from TimescaleDB.

**File**: `crates/ndp-intelligence/src/notify.rs`

**Interface**:
```rust
pub struct NotifyListener {
    connection_string: String,
    channel: String,
}

impl NotifyListener {
    pub fn new(connection_string: &str, channel: &str) -> Self;
    pub async fn listen(&self) -> Result<mpsc::Receiver<String>>;
}
```

**Note**: CAs (continuous aggregates) cannot have triggers. The NOTIFY is an optimization only. The primary wake mechanism is the timer (P2-11). If PG NOTIFY is available (e.g., from a materialized view refresh function), it reduces latency. If not, the timer handles everything.

**Reconnection**: On connection drop, retry with exponential backoff (1s, 2s, 4s, 8s, max 60s). Log at WARN on each retry.

---

### P2-11: Timer Fallback

**Description**: If no NOTIFY received within 20 minutes, poll the Gold view on a timer.

**Behavior**:
- `tokio::time::interval(Duration::from_secs(1200))` -- 20 min default, configurable
- Timer fires independently; NOTIFY can trigger between timer ticks for lower latency
- Both paths call the same `run_cycle()` method

---

### P2-12: Daemon Mode

**Description**: Continuous loop combining NOTIFY listener and timer fallback.

**Main loop**:
```
loop {
    select! {
        _ = notify_rx.recv() => run_cycle(),
        _ = timer.tick() => run_cycle(),
        _ = shutdown_signal => break,
    }
    log cycle summary
}
```

**Graceful shutdown**: Listens for SIGTERM/SIGINT via `tokio::signal`. Completes current cycle, then exits.

---

### P2-13: One-Shot Mode

**Description**: Run a single intelligence cycle, then exit.

**Behavior**:
1. Initialize IntelligenceService
2. Call `run_cycle()` once
3. Log CycleSummary
4. Exit with code 0 (success) or 1 (error)

---

### P2-14: Backfill Mode

**Description**: Process historical data to generate embeddings (no predictions).

**Behavior**:
1. Parse `--since` timestamp (or default to earliest Gold data)
2. Set `IntelligenceService::set_backfill_mode(true)`
3. Query Gold view from `since` to `now`, in batches of 100 rows
4. For each batch: call `run_cycle()` (which skips predict/evaluate in backfill mode)
5. Log total embeddings generated
6. Exit 0

---

### P2-15: Warmup Logic

**Description**: Skip predictions during warmup (first 168 observations).

**Behavior**:
- `IntelligenceService::is_warmed_up()` returns `observation_count >= warmup_threshold`
- During warmup: observe + embed + store + index, but skip search/predict
- Warmup counter persists across restarts via `SELECT count(*) FROM gold.metric_embeddings WHERE domain_id = $1`
- Backfill mode always skips predictions regardless of warmup state

---

### P2-16: Docker Container

**Description**: Multi-stage Dockerfile for ndp-intelligence-app.

**File**: `docker/intelligence/Dockerfile`

**Stages**:
1. **Builder**: `rust:1.82-slim-bookworm`, cargo build `--release`
2. **Runtime**: `debian:bookworm-slim`, copy binary + CA certificates

**docker-compose.yml addition**:
```yaml
ndp-intelligence:
  build:
    context: .
    dockerfile: docker/intelligence/Dockerfile
  depends_on:
    - timescaledb
    - etcd
  environment:
    - DATABASE_URL=postgresql://...
    - INTELLIGENCE_DOMAIN=indoor-air-quality
    - ETCD_ENDPOINTS=http://etcd:2379
    - INTELLIGENCE_POLL_INTERVAL_SECS=1200
  deploy:
    resources:
      limits:
        memory: 256M
  restart: unless-stopped
```

---

### P2-17: deploy.sh Integration

**Description**: Add ndp-intelligence service to the Pi deployment script.

**Behavior**:
- Add `ndp-intelligence` to the service list in `deploy/pi/deploy.sh`
- Build and start alongside existing services
- Health check: process is running (no HTTP health endpoint in Phase 2)
- If intelligence container fails, ingestion pipeline continues unaffected

---

## 3. Cross-Cutting Concerns

### Error Handling

All errors propagate through `IntelligenceError` (existing in `crates/ndp-intelligence/src/error.rs`). Extend with:
- `DatabaseError(String)` -- connection/query failures
- `ConfigError(String)` -- invalid configuration
- `EmbeddingError(ndp_lib::gold::embeddings::EmbeddingError)` -- from MetricEmbedder

No panics. No `unwrap()` on fallible operations. All errors logged at ERROR level with context.

### Configuration

Intelligence configuration is optional in DomainConfig. When absent, intelligence features are disabled. Domain config (including the intelligence block and objectives) is loaded from etcd via the `config-client` crate at startup. The `config/domains/` directory contains source-of-truth JSON files that `deploy.sh` seeds into etcd.

```json
{
  "intelligence": {
    "enabled": true,
    "embedding": { "type": "metric", "fields": { ... } },
    "search": { "k": 20, "min_similarity": 0.7, "prediction_horizons": ["1 hour", "4 hours"] },
    "anomaly": null
  },
  "objectives": [
    { "field": "co2_mean", "threshold": 1000.0, "direction": "above", "label": "CO2 high" }
  ]
}
```

Runtime config (DATABASE_URL, ETCD_ENDPOINTS, poll interval, pool size, warmup threshold) is read from environment variables in the app binary. The intelligence library crate accepts parsed config structs — it has no dependency on etcd or config-client.

### Logging

- INFO: cycle summaries, startup/shutdown, warmup progress
- WARN: individual operation failures (retried or skipped), reconnection attempts
- ERROR: fatal failures that stop the cycle
- DEBUG: per-row embedding details, SQL queries

### Metrics (Phase 2 scope)

Metrics are logged to tracing, not exposed via HTTP/Prometheus (Phase 5). Key metrics:
- `cycle_duration_ms`
- `embeddings_generated`
- `predictions_made`
- `prediction_accuracy` (rolling)
- `hnsw_search_latency_us`
- `observation_count`

---

## 4. Data Flow

```
Gold aligned_hourly (TimescaleDB)
        |
        v
[OBSERVE] -- query new rows since last_processed
        |
        v
[WARMUP/EMBED] -- MetricEmbedder.observe() + embed()
        |
        v
[STORE] -- StorageBackend.store_embedding()
        |
        v
[INDEX] -- SimilarityEngine.insert() (HNSW + pgvector dual-write)
        |
        v
[SEARCH] -- SimilarityEngine.search() (K-NN)
        |
        v
[PREDICT] -- PredictionEngine.generate_predictions()
        |
        v
[STORE] -- StorageBackend.store_prediction()
        |
        v
[EVALUATE] -- OutcomeTracker.evaluate_pending()
        |
        v
[RECORD] -- StorageBackend.record_outcome()
```

---

## 5. Acceptance Criteria Traceability

| AC from SCOPE.md | Deliverable(s) | Verification |
|-------------------|----------------|-------------|
| Embeddings generated | P2-01, P2-02, P2-03, P2-04, P2-09 | SELECT count(*) > 0 FROM gold.metric_embeddings |
| Predictions after warmup | P2-05, P2-06, P2-09, P2-15 | Prediction rows exist after 168 observations |
| HNSW search <1ms p99 | P2-02 | In-process timing with 1000+ vectors |
| pgvector search <10ms p99 | P2-03 | SQL timing |
| Full cycle <500ms | P2-09 | CycleSummary.duration |
| Memory <100MB actual | P2-16 | docker stats |
| Prediction accuracy logged | P2-08 | EvaluationSummary logged |
| Daemon 10+ min stable | P2-12 | Runtime duration without crash |
| One-shot works | P2-13 | Exit code 0, predictions exist |
| Backfill works | P2-14 | N embeddings generated for historical data |
| Docker builds x86+arm | P2-16 | Multi-arch build success |
| deploy.sh deploys | P2-17 | Service running on Pi |
