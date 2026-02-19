# fe-005: Alignment Report

## Assessment Date: 2026-02-17

## Overall: PASS

All 7 alignment principles satisfied. Two items flagged as WARN for awareness.

## Principle-by-Principle Assessment

### 1. Edge-Only: PASS

All processing runs on-device (Pi 5). ONNX inference via `ort` crate runs locally. No cloud dependencies for core functionality.

**Note**: Model download-on-first-use requires internet for first startup only. After download, models are persisted on volume and the service runs fully offline. This is analogous to `docker pull` requiring internet -- acceptable for initial setup.

### 2. Config-Driven: PASS

All behavior controlled via declarative JSON configuration:
- Model selection: `text_embedding.model` in domain.json
- Quantization: `text_embedding.quantization`
- Dimensions: `text_embedding.dimensions`
- Preprocessing: `text_embedding.preprocessing.type`
- Poll interval: `EMBEDDER_POLL_INTERVAL_SECS` environment variable
- No hardcoded values for model paths, thresholds, or intervals

### 3. Domain-Portable: PASS

Text embedding is fully domain-portable:
- Per-domain configuration in domain.json
- TextEmbedder trait is model-agnostic
- Preprocessing is domain-configurable (passthrough for NWS, future Drain for syslog)
- Different domains can use different models and preprocessing strategies
- Zero cost for domains without `text_embedding` config

### 4. Resource-Constrained: PASS

- Container limit: 512MB (within Pi 5 budget)
- Model size: ~33MB INT8 (fits within 512MB with runtime)
- CPU: 2 intra-op threads for inference (leaves 2 cores for other services)
- ONNX Runtime supports ARM64 (aarch64-unknown-linux-gnu)
- Inference target: <500ms on Pi 5

### 5. Integration-First: PASS

fe-005 extends the existing intelligence infrastructure:
- TextEmbedder trait lives alongside existing Embedder trait in `crates/ndp-lib/src/gold/embeddings/`
- gold.text_embeddings follows gold.metric_embeddings pattern
- ndp-embedder container follows ndp-intelligence container pattern
- Domain schema extends existing domain.json (new optional property)
- Uses existing pgvector extension, existing TimescaleDB, existing etcd config loading

### 6. Privacy by Architecture: PASS

- All data stays on-device
- ONNX inference is local (no API calls to cloud embedding services)
- No telemetry, no phone-home
- Model download is the only network activity (first-use only)

### 7. Self-Learning: WARN

fe-005 provides the embedding infrastructure but does not itself implement learning. The embeddings enable fe-006 (composite similarity search) and fe-007 (Granger causality) which ARE self-learning features. fe-005 is an enabler, not a direct self-learning component.

**Classification**: WARN -- infrastructure feature that enables self-learning without being self-learning itself. Acceptable because the feature roadmap shows the learning chain: fe-005 (embeddings) -> fe-006 (composite search) -> fe-007 (causality) -> fe-009 (SONA learning).

## Version Target Check

- Target: v1.2.x (PLANNED per roadmap)
- Predecessor: fe-004 (v1.2.0-v1.2.6, DEPLOYED)
- Dependency: dp-023 (v1.2.x, planning complete)
- Aligned with roadmap Track C (v12-E01 through E04)

## Scope Alignment

### Deliverables vs SCOPE.md

| SCOPE.md Deliverable | Specification Coverage | Status |
|---------------------|----------------------|--------|
| E-01: TextEmbedder + OnnxEmbedder | FR-01, FR-02 | Covered |
| E-02: Model management | FR-03 | Covered |
| E-05: gold.text_embeddings | FR-04 | Covered |
| E-07: ndp-embedder container | FR-05 | Covered |
| E-08: Preprocessing pipeline | FR-06 | Covered |
| E-09: Domain schema update | FR-07 | Covered |
| E-03: Template cache | DEFERRED (matches SCOPE.md) | N/A |
| E-06: Text feature extraction | DEFERRED (matches SCOPE.md) | N/A |

No scope gaps. No out-of-scope additions.

## Variances Requiring User Approval

### WARN-001: Model Download Requires Internet (First Use Only)

D-03 specifies download-on-first-use. This means the first startup of ndp-embedder on a fresh Pi requires internet access to download the ONNX model (~33MB). After download, the model persists on the Docker volume.

**Mitigation**: Models can be pre-staged on the volume before first startup (`docker cp` or volume mount from host directory). This is documented in ADR-004.

**User decision**: Accept download-on-first-use, or require pre-staging documentation?

### WARN-002: ort Crate ARM64 Compatibility Unverified

The `ort` crate (ONNX Runtime Rust bindings) claims ARM64 support, but this has not been validated on Pi 5 with kernel 6.14+. Fallback: `tract` crate (pure Rust, no C++ dependency) at the cost of ~2x inference latency.

**User decision**: Accept ort as primary with tract as documented fallback?

## Technical Constraints Compliance

| Constraint | Status |
|------------|--------|
| ARM64 | ort supports aarch64 (to be validated) |
| No DuckDB | Not used |
| No Polars | Not used |
| TimescaleDB for Silver/Gold | gold.text_embeddings uses TimescaleDB hypertable |
| Config-driven JSON | domain.json text_embedding block |
| Docker on Pi | ndp-embedder container with 512MB limit |
| Bronze->Silver->Gold data flow | Reads from Gold text view (dp-023 output) |
