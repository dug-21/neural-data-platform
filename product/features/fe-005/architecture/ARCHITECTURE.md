# fe-005: Architecture Decisions

## Summary

Eight architectural decisions for the Text Embeddings feature. Each ADR was produced after consulting the existing codebase -- file paths and types reference the code as of 2026-02-17.

## ADR-001: TextEmbedder Trait Design

### Context

The existing embedding infrastructure lives in `crates/ndp-lib/src/gold/embeddings/`:
- `mod.rs` defines the `Embedder` trait: `fn embed(&self, row: &GoldRow) -> EmbeddingResult<Embedding>`
- `metric.rs` implements `MetricEmbedder` which converts `GoldRow` (numeric BTreeMap fields) to z-score normalized vectors
- The `Embedder` trait is tightly coupled to `GoldRow` (numeric data with BTreeMap<String, Option<f64>>)

Text embedding operates on string data, not numeric GoldRow fields. Extending `Embedder` to handle text would violate the Single Responsibility Principle and pollute the numeric embedding path with text-specific concerns.

### Decision

Create a new `TextEmbedder` trait alongside the existing `Embedder` trait in `crates/ndp-lib/src/gold/embeddings/text.rs`:

```rust
use anyhow::Result;

/// Model-agnostic trait for converting text into vector embeddings.
///
/// Unlike `Embedder` (which operates on `GoldRow` numeric data), `TextEmbedder`
/// operates directly on string slices. This separation keeps metric and text
/// embedding paths independent.
pub trait TextEmbedder: Send + Sync {
    /// Embed one or more texts into vector space.
    ///
    /// Returns one Vec<f32> per input text, all with `self.dimensions()` length.
    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;

    /// Number of dimensions in the output vectors.
    fn dimensions(&self) -> usize;
}
```

The trait lives in the same `embeddings` module for discoverability but is a separate type. Uses `anyhow::Result` rather than a custom error enum to avoid over-engineering error types that ONNX and future backends would need to wrap anyway.

### Consequences

- **Positive**: Clean separation -- metric embeddings (Embedder/GoldRow) and text embeddings (TextEmbedder/&str) are independent paths
- **Positive**: The trait is simple enough that future backends (tract, candle, static vocabulary) can implement it without friction
- **Positive**: Batch API (`&[&str]`) enables efficient inference for models that benefit from batching
- **Negative**: Two embedding traits in the same module may confuse developers; clear doc comments and the naming convention (Embedder vs TextEmbedder) mitigate this
- **Tradeoff**: Using `anyhow::Result` trades compile-time error exhaustiveness for implementation simplicity

## ADR-002: OnnxEmbedder Implementation

### Context

The `ort` crate (Rust bindings for ONNX Runtime) is the standard choice for ONNX inference in Rust. It supports:
- ARM64 via pre-built ONNX Runtime libraries
- CPU execution providers (no GPU needed for Pi)
- Dynamic input shapes (needed for variable-length text)
- INT8 quantized models

The `tokenizers` crate from HuggingFace handles tokenization for transformer models. Most 384D embedding models (BGE-small, GTE-small, E5-small) use WordPiece or SentencePiece tokenizers with model-specific vocabulary files stored alongside the ONNX model.

### Decision

Implement `OnnxEmbedder` in `crates/ndp-lib/src/gold/embeddings/onnx.rs`:

```rust
use ort::{Session, SessionBuilder, Value as OrtValue};
use tokenizers::Tokenizer;

pub struct OnnxEmbedder {
    session: Session,
    tokenizer: Tokenizer,
    dimensions: usize,
    max_length: usize,  // Token limit (256 for MiniLM, 512 for BGE)
}

impl OnnxEmbedder {
    pub fn new(model_path: &Path, tokenizer_path: &Path, dimensions: usize) -> Result<Self> {
        let session = SessionBuilder::new()?
            .with_intra_threads(2)?    // Pi 5 has 4 cores, use 2 for inference
            .commit_from_file(model_path)?;
        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {}", e))?;
        Ok(Self { session, tokenizer, dimensions, max_length: 512 })
    }
}

impl TextEmbedder for OnnxEmbedder {
    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        // 1. Tokenize all texts (with padding/truncation)
        // 2. Build input tensors (input_ids, attention_mask, token_type_ids)
        // 3. Run ONNX session
        // 4. Extract last_hidden_state, apply mean pooling
        // 5. L2-normalize each vector
        // Return Vec<Vec<f32>> with self.dimensions elements each
    }

    fn dimensions(&self) -> usize { self.dimensions }
}
```

Dependencies:
- `ort = { version = "2", default-features = false, features = ["ndarray"] }` -- ONNX Runtime
- `tokenizers = { version = "0.20", default-features = false }` -- HuggingFace tokenizers
- `ndarray = "0.16"` -- array operations for mean pooling

INT8 quantization support: ONNX Runtime handles INT8 models transparently. The same `Session::commit_from_file` loads both FP32 and INT8 models. No code change needed -- model format determines quantization.

### Consequences

- **Positive**: ort is well-maintained, supports ARM64, handles quantization transparently
- **Positive**: tokenizers crate handles all tokenization complexity (vocabulary, special tokens, padding)
- **Positive**: Mean pooling + L2 normalization is the standard approach for sentence embeddings
- **Negative**: ort links to ONNX Runtime C++ library (~20MB binary size increase)
- **Negative**: tokenizers crate is large; consider feature-gating if compile time becomes an issue
- **Risk**: ARM64 support for ort must be validated on Pi 5 -- fallback to `tract` crate if ort fails

## ADR-003: ndp-embedder Container Architecture

### Context

The existing `ndp-intelligence` container (docker/intelligence/Dockerfile, compose entry in deploy/pi/docker-compose.yml) follows a well-established pattern:
- Multi-stage build: rust:1-bookworm builder + debian:bookworm-slim runtime
- Binary from workspace crate `apps/ndp-intelligence-app` (Cargo.toml at `apps/ndp-intelligence-app/`)
- Environment-based config (DATABASE_URL, INTELLIGENCE_DOMAIN, ETCD_ENDPOINTS)
- Compose service with `profiles: [intelligence]`, `depends_on` with health checks
- 256MB memory limit

SCOPE.md D-01 mandates a separate container for text embedding inference to isolate CPU-heavy ONNX workloads from ndp-intelligence's similarity search and prediction cycle.

### Decision

Create a new binary crate and container:

**Crate**: `apps/ndp-embedder/` as a new workspace member
- `Cargo.toml`: depends on `ndp-lib` (for TextEmbedder, OnnxEmbedder, config), `ndp-types`, `tokio`, `deadpool-postgres`, `config-client`
- `src/main.rs`: AppConfig from env, domain config from etcd, model loading, poll loop

**Dockerfile**: `docker/embedder/Dockerfile` following the intelligence pattern:
```dockerfile
FROM rust:1-bookworm AS builder
# ... same pattern as docker/intelligence/Dockerfile ...
RUN cargo build --release -p ndp-embedder

FROM debian:bookworm-slim AS runtime
# Additional: ONNX Runtime shared library if needed
COPY --from=builder /usr/local/bin/ndp-embedder /usr/local/bin/
```

**Compose entry**: added to `deploy/pi/docker-compose.yml`:
```yaml
ndp-embedder:
  build:
    context: ../..
    dockerfile: docker/embedder/Dockerfile
  image: neural-data-platform/ndp-embedder:latest
  container_name: ndp-embedder
  environment:
    - DATABASE_URL=postgresql://postgres:${POSTGRES_PASSWORD}@timescaledb:5432/ndp
    - EMBEDDER_DOMAIN=indoor-air-quality
    - ETCD_ENDPOINTS=http://etcd:2379
    - MODEL_VOLUME_PATH=/models
    - EMBEDDER_POLL_INTERVAL_SECS=1200
  volumes:
    - embedder-models:/models
  depends_on:
    timescaledb: { condition: service_healthy }
    etcd: { condition: service_healthy }
  deploy:
    resources:
      limits:
        memory: 512M
  profiles:
    - intelligence
```

**Volume**: `embedder-models` named volume for model persistence.

### Consequences

- **Positive**: CPU isolation -- ONNX inference does not compete with ndp-intelligence
- **Positive**: Independent restart -- model swaps via container restart without affecting predictions
- **Positive**: Follows established container patterns (Dockerfile, compose, healthcheck)
- **Negative**: Additional container adds ~5MB RSS overhead for the runtime process
- **Negative**: One more service to manage in deploy.sh
- **Tradeoff**: Using the `intelligence` profile means it starts with `docker compose --profile intelligence up -d`

## ADR-004: Model Storage and Loading

### Context

SCOPE.md D-03 specifies volume mount with download-on-first-use. Models are 22-33MB (INT8 quantized). The Pi may have limited bandwidth, so download must be resilient.

### Decision

Model storage layout on the volume:

```
/models/                           # Volume mount point
  bge-small-en-v1.5/              # Model directory (name from config)
    model.onnx                    # ONNX model file
    tokenizer.json                # Tokenizer vocabulary + config
    config.json                   # Model metadata (dimensions, max_length)
```

Model manager in `crates/ndp-lib/src/gold/embeddings/model_manager.rs`:

```rust
pub struct ModelManager {
    volume_path: PathBuf,
}

impl ModelManager {
    /// Resolve model path, downloading if necessary.
    pub async fn ensure_model(&self, model_id: &str) -> Result<ModelPaths> {
        let model_dir = self.volume_path.join(model_id);
        if model_dir.join("model.onnx").exists() {
            return Ok(ModelPaths::from_dir(&model_dir));
        }
        self.download_model(model_id, &model_dir).await?;
        Ok(ModelPaths::from_dir(&model_dir))
    }
}

pub struct ModelPaths {
    pub model: PathBuf,      // model.onnx
    pub tokenizer: PathBuf,  // tokenizer.json
}
```

Download sources: HuggingFace Hub API (`https://huggingface.co/{org}/{model}/resolve/main/{file}`). The download URL is constructed from the `model_id` in domain config. Retries with exponential backoff (3 attempts, 1s/2s/4s).

### Consequences

- **Positive**: Container image stays small -- no models baked in
- **Positive**: Model swap by changing domain config + restarting container
- **Positive**: Multiple domains can use different models on the same volume
- **Negative**: First startup is slow if model needs downloading (~30s on Pi with decent connection)
- **Risk**: Pi without internet cannot download models -- must pre-stage via `docker cp` or volume mount from host

## ADR-005: Gold Text Embeddings Schema

### Context

The existing `gold.metric_embeddings` table (created by fe-004) stores metric vectors:
```sql
-- From crates/ndp-intelligence/src/storage/postgres.rs
CREATE TABLE IF NOT EXISTS gold.metric_embeddings (
    bucket TIMESTAMPTZ NOT NULL,
    domain_id TEXT NOT NULL,
    embedding vector(7) NOT NULL,  -- 7 dimensions for metric embeddings
    metadata JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (domain_id, bucket)
);
```

Text embeddings need a different schema: 384 dimensions (not 7), per-text-field granularity (not per-domain-bucket), and provenance columns (source_stream, source_column, source_text, model_id).

### Decision

Create `gold.text_embeddings` table via an init-script (following ops-008 pattern) rather than ndp-gold-ddl generator. Rationale: the table is global (not per-domain), similar to `gold.events` -- init-scripts handle global tables, generators handle per-domain DDL.

Init-script: `deploy/pi/init-scripts/004-text-embeddings.sql`

```sql
-- fe-005: Text embedding storage
CREATE TABLE IF NOT EXISTS gold.text_embeddings (
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

SELECT create_hypertable('gold.text_embeddings', 'bucket',
    chunk_time_interval => INTERVAL '7 days',
    if_not_exists => TRUE
);

CREATE INDEX IF NOT EXISTS idx_text_embeddings_hnsw
    ON gold.text_embeddings
    USING hnsw (embedding vector_cosine_ops)
    WITH (m = 16, ef_construction = 64);

CREATE INDEX IF NOT EXISTS idx_text_embeddings_domain_bucket
    ON gold.text_embeddings (domain_id, bucket DESC);
```

HNSW parameters: `m=16, ef_construction=64` matches the metric_embeddings pattern and is appropriate for the expected data volume (1-4 embeddings per hour per domain).

### Consequences

- **Positive**: Init-script runs once at database bootstrap -- no generator complexity
- **Positive**: HNSW index enables efficient similarity search for fe-006
- **Positive**: Hypertable with 7-day chunks enables efficient time-range queries and retention
- **Positive**: `source_text` column enables debugging and provenance tracking
- **Negative**: 384D vectors are larger than 7D metric vectors -- ~1.5KB per row vs ~28B
- **Tradeoff**: `retention_tier` is nullable and unpopulated -- fe-006 will implement tiered lifecycle

## ADR-006: dp-023 Interface Contract

### Context

dp-023 creates a Gold text view per domain. From dp-023's IMPLEMENTATION-BRIEF.md:
- `TextViewGenerator` in `crates/ndp-lib/src/gold/generators/text_view.rs` creates per-domain VIEWs
- View name pattern: `gold.{domain_id}_text_latest` (with hyphens replaced by underscores)
- Schema: unpivoted rows with `bucket`, `stream_id`, `column_name`, `text_value`

fe-005 needs to read these views to get text data for embedding.

### Decision

fe-005 reads from `gold.{domain_id}_text_latest` using timer-based polling (same pattern as ndp-intelligence reading from `gold.{domain_id}_aligned`).

Query pattern:
```sql
SELECT bucket, stream_id, column_name, text_value
FROM gold.indoor_air_quality_text_latest
WHERE bucket > $1
ORDER BY bucket ASC
LIMIT 100
```

The `$1` parameter is the last processed bucket timestamp, tracked in memory (same as IntelligenceService.last_processed).

**Graceful degradation**: If the Gold text view does not exist (dp-023 not yet implemented), the embedder logs a warning and retries on the next poll cycle:
```rust
match client.query(&query, &[&last_bucket]).await {
    Ok(rows) => process_rows(rows),
    Err(e) if is_relation_not_found(&e) => {
        warn!("Gold text view not found (dp-023 not implemented?): {}", e);
        // Will retry on next poll cycle
    }
    Err(e) => return Err(e.into()),
}
```

### Consequences

- **Positive**: Clean interface -- fe-005 only depends on the view schema, not dp-023 internals
- **Positive**: Timer-based polling is proven (ndp-intelligence uses it successfully)
- **Positive**: Graceful degradation allows deploying fe-005 before dp-023 without crashes
- **Negative**: Timer-based polling has up to `poll_interval` latency (default 20 minutes)
- **Future**: PG NOTIFY on text view refresh could reduce latency; deferred to avoid complexity

## ADR-007: Preprocessing Pipeline

### Context

SCOPE.md D-05 specifies domain-configurable preprocessing. NWS forecast text is clean natural English -- no preprocessing needed. Future domains (syslog, structured logs) will need preprocessing (e.g., Drain template extraction).

The research notes in SCOPE.md confirm that classical NLP preprocessing (stopword removal, stemming) hurts transformer embeddings.

### Decision

Implement preprocessing as a trait with factory pattern in `crates/ndp-lib/src/gold/embeddings/preprocessing.rs`:

```rust
pub trait TextPreprocessor: Send + Sync {
    fn preprocess(&self, text: &str) -> String;
    fn name(&self) -> &str;
}

pub struct PassthroughPreprocessor;

impl TextPreprocessor for PassthroughPreprocessor {
    fn preprocess(&self, text: &str) -> String {
        text.to_string()
    }
    fn name(&self) -> &str { "passthrough" }
}

/// Factory function driven by domain config
pub fn create_preprocessor(config: &PreprocessingConfig) -> Box<dyn TextPreprocessor> {
    match config.preprocessing_type.as_str() {
        "passthrough" | "" => Box::new(PassthroughPreprocessor),
        other => {
            warn!("Unknown preprocessing type '{}', falling back to passthrough", other);
            Box::new(PassthroughPreprocessor)
        }
    }
}
```

The factory logs a warning for unknown types rather than failing -- this prevents config typos from crashing the service.

### Consequences

- **Positive**: Plugin architecture -- new preprocessors are one trait implementation + one factory match arm
- **Positive**: Passthrough has zero overhead (just string clone)
- **Positive**: Unknown types degrade gracefully (warning + passthrough) rather than crashing
- **Negative**: Adding a new preprocessor requires code change (match arm) -- not truly pluggable at runtime. Acceptable for the expected use case (2-3 preprocessors total)

## ADR-008: Domain Schema Extension

### Context

The current domain.json schema (`config/schemas/domain.schema.json`) has an `intelligence` property at the root level of `domain_content`. The `intelligence.embedding` sub-object configures metric embedding (fields, temporal, direct, derived).

fe-005 needs a separate `embedding` configuration for text embedding. This is distinct from `intelligence.embedding` because:
1. Different purpose: text model selection vs metric field selection
2. Different schema: model/quantization/dimensions/preprocessing vs type/fields
3. Different consumer: ndp-embedder vs ndp-intelligence

### Decision

Add a new top-level `text_embedding` property to the domain schema (NOT nested under `intelligence`). Using `text_embedding` rather than just `embedding` to avoid confusion with `intelligence.embedding`.

Schema addition to `config/schemas/domain.schema.json`:

```json
{
  "text_embedding": {
    "type": "object",
    "additionalProperties": false,
    "description": "Text embedding configuration for the ndp-embedder service.",
    "required": ["model", "dimensions"],
    "properties": {
      "model": {
        "type": "string",
        "description": "Model identifier (e.g., 'bge-small-en-v1.5')"
      },
      "quantization": {
        "type": "string",
        "enum": ["fp32", "int8"],
        "default": "int8"
      },
      "dimensions": {
        "type": "integer",
        "minimum": 1,
        "maximum": 4096,
        "description": "Output embedding dimensions"
      },
      "preprocessing": {
        "type": "object",
        "additionalProperties": false,
        "properties": {
          "type": {
            "type": "string",
            "enum": ["passthrough"],
            "default": "passthrough"
          }
        }
      }
    }
  }
}
```

This is OPTIONAL at the domain level -- domains without `text_embedding` pay zero cost. The ndp-embedder checks for the presence of this block at startup and exits gracefully if absent.

Rust config type in `crates/ndp-lib/src/gold/embeddings/text_config.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextEmbeddingConfig {
    pub model: String,
    #[serde(default = "default_quantization")]
    pub quantization: String,
    pub dimensions: usize,
    #[serde(default)]
    pub preprocessing: PreprocessingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreprocessingConfig {
    #[serde(rename = "type", default = "default_passthrough")]
    pub preprocessing_type: String,
}
```

### Consequences

- **Positive**: Clean separation from `intelligence.embedding` (metric config)
- **Positive**: Optional property -- backward compatible
- **Positive**: `text_embedding` name is unambiguous
- **Negative**: Adds another top-level property to domain.json -- schema is getting larger
- **Tradeoff**: `additionalProperties: false` enforced for safety, but requires schema update when adding new preprocessing options

## Integration Surface

| Surface | Producer | Consumer | Interface |
|---------|----------|----------|-----------|
| Gold text view | dp-023 (TextViewGenerator) | fe-005 (ndp-embedder) | `gold.{domain}_text_latest` VIEW with (bucket, stream_id, column_name, text_value) |
| Text embedding storage | fe-005 (ndp-embedder) | fe-006 (composite search) | `gold.text_embeddings` table with pgvector HNSW index |
| Domain config: text_embedding | User/deployment | fe-005 (ndp-embedder) | `text_embedding` block in domain.json via etcd |
| Domain config: intelligence | User/deployment | fe-004 (ndp-intelligence) | `intelligence` block in domain.json via etcd (UNCHANGED) |
| ONNX model files | HuggingFace / pre-staged | fe-005 (model manager) | Volume mount at `/models/{model_id}/` |
| pgvector extension | fe-004 init-script | fe-005 DDL | `CREATE EXTENSION IF NOT EXISTS vector` (already exists) |
| TimescaleDB | Database | fe-005 (storage write) | `gold.text_embeddings` hypertable insert |
| Domain JSON schema | Config schema | ndp-cli validation | `config/schemas/domain.schema.json` with `text_embedding` definition |
