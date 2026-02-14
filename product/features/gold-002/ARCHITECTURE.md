# gold-002: V1.2 Intelligence Foundation — Detailed Architecture

> **Parent roadmap:** `product/features/gold-001/FEATURE-ROADMAPv1.2.md`
> **Created:** 2026-02-13
> **Status:** Draft for Review
> **Scope:** Architecture for V1.2 implementation phases only. SONA, MCP, and sysops domain (V1.3) are out of scope.
> **Updated:** 2026-02-14 — Incorporated ruvector overlap analysis: quantization delegated to ruvector-core, ReasoningBank schema prep, SONA/ruv-fann assessment.

---

## 1. System Context

### What Exists Today (V1.1)

```
Sources ──> air-quality-app ──> Bronze (Parquet + WAL)
                             ──> Silver (TimescaleDB hypertables)
                                         │
                              Gold CAs (continuous aggregates, hourly)
                              Gold aligned view (cross-stream JOIN)
                              Gold events infrastructure
                              Gold feature registry (lag, rolling, trend)
```

**Workspace crates:** ndp-types, ndp-lib, config-client, core, air-quality-app, silver-etl, ndp-validate, ndp-gold-ddl, ndp-cli

**Existing Gold layer modules** (all in `crates/ndp-lib/src/gold/`):
- `config/` — StreamConfig, DomainConfig, GoldEtlConfig, ConfigLoader trait
- `generators/` — ContinuousAggregateGenerator, AlignedViewGenerator, EventsGenerator, StateTransitionGenerator
- `registry/` — FeatureRegistry (lag, rolling, trend generators)
- `planner/` — SyncPlanner for idempotent DDL
- `validation/` — Config validators
- `db/` — CaChecker trait for database introspection

**Existing streams:** air-quality (PurpleAir), outdoor-weather (NWS observations), outdoor-air-quality (AirNow), home-assistant-state, nws-forecast-hourly, nws-gridpoints-forecast, nws-observations

**Existing domain:** indoor-air-quality (4-stream aligned view with objectives)

### What V1.2 Adds

An **intelligence layer** that reads prepared Gold features, embeds them as vectors, stores them durably, and performs K-NN similarity search to generate predictions. The intelligence layer is architecturally separated into:

1. **Feature engineering** (embedding pipeline) — lives in `ndp-lib::gold`
2. **Intelligence** (search, predict, validate) — lives in new `ndp-intelligence` crate
3. **Orchestration** (daemon lifecycle) — lives in new `ndp-intelligence-app` binary

---

## 2. Crate Architecture

### New Workspace Members

```
crates/ndp-intelligence/          # Library: intelligence algorithms
apps/ndp-intelligence-app/        # Binary: standalone daemon
```

### Dependency Graph

```
                    ndp-types
                   /    |     \
              ndp-lib   |   config-client
             /    |     |
    ndp-intelligence    |
             \          |
       ndp-intelligence-app
```

Key design constraint: `ndp-intelligence` depends on `ndp-lib` (for Gold config types and embedding infrastructure) and `ndp-types`, but NOT on `air-quality-app`, `core`, or any ingestion code. The intelligence layer is a pure consumer of Gold layer data.

### ndp-intelligence crate (library)

Responsibilities: similarity search, prediction generation, Granger causality, graph storage, outcome tracking.

```
crates/ndp-intelligence/
  Cargo.toml
  src/
    lib.rs                    # Public API + IntelligenceService
    config.rs                 # IntelligenceConfig (domain-level)
    error.rs                  # Intelligence error types
    similarity/
      mod.rs                  # SimilarityEngine trait + dispatch
      hnsw.rs                 # ruvector-core HNSW wrapper
      pgvector.rs             # pgvector SQL fallback
    graph/
      mod.rs                  # GraphStore trait + dispatch
      ruvector.rs             # ruvector-graph backend (if compiles)
      sql.rs                  # SQL adjacency backend (fallback)
    predictions/
      mod.rs                  # PredictionEngine
      outcome.rs              # Outcome tracking + accuracy
      confidence.rs           # Confidence scoring
    granger/
      mod.rs                  # Granger causality scanner
      candidates.rs           # Candidate registry
      evidence.rs             # Evidence accumulator
      ranker.rs               # Candidate ranking
    anomaly/
      mod.rs                  # Anomaly detector (distance-based)
    storage/
      mod.rs                  # StorageBackend trait
      postgres.rs             # pgvector + predictions tables
```

### ndp-lib Gold extensions (feature engineering)

New modules added to the existing `crates/ndp-lib/src/gold/` tree:

```
crates/ndp-lib/src/gold/
  embeddings/                 # NEW module
    mod.rs                    # Embedder trait + EmbeddingConfig
    metric.rs                 # MetricEmbedder (z-score normalize)
  populator/                  # NEW module
    mod.rs                    # Populator trait
    embedding_writer.rs       # Write embeddings to pgvector tables
  generators/
    pgvector_schema.rs        # NEW: DDL for gold.metric_embeddings, etc.
```

The EventEmbedder and CompositeEmbedder (text pipeline) are Phase 4 additions and are NOT part of the initial architecture. The trait is designed to accommodate them later.

**Quantization note:** Custom quantization (PQ8, scalar, binary) is NOT implemented in ndp-lib. Quantization is delegated to ruvector-core's built-in PQ support, which handles it internally at the HNSW layer. This avoids reimplementing logic that already exists in the ruvector ecosystem.

### ndp-intelligence-app binary

```
apps/ndp-intelligence-app/
  Cargo.toml
  src/
    main.rs                   # CLI (clap): daemon, one-shot, backfill, status
```

Modes:
- `daemon` — PG LISTEN/NOTIFY loop, runs intelligence cycle on Gold CA refresh
- `one-shot` — Single intelligence cycle (for testing/debugging)
- `backfill` — Process historical Gold data
- `status` — Print current state (last run, prediction accuracy, index stats)

---

## 3. Trait Design

### Embedder Trait

Lives in `ndp-lib::gold::embeddings`. This is the core abstraction that makes the pipeline extensible.

```rust
/// Produces vector embeddings from Gold layer data.
///
/// Phase 1: MetricEmbedder only.
/// Phase 2: + EventEmbedder, CompositeEmbedder.
pub trait Embedder: Send + Sync {
    /// Embed a single Gold row into a vector.
    fn embed(&self, row: &GoldRow) -> Result<Embedding>;

    /// Output dimensionality.
    fn dimensions(&self) -> usize;

    /// Human-readable name for logging.
    fn name(&self) -> &str;
}

/// A Gold aligned view row, represented as named numeric fields.
pub struct GoldRow {
    pub bucket: chrono::DateTime<chrono::Utc>,
    pub domain_id: String,
    pub fields: BTreeMap<String, Option<f64>>,
}

/// Output of any Embedder.
pub struct Embedding {
    pub vector: Vec<f32>,
    pub dimensions: usize,
    pub metadata: HashMap<String, serde_json::Value>,
}
```

Design notes:
- `GoldRow` uses `BTreeMap<String, Option<f64>>` — domain-agnostic, works for any aligned view.
- `Option<f64>` handles NULLs from FULL OUTER JOINs in the aligned view.
- The Embedder trait has no database dependencies — it's pure transformation.

### MetricEmbedder

```rust
pub struct MetricEmbedder {
    /// Ordered list of fields to embed.
    fields: Vec<EmbeddingField>,
    /// Running z-score statistics per field.
    stats: HashMap<String, RunningStats>,
    /// Total output dimensions.
    dimensions: usize,
}

pub struct EmbeddingField {
    pub name: String,
    pub source: FieldSource,
    pub null_strategy: NullStrategy,
}

pub enum FieldSource {
    /// Direct field from aligned view row
    Direct(String),
    /// Temporal encoding (hour_sin, hour_cos, is_weekend)
    Temporal(TemporalEncoding),
}

pub enum NullStrategy {
    /// Replace with 0.0 (neutral in z-score space)
    Zero,
    /// Use last known value
    LastKnown,
    /// Use field mean
    Mean,
}
```

The MetricEmbedder is configured from the domain's `intelligence` config block. It z-score normalizes each field using running statistics (mean, std) accumulated over a warmup window.

### SimilarityEngine Trait

Lives in `ndp-intelligence::similarity`.

```rust
/// Backend for vector similarity search.
///
/// Two implementations:
/// - HnswEngine: ruvector-core in-process HNSW (fast, primary)
/// - PgVectorEngine: pgvector SQL queries (durable, fallback)
pub trait SimilarityEngine: Send + Sync {
    /// Insert a vector with metadata.
    fn insert(&mut self, entry: VectorEntry) -> Result<()>;

    /// K-NN search.
    fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>>;

    /// Number of vectors in the index.
    fn count(&self) -> usize;

    /// Rebuild index from durable storage (startup).
    fn rebuild_from_storage(&mut self, storage: &dyn StorageBackend) -> Result<usize>;
}

pub struct VectorEntry {
    pub id: String,              // "{domain_id}:{bucket_iso}"
    pub vector: Vec<f32>,
    pub metadata: serde_json::Value,
}

pub struct SearchQuery {
    pub vector: Vec<f32>,
    pub k: usize,
    pub min_similarity: f64,
}

pub struct SearchResult {
    pub id: String,
    pub similarity: f64,
    pub metadata: serde_json::Value,
}
```

### StorageBackend Trait

Lives in `ndp-intelligence::storage`.

```rust
/// Durable storage for embeddings, predictions, and candidates.
pub trait StorageBackend: Send + Sync {
    // Embeddings
    fn store_embedding(&self, embedding: &StoredEmbedding) -> Result<()>;
    fn load_embeddings(&self, domain_id: &str, since: Option<DateTime<Utc>>) -> Result<Vec<StoredEmbedding>>;

    // Predictions
    fn store_prediction(&self, prediction: &Prediction) -> Result<()>;
    fn get_pending_outcomes(&self, domain_id: &str) -> Result<Vec<Prediction>>;
    fn record_outcome(&self, prediction_id: i64, actual: &ActualOutcome) -> Result<()>;
}
```

Single implementation: `PostgresStorage` backed by TimescaleDB + pgvector extension.

Causal relationships are stored via the `GraphStore` trait (see below), not `StorageBackend`.

### GraphStore Trait

Lives in `ndp-intelligence::graph`. A generic graph capability — nodes, typed edges, basic traversal. The specific domain model (what node types and edge types exist) is shaped later when data exists to populate it.

```rust
/// Generic graph storage for typed nodes and edges.
///
/// Two implementations:
/// - RuvectorGraphStore: ruvector-graph backend (preferred, if compiles on ARM)
/// - SqlGraphStore: SQL adjacency tables (fallback)
pub trait GraphStore: Send + Sync {
    /// Add a node to the graph.
    fn add_node(&self, node: &GraphNode) -> Result<()>;

    /// Add a typed edge between two nodes.
    fn add_edge(&self, edge: &GraphEdge) -> Result<()>;

    /// Get all edges from a node, optionally filtered by edge type.
    fn get_edges(&self, node_id: &str, edge_type: Option<&str>) -> Result<Vec<GraphEdge>>;

    /// Get neighbors of a node (1-hop traversal).
    fn get_neighbors(&self, node_id: &str, edge_type: Option<&str>) -> Result<Vec<GraphNode>>;

    /// Count nodes, optionally filtered by type.
    fn node_count(&self, node_type: Option<&str>) -> Result<usize>;

    /// Count edges, optionally filtered by type.
    fn edge_count(&self, edge_type: Option<&str>) -> Result<usize>;
}

pub struct GraphNode {
    pub id: String,
    pub node_type: String,
    pub properties: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub struct GraphEdge {
    pub source_id: String,
    pub target_id: String,
    pub edge_type: String,
    pub weight: f64,
    pub properties: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
```

Design notes:
- Generic by design — no domain-specific node/edge types baked in. The specific model (metric nodes, event nodes, causal edges, etc.) is shaped when the data types exist.
- Phase 2: Granger results stored as edges between field-pair nodes.
- Phase 4: Event data introduces new node types and cross-type edges. Model is shaped then, informed by real data.
- ruvector-graph is tested in Phase 0 alongside ruvector-core. If it compiles, it's the backend. If not, SQL adjacency tables provide the same interface.

---

## 4. Database Schema

All tables live in the `gold` schema, consistent with existing Gold layer conventions.

### pgvector Extension

```sql
CREATE EXTENSION IF NOT EXISTS vector;
```

Added to the TimescaleDB container. pgvector is available as a pre-built arm64 apt package.

### Metric Embeddings Table

```sql
CREATE TABLE IF NOT EXISTS gold.metric_embeddings (
    bucket          TIMESTAMPTZ NOT NULL,
    domain_id       TEXT NOT NULL,
    embedding       vector,          -- variable dimension per domain
    dimensions      INTEGER NOT NULL,
    metadata        JSONB DEFAULT '{}',
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (bucket, domain_id)
);
SELECT create_hypertable('gold.metric_embeddings', 'bucket',
    if_not_exists => TRUE);
```

HNSW index is created per-domain after sufficient data accumulates (>100 vectors). Before that, pgvector uses sequential scan which is fast enough for small datasets.

### Predictions Table

```sql
CREATE TABLE IF NOT EXISTS gold.predictions (
    id              BIGSERIAL,
    bucket          TIMESTAMPTZ NOT NULL,
    domain_id       TEXT NOT NULL,
    metric          TEXT NOT NULL,
    horizon         INTERVAL NOT NULL,
    predicted_value DOUBLE PRECISION,
    predicted_breach BOOLEAN,
    confidence      DOUBLE PRECISION,
    k_neighbors     INTEGER,
    k_supporting    INTEGER,
    actual_value    DOUBLE PRECISION,
    actual_breach   BOOLEAN,
    correct         BOOLEAN,
    evaluated_at    TIMESTAMPTZ,
    PRIMARY KEY (id, bucket)
);
SELECT create_hypertable('gold.predictions', 'bucket',
    if_not_exists => TRUE);
```

### Graph Tables (SQL Fallback)

Used when ruvector-graph is not available. If ruvector-graph compiles on ARM, it handles its own storage and these tables are not created.

```sql
CREATE TABLE IF NOT EXISTS gold.graph_nodes (
    id              TEXT PRIMARY KEY,
    node_type       TEXT NOT NULL,
    properties      JSONB DEFAULT '{}',
    created_at      TIMESTAMPTZ DEFAULT NOW()
);
CREATE INDEX idx_graph_nodes_type ON gold.graph_nodes(node_type);

CREATE TABLE IF NOT EXISTS gold.graph_edges (
    id              SERIAL PRIMARY KEY,
    source_id       TEXT NOT NULL REFERENCES gold.graph_nodes(id),
    target_id       TEXT NOT NULL REFERENCES gold.graph_nodes(id),
    edge_type       TEXT NOT NULL,
    weight          DOUBLE PRECISION DEFAULT 1.0,
    properties      JSONB DEFAULT '{}',
    created_at      TIMESTAMPTZ DEFAULT NOW()
);
CREATE INDEX idx_graph_edges_source ON gold.graph_edges(source_id, edge_type);
CREATE INDEX idx_graph_edges_target ON gold.graph_edges(target_id, edge_type);
```

In Phase 3, Granger causal candidates are stored as graph edges (edge_type = `'causes'`) between field-pair nodes. The `properties` JSONB carries lag_minutes, correlation, granger_p_value, evidence_count, and status.

### ReasoningBank Table (V1.3 Prep — Empty in V1.2)

```sql
CREATE TABLE IF NOT EXISTS gold.reasoning_bank (
    id              SERIAL PRIMARY KEY,
    domain_id       TEXT NOT NULL,
    adapter_name    TEXT NOT NULL,
    adapter_blob    BYTEA,
    ewc_fisher      BYTEA,
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    performance     JSONB DEFAULT '{}'
);
```

Created by `PgVectorSchemaGenerator` but unused in V1.2. Prepares storage for ruvector SONA (LoRA adapters + EWC++ Fisher information) in V1.3.

### Schema DDL Generation

The pgvector schema DDL is generated by a new `PgVectorSchemaGenerator` in `ndp-lib::gold::generators::pgvector_schema`, following the same pattern as `ContinuousAggregateGenerator`. This means `ndp gold sync` and `ndp-cli` can manage intelligence tables alongside existing Gold DDL. The generator produces DDL for all four tables (metric_embeddings, predictions, causal_candidates, reasoning_bank).

---

## 5. Intelligence Cycle

The core runtime loop, executed by `ndp-intelligence-app`:

```
1. WAKE      ← PG NOTIFY from Gold CA refresh, or timer fallback
2. OBSERVE   ← Read latest aligned view row(s) not yet embedded
3. EMBED     ← MetricEmbedder: aligned row → Vec<f32>
4. STORE     ← Write to pgvector (durable) + HNSW index (fast)
5. SEARCH    ← K-NN: find 20 most similar past states
6. PREDICT   ← For each neighbor, look up what happened next
7. EVALUATE  ← Check predictions whose horizon has elapsed
8. SLEEP     ← Wait for next notification
```

### Step 1: Wake (PG NOTIFY)

```sql
-- Triggered by TimescaleDB after CA refresh completes
-- (or manually via: SELECT pg_notify('gold_refresh', 'indoor-air-quality'))
LISTEN gold_refresh;
```

Fallback: if no notification within 20 minutes, poll on timer. This handles edge cases where the CA refresh notification is missed.

### Step 2: Observe

```sql
SELECT bucket, <all_fields>
FROM gold.indoor_air_quality_aligned_hourly
WHERE bucket > $1  -- last processed bucket
ORDER BY bucket ASC
LIMIT 10;          -- batch up to 10 hours if behind
```

### Step 3: Embed

For each row:
1. Extract field values into `GoldRow`
2. Call `MetricEmbedder::embed(&row)` → `Embedding`
3. Z-score normalize each field using running statistics
4. Handle NULLs per field's `NullStrategy`

### Step 4: Store

1. INSERT into `gold.metric_embeddings` (pgvector, durable)
2. Insert into in-process HNSW index (ruvector-core, fast search)

### Step 5: Search

```rust
let results = hnsw_engine.search(&SearchQuery {
    vector: current_embedding.vector,
    k: 20,
    min_similarity: 0.7,
})?;
```

### Step 6: Predict

For each of the K neighbors:
1. Look up the NEXT hour's aligned view row
2. Check each objective: did the metric breach the threshold?
3. Aggregate: "In N/K similar past states, CO2 exceeded 800 within 1 hour"
4. Store prediction with confidence = N/K

### Step 7: Evaluate

For predictions whose horizon has elapsed:
1. Read the actual aligned view row at the predicted time
2. Compare actual vs predicted
3. Update `gold.predictions` with actual values and correctness

---

## 6. Configuration Extensions

### Domain Config: Intelligence Block

Added to the existing domain.json schema:

```json
{
  "id": "indoor-air-quality",
  "streams": ["...existing..."],
  "alignment": {"...existing..."},
  "objectives": ["...existing..."],
  "intelligence": {
    "enabled": true,
    "embedding": {
      "type": "metric",
      "fields": {
        "temporal": ["hour_sin", "hour_cos", "is_weekend"],
        "direct": [
          {"field": "indoor_co2_mean", "null_strategy": "zero"},
          {"field": "indoor_pm25_mean", "null_strategy": "zero"},
          {"field": "indoor_temperature_c_mean", "null_strategy": "mean"},
          {"field": "indoor_humidity_pct_mean", "null_strategy": "mean"},
          {"field": "outdoor_temperature_c_mean", "null_strategy": "mean"},
          {"field": "outdoor_humidity_pct_mean", "null_strategy": "mean"},
          {"field": "outdoor_wind_speed_mean", "null_strategy": "zero"},
          {"field": "outdoor_aqi_pm25_mean", "null_strategy": "zero"}
        ],
        "derived": [
          "indoor_co2_mean_trend_4h",
          "indoor_pm25_mean_trend_4h",
          "indoor_co2_mean_std_4h",
          "indoor_co2_mean_diff_1h"
        ]
      }
    },
    "search": {
      "k": 20,
      "min_similarity": 0.7,
      "prediction_horizons": ["1 hour", "4 hours"]
    },
    "anomaly": {
      "enabled": true,
      "distance_threshold_sigma": 2.5
    }
  }
}
```

This extends `DomainConfig` without breaking existing configs — the `intelligence` field is optional and defaults to `None`.

### IntelligenceConfig Type

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntelligenceConfig {
    pub enabled: bool,
    pub embedding: EmbeddingConfig,
    pub search: SearchConfig,
    pub anomaly: Option<AnomalyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    #[serde(rename = "type")]
    pub embedding_type: EmbeddingType,
    pub fields: EmbeddingFieldsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingType {
    Metric,
    // Phase 2:
    // Event,
    // Composite,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    pub k: usize,
    pub min_similarity: f64,
    pub prediction_horizons: Vec<String>,
}
```

---

## 7. Deployment Architecture

### Container Topology

```
┌─────────────────────┐  ┌──────────────────────┐  ┌────────────────────┐
│  air-quality-app    │  │  ndp-intelligence-app │  │  timescaledb       │
│  (ingestion)        │  │  (intelligence)       │  │  + pgvector ext    │
│  512 MB limit       │  │  256 MB limit (Ph1)   │  │                    │
│                     │  │  512 MB limit (Ph2+)  │  │  gold.metric_embed │
│  Bronze + Silver    │  │                       │  │  gold.predictions  │
│  writes             │  │  HNSW in-process      │  │  gold.causal_cand  │
│                     │  │  ruvector-core        │  │                    │
└────────┬────────────┘  └───────────┬───────────┘  └─────────┬──────────┘
         │                           │                         │
         └───── Silver writes ───────┴── pgvector reads/writes─┘
```

Phase 1 (metric-only): 256 MB container limit is sufficient.
Phase 2 (with MiniLM): increases to 512 MB.

### Docker Compose Addition

```yaml
ndp-intelligence:
  build:
    context: .
    dockerfile: docker/intelligence/Dockerfile
  container_name: ndp-intelligence
  restart: unless-stopped
  depends_on:
    timescaledb:
      condition: service_healthy
  environment:
    - DATABASE_URL=postgresql://ndp:${POSTGRES_PASSWORD}@timescaledb:5432/ndp
    - INTELLIGENCE_MODE=daemon
    - RUST_LOG=ndp_intelligence=info
  mem_limit: 256m
  networks:
    - ndp
```

### Dockerfile

Follows existing NDP pattern (multi-stage, cargo-chef for caching):

```dockerfile
FROM rust:1-bookworm AS chef
RUN cargo install cargo-chef --locked
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
RUN apt-get update && apt-get install -y build-essential  # For SimSIMD
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release --bin ndp-intelligence-app

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/ndp-intelligence-app /usr/local/bin/
ENTRYPOINT ["ndp-intelligence-app"]
CMD ["daemon"]
```

### TimescaleDB Container Update

Add pgvector extension to existing TimescaleDB Dockerfile:

```dockerfile
# In docker/timescaledb/Dockerfile
RUN apt-get update && apt-get install -y \
    postgresql-15-pgvector \
    && rm -rf /var/lib/apt/lists/*
```

---

## 8. Data Flow Detail

### Embedding Pipeline (Phase 1)

```
gold.indoor_air_quality_aligned_hourly (existing materialized view)
    │
    │  SELECT bucket, indoor_co2_mean, indoor_pm25_mean, ...
    │
    ▼
GoldRow { bucket, domain_id, fields: BTreeMap }
    │
    │  MetricEmbedder::embed()
    │  - Add temporal features (hour_sin, hour_cos, is_weekend)
    │  - Z-score normalize each field
    │  - Handle NULLs per strategy
    │
    ▼
Embedding { vector: Vec<f32> [~32D], metadata }
    │
    ├──> gold.metric_embeddings (pgvector INSERT, durable)
    │
    └──> HNSW in-process index (ruvector-core, fast search)
```

### Z-Score Normalization

Running statistics maintained per field:
- Warmup window: first 168 hours (1 week) of data
- During warmup: collect mean and std, do not generate predictions
- After warmup: z-score = (value - mean) / std, with exponential decay (alpha=0.01) for mean/std updates

This ensures all fields are in comparable scales regardless of their natural units.

### Temporal Encoding

```rust
fn temporal_features(bucket: &DateTime<Utc>) -> Vec<f32> {
    let hour = bucket.hour() as f32;
    let hour_sin = (2.0 * PI * hour / 24.0).sin();
    let hour_cos = (2.0 * PI * hour / 24.0).cos();
    let is_weekend = if bucket.weekday().num_days_from_monday() >= 5 { 1.0 } else { 0.0 };
    vec![hour_sin, hour_cos, is_weekend]
}
```

---

## 9. ruvector Integration

### Go/No-Go Gate

Before any implementation, validate both ruvector-core and ruvector-graph compile on aarch64:

```bash
# On Pi 5 or via cross-compilation
cargo init /tmp/ruvector-test
cd /tmp/ruvector-test
cat >> Cargo.toml <<EOF
ruvector-core = "2.0.1"
ruvector-graph = "0.1"
EOF
cargo build --release
```

**ruvector-core** (HNSW vector search):
- If compiles: primary HNSW backend for fast K-NN search
- If SimSIMD fails: try `default-features = false, features = ["storage", "hnsw", "parallel"]`
- If fails entirely: pgvector-only mode (slower search, functionally equivalent)

**ruvector-graph** (graph storage):
- If compiles: primary graph backend for nodes/edges
- If fails: SQL adjacency tables (`gold.graph_nodes`, `gold.graph_edges`) provide equivalent interface

### Integration Pattern

```rust
use ruvector_core::VectorDB;

pub struct HnswEngine {
    db: VectorDB,
    dimensions: usize,
}

impl HnswEngine {
    pub fn new(dimensions: usize, db_path: &Path) -> Result<Self> {
        let db = VectorDB::new(/* config */)?;
        Ok(Self { db, dimensions })
    }
}

impl SimilarityEngine for HnswEngine {
    fn insert(&mut self, entry: VectorEntry) -> Result<()> {
        self.db.insert(/* map to ruvector types */)?;
        Ok(())
    }

    fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>> {
        let results = self.db.search(/* map to ruvector query */)?;
        Ok(results.into_iter().map(/* map back */).collect())
    }
    // ...
}
```

### Quantization Delegation

Quantization (PQ8, PQ4, binary, scalar) is handled entirely by ruvector-core's built-in support. We do NOT implement custom quantization in ndp-lib or ndp-intelligence.

Rationale:
- ruvector-core includes production-grade PQ with ARM NEON acceleration
- Reimplementing would add ~1 week of effort for an inferior result
- The `SimilarityEngine` trait abstracts this: callers pass `Vec<f32>`, ruvector quantizes internally based on its configuration

In Phase 4, quantization recall is validated by comparing ruvector-core's PQ8 results against exact pgvector search — this confirms the acceleration layer preserves accuracy.

### ReasoningBank Schema Prep

ruvector's ReasoningBank (LoRA + EWC++ for continuous learning) has a defined storage schema. In Phase 2, we create the `gold.reasoning_bank` table structure (empty, unused) so that V1.3 SONA integration doesn't require schema migration:

```sql
CREATE TABLE IF NOT EXISTS gold.reasoning_bank (
    id              SERIAL PRIMARY KEY,
    domain_id       TEXT NOT NULL,
    adapter_name    TEXT NOT NULL,
    adapter_blob    BYTEA,           -- LoRA adapter weights
    ewc_fisher      BYTEA,           -- EWC++ Fisher information
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    performance     JSONB DEFAULT '{}'
);
```

This table is created by the `PgVectorSchemaGenerator` but is not read or written in V1.2.

### SONA / ruv-fann Assessment

ruvector has integrated SONA capability (LoRA + EWC++), which is the same architecture planned for V1.3's continuous learning. This means:

- **ruv-fann is NOT needed** — ruvector's SONA subsumes the neural model capabilities we would need
- V1.2 requires NO neural models (Granger is statistical, K-NN is non-parametric)
- V1.3 can use ruvector-core's SONA directly rather than integrating a separate ruv-fann dependency
- The `reasoning_bank` schema (above) is designed to store SONA artifacts

### Dual-Backend Strategy

Both pgvector AND ruvector-core are always present:
- **pgvector**: source of truth for durable storage, used for backup search
- **ruvector-core**: in-process HNSW for fast (<1ms) search

On startup, the HNSW index is rebuilt from pgvector data. This means pgvector is always consistent, and ruvector-core is an acceleration layer that can be disabled without data loss.

---

## 10. Granger Causality Architecture

### Similarity-Guided Candidate Selection

Instead of testing all O(n^2) field pairs, K-NN results identify which pairs to test:

1. For each prediction, record which neighbor fields correlated with the outcome
2. After 30+ predictions, rank field pairs by co-occurrence frequency
3. Run Granger causality tests only on the top-N candidate pairs

This reduces Granger computation from O(n^2) to O(top_candidates).

### Granger Implementation

Pure Rust, using ndarray for matrix operations:

```rust
pub struct GrangerScanner {
    max_lag: usize,      // Maximum lag to test (default: 6 hours)
    significance: f64,   // P-value threshold (default: 0.05)
    min_samples: usize,  // Minimum samples for valid test (default: 168 = 1 week)
}
```

The scanner reads time-series data directly from Gold aligned view or CAs, computes the Granger F-test, and stores validated relationships as edges in the graph (via `GraphStore`).

### Evidence Accumulation

Each candidate has an `evidence_count` that increments when the relationship is re-validated. Candidates are promoted through stages:
- `candidate` (initial detection)
- `confirmed` (re-validated 3+ times)
- `stable` (confirmed over 14+ days)
- `degraded` (previously stable, failed recent re-validation)

---

## 11. Anomaly Detection

Distance-based anomaly detection using the embedding space:

1. Compute distance from current embedding to its K nearest neighbors
2. If mean distance > threshold (configurable, default 2.5 sigma above historical mean), flag as anomalous
3. Store anomaly flag in prediction metadata

This catches "novel situations" — hours where the sensor state is unlike anything seen before. These are precisely the hours where predictions should be treated with lower confidence.

---

## 12. Grafana Dashboard Design

### Intelligence Overview Panel Set

| Panel | Type | Data Source |
|-------|------|------------|
| Prediction Accuracy (rolling 7d) | Time series | `gold.predictions WHERE correct IS NOT NULL` |
| Predictions by Horizon | Bar chart | `GROUP BY horizon` |
| Anomaly Timeline | Annotations | `WHERE is_anomalous = true` |
| K-NN Confidence Distribution | Histogram | `confidence` from predictions |
| Embedding Index Stats | Stat panel | Count, dimensions, last update |
| Causal Relationships | Table | Graph edges WHERE edge_type = 'causes' (via SQL view or graph query) |

### Causal Graph Panel

A table panel showing validated causal relationships:
- Source stream/field → Target stream/field
- Lag (minutes), correlation strength, Granger p-value
- Evidence count, status, last confirmed date

---

## 13. Error Handling and Resilience

### Intelligence Cycle Failures

| Failure | Behavior |
|---------|----------|
| Database connection lost | Retry with exponential backoff (1s, 2s, 4s, max 60s) |
| Aligned view empty | Skip cycle, log warning, wait for next notification |
| ruvector-core crash | Fall back to pgvector search, log error |
| Embedding generation fails (NULL fields) | Skip row, log warning with field names |
| Prediction storage fails | Retry once, then skip and continue cycle |
| Granger computation exceeds timeout (30s) | Cancel, reduce candidate set, retry next cycle |

### Graceful Degradation

The intelligence binary should never crash the system. If it fails repeatedly:
1. Log errors via tracing
2. Continue attempting cycles on timer
3. Never block or interfere with ingestion pipeline

---

## 14. Testing Strategy

### Unit Tests (in-crate)

| Module | Test Focus |
|--------|-----------|
| MetricEmbedder | Z-score normalization, NULL handling, temporal features, dimension count |
| GoldRow | Field extraction from SQL results, BTreeMap construction |
| GraphStore | Node/edge CRUD, typed traversal, neighbor queries |
| GrangerScanner | Known synthetic series with known causality |
| PredictionEngine | Outcome lookup, confidence calculation, accuracy tracking |
| AnomalyDetector | Distance threshold, sigma calculation |

### Integration Tests

| Test | Method |
|------|--------|
| End-to-end cycle | Seed Gold aligned view with test data, run one cycle, verify predictions stored |
| pgvector round-trip | Insert embedding, search, verify results |
| HNSW rebuild | Insert via pgvector, rebuild HNSW, verify search returns same results |
| Graph round-trip | Add nodes + edges, traverse neighbors, verify consistency |
| Prediction accuracy | Seed known data, generate predictions, seed outcomes, verify evaluation |

### Acceptance Tests (on Pi)

| Test | Criterion |
|------|-----------|
| Container starts | `ndp-intelligence-app daemon` runs without crash |
| Memory budget | Container stays under 256 MB (Phase 1) |
| Cycle latency | Full cycle completes in <500ms |
| Search latency | K-NN search < 1ms for 1000 vectors |
| Prediction after warmup | Predictions generated after 168 hours of data |

---

## 15. Phase 2 Extension Points

The architecture explicitly prepares for Phase 2 (event intelligence) without implementing it:

| Extension Point | How It's Prepared |
|----------------|-------------------|
| EventEmbedder | Embedder trait accepts any implementation |
| CompositeEmbedder | GoldRow can carry text fields as metadata |
| Template cache | Not needed in Phase 1, no placeholder code |
| MiniLM model | Not bundled in Phase 1, container limit stays at 256 MB |
| Quantization | Delegated to ruvector-core's built-in PQ — no custom code needed |
| Tiered retention | Tables support `retention_tier` column when event tables are added |
| Event embeddings table | Schema generator supports it, but DDL not generated until configured |
| Graph domain model | GraphStore built and working in Phase 1-2. Domain-specific node/edge types shaped in Phase 4 when event data exists |
| ReasoningBank schema | Empty `gold.reasoning_bank` table created — ready for SONA in V1.3 |
| SONA / ruv-fann | ruvector has integrated SONA (LoRA + EWC++). No separate ruv-fann dependency needed |

The key principle: later phases should be additive, not refactoring. The Phase 1 architecture should not need modification to support later features.

---

## 16. Dependency Versions

### ndp-intelligence Cargo.toml

```toml
[package]
name = "ndp-intelligence"
version = "0.1.0"
edition = "2021"

[dependencies]
# Vector search + graph
ruvector-core = { version = "2.0.1" }
ruvector-graph = { version = "0.1", optional = true }

# NDP workspace
ndp-types = { path = "../ndp-types" }
ndp-lib = { path = "../ndp-lib" }

# Async
tokio = { version = "1", features = ["full"] }
tokio-postgres = { version = "0.7", features = ["with-serde_json-1", "with-chrono-0_4"] }
async-trait = "0.1"

# Math
ndarray = "0.16"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }

# Logging
tracing = "0.1"

# Error handling
thiserror = "1.0"
```

### ndp-intelligence-app Cargo.toml

```toml
[package]
name = "ndp-intelligence-app"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "ndp-intelligence-app"
path = "src/main.rs"

[dependencies]
ndp-intelligence = { path = "../../crates/ndp-intelligence" }
ndp-lib = { path = "../../crates/ndp-lib" }
ndp-types = { path = "../../crates/ndp-types" }
tokio = { version = "1", features = ["full"] }
tokio-postgres = { version = "0.7", features = ["with-serde_json-1", "with-chrono-0_4"] }
clap = { version = "4", features = ["derive"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
```

---

## 17. Memory Budget (Phase 1)

| Component | Memory | Notes |
|-----------|--------|-------|
| ndp-intelligence-app binary | ~15 MB | Rust binary base |
| ruvector-core HNSW index | ~2 MB | For ~10K 32D vectors (1+ year of hourly data) |
| ruvector-graph (or SQL graph) | ~1 MB | Small node/edge set at V1.2 scale |
| Running z-score statistics | <1 MB | Per-field mean/std |
| Tokio runtime | ~10 MB | Async executor |
| PostgreSQL connection pool | ~5 MB | 2-3 connections |
| **Total** | **~34 MB** | Well within 256 MB limit |

Phase 2 adds MiniLM (~200 MB on demand), pushing the limit to 512 MB.

---

*Architecture grounded in the existing ndp-lib Gold layer, extending it with embedding and intelligence capabilities while preserving the config-driven, trait-based design.*
