# fe-005: Text Embeddings (Text Pipeline)

## Vision

Add text embedding capability to the intelligence layer. NWS forecast discussions contain direct air quality predictions in natural language ("stagnation advisory in effect," "gusty winds will transport smoke"). Embedding these alongside sensor metrics enables forecast-aware similarity search — distinguishing "high PM2.5 with stagnation" (getting worse) from "high PM2.5 with incoming front" (about to improve).

This feature delivers a model-agnostic text embedding service: a separate `ndp-embedder` container running ONNX inference, domain-configurable model selection and preprocessing, volume-mounted model storage, and text embedding persistence in pgvector. Text goes in, vectors come out. Composite search (combining metric + text) is fe-006.

## Tracking

- Feature: fe-005
- GitHub Issue: https://github.com/dug-21/neural-data-platform/issues/39
- Parent roadmap: `product/features/gold-001/FEATURE-ROADMAPv1.2.md` (Track C: v12-E01 through E04, v12-I08)
- Predecessor: fe-004 (similarity intelligence, deployed v1.2.0-v1.2.6)
- Dependency: dp-023 (text field pipeline, Bronze→Silver→Gold text view) — must complete before fe-005 implementation
- Version target: v1.2.x

## What fe-004 Delivered (Prerequisites)

- `crates/ndp-intelligence/` — running daemon with PgVectorEngine, K-NN search, predictions, outcome tracking
- `crates/ndp-lib/src/gold/embeddings/` — Embedder trait, MetricEmbedder, EmbeddingConfig
- `gold.metric_embeddings` pgvector table with HNSW index
- PG NOTIFY + timer wake cycle
- Docker container deployed on Pi (256MB limit currently)

## What dp-023 Delivers (Prerequisite)

- Bronze→Silver→Gold text pipeline for NWS forecast text
- Gold text view (`gold.{domain}_text_latest` or similar) that fe-005 reads from
- Text fields flowing through the existing stream pipeline — no separate ingest needed

## Decisions

Resolved during scoping (2026-02-17).

### D-01: Separate container for inference

**Decision**: Text embedding inference runs in a dedicated `ndp-embedder` container, not inside `ndp-intelligence`.

**Rationale**: CPU isolation — ONNX inference is compute-heavy and would compete with the intelligence daemon's similarity search, prediction, and outcome tracking workloads. Separate container also enables model swaps via restart without risk to the running intelligence cycle.

- Container limit: 512MB
- `ndp-intelligence` stays at 256MB, unchanged

### D-02: Multi-model architecture

**Decision**: Design for model flexibility. The embedding trait is model-agnostic; fe-005 ships only an `OnnxEmbedder` implementation backed by `ort` (Rust ONNX Runtime). Model selection is deferred to implementation time.

**Rationale**: Different domains may demand different models long-term. The 33M/384D tier (BGE-small, snowflake-arctic-s) is the quality sweet spot, but the architecture must not be locked to any single model. INT8 quantization must be supported.

**Trait signature** (model-agnostic, no ONNX in the interface):
```rust
pub trait TextEmbedder: Send + Sync {
    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;
    fn dimensions(&self) -> usize;
}
```

fe-005 delivers `OnnxEmbedder` as the sole implementation. Static/non-transformer embedders (vocabulary lookup models) are out of scope but the trait does not preclude them.

### D-03: Model loading — volume mount with download-on-first-use

**Decision**: Models stored on a Docker volume mount. On startup, if the configured model is not present on the volume, the embedder downloads it. If present, loads directly.

**Rationale**: Keeps container image small. Models can be added to the volume through deployment (pre-staging) or config change (auto-download). No image rebuild required to switch models.

### D-04: Per-domain embedding config

**Decision**: Embedding configuration lives per-domain in `domain.json`, not per-stream.

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

**Rationale**: Text embedding operates at the domain level (Gold layer). Stream configs drive Bronze→Silver; domain config drives Gold→Intelligence.

### D-05: Domain-configurable preprocessing

**Decision**: fe-005 owns the preprocessing pipeline before embedding. Preprocessing is configured per-domain (see D-04). For NWS weather, preprocessing is `passthrough` (no transformation). The pipeline slot must exist architecturally for future domain-specific preprocessors (e.g., Drain-based template extraction for syslog).

**Rationale**: dp-023 is plumbing (data pipeline). fe-005 understands domain-specific text preparation before embedding. Different domains will need different preprocessing strategies.

### D-06: Retention tier column — present, unpopulated

**Decision**: `gold.text_embeddings` includes a `retention_tier` column. fe-005 does not populate it. fe-006 will implement tiered retention lifecycle.

## Deliverables

| ID | Task | Description |
|----|------|-------------|
| E-01 | TextEmbedder trait + OnnxEmbedder | Model-agnostic `TextEmbedder` trait. `OnnxEmbedder` implementation using `ort` for ONNX inference. Batch support. INT8 quantization support. |
| E-02 | Model management | Volume mount model storage. Download-on-first-use. Model path resolution from domain config. Support for multiple models on the same volume. |
| ~~E-03~~ | ~~Template cache~~ | **DEFERRED** — Research indicates removing/templating weather forecast data hurts embedding quality. Revisit when a domain benefits from it. |
| ~~E-04~~ | ~~NWS forecast event stream~~ | **DROPPED** — dp-023 handles text ingest through the normal stream pipeline (Bronze → Silver → Gold text view). No separate event stream needed. |
| E-05 | Text embedding storage | `gold.text_embeddings` pgvector table (384D, per-text, with `retention_tier` column — unpopulated). HNSW index. |
| ~~E-06~~ | ~~Text feature extraction DDL~~ | **DEFERRED** — Extracted text features (severity, keywords) are not needed for initial embedding. Revisit when downstream consumers require structured text features. |
| E-07 | ndp-embedder container | New Docker container. 512MB limit. Volume mount for ONNX models. Reads Gold text view, writes to `gold.text_embeddings`. Domain config driven. |
| E-08 | Preprocessing pipeline | Domain-configurable preprocessing stage before embedding. `passthrough` implementation for NWS. Trait/plugin architecture for future preprocessors. |
| E-09 | Domain schema update | Add `embedding` block to domain.json schema. Model, quantization, dimensions, preprocessing config. |

## Constraints

- Model loaded on demand only when domain config includes an `embedding` block — domains without text pay zero cost
- Must NOT break existing metric-only intelligence cycle (`ndp-intelligence` is untouched)
- Preprocessing strategy is domain-configurable, not hardcoded
- INT8 quantized models must be supported
- Pi 5: inference must complete within budget (~500ms cold start acceptable)
- Volume mount model storage — no models baked into container image

## Acceptance Criteria

| Criterion | Target |
|-----------|--------|
| TextEmbedder trait is model-agnostic | No ONNX types in trait signature |
| OnnxEmbedder produces vectors | 384D Vec<f32> from text input |
| Model loads from volume | Downloads on first use if absent, loads from volume if present |
| Domain config drives model selection | Model, quantization, dimensions read from domain.json `embedding` block |
| Preprocessing pipeline exists | Passthrough impl for NWS, trait slot for future preprocessors |
| Text embeddings stored | Text embedded in `gold.text_embeddings` with HNSW index |
| Retention tier column present | Column exists, not populated (fe-006) |
| Inference latency (cold) | <500ms on Pi 5 |
| ndp-embedder container runs | Separate from ndp-intelligence, 512MB limit |
| Existing metric cycle unaffected | ndp-intelligence predictions continue generating normally |

## Terminology Change (from dp-023 scoping)

**"Event" renamed to "Text" throughout.** The research literature uses "event" for any discrete text occurrence in a time series (forecast update, log message). NDP uses "event" for detected patterns — outputs of detection procedures in `gold.events` (threshold breaches, anomalies). These are different concepts:

- **NDP events** (`gold.events`): outputs of detection procedures. Schema: `event_type`, `severity`, `entity_id`.
- **Text observations**: raw data flowing through the pipeline. A forecast arriving every hour is a data point, not a detected event.

Conflating them would route input data through an output system. Instead:

- `EventEmbedder` → **`TextEmbedder`**
- `gold.event_embeddings` → **`gold.text_embeddings`**
- E-04 (NWS forecast event stream) → **DROPPED** — dp-023 handles text ingest through the normal stream pipeline.

## Research Notes

Retained from scoping for future reference. These informed the decisions above but do not represent open questions.

### Model landscape (2025 survey)

The 33M-parameter, 384-dimension tier is the sweet spot for quality-per-FLOP: BGE-small-en-v1.5, snowflake-arctic-embed-s, E5-small-v2, and GTE-small. BGE-small and snowflake-arctic-s lead on retrieval benchmarks (51.7 and 52.0 nDCG@10). The 22M tier (MiniLM, arctic-embed-xs) trades ~10 retrieval points for 30-50% faster inference.

| Model | Params | Dims | ONNX INT8 Size | MTEB Avg | Retrieval nDCG@10 |
|-------|--------|------|----------------|----------|-------------------|
| TaylorAI/bge-micro-v2 | 17M | 384 | ~40 MB | ~55 | ~45 |
| TaylorAI/gte-tiny | 17M | 384 | ~40 MB | ~58 | ~47 |
| all-MiniLM-L6-v2 | 22M | 384 | ~22 MB | ~56 | ~41 |
| snowflake-arctic-embed-xs | 22M | 384 | ~22 MB | — | 50.2 |
| BAAI/bge-small-en-v1.5 | 33M | 384 | ~33 MB | 62.2 | 51.7 |
| snowflake-arctic-embed-s | 33M | 384 | ~33 MB | — | 52.0 |
| intfloat/e5-small-v2 | 33M | 384 | ~33 MB | 59.9 | 49.0 |
| thenlper/gte-small | 33M | 384 | ~33 MB | 61.4 | ~50 |
| nomic-embed-text-v1.5 | 137M | 768* | ~130 MB | 62.3 | ~53 |

*Nomic supports Matryoshka dimensions: 64, 128, 256, 512, 768 — allowing post-deployment dimension reduction.

Static (non-transformer) alternatives exist for extreme throughput (100-400x speed at ~87% quality) but are out of scope for fe-005.

### Text preprocessing research

Classical NLP preprocessing (stopword removal, stemming, lemmatization) **hurts** transformer embeddings (Ferraro et al. 2023, Hidayatullah 2022). These models need full sentence context. Domain-specific structuring provides gains. NWS forecast text is clean natural English — minimal preprocessing needed. Future syslog domains would need Drain-based template extraction (recommend `drain-rs` crate when that domain arrives).

### Numeric values in text

Embedding models are nearly blind to numeric magnitude ("2% annually" and "20% annually" yield 0.97 cosine similarity). Not relevant for fe-005 — NDP architecture already separates numeric values (Silver columns) from text (embedding). The forecast narrative contains qualitative language ("Excessive Heat Warning") that carries the semantic signal.

### Token limits and chunking

MiniLM truncates at 256 tokens. NWS `detailedForecast` is typically 40-60 tokens (safe). Chunking deferred — model selection may address this (some models support 512 tokens). Revisit when longer text sources are added.

### Delta embeddings

Computing `delta[t] = embedding[t] - embedding[t-1]` captures semantic change. Validated by Delta-IP Insight framework (23% detection latency improvement). Deferred to fe-006 — requires composite search infrastructure.

### Dimensionality reduction

384D → 32-64D via PCA retains 90-95% variance. Matryoshka models (nomic, snowflake-arctic) support native truncation without PCA. Deferred to fe-006 — store raw 384D for now.

### Temporal smoothing (EWMA)

EWMA of embedding vectors tracks semantic trends, reduces noise. Deferred to fe-006 — temporal analysis is composite/sequence territory.

### Where text embeddings provide the most value

Text features provide >60% MSE improvement for non-periodic, event-driven patterns (TGForecaster). This validates the core hypothesis — NWS forecast text captures weather regime shifts that PM2.5 readings alone can't predict. Value is highest for event-driven prediction, not regular diurnal/seasonal patterns.

### Rust Drain implementations (future syslog reference)

| Crate | Version | Notes |
|-------|---------|-------|
| **drain-rs** | 0.3.0 | Most mature. Serde, GROK patterns. Apache-2.0. Recommended for syslog domain. |
| drainrs | 0.1.0 | Newer, less mature. Parse tree persistence. |
| logu | — | CLI/library, Grafana Loki approach. |

## Out of Scope

- Model selection (decided at implementation time, not scoping)
- Static/non-transformer embedding models (vocabulary lookup)
- Template caching (E-03 deferred)
- Text feature extraction DDL (E-06 deferred)
- Token chunking (deferred, model choice may solve)
- Delta embeddings (fe-006)
- PCA / dimensionality reduction (fe-006, Matryoshka may eliminate)
- EWMA temporal smoothing (fe-006)
- Numeric bucketing (not relevant — numeric pipeline handles it)
- CompositeEmbedder (fe-006)
- Quantization / tiered retention lifecycle (fe-006)
- Granger causality (fe-007)
- Anomaly detection / dashboards (fe-008)
- SONA learning (fe-009)

## Release

v1.2.x — Text embedding service. Multi-model ONNX on Pi. NWS forecasts embedded. Blocked on dp-023.
