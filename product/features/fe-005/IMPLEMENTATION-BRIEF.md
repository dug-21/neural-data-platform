# fe-005: Implementation Brief -- Text Embeddings

## SPARC Artifacts

| Artifact | Path |
|----------|------|
| Scope | product/features/fe-005/SCOPE.md |
| Specification | product/features/fe-005/specification/SPECIFICATION.md |
| Task Decomposition | product/features/fe-005/specification/TASK-DECOMPOSITION.md |
| Architecture (ADRs) | product/features/fe-005/architecture/ARCHITECTURE.md |
| Pseudocode Overview | product/features/fe-005/pseudocode/OVERVIEW.md |
| Pseudocode: ndp-lib | product/features/fe-005/pseudocode/ndp-lib.md |
| Pseudocode: ndp-embedder | product/features/fe-005/pseudocode/ndp-embedder.md |
| Pseudocode: deploy | product/features/fe-005/pseudocode/deploy.md |
| Pseudocode: config | product/features/fe-005/pseudocode/config.md |
| Pseudocode: database | product/features/fe-005/pseudocode/database.md |
| Test Plan Overview | product/features/fe-005/test-plan/OVERVIEW.md |
| Test Plan: ndp-lib | product/features/fe-005/test-plan/ndp-lib.md |
| Test Plan: ndp-embedder | product/features/fe-005/test-plan/ndp-embedder.md |
| Test Plan: deploy | product/features/fe-005/test-plan/deploy.md |
| Alignment Report | product/features/fe-005/ALIGNMENT-REPORT.md |
| Acceptance Map | product/features/fe-005/ACCEPTANCE-MAP.md |
| Launch Prompt | product/features/fe-005/LAUNCH-PROMPT.md |

## Component Map

| Component | Pseudocode | Test Plan |
|-----------|-----------|-----------|
| ndp-lib | pseudocode/ndp-lib.md | test-plan/ndp-lib.md |
| ndp-embedder | pseudocode/ndp-embedder.md | test-plan/ndp-embedder.md |
| deploy | pseudocode/deploy.md | test-plan/deploy.md |
| config | pseudocode/config.md | test-plan/deploy.md |
| database | pseudocode/database.md | test-plan/deploy.md |

## Goal

Add a model-agnostic text embedding service to the intelligence layer. A new `ndp-embedder` container runs ONNX inference on text data from Gold text views (produced by dp-023), generating 384-dimensional vector embeddings stored in `gold.text_embeddings`. The service is domain-configurable, supports multiple models via volume-mounted storage with download-on-first-use, and includes a preprocessing pipeline for future domain-specific text preparation. This enables fe-006 (composite similarity search combining metric + text embeddings).

## Tracking

- GitHub Issue: https://github.com/dug-21/neural-data-platform/issues/39
- Version target: v1.2.x
- Dependency: dp-023 (Gold text view, GH Issue #37)
- Predecessor: fe-004 (similarity intelligence, deployed v1.2.0-v1.2.6)

## Resolved Decisions

| Decision | Resolution | Source | Pattern ID |
|----------|-----------|--------|-----------|
| TextEmbedder trait design | Separate trait from Embedder, operates on &[&str], anyhow::Result, in text.rs | ADR-001 | 25 |
| OnnxEmbedder implementation | ort v2 + tokenizers crate, 2 intra-op threads, mean pooling + L2 norm | ADR-002 | 26 |
| Container architecture | New apps/ndp-embedder crate, docker/embedder/Dockerfile, intelligence profile, 512MB | ADR-003 | 27 |
| Model storage and loading | Volume mount /models/{model_id}/, download-on-first-use from HuggingFace, 3 retries | ADR-004 | 28 |
| Text embeddings schema | gold.text_embeddings via init-script 004, hypertable 7-day chunks, HNSW m=16 ef=64 | ADR-005 | 29 |
| dp-023 interface | Read gold.{domain}_text_latest VIEW, timer polling, graceful degradation on missing view | ADR-006 | 30 |
| Preprocessing pipeline | TextPreprocessor trait, PassthroughPreprocessor, factory with fallback | ADR-007 | 31 |
| Domain schema extension | text_embedding top-level property (not under intelligence), optional, backward compatible | ADR-008 | 32 |

## Files to Create/Modify

### New Files

| File | Description |
|------|-------------|
| `crates/ndp-lib/src/gold/embeddings/text.rs` | TextEmbedder trait + TextEmbeddingError types |
| `crates/ndp-lib/src/gold/embeddings/onnx.rs` | OnnxEmbedder implementation (ort + tokenizers) |
| `crates/ndp-lib/src/gold/embeddings/preprocessing.rs` | TextPreprocessor trait + PassthroughPreprocessor + factory |
| `crates/ndp-lib/src/gold/embeddings/model_manager.rs` | ModelManager with ensure_model() + download logic |
| `crates/ndp-lib/src/gold/embeddings/text_config.rs` | TextEmbeddingConfig + PreprocessingConfig types |
| `apps/ndp-embedder/Cargo.toml` | New workspace member for embedding service |
| `apps/ndp-embedder/src/main.rs` | CLI entry point with daemon subcommand |
| `apps/ndp-embedder/src/service.rs` | EmbeddingService with run_cycle() |
| `docker/embedder/Dockerfile` | Multi-stage Dockerfile for ndp-embedder |
| `deploy/pi/init-scripts/004-text-embeddings.sql` | DDL for gold.text_embeddings table |
| `tests/fixtures/models/test-model/` | Tiny ONNX model fixture for testing |

### Modified Files

| File | Description |
|------|-------------|
| `crates/ndp-lib/src/gold/embeddings/mod.rs` | Add pub mod text, onnx, preprocessing, model_manager, text_config |
| `crates/ndp-lib/Cargo.toml` | Add ort, tokenizers, ndarray, reqwest dependencies |
| `config/schemas/domain.schema.json` | Add text_embedding definition and property |
| `deploy/pi/docker-compose.yml` | Add ndp-embedder service + embedder-models volume |
| `Cargo.toml` | Add apps/ndp-embedder to workspace members |

### Verified Unchanged

| File | Why |
|------|-----|
| `crates/ndp-intelligence/` | Completely separate -- metric embedding path untouched |
| `crates/ndp-lib/src/gold/embeddings/metric.rs` | MetricEmbedder unchanged |
| `crates/ndp-lib/src/gold/embeddings/config.rs` | IntelligenceConfig unchanged |

## Data Structures

### TextEmbedder Trait (new, in text.rs)

```rust
pub trait TextEmbedder: Send + Sync {
    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;
    fn dimensions(&self) -> usize;
}
```

### OnnxEmbedder (new, in onnx.rs)

```rust
pub struct OnnxEmbedder {
    session: ort::Session,
    tokenizer: tokenizers::Tokenizer,
    dimensions: usize,
    max_length: usize,
}
```

### TextPreprocessor Trait (new, in preprocessing.rs)

```rust
pub trait TextPreprocessor: Send + Sync {
    fn preprocess(&self, text: &str) -> String;
    fn name(&self) -> &str;
}
```

### TextEmbeddingConfig (new, in text_config.rs)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextEmbeddingConfig {
    pub model: String,
    pub quantization: String,     // default: "int8"
    pub dimensions: usize,
    pub preprocessing: PreprocessingConfig,
}
```

### ModelManager (new, in model_manager.rs)

```rust
pub struct ModelManager {
    volume_path: PathBuf,
}

pub struct ModelPaths {
    pub model: PathBuf,      // model.onnx
    pub tokenizer: PathBuf,  // tokenizer.json
}
```

### EmbeddingService (new, in apps/ndp-embedder/src/service.rs)

```rust
pub struct EmbeddingService {
    pool: Arc<Pool>,
    embedder: Box<dyn TextEmbedder>,
    preprocessor: Box<dyn TextPreprocessor>,
    domain_id: String,
    config: TextEmbeddingConfig,
    last_processed: Option<DateTime<Utc>>,
}
```

### gold.text_embeddings Table

```sql
CREATE TABLE gold.text_embeddings (
    id              BIGSERIAL,
    bucket          TIMESTAMPTZ NOT NULL,
    domain_id       TEXT NOT NULL,
    source_stream   TEXT NOT NULL,
    source_column   TEXT NOT NULL,
    source_text     TEXT NOT NULL,
    embedding       vector(384) NOT NULL,
    model_id        TEXT NOT NULL,
    retention_tier  SMALLINT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (id, bucket)
);
```

## Function Signatures

### ndp-lib

```rust
// text.rs
pub trait TextEmbedder: Send + Sync {
    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;
    fn dimensions(&self) -> usize;
}

// onnx.rs
impl OnnxEmbedder {
    pub fn new(model_path: &Path, tokenizer_path: &Path, dimensions: usize) -> Result<Self>;
}

// preprocessing.rs
pub trait TextPreprocessor: Send + Sync {
    fn preprocess(&self, text: &str) -> String;
    fn name(&self) -> &str;
}
pub fn create_preprocessor(preprocessing_type: &str) -> Box<dyn TextPreprocessor>;

// model_manager.rs
impl ModelManager {
    pub fn new(volume_path: &Path) -> Self;
    pub async fn ensure_model(&self, model_id: &str) -> Result<ModelPaths>;
}
```

### ndp-embedder

```rust
// service.rs
impl EmbeddingService {
    pub fn new(pool: Arc<Pool>, embedder: Box<dyn TextEmbedder>,
               preprocessor: Box<dyn TextPreprocessor>,
               domain_id: String, config: TextEmbeddingConfig) -> Self;
    pub async fn run_cycle(&mut self) -> Result<CycleSummary>;
}
```

## Test Expectations

### Unit Tests (~35)

- TextEmbedder trait object safety, error display (2)
- OnnxEmbedder: load, dimensions, batch, empty, L2 norm, long text, similarity, invalid path (8)
- Preprocessing: passthrough identity/empty/unicode/name, factory passthrough/empty/unknown, object safety (8)
- ModelManager: existing model, download failure, path resolution, dir creation, multiple models (5)
- Config: deserialize, defaults, round-trip, requires model, requires dimensions, preprocessing default (6)
- AppConfig: from_env, requires db_url, requires domain, custom poll, default model path (5)
- CycleSummary: default, display (2)

### Integration Tests (~13)

- Service: process rows, track last_processed, missing view, empty results, applies preprocessing, embedding error, provenance columns, relation_not_found (8)
- Pipeline: end-to-end, idempotency, multiple columns (3)
- DDL: table exists, is hypertable, HNSW index, insert/retrieve, retention_tier nullable (5 -- some overlap with deploy tests)

### Schema Tests (~3)

- Domain schema with text_embedding, without text_embedding (backward compat), requires fields

## Constraints

- ARM64 (aarch64-unknown-linux-gnu) -- all dependencies must compile
- 512MB container memory limit (model ~33MB INT8 + runtime)
- No ONNX types in TextEmbedder trait signature
- Config-driven via domain.json text_embedding block
- No cloud dependencies for core functionality (download is first-use only)
- Inference < 500ms cold start on Pi 5
- Volume mount model storage -- no models baked into container image
- INT8 quantized models must be supported
- Must NOT break existing ndp-intelligence (metric embedding path unchanged)

## Dependencies

### Crate Dependencies (new for ndp-lib)

| Crate | Version | Purpose |
|-------|---------|---------|
| ort | 2.x | ONNX Runtime bindings |
| tokenizers | 0.20.x | HuggingFace tokenizer |
| ndarray | 0.16.x | Array operations for mean pooling |

### Crate Dependencies (new for ndp-embedder)

| Crate | Version | Purpose |
|-------|---------|---------|
| ndp-lib | path | Library types (TextEmbedder, OnnxEmbedder, etc.) |
| ndp-types | path | Shared types |
| config-client | path | etcd config loading |
| deadpool-postgres | 0.14 | Connection pooling |
| pgvector | 0.4.x | pgvector type for Rust |

### Feature Dependencies

| Feature | Dependency | Status |
|---------|-----------|--------|
| dp-023 | Gold text view | Planning complete (GH #37), not yet implemented |
| fe-004 | pgvector extension, gold schema | Deployed (v1.2.6) |
| ops-008 | Init-script pattern | Implemented (GH #22) |

## NOT in Scope

- Model selection (decided at implementation time)
- Static/non-transformer embedding models
- Template caching (E-03 deferred)
- Text feature extraction DDL (E-06 deferred)
- Token chunking (deferred, model choice may solve)
- Delta embeddings (fe-006)
- PCA / dimensionality reduction (fe-006)
- EWMA temporal smoothing (fe-006)
- CompositeEmbedder (fe-006)
- Tiered retention lifecycle (fe-006)
- Granger causality (fe-007)
- Anomaly detection / dashboards (fe-008)
- SONA learning (fe-009)

## Alignment Status

**Overall: PASS** (from ALIGNMENT-REPORT.md)

Two WARN items:
- WARN-001: Model download requires internet for first use (mitigated by pre-staging option)
- WARN-002: ort crate ARM64 compatibility unverified on Pi 5 (fallback: tract crate)

No FAIL or VARIANCE items.

## Wave Structure

### Wave 1: Library Foundation
- W1-T1: TextEmbedder trait (M)
- W1-T2: TextPreprocessor + PassthroughPreprocessor (S)
- W1-T3: OnnxEmbedder implementation (L)
- W1-T4: Model manager (M)
- W1-T5: Text embedding config types (S)

### Wave 2: Infrastructure
- W2-T1: gold.text_embeddings DDL init-script (M)
- W2-T2: Domain schema update (S)
- W2-T3: ndp-embedder crate skeleton (M)
- W2-T4: Dockerfile (S)

### Wave 3: Integration
- W3-T1: ndp-embedder service implementation (M)
- W3-T2: Compose entry (S)
- W3-T3: deploy.sh integration (S)
- W3-T4: Integration testing (M)
