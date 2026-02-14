# fe-003 IMPLEMENTATION-BRIEF: Intelligence Foundation Phase 0 + Phase 1

> **Version target**: v1.2.0 (library-only, no Pi deployment)
> **GitHub Issue**: #17
> **Date**: 2026-02-14

## SPARC Planning Artifacts

For full details beyond this brief, read the complete SPARC artifacts:

| Artifact | Path |
|----------|------|
| **Scope** | `product/features/fe-003/SCOPE.md` |
| **Specification** | `product/features/fe-003/specification/SPECIFICATION.md` |
| **Task Decomposition** | `product/features/fe-003/specification/TASK-DECOMPOSITION.md` |
| **Architecture (ADRs)** | `product/features/fe-003/architecture/ARCHITECTURE.md` |
| **Pseudocode** | `product/features/fe-003/pseudocode/PSEUDOCODE.md` |
| **Alignment Report** | `product/features/fe-003/ALIGNMENT-REPORT.md` |
| **Parent Architecture** | `product/features/gold-002/ARCHITECTURE.md` |
| **Parent Roadmap** | `product/features/gold-002/IMPLEMENTATION-ROADMAP.md` |

---

## 1. Goal

Implement Phase 0 (Go/No-Go Gate) and Phase 1 (Foundation) of the V1.2 Intelligence Foundation. Phase 0 proves ruvector-core and ruvector-graph compile on aarch64. Phase 1 builds `ndp-intelligence` crate, database schema, Embedder trait, MetricEmbedder, and pgvector-backed storage that all subsequent intelligence phases depend on. This is library-only -- no runtime daemon, no Pi deployment.

---

## 2. Resolved Decisions

| Decision | Resolution | Source |
|----------|-----------|--------|
| ndarray dependency | DEFER to Phase 3. Do NOT add to workspace deps in Phase 1. | User decision |
| NullStrategy variants | Include ALL 3: `Zero`, `LastKnown`, `Mean`. `LastKnown` requires `last_known: HashMap<String, f64>` on MetricEmbedder. | User decision |
| RunningStats filename | Use `stats.rs` (not `running_stats.rs`) | User decision (3-of-4 artifact consensus) |
| Predictions DDL `created_at` | Include `created_at TIMESTAMPTZ DEFAULT NOW()` column | User decision |
| Pending outcomes filter | Use `actual_value IS NULL` (not `correct IS NULL`) | User decision |
| EmbeddingWriter location | Lives in `ndp-intelligence` crate (`crates/ndp-intelligence/src/populator/`) | User decision (ARCH ADR resolved) |
| GoldRow fields | `BTreeMap<String, Option<f64>>` -- deterministic ordering | ADR-001 |
| Embedding vector type | `Vec<f32>` (not f64) -- 50% memory savings | ADR-001 |
| Z-score approach | Running stats with exponential decay, alpha=0.01, warmup=168 | ADR-002 |
| Dual backend | pgvector durable + HNSW acceleration (Phase 2). Phase 1: pgvector only. | ADR-003 |
| Graph backend | ruvector-graph preferred (feature-gated). SQL adjacency fallback always compiled. Phase 0 decides. | ADR-004 |
| PgVectorSchemaGenerator | Fully config-driven: reads `IntelligenceConfig`, derives dimensions from field count, conditionally generates graph tables. No hardcoded DDL files. | ADR-005 + User decision |
| ndp-lib::gold::populator/ | NOT created. EmbeddingWriter moved to ndp-intelligence. | ARCH section 1.6 |

---

## 3. Files to Create

| Path | Description |
|------|-------------|
| `crates/ndp-intelligence/Cargo.toml` | Intelligence library crate manifest |
| `crates/ndp-intelligence/src/lib.rs` | Crate root, module declarations, public re-exports |
| `crates/ndp-intelligence/src/error.rs` | `IntelligenceError` enum (thiserror) |
| `crates/ndp-intelligence/src/similarity/mod.rs` | `SimilarityEngine` trait stub + `VectorEntry`, `SearchQuery`, `SearchResult` types |
| `crates/ndp-intelligence/src/graph/mod.rs` | `GraphStore` trait, `GraphNode`, `GraphEdge` types, factory fn |
| `crates/ndp-intelligence/src/graph/sql.rs` | `SqlGraphStore` implementation (SQL adjacency tables) |
| `crates/ndp-intelligence/src/graph/ruvector.rs` | `RuvectorGraphStore` (conditional: `cfg(feature = "ruvector-graph")`) |
| `crates/ndp-intelligence/src/storage/mod.rs` | `StorageBackend` trait, `StoredEmbedding`, `Prediction`, `ActualOutcome` types |
| `crates/ndp-intelligence/src/storage/postgres.rs` | `PostgresStorage` implementation |
| `crates/ndp-intelligence/src/populator/mod.rs` | Populator module, re-exports |
| `crates/ndp-intelligence/src/populator/embedding_writer.rs` | `EmbeddingWriter<S: StorageBackend>` |
| `apps/ndp-intelligence-app/Cargo.toml` | Intelligence app binary manifest |
| `apps/ndp-intelligence-app/src/main.rs` | Clap CLI stub (daemon/one-shot/backfill/status subcommands) |
| `crates/ndp-lib/src/gold/embeddings/mod.rs` | `Embedder` trait, `GoldRow`, `Embedding`, `EmbeddingError` types |
| `crates/ndp-lib/src/gold/embeddings/metric.rs` | `MetricEmbedder`, `EmbeddingField`, `FieldSource`, `NullStrategy`, `TemporalEncoding` |
| `crates/ndp-lib/src/gold/embeddings/stats.rs` | `RunningStats` (exponential decay mean/std) |
| `crates/ndp-lib/src/gold/embeddings/config.rs` | `IntelligenceConfig`, `EmbeddingConfig`, `SearchConfig`, `AnomalyConfig`, `EmbeddingFieldsConfig` |
| `crates/ndp-lib/src/gold/generators/pgvector_schema.rs` | `PgVectorSchemaGenerator` |
| `deploy/pi/init-scripts/006_pgvector_extension.sql` | pgvector extension init SQL |
| `docker/integration-timescaledb/Dockerfile` | Integration env TimescaleDB + pgvector |
| `product/features/fe-003/reports/phase0-go-no-go.md` | Phase 0 output report |

## 4. Files to Modify

| Path | Change |
|------|--------|
| `Cargo.toml` (workspace root) | Add `"crates/ndp-intelligence"` and `"apps/ndp-intelligence-app"` to workspace members. Do NOT add ndarray. |
| `crates/ndp-lib/src/gold/mod.rs` | Add `pub mod embeddings;` |
| `crates/ndp-lib/src/gold/config/mod.rs` | Add `pub mod intelligence;` and re-export types |
| `crates/ndp-lib/src/gold/config/domain.rs` | Add `#[serde(default)] pub intelligence: Option<IntelligenceConfig>` field to `DomainConfig` |
| `crates/ndp-lib/src/gold/generators/mod.rs` | Add `pub mod pgvector_schema;` and re-export `PgVectorSchemaGenerator` |
| `tools/ndp-cli/src/commands/gold.rs` | Add `Intelligence` variant to `GoldCommands` enum with `Schema` subcommand |
| `docker-compose.integration.yml` | Update timescaledb service to use custom Dockerfile with pgvector |

---

## 5. Data Structures

### ndp-lib types (`crates/ndp-lib/src/gold/embeddings/`)

```rust
// mod.rs
use chrono::{DateTime, Utc};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, thiserror::Error)]
pub enum EmbeddingError {
    #[error("Field '{field}' not found in GoldRow")]
    FieldNotFound { field: String },
    #[error("Insufficient data for embedding: {reason}")]
    InsufficientData { reason: String },
    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },
}

pub type EmbeddingResult<T> = std::result::Result<T, EmbeddingError>;

#[derive(Debug, Clone)]
pub struct GoldRow {
    pub bucket: DateTime<Utc>,
    pub domain_id: String,
    pub fields: BTreeMap<String, Option<f64>>,
}

#[derive(Debug, Clone)]
pub struct Embedding {
    pub vector: Vec<f32>,
    pub dimensions: usize,
    pub metadata: HashMap<String, serde_json::Value>,
}
```

```rust
// metric.rs
pub struct MetricEmbedder {
    fields: Vec<EmbeddingField>,
    stats: HashMap<String, RunningStats>,
    dimensions: usize,
    warmup_threshold: usize,
    observations: usize,
    last_known: HashMap<String, f64>,  // for NullStrategy::LastKnown
}

#[derive(Debug, Clone)]
pub struct EmbeddingField {
    pub name: String,
    pub source: FieldSource,
    pub null_strategy: NullStrategy,
}

#[derive(Debug, Clone)]
pub enum FieldSource {
    Direct(String),
    Temporal(TemporalEncoding),
}

#[derive(Debug, Clone)]
pub enum TemporalEncoding {
    HourSin,
    HourCos,
    IsWeekend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NullStrategy {
    Zero,
    LastKnown,
    Mean,
}
```

```rust
// stats.rs
#[derive(Debug, Clone)]
pub struct RunningStats {
    mean: f64,
    variance: f64,
    count: usize,
    alpha: f64,
}
```

### Config types (`crates/ndp-lib/src/gold/embeddings/config.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntelligenceConfig {
    pub enabled: bool,
    pub embedding: EmbeddingConfig,
    pub search: SearchConfig,
    #[serde(default)]
    pub anomaly: Option<AnomalyConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    #[serde(rename = "type")]
    pub embedding_type: EmbeddingType,
    pub fields: EmbeddingFieldsConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingType { Metric }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingFieldsConfig {
    #[serde(default)]
    pub temporal: Vec<String>,
    #[serde(default)]
    pub direct: Vec<DirectFieldConfig>,
    #[serde(default)]
    pub derived: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectFieldConfig {
    pub field: String,
    pub null_strategy: NullStrategyConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NullStrategyConfig { Zero, LastKnown, Mean }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    pub k: usize,
    pub min_similarity: f64,
    pub prediction_horizons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyConfig {
    pub enabled: bool,
    pub distance_threshold_sigma: f64,
}
```

### ndp-intelligence types

```rust
// similarity/mod.rs -- TRAIT ONLY, no implementation in Phase 1
#[derive(Debug, Clone)]
pub struct VectorEntry {
    pub id: String,
    pub vector: Vec<f32>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct SearchQuery {
    pub vector: Vec<f32>,
    pub k: usize,
    pub min_similarity: f64,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub id: String,
    pub similarity: f64,
    pub metadata: serde_json::Value,
}
```

```rust
// storage/mod.rs
#[derive(Debug, Clone)]
pub struct StoredEmbedding {
    pub bucket: DateTime<Utc>,
    pub domain_id: String,
    pub embedding: Vec<f32>,
    pub dimensions: usize,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct Prediction {
    pub id: Option<i64>,
    pub bucket: DateTime<Utc>,
    pub domain_id: String,
    pub metric: String,
    pub horizon: String,
    pub predicted_value: Option<f64>,
    pub predicted_breach: Option<bool>,
    pub confidence: f64,
    pub k_neighbors: i32,
    pub k_supporting: i32,
    pub actual_value: Option<f64>,
    pub actual_breach: Option<bool>,
    pub correct: Option<bool>,
    pub evaluated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct ActualOutcome {
    pub actual_value: f64,
    pub actual_breach: bool,
    pub evaluated_at: DateTime<Utc>,
}
```

```rust
// graph/mod.rs
#[derive(Debug, Clone)]
pub struct GraphNode {
    pub id: String,
    pub node_type: String,
    pub properties: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct GraphEdge {
    pub source_id: String,
    pub target_id: String,
    pub edge_type: String,
    pub weight: f64,
    pub properties: serde_json::Value,
    pub created_at: DateTime<Utc>,
}
```

---

## 6. Trait Signatures

### Embedder (ndp-lib -- full, implemented in Phase 1)

```rust
pub trait Embedder: Send + Sync {
    fn embed(&self, row: &GoldRow) -> EmbeddingResult<Embedding>;
    fn dimensions(&self) -> usize;
    fn name(&self) -> &str;
}
```

### SimilarityEngine (ndp-intelligence -- stub, NOT implemented in Phase 1)

```rust
pub trait SimilarityEngine: Send + Sync {
    fn insert(&mut self, entry: VectorEntry) -> Result<(), SimilarityError>;
    fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, SimilarityError>;
    fn count(&self) -> usize;
}

#[derive(Debug, thiserror::Error)]
pub enum SimilarityError {
    #[error("Dimension mismatch: index expects {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },
    #[error("Index is empty")]
    EmptyIndex,
    #[error("Backend error: {0}")]
    Backend(String),
}
```

### StorageBackend (ndp-intelligence -- full, implemented in Phase 1)

```rust
#[async_trait]
pub trait StorageBackend: Send + Sync {
    async fn store_embedding(&self, embedding: &StoredEmbedding) -> Result<(), StorageError>;
    async fn load_embeddings(&self, domain_id: &str, since: Option<DateTime<Utc>>)
        -> Result<Vec<StoredEmbedding>, StorageError>;
    async fn store_prediction(&self, prediction: &Prediction) -> Result<i64, StorageError>;
    async fn get_pending_outcomes(&self, domain_id: &str) -> Result<Vec<Prediction>, StorageError>;
    async fn record_outcome(&self, prediction_id: i64, actual: &ActualOutcome)
        -> Result<(), StorageError>;
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Record not found: {entity} with id {id}")]
    NotFound { entity: String, id: String },
}
```

### GraphStore (ndp-intelligence -- full, implemented in Phase 1)

```rust
#[async_trait]
pub trait GraphStore: Send + Sync {
    async fn add_node(&self, node: &GraphNode) -> Result<(), GraphError>;
    async fn add_edge(&self, edge: &GraphEdge) -> Result<(), GraphError>;
    async fn get_edges(&self, node_id: &str, edge_type: Option<&str>)
        -> Result<Vec<GraphEdge>, GraphError>;
    async fn get_neighbors(&self, node_id: &str, edge_type: Option<&str>)
        -> Result<Vec<GraphNode>, GraphError>;
    async fn node_count(&self, node_type: Option<&str>) -> Result<usize, GraphError>;
    async fn edge_count(&self, edge_type: Option<&str>) -> Result<usize, GraphError>;
}

#[derive(Debug, thiserror::Error)]
pub enum GraphError {
    #[error("Node not found: {id}")]
    NodeNotFound { id: String },
    #[error("Edge references non-existent node: {node_id}")]
    DanglingEdge { node_id: String },
    #[error("Backend error: {0}")]
    Backend(String),
}
```

---

## 7. SQL DDL (Expected Output — NOT Hardcoded)

**CRITICAL**: The SQL below is the **expected output** of `PgVectorSchemaGenerator`, not static SQL files. The generator MUST read the domain's `IntelligenceConfig` and dynamically produce DDL based on config values:

- **Vector dimensions**: derived from the field count in `EmbeddingConfig.fields` (temporal + direct + derived). Use `vector({dimensions})` not bare `vector`.
- **Table names**: could be parameterized by domain_id if needed for multi-domain isolation.
- **Graph tables**: conditionally generated based on `include_graph_tables` flag (Phase 0 outcome).
- **Hypertable chunk intervals**: could be config-driven in future; use sensible defaults now.
- **Indexes**: generated, not hand-written SQL files.

No hardcoded DDL files exist in the codebase. All schema is produced by the generator at runtime from config. The SQL below shows what the generator should produce for the indoor-air-quality domain config.

### gold.metric_embeddings (hypertable)

```sql
CREATE EXTENSION IF NOT EXISTS vector;
CREATE SCHEMA IF NOT EXISTS gold;

CREATE TABLE IF NOT EXISTS gold.metric_embeddings (
    bucket          TIMESTAMPTZ NOT NULL,
    domain_id       TEXT NOT NULL,
    embedding       vector,
    dimensions      INTEGER NOT NULL,
    metadata        JSONB DEFAULT '{}',
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (bucket, domain_id)
);

SELECT create_hypertable('gold.metric_embeddings', 'bucket',
    chunk_time_interval => INTERVAL '7 days',
    if_not_exists => TRUE);

CREATE INDEX IF NOT EXISTS idx_metric_embeddings_domain
    ON gold.metric_embeddings(domain_id, bucket DESC);
```

### gold.predictions (hypertable, with created_at)

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
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (id, bucket)
);

SELECT create_hypertable('gold.predictions', 'bucket',
    chunk_time_interval => INTERVAL '30 days',
    if_not_exists => TRUE);

CREATE INDEX IF NOT EXISTS idx_predictions_domain_metric
    ON gold.predictions(domain_id, metric, bucket DESC);

CREATE INDEX IF NOT EXISTS idx_predictions_pending
    ON gold.predictions(domain_id, bucket)
    WHERE actual_value IS NULL;
```

### gold.graph_nodes

```sql
CREATE TABLE IF NOT EXISTS gold.graph_nodes (
    id              TEXT PRIMARY KEY,
    node_type       TEXT NOT NULL,
    properties      JSONB DEFAULT '{}',
    created_at      TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_graph_nodes_type
    ON gold.graph_nodes(node_type);
```

### gold.graph_edges

```sql
CREATE TABLE IF NOT EXISTS gold.graph_edges (
    id              SERIAL PRIMARY KEY,
    source_id       TEXT NOT NULL REFERENCES gold.graph_nodes(id),
    target_id       TEXT NOT NULL REFERENCES gold.graph_nodes(id),
    edge_type       TEXT NOT NULL,
    weight          DOUBLE PRECISION DEFAULT 1.0,
    properties      JSONB DEFAULT '{}',
    created_at      TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_graph_edges_source
    ON gold.graph_edges(source_id, edge_type);
CREATE INDEX IF NOT EXISTS idx_graph_edges_target
    ON gold.graph_edges(target_id, edge_type);
```

### gold.reasoning_bank (V1.3 prep, empty)

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

CREATE INDEX IF NOT EXISTS idx_reasoning_bank_domain
    ON gold.reasoning_bank(domain_id);
```

---

## 8. Implementation Waves

### Wave 0: Go/No-Go Gate (1 day)

| Task | Description | Agent |
|------|-------------|-------|
| P0-01 | Minimal Rust project with `ruvector-core = "2.0.1"` + `ruvector-graph = "0.1"`, compile for aarch64 | ndp-rust-dev |
| P0-02 | Run binary on Pi 5, no crash | ndp-rust-dev |
| P0-03 | ruvector-core smoke: 100 vectors, K-NN search, verify correctness | ndp-rust-dev |
| P0-04 | ruvector-graph smoke: nodes + edges + traversal | ndp-rust-dev |
| P0-05 | Measure memory/latency/build-time, write go/no-go report | ndp-rust-dev |

**Deps**: None. Output: `product/features/fe-003/reports/phase0-go-no-go.md` with backend decisions.

### Wave 1: Foundation Types (no DB deps)

| Task | Description | Agent | Deps |
|------|-------------|-------|------|
| P1-01 | Create `crates/ndp-intelligence` crate skeleton | ndp-rust-dev | Wave 0 |
| P1-02 | Create `apps/ndp-intelligence-app` with clap CLI | ndp-rust-dev | P1-01 |
| P1-03 | Embedder trait + GoldRow + Embedding types | ndp-rust-dev | P1-01 |
| P1-05 | RunningStats in `stats.rs` | ndp-rust-dev | P1-03 |
| P1-06 | EmbeddingConfig types in `config.rs` | ndp-rust-dev | P1-03 |

**Parallelism**: P1-01/P1-02 first (skeleton), then P1-03/P1-05/P1-06 in parallel.

### Wave 2: Config + Generators

| Task | Description | Agent | Deps |
|------|-------------|-------|------|
| P1-04 | MetricEmbedder implementation | ndp-rust-dev | P1-03, P1-05, P1-06 |
| P1-07 | DomainConfig `intelligence` extension | ndp-rust-dev | P1-06 |
| P1-08 | PgVectorSchemaGenerator | ndp-rust-dev | P1-06 |

**Parallelism**: All three independent, can run in parallel.

### Wave 3: Storage + Graph (needs integration env)

| Task | Description | Agent | Deps |
|------|-------------|-------|------|
| P1-09 | pgvector in TimescaleDB Docker | ndp-rust-dev | None (infra) |
| P1-10 | StorageBackend + PostgresStorage | ndp-rust-dev | P1-01, P1-03, P1-09 |
| P1-11 | GraphStore + backend (per Phase 0 outcome) | ndp-rust-dev | P1-01, Wave 0, P1-09 |

**Parallelism**: P1-09 first (quick), then P1-10/P1-11 in parallel.

### Wave 4: Integration

| Task | Description | Agent | Deps |
|------|-------------|-------|------|
| P1-12 | EmbeddingWriter in ndp-intelligence | ndp-rust-dev | P1-04, P1-10 |
| P1-13 | `ndp gold intelligence schema` CLI subcommand | ndp-rust-dev | P1-08 |

**Parallelism**: P1-12 and P1-13 are independent.

---

## 9. Test Requirements

### Unit Tests (dev container, no DB)

| Component | Test | Assert |
|-----------|------|--------|
| GoldRow (P1-03) | Construction with mixed Some/None | BTreeMap ordering, field access |
| GoldRow (P1-03) | Field ordering is deterministic | Iteration order matches sort |
| Embedding (P1-03) | Construction + dimension check | vector.len() == dimensions |
| Embedder trait (P1-03) | Object safety | `dyn Embedder` compiles |
| RunningStats (P1-05) | Single observation | mean == value, std == 0.0 |
| RunningStats (P1-05) | Known series [1..5] | mean ~3.0 within tolerance |
| RunningStats (P1-05) | Z-score of mean returns ~0.0 | Absolute error < 0.01 |
| RunningStats (P1-05) | Exponential decay convergence | After 200 constant values, mean == value |
| RunningStats (P1-05) | Count increments | count() matches update() calls |
| MetricEmbedder (P1-04) | InsufficientData before warmup | Returns Err |
| MetricEmbedder (P1-04) | Temporal hour=0 | sin=0.0, cos=1.0 |
| MetricEmbedder (P1-04) | Temporal hour=6 | sin=1.0, cos=0.0 |
| MetricEmbedder (P1-04) | Weekend detection | Sat/Sun=1.0, Mon=0.0 |
| MetricEmbedder (P1-04) | Z-score known values | Exact expected output |
| MetricEmbedder (P1-04) | NullStrategy::Zero | None -> 0.0 |
| MetricEmbedder (P1-04) | NullStrategy::Mean | None -> 0.0 (in z-score space) |
| MetricEmbedder (P1-04) | NullStrategy::LastKnown | None -> z_score(last_value) |
| MetricEmbedder (P1-04) | Dimension count | Output vector len matches config |
| MetricEmbedder (P1-04) | from_config() | Parses EmbeddingConfig correctly |
| EmbeddingConfig (P1-06) | Full JSON deserialization | All fields parsed |
| EmbeddingConfig (P1-06) | Omitted anomaly defaults to None | serde(default) works |
| EmbeddingConfig (P1-06) | Round-trip serialize/deserialize | All fields preserved |
| EmbeddingConfig (P1-06) | NullStrategyConfig serialization | "zero" / "last_known" / "mean" |
| DomainConfig (P1-07) | Existing tests unchanged | Zero regressions |
| DomainConfig (P1-07) | Without intelligence key | intelligence == None |
| DomainConfig (P1-07) | With intelligence block | intelligence == Some(...) |
| PgVectorSchemaGen (P1-08) | Action::Sync output | Contains `IF NOT EXISTS` |
| PgVectorSchemaGen (P1-08) | Extension DDL | Contains `CREATE EXTENSION IF NOT EXISTS vector` |
| PgVectorSchemaGen (P1-08) | Hypertable calls | Contains `create_hypertable` for embeddings + predictions |
| PgVectorSchemaGen (P1-08) | Graph tables included | Contains `gold.graph_nodes` when flag=true |
| PgVectorSchemaGen (P1-08) | Graph tables excluded | Omits graph tables when flag=false |
| PgVectorSchemaGen (P1-08) | Predictions has created_at | DDL contains `created_at TIMESTAMPTZ DEFAULT NOW()` |
| PgVectorSchemaGen (P1-08) | Reasoning bank | Contains `adapter_blob BYTEA` and `ewc_fisher BYTEA` |
| CLI (P1-13) | `--help` output | Shows `intelligence schema` subcommand |
| CLI (P1-13) | Schema output for domain | Contains `CREATE TABLE IF NOT EXISTS gold.metric_embeddings` |

### Integration Tests (TimescaleDB + pgvector required, `#[ignore]`)

| Component | Test | Assert |
|-----------|------|--------|
| pgvector (P1-09) | Extension load | `pg_extension WHERE extname = 'vector'` returns row |
| PostgresStorage (P1-10) | store + load embedding round-trip | Vector data matches exactly |
| PostgresStorage (P1-10) | Upsert on conflict | Single row after two inserts for same bucket |
| PostgresStorage (P1-10) | load with `since` filter | Only returns newer records |
| PostgresStorage (P1-10) | store_prediction returns ID | ID > 0 |
| PostgresStorage (P1-10) | get_pending_outcomes | Returns predictions where `actual_value IS NULL` and horizon elapsed |
| PostgresStorage (P1-10) | record_outcome | Sets correct, actual_value, evaluated_at |
| SqlGraphStore (P1-11) | add_node + node_count | Count == 1 |
| SqlGraphStore (P1-11) | add_node upsert | Properties updated, count still 1 |
| SqlGraphStore (P1-11) | add_edge + edge_count | Count == 1 |
| SqlGraphStore (P1-11) | add_edge dangling source | Returns GraphError::DanglingEdge |
| SqlGraphStore (P1-11) | get_edges by type | Filtered correctly |
| SqlGraphStore (P1-11) | get_neighbors 1-hop | Returns connected nodes |
| SqlGraphStore (P1-11) | node_count with type filter | Correct count |
| SqlGraphStore (P1-11) | edge_count with type filter | Correct count |
| EmbeddingWriter (P1-12) | Write single + read back | Vector matches |
| EmbeddingWriter (P1-12) | Write batch of 10 | Count matches |
| EmbeddingWriter (P1-12) | Idempotent upsert | 1 record after 2 writes for same bucket |

---

## 10. Constraints

- **No ndarray** -- deferred to Phase 3
- **No runtime intelligence cycle** -- Phase 2 scope
- **No Pi deployment** -- library-only, tests run in dev/integration env
- **ARM64 compilation required** -- Phase 0 validates; all code must compile for `aarch64-unknown-linux-gnu`
- **Follow existing ndp-lib patterns** -- parsed structs not file paths; ConfigLoader for config; generator pattern for DDL; thiserror for errors; workspace deps
- **`intelligence` field in DomainConfig must be `Option` with `#[serde(default)]`** -- existing tests must pass unchanged
- **No modifications outside scope** -- only ndp-lib Gold extensions, new crates, CLI, Docker
- **No `todo!()`, `unimplemented!()`, or placeholder functions** -- app stubs print message and exit 0
- **Phase 0 memory measurement uses `/proc/self/status` VmRSS** -- never jemalloc (crashes on Pi 5)
- **All DDL is config-driven** -- `PgVectorSchemaGenerator` reads `IntelligenceConfig` and produces DDL dynamically. No hardcoded SQL files. Vector dimensions derived from field count. Graph tables conditional on Phase 0 outcome.
- **904+ existing tests must pass after all changes** -- run `cargo test --workspace`

---

## 11. NOT in Scope

- Runtime intelligence cycle (Phase 2)
- SimilarityEngine implementation (Phase 2 -- trait only defined here)
- PredictionEngine (Phase 2)
- Docker container for intelligence daemon (Phase 2)
- EventEmbedder / MiniLM text pipeline (Phase 4)
- Granger causality (Phase 3)
- Anomaly detection (Phase 5)
- Grafana dashboards (Phase 5)
- SONA / ruv-fann integration (V1.3)
- ndarray dependency (Phase 3)
- Modifying `config/domains/indoor-air-quality/domain.json` (intelligence block added in Phase 2 when runtime exists)

---

## 12. Alignment Status

From `product/features/fe-003/ALIGNMENT-REPORT.md`:

| Principle | Status |
|-----------|--------|
| Edge-Only | PASS |
| Config-Driven | PASS |
| Domain-Portable | PASS |
| Resource-Constrained | WARN (resolved: ndarray deferred per user decision) |
| Integration-First | PASS |
| Privacy by Architecture | PASS |
| Self-Learning | PASS |

**Overall**: 6 PASS, 1 WARN (resolved). 0 VARIANCE, 0 FAIL.

All cross-artifact consistency issues resolved by user decisions in section 2.
