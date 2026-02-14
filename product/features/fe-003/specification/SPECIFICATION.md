# fe-003: Intelligence Foundation Phase 0 + Phase 1 -- Specification

> **SPARC Phase:** Specification
> **Scope:** `product/features/fe-003/SCOPE.md`
> **Architecture:** `product/features/gold-002/ARCHITECTURE.md`
> **Roadmap:** `product/features/gold-002/IMPLEMENTATION-ROADMAP.md`
> **Date:** 2026-02-14

---

## 1. Functional Requirements

### 1.1 Phase 0: Go/No-Go Gate

Phase 0 is a time-boxed (1 day) validation spike. It produces a report, not production code.

#### FR-P0-01: Minimal Compilation Project

| Attribute | Value |
|-----------|-------|
| **ID** | FR-P0-01 |
| **Priority** | P0 (blocking) |
| **Description** | Create a standalone Rust project (outside NDP workspace) with `ruvector-core = "2.0.1"` and `ruvector-graph = "0.1"` as dependencies. Build with `cargo build --release` targeting `aarch64-unknown-linux-gnu`. |
| **Input** | Cargo.toml with ruvector dependencies |
| **Output** | Compiled binary for aarch64 |
| **Error conditions** | SimSIMD C compilation failure; linker errors on aarch64; missing system dependencies |
| **Exit criterion** | `cargo build --release` exits 0. If SimSIMD fails, retry with `default-features = false, features = ["storage", "hnsw", "parallel"]`. |

#### FR-P0-02: Runtime Execution

| Attribute | Value |
|-----------|-------|
| **ID** | FR-P0-02 |
| **Priority** | P0 (blocking) |
| **Description** | Execute the compiled binary on Pi 5 (native) or under cross-compilation verification. Binary must start and exit cleanly. |
| **Input** | Binary from FR-P0-01 |
| **Output** | Exit code 0, no SIGILL/SIGSEGV |
| **Error conditions** | Illegal instruction (missing NEON support); segfault; dynamic library not found |
| **Exit criterion** | Binary executes without crash |

#### FR-P0-03: ruvector-core Smoke Test

| Attribute | Value |
|-----------|-------|
| **ID** | FR-P0-03 |
| **Priority** | P0 (blocking) |
| **Description** | Insert 100 random 32-dimensional `Vec<f32>` vectors into a ruvector-core `VectorDB` instance. Perform a K-NN search (k=5) with a known query vector. Verify the returned results are correct by comparing against a brute-force linear scan. |
| **Input** | 100 randomly generated 32D vectors; 1 query vector |
| **Output** | 5 nearest neighbor IDs with similarity scores |
| **Error conditions** | Incorrect K-NN results (IDs differ from brute-force); panic during insert or search |
| **Exit criterion** | K-NN results match brute-force ground truth for all 5 neighbors |

#### FR-P0-04: ruvector-graph Smoke Test

| Attribute | Value |
|-----------|-------|
| **ID** | FR-P0-04 |
| **Priority** | P0 (blocking) |
| **Description** | Create a ruvector-graph instance. Add 10 nodes with typed properties. Add 15 directed edges with weights. Traverse 1-hop neighbors from a known node. Verify traversal results. |
| **Input** | 10 nodes, 15 edges with known structure |
| **Output** | Neighbor list for queried node |
| **Error conditions** | Incorrect traversal results; panic during node/edge creation |
| **Exit criterion** | Traversal returns expected neighbors with correct edge types |

#### FR-P0-05: Performance Measurement

| Attribute | Value |
|-----------|-------|
| **ID** | FR-P0-05 |
| **Priority** | P0 (informational) |
| **Description** | Measure and document: (a) peak RSS memory during 100-vector insert + search, (b) search latency (median, p99 over 100 searches), (c) total build time on target platform, (d) binary size. |
| **Input** | Test binary from FR-P0-03 |
| **Output** | Measurements documented in go/no-go report |
| **Error conditions** | None -- measurement only |
| **Exit criterion** | All four metrics documented |

#### FR-P0-GATE: Decision Gate

The go/no-go report (`product/features/fe-003/reports/phase0-go-no-go.md`) must record the outcome for each component and the resulting backend selection:

| Component | Pass | Action |
|-----------|------|--------|
| ruvector-core (default features) | Yes | Use as primary HNSW backend |
| ruvector-core (SimSIMD fails, scalar works) | Partial | Use with `default-features = false, features = ["storage", "hnsw", "parallel"]` |
| ruvector-core (fails entirely) | No | pgvector-only mode for all vector search |
| ruvector-graph (compiles and works) | Yes | Use as primary graph backend in `ndp-intelligence` |
| ruvector-graph (fails) | No | SQL adjacency tables (`gold.graph_nodes`, `gold.graph_edges`) via `SqlGraphStore` |

Phase 1 implementation MUST respect the Phase 0 outcome. The `GraphStore` trait (P1-11) and `SimilarityEngine` trait stub use conditional compilation or runtime dispatch based on this decision.

---

### 1.2 Phase 1: Foundation

Phase 1 builds the crate structure, types, traits, config extensions, database schema generator, and storage implementations. No runtime intelligence cycle. No Pi deployment.

#### FR-P1-01: Create `crates/ndp-intelligence` Crate

| Attribute | Value |
|-----------|-------|
| **ID** | FR-P1-01 |
| **Priority** | P1 (required) |
| **Description** | Create a new Rust library crate at `crates/ndp-intelligence/`. Add it to the workspace `members` list in `/workspaces/neural-data-platform/Cargo.toml`. |

**Crate structure:**

```
crates/ndp-intelligence/
  Cargo.toml
  src/
    lib.rs              # Public API re-exports
    error.rs            # IntelligenceError enum (thiserror)
    similarity/
      mod.rs            # SimilarityEngine trait (stub, no impl in Phase 1)
    graph/
      mod.rs            # GraphStore trait + GraphNode + GraphEdge types
      sql.rs            # SqlGraphStore implementation (SQL adjacency)
      ruvector.rs       # RuvectorGraphStore (conditional, per Phase 0)
    storage/
      mod.rs            # StorageBackend trait
      postgres.rs       # PostgresStorage implementation
```

**Cargo.toml dependencies (workspace-aligned where possible):**

```toml
[package]
name = "ndp-intelligence"
version = "0.1.0"
edition = "2021"

[dependencies]
ndp-types = { path = "../ndp-types" }
ndp-lib = { path = "../ndp-lib" }
tokio = { workspace = true }
tokio-postgres = { workspace = true }
async-trait = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
chrono = { workspace = true }
tracing = { workspace = true }
thiserror = { workspace = true }

# Conditional on Phase 0 outcome
# ruvector-core = { version = "2.0.1", optional = true }
# ruvector-graph = { version = "0.1", optional = true }

[dev-dependencies]
tokio = { workspace = true, features = ["test-util"] }
```

**Exit criterion:** `cargo build -p ndp-intelligence` and `cargo test -p ndp-intelligence` both exit 0. All modules compile. Crate appears in `cargo workspace --list` (or equivalent).

#### FR-P1-02: Create `apps/ndp-intelligence-app` Crate

| Attribute | Value |
|-----------|-------|
| **ID** | FR-P1-02 |
| **Priority** | P1 (required) |
| **Description** | Create a new binary crate at `apps/ndp-intelligence-app/`. Add to workspace members. Implement a minimal `main.rs` with clap CLI that prints help and version. |

**Cargo.toml:**

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

**CLI structure (Phase 1 -- stub only):**

```
ndp-intelligence-app [SUBCOMMAND]
  daemon      (Phase 2)
  one-shot    (Phase 2)
  backfill    (Phase 2)
  status      (Phase 2)
```

Phase 1 requires only that the binary compiles, `--help` prints usage, and `--version` prints `0.1.0`. Subcommands are defined but print "Not implemented in Phase 1" and exit 0.

**Exit criterion:** `cargo build -p ndp-intelligence-app` exits 0. `target/debug/ndp-intelligence-app --help` prints usage with subcommands listed. `target/debug/ndp-intelligence-app --version` prints `0.1.0`.

#### FR-P1-03: Embedder Trait + GoldRow Types

| Attribute | Value |
|-----------|-------|
| **ID** | FR-P1-03 |
| **Priority** | P1 (required) |
| **Description** | Define the `Embedder` trait, `GoldRow` struct, and `Embedding` struct in `crates/ndp-lib/src/gold/embeddings/mod.rs`. These are the core abstractions for the embedding pipeline. |

**Location:** `crates/ndp-lib/src/gold/embeddings/mod.rs` (new module)

**Register in:** `crates/ndp-lib/src/gold/mod.rs` -- add `pub mod embeddings;`

**Exact type definitions:**

```rust
use chrono::{DateTime, Utc};
use std::collections::{BTreeMap, HashMap};

/// Error type for embedding operations.
#[derive(Debug, thiserror::Error)]
pub enum EmbeddingError {
    #[error("Field '{field}' not found in GoldRow")]
    FieldNotFound { field: String },

    #[error("Insufficient data for embedding: {reason}")]
    InsufficientData { reason: String },

    #[error("Dimension mismatch: expected {expected}, got {actual}")]
    DimensionMismatch { expected: usize, actual: usize },
}

/// Result type for embedding operations.
pub type EmbeddingResult<T> = std::result::Result<T, EmbeddingError>;

/// A Gold aligned view row, represented as named numeric fields.
///
/// Domain-agnostic: works for any aligned view regardless of
/// which streams compose it. Uses `BTreeMap` for deterministic
/// field ordering (important for consistent vector layout).
#[derive(Debug, Clone)]
pub struct GoldRow {
    /// The time bucket this row represents.
    pub bucket: DateTime<Utc>,
    /// Domain identifier (e.g., "indoor-air-quality").
    pub domain_id: String,
    /// Field name -> value. `None` represents SQL NULL
    /// (from FULL OUTER JOIN gaps in the aligned view).
    pub fields: BTreeMap<String, Option<f64>>,
}

/// Output of any Embedder implementation.
#[derive(Debug, Clone)]
pub struct Embedding {
    /// The embedding vector. Length equals `dimensions`.
    pub vector: Vec<f32>,
    /// Number of dimensions in the vector.
    pub dimensions: usize,
    /// Metadata about the embedding (e.g., which fields contributed,
    /// z-score statistics snapshot, null field count).
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Produces vector embeddings from Gold layer data.
///
/// Implementors:
/// - Phase 1: `MetricEmbedder` (z-score normalized numeric fields + temporal)
/// - Phase 4: `EventEmbedder` (MiniLM text), `CompositeEmbedder` (combined)
///
/// Design constraints:
/// - No database dependencies -- pure transformation.
/// - Must be `Send + Sync` for use in async contexts.
/// - `embed()` must not perform I/O.
pub trait Embedder: Send + Sync {
    /// Embed a single Gold row into a vector.
    ///
    /// Returns `Err(EmbeddingError::FieldNotFound)` if a required field
    /// is missing from the row's `fields` map.
    ///
    /// Returns `Err(EmbeddingError::InsufficientData)` if the embedder
    /// has not accumulated enough statistics (warmup period).
    fn embed(&self, row: &GoldRow) -> EmbeddingResult<Embedding>;

    /// Output dimensionality of this embedder.
    fn dimensions(&self) -> usize;

    /// Human-readable name for logging and diagnostics.
    fn name(&self) -> &str;
}
```

**Unit tests required:**

1. `GoldRow` construction with mixed `Some`/`None` fields
2. `GoldRow` field ordering is deterministic (BTreeMap property)
3. `Embedding` creation with correct dimension count
4. `Embedding` dimension mismatch detection (vector length != dimensions)

**Exit criterion:** Module compiles. Unit tests pass. Types are re-exported from `crates/ndp-lib/src/gold/mod.rs`.

#### FR-P1-04: MetricEmbedder Implementation

| Attribute | Value |
|-----------|-------|
| **ID** | FR-P1-04 |
| **Priority** | P1 (required) |
| **Description** | Implement `MetricEmbedder` which implements the `Embedder` trait. Z-score normalizes numeric fields and adds temporal encoding. Handles NULLs per field strategy. |

**Location:** `crates/ndp-lib/src/gold/embeddings/metric.rs`

**Struct definition:**

```rust
use super::{Embedder, Embedding, EmbeddingResult, GoldRow};
use crate::gold::embeddings::stats::RunningStats;
use std::collections::HashMap;

/// Embeds Gold aligned view rows as z-score normalized vectors
/// with temporal features.
pub struct MetricEmbedder {
    /// Ordered list of fields to embed (determines vector layout).
    fields: Vec<EmbeddingField>,
    /// Running z-score statistics per field name.
    stats: HashMap<String, RunningStats>,
    /// Total output dimensions (len(fields) + temporal dimensions).
    dimensions: usize,
    /// Minimum observations before embedding is valid.
    warmup_threshold: usize,
    /// Count of observations processed.
    observations: usize,
}

/// Configuration for a single field in the embedding vector.
#[derive(Debug, Clone)]
pub struct EmbeddingField {
    /// Human-readable name (used in metadata).
    pub name: String,
    /// Where the value comes from.
    pub source: FieldSource,
    /// How to handle NULL values.
    pub null_strategy: NullStrategy,
}

/// Source of an embedding field value.
#[derive(Debug, Clone)]
pub enum FieldSource {
    /// Direct field from the GoldRow.fields map.
    Direct(String),
    /// Temporal encoding derived from the bucket timestamp.
    Temporal(TemporalEncoding),
}

/// Temporal encoding variants.
#[derive(Debug, Clone)]
pub enum TemporalEncoding {
    /// sin(2 * PI * hour / 24)
    HourSin,
    /// cos(2 * PI * hour / 24)
    HourCos,
    /// 1.0 if Saturday or Sunday, 0.0 otherwise
    IsWeekend,
}

/// Strategy for handling NULL (None) field values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NullStrategy {
    /// Replace with 0.0 (neutral in z-score space).
    Zero,
    /// Use the running mean for this field.
    Mean,
}
```

**`MetricEmbedder` public API:**

```rust
impl MetricEmbedder {
    /// Create a new MetricEmbedder from configuration.
    ///
    /// `fields` defines the vector layout. The order of fields in
    /// this vec determines their position in the output vector.
    ///
    /// `warmup_threshold` is the number of observations to collect
    /// before `embed()` produces valid output (default: 168 = 1 week
    /// of hourly data).
    pub fn new(fields: Vec<EmbeddingField>, warmup_threshold: usize) -> Self;

    /// Create a MetricEmbedder from an EmbeddingConfig.
    ///
    /// Parses the config's field definitions into `EmbeddingField` structs
    /// with temporal fields prepended.
    pub fn from_config(config: &EmbeddingConfig) -> EmbeddingResult<Self>;

    /// Update running statistics with a new observation.
    ///
    /// Call this for every GoldRow, even during warmup.
    /// This feeds the z-score normalizer with data.
    pub fn observe(&mut self, row: &GoldRow);

    /// Check if warmup period is complete.
    pub fn is_warmed_up(&self) -> bool;

    /// Get the number of observations processed.
    pub fn observation_count(&self) -> usize;
}

impl Embedder for MetricEmbedder {
    fn embed(&self, row: &GoldRow) -> EmbeddingResult<Embedding>;
    fn dimensions(&self) -> usize;
    fn name(&self) -> &str; // returns "MetricEmbedder"
}
```

**Embedding algorithm (implemented in `embed()`):**

1. If `!self.is_warmed_up()`, return `Err(EmbeddingError::InsufficientData)`.
2. For each `EmbeddingField` in order:
   - `FieldSource::Temporal(encoding)` -- compute from `row.bucket` using the formulas in section 2.3.
   - `FieldSource::Direct(field_name)` -- look up `row.fields[field_name]`:
     - `Some(value)` -- z-score normalize: `(value - mean) / std`. If std < 1e-10, use 0.0.
     - `None` -- apply `NullStrategy`: `Zero` -> 0.0, `Mean` -> 0.0 (mean in z-score space is 0).
3. Collect all values into `Vec<f32>`.
4. Return `Embedding { vector, dimensions: self.dimensions, metadata }`.
5. Metadata includes: `"null_count"` (count of None fields), `"field_names"` (ordered list).

**Unit tests required:**

1. `embed()` returns `InsufficientData` before warmup
2. `observe()` increments observation count
3. `is_warmed_up()` transitions after threshold
4. Known input/output test: 3 fields, 168 observations with known values, verify z-score output
5. NULL handling: field is `None` with `Zero` strategy produces 0.0
6. NULL handling: field is `None` with `Mean` strategy produces 0.0
7. Temporal encoding: hour=0 -> `sin(0)=0.0`, `cos(0)=1.0`; hour=6 -> `sin(PI/2)=1.0`
8. Temporal encoding: Saturday -> `is_weekend=1.0`, Monday -> `is_weekend=0.0`
9. Dimension count matches field count
10. `from_config()` correctly parses an `EmbeddingConfig`

**Exit criterion:** All 10 unit tests pass. `MetricEmbedder` implements `Embedder` trait.

#### FR-P1-05: RunningStats for Z-Score

| Attribute | Value |
|-----------|-------|
| **ID** | FR-P1-05 |
| **Priority** | P1 (required) |
| **Description** | Implement `RunningStats`, an exponential decay mean/standard deviation tracker for z-score normalization. |

**Location:** `crates/ndp-lib/src/gold/embeddings/stats.rs`

**Struct definition:**

```rust
/// Tracks running mean and standard deviation using exponential
/// moving average. Used for z-score normalization in MetricEmbedder.
///
/// During warmup (first `warmup` observations), uses simple
/// accumulation. After warmup, uses exponential decay with
/// configurable alpha.
#[derive(Debug, Clone)]
pub struct RunningStats {
    mean: f64,
    variance: f64,
    count: usize,
    alpha: f64,     // Exponential decay factor (default: 0.01)
}

impl RunningStats {
    /// Create a new RunningStats tracker.
    ///
    /// `alpha` controls the exponential decay rate.
    /// Smaller alpha = slower adaptation to new data.
    /// Recommended: 0.01 (adapts over ~100 observations).
    pub fn new(alpha: f64) -> Self;

    /// Update statistics with a new observation.
    pub fn update(&mut self, value: f64);

    /// Get the current running mean.
    pub fn mean(&self) -> f64;

    /// Get the current running standard deviation.
    pub fn std(&self) -> f64;

    /// Get the number of observations.
    pub fn count(&self) -> usize;

    /// Z-score normalize a value using current statistics.
    ///
    /// Returns 0.0 if std < 1e-10 (avoids division by near-zero).
    pub fn z_score(&self, value: f64) -> f64;
}
```

**Algorithm:**

- First observation: `mean = value`, `variance = 0.0`
- Subsequent observations: Welford's online algorithm for the first `warmup` observations, then exponential moving average:
  - `mean = (1 - alpha) * mean + alpha * value`
  - `variance = (1 - alpha) * variance + alpha * (value - mean)^2`
- `std() = variance.sqrt()`
- `z_score(value) = if std < 1e-10 { 0.0 } else { (value - mean) / std }`

**Unit tests required:**

1. Single observation: mean = value, std = 0.0, z_score returns 0.0
2. Two identical observations: mean = value, std = 0.0
3. Known series [1.0, 2.0, 3.0, 4.0, 5.0]: verify mean ~3.0 within tolerance
4. Z-score of the mean value returns ~0.0
5. Z-score of mean + 1 std returns ~1.0
6. Exponential decay: after many updates with constant value, mean converges to that value
7. Count increments correctly

**Exit criterion:** All 7 unit tests pass. `RunningStats` is used by `MetricEmbedder`.

#### FR-P1-06: EmbeddingConfig Types

| Attribute | Value |
|-----------|-------|
| **ID** | FR-P1-06 |
| **Priority** | P1 (required) |
| **Description** | Define configuration structs for the embedding pipeline. These structs deserialize from the `intelligence.embedding` block in `domain.json`. |

**Location:** `crates/ndp-lib/src/gold/embeddings/config.rs`

**Type definitions:**

```rust
use serde::{Deserialize, Serialize};

/// Top-level intelligence configuration for a domain.
/// Added as `Option<IntelligenceConfig>` to `DomainConfig`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntelligenceConfig {
    /// Whether intelligence is active for this domain.
    pub enabled: bool,
    /// Embedding pipeline configuration.
    pub embedding: EmbeddingConfig,
    /// Similarity search configuration.
    pub search: SearchConfig,
    /// Anomaly detection configuration (Phase 5, optional).
    #[serde(default)]
    pub anomaly: Option<AnomalyConfig>,
}

/// Configuration for the embedding pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    /// Embedding type. Phase 1 supports only "metric".
    #[serde(rename = "type")]
    pub embedding_type: EmbeddingType,
    /// Field definitions for the embedding vector.
    pub fields: EmbeddingFieldsConfig,
}

/// Embedding type discriminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingType {
    /// Numeric metric embedding with z-score normalization.
    Metric,
    // Future phases:
    // Event,
    // Composite,
}

/// Field definitions for the embedding vector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingFieldsConfig {
    /// Temporal encoding fields (e.g., ["hour_sin", "hour_cos", "is_weekend"]).
    #[serde(default)]
    pub temporal: Vec<String>,
    /// Direct numeric fields from the aligned view.
    #[serde(default)]
    pub direct: Vec<DirectFieldConfig>,
    /// Derived feature fields (lag, rolling, trend from feature registry).
    #[serde(default)]
    pub derived: Vec<String>,
}

/// Configuration for a direct embedding field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectFieldConfig {
    /// Field name in the aligned view (e.g., "indoor_co2_mean").
    pub field: String,
    /// Strategy for handling NULL values.
    pub null_strategy: NullStrategyConfig,
}

/// Serializable null strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NullStrategyConfig {
    /// Replace NULL with 0.0 (neutral in z-score space).
    Zero,
    /// Replace NULL with running mean (0.0 in z-score space).
    Mean,
}

/// Similarity search configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    /// Number of neighbors for K-NN search.
    pub k: usize,
    /// Minimum similarity threshold (0.0 - 1.0).
    pub min_similarity: f64,
    /// Prediction horizons (e.g., ["1 hour", "4 hours"]).
    pub prediction_horizons: Vec<String>,
}

/// Anomaly detection configuration (Phase 5).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnomalyConfig {
    /// Whether anomaly detection is enabled.
    pub enabled: bool,
    /// Distance threshold in sigma units above historical mean.
    pub distance_threshold_sigma: f64,
}
```

**Unit tests required:**

1. Deserialize full `IntelligenceConfig` from JSON matching ARCHITECTURE.md section 6
2. Deserialize with `anomaly: null` (omitted) -- defaults to `None`
3. Deserialize with empty `temporal`, `direct`, `derived` arrays
4. Serialize and re-deserialize round-trip
5. `EmbeddingType::Metric` serializes to `"metric"` and deserializes back
6. `NullStrategyConfig::Zero` serializes to `"zero"`

**Exit criterion:** All 6 unit tests pass. Config types are re-exported from `crates/ndp-lib/src/gold/embeddings/mod.rs`.

#### FR-P1-07: DomainConfig Intelligence Extension

| Attribute | Value |
|-----------|-------|
| **ID** | FR-P1-07 |
| **Priority** | P1 (required, backward-compatibility critical) |
| **Description** | Add an optional `intelligence` field to the existing `DomainConfig` struct in `crates/ndp-lib/src/gold/config/domain.rs`. |

**Change to `DomainConfig`:**

```rust
// In crates/ndp-lib/src/gold/config/domain.rs

use crate::gold::embeddings::config::IntelligenceConfig;

pub struct DomainConfig {
    pub id: String,
    #[serde(default)]
    pub description: String,
    pub streams: Vec<StreamRef>,
    pub alignment: AlignmentConfig,
    #[serde(default)]
    pub objectives: Vec<ObjectiveConfig>,
    #[serde(default)]
    pub events: Option<EventsConfig>,
    /// Intelligence layer configuration (V1.2+).
    /// Optional: existing domain configs without this field
    /// deserialize with `intelligence: None`.
    #[serde(default)]
    pub intelligence: Option<IntelligenceConfig>,  // NEW FIELD
}
```

**Critical constraint:** The `#[serde(default)]` annotation ensures that existing `domain.json` files (which lack an `intelligence` key) continue to deserialize without error. The field defaults to `None`.

**Unit tests required:**

1. Existing `test_domain_config_deserialize` in `domain.rs` still passes (backward compatibility)
2. Existing `test_domain_config_with_events_deserialize` still passes
3. Existing `test_domain_config_without_events_defaults_to_none` still passes
4. New test: deserialize domain.json WITH `intelligence` block -- `intelligence` is `Some(IntelligenceConfig)`
5. New test: deserialize existing `config/domains/indoor-air-quality/domain.json` from disk -- `intelligence` is `None`
6. New test: deserialize domain.json with `intelligence` block + all existing fields -- nothing lost

**Exit criterion:** All existing DomainConfig tests pass unchanged. Three new tests pass. The file `config/domains/indoor-air-quality/domain.json` is NOT modified in Phase 1 (the intelligence block is added in Phase 2 when the runtime exists).

#### FR-P1-08: PgVector Schema DDL Generator

| Attribute | Value |
|-----------|-------|
| **ID** | FR-P1-08 |
| **Priority** | P1 (required) |
| **Description** | Create `PgVectorSchemaGenerator` following the same pattern as `ContinuousAggregateGenerator`. Generates DDL for intelligence tables. |

**Location:** `crates/ndp-lib/src/gold/generators/pgvector_schema.rs`

**Register in:** `crates/ndp-lib/src/gold/generators/mod.rs` -- add `pub mod pgvector_schema;` and re-export `PgVectorSchemaGenerator`.

**Struct definition:**

```rust
use crate::gold::config::Action;
use crate::gold::error::Result;

/// Generates DDL for pgvector-based intelligence tables.
///
/// Tables generated:
/// 1. `gold.metric_embeddings` -- vector storage (hypertable)
/// 2. `gold.predictions` -- prediction tracking (hypertable)
/// 3. `gold.graph_nodes` -- graph node storage (SQL fallback)
/// 4. `gold.graph_edges` -- graph edge storage (SQL fallback)
/// 5. `gold.reasoning_bank` -- V1.3 prep (empty, unused in V1.2)
pub struct PgVectorSchemaGenerator {
    /// Domain ID for table naming context.
    domain_id: String,
    /// Whether to generate graph SQL tables
    /// (false if ruvector-graph is the backend).
    generate_graph_tables: bool,
}

impl PgVectorSchemaGenerator {
    /// Create a new generator.
    ///
    /// `generate_graph_tables`: set to `true` if Phase 0 determined
    /// that ruvector-graph is NOT available and SQL adjacency
    /// tables are needed.
    pub fn new(domain_id: &str, generate_graph_tables: bool) -> Self;

    /// Generate complete DDL for all intelligence tables.
    ///
    /// `action`: `Action::Sync` generates IF NOT EXISTS DDL.
    ///           `Action::Recreate` generates DROP + CREATE.
    pub fn generate(&self, action: Action) -> Result<String>;

    /// Generate DDL for metric_embeddings table only.
    pub fn generate_embeddings_table(&self, action: Action) -> Result<String>;

    /// Generate DDL for predictions table only.
    pub fn generate_predictions_table(&self, action: Action) -> Result<String>;

    /// Generate DDL for graph tables only (nodes + edges).
    pub fn generate_graph_tables(&self, action: Action) -> Result<String>;

    /// Generate DDL for reasoning_bank table only.
    pub fn generate_reasoning_bank_table(&self, action: Action) -> Result<String>;
}
```

**Generated DDL must match ARCHITECTURE.md section 4 exactly.** The SQL for each table is specified in section 2.2 of this document.

**Unit tests required:**

1. `generate()` with `Action::Sync` includes `CREATE TABLE IF NOT EXISTS` for all tables
2. `generate()` with `Action::Recreate` includes `DROP TABLE IF EXISTS` before `CREATE`
3. `generate()` with `generate_graph_tables: false` omits graph_nodes and graph_edges
4. `generate()` output includes `CREATE EXTENSION IF NOT EXISTS vector`
5. `generate()` output includes `create_hypertable` for metric_embeddings and predictions
6. `generate_embeddings_table()` output contains `vector` column type
7. `generate_predictions_table()` output contains `predicted_value`, `actual_value`, `correct` columns
8. `generate_reasoning_bank_table()` output contains `adapter_blob BYTEA` and `ewc_fisher BYTEA`
9. DDL output starts with `CREATE SCHEMA IF NOT EXISTS gold;`

**Exit criterion:** All 9 unit tests pass. Generator re-exported from `crates/ndp-lib/src/gold/generators/mod.rs`.

#### FR-P1-09: pgvector Extension in TimescaleDB Docker

| Attribute | Value |
|-----------|-------|
| **ID** | FR-P1-09 |
| **Priority** | P1 (required) |
| **Description** | Add `postgresql-15-pgvector` to the TimescaleDB Docker image. Add an init script that creates the `vector` extension. |

**Changes required:**

1. `docker/timescaledb/Dockerfile` (or equivalent) -- add `RUN apt-get update && apt-get install -y postgresql-15-pgvector`
2. Add init SQL script to create extension on database startup: `CREATE EXTENSION IF NOT EXISTS vector;`

**Verification SQL:**

```sql
SELECT * FROM pg_extension WHERE extname = 'vector';
-- Must return 1 row
```

**Exit criterion:** TimescaleDB container builds with pgvector. Extension loads on startup. `SELECT * FROM pg_extension WHERE extname = 'vector'` returns a row.

#### FR-P1-10: StorageBackend Trait + PostgresStorage

| Attribute | Value |
|-----------|-------|
| **ID** | FR-P1-10 |
| **Priority** | P1 (required) |
| **Description** | Define the `StorageBackend` trait and implement `PostgresStorage` for pgvector-backed durable storage. |

**Location:**
- Trait: `crates/ndp-intelligence/src/storage/mod.rs`
- Implementation: `crates/ndp-intelligence/src/storage/postgres.rs`

**Trait definition:**

```rust
use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// A stored embedding with metadata for database persistence.
#[derive(Debug, Clone)]
pub struct StoredEmbedding {
    /// Time bucket this embedding represents.
    pub bucket: DateTime<Utc>,
    /// Domain identifier.
    pub domain_id: String,
    /// The embedding vector.
    pub embedding: Vec<f32>,
    /// Number of dimensions.
    pub dimensions: usize,
    /// Additional metadata (stored as JSONB).
    pub metadata: serde_json::Value,
    /// When this record was created.
    pub created_at: DateTime<Utc>,
}

/// A prediction record.
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

/// Actual outcome for prediction evaluation.
#[derive(Debug, Clone)]
pub struct ActualOutcome {
    pub actual_value: f64,
    pub actual_breach: bool,
    pub evaluated_at: DateTime<Utc>,
}

/// Durable storage for embeddings and predictions.
///
/// Single implementation: `PostgresStorage` backed by
/// TimescaleDB + pgvector.
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// Store an embedding in the database.
    async fn store_embedding(
        &self,
        embedding: &StoredEmbedding,
    ) -> Result<(), StorageError>;

    /// Load embeddings for a domain, optionally filtered by time.
    async fn load_embeddings(
        &self,
        domain_id: &str,
        since: Option<DateTime<Utc>>,
    ) -> Result<Vec<StoredEmbedding>, StorageError>;

    /// Store a prediction.
    async fn store_prediction(
        &self,
        prediction: &Prediction,
    ) -> Result<i64, StorageError>;

    /// Get predictions awaiting outcome evaluation.
    async fn get_pending_outcomes(
        &self,
        domain_id: &str,
    ) -> Result<Vec<Prediction>, StorageError>;

    /// Record the actual outcome for a prediction.
    async fn record_outcome(
        &self,
        prediction_id: i64,
        actual: &ActualOutcome,
    ) -> Result<(), StorageError>;
}

/// Error type for storage operations.
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

**PostgresStorage implementation:**

```rust
pub struct PostgresStorage {
    client: tokio_postgres::Client,
}

impl PostgresStorage {
    /// Create a new PostgresStorage with an existing database connection.
    pub fn new(client: tokio_postgres::Client) -> Self;

    /// Create a PostgresStorage by connecting to the given URL.
    pub async fn connect(database_url: &str) -> Result<Self, StorageError>;
}
```

**SQL operations:**

- `store_embedding`: `INSERT INTO gold.metric_embeddings (bucket, domain_id, embedding, dimensions, metadata) VALUES ($1, $2, $3::vector, $4, $5) ON CONFLICT (bucket, domain_id) DO UPDATE SET embedding = EXCLUDED.embedding, metadata = EXCLUDED.metadata`
- `load_embeddings`: `SELECT bucket, domain_id, embedding::text, dimensions, metadata, created_at FROM gold.metric_embeddings WHERE domain_id = $1 AND ($2::timestamptz IS NULL OR bucket > $2) ORDER BY bucket ASC`
- `store_prediction`: `INSERT INTO gold.predictions (bucket, domain_id, metric, horizon, predicted_value, predicted_breach, confidence, k_neighbors, k_supporting) VALUES (...) RETURNING id`
- `get_pending_outcomes`: `SELECT * FROM gold.predictions WHERE domain_id = $1 AND correct IS NULL AND bucket + horizon::interval < NOW()`
- `record_outcome`: `UPDATE gold.predictions SET actual_value = $2, actual_breach = $3, correct = ($4 = predicted_breach), evaluated_at = $5 WHERE id = $1`

**Integration tests required (against real TimescaleDB):**

1. `store_embedding` then `load_embeddings` round-trip: vector data preserved
2. `store_embedding` with ON CONFLICT: update overwrites existing
3. `load_embeddings` with `since` filter: only returns newer records
4. `store_prediction` returns auto-generated ID
5. `get_pending_outcomes` returns only unevaluated predictions past their horizon
6. `record_outcome` sets `correct` field based on breach comparison

**Exit criterion:** All 6 integration tests pass against TimescaleDB with pgvector extension.

#### FR-P1-11: GraphStore Trait + Backend

| Attribute | Value |
|-----------|-------|
| **ID** | FR-P1-11 |
| **Priority** | P1 (required) |
| **Description** | Define the `GraphStore` trait and implement the appropriate backend based on Phase 0 outcome. If ruvector-graph compiles, implement `RuvectorGraphStore`. Otherwise, implement `SqlGraphStore` using SQL adjacency tables. |

**Location:**
- Trait + types: `crates/ndp-intelligence/src/graph/mod.rs`
- SQL backend: `crates/ndp-intelligence/src/graph/sql.rs`
- ruvector backend: `crates/ndp-intelligence/src/graph/ruvector.rs` (conditional)

**Trait definition:**

```rust
use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// A node in the intelligence graph.
#[derive(Debug, Clone)]
pub struct GraphNode {
    /// Unique node identifier (e.g., "metric:indoor_co2_mean").
    pub id: String,
    /// Node type (e.g., "metric", "event", "prediction").
    pub node_type: String,
    /// Arbitrary properties (stored as JSONB in SQL backend).
    pub properties: serde_json::Value,
    /// When this node was created.
    pub created_at: DateTime<Utc>,
}

/// A directed edge in the intelligence graph.
#[derive(Debug, Clone)]
pub struct GraphEdge {
    /// Source node ID.
    pub source_id: String,
    /// Target node ID.
    pub target_id: String,
    /// Edge type (e.g., "causes", "correlates_with", "precedes").
    pub edge_type: String,
    /// Edge weight (e.g., correlation strength, causal uplift).
    pub weight: f64,
    /// Arbitrary properties (e.g., lag_minutes, p_value, evidence_count).
    pub properties: serde_json::Value,
    /// When this edge was created.
    pub created_at: DateTime<Utc>,
}

/// Generic graph storage for typed nodes and edges.
///
/// Two implementations:
/// - `SqlGraphStore`: SQL adjacency tables (always available)
/// - `RuvectorGraphStore`: ruvector-graph backend (if compiled)
#[async_trait]
pub trait GraphStore: Send + Sync {
    /// Add a node. If a node with the same ID exists, update it.
    async fn add_node(&self, node: &GraphNode) -> Result<(), GraphError>;

    /// Add a typed edge between two nodes.
    /// Both source and target nodes must exist.
    async fn add_edge(&self, edge: &GraphEdge) -> Result<(), GraphError>;

    /// Get all edges from a node, optionally filtered by edge type.
    async fn get_edges(
        &self,
        node_id: &str,
        edge_type: Option<&str>,
    ) -> Result<Vec<GraphEdge>, GraphError>;

    /// Get neighbors of a node (1-hop traversal).
    async fn get_neighbors(
        &self,
        node_id: &str,
        edge_type: Option<&str>,
    ) -> Result<Vec<GraphNode>, GraphError>;

    /// Count nodes, optionally filtered by type.
    async fn node_count(
        &self,
        node_type: Option<&str>,
    ) -> Result<usize, GraphError>;

    /// Count edges, optionally filtered by type.
    async fn edge_count(
        &self,
        edge_type: Option<&str>,
    ) -> Result<usize, GraphError>;
}

/// Error type for graph operations.
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

**SqlGraphStore implementation:**

```rust
pub struct SqlGraphStore {
    client: tokio_postgres::Client,
}

impl SqlGraphStore {
    pub fn new(client: tokio_postgres::Client) -> Self;
    pub async fn connect(database_url: &str) -> Result<Self, GraphError>;
}
```

SQL operations use the `gold.graph_nodes` and `gold.graph_edges` tables (DDL generated by `PgVectorSchemaGenerator`).

**Tests required:**

For `SqlGraphStore` (integration tests against TimescaleDB):

1. `add_node` creates a node, `node_count` returns 1
2. `add_node` with same ID updates properties (upsert)
3. `add_edge` between existing nodes succeeds
4. `add_edge` with non-existent source returns `GraphError::DanglingEdge`
5. `get_edges` returns edges from a node, filtered by type
6. `get_neighbors` returns connected nodes via 1-hop traversal
7. `node_count` with type filter returns correct count
8. `edge_count` with type filter returns correct count

For `RuvectorGraphStore` (if Phase 0 passes): equivalent tests using ruvector-graph API.

**Exit criterion:** All 8 tests pass for the selected backend. Both `GraphStore` trait and implementation compile.

#### FR-P1-12: EmbeddingWriter Populator

| Attribute | Value |
|-----------|-------|
| **ID** | FR-P1-12 |
| **Priority** | P1 (required) |
| **Description** | Implement `EmbeddingWriter` that takes an `Embedding` (from `MetricEmbedder`) and writes it to a `StorageBackend` (PostgresStorage). |

**Location:** `crates/ndp-lib/src/gold/populator/mod.rs` and `crates/ndp-lib/src/gold/populator/embedding_writer.rs` (new module)

**Register in:** `crates/ndp-lib/src/gold/mod.rs` -- add `pub mod populator;`

**Struct definition:**

```rust
use crate::gold::embeddings::{Embedding, GoldRow};

/// Writes embeddings produced by an Embedder to durable storage.
///
/// Bridges the ndp-lib embedding pipeline (pure transformation)
/// with ndp-intelligence storage (database I/O).
pub struct EmbeddingWriter<S: StorageBackend> {
    storage: S,
}

impl<S: StorageBackend> EmbeddingWriter<S> {
    pub fn new(storage: S) -> Self;

    /// Write a single embedding to storage.
    ///
    /// Converts the `Embedding` + source `GoldRow` metadata
    /// into a `StoredEmbedding` and delegates to `StorageBackend`.
    pub async fn write(
        &self,
        row: &GoldRow,
        embedding: &Embedding,
    ) -> Result<(), Box<dyn std::error::Error>>;

    /// Write a batch of embeddings.
    pub async fn write_batch(
        &self,
        entries: &[(GoldRow, Embedding)],
    ) -> Result<usize, Box<dyn std::error::Error>>;
}
```

**Note:** `EmbeddingWriter` is generic over `StorageBackend` to allow testing with mock storage. The `StorageBackend` trait is defined in `ndp-intelligence`, so `EmbeddingWriter` either lives in `ndp-intelligence` or `ndp-lib` depends on `ndp-intelligence`. Given the dependency direction (`ndp-intelligence` depends on `ndp-lib`, not the reverse), `EmbeddingWriter` must live in `ndp-intelligence` crate, not `ndp-lib`.

**Revised location:** `crates/ndp-intelligence/src/populator/mod.rs` and `crates/ndp-intelligence/src/populator/embedding_writer.rs`

**Integration tests required:**

1. Write a single embedding, read it back via `StorageBackend::load_embeddings`, verify vector matches
2. Write batch of 10 embeddings, verify count matches
3. Write same bucket twice (idempotent upsert), verify only 1 record exists

**Exit criterion:** All 3 integration tests pass.

#### FR-P1-13: ndp-cli `gold intelligence` Subcommand

| Attribute | Value |
|-----------|-------|
| **ID** | FR-P1-13 |
| **Priority** | P1 (required) |
| **Description** | Add an `intelligence` subcommand to the existing `ndp gold` command in ndp-cli. The subcommand generates pgvector + graph DDL. |

**Location:** `tools/ndp-cli/src/commands/gold.rs` (extend existing)

**CLI interface:**

```
ndp gold intelligence schema [OPTIONS]
    --domain <DOMAIN_ID>    Domain to generate intelligence schema for (required)
    --graph-tables          Include SQL graph tables (default: true)
    --no-graph-tables       Exclude SQL graph tables
```

**Behavior:**

1. Load domain config via `FileSystemConfigLoader`
2. Create `PgVectorSchemaGenerator` with domain ID and graph table flag
3. Call `generate(Action::Sync)`
4. Print DDL to stdout

**Changes to existing code:**

Add a new variant to `GoldCommands` enum:

```rust
/// Generate intelligence layer schema DDL.
Intelligence {
    #[command(subcommand)]
    command: IntelligenceCommands,
}
```

```rust
#[derive(Subcommand)]
pub enum IntelligenceCommands {
    /// Generate pgvector + graph DDL for intelligence tables.
    Schema {
        /// Domain ID to generate schema for.
        #[arg(long, required = true)]
        domain: String,

        /// Exclude SQL graph adjacency tables.
        #[arg(long)]
        no_graph_tables: bool,
    },
}
```

**Tests required:**

1. `ndp gold intelligence schema --domain indoor-air-quality` outputs valid SQL containing `CREATE TABLE IF NOT EXISTS gold.metric_embeddings`
2. `ndp gold intelligence schema --domain indoor-air-quality --no-graph-tables` output does NOT contain `gold.graph_nodes`
3. Output contains `CREATE EXTENSION IF NOT EXISTS vector`
4. `ndp gold intelligence --help` prints usage

**Exit criterion:** All 4 tests pass. Command is accessible from `ndp gold intelligence schema`.

---

## 2. Data Requirements

### 2.1 GoldRow Structure and Field Mapping

The `GoldRow` struct maps fields from the `gold.indoor_air_quality_aligned_hourly` materialized view. The mapping is domain-agnostic -- fields are identified by string name, not by struct fields.

**Example mapping for indoor-air-quality domain:**

| Aligned View Column | GoldRow Field Key | Type | Notes |
|---------------------|-------------------|------|-------|
| `bucket` | `GoldRow.bucket` | `DateTime<Utc>` | Dedicated struct field, not in fields map |
| `indoor_co2_mean` | `"indoor_co2_mean"` | `Option<f64>` | Direct field |
| `indoor_pm25_mean` | `"indoor_pm25_mean"` | `Option<f64>` | Direct field |
| `indoor_temperature_c_mean` | `"indoor_temperature_c_mean"` | `Option<f64>` | Direct field |
| `indoor_humidity_pct_mean` | `"indoor_humidity_pct_mean"` | `Option<f64>` | Direct field |
| `outdoor_temperature_c_mean` | `"outdoor_temperature_c_mean"` | `Option<f64>` | Direct field |
| `outdoor_humidity_pct_mean` | `"outdoor_humidity_pct_mean"` | `Option<f64>` | Direct field |
| `outdoor_wind_speed_mean` | `"outdoor_wind_speed_mean"` | `Option<f64>` | Direct field |
| `outdoor_aqi_pm25_mean` | `"outdoor_aqi_pm25_mean"` | `Option<f64>` | Direct field |

Derived features (from feature registry):
- `indoor_co2_mean_trend_4h`, `indoor_pm25_mean_trend_4h`, `indoor_co2_mean_std_4h`, `indoor_co2_mean_diff_1h`

All fields use `Option<f64>` because the aligned view uses FULL OUTER JOIN, which produces NULLs when a stream has no data for a given time bucket.

### 2.2 Embedding Vector Format

| Attribute | Value |
|-----------|-------|
| Element type | `f32` |
| Default dimensions (indoor-air-quality) | 15 (3 temporal + 8 direct + 4 derived) |
| Minimum dimensions | 4 (3 temporal + 1 field) |
| Maximum dimensions | No hard limit; practical limit ~64 for HNSW efficiency |
| Normalization | Z-score per field |
| Temporal components | `hour_sin`, `hour_cos`, `is_weekend` (3 dimensions) |

### 2.3 Temporal Encoding Formulas

```
hour_sin  = sin(2 * PI * hour_of_day / 24.0)    // f32
hour_cos  = cos(2 * PI * hour_of_day / 24.0)    // f32
is_weekend = if weekday >= Saturday { 1.0 } else { 0.0 }  // f32
```

Where `hour_of_day` is `bucket.hour()` as `f32` (0.0 through 23.0) and weekday is from `bucket.weekday()`.

### 2.4 pgvector Table Schemas

All tables reside in the `gold` schema. DDL is generated by `PgVectorSchemaGenerator`.

#### gold.metric_embeddings

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
    if_not_exists => TRUE);
```

The `embedding` column uses pgvector's `vector` type without a fixed dimension constraint. The dimension is stored separately in the `dimensions` column. This allows different domains to have different embedding sizes without separate tables.

#### gold.predictions

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

#### gold.graph_nodes (SQL fallback)

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

#### gold.graph_edges (SQL fallback)

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

#### gold.reasoning_bank (V1.3 prep, empty)

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

This table is created by the generator but is NOT read or written in V1.2. It prepares storage for ruvector SONA (LoRA adapters + EWC++ Fisher information) in V1.3.

### 2.5 Intelligence Config Schema in domain.json

The `intelligence` block is added as an optional field in domain.json. Full example for indoor-air-quality (not deployed until Phase 2, but config types are defined in Phase 1):

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

**Phase 1 constraint:** The `config/domains/indoor-air-quality/domain.json` file is NOT modified. The config types and deserialization are validated via unit tests with inline JSON. The actual domain.json receives the `intelligence` block when the runtime is implemented in Phase 2.

---

## 3. Interface Requirements

### 3.1 Embedder Trait

Defined in FR-P1-03. Full signature repeated here for implementor reference:

```rust
// Location: crates/ndp-lib/src/gold/embeddings/mod.rs

pub trait Embedder: Send + Sync {
    fn embed(&self, row: &GoldRow) -> EmbeddingResult<Embedding>;
    fn dimensions(&self) -> usize;
    fn name(&self) -> &str;
}
```

**Semantics:**

- `embed()` is a pure function. It performs no I/O. It must not block or make network calls.
- `embed()` returns `Err(EmbeddingError::InsufficientData)` if the embedder needs more observations (warmup period not met).
- `embed()` returns `Err(EmbeddingError::FieldNotFound)` if a configured field is missing from the `GoldRow.fields` map.
- `dimensions()` returns a constant for the lifetime of the embedder (set at construction time).
- `name()` returns a static string identifier (e.g., `"MetricEmbedder"`).
- Thread-safety: `Send + Sync` required. `MetricEmbedder` achieves this because `observe()` takes `&mut self` (exclusive access), while `embed()` takes `&self` (shared access). In practice, the caller must use interior mutability (e.g., `RwLock<MetricEmbedder>`) if `observe()` and `embed()` are called from different threads.

### 3.2 SimilarityEngine Trait (Stub)

Defined as a trait only in Phase 1. No implementations until Phase 2.

```rust
// Location: crates/ndp-intelligence/src/similarity/mod.rs

/// Backend for vector similarity search.
///
/// Phase 1: Trait definition only. No implementations.
/// Phase 2: HnswEngine (ruvector-core) + PgVectorEngine (SQL fallback).
pub trait SimilarityEngine: Send + Sync {
    /// Insert a vector with metadata.
    fn insert(&mut self, entry: VectorEntry) -> Result<(), SimilarityError>;

    /// K-NN search.
    fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>, SimilarityError>;

    /// Number of vectors in the index.
    fn count(&self) -> usize;

    /// Rebuild index from durable storage (startup recovery).
    fn rebuild_from_storage(
        &mut self,
        storage: &dyn StorageBackend,
    ) -> Result<usize, SimilarityError>;
}

#[derive(Debug, Clone)]
pub struct VectorEntry {
    /// Unique ID: "{domain_id}:{bucket_iso}"
    pub id: String,
    /// The embedding vector.
    pub vector: Vec<f32>,
    /// Metadata stored alongside the vector.
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct SearchQuery {
    /// Query vector.
    pub vector: Vec<f32>,
    /// Number of neighbors to return.
    pub k: usize,
    /// Minimum similarity threshold (0.0 - 1.0).
    pub min_similarity: f64,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    /// ID of the matching vector.
    pub id: String,
    /// Cosine similarity score (0.0 - 1.0).
    pub similarity: f64,
    /// Metadata from the stored vector.
    pub metadata: serde_json::Value,
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

**Phase 1 scope:** Trait definition, type definitions, error enum. No implementations. The trait is compiled and tested only to ensure it is well-formed (the module compiles without errors).

### 3.3 StorageBackend Trait

Defined in FR-P1-10. Async trait using `async_trait`. See section 1.2 (FR-P1-10) for full signature.

### 3.4 GraphStore Trait

Defined in FR-P1-11. Async trait using `async_trait`. See section 1.2 (FR-P1-11) for full signature.

### 3.5 CLI Interface

```
ndp gold intelligence schema --domain <DOMAIN_ID> [--no-graph-tables]
```

See FR-P1-13 for full specification.

**Output format:** Raw SQL DDL printed to stdout. No JSON wrapping. Comments at top indicate generator version and domain ID.

---

## 4. Non-Functional Requirements

### NFR-01: ARM64 Compilation

All new code (ndp-intelligence, ndp-intelligence-app, ndp-lib extensions) must compile for `aarch64-unknown-linux-gnu`. This is validated by Phase 0 for ruvector dependencies and by the existing NDP Docker build (which targets aarch64 on Pi 5).

New Dockerfile changes (pgvector installation) must work on arm64. The `postgresql-15-pgvector` package is available as a pre-built arm64 apt package.

### NFR-02: Memory Budget

| Component | Budget | Measurement Method |
|-----------|--------|--------------------|
| ndp-intelligence-app binary | < 15 MB | `ls -la target/release/ndp-intelligence-app` |
| HNSW index (10K 32D vectors) | < 2 MB | Peak RSS minus baseline after 10K inserts |
| Total Phase 1 library memory | < 5 MB | Not applicable until runtime exists (Phase 2) |

Phase 1 does not deploy a running daemon, so memory budget is a design constraint, not a measured acceptance criterion. The constraint informs type choices (e.g., `Vec<f32>` not `Vec<f64>`, `BTreeMap` not `HashMap` for GoldRow).

### NFR-03: Backward Compatibility

Adding `intelligence: Option<IntelligenceConfig>` to `DomainConfig` MUST NOT break:

1. Deserialization of existing `config/domains/indoor-air-quality/domain.json` (which has no `intelligence` key)
2. Any existing unit test in `crates/ndp-lib/src/gold/config/domain.rs`
3. Any existing integration test that loads domain configs
4. The `ndp gold generate --domain indoor-air-quality` CLI command
5. The `ndp gold sync --domain indoor-air-quality` CLI command

This is enforced by `#[serde(default)]` on the new field, which defaults to `None`.

### NFR-04: Test Coverage

All new code must have unit tests. Specific test counts per deliverable are specified in the functional requirements. Summary:

| Deliverable | Minimum Unit Tests | Integration Tests |
|-------------|-------------------|-------------------|
| P1-03 (Embedder trait) | 4 | 0 |
| P1-04 (MetricEmbedder) | 10 | 0 |
| P1-05 (RunningStats) | 7 | 0 |
| P1-06 (EmbeddingConfig) | 6 | 0 |
| P1-07 (DomainConfig ext) | 3 new + 3 existing | 0 |
| P1-08 (PgVectorSchemaGen) | 9 | 0 |
| P1-09 (pgvector Docker) | 0 | 1 (container) |
| P1-10 (StorageBackend) | 0 | 6 |
| P1-11 (GraphStore) | 0 | 8 |
| P1-12 (EmbeddingWriter) | 0 | 3 |
| P1-13 (CLI) | 4 | 0 |
| **Total** | **43+** | **18+** |

Integration tests (P1-10, P1-11, P1-12) require the integration environment (TimescaleDB with pgvector running). They should be gated behind a feature flag or `#[cfg(feature = "integration")]` / `#[ignore]` attribute with documentation on how to run them.

### NFR-05: Code Quality

- All public types and functions must have doc comments (Rust `///` comments).
- No `todo!()`, `unimplemented!()`, or placeholder functions. If a trait method is Phase 2 scope, the trait is defined but no implementation exists.
- All code must pass `cargo clippy` without warnings.
- All code must be formatted with `cargo fmt`.

---

## 5. Constraints

### C-01: No Runtime Intelligence Cycle

Phase 1 builds infrastructure only. There is no intelligence loop, no PG LISTEN/NOTIFY, no periodic processing. The `ndp-intelligence-app` binary compiles and prints help, but its subcommands (`daemon`, `one-shot`, `backfill`, `status`) print "Not implemented in Phase 1" and exit 0.

### C-02: No Deployment to Pi

Phase 1 produces library crates and CLI extensions. No Docker container is deployed. No changes to `docker-compose.yml` (except pgvector in TimescaleDB). No changes to `deploy/pi/deploy.sh`.

### C-03: Phase 0 Determines Backend Selection

The `GraphStore` implementation in P1-11 depends on Phase 0 outcome:

- If ruvector-graph compiles: implement `RuvectorGraphStore` as primary, `SqlGraphStore` as fallback.
- If ruvector-graph fails: implement `SqlGraphStore` only. `PgVectorSchemaGenerator` generates graph tables.

The implementation approach (conditional compilation via Cargo features, or runtime dispatch) is an implementation decision, not a specification concern. Both approaches are acceptable as long as the `GraphStore` trait is the stable interface.

### C-04: Follows Existing ndp-lib Patterns

New code must follow established patterns:

1. **Parsed structs, not file paths:** Functions receive parsed `DomainConfig`, `StreamConfig`, etc. -- not raw file paths. (Pattern from `crates/ndp-lib/src/gold/mod.rs` public API.)
2. **ConfigLoader trait for config access:** Use `ConfigLoader::load_domain_config()`, not direct file reads. (Pattern from `crates/ndp-lib/src/gold/config/loader.rs`.)
3. **Generator pattern:** `PgVectorSchemaGenerator` follows the same `from_*_config()` + `generate(action)` pattern as `ContinuousAggregateGenerator`. (Pattern from `crates/ndp-lib/src/gold/generators/continuous_aggregate.rs`.)
4. **Error types:** Use `thiserror` derive macro. Follow `GoldDdlError` pattern with error codes. (Pattern from `crates/ndp-lib/src/gold/error.rs`.)
5. **Module organization:** Public types re-exported from `mod.rs`. Implementation in separate files. (Pattern from `crates/ndp-lib/src/gold/config/mod.rs`.)
6. **Workspace dependencies:** Use `{ workspace = true }` for shared dependencies. (Pattern from `/workspaces/neural-data-platform/Cargo.toml`.)

### C-05: No Modifications Outside Scope

Phase 1 changes are limited to:

| Path | Change Type |
|------|-------------|
| `Cargo.toml` (workspace root) | Add 2 workspace members |
| `crates/ndp-intelligence/` | New crate (entire directory) |
| `apps/ndp-intelligence-app/` | New crate (entire directory) |
| `crates/ndp-lib/src/gold/mod.rs` | Add `pub mod embeddings;` and `pub mod populator;` |
| `crates/ndp-lib/src/gold/embeddings/` | New module (entire directory) |
| `crates/ndp-lib/src/gold/config/domain.rs` | Add `intelligence` field to `DomainConfig` |
| `crates/ndp-lib/src/gold/generators/mod.rs` | Add `pub mod pgvector_schema;` and re-export |
| `crates/ndp-lib/src/gold/generators/pgvector_schema.rs` | New file |
| `tools/ndp-cli/src/commands/gold.rs` | Add `Intelligence` variant to `GoldCommands` |
| `tools/ndp-cli/src/commands/mod.rs` | No change needed (gold module already registered) |
| `docker/timescaledb/Dockerfile` | Add pgvector package |
| Docker init scripts | Add `CREATE EXTENSION IF NOT EXISTS vector` |

No other files are modified. No changes to ingestion pipeline, Bronze layer, Silver layer, or existing Gold generators.

---

## 6. Acceptance Criteria

### Phase 0 Acceptance Criteria

| ID | Criterion | Verification Method |
|----|-----------|-------------------|
| AC-P0-01 | Go/no-go report exists at `product/features/fe-003/reports/phase0-go-no-go.md` | File exists with required sections |
| AC-P0-02 | Report documents compilation outcome for ruvector-core | Report contains pass/fail/partial for ruvector-core |
| AC-P0-03 | Report documents compilation outcome for ruvector-graph | Report contains pass/fail for ruvector-graph |
| AC-P0-04 | Report documents performance measurements | Report contains memory, latency, build time, binary size |
| AC-P0-05 | Report states clear backend decision | Report states which backend (ruvector vs pgvector-only, ruvector-graph vs SQL) |

### Phase 1 Acceptance Criteria

#### Per-Deliverable

| ID | Deliverable | Criterion | Verification |
|----|-------------|-----------|-------------|
| AC-P1-01 | ndp-intelligence crate | `cargo build -p ndp-intelligence` exits 0 | CI build |
| AC-P1-02 | ndp-intelligence-app | `cargo build -p ndp-intelligence-app` exits 0; `--help` prints usage | CLI test |
| AC-P1-03 | Embedder trait | `GoldRow`, `Embedding`, `Embedder` compile; 4 unit tests pass | `cargo test -p ndp-lib` |
| AC-P1-04 | MetricEmbedder | 10 unit tests pass (z-score, NULL, temporal, warmup) | `cargo test -p ndp-lib` |
| AC-P1-05 | RunningStats | 7 unit tests pass (mean, std, z_score, convergence) | `cargo test -p ndp-lib` |
| AC-P1-06 | EmbeddingConfig | 6 deserialization tests pass | `cargo test -p ndp-lib` |
| AC-P1-07 | DomainConfig ext | 3 existing tests pass; 3 new tests pass | `cargo test -p ndp-lib` |
| AC-P1-08 | PgVectorSchemaGen | 9 DDL generation tests pass | `cargo test -p ndp-lib` |
| AC-P1-09 | pgvector Docker | `SELECT * FROM pg_extension WHERE extname = 'vector'` returns 1 row | Integration env |
| AC-P1-10 | StorageBackend | 6 integration tests pass | `cargo test -p ndp-intelligence` (integration) |
| AC-P1-11 | GraphStore | 8 integration tests pass | `cargo test -p ndp-intelligence` (integration) |
| AC-P1-12 | EmbeddingWriter | 3 integration tests pass | `cargo test -p ndp-intelligence` (integration) |
| AC-P1-13 | CLI | `ndp gold intelligence schema --domain indoor-air-quality` outputs valid SQL | CLI test |

#### Cross-Cutting

| ID | Criterion | Verification |
|----|-----------|-------------|
| AC-X-01 | All existing tests pass | `cargo test --workspace` exits 0 (904+ tests) |
| AC-X-02 | No clippy warnings | `cargo clippy --workspace` exits 0 |
| AC-X-03 | Backward compatibility | `config/domains/indoor-air-quality/domain.json` deserializes with `intelligence: None` |
| AC-X-04 | No new crate compilation errors | `cargo build --workspace` exits 0 |
| AC-X-05 | Workspace members updated | `Cargo.toml` workspace.members includes `crates/ndp-intelligence` and `apps/ndp-intelligence-app` |

#### Integration Test Requirements

Integration tests (P1-09, P1-10, P1-11, P1-12) require:

1. TimescaleDB running with pgvector extension loaded
2. `gold` schema created
3. Intelligence tables created (via `PgVectorSchemaGenerator` DDL)
4. `DATABASE_URL` or `TIMESCALE_URL` environment variable set

These tests should be gated behind `#[ignore]` and run explicitly:

```bash
# Start integration environment
docker compose -f docker-compose.integration.yml up -d

# Run integration tests
cargo test -p ndp-intelligence -- --ignored
```

#### Backward Compatibility Validation

Run the full existing test suite to confirm no regressions:

```bash
cargo test --workspace
# Must report 904+ tests passing (current baseline)
# Zero new failures
```

---

## 7. Dependency Summary

### New Workspace Members

```toml
# Added to Cargo.toml workspace.members
"crates/ndp-intelligence",
"apps/ndp-intelligence-app",
```

### New Dependencies (not already in workspace)

| Dependency | Version | Used By | Purpose |
|------------|---------|---------|---------|
| `ndarray` | 0.16 | ndp-intelligence | Matrix operations (Phase 3 Granger, but declare now) |

All other dependencies (`tokio`, `serde`, `chrono`, `tokio-postgres`, `async-trait`, `clap`, `thiserror`, `tracing`, `tracing-subscriber`, `serde_json`) are already in the workspace `[workspace.dependencies]`.

### Conditional Dependencies (Phase 0 outcome)

| Dependency | Version | Condition |
|------------|---------|-----------|
| `ruvector-core` | 2.0.1 | Phase 0: ruvector-core compiles on aarch64 |
| `ruvector-graph` | 0.1 | Phase 0: ruvector-graph compiles on aarch64 |

If Phase 0 determines these do not compile, they are excluded. The `SimilarityEngine` trait and `GraphStore` trait remain defined; only their implementations change.

---

## 8. File Inventory

Complete list of files created or modified by Phase 1:

### New Files

```
crates/ndp-intelligence/Cargo.toml
crates/ndp-intelligence/src/lib.rs
crates/ndp-intelligence/src/error.rs
crates/ndp-intelligence/src/similarity/mod.rs
crates/ndp-intelligence/src/graph/mod.rs
crates/ndp-intelligence/src/graph/sql.rs
crates/ndp-intelligence/src/graph/ruvector.rs          (conditional)
crates/ndp-intelligence/src/storage/mod.rs
crates/ndp-intelligence/src/storage/postgres.rs
crates/ndp-intelligence/src/populator/mod.rs
crates/ndp-intelligence/src/populator/embedding_writer.rs
apps/ndp-intelligence-app/Cargo.toml
apps/ndp-intelligence-app/src/main.rs
crates/ndp-lib/src/gold/embeddings/mod.rs
crates/ndp-lib/src/gold/embeddings/metric.rs
crates/ndp-lib/src/gold/embeddings/stats.rs
crates/ndp-lib/src/gold/embeddings/config.rs
crates/ndp-lib/src/gold/generators/pgvector_schema.rs
product/features/fe-003/reports/phase0-go-no-go.md     (Phase 0 output)
```

### Modified Files

```
Cargo.toml                                              (workspace members)
crates/ndp-lib/src/gold/mod.rs                          (add pub mod embeddings, populator)
crates/ndp-lib/src/gold/config/domain.rs                (add intelligence field)
crates/ndp-lib/src/gold/config/mod.rs                   (re-export IntelligenceConfig)
crates/ndp-lib/src/gold/generators/mod.rs               (add pgvector_schema module)
tools/ndp-cli/src/commands/gold.rs                      (add Intelligence subcommand)
docker/timescaledb/Dockerfile                           (add pgvector package)
```

---

## 9. Risk and Mitigation

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|-----------|
| ruvector-core fails aarch64 compilation | Medium | Medium | pgvector-only fallback. SimilarityEngine trait abstracts backend. |
| ruvector-graph fails aarch64 compilation | Medium | Low | SqlGraphStore provides identical GraphStore interface via SQL adjacency tables. |
| pgvector `vector` type incompatible with tokio-postgres | Low | Medium | Use text serialization (`::text` cast) for vector data. Parse `[0.1,0.2,...]` format. |
| DomainConfig backward compatibility break | Low | High | `#[serde(default)]` ensures None default. Validated by 3 existing + 3 new tests. |
| Integration tests flaky (TimescaleDB connection) | Medium | Low | Use `#[ignore]` gate. Retry logic in test setup. |
| ndp-lib dependency on ndp-intelligence (circular) | N/A | N/A | Prevented by design: EmbeddingWriter lives in ndp-intelligence, not ndp-lib. Dependency is one-way: ndp-intelligence -> ndp-lib. |

---

## 10. Glossary

| Term | Definition |
|------|-----------|
| **GoldRow** | A single row from the Gold aligned view, represented as named numeric fields |
| **Embedding** | A fixed-length `Vec<f32>` representing a GoldRow in vector space |
| **Embedder** | Trait that transforms a GoldRow into an Embedding |
| **MetricEmbedder** | Embedder implementation using z-score normalization + temporal encoding |
| **RunningStats** | Exponential moving average tracker for mean and standard deviation |
| **StorageBackend** | Trait for durable storage of embeddings and predictions in PostgreSQL |
| **GraphStore** | Trait for graph CRUD operations (nodes and edges) |
| **PgVectorSchemaGenerator** | DDL generator for intelligence tables (follows ContinuousAggregateGenerator pattern) |
| **HNSW** | Hierarchical Navigable Small World -- approximate nearest neighbor search algorithm |
| **K-NN** | K-Nearest Neighbors search |
| **Z-score** | `(value - mean) / std` -- normalizes values to comparable scales |
| **Warmup** | Initial period (168 hours) where statistics are collected but no predictions are made |

---

*Specification for fe-003 Intelligence Foundation Phase 0 + Phase 1. This document is the input for implementation agents. Reference SCOPE.md for constraints and ARCHITECTURE.md for full system context.*
