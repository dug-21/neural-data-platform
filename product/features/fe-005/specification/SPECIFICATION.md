# fe-005: Text Embeddings -- Specification

## Overview

fe-005 delivers a model-agnostic text embedding service as a new `ndp-embedder` container, separate from `ndp-intelligence`. The service reads text data from Gold text views (produced by dp-023), runs ONNX inference to generate 384-dimensional vector embeddings, and stores them in `gold.text_embeddings` for downstream similarity search (fe-006).

The design follows the existing intelligence layer pattern (fe-004) but targets text data rather than metrics. The TextEmbedder trait is model-agnostic (no ONNX types leak into the interface), with `OnnxEmbedder` as the initial implementation using the `ort` crate for Rust ONNX Runtime. Models are stored on a Docker volume mount with download-on-first-use semantics.

## Functional Requirements

### FR-01: TextEmbedder Trait (E-01)

A model-agnostic trait for converting text into vector embeddings.

```rust
pub trait TextEmbedder: Send + Sync {
    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;
    fn dimensions(&self) -> usize;
}
```

Requirements:
- The trait signature contains no ONNX-specific types
- Batch support: accepts multiple texts, returns one vector per text
- Thread-safe (`Send + Sync`) for shared usage behind `Arc`
- Object-safe for trait object usage (`Box<dyn TextEmbedder>`)
- Returns `Vec<f32>` vectors of fixed dimensionality
- Error type covers model load failure, inference failure, dimension mismatch

### FR-02: OnnxEmbedder Implementation (E-01)

The initial `TextEmbedder` implementation using `ort` for ONNX Runtime inference.

Requirements:
- Loads ONNX model file from a configurable path
- Runs tokenization (via `tokenizers` crate or embedded vocabulary)
- Supports batch inference (multiple texts in one forward pass)
- Supports INT8 quantized models
- Produces 384-dimensional `Vec<f32>` output (configurable via domain config)
- Handles model load errors gracefully (returns error, does not panic)
- ARM64 (aarch64-unknown-linux-gnu) compatible -- ort supports ARM64

### FR-03: Model Management (E-02)

Volume-mounted model storage with download-on-first-use.

Requirements:
- Models stored on a Docker volume mount at a configurable path
- On startup, checks if the configured model exists on the volume
- If absent, downloads the model (ONNX format) from a configurable URL or HuggingFace
- If present, loads directly from volume
- Supports multiple models on the same volume (subdirectories per model)
- Model path resolution uses domain config `embedding.model` field
- Download progress logging
- No models baked into the container image

### FR-04: Text Embedding Storage (E-05)

Persistent storage for text embeddings in pgvector.

Table: `gold.text_embeddings`

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

Requirements:
- pgvector extension (already installed from fe-004)
- HNSW index on the embedding column (matching `gold.metric_embeddings` pattern)
- `retention_tier` column present but nullable, not populated by fe-005
- `source_stream` and `source_column` identify which Gold text view row the embedding came from
- `source_text` stores the original text for debugging and provenance
- `model_id` records which model produced the embedding
- Partitioned by time (hypertable via TimescaleDB `create_hypertable`)
- DDL generated through `ndp-gold-ddl` or init-script pattern (ADR to decide)

### FR-05: ndp-embedder Container (E-07)

A new Docker container running the text embedding service.

Requirements:
- Separate binary crate (new workspace member)
- Separate Dockerfile following `docker/intelligence/Dockerfile` pattern
- Compose entry in `deploy/pi/docker-compose.yml` with `intelligence` profile
- 512MB memory limit
- Volume mount for ONNX models
- Depends on: timescaledb (service_healthy), etcd (service_healthy)
- Environment variables: `DATABASE_URL`, `EMBEDDER_DOMAIN`, `ETCD_ENDPOINTS`, `MODEL_VOLUME_PATH`, `EMBEDDER_POLL_INTERVAL_SECS`
- Healthcheck via curl or process check
- Startup sequence:
  1. Read domain config from etcd
  2. Check if `embedding` block exists in domain config
  3. If no embedding config, log and exit gracefully
  4. Resolve model path from config
  5. Check model on volume; download if absent
  6. Load ONNX model into memory
  7. Enter poll loop: query Gold text view for new rows, embed, store

### FR-06: Preprocessing Pipeline (E-08)

Domain-configurable preprocessing stage before embedding.

```rust
pub trait TextPreprocessor: Send + Sync {
    fn preprocess(&self, text: &str) -> String;
}
```

Requirements:
- `PassthroughPreprocessor`: returns text unchanged (for NWS weather)
- Preprocessing type selected from domain config `embedding.preprocessing.type`
- Plugin architecture: future preprocessors (e.g., Drain for syslog) implement the trait
- Preprocessing runs before tokenization/embedding
- No preprocessing is applied if `preprocessing.type` is "passthrough" or omitted

### FR-07: Domain Schema Update (E-09)

Add `embedding` block to domain.json schema for text embedding configuration.

```json
{
  "embedding": {
    "model": "bge-small-en-v1.5",
    "quantization": "int8",
    "dimensions": 384,
    "preprocessing": {
      "type": "passthrough"
    }
  }
}
```

Requirements:
- New `embedding` property in `domain_content` definition (separate from existing `intelligence.embedding`)
- Optional: domains without an `embedding` block pay zero cost
- Backward compatible: existing domain configs remain valid
- JSON Schema definition added to `config/schemas/domain.schema.json`
- Schema distinguishes between `intelligence.embedding` (metric embedding config) and top-level `embedding` (text embedding config)

## Interface Contract with dp-023

fe-005 reads from the Gold text view that dp-023 creates. The interface:

- **View name pattern**: `gold.{domain_id}_text_latest` (domain_id with hyphens replaced by underscores)
- **Expected columns**: `bucket` (TIMESTAMPTZ), `stream_id` (TEXT), `column_name` (TEXT), `text_value` (TEXT), plus optional metadata columns
- **Wake mechanism**: Timer-based polling (same pattern as ndp-intelligence) -- NOT PG NOTIFY initially
- **Dependency**: dp-023 must be implemented before fe-005 can run. If the Gold text view does not exist, ndp-embedder logs a warning and retries on the next poll cycle

## Non-Functional Requirements

| Requirement | Target |
|-------------|--------|
| ARM64 compatibility | All dependencies compile for aarch64-unknown-linux-gnu |
| Inference latency (cold) | <500ms per text on Pi 5 |
| Memory footprint | <512MB including loaded model |
| Config-driven | All behavior from domain.json, no hardcoded values |
| Edge-only | No cloud dependencies for core functionality |
| Model agnostic | TextEmbedder trait has no ONNX types |
| Backward compatible | Existing ndp-intelligence unmodified |
| Zero-cost for non-text domains | No model loaded if no `embedding` block in domain config |

## Not In Scope

- Model selection (decided at implementation time, not specification)
- Static/non-transformer embedding models
- Template caching (E-03 deferred)
- Text feature extraction DDL (E-06 deferred)
- Token chunking (deferred)
- Delta embeddings (fe-006)
- PCA / dimensionality reduction (fe-006)
- EWMA temporal smoothing (fe-006)
- CompositeEmbedder combining metric + text (fe-006)
- Quantization / tiered retention lifecycle (fe-006)
- Granger causality (fe-007)
- Anomaly detection / dashboards (fe-008)
- SONA learning (fe-009)

## Terminology

- **TextEmbedder**: trait for text-to-vector conversion (new in fe-005)
- **Embedder**: existing trait for metric-to-vector conversion (fe-004, in ndp-lib)
- **MetricEmbedder**: existing implementation of Embedder (fe-004)
- **OnnxEmbedder**: new implementation of TextEmbedder using ONNX Runtime
- **gold.text_embeddings**: new table for text vectors (parallel to gold.metric_embeddings)
- **gold.metric_embeddings**: existing table for metric vectors (fe-004)
- **ndp-embedder**: new container running text embedding service
- **ndp-intelligence**: existing container running metric embedding + similarity + prediction
