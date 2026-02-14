# fe-003: Intelligence Foundation Architecture

> **SPARC Phase:** Architecture
> **Parent scope:** `product/features/fe-003/SCOPE.md`
> **Gold-002 architecture:** `product/features/gold-002/ARCHITECTURE.md`
> **Gold-002 roadmap:** `product/features/gold-002/IMPLEMENTATION-ROADMAP.md`
> **Created:** 2026-02-14

This document specifies the implementation architecture for Phase 0 (Go/No-Go Gate) and Phase 1 (Foundation) of the V1.2 Intelligence Foundation. It contains the module layout, trait definitions, database schemas, data flow diagrams, integration points, decision tree, and risk assessment that guide implementation agents.

---

## Table of Contents

1. [Module Architecture](#1-module-architecture)
2. [Trait Design (ADRs)](#2-trait-design-adrs)
3. [Data Flow Diagrams](#3-data-flow-diagrams)
4. [Database Schema Design](#4-database-schema-design)
5. [Integration Points](#5-integration-points)
6. [Phase 0 Decision Tree](#6-phase-0-decision-tree)
7. [Risk Assessment](#7-risk-assessment)

---

## 1. Module Architecture

### 1.1 Crate Dependency Graph

Phase 1 adds two new workspace members and extends ndp-lib. The dependency graph is strictly layered -- intelligence depends on ndp-lib but never on ingestion code.

```
                       ndp-types
                      /    |     \
                 ndp-lib   |   config-client
                /    |     |
       ndp-intelligence    |
                \          |
          ndp-intelligence-app
```

Forbidden dependencies (enforced by review, not by Cargo):
- `ndp-intelligence` must NOT depend on `core`, `air-quality-app`, `silver-etl`, or any `domains/*` crate
- `ndp-intelligence` must NOT depend on `config-client` (etcd is a future concern, V1.3)
- `ndp-lib` must NOT gain a dependency on `ndp-intelligence` (no circular deps)

### 1.2 Workspace Cargo.toml Changes

Add two new members to `/workspaces/neural-data-platform/Cargo.toml`:

```toml
[workspace]
members = [
    # ... existing members ...
    "crates/ndp-intelligence",
    "apps/ndp-intelligence-app",
]
```

Add shared workspace dependencies used by intelligence crates:

```toml
[workspace.dependencies]
# ... existing ...
ndarray = "0.16"
```

### 1.3 Module Tree: `crates/ndp-intelligence/src/`

```
crates/ndp-intelligence/
  Cargo.toml
  src/
    lib.rs                       # Public API surface, re-exports
    error.rs                     # IntelligenceError enum (thiserror)
    config.rs                    # IntelligenceConfig, EmbeddingConfig,
                                 #   SearchConfig, AnomalyConfig
    storage/
      mod.rs                     # StorageBackend trait definition
      postgres.rs                # PostgresStorage: pgvector INSERT/SELECT
                                 #   for embeddings and predictions
    graph/
      mod.rs                     # GraphStore trait definition,
                                 #   GraphNode, GraphEdge types,
                                 #   dispatch factory function
      ruvector.rs                # RuvectorGraphStore (conditional: cfg(feature = "ruvector-graph"))
      sql.rs                     # SqlGraphStore (SQL adjacency tables, always compiled)
```

File responsibilities:

| File | P1 Task | Responsibility |
|------|---------|----------------|
| `lib.rs` | P1-01 | Crate root. Declares modules, re-exports public types. No logic. |
| `error.rs` | P1-01 | `IntelligenceError` with variants: `Config`, `Storage`, `Graph`, `Embedding`, `Database`. Uses `thiserror`. |
| `config.rs` | P1-06 | `IntelligenceConfig`, `EmbeddingConfig`, `EmbeddingFieldsConfig`, `DirectFieldConfig`, `SearchConfig`, `AnomalyConfig`. All `Deserialize + Serialize + Clone + Debug`. |
| `storage/mod.rs` | P1-10 | `StorageBackend` async trait. Methods: `store_embedding`, `load_embeddings`, `store_prediction`, `get_pending_outcomes`, `record_outcome`. |
| `storage/postgres.rs` | P1-10 | `PostgresStorage` struct wrapping `tokio_postgres::Client`. Implements `StorageBackend`. Uses `pgvector` SQL for vector INSERT/SELECT. |
| `graph/mod.rs` | P1-11 | `GraphStore` async trait, `GraphNode`/`GraphEdge` structs, `create_graph_store()` factory. |
| `graph/ruvector.rs` | P1-11 | `RuvectorGraphStore` behind `#[cfg(feature = "ruvector-graph")]`. Wraps ruvector-graph crate. |
| `graph/sql.rs` | P1-11 | `SqlGraphStore`. Implements `GraphStore` via SQL INSERT/SELECT against `gold.graph_nodes`/`gold.graph_edges`. Always available. |

### 1.4 Module Tree: `crates/ndp-lib/src/gold/` Extensions

New modules added to the existing Gold tree at `/workspaces/neural-data-platform/crates/ndp-lib/src/gold/`:

```
crates/ndp-lib/src/gold/
  mod.rs                         # MODIFIED: add `pub mod embeddings;` and `pub mod populator;`
  embeddings/                    # NEW module
    mod.rs                       # Embedder trait, GoldRow, Embedding types, EmbeddingField,
                                 #   FieldSource, NullStrategy, TemporalEncoding
    metric.rs                    # MetricEmbedder: z-score normalize + temporal encode
    running_stats.rs             # RunningStats: exponential decay mean/std tracker
  populator/                     # NEW module
    mod.rs                       # Populator trait (future extensibility)
    embedding_writer.rs          # EmbeddingWriter: writes Embedding to StorageBackend
  generators/
    mod.rs                       # MODIFIED: add `pub mod pgvector_schema;` and re-export
    pgvector_schema.rs           # NEW: PgVectorSchemaGenerator
```

File responsibilities:

| File | P1 Task | Responsibility |
|------|---------|----------------|
| `embeddings/mod.rs` | P1-03 | `Embedder` trait (sync, no DB deps), `GoldRow`, `Embedding`, `EmbeddingField`, `FieldSource`, `NullStrategy`, `TemporalEncoding` enums. Pure types and trait. |
| `embeddings/metric.rs` | P1-04 | `MetricEmbedder` struct. Configured from `EmbeddingConfig`. Implements `Embedder`. Z-score normalization, temporal sin/cos encoding, NULL handling per field strategy. |
| `embeddings/running_stats.rs` | P1-05 | `RunningStats` struct. Tracks per-field running mean and standard deviation with exponential decay (alpha=0.01). Methods: `update(value)`, `z_score(value) -> f64`, `is_warmed_up() -> bool`. Warmup threshold: 168 samples (1 week hourly). |
| `populator/mod.rs` | P1-12 | `Populator` trait definition (async: `fn populate(&self, ...) -> Result<usize>`). |
| `populator/embedding_writer.rs` | P1-12 | `EmbeddingWriter` struct. Takes a `StorageBackend` reference. Converts `Embedding` to storage format, calls `store_embedding`. |
| `generators/pgvector_schema.rs` | P1-08 | `PgVectorSchemaGenerator`. Follows `ContinuousAggregateGenerator` pattern. Method: `generate(&self, domain_config: &DomainConfig, action: Action) -> Result<String>`. Produces DDL for all intelligence tables. |

### 1.5 Module Tree: `apps/ndp-intelligence-app/src/`

```
apps/ndp-intelligence-app/
  Cargo.toml
  src/
    main.rs                      # clap CLI: --help, version. Subcommands: daemon, one-shot,
                                 #   backfill, status. Phase 1 is stub-only (no runtime logic).
```

Phase 1 deliverable (P1-02): the binary compiles, `--help` prints usage, subcommand parsing works. No runtime intelligence logic -- that is Phase 2.

### 1.6 Crate Cargo.toml Specifications

#### `crates/ndp-intelligence/Cargo.toml`

```toml
[package]
name = "ndp-intelligence"
version = "0.1.0"
edition = "2021"

[features]
default = []
ruvector-graph = ["dep:ruvector-graph"]

[dependencies]
# NDP workspace
ndp-types = { path = "../ndp-types" }
ndp-lib = { path = "../ndp-lib" }

# Vector search (Phase 2 will add ruvector-core)
# ruvector-core = { version = "2.0.1" }   # Added in Phase 2 after Phase 0 gate

# Graph storage
ruvector-graph = { version = "0.1", optional = true }

# Async
tokio = { workspace = true }
tokio-postgres = { workspace = true }
async-trait = { workspace = true }

# Serialization
serde = { workspace = true }
serde_json = { workspace = true }
chrono = { workspace = true }

# Logging
tracing = { workspace = true }

# Error handling
thiserror = { workspace = true }

[dev-dependencies]
tokio = { workspace = true, features = ["rt-multi-thread", "macros"] }
```

Design decision: `ruvector-core` is NOT added in Phase 1. Phase 1 builds the traits and storage infrastructure. Phase 2 adds ruvector-core after Phase 0 confirms aarch64 compatibility. The `SimilarityEngine` trait (Phase 2) will wrap ruvector-core.

`ruvector-graph` IS an optional dependency in Phase 1 because P1-11 (GraphStore) needs to provide a ruvector-graph backend if Phase 0 succeeds. It is gated behind a Cargo feature flag.

#### `apps/ndp-intelligence-app/Cargo.toml`

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
tokio = { workspace = true }
clap = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
chrono = { workspace = true }
```

#### ndp-lib Cargo.toml Changes

No new dependencies required for Phase 1 embeddings. The `embeddings` module is pure Rust using only `chrono` (already a dependency) and `std::collections::BTreeMap`. The `populator/embedding_writer.rs` uses types from `ndp-intelligence::storage`, but since ndp-lib does NOT depend on ndp-intelligence, `EmbeddingWriter` takes a generic `StorageBackend` trait object passed in by the caller. This means ndp-lib needs the trait definition.

**Architectural resolution:** The `StorageBackend` trait is defined in `ndp-intelligence`, but `EmbeddingWriter` lives in `ndp-lib`. This creates a dependency problem. Two options:

- **Option A:** Move `EmbeddingWriter` to `ndp-intelligence`. EmbeddingWriter becomes `ndp-intelligence::populator::EmbeddingWriter`.
- **Option B:** Define a minimal `EmbeddingStore` trait in `ndp-lib::gold::populator` that `ndp-intelligence::storage::PostgresStorage` implements.

**Decision: Option A.** Move `EmbeddingWriter` to `ndp-intelligence::populator`. This keeps ndp-lib free of any storage-layer coupling and preserves the dependency direction (ndp-intelligence depends on ndp-lib, not the reverse). The SCOPE.md reference to `ndp-lib::gold::populator::embedding_writer` is updated here -- the actual location is `crates/ndp-intelligence/src/populator/embedding_writer.rs`.

Updated `ndp-intelligence` module tree:

```
crates/ndp-intelligence/src/
  lib.rs
  error.rs
  config.rs
  storage/
    mod.rs
    postgres.rs
  graph/
    mod.rs
    ruvector.rs
    sql.rs
  populator/                     # MOVED from ndp-lib
    mod.rs                       # Populator trait
    embedding_writer.rs          # EmbeddingWriter
```

The `ndp-lib::gold::populator/` module is NOT created. The SCOPE.md P1-12 task description (`ndp-lib::gold::populator::embedding_writer`) is superseded by this architecture decision.

---

## 2. Trait Design (ADRs)

### ADR-001: Embedder Trait Design

**Status:** Accepted

**Context:** The intelligence layer needs to convert Gold aligned view rows into fixed-dimensional vectors. Different embedding strategies exist (metric z-score, text/MiniLM, composite), and the system must support adding new strategies without modifying existing code.

**Decision:** Define an `Embedder` trait in `ndp-lib::gold::embeddings` with these design choices:

1. **`GoldRow` uses `BTreeMap<String, Option<f64>>` for fields.** BTreeMap provides deterministic iteration order (alphabetical by key), which guarantees consistent vector dimension ordering across runs. HashMap would produce non-deterministic ordering, causing the same data to produce different vectors on different runs.

2. **`Option<f64>` for field values.** Gold aligned views use FULL OUTER JOIN, which produces NULLs when streams have no data for a time bucket. SQL NULLs map to `None`. The embedding layer must handle these explicitly rather than crashing on unexpected NULLs.

3. **No database dependency in the trait.** The Embedder trait is `fn embed(&self, row: &GoldRow) -> Result<Embedding>` -- a pure function from input to output. Database reads (fetching the GoldRow) and writes (storing the Embedding) are handled by separate components (the caller and EmbeddingWriter, respectively). This makes MetricEmbedder unit-testable without a database.

4. **`Embedding` carries metadata.** `HashMap<String, serde_json::Value>` allows each embedder to attach provenance information (which fields were NULL, which strategy was used, warmup state) without changing the struct definition.

```rust
// Located in: crates/ndp-lib/src/gold/embeddings/mod.rs

pub trait Embedder: Send + Sync {
    fn embed(&self, row: &GoldRow) -> Result<Embedding, EmbeddingError>;
    fn dimensions(&self) -> usize;
    fn name(&self) -> &str;
}

pub struct GoldRow {
    pub bucket: chrono::DateTime<chrono::Utc>,
    pub domain_id: String,
    pub fields: BTreeMap<String, Option<f64>>,
}

pub struct Embedding {
    pub vector: Vec<f32>,
    pub dimensions: usize,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug)]
pub enum EmbeddingError {
    DimensionMismatch { expected: usize, actual: usize },
    AllFieldsNull,
    StatsNotWarmedUp { field: String, samples: usize, required: usize },
}
```

**Consequences:**
- Easier: Adding new embedder types (EventEmbedder in Phase 4, CompositeEmbedder in Phase 4) requires only implementing the trait.
- Easier: Unit testing MetricEmbedder with synthetic GoldRow data.
- Harder: Caller must construct GoldRow from SQL results (a few lines of mapping code).

**Alternatives Considered:**
- *Typed struct per domain* (e.g., `IndoorAirQualityRow`): Rejected. Would require a new struct for every domain, defeating the config-driven design.
- *`Vec<f64>` instead of BTreeMap*: Rejected. Loses field names, making debugging and metadata impossible.
- *`HashMap` instead of BTreeMap*: Rejected. Non-deterministic iteration order would produce inconsistent vectors.

---

### ADR-002: MetricEmbedder Z-Score Approach

**Status:** Accepted

**Context:** Gold aligned view fields have different units and scales (CO2: 400-2000 ppm, PM2.5: 0-500 ug/m3, temperature: -10 to 45 C, humidity: 0-100%). K-NN cosine similarity requires all dimensions to be on comparable scales, otherwise high-magnitude fields dominate distance calculations.

**Decision:** Use z-score normalization with running statistics and exponential decay.

1. **Running statistics, not batch statistics.** The system processes data in real-time as new aligned view rows arrive. A batch approach would require re-reading all historical data on every cycle. Running statistics (online algorithm) update incrementally.

2. **Exponential decay (alpha=0.01).** Sensor characteristics drift over time (seasonal temperature shifts, sensor aging). Exponential decay gives recent observations more weight than old ones. Alpha=0.01 means the effective window is approximately 100 samples (about 4 days of hourly data), balancing responsiveness with stability.

3. **Warmup window: 168 samples (1 week).** Z-scores are meaningless with insufficient data. During warmup, the system stores embeddings (for later backfill recalculation) but does not generate predictions. 168 hours provides at least one full weekday/weekend cycle.

4. **RunningStats is per-field, not global.** Each field tracks its own mean and standard deviation independently. This is stored in a `HashMap<String, RunningStats>` on the MetricEmbedder.

```rust
// Located in: crates/ndp-lib/src/gold/embeddings/running_stats.rs

pub struct RunningStats {
    mean: f64,
    variance: f64,       // Running variance (Welford's online algorithm)
    count: usize,
    alpha: f64,           // Exponential decay factor
    warmup_threshold: usize,
}

impl RunningStats {
    pub fn new(alpha: f64, warmup_threshold: usize) -> Self;
    pub fn update(&mut self, value: f64);
    pub fn z_score(&self, value: f64) -> f64;
    pub fn mean(&self) -> f64;
    pub fn std_dev(&self) -> f64;
    pub fn is_warmed_up(&self) -> bool;
    pub fn count(&self) -> usize;
}
```

Implementation notes:
- Uses Welford's online algorithm for numerically stable variance computation.
- After warmup, switches from simple mean/variance to exponentially weighted moving average/variance.
- `z_score()` returns 0.0 if std_dev is 0.0 (all values identical), preventing division by zero.
- `z_score()` returns `Err` (via the Embedder layer) if not warmed up.

**Consequences:**
- Easier: No batch recalculation needed on each cycle.
- Easier: Naturally adapts to seasonal drift.
- Harder: First week produces no predictions (warmup period).
- Harder: If the system restarts, RunningStats must be rebuilt from stored embeddings or persisted separately. Phase 1 does not persist RunningStats -- it rebuilds from the most recent 168 embeddings on startup (Phase 2 concern).

**Alternatives Considered:**
- *Min-max normalization*: Rejected. Sensitive to outliers; a single extreme reading shifts the entire scale.
- *Fixed normalization ranges from config*: Rejected. Requires manual tuning per sensor, defeats config-driven design.
- *Standard batch z-score (compute mean/std over all history)*: Rejected. Requires re-reading all data; does not adapt to drift.

---

### ADR-003: Dual-Backend Strategy (pgvector + HNSW)

**Status:** Accepted

**Context:** The system needs both durable storage (survives crashes/restarts) and fast search (<1ms K-NN). pgvector provides durability inside TimescaleDB. ruvector-core's HNSW provides in-process speed. Using only one sacrifices either durability or performance.

**Decision:** Both backends are always present. pgvector is the durable source of truth; HNSW is the acceleration layer.

1. **Write path:** Every embedding is written to pgvector first (durable), then inserted into the in-process HNSW index (fast).
2. **Read path (search):** K-NN search uses HNSW. If HNSW is unavailable (Phase 0 failure, or startup before rebuild), fall back to pgvector SQL search.
3. **Startup:** Rebuild HNSW index from pgvector data. This means pgvector is always consistent. HNSW can be thrown away and rebuilt.
4. **Phase 1 scope:** Phase 1 implements pgvector storage only (P1-10). HNSW integration is Phase 2 (after Phase 0 gate).

```
Write:  Embedding --> PostgresStorage.store_embedding() --> gold.metric_embeddings (pgvector)
                  \--> [Phase 2] HnswEngine.insert() --> in-process HNSW index

Search: [Phase 2] HnswEngine.search() --> HNSW index (primary, <1ms)
        PostgresStorage.search() --> pgvector SQL (fallback, <10ms)

Startup: PostgresStorage.load_embeddings() --> HnswEngine.rebuild()
```

**Consequences:**
- Easier: Crash recovery is trivial -- rebuild HNSW from pgvector.
- Easier: Phase 1 can deliver full storage without waiting for Phase 0 outcome.
- Harder: Two write paths means slightly higher per-cycle latency (but pgvector INSERT is ~1ms).
- Harder: HNSW memory consumption grows with vector count (but ~2 MB for 10K 32D vectors).

**Alternatives Considered:**
- *pgvector only*: Viable as fallback (if ruvector-core fails Phase 0), but 10ms search vs <1ms matters at scale.
- *HNSW only with redb persistence*: Rejected. Would mean intelligence data is outside TimescaleDB, breaking the single-database principle and making Grafana dashboarding harder.
- *ruvector-core redb as primary*: Rejected. redb is embedded and not queryable from SQL tools.

---

### ADR-004: GraphStore Backend Selection

**Status:** Accepted (outcome depends on Phase 0 gate)

**Context:** The intelligence layer needs a graph capability for storing relationships between entities (causal links in Phase 3, cross-type links in Phase 4). Two viable backends exist: ruvector-graph (pure Rust, in-process) and SQL adjacency tables (gold.graph_nodes, gold.graph_edges).

**Decision:** ruvector-graph is the preferred backend if it compiles on aarch64. SQL adjacency tables are the fallback. Both implement the same `GraphStore` trait.

1. **Feature flag:** `ruvector-graph` is a Cargo feature in `ndp-intelligence`. When enabled, `RuvectorGraphStore` is compiled. When disabled, only `SqlGraphStore` is available.
2. **Factory function:** `graph::create_graph_store(config, db_client)` returns `Box<dyn GraphStore>`. It checks the feature flag and config to select the backend.
3. **SQL fallback is always compiled.** Even when ruvector-graph is available, the SQL backend provides a query path accessible from Grafana dashboards.
4. **Phase 0 determines the default.** If ruvector-graph compiles, `default = ["ruvector-graph"]` is set in Cargo.toml. If not, the feature is left off.

```rust
// Located in: crates/ndp-intelligence/src/graph/mod.rs

#[async_trait]
pub trait GraphStore: Send + Sync {
    async fn add_node(&self, node: &GraphNode) -> Result<(), IntelligenceError>;
    async fn add_edge(&self, edge: &GraphEdge) -> Result<(), IntelligenceError>;
    async fn get_edges(&self, node_id: &str, edge_type: Option<&str>)
        -> Result<Vec<GraphEdge>, IntelligenceError>;
    async fn get_neighbors(&self, node_id: &str, edge_type: Option<&str>)
        -> Result<Vec<GraphNode>, IntelligenceError>;
    async fn node_count(&self, node_type: Option<&str>)
        -> Result<usize, IntelligenceError>;
    async fn edge_count(&self, edge_type: Option<&str>)
        -> Result<usize, IntelligenceError>;
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

/// Factory: select graph backend based on feature flags and config
pub fn create_graph_store(
    config: &GraphConfig,
    db_client: Option<tokio_postgres::Client>,
) -> Box<dyn GraphStore> {
    #[cfg(feature = "ruvector-graph")]
    if config.backend == GraphBackend::Ruvector {
        return Box::new(RuvectorGraphStore::new(config));
    }

    Box::new(SqlGraphStore::new(
        db_client.expect("SQL graph backend requires database client"),
    ))
}
```

**Consequences:**
- Easier: Single trait for all graph operations regardless of backend.
- Easier: Switching backends requires only a config/feature change, no code changes.
- Harder: ruvector-graph's API may not map 1:1 to our trait -- the `RuvectorGraphStore` adapter must handle any impedance mismatch.

**Alternatives Considered:**
- *SQL only*: Simpler but loses in-process graph traversal performance for Phase 3 Granger analysis.
- *ruvector-graph only*: No SQL query path for dashboards. Would require a separate export mechanism.
- *Neo4j*: Too heavy for Pi edge deployment. 1+ GB memory footprint.

---

### ADR-005: PgVectorSchemaGenerator Pattern

**Status:** Accepted

**Context:** Intelligence tables (metric_embeddings, predictions, graph_nodes, graph_edges, reasoning_bank) need DDL generation that integrates with the existing Gold DDL pipeline. The existing pattern is `ContinuousAggregateGenerator` at `/workspaces/neural-data-platform/crates/ndp-lib/src/gold/generators/continuous_aggregate.rs`.

**Decision:** Create `PgVectorSchemaGenerator` following the existing generator pattern.

The existing pattern observed in `ContinuousAggregateGenerator`:
1. Constructor `from_stream_config()` or `from_domain_config()` validates required config fields.
2. `generate()` method takes config and `Action` enum, returns `Result<String>`.
3. Generated SQL includes schema creation (`CREATE SCHEMA IF NOT EXISTS gold;`).
4. Sync mode uses idempotent DDL (`IF NOT EXISTS`, `DO $$ ... $$`).
5. Recreate mode includes `DROP ... CASCADE` before `CREATE`.

`PgVectorSchemaGenerator` adapts this pattern:
1. Constructor `from_domain_config(config: &DomainConfig)` validates that the domain has an `intelligence` config block.
2. `generate(action: Action) -> Result<String>` produces DDL for all intelligence tables.
3. Tables use `IF NOT EXISTS` for idempotency (no need for CA-SYNC-CHECK markers because these are regular tables, not continuous aggregates).
4. pgvector extension creation is included at the top of the output.
5. The generator is domain-scoped: it reads the domain's intelligence config to determine vector dimensions.

```rust
// Located in: crates/ndp-lib/src/gold/generators/pgvector_schema.rs

pub struct PgVectorSchemaGenerator {
    domain_id: String,
    intelligence_config: IntelligenceConfig,
}

impl PgVectorSchemaGenerator {
    pub fn from_domain_config(config: &DomainConfig) -> Result<Self>;
    pub fn generate(&self, action: Action) -> Result<String>;
    pub fn generate_extension_ddl(&self) -> String;
    pub fn generate_metric_embeddings_ddl(&self, action: Action) -> String;
    pub fn generate_predictions_ddl(&self, action: Action) -> String;
    pub fn generate_graph_tables_ddl(&self, action: Action) -> String;
    pub fn generate_reasoning_bank_ddl(&self, action: Action) -> String;
}
```

Note: `IntelligenceConfig` is defined in `ndp-intelligence::config` but `PgVectorSchemaGenerator` lives in `ndp-lib::gold::generators`. This creates the same dependency problem as ADR-001/EmbeddingWriter.

**Resolution:** Define a minimal `IntelligenceSchemaConfig` struct in `ndp-lib::gold::config` that contains only the fields needed for DDL generation (no runtime config). The full `IntelligenceConfig` in ndp-intelligence can `From`-convert to this. Alternatively, `PgVectorSchemaGenerator` can accept raw parameters (domain_id, vector_dimensions, graph_backend_sql: bool) instead of a config struct.

**Decision: Accept raw parameters.** The generator takes:
- `domain_id: &str`
- `graph_backend_sql: bool` (whether to generate SQL graph tables)

Vector dimensions are not needed in the DDL because `gold.metric_embeddings` uses `vector` type without a fixed dimension (pgvector supports variable-dimension vectors per row, constrained by the `dimensions` integer column). This avoids needing intelligence config in ndp-lib entirely.

```rust
impl PgVectorSchemaGenerator {
    pub fn new(domain_id: &str, include_graph_tables: bool) -> Self;
    pub fn generate(&self, action: Action) -> Result<String>;
}
```

**Consequences:**
- Easier: ndp-lib remains independent of ndp-intelligence.
- Easier: Generator is simple, testable with string assertions.
- Harder: If future tables need domain-specific config, the parameter list grows. Cross that bridge in Phase 2+.

---

## 3. Data Flow Diagrams

### 3.1 Embedding Pipeline (Phase 1 scope: types + storage, no runtime cycle)

```
gold.indoor_air_quality_aligned_hourly    (existing materialized view)
         |
         |  SQL SELECT bucket, indoor_co2_mean, indoor_pm25_mean, ...
         |  WHERE bucket > $last_processed
         |
         v
+------------------+
|    GoldRow        |    BTreeMap<String, Option<f64>>
|    .bucket        |    chrono::DateTime<Utc>
|    .domain_id     |    "indoor-air-quality"
|    .fields        |    { "indoor_co2_mean": Some(650.0),
|                   |      "indoor_pm25_mean": Some(8.2),
|                   |      "outdoor_temperature_c_mean": None, ... }
+------------------+
         |
         |  MetricEmbedder::embed(&row)
         |
         |  Step 1: Add temporal features
         |    hour_sin = sin(2*PI*hour/24)
         |    hour_cos = cos(2*PI*hour/24)
         |    is_weekend = weekday >= 5 ? 1.0 : 0.0
         |
         |  Step 2: For each direct field:
         |    if value is Some(v):
         |      stats[field].update(v)
         |      z = stats[field].z_score(v)
         |    if value is None:
         |      apply NullStrategy (zero | last_known | mean)
         |
         |  Step 3: For each derived field (from feature registry):
         |    look up in GoldRow.fields (they come from Gold CAs)
         |    z-score normalize same as direct fields
         |
         |  Step 4: Concatenate into Vec<f32>
         |    [hour_sin, hour_cos, is_weekend, z_co2, z_pm25, z_temp, ...]
         |
         v
+------------------+
|    Embedding      |    Vec<f32> [~19D for indoor-air-quality]
|    .vector        |    [0.86, 0.5, 0.0, 1.2, -0.3, ...]
|    .dimensions    |    19
|    .metadata      |    { "null_fields": ["outdoor_wind_speed_mean"],
|                   |      "warmup_complete": true }
+------------------+
         |
         |  EmbeddingWriter::write(embedding, domain_id, bucket)
         |
         v
+-------------------------------+
|  gold.metric_embeddings       |    pgvector INSERT
|  (bucket, domain_id, vector)  |
+-------------------------------+
```

Dimension calculation for indoor-air-quality domain:
- 3 temporal features (hour_sin, hour_cos, is_weekend)
- 8 direct fields (indoor_co2_mean, indoor_pm25_mean, indoor_temperature_c_mean, indoor_humidity_pct_mean, outdoor_temperature_c_mean, outdoor_humidity_pct_mean, outdoor_wind_speed_mean, outdoor_aqi_pm25_mean)
- 4 derived fields (indoor_co2_mean_trend_4h, indoor_pm25_mean_trend_4h, indoor_co2_mean_std_4h, indoor_co2_mean_diff_1h)
- **Total: 15 dimensions** (3 + 8 + 4)

Note: The gold-002 ARCHITECTURE.md estimated ~32D. The actual dimension count depends on which derived features exist in the Gold feature registry. The 15 above matches the current domain.json intelligence config from gold-002 section 6. Additional derived features in the registry will increase this.

### 3.2 Schema Generation Flow

```
domain.json                         (config file, with intelligence block)
     |
     |  FileSystemConfigLoader.load_domain_config("indoor-air-quality")
     |
     v
+---------------------+
|  DomainConfig       |
|  .id                |   "indoor-air-quality"
|  .intelligence      |   Some(IntelligenceConfig { ... })
|  .streams           |   [air-quality, outdoor-weather, ...]
+---------------------+
     |
     |  PgVectorSchemaGenerator::new(domain_id, include_graph_tables)
     |
     v
+-----------------------------+
| PgVectorSchemaGenerator     |
|   .domain_id                |
|   .include_graph_tables     |
+-----------------------------+
     |
     |  .generate(Action::Sync)
     |
     v
+---------------------------------------------------------------+
|  SQL DDL output:                                              |
|                                                               |
|  1. CREATE EXTENSION IF NOT EXISTS vector;                    |
|  2. CREATE SCHEMA IF NOT EXISTS gold;                         |
|  3. CREATE TABLE IF NOT EXISTS gold.metric_embeddings (...)   |
|     SELECT create_hypertable(...)                             |
|  4. CREATE TABLE IF NOT EXISTS gold.predictions (...)         |
|     SELECT create_hypertable(...)                             |
|  5. [if include_graph_tables]                                 |
|     CREATE TABLE IF NOT EXISTS gold.graph_nodes (...)         |
|     CREATE TABLE IF NOT EXISTS gold.graph_edges (...)         |
|  6. CREATE TABLE IF NOT EXISTS gold.reasoning_bank (...)      |
+---------------------------------------------------------------+
     |
     |  stdout (from ndp-cli) or execute against TimescaleDB
     v
```

### 3.3 Graph Storage Flow (Phase 1: trait + SQL backend)

```
GraphNode / GraphEdge
     |
     |  GraphStore::add_node() / add_edge()
     |
     v
+-----------------------------------+
| dispatch via create_graph_store() |
+-----------------------------------+
     |                        |
     | [feature=ruvector-graph]   | [default / fallback]
     v                        v
+-----------------+    +------------------+
| RuvectorGraph   |    | SqlGraphStore    |
| Store           |    |                  |
| (in-process)    |    | INSERT INTO      |
|                 |    | gold.graph_nodes |
|                 |    | gold.graph_edges |
+-----------------+    +------------------+
```

In Phase 1, the SQL backend is the safe default. The ruvector-graph backend is compiled only if the feature flag is enabled (which depends on Phase 0 outcome).

---

## 4. Database Schema Design

### 4.1 pgvector Extension Installation

The pgvector extension is already installed in the production TimescaleDB Docker image at `/workspaces/neural-data-platform/docker/timescaledb/Dockerfile` (line 8: `postgresql-16-pgvector`).

For the integration environment (which uses `timescale/timescaledb:latest-pg15` per `/workspaces/neural-data-platform/docker-compose.integration.yml` line 64), pgvector must be installed separately. Two approaches:

**Approach A (init script):** Add a new init script to `/workspaces/neural-data-platform/deploy/pi/init-scripts/`:

File: `006_pgvector_extension.sql`
```sql
-- Enable pgvector extension for intelligence layer (V1.2)
-- Requires postgresql-NN-pgvector package installed in the Docker image

DO $$
BEGIN
    -- Only create if the shared library is available
    IF EXISTS (
        SELECT 1 FROM pg_available_extensions WHERE name = 'vector'
    ) THEN
        CREATE EXTENSION IF NOT EXISTS vector;
        RAISE NOTICE 'pgvector extension installed successfully';
    ELSE
        RAISE WARNING 'pgvector extension not available - install postgresql-15-pgvector package';
    END IF;
END $$;
```

**Approach B (Docker image):** Add pgvector to the integration TimescaleDB image. This requires either a custom Dockerfile or adding an apt-get layer.

**Decision: Both.** The init script handles extension creation idempotently. For the integration environment, add pgvector to the Docker image by creating a custom Dockerfile or using an image that includes it. The production Dockerfile already has it.

For the integration environment, modify `/workspaces/neural-data-platform/docker-compose.integration.yml` to use a custom Dockerfile that extends `timescale/timescaledb:latest-pg15` with pgvector:

File: `docker/integration-timescaledb/Dockerfile`
```dockerfile
FROM timescale/timescaledb:latest-pg15
RUN apt-get update && apt-get install -y postgresql-15-pgvector && rm -rf /var/lib/apt/lists/*
```

### 4.2 All Table DDL

All tables live in the `gold` schema, consistent with existing Gold layer conventions established in `/workspaces/neural-data-platform/crates/ndp-lib/src/gold/generators/continuous_aggregate.rs` (line 65: `CREATE SCHEMA IF NOT EXISTS gold;`).

#### 4.2.1 Metric Embeddings

```sql
-- Intelligence: metric embeddings (V1.2 Phase 1)
CREATE TABLE IF NOT EXISTS gold.metric_embeddings (
    bucket          TIMESTAMPTZ NOT NULL,
    domain_id       TEXT NOT NULL,
    embedding       vector,           -- pgvector type, variable dimension per domain
    dimensions      INTEGER NOT NULL,
    metadata        JSONB DEFAULT '{}',
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (bucket, domain_id)
);

-- Hypertable for time-series partitioning
SELECT create_hypertable('gold.metric_embeddings', 'bucket',
    chunk_time_interval => INTERVAL '7 days',
    if_not_exists => TRUE
);

-- Index on domain_id for filtered queries
CREATE INDEX IF NOT EXISTS idx_metric_embeddings_domain
    ON gold.metric_embeddings (domain_id, bucket DESC);
```

**HNSW index strategy:** Do NOT create an HNSW index at table creation time. pgvector's sequential scan is fast enough for <1000 vectors. Creating HNSW indexes on small datasets wastes memory and slows inserts. The HNSW index should be created when the vector count exceeds a threshold (Phase 2 responsibility):

```sql
-- Created by the intelligence daemon when count > 1000
-- NOT part of Phase 1 DDL generation
CREATE INDEX idx_metric_embeddings_hnsw
    ON gold.metric_embeddings
    USING hnsw (embedding vector_cosine_ops)
    WITH (m = 16, ef_construction = 64);
```

#### 4.2.2 Predictions

```sql
-- Intelligence: predictions (V1.2 Phase 1 schema, Phase 2 writes)
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
    if_not_exists => TRUE
);

CREATE INDEX IF NOT EXISTS idx_predictions_domain_metric
    ON gold.predictions (domain_id, metric, bucket DESC);

CREATE INDEX IF NOT EXISTS idx_predictions_pending
    ON gold.predictions (domain_id, bucket DESC)
    WHERE correct IS NULL;
```

The `idx_predictions_pending` partial index accelerates the outcome evaluation query (Phase 2: find predictions whose horizon has elapsed but not yet evaluated).

#### 4.2.3 Graph Tables (SQL Fallback)

Generated only when `include_graph_tables = true` (i.e., when ruvector-graph is NOT the primary backend).

```sql
-- Intelligence: graph nodes (V1.2 Phase 1, SQL adjacency backend)
CREATE TABLE IF NOT EXISTS gold.graph_nodes (
    id              TEXT PRIMARY KEY,
    node_type       TEXT NOT NULL,
    properties      JSONB DEFAULT '{}',
    created_at      TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_graph_nodes_type
    ON gold.graph_nodes (node_type);

-- Intelligence: graph edges (V1.2 Phase 1, SQL adjacency backend)
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
    ON gold.graph_edges (source_id, edge_type);
CREATE INDEX IF NOT EXISTS idx_graph_edges_target
    ON gold.graph_edges (target_id, edge_type);
```

Note: graph_nodes and graph_edges are NOT hypertables. They are regular tables with B-tree indexes. Graph data does not have a natural time-series dimension -- nodes represent entities, not time points.

#### 4.2.4 ReasoningBank (V1.3 Prep)

```sql
-- Intelligence: reasoning bank (V1.3 SONA prep, empty in V1.2)
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
    ON gold.reasoning_bank (domain_id);
```

This table is created by the schema generator but is unused in V1.2. It prepares storage for ruvector SONA (LoRA adapters + EWC++ Fisher information) in V1.3, avoiding a schema migration later.

### 4.3 Hypertable Configuration Summary

| Table | Hypertable | Chunk Interval | Rationale |
|-------|-----------|----------------|-----------|
| `gold.metric_embeddings` | Yes | 7 days | ~168 rows/week (hourly), vectors are large rows |
| `gold.predictions` | Yes | 30 days | Higher row volume (multiple predictions per hour), but each row is small |
| `gold.graph_nodes` | No | -- | Not time-series; entity-based |
| `gold.graph_edges` | No | -- | Not time-series; relationship-based |
| `gold.reasoning_bank` | No | -- | Small table; few rows per domain |

---

## 5. Integration Points

### 5.1 P1-08: PgVectorSchemaGenerator and SyncPlanner

The existing `SyncPlanner` at `/workspaces/neural-data-platform/crates/ndp-lib/src/gold/planner/sync.rs` is designed for continuous aggregates. It checks whether CAs exist via the `CaChecker` trait and generates DDL only for missing ones.

`PgVectorSchemaGenerator` does NOT integrate with `SyncPlanner` because:
1. Intelligence tables are regular tables, not continuous aggregates.
2. `CREATE TABLE IF NOT EXISTS` provides built-in idempotency -- no need for existence checks.
3. The SyncPlanner's `CaAction` (Create/Skip/Recreate) is CA-specific.

Instead, `PgVectorSchemaGenerator` is a standalone generator called directly by the CLI and by `generate_domain()`. Its DDL is inherently idempotent.

Integration path: `ndp-lib::gold::generate_domain()` (at `/workspaces/neural-data-platform/crates/ndp-lib/src/gold/mod.rs` line 155) gains an optional code path:

```rust
// In generate_domain(), after existing aligned view / events generation:
if let Some(intelligence) = &domain_config.intelligence {
    if intelligence.enabled {
        let pgvector_gen = PgVectorSchemaGenerator::new(
            &domain_config.id,
            /* include_graph_tables: determined by config or Phase 0 */
            true,
        );
        let intel_ddl = pgvector_gen.generate(action)?;
        // append to output
    }
}
```

This means `ndp gold generate --domain indoor-air-quality` produces intelligence DDL alongside existing Gold DDL when the domain has an `intelligence` config block.

### 5.2 P1-07: DomainConfig Extension (Backward Compatibility)

The `DomainConfig` struct at `/workspaces/neural-data-platform/crates/ndp-lib/src/gold/config/domain.rs` currently has these fields:
- `id: String`
- `description: String`
- `streams: Vec<StreamRef>`
- `alignment: AlignmentConfig`
- `objectives: Vec<ObjectiveConfig>` (with `#[serde(default)]`)
- `events: Option<EventsConfig>` (with `#[serde(default)]`)

Add one field:

```rust
/// Optional intelligence layer configuration (V1.2)
#[serde(default)]
pub intelligence: Option<IntelligenceConfig>,
```

Where `IntelligenceConfig` is a struct defined in ndp-lib (NOT imported from ndp-intelligence). This is a simpler config struct containing only what ndp-lib needs for DDL generation and config validation:

```rust
// In crates/ndp-lib/src/gold/config/intelligence.rs (NEW file)

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingType {
    Metric,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingFieldsConfig {
    pub temporal: Vec<String>,
    pub direct: Vec<DirectFieldConfig>,
    #[serde(default)]
    pub derived: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectFieldConfig {
    pub field: String,
    pub null_strategy: String,
}

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

**Backward compatibility:** The `#[serde(default)]` attribute on the `intelligence` field means existing domain.json files without an `intelligence` block deserialize to `intelligence: None`. All existing tests continue to pass without modification.

The test at `/workspaces/neural-data-platform/crates/ndp-lib/src/gold/config/domain.rs` line 316 (`test_domain_config_deserialize`) uses JSON without an `intelligence` field and will continue to pass because `Option<T>` with `#[serde(default)]` defaults to `None`.

A new test verifies the intelligence block deserializes correctly:

```rust
#[test]
fn test_domain_config_with_intelligence_deserialize() {
    let json = r#"{
        "id": "indoor-air-quality",
        "streams": [...],
        "alignment": {...},
        "intelligence": {
            "enabled": true,
            "embedding": {
                "type": "metric",
                "fields": {
                    "temporal": ["hour_sin", "hour_cos", "is_weekend"],
                    "direct": [
                        {"field": "indoor_co2_mean", "null_strategy": "zero"}
                    ],
                    "derived": ["indoor_co2_mean_trend_4h"]
                }
            },
            "search": { "k": 20, "min_similarity": 0.7, "prediction_horizons": ["1 hour"] }
        }
    }"#;
    let config: DomainConfig = serde_json::from_str(json).unwrap();
    assert!(config.intelligence.is_some());
    assert!(config.intelligence.unwrap().enabled);
}
```

### 5.3 P1-13: CLI Command Integration

The existing CLI at `/workspaces/neural-data-platform/tools/ndp-cli/src/main.rs` uses entity/verb structure. The `Gold` entity is defined at `/workspaces/neural-data-platform/tools/ndp-cli/src/commands/gold.rs`.

Add a new subcommand under Gold:

```rust
// In tools/ndp-cli/src/commands/gold.rs

#[derive(Subcommand)]
pub enum GoldCommands {
    // ... existing Generate, Sync, Recreate ...

    /// Generate intelligence schema DDL.
    Intelligence {
        #[command(subcommand)]
        command: IntelligenceCommands,
    },
}

#[derive(Subcommand)]
pub enum IntelligenceCommands {
    /// Generate pgvector + graph DDL for a domain.
    Schema {
        /// Target domain ID.
        #[arg(long)]
        domain: String,

        /// Include SQL graph tables (for SQL adjacency backend).
        #[arg(long, default_value = "true")]
        graph_tables: bool,
    },
}
```

Usage:
```bash
# Generate intelligence DDL
ndp gold intelligence schema --domain indoor-air-quality

# Generate without graph tables (ruvector-graph is the backend)
ndp gold intelligence schema --domain indoor-air-quality --graph-tables false
```

The implementation calls `PgVectorSchemaGenerator::new(domain, graph_tables).generate(Action::Sync)` and prints the SQL to stdout, following the same pattern as `ndp gold generate --domain`.

### 5.4 P1-12: EmbeddingWriter (in ndp-intelligence)

As decided in section 1.6, `EmbeddingWriter` lives in `ndp-intelligence::populator`, NOT in ndp-lib. It bridges the embedding layer (ndp-lib types) with the storage layer (ndp-intelligence storage):

```rust
// In crates/ndp-intelligence/src/populator/embedding_writer.rs

use ndp_lib::gold::embeddings::Embedding;
use crate::storage::StorageBackend;

pub struct EmbeddingWriter<S: StorageBackend> {
    storage: S,
}

impl<S: StorageBackend> EmbeddingWriter<S> {
    pub fn new(storage: S) -> Self {
        Self { storage }
    }

    pub async fn write(
        &self,
        embedding: &Embedding,
        domain_id: &str,
        bucket: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), IntelligenceError> {
        let stored = StoredEmbedding {
            bucket,
            domain_id: domain_id.to_string(),
            vector: embedding.vector.clone(),
            dimensions: embedding.dimensions,
            metadata: serde_json::to_value(&embedding.metadata)?,
        };
        self.storage.store_embedding(&stored).await
    }
}
```

---

## 6. Phase 0 Decision Tree

### 6.1 Complete Decision Matrix

Phase 0 tests ruvector-core and ruvector-graph on aarch64. The test procedure is documented in SCOPE.md (P0-01 through P0-05). Based on the research at `/workspaces/neural-data-platform/product/research/ruvector/06-pi5-compilation-feasibility.md`, the outcomes and their impacts on Phase 1 are:

```
                         ruvector-core
                    /                     \
               COMPILES                  FAILS
              /        \                    |
         Full SimSIMD   Scalar only         |
         (NEON accel)   (no SimSIMD)        |
              |              |              |
              v              v              v
         OUTCOME A       OUTCOME B      OUTCOME C
```

| Outcome | ruvector-core | SimSIMD | Phase 1 Impact |
|---------|--------------|---------|----------------|
| **A** | Compiles, full features | NEON acceleration works | Add `ruvector-core = "2.0.1"` to ndp-intelligence Phase 2 deps. Full HNSW + NEON. |
| **B** | Compiles, scalar fallback | SimSIMD fails or disabled | Add `ruvector-core = { version = "2.0.1", default-features = false, features = ["storage", "hnsw", "parallel"] }`. Slower distance calc but functional. |
| **C** | Fails entirely | N/A | pgvector-only mode. No HNSW acceleration in Phase 2. All search via SQL. SimilarityEngine trait has PgVectorEngine only. |

```
                         ruvector-graph
                    /                     \
               COMPILES                  FAILS
                  |                        |
                  v                        v
             OUTCOME D                 OUTCOME E
```

| Outcome | ruvector-graph | Phase 1 Impact |
|---------|---------------|----------------|
| **D** | Compiles and works | Enable `ruvector-graph` feature in ndp-intelligence. `RuvectorGraphStore` is the default GraphStore backend. SQL graph tables still generated for dashboard queries. |
| **E** | Fails | `SqlGraphStore` is the only GraphStore backend. `ruvector-graph` feature stays disabled. SQL graph tables are always generated. |

### 6.2 Combined Outcome Matrix

| | ruvector-graph D (works) | ruvector-graph E (fails) |
|---|---|---|
| **ruvector-core A (full)** | Best case. HNSW+NEON + in-process graph. | HNSW works, graph via SQL. |
| **ruvector-core B (scalar)** | Slower HNSW + in-process graph. | Slower HNSW + SQL graph. Functional but suboptimal. |
| **ruvector-core C (fails)** | pgvector search + in-process graph. Unusual combo. | pgvector search + SQL graph. Most conservative. Fully functional. |

### 6.3 Feature Flag / Conditional Compilation Strategy

```toml
# crates/ndp-intelligence/Cargo.toml

[features]
default = []                        # Phase 0 determines whether defaults change
ruvector-graph = ["dep:ruvector-graph"]

# After Phase 0, if outcome D:
# default = ["ruvector-graph"]
```

In code, conditional compilation guards the ruvector-graph backend:

```rust
// crates/ndp-intelligence/src/graph/mod.rs

#[cfg(feature = "ruvector-graph")]
pub mod ruvector;
pub mod sql;

pub fn create_graph_store(...) -> Box<dyn GraphStore> {
    #[cfg(feature = "ruvector-graph")]
    {
        if config.prefer_ruvector {
            return Box::new(ruvector::RuvectorGraphStore::new(config));
        }
    }
    Box::new(sql::SqlGraphStore::new(db_client))
}
```

ruvector-core is NOT a Phase 1 dependency. It is added in Phase 2 after Phase 0 confirms the outcome. The `SimilarityEngine` trait and its implementations are Phase 2 deliverables.

### 6.4 How Phase 0 Outcomes Propagate

After Phase 0 completes, the go/no-go report at `product/features/fe-003/reports/phase0-go-no-go.md` records the outcome. The following files are updated based on the outcome:

| Outcome | File Change |
|---------|-------------|
| A or B | `crates/ndp-intelligence/Cargo.toml`: ruvector-core dependency is documented for Phase 2 (not added yet in Phase 1) |
| C | `crates/ndp-intelligence/Cargo.toml`: no ruvector-core dep. Architecture note: SimilarityEngine will only have PgVectorEngine. |
| D | `crates/ndp-intelligence/Cargo.toml`: `default = ["ruvector-graph"]`, `ruvector-graph` feature enabled |
| E | `crates/ndp-intelligence/Cargo.toml`: no change, `ruvector-graph` feature stays disabled |
| D | `PgVectorSchemaGenerator`: `include_graph_tables` defaults to `true` (SQL tables generated alongside ruvector-graph for dashboard queries) |
| E | `PgVectorSchemaGenerator`: `include_graph_tables` always `true` (SQL tables are the only graph storage) |

---

## 7. Risk Assessment

### 7.1 Dependency Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| ruvector-core does not compile on aarch64 | Medium | Medium | Phase 0 gate runs before any Phase 1 investment. pgvector-only mode is functionally equivalent (ADR-003). Research doc (`06-pi5-compilation-feasibility.md`) confirms SimSIMD has ARM NEON support and hnsw_rs is pure Rust. |
| ruvector-graph does not compile on aarch64 | Medium | Low | SqlGraphStore provides identical GraphStore interface (ADR-004). Graph tables are always generated. |
| ruvector API changes between 2.0.1 and next release | Medium | Medium | Pin exact version in Cargo.toml. SimilarityEngine and GraphStore traits abstract the API -- changes are isolated to adapter modules (`hnsw.rs`, `ruvector.rs`). |
| ruvector-core 2.0.1 has low adoption (3,648 downloads) | Informational | Low | Underlying dependencies are proven (redb: 3M+, SimSIMD: 101K/month). ruvector is the integration layer. The traits ensure we can swap it out. |
| pgvector arm64 package not available for PG15 | Low | Medium | Production Dockerfile already installs `postgresql-16-pgvector` (confirmed at `/workspaces/neural-data-platform/docker/timescaledb/Dockerfile` line 8). For PG15 integration env, package name is `postgresql-15-pgvector`. pgvector has had arm64 packages since v0.5. |

### 7.2 Schema Evolution Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Adding tables to gold schema conflicts with existing DDL | Low | Low | All new tables use `CREATE TABLE IF NOT EXISTS`. No modification to existing tables. Schema `gold` already exists from current Gold layer. |
| `gold.metric_embeddings` PRIMARY KEY (bucket, domain_id) is too restrictive | Low | Medium | One embedding per hour per domain is the intended granularity. If sub-hourly embeddings are needed (Phase 4 events), a new table `gold.event_embeddings` is added -- no PK change to metric_embeddings. |
| pgvector `vector` type without dimension constraint allows mismatched inserts | Medium | Medium | The `dimensions` INTEGER column provides a soft check. Application code validates dimensions before INSERT. A CHECK constraint (`CHECK (vector_dims(embedding) = dimensions)`) can be added if mismatches are observed. Not added initially because it requires pgvector function calls in CHECK constraints which may have performance implications. |
| Hypertable creation fails if table already exists as regular table | Low | High | `if_not_exists => TRUE` on `create_hypertable` handles this. First-time creation converts to hypertable. Subsequent runs skip. |

### 7.3 Config Compatibility Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Adding `intelligence` field to DomainConfig breaks existing deserialization | Very Low | High | `#[serde(default)]` on `Option<IntelligenceConfig>` means existing JSON without the field deserializes to `None`. Verified by existing test patterns (the `events` field uses the same approach, added at domain.rs line 32). |
| domain.json with intelligence block fails validation | Low | Medium | Intelligence config types use permissive deserialization (all fields have types that serde handles natively). No custom validators in Phase 1. Phase 2 adds semantic validation for field references. |
| Config changes to domain.json break etcd sync | Very Low | Low | etcd is not used for domain config in V1.1. Domain config is file-based via `FileSystemConfigLoader`. etcd integration is V1.3. |

### 7.4 Integration Risks

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| ndp-lib compile time increases significantly | Low | Low | Phase 1 adds only pure Rust modules (no new heavy dependencies to ndp-lib). The `embeddings` and `generators/pgvector_schema` modules are lightweight. |
| Workspace build breaks when adding new members | Low | Medium | Test by running `cargo check --workspace` after adding crate stubs. This is a standard Cargo operation that should be the first CI-equivalent check in Phase 1. |
| Integration tests require running TimescaleDB with pgvector | Expected | Low | The integration environment (`docker-compose.integration.yml`) needs a custom TimescaleDB image with pgvector (section 4.1). This is a one-time Docker setup, not a recurring risk. |

---

## Appendix A: File Path Reference

All paths are absolute from the repository root `/workspaces/neural-data-platform/`.

### New Files (Phase 1)

| Path | P1 Task | Description |
|------|---------|-------------|
| `crates/ndp-intelligence/Cargo.toml` | P1-01 | Intelligence library crate manifest |
| `crates/ndp-intelligence/src/lib.rs` | P1-01 | Crate root, module declarations |
| `crates/ndp-intelligence/src/error.rs` | P1-01 | IntelligenceError enum |
| `crates/ndp-intelligence/src/config.rs` | P1-06 | IntelligenceConfig types (runtime config, re-exports ndp-lib config types) |
| `crates/ndp-intelligence/src/storage/mod.rs` | P1-10 | StorageBackend trait |
| `crates/ndp-intelligence/src/storage/postgres.rs` | P1-10 | PostgresStorage implementation |
| `crates/ndp-intelligence/src/graph/mod.rs` | P1-11 | GraphStore trait, types, factory |
| `crates/ndp-intelligence/src/graph/ruvector.rs` | P1-11 | RuvectorGraphStore (feature-gated) |
| `crates/ndp-intelligence/src/graph/sql.rs` | P1-11 | SqlGraphStore |
| `crates/ndp-intelligence/src/populator/mod.rs` | P1-12 | Populator trait |
| `crates/ndp-intelligence/src/populator/embedding_writer.rs` | P1-12 | EmbeddingWriter |
| `apps/ndp-intelligence-app/Cargo.toml` | P1-02 | Intelligence app binary manifest |
| `apps/ndp-intelligence-app/src/main.rs` | P1-02 | CLI stub with clap |
| `crates/ndp-lib/src/gold/embeddings/mod.rs` | P1-03 | Embedder trait, GoldRow, Embedding |
| `crates/ndp-lib/src/gold/embeddings/metric.rs` | P1-04 | MetricEmbedder |
| `crates/ndp-lib/src/gold/embeddings/running_stats.rs` | P1-05 | RunningStats |
| `crates/ndp-lib/src/gold/generators/pgvector_schema.rs` | P1-08 | PgVectorSchemaGenerator |
| `crates/ndp-lib/src/gold/config/intelligence.rs` | P1-07 | IntelligenceConfig for ndp-lib |
| `deploy/pi/init-scripts/006_pgvector_extension.sql` | P1-09 | pgvector extension init |
| `docker/integration-timescaledb/Dockerfile` | P1-09 | Integration env TimescaleDB + pgvector |

### Modified Files (Phase 1)

| Path | P1 Task | Change |
|------|---------|--------|
| `Cargo.toml` (workspace root) | P1-01, P1-02 | Add two workspace members, add ndarray to workspace deps |
| `crates/ndp-lib/src/gold/mod.rs` | P1-03 | Add `pub mod embeddings;` |
| `crates/ndp-lib/src/gold/generators/mod.rs` | P1-08 | Add `pub mod pgvector_schema;`, re-export `PgVectorSchemaGenerator` |
| `crates/ndp-lib/src/gold/config/mod.rs` | P1-07 | Add `pub mod intelligence;`, re-export types |
| `crates/ndp-lib/src/gold/config/domain.rs` | P1-07 | Add `intelligence: Option<IntelligenceConfig>` field |
| `tools/ndp-cli/src/commands/gold.rs` | P1-13 | Add `Intelligence` subcommand |
| `tools/ndp-cli/src/commands/mod.rs` | P1-13 | No change needed (gold module already exported) |
| `config/domains/indoor-air-quality/domain.json` | P1-07 | Add `intelligence` block |
| `docker-compose.integration.yml` | P1-09 | Change timescaledb image to custom build |

### Existing Files Referenced (Read-Only Context)

| Path | Why Referenced |
|------|---------------|
| `crates/ndp-lib/src/gold/generators/continuous_aggregate.rs` | Generator pattern to follow for PgVectorSchemaGenerator |
| `crates/ndp-lib/src/gold/planner/sync.rs` | SyncPlanner pattern (NOT used for intelligence tables) |
| `crates/ndp-lib/src/gold/config/domain.rs` | DomainConfig struct to extend |
| `docker/timescaledb/Dockerfile` | Confirms pgvector already installed in production |
| `deploy/pi/init-scripts/00-create-extensions.sql` | Extension creation pattern to follow |
| `product/research/ruvector/06-pi5-compilation-feasibility.md` | ARM64 feasibility research |

---

## Appendix B: Task-to-Architecture Mapping

Quick reference for implementation agents: which architecture section covers which SCOPE.md task.

| P1 Task | Primary Section | Key Files |
|---------|----------------|-----------|
| P1-01 | 1.3, 1.6, Appendix A | `crates/ndp-intelligence/` |
| P1-02 | 1.5, 1.6 | `apps/ndp-intelligence-app/` |
| P1-03 | 1.4, 2 (ADR-001), 3.1 | `crates/ndp-lib/src/gold/embeddings/mod.rs` |
| P1-04 | 1.4, 2 (ADR-002), 3.1 | `crates/ndp-lib/src/gold/embeddings/metric.rs` |
| P1-05 | 1.4, 2 (ADR-002) | `crates/ndp-lib/src/gold/embeddings/running_stats.rs` |
| P1-06 | 1.3, 5.2 | `crates/ndp-intelligence/src/config.rs`, `crates/ndp-lib/src/gold/config/intelligence.rs` |
| P1-07 | 5.2 | `crates/ndp-lib/src/gold/config/domain.rs`, `domain.json` |
| P1-08 | 2 (ADR-005), 3.2, 5.1 | `crates/ndp-lib/src/gold/generators/pgvector_schema.rs` |
| P1-09 | 4.1 | `deploy/pi/init-scripts/006_pgvector_extension.sql`, Docker |
| P1-10 | 1.3, 2 (ADR-003), 4.2 | `crates/ndp-intelligence/src/storage/` |
| P1-11 | 1.3, 2 (ADR-004), 3.3, 4.2.3 | `crates/ndp-intelligence/src/graph/` |
| P1-12 | 1.3, 5.4 | `crates/ndp-intelligence/src/populator/` |
| P1-13 | 5.3 | `tools/ndp-cli/src/commands/gold.rs` |
